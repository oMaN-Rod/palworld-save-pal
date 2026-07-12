//! Pal-management WS handlers — ports of
//! `palworld_save_pal/ws/handlers/pal_handler.py`. `get_pals` is NOT here
//! (it is already implemented + registered in `handlers::game_data`); this
//! module covers only the genuinely-new Phase-2 pal message types.

use serde::Deserialize;
use serde_json::json;

use psp_core::domain::pal;
use psp_core::dto::pal::PalDto;

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;

/// ws/messages.py:176-183 AddPalData. Reused by both `add_pal` and
/// `add_dps_pal` (ws/messages.py:191-193 `AddDpsPalMessage.data: AddPalData`).
#[derive(Debug, Deserialize)]
pub struct AddPalData {
    #[serde(default)]
    pub player_id: Option<uuid::Uuid>,
    #[serde(default)]
    pub guild_id: Option<uuid::Uuid>,
    #[serde(default)]
    pub base_id: Option<uuid::Uuid>,
    pub character_id: String,
    pub nickname: String,
    #[serde(default)]
    pub container_id: Option<uuid::Uuid>,
    #[serde(default)]
    pub storage_slot: Option<i32>,
}

/// ws/messages.py:196-199 MovePalData.
#[derive(Debug, Deserialize)]
pub struct MovePalData {
    pub player_id: uuid::Uuid,
    pub pal_id: uuid::Uuid,
    pub container_id: uuid::Uuid,
}

/// ws/messages.py:207-210 ClonePalData.
#[derive(Debug, Deserialize)]
pub struct ClonePalData {
    pub pal: PalDto,
    #[serde(default)]
    pub guild_id: Option<uuid::Uuid>,
    #[serde(default)]
    pub base_id: Option<uuid::Uuid>,
}

/// ws/messages.py:243-248 DeletePalsData. Reused by `delete_dps_pals`
/// (ws/messages.py:256-258).
#[derive(Debug, Deserialize)]
pub struct DeletePalsData {
    #[serde(default)]
    pub pal_indexes: Option<Vec<i32>>,
    #[serde(default)]
    pub pal_ids: Option<Vec<uuid::Uuid>>,
    #[serde(default)]
    pub player_id: Option<uuid::Uuid>,
    #[serde(default)]
    pub guild_id: Option<uuid::Uuid>,
    #[serde(default)]
    pub base_id: Option<uuid::Uuid>,
}

/// ws/messages.py:398-401 HealAllPalData.
#[derive(Debug, Deserialize)]
pub struct HealAllPalData {
    #[serde(default)]
    pub player_id: Option<uuid::Uuid>,
    #[serde(default)]
    pub guild_id: Option<uuid::Uuid>,
    #[serde(default)]
    pub base_id: Option<uuid::Uuid>,
}

/// get_pal_summaries_handler (pal_handler.py:53-64): no save → the
/// plain-object `{"error": "No save file loaded"}` payload (NOT the dispatcher's
/// `{message, trace}` error frame).
pub async fn handle_get_pal_summaries(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let Some(session) = ctx.session.save.as_ref() else {
        ctx.emitter.emit(
            MessageType::GetPalSummaries,
            &json!({"error": "No save file loaded"}),
        );
        return Ok(());
    };
    let summaries = pal::pal_summaries(session, &ctx.app.game_data)?;
    ctx.emitter
        .emit(MessageType::GetPalSummaries, &json!({ "pals": summaries }));
    Ok(())
}

