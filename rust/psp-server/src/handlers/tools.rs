//! Standalone conversion utilities (Task 3E-1), ported from
//! `ws/handlers/steam_id_handler.py`. Unlike the other handler modules this
//! one does not touch `ctx.session`/`ctx.app` at all -- `convert_steam_id` is
//! a pure input->output tool available with no save file loaded.
//!
//! Task 3E-3 adds the player-transfer WS surface (`load_source_save`/
//! `get_source_players`/`transfer_player`/`unload_source_save`), a port of
//! `ws/handlers/transfer_handler.py`. Unlike `convert_steam_id`, these DO
//! touch `ctx.session` (`source`/`transfer_target`, and `save` as the
//! transfer_player fallback target) and reuse Phase-1/2's Steam-load and
//! disk-write plumbing from `handlers::save_file` rather than reinventing it.
//!
//! Task 3E-4 adds `swap_player_uids`, a port of
//! `ws/handlers/uid_swap_handler.py` over `psp_core::domain::uid_swap`.
//! Unlike `transfer_player` (two distinct `SaveSession`s), this operates on
//! the single main `ctx.session.save`.

use std::path::PathBuf;

use psp_core::error::CoreError;
use psp_core::progress::ProgressSink;
use psp_core::session::{SaveKind, SaveSession, TransferSaveInfo, TransferTarget};
use uuid::Uuid;

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::handlers::save_file;
use crate::messages::MessageType;

#[derive(Debug, serde::Deserialize)]
pub struct ConvertSteamIdData {
    pub steam_input: String,
}

pub async fn handle_convert_steam_id(
    data: ConvertSteamIdData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    use psp_core::steam_id as sid;
    let raw = data.steam_input.as_str();
    let payload = if sid::is_palworld_uid(raw) {
        match sid::parse_palworld_uid(raw) {
            Ok(palworld_uid) => serde_json::json!({
                "palworld_uid": palworld_uid.to_string().to_uppercase(),
                "nosteam_uid": sid::player_uid_to_nosteam(palworld_uid).to_uppercase(),
                "from_uid": true,
            }),
            // Near-unreachable: `is_palworld_uid` already validated `raw` as
            // 32-hex / dashed-hex, and every such string parses as a UUID, so
            // Python never actually hits `parse_palworld_uid`'s error path
            // either. Kept only so the branch is total; the emitted text is not
            // load-bearing (no Python fixture exercises it).
            Err(error) => serde_json::json!({ "error": error.to_string() }),
        }
    } else {
        match sid::parse_steam_input(raw) {
            Ok(steam_id) => {
                let palworld_uid = sid::steam_id_to_player_uid(steam_id);
                serde_json::json!({
                    "palworld_uid": palworld_uid.to_string().to_uppercase(),
                    "nosteam_uid": sid::player_uid_to_nosteam(palworld_uid).to_uppercase(),
                })
            }
            // Emit the error's own message verbatim. For a non-numeric input
            // this is Python's `int()` text ("invalid literal for int() with
            // base 10: '<processed>'"); for a vanity URL it is the distinct
            // VanityUrl message. steam_id_handler.py:39-42's `str(e) if str(e)
            // else <generic>` fallback is dead code in real Python (`int()`'s
            // ValueError message is never empty), so no generic remap here.
            Err(error) => serde_json::json!({ "error": error.to_string() }),
        }
    };
    ctx.emitter.emit(MessageType::ConvertSteamId, &payload);
    Ok(())
}

// ---------------------------------------------------------------------------
// Player transfer (Task 3E-3) -- port of ws/handlers/transfer_handler.py.
// ---------------------------------------------------------------------------

fn default_role() -> String {
    "source".to_string()
}

fn bool_true() -> bool {
    true
}

