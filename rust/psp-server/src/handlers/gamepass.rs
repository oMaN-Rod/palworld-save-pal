//! Gamepass container handlers: scan / delete-save / delete-player /
//! rename-world (`convert_handler.py`) plus the gamepass LOAD path
//! `select_gamepass_save` (`local_file_handler.py:255-374`). Ports of
//! `convert_handler.py`'s scan_gamepass_saves_handler (47-68),
//! delete_gamepass_save_handler (680-730), delete_gamepass_player_handler
//! (733-791) and rename_gamepass_world_handler (794-863).
//!
//! The four container-MANAGEMENT handlers always target the REAL install via
//! `psp_core::gamepass::store::find_container_dir()` (env-overridable via
//! `PSP_GAMEPASS_PACKAGES_ROOT` / `PSP_BACKUPS_ROOT` — see that function's doc
//! comment), matching Python's unconditional `find_container_path()` call, and
//! none of the four touch `ctx.session`: gamepass container management is
//! independent of any loaded save. `handle_select_gamepass_save` is the
//! exception on BOTH counts — it reads the container dir from
//! `settings.save_dir` (the desktop dialog's chosen path) and LOADS the save
//! into `ctx.session.save`.

use serde::{Deserialize, Serialize};

use psp_core::dto::gamepass::GamepassSaveData;
use psp_core::dto::ordered_map::OrderedMap;
use psp_core::gamepass::format::{guid_file_name, ContainerIndex};
use psp_core::gamepass::{scan, store};
use psp_core::session::{PlayerFileData, SaveKind, SaveSession};

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;

/// Wire shape of `scan_gamepass_saves`'s response. `saves` is `OrderedMap`,
/// not `indexmap::IndexMap`: this port deliberately keeps `indexmap` out of
/// psp-server's dependencies (mirrors `psp-core`'s own `dto::ordered_map`
/// reconciliation) — `scan::scan_saves` already returns the same
/// `OrderedMap`, so this struct just carries it straight through to
/// `serde_json`, which serializes it as a JSON object in insertion order.
#[derive(Debug, Serialize)]
struct ScanGamepassSavesResponse {
    saves: OrderedMap<String, GamepassSaveData>,
    container_path: Option<String>,
}

/// convert_handler.py:47-68 — a missing GamePass install is a normal empty
/// response ({"saves": {}, "container_path": null}), not an `error` frame.
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

/// convert_handler.py:680-730.
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

/// convert_handler.py:733-791.
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

/// convert_handler.py:794-863 — reads the save's current LevelMeta blob,
/// rewrites `SaveData.WorldName`, and appends a brand-new LevelMeta
/// container (not an in-place edit); `ContainerIndex::latest_save_containers`
/// (Task 5's seq/mtime "latest" rule) then makes the new container win over
/// the old one on every subsequent read, exactly like Python's
/// `create_new_container` + `container_index.containers.append(...)`.
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

/// Port of `select_gamepass_save_handler` (local_file_handler.py:255-374). The
/// message data is a BARE save-id string. Unlike the scan/delete/rename
/// handlers, this one loads a save into `ctx.session.save`, so the container
/// directory comes from `settings.save_dir` (set by the desktop file dialog in
/// production) — NOT `find_container_dir()`.
///
/// Silent-return contract: a missing Level OR LevelMeta *container entry*
/// returns `Ok(())` with NO frame emitted at all (Python `return`s), whereas a
/// missing Level *payload* or an empty player set raises (surfacing an `error`
/// frame). On success emits `loaded_save_files` (`type: "gamepass"`,
/// `has_gps: false`) then `get_player_summaries` / `get_guild_summaries`.
pub async fn handle_select_gamepass_save(
    save_id: String,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    // Python sets `selected_gamepass_save` up front, before the silent-return
    // guards, so the modded-save path can find the original containers even if
    // this load returns early.
    ctx.session.selected_gamepass_save = ctx.session.gamepass_saves.get(&save_id).cloned();

    let container_dir = PathBuf::from(psp_db::settings::get_settings(&ctx.app.db).await?.save_dir);
    let index = ContainerIndex::read_from_dir(&container_dir)?;
    let containers = index.latest_save_containers(&save_id);

    // Missing Level / LevelMeta container ENTRY: Python returns silently
    // (local_file_handler.py:270-272, 287-289).
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

    // Player containers → in-memory byte refs. `player_order` preserves the
    // first-seen order across player containers so the wire `players` array
    // matches Python's `[str(p) for p in player_files.keys()]` (insertion
    // order), rather than the `BTreeMap`'s uuid-sorted order.
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
        // sync_app_state emits this as the loaded save's `type` field
        // (app_state_handler.py:30 `save_type.name.lower()` → "gamepass").
        "gamepass",
        &level_sav,
        level_meta.as_deref(),
        player_file_refs,
        None,
        // select_gamepass_save goes through Python's process_save_files, which
        // emits the leading generic "Loading Level.sav..." frame.
        true,
        &progress,
    )?;

    let world_name = if session.world_name.is_empty() {
        "Unknown".to_string()
    } else {
        session.world_name.clone()
    };
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
    });
    ctx.emitter
        .emit(MessageType::LoadedSaveFiles, &loaded_payload);
    crate::handlers::save_file::emit_summary_messages(&session, ctx.emitter);
    ctx.session.save = Some(session);
    Ok(())
}