/// add_pal_handler (pal_handler.py:67-109). No save → `warning` "No save file
/// loaded" (a plain string). Neither id → `warning` "No player_id or guild_id
/// provided". Player path takes precedence over guild path, matching Python's
/// `if player_id: ... elif guild_id: ...`.
pub async fn handle_add_pal(
    data: AddPalData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Ok(session) = ctx.session.save_mut() else {
        ctx.emitter
            .emit(MessageType::Warning, &"No save file loaded");
        return Ok(());
    };
    if let Some(player_id) = data.player_id {
        // Python passes container_id straight through; add_player_pal itself
        // resolves the mutation target and writes the raw id (see its doc
        // comment). A missing container_id is a real Python `None` there.
        let container_id = data.container_id.ok_or_else(|| {
            HandlerError::Other("container_id required for player add".to_string())
        })?;
        let new_pal = pal::add_player_pal(
            session,
            &ctx.app.game_data,
            player_id,
            &data.character_id,
            &data.nickname,
            container_id,
            data.storage_slot,
        )?;
        ctx.emitter.emit(
            MessageType::AddPal,
            &json!({ "player_id": player_id, "pal": new_pal }),
        );
    } else if let Some(guild_id) = data.guild_id {
        let base_id = data
            .base_id
            .ok_or_else(|| HandlerError::Other("base_id required for guild add".to_string()))?;
        let new_pal = pal::add_guild_pal(
            session,
            &ctx.app.game_data,
            guild_id,
            base_id,
            &data.character_id,
            &data.nickname,
            data.storage_slot,
        )?;
        ctx.emitter.emit(
            MessageType::AddPal,
            &json!({ "guild_id": guild_id, "base_id": base_id, "pal": new_pal }),
        );
    } else {
        ctx.emitter
            .emit(MessageType::Warning, &"No player_id or guild_id provided");
    }
    Ok(())
}

/// add_dps_pal_handler (pal_handler.py:112-127). Python only builds a response
/// when `player_id` is set (else `data` is undefined → NameError → dispatcher
/// error frame); reproduced as an `Err` for a missing id. A `None` result
/// (container full — Python would `TypeError` unpacking it) emits nothing, per
/// this port's graceful policy documented on `add_player_dps_pal`.
pub async fn handle_add_dps_pal(
    data: AddPalData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let session = ctx.session.save_mut()?;
    let player_id = data
        .player_id
        .ok_or_else(|| HandlerError::Other("player_id required".to_string()))?;
    let result = pal::add_player_dps_pal(
        session,
        &ctx.app.game_data,
        player_id,
        &data.character_id,
        &data.nickname,
        data.storage_slot,
    )?;
    if let Some((slot_index, new_pal)) = result {
        ctx.emitter.emit(
            MessageType::AddDpsPal,
            &json!({ "player_id": player_id, "pal": new_pal, "index": slot_index }),
        );
    }
    Ok(())
}

/// clone_pal_handler (pal_handler.py:151-171). Guild path → `add_pal`
/// `{guild_id, base_id, pal}`; player path → `add_pal`
/// `{player_id: pal.owner_uid or None, pal}`.
pub async fn handle_clone_pal(
    data: ClonePalData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let session = ctx.session.save_mut()?;
    if let Some(guild_id) = data.guild_id {
        let base_id = data
            .base_id
            .ok_or_else(|| HandlerError::Other("base_id required".to_string()))?;
        let new_pal =
            pal::clone_guild_pal(session, &ctx.app.game_data, guild_id, base_id, &data.pal)?;
        ctx.emitter.emit(
            MessageType::AddPal,
            &json!({ "guild_id": guild_id, "base_id": base_id, "pal": new_pal }),
        );
    } else {
        let new_pal = pal::clone_pal(session, &ctx.app.game_data, &data.pal)?;
        ctx.emitter.emit(
            MessageType::AddPal,
            &json!({ "player_id": data.pal.owner_uid, "pal": new_pal }),
        );
    }
    Ok(())
}

/// clone_dps_pal_handler (pal_handler.py:174-196). A `None` result →
/// `add_dps_pal` `{"error": ...}`; success → `add_dps_pal`
/// `{player_id: pal.owner_uid or None, pal, index}`.
pub async fn handle_clone_dps_pal(
    data: ClonePalData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let session = ctx.session.save_mut()?;
    match pal::clone_dps_pal(session, &ctx.app.game_data, &data.pal)? {
        Some((slot_index, new_pal)) => {
            ctx.emitter.emit(
                MessageType::AddDpsPal,
                &json!({ "player_id": data.pal.owner_uid, "pal": new_pal, "index": slot_index }),
            );
        }
        None => {
            ctx.emitter.emit(
                MessageType::AddDpsPal,
                &json!({"error": "Failed to clone pal. No DPS storage or available slots."}),
            );
        }
    }
    Ok(())
}

