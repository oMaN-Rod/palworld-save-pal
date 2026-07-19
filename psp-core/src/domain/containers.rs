//! Character- and item-container slot operations.
//!
//! `character_container_add_pal`/`remove_pal` mutate only the `Slots` array
//! nested inside an already-positioned `CharacterContainerSaveData` entry;
//! no entry's position changes, so the container-id → entry-position caches
//! stay valid across them.

use crate::dto::container::{
    CharacterContainerSlotDto, DynamicItemDto, ItemContainerDto, ItemContainerSlotDto,
};
use crate::dto::guild::BaseDto;
use crate::dto::pal::PalGender;
use crate::error::CoreError;
use crate::gamedata::GameData;
use crate::props;
use crate::session::{SaveSession, WorldCaches};
use crate::ue::{ByteArray, Properties, Property, PropertyKey, StructValue, ValueVec};

/// An egg carries a pal's `SaveParameter` names under the dynamic item's `RawData`.
pub const EGG_SAVE_PARAMETER_PREFIX: &str =
    "worldSaveData.DynamicItemSaveData.RawData.SaveParameter";

/// A schema path carries no array index, so an array that is empty on disk records
/// no element schema at all -- and a world with no pal, or a player with no weapon,
/// is exactly the one that cannot accept the first one written.
pub fn ensure_container_schemas(level: &mut crate::ue::Save) {
    use crate::ue::{PropertyTagDataPartial as Data, PropertyTagPartial, PropertyType};

    let byte_array = || Data::Array(Box::new(Data::Byte(None)));
    // `struct_type_for` resolves a name the way the READER does: a Palworld game
    // struct (`PalCharacterContainer`, ...) becomes a `StructType::Game`, anything
    // else a plain named struct. Hand-picking `Raw` here instead marks the type
    // unknown, and uesave then writes the payload with the wrong codec -- a save
    // that no longer parses back.
    let pal_struct = |name: &str| Data::Struct {
        struct_type: crate::ue::struct_type_for(name),
        id: crate::ue::FGuid::nil(),
    };
    let struct_array = |name: &str| Data::Array(Box::new(pal_struct(name)));

    let entries: [(&str, Data); 10] = [
        (
            "worldSaveData.CharacterContainerSaveData.Slots",
            struct_array("PalCharacterSlotSaveData"),
        ),
        (
            "worldSaveData.CharacterContainerSaveData.Slots.SlotIndex",
            Data::Other(PropertyType::IntProperty),
        ),
        (
            "worldSaveData.CharacterContainerSaveData.Slots.RawData",
            pal_struct("PalCharacterContainer"),
        ),
        (
            "worldSaveData.CharacterContainerSaveData.Slots.CustomVersionData",
            byte_array(),
        ),
        (
            "worldSaveData.ItemContainerSaveData.Slots",
            struct_array("PalItemSlotSaveData"),
        ),
        (
            "worldSaveData.ItemContainerSaveData.Slots.RawData",
            pal_struct("PalItemContainerSlots"),
        ),
        (
            "worldSaveData.ItemContainerSaveData.Slots.CustomVersionData",
            byte_array(),
        ),
        (
            "worldSaveData.DynamicItemSaveData.RawData",
            pal_struct("PalDynamicItem"),
        ),
        // An egg's `SaveParameter` struct node. A save with no egg records only
        // its children (via `ensure_save_parameter_schemas` below), never this
        // node -- so the first egg written failed to serialize. Real saves that
        // hold an egg record it as `PalIndividualCharacterSaveParameter`, the
        // same type a pal's `CharacterSaveParameterMap` SaveParameter carries.
        (
            EGG_SAVE_PARAMETER_PREFIX,
            pal_struct("PalIndividualCharacterSaveParameter"),
        ),
        (
            "worldSaveData.DynamicItemSaveData.CustomVersionData",
            byte_array(),
        ),
    ];
    for (path, data) in entries {
        props::ensure_schema(
            level,
            path.to_string(),
            PropertyTagPartial { id: None, data },
        );
    }

    crate::domain::pal::ensure_save_parameter_schemas(level, EGG_SAVE_PARAMETER_PREFIX);
}

use super::world;

/// Internal read view, not a wire type — `dto::container::CharacterContainerDto`
/// is the response shape callers assemble from this.
pub struct CharacterContainerView {
    pub container_id: uuid::Uuid,
    pub size: i32,
    pub slots: Vec<CharacterContainerSlotDto>,
}

fn container_value_props(level: &crate::ue::Save, entry_index: usize) -> Option<&Properties> {
    let entries = world::character_container_map(level).ok()?;
    props::struct_props(&entries.get(entry_index)?.value)
}

