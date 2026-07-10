//! select_save / load_zip_file — ports of
//! palworld_save_pal/ws/handlers/local_file_handler.py::process_steam_save
//! and palworld_save_pal/ws/handlers/save_file_handler.py::load_zip_file_handler.
//!
//! `select_save`'s desktop-native-file-dialog / "no file selected" branch
//! (desktop.py:93-117) is NOT part of this handler in Python either: it
//! lives entirely in `desktop.py`'s own separate `/ws/{client_id}` endpoint,
//! which intercepts a `select_save` message, drives a native OS file picker,
//! and only forwards the (path-rewritten) message into the normal dispatcher
//! when a file was actually chosen -- otherwise it responds
//! `no_file_selected` itself and never calls `select_save_files_handler` at
//! all. That whole layer is Phase 5 scope (ServerConfig::desktop_mode's own
//! doc comment: "Swaps select_save/open_folder behavior in Phase 5"), so
//! `handle_select_save` below is deliberately ignorant of desktop mode.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use psp_core::error::CoreError;
use psp_core::session::{PlayerFileData, SaveKind, SaveSession};
use uuid::Uuid;

use crate::dispatcher::HandlerCtx;
use crate::emitter::Emitter;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;

#[derive(Debug, serde::Deserialize)]
pub struct SelectSaveData {
    pub r#type: String,
    pub path: String,
    /// Wire-compat only: Python threads `local` through to `AppState.local`,
    /// which only affects the write path (later phases). Unused here.
    #[allow(dead_code)]
    pub local: bool,
}

#[derive(Debug, serde::Serialize)]
struct LoadedSaveFilesData {
    level: String,
    players: Vec<String>,
    world_name: String,
    r#type: &'static str,
    size: u64,
    has_gps: bool,
}

/// Shared by select_save, load_zip_file and sync_app_state's save branch:
/// get_player_summaries then get_guild_summaries, in that order, every time
/// a load completes.
pub(crate) fn emit_summary_messages(session: &SaveSession, emitter: &Emitter) {
    emitter.emit(MessageType::GetPlayerSummaries, &session.player_summaries);
    emitter.emit(MessageType::GetGuildSummaries, &session.guild_summaries);
}

/// A `Players/*.sav` or `Players/*_dps.sav` file stem, split into its player
/// id and whether it's the "_dps" companion file. Shared by the Steam
/// directory scan (`discover_player_file_refs`) and the zip-upload scan
/// (`handle_load_zip_file`) -- both mirror the same Python logic
/// (`"_dps" in player_id`, `uuid.UUID(player_id)`) in
/// `FileManager.get_player_save_paths` and `load_zip_file_handler`
/// respectively. Returns `None` for anything that doesn't parse as a UUID
/// once "_dps" is stripped, matching Python's blanket `except: continue` /
/// `except ValueError: continue`.
fn parse_player_file_stem(stem: &str) -> Option<(Uuid, bool)> {
    let is_dps = stem.contains("_dps");
    stem.replace("_dps", "")
        .parse::<Uuid>()
        .ok()
        .map(|uid| (uid, is_dps))
}

#[derive(Debug)]
struct SteamSaveLayout {
    level_sav: PathBuf,
    level_meta: Option<PathBuf>,
    players_dir: PathBuf,
    global_pal_storage_sav: Option<PathBuf>,
}

/// Port of `FileManager.validate_steam_save_directory` -- error strings and
/// check order are wire-visible and must match exactly.
fn validate_steam_save_directory(save_path: &str) -> Result<SteamSaveLayout, HandlerError> {
    let save_dir = Path::new(save_path)
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .to_path_buf();

    let level_sav = save_dir.join("Level.sav");
    if !level_sav.exists() {
        return Err(HandlerError::Other(
            "Level.sav file not found in the selected directory.".to_string(),
        ));
    }

    let players_dir = save_dir.join("Players");
    if !players_dir.is_dir() {
        return Err(HandlerError::Other(
            "Players directory not found in the selected directory.".to_string(),
        ));
    }

    let level_meta_path = save_dir.join("LevelMeta.sav");
    let level_meta = level_meta_path.exists().then_some(level_meta_path);

    let has_player_sav = std::fs::read_dir(&players_dir)
        .map_err(CoreError::Io)?
        .filter_map(|dir_entry| dir_entry.ok())
        .any(|dir_entry| {
            dir_entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "sav")
        });
    if !has_player_sav {
        return Err(HandlerError::Other(
            "No player save files found in the Players directory.".to_string(),
        ));
    }

    let global_pal_storage_sav = save_dir
        .parent()
        .map(|parent_dir| parent_dir.join("GlobalPalStorage.sav"))
        .filter(|gps_path| gps_path.exists());

    Ok(SteamSaveLayout {
        level_sav,
        level_meta,
        players_dir,
        global_pal_storage_sav,
    })
}

