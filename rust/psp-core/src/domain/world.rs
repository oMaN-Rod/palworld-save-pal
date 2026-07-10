//! World-tree navigation — port of `IndexingMixin` (`game/mixins/indexing.py`)
//! plus the handful of `worldSaveData` accessors the Python `SaveManager`
//! properties (`_character_save_parameter_map`, `_item_container_save_data`,
//! ...) wrap. Every accessor here takes `&uesave::Save`/`&mut uesave::Save`
//! directly (not `&SaveSession`) so Phase 2's DTO/CRUD code (Tasks 4-11) can
//! navigate a player's or a source-transfer save's tree the same way it
//! navigates `session.level`.
//!
//! Deviation from the brief: `base_camp_map`/`guild_extra_map`/
//! `map_object_values` (and their `_mut` counterparts) return
//! `Result<Option<&Vec<_>>, CoreError>`, not the brief's uniform
//! `Result<&Vec<_>, CoreError>`. The brief applied the same "required" macro
//! to all six/eight maps and struct arrays uniformly, but Python guards
//! `BaseCampSaveData`/`GuildExtraSaveDataMap`/`MapObjectSaveData`
//! specifically with `"... in world_save_data"` (`save_manager.py`'s
//! `_set_data`):
//!
//! ```python
//! self._map_object_save_data = PalObjects.get_value(
//!     world_save_data["MapObjectSaveData"]
//!     if "MapObjectSaveData" in world_save_data
//!     else None
//! )
//! ```
//!
//! All three are genuinely absent from any save that has never placed the
//! corresponding data (no base built, no guild extras, no map objects
//! placed), which is normal, common save data, not a malformed save. Phase
//! 1's `SaveSession::base_camp_map`/`guild_extra_map` already encode exactly
//! this optionality (see `session.rs`); treating any of the three absences
//! as a hard `Err` here would regress that and break every operation that
//! touches one of these on a save that has never populated it. `Err` is
//! still reserved for a genuine structural problem (a missing/malformed
//! `worldSaveData` itself, or a present-but-wrong-typed value); `Ok(None)` is
//! "this specific optional field isn't present," matching Python's own `if
//! "..." in world_save_data` guard exactly. `DynamicItemSaveData` is NOT
//! given this treatment -- Python's `_set_data` indexes it unconditionally
//! (`world_save_data["DynamicItemSaveData"]`, no `in` guard), so
//! `dynamic_item_values`/`_mut` stay on the required macro.
//!
//! Every lookup here goes through `props::get`/`props::get_mut` (name-only
//! matching) rather than indexing `Properties`' underlying `IndexMap`
//! directly by a hand-built `PropertyKey`. A `PropertyKey` is `(u32, String)`
//! -- the `u32` disambiguates same-named sibling properties -- and nothing
//! about a top-level `worldSaveData` field name guarantees that index is
//! always `0`. `props::get` already established the robust convention
//! (linear scan matching the name only) for every Phase-1 read path; this
//! module stays consistent with it instead of introducing a second, stricter
//! lookup rule that could resolve a different node than the rest of the
//! codebase would for the same path.

use crate::error::CoreError;
use crate::props;
use uesave::games::palworld::PalCharacterData;
use uesave::{MapEntry, Properties, Property, Save, StructValue};

/// `Level.sav`'s `worldSaveData` struct — the root of every map/array this
/// module navigates.
pub fn world_props(level: &Save) -> Result<&Properties, CoreError> {
    props::get(&level.root.properties, &["worldSaveData"])
        .and_then(props::struct_props)
        .ok_or_else(|| CoreError::Parse("worldSaveData missing from Level.sav".to_string()))
}

/// Mutable counterpart of `world_props`.
pub fn world_props_mut(level: &mut Save) -> Result<&mut Properties, CoreError> {
    props::get_mut(&mut level.root.properties, &["worldSaveData"])
        .and_then(props::struct_props_mut)
        .ok_or_else(|| CoreError::Parse("worldSaveData missing from Level.sav".to_string()))
}

/// A named map directly under `worldSaveData` that every real save is
/// expected to carry (Python's `_set_data` indexes it unconditionally,
/// raising `KeyError` on absence).
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

/// A named map directly under `worldSaveData` that a real save may
/// legitimately not carry (Python guards with `"... in world_save_data"`):
/// `Ok(None)` for "not present", `Err` only for a structurally broken
/// `worldSaveData` or a present-but-wrong-typed value.
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

/// A named struct array directly under `worldSaveData` that a real save may
/// legitimately not carry (Python guards with `"... in world_save_data"`):
/// `Ok(None)` for "not present", `Err` only for a structurally broken
/// `worldSaveData` or a present-but-wrong-typed value. Mirrors
/// `world_optional_map_accessors!` above, for struct-array-shaped fields
/// instead of map-shaped ones.
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

// ---- character-map entry helpers ----

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

