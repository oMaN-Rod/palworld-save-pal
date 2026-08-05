//! Places a captured blueprint into a save.
//!
//! Every mutation is computed against a private clone of the blueprint first;
//! only once the whole set has been built does `commit` touch a single `_mut`
//! accessor. A placement that fails -- a blocking finding, an unresolvable
//! guild, a merge target that is not there -- therefore leaves the session
//! exactly as it found it. A half-applied placement (structures added but no
//! base camp, ids rebound on some objects but not others) is worse than one
//! that refuses outright.

use std::collections::HashSet;

use uuid::Uuid;

use super::validate::{self, Anchor, PlacementMode};
use super::{capture, gvas, remap, transform, BaseBlueprint};
use crate::domain::{guild, guild_tail, world};
use crate::error::CoreError;
use crate::gamedata::GameData;
use crate::palbin;
use crate::props;
use crate::session::SaveSession;
use crate::ue::games::palworld::{
    PalInstanceId, PalMapConcreteModelModuleData, PalMapConcreteModelVariant, PalTransform, PalWork,
};
use crate::ue::{
    Double, MapEntry, PalStruct, Properties, Property, PropertyKey, StructValue, Vector,
};

/// A base's Pal Box: the structure that makes the base a base. `MergeInto`
/// drops it, since the target base already has one.
const PAL_BOX_MAP_OBJECT_ID: &str = "PalBoxV2";

/// The only `GroupSaveDataMap` group type that owns bases. The map also holds
/// `Organization`, `Party` and friends, none of which carry the `base_ids` and
/// `map_object_instance_ids_base_camp_points` lists a placed base registers in.
const GUILD_GROUP_TYPE: &str = "EPalGroupType::Guild";

#[derive(Debug, Clone)]
pub struct PlacementResult {
    pub base_id: Option<Uuid>,
    pub structures_placed: u32,
    pub findings: Vec<validate::Finding>,
}

/// Everything about a placement that is not the save or the blueprint itself.
#[derive(Debug, Clone, Copy)]
pub struct PlacementRequest {
    pub anchor: Anchor,
    pub mode: PlacementMode,
    pub owner_player_uid: Uuid,
    /// Lets a placement past non-blocking findings. Blocking ones are never
    /// overridable.
    pub override_warnings: bool,
}

pub fn place(
    session: &mut SaveSession,
    blueprint: &BaseBlueprint,
    request: &PlacementRequest,
    game_data: &GameData,
) -> Result<PlacementResult, CoreError> {
    let PlacementRequest {
        anchor,
        mode,
        owner_player_uid,
        override_warnings,
    } = *request;

    let findings = validate::check(session, game_data, blueprint, &anchor, &mode);
    if validate::has_blocking(&findings) {
        return Err(CoreError::Parse(format!("placement blocked: {findings:?}")));
    }
    if !override_warnings && !findings.is_empty() {
        return Err(CoreError::Parse(format!(
            "placement has warnings: {findings:?}"
        )));
    }

    let guild_id = target_guild(session, &mode)?;
    preflight(session, blueprint, &mode)?;

    let mut staged = blueprint.clone();
    let remap = remap::remap_blueprint(&mut staged)?;

    let anchor_transform = PalTransform {
        rotation: transform::yaw_quat(anchor.yaw_radians),
        translation: Vector {
            x: Double(anchor.x),
            y: Double(anchor.y),
            z: Double(anchor.z),
        },
        scale: Vector {
            x: Double(1.0),
            y: Double(1.0),
            z: Double(1.0),
        },
    };

    stage_transforms(&mut staged, &anchor_transform);
    let base_id = stage_identity(
        &mut staged,
        &mode,
        &remap,
        &anchor_transform,
        guild_id,
        owner_player_uid,
    )?;
    let structures_placed = staged.structures.len() as u32;

    commit(session, staged, &mode, guild_id, base_id)?;
    session.invalidate_performance_caches();

    Ok(PlacementResult {
        base_id,
        structures_placed,
        findings,
    })
}

