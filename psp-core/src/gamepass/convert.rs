//! Steam↔gamepass conversion plumbing.
//! Port of convert_handler.py (recompress_to_steam, _convert_gamepass_save_to_steam,
//! _standalone_gamepass_to_steam, _standalone_steam_to_gamepass).

use std::io::Cursor;
use std::path::{Path, PathBuf};

use crate::dto::ordered_map::OrderedMap;
use crate::error::CoreError;
use crate::gamepass::format::{ContainerEntry, ContainerIndex};
use crate::gamepass::store::{backup_container_dir, create_container, read_first_blob};
use crate::progress::ProgressSink;

/// Port of recompress_to_steam (convert_handler.py:38-44).
pub fn recompress_to_plm(data: &[u8]) -> Result<Vec<u8>, CoreError> {
    if data.len() > 12 && &data[8..12] == b"PlM1" {
        return Ok(data.to_vec());
    }
    let gvas_bytes = uesave::compression::decompress_save(&mut Cursor::new(data))
        .map_err(|error| CoreError::Parse(error.to_string()))?;
    uesave::compression::compress_save(&gvas_bytes, uesave::compression::CompressionFormat::Oodle)
        .map_err(|error| CoreError::Parse(error.to_string()))
}

/// Selects the exact progress strings the two Python code paths emit:
/// SelectedSave = _convert_gamepass_save_to_steam (convert_handler.py:213-234),
/// AllSaves     = _standalone_gamepass_to_steam   (convert_handler.py:558-578).
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
///
/// `containers` is `OrderedMap`, not `indexmap::IndexMap`: this port deliberately keeps
/// `indexmap` out of `psp-core`'s direct dependencies (see `dto::ordered_map`'s module
/// doc). Callers pass the result of `ContainerIndex::latest_save_containers` (Task 5),
/// which already returns `OrderedMap` — no conversion needed at the call site.
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
            continue; // missing dir or empty file list: Python logs and skips
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

/// First-seen-order unique save ids from container names ("<id>-<suffix>").
/// Python builds this with an unordered `set()` (convert_handler.py:502-506); this is
/// a deliberate deterministic improvement, not a parity gap — see Task 13's parity note.
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

/// Port of _standalone_steam_to_gamepass after index read (convert_handler.py:618-664):
/// backup, drop EggTest ghosts, create Level/LevelMeta/player containers under a fresh
/// uppercase dashless uuid4 save id, write the index. Progress strings must match.
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
        build_wgs_tree, python_testdata_dir, SyntheticPlayer, SyntheticSave,
    };
    use crate::gamepass::format::ContainerIndex;
    use crate::progress::null_progress;

    fn testdata_or_skip() -> Option<std::path::PathBuf> {
        let dir = python_testdata_dir();
        if dir.is_none() {
            eprintln!("SKIP: python testdata not found (set PSP_PY_TESTDATA)");
        }
        dir
    }

    #[test]
    fn recompress_to_plm_converts_plz_and_passes_plm_through() {
        let Some(testdata) = testdata_or_skip() else {
            return;
        };
        // Committed test saves are PlZ (zlib).
        let plz_bytes = std::fs::read(testdata.join("LevelMeta.sav")).unwrap();
        assert_eq!(&plz_bytes[8..11], b"PlZ");

        let plm_bytes = recompress_to_plm(&plz_bytes).unwrap();
        assert_eq!(&plm_bytes[8..12], b"PlM1");
        // Same GVAS payload after decompression.
        let original_gvas =
            uesave::compression::decompress_save(&mut std::io::Cursor::new(plz_bytes.as_slice()))
                .unwrap();
        let recompressed_gvas =
            uesave::compression::decompress_save(&mut std::io::Cursor::new(plm_bytes.as_slice()))
                .unwrap();
        assert_eq!(original_gvas, recompressed_gvas);

        // Already PlM: byte-identical pass-through.
        assert_eq!(recompress_to_plm(&plm_bytes).unwrap(), plm_bytes);
    }

    /// Recompresses a REAL Xbox gamepass `Level.sav` (CNK-compressed, pulled from the
    /// on-disk backup corpus) to PlM and asserts the GVAS payload survives unchanged.
    /// This is the actual CNK->PlM Xbox conversion path validated end-to-end against
    /// real data, not just the synthetic/PlZ testdata fixture. Skipped, not failed,
    /// when the corpus isn't checked out.
    #[test]
    fn recompress_to_plm_converts_real_gamepass_cnk_level_to_plm() {
        let level_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../backups/gamepass/000900000487F3B6_0000000000000000000000006B210A9C_20260325231642/CC9746994B05F767129BC48B346B691D/Level.sav",
        );
        if !level_path.exists() {
            eprintln!(
                "skipping recompress_to_plm_converts_real_gamepass_cnk_level_to_plm: {} not found",
                level_path.display()
            );
            return;
        }
        let cnk_bytes = std::fs::read(&level_path).unwrap();
        assert_eq!(&cnk_bytes[8..11], b"CNK");

        let plm_bytes = recompress_to_plm(&cnk_bytes).unwrap();
        assert_eq!(&plm_bytes[8..12], b"PlM1");

        let original_gvas =
            uesave::compression::decompress_save(&mut std::io::Cursor::new(cnk_bytes.as_slice()))
                .unwrap();
        let recompressed_gvas =
            uesave::compression::decompress_save(&mut std::io::Cursor::new(plm_bytes.as_slice()))
                .unwrap();
        assert_eq!(
            original_gvas, recompressed_gvas,
            "GVAS payload must be preserved when recompressing a real CNK Level.sav to PlM"
        );
    }

    #[test]
    fn extract_writes_steam_layout_with_selected_save_progress_labels() {
        let Some(testdata) = testdata_or_skip() else {
            return;
        };
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
        let Some(testdata) = testdata_or_skip() else {
            return;
        };
        let temp = tempfile::tempdir().unwrap();

        // Steam-style source dir from the committed test saves.
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
