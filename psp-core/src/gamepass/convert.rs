//! Steam↔gamepass conversion plumbing.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use crate::dto::ordered_map::OrderedMap;
use crate::error::CoreError;
use crate::gamepass::format::{ContainerEntry, ContainerIndex};
use crate::gamepass::store::{backup_container_dir, create_container, read_first_blob};
use crate::progress::ProgressSink;

/// Re-emits a save as PlM/Oodle, the format Steam expects. Xbox saves arrive as
/// PlZ (zlib) or CNK; already-PlM input passes through byte-identical.
pub fn recompress_to_plm(data: &[u8]) -> Result<Vec<u8>, CoreError> {
    if data.len() > 12 && &data[8..12] == b"PlM1" {
        return Ok(data.to_vec());
    }
    let gvas_bytes = crate::ue::compression::decompress_save(&mut Cursor::new(data))
        .map_err(|error| CoreError::Parse(error.to_string()))?;
    crate::ue::compression::compress_save(&gvas_bytes, crate::ue::compression::CompressionFormat::Oodle)
        .map_err(|error| CoreError::Parse(error.to_string()))
}

/// Picks which progress strings `extract_containers_to_steam_dir` emits: converting
/// one chosen save, or one of many (where the save id is worth naming).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractLabels {
    SelectedSave,
    AllSaves,
}

impl ExtractLabels {
    fn for_key(&self, key: &str, save_id: &str) -> Option<String> {
        let label = match (self, key) {
            (ExtractLabels::SelectedSave, "Level") => {
                "Converting Level.sav to Steam format...".to_string()
            }
            (ExtractLabels::SelectedSave, "LevelMeta") => "Extracting LevelMeta.sav...".to_string(),
            (ExtractLabels::SelectedSave, "LocalData") => "Extracting LocalData.sav...".to_string(),
            (ExtractLabels::SelectedSave, "WorldOption") => {
                "Extracting WorldOption.sav...".to_string()
            }
            (ExtractLabels::AllSaves, "Level") => {
                format!("Converting Level.sav for {save_id}...")
            }
            (ExtractLabels::AllSaves, "LevelMeta") => {
                format!("Extracting LevelMeta.sav for {save_id}...")
            }
            (ExtractLabels::AllSaves, "LocalData") => {
                format!("Extracting LocalData.sav for {save_id}...")
            }
            (ExtractLabels::AllSaves, "WorldOption") => {
                format!("Extracting WorldOption.sav for {save_id}...")
            }
            _ => return None,
        };
        Some(label)
    }
}

/// Writes one gamepass save as a steam save directory; returns `<output_root>/<save_id>`.
pub fn extract_containers_to_steam_dir(
    container_dir: &Path,
    save_id: &str,
    containers: &OrderedMap<String, ContainerEntry>,
    output_root: &Path,
    labels: ExtractLabels,
    progress: &ProgressSink,
) -> Result<PathBuf, CoreError> {
    let save_dir = output_root.join(save_id);
    std::fs::create_dir_all(&save_dir)?;
    let players_dir = save_dir.join("Players");
    std::fs::create_dir_all(&players_dir)?;

    for (key, entry) in containers.iter() {
        let Some((_seq, payload)) = read_first_blob(container_dir, entry)? else {
            continue; // missing blob dir or empty file list
        };
        if let Some(player_id) = key.strip_prefix("Players-") {
            progress(&format!("Extracting player {player_id}..."));
            std::fs::write(
                players_dir.join(format!("{player_id}.sav")),
                recompress_to_plm(&payload)?,
            )?;
        } else if let Some(label) = labels.for_key(key, save_id) {
            progress(&label);
            std::fs::write(
                save_dir.join(format!("{key}.sav")),
                recompress_to_plm(&payload)?,
            )?;
        }
    }
    Ok(save_dir)
}

/// Unique save ids from container names (`<id>-<suffix>`), in first-seen order so
/// bulk conversions process saves deterministically.
pub fn unique_save_ids(index: &ContainerIndex) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for entry in &index.containers {
        if let Some((save_id, _)) = entry.container_name.split_once('-') {
            if !seen.iter().any(|known| known == save_id) {
                seen.push(save_id.to_string());
            }
        }
    }
    seen
}

