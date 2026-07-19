//! World-tree navigation. Accessors take `&crate::ue::Save` rather than
//! `&SaveSession` so a player's or a transfer source's tree can be navigated
//! the same way as `session.level`.
//!
//! `BaseCampSaveData`/`GuildExtraSaveDataMap`/`MapObjectSaveData` are absent
//! from any save that has never built a base, formed a guild, or placed a map
//! object -- normal data, not corruption -- so their accessors return
//! `Ok(None)`. `Err` is reserved for a broken `worldSaveData` or a
//! present-but-wrong-typed value.
//!
//! Lookups go through `props::get` (name-only matching) rather than indexing
//! `Properties`' `IndexMap` by a hand-built `PropertyKey`: a `PropertyKey` is
//! `(u32, String)` whose `u32` disambiguates same-named siblings, and nothing
//! guarantees a `worldSaveData` field sits at index `0`.

use crate::error::CoreError;
use crate::props;
use crate::ue::games::palworld::PalCharacterData;
use crate::ue::{MapEntry, Properties, Property, Save, StructValue};

/// `Level.sav`'s `worldSaveData` struct — the root of every map/array this
/// module navigates.
pub fn world_props(level: &Save) -> Result<&Properties, CoreError> {
    props::get(&level.root.properties, &["worldSaveData"])
        .and_then(props::struct_props)
        .ok_or_else(|| CoreError::Parse("worldSaveData missing from Level.sav".to_string()))
}

pub fn world_props_mut(level: &mut Save) -> Result<&mut Properties, CoreError> {
    props::get_mut(&mut level.root.properties, &["worldSaveData"])
        .and_then(props::struct_props_mut)
        .ok_or_else(|| CoreError::Parse("worldSaveData missing from Level.sav".to_string()))
}

/// A named map under `worldSaveData` that every real save carries; absence is
/// an `Err`.
macro_rules! world_map_accessors {
    ($get:ident, $get_mut:ident, $name:literal) => {
        pub fn $get(level: &Save) -> Result<&Vec<MapEntry>, CoreError> {
            let world = world_props(level)?;
            props::get(world, &[$name])
                .and_then(props::map_entries)
                .ok_or_else(|| {
                    CoreError::Parse(
                        concat!($name, " missing or not a map in worldSaveData").to_string(),
                    )
                })
        }
        pub fn $get_mut(level: &mut Save) -> Result<&mut Vec<MapEntry>, CoreError> {
            let world = world_props_mut(level)?;
            props::get_mut(world, &[$name])
                .and_then(props::map_entries_mut)
                .ok_or_else(|| {
                    CoreError::Parse(
                        concat!($name, " missing or not a map in worldSaveData").to_string(),
                    )
                })
        }
    };
}

/// A named map under `worldSaveData` a real save may legitimately not carry:
/// `Ok(None)` for absent, `Err` only for a present-but-wrong-typed value.
macro_rules! world_optional_map_accessors {
    ($get:ident, $get_mut:ident, $name:literal) => {
        pub fn $get(level: &Save) -> Result<Option<&Vec<MapEntry>>, CoreError> {
            let world = world_props(level)?;
            match props::get(world, &[$name]) {
                None => Ok(None),
                Some(property) => props::map_entries(property).map(Some).ok_or_else(|| {
                    CoreError::Parse(concat!($name, " present but not a map").to_string())
                }),
            }
        }
        pub fn $get_mut(level: &mut Save) -> Result<Option<&mut Vec<MapEntry>>, CoreError> {
            let world = world_props_mut(level)?;
            match props::get_mut(world, &[$name]) {
                None => Ok(None),
                Some(property) => props::map_entries_mut(property).map(Some).ok_or_else(|| {
                    CoreError::Parse(concat!($name, " present but not a map").to_string())
                }),
            }
        }
    };
}

world_map_accessors!(
    character_map,
    character_map_mut,
    "CharacterSaveParameterMap"
);
world_map_accessors!(
    item_container_map,
    item_container_map_mut,
    "ItemContainerSaveData"
);
world_map_accessors!(
    character_container_map,
    character_container_map_mut,
    "CharacterContainerSaveData"
);
world_map_accessors!(group_map, group_map_mut, "GroupSaveDataMap");
world_optional_map_accessors!(
    guild_extra_map,
    guild_extra_map_mut,
    "GuildExtraSaveDataMap"
);
world_optional_map_accessors!(base_camp_map, base_camp_map_mut, "BaseCampSaveData");

