//! Regenerates every GUID a captured `BaseBlueprint` defines and rewrites every reference
//! to it, so one blueprint can be placed repeatedly -- including back into the save it was
//! captured from -- without any two placements colliding on instance ids.
//!
//! A reference whose target lies outside the blueprint is written as a nil guid rather
//! than dropped: the GVAS encoder writes the key unconditionally, and removing it breaks
//! re-encoding.
//!
//! Deliberately NOT remapped, because placement retargets them to the destination world
//! rather than to anything inside the blueprint: `PalMapModel.base_camp_id_belong_to`,
//! `PalMapModel.group_id_belong_to`, `PalBaseCamp.id`, `PalCharacterData`'s group id, and
//! `PalDynamicId.created_world_id` -- the last of which sits right beside
//! `local_id_in_created_world`, which IS remapped, being the identity of the item itself.

use std::collections::HashMap;
use uuid::Uuid;

use super::capture;
use super::BaseBlueprint;
use crate::domain::world;
use crate::error::CoreError;
use crate::palbin;
use crate::props;
use crate::ue::games::palworld::{
    PalInstanceId, PalMapConcreteModelModuleData, PalWork, PalWorkAssign, PalWorkTypeSpecificData,
};
use crate::ue::{FGuid, MapEntry, PalStruct, Properties, Property, PropertyKey, StructValue};

#[derive(Debug, Default)]
pub struct IdRemap {
    mapping: HashMap<Uuid, Uuid>,
}

impl IdRemap {
    /// The fresh id for `old`, minting on first sight and stable thereafter. The nil guid
    /// maps to itself: it means "no owner"/"no target", not an identity to regenerate.
    pub fn new_for(&mut self, old: Uuid) -> Uuid {
        if old.is_nil() {
            return Uuid::nil();
        }
        *self.mapping.entry(old).or_insert_with(Uuid::new_v4)
    }

    /// Mints `old`'s fresh id without writing it anywhere, for the case where
    /// the value is only ever read back through the field that owns it.
    pub fn reserve(&mut self, old: Uuid) {
        self.new_for(old);
    }

    pub fn get(&self, old: Uuid) -> Option<Uuid> {
        self.mapping.get(&old).copied()
    }

    /// Every old -> new pair in the raw Palworld guid byte encoding, for substituting ids inside opaque blobs.
    fn byte_pairs(&self) -> HashMap<[u8; 16], [u8; 16]> {
        self.mapping
            .iter()
            .map(|(old, new)| (palbin::guid_bytes(*old), palbin::guid_bytes(*new)))
            .collect()
    }
}

pub fn remap_blueprint(blueprint: &mut BaseBlueprint) -> Result<IdRemap, CoreError> {
    let mut remap = IdRemap::default();
    register_definitions(blueprint, &mut remap);
    rewrite_references(blueprint, &remap)?;
    rebuild_work_collection(blueprint, &mut remap)?;
    Ok(remap)
}

fn register(remap: &mut IdRemap, guid: &mut FGuid) {
    let old = props::guid_to_uuid(guid);
    *guid = props::uuid_to_guid(remap.new_for(old));
}

fn rewrite(remap: &IdRemap, guid: &mut FGuid) {
    let old = props::guid_to_uuid(guid);
    let new = remap.get(old).unwrap_or(Uuid::nil());
    *guid = props::uuid_to_guid(new);
}

fn dynamic_item_local_id_mut(value: &mut StructValue) -> Option<&mut FGuid> {
    let StructValue::Struct(item_props) = value else { return None };
    match item_props.0.get_mut(&PropertyKey::from("RawData")) {
        Some(Property::Struct(StructValue::Game(PalStruct::DynamicItem(dynamic_item)))) => {
            Some(&mut dynamic_item.id.local_id_in_created_world)
        }
        _ => None,
    }
}

fn work_raw_mut(value: &mut StructValue) -> Option<&mut PalWork> {
    let StructValue::Struct(work_props) = value else { return None };
    match work_props.0.get_mut(&PropertyKey::from("RawData")) {
        Some(Property::Struct(StructValue::Game(PalStruct::Work(raw)))) => Some(raw),
        _ => None,
    }
}

