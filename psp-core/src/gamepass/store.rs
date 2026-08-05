//! Filesystem operations over a wgs container directory.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::dto::ordered_map::OrderedMap;
use crate::error::CoreError;
use crate::gamepass::format::{
    guid_file_name, write_utf16_fixed_64, ContainerEntry, ContainerFileList, ContainerIndex,
    Filetime, CONTAINER_FILE_LIST_VERSION,
};
use crate::gamepass::PlayerSavBytes;

/// wgs names its container dirs `<16 hex>_<32 hex>`, uppercase only.
pub fn is_wgs_container_dir_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() == 49
        && bytes[16] == b'_'
        && bytes[..16]
            .iter()
            .chain(&bytes[17..])
            .all(|c| c.is_ascii_digit() || (b'A'..=b'F').contains(c))
}

/// The Packages dir of the Xbox Palworld install, under `%LOCALAPPDATA%`.
/// PSP_GAMEPASS_PACKAGES_ROOT lets tests point at a synthetic tree instead.
fn default_packages_root() -> PathBuf {
    if let Ok(root) = std::env::var("PSP_GAMEPASS_PACKAGES_ROOT") {
        return PathBuf::from(root);
    }
    let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_default();
    Path::new(&local_app_data)
        .join("Packages")
        .join("PocketpairInc.Palworld_ad4psfrxyesvt")
}

pub fn find_container_dir() -> Result<PathBuf, CoreError> {
    find_container_dir_under(&default_packages_root())
}

pub fn find_container_dir_under(packages_root: &Path) -> Result<PathBuf, CoreError> {
    if !packages_root.exists() {
        return Err(CoreError::Other(
            "Could not find Xbox Palworld installation".to_string(),
        ));
    }
    let wgs_dir = packages_root.join("SystemAppData").join("wgs");
    if wgs_dir.exists() {
        for dir_entry in std::fs::read_dir(&wgs_dir)?.flatten() {
            let name = dir_entry.file_name().to_string_lossy().to_string();
            if is_wgs_container_dir_name(&name) {
                return Ok(dir_entry.path());
            }
        }
    }
    Err(CoreError::Other(
        "Could not find container path. Try running the game once.".to_string(),
    ))
}

