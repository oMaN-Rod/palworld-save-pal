//! Gamepass save discovery (FileManager.parse_gamepass_saves, file_manager.py:261-392)
//! and LevelMeta world-name access.

use std::collections::HashMap;
use std::path::Path;

use crate::dto::gamepass::{GamepassContainerInfo, GamepassSaveData};
use crate::dto::ordered_map::OrderedMap;
use crate::error::CoreError;
use crate::gamepass::format::{guid_file_name, ContainerEntry, ContainerFileList, ContainerIndex};
use crate::savio;
use crate::session;

/// Returns the LevelMeta `WorldName`, or `"Unknown World"` when the property
/// is absent or empty — port of `FileManager.read_level_meta`
/// (file_manager.py:252-258). Reads through `savio::read_sav_bytes`, the
/// same compressed-layer read every other `.sav` in this port goes through
/// (registers `palworld_types()` and handles PlM/Oodle, PlZ/zlib, and CNK
/// payloads alike), so a real Xbox LevelMeta.sav parses here regardless of
/// which compression format it was written in. Property navigation is
/// shared with `session::world_name_from_meta_properties`
/// (`SaveManager._load_world_name`'s fallback-`"Unknown"` cousin) via
/// `session::world_name_property`, not reimplemented — the two Python
/// originals read the identical `SaveData.WorldName` path and differ only
/// in their fallback text.
pub fn world_name_from_level_meta(level_meta_sav: &[u8]) -> Result<String, CoreError> {
    let save = savio::read_sav_bytes(level_meta_sav)?;
    Ok(session::world_name_property(&save.root.properties)
        .unwrap_or_else(|| "Unknown World".to_string()))
}

/// Sets `SaveData.WorldName` and re-emits as PlM/Oodle (save-type `0x31`),
/// matching `SaveManager().sav(level_meta)` as used by
/// `rename_gamepass_world` / `copy_container`. Property mutation is shared
/// with `SaveSession::set_world_name` via `session::set_world_name_property`
/// — this function only adds the standalone read/write bookends around it.
pub fn set_world_name_in_level_meta(
    level_meta_sav: &[u8],
    world_name: &str,
) -> Result<Vec<u8>, CoreError> {
    let mut save = savio::read_sav_bytes(level_meta_sav)?;
    session::set_world_name_property(&mut save.root.properties, world_name)?;
    savio::write_sav_bytes(&save)
}

