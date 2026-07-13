//! `PlayerDto` covers both the frontend's edit payload and the server's
//! response. Field declaration order is a wire contract: `serde` serializes
//! in declaration order and the frontend consumes this JSON as-is.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::container::{CharacterContainerDto, ItemContainerDto};
use super::ordered_map::OrderedMap;
use super::pal::PalDto;
use super::summary::IsoDateTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldMapPointDto {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

fn default_player_hp() -> i64 {
    5000
}

fn default_hundred() -> f64 {
    100.0
}

/// Output-only fields are `#[serde(default)]` so edit payloads, which never
/// send them, still deserialize. `hp`/`stomach`/`sanity` carry explicit
/// defaults an omitting edit payload must land on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerDto {
    #[serde(default)]
    pub pals: OrderedMap<uuid::Uuid, PalDto>, // output-only
    #[serde(default)]
    pub common_container: Option<ItemContainerDto>,
    #[serde(default)]
    pub essential_container: Option<ItemContainerDto>,
    #[serde(default)]
    pub weapon_load_out_container: Option<ItemContainerDto>,
    #[serde(default)]
    pub player_equipment_armor_container: Option<ItemContainerDto>,
    #[serde(default)]
    pub food_equip_container: Option<ItemContainerDto>,
    #[serde(default)]
    pub pal_box: Option<CharacterContainerDto>, // output-only
    #[serde(default)]
    pub party: Option<CharacterContainerDto>, // output-only
    #[serde(default)]
    pub guild_id: Option<uuid::Uuid>, // output-only
    pub uid: uuid::Uuid,
    #[serde(default)]
    pub instance_id: Option<uuid::Uuid>,
    pub nickname: String,
    pub level: i64,
    #[serde(default)]
    pub technologies: Vec<String>,
    #[serde(default)]
    pub technology_points: i64,
    #[serde(default)]
    pub boss_technology_points: i64,
    pub exp: i64,
    #[serde(default = "default_player_hp")]
    pub hp: i64,
    #[serde(default = "default_hundred")]
    pub stomach: f64,
    #[serde(default = "default_hundred")]
    pub sanity: f64,
    #[serde(default)]
    pub status_point_list: OrderedMap<String, i64>,
    #[serde(default)]
    pub ext_status_point_list: OrderedMap<String, i64>,
    #[serde(default)]
    pub pal_box_id: Option<uuid::Uuid>,
    #[serde(default)]
    pub otomo_container_id: Option<uuid::Uuid>,
    #[serde(default)]
    pub completed_missions: Vec<String>,
    #[serde(default)]
    pub current_missions: Vec<String>,
    #[serde(default)]
    pub unlocked_fast_travel_points: Option<Vec<String>>,
    #[serde(default)]
    pub collected_effigies: Option<Vec<String>>,
    /// Collected relic instance-flag keys per relic type (bare key from
    /// `relic::RELIC_TYPE_MAP`), read from and written back to
    /// `RelicObtainForInstanceFlagByType`.
    ///
    /// `collected_relics["capture_power"]` equals `collected_effigies` on a 1.0 save. On
    /// write, CapturePower is taken from `collected_effigies` -- the list that also drives
    /// the legacy flat flag map -- so this key is read-only in practice. A pre-1.0 save
    /// reads as an empty map, and writing one back invents nothing.
    #[serde(default)]
    pub collected_relics: Option<BTreeMap<String, Vec<String>>>,
    /// `NormalBossDefeatFlag` + `TowerBossDefeatFlag` keys merged, read-only:
    /// the UI only needs "is this boss defeated" for the map overlay.
    #[serde(default)]
    pub defeated_bosses: Option<Vec<String>>, // output-only
    #[serde(default)]
    pub effigy_possess_num: i64, // output-only
    #[serde(default)]
    pub location: Option<WorldMapPointDto>, // output-only
    #[serde(default)]
    pub last_online_time: Option<IsoDateTime>, // output-only
    /// `Option` because `null` is a real wire shape: a player only has DPS
    /// data when the save carries a separate per-player DPS-arena `.sav`
    /// file. Absent file serializes `null`; present-but-empty serializes `{}`.
    #[serde(default)]
    pub dps: Option<OrderedMap<i32, PalDto>>, // output-only
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::pal::PalGender;

    fn sample_pal(seed: &str) -> PalDto {
        PalDto {
            instance_id: seed.parse().unwrap(),
            character_id: "SheepBall".to_string(),
            character_key: "sheepball".to_string(),
            owner_uid: None,
            is_lucky: Some(false),
            is_boss: Some(false),
            is_predator: false,
            is_tower: false,
            gender: PalGender::Female,
            nickname: None,
            filtered_nickname: None,
            group_id: None,
            stomach: 150.0,
            sanity: 100.0,
            hp: 1000,
            level: 1,
            exp: 0,
            rank: 1,
            rank_hp: 0,
            rank_attack: 0,
            rank_defense: 0,
            rank_craftspeed: 0,
            talent_hp: 0,
            talent_shot: 0,
            talent_defense: 0,
            max_hp: 1000,
            storage_slot: 0,
            storage_id: seed.parse().unwrap(),
            learned_skills: vec![],
            active_skills: vec![],
            passive_skills: vec![],
            work_suitability: OrderedMap::new(),
            is_sick: false,
            friendship_point: 0,
        }
    }

    fn minimal_player_dto_request_payload() -> serde_json::Value {
        // Shape sent by the frontend for update_player: required fields only.
        serde_json::json!({
            "uid": "99999999-2222-3333-4444-555555555555",
            "nickname": "Tester",
            "level": 25,
            "exp": 12345
        })
    }

    #[test]
    fn player_dto_deserializes_minimal_python_input_shape_with_defaults() {
        let dto: PlayerDto = serde_json::from_value(minimal_player_dto_request_payload()).unwrap();
        assert_eq!(dto.nickname, "Tester");
        assert_eq!(dto.level, 25);
        assert_eq!(dto.hp, 5000);
        assert_eq!(dto.stomach, 100.0);
        assert_eq!(dto.sanity, 100.0);
        // Output-only fields default when the frontend payload omits them.
        assert!(dto.pals.is_empty());
        assert_eq!(dto.pal_box, None);
        assert_eq!(dto.last_online_time, None);
    }

    #[test]
    fn player_dto_ignores_unknown_keys_from_the_frontend() {
        let mut payload = minimal_player_dto_request_payload();
        payload
            .as_object_mut()
            .unwrap()
            .insert("some_future_field".to_string(), serde_json::json!(42));
        let dto: Result<PlayerDto, _> = serde_json::from_value(payload);
        assert!(
            dto.is_ok(),
            "unknown key must be ignored, not rejected: {dto:?}"
        );
    }

    #[test]
    fn player_dto_pins_exact_wire_order() {
        let mut pals = OrderedMap::new();
        pals.insert(
            "11111111-2222-3333-4444-555555555555".parse().unwrap(),
            sample_pal("11111111-2222-3333-4444-555555555555"),
        );
        let mut status_point_list = OrderedMap::new();
        status_point_list.insert("HP".to_string(), 3);

        let dto = PlayerDto {
            pals,
            common_container: None,
            essential_container: None,
            weapon_load_out_container: None,
            player_equipment_armor_container: None,
            food_equip_container: None,
            pal_box: None,
            party: None,
            guild_id: None,
            uid: "99999999-2222-3333-4444-555555555555".parse().unwrap(),
            instance_id: None,
            nickname: "Tester".to_string(),
            level: 25,
            technologies: vec!["Tech1".to_string()],
            technology_points: 3,
            boss_technology_points: 1,
            exp: 12345,
            hp: 5000,
            stomach: 100.0,
            sanity: 100.0,
            status_point_list,
            ext_status_point_list: OrderedMap::new(),
            pal_box_id: None,
            otomo_container_id: None,
            completed_missions: vec![],
            current_missions: vec![],
            unlocked_fast_travel_points: None,
            collected_effigies: None,
            collected_relics: None,
            defeated_bosses: None,
            effigy_possess_num: 0,
            location: None,
            last_online_time: None,
            dps: None,
        };

        let value = serde_json::to_value(&dto).unwrap();
        let keys: Vec<&String> = value.as_object().unwrap().keys().collect();
        assert_eq!(
            vec![
                "pals",
                "common_container",
                "essential_container",
                "weapon_load_out_container",
                "player_equipment_armor_container",
                "food_equip_container",
                "pal_box",
                "party",
                "guild_id",
                "uid",
                "instance_id",
                "nickname",
                "level",
                "technologies",
                "technology_points",
                "boss_technology_points",
                "exp",
                "hp",
                "stomach",
                "sanity",
                "status_point_list",
                "ext_status_point_list",
                "pal_box_id",
                "otomo_container_id",
                "completed_missions",
                "current_missions",
                "unlocked_fast_travel_points",
                "collected_effigies",
                "collected_relics",
                "defeated_bosses",
                "effigy_possess_num",
                "location",
                "last_online_time",
                "dps",
            ],
            keys
        );
        assert_eq!(value["nickname"], "Tester");
        assert_eq!(value["status_point_list"]["HP"], 3);
    }

    #[test]
    fn dps_none_and_empty_map_are_both_legitimate_and_distinct_wire_shapes() {
        // `null` is the real shape for a player with no DPS-arena save file,
        // not a defect to be normalized away (see `PlayerDto::dps`).
        let none_payload = minimal_player_dto_request_payload();
        let dto: PlayerDto = serde_json::from_value(none_payload).unwrap();
        assert!(dto.dps.is_none());
        let value = serde_json::to_value(&dto).unwrap();
        assert!(value["dps"].is_null());

        // `Some({})` is the distinct case where the file exists but holds no pals.
        let mut with_empty_dps = dto;
        with_empty_dps.dps = Some(OrderedMap::new());
        let value = serde_json::to_value(&with_empty_dps).unwrap();
        assert_eq!(value["dps"], serde_json::json!({}));
    }

    #[test]
    fn world_map_point_dto_wire_shape() {
        let point = WorldMapPointDto {
            x: 1.5,
            y: -2.0,
            z: 0.0,
        };
        let serialized = serde_json::to_string(&point).unwrap();
        assert_eq!("{\"x\":1.5,\"y\":-2.0,\"z\":0.0}", serialized);
    }
}
