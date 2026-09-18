use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

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
    pub world_key: String,
    pub mod_profile: Option<SaveModProfile>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SaveModProfile {
    pub profile_id: String,
    pub profile_name: String,
    pub target_id: String,
}

pub fn world_key_of(save_dir: &Path) -> String {
    ps_core::mods::native_separators(&save_dir.to_string_lossy(), cfg!(windows))
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
                world_key: world_key_of(dir),
                mod_profile: None,
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

#[derive(Debug, Default, serde::Deserialize)]
pub struct ListLocalSavesData {
    #[serde(default)]
    pub include_gamepass: bool,
}

pub async fn handle_list_local_saves(
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if !ctx.app.config.desktop_mode {
        refuse(
            ctx.emitter,
            MessageType::ListLocalSaves,
            "Desktop mode is required to list local saves",
        );
        return Ok(());
    }
    // `null`/absent means "no payload", and the derived Deserialize rejects
    // null outright, so it needs its own branch rather than a default field.
    let ListLocalSavesData { include_gamepass } = if data.is_null() {
        ListLocalSavesData::default()
    } else {
        serde_json::from_value(data)?
    };
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
    let mut saves = scan_save_roots(&roots);
    let db = &*ctx.app.driver;
    for entry in &mut saves {
        entry.mod_profile = mod_profile_for_world(db, &entry.world_key).await;
    }
    if include_gamepass {
        saves.extend(gamepass_save_entries(db).await);
    }
    ctx.emitter.emit(
        MessageType::ListLocalSaves,
        &serde_json::json!({ "saves": saves }),
    );
    Ok(())
}

/// A machine without Game Pass installed is the normal case: no container,
/// no entries, no log. A scan failure on an existing container is unusual
/// enough to log once, but still yields no entries rather than a refusal.
async fn gamepass_save_entries(db: &dyn ps_db::DbDriver) -> Vec<LocalSaveEntry> {
    let Ok(container_dir) = ps_core::gamepass::store::find_container_dir() else {
        return Vec::new();
    };
    let saves = match ps_core::gamepass::scan::scan_saves(&container_dir) {
        Ok(saves) => saves,
        Err(error) => {
            tracing::warn!(%error, "failed to scan gamepass saves for list_local_saves");
            return Vec::new();
        }
    };
    let path = ps_core::mods::native_separators(&container_dir.to_string_lossy(), cfg!(windows));
    let mut entries = Vec::new();
    for (save_id, save_data) in saves.iter() {
        let world_key = format!("gamepass:{save_id}");
        let mod_profile = mod_profile_for_world(db, &world_key).await;
        entries.push(LocalSaveEntry {
            path: path.clone(),
            name: save_data.world_name.clone(),
            save_type: "gamepass",
            modified_ms: (save_data.last_modified * 1000.0) as u64,
            world_key,
            mod_profile,
        });
    }
    entries
}

async fn mod_profile_for_world(
    db: &dyn ps_db::DbDriver,
    world_key: &str,
) -> Option<SaveModProfile> {
    let link = match ps_db::mod_profiles::world_link(db, world_key).await {
        Ok(link) => link?,
        Err(error) => {
            tracing::warn!(%error, %world_key, "failed to look up world link");
            return None;
        }
    };
    match ps_db::mod_profiles::get(db, &link.profile_id).await {
        Ok(Some(profile)) => Some(SaveModProfile {
            profile_id: profile.id,
            profile_name: profile.name,
            target_id: profile.target_id,
        }),
        Ok(None) => None,
        Err(error) => {
            tracing::warn!(%error, profile_id = %link.profile_id, "failed to load linked profile");
            None
        }
    }
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
        assert_eq!(
            entries[0].world_key,
            ps_core::mods::native_separators(&with_save.to_string_lossy(), cfg!(windows))
        );
        assert_eq!(entries[0].mod_profile, None);
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
        assert_eq!(
            entries[0].world_key,
            ps_core::mods::native_separators(&world_dir.to_string_lossy(), cfg!(windows))
        );
        assert_eq!(entries[0].mod_profile, None);
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
        std::sync::Arc::get_mut(&mut test.app).unwrap().config.desktop_mode = true;
        let mut ctx = HandlerCtx {
            session: &mut test.session,
            app: &test.app,
            emitter: &test.emitter,
            blueprints: &mut test.blueprints,
            is_loopback: false,
            mod_verification_subscribed: None,
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
            mod_verification_subscribed: None,
            attachment: None,
        };

        handle_browse_directory(BrowseDirectoryData { path: None }, &mut ctx)
            .await
            .unwrap();

        let frame = test.next_frame_json();
        assert_eq!(frame["type"], "browse_directory");
        let error = frame["data"]["error"].as_str().expect("refusal carries data.error");
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
            mod_verification_subscribed: None,
            attachment: None,
        };

        handle_list_local_saves(Value::Null, &mut ctx).await.unwrap();

        let frame = test.next_frame_json();
        assert_eq!(frame["type"], "list_local_saves");
        let error = frame["data"]["error"].as_str().expect("refusal carries data.error");
        assert!(error.to_lowercase().contains("desktop mode"));
        test.assert_no_more_frames();
    }
}