/// Port of `FileManager.get_player_save_paths`: every `Players/*.sav`, its
/// "_dps" companion folded into the same map entry, invalid names skipped
/// (logged, matching Python's blanket `except:` continue). The output is a
/// `BTreeMap`, so it always iterates in UUID order regardless of filesystem
/// enumeration order -- unlike Python's dict (which preserves `glob()`
/// encounter order), this is a deliberate, deterministic simplification
/// already baked into `SaveSession::player_file_refs`'s type (Task 7).
fn discover_player_file_refs(
    players_dir: &Path,
) -> Result<BTreeMap<Uuid, PlayerFileData>, HandlerError> {
    let mut player_file_refs: BTreeMap<Uuid, PlayerFileData> = BTreeMap::new();
    for dir_entry in std::fs::read_dir(players_dir).map_err(CoreError::Io)? {
        let Ok(dir_entry) = dir_entry else { continue };
        let path = dir_entry.path();
        if path.extension().is_none_or(|extension| extension != "sav") {
            continue;
        }
        let Some(stem) = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
        else {
            continue;
        };
        let Some((uid, is_dps)) = parse_player_file_stem(&stem) else {
            tracing::error!("Failed to parse player save path: {}", path.display());
            continue;
        };
        let file_ref = player_file_refs
            .entry(uid)
            .or_insert(PlayerFileData::Paths {
                sav: None,
                dps: None,
            });
        if let PlayerFileData::Paths { sav, dps } = file_ref {
            if is_dps {
                *dps = Some(path);
            } else {
                *sav = Some(path);
            }
        }
    }
    Ok(player_file_refs)
}

pub async fn handle_select_save(
    data: SelectSaveData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if data.r#type != "steam" {
        // GamePass save loading (get_gamepass_saves / select_gamepass_save)
        // is Phase 4 scope; Steam is the only save_type Phase 1 supports.
        return Err(HandlerError::Other(
            "GamePass saves are not supported yet".to_string(),
        ));
    }

    let layout = validate_steam_save_directory(&data.path)?;
    let level_sav_bytes = std::fs::read(&layout.level_sav).map_err(CoreError::Io)?;
    let level_meta_bytes = match &layout.level_meta {
        Some(meta_path) => Some(std::fs::read(meta_path).map_err(CoreError::Io)?),
        None => None,
    };
    let player_file_refs = discover_player_file_refs(&layout.players_dir)?;

    let progress = ctx.emitter.progress_sink();
    let session = SaveSession::load(
        SaveKind::Steam {
            level_path: layout.level_sav.clone(),
        },
        data.path.clone(),
        "steam",
        &level_sav_bytes,
        level_meta_bytes.as_deref(),
        player_file_refs,
        layout.global_pal_storage_sav.clone(),
        &progress,
    )?;

    let payload = LoadedSaveFilesData {
        level: layout.level_sav.to_string_lossy().into_owned(),
        players: session
            .player_file_refs
            .keys()
            .map(|uid| uid.to_string())
            .collect(),
        world_name: session.world_name.clone(),
        r#type: "steam",
        size: session.size,
        has_gps: layout.global_pal_storage_sav.is_some(),
    };
    ctx.emitter.emit(MessageType::LoadedSaveFiles, &payload);
    emit_summary_messages(&session, ctx.emitter);

    ctx.session.save = Some(session);
    Ok(())
}

