//! Filesystem operations over a wgs container directory.
//! Port of palworld_save_pal/utils/gamepass/container_utils.py.

use std::path::{Path, PathBuf};

use crate::error::CoreError;
use crate::gamepass::format::{
    guid_file_name, write_utf16_fixed_64, ContainerEntry, ContainerFileList, Filetime,
    CONTAINER_FILE_LIST_VERSION,
};

/// Python: re.compile(r"[0-9A-F]{16}_[0-9A-F]{32}$")
pub fn is_wgs_container_dir_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() == 49
        && bytes[16] == b'_'
        && bytes[..16]
            .iter()
            .chain(&bytes[17..])
            .all(|c| c.is_ascii_digit() || (b'A'..=b'F').contains(c))
}

/// The Packages dir of the Xbox Palworld install. Env hook PSP_GAMEPASS_PACKAGES_ROOT
/// lets tests point at a synthetic tree (inert in production).
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

/// Port of find_container_path (container_utils.py:29-37). Error strings must match.
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

/// Cwd-relative "backups" like the Python app, overridable for tests.
pub fn backups_root() -> PathBuf {
    std::env::var("PSP_BACKUPS_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("backups"))
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

/// Port of backup_container_path (container_utils.py:40-50).
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

/// Reads the payload for a container entry: iterates `container.*` files in name order
/// and returns the first blob of the last non-empty file list, with that list's seq.
/// (Python reads every `container.*` file and keeps the last `files[0].data`;
/// select_gamepass_save additionally records the seq — local_file_handler.py:277-285.)
pub fn read_first_blob(
    container_dir: &Path,
    entry: &ContainerEntry,
) -> Result<Option<(u32, Vec<u8>)>, CoreError> {
    let blob_dir = container_dir.join(guid_file_name(&entry.container_uuid));
    if !blob_dir.exists() {
        return Ok(None);
    }
    let mut list_paths: Vec<PathBuf> = std::fs::read_dir(&blob_dir)?
        .flatten()
        .map(|dir_entry| dir_entry.path())
        .filter(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().starts_with("container."))
                .unwrap_or(false)
        })
        .collect();
    list_paths.sort();
    let mut newest: Option<(u32, Vec<u8>)> = None;
    for list_path in list_paths {
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

/// Port of create_new_container (container_utils.py:121-173): fresh container GUID dir,
/// container.1 with a single file, blob beside it; returns the new index entry
/// (seq=1, flag=5, mtime=now). NOT appended to any index — callers do that.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gamepass::format::ContainerFileList;

    #[test]
    fn wgs_dir_name_matcher_mirrors_python_regex() {
        // Python: re.compile(r"[0-9A-F]{16}_[0-9A-F]{32}$")
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

    /// Reads a real wgs `container.<seq>` file list and its blobs from the gamepass
    /// backup corpus, when present. Strong validation the fixed-64 name codec and blob
    /// naming match the real Xbox on-disk format, not just a synthetic round trip.
    /// Skipped (not failed) when the corpus isn't checked out.
    #[test]
    fn reads_real_container_file_list_and_blobs_from_corpus_when_present() {
        let container_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../backups/gamepass/000900000487F3B6_0000000000000000000000006B210A9C_20260328021933",
        );
        if !container_dir.exists() {
            eprintln!(
                "skipping reads_real_container_file_list_and_blobs_from_corpus_when_present: {} not found",
                container_dir.display()
            );
            return;
        }
        // Find one blob subdir under the container dir that has a container.<seq> file.
        let blob_dir = std::fs::read_dir(&container_dir)
            .unwrap()
            .flatten()
            .map(|dir_entry| dir_entry.path())
            .find(|path| path.is_dir() && path.join("container.1").exists())
            .expect("expected at least one container subdir with a container.1 file list");

        let list = ContainerFileList::read_from_file(&blob_dir.join("container.1")).unwrap();
        assert_eq!(list.seq, 1);
        assert!(
            !list.files.is_empty(),
            "expected at least one file entry in the real container.1 file list"
        );
        assert!(
            !list.files[0].data.is_empty(),
            "expected non-empty blob data for the real container's first file"
        );
    }
}