/// The guild the placement binds everything to, and the point at which an
/// unresolvable target is rejected: `NewBase` names its guild outright, while
/// `MergeInto` inherits the target base's.
///
/// A group that is not a `Guild` is refused here rather than shrugged off
/// later: nothing downstream can register a base with it, so letting it
/// through would land 500 structures and a base camp that no guild owns.
fn target_guild(session: &SaveSession, mode: &PlacementMode) -> Result<Uuid, CoreError> {
    let guild_id = match mode {
        PlacementMode::NewBase { guild_id } => *guild_id,
        PlacementMode::MergeInto { base_id } => base_camp_guild(session, *base_id)
            .ok_or_else(|| CoreError::Parse(format!("target base {base_id} not found")))?,
    };
    let Some(entry_index) = guild::guild_entry_index(session, guild_id)? else {
        return Err(CoreError::Parse(format!("target guild {guild_id} not found")));
    };
    let entry = &world::group_map(&session.level)?[entry_index];
    let group_type = guild_tail::entry_group_type(entry);
    if group_type.as_deref() != Some(GUILD_GROUP_TYPE) {
        return Err(CoreError::Parse(format!(
            "target group {guild_id} is a {}, not a guild",
            group_type.as_deref().unwrap_or("group of unknown type")
        )));
    }
    if guild_tail::entry_group_data(entry).and_then(guild_tail::as_guild).is_none() {
        return Err(CoreError::Parse(format!(
            "target guild {guild_id} carries no decodable guild data"
        )));
    }
    Ok(guild_id)
}

fn base_camp_guild(session: &SaveSession, base_id: Uuid) -> Option<Uuid> {
    let entries = world::base_camp_map(&session.level).ok().flatten()?;
    let entry = entries
        .iter()
        .find(|entry| props::as_uuid(&entry.key) == Some(base_id))?;
    let value_props = props::struct_props(&entry.value)?;
    match value_props.0.get(&PropertyKey::from("RawData")) {
        Some(Property::Struct(StructValue::Game(PalStruct::BaseCamp(raw)))) => {
            Some(props::guid_to_uuid(&raw.group_id_belong_to))
        }
        _ => None,
    }
}

/// Confirms every collection the placement will append to is present and of
/// the expected shape, so `commit` cannot discover a missing one halfway
/// through. A world that has never held a map object or a base camp carries no
/// such array at all.
fn preflight(
    session: &SaveSession,
    blueprint: &BaseBlueprint,
    mode: &PlacementMode,
) -> Result<(), CoreError> {
    let missing = |name: &str| CoreError::Parse(format!("{name} missing from the target save"));

    if !blueprint.structures.is_empty()
        && world::map_object_values(&session.level)?.is_none()
    {
        return Err(missing("MapObjectSaveData"));
    }
    if !blueprint.works.is_empty() && world::work_values(&session.level)?.is_none() {
        return Err(missing("WorkSaveData"));
    }
    if !blueprint.dynamic_items.is_empty() {
        world::dynamic_item_values(&session.level)?;
    }
    if !blueprint.item_containers.is_empty() {
        world::item_container_map(&session.level)?;
    }
    if !blueprint.character_containers.is_empty() {
        world::character_container_map(&session.level)?;
    }
    if !blueprint.characters.is_empty() {
        world::character_map(&session.level)?;
    }
    world::group_map(&session.level)?;
    if world::base_camp_map(&session.level)?.is_none() {
        return Err(missing("BaseCampSaveData"));
    }
    if matches!(mode, PlacementMode::NewBase { .. }) && blueprint.base_camp.is_none() {
        return Err(CoreError::Parse(
            "blueprint carries no base camp to found a base with".to_string(),
        ));
    }
    // `append_work_ids` runs from inside `commit`, after the structures have
    // landed, so the blob it has to rewrite is decoded here instead -- while a
    // refusal is still free.
    if let PlacementMode::MergeInto { base_id } = mode {
        if !blueprint.works.is_empty() {
            check_target_work_collection(session, *base_id)?;
        }
    }
    Ok(())
}