#[derive(Debug, serde::Deserialize)]
pub struct LoadSourceSaveData {
    #[serde(rename = "type")]
    pub save_type: String,
    pub path: String,
    #[serde(default = "default_role")]
    pub role: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct TransferPlayerData {
    pub source_player_uid: Uuid,
    pub target_player_uid: Option<Uuid>,
    #[serde(default = "bool_true")]
    pub transfer_character: bool,
    #[serde(default = "bool_true")]
    pub transfer_inventory: bool,
    #[serde(default = "bool_true")]
    pub transfer_pals: bool,
    #[serde(default = "bool_true")]
    pub transfer_tech: bool,
    #[serde(default = "bool_true")]
    pub transfer_appearance: bool,
}

/// A save's player summaries, wire-shaped exactly like `get_player_summaries`
/// (`handlers::save_file::emit_summary_messages` emits the same field
/// directly) -- `SaveSession::player_summaries` is a plain `BTreeMap<Uuid,
/// PlayerSummary>` field, not a method, so this just re-serializes it.
fn summaries_json(save: &SaveSession) -> serde_json::Value {
    serde_json::to_value(&save.player_summaries).expect("player summaries always serialize")
}

/// Port of `transfer_handler.py::_load_steam_save`: resolves a directory-or-
/// Level.sav `save_path` to its Steam save layout, reusing the EXACT same
/// `FileManager.validate_steam_save_directory` / `get_player_save_paths`
/// helpers `handle_select_save` uses (`handlers::save_file`) so a bad
/// directory produces the identical error string `select_save` would. Returns
/// `Err(message)` (not `HandlerError`) since every failure here becomes a
/// SOFT `{"error": ...}` response, never the hard WS `error` frame -- see
/// `handle_load_source_save`'s try/catch-shaped `match`.
///
/// The caller emits its own `"Loading {label} Level.sav..."` progress frame
/// (see `handle_load_source_save`) BEFORE the `SaveSession::load` below, and
/// passes `emit_top_level_progress = false` to that load so it does NOT ALSO
/// emit the generic `"Loading Level.sav..."` frame. This mirrors Python's
/// transfer path exactly: `_load_steam_save` (transfer_handler.py:38) calls
/// `SaveManager.load_sav_files` DIRECTLY, bypassing
/// `AppState.process_save_files` -- the only place the generic frame lives
/// (state.py:69). `select_save`/`load_zip` go through `process_save_files`
/// and keep the generic frame (their sequence is Phase-1-parity-verified);
/// emitting it here too would put ONE extra frame on the transfer wire that
/// Python never sends.
fn load_steam_save_for_transfer(
    save_path: &str,
    label: &str,
    progress: &ProgressSink,
) -> Result<(SaveSession, TransferSaveInfo), String> {
    let mut level_path = PathBuf::from(save_path);
    if level_path.is_dir() {
        level_path = level_path.join("Level.sav");
    }
    let layout = save_file::validate_steam_save_directory(&level_path.to_string_lossy())
        .map_err(|error| error.to_string())?;

    progress(&format!("Loading {label} Level.sav..."));

    let level_sav_bytes =
        std::fs::read(&layout.level_sav).map_err(|error| CoreError::Io(error).to_string())?;
    let level_meta_bytes = match &layout.level_meta {
        Some(meta_path) => {
            Some(std::fs::read(meta_path).map_err(|error| CoreError::Io(error).to_string())?)
        }
        None => None,
    };
    let (player_file_refs, _discovery_order) =
        save_file::discover_player_file_refs(&layout.players_dir)
            .map_err(|error| error.to_string())?;

    let save_dir = layout
        .level_sav
        .parent()
        .expect("Level.sav always has a parent directory")
        .to_path_buf();
    let save_info = TransferSaveInfo {
        level_sav: layout.level_sav.clone(),
        level_meta: layout.level_meta.clone(),
        players_dir: layout.players_dir.clone(),
        save_dir,
    };

    let session = SaveSession::load(
        SaveKind::Steam {
            level_path: layout.level_sav.clone(),
        },
        level_path.to_string_lossy().into_owned(),
        "steam",
        &level_sav_bytes,
        level_meta_bytes.as_deref(),
        player_file_refs,
        layout.global_pal_storage_sav.clone(),
        // false: the transfer path bypasses process_save_files (see this
        // function's doc comment); the "Loading {label} Level.sav..." frame
        // above is the only leading frame Python's transfer path emits.
        false,
        progress,
    )
    .map_err(|error| error.to_string())?;

    Ok((session, save_info))
}

/// Port of `load_source_save_handler`. `role` selects whether the loaded save
/// becomes `ctx.session.source` (the default, "source") or a standalone
/// `ctx.session.transfer_target` ("target"). Every failure -- unsupported
/// type, no desktop window for `"__select__"`, or a load error -- responds
/// with `{"error": ...}` on this same wire type, never the hard `error` frame
/// (Python wraps the whole body in `try/except Exception`).
pub async fn handle_load_source_save(
    data: LoadSourceSaveData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if data.save_type != "steam" {
        ctx.emitter.emit(
            MessageType::LoadSourceSave,
            &serde_json::json!({"error": "Only Steam saves are supported."}),
        );
        return Ok(());
    }
    if data.path == "__select__" {
        // `_prompt_folder` (transfer_handler.py:70-81): Phase 5 wires the
        // native folder dialog behind desktop_mode. No desktop window exists
        // yet, so this always takes Python's `if not window: raise` branch.
        ctx.emitter.emit(
            MessageType::LoadSourceSave,
            &serde_json::json!({"error": "Desktop mode required for file selection."}),
        );
        return Ok(());
    }

    let is_target = data.role == "target";
    let label = if is_target { "target" } else { "source" };
    let progress = ctx.emitter.progress_sink();

    match load_steam_save_for_transfer(&data.path, label, &progress) {
        Ok((session, save_info)) => {
            let player_count = session.player_summaries.len();
            let world_name = session.world_name.clone();
            if is_target {
                ctx.session.transfer_target = Some(TransferTarget { session, save_info });
            } else {
                ctx.session.source = Some(session);
            }
            ctx.emitter.emit(
                MessageType::LoadSourceSave,
                &serde_json::json!({
                    "success": true,
                    "role": data.role,
                    "player_count": player_count,
                    "world_name": world_name,
                }),
            );
        }
        Err(message) => {
            ctx.emitter.emit(
                MessageType::LoadSourceSave,
                &serde_json::json!({"error": message}),
            );
        }
    }
    Ok(())
}

/// Port of `get_source_players_handler`: `source`/`target` each report their
/// live `player_summaries` map (refreshed in place whenever a transfer
/// mutates the underlying `SaveSession` -- `transfer_player` ends with
/// `target.rebuild_player_caches()`, so no separate refresh step is needed
/// here), or `{}` when that role has nothing loaded.
pub async fn handle_get_source_players(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let source = ctx
        .session
        .source
        .as_ref()
        .map(summaries_json)
        .unwrap_or_else(|| serde_json::json!({}));
    let target = ctx
        .session
        .transfer_target
        .as_ref()
        .map(|target| summaries_json(&target.session))
        .unwrap_or_else(|| serde_json::json!({}));
    ctx.emitter.emit(
        MessageType::GetSourcePlayers,
        &serde_json::json!({"source": source, "target": target}),
    );
    Ok(())
}

/// `shutil.copytree(..., ignore=ignore_patterns("backups"))` equivalent
/// (`transfer_handler.py:207-211`): recursively copies `from` into `to`,
/// skipping any entry (file or directory) literally named `ignore_name`.
fn copy_dir_ignoring(
    from: &std::path::Path,
    to: &std::path::Path,
    ignore_name: &str,
) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(to)?;
    for dir_entry in std::fs::read_dir(from)? {
        let dir_entry = dir_entry?;
        let name = dir_entry.file_name();
        if name.to_string_lossy() == ignore_name {
            continue;
        }
        let destination = to.join(&name);
        if dir_entry.file_type()?.is_dir() {
            copy_dir_ignoring(&dir_entry.path(), &destination, ignore_name)?;
        } else {
            std::fs::copy(dir_entry.path(), destination)?;
        }
    }
    Ok(())
}