/// Every `WorkAssignMap` entry's typed `RawData` on one `WorkSaveData` element.
fn for_each_work_assign_mut(
    value: &mut StructValue,
    mut visit: impl FnMut(&mut PalWorkAssign),
) {
    let StructValue::Struct(work_props) = value else { return };
    let Some(Property::Map(entries)) = work_props.0.get_mut(&PropertyKey::from("WorkAssignMap"))
    else {
        return;
    };
    for entry in entries {
        let Some(assign_props) = props::struct_props_mut(&mut entry.value) else { continue };
        if let Some(Property::Struct(StructValue::Game(PalStruct::WorkAssign(raw)))) =
            assign_props.0.get_mut(&PropertyKey::from("RawData"))
        {
            visit(raw);
        }
    }
}

fn container_key_id_mut(entry: &mut MapEntry) -> Option<&mut FGuid> {
    let key_props = props::struct_props_mut(&mut entry.key)?;
    match key_props.0.get_mut(&PropertyKey::from("ID")) {
        Some(Property::Struct(StructValue::Guid(guid))) => Some(guid),
        _ => None,
    }
}

/// A `CharacterSaveParameterMap` entry's key `InstanceId` -- the pal's own
/// identity, which its container slot and every work assignment name.
fn character_instance_id_mut(entry: &mut MapEntry) -> Option<&mut FGuid> {
    let key_props = props::struct_props_mut(&mut entry.key)?;
    match key_props.0.get_mut(&PropertyKey::from("InstanceId")) {
        Some(Property::Struct(StructValue::Guid(guid))) => Some(guid),
        _ => None,
    }
}

/// Mints a new id for every id the blueprint DEFINES. For a structure that is its
/// `Model`'s own `instance_id` and its `ConcreteModel`'s own `instance_id`; the latter is
/// the same underlying value `Model.concrete_model_instance_id` names, so reserving it
/// here lets pass two's lookup of the other field resolve to the same fresh id.
fn register_definitions(blueprint: &mut BaseBlueprint, remap: &mut IdRemap) {
    for structure in &mut blueprint.structures {
        if let Some(model) = capture::map_object_model_mut(&mut structure.properties) {
            register(remap, &mut model.instance_id);
            remap.reserve(props::guid_to_uuid(&model.concrete_model_instance_id));
        }
        if let Some(concrete) = capture::map_object_concrete_model_mut(&mut structure.properties) {
            register(remap, &mut concrete.instance_id);
        }
    }

    for work in &mut blueprint.works {
        if let Some(raw) = work_raw_mut(work) {
            if let Some(base) = raw.base_data.as_mut() {
                register(remap, &mut base.id);
            }
            // Structurally the same slot as `PalWorkAssign.id`: the assignment
            // record's own identity, not a pointer at one.
            if let PalWorkTypeSpecificData::Assign { handle_id, .. } = &mut raw.work_specific_data {
                register(remap, handle_id);
            }
        }
        for_each_work_assign_mut(work, |assign| register(remap, &mut assign.id));
    }

    for entry in &mut blueprint.item_containers {
        if let Some(id) = container_key_id_mut(entry) {
            register(remap, id);
        }
    }
    for entry in &mut blueprint.character_containers {
        if let Some(id) = container_key_id_mut(entry) {
            register(remap, id);
        }
    }

    // A placed pal must not collide with one already in the target save, and
    // `world::build_character_index` keys the whole character map on this.
    for entry in &mut blueprint.characters {
        if let Some(id) = character_instance_id_mut(entry) {
            register(remap, id);
        }
    }

    for item in &mut blueprint.dynamic_items {
        if let Some(id) = dynamic_item_local_id_mut(item) {
            register(remap, id);
        }
    }
}

