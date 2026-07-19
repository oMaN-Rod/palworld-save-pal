//! Save-file load/write handlers: select_save, load_zip_file,
//! update_save_file, download_save_file, save_modded_save, rename_world,
//! convert_sav_file, unlock_map.
//!
//! In desktop mode `handle_select_save` drives a native file picker up front:
//! a cancelled pick answers `no_file_selected` and loads nothing.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use base64::Engine as _;
use psp_core::domain::{guild, pal, player};
use psp_core::dto::guild::GuildDto;
use psp_core::dto::ordered_map::OrderedMap;
use psp_core::dto::pal::PalDto;
use psp_core::dto::player::PlayerDto;
use psp_core::error::CoreError;
use psp_core::progress::ProgressSink;
use psp_core::session::{PlayerFileData, SaveKind, SaveSession};
use serde_json::json;
use uuid::Uuid;

use crate::dispatcher::HandlerCtx;
use crate::emitter::Emitter;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;

#[derive(Debug, serde::Deserialize)]
pub struct SelectSaveData {
    pub r#type: String,
    /// Required in web mode; absent in desktop mode, where the frontend omits
    /// it and `handle_select_save` resolves it via a native file dialog.
    pub path: Option<String>,
    /// Accepted for wire compatibility; not read.
    #[allow(dead_code)]
    pub local: bool,
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct LoadedSaveFilesData {
    level: String,
    players: Vec<String>,
    world_name: String,
    r#type: &'static str,
    size: u64,
    has_gps: bool,
    /// The single fact the WorldOption button gates on, across all three platforms.
    world_option_present: bool,
    /// The id the session was registered under, so the frontend can reattach
    /// after a refresh.
    session_id: String,
}

impl LoadedSaveFilesData {
    /// Rebuilds the load overview from an already-parsed session, for
    /// `reattach_session`. `level`/`players` are derived the way
    /// `sync_app_state` derives them (save id + summary order) — the only
    /// values a reattach has.
    pub(crate) fn from_session(session: &SaveSession, session_id: Uuid) -> Self {
        Self {
            level: session.save_id.clone(),
            players: session
                .player_summary_order
                .iter()
                .map(|uid| uid.to_string())
                .collect(),
            world_name: session.world_name.clone(),
            r#type: session.save_type_label,
            size: session.size,
            has_gps: session.gps_available(),
            world_option_present: session.world_option.is_some(),
            session_id: session_id.to_string(),
        }
    }
}

/// Emits the load overview (`loaded_save_files` + both summaries) for a
/// reattached session — the same tail a fresh load emits.
pub(crate) fn emit_reattach_overview(session: &SaveSession, session_id: Uuid, emitter: &Emitter) {
    emitter.emit(
        MessageType::LoadedSaveFiles,
        &LoadedSaveFilesData::from_session(session, session_id),
    );
    emit_summary_messages(session, emitter);
}

/// `get_player_summaries` then `get_guild_summaries`, in that order, every time
/// a load completes — the frontend relies on both arriving, and on the order.
pub(crate) fn emit_summary_messages(session: &SaveSession, emitter: &Emitter) {
    emitter.emit(MessageType::GetPlayerSummaries, &session.player_summaries);
    emitter.emit(MessageType::GetGuildSummaries, &session.guild_summaries);
}

/// A `Players/*.sav` or `Players/*_dps.sav` file stem, split into its player id
/// and whether it's the "_dps" companion file. `None` for anything that doesn't
/// parse as a UUID once "_dps" is stripped; callers skip those.
fn parse_player_file_stem(stem: &str) -> Option<(Uuid, bool)> {
    let is_dps = stem.contains("_dps");
    stem.replace("_dps", "")
        .parse::<Uuid>()
        .ok()
        .map(|uid| (uid, is_dps))
}

/// `pub(crate)` so `handlers::tools`'s `load_source_save` reuses this exact
/// layout and validation rather than re-implementing it with its own,
/// necessarily divergent, error strings.
#[derive(Debug)]
pub(crate) struct SteamSaveLayout {
    pub(crate) level_sav: PathBuf,
    pub(crate) level_meta: Option<PathBuf>,
    pub(crate) world_option: Option<PathBuf>,
    pub(crate) players_dir: PathBuf,
    pub(crate) global_pal_storage_sav: Option<PathBuf>,
}

/// Error strings AND check order are wire-visible: the frontend shows the first
/// failure it gets back, so a reordering changes what the user is told.
pub(crate) fn validate_steam_save_directory(
    save_path: &str,
) -> Result<SteamSaveLayout, HandlerError> {
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

    // Optional, exactly like LevelMeta: a world without one simply has no
    // editable options.
    let world_option_path = save_dir.join("WorldOption.sav");
    let world_option = world_option_path.exists().then_some(world_option_path);

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
        world_option,
        players_dir,
        global_pal_storage_sav,
    })
}

/// Every `Players/*.sav`, its "_dps" companion folded into the same map entry,
/// invalid names logged and skipped. Returns BOTH the pairing map (uuid-sorted,
/// the type `SaveSession::player_file_refs` requires) and the order in which
/// players were first encountered — `handle_select_save` builds the wire
/// `players` array from the discovery order, NOT from the sorted map.
pub(crate) fn discover_player_file_refs(
    players_dir: &Path,
) -> Result<(BTreeMap<Uuid, PlayerFileData>, Vec<Uuid>), HandlerError> {
    let dir_entries = std::fs::read_dir(players_dir).map_err(CoreError::Io)?;
    let paths = dir_entries
        .filter_map(|dir_entry| dir_entry.ok())
        .map(|dir_entry| dir_entry.path());
    Ok(collect_player_file_refs(paths))
}

