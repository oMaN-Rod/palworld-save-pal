//! Item/character container wire DTOs. Field declaration order is a wire
//! contract: `serde` serializes in declaration order and the frontend
//! consumes this JSON as-is.
use serde::{Deserialize, Serialize};

use super::pal::PalGender;

/// One struct covers both the frontend's edit payload and the server's
/// response. `modified` is an input-only flag; `skip_serializing` keeps it
/// out of responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicItemDto {
    pub local_id: uuid::Uuid,
    #[serde(default, skip_serializing)]
    pub modified: bool, // input-only
    #[serde(default)]
    pub character_id: Option<String>,
    #[serde(default)]
    pub character_key: Option<String>, // output-only
    #[serde(default)]
    pub durability: Option<f64>,
    #[serde(default)]
    pub passive_skill_list: Option<Vec<String>>,
    #[serde(default)]
    pub remaining_bullets: Option<i64>,
    #[serde(default, rename = "type")]
    pub r#type: Option<String>,
    #[serde(default)]
    pub static_id: Option<String>, // output-only
    #[serde(default)]
    pub gender: Option<PalGender>,
    #[serde(default)]
    pub active_skills: Option<Vec<String>>,
    #[serde(default)]
    pub learned_skills: Option<Vec<String>>,
    #[serde(default)]
    pub passive_skills: Option<Vec<String>>,
    #[serde(default)]
    pub talent_hp: Option<i64>,
    #[serde(default)]
    pub talent_shot: Option<i64>,
    #[serde(default)]
    pub talent_defense: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemContainerSlotDto {
    #[serde(default)]
    pub dynamic_item: Option<DynamicItemDto>,
    pub slot_index: i32,
    pub count: i32,
    #[serde(default)]
    pub static_id: Option<String>,
    #[serde(default)]
    pub local_id: Option<uuid::Uuid>, // output-only
}

/// `type` wire values: `"CommonContainer"`, `"EssentialContainer"`,
/// `"WeaponLoadOutContainer"`, `"PlayerEquipArmorContainer"`,
/// `"FoodEquipContainer"`, `"BaseContainer"`, `"GuildChest"`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemContainerDto {
    pub id: uuid::Uuid,
    #[serde(rename = "type")]
    pub r#type: String,
    #[serde(default)]
    pub slots: Vec<ItemContainerSlotDto>,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub slot_num: i32, // output-only
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CharacterContainerSlotDto {
    pub slot_index: i32,
    pub pal_id: Option<uuid::Uuid>,
}

