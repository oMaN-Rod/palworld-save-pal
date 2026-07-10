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

use crate::dto::container::{
    CharacterContainerSlotDto, DynamicItemDto, ItemContainerDto, ItemContainerSlotDto,
};
use crate::error::CoreError;
use crate::gamedata::GameData;
use crate::props;
use crate::session::WorldCaches;
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

/// `game/item_container.py::ItemContainer`'s read side (`_get_items`) plus
/// `game/item_container_slot.py`'s per-slot getters. Slot `RawData` is a
/// typed `PalItemContainerSlots` (`../uesave-rs/uesave/src/games/palworld/
/// items.rs`: `PalItemContainerSlot { slot_index, count, item: PalItemId,
/// trailing_bytes }`, `PalItemId { static_id, dynamic_id: PalDynamicId }`,
/// `PalDynamicId { created_world_id, local_id_in_created_world }`) --
/// verified against `item_container_slot.py`'s own field paths
/// (`item.static_id`, `item.dynamic_id.local_id_in_created_world`), which
/// address the exact same nesting despite Python's dict-based decoder
/// grouping fields differently at the top level (see `read_dynamic_item`'s
/// doc comment for the analogous, more surprising case). `game_data` is
/// needed only for the Egg dynamic-item branch, which reuses
/// `domain::pal::read_save_parameter_dto` to decode an egg's embedded
/// SaveParameter bag the same way a Level.sav pal is decoded.
pub fn read_item_container(
    level: &uesave::Save,
    session_caches: &mut WorldCaches,
    game_data: &GameData,
    container_id: uuid::Uuid,
    container_type: &str,
    key: Option<String>,
) -> Option<ItemContainerDto> {
    let container_index = session_caches
        .item_container_index
        .get_or_insert_with(|| world::build_item_container_index(level));
    let entry_index = *container_index.get(&container_id)?;
    let entries = world::item_container_map(level).ok()?;
    let entry = entries.get(entry_index)?;
    let value_props = props::struct_props(&entry.value)?;
    let slot_num = props::get(value_props, &["SlotNum"])
        .and_then(props::as_i32)
        .unwrap_or(0);

    let dynamic_index = session_caches
        .dynamic_item_index
        .get_or_insert_with(|| world::build_dynamic_item_index(level));

    let mut slots = Vec::new();
    if let Some(slot_values) = props::get(value_props, &["Slots"]).and_then(props::struct_values) {
        for slot_value in slot_values {
            let StructValue::Struct(slot_props) = slot_value else {
                continue;
            };
            let Some(Property::Struct(StructValue::PalItemContainerSlots(raw_slot))) =
                slot_props.0.get(&PropertyKey::from("RawData"))
            else {
                continue;
            };
            let slot_index = raw_slot.slot_index;
            let count = raw_slot.count;
            let static_id = Some(raw_slot.item.static_id.clone());
            let local_id = {
                let id = props::guid_to_uuid(&raw_slot.item.dynamic_id.local_id_in_created_world);
                (id != props::EMPTY_UUID).then_some(id)
            };
            let dynamic_item = local_id.and_then(|dynamic_local_id| {
                let dynamic_entry_index = *dynamic_index.get(&dynamic_local_id)?;
                read_dynamic_item(level, dynamic_entry_index, dynamic_local_id, game_data)
            });
            // Python drops slots whose dynamic item is missing
            // (item_container.py's `_get_items`: `if not dynamic_item: ...
            // continue`).
            if local_id.is_some() && dynamic_item.is_none() {
                continue;
            }
            slots.push(ItemContainerSlotDto {
                dynamic_item,
                slot_index,
                count,
                static_id,
                local_id,
            });
        }
    }

    Some(ItemContainerDto {
        id: container_id,
        r#type: container_type.to_string(),
        slots,
        key,
        slot_num,
    })
}