/// Pure core of `discover_player_file_refs`, split out so the discovery-order
/// guarantee can be tested against a deliberately non-UUID-ascending path
/// sequence: `std::fs::read_dir` is name-sorted on NTFS, which for UUID-named
/// files coincides with UUID-ascending order and would hide a resort.
fn collect_player_file_refs<I>(paths: I) -> (BTreeMap<Uuid, PlayerFileData>, Vec<Uuid>)
where
    I: IntoIterator<Item = PathBuf>,
{
    let mut player_file_refs: BTreeMap<Uuid, PlayerFileData> = BTreeMap::new();
    let mut discovery_order: Vec<Uuid> = Vec::new();
    for path in paths {
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
        if !player_file_refs.contains_key(&uid) {
            discovery_order.push(uid);
        }
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
    (player_file_refs, discovery_order)
}

/// Gamepass branch of `select_save`. The two failure payloads are `error`
/// frames whose data is a PLAIN STRING, not the `{message, trace}` shape the
/// dispatcher emits for a raised `HandlerError`. On success the
/// `select_gamepass_save` data IS the saves map (`{<save_id>: GamepassSaveData}`)
/// — NOT the `{"saves": ...}` wrapper that only `scan_gamepass_saves` uses.
pub(crate) async fn select_gamepass_directory(
    index_file_path: &str,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let containers_dir = Path::new(index_file_path)
        .parent()
        .map(|parent| parent.to_path_buf())
        .unwrap_or_default();
    if !containers_dir.join("containers.index").exists() {
        ctx.emitter.emit(
            MessageType::Error,
            &"containers.index file not found in the selected directory.",
        );
        return Ok(());
    }
    let saves = psp_core::gamepass::scan::scan_saves(&containers_dir)?;
    if saves.is_empty() {
        ctx.emitter.emit(
            MessageType::Error,
            &"No valid Palworld saves found in the selected directory.",
        );
        return Ok(());
    }
    // Cache the discovered saves so `select_gamepass_save` can resolve the
    // selected save's metadata later.
    ctx.session.gamepass_saves = saves
        .iter()
        .map(|(save_id, save_data)| (save_id.clone(), save_data.clone()))
        .collect();
    ctx.emitter.emit(MessageType::SelectGamepassSave, &saves);
    Ok(())
}

/// Desktop-mode dialog flow shared by select_save and unlock_map.
/// `Ok(Some(path))` = picked and valid; `Ok(None)` = cancelled (`no_file_selected`
/// already emitted, caller just returns); `Err` = invalid pick (the dispatcher
/// emits `error`).
async fn pick_save_file_via_dialog(
    save_type: &str,
    ctx: &mut HandlerCtx<'_>,
) -> Result<Option<std::path::PathBuf>, HandlerError> {
    let saved_dir = psp_db::settings::saved_save_dir(&ctx.app.db).await?;
    let request = crate::desktop_dialogs::dialog_request_for(save_type, saved_dir.as_deref());
    let Some(selected) = ctx.app.dialogs.pick_file(request).await else {
        ctx.emitter
            .emit(MessageType::NoFileSelected, &"No file selected");
        return Ok(None);
    };
    crate::desktop_dialogs::validate_selected_file(
        save_type,
        &selected,
        &crate::desktop_dialogs::application_root(),
    )
    .map_err(HandlerError::Other)?;
    Ok(Some(selected))
}

pub async fn handle_select_save(
    data: SelectSaveData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let mut resolved_path = data.path.clone();
    if ctx.app.config.desktop_mode {
        let Some(selected) = pick_save_file_via_dialog(&data.r#type, ctx).await? else {
            return Ok(()); // canceled; no_file_selected already emitted
        };
        if let Some(parent_dir) = selected.parent() {
            // Persist BEFORE loading: a load failure must not lose the dir the
            // user just picked.
            psp_db::settings::update_save_dir(&ctx.app.db, &parent_dir.to_string_lossy()).await?;
        }
        resolved_path = Some(selected.to_string_lossy().into_owned());
    }
    let save_path = resolved_path
        .ok_or_else(|| HandlerError::Other("select_save requires a path".to_string()))?;

    if data.r#type != "steam" {
        // Every non-"steam" save type is handled as gamepass.
        return select_gamepass_directory(&save_path, ctx).await;
    }

    let layout = validate_steam_save_directory(&save_path)?;
    let level_sav_bytes = std::fs::read(&layout.level_sav).map_err(CoreError::Io)?;
    let level_meta_bytes = match &layout.level_meta {
        Some(meta_path) => Some(std::fs::read(meta_path).map_err(CoreError::Io)?),
        None => None,
    };
    let world_option_bytes = match &layout.world_option {
        Some(world_option_path) => Some(std::fs::read(world_option_path).map_err(CoreError::Io)?),
        None => None,
    };
    let (player_file_refs, player_discovery_order) =
        discover_player_file_refs(&layout.players_dir)?;

    let progress = ctx.emitter.progress_sink();
    let session = SaveSession::load(
        SaveKind::Steam {
            level_path: layout.level_sav.clone(),
        },
        save_path.clone(),
        "steam",
        &level_sav_bytes,
        level_meta_bytes.as_deref(),
        world_option_bytes.as_deref(),
        player_file_refs,
        layout.global_pal_storage_sav.clone(),
        // Emit the leading generic "Loading Level.sav..." progress frame.
        true,
        &progress,
    )?;

    let session_id = ctx.register_current_session();
    let payload = LoadedSaveFilesData {
        level: layout.level_sav.to_string_lossy().into_owned(),
        players: player_discovery_order
            .iter()
            .map(|uid| uid.to_string())
            .collect(),
        world_name: session.world_name.clone(),
        r#type: "steam",
        size: session.size,
        has_gps: layout.global_pal_storage_sav.is_some(),
        world_option_present: session.world_option.is_some(),
        session_id: session_id.to_string(),
    };
    ctx.emitter.emit(MessageType::LoadedSaveFiles, &payload);
    emit_summary_messages(&session, ctx.emitter);

    ctx.session.save = Some(session);
    Ok(())
}

/// Per-entry decompressed-size ceiling, bounding a zip bomb's memory/CPU
/// blow-up. It caps bytes actually read back OUT (a bomb is real decompression
/// amplification, not just a lie in the declared-size header). 1 GiB matches the
/// headroom real Level.sav files already get from `ws::MAX_WS_MESSAGE_BYTES`.
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
    world_option_name: String,
    players_folder: String,
    gps_name: String,
}

/// "Nested vs flat" detection: the zip is nested (everything under a single
/// top-level save-id folder) unless a top-level `Level.sav` entry exists.
///
/// `save_id` is always the piece of the FIRST entry's name before its first
/// `/`, so it structurally can never itself contain a `/` — that is what stops
/// a crafted entry name like `"../../evil/Level.sav"` from building a path that
/// escapes a single component (see `zip_gps_temp_path`).
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
        world_option_name: if nested {
            format!("{save_id}/WorldOption.sav")
        } else {
            "WorldOption.sav".to_string()
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

/// Where an uploaded zip's `GlobalPalStorage.sav` entry is staged. `save_id` is
/// guaranteed slash-free by `resolve_zip_layout`, so this can never resolve
/// outside `std::env::temp_dir()`, whatever the zip's entry names contain.
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

    // Index explicitly to get central-directory order: `file_names()` is
    // unordered, and `resolve_zip_layout` below reads the FIRST entry.
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
    // Optional. Unlike other unrecognized entries, this one is retained: the
    // editor needs it and the download zip must round-trip it.
    let world_option_bytes = if file_list
        .iter()
        .any(|name| name == &layout.world_option_name)
    {
        Some(read_entry(&layout.world_option_name)?)
    } else {
        None
    };

    // Zip encounter order, not a UUID sort: the wire "players" array follows
    // this, same as the Steam directory scan.
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
        world_option_bytes.as_deref(),
        player_file_refs,
        gps_file_path.clone(),
        // Emit the leading generic "Loading Level.sav..." progress frame.
        true,
        &progress,
    )?;

    let world_name_display = if session.world_name.is_empty() {
        "Unknown".to_string()
    } else {
        session.world_name.clone()
    };

    progress("Zip file uploaded and processed successfully, results coming right up!");

    let session_id = ctx.register_current_session();
    let payload = LoadedSaveFilesData {
        level: layout.save_id,
        players: player_order.iter().map(|uid| uid.to_string()).collect(),
        world_name: world_name_display,
        r#type: "steam",
        size: session.size,
        has_gps: gps_file_path.is_some(),
        world_option_present: session.world_option.is_some(),
        session_id: session_id.to_string(),
    };
    ctx.emitter.emit(MessageType::LoadedSaveFiles, &payload);
    emit_summary_messages(&session, ctx.emitter);

    ctx.session.save = Some(session);
    Ok(())
}

