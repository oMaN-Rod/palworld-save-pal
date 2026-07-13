//! `PalDto` covers both directions of the wire: what the frontend sends for
//! edits/clones (a strict subset) and what the server sends back in
//! player/base details responses. Field declaration order is a wire
//! contract — `serde` serializes in declaration order and the frontend
//! consumes this JSON as-is over the WebSocket.
use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::ordered_map::OrderedMap;

/// Wire values are the bare variant names; save data carries them with an
/// `EPalGenderType::` prefix (see [`PalGender::from_prefixed`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PalGender {
    None,
    Male,
    Female,
}

impl PalGender {
    /// Strips the `EPalGenderType::` prefix save data carries. Anything
    /// unrecognized falls back to `Female` rather than erroring.
    pub fn from_prefixed(value: &str) -> PalGender {
        match value.trim_start_matches("EPalGenderType::") {
            "None" => PalGender::None,
            "Male" => PalGender::Male,
            _ => PalGender::Female,
        }
    }

    /// Re-adds the `EPalGenderType::` prefix for writing back into save data.
    pub fn prefixed(&self) -> String {
        let bare = match self {
            PalGender::None => "None",
            PalGender::Male => "Male",
            PalGender::Female => "Female",
        };
        format!("EPalGenderType::{bare}")
    }
}

/// Wire keys of `work_suitability` maps. Order is significant: it is the key
/// order the frontend expects to receive them in.
pub const WORK_SUITABILITIES: [&str; 13] = [
    "EmitFlame",
    "Watering",
    "Seeding",
    "GenerateElectricity",
    "Handcraft",
    "Collection",
    "Deforest",
    "Mining",
    "OilExtraction",
    "ProductMedicine",
    "Cool",
    "Transport",
    "MonsterFarm",
];

/// Normalizes a raw save-data character id into the lowercase pal-database
/// key. `known_pal_keys` guards the `boss_` strip: an id that is itself a
/// known key keeps its prefix.
pub fn format_character_key(character_id: &str, known_pal_keys: &HashSet<String>) -> String {
    let lowered = character_id.to_lowercase();
    if !known_pal_keys.contains(character_id) {
        if let Some(stripped) = lowered.strip_prefix("boss_") {
            return stripped.to_string();
        }
    }
    if let Some(stripped) = lowered.strip_prefix("predator_") {
        stripped.to_string()
    } else if let Some(stripped) = lowered.strip_suffix("_avatar") {
        stripped.to_string()
    } else {
        lowered
    }
}

/// Output-only fields (`character_key`, `is_predator`, `filtered_nickname`)
/// are `#[serde(default)]` so frontend edit/clone payloads, which never send
/// them, still deserialize.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PalDto {
    pub instance_id: uuid::Uuid,
    pub character_id: String,
    #[serde(default)]
    pub character_key: String, // output-only
    pub owner_uid: Option<uuid::Uuid>,
    pub is_lucky: Option<bool>,
    pub is_boss: Option<bool>,
    #[serde(default)]
    pub is_predator: bool, // output-only
    pub is_tower: bool,
    pub gender: PalGender,
    pub nickname: Option<String>,
    #[serde(default)]
    pub filtered_nickname: Option<String>, // output-only, DPS pals only
    pub group_id: Option<uuid::Uuid>,
    pub stomach: f64,
    pub sanity: f64,
    pub hp: i64,
    pub level: i64,
    pub exp: i64,
    pub rank: i64,
    pub rank_hp: i64,
    pub rank_attack: i64,
    pub rank_defense: i64,
    pub rank_craftspeed: i64,
    pub talent_hp: i64,
    pub talent_shot: i64,
    pub talent_defense: i64,
    pub max_hp: i64,
    pub storage_slot: i64,
    pub storage_id: uuid::Uuid,
    pub learned_skills: Vec<String>,
    pub active_skills: Vec<String>,
    pub passive_skills: Vec<String>,
    pub work_suitability: OrderedMap<String, i64>,
    pub is_sick: bool,
    pub friendship_point: i64,
}