/// Response-only. `type` wire values: `"PalBox"` | `"Party"` | `"Base"`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CharacterContainerDto {
    pub id: uuid::Uuid,
    pub player_uid: uuid::Uuid,
    #[serde(rename = "type")]
    pub r#type: String,
    pub size: i32,
    pub slots: Vec<CharacterContainerSlotDto>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_dynamic_item() -> DynamicItemDto {
        DynamicItemDto {
            local_id: "11111111-2222-3333-4444-555555555555".parse().unwrap(),
            modified: true,
            character_id: Some("SheepBall".to_string()),
            character_key: Some("sheepball".to_string()),
            durability: Some(80.5),
            passive_skill_list: Some(vec!["Rare".to_string()]),
            remaining_bullets: Some(30),
            r#type: Some("weapon".to_string()),
            static_id: Some("WeaponRifle".to_string()),
            gender: Some(PalGender::Female),
            active_skills: Some(vec!["Waza1".to_string()]),
            learned_skills: Some(vec!["Waza2".to_string()]),
            passive_skills: Some(vec!["Waza3".to_string()]),
            talent_hp: Some(50),
            talent_shot: Some(60),
            talent_defense: Some(70),
        }
    }

    #[test]
    fn dynamic_item_dto_pins_exact_wire_order_and_hides_modified() {
        let serialized = serde_json::to_string(&sample_dynamic_item()).unwrap();
        assert_eq!(
            concat!(
                "{\"local_id\":\"11111111-2222-3333-4444-555555555555\",",
                "\"character_id\":\"SheepBall\",",
                "\"character_key\":\"sheepball\",",
                "\"durability\":80.5,",
                "\"passive_skill_list\":[\"Rare\"],",
                "\"remaining_bullets\":30,",
                "\"type\":\"weapon\",",
                "\"static_id\":\"WeaponRifle\",",
                "\"gender\":\"Female\",",
                "\"active_skills\":[\"Waza1\"],",
                "\"learned_skills\":[\"Waza2\"],",
                "\"passive_skills\":[\"Waza3\"],",
                "\"talent_hp\":50,",
                "\"talent_shot\":60,",
                "\"talent_defense\":70}"
            ),
            serialized
        );
    }

    #[test]
    fn dynamic_item_dto_ignores_unknown_keys_and_tolerates_all_fields_absent() {
        let dto: Result<DynamicItemDto, _> = serde_json::from_value(serde_json::json!({
            "local_id": "11111111-2222-3333-4444-555555555555",
            "modified": true,
            "some_future_field": 123
        }));
        let dto = dto.unwrap();
        assert!(dto.modified);
        assert_eq!(dto.character_id, None);
        assert_eq!(dto.talent_hp, None);
    }

    fn sample_item_container_slot() -> ItemContainerSlotDto {
        ItemContainerSlotDto {
            dynamic_item: Some(sample_dynamic_item()),
            slot_index: 4,
            count: 1,
            static_id: Some("WeaponRifle".to_string()),
            local_id: Some("11111111-2222-3333-4444-555555555555".parse().unwrap()),
        }
    }

    #[test]
    fn item_container_slot_dto_pins_exact_wire_order() {
        let mut slot = sample_item_container_slot();
        slot.dynamic_item = None; // keep this assertion focused on the slot's own order
        let serialized = serde_json::to_string(&slot).unwrap();
        assert_eq!(
            concat!(
                "{\"dynamic_item\":null,",
                "\"slot_index\":4,",
                "\"count\":1,",
                "\"static_id\":\"WeaponRifle\",",
                "\"local_id\":\"11111111-2222-3333-4444-555555555555\"}"
            ),
            serialized
        );
    }

    #[test]
    fn item_container_slot_dto_deserializes_python_input_shape_without_local_id() {
        let payload = serde_json::json!({
            "slot_index": 0,
            "count": 5,
            "static_id": "Wood",
            "dynamic_item": null,
            "unexpected_key": "ignored"
        });
        let dto: ItemContainerSlotDto = serde_json::from_value(payload).unwrap();
        assert_eq!(dto.slot_index, 0);
        assert_eq!(dto.count, 5);
        assert_eq!(dto.local_id, None);
    }

    #[test]
    fn item_container_dto_pins_exact_wire_order() {
        let container = ItemContainerDto {
            id: "22222222-2222-3333-4444-555555555555".parse().unwrap(),
            r#type: "CommonContainer".to_string(),
            slots: vec![sample_item_container_slot()],
            key: None,
            slot_num: 42,
        };
        let value = serde_json::to_value(&container).unwrap();
        let keys: Vec<&String> = value.as_object().unwrap().keys().collect();
        assert_eq!(vec!["id", "type", "slots", "key", "slot_num"], keys);
        assert_eq!(value["type"], "CommonContainer");
    }

    #[test]
    fn item_container_dto_ignores_unknown_keys() {
        let payload = serde_json::json!({
            "id": "22222222-2222-3333-4444-555555555555",
            "type": "CommonContainer",
            "totally_unexpected": true
        });
        let dto: ItemContainerDto = serde_json::from_value(payload).unwrap();
        assert_eq!(dto.r#type, "CommonContainer");
        assert_eq!(dto.slot_num, 0);
        assert!(dto.slots.is_empty());
    }

    #[test]
    fn character_container_dto_pins_exact_wire_order() {
        let container = CharacterContainerDto {
            id: "33333333-2222-3333-4444-555555555555".parse().unwrap(),
            player_uid: "44444444-2222-3333-4444-555555555555".parse().unwrap(),
            r#type: "PalBox".to_string(),
            size: 60,
            slots: vec![CharacterContainerSlotDto {
                slot_index: 0,
                pal_id: Some("11111111-2222-3333-4444-555555555555".parse().unwrap()),
            }],
        };
        let serialized = serde_json::to_string(&container).unwrap();
        assert_eq!(
            concat!(
                "{\"id\":\"33333333-2222-3333-4444-555555555555\",",
                "\"player_uid\":\"44444444-2222-3333-4444-555555555555\",",
                "\"type\":\"PalBox\",",
                "\"size\":60,",
                "\"slots\":[{\"slot_index\":0,\"pal_id\":\"11111111-2222-3333-4444-555555555555\"}]}"
            ),
            serialized
        );
    }
}
