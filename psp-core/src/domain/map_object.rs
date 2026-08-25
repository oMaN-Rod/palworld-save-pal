//! World-wide operations over `MapObjectSaveData` entries.

use std::collections::HashSet;

use crate::domain::world;
use crate::error::CoreError;
use crate::props;
use crate::session::SaveSession;
use crate::ue::games::palworld::{PalMapConcreteModelVariant, PalMapModel};
use crate::ue::{FGuid, PalStruct, Property, PropertyKey, StructValue};

#[derive(Debug, Clone)]
pub struct MapObjectView {
    pub instance_id: uuid::Uuid,
    pub map_object_id: String,
    pub base_id: Option<uuid::Uuid>,
    pub guild_id: Option<uuid::Uuid>,
    pub build_player_uid: Option<uuid::Uuid>,
    pub hp: i32,
    pub max_hp: i32,
    pub kind: String,
}

fn concrete_variant(object: &StructValue) -> Option<&PalMapConcreteModelVariant<crate::ue::Arch>> {
    let StructValue::Struct(properties) = object else {
        return None;
    };
    let concrete = properties.0.get(&PropertyKey::from("ConcreteModel")).and_then(props::struct_props)?;
    match concrete.0.get(&PropertyKey::from("RawData"))? {
        Property::Struct(StructValue::Game(PalStruct::MapConcreteModel(raw))) => Some(&raw.model_data),
        _ => None,
    }
}

