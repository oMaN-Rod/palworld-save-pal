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
use crate::dto::guild::BaseDto;
use crate::dto::pal::PalGender;
use crate::error::CoreError;
use crate::gamedata::GameData;
use crate::props;
use crate::session::{SaveSession, WorldCaches};
use uesave::{ByteArray, Properties, Property, PropertyKey, StructValue, ValueVec};

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

// ============================================================================
// Item-container write-back (Task 10) -- port of `ItemContainer.update_from`/
// `_update_common_container_slots`/`_clean_up_inventory`/
// `_update_or_create_container_slot` (`game/item_container.py`),
// `ItemContainerSlot.update_from` (`game/item_container_slot.py`), and
// `DynamicItem.update_from` (`game/dynamic_item.py`). `Base.update_from`
// (`game/base.py`) is `apply_base_dto` below.
// ============================================================================

/// `PalObjects.ItemContainerSlot`'s literal `CustomVersionData` byte payload
/// (`pal_objects.py`) for a freshly created raw item-container slot.
const ITEM_CONTAINER_SLOT_CUSTOM_VERSION_DATA: [u8; 24] = [
    1, 0, 0, 0, 126, 180, 234, 18, 154, 27, 90, 255, 113, 170, 113, 188, 223, 51, 214, 14, 1, 0, 0,
    0,
];
/// `PalObjects.DynamicItem`'s literal `CustomVersionData` byte payload for a
/// freshly created weapon/armor dynamic item (`pal_objects.py`).
const DYNAMIC_ITEM_WEAPON_ARMOR_CUSTOM_VERSION_DATA: [u8; 24] = [
    1, 0, 0, 0, 56, 11, 0, 222, 73, 73, 215, 206, 151, 223, 45, 153, 192, 193, 195, 105, 1, 0, 0, 0,
];
/// `PalObjects.DynamicItem`'s literal `CustomVersionData` byte payload for a
/// freshly created egg dynamic item (`pal_objects.py`).
const DYNAMIC_ITEM_EGG_CUSTOM_VERSION_DATA: [u8; 44] = [
    2, 0, 0, 0, 56, 11, 0, 222, 73, 73, 215, 206, 151, 223, 45, 153, 192, 193, 195, 105, 1, 0, 0,
    0, 108, 246, 252, 15, 153, 72, 144, 17, 248, 156, 96, 177, 94, 71, 70, 74, 1, 0, 0, 0,
];

fn container_slots_mut(
    level: &mut uesave::Save,
    entry_index: usize,
) -> Option<&mut Vec<StructValue>> {
    let entries = world::item_container_map_mut(level).ok()?;
    let value_props = props::struct_props_mut(&mut entries.get_mut(entry_index)?.value)?;
    props::get_mut(value_props, &["Slots"]).and_then(props::struct_values_mut)
}

fn raw_container_slot(
    slot: &StructValue,
    slot_index: i32,
) -> Option<&uesave::games::palworld::PalItemContainerSlot> {
    let StructValue::Struct(slot_props) = slot else {
        return None;
    };
    match slot_props.0.get(&PropertyKey::from("RawData")) {
        Some(Property::Struct(StructValue::PalItemContainerSlots(raw)))
            if raw.slot_index == slot_index =>
        {
            Some(raw)
        }
        _ => None,
    }
}

/// `ItemContainerSlot.local_id` getter's underlying raw field
/// (`item_container_slot.py`): `None` (this port's stand-in for Python's
/// falsy/absent `local_id`) when no slot at `slot_index` exists, or its
/// dynamic id is the nil GUID.
fn raw_slot_local_id(
    level: &uesave::Save,
    entry_index: usize,
    slot_index: i32,
) -> Option<uuid::Uuid> {
    let entries = world::item_container_map(level).ok()?;
    let value_props = props::struct_props(&entries.get(entry_index)?.value)?;
    let slots = props::get(value_props, &["Slots"]).and_then(props::struct_values)?;
    slots.iter().find_map(|slot| {
        let raw = raw_container_slot(slot, slot_index)?;
        let id = props::guid_to_uuid(&raw.item.dynamic_id.local_id_in_created_world);
        (id != props::EMPTY_UUID).then_some(id)
    })
}

/// `ItemContainerSlot.local_id` setter (`item_container_slot.py`).
fn set_raw_slot_local_id(
    level: &mut uesave::Save,
    entry_index: usize,
    slot_index: i32,
    local_id: uuid::Uuid,
) {
    let Some(slots) = container_slots_mut(level, entry_index) else {
        return;
    };
    for slot in slots.iter_mut() {
        let StructValue::Struct(slot_props) = slot else {
            continue;
        };
        if let Some(Property::Struct(StructValue::PalItemContainerSlots(raw))) =
            slot_props.0.get_mut(&PropertyKey::from("RawData"))
        {
            if raw.slot_index == slot_index {
                raw.item.dynamic_id.local_id_in_created_world = props::uuid_to_guid(local_id);
                return;
            }
        }
    }
}

/// `ItemContainer._remove_container_slot` (`item_container.py`): removes the
/// first `Slots` entry whose `slot_index` matches -- a silent no-op for an
/// index that isn't present, matching Python's own `for ... if ...: ...
/// break` (never raises when nothing matches).
fn remove_raw_slot(level: &mut uesave::Save, entry_index: usize, slot_index: i32) {
    let Some(slots) = container_slots_mut(level, entry_index) else {
        return;
    };
    if let Some(position) = slots
        .iter()
        .position(|slot| raw_container_slot(slot, slot_index).is_some())
    {
        slots.remove(position);
    }
}