/// `backups` under the app root; override with PSP_BACKUPS_ROOT.
pub fn backups_root() -> PathBuf {
    std::env::var("PSP_BACKUPS_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| crate::paths::app_root().join("backups"))
}

pub(crate) fn copy_dir_recursive(source: &Path, destination: &Path) -> Result<(), CoreError> {
    std::fs::create_dir_all(destination)?;
    for dir_entry in std::fs::read_dir(source)?.flatten() {
        let target = destination.join(dir_entry.file_name());
        if dir_entry.file_type()?.is_dir() {
            copy_dir_recursive(&dir_entry.path(), &target)?;
        } else {
            std::fs::copy(dir_entry.path(), target)?;
        }
    }
    Ok(())
}

pub fn backup_container_dir(
    container_dir: &Path,
    backups_root: &Path,
) -> Result<PathBuf, CoreError> {
    std::fs::create_dir_all(backups_root)?;
    let timestamp = chrono::Local::now().format("%Y%m%d%H%M%S");
    let dir_name = container_dir
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    let backup_path = backups_root.join(format!("{dir_name}_{timestamp}"));
    copy_dir_recursive(container_dir, &backup_path)?;
    Ok(backup_path)
}

/// Reads a container entry's payload: the first blob of its newest non-empty
/// `container.<seq>` file list, with that list's seq. `None` when the blob dir is
/// absent or holds no usable file list.
pub fn read_first_blob(
    container_dir: &Path,
    entry: &ContainerEntry,
) -> Result<Option<(u32, Vec<u8>)>, CoreError> {
    let blob_dir = container_dir.join(guid_file_name(&entry.container_uuid));
    if !blob_dir.exists() {
        return Ok(None);
    }
    // Visit file lists in ascending NUMERIC seq. String sorting ranks
    // "container.10" before "container.2" and would return a stale blob once a
    // dir reaches 10+ revisions; the seq is the authoritative "latest" marker.
    let mut list_paths: Vec<(u32, PathBuf)> = std::fs::read_dir(&blob_dir)?
        .flatten()
        .map(|dir_entry| dir_entry.path())
        .filter_map(|path| {
            let seq: u32 = path.file_name().and_then(|name| {
                name.to_string_lossy()
                    .strip_prefix("container.")?
                    .parse()
                    .ok()
            })?;
            Some((seq, path))
        })
        .collect();
    list_paths.sort_by_key(|(seq, _)| *seq);
    let mut newest: Option<(u32, Vec<u8>)> = None;
    for (_, list_path) in list_paths {
        let file_list = ContainerFileList::read_from_file(&list_path)?;
        if let Some(first) = file_list.files.first() {
            newest = Some((file_list.seq, first.data.clone()));
        }
    }
    Ok(newest)
}

pub(crate) fn write_container_file_list(
    blob_dir: &Path,
    files: &[(String, uuid::Uuid, &[u8])],
) -> Result<(), CoreError> {
    std::fs::create_dir_all(blob_dir)?;
    let mut buffer = Vec::new();
    buffer.extend_from_slice(&CONTAINER_FILE_LIST_VERSION.to_le_bytes());
    buffer.extend_from_slice(&(files.len() as u32).to_le_bytes());
    for (name, file_uuid, data) in files {
        write_utf16_fixed_64(&mut buffer, name)?;
        buffer.extend_from_slice(&[0u8; 16]); // cloud UUID
        buffer.extend_from_slice(file_uuid.as_bytes());
        std::fs::write(blob_dir.join(guid_file_name(file_uuid)), data)?;
    }
    std::fs::write(blob_dir.join("container.1"), buffer)?;
    Ok(())
}

/// Writes a fresh GUID-named container dir holding `container.1` and one blob, and
/// returns its index entry (seq 1; flag 5 = local-only, i.e. no cloud id). The entry
/// is NOT appended to any index — callers do that.
pub fn create_container(
    container_dir: &Path,
    save_id: &str,
    data: &[u8],
    file_name: &str,
    container_suffix: &str,
) -> Result<ContainerEntry, CoreError> {
    let container_uuid = uuid::Uuid::new_v4();
    let file_uuid = uuid::Uuid::new_v4();
    let blob_dir = container_dir.join(guid_file_name(&container_uuid));
    write_container_file_list(&blob_dir, &[(file_name.to_string(), file_uuid, data)])?;
    Ok(ContainerEntry {
        container_name: format!("{save_id}-{container_suffix}"),
        cloud_id: String::new(),
        seq: 1,
        flag: 5,
        container_uuid,
        mtime: Filetime::now(),
        size: data.len() as u64,
    })
}

/// Collapses `-Slot<digits>-` to `-` and drops a trailing `-<exactly 2 digits>`,
/// the slot/revision decorations the game adds to container names.
pub fn clean_container_file_name(name: &str) -> String {
    let mut without_slot = String::with_capacity(name.len());
    let mut rest = name;
    while let Some(start) = rest.find("-Slot") {
        let after_slot = &rest[start + 5..];
        let digit_count = after_slot
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .count();
        if after_slot[digit_count..].starts_with('-') {
            without_slot.push_str(&rest[..start]);
            without_slot.push('-');
            rest = &after_slot[digit_count + 1..];
        } else {
            without_slot.push_str(&rest[..start + 5]);
            rest = after_slot;
        }
    }
    without_slot.push_str(rest);

    let bytes = without_slot.as_bytes();
    if bytes.len() >= 3
        && bytes[bytes.len() - 3] == b'-'
        && bytes[bytes.len() - 2].is_ascii_digit()
        && bytes[bytes.len() - 1].is_ascii_digit()
    {
        without_slot.truncate(without_slot.len() - 3);
    }
    without_slot
}

/// Copies every file of the source container into a fresh container dir under
/// `dest_dir`, renamed to `new_save_id`. LevelMeta payloads get the world name
/// rewritten; player AND WorldOption payloads are replaced when
/// `replacement_player_data` is given (`None` passes the original through).
///
/// Aggregates ALL `container.*` revisions rather than picking a latest one, so the
/// numeric-vs-lexicographic seq pitfall on `read_first_blob` doesn't apply: the sort
/// below only makes aggregation order deterministic, it never discards a revision.
pub fn copy_container(
    source: &ContainerEntry,
    source_dir: &Path,
    dest_dir: &Path,
    new_save_id: &str,
    key: &str,
    world_name: &str,
    replacement_player_data: Option<&[u8]>,
) -> Result<ContainerEntry, CoreError> {
    let source_blob_dir = source_dir.join(guid_file_name(&source.container_uuid));
    let mut source_files: Vec<ContainerFileList> = Vec::new();
    let mut list_paths: Vec<PathBuf> = std::fs::read_dir(&source_blob_dir)?
        .flatten()
        .map(|dir_entry| dir_entry.path())
        .filter(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().starts_with("container."))
                .unwrap_or(false)
        })
        .collect();
    list_paths.sort();
    for list_path in list_paths {
        source_files.push(ContainerFileList::read_from_file(&list_path)?);
    }

    let old_save_id = source
        .container_name
        .split('-')
        .next()
        .unwrap_or_default()
        .to_string();
    let new_container_name =
        clean_container_file_name(&source.container_name.replace(&old_save_id, new_save_id));

    let mut new_files: Vec<(String, uuid::Uuid, Vec<u8>)> = Vec::new();
    for file_list in &source_files {
        for file in &file_list.files {
            let data = if key == "LevelMeta" {
                crate::gamepass::scan::set_world_name_in_level_meta(&file.data, world_name)?
            } else if key.contains("Player") || key == "WorldOption" {
                match replacement_player_data {
                    Some(replacement) => replacement.to_vec(),
                    None => file.data.clone(),
                }
            } else {
                file.data.clone()
            };
            new_files.push((file.name.clone(), uuid::Uuid::new_v4(), data));
        }
    }

    let new_container_uuid = uuid::Uuid::new_v4();
    let new_blob_dir = dest_dir.join(guid_file_name(&new_container_uuid));
    let borrowed: Vec<(String, uuid::Uuid, &[u8])> = new_files
        .iter()
        .map(|(name, file_uuid, data)| (name.clone(), *file_uuid, data.as_slice()))
        .collect();
    write_container_file_list(&new_blob_dir, &borrowed)?;

    Ok(ContainerEntry {
        container_name: new_container_name,
        cloud_id: String::new(),
        seq: 1,
        flag: 5,
        container_uuid: new_container_uuid,
        mtime: Filetime::now(),
        size: new_files.iter().map(|(_, _, data)| data.len() as u64).sum(),
    })
}