/// Every field is optional, and a present-but-EMPTY map skips its update
/// section just like an absent one.
///
/// The int-keyed maps (`modified_dps_pals`, `modified_gps_pals`) arrive as JSON
/// objects with string keys; `OrderedMap`'s `Deserialize` coerces `"0"` → `0i32`.
#[derive(Debug, serde::Deserialize)]
pub struct UpdateSaveFileData {
    #[serde(default)]
    pub modified_pals: Option<OrderedMap<Uuid, PalDto>>,
    #[serde(default)]
    pub modified_dps_pals: Option<OrderedMap<i32, PalDto>>,
    #[serde(default)]
    pub modified_players: Option<OrderedMap<Uuid, PlayerDto>>,
    #[serde(default)]
    pub modified_guilds: Option<OrderedMap<Uuid, GuildDto>>,
    #[serde(default)]
    pub modified_gps_pals: Option<OrderedMap<i32, PalDto>>,
}

/// Apply order is load-bearing: pals → players → guilds → dps → gps. A section
/// is skipped when its map is absent or empty.
///
/// The no-save path raises `HandlerError::Other("No save file loaded")` rather
/// than using `save_mut()?`, whose `CoreError::SaveNotLoaded` renders the
/// different string `"no save loaded"` — the frontend matches on this one.
pub async fn handle_update_save_file(
    data: UpdateSaveFileData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let progress = ctx.emitter.progress_sink();
    let game_data = &ctx.app.game_data;
    let Some(session) = ctx.session.save.as_mut() else {
        return Err(HandlerError::Other("No save file loaded".to_string()));
    };

    if let Some(modified_pals) = data.modified_pals.filter(|map| !map.is_empty()) {
        pal::update_pals(session, game_data, &modified_pals, &progress)?;
    }
    if let Some(modified_players) = data.modified_players.filter(|map| !map.is_empty()) {
        player::update_players(session, game_data, &modified_players, &progress)?;
    }
    if let Some(modified_guilds) = data.modified_guilds.filter(|map| !map.is_empty()) {
        guild::update_guilds(session, game_data, &modified_guilds, &progress)?;
    }
    if let Some(modified_dps_pals) = data.modified_dps_pals.filter(|map| !map.is_empty()) {
        pal::update_dps_pals(session, game_data, &modified_dps_pals, &progress)?;
    }
    if let Some(modified_gps_pals) = data.modified_gps_pals.filter(|map| !map.is_empty()) {
        session.update_gps_pals(game_data, &modified_gps_pals, &progress)?;
    }

    ctx.emitter
        .emit(MessageType::UpdateSaveFile, &"Changes saved");
    Ok(())
}

/// A player's uuid as it appears inside the DOWNLOAD zip: LOWERCASE hex, no
/// dashes. Deliberately different from `save_modded_player_stem` (uppercase);
/// the two must not be collapsed.
fn download_player_stem(player_id: &Uuid) -> String {
    player_id.simple().to_string()
}

