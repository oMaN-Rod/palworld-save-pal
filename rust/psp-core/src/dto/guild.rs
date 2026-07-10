//! `GuildDto`/`BaseDto` — field-for-field ports of the union of
//! `dto/guild.py::GuildDTO`/`dto/base.py::BaseDTO` (input) and
//! `game/guild.py::Guild`/`game/base.py::Base`'s dumps (output). Field
//! order matches the game models' dumps, not the DTOs' declaration order —
//! see `dto/pal.rs`'s module doc for why. Verified directly against the
//! real classes:
//! `.venv/Scripts/python.exe -c "from palworld_save_pal.game.guild import
//! Guild; from palworld_save_pal.game.base import Base; print(list(
//! Guild.model_fields), list(Guild.model_computed_fields)); print(list(
//! Base.model_fields), list(Base.model_computed_fields))"`.
use serde::{Deserialize, Serialize};

use super::container::{CharacterContainerDto, ItemContainerDto};
use super::ordered_map::OrderedMap;
use super::pal::PalDto;
use super::player::WorldMapPointDto;

/// `game/guild_lab_research_info.py::GuildLabResearchInfo` — a plain
/// `BaseModel` used directly (not through a separate DTO) as both the
/// `Guild.lab_research`/`lab_research_data` wire shape *and* the
/// `update_lab_research` request payload's item shape. `work_amount` is
/// `f64`, not `f32`: Python's `float` is IEEE-754 double precision, and
/// this value is an accumulated in-game progress counter, not a small
/// literal -- an `f32` round-trip could visibly perturb its decimal digits
/// relative to what Python would emit for the same double.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuildLabResearchInfo {
    pub research_id: String,
    pub work_amount: f64,
}

/// Union of `dto/base.py::BaseDTO` (input) and `game/base.py::Base`'s dump
/// (output: `pals`, `container_id`, `slot_count`, `storage_containers`,
/// `pal_container` (plain fields, declaration order), then `id`, `name`,
/// `location`, `area_range` (computed fields, declaration order)).
/// `storage_containers` is required (non-`Option`) on `BaseDTO`'s input but
/// `Optional[...] = None` on `Base`'s output; unified here as a plain
/// (non-`Option`) map defaulting empty -- an absent/`None` map and an empty
/// map are interchangeable for every downstream consumer of this field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseDto {
    #[serde(default)]
    pub pals: OrderedMap<uuid::Uuid, PalDto>, // output-only
    #[serde(default)]
    pub container_id: Option<uuid::Uuid>, // output-only
    #[serde(default)]
    pub slot_count: Option<i32>, // output-only
    #[serde(default)]
    pub storage_containers: OrderedMap<uuid::Uuid, ItemContainerDto>,
    #[serde(default)]
    pub pal_container: Option<CharacterContainerDto>, // output-only
    pub id: uuid::Uuid,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub location: Option<WorldMapPointDto>, // output-only
    #[serde(default)]
    pub area_range: Option<f64>,
}