/// Per-entry decompressed-size ceiling: bounds a maliciously crafted zip
/// entry's memory/CPU blow-up (a "zip bomb" is real decompression
/// amplification, not merely a lie in the declared-size header -- capping
/// the number of bytes actually read back out is what matters). 1 GiB
/// matches the "real Level.sav files are 100s of MB" headroom already used
/// for the whole WS frame (`ws::MAX_WS_MESSAGE_BYTES`) and the
/// `/api/convert` body limit.
const MAX_ZIP_ENTRY_BYTES: u64 = 1 << 30;

/// Ceiling on the number of central-directory entries a zip may declare. A
/// legitimate save zip has a handful of entries (Level.sav, LevelMeta.sav, a
/// few players, optionally GlobalPalStorage.sav); this only trips on an
/// abusive upload (e.g. a huge number of empty entries).
const MAX_ZIP_ENTRIES: usize = 100_000;

fn ensure_entry_count_within_limit(entry_count: usize, limit: usize) -> Result<(), HandlerError> {
    if entry_count > limit {
        return Err(HandlerError::Other(format!(
            "Zip file declares {entry_count} entries, exceeding the {limit}-entry limit"
        )));
    }
    Ok(())
}

fn ensure_entry_size_within_limit(
    entry_name: &str,
    observed_len: usize,
    limit: u64,
) -> Result<(), HandlerError> {
    if observed_len as u64 > limit {
        return Err(HandlerError::Other(format!(
            "Zip entry '{entry_name}' exceeds the {limit}-byte decompression limit"
        )));
    }
    Ok(())
}

struct ZipLayout {
    save_id: String,
    level_sav_name: String,
    level_meta_name: String,
    players_folder: String,
    gps_name: String,
}

/// Port of `load_zip_file_handler`'s "nested vs flat" detection: the zip is
/// "nested" (everything under a single top-level save-id folder) unless a
/// top-level `Level.sav` entry exists. `save_id` is always the piece of the
/// FIRST entry's name before its first `/` -- which by construction can
/// never itself contain `/` -- so it can never be used to build a path that
/// escapes a single path component (see `zip_gps_temp_path`), even when an
/// attacker crafts an entry name like `"../../evil/Level.sav"`.
fn resolve_zip_layout(file_list: &[String]) -> ZipLayout {
    let nested = !file_list.iter().any(|name| name == "Level.sav");
    let save_id = if nested {
        file_list
            .first()
            .and_then(|name| name.split('/').next())
            .unwrap_or("")
            .to_string()
    } else {
        "uploaded_save".to_string()
    };
    ZipLayout {
        level_sav_name: if nested {
            format!("{save_id}/Level.sav")
        } else {
            "Level.sav".to_string()
        },
        level_meta_name: if nested {
            format!("{save_id}/LevelMeta.sav")
        } else {
            "LevelMeta.sav".to_string()
        },
        players_folder: if nested {
            format!("{save_id}/Players/")
        } else {
            "Players/".to_string()
        },
        gps_name: if nested {
            format!("{save_id}/GlobalPalStorage.sav")
        } else {
            "GlobalPalStorage.sav".to_string()
        },
        save_id,
    }
}

/// Where an uploaded zip's `GlobalPalStorage.sav` entry is staged, mirroring
/// Python's `os.path.join(tempfile.gettempdir(), f"{save_id}_GlobalPalStorage.sav")`.
/// `save_id` is guaranteed slash-free by `resolve_zip_layout` (see its doc
/// comment), so this can never resolve outside `std::env::temp_dir()`
/// regardless of what the uploaded zip's entry names contain.
fn zip_gps_temp_path(save_id: &str) -> PathBuf {
    std::env::temp_dir().join(format!("{save_id}_GlobalPalStorage.sav"))
}