/// `ItemContainer._update_or_create_container_slot`'s raw-slot half
/// (`item_container.py`): updates an existing slot's `count`/`static_id` in
/// place (`ItemContainerSlot.update_from`'s `slot_index`/`count`/`static_id`
/// setters -- `slot_index` itself is a no-op re-write since the match is by
/// that same value), or appends a fresh `PalItemContainerSlots` value
/// (`PalObjects.ItemContainerSlot`) when none exists yet. Never touches the
/// dynamic-id field either way -- that's `set_raw_slot_local_id`'s job,
/// called separately by the caller once the dynamic item's final local id is
/// known (see `apply_item_container_dto`).
fn upsert_raw_slot(level: &mut uesave::Save, entry_index: usize, slot: &ItemContainerSlotDto) {
    let Some(slots) = container_slots_mut(level, entry_index) else {
        return;
    };
    let existing = slots.iter_mut().find_map(|value| {
        let StructValue::Struct(slot_props) = value else {
            return None;
        };
        match slot_props.0.get_mut(&PropertyKey::from("RawData")) {
            Some(Property::Struct(StructValue::PalItemContainerSlots(raw)))
                if raw.slot_index == slot.slot_index =>
            {
                Some(raw)
            }
            _ => None,
        }
    });
    let static_id = slot.static_id.clone().unwrap_or_default();
    match existing {
        Some(raw) => {
            raw.count = slot.count;
            raw.item.static_id = static_id;
        }
        None => {
            let mut slot_props = Properties::default();
            slot_props.insert(
                "RawData",
                Property::Struct(StructValue::PalItemContainerSlots(
                    uesave::games::palworld::PalItemContainerSlot {
                        slot_index: slot.slot_index,
                        count: slot.count,
                        item: uesave::games::palworld::PalItemId {
                            static_id,
                            dynamic_id: uesave::games::palworld::PalDynamicId {
                                created_world_id: uesave::FGuid::nil(),
                                local_id_in_created_world: props::uuid_to_guid(props::EMPTY_UUID),
                            },
                        },
                        trailing_bytes: vec![0u8; 16],
                    },
                )),
            );
            slot_props.insert(
                "CustomVersionData",
                Property::Array(ValueVec::Byte(ByteArray::Byte(
                    ITEM_CONTAINER_SLOT_CUSTOM_VERSION_DATA.to_vec(),
                ))),
            );
            slots.push(StructValue::Struct(slot_props));
        }
    }
}

/// `ItemContainer.set_slot_count` (`item_container.py`): writes `SlotNum`,
/// then truncates `Slots` to `slot_count` entries.
///
/// Deviation from the brief: the brief truncated by comparing each slot's
/// `slot_index` VALUE against `slot_count`. Real Python's `set_slot_count`
/// does `self.slots[slot_count:]` -- a truncation by ARRAY POSITION (the
/// order `Slots` entries were originally read in), not by `slot_index`
/// value. The two coincide whenever a container's slots happen to already be
/// stored in ascending-`slot_index` order (the common case for an
/// Essential/Common container, populated sequentially), but are NOT the same
/// operation in general. `Vec::truncate` reproduces Python's real,
/// array-position-based behavior directly.
fn set_item_container_slot_count(
    session: &mut SaveSession,
    container_id: uuid::Uuid,
    slot_count: i32,
) -> Result<(), CoreError> {
    if session.caches.item_container_index.is_none() {
        session.caches.item_container_index =
            Some(world::build_item_container_index(&session.level));
    }
    let Some(entry_index) = session
        .caches
        .item_container_index
        .as_ref()
        .expect("just built")
        .get(&container_id)
        .copied()
    else {
        return Ok(());
    };
    let entries = world::item_container_map_mut(&mut session.level)?;
    let Some(entry) = entries.get_mut(entry_index) else {
        return Ok(());
    };
    let Some(value_props) = props::struct_props_mut(&mut entry.value) else {
        return Ok(());
    };
    if let Some(slot_num_property) = props::get_mut(value_props, &["SlotNum"]) {
        *slot_num_property = props::int_property(slot_count);
    }
    if let Some(slots) = props::get_mut(value_props, &["Slots"]).and_then(props::struct_values_mut)
    {
        if slots.len() as i32 > slot_count {
            slots.truncate(slot_count.max(0) as usize);
        }
    }
    Ok(())
}

/// `IndexedCollection.remove_by_key` applied to `DynamicItemSaveData`
/// (`item_container.py`'s `_clean_up_inventory`): removes the entry at
/// `local_id`, a silent no-op when it isn't present. Invalidates
/// `dynamic_item_index` IMMEDIATELY (not deferred to the end of
/// `apply_item_container_dto`) since a removal shifts every later entry's
/// position -- a second removal/insertion later in the SAME
/// `apply_item_container_dto` call must never resolve a position through a
/// now-stale cached index.
fn remove_dynamic_item(session: &mut SaveSession, local_id: uuid::Uuid) -> Result<(), CoreError> {
    if session.caches.dynamic_item_index.is_none() {
        session.caches.dynamic_item_index = Some(world::build_dynamic_item_index(&session.level));
    }
    let Some(position) = session
        .caches
        .dynamic_item_index
        .as_ref()
        .expect("just built")
        .get(&local_id)
        .copied()
    else {
        return Ok(());
    };
    let values = world::dynamic_item_values_mut(&mut session.level)?;
    if position < values.len() {
        values.remove(position);
    }
    session.caches.dynamic_item_index = None;
    Ok(())
}

fn existing_item_type(
    values: &[StructValue],
    position: usize,
) -> Option<&uesave::games::palworld::PalDynamicItemType> {
    let StructValue::Struct(item_props) = values.get(position)? else {
        return None;
    };
    match item_props.0.get(&PropertyKey::from("RawData")) {
        Some(Property::Struct(StructValue::PalDynamicItem(existing))) => Some(&existing.item_type),
        _ => None,
    }
}

