use uuid::Uuid;

use super::{
    scrub, transform, BaseBlueprint, BlueprintHeader, BlueprintStructure, CaptureOptions,
    SCHEMA_VERSION,
};
use crate::domain::{guild, world};
use crate::error::CoreError;
use crate::props;
use crate::session::SaveSession;
use crate::ue::games::palworld::{
    PalMapConcreteModelModuleData, PalMapConcreteModelVariant, PalMapModel, PalTransform,
};
use crate::ue::{FGuid, MapEntry, PalStruct, Properties, Property, PropertyKey, StructValue, ValueVec};

/// `true` only when `MapObjectId` is still a `Property::Name` -- the
/// regression the Task 3/6 amendment exists to prevent (`MapObjectId`
/// silently collapsing to `Property::Byte(Label(..))`).
pub fn map_object_id_is_name_property(properties: &Properties) -> bool {
    matches!(
        properties.0.get(&PropertyKey::from("MapObjectId")),
        Some(Property::Name(_))
    )
}

fn map_object_model(object_props: &Properties) -> Option<&PalMapModel> {
    let model = object_props.0.get(&PropertyKey::from("Model")).and_then(props::struct_props)?;
    match model.0.get(&PropertyKey::from("RawData"))? {
        Property::Struct(StructValue::Game(PalStruct::MapModel(model))) => Some(model),
        _ => None,
    }
}

pub fn first_build_player_uid(blueprint: &BaseBlueprint) -> Option<Uuid> {
    blueprint
        .structures
        .iter()
        .find_map(|s| map_object_model(&s.properties).map(|m| props::guid_to_uuid(&m.build_player_uid)))
}

/// Every structure's `Model.RawData.build_player_uid`, one entry per
/// structure that has a `Model` at all.
pub fn structure_build_player_uids(blueprint: &BaseBlueprint) -> Vec<Uuid> {
    blueprint
        .structures
        .iter()
        .filter_map(|s| map_object_model(&s.properties))
        .map(|m| props::guid_to_uuid(&m.build_player_uid))
        .collect()
}

/// Every player-identifying UID a structure's `ConcreteModel` may carry: the
/// per-variant ownership/lock fields plus the `ModuleMap`'s `PasswordLock`
/// player list. Used by tests to assert none survive scrubbing; deliberately
/// separate from `scrub`'s own logic so a bug in one is unlikely to be
/// mirrored in the other.
pub fn structure_concrete_player_uids(properties: &Properties) -> Vec<Uuid> {
    let mut uids = Vec::new();
    let Some(concrete) =
        properties.0.get(&PropertyKey::from("ConcreteModel")).and_then(props::struct_props)
    else {
        return uids;
    };

    if let Some(Property::Struct(StructValue::Game(PalStruct::MapConcreteModel(raw)))) =
        concrete.0.get(&PropertyKey::from("RawData"))
    {
        match &raw.model_data {
            PalMapConcreteModelVariant::ItemChest(model) => {
                uids.push(props::guid_to_uuid(&model.private_lock_player_uid));
            }
            PalMapConcreteModelVariant::ItemChestAffectCorruption(model) => {
                uids.push(props::guid_to_uuid(&model.private_lock_player_uid));
            }
            PalMapConcreteModelVariant::ItemBooth(model) => {
                uids.push(props::guid_to_uuid(&model.private_lock_player_uid));
                for trade in &model.trade_infos {
                    uids.push(props::guid_to_uuid(&trade.seller_player_uid));
                }
            }
            PalMapConcreteModelVariant::DeathDroppedCharacter(model) => {
                uids.push(props::guid_to_uuid(&model.owner_player_uid));
            }
            PalMapConcreteModelVariant::DeathPenaltyStorage(model) => {
                uids.push(props::guid_to_uuid(&model.owner_player_uid));
            }
            PalMapConcreteModelVariant::DropItem(model) => {
                uids.push(props::guid_to_uuid(&model.pickupable_player_uid));
            }
            PalMapConcreteModelVariant::Signboard(model) => {
                uids.push(props::guid_to_uuid(&model.last_modified_player_uid));
            }
            PalMapConcreteModelVariant::PalEgg(model) => {
                uids.push(props::guid_to_uuid(&model.pickupdable_player_uid));
            }
            _ => {}
        }
    }

    if let Some(module_entries) =
        concrete.0.get(&PropertyKey::from("ModuleMap")).and_then(props::map_entries)
    {
        for module in module_entries {
            let Some(module_props) = props::struct_props(&module.value) else { continue };
            if let Some(Property::Struct(StructValue::Game(PalStruct::MapConcreteModelModule(raw)))) =
                module_props.0.get(&PropertyKey::from("RawData"))
            {
                if let PalMapConcreteModelModuleData::PasswordLock { player_infos, .. } = &raw.data {
                    for info in player_infos {
                        uids.push(props::guid_to_uuid(&info.player_uid));
                    }
                }
            }
        }
    }

    uids
}