/// Removes container dirs that are empty, have an empty file list, or have no index
/// entry, dropping any matching index entry along with them.
pub fn cleanup_container_dir(
    index: &mut ContainerIndex,
    container_dir: &Path,
) -> Result<(), CoreError> {
    for dir_entry in std::fs::read_dir(container_dir)?.flatten() {
        if !dir_entry.file_type()?.is_dir() {
            continue;
        }
        let dir_path = dir_entry.path();
        let dir_name = dir_entry.file_name().to_string_lossy().to_string();
        let mut should_remove = false;

        let child_paths: Vec<PathBuf> = std::fs::read_dir(&dir_path)?
            .flatten()
            .map(|child| child.path())
            .collect();
        if child_paths.is_empty() {
            should_remove = true;
        }
        if let Some(list_path) = child_paths.iter().find(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().starts_with("container."))
                .unwrap_or(false)
        }) {
            let file_list = ContainerFileList::read_from_file(list_path)?;
            if file_list.files.is_empty() {
                should_remove = true;
            }
        }

        let matching_position = index
            .containers
            .iter()
            .position(|entry| guid_file_name(&entry.container_uuid) == dir_name);
        if matching_position.is_none() && !should_remove {
            should_remove = true;
        }
        if should_remove {
            if let Some(position) = matching_position {
                index.containers.remove(position);
            }
            std::fs::remove_dir_all(&dir_path)?;
        }
    }
    Ok(())
}