/// `PalObjects.SaveParameter` (`pal_objects.py`): the embedded
/// `{"SaveParameter": {...}}` properties bag an egg dynamic item's `object`
/// field carries when actually populated (`DynamicItem.update_from`'s egg
/// branch, `modified: true`). Mirrors `pal::new_pal_entry`'s literal
/// defaults for `Hp`/`FullStomach` (this constructor's own hardcoded 545000/
/// 400.0, distinct from `new_pal_entry`'s 545000/300.0 -- verified against
/// `pal_objects.py`'s own two, genuinely different literals).
fn build_egg_save_parameter(dto: &DynamicItemDto) -> Properties {
    let mut save_parameter = Properties::default();
    save_parameter.insert(
        "CharacterID",
        props::name_property(dto.character_id.as_deref().unwrap_or_default()),
    );
    let gender = dto.gender.unwrap_or(PalGender::Female);
    save_parameter.insert("Gender", props::enum_property(&gender.prefixed()));
    save_parameter.insert(
        "EquipWaza",
        props::enum_array_property(dto.active_skills.clone().unwrap_or_default()),
    );
    save_parameter.insert(
        "MasteredWaza",
        props::enum_array_property(dto.learned_skills.clone().unwrap_or_default()),
    );
    save_parameter.insert("Hp", props::fixed_point64_property(545_000));
    save_parameter.insert(
        "Talent_HP",
        props::byte_property(dto.talent_hp.unwrap_or(0).clamp(0, 255) as u8),
    );
    save_parameter.insert(
        "Talent_Shot",
        props::byte_property(dto.talent_shot.unwrap_or(0).clamp(0, 255) as u8),
    );
    save_parameter.insert(
        "Talent_Defense",
        props::byte_property(dto.talent_defense.unwrap_or(0).clamp(0, 255) as u8),
    );
    save_parameter.insert("FullStomach", props::float_property(400.0));
    save_parameter.insert(
        "PassiveSkillList",
        props::name_array_property(dto.passive_skills.clone().unwrap_or_default()),
    );
    let mut food_regen = Properties::default();
    food_regen.insert("EffectTime", props::int_property(2));
    save_parameter.insert(
        "FoodRegeneEffectInfo",
        Property::Struct(StructValue::Struct(food_regen)),
    );
    let mut object = Properties::default();
    object.insert(
        "SaveParameter",
        Property::Struct(StructValue::Struct(save_parameter)),
    );
    object
}

/// Port of `DynamicItem.update_from` (`dynamic_item.py`), narrowed to the net
/// effect this port's strongly-typed `PalDynamicItemType` enum can represent
/// (see `upsert_dynamic_item`'s own doc comment for what's deliberately not
/// reproduced, and why).
///
/// `existing` is the CURRENT variant at this local id, if any -- `leading_
/// bytes`/`trailing_bytes` are preserved from it whenever the type doesn't
/// change (matching Python: NEITHER `update_from`'s per-type cleanup block
/// nor its generic per-field loop ever rewrites those two fields for an
/// existing item of unchanged type; the reference `palworld_save_tools`
/// codec's `"unknown_bytes"`/`"unknown_id"` writes in `dynamic_item.py`'s own
/// egg branch are dead code -- verified against `.venv`'s real `encode_bytes`,
/// which never reads those two key names at all, only `"leading_bytes"`/
/// `"trailing_bytes"`; see this task's report). A type CHANGE (or a brand
/// new item) always gets zero-filled leading/trailing bytes, matching
/// `PalObjects._set_leading_trailing_bytes`'s literal `[0] * N` for a freshly
/// built entry -- Python's own real behavior for an existing item that
/// genuinely changes type is closer to memory-unsafe (it would carry over a
/// WRONG-LENGTH byte array from the old type into the new type's encoder,
/// corrupting the write); this port declines to reproduce that, on the
/// "never intentionally corrupt save data" side of this project's "never
/// panic on malformed input" policy. This is a deliberate, DOCUMENTED
/// divergence, not a silently-reproduced bug.
fn build_dynamic_item_type(
    dto: &DynamicItemDto,
    existing: Option<&uesave::games::palworld::PalDynamicItemType>,
) -> uesave::games::palworld::PalDynamicItemType {
    use uesave::games::palworld::PalDynamicItemType;

    match dto.r#type.as_deref() {
        Some("armor") => {
            let (leading_bytes, trailing_bytes) = match existing {
                Some(PalDynamicItemType::Armor {
                    leading_bytes,
                    trailing_bytes,
                    ..
                }) => (*leading_bytes, *trailing_bytes),
                _ => ([0u8; 4], [0u8; 4]),
            };
            let durability =
                dto.durability
                    .map(|value| value as f32)
                    .unwrap_or_else(|| match existing {
                        Some(PalDynamicItemType::Armor { durability, .. }) => *durability,
                        _ => 0.0,
                    });
            PalDynamicItemType::Armor {
                leading_bytes,
                durability,
                trailing_bytes,
            }
        }
        Some("weapon") => {
            let (leading_bytes, trailing_bytes) = match existing {
                Some(PalDynamicItemType::Weapon {
                    leading_bytes,
                    trailing_bytes,
                    ..
                }) => (*leading_bytes, *trailing_bytes),
                _ => ([0u8; 4], [0u8; 4]),
            };
            let (existing_durability, existing_bullets, existing_passives) = match existing {
                Some(PalDynamicItemType::Weapon {
                    durability,
                    remaining_bullets,
                    passive_skill_list,
                    ..
                }) => (*durability, *remaining_bullets, passive_skill_list.clone()),
                _ => (0.0, 0, Vec::new()),
            };
            PalDynamicItemType::Weapon {
                leading_bytes,
                durability: dto
                    .durability
                    .map(|value| value as f32)
                    .unwrap_or(existing_durability),
                remaining_bullets: dto
                    .remaining_bullets
                    .map(|value| value as i32)
                    .unwrap_or(existing_bullets),
                passive_skill_list: dto.passive_skill_list.clone().unwrap_or(existing_passives),
                trailing_bytes,
            }
        }
        Some("egg") => {
            let (leading_bytes, trailing_bytes, existing_object) = match existing {
                Some(PalDynamicItemType::Egg {
                    leading_bytes,
                    trailing_bytes,
                    object,
                    ..
                }) => (*leading_bytes, *trailing_bytes, Some(object.clone())),
                _ => ([0u8; 4], [0u8; 28], None),
            };
            let object = if dto.modified {
                build_egg_save_parameter(dto)
            } else {
                existing_object.unwrap_or_default()
            };
            PalDynamicItemType::Egg {
                leading_bytes,
                character_id: dto.character_id.clone().unwrap_or_default(),
                object,
                trailing_bytes,
            }
        }
        _ => {
            let trailer = match existing {
                Some(PalDynamicItemType::Unknown { trailer }) => trailer.clone(),
                _ => Vec::new(),
            };
            PalDynamicItemType::Unknown { trailer }
        }
    }
}