/// A captured `CharacterSaveParameterMap` entry's three player-identifying
/// UIDs: the key's `PlayerUId`, the `SaveParameter` bag's `OwnerPlayerUId`,
/// and every UID in `OldOwnerPlayerUIds`. Missing fields read as the nil
/// UUID. Used by tests to assert scrubbing left nothing behind.
pub fn character_entry_player_uids(entry: &MapEntry) -> (Uuid, Uuid, Vec<Uuid>) {
    let key_uid = world::entry_player_uid(entry).unwrap_or(Uuid::nil());
    let save_parameter = world::entry_save_parameter(entry);
    let owner_uid = save_parameter
        .and_then(|params| props::get(params, &["OwnerPlayerUId"]))
        .and_then(props::as_uuid)
        .unwrap_or(Uuid::nil());

    let mut old_owner_uids = Vec::new();
    if let Some(Property::Array(ValueVec::Struct(values))) =
        save_parameter.and_then(|params| params.0.get(&PropertyKey::from("OldOwnerPlayerUIds")))
    {
        for value in values {
            if let StructValue::Guid(guid) = value {
                old_owner_uids.push(props::guid_to_uuid(guid));
            }
        }
    }

    (key_uid, owner_uid, old_owner_uids)
}

pub fn capture(
    session: &SaveSession,
    base_id: Uuid,
    options: CaptureOptions,
    name: &str,
) -> Result<BaseBlueprint, CoreError> {
    let mut blueprint = capture_unscrubbed(session, base_id, options, name)?;
    scrub::scrub_blueprint(&mut blueprint);
    Ok(blueprint)
}

pub fn capture_unscrubbed(
    session: &SaveSession,
    base_id: Uuid,
    options: CaptureOptions,
    name: &str,
) -> Result<BaseBlueprint, CoreError> {
    let (base_camp_props, anchor, area_range, base_name) = base_camp_of(session, base_id)?;

    let mut structures = Vec::new();
    let mut item_container_ids: Vec<Uuid> = Vec::new();
    let mut housed_container_ids: Vec<Uuid> = Vec::new();
    if let Some(map_objects) = world::map_object_values(&session.level)? {
        for value in map_objects {
            let StructValue::Struct(object_props) = value else { continue };
            let Some(model) = map_object_model(object_props) else { continue };
            if props::guid_to_uuid(&model.base_camp_id_belong_to) != base_id {
                continue;
            }
            let Some(map_object_id) = object_props
                .0
                .get(&PropertyKey::from("MapObjectId"))
                .and_then(props::as_str)
            else {
                continue;
            };
            let relative_transform =
                transform::to_relative(&anchor, &model.initial_transform_cache);

            let (module_item_ids, module_character_ids) = module_target_container_ids(object_props);
            if options.container_contents {
                push_unique(&mut item_container_ids, &module_item_ids);
            }
            if options.housed_pals {
                push_unique(&mut housed_container_ids, &module_character_ids);
            }

            let mut properties = object_props.clone();
            apply_layer_gating(&mut properties, options);

            structures.push(BlueprintStructure {
                map_object_id: map_object_id.to_string(),
                relative_transform,
                properties,
            });
        }
    }

    let works = works_of(session, base_id, options)?;

    let mut item_containers = Vec::new();
    let mut dynamic_item_ids: Vec<Uuid> = Vec::new();
    if options.container_contents {
        let entries = world::item_container_map(&session.level)?;
        for container_id in &item_container_ids {
            let Some(entry) = entries.iter().find(|entry| container_entry_id(entry) == Some(*container_id))
            else {
                continue;
            };
            push_unique(&mut dynamic_item_ids, &container_slot_dynamic_item_ids(entry));
            item_containers.push(entry.clone());
        }
    }

    let mut dynamic_items = Vec::new();
    if options.container_contents {
        for value in world::dynamic_item_values(&session.level)? {
            let Some(id) = dynamic_item_local_id(value) else { continue };
            if dynamic_item_ids.contains(&id) {
                dynamic_items.push(value.clone());
            }
        }
    }

    let mut character_containers = Vec::new();
    let mut character_instance_ids: Vec<Uuid> = Vec::new();
    if options.worker_pals {
        if let Some(base_entry) = base_camp_entry(session, base_id)? {
            if let Some((_guild_id, worker_container_id)) = guild::base_guild_and_container(base_entry) {
                let entries = world::character_container_map(&session.level)?;
                if let Some(entry) =
                    entries.iter().find(|entry| container_entry_id(entry) == Some(worker_container_id))
                {
                    push_unique(
                        &mut character_instance_ids,
                        &character_container_slot_instance_ids(entry),
                    );
                    character_containers.push(entry.clone());
                }
            }
        }
    }
    if options.housed_pals {
        let entries = world::character_container_map(&session.level)?;
        for container_id in &housed_container_ids {
            if character_containers.iter().any(|entry| container_entry_id(entry) == Some(*container_id)) {
                continue;
            }
            let Some(entry) = entries.iter().find(|entry| container_entry_id(entry) == Some(*container_id))
            else {
                continue;
            };
            push_unique(&mut character_instance_ids, &character_container_slot_instance_ids(entry));
            character_containers.push(entry.clone());
        }
    }

    let mut characters = Vec::new();
    if !character_instance_ids.is_empty() {
        let entries = world::character_map(&session.level)?;
        for instance_id in &character_instance_ids {
            if let Some(entry) = entries.iter().find(|entry| world::entry_instance_id(entry) == Some(*instance_id)) {
                characters.push(entry.clone());
            }
        }
    }

    Ok(BaseBlueprint {
        source_header: session.level.header.clone(),
        header: BlueprintHeader {
            schema_version: SCHEMA_VERSION,
            game_data_version: String::new(),
            uesave_struct_version: env!("CARGO_PKG_VERSION").to_string(),
            manifest: options,
            name: name.to_string(),
            source_world: session.world_name.clone(),
            source_base: if options.base_identity { base_name } else { String::new() },
            created_at: 0,
            structure_count: structures.len() as u32,
            footprint_radius: area_range,
            anchor_height_above_terrain: 0.0,
        },
        base_camp: Some(base_camp_props),
        structures,
        item_containers,
        character_containers,
        characters,
        works,
        dynamic_items,
    })
}