/// Backs up the container dir, drops `EggTest` ghost entries, then writes Level,
/// LevelMeta and player containers under a fresh save id. Save ids are uppercase
/// dashless uuid4 — the form the game itself writes into container names.
pub fn import_steam_dir_to_gamepass(
    source_dir: &Path,
    container_dir: &Path,
    mut index: ContainerIndex,
    backups_root: &Path,
    progress: &ProgressSink,
) -> Result<String, CoreError> {
    progress("Creating backup...");
    backup_container_dir(container_dir, backups_root)?;
    index
        .containers
        .retain(|entry| !entry.container_name.starts_with("EggTest"));

    let new_save_id = uuid::Uuid::new_v4().as_simple().to_string().to_uppercase();

    progress("Creating Level container...");
    let level_bytes = std::fs::read(source_dir.join("Level.sav"))?;
    index.containers.push(create_container(
        container_dir,
        &new_save_id,
        &level_bytes,
        "Data",
        "Level",
    )?);

    let level_meta_path = source_dir.join("LevelMeta.sav");
    if level_meta_path.exists() {
        progress("Creating LevelMeta container...");
        let meta_bytes = std::fs::read(&level_meta_path)?;
        index.containers.push(create_container(
            container_dir,
            &new_save_id,
            &meta_bytes,
            "Data",
            "LevelMeta",
        )?);
    }

    let players_dir = source_dir.join("Players");
    if players_dir.exists() {
        let mut player_paths: Vec<PathBuf> = std::fs::read_dir(&players_dir)?
            .flatten()
            .map(|dir_entry| dir_entry.path())
            .filter(|path| path.extension().map(|ext| ext == "sav").unwrap_or(false))
            .collect();
        player_paths.sort();
        for player_path in player_paths {
            let player_id = player_path
                .file_stem()
                .map(|stem| stem.to_string_lossy().to_string())
                .unwrap_or_default();
            progress(&format!("Creating player container: {player_id}..."));
            let player_bytes = std::fs::read(&player_path)?;
            index.containers.push(create_container(
                container_dir,
                &new_save_id,
                &player_bytes,
                "Data",
                &format!("Players-{player_id}"),
            )?);
        }
    }

    progress("Writing container index...");
    index.write_to_dir(container_dir)?;
    Ok(new_save_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::ordered_map::OrderedMap;
    use crate::gamepass::fixture::{
        build_wgs_tree, reference_saves_dir, SyntheticPlayer, SyntheticSave,
    };
    use crate::gamepass::format::ContainerIndex;
    use crate::progress::null_progress;

    #[test]
    fn recompress_to_plm_converts_plz_and_passes_plm_through() {
        // The committed reference saves are PlZ (zlib).
        let plz_bytes = std::fs::read(reference_saves_dir().join("LevelMeta.sav")).unwrap();
        assert_eq!(&plz_bytes[8..11], b"PlZ");

        let plm_bytes = recompress_to_plm(&plz_bytes).unwrap();
        assert_eq!(&plm_bytes[8..12], b"PlM1");
        let original_gvas =
            crate::ue::compression::decompress_save(&mut std::io::Cursor::new(plz_bytes.as_slice()))
                .unwrap();
        let recompressed_gvas =
            crate::ue::compression::decompress_save(&mut std::io::Cursor::new(plm_bytes.as_slice()))
                .unwrap();
        assert_eq!(original_gvas, recompressed_gvas);

        assert_eq!(recompress_to_plm(&plm_bytes).unwrap(), plm_bytes);
    }

    #[test]
    fn extract_writes_steam_layout_with_selected_save_progress_labels() {
        let testdata = reference_saves_dir();
        let meta_bytes = std::fs::read(testdata.join("LevelMeta.sav")).unwrap();
        let level_bytes = std::fs::read(testdata.join("Level.sav")).unwrap();
        let player_bytes =
            std::fs::read(testdata.join("00000000000000000000000000000001.sav")).unwrap();

        let temp = tempfile::tempdir().unwrap();
        let player_id = uuid::Uuid::parse_str("00000000000000000000000000000001").unwrap();
        let save = SyntheticSave {
            save_id: "0123456789ABCDEF0123456789ABCDEF".to_string(),
            level_sav: level_bytes,
            level_meta: Some(meta_bytes),
            local_data: None,
            world_option: None,
            players: vec![SyntheticPlayer {
                id: player_id,
                sav: player_bytes,
                dps: None,
            }],
        };
        let container_dir = build_wgs_tree(temp.path(), &[save]).unwrap();
        let index = ContainerIndex::read_from_dir(&container_dir).unwrap();
        let containers: OrderedMap<String, crate::gamepass::format::ContainerEntry> =
            index.latest_save_containers("0123456789ABCDEF0123456789ABCDEF");

        let messages = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let sink_messages = messages.clone();
        let progress: crate::progress::ProgressSink = std::sync::Arc::new(move |message: &str| {
            sink_messages.lock().unwrap().push(message.to_string());
        });

        let output_root = temp.path().join("steam-out");
        let save_dir = extract_containers_to_steam_dir(
            &container_dir,
            "0123456789ABCDEF0123456789ABCDEF",
            &containers,
            &output_root,
            ExtractLabels::SelectedSave,
            &progress,
        )
        .unwrap();

        assert_eq!(
            save_dir,
            output_root.join("0123456789ABCDEF0123456789ABCDEF")
        );
        for relative in [
            "Level.sav",
            "LevelMeta.sav",
            "Players/00000000000000000000000000000001.sav",
        ] {
            let written = std::fs::read(save_dir.join(relative)).unwrap();
            assert_eq!(&written[8..12], b"PlM1", "not PlM: {relative}");
        }
        let recorded = messages.lock().unwrap();
        assert!(recorded.contains(&"Converting Level.sav to Steam format...".to_string()));
        assert!(recorded.contains(&"Extracting LevelMeta.sav...".to_string()));
        assert!(
            recorded.contains(&"Extracting player 00000000000000000000000000000001...".to_string())
        );
    }

    #[test]
    fn import_steam_dir_creates_containers_under_new_save_id() {
        let testdata = reference_saves_dir();
        let temp = tempfile::tempdir().unwrap();

        let source_dir = temp.path().join("steam-src");
        std::fs::create_dir_all(source_dir.join("Players")).unwrap();
        std::fs::copy(testdata.join("Level.sav"), source_dir.join("Level.sav")).unwrap();
        std::fs::copy(
            testdata.join("LevelMeta.sav"),
            source_dir.join("LevelMeta.sav"),
        )
        .unwrap();
        std::fs::copy(
            testdata.join("00000000000000000000000000000001.sav"),
            source_dir
                .join("Players")
                .join("00000000000000000000000000000001.sav"),
        )
        .unwrap();

        // Existing gamepass container dir with an EggTest ghost entry.
        let container_dir = build_wgs_tree(temp.path(), &[]).unwrap();
        let mut index = ContainerIndex::read_from_dir(&container_dir).unwrap();
        index
            .containers
            .push(crate::gamepass::format::ContainerEntry {
                container_name: "EggTest-Level".to_string(),
                cloud_id: String::new(),
                seq: 1,
                flag: 5,
                container_uuid: uuid::Uuid::new_v4(),
                mtime: crate::gamepass::format::Filetime(1),
                size: 0,
            });

        let backups = temp.path().join("backups");
        let new_save_id = import_steam_dir_to_gamepass(
            &source_dir,
            &container_dir,
            index,
            &backups,
            &null_progress(),
        )
        .unwrap();
        assert_eq!(new_save_id.len(), 32);
        assert!(new_save_id
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()));

        let reloaded = ContainerIndex::read_from_dir(&container_dir).unwrap();
        assert!(!reloaded
            .containers
            .iter()
            .any(|entry| entry.container_name.starts_with("EggTest")));
        let latest = reloaded.latest_save_containers(&new_save_id);
        assert!(latest.get("Level").is_some());
        assert!(latest.get("LevelMeta").is_some());
        assert!(latest
            .get("Players-00000000000000000000000000000001")
            .is_some());
    }
}
