//! Gamepass container handlers: scan / delete-save / delete-player /
//! rename-world, plus the gamepass load path `select_gamepass_save`.
//!
//! The four container-MANAGEMENT handlers target the REAL install via
//! `store::find_container_dir()` and never touch `ctx.session` — container
//! management is independent of any loaded save. `handle_select_gamepass_save`
//! is the exception on both counts: it takes the container dir from
//! `settings.save_dir` (the desktop dialog's chosen path) and loads the save
//! into `ctx.session.save`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use psp_core::dto::gamepass::GamepassSaveData;
use psp_core::dto::ordered_map::OrderedMap;
use psp_core::gamepass::convert as gamepass_convert;
use psp_core::gamepass::format::{guid_file_name, ContainerIndex};
use psp_core::gamepass::{scan, store};
use psp_core::session::{PlayerFileData, SaveKind, SaveSession};

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;

/// Wire shape of `scan_gamepass_saves`'s response. `saves` is an `OrderedMap`
/// so it serializes as a JSON object in scan (insertion) order, which the
/// frontend renders directly.
#[derive(Debug, Serialize)]
struct ScanGamepassSavesResponse {
    saves: OrderedMap<String, GamepassSaveData>,
    container_path: Option<String>,
}

/// A missing GamePass install is a normal empty response
/// (`{"saves": {}, "container_path": null}`), not an `error` frame — the
/// frontend treats it as "no gamepass saves here", not as a failure.
pub async fn handle_scan_gamepass_saves(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let container_dir = match store::find_container_dir() {
        Ok(dir) => dir,
        Err(_) => {
            ctx.emitter.emit(
                MessageType::ScanGamepassSaves,
                &ScanGamepassSavesResponse {
                    saves: OrderedMap::new(),
                    container_path: None,
                },
            );
            return Ok(());
        }
    };
    let saves = scan::scan_saves(&container_dir)?;
    ctx.emitter.emit(
        MessageType::ScanGamepassSaves,
        &ScanGamepassSavesResponse {
            saves,
            container_path: Some(container_dir.to_string_lossy().to_string()),
        },
    );
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct DeleteGamepassSaveData {
    pub save_id: String,
}

pub async fn handle_delete_gamepass_save(
    data: DeleteGamepassSaveData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let container_dir = match store::find_container_dir() {
        Ok(dir) => dir,
        Err(error) => {
            ctx.emitter.emit(
                MessageType::DeleteGamepassSave,
                &serde_json::json!({
                    "error": format!("Could not find GamePass installation: {error}")
                }),
            );
            return Ok(());
        }
    };
    let backups = store::backups_root().join("gamepass");
    let removed_count = store::delete_save_containers(&container_dir, &data.save_id, &backups)?;
    if removed_count == 0 {
        ctx.emitter.emit(
            MessageType::DeleteGamepassSave,
            &serde_json::json!({
                "error": format!("No containers found for save: {}", data.save_id)
            }),
        );
        return Ok(());
    }
    ctx.emitter.emit(
        MessageType::DeleteGamepassSave,
        &serde_json::json!({
            "message": format!("Deleted save with {removed_count} containers")
        }),
    );
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct DeleteGamepassPlayerData {
    pub save_id: String,
    /// Player UUID, uppercase hex, no dashes (as it appears in container names).
    pub player_id: String,
}

pub async fn handle_delete_gamepass_player(
    data: DeleteGamepassPlayerData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let container_dir = match store::find_container_dir() {
        Ok(dir) => dir,
        Err(error) => {
            ctx.emitter.emit(
                MessageType::DeleteGamepassPlayer,
                &serde_json::json!({
                    "error": format!("Could not find GamePass installation: {error}")
                }),
            );
            return Ok(());
        }
    };
    let backups = store::backups_root().join("gamepass");
    let removed_count =
        store::delete_player_containers(&container_dir, &data.save_id, &data.player_id, &backups)?;
    if removed_count == 0 {
        ctx.emitter.emit(
            MessageType::DeleteGamepassPlayer,
            &serde_json::json!({
                "error": format!("No containers found for player: {}", data.player_id)
            }),
        );
        return Ok(());
    }
    ctx.emitter.emit(
        MessageType::DeleteGamepassPlayer,
        &serde_json::json!({
            "message": format!("Deleted player {}", data.player_id)
        }),
    );
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct RenameGamepassWorldData {
    pub save_id: String,
    pub new_name: String,
}

/// Appends a brand-new LevelMeta container rather than editing in place;
/// `ContainerIndex::latest_save_containers`'s seq/mtime rule then makes the
/// new container win on every subsequent read.
pub async fn handle_rename_gamepass_world(
    data: RenameGamepassWorldData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let container_dir = match store::find_container_dir() {
        Ok(dir) => dir,
        Err(error) => {
            ctx.emitter.emit(
                MessageType::RenameGamepassWorld,
                &serde_json::json!({
                    "error": format!("Could not find GamePass installation: {error}")
                }),
            );
            return Ok(());
        }
    };
    store::backup_container_dir(&container_dir, &store::backups_root().join("gamepass"))?;
    let mut index = ContainerIndex::read_from_dir(&container_dir)?;
    let latest = index.latest_save_containers(&data.save_id);

    let Some(level_meta_entry) = latest.get("LevelMeta") else {
        ctx.emitter.emit(
            MessageType::RenameGamepassWorld,
            &serde_json::json!({
                "error": format!("No LevelMeta found for save: {}", data.save_id)
            }),
        );
        return Ok(());
    };
    let Some((_seq, meta_bytes)) = store::read_first_blob(&container_dir, level_meta_entry)? else {
        ctx.emitter.emit(
            MessageType::RenameGamepassWorld,
            &serde_json::json!({"error": "Could not read LevelMeta data"}),
        );
        return Ok(());
    };

    let renamed_meta = scan::set_world_name_in_level_meta(&meta_bytes, &data.new_name)?;
    let new_entry = store::create_container(
        &container_dir,
        &data.save_id,
        &renamed_meta,
        "Data",
        "LevelMeta",
    )?;
    index.containers.push(new_entry);
    index.write_to_dir(&container_dir)?;

    ctx.emitter.emit(
        MessageType::RenameGamepassWorld,
        &serde_json::json!({
            "message": format!("World renamed to '{}'", data.new_name)
        }),
    );
    Ok(())
}

/// The message data is a BARE save-id string. Unlike the scan/delete/rename
/// handlers this one loads a save into `ctx.session.save`, so the container
/// directory comes from `settings.save_dir` (set by the desktop file dialog),
/// NOT `find_container_dir()`.
///
/// Silent-return contract: a missing Level or LevelMeta *container entry*
/// returns `Ok(())` with NO frame at all, whereas a missing Level *payload* or
/// an empty player set errors (surfacing an `error` frame). On success emits
/// `loaded_save_files` (`type: "gamepass"`, `has_gps: false`) then
/// `get_player_summaries` / `get_guild_summaries`.
pub async fn handle_select_gamepass_save(
    save_id: String,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    // Recorded before the silent-return guards below, so the modded-save path
    // can still find the original containers if this load returns early.
    ctx.session.selected_gamepass_save = ctx.session.gamepass_saves.get(&save_id).cloned();

    let container_dir = PathBuf::from(psp_db::settings::get_settings(&ctx.app.db).await?.save_dir);
    let index = ContainerIndex::read_from_dir(&container_dir)?;
    let containers = index.latest_save_containers(&save_id);

    let Some(level_entry) = containers.get("Level") else {
        return Ok(());
    };
    let level_dir = container_dir.join(guid_file_name(&level_entry.container_uuid));
    let level_blob = store::read_first_blob(&container_dir, level_entry)?;

    let Some(level_meta_entry) = containers.get("LevelMeta") else {
        return Ok(());
    };
    let level_meta =
        store::read_first_blob(&container_dir, level_meta_entry)?.map(|(_seq, bytes)| bytes);

    // Optional: absent container => no editable options, matching the silent-return
    // contract used for other absent containers.
    let world_option = match containers.get("WorldOption") {
        Some(entry) => store::read_first_blob(&container_dir, entry)?.map(|(_seq, bytes)| bytes),
        None => None,
    };

    // `player_order` records first-seen container order: the wire `players`
    // array must follow it, not the `BTreeMap`'s uuid-sorted order.
    let mut player_order: Vec<uuid::Uuid> = Vec::new();
    let mut player_file_refs: BTreeMap<uuid::Uuid, PlayerFileData> = BTreeMap::new();
    for (key, entry) in containers.iter() {
        if !key.contains("Player") {
            continue;
        }
        let raw_player_id = entry
            .container_name
            .split('-')
            .next_back()
            .unwrap_or_default();
        let is_dps = raw_player_id.contains("_dps");
        let cleaned_id = raw_player_id.replace("_dps", "");
        let Ok(player_uuid) = uuid::Uuid::parse_str(&cleaned_id) else {
            tracing::warn!("Invalid player UUID: {cleaned_id}");
            continue;
        };
        let Some((_seq, payload)) = store::read_first_blob(&container_dir, entry)? else {
            continue;
        };
        if !player_file_refs.contains_key(&player_uuid) {
            player_order.push(player_uuid);
        }
        let file_ref = player_file_refs
            .entry(player_uuid)
            .or_insert(PlayerFileData::Bytes {
                sav: None,
                dps: None,
            });
        if let PlayerFileData::Bytes { sav, dps } = file_ref {
            if is_dps {
                *dps = Some(payload);
            } else {
                *sav = Some(payload);
            }
        }
    }

    let Some((level_seq, level_sav)) = level_blob else {
        return Err(HandlerError::Other(
            "Level.sav not found in selected save".to_string(),
        ));
    };
    if player_file_refs.is_empty() {
        return Err(HandlerError::Other(
            "No player saves found in selected save".to_string(),
        ));
    }

    let progress = ctx.emitter.progress_sink();
    let session = SaveSession::load(
        SaveKind::GamePass {
            container_id: save_id.clone(),
        },
        save_id.clone(),
        // `sync_app_state` re-emits this as the loaded save's `type` field.
        "gamepass",
        &level_sav,
        level_meta.as_deref(),
        world_option.as_deref(),
        player_file_refs,
        None,
        // Emit the leading generic "Loading Level.sav..." progress frame.
        true,
        &progress,
    )?;

    let world_name = if session.world_name.is_empty() {
        "Unknown".to_string()
    } else {
        session.world_name.clone()
    };
    let session_id = ctx.register_current_session();
    let loaded_payload = serde_json::json!({
        "level": format!("{}/container.{}", level_dir.display(), level_seq),
        "players": player_order
            .iter()
            .map(|player_uuid| player_uuid.to_string())
            .collect::<Vec<_>>(),
        "world_name": world_name,
        "type": "gamepass",
        "size": session.size,
        "has_gps": false,
        "world_option_present": session.world_option.is_some(),
        "session_id": session_id.to_string(),
    });
    ctx.emitter
        .emit(MessageType::LoadedSaveFiles, &loaded_payload);
    crate::handlers::save_file::emit_summary_messages(&session, ctx.emitter);
    ctx.session.save = Some(session);
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct ConvertSaveFormatData {
    pub target_format: String,
    #[serde(default)]
    pub source_path: Option<String>,
    #[serde(default)]
    pub output_path: Option<String>,
    #[serde(default)]
    pub save_id: Option<String>,
}

/// A canceled convert dialog: answers under `convert_save_format` (not a shared
/// `no_file_selected` type) so the tools UI's `sendAndWait`, which correlates by
/// message type, resolves quietly.
fn emit_convert_canceled(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    ctx.emitter.emit(
        MessageType::ConvertSaveFormat,
        &serde_json::json!({"canceled": true}),
    );
    Ok(())
}

/// A soft convert failure under the same `convert_save_format` type.
fn emit_convert_error(ctx: &mut HandlerCtx<'_>, message: String) -> Result<(), HandlerError> {
    ctx.emitter
        .emit(MessageType::ConvertSaveFormat, &serde_json::json!({"error": message}));
    Ok(())
}

async fn resolve_convert_dir(
    save_type: &str,
    ctx: &mut HandlerCtx<'_>,
) -> Result<Option<String>, String> {
    let saved_dir = psp_db::settings::saved_save_dir(&ctx.app.db)
        .await
        .map_err(|error| error.to_string())?;
    let request = crate::desktop_dialogs::dialog_request_for(save_type, saved_dir.as_deref());
    let Some(selected) = ctx.app.dialogs.pick_file(request).await else {
        return Ok(None);
    };
    crate::desktop_dialogs::validate_selected_file(
        save_type,
        &selected,
        &crate::desktop_dialogs::application_root(),
    )?;
    let dir = selected
        .parent()
        .map(|parent| parent.to_string_lossy().into_owned())
        .ok_or_else(|| "Selected file has no parent directory.".to_string())?;
    Ok(Some(dir))
}

async fn resolve_convert_output_dir(ctx: &mut HandlerCtx<'_>) -> Result<Option<PathBuf>, String> {
    let saved_dir = psp_db::settings::saved_save_dir(&ctx.app.db)
        .await
        .map_err(|error| error.to_string())?;
    Ok(ctx.app.dialogs.pick_folder(saved_dir.map(PathBuf::from)).await)
}

async fn extract_named_gamepass_save_to_steam(
    save_id: &str,
    output_dir: &Path,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let progress = ctx.emitter.progress_sink();
    progress("Finding GamePass container path...");
    let container_dir = match store::find_container_dir() {
        Ok(dir) => dir,
        Err(error) => {
            return emit_convert_error(
                ctx,
                format!("Could not find GamePass installation: {error}"),
            )
        }
    };
    progress("Reading GamePass container index...");
    let index = ContainerIndex::read_from_dir(&container_dir)?;
    let containers = index.latest_save_containers(save_id);
    if containers.get("Level").is_none() {
        return emit_convert_error(ctx, format!("Save {save_id} not found in GamePass containers."));
    }
    let save_dir = gamepass_convert::extract_containers_to_steam_dir(
        &container_dir,
        save_id,
        &containers,
        output_dir,
        gamepass_convert::ExtractLabels::SelectedSave,
        &progress,
    )?;
    progress("Conversion complete!");
    let save_dir = save_dir.to_string_lossy().into_owned();
    ctx.emitter.emit(
        MessageType::ConvertSaveFormat,
        &serde_json::json!({
            "message": format!("GamePass save extracted to Steam format at: {save_dir}"),
            "output_path": save_dir,
        }),
    );
    Ok(())
}

async fn write_loaded_save_to_steam_dir(
    output_dir: &Path,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let progress = ctx.emitter.progress_sink();
    progress("Writing Steam save...");
    let session = ctx
        .session
        .save
        .as_ref()
        .ok_or_else(|| HandlerError::Other("No save file loaded".to_string()))?;
    let save_info = psp_core::session::TransferSaveInfo {
        level_sav: output_dir.join("Level.sav"),
        level_meta: Some(output_dir.join("LevelMeta.sav")),
        players_dir: output_dir.join("Players"),
        save_dir: output_dir.to_path_buf(),
    };
    crate::handlers::save_file::write_transfer_target_save(session, &save_info)?;
    progress("Conversion complete!");
    let output = output_dir.to_string_lossy().into_owned();
    ctx.emitter.emit(
        MessageType::ConvertSaveFormat,
        &serde_json::json!({
            "message": format!("Save converted to Steam format at: {output}"),
            "output_path": output,
        }),
    );
    Ok(())
}

/// Branch order is load-bearing: a `save_id` + "steam" request means "extract
/// the named gamepass save", which outranks the standalone source/output pair,
/// which in turn outranks converting the currently loaded save.
pub async fn handle_convert_save_format(
    data: ConvertSaveFormatData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if let Some(save_id) = data.save_id.clone() {
        if data.target_format == "steam" {
            if !ctx.app.config.desktop_mode {
                ctx.emitter.emit(
                    MessageType::ConvertSaveFormat,
                    &serde_json::json!({"error": "Desktop mode required."}),
                );
                return Ok(());
            }
            let output_dir = match resolve_convert_output_dir(ctx).await {
                Ok(Some(dir)) => dir,
                Ok(None) => return emit_convert_canceled(ctx),
                Err(message) => return emit_convert_error(ctx, message),
            };
            return extract_named_gamepass_save_to_steam(&save_id, &output_dir, ctx).await;
        }
    }
    let source_is_select = data.source_path.as_deref() == Some("__select__");
    let output_is_select = data.output_path.as_deref() == Some("__select__");
    let mut source_path = data.source_path.clone();
    let mut output_path = data.output_path.clone();
    if source_is_select || output_is_select {
        if !ctx.app.config.desktop_mode {
            ctx.emitter.emit(
                MessageType::ConvertSaveFormat,
                &serde_json::json!({"error": "No file selected."}),
            );
            return Ok(());
        }
        let (source_type, output_type) = if data.target_format == "gamepass" {
            ("steam", "gamepass")
        } else {
            ("gamepass", "steam")
        };
        if source_is_select {
            match resolve_convert_dir(source_type, ctx).await {
                Ok(Some(dir)) => source_path = Some(dir),
                Ok(None) => return emit_convert_canceled(ctx),
                Err(message) => return emit_convert_error(ctx, message),
            }
        }
        if output_is_select {
            match resolve_convert_dir(output_type, ctx).await {
                Ok(Some(dir)) => output_path = Some(dir),
                Ok(None) => return emit_convert_canceled(ctx),
                Err(message) => return emit_convert_error(ctx, message),
            }
        }
    }
    if let (Some(source_path), Some(output_path)) = (source_path, output_path) {
        return convert_standalone(&source_path, &output_path, &data.target_format, ctx).await;
    }
    if ctx.session.save.is_some() {
        return convert_loaded_save(&data.target_format, ctx).await;
    }
    ctx.emitter.emit(
        MessageType::ConvertSaveFormat,
        &serde_json::json!({"error": "No save file loaded and no source path provided."}),
    );
    Ok(())
}

async fn convert_standalone(
    source_path: &str,
    output_path: &str,
    target_format: &str,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    match target_format {
        "steam" => standalone_gamepass_to_steam(source_path, output_path, ctx).await,
        "gamepass" => standalone_steam_to_gamepass(source_path, output_path, ctx).await,
        other => {
            ctx.emitter.emit(
                MessageType::ConvertSaveFormat,
                &serde_json::json!({"error": format!("Unknown target format: {other}")}),
            );
            Ok(())
        }
    }
}

/// Extracts EVERY save found in the container dir, not just one.
async fn standalone_gamepass_to_steam(
    source_path: &str,
    output_path: &str,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let progress = ctx.emitter.progress_sink();
    progress("Reading GamePass container index...");
    let container_dir = PathBuf::from(source_path);
    // An unreadable index is a hard failure: it surfaces as the dispatcher's
    // `error` frame rather than a soft `{"error": ...}` payload.
    let index = ContainerIndex::read_from_dir(&container_dir)?;

    let save_ids = gamepass_convert::unique_save_ids(&index);
    if save_ids.is_empty() {
        ctx.emitter.emit(
            MessageType::ConvertSaveFormat,
            &serde_json::json!({"error": "No saves found in GamePass containers."}),
        );
        return Ok(());
    }
    let output_root = PathBuf::from(output_path);
    for save_id in &save_ids {
        let containers = index.latest_save_containers(save_id);
        if containers.get("Level").is_none() {
            continue;
        }
        gamepass_convert::extract_containers_to_steam_dir(
            &container_dir,
            save_id,
            &containers,
            &output_root,
            gamepass_convert::ExtractLabels::AllSaves,
            &progress,
        )?;
    }
    progress("Conversion complete!");
    ctx.emitter.emit(
        MessageType::ConvertSaveFormat,
        &serde_json::json!({
            "message": format!("GamePass saves extracted to Steam format at: {output_path}"),
            "output_path": output_path,
        }),
    );
    Ok(())
}

async fn standalone_steam_to_gamepass(
    source_path: &str,
    output_path: &str,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let source_dir = PathBuf::from(source_path);
    if !source_dir.join("Level.sav").exists() {
        ctx.emitter.emit(
            MessageType::ConvertSaveFormat,
            &serde_json::json!({
                "error": "Level.sav not found in source directory. Is this a valid Steam save?"
            }),
        );
        return Ok(());
    }
    let progress = ctx.emitter.progress_sink();
    progress("Reading GamePass container index...");
    let container_dir = PathBuf::from(output_path);
    let index = match ContainerIndex::read_from_dir(&container_dir) {
        Ok(index) => index,
        Err(error) => {
            ctx.emitter.emit(
                MessageType::ConvertSaveFormat,
                &serde_json::json!({
                    "error": format!("Could not read GamePass container index: {error}")
                }),
            );
            return Ok(());
        }
    };
    let new_save_id = gamepass_convert::import_steam_dir_to_gamepass(
        &source_dir,
        &container_dir,
        index,
        &store::backups_root().join("gamepass"),
        &progress,
    )?;
    progress("Conversion complete!");
    ctx.emitter.emit(
        MessageType::ConvertSaveFormat,
        &serde_json::json!({
            "message": format!("Steam save imported to GamePass format (ID: {new_save_id})"),
            "save_id": new_save_id,
        }),
    );
    Ok(())
}

async fn convert_loaded_save(
    target_format: &str,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    match target_format {
        "gamepass" => loaded_save_to_gamepass(ctx).await,
        "steam" => {
            let already_steam = matches!(
                ctx.session.save.as_ref().map(|save| &save.kind),
                Some(SaveKind::Steam { .. })
            );
            if already_steam {
                ctx.emitter.emit(
                    MessageType::ConvertSaveFormat,
                    &serde_json::json!({"error": "Save is already in Steam format."}),
                );
                return Ok(());
            }
            // Writing the steam layout needs a native output-dir dialog.
            if !ctx.app.config.desktop_mode {
                ctx.emitter.emit(
                    MessageType::ConvertSaveFormat,
                    &serde_json::json!({
                        "error": "Desktop mode required for Steam directory selection."
                    }),
                );
                return Ok(());
            }
            let output_dir = match resolve_convert_output_dir(ctx).await {
                Ok(Some(dir)) => dir,
                Ok(None) => return emit_convert_canceled(ctx),
                Err(message) => return emit_convert_error(ctx, message),
            };
            write_loaded_save_to_steam_dir(&output_dir, ctx).await
        }
        other => {
            ctx.emitter.emit(
                MessageType::ConvertSaveFormat,
                &serde_json::json!({"error": format!("Unknown target format: {other}")}),
            );
            Ok(())
        }
    }
}

/// Writes the LOADED save into the real gamepass install under a fresh save id.
async fn loaded_save_to_gamepass(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let already_gamepass = matches!(
        ctx.session.save.as_ref().map(|save| &save.kind),
        Some(SaveKind::GamePass { .. })
    );
    if already_gamepass {
        ctx.emitter.emit(
            MessageType::ConvertSaveFormat,
            &serde_json::json!({"error": "Save is already in GamePass format."}),
        );
        return Ok(());
    }
    let progress = ctx.emitter.progress_sink();
    progress("Finding GamePass container path...");
    let container_dir = match store::find_container_dir() {
        Ok(dir) => dir,
        Err(error) => {
            ctx.emitter.emit(
                MessageType::ConvertSaveFormat,
                &serde_json::json!({
                    "error": format!("Could not find GamePass installation: {error}")
                }),
            );
            return Ok(());
        }
    };
    progress("Creating backup of GamePass containers...");
    store::backup_container_dir(&container_dir, &store::backups_root().join("gamepass"))?;
    progress("Reading container index...");
    let mut index = ContainerIndex::read_from_dir(&container_dir)?;
    index
        .containers
        .retain(|entry| !entry.container_name.starts_with("EggTest"));

    let new_save_id = uuid::Uuid::new_v4().as_simple().to_string().to_uppercase();
    let save = ctx.session.save_mut()?;
    progress("Converting Level.sav...");
    let level_bytes = save.level_sav_bytes()?;
    progress("Converting player save files...");
    let player_bytes = save.player_sav_bytes()?;
    let level_meta_bytes = save.level_meta_sav_bytes()?;

    progress("Creating GamePass containers...");
    index.containers.push(store::create_container(
        &container_dir,
        &new_save_id,
        &level_bytes,
        "Data",
        "Level",
    )?);
    if let Some(meta_bytes) = level_meta_bytes {
        index.containers.push(store::create_container(
            &container_dir,
            &new_save_id,
            &meta_bytes,
            "Data",
            "LevelMeta",
        )?);
    }
    for (player_uuid, (sav_bytes, dps_bytes)) in &player_bytes {
        let player_hex = player_uuid.as_simple().to_string().to_uppercase();
        index.containers.push(store::create_container(
            &container_dir,
            &new_save_id,
            sav_bytes,
            "Data",
            &format!("Players-{player_hex}"),
        )?);
        if let Some(dps_bytes) = dps_bytes {
            index.containers.push(store::create_container(
                &container_dir,
                &new_save_id,
                dps_bytes,
                "Data",
                &format!("Players-{player_hex}_dps"),
            )?);
        }
    }
    progress("Writing container index...");
    index.write_to_dir(&container_dir)?;
    progress("Conversion complete!");
    ctx.emitter.emit(
        MessageType::ConvertSaveFormat,
        &serde_json::json!({
            "message": format!("Save converted to GamePass format (ID: {new_save_id})"),
            "save_id": new_save_id,
        }),
    );
    Ok(())
}
