//! Walking the per-player Dimensional Pal Storage (DPS) `SaveParameterArray`
//! so the legality validator can reach pals that live outside `Level.sav`'s
//! character map. Slots mirror [`crate::domain::pal::pal_dto_from_dps_slot`]'s
//! shape: a struct holding a `SaveParameter` and a nested `InstanceId` guid.

use crate::props;
use crate::ue::{Properties, PropertyKey, Save, StructValue};

/// One pal stored in a player's DPS save.
pub(crate) struct DpsPal<'a> {
    pub(crate) save_parameter: &'a Properties,
    pub(crate) character_id: &'a str,
    pub(crate) instance_id: uuid::Uuid,
}

/// Every readable pal slot in a parsed DPS save, in slot order. Slots whose
/// `CharacterID` is missing or literally `"None"` (the game's empty-marker)
/// are skipped, matching the player-details dump.
pub(crate) fn pals_in(save: &Save) -> Vec<DpsPal<'_>> {
    let Some(slots) = save
        .root
        .properties
        .0
        .get(&PropertyKey::from("SaveParameterArray"))
        .and_then(props::struct_values)
    else {
        return Vec::new();
    };
    slots
        .iter()
        .filter_map(|slot| {
            let StructValue::Struct(slot_props) = slot else {
                return None;
            };
            let save_parameter =
                props::struct_props(slot_props.0.get(&PropertyKey::from("SaveParameter"))?)?;
            let character_id = save_parameter
                .0
                .get(&PropertyKey::from("CharacterID"))
                .and_then(props::as_str)?;
            if character_id.is_empty() || character_id == "None" {
                return None;
            }
            let instance_id = slot_props
                .0
                .get(&PropertyKey::from("InstanceId"))
                .and_then(props::struct_props)
                .and_then(|inner| inner.0.get(&PropertyKey::from("InstanceId")))
                .and_then(props::as_uuid)
                .unwrap_or(uuid::Uuid::nil());
            Some(DpsPal {
                save_parameter,
                character_id,
                instance_id,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ue::{Byte, Property};

    fn guid_property(text: &str) -> Property {
        Property::Struct(StructValue::Guid(
            serde_json::from_value(serde_json::Value::String(text.to_string())).unwrap(),
        ))
    }

    fn slot(character_id: &str, instance_id: &str) -> StructValue {
        let mut save_parameter = Properties::default();
        save_parameter.insert("CharacterID", Property::Str(character_id.to_string()));
        save_parameter.insert("Level", Property::Byte(Byte::Byte(30)));
        let mut slot_props = Properties::default();
        slot_props.insert(
            "SaveParameter",
            Property::Struct(StructValue::Struct(save_parameter)),
        );
        let mut inner = Properties::default();
        inner.insert("InstanceId", guid_property(instance_id));
        slot_props.insert("InstanceId", Property::Struct(StructValue::Struct(inner)));
        StructValue::Struct(slot_props)
    }

    fn dps_save(slots: Vec<StructValue>) -> Save {
        let mut root_properties = Properties::default();
        root_properties.insert(
            "SaveParameterArray",
            Property::Array(crate::ue::ValueVec::Struct(slots)),
        );
        Save {
            header: crate::ue::Header {
                magic: 0,
                save_game_version: 0,
                package_version: crate::ue::PackageVersion { ue4: 0, ue5: None },
                engine_version_major: 0,
                engine_version_minor: 0,
                engine_version_patch: 0,
                engine_version_build: 0,
                engine_version: String::new(),
                custom_version: None,
            },
            schemas: crate::ue::PropertySchemas::default(),
            root: crate::ue::Root {
                save_game_type: String::new(),
                properties: root_properties,
            },
            extra: Vec::new(),
        }
    }

    #[test]
    fn walks_slots_and_skips_empty_markers() {
        let save = dps_save(vec![
            slot("Sheepball", "aaaaaaaa-0000-0000-0000-000000000001"),
            slot("None", "aaaaaaaa-0000-0000-0000-000000000002"),
            // A non-struct slot (malformed) is skipped, not fatal.
            StructValue::Guid(crate::ue::FGuid::nil()),
        ]);
        let pals = pals_in(&save);
        assert_eq!(pals.len(), 1);
        assert_eq!(pals[0].character_id, "Sheepball");
        assert_eq!(
            pals[0].instance_id,
            "aaaaaaaa-0000-0000-0000-000000000001"
                .parse::<uuid::Uuid>()
                .unwrap()
        );
    }

    #[test]
    fn a_save_without_the_array_is_empty() {
        assert!(pals_in(&dps_save(vec![])).is_empty());
        let mut save = dps_save(vec![]);
        let mut root_properties = Properties::default();
        root_properties.insert("Unrelated", Property::Str("x".into()));
        save.root.properties = root_properties;
        assert!(pals_in(&save).is_empty());
    }
}
