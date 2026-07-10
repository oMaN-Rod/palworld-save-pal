//! Character-container slot operations — port of `game/character_container.py`
//! (`CharacterContainer.add_pal`/`remove_pal`). Item-container slot
//! operations are Task 10 scope, not this module's.
//!
//! Neither function here inserts or removes a `CharacterContainerSaveData`
//! `MapEntry` — both only mutate the `Slots` array *nested inside* an
//! already-positioned entry (found by `entry_index`, supplied by the
//! caller). `SaveSession::character_container_index`/`caches.
//! character_container_index` map container id → **entry position**, and
//! neither function changes any entry's position or the map's length, so
//! neither needs to call `invalidate_performance_caches` — see this task's
//! report for the empirical proof (`build_character_container_index`
//! resolves the same position before and after `character_container_add_pal`).

use crate::dto::container::CharacterContainerSlotDto;
use crate::error::CoreError;
use crate::props;
use uesave::{Properties, Property, PropertyKey, StructValue};

use super::world;

/// `game/character_container.py::CharacterContainer` — not a wire type (no
/// DTO counterpart lives here; `dto::container::CharacterContainerDto` is
/// the response shape a higher-level Task assembles from this).
pub struct CharacterContainerView {
    pub container_id: uuid::Uuid,
    pub size: i32,
    pub slots: Vec<CharacterContainerSlotDto>,
}

fn container_value_props(level: &uesave::Save, entry_index: usize) -> Option<&Properties> {
    let entries = world::character_container_map(level).ok()?;
    props::struct_props(&entries.get(entry_index)?.value)
}

/// Port of `CharacterContainer._load_from_container_data`
/// (`game/character_container.py`): `size` from `SlotNum`, one
/// `CharacterContainerSlot` per `Slots` element (`slot_index` +
/// `RawData.instance_id`, decoded as a `PalCharacterContainer`).
pub fn read_character_container(
    level: &uesave::Save,
    entry_index: usize,
) -> Option<CharacterContainerView> {
    let entries = world::character_container_map(level).ok()?;
    let entry = entries.get(entry_index)?;
    let key_props = props::struct_props(&entry.key)?;
    let container_id = props::get(key_props, &["ID"]).and_then(props::as_uuid)?;

    let value_props = container_value_props(level, entry_index)?;
    let size = props::get(value_props, &["SlotNum"])
        .and_then(props::as_i32)
        .unwrap_or(0);

    let mut slots = Vec::new();
    if let Some(slot_values) = props::get(value_props, &["Slots"]).and_then(props::struct_values) {
        for slot_value in slot_values {
            let StructValue::Struct(slot_props) = slot_value else {
                continue;
            };
            let slot_index = slot_props
                .0
                .get(&PropertyKey::from("SlotIndex"))
                .and_then(props::as_i32)
                .unwrap_or(0);
            let pal_id = match slot_props.0.get(&PropertyKey::from("RawData")) {
                Some(Property::Struct(StructValue::PalCharacterContainer(raw))) => {
                    let id = props::guid_to_uuid(&raw.instance_id);
                    (id != props::EMPTY_UUID).then_some(id)
                }
                _ => None,
            };
            slots.push(CharacterContainerSlotDto { slot_index, pal_id });
        }
    }
    Some(CharacterContainerView {
        container_id,
        size,
        slots,
    })
}