/// Read-only twin of the rewrite `append_work_ids` performs: it decodes the
/// same blob, so a merge that could not register its works is refused before
/// any of them land. A base camp carrying no such property at all is not an
/// error -- `append_work_ids` skips it too.
fn check_target_work_collection(
    session: &SaveSession,
    base_id: Uuid,
) -> Result<(), CoreError> {
    let entries = world::base_camp_map(&session.level)?
        .ok_or_else(|| CoreError::Parse("BaseCampSaveData missing from the target save".into()))?;
    let entry = entries
        .iter()
        .find(|entry| props::as_uuid(&entry.key) == Some(base_id))
        .ok_or_else(|| CoreError::Parse(format!("target base {base_id} not found")))?;
    let Some(value_props) = props::struct_props(&entry.value) else {
        return Ok(());
    };
    let Some(bytes) =
        props::get(value_props, &["WorkCollection", "RawData"]).and_then(props::as_byte_array)
    else {
        return Ok(());
    };
    palbin::read_work_collection(bytes).map(|_| ())
}

fn stage_transforms(blueprint: &mut BaseBlueprint, anchor_transform: &PalTransform) {
    for structure in &mut blueprint.structures {
        let world_transform = transform::to_world(anchor_transform, &structure.relative_transform);
        if let Some(model) = capture::map_object_model_mut(&mut structure.properties) {
            model.initial_transform_cache = world_transform;
        }
    }
}

/// Rebinds ownership and base membership onto the staged clone. Returns the
/// minted base id for `NewBase`, and `None` for `MergeInto`, which founds no
/// base.
fn stage_identity(
    blueprint: &mut BaseBlueprint,
    mode: &PlacementMode,
    remap: &remap::IdRemap,
    anchor_transform: &PalTransform,
    guild_id: Uuid,
    owner_player_uid: Uuid,
) -> Result<Option<Uuid>, CoreError> {
    let (target_base_id, minted) = match mode {
        PlacementMode::NewBase { .. } => {
            let base_id = mint_base_id(blueprint, remap)?;
            (base_id, Some(base_id))
        }
        PlacementMode::MergeInto { base_id } => {
            blueprint.base_camp = None;
            blueprint
                .structures
                .retain(|structure| structure.map_object_id != PAL_BOX_MAP_OBJECT_ID);
            drop_empty_unreferenced_character_containers(blueprint);
            (*base_id, None)
        }
    };

    for structure in &mut blueprint.structures {
        if let Some(model) = capture::map_object_model_mut(&mut structure.properties) {
            model.base_camp_id_belong_to = props::uuid_to_guid(target_base_id);
            model.group_id_belong_to = props::uuid_to_guid(guild_id);
            model.build_player_uid = props::uuid_to_guid(owner_player_uid);
        }
        if let Some(concrete) = capture::map_object_concrete_model_mut(&mut structure.properties) {
            if let PalMapConcreteModelVariant::BaseCampPoint(point) = &mut concrete.model_data {
                point.base_camp_id = props::uuid_to_guid(target_base_id);
            }
        }
    }

    for work in &mut blueprint.works {
        if let Some(raw) = work_raw_mut(work) {
            if let Some(base) = raw.base_data.as_mut() {
                base.base_camp_id_belong_to = props::uuid_to_guid(target_base_id);
            }
        }
    }

    for entry in &mut blueprint.character_containers {
        set_container_slot_owners(entry, owner_player_uid);
    }
    for entry in &mut blueprint.characters {
        if let Some(data) = world::entry_character_data_mut(entry) {
            data.group_id = props::uuid_to_guid(guild_id);
        }
        if let Some(save_parameter) = world::entry_save_parameter_mut(entry) {
            save_parameter.insert("OwnerPlayerUId", props::guid_property(owner_player_uid));
        }
    }

    if let Some(base_camp) = &mut blueprint.base_camp {
        let Some(Property::Struct(StructValue::Game(PalStruct::BaseCamp(raw)))) =
            base_camp.0.get_mut(&PropertyKey::from("RawData"))
        else {
            return Err(CoreError::Parse(
                "blueprint base camp carries no typed RawData".to_string(),
            ));
        };
        // The captured transform is the anchor every relative offset was taken
        // against, so it is also what the `WorkerDirector`'s world-space spawn
        // point has to be re-based off -- read it before it is overwritten.
        let source_anchor = raw.transform.clone();
        raw.id = props::uuid_to_guid(target_base_id);
        raw.group_id_belong_to = props::uuid_to_guid(guild_id);
        raw.transform = anchor_transform.clone();

        stage_worker_director(base_camp, target_base_id, &source_anchor, anchor_transform)?;
    }

    Ok(minted)
}