/// Writes a modified save under a NEW save id — a new Level container plus copies of
/// every other original container (player payloads replaced, LevelMeta world name
/// rewritten) — then refreshes the index mtime and rewrites containers.index.
///
/// The old save's containers are left in place; callers run `cleanup_container_dir`
/// beforehand if they want them gone. `original_containers` must already be the
/// resolved "latest" set from `ContainerIndex::latest_save_containers`.
pub fn save_modified_gamepass(
    index: &mut ContainerIndex,
    container_dir: &Path,
    save_id: &str,
    modified_level_data: &[u8],
    player_sav_data: &HashMap<uuid::Uuid, PlayerSavBytes>,
    original_containers: &OrderedMap<String, ContainerEntry>,
    world_name: &str,
    modified_world_option: Option<&[u8]>,
) -> Result<(), CoreError> {
    let level_container =
        create_container(container_dir, save_id, modified_level_data, "Data", "Level")?;
    index.containers.push(level_container);

    for (key, original) in original_containers.iter() {
        if key == "Level" {
            continue;
        }
        let mut replacement: Option<Vec<u8>> = None;
        if key.contains("Player") && !key.contains("_dps") {
            let raw_id = key.split('-').nth(1).unwrap_or_default();
            match uuid::Uuid::parse_str(raw_id) {
                Ok(player_uuid) => {
                    replacement = player_sav_data
                        .get(&player_uuid)
                        .and_then(|player| player.sav.clone());
                }
                Err(_) => continue, // unparseable player uuid in the container name
            }
        } else if key.contains("_dps") {
            let raw_id = key
                .split('-')
                .nth(1)
                .unwrap_or_default()
                .replace("_dps", "");
            match uuid::Uuid::parse_str(&raw_id) {
                Ok(player_uuid) => {
                    let player = player_sav_data.get(&player_uuid).ok_or_else(|| {
                        CoreError::Other(format!("player {player_uuid} missing dps data"))
                    })?;
                    replacement = player.dps.clone();
                }
                Err(_) => continue,
            }
        } else if key == "WorldOption" {
            // None => copy_container passes the original bytes through untouched.
            replacement = modified_world_option.map(<[u8]>::to_vec);
        }
        let copied = copy_container(
            original,
            container_dir,
            container_dir,
            save_id,
            key,
            world_name,
            replacement.as_deref(),
        )?;
        index.containers.push(copied);
    }

    index.mtime = Filetime::now();
    index.write_to_dir(container_dir)?;
    Ok(())
}

fn delete_matching_containers(
    container_dir: &Path,
    backups_root: &Path,
    matches: impl Fn(&ContainerEntry) -> bool,
) -> Result<usize, CoreError> {
    backup_container_dir(container_dir, backups_root)?;
    let mut index = ContainerIndex::read_from_dir(container_dir)?;
    let (doomed, kept): (Vec<ContainerEntry>, Vec<ContainerEntry>) =
        index.containers.into_iter().partition(&matches);
    index.containers = kept;
    if doomed.is_empty() {
        return Ok(0);
    }
    for entry in &doomed {
        let blob_dir = container_dir.join(guid_file_name(&entry.container_uuid));
        if blob_dir.exists() {
            std::fs::remove_dir_all(&blob_dir)?;
        }
    }
    index.write_to_dir(container_dir)?;
    Ok(doomed.len())
}

pub fn delete_save_containers(
    container_dir: &Path,
    save_id: &str,
    backups_root: &Path,
) -> Result<usize, CoreError> {
    let prefix = format!("{save_id}-");
    delete_matching_containers(container_dir, backups_root, |entry| {
        entry.container_name.starts_with(&prefix)
    })
}