/// Port of `CharacterContainer.add_pal` (`game/character_container.py`):
/// `None` when the container is full (`len(slots) >= size`, matching
/// `available_slots()`); otherwise appends a new `Slots` element at
/// `requested_slot` (or the first free index, matching
/// `find_first_available_slot`) and returns the assigned index.
///
/// Does not insert/remove a `CharacterContainerSaveData` entry — see this
/// module's doc comment on cache invalidation.
pub fn character_container_add_pal(
    level: &mut uesave::Save,
    entry_index: usize,
    pal_id: uuid::Uuid,
    requested_slot: Option<i32>,
) -> Result<Option<i32>, CoreError> {
    let view = read_character_container(level, entry_index)
        .ok_or_else(|| CoreError::Parse("character container unreadable".into()))?;
    if view.slots.len() as i32 >= view.size {
        return Ok(None);
    }
    let used: std::collections::HashSet<i32> =
        view.slots.iter().map(|slot| slot.slot_index).collect();
    let assigned = requested_slot.unwrap_or_else(|| {
        (0..view.size)
            .find(|candidate| !used.contains(candidate))
            .unwrap_or(0)
    });

    let entries = world::character_container_map_mut(level)?;
    let entry = entries
        .get_mut(entry_index)
        .ok_or_else(|| CoreError::Parse("container entry index out of range".into()))?;
    let value_props = props::struct_props_mut(&mut entry.value)
        .ok_or_else(|| CoreError::Parse("container value not a struct".into()))?;
    let slot_values = props::get_mut(value_props, &["Slots"])
        .and_then(props::struct_values_mut)
        .ok_or_else(|| CoreError::Parse("container Slots missing".into()))?;

    let mut slot_props = Properties::default();
    slot_props.insert("SlotIndex", props::int_property(assigned));
    slot_props.insert(
        "RawData",
        Property::Struct(StructValue::PalCharacterContainer(
            uesave::games::palworld::PalCharacterContainer {
                player_uid: props::uuid_to_guid(props::EMPTY_UUID),
                instance_id: props::uuid_to_guid(pal_id),
                permission_tribe_id: 0,
                trailing_bytes: None,
            },
        )),
    );
    slot_values.push(StructValue::Struct(slot_props));
    Ok(Some(assigned))
}