/// Port of `transfer_player_handler`. Target resolution order matches Python
/// exactly: `transfer_target` (a standalone-loaded save) first, falling back
/// to the main `save` session -- `target_transfer_save or app_state.save_file`
/// (`transfer_handler.py:157`). Missing target is checked BEFORE missing
/// source, matching Python's check order. A successful transfer into a
/// standalone target additionally auto-saves it to disk (no separate "save"
/// button exists for it): backs up the target's save directory, then writes
/// `Level.sav`/`LevelMeta.sav`/`Players/*.sav` via
/// `handlers::save_file::write_transfer_target_save`, and extends the result
/// with `saved_to`.
pub async fn handle_transfer_player(
    data: TransferPlayerData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let has_standalone_target = ctx.session.transfer_target.is_some();
    if !has_standalone_target && ctx.session.save.is_none() {
        ctx.emitter.emit(
            MessageType::TransferPlayer,
            &serde_json::json!({"error": "No target save loaded."}),
        );
        return Ok(());
    }
    if ctx.session.source.is_none() {
        ctx.emitter.emit(
            MessageType::TransferPlayer,
            &serde_json::json!({"error": "No source save loaded."}),
        );
        return Ok(());
    }

    let options = psp_core::transfer::TransferOptions {
        transfer_character: data.transfer_character,
        transfer_inventory: data.transfer_inventory,
        transfer_pals: data.transfer_pals,
        transfer_tech: data.transfer_tech,
        transfer_appearance: data.transfer_appearance,
    };
    let progress = ctx.emitter.progress_sink();

    let transfer_result = {
        let source = ctx.session.source.as_mut().expect("checked Some above");
        let target: &mut SaveSession = if has_standalone_target {
            &mut ctx
                .session
                .transfer_target
                .as_mut()
                .expect("checked Some above")
                .session
        } else {
            ctx.session.save.as_mut().expect("checked Some above")
        };
        psp_core::transfer::transfer_player(
            source,
            target,
            data.source_player_uid,
            data.target_player_uid,
            &options,
            &progress,
        )
    };

    let (succeeded, mut result) = match transfer_result {
        Ok(()) => (true, serde_json::json!({"success": true})),
        Err(psp_core::transfer::TransferError::Rejected(message)) => {
            (false, serde_json::json!({"error": message}))
        }
        Err(psp_core::transfer::TransferError::Core(error)) => {
            (false, serde_json::json!({"error": error.to_string()}))
        }
    };

    if succeeded && has_standalone_target {
        // Auto-save the standalone target to disk -- it has no separate
        // "save" button (transfer_handler.py:194-219).
        ctx.emitter.emit(
            MessageType::ProgressMessage,
            &"Saving modified target save to disk...",
        );
        let target = ctx
            .session
            .transfer_target
            .as_ref()
            .expect("checked Some above at the top of this handler");
        // Python's `time.strftime` (no explicit tz) uses LOCAL time, matched
        // here with `chrono::Local` -- same convention as the existing
        // `backup_save_directory` helper this mirrors.
        let timestamp = chrono::Local::now().format("%Y-%m-%d-%H-%M-%S").to_string();
        let backup_path = target
            .save_info
            .save_dir
            .join("backups")
            .join("transfer")
            .join(format!("backup_{timestamp}"));
        if !backup_path.exists() {
            copy_dir_ignoring(&target.save_info.save_dir, &backup_path, "backups")
                .map_err(|error| HandlerError::Other(error.to_string()))?;
        }
        save_file::write_transfer_target_save(&target.session, &target.save_info)?;
        ctx.emitter.emit(
            MessageType::ProgressMessage,
            &"Target save written to disk.",
        );
        result["saved_to"] = serde_json::json!(target.save_info.save_dir.to_string_lossy());
    }

    ctx.emitter.emit(MessageType::TransferPlayer, &result);
    Ok(())
}