fn push_unique(target: &mut Vec<Uuid>, ids: &[Uuid]) {
    for id in ids {
        if !target.contains(id) {
            target.push(*id);
        }
    }
}

/// `target_container_id` from every `ItemContainer`/`CharacterContainer`
/// module in a structure's `ConcreteModel.ModuleMap`, as `(item_ids,
/// character_ids)`.
fn module_target_container_ids(properties: &Properties) -> (Vec<Uuid>, Vec<Uuid>) {
    let mut item_ids = Vec::new();
    let mut character_ids = Vec::new();
    let Some(concrete) =
        properties.0.get(&PropertyKey::from("ConcreteModel")).and_then(props::struct_props)
    else {
        return (item_ids, character_ids);
    };
    let Some(module_entries) =
        concrete.0.get(&PropertyKey::from("ModuleMap")).and_then(props::map_entries)
    else {
        return (item_ids, character_ids);
    };
    for module in module_entries {
        let Some(module_props) = props::struct_props(&module.value) else { continue };
        let Some(Property::Struct(StructValue::Game(PalStruct::MapConcreteModelModule(raw)))) =
            module_props.0.get(&PropertyKey::from("RawData"))
        else {
            continue;
        };
        match &raw.data {
            PalMapConcreteModelModuleData::ItemContainer { target_container_id, .. } => {
                item_ids.push(props::guid_to_uuid(target_container_id));
            }
            PalMapConcreteModelModuleData::CharacterContainer { target_container_id, .. } => {
                character_ids.push(props::guid_to_uuid(target_container_id));
            }
            _ => {}
        }
    }
    (item_ids, character_ids)
}

/// `ItemContainerSaveData`/`CharacterContainerSaveData` both key by
/// `key.ID`.
pub fn container_entry_id(entry: &MapEntry) -> Option<Uuid> {
    props::get(props::struct_props(&entry.key)?, &["ID"]).and_then(props::as_uuid)
}

/// The non-nil `local_id_in_created_world` of every occupied slot in an
/// `ItemContainerSaveData` entry.
pub fn container_slot_dynamic_item_ids(entry: &MapEntry) -> Vec<Uuid> {
    let mut ids = Vec::new();
    let Some(value_props) = props::struct_props(&entry.value) else { return ids };
    let Some(slots) = props::get(value_props, &["Slots"]).and_then(props::struct_values) else {
        return ids;
    };
    for slot in slots {
        let StructValue::Struct(slot_props) = slot else { continue };
        if let Some(Property::Struct(StructValue::Game(PalStruct::ItemContainerSlots(raw)))) =
            slot_props.0.get(&PropertyKey::from("RawData"))
        {
            let id = props::guid_to_uuid(&raw.item.dynamic_id.local_id_in_created_world);
            if id != props::EMPTY_UUID {
                ids.push(id);
            }
        }
    }
    ids
}