/// Port of `CharacterContainer.remove_pal`/`_delete_slot_data`
/// (`game/character_container.py`): removes the first slot whose
/// `instance_id` matches `pal_id` (Python's own loop `break`s after the
/// first match too). A `pal_id` that isn't present is a silent no-op,
/// matching Python (the `for` loop simply never matches).
pub fn character_container_remove_pal(
    level: &mut uesave::Save,
    entry_index: usize,
    pal_id: uuid::Uuid,
) -> Result<(), CoreError> {
    let entries = world::character_container_map_mut(level)?;
    let Some(entry) = entries.get_mut(entry_index) else {
        return Ok(());
    };
    let Some(value_props) = props::struct_props_mut(&mut entry.value) else {
        return Ok(());
    };
    let Some(slot_values) =
        props::get_mut(value_props, &["Slots"]).and_then(props::struct_values_mut)
    else {
        return Ok(());
    };

    if let Some(position) = slot_values.iter().position(|slot_value| {
        let StructValue::Struct(slot_props) = slot_value else {
            return false;
        };
        matches!(
            slot_props.0.get(&PropertyKey::from("RawData")),
            Some(Property::Struct(StructValue::PalCharacterContainer(raw)))
                if props::guid_to_uuid(&raw.instance_id) == pal_id
        )
    }) {
        slot_values.remove(position);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uesave::{Header, PackageVersion, PropertySchemas, Root, Save, ValueVec};

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

    fn guid_property(value: uuid::Uuid) -> Property {
        Property::Struct(StructValue::Guid(props::uuid_to_guid(value)))
    }

    fn container_slot(slot_index: i32, pal_id: uuid::Uuid) -> StructValue {
        let mut slot_props = Properties::default();
        slot_props.insert("SlotIndex", props::int_property(slot_index));
        slot_props.insert(
            "RawData",
            Property::Struct(StructValue::PalCharacterContainer(
                uesave::games::palworld::PalCharacterContainer {
                    player_uid: props::uuid_to_guid(props::EMPTY_UUID),
                    instance_id: props::uuid_to_guid(pal_id),
                    permission_tribe_id: 0,
                    trailing_bytes: None,
                },
            )),
        );
        StructValue::Struct(slot_props)
    }

    /// A one-container `Save` whose `CharacterContainerSaveData` has `size`
    /// slots and the given pre-existing occupants — enough to exercise every
    /// function in this module without a real save file on disk.
    fn save_with_one_container(
        container_id: uuid::Uuid,
        size: i32,
        occupants: Vec<(i32, uuid::Uuid)>,
    ) -> Save {
        let mut key_props = Properties::default();
        key_props.insert("ID", guid_property(container_id));

        let mut value_props = Properties::default();
        value_props.insert("SlotNum", props::int_property(size));
        let slots: Vec<StructValue> = occupants
            .into_iter()
            .map(|(slot_index, pal_id)| container_slot(slot_index, pal_id))
            .collect();
        value_props.insert("Slots", Property::Array(ValueVec::Struct(slots)));

        let mut world_save_data = Properties::default();
        world_save_data.insert(
            "CharacterContainerSaveData",
            Property::Map(vec![uesave::MapEntry {
                key: struct_property(key_props),
                value: struct_property(value_props),
            }]),
        );
        let mut root_properties = Properties::default();
        root_properties.insert("worldSaveData", struct_property(world_save_data));
        minimal_save(root_properties)
    }

    #[test]
    fn read_character_container_decodes_size_and_slots() {
        let container_id = uuid::Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap();
        let pal_id = uuid::Uuid::parse_str("aaaaaaaa-2222-3333-4444-555555555555").unwrap();
        let save = save_with_one_container(container_id, 5, vec![(2, pal_id)]);

        let view = read_character_container(&save, 0).expect("readable");
        assert_eq!(view.container_id, container_id);
        assert_eq!(view.size, 5);
        assert_eq!(view.slots.len(), 1);
        assert_eq!(view.slots[0].slot_index, 2);
        assert_eq!(view.slots[0].pal_id, Some(pal_id));
    }

    #[test]
    fn read_character_container_returns_none_for_missing_entry_index() {
        let container_id = uuid::Uuid::nil();
        let save = save_with_one_container(container_id, 5, vec![]);
        assert!(read_character_container(&save, 99).is_none());
    }

    #[test]
    fn add_pal_assigns_the_first_free_slot_when_no_slot_requested() {
        let container_id = uuid::Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap();
        let existing_pal = uuid::Uuid::parse_str("aaaaaaaa-2222-3333-4444-555555555555").unwrap();
        let mut save = save_with_one_container(container_id, 3, vec![(0, existing_pal)]);

        let new_pal = uuid::Uuid::parse_str("bbbbbbbb-2222-3333-4444-555555555555").unwrap();
        let assigned = character_container_add_pal(&mut save, 0, new_pal, None).unwrap();

        assert_eq!(
            assigned,
            Some(1),
            "slot 0 taken, slot 1 is the first free index"
        );
        let view = read_character_container(&save, 0).unwrap();
        assert_eq!(view.slots.len(), 2);
        assert!(view
            .slots
            .iter()
            .any(|slot| slot.pal_id == Some(new_pal) && slot.slot_index == 1));
    }

    #[test]
    fn add_pal_honors_an_explicit_requested_slot() {
        let container_id = uuid::Uuid::nil();
        let mut save = save_with_one_container(container_id, 5, vec![]);
        let pal_id = uuid::Uuid::parse_str("cccccccc-2222-3333-4444-555555555555").unwrap();

        let assigned = character_container_add_pal(&mut save, 0, pal_id, Some(4)).unwrap();

        assert_eq!(assigned, Some(4));
        let view = read_character_container(&save, 0).unwrap();
        assert_eq!(view.slots[0].slot_index, 4);
    }

    #[test]
    fn add_pal_returns_none_when_the_container_is_full() {
        let container_id = uuid::Uuid::nil();
        let occupant_a = uuid::Uuid::parse_str("aaaaaaaa-0000-0000-0000-000000000000").unwrap();
        let occupant_b = uuid::Uuid::parse_str("bbbbbbbb-0000-0000-0000-000000000000").unwrap();
        let mut save =
            save_with_one_container(container_id, 2, vec![(0, occupant_a), (1, occupant_b)]);

        let new_pal = uuid::Uuid::parse_str("cccccccc-0000-0000-0000-000000000000").unwrap();
        let assigned = character_container_add_pal(&mut save, 0, new_pal, None).unwrap();

        assert_eq!(
            assigned, None,
            "size == occupied slots: full container returns None"
        );
        assert_eq!(
            read_character_container(&save, 0).unwrap().slots.len(),
            2,
            "a full container must not gain a slot"
        );
    }

    #[test]
    fn remove_pal_removes_only_the_first_matching_slot() {
        let container_id = uuid::Uuid::nil();
        let pal_id = uuid::Uuid::parse_str("dddddddd-0000-0000-0000-000000000000").unwrap();
        let other_pal = uuid::Uuid::parse_str("eeeeeeee-0000-0000-0000-000000000000").unwrap();
        let mut save = save_with_one_container(container_id, 5, vec![(0, pal_id), (1, other_pal)]);

        character_container_remove_pal(&mut save, 0, pal_id).unwrap();

        let view = read_character_container(&save, 0).unwrap();
        assert_eq!(view.slots.len(), 1);
        assert_eq!(view.slots[0].pal_id, Some(other_pal));
    }

    #[test]
    fn remove_pal_absent_pal_id_is_a_silent_no_op() {
        let container_id = uuid::Uuid::nil();
        let pal_id = uuid::Uuid::parse_str("dddddddd-0000-0000-0000-000000000000").unwrap();
        let mut save = save_with_one_container(container_id, 5, vec![(0, pal_id)]);

        let absent = uuid::Uuid::parse_str("ffffffff-0000-0000-0000-000000000000").unwrap();
        character_container_remove_pal(&mut save, 0, absent).unwrap();

        assert_eq!(read_character_container(&save, 0).unwrap().slots.len(), 1);
    }

    /// Direct proof that `character_container_add_pal`/`remove_pal` never
    /// change *which position* a container lives at in
    /// `CharacterContainerSaveData` -- the fact this module's doc comment
    /// relies on to justify never calling `invalidate_performance_caches`.
    /// Two containers; mutate the first one's slots repeatedly; the second
    /// container's index-built position must never move.
    #[test]
    fn container_position_in_the_map_is_stable_across_add_and_remove() {
        let first_id = uuid::Uuid::parse_str("11111111-0000-0000-0000-000000000000").unwrap();
        let second_id = uuid::Uuid::parse_str("22222222-0000-0000-0000-000000000000").unwrap();

        let mut first_key = Properties::default();
        first_key.insert("ID", guid_property(first_id));
        let mut first_value = Properties::default();
        first_value.insert("SlotNum", props::int_property(5));
        first_value.insert("Slots", Property::Array(ValueVec::Struct(vec![])));

        let mut second_key = Properties::default();
        second_key.insert("ID", guid_property(second_id));
        let mut second_value = Properties::default();
        second_value.insert("SlotNum", props::int_property(5));
        second_value.insert("Slots", Property::Array(ValueVec::Struct(vec![])));

        let mut world_save_data = Properties::default();
        world_save_data.insert(
            "CharacterContainerSaveData",
            Property::Map(vec![
                uesave::MapEntry {
                    key: struct_property(first_key),
                    value: struct_property(first_value),
                },
                uesave::MapEntry {
                    key: struct_property(second_key),
                    value: struct_property(second_value),
                },
            ]),
        );
        let mut root_properties = Properties::default();
        root_properties.insert("worldSaveData", struct_property(world_save_data));
        let mut save = minimal_save(root_properties);

        let before = world::build_character_container_index(&save);
        assert_eq!(before.get(&second_id), Some(&1));

        let pal_id = uuid::Uuid::parse_str("33333333-0000-0000-0000-000000000000").unwrap();
        character_container_add_pal(&mut save, 0, pal_id, None).unwrap();
        character_container_remove_pal(&mut save, 0, pal_id).unwrap();

        let after = world::build_character_container_index(&save);
        assert_eq!(
            after.get(&second_id),
            Some(&1),
            "mutating container 0's Slots must never move container 1's position"
        );
    }
}