impl PalDto {
    /// Tolerant construction from arbitrary JSON: unknown keys are ignored and
    /// null/missing values fall back to defaults. Errors only if `character_id`
    /// is not a string.
    pub fn from_json_lenient(value: &serde_json::Value) -> Result<Self, crate::error::CoreError> {
        let source = value
            .as_object()
            .ok_or_else(|| crate::error::CoreError::Other("pal_data is not an object".into()))?;
        let mut normalized = serde_json::Map::new();
        let defaults = serde_json::json!({
            "instance_id": "00000000-0000-0000-0000-000000000000",
            "owner_uid": null,
            "character_id": "",
            "is_lucky": null,
            "is_boss": null,
            "gender": "Male",
            "rank_hp": 0, "rank_attack": 0, "rank_defense": 0, "rank_craftspeed": 0,
            "talent_hp": 0, "talent_shot": 0, "talent_defense": 0,
            "rank": 0, "level": 1, "exp": 0,
            "nickname": null,
            "is_tower": false,
            "storage_id": "00000000-0000-0000-0000-000000000000",
            "stomach": 0.0,
            "storage_slot": 0,
            "learned_skills": [], "active_skills": [], "passive_skills": [],
            "hp": 1, "max_hp": 1,
            "group_id": null,
            "sanity": 1.0,
            "work_suitability": {},
            "is_sick": false,
            "friendship_point": 0
        });
        for (key, default_value) in defaults.as_object().unwrap() {
            let candidate = source.get(key).cloned().unwrap_or(serde_json::Value::Null);
            let accepted = match candidate {
                serde_json::Value::Null => default_value.clone(),
                other => other,
            };
            normalized.insert(key.clone(), accepted);
        }
        if !normalized["character_id"].is_string() {
            return Err(crate::error::CoreError::Other(
                "character_id is not a string".into(),
            ));
        }
        serde_json::from_value(serde_json::Value::Object(normalized))
            .map_err(|e| crate::error::CoreError::Other(format!("invalid pal_data: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pal_dto_deserializes_python_input_shape() {
        // Shape sent by the frontend for clone_pal: no character_key,
        // is_predator or filtered_nickname.
        let payload = serde_json::json!({
            "instance_id": "11111111-2222-3333-4444-555555555555",
            "owner_uid": "99999999-2222-3333-4444-555555555555",
            "character_id": "SheepBall",
            "is_lucky": false,
            "is_boss": false,
            "gender": "Female",
            "rank_hp": 0, "rank_attack": 0, "rank_defense": 0, "rank_craftspeed": 0,
            "talent_hp": 50, "talent_shot": 50, "talent_defense": 50,
            "rank": 1, "level": 10, "exp": 1000,
            "nickname": "wooly",
            "is_tower": false,
            "storage_id": "88888888-2222-3333-4444-555555555555",
            "stomach": 150.0,
            "storage_slot": 3,
            "learned_skills": [], "active_skills": [], "passive_skills": ["Rare"],
            "hp": 545000, "max_hp": 545000,
            "group_id": null,
            "sanity": 100.0,
            "work_suitability": {"EmitFlame": 0, "Handcraft": 1},
            "is_sick": false,
            "friendship_point": 0
        });
        let dto: PalDto = serde_json::from_value(payload).unwrap();
        assert_eq!(dto.character_id, "SheepBall");
        assert_eq!(dto.gender, PalGender::Female);
        assert_eq!(dto.work_suitability.get("Handcraft"), Some(&1));
        assert_eq!(dto.storage_slot, 3);
        // Output-only fields default when the frontend payload omits them.
        assert_eq!(dto.character_key, "");
        assert!(!dto.is_predator);
        assert_eq!(dto.filtered_nickname, None);
    }

    #[test]
    fn gender_wire_values() {
        assert_eq!(serde_json::to_value(PalGender::Female).unwrap(), "Female");
        assert_eq!(serde_json::to_value(PalGender::None).unwrap(), "None");
        assert_eq!(
            PalGender::from_prefixed("EPalGenderType::Male"),
            PalGender::Male
        );
        assert_eq!(PalGender::from_prefixed("garbage"), PalGender::Female);
        assert_eq!(PalGender::Female.prefixed(), "EPalGenderType::Female");
    }

    #[test]
    fn character_key_strips_prefixes() {
        let known = HashSet::new();
        assert_eq!(format_character_key("BOSS_SheepBall", &known), "sheepball");
        assert_eq!(format_character_key("PREDATOR_Deer", &known), "deer");
        assert_eq!(
            format_character_key("Kitsunebi_Avatar", &known),
            "kitsunebi"
        );
        assert_eq!(format_character_key("SheepBall", &known), "sheepball");
        // BOSS_ prefix retained when the raw id is itself a known pal key.
        let mut known_with_boss = HashSet::new();
        known_with_boss.insert("BOSS_SheepBall".to_string());
        assert_eq!(
            format_character_key("BOSS_SheepBall", &known_with_boss),
            "boss_sheepball"
        );
    }

    fn sample_pal_dto() -> PalDto {
        let mut work_suitability = OrderedMap::new();
        work_suitability.insert("Handcraft".to_string(), 2);
        work_suitability.insert("Mining".to_string(), 1);
        PalDto {
            instance_id: "11111111-2222-3333-4444-555555555555".parse().unwrap(),
            character_id: "SheepBall".to_string(),
            character_key: "sheepball".to_string(),
            owner_uid: Some("99999999-2222-3333-4444-555555555555".parse().unwrap()),
            is_lucky: Some(false),
            is_boss: Some(false),
            is_predator: false,
            is_tower: false,
            gender: PalGender::Female,
            nickname: Some("wooly".to_string()),
            filtered_nickname: None,
            group_id: None,
            stomach: 150.0,
            sanity: 100.0,
            hp: 545000,
            level: 10,
            exp: 1000,
            rank: 1,
            rank_hp: 0,
            rank_attack: 0,
            rank_defense: 0,
            rank_craftspeed: 0,
            talent_hp: 50,
            talent_shot: 50,
            talent_defense: 50,
            max_hp: 545000,
            storage_slot: 3,
            storage_id: "88888888-2222-3333-4444-555555555555".parse().unwrap(),
            learned_skills: vec![],
            active_skills: vec![],
            passive_skills: vec!["Rare".to_string()],
            work_suitability,
            is_sick: false,
            friendship_point: 0,
        }
    }

    #[test]
    fn pal_dto_pins_exact_wire_order() {
        let serialized = serde_json::to_string(&sample_pal_dto()).unwrap();
        assert_eq!(
            concat!(
                "{\"instance_id\":\"11111111-2222-3333-4444-555555555555\",",
                "\"character_id\":\"SheepBall\",",
                "\"character_key\":\"sheepball\",",
                "\"owner_uid\":\"99999999-2222-3333-4444-555555555555\",",
                "\"is_lucky\":false,",
                "\"is_boss\":false,",
                "\"is_predator\":false,",
                "\"is_tower\":false,",
                "\"gender\":\"Female\",",
                "\"nickname\":\"wooly\",",
                "\"filtered_nickname\":null,",
                "\"group_id\":null,",
                "\"stomach\":150.0,",
                "\"sanity\":100.0,",
                "\"hp\":545000,",
                "\"level\":10,",
                "\"exp\":1000,",
                "\"rank\":1,",
                "\"rank_hp\":0,",
                "\"rank_attack\":0,",
                "\"rank_defense\":0,",
                "\"rank_craftspeed\":0,",
                "\"talent_hp\":50,",
                "\"talent_shot\":50,",
                "\"talent_defense\":50,",
                "\"max_hp\":545000,",
                "\"storage_slot\":3,",
                "\"storage_id\":\"88888888-2222-3333-4444-555555555555\",",
                "\"learned_skills\":[],",
                "\"active_skills\":[],",
                "\"passive_skills\":[\"Rare\"],",
                "\"work_suitability\":{\"Handcraft\":2,\"Mining\":1},",
                "\"is_sick\":false,",
                "\"friendship_point\":0}"
            ),
            serialized
        );
    }

    #[test]
    fn pal_dto_round_trips_through_its_own_wire_format() {
        let original = sample_pal_dto();
        let serialized = serde_json::to_string(&original).unwrap();
        let round_tripped: PalDto = serde_json::from_str(&serialized).unwrap();
        assert_eq!(original.instance_id, round_tripped.instance_id);
        assert_eq!(original.character_id, round_tripped.character_id);
        assert_eq!(
            original.work_suitability.get("Mining"),
            round_tripped.work_suitability.get("Mining")
        );
    }

    #[test]
    fn pal_dto_ignores_unknown_keys_from_the_frontend() {
        let mut payload = serde_json::to_value(sample_pal_dto()).unwrap();
        payload.as_object_mut().unwrap().insert(
            "some_future_field_the_frontend_added".to_string(),
            serde_json::json!(true),
        );
        let dto: Result<PalDto, _> = serde_json::from_value(payload);
        assert!(
            dto.is_ok(),
            "unknown key must be ignored, not rejected: {dto:?}"
        );
    }
}