/// A slot's occupant is `RawData.instance_id`; the nil GUID means empty.
pub fn read_character_container(
    level: &crate::ue::Save,
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
                Some(Property::Struct(StructValue::Game(crate::ue::PalStruct::CharacterContainer(raw)))) => {
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

/// Returns the assigned slot index, or `None` when the container is full.
/// Without a `requested_slot`, the first free index below `SlotNum` is used.
pub fn character_container_add_pal(
    level: &mut crate::ue::Save,
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
        Property::Struct(StructValue::Game(crate::ue::PalStruct::CharacterContainer(
            crate::ue::games::palworld::PalCharacterContainer {
                player_uid: props::uuid_to_guid(props::EMPTY_UUID),
                instance_id: props::uuid_to_guid(pal_id),
                permission_tribe_id: 0,
                trailing_bytes: None,
            },
        ))),
    );
    slot_values.push(StructValue::Struct(slot_props));
    Ok(Some(assigned))
}

/// Removes the first slot holding `pal_id`; an absent `pal_id` is a no-op.
pub fn character_container_remove_pal(
    level: &mut crate::ue::Save,
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
            Some(Property::Struct(StructValue::Game(crate::ue::PalStruct::CharacterContainer(raw))))
                if props::guid_to_uuid(&raw.instance_id) == pal_id
        )
    }) {
        slot_values.remove(position);
    }
    Ok(())
}

