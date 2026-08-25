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
}