/// The non-nil `instance_id` of every occupied slot in a
/// `CharacterContainerSaveData` entry.
fn character_container_slot_instance_ids(entry: &MapEntry) -> Vec<Uuid> {
    let mut ids = Vec::new();
    let Some(value_props) = props::struct_props(&entry.value) else { return ids };
    let Some(slots) = props::get(value_props, &["Slots"]).and_then(props::struct_values) else {
        return ids;
    };
    for slot in slots {
        let StructValue::Struct(slot_props) = slot else { continue };
        if let Some(Property::Struct(StructValue::Game(PalStruct::CharacterContainer(raw)))) =
            slot_props.0.get(&PropertyKey::from("RawData"))
        {
            let id = props::guid_to_uuid(&raw.instance_id);
            if id != props::EMPTY_UUID {
                ids.push(id);
            }
        }
    }
    ids
}

/// A `DynamicItemSaveData` element's `RawData.id.local_id_in_created_world`.
pub fn dynamic_item_local_id(value: &StructValue) -> Option<Uuid> {
    let StructValue::Struct(item_props) = value else { return None };
    match item_props.0.get(&PropertyKey::from("RawData")) {
        Some(Property::Struct(StructValue::Game(PalStruct::DynamicItem(dynamic_item)))) => {
            Some(props::guid_to_uuid(&dynamic_item.id.local_id_in_created_world))
        }
        _ => None,
    }
}

/// `BaseCampSaveData`'s full `MapEntry` for `base_id`, needed (rather than
/// just its `Properties`) so `guild::base_guild_and_container` can read the
/// `WorkerDirector` raw byte blob alongside `RawData`.
fn base_camp_entry(session: &SaveSession, base_id: Uuid) -> Result<Option<&MapEntry>, CoreError> {
    Ok(world::base_camp_map(&session.level)?
        .and_then(|entries| entries.iter().find(|entry| props::as_uuid(&entry.key) == Some(base_id))))
}

/// Locates `base_id`'s `BaseCampSaveData` entry and reads its typed
/// `PalStruct::BaseCamp` raw data: the anchor transform, footprint radius,
/// and display name every captured structure is made relative to.
fn base_camp_of(
    session: &SaveSession,
    base_id: Uuid,
) -> Result<(Properties, PalTransform, f64, String), CoreError> {
    let not_found = || CoreError::Parse(format!("base {base_id} not found"));

    let entries = world::base_camp_map(&session.level)?.ok_or_else(not_found)?;
    let entry = entries
        .iter()
        .find(|entry| props::as_uuid(&entry.key) == Some(base_id))
        .ok_or_else(not_found)?;
    let value_props = props::struct_props(&entry.value).ok_or_else(not_found)?;
    match value_props.0.get(&PropertyKey::from("RawData")) {
        Some(Property::Struct(StructValue::Game(PalStruct::BaseCamp(raw)))) => Ok((
            value_props.clone(),
            raw.transform.clone(),
            raw.area_range as f64,
            raw.name.clone(),
        )),
        _ => Err(not_found()),
    }
}

fn with_concrete_variant_mut(
    properties: &mut Properties,
    f: impl FnOnce(&mut PalMapConcreteModelVariant<crate::ue::Arch>),
) {
    let Some(concrete) = properties
        .0
        .get_mut(&PropertyKey::from("ConcreteModel"))
        .and_then(props::struct_props_mut)
    else {
        return;
    };
    if let Some(Property::Struct(StructValue::Game(PalStruct::MapConcreteModel(raw)))) =
        concrete.0.get_mut(&PropertyKey::from("RawData"))
    {
        f(&mut raw.model_data);
    }
}

fn reset_structure_condition(properties: &mut Properties) {
    let Some(model) = properties
        .0
        .get_mut(&PropertyKey::from("Model"))
        .and_then(props::struct_props_mut)
    else {
        return;
    };
    if let Some(Property::Struct(StructValue::Game(PalStruct::MapModel(raw)))) =
        model.0.get_mut(&PropertyKey::from("RawData"))
    {
        raw.hp.current = raw.hp.max;
        raw.deterioration_damage = 0.0;
    }
}