/// `entry.value.RawData`, decoded as `PalCharacterData` -- the typed struct
/// backing every character-map entry (player or pal). `None` for anything
/// that isn't shaped this way, matching `domain::summaries::save_parameter`'s
/// same non-panicking guard on untrusted save data.
pub fn entry_character_data(entry: &MapEntry) -> Option<&PalCharacterData> {
    let value_props = props::struct_props(&entry.value)?;
    match props::get(value_props, &["RawData"])? {
        Property::Struct(StructValue::PalCharacterData(data)) => Some(data),
        _ => None,
    }
}

/// Mutable counterpart of `entry_character_data`.
pub fn entry_character_data_mut(entry: &mut MapEntry) -> Option<&mut PalCharacterData> {
    let value_props = props::struct_props_mut(&mut entry.value)?;
    match props::get_mut(value_props, &["RawData"])? {
        Property::Struct(StructValue::PalCharacterData(data)) => Some(data),
        _ => None,
    }
}

/// `PalCharacterData.object`'s one `"SaveParameter"` struct property -- the
/// property bag every pal/player field (nickname, level, stats, ...) lives
/// under.
pub fn entry_save_parameter(entry: &MapEntry) -> Option<&Properties> {
    let data = entry_character_data(entry)?;
    props::get(&data.object, &["SaveParameter"]).and_then(props::struct_props)
}

/// Mutable counterpart of `entry_save_parameter`.
pub fn entry_save_parameter_mut(entry: &mut MapEntry) -> Option<&mut Properties> {
    let data = entry_character_data_mut(entry)?;
    props::get_mut(&mut data.object, &["SaveParameter"]).and_then(props::struct_props_mut)
}

/// `SaveParameter.IsPlayer` -- `false` when absent, matching every other
/// `IsPlayer` read in this port (`domain::summaries::is_player_entry`).
pub fn entry_is_player(entry: &MapEntry) -> bool {
    entry_save_parameter(entry)
        .and_then(|parameters| props::get(parameters, &["IsPlayer"]))
        .and_then(props::as_bool)
        .unwrap_or(false)
}

// ---- index builders (each returns a fresh map; `SaveSession` caches them
// behind `WorldCaches`, invalidated on every character/container-map
// mutation -- see `session.rs`'s `invalidate_performance_caches`) ----

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

/// A container map entry's `key.ID` -- shared by item- and
/// character-container indexing (`ItemContainerSaveData`/
/// `CharacterContainerSaveData` both key this way).
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

/// Dynamic items are an array of structs whose `RawData` is `PalDynamicItem`;
/// key = `RawData.id.local_id_in_created_world` (`indexing.py`'s
/// `_build_dynamic_items_collection` key extractor).
pub fn build_dynamic_item_index(level: &Save) -> std::collections::HashMap<uuid::Uuid, usize> {
    let mut index = std::collections::HashMap::new();
    if let Ok(values) = dynamic_item_values(level) {
        for (position, value) in values.iter().enumerate() {
            let StructValue::Struct(item_props) = value else {
                continue;
            };
            let Some(Property::Struct(StructValue::PalDynamicItem(dynamic_item))) =
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
    use uesave::games::palworld::{PalDynamicId, PalDynamicItem, PalDynamicItemType};
    use uesave::{Header, PackageVersion, PropertySchemas, Root, ValueVec};

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

    /// A `Save` whose `worldSaveData` is `world_save_data` -- enough to
    /// exercise every accessor/index builder in this module, which all
    /// start from `world_props`/`world_props_mut`.
    fn world_save(world_save_data: Properties) -> Save {
        let mut root_properties = Properties::default();
        root_properties.insert("worldSaveData", struct_property(world_save_data));
        minimal_save(root_properties)
    }

    fn guid_property(value: uuid::Uuid) -> Property {
        Property::Struct(StructValue::Guid(props::uuid_to_guid(value)))
    }

    // ---- map_object_values optionality (MapObjectSaveData is guarded by
    // Python's `"MapObjectSaveData" in world_save_data`, same as
    // base_camp_map/guild_extra_map -- see this file's top-level doc
    // comment) ----

    #[test]
    fn map_object_values_absent_returns_ok_none() {
        // worldSaveData is present but carries no "MapObjectSaveData" key --
        // a young world that has never had a map object placed.
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

    // ---- build_character_container_index (never exercised by any prior
    // test -- Important 2 of the follow-up review) ----

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

    // ---- build_dynamic_item_index (never exercised by any prior test, and
    // the `PalDynamicItem.id.local_id_in_created_world` field path was
    // unverified against real save data -- Important 2 of the follow-up
    // review; the real-save half of this coverage lives in
    // tests/world_index.rs) ----

    fn dynamic_item_struct_value(local_id: uuid::Uuid) -> StructValue {
        let dynamic_item = PalDynamicItem {
            id: PalDynamicId {
                created_world_id: uesave::FGuid::nil(),
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
            Property::Struct(StructValue::PalDynamicItem(Box::new(dynamic_item))),
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
                StructValue::Guid(uesave::FGuid::nil()), // not StructValue::Struct at all
                StructValue::Struct(Properties::default()), // Struct, but no "RawData"
            ])),
        );
        let save = world_save(world_save_data);

        assert!(build_dynamic_item_index(&save).is_empty());
    }
}
