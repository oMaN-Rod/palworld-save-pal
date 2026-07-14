//! Guild/base wire DTOs. Field declaration order is a wire contract: `serde`
//! serializes in declaration order and the frontend consumes this JSON as-is.
use serde::{Deserialize, Serialize};

use super::container::{CharacterContainerDto, ItemContainerDto};
use super::ordered_map::OrderedMap;
use super::pal::PalDto;
use super::player::WorldMapPointDto;

/// Serves as both the `lab_research`/`lab_research_data` wire shape and the
/// `update_lab_research` request item shape. `work_amount` is an accumulated
/// progress counter, so it stays `f64` to avoid `f32` round-trip drift.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuildLabResearchInfo {
    pub research_id: String,
    pub work_amount: f64,
}

/// `storage_containers` is a plain map defaulting to empty rather than an
/// `Option`: no consumer distinguishes an absent map from an empty one.
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

/// `bases` and `lab_research` stay `Option` (unlike `BaseDto`'s
/// `storage_containers`): guild updates treat an omitted collection and an
/// empty one differently, so that distinction must survive the wire.
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