/// Rebinds the two fields the base camp's opaque `WorkerDirector` blob carries
/// that placement owns: the base camp it belongs to, and the world-space point
/// its workers spawn at, which travels verbatim out of the source save
/// otherwise. The blob's `container_id` is remapped by `remap`, alongside every
/// other id the blueprint references.
///
/// A blob that does not decode refuses the placement rather than degrading to
/// the source save's values: kept as it is, the placed base would still send its
/// workers to the coordinates the blueprint was captured at and still call
/// itself by the base id it was captured from. Staging runs before `commit`
/// touches the session, so refusing here costs nothing.
fn stage_worker_director(
    base_camp: &mut Properties,
    base_id: Uuid,
    source_anchor: &PalTransform,
    anchor_transform: &PalTransform,
) -> Result<(), CoreError> {
    let Some(raw_data) = props::get_mut(base_camp, &["WorkerDirector", "RawData"]) else {
        return Ok(());
    };
    let Some(bytes) = props::as_byte_array_mut(raw_data) else {
        return Ok(());
    };
    let mut director = palbin::read_worker_director(bytes)?;
    director.id = base_id;
    let relative = transform::to_relative(source_anchor, &director.spawn_transform);
    director.spawn_transform = transform::to_world(anchor_transform, &relative);
    *bytes = director.to_bytes();
    Ok(())
}

/// A merge founds no base, so it drops the blueprint's base camp -- and with it
/// the `WorkerDirector` that was the only thing naming the base's worker
/// container. An empty container left behind that way would land with nothing
/// pointing at it, one orphan per merge, so it goes.
///
/// A container still holding pals stays, orphan or not. `remap` has already
/// rewritten every merged work's `assigned_individual_id` to those pals' fresh
/// ids, so dropping them here would land works naming
/// `CharacterSaveParameterMap` keys that do not exist -- and would silently
/// discard the base's whole workforce. An unreferenced container is inert; a
/// dangling reference is not.
///
/// Runs after `remap`, so both the container ids and the module references
/// compared here are the post-remap ones.
fn drop_empty_unreferenced_character_containers(blueprint: &mut BaseBlueprint) {
    let mut referenced: HashSet<Uuid> = HashSet::new();
    for structure in &blueprint.structures {
        capture::for_each_module_raw(&structure.properties, |raw| {
            if let PalMapConcreteModelModuleData::CharacterContainer {
                target_container_id,
                ..
            } = &raw.data
            {
                referenced.insert(props::guid_to_uuid(target_container_id));
            }
        });
    }

    blueprint.character_containers.retain(|entry| {
        capture::container_entry_id(entry)
            .is_some_and(|container_id| referenced.contains(&container_id))
            || !capture::character_container_slot_instance_ids(entry).is_empty()
    });
}