fn clear_production_config(properties: &mut Properties) {
    with_concrete_variant_mut(properties, |variant| match variant {
        PalMapConcreteModelVariant::ConvertItem(model) => {
            model.current_recipe_id.clear();
            model.requested_product_num = 0;
        }
        PalMapConcreteModelVariant::FarmBlockV2(model) => {
            model.crop_data_id.clear();
        }
        PalMapConcreteModelVariant::Signboard(model) => {
            model.signboard_text.clear();
        }
        PalMapConcreteModelVariant::DefenseBulletLauncher(model) => {
            model.bullet_item_name.clear();
        }
        _ => {}
    });
}

fn clear_production_progress(properties: &mut Properties) {
    with_concrete_variant_mut(properties, |variant| match variant {
        PalMapConcreteModelVariant::FarmBlockV2(model) => {
            model.crop_progress_rate = 0.0;
        }
        PalMapConcreteModelVariant::EnergyStorage(model) => {
            model.stored_energy_amount = 0.0;
        }
        PalMapConcreteModelVariant::BreedFarm(model) => {
            model.spawned_egg_instance_ids.clear();
        }
        _ => {}
    });
}

/// Clears the private-lock ownership marker on lockable concrete-model
/// variants, and the `ConcreteModel.ModuleMap`'s `PasswordLock`/`Switch`
/// module state.
fn clear_access_config(properties: &mut Properties) {
    with_concrete_variant_mut(properties, |variant| match variant {
        PalMapConcreteModelVariant::ItemChest(model) => {
            model.private_lock_player_uid = FGuid::nil();
        }
        PalMapConcreteModelVariant::ItemChestAffectCorruption(model) => {
            model.private_lock_player_uid = FGuid::nil();
        }
        PalMapConcreteModelVariant::ItemBooth(model) => {
            model.private_lock_player_uid = FGuid::nil();
        }
        _ => {}
    });

    let Some(concrete) = properties
        .0
        .get_mut(&PropertyKey::from("ConcreteModel"))
        .and_then(props::struct_props_mut)
    else {
        return;
    };
    let Some(module_entries) = concrete
        .0
        .get_mut(&PropertyKey::from("ModuleMap"))
        .and_then(props::map_entries_mut)
    else {
        return;
    };
    for module in module_entries {
        let Some(module_props) = props::struct_props_mut(&mut module.value) else {
            continue;
        };
        if let Some(Property::Struct(StructValue::Game(PalStruct::MapConcreteModelModule(raw)))) =
            module_props.0.get_mut(&PropertyKey::from("RawData"))
        {
            match &mut raw.data {
                PalMapConcreteModelModuleData::PasswordLock { password, player_infos, .. } => {
                    password.clear();
                    player_infos.clear();
                }
                PalMapConcreteModelModuleData::Switch { switch_state, .. } => {
                    *switch_state = 0;
                }
                _ => {}
            }
        }
    }
}

fn apply_layer_gating(properties: &mut Properties, options: CaptureOptions) {
    if !options.structure_condition {
        reset_structure_condition(properties);
    }
    if !options.production_config {
        clear_production_config(properties);
    }
    if !options.production_progress {
        clear_production_progress(properties);
    }
    if !options.access_config {
        clear_access_config(properties);
    }
}

/// The base's `WorkSaveData` entries, matched by `RawData.base_data.base_camp_id_belong_to`.
fn works_of(
    session: &SaveSession,
    base_id: Uuid,
    options: CaptureOptions,
) -> Result<Vec<StructValue>, CoreError> {
    let mut works = Vec::new();
    let Some(values) = world::work_values(&session.level)? else {
        return Ok(works);
    };
    for value in values {
        let StructValue::Struct(work_props) = value else { continue };
        let Some(Property::Struct(StructValue::Game(PalStruct::Work(raw)))) =
            work_props.0.get(&PropertyKey::from("RawData"))
        else {
            continue;
        };
        let Some(base_data) = &raw.base_data else { continue };
        if props::guid_to_uuid(&base_data.base_camp_id_belong_to) != base_id {
            continue;
        }

        let mut work_value = value.clone();
        if !options.production_progress {
            zero_work_progress(&mut work_value);
        }
        works.push(work_value);
    }
    Ok(works)
}

fn zero_work_progress(work_value: &mut StructValue) {
    let StructValue::Struct(work_props) = work_value else { return };
    if let Some(Property::Struct(StructValue::Game(PalStruct::Work(raw)))) =
        work_props.0.get_mut(&PropertyKey::from("RawData"))
    {
        use crate::ue::games::palworld::PalWorkTypeSpecificData;
        match &mut raw.work_specific_data {
            PalWorkTypeSpecificData::Progress { current_work_amount, .. }
            | PalWorkTypeSpecificData::ProgressMultiType { current_work_amount, .. } => {
                *current_work_amount = 0.0;
            }
            _ => {}
        }
    }
}