/// Rewrites every id the blueprint only REFERENCES, via `remap`. A reference
/// whose target was never registered in pass one points outside the blueprint
/// and becomes the nil guid -- never a removed key.
fn rewrite_references(blueprint: &mut BaseBlueprint, remap: &IdRemap) -> Result<(), CoreError> {
    let connector_substitutions = remap.byte_pairs();

    for structure in &mut blueprint.structures {
        if let Some(model) = capture::map_object_model_mut(&mut structure.properties) {
            rewrite(remap, &mut model.concrete_model_instance_id);
            rewrite(remap, &mut model.repair_work_id);
            rewrite(remap, &mut model.owner_instance_id);
            rewrite(remap, &mut model.stage_instance_id_belong_to.id);
            // Always names a level spawner outside the blueprint.
            model.owner_spawner_level_object_instance_id = FGuid::nil();
        }
        if let Some(concrete) = capture::map_object_concrete_model_mut(&mut structure.properties) {
            rewrite(remap, &mut concrete.model_instance_id);
        }
        rewrite_module_map(remap, &mut structure.properties);
        if let Some(connector) = capture::map_object_connector_mut(&mut structure.properties) {
            for item in &mut connector.connect.any_place {
                rewrite(remap, &mut item.connect_to_model_instance_id);
            }
            substitute_guid_bytes(&mut connector.unknown_bytes, &connector_substitutions);
        }
    }

    for entry in &mut blueprint.character_containers {
        rewrite_container_slots(remap, entry, |slot_props| {
            match slot_props.0.get_mut(&PropertyKey::from("RawData")) {
                Some(Property::Struct(StructValue::Game(PalStruct::CharacterContainer(raw)))) => {
                    Some(&mut raw.instance_id)
                }
                _ => None,
            }
        });
    }
    for entry in &mut blueprint.item_containers {
        rewrite_container_slots(remap, entry, |slot_props| {
            match slot_props.0.get_mut(&PropertyKey::from("RawData")) {
                Some(Property::Struct(StructValue::Game(PalStruct::ItemContainerSlots(raw)))) => {
                    Some(&mut raw.item.dynamic_id.local_id_in_created_world)
                }
                _ => None,
            }
        });
    }

    for entry in &mut blueprint.characters {
        rewrite_pal_container_back_pointer(remap, entry);
    }

    for work in &mut blueprint.works {
        rewrite_work(remap, work);
    }

    if let Some(base_camp) = &mut blueprint.base_camp {
        if let Some(Property::Struct(StructValue::Game(PalStruct::BaseCamp(raw)))) =
            base_camp.0.get_mut(&PropertyKey::from("RawData"))
        {
            rewrite(remap, &mut raw.owner_map_object_instance_id);
        }
        rewrite_worker_director_container(remap, base_camp)?;
    }

    Ok(())
}

/// A base names its worker character container a second time, inside the opaque
/// `WorkerDirector` blob uesave keeps raw. Missed, the placed base's workers resolve to
/// the SOURCE base's container.
///
/// A blob that does not decode is refused rather than carried over: the layout is fixed at
/// 118 bytes, so a game update changing it would silently reinstate that defect.
fn rewrite_worker_director_container(
    remap: &IdRemap,
    base_camp: &mut Properties,
) -> Result<(), CoreError> {
    let Some(raw_data) = props::get_mut(base_camp, &["WorkerDirector", "RawData"]) else {
        return Ok(());
    };
    let Some(bytes) = props::as_byte_array_mut(raw_data) else { return Ok(()) };
    let mut director = palbin::read_worker_director(bytes)?;
    director.container_id = remap.get(director.container_id).unwrap_or(Uuid::nil());
    *bytes = director.to_bytes();
    Ok(())
}

/// `PalConnector::read` parses only the first connect group and keeps the rest
/// of the payload as opaque `unknown_bytes`, where the great majority of a
/// structure's `connect_to_model_instance_id` guids actually live. Substituting
/// them 16 bytes for 16 bytes leaves the payload's length -- and therefore
/// `PalConnector::write` -- untouched. The scan advances a full guid past every
/// hit, so an id just written can never be substituted a second time.
fn substitute_guid_bytes(bytes: &mut [u8], substitutions: &HashMap<[u8; 16], [u8; 16]>) {
    if bytes.len() < 16 || substitutions.is_empty() {
        return;
    }
    let mut offset = 0;
    while offset + 16 <= bytes.len() {
        let window: [u8; 16] = bytes[offset..offset + 16].try_into().expect("16-byte window");
        match substitutions.get(&window) {
            Some(replacement) => {
                bytes[offset..offset + 16].copy_from_slice(replacement);
                offset += 16;
            }
            None => offset += 1,
        }
    }
}

