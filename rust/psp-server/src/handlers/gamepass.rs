//! Gamepass container handlers: scan / delete-save / delete-player /
//! rename-world. Port of `palworld_save_pal/ws/handlers/convert_handler.py`'s
//! scan_gamepass_saves_handler (47-68), delete_gamepass_save_handler
//! (680-730), delete_gamepass_player_handler (733-791) and
//! rename_gamepass_world_handler (794-863).
//!
//! All four always target the REAL install via `psp_core::gamepass::store::
//! find_container_dir()` (env-overridable via `PSP_GAMEPASS_PACKAGES_ROOT` /
//! `PSP_BACKUPS_ROOT` — see that function's doc comment), matching Python's
//! unconditional `find_container_path()` call. None of the four touch
//! `ctx.session`/`ctx.app`: gamepass container management is independent of
//! any loaded save.

use serde::{Deserialize, Serialize};

use psp_core::dto::gamepass::GamepassSaveData;
use psp_core::dto::ordered_map::OrderedMap;
use psp_core::gamepass::format::ContainerIndex;
use psp_core::gamepass::{scan, store};

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