/// Port of `DynamicItem.update_from` (`dynamic_item.py`) plus
/// `PalObjects.DynamicItem`'s constructor (`pal_objects.py`) for the
/// not-yet-present case. `slot_static_id` is the CONTAINING SLOT's
/// `static_id` (`ItemContainerSlotDto::static_id`), used ONLY when creating a
/// brand new entry -- `PalDynamicItem.static_id` is a WIRE SIBLING of `id`
/// (not nested inside it despite Python's dict-based decoder grouping them
/// together; verified against `../uesave-rs/uesave/src/games/palworld/
/// items.rs`), and `DynamicItemDTO` (the wire DTO's INPUT shape) has no
/// `static_id` field of its own at all -- Python's `update_from` generic loop
/// (`for key, value in other.items(): if hasattr(self, key): setattr(...)`)
/// therefore NEVER touches `static_id` for an EXISTING item (no `"static_id"`
/// key ever appears in `other.items()`), leaving it untouched; only
/// `PalObjects.DynamicItem(container_slot)`'s ONE-TIME construction for a
/// brand-new entry ever sets it, from the slot's own `static_id`.
pub fn upsert_dynamic_item(
    session: &mut SaveSession,
    dto: &DynamicItemDto,
    slot_static_id: &str,
) -> Result<(), CoreError> {
    if session.caches.dynamic_item_index.is_none() {
        session.caches.dynamic_item_index = Some(world::build_dynamic_item_index(&session.level));
    }
    let existing_position = session
        .caches
        .dynamic_item_index
        .as_ref()
        .expect("just built")
        .get(&dto.local_id)
        .copied();

    let existing_type = existing_position.and_then(|position| {
        world::dynamic_item_values(&session.level)
            .ok()
            .and_then(|values| existing_item_type(values, position))
    });
    let item_type = build_dynamic_item_type(dto, existing_type);

    match existing_position {
        Some(position) => {
            let values = world::dynamic_item_values_mut(&mut session.level)?;
            if let Some(StructValue::Struct(item_props)) = values.get_mut(position) {
                if let Some(Property::Struct(StructValue::PalDynamicItem(existing))) =
                    item_props.0.get_mut(&PropertyKey::from("RawData"))
                {
                    existing.item_type = item_type;
                }
            }
        }
        None => {
            let mut item_props = Properties::default();
            item_props.insert(
                "RawData",
                Property::Struct(StructValue::PalDynamicItem(Box::new(
                    uesave::games::palworld::PalDynamicItem {
                        id: uesave::games::palworld::PalDynamicId {
                            created_world_id: uesave::FGuid::nil(),
                            local_id_in_created_world: props::uuid_to_guid(dto.local_id),
                        },
                        static_id: slot_static_id.to_string(),
                        item_type,
                    },
                ))),
            );
            let custom_version_data: Option<&[u8]> = match dto.r#type.as_deref() {
                Some("weapon") | Some("armor") => {
                    Some(&DYNAMIC_ITEM_WEAPON_ARMOR_CUSTOM_VERSION_DATA)
                }
                Some("egg") => Some(&DYNAMIC_ITEM_EGG_CUSTOM_VERSION_DATA),
                _ => None,
            };
            if let Some(bytes) = custom_version_data {
                item_props.insert(
                    "CustomVersionData",
                    Property::Array(ValueVec::Byte(ByteArray::Byte(bytes.to_vec()))),
                );
            }
            let values = world::dynamic_item_values_mut(&mut session.level)?;
            values.push(StructValue::Struct(item_props));
            session.caches.dynamic_item_index = None;
        }
    }
    Ok(())
}

/// Port of `ItemContainer.update_from` (`item_container.py`): resizes the
/// paired common container (essential only), cleans up removed dynamic
/// items/`static_id == "None"` slots, then upserts every remaining incoming
/// slot. `container_id` is the CALLER-RESOLVED, session-trusted target (see
/// `player::apply_player_dto`'s own doc comment on why `dto.id` is never used
/// for routing) -- `dto` supplies only `type`/`slots` content.
///
/// **A genuine, newly-found Python bug, reproduced deliberately for save-file
/// byte parity, not on the known list (legacy `"HP"` write /
/// `ext_status_point_list`'s missing guard / `Guild.players`'
/// `UnboundLocalError` / `_load_pals_for_container`'s `"SlotId"`-only
/// spelling / `safe_remove(character_save, "OwnerPlayerUId")`'s wrong-level
/// no-op / `Pal.reset()` never clearing `IsRarePal`):** when an incoming slot
/// has no `dynamic_item` but the existing raw slot does,
/// `_clean_up_inventory` removes the `DynamicItemSaveData` entry AND sets
/// `container_slot.dynamic_item = None` -- but `dynamic_item` is a PLAIN
/// pydantic field on `ItemContainerSlot` (`item_container_slot.py`), not a
/// computed property with a setter that writes into `_raw_data`. That
/// assignment only updates the in-memory Python object; the RAW slot's
/// `item.dynamic_id.local_id_in_created_world` is left untouched, still
/// pointing at the now-deleted dynamic item. Nothing later in
/// `_update_or_create_container_slot` fixes it either: for a slot whose
/// incoming DTO has no `dynamic_item`, that function's `if not slot_dto.
/// dynamic_item: return` bails out immediately, before its own `slot.
/// local_id = ...` line ever runs. The dangling reference is harmless in
/// practice -- the NEXT time this exact container is read (`read_item_
/// container`, this port's own `_get_items` equivalent), a `local_id` that
/// resolves to nothing already causes the whole slot to be dropped (see that
/// function's own doc comment) -- but it IS what a save-out immediately after
/// this edit would write to disk, so it is reproduced here rather than
/// proactively cleared. See this task's report.
pub fn apply_item_container_dto(
    session: &mut SaveSession,
    _game_data: &GameData,
    container_id: uuid::Uuid,
    dto: &ItemContainerDto,
    paired_common_container_id: Option<uuid::Uuid>,
) -> Result<(), CoreError> {
    if dto.r#type == "EssentialContainer" {
        if let Some(common_id) = paired_common_container_id {
            let additional_inventory_count = dto
                .slots
                .iter()
                .filter(|slot| {
                    slot.static_id
                        .as_deref()
                        .map(|id| id.starts_with("AdditionalInventory_"))
                        .unwrap_or(false)
                })
                .count() as i32;
            set_item_container_slot_count(
                session,
                common_id,
                42 + additional_inventory_count.min(4) * 3,
            )?;
        }
    }
    if session.caches.item_container_index.is_none() {
        session.caches.item_container_index =
            Some(world::build_item_container_index(&session.level));
    }
    let Some(container_entry_index) = session
        .caches
        .item_container_index
        .as_ref()
        .expect("just built")
        .get(&container_id)
        .copied()
    else {
        return Ok(());
    };

    // Cleanup pass (`_clean_up_inventory`).
    for incoming_slot in &dto.slots {
        let existing_local_id = raw_slot_local_id(
            &session.level,
            container_entry_index,
            incoming_slot.slot_index,
        );
        if incoming_slot.dynamic_item.is_none() {
            if let Some(local_id) = existing_local_id {
                remove_dynamic_item(session, local_id)?;
                // Raw slot's own local_id is deliberately left dangling here
                // -- see this function's own doc comment (a real Python bug,
                // reproduced for byte parity).
            }
        }
        if incoming_slot.static_id.as_deref() == Some("None") {
            remove_raw_slot(
                &mut session.level,
                container_entry_index,
                incoming_slot.slot_index,
            );
        }
    }

    // Apply pass (`_update_or_create_container_slot`).
    for incoming_slot in &dto.slots {
        if incoming_slot.static_id.is_none() || incoming_slot.static_id.as_deref() == Some("None") {
            continue;
        }
        upsert_raw_slot(&mut session.level, container_entry_index, incoming_slot);
        if let Some(dynamic_dto) = &incoming_slot.dynamic_item {
            let mut resolved = dynamic_dto.clone();
            if resolved.local_id == props::EMPTY_UUID {
                resolved.local_id = raw_slot_local_id(
                    &session.level,
                    container_entry_index,
                    incoming_slot.slot_index,
                )
                .unwrap_or_else(uuid::Uuid::new_v4);
            }
            let static_id = incoming_slot.static_id.clone().unwrap_or_default();
            upsert_dynamic_item(session, &resolved, &static_id)?;
            set_raw_slot_local_id(
                &mut session.level,
                container_entry_index,
                incoming_slot.slot_index,
                resolved.local_id,
            );
        }
    }

    session.caches.dynamic_item_index = None;
    session.caches.item_container_index = None;
    Ok(())
}

