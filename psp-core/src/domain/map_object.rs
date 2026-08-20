//! World-wide operations over `MapObjectSaveData` entries.

use crate::domain::world;
use crate::error::CoreError;
use crate::props;
use crate::session::SaveSession;
use crate::ue::games::palworld::PalMapConcreteModelVariant;
use crate::ue::{FGuid, PalStruct, Property, PropertyKey, StructValue};

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

/// Mirrors `blueprint::capture::with_concrete_variant_mut`; kept separate
/// because that helper is private to `blueprint`.
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

/// The single definition of "locked" -- `count_private_chest_locks` and
/// `unlock_private_chests` both decide through this, so they cannot disagree.
fn is_locked(variant: &PalMapConcreteModelVariant<crate::ue::Arch>) -> bool {
    lock_field(variant).is_some_and(|lock| *lock != FGuid::nil())
}

/// Deliberately leaves `PasswordLock` module state untouched, matching
/// PalworldSaveTools' original private-chest unlock.
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
}
