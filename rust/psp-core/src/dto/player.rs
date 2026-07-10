//! `PlayerDto` — a field-for-field port of the union of
//! `palworld_save_pal/dto/player.py::PlayerDTO` (input) and
//! `palworld_save_pal/game/player.py::Player`'s dump (output). Field order
//! matches `Player`'s dump, not `PlayerDTO`'s declaration order — see
//! `dto/pal.rs`'s module doc for why (plain fields first in declaration
//! order, then computed fields in declaration order; the two groups are
//! never interleaved by source position). Verified directly against the
//! real class:
//! `.venv/Scripts/python.exe -c "from palworld_save_pal.game.player import
//! Player; print(list(Player.model_fields.keys()),
//! list(Player.model_computed_fields.keys()))"` — 8 plain fields (`pals` ..
//! `party`) then 24 computed fields (`guild_id` .. `dps`), 32 total.
use serde::{Deserialize, Serialize};

use super::container::{CharacterContainerDto, ItemContainerDto};
use super::ordered_map::OrderedMap;
use super::pal::PalDto;
use super::summary::IsoDateTime;

/// `game/map.py::WorldMapPoint` — plain `BaseModel`, no DTO counterpart
/// (response-only).
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

/// Union of `dto/player.py::PlayerDTO` (input) and
/// `game/player.py::Player`'s dump (output). See this module's doc comment
/// for the wire-order derivation. Fields absent from `PlayerDTO` entirely
/// (`pals`, `pal_box`, `party`, `relic_possess_num`, `location`,
/// `last_online_time`, `dps`) are `#[serde(default)]`; fields the DTO
/// declares with a Python default (`hp = 5000`, `stomach = 100.0`,
/// `sanity = 100.0`) carry the matching Rust default so an edit payload
/// that omits them behaves the same as it does in Python.
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
    #[serde(default)]
    pub relic_possess_num: i64, // output-only
    #[serde(default)]
    pub location: Option<WorldMapPointDto>, // output-only
    #[serde(default)]
    pub last_online_time: Option<IsoDateTime>, // output-only
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
        // Shape sent by the frontend for update_player (dto/player.py's
        // required fields only: uid, nickname, level, exp).
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
        // Python defaults: hp=5000, stomach=100.0, sanity=100.0.
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
            relic_possess_num: 0,
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
                "relic_possess_num",
                "location",
                "last_online_time",
                "dps",
            ],
            keys
        );
        // A couple of value spot-checks alongside the key-order pin.
        assert_eq!(value["nickname"], "Tester");
        assert_eq!(value["status_point_list"]["HP"], 3);
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