pub async fn handle_load_zip_file(
    data: Vec<u8>,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data))
        .map_err(|zip_error| HandlerError::Other(zip_error.to_string()))?;
    ensure_entry_count_within_limit(archive.len(), MAX_ZIP_ENTRIES)?;

    // Central-directory order (matches Python's `namelist()` order) -- the
    // zip crate's `file_names()` iterator is unordered, so index explicitly.
    let mut file_list = Vec::with_capacity(archive.len());
    for index in 0..archive.len() {
        let name = archive
            .by_index(index)
            .map_err(|zip_error| HandlerError::Other(zip_error.to_string()))?
            .name()
            .to_string();
        file_list.push(name);
    }
    if file_list.is_empty() {
        return Err(HandlerError::Other("Zip file is empty".to_string()));
    }

    let layout = resolve_zip_layout(&file_list);
    if !file_list.iter().any(|name| name == &layout.level_sav_name) {
        return Err(HandlerError::Other(
            "Zip file does not contain 'Level.sav'".to_string(),
        ));
    }
    if !file_list
        .iter()
        .any(|name| name.starts_with(&layout.players_folder))
    {
        return Err(HandlerError::Other(
            "Zip file does not contain 'Players' folder".to_string(),
        ));
    }

    let mut read_entry = |name: &str| -> Result<Vec<u8>, HandlerError> {
        let mut entry = archive
            .by_name(name)
            .map_err(|zip_error| HandlerError::Other(zip_error.to_string()))?;
        let mut bytes = Vec::new();
        entry
            .by_ref()
            .take(MAX_ZIP_ENTRY_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(CoreError::Io)?;
        ensure_entry_size_within_limit(name, bytes.len(), MAX_ZIP_ENTRY_BYTES)?;
        Ok(bytes)
    };

    let level_sav_bytes = read_entry(&layout.level_sav_name)?;
    let level_meta_bytes = if file_list.iter().any(|name| name == &layout.level_meta_name) {
        Some(read_entry(&layout.level_meta_name)?)
    } else {
        None
    };

    // Preserve zip encounter order for the wire "players" array: Python's
    // `player_saves` is a plain dict built by walking `namelist()` in order,
    // and `[str(p) for p in player_saves.keys()]` reflects dict insertion
    // (first-encounter) order, not a UUID sort.
    let mut player_order: Vec<Uuid> = Vec::new();
    let mut player_file_refs: BTreeMap<Uuid, PlayerFileData> = BTreeMap::new();
    let player_entry_names: Vec<String> = file_list
        .iter()
        .filter(|name| name.contains("Players") && name.ends_with(".sav"))
        .cloned()
        .collect();
    for name in player_entry_names {
        let Some(stem) = Path::new(&name)
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
        else {
            continue;
        };
        let Some((uid, is_dps)) = parse_player_file_stem(&stem) else {
            tracing::warn!("Skipping invalid player file name: {name}");
            continue;
        };
        let entry_bytes = read_entry(&name)?;
        if !player_file_refs.contains_key(&uid) {
            player_order.push(uid);
        }
        let file_ref = player_file_refs
            .entry(uid)
            .or_insert(PlayerFileData::Bytes {
                sav: None,
                dps: None,
            });
        if let PlayerFileData::Bytes { sav, dps } = file_ref {
            if is_dps {
                *dps = Some(entry_bytes);
            } else {
                *sav = Some(entry_bytes);
            }
        }
    }
    if player_file_refs.is_empty() {
        return Err(HandlerError::Other(
            "No valid player save files found in the 'Players' folder".to_string(),
        ));
    }

    let gps_file_path = if file_list.iter().any(|name| name == &layout.gps_name) {
        let temp_path = zip_gps_temp_path(&layout.save_id);
        std::fs::write(&temp_path, read_entry(&layout.gps_name)?).map_err(CoreError::Io)?;
        Some(temp_path)
    } else {
        None
    };

    let progress = ctx.emitter.progress_sink();
    let session = SaveSession::load(
        SaveKind::InMemory,
        layout.save_id.clone(),
        "steam",
        &level_sav_bytes,
        level_meta_bytes.as_deref(),
        player_file_refs,
        gps_file_path.clone(),
        &progress,
    )?;

    let world_name_display = if session.world_name.is_empty() {
        "Unknown".to_string()
    } else {
        session.world_name.clone()
    };

    progress("Zip file uploaded and processed successfully, results coming right up!");

    let payload = LoadedSaveFilesData {
        level: layout.save_id,
        players: player_order.iter().map(|uid| uid.to_string()).collect(),
        world_name: world_name_display,
        r#type: "steam",
        size: session.size,
        has_gps: gps_file_path.is_some(),
    };
    ctx.emitter.emit(MessageType::LoadedSaveFiles, &payload);
    emit_summary_messages(&session, ctx.emitter);

    ctx.session.save = Some(session);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAYER_ONE: &str = "11111111-1111-1111-1111-111111111111";

    #[test]
    fn test_parse_player_file_stem() {
        assert_eq!(
            Some((PLAYER_ONE.parse().unwrap(), false)),
            parse_player_file_stem(PLAYER_ONE)
        );
        assert_eq!(
            Some((PLAYER_ONE.parse().unwrap(), true)),
            parse_player_file_stem(&format!("{PLAYER_ONE}_dps"))
        );
        assert_eq!(None, parse_player_file_stem("not-a-uuid"));
        assert_eq!(None, parse_player_file_stem(""));
    }

    #[test]
    fn test_validate_steam_save_directory_reports_missing_level_sav_first() {
        let temp_dir = tempfile::tempdir().unwrap();
        let level_sav_path = temp_dir.path().join("Level.sav");

        let error = validate_steam_save_directory(&level_sav_path.to_string_lossy())
            .unwrap_err()
            .to_string();
        assert_eq!("Level.sav file not found in the selected directory.", error);
    }

    #[test]
    fn test_validate_steam_save_directory_reports_missing_players_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(temp_dir.path().join("Level.sav"), b"x").unwrap();
        let level_sav_path = temp_dir.path().join("Level.sav");

        let error = validate_steam_save_directory(&level_sav_path.to_string_lossy())
            .unwrap_err()
            .to_string();
        assert_eq!(
            "Players directory not found in the selected directory.",
            error
        );
    }

    #[test]
    fn test_validate_steam_save_directory_reports_no_player_saves() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(temp_dir.path().join("Level.sav"), b"x").unwrap();
        std::fs::create_dir(temp_dir.path().join("Players")).unwrap();
        let level_sav_path = temp_dir.path().join("Level.sav");

        let error = validate_steam_save_directory(&level_sav_path.to_string_lossy())
            .unwrap_err()
            .to_string();
        assert_eq!(
            "No player save files found in the Players directory.",
            error
        );
    }

    #[test]
    fn test_validate_steam_save_directory_succeeds_without_optional_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(temp_dir.path().join("Level.sav"), b"x").unwrap();
        let players_dir = temp_dir.path().join("Players");
        std::fs::create_dir(&players_dir).unwrap();
        std::fs::write(players_dir.join(format!("{PLAYER_ONE}.sav")), b"x").unwrap();
        let level_sav_path = temp_dir.path().join("Level.sav");

        let layout = validate_steam_save_directory(&level_sav_path.to_string_lossy()).unwrap();
        assert!(layout.level_meta.is_none());
        assert!(layout.global_pal_storage_sav.is_none());
        assert_eq!(players_dir, layout.players_dir);
    }

    #[test]
    fn test_validate_steam_save_directory_finds_optional_files_when_present() {
        // save_dir = temp_dir/world (so save_dir.parent() = temp_dir, a
        // location this test can safely write GlobalPalStorage.sav into
        // without touching anything outside its own sandbox).
        let temp_dir = tempfile::tempdir().unwrap();
        let save_dir = temp_dir.path().join("world");
        std::fs::create_dir(&save_dir).unwrap();
        std::fs::write(save_dir.join("Level.sav"), b"x").unwrap();
        std::fs::write(save_dir.join("LevelMeta.sav"), b"x").unwrap();
        let players_dir = save_dir.join("Players");
        std::fs::create_dir(&players_dir).unwrap();
        std::fs::write(players_dir.join(format!("{PLAYER_ONE}.sav")), b"x").unwrap();
        std::fs::write(temp_dir.path().join("GlobalPalStorage.sav"), b"x").unwrap();
        let level_sav_path = save_dir.join("Level.sav");

        let layout = validate_steam_save_directory(&level_sav_path.to_string_lossy()).unwrap();
        assert!(layout.level_meta.is_some());
        assert_eq!(
            temp_dir.path().join("GlobalPalStorage.sav"),
            layout.global_pal_storage_sav.unwrap()
        );
    }

    #[test]
    fn test_discover_player_file_refs_pairs_sav_and_dps_and_skips_invalid_names() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(temp_dir.path().join(format!("{PLAYER_ONE}.sav")), b"x").unwrap();
        std::fs::write(temp_dir.path().join(format!("{PLAYER_ONE}_dps.sav")), b"y").unwrap();
        std::fs::write(temp_dir.path().join("not-a-uuid.sav"), b"z").unwrap();
        std::fs::write(temp_dir.path().join("ignored.txt"), b"w").unwrap();

        let refs = discover_player_file_refs(temp_dir.path()).unwrap();
        assert_eq!(1, refs.len());
        let uid: Uuid = PLAYER_ONE.parse().unwrap();
        match &refs[&uid] {
            PlayerFileData::Paths { sav, dps } => {
                assert!(sav.is_some());
                assert!(dps.is_some());
            }
            other => panic!("expected Paths variant, got {other:?}"),
        }
    }

    #[test]
    fn test_ensure_entry_count_within_limit() {
        assert!(ensure_entry_count_within_limit(10, 10).is_ok());
        let error = ensure_entry_count_within_limit(11, 10)
            .unwrap_err()
            .to_string();
        assert!(error.contains("11 entries"));
        assert!(error.contains("exceeding the 10-entry limit"));
    }

    #[test]
    fn test_ensure_entry_size_within_limit() {
        assert!(ensure_entry_size_within_limit("Level.sav", 10, 10).is_ok());
        let error = ensure_entry_size_within_limit("Level.sav", 11, 10)
            .unwrap_err()
            .to_string();
        assert!(error.contains("'Level.sav'"));
        assert!(error.contains("exceeds the 10-byte decompression limit"));
    }

    #[test]
    fn test_resolve_zip_layout_flat_zip_uses_uploaded_save_as_id() {
        let file_list = vec!["Level.sav".to_string(), "Players/x.sav".to_string()];
        let layout = resolve_zip_layout(&file_list);
        assert_eq!("uploaded_save", layout.save_id);
        assert_eq!("Level.sav", layout.level_sav_name);
        assert_eq!("Players/", layout.players_folder);
        assert_eq!("GlobalPalStorage.sav", layout.gps_name);
    }

    #[test]
    fn test_resolve_zip_layout_nested_zip_uses_top_level_folder_as_id() {
        let file_list = vec![
            "MyWorld/Level.sav".to_string(),
            "MyWorld/Players/x.sav".to_string(),
        ];
        let layout = resolve_zip_layout(&file_list);
        assert_eq!("MyWorld", layout.save_id);
        assert_eq!("MyWorld/Level.sav", layout.level_sav_name);
        assert_eq!("MyWorld/Players/", layout.players_folder);
    }

    /// Security-critical property: even a maliciously crafted top-level
    /// entry name (directory-traversal-shaped) can never leave `save_id`
    /// holding a '/' -- `split('/').next()` structurally guarantees this,
    /// which is what makes `zip_gps_temp_path` safe below. This test would
    /// fail immediately if `resolve_zip_layout` were ever rewritten to use
    /// the whole first path segment chain instead of just the first piece.
    #[test]
    fn test_resolve_zip_layout_save_id_never_contains_a_path_separator() {
        for first_entry in [
            "../../evil/Level.sav",
            "/etc/passwd/Level.sav",
            "..\\windows\\Level.sav", // backslash is not '/' -- stays in one segment
            "Level.sav",
        ] {
            let layout = resolve_zip_layout(&[first_entry.to_string()]);
            assert!(
                !layout.save_id.contains('/'),
                "save_id must never contain '/', got {:?} from {first_entry:?}",
                layout.save_id
            );
        }
    }

    /// Direct proof that a hostile `save_id` cannot make the GPS temp file
    /// escape the OS temp directory: `Path::join` only escapes its base when
    /// the joined string contains a `/`-separated `..` component, and
    /// `save_id` can never contain `/` (see the test above), so the result's
    /// parent is always exactly `std::env::temp_dir()`.
    #[test]
    fn test_zip_gps_temp_path_never_escapes_the_temp_directory() {
        for save_id in ["..", "...", "normal_id", "", "%2e%2e"] {
            let path = zip_gps_temp_path(save_id);
            assert_eq!(std::env::temp_dir(), path.parent().unwrap());
        }
    }
}