macro_rules! world_struct_array_accessors {
    ($get:ident, $get_mut:ident, $name:literal) => {
        pub fn $get(level: &Save) -> Result<&Vec<StructValue>, CoreError> {
            let world = world_props(level)?;
            props::get(world, &[$name])
                .and_then(props::struct_values)
                .ok_or_else(|| {
                    CoreError::Parse(
                        concat!($name, " missing or not a struct array in worldSaveData")
                            .to_string(),
                    )
                })
        }
        pub fn $get_mut(level: &mut Save) -> Result<&mut Vec<StructValue>, CoreError> {
            let world = world_props_mut(level)?;
            props::get_mut(world, &[$name])
                .and_then(props::struct_values_mut)
                .ok_or_else(|| {
                    CoreError::Parse(
                        concat!($name, " missing or not a struct array in worldSaveData")
                            .to_string(),
                    )
                })
        }
    };
}

/// `world_optional_map_accessors!` for struct-array-shaped fields.
macro_rules! world_optional_struct_array_accessors {
    ($get:ident, $get_mut:ident, $name:literal) => {
        pub fn $get(level: &Save) -> Result<Option<&Vec<StructValue>>, CoreError> {
            let world = world_props(level)?;
            match props::get(world, &[$name]) {
                None => Ok(None),
                Some(property) => props::struct_values(property).map(Some).ok_or_else(|| {
                    CoreError::Parse(concat!($name, " present but not a struct array").to_string())
                }),
            }
        }
        pub fn $get_mut(level: &mut Save) -> Result<Option<&mut Vec<StructValue>>, CoreError> {
            let world = world_props_mut(level)?;
            match props::get_mut(world, &[$name]) {
                None => Ok(None),
                Some(property) => props::struct_values_mut(property).map(Some).ok_or_else(|| {
                    CoreError::Parse(concat!($name, " present but not a struct array").to_string())
                }),
            }
        }
    };
}

world_struct_array_accessors!(
    dynamic_item_values,
    dynamic_item_values_mut,
    "DynamicItemSaveData"
);
world_optional_struct_array_accessors!(
    map_object_values,
    map_object_values_mut,
    "MapObjectSaveData"
);

/// A `CharacterSaveParameterMap` entry's key bag (`PlayerUId`, `InstanceId`).
pub fn entry_key_props(entry: &MapEntry) -> Option<&Properties> {
    props::struct_props(&entry.key)
}

pub fn entry_instance_id(entry: &MapEntry) -> Option<uuid::Uuid> {
    props::get(entry_key_props(entry)?, &["InstanceId"]).and_then(props::as_uuid)
}

pub fn entry_player_uid(entry: &MapEntry) -> Option<uuid::Uuid> {
    props::get(entry_key_props(entry)?, &["PlayerUId"]).and_then(props::as_uuid)
}

/// Overwrites the key's existing `PlayerUId` in place: `insert` on an
/// already-present key updates the value without disturbing uesave's recorded
/// write schema, so this never risks `Error::MissingPropertySchema`. A no-op
/// when `entry.key` isn't a struct.
pub fn set_entry_player_uid(entry: &mut MapEntry, uid: uuid::Uuid) {
    if let Some(key_props) = props::struct_props_mut(&mut entry.key) {
        key_props.insert("PlayerUId", props::guid_property(uid));
    }
}

/// `entry.value.RawData`, decoded as `PalCharacterData` -- the typed struct
/// backing every character-map entry, player or pal.
pub fn entry_character_data(entry: &MapEntry) -> Option<&PalCharacterData<crate::ue::Arch>> {
    let value_props = props::struct_props(&entry.value)?;
    match props::get(value_props, &["RawData"])? {
        Property::Struct(StructValue::Game(crate::ue::PalStruct::CharacterData(data))) => Some(data),
        _ => None,
    }
}

pub fn entry_character_data_mut(entry: &mut MapEntry) -> Option<&mut PalCharacterData<crate::ue::Arch>> {
    let value_props = props::struct_props_mut(&mut entry.value)?;
    match props::get_mut(value_props, &["RawData"])? {
        Property::Struct(StructValue::Game(crate::ue::PalStruct::CharacterData(data))) => Some(data),
        _ => None,
    }
}

/// The property bag every pal/player field (nickname, level, stats, ...)
/// lives under.
pub fn entry_save_parameter(entry: &MapEntry) -> Option<&Properties> {
    let data = entry_character_data(entry)?;
    props::get(&data.object, &["SaveParameter"]).and_then(props::struct_props)
}

pub fn entry_save_parameter_mut(entry: &mut MapEntry) -> Option<&mut Properties> {
    let data = entry_character_data_mut(entry)?;
    props::get_mut(&mut data.object, &["SaveParameter"]).and_then(props::struct_props_mut)
}

/// `SaveParameter.IsPlayer` -- `false` when absent.
pub fn entry_is_player(entry: &MapEntry) -> bool {
    entry_save_parameter(entry)
        .and_then(|parameters| props::get(parameters, &["IsPlayer"]))
        .and_then(props::as_bool)
        .unwrap_or(false)
}