/// Port of `Base.update_from` (`game/base.py`).
///
/// **Membership-scoped, unlike the brief.** The brief's reference code
/// applied every entry in `dto.storage_containers` unconditionally. Real
/// Python's `self.storage_containers[id].update_from(...)` indexes a dict
/// scoped to THIS base's own already-loaded storage containers -- an `id`
/// that isn't one of them raises `KeyError` (Python crashes rather than
/// silently touching a container elsewhere in the save). Reproduced here as
/// a skip (this port's established "never panic on malformed/adversarial
/// input" policy) rather than a crash, but the MEMBERSHIP CHECK itself is
/// real and load-bearing: without it, a forged `container_id` key in the
/// DTO's `storage_containers` map could redirect a base-storage edit onto an
/// unrelated container anywhere in the world (a different base's storage, a
/// player's inventory, ...) -- the same cross-entity class of hole Task 9's
/// review flagged Critical for `delete_player_pals`. See this task's report.
pub fn apply_base_dto(
    session: &mut SaveSession,
    game_data: &GameData,
    base_id: uuid::Uuid,
    dto: &BaseDto,
) -> Result<(), CoreError> {
    let real_container_ids = super::guild::base_storage_container_ids(session, base_id);
    for (container_id, container_dto) in dto.storage_containers.iter() {
        if !real_container_ids.contains(container_id) {
            continue;
        }
        apply_item_container_dto(session, game_data, *container_id, container_dto, None)?;
    }

    let Some(entries) = world::base_camp_map_mut(&mut session.level)? else {
        return Ok(());
    };
    let Some(entry) = entries
        .iter_mut()
        .find(|entry| props::as_uuid(&entry.key) == Some(base_id))
    else {
        return Ok(());
    };
    let Some(value_props) = props::struct_props_mut(&mut entry.value) else {
        return Ok(());
    };
    if let Some(Property::Struct(StructValue::PalBaseCamp(base_camp))) =
        value_props.0.get_mut(&PropertyKey::from("RawData"))
    {
        if let Some(name) = &dto.name {
            if !name.is_empty() {
                base_camp.name = name.clone();
            }
        }
        if let Some(area_range) = dto.area_range {
            base_camp.area_range = area_range as f32;
        }
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

    // ---- apply_item_container_dto / apply_base_dto (Task 10 write-back) ----

    use crate::session::{SaveKind, SaveSession};

    fn session_with_item_container(
        container_id: uuid::Uuid,
        slot_num: i32,
        slots: Vec<StructValue>,
        dynamic_items: Vec<StructValue>,
    ) -> SaveSession {
        let save = save_with_item_container(container_id, slot_num, slots, dynamic_items);
        SaveSession::new_for_tests(SaveKind::InMemory, save)
    }

    fn slot_dto(
        slot_index: i32,
        count: i32,
        static_id: &str,
        dynamic_item: Option<DynamicItemDto>,
    ) -> ItemContainerSlotDto {
        ItemContainerSlotDto {
            dynamic_item,
            slot_index,
            count,
            static_id: Some(static_id.to_string()),
            local_id: None,
        }
    }

    fn weapon_dynamic_item_dto(local_id: uuid::Uuid) -> DynamicItemDto {
        DynamicItemDto {
            local_id,
            modified: false,
            character_id: None,
            character_key: None,
            durability: Some(75.0),
            passive_skill_list: Some(vec!["Rare".to_string()]),
            remaining_bullets: Some(20),
            r#type: Some("weapon".to_string()),
            static_id: None,
            gender: None,
            active_skills: None,
            learned_skills: None,
            passive_skills: None,
            talent_hp: None,
            talent_shot: None,
            talent_defense: None,
        }
    }

    /// Positive cache-invalidation proof for a STRUCTURAL ADD: a brand new
    /// dynamic item pushed onto `DynamicItemSaveData` must (a) clear
    /// `session.caches.dynamic_item_index` (not just leave a coincidentally
    /// matching stale one), and (b) a freshly rebuilt index must actually
    /// resolve the new entry at its real position -- not merely "the cache
    /// is now `None`", which would trivially pass even if the item were
    /// never actually appended (Task 9's own established standard).
    #[test]
    fn apply_item_container_dto_adds_a_new_dynamic_item_and_invalidates_the_cache() {
        let container_id = uuid::Uuid::parse_str("11111111-0000-0000-0000-000000000000").unwrap();
        let mut session = session_with_item_container(container_id, 10, vec![], vec![]);
        // Warm the cache with the pre-edit (empty) state, proving the SAME
        // stale cache object gets thrown away, not just left conveniently
        // unbuilt.
        session.caches.dynamic_item_index = Some(world::build_dynamic_item_index(&session.level));
        assert!(session
            .caches
            .dynamic_item_index
            .as_ref()
            .unwrap()
            .is_empty());

        let local_id = uuid::Uuid::parse_str("22222222-0000-0000-0000-000000000000").unwrap();
        let dto = ItemContainerDto {
            id: container_id,
            r#type: "CommonContainer".to_string(),
            slots: vec![slot_dto(
                0,
                1,
                "SFBow_5",
                Some(weapon_dynamic_item_dto(local_id)),
            )],
            key: None,
            slot_num: 0,
        };
        let data = game_data();
        apply_item_container_dto(&mut session, &data, container_id, &dto, None).unwrap();

        assert!(
            session.caches.dynamic_item_index.is_none(),
            "a structural add must invalidate the cache"
        );
        let rebuilt = world::build_dynamic_item_index(&session.level);
        assert_eq!(
            rebuilt.get(&local_id),
            Some(&0),
            "the new dynamic item must actually be resolvable at its real position"
        );
        let values = world::dynamic_item_values(&session.level).unwrap();
        assert_eq!(values.len(), 1);
    }

    /// Positive cache-invalidation proof for a STRUCTURAL REMOVE (the
    /// counterpart of the add proof above): a dynamic item that gets removed
    /// via `_clean_up_inventory`'s "incoming slot has no dynamic_item, the
    /// existing one does" branch must vanish from a freshly rebuilt index,
    /// and the cache field itself must be cleared.
    #[test]
    fn apply_item_container_dto_removes_a_dynamic_item_and_invalidates_the_cache() {
        let container_id = uuid::Uuid::nil();
        let local_id = uuid::Uuid::parse_str("33333333-0000-0000-0000-000000000000").unwrap();
        let slot = item_container_slot(0, 1, "SFBow_5", local_id);
        let weapon = dynamic_item_entry(
            local_id,
            "SFBow_5",
            uesave::games::palworld::PalDynamicItemType::Weapon {
                leading_bytes: [0; 4],
                durability: 80.0,
                remaining_bullets: 12,
                passive_skill_list: vec![],
                trailing_bytes: [0; 4],
            },
        );
        let mut session = session_with_item_container(container_id, 10, vec![slot], vec![weapon]);
        session.caches.dynamic_item_index = Some(world::build_dynamic_item_index(&session.level));
        assert_eq!(
            session
                .caches
                .dynamic_item_index
                .as_ref()
                .unwrap()
                .get(&local_id),
            Some(&0)
        );

        // Incoming slot keeps the same static_id but drops the dynamic item.
        let dto = ItemContainerDto {
            id: container_id,
            r#type: "CommonContainer".to_string(),
            slots: vec![slot_dto(0, 1, "SFBow_5", None)],
            key: None,
            slot_num: 0,
        };
        let data = game_data();
        apply_item_container_dto(&mut session, &data, container_id, &dto, None).unwrap();

        assert!(
            session.caches.dynamic_item_index.is_none(),
            "a structural removal must invalidate the cache"
        );
        let rebuilt = world::build_dynamic_item_index(&session.level);
        assert!(
            !rebuilt.contains_key(&local_id),
            "the removed dynamic item must actually be gone, not merely absent from a stale cache"
        );
        assert!(world::dynamic_item_values(&session.level)
            .unwrap()
            .is_empty());
    }

    /// `_update_common_container_slots` (`item_container.py`): an essential
    /// container with `additional_inventory_count` `AdditionalInventory_*`
    /// slots resizes its paired common container to `42 + min(count, 4)*3`
    /// slots, truncating the common container's own `Slots` array by
    /// POSITION when it shrinks -- proven with 6 existing common slots and 2
    /// `AdditionalInventory_` essential slots (expected: 42 + 2*3 = 48, well
    /// above 6, so nothing is truncated here; the truncation half is proven
    /// separately below).
    #[test]
    fn apply_item_container_dto_essential_resizes_the_paired_common_container() {
        let common_id = uuid::Uuid::parse_str("44444444-0000-0000-0000-000000000000").unwrap();
        let essential_id = uuid::Uuid::parse_str("55555555-0000-0000-0000-000000000000").unwrap();

        // Build a save with BOTH containers present.
        let mut key_common = Properties::default();
        key_common.insert("ID", guid_property(common_id));
        let mut value_common = Properties::default();
        value_common.insert("SlotNum", props::int_property(42));
        value_common.insert("Slots", Property::Array(ValueVec::Struct(vec![])));

        let mut key_essential = Properties::default();
        key_essential.insert("ID", guid_property(essential_id));
        let mut value_essential = Properties::default();
        value_essential.insert("SlotNum", props::int_property(5));
        value_essential.insert("Slots", Property::Array(ValueVec::Struct(vec![])));

        let mut world_save_data = Properties::default();
        world_save_data.insert(
            "ItemContainerSaveData",
            Property::Map(vec![
                uesave::MapEntry {
                    key: struct_property(key_common),
                    value: struct_property(value_common),
                },
                uesave::MapEntry {
                    key: struct_property(key_essential),
                    value: struct_property(value_essential),
                },
            ]),
        );
        world_save_data.insert(
            "DynamicItemSaveData",
            Property::Array(ValueVec::Struct(vec![])),
        );
        let mut root_properties = Properties::default();
        root_properties.insert("worldSaveData", struct_property(world_save_data));
        let mut session =
            SaveSession::new_for_tests(SaveKind::InMemory, minimal_save(root_properties));

        let dto = ItemContainerDto {
            id: essential_id,
            r#type: "EssentialContainer".to_string(),
            slots: vec![
                slot_dto(0, 1, "AdditionalInventory_1", None),
                slot_dto(1, 1, "AdditionalInventory_2", None),
            ],
            key: None,
            slot_num: 5,
        };
        let data = game_data();
        apply_item_container_dto(&mut session, &data, essential_id, &dto, Some(common_id)).unwrap();

        let entries = world::item_container_map(&session.level).unwrap();
        let common_entry = entries
            .iter()
            .find(|entry| {
                props::struct_props(&entry.key)
                    .and_then(|key| props::get(key, &["ID"]))
                    .and_then(props::as_uuid)
                    == Some(common_id)
            })
            .unwrap();
        let common_value = props::struct_props(&common_entry.value).unwrap();
        let slot_num = props::get(common_value, &["SlotNum"])
            .and_then(props::as_i32)
            .unwrap();
        assert_eq!(slot_num, 42 + 2 * 3, "42 + min(2,4)*3 = 48");
    }

    /// The truncation half of `set_item_container_slot_count`: shrinking
    /// past the container's current slot COUNT removes the excess entries
    /// by array position (see `set_item_container_slot_count`'s own doc
    /// comment on why this is position-based, not `slot_index`-value-based).
    #[test]
    fn apply_item_container_dto_essential_resize_truncates_excess_common_slots() {
        let common_id = uuid::Uuid::parse_str("66666666-0000-0000-0000-000000000000").unwrap();
        let essential_id = uuid::Uuid::parse_str("77777777-0000-0000-0000-000000000000").unwrap();

        let common_slots: Vec<StructValue> = (0..45)
            .map(|index| item_container_slot(index, 1, "Wood", props::EMPTY_UUID))
            .collect();
        let mut key_common = Properties::default();
        key_common.insert("ID", guid_property(common_id));
        let mut value_common = Properties::default();
        value_common.insert("SlotNum", props::int_property(45));
        value_common.insert("Slots", Property::Array(ValueVec::Struct(common_slots)));

        let mut key_essential = Properties::default();
        key_essential.insert("ID", guid_property(essential_id));
        let mut value_essential = Properties::default();
        value_essential.insert("SlotNum", props::int_property(0));
        value_essential.insert("Slots", Property::Array(ValueVec::Struct(vec![])));

        let mut world_save_data = Properties::default();
        world_save_data.insert(
            "ItemContainerSaveData",
            Property::Map(vec![
                uesave::MapEntry {
                    key: struct_property(key_common),
                    value: struct_property(value_common),
                },
                uesave::MapEntry {
                    key: struct_property(key_essential),
                    value: struct_property(value_essential),
                },
            ]),
        );
        world_save_data.insert(
            "DynamicItemSaveData",
            Property::Array(ValueVec::Struct(vec![])),
        );
        let mut root_properties = Properties::default();
        root_properties.insert("worldSaveData", struct_property(world_save_data));
        let mut session =
            SaveSession::new_for_tests(SaveKind::InMemory, minimal_save(root_properties));

        // No AdditionalInventory_ slots at all -> target size 42, below the
        // 45 the common container currently has.
        let dto = ItemContainerDto {
            id: essential_id,
            r#type: "EssentialContainer".to_string(),
            slots: vec![],
            key: None,
            slot_num: 0,
        };
        let data = game_data();
        apply_item_container_dto(&mut session, &data, essential_id, &dto, Some(common_id)).unwrap();

        let entries = world::item_container_map(&session.level).unwrap();
        let common_entry = entries
            .iter()
            .find(|entry| {
                props::struct_props(&entry.key)
                    .and_then(|key| props::get(key, &["ID"]))
                    .and_then(props::as_uuid)
                    == Some(common_id)
            })
            .unwrap();
        let common_value = props::struct_props(&common_entry.value).unwrap();
        let remaining_slots = props::get(common_value, &["Slots"])
            .and_then(props::struct_values)
            .unwrap();
        assert_eq!(remaining_slots.len(), 42);
    }

    /// `_update_or_create_container_slot`: a `static_id == "None"` incoming
    /// slot removes the existing raw slot entirely.
    #[test]
    fn apply_item_container_dto_removes_a_slot_whose_static_id_is_none() {
        let container_id = uuid::Uuid::nil();
        let slot = item_container_slot(0, 1, "Wood", props::EMPTY_UUID);
        let mut session = session_with_item_container(container_id, 10, vec![slot], vec![]);
        let dto = ItemContainerDto {
            id: container_id,
            r#type: "CommonContainer".to_string(),
            slots: vec![slot_dto(0, 0, "None", None)],
            key: None,
            slot_num: 10,
        };
        let data = game_data();
        apply_item_container_dto(&mut session, &data, container_id, &dto, None).unwrap();

        let entries = world::item_container_map(&session.level).unwrap();
        let value_props = props::struct_props(&entries[0].value).unwrap();
        let slots = props::get(value_props, &["Slots"])
            .and_then(props::struct_values)
            .unwrap();
        assert!(slots.is_empty());
    }

    /// An `apply_item_container_dto` call for a `container_id` this session
    /// doesn't have at all is a silent no-op, matching this port's
    /// established "skip an unresolvable id, never panic" policy.
    #[test]
    fn apply_item_container_dto_unknown_container_id_is_a_no_op() {
        let mut session = session_with_item_container(uuid::Uuid::nil(), 5, vec![], vec![]);
        let unknown = uuid::Uuid::parse_str("88888888-0000-0000-0000-000000000000").unwrap();
        let dto = ItemContainerDto {
            id: unknown,
            r#type: "CommonContainer".to_string(),
            slots: vec![slot_dto(0, 1, "Wood", None)],
            key: None,
            slot_num: 5,
        };
        let data = game_data();
        apply_item_container_dto(&mut session, &data, unknown, &dto, None).unwrap();
        // The real (nil-id) container must be completely untouched.
        let entries = world::item_container_map(&session.level).unwrap();
        let value_props = props::struct_props(&entries[0].value).unwrap();
        assert!(props::get(value_props, &["Slots"])
            .and_then(props::struct_values)
            .unwrap()
            .is_empty());
    }

    /// A brand new egg dynamic item with `modified: true` rebuilds a full
    /// embedded `SaveParameter` (`PalObjects.SaveParameter`) -- gender,
    /// talents, and skills must all be readable back afterward via the same
    /// `read_item_container`/`read_dynamic_item` path Task 5 already
    /// verified.
    #[test]
    fn apply_item_container_dto_new_egg_modified_rebuilds_embedded_save_parameter() {
        let container_id = uuid::Uuid::nil();
        let local_id = uuid::Uuid::parse_str("99999999-0000-0000-0000-000000000000").unwrap();
        let mut session = session_with_item_container(container_id, 10, vec![], vec![]);
        let egg_dto = DynamicItemDto {
            local_id: props::EMPTY_UUID, // unset -> a fresh uuid is minted
            modified: true,
            character_id: Some("SheepBall".to_string()),
            character_key: None,
            durability: None,
            passive_skill_list: None,
            remaining_bullets: None,
            r#type: Some("egg".to_string()),
            static_id: None,
            gender: Some(PalGender::Male),
            active_skills: Some(vec!["EPalWazaID::Move_Fire".to_string()]),
            learned_skills: Some(vec![]),
            passive_skills: Some(vec![]),
            talent_hp: Some(70),
            talent_shot: Some(60),
            talent_defense: Some(50),
        };
        let dto = ItemContainerDto {
            id: container_id,
            r#type: "CommonContainer".to_string(),
            slots: vec![slot_dto(0, 1, "EggSheepBall", Some(egg_dto))],
            key: None,
            slot_num: 10,
        };
        let data = game_data();
        apply_item_container_dto(&mut session, &data, container_id, &dto, None).unwrap();
        let _ = local_id; // not the minted id; kept for readability of intent

        let mut caches = WorldCaches::default();
        let reread = read_item_container(
            &session.level,
            &mut caches,
            &data,
            container_id,
            "CommonContainer",
            None,
        )
        .expect("container resolves");
        let item = reread.slots[0].dynamic_item.as_ref().expect("egg resolves");
        assert_eq!(item.r#type, Some("egg".to_string()));
        assert_eq!(item.character_id, Some("SheepBall".to_string()));
        assert_eq!(item.gender, Some(PalGender::Male));
        assert_eq!(item.talent_hp, Some(70));
        assert_eq!(item.talent_shot, Some(60));
        assert_eq!(item.talent_defense, Some(50));
        assert_eq!(
            item.active_skills,
            Some(vec!["EPalWazaID::Move_Fire".to_string()])
        );
    }

    /// An existing egg dynamic item updated with `modified: false` must
    /// preserve its embedded `object` exactly, even though the incoming DTO
    /// carries different gender/talent values -- Python's `DynamicItem.
    /// update_from` only rebuilds `object` when `modified` is truthy.
    #[test]
    fn apply_item_container_dto_existing_egg_unmodified_preserves_embedded_save_parameter() {
        let container_id = uuid::Uuid::nil();
        let local_id = uuid::Uuid::parse_str("aaaaaaab-0000-0000-0000-000000000000").unwrap();
        let slot = item_container_slot(0, 1, "EggSheepBall", local_id);

        let mut save_parameter = Properties::default();
        save_parameter.insert("Gender", props::enum_property("EPalGenderType::Female"));
        save_parameter.insert("Talent_HP", props::byte_property(30));
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
        let mut session = session_with_item_container(container_id, 10, vec![slot], vec![egg]);

        // Incoming DTO claims different stats but modified: false -- must be
        // ignored; the original embedded object survives untouched.
        let egg_dto = DynamicItemDto {
            local_id,
            modified: false,
            character_id: Some("SheepBall".to_string()),
            character_key: None,
            durability: None,
            passive_skill_list: None,
            remaining_bullets: None,
            r#type: Some("egg".to_string()),
            static_id: None,
            gender: Some(PalGender::Male),
            active_skills: None,
            learned_skills: None,
            passive_skills: None,
            talent_hp: Some(99),
            talent_shot: None,
            talent_defense: None,
        };
        let dto = ItemContainerDto {
            id: container_id,
            r#type: "CommonContainer".to_string(),
            slots: vec![slot_dto(0, 1, "EggSheepBall", Some(egg_dto))],
            key: None,
            slot_num: 10,
        };
        let data = game_data();
        apply_item_container_dto(&mut session, &data, container_id, &dto, None).unwrap();

        let mut caches = WorldCaches::default();
        let reread = read_item_container(
            &session.level,
            &mut caches,
            &data,
            container_id,
            "CommonContainer",
            None,
        )
        .unwrap();
        let item = reread.slots[0].dynamic_item.as_ref().unwrap();
        assert_eq!(
            item.gender,
            Some(crate::dto::pal::PalGender::Female),
            "unmodified egg must keep its ORIGINAL embedded gender, not the DTO's"
        );
        assert_eq!(item.talent_hp, Some(30));
    }

    /// `apply_base_dto` must reject a `storage_containers` map key that
    /// isn't actually one of THIS base's own item containers -- see
    /// `apply_base_dto`'s own doc comment for why this membership check
    /// exists (a Critical-class fix over the brief's unconditional-apply
    /// reference code). A forged container id must be silently skipped,
    /// leaving the unrelated container completely untouched.
    #[test]
    fn apply_base_dto_rejects_a_container_id_that_does_not_belong_to_this_base() {
        let base_id = uuid::Uuid::parse_str("bbbbbbbb-0000-0000-0000-000000000000").unwrap();
        let unrelated_container_id =
            uuid::Uuid::parse_str("cccccccc-0000-0000-0000-000000000000").unwrap();
        // A container that exists in the save but is NOT registered as one
        // of base_id's own storage containers (no MapObjectSaveData entry
        // links them) -- base_storage_container_ids(session, base_id) must
        // therefore come back empty, and apply_base_dto must not touch it.
        let mut session = session_with_item_container(unrelated_container_id, 5, vec![], vec![]);

        let mut storage_containers = crate::dto::ordered_map::OrderedMap::new();
        storage_containers.insert(
            unrelated_container_id,
            ItemContainerDto {
                id: unrelated_container_id,
                r#type: "BaseContainer".to_string(),
                slots: vec![slot_dto(0, 1, "Wood", None)],
                key: None,
                slot_num: 5,
            },
        );
        let base_dto = BaseDto {
            pals: crate::dto::ordered_map::OrderedMap::new(),
            container_id: None,
            slot_count: None,
            storage_containers,
            pal_container: None,
            id: base_id,
            name: None,
            location: None,
            area_range: None,
        };
        let data = game_data();
        apply_base_dto(&mut session, &data, base_id, &base_dto).unwrap();

        let entries = world::item_container_map(&session.level).unwrap();
        let value_props = props::struct_props(&entries[0].value).unwrap();
        let slots = props::get(value_props, &["Slots"])
            .and_then(props::struct_values)
            .unwrap();
        assert!(
            slots.is_empty(),
            "a container id outside this base's real storage set must never be mutated"
        );
    }
}