/// `game/dynamic_item.py::DynamicItem`'s computed-field dump, applied to an
/// already-resolved `DynamicItemSaveData` entry's typed `PalDynamicItem`.
///
/// Deviation from the brief: the brief's reference code read
/// `dynamic_item.id.static_id` and matched on `dynamic_item.data`. Neither
/// exists on the real `uesave-rs` struct (`items.rs`): `PalDynamicItem` is
/// `{ id: PalDynamicId, static_id: String, item_type: PalDynamicItemType }`
/// -- `static_id` is a SIBLING of `id`, not nested inside it, and the enum
/// field is named `item_type`, not `data`. Python's own dict-based decoder
/// (`palworld_save_tools/rawdata/dynamic_item.py::decode_bytes`) nests
/// `static_id` inside a `data["id"]` dict purely as an artifact of how it
/// groups the three fields it reads consecutively off the wire
/// (`created_world_id`, `local_id_in_created_world`, then `static_id`) --
/// the actual byte layout is identical either way, only the two
/// implementations' in-memory grouping differs. This port follows the real
/// `uesave-rs` struct shape, verified directly against `items.rs`, not the
/// brief's assumption.
///
/// The brief's match arms also omitted `PalDynamicItemType::Unknown`, which
/// does not compile (a non-exhaustive match) against the real 4-variant enum
/// (`Unknown`, `Egg`, `Armor`, `Weapon`). `Unknown` mirrors Python's own
/// `data["type"] = "unknown"` default (`decode_bytes`, before any of the
/// egg/armor/weapon trial-parses succeed) -- every other field stays at its
/// `None` default, matching Python's raw dict carrying no other keys for
/// that shape.
fn read_dynamic_item(
    level: &uesave::Save,
    dynamic_entry_index: usize,
    local_id: uuid::Uuid,
    game_data: &GameData,
) -> Option<DynamicItemDto> {
    let values = world::dynamic_item_values(level).ok()?;
    let StructValue::Struct(item_props) = values.get(dynamic_entry_index)? else {
        return None;
    };
    let Some(Property::Struct(StructValue::PalDynamicItem(dynamic_item))) =
        item_props.0.get(&PropertyKey::from("RawData"))
    else {
        return None;
    };

    let mut dto = DynamicItemDto {
        local_id,
        modified: false,
        character_id: None,
        character_key: None,
        durability: None,
        passive_skill_list: None,
        remaining_bullets: None,
        r#type: None,
        static_id: Some(dynamic_item.static_id.clone()),
        gender: None,
        talent_hp: None,
        talent_shot: None,
        talent_defense: None,
        learned_skills: None,
        active_skills: None,
        passive_skills: None,
    };

    match &dynamic_item.item_type {
        uesave::games::palworld::PalDynamicItemType::Unknown { .. } => {
            dto.r#type = Some("unknown".to_string());
        }
        uesave::games::palworld::PalDynamicItemType::Armor { durability, .. } => {
            dto.r#type = Some("armor".to_string());
            dto.durability = Some(*durability as f64);
        }
        uesave::games::palworld::PalDynamicItemType::Weapon {
            durability,
            remaining_bullets,
            passive_skill_list,
            ..
        } => {
            dto.r#type = Some("weapon".to_string());
            dto.durability = Some(*durability as f64);
            dto.remaining_bullets = Some(*remaining_bullets as i64);
            dto.passive_skill_list = Some(passive_skill_list.clone());
        }
        uesave::games::palworld::PalDynamicItemType::Egg {
            character_id,
            object,
            ..
        } => {
            dto.r#type = Some("egg".to_string());
            dto.character_id = Some(character_id.clone());
            dto.character_key = Some(crate::dto::pal::format_character_key(
                character_id,
                &super::pal::known_pal_keys(game_data),
            ));
            // `_save_parameter` (dynamic_item.py): `PalObjects.get_nested(
            // self._raw_data, "object", "SaveParameter", "value")` -- `object`
            // here is itself a `Properties` bag (Python's
            // `reader.properties_until_end()`), which, when the egg's
            // in-game data was actually populated, carries one
            // "SaveParameter" struct property holding the same shape a
            // Level.sav pal's `SaveParameter` does.
            if let Some(save_parameter) = object
                .0
                .get(&PropertyKey::from("SaveParameter"))
                .and_then(props::struct_props)
            {
                let egg_dto = super::pal::read_save_parameter_dto(
                    save_parameter,
                    props::EMPTY_UUID,
                    false,
                    game_data,
                );
                dto.gender = Some(egg_dto.gender);
                dto.talent_hp = Some(egg_dto.talent_hp);
                dto.talent_shot = Some(egg_dto.talent_shot);
                dto.talent_defense = Some(egg_dto.talent_defense);
                dto.learned_skills = Some(egg_dto.learned_skills);
                dto.active_skills = Some(egg_dto.active_skills);
                dto.passive_skills = Some(egg_dto.passive_skills);
            }
        }
    }
    Some(dto)
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

    // ---- read_item_container / read_dynamic_item (item containers, read
    // side) -- synthetic saves, since world1/world2's own dynamic items are
    // exercised for real by `tests/player_details.rs` instead (weapon/armor
    // via player 8C2F1930's real equipment containers) or -- for Egg, which
    // no fixture player's containers reach -- by the synthetic test below. ----

    fn game_data() -> GameData {
        let json_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/json");
        GameData::load(&json_dir).expect("data dir")
    }

    fn item_container_slot(
        slot_index: i32,
        count: i32,
        static_id: &str,
        local_id: uuid::Uuid,
    ) -> StructValue {
        let mut slot_props = Properties::default();
        slot_props.insert(
            "RawData",
            Property::Struct(StructValue::PalItemContainerSlots(
                uesave::games::palworld::PalItemContainerSlot {
                    slot_index,
                    count,
                    item: uesave::games::palworld::PalItemId {
                        static_id: static_id.to_string(),
                        dynamic_id: uesave::games::palworld::PalDynamicId {
                            created_world_id: uesave::FGuid::nil(),
                            local_id_in_created_world: props::uuid_to_guid(local_id),
                        },
                    },
                    trailing_bytes: Vec::new(),
                },
            )),
        );
        StructValue::Struct(slot_props)
    }

    fn dynamic_item_entry(
        local_id: uuid::Uuid,
        static_id: &str,
        item_type: uesave::games::palworld::PalDynamicItemType,
    ) -> StructValue {
        let dynamic_item = uesave::games::palworld::PalDynamicItem {
            id: uesave::games::palworld::PalDynamicId {
                created_world_id: uesave::FGuid::nil(),
                local_id_in_created_world: props::uuid_to_guid(local_id),
            },
            static_id: static_id.to_string(),
            item_type,
        };
        let mut item_props = Properties::default();
        item_props.insert(
            "RawData",
            Property::Struct(StructValue::PalDynamicItem(Box::new(dynamic_item))),
        );
        StructValue::Struct(item_props)
    }

    /// A `Save` whose `ItemContainerSaveData` has one container (`container_id`,
    /// `slots`) and whose `DynamicItemSaveData` is `dynamic_items` -- enough to
    /// exercise `read_item_container`/`read_dynamic_item` end to end without a
    /// real save file on disk.
    fn save_with_item_container(
        container_id: uuid::Uuid,
        slot_num: i32,
        slots: Vec<StructValue>,
        dynamic_items: Vec<StructValue>,
    ) -> Save {
        let mut key_props = Properties::default();
        key_props.insert("ID", guid_property(container_id));
        let mut value_props = Properties::default();
        value_props.insert("SlotNum", props::int_property(slot_num));
        value_props.insert("Slots", Property::Array(ValueVec::Struct(slots)));

        let mut world_save_data = Properties::default();
        world_save_data.insert(
            "ItemContainerSaveData",
            Property::Map(vec![uesave::MapEntry {
                key: struct_property(key_props),
                value: struct_property(value_props),
            }]),
        );
        world_save_data.insert(
            "DynamicItemSaveData",
            Property::Array(ValueVec::Struct(dynamic_items)),
        );
        let mut root_properties = Properties::default();
        root_properties.insert("worldSaveData", struct_property(world_save_data));
        minimal_save(root_properties)
    }

    #[test]
    fn read_item_container_reads_slot_num_type_and_key() {
        let container_id = uuid::Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap();
        let save = save_with_item_container(container_id, 42, vec![], vec![]);
        let data = game_data();
        let mut caches = WorldCaches::default();

        let dto = read_item_container(
            &save,
            &mut caches,
            &data,
            container_id,
            "CommonContainer",
            Some("SomeKey".to_string()),
        )
        .expect("container resolves");

        assert_eq!(dto.id, container_id);
        assert_eq!(dto.r#type, "CommonContainer");
        assert_eq!(dto.slot_num, 42);
        assert_eq!(dto.key, Some("SomeKey".to_string()));
        assert!(dto.slots.is_empty());
    }

    #[test]
    fn read_item_container_returns_none_for_an_unknown_container_id() {
        let save = save_with_item_container(uuid::Uuid::nil(), 1, vec![], vec![]);
        let data = game_data();
        let mut caches = WorldCaches::default();

        assert!(read_item_container(
            &save,
            &mut caches,
            &data,
            uuid::Uuid::parse_str("99999999-2222-3333-4444-555555555555").unwrap(),
            "CommonContainer",
            None,
        )
        .is_none());
    }

    #[test]
    fn read_item_container_keeps_a_slot_with_no_dynamic_item_reference() {
        let container_id = uuid::Uuid::nil();
        let slot = item_container_slot(0, 5, "Wood", props::EMPTY_UUID);
        let save = save_with_item_container(container_id, 10, vec![slot], vec![]);
        let data = game_data();
        let mut caches = WorldCaches::default();

        let dto = read_item_container(
            &save,
            &mut caches,
            &data,
            container_id,
            "CommonContainer",
            None,
        )
        .unwrap();

        assert_eq!(dto.slots.len(), 1);
        assert_eq!(dto.slots[0].static_id, Some("Wood".to_string()));
        assert_eq!(dto.slots[0].count, 5);
        assert_eq!(dto.slots[0].local_id, None);
        assert!(dto.slots[0].dynamic_item.is_none());
    }

    /// Python drops a slot whose `local_id` is set but no matching
    /// `DynamicItemSaveData` entry exists (`item_container.py::_get_items`:
    /// `if not dynamic_item: ... continue`) -- proven here by a slot whose
    /// `local_id` resolves to nothing in `DynamicItemSaveData`.
    #[test]
    fn read_item_container_drops_a_slot_whose_dynamic_item_is_missing() {
        let container_id = uuid::Uuid::nil();
        let dangling_local_id =
            uuid::Uuid::parse_str("dddddddd-0000-0000-0000-000000000000").unwrap();
        let slot = item_container_slot(0, 1, "SomeWeapon", dangling_local_id);
        let save = save_with_item_container(container_id, 10, vec![slot], vec![]);
        let data = game_data();
        let mut caches = WorldCaches::default();

        let dto = read_item_container(
            &save,
            &mut caches,
            &data,
            container_id,
            "CommonContainer",
            None,
        )
        .unwrap();

        assert!(
            dto.slots.is_empty(),
            "a slot with a dangling dynamic-item reference must be dropped, not kept with dynamic_item: None"
        );
    }

    #[test]
    fn read_item_container_resolves_a_weapon_dynamic_item() {
        let container_id = uuid::Uuid::nil();
        let local_id = uuid::Uuid::parse_str("aaaaaaaa-0000-0000-0000-000000000000").unwrap();
        let slot = item_container_slot(0, 1, "SFBow_5", local_id);
        let weapon = dynamic_item_entry(
            local_id,
            "SFBow_5",
            uesave::games::palworld::PalDynamicItemType::Weapon {
                leading_bytes: [0; 4],
                durability: 80.5,
                remaining_bullets: 12,
                passive_skill_list: vec!["Rare".to_string()],
                trailing_bytes: [0; 4],
            },
        );
        let save = save_with_item_container(container_id, 10, vec![slot], vec![weapon]);
        let data = game_data();
        let mut caches = WorldCaches::default();

        let dto = read_item_container(
            &save,
            &mut caches,
            &data,
            container_id,
            "WeaponLoadOutContainer",
            None,
        )
        .unwrap();

        assert_eq!(dto.slots.len(), 1);
        let item = dto.slots[0].dynamic_item.as_ref().expect("weapon resolves");
        assert_eq!(item.local_id, local_id);
        assert_eq!(item.static_id, Some("SFBow_5".to_string()));
        assert_eq!(item.r#type, Some("weapon".to_string()));
        assert_eq!(item.durability, Some(80.5));
        assert_eq!(item.remaining_bullets, Some(12));
        assert_eq!(item.passive_skill_list, Some(vec!["Rare".to_string()]));
        // Weapon carries none of the egg-only fields.
        assert!(item.character_id.is_none());
        assert!(item.gender.is_none());
    }

    #[test]
    fn read_item_container_resolves_an_armor_dynamic_item() {
        let container_id = uuid::Uuid::nil();
        let local_id = uuid::Uuid::parse_str("bbbbbbbb-0000-0000-0000-000000000000").unwrap();
        let slot = item_container_slot(0, 1, "HeadEquip041", local_id);
        let armor = dynamic_item_entry(
            local_id,
            "HeadEquip041",
            uesave::games::palworld::PalDynamicItemType::Armor {
                leading_bytes: [0; 4],
                durability: 42.0,
                trailing_bytes: [0; 4],
            },
        );
        let save = save_with_item_container(container_id, 10, vec![slot], vec![armor]);
        let data = game_data();
        let mut caches = WorldCaches::default();

        let dto = read_item_container(
            &save,
            &mut caches,
            &data,
            container_id,
            "PlayerEquipArmorContainer",
            None,
        )
        .unwrap();

        let item = dto.slots[0].dynamic_item.as_ref().expect("armor resolves");
        assert_eq!(item.r#type, Some("armor".to_string()));
        assert_eq!(item.durability, Some(42.0));
        assert!(item.remaining_bullets.is_none());
        assert!(item.passive_skill_list.is_none());
    }

    #[test]
    fn read_item_container_resolves_an_unknown_dynamic_item() {
        let container_id = uuid::Uuid::nil();
        let local_id = uuid::Uuid::parse_str("cccccccc-0000-0000-0000-000000000000").unwrap();
        let slot = item_container_slot(0, 1, "Weird", local_id);
        let unknown = dynamic_item_entry(
            local_id,
            "Weird",
            uesave::games::palworld::PalDynamicItemType::Unknown {
                trailer: vec![1, 2, 3],
            },
        );
        let save = save_with_item_container(container_id, 10, vec![slot], vec![unknown]);
        let data = game_data();
        let mut caches = WorldCaches::default();

        let dto = read_item_container(
            &save,
            &mut caches,
            &data,
            container_id,
            "CommonContainer",
            None,
        )
        .unwrap();

        let item = dto.slots[0]
            .dynamic_item
            .as_ref()
            .expect("unknown item still resolves");
        assert_eq!(item.r#type, Some("unknown".to_string()));
        assert_eq!(item.static_id, Some("Weird".to_string()));
        assert!(item.durability.is_none());
    }

    /// No fixture player's containers reach an Egg dynamic item (see this
    /// task's report: world1's 34 eggs live outside any player-reachable
    /// container in that particular save) -- this synthetic save is the
    /// real-behavior proof for the Egg branch, including its embedded
    /// `SaveParameter` bag (gender/talents/skills), reusing
    /// `domain::pal::read_save_parameter_dto` the same way
    /// `read_dynamic_item` does.
    #[test]
    fn read_item_container_resolves_an_egg_dynamic_item_with_embedded_save_parameter() {
        let container_id = uuid::Uuid::nil();
        let local_id = uuid::Uuid::parse_str("eeeeeeee-0000-0000-0000-000000000000").unwrap();
        let slot = item_container_slot(0, 1, "EggSheepBall", local_id);

        let mut save_parameter = Properties::default();
        save_parameter.insert("Gender", props::enum_property("EPalGenderType::Male"));
        save_parameter.insert("Talent_HP", props::byte_property(70));
        save_parameter.insert("Talent_Shot", props::byte_property(60));
        save_parameter.insert("Talent_Defense", props::byte_property(50));
        save_parameter.insert(
            "EquipWaza",
            props::enum_array_property(vec!["EPalWazaID::Move_Fire".to_string()]),
        );
        let mut object = Properties::default();
        object.insert("SaveParameter", struct_property(save_parameter));

        let egg = dynamic_item_entry(
            local_id,
            "EggSheepBall",
            uesave::games::palworld::PalDynamicItemType::Egg {
                leading_bytes: [0; 4],
                character_id: "SheepBall".to_string(),
                object,
                trailing_bytes: [0; 28],
            },
        );
        let save = save_with_item_container(container_id, 10, vec![slot], vec![egg]);
        let data = game_data();
        let mut caches = WorldCaches::default();

        let dto = read_item_container(
            &save,
            &mut caches,
            &data,
            container_id,
            "CommonContainer",
            None,
        )
        .unwrap();

        let item = dto.slots[0].dynamic_item.as_ref().expect("egg resolves");
        assert_eq!(item.r#type, Some("egg".to_string()));
        assert_eq!(item.character_id, Some("SheepBall".to_string()));
        assert_eq!(item.character_key, Some("sheepball".to_string()));
        assert_eq!(item.gender, Some(crate::dto::pal::PalGender::Male));
        assert_eq!(item.talent_hp, Some(70));
        assert_eq!(item.talent_shot, Some(60));
        assert_eq!(item.talent_defense, Some(50));
        assert_eq!(
            item.active_skills,
            Some(vec!["EPalWazaID::Move_Fire".to_string()])
        );
        // Egg carries none of the weapon/armor-only fields.
        assert!(item.durability.is_none());
        assert!(item.remaining_bullets.is_none());
    }

    /// An egg whose `object` never gained a `"SaveParameter"` property (a
    /// freshly hatched slot with no embedded pal data yet) leaves every
    /// save-parameter-derived field at its `None` default, matching
    /// Python's `_save_parameter` returning `None` through
    /// `PalObjects.get_nested`'s missing-key guard.
    #[test]
    fn read_item_container_egg_without_save_parameter_leaves_stats_none() {
        let container_id = uuid::Uuid::nil();
        let local_id = uuid::Uuid::parse_str("fffffffa-0000-0000-0000-000000000000").unwrap();
        let slot = item_container_slot(0, 1, "EggSheepBall", local_id);
        let egg = dynamic_item_entry(
            local_id,
            "EggSheepBall",
            uesave::games::palworld::PalDynamicItemType::Egg {
                leading_bytes: [0; 4],
                character_id: "SheepBall".to_string(),
                object: Properties::default(),
                trailing_bytes: [0; 28],
            },
        );
        let save = save_with_item_container(container_id, 10, vec![slot], vec![egg]);
        let data = game_data();
        let mut caches = WorldCaches::default();

        let dto = read_item_container(
            &save,
            &mut caches,
            &data,
            container_id,
            "CommonContainer",
            None,
        )
        .unwrap();

        let item = dto.slots[0].dynamic_item.as_ref().unwrap();
        assert_eq!(item.character_id, Some("SheepBall".to_string()));
        assert!(item.gender.is_none());
        assert!(item.talent_hp.is_none());
        assert!(item.active_skills.is_none());
    }
}