/// Reads a container's slots, resolving each slot's dynamic item (if any).
/// `game_data` is used only by the Egg branch of `read_dynamic_item`.
pub fn read_item_container(
    level: &crate::ue::Save,
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
            let Some(Property::Struct(StructValue::Game(crate::ue::PalStruct::ItemContainerSlots(raw_slot)))) =
                slot_props.0.get(&PropertyKey::from("RawData"))
            else {
                continue;
            };
            let slot_index = raw_slot.slot_index;
            let count = raw_slot.count;
            let static_id = Some(raw_slot.item.static_id.clone());
            let raw_local_id =
                props::guid_to_uuid(&raw_slot.item.dynamic_id.local_id_in_created_world);
            // A plain stackable item carries the nil GUID here; only a
            // non-nil id backs a `DynamicItemSaveData` entry.
            let dynamic_local_id = (raw_local_id != props::EMPTY_UUID).then_some(raw_local_id);
            let dynamic_item = dynamic_local_id.and_then(|dynamic_local_id| {
                let dynamic_entry_index = *dynamic_index.get(&dynamic_local_id)?;
                read_dynamic_item(level, dynamic_entry_index, dynamic_local_id, game_data)
            });
            // A dangling dynamic-item reference drops the whole slot; a plain
            // (nil-id) item slot is always kept.
            if dynamic_local_id.is_some() && dynamic_item.is_none() {
                continue;
            }
            slots.push(ItemContainerSlotDto {
                dynamic_item,
                slot_index,
                count,
                static_id,
                // Emitted raw: a plain item's nil UUID is serialized as
                // "00000000-…", never as `null`.
                local_id: Some(raw_local_id),
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

/// Decodes one `DynamicItemSaveData` entry. `Unknown` is the wire default for
/// a dynamic item whose payload matches none of the egg/armor/weapon layouts;
/// its DTO carries only `type` and `static_id`.
fn read_dynamic_item(
    level: &crate::ue::Save,
    dynamic_entry_index: usize,
    local_id: uuid::Uuid,
    game_data: &GameData,
) -> Option<DynamicItemDto> {
    let values = world::dynamic_item_values(level).ok()?;
    let StructValue::Struct(item_props) = values.get(dynamic_entry_index)? else {
        return None;
    };
    let Some(Property::Struct(StructValue::Game(crate::ue::PalStruct::DynamicItem(dynamic_item)))) =
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
        crate::ue::games::palworld::PalDynamicItemType::Unknown { .. } => {
            dto.r#type = Some("unknown".to_string());
        }
        crate::ue::games::palworld::PalDynamicItemType::Armor { durability, .. } => {
            dto.r#type = Some("armor".to_string());
            dto.durability = Some(*durability as f64);
        }
        crate::ue::games::palworld::PalDynamicItemType::Weapon {
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
        crate::ue::games::palworld::PalDynamicItemType::Egg {
            character_id,
            object,
            ..
        } => {
            dto.r#type = Some("egg".to_string());
            dto.character_id = Some(character_id.clone());
            dto.character_key = Some(crate::dto::pal::format_character_key(
                character_id,
                super::pal::known_pal_keys(game_data),
            ));
            // An egg's `object` bag carries a "SaveParameter" struct — the same
            // shape a Level.sav pal has — only once its pal data is populated.
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

/// `CustomVersionData` the game writes on a freshly created item-container slot.
const ITEM_CONTAINER_SLOT_CUSTOM_VERSION_DATA: [u8; 24] = [
    1, 0, 0, 0, 126, 180, 234, 18, 154, 27, 90, 255, 113, 170, 113, 188, 223, 51, 214, 14, 1, 0, 0,
    0,
];
/// `CustomVersionData` for a freshly created weapon/armor dynamic item.
const DYNAMIC_ITEM_WEAPON_ARMOR_CUSTOM_VERSION_DATA: [u8; 24] = [
    1, 0, 0, 0, 56, 11, 0, 222, 73, 73, 215, 206, 151, 223, 45, 153, 192, 193, 195, 105, 1, 0, 0, 0,
];
/// `CustomVersionData` for a freshly created egg dynamic item — a second GUID
/// entry longer than the weapon/armor payload.
const DYNAMIC_ITEM_EGG_CUSTOM_VERSION_DATA: [u8; 44] = [
    2, 0, 0, 0, 56, 11, 0, 222, 73, 73, 215, 206, 151, 223, 45, 153, 192, 193, 195, 105, 1, 0, 0,
    0, 108, 246, 252, 15, 153, 72, 144, 17, 248, 156, 96, 177, 94, 71, 70, 74, 1, 0, 0, 0,
];

fn container_slots_mut(
    level: &mut crate::ue::Save,
    entry_index: usize,
) -> Option<&mut Vec<StructValue>> {
    let entries = world::item_container_map_mut(level).ok()?;
    let value_props = props::struct_props_mut(&mut entries.get_mut(entry_index)?.value)?;
    props::get_mut(value_props, &["Slots"]).and_then(props::struct_values_mut)
}

fn raw_container_slot(
    slot: &StructValue,
    slot_index: i32,
) -> Option<&crate::ue::games::palworld::PalItemContainerSlot> {
    let StructValue::Struct(slot_props) = slot else {
        return None;
    };
    match slot_props.0.get(&PropertyKey::from("RawData")) {
        Some(Property::Struct(StructValue::Game(crate::ue::PalStruct::ItemContainerSlots(raw))))
            if raw.slot_index == slot_index =>
        {
            Some(raw)
        }
        _ => None,
    }
}

/// `None` when no slot at `slot_index` exists, or its dynamic id is the nil
/// GUID (a plain, non-dynamic item).
fn raw_slot_local_id(
    level: &crate::ue::Save,
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

fn set_raw_slot_local_id(
    level: &mut crate::ue::Save,
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
        if let Some(Property::Struct(StructValue::Game(crate::ue::PalStruct::ItemContainerSlots(raw)))) =
            slot_props.0.get_mut(&PropertyKey::from("RawData"))
        {
            if raw.slot_index == slot_index {
                raw.item.dynamic_id.local_id_in_created_world = props::uuid_to_guid(local_id);
                return;
            }
        }
    }
}

/// Removes the first `Slots` entry at `slot_index`; an absent index is a no-op.
fn remove_raw_slot(level: &mut crate::ue::Save, entry_index: usize, slot_index: i32) {
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

/// Updates an existing slot's `count`/`static_id`, or appends a fresh one.
/// Never touches the slot's dynamic id — the caller sets that separately, once
/// the dynamic item's final local id is known (see `apply_item_container_dto`).
fn upsert_raw_slot(level: &mut crate::ue::Save, entry_index: usize, slot: &ItemContainerSlotDto) {
    let Some(slots) = container_slots_mut(level, entry_index) else {
        return;
    };
    let existing = slots.iter_mut().find_map(|value| {
        let StructValue::Struct(slot_props) = value else {
            return None;
        };
        match slot_props.0.get_mut(&PropertyKey::from("RawData")) {
            Some(Property::Struct(StructValue::Game(crate::ue::PalStruct::ItemContainerSlots(raw))))
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
                Property::Struct(StructValue::Game(crate::ue::PalStruct::ItemContainerSlots(
                    crate::ue::games::palworld::PalItemContainerSlot {
                        slot_index: slot.slot_index,
                        count: slot.count,
                        item: crate::ue::games::palworld::PalItemId {
                            static_id,
                            dynamic_id: crate::ue::games::palworld::PalDynamicId {
                                created_world_id: crate::ue::FGuid::nil(),
                                local_id_in_created_world: props::uuid_to_guid(props::EMPTY_UUID),
                            },
                        },
                        trailing_bytes: vec![0u8; 16],
                    },
                ))),
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

/// Writes `SlotNum`, then truncates `Slots` to `slot_count` entries by ARRAY
/// POSITION, not by `slot_index` value — the two differ whenever a container's
/// slots are not stored in ascending-`slot_index` order.
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

/// Removes the `DynamicItemSaveData` entry at `local_id` (a no-op if absent).
/// Invalidates `dynamic_item_index` immediately, never deferred: the removal
/// shifts every later entry's position, so any further lookup in the same
/// caller must not resolve through the now-stale index.
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
) -> Option<&crate::ue::games::palworld::PalDynamicItemType<crate::ue::Arch>> {
    let StructValue::Struct(item_props) = values.get(position)? else {
        return None;
    };
    match item_props.0.get(&PropertyKey::from("RawData")) {
        Some(Property::Struct(StructValue::Game(crate::ue::PalStruct::DynamicItem(existing)))) => Some(&existing.item_type),
        _ => None,
    }
}

/// Builds the `{"SaveParameter": {...}}` bag an egg dynamic item's `object`
/// field carries. `Hp` 545000 / `FullStomach` 400.0 are the game's own egg
/// defaults, deliberately different from a hatched pal's (545000 / 300.0).
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

/// `existing` is the current variant at this local id, if any. Its opaque
/// leading/trailing bytes are carried over only when the type is unchanged:
/// each variant's byte arrays have a different, fixed length, so reusing them
/// across a type change would encode a wrong-length payload and corrupt the
/// save. A type change (or a new item) therefore zero-fills them.
fn build_dynamic_item_type(
    dto: &DynamicItemDto,
    existing: Option<&crate::ue::games::palworld::PalDynamicItemType<crate::ue::Arch>>,
) -> crate::ue::games::palworld::PalDynamicItemType<crate::ue::Arch> {
    use crate::ue::games::palworld::PalDynamicItemType;

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
            let (existing_durability, existing_bullets, existing_passives, existing_unknown_str) =
                match existing {
                    Some(PalDynamicItemType::Weapon {
                        durability,
                        remaining_bullets,
                        passive_skill_list,
                        unknown_str,
                        ..
                    }) => (
                        *durability,
                        *remaining_bullets,
                        passive_skill_list.clone(),
                        unknown_str.clone(),
                    ),
                    _ => (0.0, 0, Vec::new(), None),
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
                unknown_str: existing_unknown_str,
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

/// `slot_static_id` is the containing slot's `static_id`, used ONLY when
/// creating a brand-new entry: an existing dynamic item's `static_id` is never
/// rewritten, since the incoming DTO has no authoritative value for it.
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
                if let Some(Property::Struct(StructValue::Game(crate::ue::PalStruct::DynamicItem(existing)))) =
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
                Property::Struct(StructValue::Game(crate::ue::PalStruct::DynamicItem(Box::new(
                    crate::ue::games::palworld::PalDynamicItem {
                        id: crate::ue::games::palworld::PalDynamicId {
                            created_world_id: crate::ue::FGuid::nil(),
                            local_id_in_created_world: props::uuid_to_guid(dto.local_id),
                        },
                        static_id: slot_static_id.to_string(),
                        item_type,
                    },
                )))),
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

/// Resizes the paired common container (essential containers only), cleans up
/// removed dynamic items and `static_id == "None"` slots, then upserts every
/// remaining incoming slot.
///
/// `container_id` is the caller-resolved, session-trusted target; `dto` supplies
/// only `type`/`slots` content, and `dto.id` is never used for routing.
pub fn apply_item_container_dto(
    session: &mut SaveSession,
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

    // Cleanup pass.
    for incoming_slot in &dto.slots {
        let existing_local_id = raw_slot_local_id(
            &session.level,
            container_entry_index,
            incoming_slot.slot_index,
        );
        if incoming_slot.dynamic_item.is_none() {
            if let Some(local_id) = existing_local_id {
                remove_dynamic_item(session, local_id)?;
                // The raw slot keeps pointing at the removed entry; a slot with
                // a dangling reference is dropped on the next read.
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

    // Apply pass.
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

/// Applies a base's storage-container edits and its name/area-range.
///
/// The membership check is load-bearing: a `storage_containers` key this base
/// does not actually own is skipped, so a forged container id cannot redirect a
/// base-storage edit onto an unrelated container elsewhere in the world.
pub fn apply_base_dto(
    session: &mut SaveSession,
    base_id: uuid::Uuid,
    dto: &BaseDto,
) -> Result<(), CoreError> {
    let real_container_ids = super::guild::base_storage_container_ids(session, base_id);
    for (container_id, container_dto) in dto.storage_containers.iter() {
        if !real_container_ids.contains(container_id) {
            continue;
        }
        apply_item_container_dto(session, *container_id, container_dto, None)?;
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
    if let Some(Property::Struct(StructValue::Game(crate::ue::PalStruct::BaseCamp(base_camp)))) =
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

/// For each container id, removes its slots' dynamic items, then the
/// `ItemContainerSaveData` entry itself. An id that doesn't resolve to a real
/// container is a silent no-op.
pub fn delete_item_containers(
    session: &mut SaveSession,
    container_ids: &[uuid::Uuid],
) -> Result<(), CoreError> {
    for &container_id in container_ids {
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
            continue;
        };

        // Collect this container's dynamic-item local ids before removing it.
        let local_ids: Vec<uuid::Uuid> = {
            let entries = world::item_container_map(&session.level)?;
            let mut ids = Vec::new();
            if let Some(value_props) = entries
                .get(entry_index)
                .and_then(|entry| props::struct_props(&entry.value))
            {
                if let Some(slot_values) =
                    props::get(value_props, &["Slots"]).and_then(props::struct_values)
                {
                    for slot_value in slot_values {
                        let StructValue::Struct(slot_props) = slot_value else {
                            continue;
                        };
                        if let Some(Property::Struct(StructValue::Game(crate::ue::PalStruct::ItemContainerSlots(raw)))) =
                            slot_props.0.get(&PropertyKey::from("RawData"))
                        {
                            let local_id =
                                props::guid_to_uuid(&raw.item.dynamic_id.local_id_in_created_world);
                            if local_id != props::EMPTY_UUID {
                                ids.push(local_id);
                            }
                        }
                    }
                }
            }
            ids
        };
        for local_id in local_ids {
            remove_dynamic_item(session, local_id)?;
        }

        let entries = world::item_container_map_mut(&mut session.level)?;
        if entry_index < entries.len() {
            entries.remove(entry_index);
        }
        // The removal above shifts every later entry's position; the next
        // iteration must rebuild the index.
        session.caches.item_container_index = None;
    }
    Ok(())
}

/// Removes every `CharacterContainerSaveData` entry whose key matches one of
/// `container_ids`; an id that isn't present is a silent no-op.
pub fn delete_character_containers(
    session: &mut SaveSession,
    container_ids: &[uuid::Uuid],
) -> Result<(), CoreError> {
    let targets: std::collections::HashSet<uuid::Uuid> = container_ids.iter().copied().collect();
    let entries = world::character_container_map_mut(&mut session.level)?;
    entries.retain(|entry| {
        props::struct_props(&entry.key)
            .and_then(|key| key.0.get(&PropertyKey::from("ID")))
            .and_then(props::as_uuid)
            .map(|id| !targets.contains(&id))
            .unwrap_or(true)
    });
    session.caches.character_container_index = None;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ue::{Header, PackageVersion, PropertySchemas, Root, Save, ValueVec};

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
            Property::Struct(StructValue::Game(crate::ue::PalStruct::CharacterContainer(
                crate::ue::games::palworld::PalCharacterContainer {
                    player_uid: props::uuid_to_guid(props::EMPTY_UUID),
                    instance_id: props::uuid_to_guid(pal_id),
                    permission_tribe_id: 0,
                    trailing_bytes: None,
                },
            ))),
        );
        StructValue::Struct(slot_props)
    }

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
            Property::Map(vec![crate::ue::MapEntry {
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

    /// Pins the invariant this module's doc comment relies on: slot mutations
    /// never move a container's position in `CharacterContainerSaveData`.
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
                crate::ue::MapEntry {
                    key: struct_property(first_key),
                    value: struct_property(first_value),
                },
                crate::ue::MapEntry {
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

    fn game_data() -> GameData {
        let json_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/json");
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
            Property::Struct(StructValue::Game(crate::ue::PalStruct::ItemContainerSlots(
                crate::ue::games::palworld::PalItemContainerSlot {
                    slot_index,
                    count,
                    item: crate::ue::games::palworld::PalItemId {
                        static_id: static_id.to_string(),
                        dynamic_id: crate::ue::games::palworld::PalDynamicId {
                            created_world_id: crate::ue::FGuid::nil(),
                            local_id_in_created_world: props::uuid_to_guid(local_id),
                        },
                    },
                    trailing_bytes: Vec::new(),
                },
            ))),
        );
        StructValue::Struct(slot_props)
    }

    fn dynamic_item_entry(
        local_id: uuid::Uuid,
        static_id: &str,
        item_type: crate::ue::games::palworld::PalDynamicItemType<crate::ue::Arch>,
    ) -> StructValue {
        let dynamic_item = crate::ue::games::palworld::PalDynamicItem {
            id: crate::ue::games::palworld::PalDynamicId {
                created_world_id: crate::ue::FGuid::nil(),
                local_id_in_created_world: props::uuid_to_guid(local_id),
            },
            static_id: static_id.to_string(),
            item_type,
        };
        let mut item_props = Properties::default();
        item_props.insert(
            "RawData",
            Property::Struct(StructValue::Game(crate::ue::PalStruct::DynamicItem(Box::new(dynamic_item)))),
        );
        StructValue::Struct(item_props)
    }

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
            Property::Map(vec![crate::ue::MapEntry {
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
        let mut caches = WorldCaches::default();
        let data = game_data();

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
        let mut caches = WorldCaches::default();
        let data = game_data();

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
        let mut caches = WorldCaches::default();
        let data = game_data();

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
        // A plain stackable item's `local_id` is the raw nil UUID, not null.
        assert_eq!(dto.slots[0].local_id, Some(props::EMPTY_UUID));
        assert_eq!(
            serde_json::to_value(&dto.slots[0]).unwrap()["local_id"],
            serde_json::json!("00000000-0000-0000-0000-000000000000"),
            "a plain slot's local_id must serialize as the nil UUID string, not null"
        );
        assert!(dto.slots[0].dynamic_item.is_none());
    }

    #[test]
    fn read_item_container_drops_a_slot_whose_dynamic_item_is_missing() {
        let container_id = uuid::Uuid::nil();
        let dangling_local_id =
            uuid::Uuid::parse_str("dddddddd-0000-0000-0000-000000000000").unwrap();
        let slot = item_container_slot(0, 1, "SomeWeapon", dangling_local_id);
        let save = save_with_item_container(container_id, 10, vec![slot], vec![]);
        let mut caches = WorldCaches::default();
        let data = game_data();

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
            crate::ue::games::palworld::PalDynamicItemType::Weapon {
                leading_bytes: [0; 4],
                durability: 80.5,
                remaining_bullets: 12,
                passive_skill_list: vec!["Rare".to_string()],
                unknown_str: None,
                trailing_bytes: [0; 4],
            },
        );
        let save = save_with_item_container(container_id, 10, vec![slot], vec![weapon]);
        let mut caches = WorldCaches::default();
        let data = game_data();

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
            crate::ue::games::palworld::PalDynamicItemType::Armor {
                leading_bytes: [0; 4],
                durability: 42.0,
                trailing_bytes: [0; 4],
            },
        );
        let save = save_with_item_container(container_id, 10, vec![slot], vec![armor]);
        let mut caches = WorldCaches::default();
        let data = game_data();

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
            crate::ue::games::palworld::PalDynamicItemType::Unknown {
                trailer: vec![1, 2, 3],
            },
        );
        let save = save_with_item_container(container_id, 10, vec![slot], vec![unknown]);
        let mut caches = WorldCaches::default();
        let data = game_data();

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
            crate::ue::games::palworld::PalDynamicItemType::Egg {
                leading_bytes: [0; 4],
                character_id: "SheepBall".to_string(),
                object,
                trailing_bytes: [0; 28],
            },
        );
        let save = save_with_item_container(container_id, 10, vec![slot], vec![egg]);
        let mut caches = WorldCaches::default();
        let data = game_data();

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

    /// An egg whose `object` carries no `"SaveParameter"` yet leaves every
    /// save-parameter-derived field at its `None` default.
    #[test]
    fn read_item_container_egg_without_save_parameter_leaves_stats_none() {
        let container_id = uuid::Uuid::nil();
        let local_id = uuid::Uuid::parse_str("fffffffa-0000-0000-0000-000000000000").unwrap();
        let slot = item_container_slot(0, 1, "EggSheepBall", local_id);
        let egg = dynamic_item_entry(
            local_id,
            "EggSheepBall",
            crate::ue::games::palworld::PalDynamicItemType::Egg {
                leading_bytes: [0; 4],
                character_id: "SheepBall".to_string(),
                object: Properties::default(),
                trailing_bytes: [0; 28],
            },
        );
        let save = save_with_item_container(container_id, 10, vec![slot], vec![egg]);
        let mut caches = WorldCaches::default();
        let data = game_data();

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

    #[test]
    fn apply_item_container_dto_adds_a_new_dynamic_item_and_invalidates_the_cache() {
        let container_id = uuid::Uuid::parse_str("11111111-0000-0000-0000-000000000000").unwrap();
        let mut session = session_with_item_container(container_id, 10, vec![], vec![]);
        // Warm the cache with the pre-edit state, so the assertions below prove
        // the stale index is discarded rather than merely never built.
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
        apply_item_container_dto(&mut session, container_id, &dto, None).unwrap();

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

    #[test]
    fn apply_item_container_dto_removes_a_dynamic_item_and_invalidates_the_cache() {
        let container_id = uuid::Uuid::nil();
        let local_id = uuid::Uuid::parse_str("33333333-0000-0000-0000-000000000000").unwrap();
        let slot = item_container_slot(0, 1, "SFBow_5", local_id);
        let weapon = dynamic_item_entry(
            local_id,
            "SFBow_5",
            crate::ue::games::palworld::PalDynamicItemType::Weapon {
                leading_bytes: [0; 4],
                durability: 80.0,
                remaining_bullets: 12,
                passive_skill_list: vec![],
                unknown_str: None,
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
        apply_item_container_dto(&mut session, container_id, &dto, None).unwrap();

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

    /// Each `AdditionalInventory_*` slot in an essential container grows its
    /// paired common container by 3 slots over the base 42, capped at 4.
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
                crate::ue::MapEntry {
                    key: struct_property(key_common),
                    value: struct_property(value_common),
                },
                crate::ue::MapEntry {
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
        apply_item_container_dto(&mut session, essential_id, &dto, Some(common_id)).unwrap();

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

    /// Shrinking below the container's current slot count removes the excess
    /// entries by array position.
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
                crate::ue::MapEntry {
                    key: struct_property(key_common),
                    value: struct_property(value_common),
                },
                crate::ue::MapEntry {
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
        apply_item_container_dto(&mut session, essential_id, &dto, Some(common_id)).unwrap();

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
        apply_item_container_dto(&mut session, container_id, &dto, None).unwrap();

        let entries = world::item_container_map(&session.level).unwrap();
        let value_props = props::struct_props(&entries[0].value).unwrap();
        let slots = props::get(value_props, &["Slots"])
            .and_then(props::struct_values)
            .unwrap();
        assert!(slots.is_empty());
    }

    /// An unresolvable `container_id` is skipped, never a panic.
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
        apply_item_container_dto(&mut session, unknown, &dto, None).unwrap();
        // The real (nil-id) container must be completely untouched.
        let entries = world::item_container_map(&session.level).unwrap();
        let value_props = props::struct_props(&entries[0].value).unwrap();
        assert!(props::get(value_props, &["Slots"])
            .and_then(props::struct_values)
            .unwrap()
            .is_empty());
    }

    /// A new egg with `modified: true` rebuilds a full embedded `SaveParameter`,
    /// readable back through `read_item_container`.
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
        apply_item_container_dto(&mut session, container_id, &dto, None).unwrap();
        let _ = local_id; // not the minted id; kept for readability of intent

        let mut caches = WorldCaches::default();
        let data = game_data();
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

    /// An existing egg updated with `modified: false` preserves its embedded
    /// `object` exactly, even when the incoming DTO carries different stats.
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
            crate::ue::games::palworld::PalDynamicItemType::Egg {
                leading_bytes: [0; 4],
                character_id: "SheepBall".to_string(),
                object,
                trailing_bytes: [0; 28],
            },
        );
        let mut session = session_with_item_container(container_id, 10, vec![slot], vec![egg]);

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
        apply_item_container_dto(&mut session, container_id, &dto, None).unwrap();

        let mut caches = WorldCaches::default();
        let data = game_data();
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

    /// `PalMapModel.initial_transform_cache` needs a value; no test inspects it.
    fn zero_map_transform() -> crate::ue::games::palworld::PalTransform {
        crate::ue::games::palworld::PalTransform {
            rotation: crate::ue::Quat {
                x: crate::ue::Double(0.0),
                y: crate::ue::Double(0.0),
                z: crate::ue::Double(0.0),
                w: crate::ue::Double(1.0),
            },
            translation: crate::ue::Vector {
                x: crate::ue::Double(0.0),
                y: crate::ue::Double(0.0),
                z: crate::ue::Double(0.0),
            },
            scale: crate::ue::Vector {
                x: crate::ue::Double(1.0),
                y: crate::ue::Double(1.0),
                z: crate::ue::Double(1.0),
            },
        }
    }

    /// One `MapObjectSaveData` element linking `base_id` to `container_id` via
    /// an `ItemContainer` module — the shape `guild::base_storage_container_ids`
    /// enumerates to decide base ownership.
    fn map_object_owning_container(base_id: uuid::Uuid, container_id: uuid::Uuid) -> StructValue {
        let model = crate::ue::games::palworld::PalMapModel {
            instance_id: crate::ue::FGuid::nil(),
            concrete_model_instance_id: crate::ue::FGuid::nil(),
            base_camp_id_belong_to: props::uuid_to_guid(base_id),
            group_id_belong_to: crate::ue::FGuid::nil(),
            hp: crate::ue::games::palworld::PalMapObjectHp { current: 0, max: 0 },
            initial_transform_cache: zero_map_transform(),
            repair_work_id: crate::ue::FGuid::nil(),
            owner_spawner_level_object_instance_id: crate::ue::FGuid::nil(),
            owner_instance_id: crate::ue::FGuid::nil(),
            build_player_uid: crate::ue::FGuid::nil(),
            interact_restrict_type: 0,
            deterioration_damage: 0.0,
            stage_instance_id_belong_to: crate::ue::games::palworld::PalStageInstanceId {
                id: crate::ue::FGuid::nil(),
                valid: 0,
            },
            unknown_bytes: vec![],
        };
        let mut model_props = Properties::default();
        model_props.insert(
            "RawData",
            Property::Struct(StructValue::Game(crate::ue::PalStruct::MapModel(Box::new(model)))),
        );

        let module = crate::ue::games::palworld::PalMapConcreteModelModule {
            module_type: "EPalMapObjectConcreteModelModuleType::ItemContainer".to_string(),
            data: crate::ue::games::palworld::PalMapConcreteModelModuleData::ItemContainer {
                target_container_id: props::uuid_to_guid(container_id),
                slot_attribute_indexes: vec![],
                all_slot_attribute: vec![],
                drop_item_at_disposed: 0,
                usage_type: 0,
                trailing_bytes: [0; 4],
            },
            custom_version_data: vec![],
        };
        let mut module_value_props = Properties::default();
        module_value_props.insert(
            "RawData",
            Property::Struct(StructValue::Game(crate::ue::PalStruct::MapConcreteModelModule(module))),
        );
        let module_entries = vec![crate::ue::MapEntry {
            key: Property::Enum("EPalMapObjectConcreteModelModuleType::ItemContainer".to_string()),
            value: Property::Struct(StructValue::Struct(module_value_props)),
        }];
        let mut concrete_props = Properties::default();
        concrete_props.insert("ModuleMap", Property::Map(module_entries));

        let mut object_props = Properties::default();
        object_props.insert("Model", Property::Struct(StructValue::Struct(model_props)));
        object_props.insert(
            "ConcreteModel",
            Property::Struct(StructValue::Struct(concrete_props)),
        );
        StructValue::Struct(object_props)
    }

    /// Two item containers: one linked to `base_id` by a `MapObjectSaveData`
    /// module, one with no link at all — a positive and a negative for
    /// `apply_base_dto`'s membership check in the same fixture.
    fn session_with_base_owning_one_of_two_containers(
        base_id: uuid::Uuid,
        owned_container_id: uuid::Uuid,
        foreign_container_id: uuid::Uuid,
    ) -> SaveSession {
        let mut key_owned = Properties::default();
        key_owned.insert("ID", guid_property(owned_container_id));
        let mut value_owned = Properties::default();
        value_owned.insert("SlotNum", props::int_property(5));
        value_owned.insert("Slots", Property::Array(ValueVec::Struct(vec![])));

        let mut key_foreign = Properties::default();
        key_foreign.insert("ID", guid_property(foreign_container_id));
        let mut value_foreign = Properties::default();
        value_foreign.insert("SlotNum", props::int_property(5));
        value_foreign.insert("Slots", Property::Array(ValueVec::Struct(vec![])));

        let mut world_save_data = Properties::default();
        world_save_data.insert(
            "ItemContainerSaveData",
            Property::Map(vec![
                crate::ue::MapEntry {
                    key: struct_property(key_owned),
                    value: struct_property(value_owned),
                },
                crate::ue::MapEntry {
                    key: struct_property(key_foreign),
                    value: struct_property(value_foreign),
                },
            ]),
        );
        world_save_data.insert(
            "DynamicItemSaveData",
            Property::Array(ValueVec::Struct(vec![])),
        );
        world_save_data.insert(
            "MapObjectSaveData",
            Property::Array(ValueVec::Struct(vec![map_object_owning_container(
                base_id,
                owned_container_id,
            )])),
        );
        let mut root_properties = Properties::default();
        root_properties.insert("worldSaveData", struct_property(world_save_data));
        SaveSession::new_for_tests(SaveKind::InMemory, minimal_save(root_properties))
    }

    /// The membership check must accept an owned container and reject a foreign
    /// one in the same call — proving it is not an always-reject.
    #[test]
    fn apply_base_dto_accepts_the_owned_container_and_rejects_the_foreign_one() {
        let base_id = uuid::Uuid::parse_str("bbbbbbbb-0000-0000-0000-000000000000").unwrap();
        let owned_container_id =
            uuid::Uuid::parse_str("dddddddd-0000-0000-0000-000000000000").unwrap();
        let foreign_container_id =
            uuid::Uuid::parse_str("cccccccc-0000-0000-0000-000000000000").unwrap();
        let mut session = session_with_base_owning_one_of_two_containers(
            base_id,
            owned_container_id,
            foreign_container_id,
        );

        let mut storage_containers = crate::dto::ordered_map::OrderedMap::new();
        storage_containers.insert(
            owned_container_id,
            ItemContainerDto {
                id: owned_container_id,
                r#type: "BaseContainer".to_string(),
                slots: vec![slot_dto(0, 5, "Wood", None)],
                key: None,
                slot_num: 5,
            },
        );
        storage_containers.insert(
            foreign_container_id,
            ItemContainerDto {
                id: foreign_container_id,
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
        apply_base_dto(&mut session, base_id, &base_dto).unwrap();

        let entries = world::item_container_map(&session.level).unwrap();
        let owned_slots = entries
            .iter()
            .find(|entry| {
                props::struct_props(&entry.key)
                    .and_then(|key| props::get(key, &["ID"]))
                    .and_then(props::as_uuid)
                    == Some(owned_container_id)
            })
            .and_then(|entry| props::struct_props(&entry.value))
            .and_then(|value_props| props::get(value_props, &["Slots"]))
            .and_then(props::struct_values)
            .unwrap();
        assert_eq!(
            owned_slots.len(),
            1,
            "the container this base genuinely owns must actually be mutated"
        );
        let raw = raw_container_slot(&owned_slots[0], 0).expect("real slot written");
        assert_eq!(raw.count, 5, "the real edit's content must have landed");
        assert_eq!(raw.item.static_id, "Wood");

        let foreign_slots = entries
            .iter()
            .find(|entry| {
                props::struct_props(&entry.key)
                    .and_then(|key| props::get(key, &["ID"]))
                    .and_then(props::as_uuid)
                    == Some(foreign_container_id)
            })
            .and_then(|entry| props::struct_props(&entry.value))
            .and_then(|value_props| props::get(value_props, &["Slots"]))
            .and_then(props::struct_values)
            .unwrap();
        assert!(
            foreign_slots.is_empty(),
            "a container id outside this base's real storage set must never be mutated, \
             even when a genuinely-owned container is edited in the SAME call"
        );
    }
}