fn rewrite_module_map(remap: &IdRemap, properties: &mut Properties) {
    capture::for_each_module_raw_mut(properties, |raw| match &mut raw.data {
        PalMapConcreteModelModuleData::ItemContainer { target_container_id, .. } => {
            rewrite(remap, target_container_id);
        }
        PalMapConcreteModelModuleData::CharacterContainer { target_container_id, .. } => {
            rewrite(remap, target_container_id);
        }
        PalMapConcreteModelModuleData::Workee { target_work_id, .. } => {
            rewrite(remap, target_work_id);
        }
        _ => {}
    });
}

/// Walks an `ItemContainerSaveData`/`CharacterContainerSaveData` entry's `Slots`,
/// rewriting the one guid `field` picks out of each slot's typed `RawData`.
fn rewrite_container_slots(
    remap: &IdRemap,
    entry: &mut MapEntry,
    mut field: impl FnMut(&mut Properties) -> Option<&mut FGuid>,
) {
    let Some(value_props) = props::struct_props_mut(&mut entry.value) else {
        return;
    };
    let Some(slots) = props::get_mut(value_props, &["Slots"]).and_then(props::struct_values_mut)
    else {
        return;
    };
    for slot in slots {
        let StructValue::Struct(slot_props) = slot else {
            continue;
        };
        if let Some(guid) = field(slot_props) {
            rewrite(remap, guid);
        }
    }
}

/// A captured pal names the character container holding it. Both spellings of
/// the slot key occur in the wild.
fn rewrite_pal_container_back_pointer(remap: &IdRemap, entry: &mut MapEntry) {
    let Some(save_parameter) = world::entry_save_parameter_mut(entry) else {
        return;
    };
    let key = if props::get(save_parameter, &["SlotID"]).is_some() { "SlotID" } else { "SlotId" };
    if let Some(Property::Struct(StructValue::Guid(guid))) =
        props::get_mut(save_parameter, &[key, "ContainerId", "ID"])
    {
        rewrite(remap, guid);
    }
}

fn rewrite_instance_id(remap: &IdRemap, individual: &mut PalInstanceId) {
    rewrite(remap, &mut individual.instance_id);
}

fn rewrite_work(remap: &IdRemap, work: &mut StructValue) {
    if let Some(raw) = work_raw_mut(work) {
        if let Some(base) = raw.base_data.as_mut() {
            rewrite(remap, &mut base.owner_map_object_model_id);
            rewrite(remap, &mut base.owner_map_object_concrete_model_id);
        }
        if let Some(transform) = raw.transform.as_mut() {
            if let Some(id) = transform.map_object_instance_id.as_mut() {
                rewrite(remap, id);
            }
        }
        match &mut raw.work_specific_data {
            PalWorkTypeSpecificData::ReviveCharacter { target_individual_id } => {
                rewrite_instance_id(remap, target_individual_id);
            }
            PalWorkTypeSpecificData::Assign { assigned_individual_id, .. } => {
                rewrite_instance_id(remap, assigned_individual_id);
            }
            _ => {}
        }
    }
    for_each_work_assign_mut(work, |assign| {
        rewrite_instance_id(remap, &mut assign.assigned_individual_id)
    });
}

/// The base camp names its works a second time, in an opaque blob uesave keeps raw.
/// `own_id` is a definition; the list is replaced wholesale with the blueprint's
/// post-remap work ids, dropping ids already dangling in the source save.
///
/// A blob that does not decode is refused rather than carried over: kept, it would leave
/// the placed base naming the SOURCE save's works and calling itself by its base id.
fn rebuild_work_collection(
    blueprint: &mut BaseBlueprint,
    remap: &mut IdRemap,
) -> Result<(), CoreError> {
    let work_ids: Vec<Uuid> = blueprint
        .works
        .iter()
        .filter_map(capture::work_base_id)
        .filter(|id| !id.is_nil())
        .collect();

    let Some(base_camp) = &mut blueprint.base_camp else { return Ok(()) };
    let Some(raw_data) = props::get_mut(base_camp, &["WorkCollection", "RawData"]) else {
        return Ok(());
    };
    let Some(bytes) = props::as_byte_array_mut(raw_data) else { return Ok(()) };
    let mut collection = palbin::read_work_collection(bytes)?;
    collection.own_id = remap.new_for(collection.own_id);
    collection.work_ids = work_ids;
    *bytes = collection.to_bytes();
    Ok(())
}