fn with_concrete_variant_mut(
    object: &mut StructValue,
    f: impl FnOnce(&mut PalMapConcreteModelVariant<crate::ue::Arch>),
) {
    let StructValue::Struct(properties) = object else {
        return;
    };
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

fn lock_field(variant: &PalMapConcreteModelVariant<crate::ue::Arch>) -> Option<&FGuid> {
    match variant {
        PalMapConcreteModelVariant::ItemChest(model) => Some(&model.private_lock_player_uid),
        PalMapConcreteModelVariant::ItemChestAffectCorruption(model) => {
            Some(&model.private_lock_player_uid)
        }
        PalMapConcreteModelVariant::ItemBooth(model) => Some(&model.private_lock_player_uid),
        _ => None,
    }
}

fn lock_field_mut(variant: &mut PalMapConcreteModelVariant<crate::ue::Arch>) -> Option<&mut FGuid> {
    match variant {
        PalMapConcreteModelVariant::ItemChest(model) => Some(&mut model.private_lock_player_uid),
        PalMapConcreteModelVariant::ItemChestAffectCorruption(model) => {
            Some(&mut model.private_lock_player_uid)
        }
        PalMapConcreteModelVariant::ItemBooth(model) => Some(&mut model.private_lock_player_uid),
        _ => None,
    }
}

fn is_locked(variant: &PalMapConcreteModelVariant<crate::ue::Arch>) -> bool {
    lock_field(variant).is_some_and(|lock| *lock != FGuid::nil())
}

/// Deliberately leaves `PasswordLock` module state untouched -- only the lock flag is cleared.
fn clear_private_lock(object: &mut StructValue) -> bool {
    let mut changed = false;
    with_concrete_variant_mut(object, |variant| {
        if is_locked(variant) {
            if let Some(lock) = lock_field_mut(variant) {
                *lock = FGuid::nil();
                changed = true;
            }
        }
    });
    changed
}

pub fn unlock_private_chests(session: &mut SaveSession) -> Result<usize, CoreError> {
    let Some(objects) = world::map_object_values_mut(&mut session.level)? else {
        return Ok(0);
    };
    let mut cleared = 0;
    for object in objects.iter_mut() {
        if clear_private_lock(object) {
            cleared += 1;
        }
    }
    Ok(cleared)
}

pub fn count_private_chest_locks(session: &SaveSession) -> Result<usize, CoreError> {
    let Some(objects) = world::map_object_values(&session.level)? else {
        return Ok(0);
    };
    Ok(objects
        .iter()
        .filter(|object| concrete_variant(object).is_some_and(is_locked))
        .count())
}

fn model_of(object: &StructValue) -> Option<&PalMapModel> {
    let StructValue::Struct(properties) = object else { return None };
    let model = properties.0.get(&PropertyKey::from("Model")).and_then(props::struct_props)?;
    match model.0.get(&PropertyKey::from("RawData"))? {
        Property::Struct(StructValue::Game(PalStruct::MapModel(raw))) => Some(raw),
        _ => None,
    }
}

fn model_of_mut(object: &mut StructValue) -> Option<&mut PalMapModel> {
    let StructValue::Struct(properties) = object else { return None };
    let model = properties
        .0
        .get_mut(&PropertyKey::from("Model"))
        .and_then(props::struct_props_mut)?;
    match model.0.get_mut(&PropertyKey::from("RawData"))? {
        Property::Struct(StructValue::Game(PalStruct::MapModel(raw))) => Some(raw),
        _ => None,
    }
}

fn optional_uuid(guid: &FGuid) -> Option<uuid::Uuid> {
    let id = props::guid_to_uuid(guid);
    (!id.is_nil()).then_some(id)
}

fn concrete_kind(object: &StructValue) -> String {
    let StructValue::Struct(properties) = object else { return String::new() };
    let Some(concrete) =
        properties.0.get(&PropertyKey::from("ConcreteModel")).and_then(props::struct_props)
    else {
        return String::new();
    };
    match concrete.0.get(&PropertyKey::from("RawData")) {
        Some(Property::Struct(StructValue::Game(PalStruct::MapConcreteModel(raw)))) => {
            raw.concrete_model_type.clone()
        }
        _ => String::new(),
    }
}

fn view_of(object: &StructValue) -> Option<MapObjectView> {
    let model = model_of(object)?;
    let map_object_id = match object {
        StructValue::Struct(properties) => properties
            .0
            .get(&PropertyKey::from("MapObjectId"))
            .and_then(props::as_str)
            .unwrap_or_default()
            .to_string(),
        _ => String::new(),
    };
    Some(MapObjectView {
        instance_id: props::guid_to_uuid(&model.instance_id),
        map_object_id,
        base_id: optional_uuid(&model.base_camp_id_belong_to),
        guild_id: optional_uuid(&model.group_id_belong_to),
        build_player_uid: optional_uuid(&model.build_player_uid),
        hp: model.hp.current,
        max_hp: model.hp.max,
        kind: concrete_kind(object),
    })
}

pub fn map_object_views(session: &SaveSession) -> Result<Vec<MapObjectView>, CoreError> {
    let Some(objects) = world::map_object_values(&session.level)? else {
        return Ok(Vec::new());
    };
    Ok(objects.iter().filter_map(view_of).collect())
}

pub fn map_object_ids(session: &SaveSession) -> Result<Vec<uuid::Uuid>, CoreError> {
    let Some(objects) = world::map_object_values(&session.level)? else {
        return Ok(Vec::new());
    };
    Ok(objects
        .iter()
        .filter_map(|object| model_of(object).map(|m| props::guid_to_uuid(&m.instance_id)))
        .collect())
}

pub fn read_map_object(session: &SaveSession, id: uuid::Uuid) -> Option<MapObjectView> {
    let objects = world::map_object_values(&session.level).ok()??;
    objects
        .iter()
        .filter_map(view_of)
        .find(|view| view.instance_id == id)
}

pub fn set_map_object_hp(
    session: &mut SaveSession,
    id: uuid::Uuid,
    hp: i32,
) -> Result<bool, CoreError> {
    let Some(objects) = world::map_object_values_mut(&mut session.level)? else {
        return Ok(false);
    };
    for object in objects.iter_mut() {
        let Some(model) = model_of_mut(object) else { continue };
        if props::guid_to_uuid(&model.instance_id) == id {
            model.hp.current = hp;
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn set_map_object_builder(
    session: &mut SaveSession,
    id: uuid::Uuid,
    build_player_uid: Option<uuid::Uuid>,
) -> Result<bool, CoreError> {
    let Some(objects) = world::map_object_values_mut(&mut session.level)? else {
        return Ok(false);
    };
    for object in objects.iter_mut() {
        let Some(model) = model_of_mut(object) else { continue };
        if props::guid_to_uuid(&model.instance_id) == id {
            model.build_player_uid =
                build_player_uid.map(props::uuid_to_guid).unwrap_or_else(FGuid::nil);
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn remove_map_objects(
    session: &mut SaveSession,
    ids: &[uuid::Uuid],
) -> Result<usize, CoreError> {
    let doomed: HashSet<uuid::Uuid> = ids.iter().copied().collect();
    let Some(objects) = world::map_object_values_mut(&mut session.level)? else {
        return Ok(0);
    };
    let before = objects.len();
    objects.retain(|object| match model_of(object) {
        Some(model) => !doomed.contains(&props::guid_to_uuid(&model.instance_id)),
        None => true,
    });
    Ok(before - objects.len())
}

/// A `WorkSaveData` element's `RawData.base_data.owner_map_object_model_id`; `None`
/// when the element carries no work base at all.
fn work_owner(value: &StructValue) -> Option<uuid::Uuid> {
    let StructValue::Struct(properties) = value else { return None };
    match properties.0.get(&PropertyKey::from("RawData"))? {
        Property::Struct(StructValue::Game(PalStruct::Work(work))) => {
            work.base_data.as_ref().map(|base| props::guid_to_uuid(&base.owner_map_object_model_id))
        }
        _ => None,
    }
}

pub fn remove_orphaned_works(session: &mut SaveSession) -> Result<usize, CoreError> {
    let surviving: HashSet<uuid::Uuid> = map_object_ids(session)?.into_iter().collect();
    let Some(works) = world::work_values_mut(&mut session.level)? else {
        return Ok(0);
    };
    let before = works.len();
    works.retain(|work| match work_owner(work) {
        Some(owner) => surviving.contains(&owner),
        None => true,
    });
    Ok(before - works.len())
}

/// The same predicate `remove_orphaned_works` applies, against an externally
/// supplied surviving-id set rather than the session's own current map
/// objects -- lets a caller predict the count before actually removing them.
pub fn count_orphaned_works(
    session: &SaveSession,
    surviving: &HashSet<uuid::Uuid>,
) -> Result<usize, CoreError> {
    let Some(works) = world::work_values(&session.level)? else {
        return Ok(0);
    };
    Ok(works
        .iter()
        .filter(|work| match work_owner(work) {
            Some(owner) => !surviving.contains(&owner),
            None => false,
        })
        .count())
}

/// A `DynamicItemSaveData` element's own `RawData.id.local_id_in_created_world`;
/// `None` for an entry that is not a `PalDynamicItem` at all.
fn dynamic_item_id(value: &StructValue) -> Option<uuid::Uuid> {
    let StructValue::Struct(item_props) = value else { return None };
    match item_props.0.get(&PropertyKey::from("RawData"))? {
        Property::Struct(StructValue::Game(PalStruct::DynamicItem(item))) => {
            Some(props::guid_to_uuid(&item.id.local_id_in_created_world))
        }
        _ => None,
    }
}

fn item_dynamic_id(item: &crate::ue::games::palworld::PalItemId) -> Option<uuid::Uuid> {
    let id = props::guid_to_uuid(&item.dynamic_id.local_id_in_created_world);
    (id != props::EMPTY_UUID).then_some(id)
}

/// Every dynamic item id something in the save still points at: an
/// item-container slot, a `DropItem` map object's held item, an item booth's
/// trade goods, or a damage-triggered drop table's payout. The last two are
/// checked defensively -- every instance of either in this repository's
/// fixtures carries an empty trade/drop list, so nothing has yet been
/// observed to actually use them for a dynamic id -- but the type they share
/// with a real slot's item (`PalItemId`, complete with a `dynamic_id`) makes
/// them capable of carrying one, and checking them costs nothing a real
/// orphan sweep would otherwise get wrong.
fn referenced_dynamic_item_ids(
    level: &crate::ue::Save,
    surviving: Option<&HashSet<uuid::Uuid>>,
) -> Result<HashSet<uuid::Uuid>, CoreError> {
    let mut ids = HashSet::new();

    // Propagated, never swallowed: a container map that cannot be read leaves
    // this set empty, and an empty set means "nothing references anything",
    // which would present every live dynamic item as an orphan.
    {
        let entries = world::item_container_map(level)?;
        for entry in entries {
            let Some(value_props) = props::struct_props(&entry.value) else { continue };
            let Some(slot_values) =
                props::get(value_props, &["Slots"]).and_then(props::struct_values)
            else {
                continue;
            };
            for slot_value in slot_values {
                let StructValue::Struct(slot_props) = slot_value else { continue };
                if let Some(Property::Struct(StructValue::Game(PalStruct::ItemContainerSlots(raw)))) =
                    slot_props.0.get(&PropertyKey::from("RawData"))
                {
                    if let Some(id) = item_dynamic_id(&raw.item) {
                        ids.insert(id);
                    }
                }
            }
        }
    }

    if let Some(objects) = world::map_object_values(level)? {
        for object in objects {
            // A caller predicting a dry run's outcome passes the ids that would
            // survive it; an object about to be removed must not still count as
            // referencing an item, or the prediction under-reports the orphans a
            // real run would find.
            if let Some(surviving) = surviving {
                match model_of(object) {
                    Some(model) if surviving.contains(&props::guid_to_uuid(&model.instance_id)) => {}
                    _ => continue,
                }
            }
            let Some(variant) = concrete_variant(object) else { continue };
            match variant {
                PalMapConcreteModelVariant::DropItem(model) => {
                    if let Some(id) = item_dynamic_id(&model.item_id) {
                        ids.insert(id);
                    }
                }
                PalMapConcreteModelVariant::ItemBooth(model) => {
                    for trade in &model.trade_infos {
                        for item in [&trade.product, &trade.cost] {
                            if let Some(id) = item_dynamic_id(&item.item_id) {
                                ids.insert(id);
                            }
                        }
                    }
                }
                PalMapConcreteModelVariant::ItemDropOnDamag(model) => {
                    for item in &model.drop_item_infos {
                        if let Some(id) = item_dynamic_id(&item.item_id) {
                            ids.insert(id);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    Ok(ids)
}

pub fn count_orphaned_dynamic_items(session: &SaveSession) -> Result<usize, CoreError> {
    count_orphaned_dynamic_items_among(session, None)
}

/// The same predicate against an externally supplied surviving-map-object set,
/// so a caller can predict the count before actually removing those objects.
pub fn count_orphaned_dynamic_items_among(
    session: &SaveSession,
    surviving: Option<&HashSet<uuid::Uuid>>,
) -> Result<usize, CoreError> {
    let referenced = referenced_dynamic_item_ids(&session.level, surviving)?;
    let Ok(values) = world::dynamic_item_values(&session.level) else {
        return Ok(0);
    };
    Ok(values
        .iter()
        .filter(|value| match dynamic_item_id(value) {
            Some(id) => !referenced.contains(&id),
            None => false,
        })
        .count())
}

pub fn remove_orphaned_dynamic_items(session: &mut SaveSession) -> Result<usize, CoreError> {
    let referenced = referenced_dynamic_item_ids(&session.level, None)?;
    let Ok(values) = world::dynamic_item_values_mut(&mut session.level) else {
        return Ok(0);
    };
    let before = values.len();
    values.retain(|value| match dynamic_item_id(value) {
        Some(id) => referenced.contains(&id),
        None => true,
    });
    Ok(before - values.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_fixture_session(name: &str) -> SaveSession {
        let save_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/saves")
            .join(name);
        let level_sav_bytes =
            std::fs::read(save_dir.join("Level.sav")).expect("read fixture Level.sav");
        let level_meta_bytes = std::fs::read(save_dir.join("LevelMeta.sav")).ok();

        let mut player_file_refs: std::collections::BTreeMap<
            uuid::Uuid,
            crate::session::PlayerFileData,
        > = std::collections::BTreeMap::new();
        if let Ok(entries) = std::fs::read_dir(save_dir.join("Players")) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_none_or(|ext| ext != "sav") {
                    continue;
                }
                let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                let (uid_part, is_dps) = match stem.strip_suffix("_dps") {
                    Some(base) => (base, true),
                    None => (stem, false),
                };
                let Ok(uid) = uid_part.parse::<uuid::Uuid>() else {
                    continue;
                };
                let file_ref =
                    player_file_refs
                        .entry(uid)
                        .or_insert(crate::session::PlayerFileData::Paths {
                            sav: None,
                            dps: None,
                        });
                if let crate::session::PlayerFileData::Paths { sav, dps } = file_ref {
                    if is_dps {
                        *dps = Some(path);
                    } else {
                        *sav = Some(path);
                    }
                }
            }
        }

        SaveSession::load(
            crate::session::SaveKind::Steam {
                level_path: save_dir.join("Level.sav"),
            },
            save_dir.to_string_lossy().into_owned(),
            "steam",
            &level_sav_bytes,
            level_meta_bytes.as_deref(),
            None,
            player_file_refs,
            None,
            true,
            &crate::progress::null_progress(),
        )
        .expect("load fixture session")
    }

    fn count_locked_models(session: &SaveSession) -> usize {
        count_private_chest_locks(session).expect("counts")
    }

    #[test]
    fn unlock_private_chests_clears_locks_and_counts_only_changed_models() {
        let mut session = load_fixture_session("v1_relics");

        let locked_before = count_locked_models(&session);
        assert!(
            locked_before > 0,
            "the fixture must carry at least one locked chest; seed one rather than asserting zero"
        );

        let cleared = unlock_private_chests(&mut session).expect("unlocks");
        assert_eq!(cleared, locked_before, "every locked model must be counted exactly once");
        assert_eq!(count_locked_models(&session), 0, "no lock may survive");

        let again = unlock_private_chests(&mut session).expect("unlocks");
        assert_eq!(again, 0, "a second run has nothing left to clear");
    }

    #[test]
    fn the_lock_count_predicts_exactly_what_the_unlock_changes() {
        let mut session = load_fixture_session("v1_relics");
        let predicted = count_private_chest_locks(&session).expect("counts");
        let changed = unlock_private_chests(&mut session).expect("unlocks");
        assert_eq!(predicted, changed);
        assert_eq!(count_private_chest_locks(&session).expect("counts"), 0);
    }

    #[test]
    fn every_map_object_view_carries_the_identity_the_save_stores() {
        let session = load_fixture_session("v1_relics");
        let views = map_object_views(&session).expect("views");
        assert_eq!(views.len(), 5452, "the fixture's map object count");

        assert!(
            views.iter().all(|v| !v.instance_id.is_nil()),
            "an instance id is how a handle addresses an object; a nil one is unaddressable"
        );
        assert!(
            views.iter().all(|v| !v.map_object_id.is_empty()),
            "MapObjectId is a Name property on every entry of every fixture"
        );

        let attached = views.iter().filter(|v| v.base_id.is_some()).count();
        assert_eq!(attached, 2144, "map objects belonging to a base");
        assert!(
            views.iter().all(|v| v.base_id != Some(uuid::Uuid::nil())),
            "an unattached object reports None, never the nil sentinel"
        );

        let ids = map_object_ids(&session).expect("ids");
        assert_eq!(ids.len(), views.len(), "ids and views must agree in length");
    }

    #[test]
    fn a_view_round_trips_through_read_map_object() {
        let session = load_fixture_session("v1_relics");
        let views = map_object_views(&session).expect("views");
        let wanted = views.first().expect("the fixture has map objects");
        let found = read_map_object(&session, wanted.instance_id).expect("the id resolves");
        assert_eq!(found.instance_id, wanted.instance_id);
        assert_eq!(found.map_object_id, wanted.map_object_id);
        assert_eq!(found.hp, wanted.hp);
        assert!(read_map_object(&session, uuid::Uuid::nil()).is_none());
    }

    #[test]
    fn damaged_structures_exist_to_repair_and_hp_writes_land() {
        let mut session = load_fixture_session("v1_relics");
        let damaged: Vec<uuid::Uuid> = map_object_views(&session)
            .expect("views")
            .into_iter()
            .filter(|v| v.hp < v.max_hp)
            .map(|v| v.instance_id)
            .collect();
        assert_eq!(damaged.len(), 467, "the fixture's damaged structure count");

        let target = damaged[0];
        let before = read_map_object(&session, target).expect("resolves");
        assert!(set_map_object_hp(&mut session, target, before.max_hp).expect("write"));
        let after = read_map_object(&session, target).expect("resolves");
        assert_eq!(after.hp, before.max_hp);

        assert!(
            !set_map_object_hp(&mut session, uuid::Uuid::nil(), 1).expect("write"),
            "an unresolvable id reports false rather than erroring"
        );
    }

    #[test]
    fn remove_map_objects_removes_exactly_the_named_entries_in_one_pass() {
        let mut session = load_fixture_session("v1_relics");
        let before = map_object_ids(&session).expect("ids");
        let doomed: Vec<uuid::Uuid> = before.iter().take(3).copied().collect();

        let removed = remove_map_objects(&mut session, &doomed).expect("remove");
        assert_eq!(removed, 3);

        let after = map_object_ids(&session).expect("ids");
        assert_eq!(after.len(), before.len() - 3);
        for id in &doomed {
            assert!(!after.contains(id), "a removed object must not survive");
        }

        assert_eq!(
            remove_map_objects(&mut session, &doomed).expect("remove"),
            0,
            "a second removal of the same ids finds nothing"
        );
    }

    fn seed_dangling_work_owner(session: &mut SaveSession, dangling_owner: uuid::Uuid) -> usize {
        let works = world::work_values_mut(&mut session.level)
            .expect("work values")
            .expect("the fixture has work entries");
        for work in works.iter_mut() {
            let StructValue::Struct(properties) = work else { continue };
            let Some(Property::Struct(StructValue::Game(PalStruct::Work(raw)))) =
                properties.0.get_mut(&PropertyKey::from("RawData"))
            else {
                continue;
            };
            if let Some(base) = raw.base_data.as_mut() {
                base.owner_map_object_model_id = props::uuid_to_guid(dangling_owner);
                return 1;
            }
        }
        0
    }

    #[test]
    fn no_work_is_orphaned_in_the_untouched_fixture() {
        let session = load_fixture_session("v1_relics");
        let surviving: HashSet<uuid::Uuid> = map_object_ids(&session).expect("ids").into_iter().collect();
        assert_eq!(
            count_orphaned_works(&session, &surviving).expect("count"),
            0,
            "every work's owner must resolve to a surviving map object"
        );
    }

    #[test]
    fn remove_orphaned_works_removes_exactly_what_a_dangling_owner_creates() {
        let mut session = load_fixture_session("v1_relics");
        let works_before =
            world::work_values(&session.level).expect("work values").expect("work entries").len();

        let dangling_owner = uuid::Uuid::new_v4();
        let seeded = seed_dangling_work_owner(&mut session, dangling_owner);
        assert!(seeded > 0, "the fixture must carry at least one work entry with a base to dangle");

        let surviving: HashSet<uuid::Uuid> = map_object_ids(&session).expect("ids").into_iter().collect();
        assert_eq!(count_orphaned_works(&session, &surviving).expect("count"), seeded);

        let removed = remove_orphaned_works(&mut session).expect("remove");
        assert_eq!(removed, seeded);

        let works_after =
            world::work_values(&session.level).expect("work values").expect("work entries").len();
        assert_eq!(works_after, works_before - seeded);
        assert_eq!(count_orphaned_works(&session, &surviving).expect("count"), 0);
    }

    #[test]
    fn set_map_object_builder_writes_and_clears_build_player_uid() {
        let mut session = load_fixture_session("v1_relics");
        let target = map_object_views(&session)
            .expect("views")
            .into_iter()
            .find(|v| v.build_player_uid.is_some())
            .expect("the fixture must carry a built structure");

        let new_builder = uuid::Uuid::new_v4();
        assert!(set_map_object_builder(&mut session, target.instance_id, Some(new_builder))
            .expect("write"));
        assert_eq!(
            read_map_object(&session, target.instance_id).expect("resolves").build_player_uid,
            Some(new_builder)
        );

        assert!(set_map_object_builder(&mut session, target.instance_id, None).expect("write"));
        assert_eq!(
            read_map_object(&session, target.instance_id).expect("resolves").build_player_uid,
            None
        );

        assert!(
            !set_map_object_builder(&mut session, uuid::Uuid::nil(), Some(new_builder))
                .expect("write"),
            "an unresolvable id reports false rather than erroring"
        );
    }

    #[test]
    fn count_orphaned_dynamic_items_matches_the_measured_fixture_counts() {
        for (fixture, expected) in
            [("v1_relics", 1025), ("v1_stats", 13), ("world1", 26), ("world2", 0)]
        {
            let session = load_fixture_session(fixture);
            assert_eq!(
                count_orphaned_dynamic_items(&session).expect("count"),
                expected,
                "{fixture}: orphaned dynamic item count"
            );
        }
    }

    #[test]
    fn remove_orphaned_dynamic_items_removes_exactly_what_the_count_predicts() {
        let mut session = load_fixture_session("v1_relics");
        let items_before =
            world::dynamic_item_values(&session.level).expect("dynamic item values").len();
        let predicted = count_orphaned_dynamic_items(&session).expect("count");
        assert!(predicted > 0, "the fixture must carry orphaned dynamic items");
        assert!(predicted < items_before, "not all of them may be orphans");

        let removed = remove_orphaned_dynamic_items(&mut session).expect("remove");
        assert_eq!(removed, predicted);

        let items_after =
            world::dynamic_item_values(&session.level).expect("dynamic item values").len();
        assert_eq!(items_after, items_before - removed);
        assert_eq!(count_orphaned_dynamic_items(&session).expect("count"), 0);

        assert_eq!(
            remove_orphaned_dynamic_items(&mut session).expect("remove"),
            0,
            "a second removal finds nothing left to remove"
        );
    }

    #[test]
    fn remove_orphaned_dynamic_items_spares_a_referenced_entry() {
        let mut session = load_fixture_session("v1_relics");
        let referenced_id = {
            let entries = world::item_container_map(&session.level).expect("item container map");
            entries
                .iter()
                .find_map(|entry| {
                    let value_props = props::struct_props(&entry.value)?;
                    let slot_values =
                        props::get(value_props, &["Slots"]).and_then(props::struct_values)?;
                    slot_values.iter().find_map(|slot_value| {
                        let StructValue::Struct(slot_props) = slot_value else { return None };
                        let Property::Struct(StructValue::Game(PalStruct::ItemContainerSlots(raw))) =
                            slot_props.0.get(&PropertyKey::from("RawData"))?
                        else {
                            return None;
                        };
                        item_dynamic_id(&raw.item)
                    })
                })
                .expect("the fixture must carry at least one slotted dynamic item")
        };

        remove_orphaned_dynamic_items(&mut session).expect("remove");

        let ids = world::dynamic_item_values(&session.level)
            .expect("dynamic item values")
            .iter()
            .filter_map(dynamic_item_id)
            .collect::<HashSet<_>>();
        assert!(
            ids.contains(&referenced_id),
            "a dynamic item a container slot still points at must survive the sweep"
        );
    }
}