// Index builders each return a fresh map; `SaveSession` caches them behind
// `WorldCaches` and invalidates on every character/container-map mutation.

pub fn build_character_index(level: &Save) -> std::collections::HashMap<uuid::Uuid, usize> {
    let mut index = std::collections::HashMap::new();
    if let Ok(entries) = character_map(level) {
        for (position, entry) in entries.iter().enumerate() {
            if let Some(instance_id) = entry_instance_id(entry) {
                index.insert(instance_id, position);
            }
        }
    }
    index
}

/// `ItemContainerSaveData` and `CharacterContainerSaveData` both key by
/// `key.ID`.
fn container_key_id(entry: &MapEntry) -> Option<uuid::Uuid> {
    props::get(props::struct_props(&entry.key)?, &["ID"]).and_then(props::as_uuid)
}

pub fn build_item_container_index(level: &Save) -> std::collections::HashMap<uuid::Uuid, usize> {
    let mut index = std::collections::HashMap::new();
    if let Ok(entries) = item_container_map(level) {
        for (position, entry) in entries.iter().enumerate() {
            if let Some(container_id) = container_key_id(entry) {
                index.insert(container_id, position);
            }
        }
    }
    index
}

pub fn build_character_container_index(
    level: &Save,
) -> std::collections::HashMap<uuid::Uuid, usize> {
    let mut index = std::collections::HashMap::new();
    if let Ok(entries) = character_container_map(level) {
        for (position, entry) in entries.iter().enumerate() {
            if let Some(container_id) = container_key_id(entry) {
                index.insert(container_id, position);
            }
        }
    }
    index
}