/// Port of `unload_source_save_handler`: clears both the transfer source and
/// any standalone transfer target.
pub async fn handle_unload_source_save(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    ctx.session.source = None;
    ctx.session.transfer_target = None;
    ctx.emitter.emit(
        MessageType::UnloadSourceSave,
        &serde_json::json!({"success": true}),
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Player UID swap (Task 3E-4) -- port of ws/handlers/uid_swap_handler.py.
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
pub struct SwapPlayerUidsData {
    pub old_player_uid: Uuid,
    pub new_player_uid: Uuid,
}

/// Port of `uid_swap_handler.py`. No save loaded -> `{"error": "No save file
/// loaded."}` on this SAME `swap_player_uids` wire type (a soft response,
/// like every other rejection here -- never the hard WS `error` frame); a
/// soft rejection from `SaveSession::swap_player_uids` (same player,
/// missing player, invalid SaveData) -> `{"error": message}`; a genuine
/// `CoreError` -> `{"error": error.to_string()}`; success -> `{"success":
/// true}`. No separate summary-refresh step is needed here -- `
/// swap_player_uids`'s own trailing `rebuild_player_caches()` call already
/// recomputes `session.player_summaries`/`guild_summaries` in place, which
/// `sync_app_state`/`get_player_summaries` read on demand.
pub async fn handle_swap_player_uids(
    data: SwapPlayerUidsData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(save) = ctx.session.save.as_mut() else {
        ctx.emitter.emit(
            MessageType::SwapPlayerUids,
            &serde_json::json!({"error": "No save file loaded."}),
        );
        return Ok(());
    };
    let progress = ctx.emitter.progress_sink();
    let result = match save.swap_player_uids(data.old_player_uid, data.new_player_uid, &progress) {
        Ok(()) => serde_json::json!({"success": true}),
        Err(psp_core::transfer::TransferError::Rejected(message)) => {
            serde_json::json!({"error": message})
        }
        Err(psp_core::transfer::TransferError::Core(error)) => {
            serde_json::json!({"error": error.to_string()})
        }
    };
    ctx.emitter.emit(MessageType::SwapPlayerUids, &result);
    Ok(())
}