/// delete_pals_handler (pal_handler.py:199-222). Emits NO `delete_pals`
/// response: it refreshes both summary maps and emits `get_player_summaries`
/// then `get_guild_summaries`, in that order. Python does this silently via
/// `get_player_summaries()`/`get_guild_summaries()` (no progress callback),
/// so the refresh here uses `null_progress` — reusing `extract_summaries`
/// with the connection's progress sink would inject spurious
/// `progress_message` frames and break frame-order parity.
pub async fn handle_delete_pals(
    data: DeletePalsData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let session = ctx.session.save_mut()?;
    let pal_ids = data.pal_ids.unwrap_or_default();
    if let Some(player_id) = data.player_id {
        pal::delete_player_pals(session, player_id, &pal_ids)?;
    }
    if let Some(guild_id) = data.guild_id {
        let base_id = data
            .base_id
            .ok_or_else(|| HandlerError::Other("base_id required".to_string()))?;
        pal::delete_guild_pals(session, guild_id, base_id, &pal_ids)?;
    }
    // Rebuild both summary maps from the post-delete world tree, silently
    // (matching Python's non-callback recompute), then emit them in order.
    psp_core::domain::summaries::extract_summaries(session, &psp_core::progress::null_progress())?;
    crate::handlers::save_file::emit_summary_messages(session, ctx.emitter);
    Ok(())
}

/// delete_dps_pals_handler (pal_handler.py:225-230): no response.
pub async fn handle_delete_dps_pals(
    data: DeletePalsData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let session = ctx.session.save_mut()?;
    let player_id = data
        .player_id
        .ok_or_else(|| HandlerError::Other("player_id required".to_string()))?;
    let indexes = data.pal_indexes.unwrap_or_default();
    pal::delete_player_dps_pals(session, &ctx.app.game_data, player_id, &indexes)?;
    Ok(())
}

/// move_pal_handler (pal_handler.py:130-148). Success → `move_pal` with every
/// id as a STRING; a `None` (container full) → `warning` "Pal container is
/// full".
pub async fn handle_move_pal(
    data: MovePalData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let session = ctx.session.save_mut()?;
    match pal::move_pal(
        session,
        &ctx.app.game_data,
        data.player_id,
        data.pal_id,
        data.container_id,
    )? {
        Some(moved_pal) => {
            ctx.emitter.emit(
                MessageType::MovePal,
                &json!({
                    "player_id": data.player_id.to_string(),
                    "pal_id": moved_pal.instance_id.to_string(),
                    "container_id": data.container_id.to_string(),
                    "slot_index": moved_pal.storage_slot,
                }),
            );
        }
        None => ctx
            .emitter
            .emit(MessageType::Warning, &"Pal container is full"),
    }
    Ok(())
}

/// heal_pals_handler (pal_handler.py:233-237): `data` is a bare UUID list, no
/// response.
pub async fn handle_heal_pals(
    data: Vec<uuid::Uuid>,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let session = ctx.session.save_mut()?;
    pal::heal_pals(session, &ctx.app.game_data, &data)?;
    Ok(())
}

/// heal_all_pals_handler (pal_handler.py:240-249): no response.
pub async fn handle_heal_all_pals(
    data: HealAllPalData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let session = ctx.session.save_mut()?;
    if let Some(player_id) = data.player_id {
        pal::heal_all_player_pals(session, &ctx.app.game_data, player_id)?;
    }
    if let Some(guild_id) = data.guild_id {
        let base_id = data
            .base_id
            .ok_or_else(|| HandlerError::Other("base_id required".to_string()))?;
        pal::heal_all_base_pals(session, &ctx.app.game_data, guild_id, base_id)?;
    }
    Ok(())
}