/// Union of `dto/guild.py::GuildDTO` (input) and `game/guild.py::Guild`'s
/// dump (output: `bases`, `guild_chest`, `lab_research` (plain fields,
/// declaration order), then `name`, `base_camp_level`, `id`,
/// `admin_player_uid`, `players`, `container_id`, `lab_research_data`
/// (computed fields, declaration order)).
///
/// `bases` and `lab_research` stay `Option` (unlike `BaseDto`'s
/// `storage_containers`): `Guild.update_from` treats `None` and an empty
/// collection differently on input (`if guildDTO.bases: ... else:
/// logger.warning(...)`), so collapsing that distinction here would lose
/// real behavior a later task needs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuildDto {
    #[serde(default)]
    pub bases: Option<OrderedMap<uuid::Uuid, BaseDto>>,
    #[serde(default)]
    pub guild_chest: Option<ItemContainerDto>,
    #[serde(default)]
    pub lab_research: Option<Vec<GuildLabResearchInfo>>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub base_camp_level: Option<i32>,
    #[serde(default)]
    pub id: Option<uuid::Uuid>, // output-only
    #[serde(default)]
    pub admin_player_uid: Option<uuid::Uuid>, // output-only
    #[serde(default)]
    pub players: Vec<uuid::Uuid>, // output-only
    #[serde(default)]
    pub container_id: Option<uuid::Uuid>, // output-only
    #[serde(default)]
    pub lab_research_data: Vec<GuildLabResearchInfo>, // output-only
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guild_lab_research_info_wire_shape() {
        let info = GuildLabResearchInfo {
            research_id: "Lab_001".to_string(),
            work_amount: 33.5,
        };
        let serialized = serde_json::to_string(&info).unwrap();
        assert_eq!(
            "{\"research_id\":\"Lab_001\",\"work_amount\":33.5}",
            serialized
        );
    }

    #[test]
    fn guild_lab_research_info_ignores_unknown_keys() {
        let payload = serde_json::json!({
            "research_id": "Lab_001",
            "work_amount": 10.0,
            "unexpected": "field"
        });
        let info: Result<GuildLabResearchInfo, _> = serde_json::from_value(payload);
        assert!(
            info.is_ok(),
            "unknown key must be ignored, not rejected: {info:?}"
        );
    }

    fn sample_base_dto() -> BaseDto {
        let mut storage_containers = OrderedMap::new();
        storage_containers.insert(
            "22222222-2222-3333-4444-555555555555".parse().unwrap(),
            ItemContainerDto {
                id: "22222222-2222-3333-4444-555555555555".parse().unwrap(),
                r#type: "BaseContainer".to_string(),
                slots: vec![],
                key: None,
                slot_num: 0,
            },
        );
        BaseDto {
            pals: OrderedMap::new(),
            container_id: Some("33333333-2222-3333-4444-555555555555".parse().unwrap()),
            slot_count: Some(60),
            storage_containers,
            pal_container: None,
            id: "11111111-2222-3333-4444-555555555555".parse().unwrap(),
            name: Some("Home Base".to_string()),
            location: Some(WorldMapPointDto {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            }),
            area_range: Some(1500.0),
        }
    }

    #[test]
    fn base_dto_pins_exact_wire_order() {
        let value = serde_json::to_value(sample_base_dto()).unwrap();
        let keys: Vec<&String> = value.as_object().unwrap().keys().collect();
        assert_eq!(
            vec![
                "pals",
                "container_id",
                "slot_count",
                "storage_containers",
                "pal_container",
                "id",
                "name",
                "location",
                "area_range",
            ],
            keys
        );
        assert_eq!(value["name"], "Home Base");
        assert_eq!(value["area_range"], 1500.0);
    }

    #[test]
    fn base_dto_deserializes_python_input_shape() {
        // dto/base.py::BaseDTO required fields: id, storage_containers.
        let payload = serde_json::json!({
            "id": "11111111-2222-3333-4444-555555555555",
            "storage_containers": {},
            "unexpected": "field"
        });
        let dto: BaseDto = serde_json::from_value(payload).unwrap();
        let expected_id: uuid::Uuid = "11111111-2222-3333-4444-555555555555".parse().unwrap();
        assert_eq!(dto.id, expected_id);
        assert!(dto.storage_containers.is_empty());
        assert_eq!(dto.name, None);
    }

    #[test]
    fn guild_dto_pins_exact_wire_order() {
        let mut bases = OrderedMap::new();
        bases.insert(
            "11111111-2222-3333-4444-555555555555".parse().unwrap(),
            sample_base_dto(),
        );

        let dto = GuildDto {
            bases: Some(bases),
            guild_chest: None,
            lab_research: Some(vec![GuildLabResearchInfo {
                research_id: "Lab_001".to_string(),
                work_amount: 5.0,
            }]),
            name: Some("The Guild".to_string()),
            base_camp_level: Some(3),
            id: Some("44444444-2222-3333-4444-555555555555".parse().unwrap()),
            admin_player_uid: Some("55555555-2222-3333-4444-555555555555".parse().unwrap()),
            players: vec!["55555555-2222-3333-4444-555555555555".parse().unwrap()],
            container_id: Some("66666666-2222-3333-4444-555555555555".parse().unwrap()),
            lab_research_data: vec![GuildLabResearchInfo {
                research_id: "Lab_001".to_string(),
                work_amount: 5.0,
            }],
        };

        let value = serde_json::to_value(&dto).unwrap();
        let keys: Vec<&String> = value.as_object().unwrap().keys().collect();
        assert_eq!(
            vec![
                "bases",
                "guild_chest",
                "lab_research",
                "name",
                "base_camp_level",
                "id",
                "admin_player_uid",
                "players",
                "container_id",
                "lab_research_data",
            ],
            keys
        );
        assert_eq!(value["name"], "The Guild");
        assert_eq!(value["base_camp_level"], 3);
    }

    #[test]
    fn guild_dto_distinguishes_omitted_bases_from_empty_bases() {
        // Guild.update_from branches on `if guildDTO.bases:` -- None (key
        // omitted) and {} (key present, empty) must stay distinguishable.
        let omitted: GuildDto = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(omitted.bases.is_none());

        let empty: GuildDto = serde_json::from_value(serde_json::json!({"bases": {}})).unwrap();
        assert!(empty.bases.is_some());
        assert!(empty.bases.unwrap().is_empty());
    }

    #[test]
    fn guild_dto_ignores_unknown_keys_from_the_frontend() {
        let payload = serde_json::json!({
            "name": "Renamed Guild",
            "some_future_field": true
        });
        let dto: Result<GuildDto, _> = serde_json::from_value(payload);
        assert!(
            dto.is_ok(),
            "unknown key must be ignored, not rejected: {dto:?}"
        );
        assert_eq!(dto.unwrap().name, Some("Renamed Guild".to_string()));
    }
}