/// Port of `FileManager.parse_gamepass_saves` (file_manager.py:261-392).
///
/// Returns `OrderedMap`, not `indexmap::IndexMap`: this port deliberately
/// keeps `indexmap` out of `psp-core`'s direct dependencies (see
/// `dto::ordered_map`'s module doc and `session.rs`'s `loaded_players` doc
/// comment for the project-wide reconciliation). Insertion order still
/// matches Python's `saves` dict, which follows `latest_containers`'
/// insertion order — itself the first-appearance order of each save id
/// while scanning `container_index.containers` — so results are collected
/// into `saves` in exactly that order below.
pub fn scan_saves(container_dir: &Path) -> Result<OrderedMap<String, GamepassSaveData>, CoreError> {
    let index = ContainerIndex::read_from_dir(container_dir)?;

    // Every container entry grouped by save id, plus the first-seen order of
    // each save id — port of Python's `all_containers_by_save` dict, whose
    // insertion order the outer `saves` result also follows. The grouping
    // itself (which entries land under which save id) doesn't depend on
    // iteration order, so a plain `HashMap` holds the groups; `save_order`
    // is the separate ordered list that actually drives output order.
    let mut save_order: Vec<String> = Vec::new();
    let mut containers_by_save: HashMap<String, Vec<ContainerEntry>> = HashMap::new();
    for entry in &index.containers {
        let Some((save_id, _suffix)) = entry.container_name.split_once('-') else {
            continue;
        };
        if !containers_by_save.contains_key(save_id) {
            save_order.push(save_id.to_string());
        }
        containers_by_save
            .entry(save_id.to_string())
            .or_default()
            .push(entry.clone());
    }

    let mut saves: OrderedMap<String, GamepassSaveData> = OrderedMap::new();
    for save_id in &save_order {
        let all_for_save = containers_by_save
            .get(save_id)
            .expect("save_order only contains keys present in containers_by_save");
        let latest = index.latest_save_containers(save_id);
        let Some(level_meta_entry) = latest.get("LevelMeta") else {
            continue;
        };
        let meta_dir = container_dir.join(guid_file_name(&level_meta_entry.container_uuid));
        if !meta_dir.exists() {
            continue;
        }

        // First container.* file only, matching Python's `break`
        // (file_manager.py:318-328).
        let mut world_name = "Unknown World".to_string();
        let mut valid = false;
        let mut list_paths: Vec<std::path::PathBuf> = std::fs::read_dir(&meta_dir)?
            .flatten()
            .map(|dir_entry| dir_entry.path())
            .filter(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().starts_with("container."))
                    .unwrap_or(false)
            })
            .collect();
        list_paths.sort();
        if let Some(first_list_path) = list_paths.first() {
            match ContainerFileList::read_from_file(first_list_path) {
                Ok(file_list) => {
                    if let Some(first_file) = file_list.files.first() {
                        match world_name_from_level_meta(&first_file.data) {
                            Ok(name) => {
                                valid = true;
                                world_name = name;
                            }
                            Err(_) => continue, // unreadable meta: skip this save
                        }
                    }
                }
                Err(_) => continue,
            }
        }
        if !valid {
            continue;
        }

        let player_count = latest
            .iter()
            .filter(|(_, entry)| {
                entry.container_name.contains("Player") && !entry.container_name.contains("_dps")
            })
            .count();

        let mut container_infos: Vec<GamepassContainerInfo> = all_for_save
            .iter()
            .map(|entry| {
                let container_type = entry
                    .container_name
                    .split_once('-')
                    .map(|(_, suffix)| suffix.to_string())
                    .unwrap_or_else(|| entry.container_name.clone());
                GamepassContainerInfo {
                    container_type,
                    seq: entry.seq,
                    last_modified: entry.mtime.to_unix_seconds(),
                    size: entry.size,
                    container_name: entry.container_name.clone(),
                }
            })
            .collect();
        container_infos.sort_by(|a, b| {
            a.container_type
                .cmp(&b.container_type)
                .then(b.seq.cmp(&a.seq))
        });

        let last_modified = latest
            .iter()
            .map(|(_, entry)| entry.mtime.to_unix_seconds())
            .fold(0.0_f64, f64::max);
        let total_size: u64 = latest.iter().map(|(_, entry)| entry.size).sum();

        saves.insert(
            save_id.clone(),
            GamepassSaveData {
                save_id: save_id.clone(),
                world_name,
                player_count,
                last_modified,
                total_size,
                containers: container_infos,
            },
        );
    }
    Ok(saves)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gamepass::fixture::{
        build_wgs_tree, python_testdata_dir, SyntheticPlayer, SyntheticSave,
    };

    fn testdata_or_skip() -> Option<std::path::PathBuf> {
        let dir = python_testdata_dir();
        if dir.is_none() {
            eprintln!("SKIP: python testdata not found (set PSP_PY_TESTDATA)");
        }
        dir
    }

    #[test]
    fn world_name_round_trips_through_level_meta() {
        let Some(testdata) = testdata_or_skip() else {
            return;
        };
        let meta_bytes = std::fs::read(testdata.join("LevelMeta.sav")).unwrap();
        let original_name = world_name_from_level_meta(&meta_bytes).unwrap();
        assert!(!original_name.is_empty());

        let renamed = set_world_name_in_level_meta(&meta_bytes, "Rust World").unwrap();
        assert_eq!(&renamed[8..12], b"PlM1"); // PlM magic + save_type 0x31
        assert_eq!(world_name_from_level_meta(&renamed).unwrap(), "Rust World");
    }

    #[test]
    fn scan_saves_builds_gamepass_save_data() {
        let Some(testdata) = testdata_or_skip() else {
            return;
        };
        let meta_bytes = std::fs::read(testdata.join("LevelMeta.sav")).unwrap();
        let expected_world = world_name_from_level_meta(&meta_bytes).unwrap();

        let temp = tempfile::tempdir().unwrap();
        let player_id = uuid::Uuid::new_v4();
        let save = SyntheticSave {
            save_id: "0123456789ABCDEF0123456789ABCDEF".to_string(),
            level_sav: b"LEVEL-PLACEHOLDER".to_vec(), // scan never parses Level.sav
            level_meta: Some(meta_bytes),
            local_data: None,
            world_option: None,
            players: vec![SyntheticPlayer {
                id: player_id,
                sav: b"P".to_vec(),
                dps: Some(b"D".to_vec()),
            }],
        };
        let container_dir = build_wgs_tree(temp.path(), &[save]).unwrap();

        let saves = scan_saves(&container_dir).unwrap();
        assert_eq!(saves.len(), 1);
        let data = saves.get("0123456789ABCDEF0123456789ABCDEF").unwrap();
        assert_eq!(data.world_name, expected_world);
        assert_eq!(data.player_count, 1); // dps container excluded
        assert_eq!(data.containers.len(), 4);
        assert!(data.last_modified > 0.0);
        assert!(data.total_size > 0);
        // Sorted by (container_type asc, seq desc)
        let types: Vec<&str> = data
            .containers
            .iter()
            .map(|c| c.container_type.as_str())
            .collect();
        let mut sorted = types.clone();
        sorted.sort();
        assert_eq!(types, sorted);
    }

    #[test]
    fn scan_saves_skips_saves_without_level_meta() {
        let temp = tempfile::tempdir().unwrap();
        let save = SyntheticSave {
            save_id: "FFFF0000FFFF0000FFFF0000FFFF0000".to_string(),
            level_sav: b"LEVEL".to_vec(),
            level_meta: None,
            local_data: None,
            world_option: None,
            players: vec![],
        };
        let container_dir = build_wgs_tree(temp.path(), &[save]).unwrap();
        assert!(scan_saves(&container_dir).unwrap().is_empty());
    }

    /// Reads a real gamepass `LevelMeta.sav` pulled from the on-disk gamepass
    /// backup corpus (not the synthetic `python_testdata_dir()` fixture,
    /// which is skipped when `PSP_PY_TESTDATA` is unset) and asserts the
    /// world name comes back non-empty. Real Xbox `Level.sav` files in this
    /// corpus are CNK-compressed; this specific `LevelMeta.sav` happens to
    /// already be PlM/Oodle, but both are real Xbox-produced payloads read
    /// through the exact same `savio::read_sav_bytes` -> `palworld_types()`
    /// path production code uses, so this still validates that path
    /// end-to-end against real (not synthetic) data. Skipped, not failed,
    /// when the corpus isn't checked out.
    #[test]
    fn world_name_from_real_gamepass_level_meta_corpus_when_present() {
        let level_meta_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../backups/gamepass/000900000487F3B6_0000000000000000000000006B210A9C_20260325231642/4F64BAB699AE4B4A97A5862116E07C6D/LevelMeta.sav",
        );
        if !level_meta_path.exists() {
            eprintln!(
                "skipping world_name_from_real_gamepass_level_meta_corpus_when_present: {} not found",
                level_meta_path.display()
            );
            return;
        }
        let meta_bytes = std::fs::read(&level_meta_path).unwrap();
        let world_name = world_name_from_level_meta(&meta_bytes).unwrap();
        assert!(
            !world_name.is_empty(),
            "expected a non-empty world name from the real gamepass LevelMeta.sav"
        );
    }
}