/// The new base's id. `remap` already minted one for the captured base id when
/// it rewrote the base camp's `WorkCollection.own_id`, which names the base
/// itself; reusing it is what keeps the two in agreement.
fn mint_base_id(blueprint: &BaseBlueprint, remap: &remap::IdRemap) -> Result<Uuid, CoreError> {
    let base_camp = blueprint.base_camp.as_ref().ok_or_else(|| {
        CoreError::Parse("blueprint carries no base camp to found a base with".to_string())
    })?;
    let captured = match base_camp.0.get(&PropertyKey::from("RawData")) {
        Some(Property::Struct(StructValue::Game(PalStruct::BaseCamp(raw)))) => {
            props::guid_to_uuid(&raw.id)
        }
        _ => {
            return Err(CoreError::Parse(
                "blueprint base camp carries no typed RawData".to_string(),
            ))
        }
    };
    Ok(remap.get(captured).unwrap_or_else(Uuid::new_v4))
}

fn work_raw_mut(value: &mut StructValue) -> Option<&mut PalWork> {
    let StructValue::Struct(work_props) = value else {
        return None;
    };
    match work_props.0.get_mut(&PropertyKey::from("RawData")) {
        Some(Property::Struct(StructValue::Game(PalStruct::Work(raw)))) => Some(raw),
        _ => None,
    }
}

/// Points every slot of a `CharacterContainerSaveData` entry at the placing
/// player; capture scrubbed them to nil.
fn set_container_slot_owners(entry: &mut MapEntry, owner_player_uid: Uuid) {
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
        if let Some(Property::Struct(StructValue::Game(PalStruct::CharacterContainer(raw)))) =
            slot_props.0.get_mut(&PropertyKey::from("RawData"))
        {
            raw.player_uid = props::uuid_to_guid(owner_player_uid);
        }
    }
}

/// The only function here that writes to the session, and only from fully
/// staged values. `preflight` has already established that every collection it
/// reaches for exists.
fn commit(
    session: &mut SaveSession,
    staged: BaseBlueprint,
    mode: &PlacementMode,
    guild_id: Uuid,
    base_id: Option<Uuid>,
) -> Result<(), CoreError> {
    // Computed before anything is written, so a blueprint whose property tree
    // cannot be described still refuses instead of half-applying.
    let schemas = gvas::placement_schemas(&staged)?;

    let BaseBlueprint {
        base_camp,
        structures,
        item_containers,
        character_containers,
        characters,
        works,
        dynamic_items,
        ..
    } = staged;

    // Teaches the destination how to write back every property the blueprint
    // introduces; without it `level_sav_bytes` fails on the first one the
    // target save never happened to carry.
    for (path, tag) in schemas.schemas().clone() {
        props::ensure_schema(&mut session.level, path, tag);
    }

    let missing = |name: &str| CoreError::Parse(format!("{name} missing from the target save"));
    let work_ids: Vec<Uuid> = works
        .iter()
        .filter_map(capture::work_base_id)
        .filter(|id| !id.is_nil())
        .collect();
    let character_ids: Vec<Uuid> = characters
        .iter()
        .filter_map(world::entry_instance_id)
        .collect();
    let pal_box_instance_id = base_camp.as_ref().and_then(base_camp_owner_instance_id);

    if !structures.is_empty() {
        world::map_object_values_mut(&mut session.level)?
            .ok_or_else(|| missing("MapObjectSaveData"))?
            .extend(
                structures
                    .into_iter()
                    .map(|structure| StructValue::Struct(structure.properties)),
            );
    }
    if !works.is_empty() {
        world::work_values_mut(&mut session.level)?
            .ok_or_else(|| missing("WorkSaveData"))?
            .extend(works);
    }
    if !item_containers.is_empty() {
        world::item_container_map_mut(&mut session.level)?.extend(item_containers);
    }
    if !character_containers.is_empty() {
        world::character_container_map_mut(&mut session.level)?.extend(character_containers);
    }
    if !characters.is_empty() {
        world::character_map_mut(&mut session.level)?.extend(characters);
    }
    if !dynamic_items.is_empty() {
        world::dynamic_item_values_mut(&mut session.level)?.extend(dynamic_items);
    }

    match mode {
        PlacementMode::NewBase { .. } => {
            let new_base_id = base_id.ok_or_else(|| {
                CoreError::Parse("a new base was staged without an id".to_string())
            })?;
            let base_camp =
                base_camp.ok_or_else(|| CoreError::Parse("no base camp staged".to_string()))?;
            world::base_camp_map_mut(&mut session.level)?
                .ok_or_else(|| missing("BaseCampSaveData"))?
                .push(MapEntry {
                    key: props::guid_property(new_base_id),
                    value: Property::Struct(StructValue::Struct(base_camp)),
                });
        }
        PlacementMode::MergeInto { base_id } => {
            append_work_ids(session, *base_id, &work_ids)?;
        }
    }

    register_with_guild(
        session,
        guild_id,
        base_id,
        pal_box_instance_id,
        &character_ids,
    )
}