/// Dynamic items are an array of structs whose `RawData` is `PalDynamicItem`,
/// keyed by `RawData.id.local_id_in_created_world`.
pub fn build_dynamic_item_index(level: &Save) -> std::collections::HashMap<uuid::Uuid, usize> {
    let mut index = std::collections::HashMap::new();
    if let Ok(values) = dynamic_item_values(level) {
        for (position, value) in values.iter().enumerate() {
            let StructValue::Struct(item_props) = value else {
                continue;
            };
            let Some(Property::Struct(StructValue::Game(crate::ue::PalStruct::DynamicItem(dynamic_item)))) =
                props::get(item_props, &["RawData"])
            else {
                continue;
            };
            index.insert(
                props::guid_to_uuid(&dynamic_item.id.local_id_in_created_world),
                position,
            );
        }
    }
    index
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ue::games::palworld::{PalDynamicId, PalDynamicItem, PalDynamicItemType};
    use crate::ue::{Header, PackageVersion, PropertySchemas, Root, ValueVec};

    fn minimal_save(properties: Properties) -> Save {
        Save {
            header: Header {
                magic: 0,
                save_game_version: 0,
                package_version: PackageVersion { ue4: 0, ue5: None },
                engine_version_major: 0,
                engine_version_minor: 0,
                engine_version_patch: 0,
                engine_version_build: 0,
                engine_version: String::new(),
                custom_version: None,
            },
            schemas: PropertySchemas::default(),
            root: Root {
                save_game_type: String::new(),
                properties,
            },
            extra: Vec::new(),
        }
    }

    fn struct_property(properties: Properties) -> Property {
        Property::Struct(StructValue::Struct(properties))
    }

    fn world_save(world_save_data: Properties) -> Save {
        let mut root_properties = Properties::default();
        root_properties.insert("worldSaveData", struct_property(world_save_data));
        minimal_save(root_properties)
    }

    fn guid_property(value: uuid::Uuid) -> Property {
        Property::Struct(StructValue::Guid(props::uuid_to_guid(value)))
    }

    #[test]
    fn map_object_values_absent_returns_ok_none() {
        // A world that has never had a map object placed.
        let save = world_save(Properties::default());
        assert!(
            map_object_values(&save).unwrap().is_none(),
            "an absent MapObjectSaveData must not hard-fail"
        );
    }

    #[test]
    fn map_object_values_mut_absent_returns_ok_none() {
        let mut save = world_save(Properties::default());
        assert!(map_object_values_mut(&mut save).unwrap().is_none());
    }

    #[test]
    fn map_object_values_present_returns_its_entries() {
        let mut world_save_data = Properties::default();
        world_save_data.insert(
            "MapObjectSaveData",
            Property::Array(ValueVec::Struct(vec![
                StructValue::Struct(Properties::default()),
                StructValue::Struct(Properties::default()),
            ])),
        );
        let save = world_save(world_save_data);

        let entries = map_object_values(&save)
            .unwrap()
            .expect("MapObjectSaveData is present");
        assert_eq!(2, entries.len());
    }

    #[test]
    fn map_object_values_mut_present_supports_mutation() {
        let mut world_save_data = Properties::default();
        world_save_data.insert(
            "MapObjectSaveData",
            Property::Array(ValueVec::Struct(vec![StructValue::Struct(
                Properties::default(),
            )])),
        );
        let mut save = world_save(world_save_data);

        map_object_values_mut(&mut save)
            .unwrap()
            .expect("present")
            .push(StructValue::Struct(Properties::default()));

        assert_eq!(2, map_object_values(&save).unwrap().unwrap().len());
    }

    #[test]
    fn build_character_container_index_maps_key_id_to_position() {
        let first_id = uuid::Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let second_id = uuid::Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();

        let mut key_one = Properties::default();
        key_one.insert("ID", guid_property(first_id));
        let mut key_two = Properties::default();
        key_two.insert("ID", guid_property(second_id));

        let mut world_save_data = Properties::default();
        world_save_data.insert(
            "CharacterContainerSaveData",
            Property::Map(vec![
                MapEntry {
                    key: struct_property(key_one),
                    value: Property::Bool(true),
                },
                MapEntry {
                    key: struct_property(key_two),
                    value: Property::Bool(true),
                },
            ]),
        );
        let save = world_save(world_save_data);

        let index = build_character_container_index(&save);
        assert_eq!(2, index.len());
        assert_eq!(Some(&0), index.get(&first_id));
        assert_eq!(Some(&1), index.get(&second_id));
    }

    #[test]
    fn build_character_container_index_skips_entries_with_unresolvable_key() {
        let mut world_save_data = Properties::default();
        world_save_data.insert(
            "CharacterContainerSaveData",
            Property::Map(vec![MapEntry {
                key: Property::Bool(true), // not a struct with an "ID" field
                value: Property::Bool(true),
            }]),
        );
        let save = world_save(world_save_data);

        assert!(build_character_container_index(&save).is_empty());
    }

    // The real-save half of this coverage lives in tests/world_index.rs.

    fn dynamic_item_struct_value(local_id: uuid::Uuid) -> StructValue {
        let dynamic_item = PalDynamicItem {
            id: PalDynamicId {
                created_world_id: crate::ue::FGuid::nil(),
                local_id_in_created_world: props::uuid_to_guid(local_id),
            },
            static_id: "Weapon_Test".to_string(),
            item_type: PalDynamicItemType::Unknown {
                trailer: Vec::new(),
            },
        };
        let mut item_props = Properties::default();
        item_props.insert(
            "RawData",
            Property::Struct(StructValue::Game(crate::ue::PalStruct::DynamicItem(Box::new(dynamic_item)))),
        );
        StructValue::Struct(item_props)
    }

    #[test]
    fn build_dynamic_item_index_keys_by_local_id_in_created_world() {
        let local_id = uuid::Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap();

        let mut world_save_data = Properties::default();
        world_save_data.insert(
            "DynamicItemSaveData",
            Property::Array(ValueVec::Struct(vec![dynamic_item_struct_value(local_id)])),
        );
        let save = world_save(world_save_data);

        let index = build_dynamic_item_index(&save);
        assert_eq!(1, index.len());
        assert_eq!(Some(&0), index.get(&local_id));
    }

    #[test]
    fn build_dynamic_item_index_skips_non_struct_and_missing_raw_data_entries() {
        let mut world_save_data = Properties::default();
        world_save_data.insert(
            "DynamicItemSaveData",
            Property::Array(ValueVec::Struct(vec![
                StructValue::Guid(crate::ue::FGuid::nil()), // not StructValue::Struct at all
                StructValue::Struct(Properties::default()), // Struct, but no "RawData"
            ])),
        );
        let save = world_save(world_save_data);

        assert!(build_dynamic_item_index(&save).is_empty());
    }

    #[test]
    fn set_entry_player_uid_overwrites_existing_key_field() {
        let old_uid = uuid::Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let new_uid = uuid::Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();

        let mut key_props = Properties::default();
        key_props.insert("PlayerUId", guid_property(old_uid));
        key_props.insert("InstanceId", guid_property(uuid::Uuid::nil()));
        let mut entry = MapEntry {
            key: struct_property(key_props),
            value: Property::Bool(true),
        };

        set_entry_player_uid(&mut entry, new_uid);

        assert_eq!(entry_player_uid(&entry), Some(new_uid));
        // The sibling field must survive the overwrite untouched.
        assert_eq!(entry_instance_id(&entry), Some(uuid::Uuid::nil()));
    }

    #[test]
    fn set_entry_player_uid_is_a_no_op_on_a_non_struct_key() {
        let mut entry = MapEntry {
            key: Property::Bool(true),
            value: Property::Bool(true),
        };

        set_entry_player_uid(&mut entry, uuid::Uuid::nil());

        assert!(matches!(entry.key, Property::Bool(true)));
    }
}
