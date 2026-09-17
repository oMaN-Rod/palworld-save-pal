use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;
use crate::signal_handlers::refuse;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct LocalSaveEntry {
    pub path: String,
    pub name: String,
    pub save_type: &'static str,
    pub modified_ms: u64,
}

fn modified_ms(path: &Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn scan_dir(
    dir: &Path,
    remaining_depth: u32,
    seen: &mut HashSet<PathBuf>,
    results: &mut Vec<LocalSaveEntry>,
) {
    if !dir.is_dir() {
        return;
    }
    let level_sav = dir.join("Level.sav");
    if level_sav.is_file() {
        let canonical = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
        if seen.insert(canonical) {
            results.push(LocalSaveEntry {
                path: level_sav.to_string_lossy().into_owned(),
                name: dir
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                save_type: "steam",
                modified_ms: modified_ms(&level_sav),
            });
        }
    }
    if remaining_depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            scan_dir(&path, remaining_depth - 1, seen, results);
        }
    }
}

pub fn scan_save_roots(roots: &[PathBuf]) -> Vec<LocalSaveEntry> {
    let mut seen = HashSet::new();
    let mut results = Vec::new();
    for root in roots {
        scan_dir(root, 2, &mut seen, &mut results);
    }
    results
}

pub async fn handle_list_local_saves(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    if !ctx.app.config.desktop_mode {
        refuse(
            ctx.emitter,
            MessageType::ListLocalSaves,
            "Desktop mode is required to list local saves",
        );
        return Ok(());
    }
    let mut roots = Vec::new();
    let default_root = ps_db::settings::default_steam_save_dir();
    if !default_root.is_empty() {
        roots.push(PathBuf::from(default_root));
    }
    if let Some(saved_dir) = ps_db::settings::saved_save_dir(&*ctx.app.driver).await? {
        if !saved_dir.is_empty() {
            roots.push(PathBuf::from(saved_dir));
        }
    }
    let saves = scan_save_roots(&roots);
    ctx.emitter.emit(
        MessageType::ListLocalSaves,
        &serde_json::json!({ "saves": saves }),
    );
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct BrowseDirectoryData {
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct DirEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

#[cfg(target_os = "windows")]
fn filesystem_roots() -> Vec<DirEntry> {
    (b'A'..=b'Z')
        .map(|letter| format!("{}:\\", letter as char))
        .filter(|drive| Path::new(drive).exists())
        .map(|drive| DirEntry {
            name: drive.trim_end_matches('\\').to_string(),
            path: drive,
            is_dir: true,
        })
        .collect()
}

#[cfg(not(target_os = "windows"))]
fn filesystem_roots() -> Vec<DirEntry> {
    vec![DirEntry {
        name: "/".to_string(),
        path: "/".to_string(),
        is_dir: true,
    }]
}

pub fn browse_directory(path: Option<&str>) -> Result<(String, Vec<DirEntry>), HandlerError> {
    let Some(path) = path.filter(|path| !path.is_empty()) else {
        return Ok((String::new(), filesystem_roots()));
    };

    let read_dir = std::fs::read_dir(path)
        .map_err(|_| HandlerError::Other(format!("Cannot read directory: {path}")))?;
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    for entry in read_dir.filter_map(Result::ok) {
        let entry_path = entry.path();
        let is_dir = entry_path.is_dir();
        let dir_entry = DirEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            path: entry_path.to_string_lossy().into_owned(),
            is_dir,
        };
        if is_dir {
            dirs.push(dir_entry);
        } else {
            files.push(dir_entry);
        }
    }
    dirs.sort_by(|a, b| a.name.cmp(&b.name));
    files.sort_by(|a, b| a.name.cmp(&b.name));
    dirs.extend(files);
    Ok((path.to_string(), dirs))
}

pub async fn handle_browse_directory(
    data: BrowseDirectoryData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if !ctx.app.config.desktop_mode {
        refuse(
            ctx.emitter,
            MessageType::BrowseDirectory,
            "Desktop mode is required to browse the filesystem",
        );
        return Ok(());
    }
    match browse_directory(data.path.as_deref()) {
        Ok((path, entries)) => {
            ctx.emitter.emit(
                MessageType::BrowseDirectory,
                &serde_json::json!({ "path": path, "entries": entries }),
            );
        }
        Err(error) => {
            ctx.emitter.emit(
                MessageType::BrowseDirectory,
                &serde_json::json!({ "error": error.to_string() }),
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_save_dirs_containing_level_sav_and_skips_others() {
        let root = tempfile::tempdir().unwrap();
        let with_save = root.path().join("world_a");
        let without_save = root.path().join("world_b");
        std::fs::create_dir_all(&with_save).unwrap();
        std::fs::create_dir_all(&without_save).unwrap();
        std::fs::write(with_save.join("Level.sav"), b"data").unwrap();

        let entries = scan_save_roots(&[root.path().to_path_buf()]);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "world_a");
        assert_eq!(entries[0].save_type, "steam");
        assert_eq!(
            entries[0].path,
            with_save.join("Level.sav").to_string_lossy()
        );
    }

    #[test]
    fn merges_the_settings_dir_without_duplicates() {
        let steam_root = tempfile::tempdir().unwrap();
        let world_dir = steam_root.path().join("76500123").join("world1");
        std::fs::create_dir_all(&world_dir).unwrap();
        std::fs::write(world_dir.join("Level.sav"), b"data").unwrap();

        let entries = scan_save_roots(&[steam_root.path().to_path_buf(), world_dir.clone()]);

        assert_eq!(
            entries.len(),
            1,
            "the same save dir reached via both roots must appear once"
        );
        assert_eq!(entries[0].name, "world1");
    }

    #[test]
    fn browse_lists_directories_first_then_files_sorted() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("b_dir")).unwrap();
        std::fs::create_dir_all(root.path().join("a_dir")).unwrap();
        std::fs::write(root.path().join("z.txt"), b"z").unwrap();
        std::fs::write(root.path().join("a.txt"), b"a").unwrap();

        let (path, entries) = browse_directory(Some(&root.path().to_string_lossy())).unwrap();

        assert_eq!(path, root.path().to_string_lossy());
        let names: Vec<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
        assert_eq!(names, vec!["a_dir", "b_dir", "a.txt", "z.txt"]);
        assert!(entries[0].is_dir && entries[1].is_dir);
        assert!(!entries[2].is_dir && !entries[3].is_dir);
    }

    #[test]
    fn pure_browse_of_a_missing_path_is_an_error() {
        let missing = tempfile::tempdir().unwrap().path().join("does_not_exist");
        let result = browse_directory(Some(&missing.to_string_lossy()));
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn browse_of_a_missing_path_replies_with_an_inline_error() {
        let mut test = ps_app::test_support::TestContext::new(|_| {}).await;
        std::sync::Arc::get_mut(&mut test.app)
            .unwrap()
            .config
            .desktop_mode = true;
        let mut ctx = HandlerCtx {
            session: &mut test.session,
            app: &test.app,
            emitter: &test.emitter,
            blueprints: &mut test.blueprints,
            is_loopback: false,
            write_allowed: true,
            attachment: None,
        };
        let missing = tempfile::tempdir().unwrap().path().join("does_not_exist");

        handle_browse_directory(
            BrowseDirectoryData {
                path: Some(missing.to_string_lossy().into_owned()),
            },
            &mut ctx,
        )
        .await
        .unwrap();

        let frame = test.next_frame_json();
        assert_eq!(frame["type"], "browse_directory");
        assert!(frame["data"]["error"].is_string());
        test.assert_no_more_frames();
    }

    #[tokio::test]
    async fn browse_directory_refuses_without_desktop_mode() {
        let mut test = ps_app::test_support::TestContext::new(|_| {}).await;
        assert!(!test.app.config.desktop_mode);
        let mut ctx = HandlerCtx {
            session: &mut test.session,
            app: &test.app,
            emitter: &test.emitter,
            blueprints: &mut test.blueprints,
            is_loopback: false,
            write_allowed: true,
            attachment: None,
        };

        handle_browse_directory(BrowseDirectoryData { path: None }, &mut ctx)
            .await
            .unwrap();

        let frame = test.next_frame_json();
        assert_eq!(frame["type"], "browse_directory");
        let error = frame["data"]["error"]
            .as_str()
            .expect("refusal carries data.error");
        assert!(error.to_lowercase().contains("desktop mode"));
        test.assert_no_more_frames();
    }

    #[tokio::test]
    async fn list_local_saves_refuses_without_desktop_mode() {
        let mut test = ps_app::test_support::TestContext::new(|_| {}).await;
        assert!(!test.app.config.desktop_mode);
        let mut ctx = HandlerCtx {
            session: &mut test.session,
            app: &test.app,
            emitter: &test.emitter,
            blueprints: &mut test.blueprints,
            is_loopback: false,
            write_allowed: true,
            attachment: None,
        };

        handle_list_local_saves(&mut ctx).await.unwrap();

        let frame = test.next_frame_json();
        assert_eq!(frame["type"], "list_local_saves");
        let error = frame["data"]["error"]
            .as_str()
            .expect("refusal carries data.error");
        assert!(error.to_lowercase().contains("desktop mode"));
        test.assert_no_more_frames();
    }
}