/// Builds an in-memory DEFLATE zip (`Level.sav`, then `Players/<lower-hex>.sav`
/// and its `_dps.sav` companion per loaded player) and emits it as a
/// ONE-ELEMENT ARRAY `[{"name": ..., "content": <base64>}]` — the frontend
/// iterates the array, so the wrapper must stay even for a single file.
pub async fn handle_download_save_file(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let progress = ctx.emitter.progress_sink();
    let Some(session) = ctx.session.save.as_ref() else {
        return Err(HandlerError::Other("No save file loaded".to_string()));
    };

    progress("Generating save files in memory... 💾");
    let level_sav_bytes = session.level_sav_bytes()?;
    let player_files = session.player_sav_bytes()?;

    progress("Creating ZIP archive... 🤏");
    let mut zip_cursor = std::io::Cursor::new(Vec::new());
    let mut player_count: usize = 0;
    {
        let mut zip_writer = zip::ZipWriter::new(&mut zip_cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip_writer
            .start_file("Level.sav", options)
            .map_err(|zip_error| HandlerError::Other(zip_error.to_string()))?;
        zip_writer
            .write_all(&level_sav_bytes)
            .map_err(CoreError::Io)?;
        // Included whenever present, dirty or not: the download zip IS the user's
        // copy of the save, so omitting an unmodified file would lose it.
        if let Some(world_option) = &session.world_option {
            zip_writer
                .start_file("WorldOption.sav", options)
                .map_err(|zip_error| HandlerError::Other(zip_error.to_string()))?;
            zip_writer
                .write_all(&psp_core::savio::write_sav_bytes(world_option)?)
                .map_err(CoreError::Io)?;
        }
        for (player_id, (sav_bytes, dps_bytes)) in &player_files {
            let stem = download_player_stem(player_id);
            zip_writer
                .start_file(format!("Players/{stem}.sav"), options)
                .map_err(|zip_error| HandlerError::Other(zip_error.to_string()))?;
            zip_writer.write_all(sav_bytes).map_err(CoreError::Io)?;
            player_count += 1;
            if let Some(dps_bytes) = dps_bytes {
                zip_writer
                    .start_file(format!("Players/{stem}_dps.sav"), options)
                    .map_err(|zip_error| HandlerError::Other(zip_error.to_string()))?;
                zip_writer.write_all(dps_bytes).map_err(CoreError::Io)?;
            }
        }
        // Non-nested layout: GlobalPalStorage.sav sits beside Level.sav, the
        // shape `build_zip_layout` reads back on a re-upload. Only present when
        // GPS was loaded and possibly edited.
        if let Some(gps_bytes) = session.gps_sav_bytes()? {
            zip_writer
                .start_file("GlobalPalStorage.sav", options)
                .map_err(|zip_error| HandlerError::Other(zip_error.to_string()))?;
            zip_writer.write_all(&gps_bytes).map_err(CoreError::Io)?;
        }
        zip_writer
            .finish()
            .map_err(|zip_error| HandlerError::Other(zip_error.to_string()))?;
    }
    let zip_bytes = zip_cursor.into_inner();

    progress(&format!(
        "Archive created with Level.sav and {player_count} player(s) data. Encoding..."
    ));
    let encoded_zip = base64::engine::general_purpose::STANDARD.encode(&zip_bytes);

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let world_name = if session.world_name.is_empty() {
        "PSP"
    } else {
        session.world_name.as_str()
    };
    let filename = format!("{world_name}_{timestamp}.zip");

    progress("Sending ZIP file to client... 🚀");
    ctx.emitter.emit(
        MessageType::DownloadSaveFile,
        &json!([{ "name": filename, "content": encoded_zip }]),
    );
    Ok(())
}

/// A player's uuid as it appears in the ON-DISK Steam write: UPPERCASE hex, no
/// dashes, which is what Palworld's own read path looks for. Deliberately the
/// opposite casing from `download_player_stem`.
fn save_modded_player_stem(player_id: &Uuid) -> String {
    player_id.simple().to_string().to_uppercase()
}

/// A `Players/` entry that is a genuine player save: a 32-char hex UUID stem,
/// optionally suffixed `_dps`, ending in `.sav`. Everything else in that
/// directory (junk `.sav`, non-hex stems, subfolders) is excluded from backups.
fn is_player_save_file(file_name: &str) -> bool {
    let Some(stem) = file_name.strip_suffix(".sav") else {
        return false;
    };
    let stem = stem.strip_suffix("_dps").unwrap_or(stem);
    stem.len() == 32 && stem.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for dir_entry in std::fs::read_dir(src)? {
        let dir_entry = dir_entry?;
        let entry_path = dir_entry.path();
        let dest_path = dst.join(dir_entry.file_name());
        if entry_path.is_dir() {
            copy_dir_recursive(&entry_path, &dest_path)?;
        } else {
            std::fs::copy(&entry_path, &dest_path)?;
        }
    }
    Ok(())
}

/// World-save files a backup keeps. Only these top-level names are copied;
/// anything else in the directory is skipped so stray user files never bloat
/// the backups root. Each is optional (copied only if present); load-time
/// validation guarantees `Level.sav`, the rest may be absent.
const BACKUP_ROOT_FILES: [&str; 4] = [
    "Level.sav",
    "LevelMeta.sav",
    "LocalData.sav",
    "WorldOption.sav",
];

/// Selectively copies whitelisted root files and validated `Players/` saves from
/// `save_dir` to `{backup_base}/{basename}_{%Y-%m-%d-%H-%M}`, appending a
/// `_{%S}` suffix on collision. `backup_base` is a parameter (not a constant) so
/// tests can point it at a `TempDir` instead of the real backups root. A missing
/// `save_dir` reports "skipping backup" rather than failing.
fn backup_save_directory(
    save_dir: &Path,
    backup_base: &Path,
    progress: &ProgressSink,
) -> Result<(), HandlerError> {
    std::fs::create_dir_all(backup_base).map_err(CoreError::Io)?;
    let dir_basename = save_dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let timestamp = chrono::Local::now().format("%Y-%m-%d-%H-%M").to_string();
    let mut backup_path = backup_base.join(format!("{dir_basename}_{timestamp}"));
    if backup_path.exists() {
        let seconds = chrono::Local::now().format("%S").to_string();
        backup_path = backup_base.join(format!("{dir_basename}_{timestamp}_{seconds}"));
    }

    progress("Backing up save directory... 🤓");
    if !save_dir.exists() {
        progress(&format!(
            "Save directory {} not found, skipping backup",
            save_dir.display()
        ));
        return Ok(());
    }

    std::fs::create_dir_all(&backup_path).map_err(CoreError::Io)?;
    for name in BACKUP_ROOT_FILES {
        let source = save_dir.join(name);
        if source.is_file() {
            std::fs::copy(&source, backup_path.join(name)).map_err(CoreError::Io)?;
        }
    }

    let players_dir = save_dir.join("Players");
    if players_dir.is_dir() {
        let backup_players_dir = backup_path.join("Players");
        std::fs::create_dir_all(&backup_players_dir).map_err(CoreError::Io)?;
        for entry in std::fs::read_dir(&players_dir).map_err(CoreError::Io)? {
            let entry = entry.map_err(CoreError::Io)?;
            let entry_path = entry.path();
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();
            if entry_path.is_file() && is_player_save_file(&file_name) {
                std::fs::copy(&entry_path, backup_players_dir.join(&*file_name))
                    .map_err(CoreError::Io)?;
            }
        }
    }

    Ok(())
}

/// Overwrites the loaded `GlobalPalStorage.sav` in place when GPS was loaded
/// and edited; a no-op when GPS was never opened (`gps_sav_bytes` is `None`).
///
/// GPS lives beside the world directory, NOT inside it, so the world-dir
/// backup does not cover it — a sibling `.backup` copy is taken first. Repeated
/// saves overwrite that copy, so it protects the last state, not the original.
fn write_gps_if_loaded(session: &SaveSession, progress: &ProgressSink) -> Result<(), HandlerError> {
    let Some(gps_bytes) = session.gps_sav_bytes()? else {
        return Ok(());
    };
    let Some(gps_path) = session.gps.file_path.as_ref() else {
        return Ok(());
    };
    progress("Writing Global Pal Storage file");
    if gps_path.exists() {
        std::fs::copy(gps_path, format!("{}.backup", gps_path.display()))
            .map_err(CoreError::Io)?;
    }
    std::fs::write(gps_path, &gps_bytes).map_err(CoreError::Io)?;
    Ok(())
}

/// Write half of `save_modded_steam_save`, split out so the backup+overwrite
/// path is testable against a `TempDir` without touching the user's real
/// save_dir or the process CWD. `Level.sav` goes to `level_path` (which may
/// differ from `save_dir`); `LevelMeta.sav` and the players go under `save_dir`.
fn write_steam_modded_save(
    session: &SaveSession,
    level_path: &Path,
    save_dir: &Path,
    backup_base: &Path,
    progress: &ProgressSink,
) -> Result<(), HandlerError> {
    backup_save_directory(save_dir, backup_base, progress)?;

    progress("Writing new save file... 🚀");
    let level_sav_bytes = session.level_sav_bytes()?;
    if let Some(parent) = level_path.parent() {
        std::fs::create_dir_all(parent).map_err(CoreError::Io)?;
    }
    std::fs::write(level_path, &level_sav_bytes).map_err(CoreError::Io)?;

    progress("Writing Level Meta file");
    let level_meta_bytes = session
        .level_meta_sav_bytes()?
        .ok_or_else(|| HandlerError::Other("No LevelMeta GvasFile has been loaded.".to_string()))?;
    std::fs::write(save_dir.join("LevelMeta.sav"), &level_meta_bytes).map_err(CoreError::Io)?;

    // Gated on dirty: an untouched WorldOption must not be rewritten. The save_dir
    // backup taken above already covers this file.
    if session.world_option_dirty {
        if let Some(world_option) = &session.world_option {
            progress("Saving WorldOption.sav...");
            std::fs::write(
                save_dir.join("WorldOption.sav"),
                psp_core::savio::write_sav_bytes(world_option)?,
            )
            .map_err(CoreError::Io)?;
        }
    }

    progress("Writing player files");
    let players_dir = save_dir.join("Players");
    std::fs::create_dir_all(&players_dir).map_err(CoreError::Io)?;
    for (player_id, (sav_bytes, dps_bytes)) in session.player_sav_bytes()? {
        let stem = save_modded_player_stem(&player_id);
        std::fs::write(players_dir.join(format!("{stem}.sav")), &sav_bytes)
            .map_err(CoreError::Io)?;
        if let Some(dps_bytes) = dps_bytes {
            std::fs::write(players_dir.join(format!("{stem}_dps.sav")), &dps_bytes)
                .map_err(CoreError::Io)?;
        }
    }
    write_gps_if_loaded(session, progress)?;
    Ok(())
}

/// Writes a standalone transfer target's `Level.sav` (+ `LevelMeta.sav` when
/// present) and every loaded player's `.sav`/`_dps.sav` to the locations
/// recorded in its `TransferSaveInfo`.
///
/// Deliberately does NOT back up first, unlike `write_steam_modded_save`: the
/// caller (`handlers::tools::handle_transfer_player`) already backs up via
/// `copy_dir_ignoring` before calling this. Backing up here too would double
/// the I/O for no benefit.
pub(crate) fn write_transfer_target_save(
    session: &SaveSession,
    save_info: &psp_core::session::TransferSaveInfo,
) -> Result<(), HandlerError> {
    let level_sav_bytes = session.level_sav_bytes()?;
    if let Some(parent) = save_info.level_sav.parent() {
        std::fs::create_dir_all(parent).map_err(CoreError::Io)?;
    }
    std::fs::write(&save_info.level_sav, &level_sav_bytes).map_err(CoreError::Io)?;

    if let Some(level_meta_path) = &save_info.level_meta {
        if let Some(level_meta_bytes) = session.level_meta_sav_bytes()? {
            std::fs::write(level_meta_path, &level_meta_bytes).map_err(CoreError::Io)?;
        }
    }

    std::fs::create_dir_all(&save_info.players_dir).map_err(CoreError::Io)?;
    for (player_id, (sav_bytes, dps_bytes)) in session.player_sav_bytes()? {
        let stem = save_modded_player_stem(&player_id);
        std::fs::write(
            save_info.players_dir.join(format!("{stem}.sav")),
            &sav_bytes,
        )
        .map_err(CoreError::Io)?;
        if let Some(dps_bytes) = dps_bytes {
            std::fs::write(
                save_info.players_dir.join(format!("{stem}_dps.sav")),
                &dps_bytes,
            )
            .map_err(CoreError::Io)?;
        }
    }
    Ok(())
}

/// Backup root for Steam saves, anchored to the app root (`backups/steam`).
fn steam_backup_base() -> std::path::PathBuf {
    psp_core::paths::app_root().join("backups").join("steam")
}

/// `data` is a bare world-name string, used only by the GamePass branch. It MUST
/// stay `Option<String>`: the frontend sends `null` for Steam saves, and a
/// strict `String` would reject the Steam write with a serde parse error.
/// Dispatches on the loaded save's `kind` — GamePass writes new wgs containers,
/// everything else takes the on-disk Steam path.
pub async fn handle_save_modded_save(
    world_name: Option<String>,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(session) = ctx.session.save.as_ref() else {
        return Err(HandlerError::Other("No save file loaded".to_string()));
    };
    if matches!(session.kind, SaveKind::GamePass { .. }) {
        return save_modded_gamepass_save(&world_name.unwrap_or_default(), ctx).await;
    }
    save_modded_steam_save(ctx).await
}

async fn save_modded_steam_save(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let save_dir = psp_db::settings::get_settings(&ctx.app.db).await?.save_dir;
    let progress = ctx.emitter.progress_sink();

    let Some(session) = ctx.session.save.as_ref() else {
        return Err(HandlerError::Other("No save file loaded".to_string()));
    };
    let SaveKind::Steam { level_path } = &session.kind else {
        // A zip-uploaded (InMemory) session has no on-disk level path to
        // overwrite; disk write-back is only defined for real Steam loads.
        return Err(HandlerError::Other(
            "Only on-disk Steam saves can be written back yet".to_string(),
        ));
    };
    let level_path = level_path.clone();

    write_steam_modded_save(
        session,
        &level_path,
        Path::new(&save_dir),
        &steam_backup_base(),
        &progress,
    )?;

    ctx.emitter.emit(
        MessageType::SaveModdedSave,
        &"Modded save file saved successfully",
    );
    Ok(())
}

/// On failure this emits an `error` frame with plain-string data AND re-raises,
/// so the dispatcher emits its own `{message, trace}` frame too — TWO error
/// frames, not one. The frontend expects both.
async fn save_modded_gamepass_save(
    world_name: &str,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    match write_modded_gamepass_containers(world_name, ctx).await {
        Ok(()) => {
            ctx.emitter
                .emit(MessageType::SaveModdedSave, &"Created modded save");
            Ok(())
        }
        Err(error) => {
            ctx.emitter.emit(
                MessageType::Error,
                &format!("Failed to save gamepass save: {error}"),
            );
            Err(error)
        }
    }
}

/// The write half of `save_modded_gamepass_save`, split out so the two-error
/// failure contract above wraps a single fallible body.
async fn write_modded_gamepass_containers(
    world_name: &str,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    use psp_core::gamepass::format::ContainerIndex;
    use psp_core::gamepass::{store, PlayerSavBytes};

    let selected = ctx
        .session
        .selected_gamepass_save
        .clone()
        .ok_or_else(|| HandlerError::Other("No GamePass save selected".to_string()))?;
    let container_dir = PathBuf::from(psp_db::settings::get_settings(&ctx.app.db).await?.save_dir);
    let progress = ctx.emitter.progress_sink();

    progress("Creating backup of container path...");
    let backup_path =
        store::backup_container_dir(&container_dir, &store::backups_root().join("gamepass"))?;
    progress(&format!("Created backup at: {}", backup_path.display()));

    let mut index = ContainerIndex::read_from_dir(&container_dir)?;
    store::cleanup_container_dir(&mut index, &container_dir)?;
    let original_containers = index.latest_save_containers(&selected.save_id);
    if original_containers.is_empty() {
        return Err(HandlerError::Other(format!(
            "No containers found for save: {}",
            selected.save_id
        )));
    }

    progress("Converting modified save to SAV format...");
    let new_save_id = Uuid::new_v4().as_simple().to_string().to_uppercase();
    let session = ctx.session.save_mut()?;
    let level_bytes = session.level_sav_bytes()?;
    let player_map: std::collections::HashMap<Uuid, PlayerSavBytes> = session
        .player_sav_bytes()?
        .into_iter()
        .map(|(player_uuid, (sav_bytes, dps_bytes))| {
            (
                player_uuid,
                PlayerSavBytes {
                    sav: Some(sav_bytes),
                    dps: dps_bytes,
                },
            )
        })
        .collect();

    // Only re-emit when the user actually changed something.
    let modified_world_option = match (&session.world_option, session.world_option_dirty) {
        (Some(save), true) => Some(psp_core::savio::write_sav_bytes(save)?),
        _ => None,
    };

    progress("Creating new containers for modified save...");
    store::save_modified_gamepass(
        &mut index,
        &container_dir,
        &new_save_id,
        &level_bytes,
        &player_map,
        &original_containers,
        world_name,
        modified_world_option.as_deref(),
    )?;
    progress("Modded save created");
    Ok(())
}

/// `data` is a bare new-name STRING. The old name is read BEFORE the no-save
/// guard, falling back to "Unknown" only when no save is loaded — an empty
/// world_name on a LOADED save is reported as empty, not as "Unknown".
pub async fn handle_rename_world(
    new_world_name: String,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let old_world_name = ctx
        .session
        .save
        .as_ref()
        .map(|session| session.world_name.clone())
        .unwrap_or_else(|| "Unknown".to_string());
    let Some(session) = ctx.session.save.as_mut() else {
        return Err(HandlerError::Other("No save file loaded".to_string()));
    };
    session.set_world_name(&new_world_name)?;
    ctx.emitter.emit(
        MessageType::RenameWorld,
        &format!("World renamed from '{old_world_name}' to '{new_world_name}'"),
    );
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct SaveEditedSavData {
    /// The uesave JSON the editor holds in Monaco.
    pub json: String,
    /// Seeds the "Save As" dialog's filename; the loaded save's name.
    #[serde(default)]
    pub file_name: Option<String>,
}

/// The JSON editor's Save button in desktop mode: the webview ignores browser
/// `<a download>`, so the edited uesave JSON is converted to a `.sav` and
/// written to a native-picked path. Answers on its OWN `save_edited_sav` type
/// for success, cancel, AND soft failure alike — never a bare `error` or
/// `no_file_selected` — so the editor's `sendAndWait`, which correlates by
/// message type, always resolves.
pub async fn handle_save_edited_sav(
    data: SaveEditedSavData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if !ctx.app.config.desktop_mode {
        ctx.emitter.emit(
            MessageType::SaveEditedSav,
            &json!({"error": "Desktop mode required."}),
        );
        return Ok(());
    }

    // Convert BEFORE prompting: a bad edit must not open a save dialog only to
    // fail after the user has already picked a location.
    let sav_bytes = match psp_core::convert::json_to_sav_bytes(data.json.as_bytes()) {
        Ok(bytes) => bytes,
        Err(error) => {
            ctx.emitter.emit(
                MessageType::SaveEditedSav,
                &json!({"error": format!("Invalid save JSON: {error}")}),
            );
            return Ok(());
        }
    };

    let request = crate::desktop_dialogs::FileSaveRequest {
        filter_name: "Save Files",
        filter_extensions: &["sav"],
        suggested_file_name: data
            .file_name
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "modified_save.sav".to_string()),
        initial_directory: None,
    };
    let Some(path) = ctx.app.dialogs.save_file(request).await else {
        ctx.emitter
            .emit(MessageType::SaveEditedSav, &json!({"canceled": true}));
        return Ok(());
    };

    if let Err(error) = std::fs::write(&path, &sav_bytes) {
        ctx.emitter.emit(
            MessageType::SaveEditedSav,
            &json!({"error": format!("Failed to write save file: {error}")}),
        );
        return Ok(());
    }

    ctx.emitter.emit(
        MessageType::SaveEditedSav,
        &json!({
            "message": "Save file written successfully",
            "file_path": path,
        }),
    );
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct ConvertSavFileData {
    /// The frontend sends the file as a JSON array of ints.
    pub file_data: Vec<u8>,
    pub target_type: String,
}

/// JSON output uses the uesave schema.
pub async fn handle_convert_sav_file(
    data: ConvertSavFileData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if data.target_type == "json" {
        let json_text = psp_core::convert::sav_to_json_string(&data.file_data)?;
        ctx.emitter.emit(MessageType::ConvertSavFile, &json_text);
    } else {
        let sav_bytes = psp_core::convert::json_to_sav_bytes(&data.file_data)?;
        let encoded = base64::engine::general_purpose::STANDARD.encode(&sav_bytes);
        ctx.emitter.emit(MessageType::ConvertSavFile, &encoded);
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct UnlockMapData {
    #[serde(default)]
    pub path: Option<String>,
}

/// Every failure is reported by THIS handler as
/// `error {message: "Failed to unlock map: <inner>", trace}` and never bubbles
/// to the dispatcher, so the frontend always sees the prefixed message.
pub async fn handle_unlock_map(
    mut data: UnlockMapData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if ctx.app.config.desktop_mode && data.path.is_none() {
        let Some(selected) = pick_save_file_via_dialog("local_data", ctx).await? else {
            return Ok(()); // canceled; no_file_selected already emitted
        };
        // Deliberately NO update_save_dir here: LocalData.sav does not live in
        // the save dir, so persisting its parent would corrupt select_save's
        // starting directory.
        data.path = Some(selected.to_string_lossy().into_owned());
    }

    if let Err(failure_message) = unlock_map_on_disk(&data) {
        ctx.emitter
            .emit_error(&format!("Failed to unlock map: {failure_message}"), "");
        return Ok(());
    }
    ctx.emitter.emit(
        MessageType::UnlockMap,
        &serde_json::json!({
            "success": true,
            "message": "Map unlocked successfully! Restart the game to see changes.",
        }),
    );
    Ok(())
}

fn unlock_map_on_disk(data: &UnlockMapData) -> Result<(), String> {
    let path = data
        .path
        .as_deref()
        .filter(|candidate| !candidate.is_empty())
        .ok_or("No file path provided")?;
    let file_path = Path::new(path);
    let is_local_data = file_path
        .file_name()
        .map(|name| name == "LocalData.sav")
        .unwrap_or(false);
    if !is_local_data {
        return Err("Please select the LocalData.sav file.".to_string());
    }
    std::fs::copy(file_path, format!("{path}.backup")).map_err(|error| error.to_string())?;
    let local_data = std::fs::read(file_path).map_err(|error| error.to_string())?;
    let outcome =
        psp_core::localdata::unlock_world_map(&local_data).map_err(|error| error.to_string())?;
    std::fs::write(file_path, outcome.sav_bytes).map_err(|error| error.to_string())?;
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
    fn validate_steam_save_directory_finds_optional_world_option() {
        let temp_dir = tempfile::tempdir().unwrap();
        let save_dir = temp_dir.path().join("MyWorld");
        std::fs::create_dir_all(save_dir.join("Players")).unwrap();
        std::fs::write(save_dir.join("Level.sav"), b"L").unwrap();
        std::fs::write(
            save_dir.join("Players").join(format!("{PLAYER_ONE}.sav")),
            b"P",
        )
        .unwrap();
        std::fs::write(save_dir.join("WorldOption.sav"), b"W").unwrap();

        let level_sav_path = save_dir.join("Level.sav");
        let layout = validate_steam_save_directory(&level_sav_path.to_string_lossy()).unwrap();

        assert_eq!(Some(save_dir.join("WorldOption.sav")), layout.world_option);
    }

    #[test]
    fn validate_steam_save_directory_tolerates_missing_world_option() {
        let temp_dir = tempfile::tempdir().unwrap();
        let save_dir = temp_dir.path().join("MyWorld");
        std::fs::create_dir_all(save_dir.join("Players")).unwrap();
        std::fs::write(save_dir.join("Level.sav"), b"L").unwrap();
        std::fs::write(
            save_dir.join("Players").join(format!("{PLAYER_ONE}.sav")),
            b"P",
        )
        .unwrap();

        let level_sav_path = save_dir.join("Level.sav");
        let layout = validate_steam_save_directory(&level_sav_path.to_string_lossy()).unwrap();

        assert_eq!(
            None, layout.world_option,
            "WorldOption is optional, like LevelMeta"
        );
    }

    #[test]
    fn test_discover_player_file_refs_pairs_sav_and_dps_and_skips_invalid_names() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(temp_dir.path().join(format!("{PLAYER_ONE}.sav")), b"x").unwrap();
        std::fs::write(temp_dir.path().join(format!("{PLAYER_ONE}_dps.sav")), b"y").unwrap();
        std::fs::write(temp_dir.path().join("not-a-uuid.sav"), b"z").unwrap();
        std::fs::write(temp_dir.path().join("ignored.txt"), b"w").unwrap();

        let (refs, discovery_order) = discover_player_file_refs(temp_dir.path()).unwrap();
        assert_eq!(1, refs.len());
        let uid: Uuid = PLAYER_ONE.parse().unwrap();
        assert_eq!(vec![uid], discovery_order);
        match &refs[&uid] {
            PlayerFileData::Paths { sav, dps } => {
                assert!(sav.is_some());
                assert!(dps.is_some());
            }
            other => panic!("expected Paths variant, got {other:?}"),
        }
    }

    /// Real-filesystem companion to the synthetic test below: whatever order
    /// `std::fs::read_dir` hands back on this platform, the discovery order must
    /// match it. On NTFS that is name-sorted, which for UUID names coincides
    /// with `Uuid`'s `Ord`, so this alone cannot catch a resort — see
    /// `test_collect_player_file_refs_preserves_a_non_sorted_discovery_order`.
    #[test]
    fn test_discover_player_file_refs_discovery_order_matches_raw_read_dir_order() {
        let temp_dir = tempfile::tempdir().unwrap();
        let uuids = [
            "33333333-3333-3333-3333-333333333333",
            "11111111-1111-1111-1111-111111111111",
            "22222222-2222-2222-2222-222222222222",
        ];
        for uuid in uuids {
            std::fs::write(temp_dir.path().join(format!("{uuid}.sav")), b"x").unwrap();
        }

        let expected_order: Vec<Uuid> = std::fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|dir_entry| dir_entry.ok())
            .filter_map(|dir_entry| {
                dir_entry
                    .path()
                    .file_stem()
                    .and_then(|stem| stem.to_string_lossy().parse::<Uuid>().ok())
            })
            .collect();

        let (_, discovery_order) = discover_player_file_refs(temp_dir.path()).unwrap();
        assert_eq!(expected_order, discovery_order);
    }

    /// Feeds `collect_player_file_refs` a hand-built, deliberately
    /// NON-ascending path sequence, bypassing `read_dir`'s platform-specific
    /// enumeration order. This is the test that catches a regression to
    /// emitting `player_file_refs.keys()` (UUID-sorted) as the wire order.
    #[test]
    fn test_collect_player_file_refs_preserves_a_non_sorted_discovery_order() {
        let highest: Uuid = "ffffffff-ffff-ffff-ffff-ffffffffffff".parse().unwrap();
        let lowest: Uuid = "00000000-0000-0000-0000-000000000000".parse().unwrap();
        let middle: Uuid = "77777777-7777-7777-7777-777777777777".parse().unwrap();
        // Descending: the opposite of BTreeMap<Uuid, _>'s iteration order, so a
        // resort anywhere in the pipeline is visible.
        let paths = vec![
            PathBuf::from(format!("{highest}.sav")),
            PathBuf::from(format!("{middle}.sav")),
            PathBuf::from(format!("{lowest}.sav")),
        ];

        let (refs, discovery_order) = collect_player_file_refs(paths);

        assert_eq!(vec![highest, middle, lowest], discovery_order);
        // Sanity: the map itself is still UUID-sorted, proving the two are
        // genuinely different orderings and this isn't a vacuous check.
        assert_eq!(
            vec![lowest, middle, highest],
            refs.keys().copied().collect::<Vec<_>>()
        );
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

    #[test]
    fn resolve_zip_layout_names_world_option_for_flat_and_nested() {
        let flat = resolve_zip_layout(&["Level.sav".to_string(), "Players/x.sav".to_string()]);
        assert_eq!("WorldOption.sav", flat.world_option_name);

        let nested = resolve_zip_layout(&[
            "MySave/Level.sav".to_string(),
            "MySave/Players/x.sav".to_string(),
        ]);
        assert_eq!("MySave/WorldOption.sav", nested.world_option_name);
    }

    /// Security-critical: a traversal-shaped top-level entry name can never
    /// leave `save_id` holding a '/'. That is what makes `zip_gps_temp_path`
    /// safe, and it breaks the moment `resolve_zip_layout` takes more than the
    /// first path segment.
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

    /// A hostile `save_id` cannot make the GPS temp file escape the OS temp
    /// directory: `Path::join` only escapes its base via a `/`-separated `..`
    /// component, and `save_id` can never contain `/` (test above).
    #[test]
    fn test_zip_gps_temp_path_never_escapes_the_temp_directory() {
        for save_id in ["..", "...", "normal_id", "", "%2e%2e"] {
            let path = zip_gps_temp_path(save_id);
            assert_eq!(std::env::temp_dir(), path.parent().unwrap());
        }
    }

    /// The download zip uses LOWERCASE hex, no dashes.
    #[test]
    fn download_player_stem_is_lowercase() {
        let uid: Uuid = "ABCDEF12-3456-7890-ABCD-EF1234567890".parse().unwrap();
        assert_eq!(
            "abcdef1234567890abcdef1234567890",
            download_player_stem(&uid)
        );
    }

    /// The on-disk Steam write uses UPPERCASE hex, no dashes — the exact
    /// opposite casing from `download_player_stem`. The two must never be
    /// collapsed into one helper.
    #[test]
    fn save_modded_player_stem_is_uppercase() {
        let uid: Uuid = "abcdef12-3456-7890-abcd-ef1234567890".parse().unwrap();
        assert_eq!(
            "ABCDEF1234567890ABCDEF1234567890",
            save_modded_player_stem(&uid)
        );
        // Guard the divergence directly: same uuid, opposite case.
        assert_ne!(save_modded_player_stem(&uid), download_player_stem(&uid));
    }

    #[test]
    fn is_player_save_file_accepts_uuid_saves_and_rejects_junk() {
        assert!(is_player_save_file(
            "00000000000000000000000000000001.sav"
        ));
        assert!(is_player_save_file(
            "0123456789ABCDEFfedcba9876543210.sav"
        ));
        assert!(is_player_save_file(
            "00000000000000000000000000000001_dps.sav"
        ));

        assert!(!is_player_save_file("notes.sav")); // non-hex stem
        assert!(!is_player_save_file("0000.sav")); // too short
        assert!(!is_player_save_file(
            "00000000000000000000000000000001.txt"
        )); // wrong extension
        assert!(!is_player_save_file(
            "0000000000000000000000000000000G.sav"
        )); // non-hex digit
        assert!(!is_player_save_file(
            "00000000000000000000000000000001"
        )); // no extension
        assert!(!is_player_save_file("_dps.sav")); // empty hex stem
    }

    /// `OrderedMap<i32, _>` must round-trip the string-keyed JSON objects the
    /// wire uses for `modified_dps_pals`/`modified_gps_pals`. Parsed from raw
    /// text, not `json!`, so the key genuinely flows through serde_json's
    /// map-key coercion (`"7"` → `7i32`).
    #[test]
    fn int_keyed_ordered_map_deserializes_from_string_keys() {
        let map: OrderedMap<i32, i64> = serde_json::from_str(r#"{"7": 70, "3": 30}"#).unwrap();
        let entries: Vec<(i32, i64)> = map.into_iter().collect();
        assert_eq!(vec![(7, 70), (3, 30)], entries);
    }

    #[test]
    fn copy_dir_recursive_mirrors_nested_tree() {
        let temp_dir = tempfile::tempdir().unwrap();
        let src = temp_dir.path().join("src");
        let nested = src.join("Players");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(src.join("Level.sav"), b"level-bytes").unwrap();
        std::fs::write(nested.join("PLAYER.sav"), b"player-bytes").unwrap();

        let dst = temp_dir.path().join("dst");
        copy_dir_recursive(&src, &dst).unwrap();

        assert_eq!(
            b"level-bytes".to_vec(),
            std::fs::read(dst.join("Level.sav")).unwrap()
        );
        assert_eq!(
            b"player-bytes".to_vec(),
            std::fs::read(dst.join("Players/PLAYER.sav")).unwrap()
        );
    }

    /// A missing `save_dir` must copy nothing and report "skipping backup".
    #[test]
    fn backup_save_directory_skips_and_reports_a_missing_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let backup_base = temp_dir.path().join("backups/steam");
        let absent_save_dir = temp_dir.path().join("does_not_exist");

        let recorded: std::sync::Arc<std::sync::Mutex<Vec<String>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let recorded_for_sink = recorded.clone();
        let progress: ProgressSink = std::sync::Arc::new(move |message: &str| {
            recorded_for_sink.lock().unwrap().push(message.to_string());
        });

        backup_save_directory(&absent_save_dir, &backup_base, &progress).unwrap();

        let messages = recorded.lock().unwrap();
        assert!(messages
            .iter()
            .any(|m| m == "Backing up save directory... 🤓"));
        assert!(messages.iter().any(|m| m
            == &format!(
                "Save directory {} not found, skipping backup",
                absent_save_dir.display()
            )));
        // Nothing was copied: backup_base holds no per-save subdirectory.
        let created: Vec<_> = std::fs::read_dir(&backup_base).unwrap().collect();
        assert!(created.is_empty(), "no backup dir should be created");
    }

    /// Only whitelisted files reach the backup; stray root files, unknown
    /// subfolders, and non-UUID `Players/` entries are excluded.
    #[test]
    fn backup_save_directory_copies_only_whitelisted_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let save_dir = temp_dir.path().join("world");
        let players_dir = save_dir.join("Players");
        std::fs::create_dir_all(&players_dir).unwrap();

        // Whitelisted contents.
        std::fs::write(save_dir.join("Level.sav"), b"level").unwrap();
        std::fs::write(save_dir.join("LevelMeta.sav"), b"meta").unwrap();
        std::fs::write(save_dir.join("LocalData.sav"), b"local").unwrap();
        std::fs::write(save_dir.join("WorldOption.sav"), b"option").unwrap();
        let player = "00000000000000000000000000000001.sav";
        let player_dps = "00000000000000000000000000000001_dps.sav";
        std::fs::write(players_dir.join(player), b"p").unwrap();
        std::fs::write(players_dir.join(player_dps), b"pd").unwrap();

        // Decoys that must NOT be backed up.
        std::fs::write(save_dir.join("readme.txt"), b"junk").unwrap();
        std::fs::create_dir_all(save_dir.join("mods")).unwrap();
        std::fs::write(save_dir.join("mods/foo.pak"), b"pak").unwrap();
        std::fs::write(players_dir.join("notes.sav"), b"junk").unwrap();
        std::fs::create_dir_all(players_dir.join("junk")).unwrap();
        std::fs::write(players_dir.join("junk/inner.sav"), b"junk").unwrap();

        let backup_base = temp_dir.path().join("backups/steam");
        backup_save_directory(
            &save_dir,
            &backup_base,
            &psp_core::progress::null_progress(),
        )
        .unwrap();

        let backup_dirs: Vec<PathBuf> = std::fs::read_dir(&backup_base)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .collect();
        assert_eq!(1, backup_dirs.len(), "one backup dir expected");
        let backup = &backup_dirs[0];

        // Whitelist present.
        assert!(backup.join("Level.sav").is_file());
        assert!(backup.join("LevelMeta.sav").is_file());
        assert!(backup.join("LocalData.sav").is_file());
        assert!(backup.join("WorldOption.sav").is_file());
        assert!(backup.join("Players").join(player).is_file());
        assert!(backup.join("Players").join(player_dps).is_file());

        // Decoys absent.
        assert!(!backup.join("readme.txt").exists());
        assert!(!backup.join("mods").exists());
        assert!(!backup.join("Players").join("notes.sav").exists());
        assert!(!backup.join("Players").join("junk").exists());
    }

    /// Hermetic full write-path test: the `world1` fixture is copied into a
    /// `TempDir` and BOTH the session level_path and save_dir point at that
    /// copy, so nothing can touch the user's real save_dir, the committed
    /// fixtures, or the process CWD.
    #[test]
    fn write_steam_modded_save_backs_up_and_writes_into_a_temp_dir() {
        let fixture = fixture_world1_dir();
        let temp_dir = tempfile::tempdir().unwrap();
        let save_dir = temp_dir.path().join("world");
        copy_dir_recursive(&fixture, &save_dir).unwrap();

        let level_path = save_dir.join("Level.sav");
        let level_bytes = std::fs::read(&level_path).unwrap();
        let meta_bytes = std::fs::read(save_dir.join("LevelMeta.sav")).unwrap();

        let session = SaveSession::load(
            SaveKind::Steam {
                level_path: level_path.clone(),
            },
            level_path.to_string_lossy().into_owned(),
            "steam",
            &level_bytes,
            Some(&meta_bytes),
            None,
            BTreeMap::new(),
            None,
            true,
            &psp_core::progress::null_progress(),
        )
        .unwrap();

        // Overwrite Level.sav/LevelMeta.sav with sentinels first, so a
        // successful write is observable as their disappearance.
        std::fs::write(&level_path, b"STALE").unwrap();
        std::fs::write(save_dir.join("LevelMeta.sav"), b"STALE").unwrap();

        let backup_base = temp_dir.path().join("backups/steam");
        write_steam_modded_save(
            &session,
            &level_path,
            &save_dir,
            &backup_base,
            &psp_core::progress::null_progress(),
        )
        .unwrap();

        // Both files were re-serialized: the STALE sentinels are gone.
        let written_level = std::fs::read(&level_path).unwrap();
        let written_meta = std::fs::read(save_dir.join("LevelMeta.sav")).unwrap();
        assert_ne!(b"STALE".to_vec(), written_level);
        assert_ne!(b"STALE".to_vec(), written_meta);
        assert!(!written_level.is_empty());
        assert!(!written_meta.is_empty());

        // Exactly one timestamped backup dir, and it captured the pre-write
        // (STALE) Level.sav — proving the backup ran BEFORE the overwrite.
        let backup_dirs: Vec<PathBuf> = std::fs::read_dir(&backup_base)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .collect();
        assert_eq!(1, backup_dirs.len(), "one backup dir expected");
        assert_eq!(
            b"STALE".to_vec(),
            std::fs::read(backup_dirs[0].join("Level.sav")).unwrap(),
            "backup must contain the pre-overwrite Level.sav"
        );
    }

    fn fixture_world1_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/saves/world1")
    }
}
