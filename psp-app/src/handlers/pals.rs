//! Pal-management WS handlers. (`get_pals` lives in `handlers::game_data`.)

use serde::Deserialize;
use serde_json::json;

use psp_core::domain::pal;
use psp_core::dto::pal::PalDto;

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;

/// Shared by both the `add_pal` and `add_dps_pal` messages.
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

#[derive(Debug, Deserialize)]
pub struct MovePalData {
    pub player_id: uuid::Uuid,
    pub pal_id: uuid::Uuid,
    pub container_id: uuid::Uuid,
}

#[derive(Debug, Deserialize)]
pub struct ClonePalData {
    pub pal: PalDto,
    #[serde(default)]
    pub guild_id: Option<uuid::Uuid>,
    #[serde(default)]
    pub base_id: Option<uuid::Uuid>,
}

/// Shared by both the `delete_pals` and `delete_dps_pals` messages.
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

#[derive(Debug, Deserialize)]
pub struct HealAllPalData {
    #[serde(default)]
    pub player_id: Option<uuid::Uuid>,
    #[serde(default)]
    pub guild_id: Option<uuid::Uuid>,
    #[serde(default)]
    pub base_id: Option<uuid::Uuid>,
}

/// With no save loaded, answers under `get_pal_summaries` with
/// `{"error": ...}` rather than an `error` frame — the frontend correlates the
/// failure to this request by message type.
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

/// `player_id` takes precedence over `guild_id` when both are present.
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

/// A full DPS container emits NO response frame at all; only a missing
/// `player_id` is an error.
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

/// Both branches answer under `add_pal` (not `clone_pal`): the frontend
/// inserts the new pal off the same message it uses for a fresh add.
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

/// Answers under `add_dps_pal` (not `clone_dps_pal`) on both the success and
/// the failure branch.
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

/// Emits NO `delete_pals` response. Instead it rebuilds both summary maps and
/// emits `get_player_summaries` then `get_guild_summaries`, in that order —
/// the frontend refreshes its lists off those two frames.
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
    // `null_progress`, not the connection's sink: a progress sink here would
    // inject `progress_message` frames the frontend does not expect on a
    // delete.
    psp_core::domain::summaries::extract_summaries(session, &psp_core::progress::null_progress())?;
    crate::handlers::save_file::emit_summary_messages(session, ctx.emitter);
    Ok(())
}

/// Emits no response frame.
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

/// On success the `move_pal` payload carries every id as a STRING; a full
/// destination container answers with a `warning` frame instead.
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

/// `data` is a bare UUID list; emits no response frame.
pub async fn handle_heal_pals(
    data: Vec<uuid::Uuid>,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let session = ctx.session.save_mut()?;
    pal::heal_pals(session, &ctx.app.game_data, &data)?;
    Ok(())
}

/// Emits no response frame.
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