/// A base camp names its Pal Box in `owner_map_object_instance_id`; the guild
/// tail lists the same id under `map_object_instance_ids_base_camp_points`.
fn base_camp_owner_instance_id(base_camp: &Properties) -> Option<Uuid> {
    match base_camp.0.get(&PropertyKey::from("RawData")) {
        Some(Property::Struct(StructValue::Game(PalStruct::BaseCamp(raw)))) => {
            Some(props::guid_to_uuid(&raw.owner_map_object_instance_id))
        }
        _ => None,
    }
}

/// Adds the merged works to the target base's `WorkCollection`, the opaque
/// blob in which a base names its works a second time.
fn append_work_ids(
    session: &mut SaveSession,
    base_id: Uuid,
    work_ids: &[Uuid],
) -> Result<(), CoreError> {
    if work_ids.is_empty() {
        return Ok(());
    }
    let entries = world::base_camp_map_mut(&mut session.level)?
        .ok_or_else(|| CoreError::Parse("BaseCampSaveData missing from the target save".into()))?;
    let entry = entries
        .iter_mut()
        .find(|entry| props::as_uuid(&entry.key) == Some(base_id))
        .ok_or_else(|| CoreError::Parse(format!("target base {base_id} not found")))?;
    let Some(value_props) = props::struct_props_mut(&mut entry.value) else {
        return Ok(());
    };
    let Some(raw_data) = props::get_mut(value_props, &["WorkCollection", "RawData"]) else {
        return Ok(());
    };
    let Some(bytes) = props::as_byte_array_mut(raw_data) else {
        return Ok(());
    };
    // `preflight` already decoded this blob, so the merged works cannot be
    // dropped on the floor here after the rest of the placement has landed.
    let mut collection = palbin::read_work_collection(bytes)?;
    collection.work_ids.extend_from_slice(work_ids);
    *bytes = collection.to_bytes();
    Ok(())
}

fn register_with_guild(
    session: &mut SaveSession,
    guild_id: Uuid,
    base_id: Option<Uuid>,
    pal_box_instance_id: Option<Uuid>,
    character_ids: &[Uuid],
) -> Result<(), CoreError> {
    let Some(entry_index) = guild::guild_entry_index(session, guild_id)? else {
        return Ok(());
    };
    let entries = world::group_map_mut(&mut session.level)?;
    let Some(group_data) = guild_tail::entry_group_data_mut(&mut entries[entry_index]) else {
        return Ok(());
    };
    for instance_id in character_ids {
        group_data
            .individual_character_handle_ids
            .push(PalInstanceId {
                guid: props::uuid_to_guid(props::EMPTY_UUID),
                instance_id: props::uuid_to_guid(*instance_id),
            });
    }
    let Some(new_base_id) = base_id else {
        return Ok(());
    };
    if let Some(guild) = guild_tail::as_guild_mut(group_data) {
        guild.base_ids.push(props::uuid_to_guid(new_base_id));
        if let Some(instance_id) = pal_box_instance_id.filter(|id| !id.is_nil()) {
            guild
                .map_object_instance_ids_base_camp_points
                .push(props::uuid_to_guid(instance_id));
        }
    }
    Ok(())
}