pub fn delete_player_containers(
    container_dir: &Path,
    save_id: &str,
    player_id: &str,
    backups_root: &Path,
) -> Result<usize, CoreError> {
    let prefix = format!("{save_id}-");
    let player_fragment = format!("Players-{player_id}");
    delete_matching_containers(container_dir, backups_root, |entry| {
        entry.container_name.starts_with(&prefix) && entry.container_name.contains(&player_fragment)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gamepass::format::ContainerFileList;
    use std::sync::Mutex;

    /// Serializes tests that mutate process-global env vars so they can't race
    /// tests reading production defaults (cargo runs tests in parallel threads).
    static ENV_GUARD: Mutex<()> = Mutex::new(());

    /// Writes a `container.<seq>` file list plus its blob at a caller-chosen seq;
    /// `create_container` itself only ever writes `container.1`.
    fn write_file_list_at_seq(blob_dir: &Path, seq: u32, name: &str, data: &[u8]) {
        std::fs::create_dir_all(blob_dir).unwrap();
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
    fn wgs_dir_name_matcher_mirrors_python_regex() {
        assert!(is_wgs_container_dir_name(
            "000900000487F3B6_0000000000000000000000006B210A9C"
        ));
        assert!(!is_wgs_container_dir_name(
            "t_0000000000000000000000006B210A9C"
        ));
        assert!(!is_wgs_container_dir_name(
            "000900000487f3b6_0000000000000000000000006B210A9C" // lowercase
        ));
        assert!(!is_wgs_container_dir_name("containers.index"));
    }

    #[test]
    fn find_container_dir_under_reports_python_error_strings() {
        let missing = std::path::Path::new("Z:/definitely/not/here");
        let error = find_container_dir_under(missing).unwrap_err();
        assert_eq!(
            error.to_string(),
            "Could not find Xbox Palworld installation"
        );

        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join("SystemAppData").join("wgs")).unwrap();
        let error = find_container_dir_under(temp.path()).unwrap_err();
        assert_eq!(
            error.to_string(),
            "Could not find container path. Try running the game once."
        );

        let wgs_leaf = temp
            .path()
            .join("SystemAppData")
            .join("wgs")
            .join("000900000487F3B6_0000000000000000000000006B210A9C");
        std::fs::create_dir_all(&wgs_leaf).unwrap();
        assert_eq!(find_container_dir_under(temp.path()).unwrap(), wgs_leaf);
    }

    #[test]
    fn create_container_writes_list_blob_and_entry_and_reads_back() {
        let temp = tempfile::tempdir().unwrap();
        let entry = create_container(temp.path(), "AAAA", b"level-bytes", "Data", "Level").unwrap();
        assert_eq!(entry.container_name, "AAAA-Level");
        assert_eq!(entry.seq, 1);
        assert_eq!(entry.flag, 5);
        assert_eq!(entry.size, 11);

        let container_dir = temp.path().join(crate::gamepass::format::guid_file_name(
            &entry.container_uuid,
        ));
        let list = ContainerFileList::read_from_file(&container_dir.join("container.1")).unwrap();
        assert_eq!(list.seq, 1);
        assert_eq!(list.files.len(), 1);
        assert_eq!(list.files[0].name, "Data");
        assert_eq!(list.files[0].data, b"level-bytes");

        let blob = read_first_blob(temp.path(), &entry).unwrap();
        assert_eq!(blob, Some((1, b"level-bytes".to_vec())));
    }

    #[test]
    fn read_first_blob_returns_none_for_missing_dir() {
        let temp = tempfile::tempdir().unwrap();
        let entry = create_container(temp.path(), "AAAA", b"x", "Data", "Level").unwrap();
        let ghost = crate::gamepass::format::ContainerEntry {
            container_uuid: uuid::Uuid::new_v4(),
            ..entry
        };
        assert_eq!(read_first_blob(temp.path(), &ghost).unwrap(), None);
    }

    #[test]
    fn backup_copies_whole_tree() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp
            .path()
            .join("000900000487F3B6_0000000000000000000000006B210A9C");
        std::fs::create_dir_all(source.join("SUB")).unwrap();
        std::fs::write(source.join("containers.index"), b"idx").unwrap();
        std::fs::write(source.join("SUB").join("blob"), b"data").unwrap();

        let backups = temp.path().join("backups").join("gamepass");
        let backup_path = backup_container_dir(&source, &backups).unwrap();
        assert!(backup_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("000900000487F3B6_0000000000000000000000006B210A9C_"));
        assert_eq!(
            std::fs::read(backup_path.join("containers.index")).unwrap(),
            b"idx"
        );
        assert_eq!(
            std::fs::read(backup_path.join("SUB").join("blob")).unwrap(),
            b"data"
        );
        assert!(source.join("containers.index").exists()); // source untouched
    }

    #[test]
    fn read_first_blob_picks_numeric_latest_seq_not_lexicographic() {
        // container.10 is the true latest but sorts BEFORE container.2 as a string,
        // so a lexicographic visit order would keep .2's stale blob.
        let temp = tempfile::tempdir().unwrap();
        let entry = create_container(temp.path(), "AAAA", b"seq1", "Data", "Level").unwrap();
        let blob_dir = temp.path().join(crate::gamepass::format::guid_file_name(
            &entry.container_uuid,
        ));
        write_file_list_at_seq(&blob_dir, 2, "Data", b"seq2-blob");
        write_file_list_at_seq(&blob_dir, 10, "Data", b"seq10-blob");

        let blob = read_first_blob(temp.path(), &entry).unwrap();
        assert_eq!(blob, Some((10, b"seq10-blob".to_vec())));
    }

    #[test]
    fn backups_root_honors_env_override() {
        let _guard = ENV_GUARD.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let override_path = temp.path().join("custom-backups");
        std::env::set_var("PSP_BACKUPS_ROOT", &override_path);
        let resolved = backups_root();
        std::env::remove_var("PSP_BACKUPS_ROOT");
        assert_eq!(resolved, override_path);
    }

    #[test]
    fn find_container_dir_honors_packages_root_env_override() {
        let _guard = ENV_GUARD.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let wgs_leaf = temp
            .path()
            .join("SystemAppData")
            .join("wgs")
            .join("000900000487F3B6_0000000000000000000000006B210A9C");
        std::fs::create_dir_all(&wgs_leaf).unwrap();

        std::env::set_var("PSP_GAMEPASS_PACKAGES_ROOT", temp.path());
        let resolved = find_container_dir();
        std::env::remove_var("PSP_GAMEPASS_PACKAGES_ROOT");
        assert_eq!(resolved.unwrap(), wgs_leaf);
    }

    #[test]
    fn clean_container_file_name_strips_slot_and_trailing_counter() {
        assert_eq!(
            clean_container_file_name("AAAA-Slot1-Players-0123"),
            "AAAA-Players-0123"
        );
        assert_eq!(clean_container_file_name("AAAA-Level-01"), "AAAA-Level");
        assert_eq!(
            clean_container_file_name("AAAA-Level-123"),
            "AAAA-Level-123"
        ); // 3 digits: no match
        assert_eq!(clean_container_file_name("AAAA-Level"), "AAAA-Level");
        assert_eq!(clean_container_file_name("AAAA-Slot-Level"), "AAAA-Level"); // zero digits still matches
    }

    #[test]
    fn copy_container_renames_and_replaces_player_data() {
        let temp = tempfile::tempdir().unwrap();
        let source_entry =
            create_container(temp.path(), "OLDID", b"player-old", "Data", "Players-ABCD").unwrap();
        let copied = copy_container(
            &source_entry,
            temp.path(),
            temp.path(),
            "NEWID",
            "Players-ABCD",
            "Ignored World",
            Some(b"player-new"),
        )
        .unwrap();
        assert_eq!(copied.container_name, "NEWID-Players-ABCD");
        assert_ne!(copied.container_uuid, source_entry.container_uuid);
        assert_eq!(copied.size, b"player-new".len() as u64);
        let (_, blob) = read_first_blob(temp.path(), &copied).unwrap().unwrap();
        assert_eq!(blob, b"player-new");
        // Source blob untouched.
        let (_, source_blob) = read_first_blob(temp.path(), &source_entry)
            .unwrap()
            .unwrap();
        assert_eq!(source_blob, b"player-old");
    }

    #[test]
    fn cleanup_removes_orphaned_and_empty_container_dirs() {
        let temp = tempfile::tempdir().unwrap();
        let keep = create_container(temp.path(), "AAAA", b"data", "Data", "Level").unwrap();
        let orphan = create_container(temp.path(), "AAAA", b"ghost", "Data", "LevelMeta").unwrap();
        let empty_dir = temp.path().join("00000000000000000000000000000000");
        std::fs::create_dir_all(&empty_dir).unwrap();

        let mut index = crate::gamepass::format::ContainerIndex {
            flag1: 0,
            package_name: String::new(),
            mtime: crate::gamepass::format::Filetime(0),
            flag2: 0,
            index_uuid: String::new(),
            unknown: 0,
            containers: vec![keep.clone()], // orphan is on disk but NOT in the index
        };
        cleanup_container_dir(&mut index, temp.path()).unwrap();

        assert_eq!(index.containers.len(), 1);
        assert!(temp
            .path()
            .join(crate::gamepass::format::guid_file_name(
                &keep.container_uuid
            ))
            .exists());
        assert!(!temp
            .path()
            .join(crate::gamepass::format::guid_file_name(
                &orphan.container_uuid
            ))
            .exists());
        assert!(!empty_dir.exists());
    }

    #[test]
    fn save_modified_gamepass_creates_new_containers_and_rewrites_index() {
        use crate::gamepass::PlayerSavBytes;
        let testdata = crate::gamepass::fixture::reference_saves_dir();
        let meta_bytes = std::fs::read(testdata.join("LevelMeta.sav")).unwrap();

        let temp = tempfile::tempdir().unwrap();
        let player_id = uuid::Uuid::new_v4();
        let player_hex = player_id.as_simple().to_string().to_uppercase();
        let save = crate::gamepass::fixture::SyntheticSave {
            save_id: "OLDID000OLDID000OLDID000OLDID000".to_string(),
            level_sav: b"OLD-LEVEL".to_vec(),
            level_meta: Some(meta_bytes),
            local_data: None,
            world_option: None,
            players: vec![crate::gamepass::fixture::SyntheticPlayer {
                id: player_id,
                sav: b"OLD-PLAYER".to_vec(),
                dps: None,
            }],
        };
        let container_dir = crate::gamepass::fixture::build_wgs_tree(temp.path(), &[save]).unwrap();

        let mut index =
            crate::gamepass::format::ContainerIndex::read_from_dir(&container_dir).unwrap();
        let originals = index.latest_save_containers("OLDID000OLDID000OLDID000OLDID000");
        let mut player_data = std::collections::HashMap::new();
        player_data.insert(
            player_id,
            PlayerSavBytes {
                sav: Some(b"NEW-PLAYER".to_vec()),
                dps: None,
            },
        );

        save_modified_gamepass(
            &mut index,
            &container_dir,
            "NEWID000NEWID000NEWID000NEWID000",
            b"NEW-LEVEL",
            &player_data,
            &originals,
            "Renamed World",
            None,
        )
        .unwrap();

        let reloaded =
            crate::gamepass::format::ContainerIndex::read_from_dir(&container_dir).unwrap();
        let new_latest = reloaded.latest_save_containers("NEWID000NEWID000NEWID000NEWID000");
        assert_eq!(new_latest.len(), 3); // Level, LevelMeta, player

        let (_, level_blob) = read_first_blob(&container_dir, new_latest.get("Level").unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(level_blob, b"NEW-LEVEL");

        let (_, player_blob) = read_first_blob(
            &container_dir,
            new_latest.get(&format!("Players-{player_hex}")).unwrap(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(player_blob, b"NEW-PLAYER");

        let (_, meta_blob) = read_first_blob(&container_dir, new_latest.get("LevelMeta").unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(
            crate::gamepass::scan::world_name_from_level_meta(&meta_blob).unwrap(),
            "Renamed World"
        );
        // Old containers remain: cleanup is a separate pass.
        assert!(!reloaded
            .latest_save_containers("OLDID000OLDID000OLDID000OLDID000")
            .is_empty());
    }

    #[test]
    fn delete_save_containers_removes_all_and_reports_count() {
        let temp = tempfile::tempdir().unwrap();
        let player_id = uuid::Uuid::new_v4();
        let doomed = crate::gamepass::fixture::SyntheticSave {
            save_id: "DEAD0000DEAD0000DEAD0000DEAD0000".to_string(),
            level_sav: b"L".to_vec(),
            level_meta: Some(b"M".to_vec()),
            local_data: None,
            world_option: None,
            players: vec![crate::gamepass::fixture::SyntheticPlayer {
                id: player_id,
                sav: b"P".to_vec(),
                dps: Some(b"D".to_vec()),
            }],
        };
        let survivor = crate::gamepass::fixture::SyntheticSave {
            save_id: "A11FE000A11FE000A11FE000A11FE000".to_string(),
            level_sav: b"L2".to_vec(),
            level_meta: Some(b"M2".to_vec()),
            local_data: None,
            world_option: None,
            players: vec![],
        };
        let container_dir =
            crate::gamepass::fixture::build_wgs_tree(temp.path(), &[doomed, survivor]).unwrap();
        let backups = temp.path().join("backups");

        let removed =
            delete_save_containers(&container_dir, "DEAD0000DEAD0000DEAD0000DEAD0000", &backups)
                .unwrap();
        assert_eq!(removed, 4);

        let index = crate::gamepass::format::ContainerIndex::read_from_dir(&container_dir).unwrap();
        assert_eq!(index.containers.len(), 2); // survivor's Level + LevelMeta
        assert!(index
            .containers
            .iter()
            .all(|entry| entry.container_name.starts_with("A11FE000")));
        assert!(backups.read_dir().unwrap().next().is_some()); // backup was taken

        // Nothing matched: 0, no error.
        let removed =
            delete_save_containers(&container_dir, "0000000000000000000000000000BEEF", &backups)
                .unwrap();
        assert_eq!(removed, 0);
    }

    #[test]
    fn save_modified_gamepass_substitutes_world_option_bytes() {
        let temp = tempfile::tempdir().unwrap();
        let save_id = "0123456789ABCDEF0123456789ABCDEF";
        let synthetic = crate::gamepass::fixture::SyntheticSave {
            save_id: save_id.to_string(),
            level_sav: b"LEVEL".to_vec(),
            // None, not fake bytes: save_modified_gamepass runs every non-Level
            // container through copy_container, and a LevelMeta container is
            // rewritten via a real GVAS parse (set_world_name_in_level_meta) that
            // fake bytes can't survive. Omitting it keeps this test focused on
            // WorldOption substitution.
            level_meta: None,
            local_data: None,
            world_option: Some(b"OLD_WORLD_OPTION".to_vec()),
            players: vec![],
        };
        let container_dir =
            crate::gamepass::fixture::build_wgs_tree(temp.path(), &[synthetic]).unwrap();
        let mut index = ContainerIndex::read_from_dir(&container_dir).unwrap();
        let originals = index.latest_save_containers(save_id);

        save_modified_gamepass(
            &mut index,
            &container_dir,
            save_id,
            b"NEW_LEVEL",
            &HashMap::new(),
            &originals,
            "MyWorld",
            Some(b"NEW_WORLD_OPTION"),
        )
        .unwrap();

        let latest = index.latest_save_containers(save_id);
        let entry = latest.get("WorldOption").unwrap();
        let (_seq, blob) = read_first_blob(&container_dir, entry).unwrap().unwrap();
        assert_eq!(blob, b"NEW_WORLD_OPTION");
    }

    #[test]
    fn save_modified_gamepass_passes_world_option_through_when_not_modified() {
        let temp = tempfile::tempdir().unwrap();
        let save_id = "0123456789ABCDEF0123456789ABCDEF";
        let synthetic = crate::gamepass::fixture::SyntheticSave {
            save_id: save_id.to_string(),
            level_sav: b"LEVEL".to_vec(),
            // See the comment in the substitution test above: fake LevelMeta
            // bytes can't survive save_modified_gamepass's real GVAS rewrite.
            level_meta: None,
            local_data: None,
            world_option: Some(b"ORIGINAL".to_vec()),
            players: vec![],
        };
        let container_dir =
            crate::gamepass::fixture::build_wgs_tree(temp.path(), &[synthetic]).unwrap();
        let mut index = ContainerIndex::read_from_dir(&container_dir).unwrap();
        let originals = index.latest_save_containers(save_id);

        save_modified_gamepass(
            &mut index,
            &container_dir,
            save_id,
            b"NEW_LEVEL",
            &HashMap::new(),
            &originals,
            "MyWorld",
            None,
        )
        .unwrap();

        let latest = index.latest_save_containers(save_id);
        let entry = latest.get("WorldOption").unwrap();
        let (_seq, blob) = read_first_blob(&container_dir, entry).unwrap().unwrap();
        assert_eq!(
            blob, b"ORIGINAL",
            "an unmodified WorldOption must survive untouched"
        );
    }

    #[test]
    fn delete_player_containers_removes_sav_and_dps_only() {
        let temp = tempfile::tempdir().unwrap();
        let victim = uuid::Uuid::new_v4();
        let bystander = uuid::Uuid::new_v4();
        let save = crate::gamepass::fixture::SyntheticSave {
            save_id: "CAFE0000CAFE0000CAFE0000CAFE0000".to_string(),
            level_sav: b"L".to_vec(),
            level_meta: Some(b"M".to_vec()),
            local_data: None,
            world_option: None,
            players: vec![
                crate::gamepass::fixture::SyntheticPlayer {
                    id: victim,
                    sav: b"P1".to_vec(),
                    dps: Some(b"D1".to_vec()),
                },
                crate::gamepass::fixture::SyntheticPlayer {
                    id: bystander,
                    sav: b"P2".to_vec(),
                    dps: None,
                },
            ],
        };
        let container_dir = crate::gamepass::fixture::build_wgs_tree(temp.path(), &[save]).unwrap();
        let backups = temp.path().join("backups");

        let victim_hex = victim.as_simple().to_string().to_uppercase();
        let removed = delete_player_containers(
            &container_dir,
            "CAFE0000CAFE0000CAFE0000CAFE0000",
            &victim_hex,
            &backups,
        )
        .unwrap();
        assert_eq!(removed, 2); // sav + dps

        let index = crate::gamepass::format::ContainerIndex::read_from_dir(&container_dir).unwrap();
        assert_eq!(index.containers.len(), 3); // Level, LevelMeta, bystander
        assert!(!index
            .containers
            .iter()
            .any(|entry| entry.container_name.contains(&victim_hex)));
    }
}
