//! Standalone tools: `convert_steam_id` (a pure input->output tool that works
//! with no save loaded), the player-transfer surface (`load_source_save` /
//! `get_source_players` / `transfer_player` / `unload_source_save`, operating
//! on `ctx.session.source` and `ctx.session.transfer_target`),
//! `swap_player_uids` (operating on the main `ctx.session.save`), and the
//! raw-data inspector.

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
            // 32-hex / dashed-hex, and every such string parses as a UUID.
            // Present only to make the branch total.
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
            // The error's own message goes on the wire verbatim (a vanity URL
            // and a malformed number are told apart by their text alone).
            Err(error) => serde_json::json!({ "error": error.to_string() }),
        }
    };
    ctx.emitter.emit(MessageType::ConvertSteamId, &payload);
    Ok(())
}

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

/// A save's player summaries, wire-shaped exactly like `get_player_summaries`.
fn summaries_json(save: &SaveSession) -> serde_json::Value {
    serde_json::to_value(&save.player_summaries).expect("player summaries always serialize")
}

/// Resolves a directory-or-Level.sav `save_path` to its Steam layout, reusing
/// `handlers::save_file`'s validation and discovery helpers so a bad directory
/// produces the identical error string `select_save` would. Returns
/// `Err(message)`, not `HandlerError`: every failure here becomes a SOFT
/// `{"error": ...}` response, never the hard WS `error` frame.
///
/// The caller emits its own `"Loading {label} Level.sav..."` progress frame, so
/// the load below is told NOT to also emit the generic `"Loading Level.sav..."`
/// one — that would put an extra frame on the transfer wire.
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
        None,
        player_file_refs,
        layout.global_pal_storage_sav.clone(),
        // The "Loading {label} Level.sav..." frame above is the only leading
        // frame the transfer path emits; suppress the generic one.
        false,
        progress,
    )
    .map_err(|error| error.to_string())?;

    Ok((session, save_info))
}

async fn resolve_transfer_source_path(ctx: &mut HandlerCtx<'_>) -> Result<Option<String>, String> {
    let saved_dir = psp_db::settings::saved_save_dir(&ctx.app.db)
        .await
        .map_err(|error| error.to_string())?;
    let request = crate::desktop_dialogs::dialog_request_for("steam", saved_dir.as_deref());
    let Some(selected) = ctx.app.dialogs.pick_file(request).await else {
        return Ok(None);
    };
    crate::desktop_dialogs::validate_selected_file(
        "steam",
        &selected,
        &crate::desktop_dialogs::application_root(),
    )?;

    if let Some(parent_dir) = selected.parent() {
        psp_db::settings::update_save_dir(&ctx.app.db, &parent_dir.to_string_lossy())
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(Some(selected.to_string_lossy().into_owned()))
}

/// `role` selects whether the loaded save becomes `ctx.session.source` (the
/// default) or a standalone `ctx.session.transfer_target`. EVERY failure answers
/// `{"error": ...}` under this same wire type, never the hard `error` frame —
/// the transfer UI correlates both outcomes to this request.
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
    let resolved_path = if data.path == "__select__" {
        if !ctx.app.config.desktop_mode {
            ctx.emitter.emit(
                MessageType::LoadSourceSave,
                &serde_json::json!({"error": "Desktop mode required for file selection."}),
            );
            return Ok(());
        }
        match resolve_transfer_source_path(ctx).await {
            Ok(Some(path)) => path,
            Ok(None) => {
                ctx.emitter.emit(
                    MessageType::LoadSourceSave,
                    &serde_json::json!({"canceled": true}),
                );
                return Ok(());
            }
            Err(message) => {
                ctx.emitter
                    .emit(MessageType::LoadSourceSave, &serde_json::json!({"error": message}));
                return Ok(());
            }
        }
    } else {
        data.path.clone()
    };

    let is_target = data.role == "target";
    let label = if is_target { "target" } else { "source" };
    let progress = ctx.emitter.progress_sink();

    match load_steam_save_for_transfer(&resolved_path, label, &progress) {
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

/// `source` and `target` each report their live `player_summaries`, or `{}`
/// when that role has nothing loaded — both keys are always present, so the
/// frontend never has to null-check them.
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

/// Recursively copies `from` into `to`, skipping any entry (file or directory)
/// literally named `ignore_name`, which keeps a backup dir out of its own
/// backup.
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

/// Target resolution order: a standalone `transfer_target` first, falling back
/// to the main `save` session. A missing target is reported BEFORE a missing
/// source.
///
/// A successful transfer into a standalone target ALSO auto-saves it to disk
/// (there is no separate "save" button for it): backup first, then write, then
/// extend the result with `saved_to`.
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
        ctx.emitter.emit(
            MessageType::ProgressMessage,
            &"Saving modified target save to disk...",
        );
        let target = ctx
            .session
            .transfer_target
            .as_ref()
            .expect("checked Some above at the top of this handler");
        // Local time, matching `backup_save_directory`'s naming convention so
        // both backup roots sort together for the user.
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

/// Clears both the transfer source and any standalone transfer target.
pub async fn handle_unload_source_save(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    ctx.session.source = None;
    ctx.session.transfer_target = None;
    ctx.emitter.emit(
        MessageType::UnloadSourceSave,
        &serde_json::json!({"success": true}),
    );
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct SwapPlayerUidsData {
    pub old_player_uid: Uuid,
    pub new_player_uid: Uuid,
}

/// EVERY outcome answers under `swap_player_uids` — success as
/// `{"success": true}`, any rejection as `{"error": message}`, never the hard
/// WS `error` frame. `SaveSession::swap_player_uids` rebuilds the player caches
/// itself, so no summary refresh is needed here.
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

#[derive(Debug, serde::Deserialize)]
pub struct GetRawDataData {
    pub guild_id: Option<Uuid>,
    pub player_id: Option<Uuid>,
    pub pal_id: Option<Uuid>,
    pub base_id: Option<Uuid>,
    pub item_container_id: Option<Uuid>,
    pub character_container_id: Option<Uuid>,
    #[serde(default)]
    pub level: bool,
}

/// The six ids are tried in order (guild -> player -> pal -> base ->
/// item_container -> character_container), falling back to `level`. No save
/// loaded, an unresolved id, or no field set at all each answer `{}` rather
/// than an error.
pub async fn handle_get_raw_data(
    data: GetRawDataData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    use psp_core::domain::RawTarget;
    let target = if let Some(id) = data.guild_id {
        Some(RawTarget::Guild(id))
    } else if let Some(id) = data.player_id {
        Some(RawTarget::Player(id))
    } else if let Some(id) = data.pal_id {
        Some(RawTarget::Pal(id))
    } else if let Some(id) = data.base_id {
        Some(RawTarget::Base(id))
    } else if let Some(id) = data.item_container_id {
        Some(RawTarget::ItemContainer(id))
    } else if let Some(id) = data.character_container_id {
        Some(RawTarget::CharacterContainer(id))
    } else if data.level {
        Some(RawTarget::Level)
    } else {
        None
    };
    let payload = target
        .and_then(|target| {
            ctx.session
                .save
                .as_ref()
                .and_then(|save| save.raw_json_for(target))
        })
        .unwrap_or_else(|| serde_json::json!({}));
    ctx.emitter.emit(MessageType::GetRawData, &payload);
    Ok(())
}
