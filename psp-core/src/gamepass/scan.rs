//! Gamepass save discovery and LevelMeta world-name access.

use std::collections::HashMap;
use std::path::Path;

use crate::dto::gamepass::{GamepassContainerInfo, GamepassSaveData};
use crate::dto::ordered_map::OrderedMap;
use crate::error::CoreError;
use crate::gamepass::format::{guid_file_name, ContainerEntry, ContainerIndex};
use crate::gamepass::store;
use crate::savio;
use crate::session;

/// Returns the LevelMeta `WorldName`, or `"Unknown World"` when the property is
/// absent or empty. `savio::read_sav_bytes` handles PlM/Oodle, PlZ/zlib and CNK
/// alike, so a real Xbox LevelMeta.sav parses whichever way it was compressed.
pub fn world_name_from_level_meta(level_meta_sav: &[u8]) -> Result<String, CoreError> {
    let save = savio::read_sav_bytes(level_meta_sav)?;
    Ok(session::world_name_property(&save.root.properties)
        .unwrap_or_else(|| "Unknown World".to_string()))
}

/// Sets `SaveData.WorldName` and re-emits as PlM/Oodle (save-type `0x31`).
pub fn set_world_name_in_level_meta(
    level_meta_sav: &[u8],
    world_name: &str,
) -> Result<Vec<u8>, CoreError> {
    let mut save = savio::read_sav_bytes(level_meta_sav)?;
    session::set_world_name_property(&mut save, world_name)?;
    savio::write_sav_bytes(&save)
}

/// Discovers every save in a wgs container dir. Saves come back in first-appearance
/// order within `containers.index`; a save with no readable LevelMeta is skipped.
pub fn scan_saves(container_dir: &Path) -> Result<OrderedMap<String, GamepassSaveData>, CoreError> {
    let index = ContainerIndex::read_from_dir(container_dir)?;

    // The HashMap only groups entries; `save_order` is what fixes output order.
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

        // read_first_blob picks the numerically-latest `container.<seq>` revision;
        // a lexicographic pick would read the stale seq 1 once a dir hits 10+.
        let world_name = match store::read_first_blob(container_dir, level_meta_entry)? {
            Some((_seq, blob)) => match world_name_from_level_meta(&blob) {
                Ok(name) => name,
                Err(_) => continue,
            },
            None => continue,
        };

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
        build_wgs_tree, reference_saves_dir, SyntheticPlayer, SyntheticSave,
    };

    /// A real, committed `LevelMeta.sav` (upstream PlZ reference corpus) to
    /// re-stamp in tests. Always present, so the tests using it run on a clean
    /// checkout rather than skipping.
    fn base_level_meta_bytes() -> Vec<u8> {
        std::fs::read(reference_saves_dir().join("LevelMeta.sav")).unwrap()
    }

    /// Adds a `container.<seq>` file list plus its blob to an existing blob dir,
    /// so a test can stage a revision above the fixture builder's `container.1`.
    fn write_extra_container_revision(
        blob_dir: &std::path::Path,
        seq: u32,
        name: &str,
        data: &[u8],
    ) {
        use crate::gamepass::format::{write_utf16_fixed_64, CONTAINER_FILE_LIST_VERSION};
        let file_uuid = uuid::Uuid::new_v4();
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&CONTAINER_FILE_LIST_VERSION.to_le_bytes());
        buffer.extend_from_slice(&1u32.to_le_bytes()); // file count
        write_utf16_fixed_64(&mut buffer, name).unwrap();
        buffer.extend_from_slice(&[0u8; 16]); // cloud UUID
        buffer.extend_from_slice(file_uuid.as_bytes());
        std::fs::write(blob_dir.join(guid_file_name(&file_uuid)), data).unwrap();
        std::fs::write(blob_dir.join(format!("container.{seq}")), buffer).unwrap();
    }

    #[test]
    fn world_name_round_trips_through_level_meta() {
        let meta_bytes = base_level_meta_bytes();
        let original_name = world_name_from_level_meta(&meta_bytes).unwrap();
        assert!(!original_name.is_empty());

        let renamed = set_world_name_in_level_meta(&meta_bytes, "Rust World").unwrap();
        assert_eq!(&renamed[8..12], b"PlM1"); // PlM magic + save_type 0x31
        assert_eq!(world_name_from_level_meta(&renamed).unwrap(), "Rust World");
    }

    #[test]
    fn scan_saves_builds_gamepass_save_data() {
        let meta_bytes = base_level_meta_bytes();
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

    /// Regression: `"container.1" < "container.10"` as strings, so a lexicographic
    /// revision pick reads the stale seq-1 blob and yields "OLD" instead of "NEW".
    #[test]
    fn scan_saves_reads_numerically_latest_level_meta_revision_not_lexicographic() {
        let base_meta = base_level_meta_bytes();
        let old_meta = set_world_name_in_level_meta(&base_meta, "OLD").unwrap();
        let new_meta = set_world_name_in_level_meta(&base_meta, "NEW").unwrap();
        assert_eq!(world_name_from_level_meta(&old_meta).unwrap(), "OLD");
        assert_eq!(world_name_from_level_meta(&new_meta).unwrap(), "NEW");

        let temp = tempfile::tempdir().unwrap();
        let save = SyntheticSave {
            save_id: "0123456789ABCDEF0123456789ABCDEF".to_string(),
            level_sav: b"LEVEL".to_vec(),
            level_meta: Some(old_meta), // fixture writes this as container.1 ("OLD")
            local_data: None,
            world_option: None,
            players: vec![],
        };
        let container_dir = build_wgs_tree(temp.path(), &[save]).unwrap();

        // Add container.10 ("NEW") into the same LevelMeta blob dir.
        let index = ContainerIndex::read_from_dir(&container_dir).unwrap();
        let latest = index.latest_save_containers("0123456789ABCDEF0123456789ABCDEF");
        let meta_entry = latest.get("LevelMeta").unwrap();
        let meta_dir = container_dir.join(guid_file_name(&meta_entry.container_uuid));
        write_extra_container_revision(&meta_dir, 10, "Data", &new_meta);

        let saves = scan_saves(&container_dir).unwrap();
        let data = saves.get("0123456789ABCDEF0123456789ABCDEF").unwrap();
        assert_eq!(
            data.world_name, "NEW",
            "scan_saves must read the numerically-latest LevelMeta revision (container.10), not the lexicographically-first (container.1)"
        );
    }

    /// Reads world name from a real PlM/Oodle-compressed `LevelMeta.sav` (the
    /// committed world1 fixture), covering the Oodle decompression path that the
    /// PlZ reference corpus does not exercise.
    #[test]
    fn world_name_from_oodle_level_meta() {
        let level_meta_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/saves/world1/LevelMeta.sav");
        let meta_bytes = std::fs::read(&level_meta_path).unwrap();
        assert_eq!(&meta_bytes[8..12], b"PlM1", "world1 LevelMeta is PlM/Oodle");
        let world_name = world_name_from_level_meta(&meta_bytes).unwrap();
        assert!(
            !world_name.is_empty(),
            "expected a non-empty world name from the Oodle LevelMeta.sav"
        );
    }
}
