//! Guild-management WS handlers — ports of
//! `palworld_save_pal/ws/handlers/lazy_load_handler.py` (request_guild_details),
//! `lab_research_handler.py` (update_lab_research) and `guild_handler.py`
//! (delete_guild).
//!
//! `get_lab_research` is NOT here — it is already implemented and registered
//! in `handlers::game_data`.

use serde::Deserialize;
use serde_json::json;

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;

/// ws/messages.py:460-462.
#[derive(Debug, Deserialize)]
pub struct DeleteGuildData {
    pub guild_id: uuid::Uuid,
    pub origin: String,
}

/// ws/messages.py:488-490.
#[derive(Debug, Deserialize)]
pub struct UpdateLabResearchData {
    pub guild_id: uuid::Uuid,
    pub research_updates: Vec<psp_core::dto::guild::GuildLabResearchInfo>,
}

/// request_guild_details_handler (lazy_load_handler.py:67-107). `data` is a
/// BARE UUID string (ws/messages.py:755 `data: UUID`). No save →
/// `get_guild_details_response` `{error}`; found → `{guild, guild_id (string)}`;
/// missing → `{error}`.
pub async fn handle_request_guild_details(
    guild_id: uuid::Uuid,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Ok(session) = ctx.session.save_mut() else {
        ctx.emitter.emit(
            MessageType::GetGuildDetailsResponse,
            &json!({"error": "No save file loaded"}),
        );
        return Ok(());
    };
    match psp_core::domain::guild::get_guild_details(session, &ctx.app.game_data, guild_id)? {
        Some(guild) => ctx.emitter.emit(
            MessageType::GetGuildDetailsResponse,
            &json!({ "guild": guild, "guild_id": guild_id.to_string() }),
        ),
        None => ctx.emitter.emit(
            MessageType::GetGuildDetailsResponse,
            &json!({ "error": format!("Guild {guild_id} not found") }),
        ),
    }
    Ok(())
}

/// update_lab_research_handler (lab_research_handler.py:38-66). Uses
/// PLAIN-STRING `error` payloads (NOT the dispatcher `{message, trace}`
/// shape): no save → `error` "No save file loaded."; guild missing → `error`
/// "Guild {id} not found."; other failure → `error` "Failed to update lab
/// research: {e}". Success → `update_lab_research` `{success: true, guild_id}`.
pub async fn handle_update_lab_research(
    data: UpdateLabResearchData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Ok(session) = ctx.session.save_mut() else {
        ctx.emitter
            .emit(MessageType::Error, &"No save file loaded.");
        return Ok(());
    };
    match psp_core::domain::guild::update_lab_research(
        session,
        data.guild_id,
        &data.research_updates,
    ) {
        Ok(()) => ctx.emitter.emit(
            MessageType::UpdateLabResearch,
            &json!({"success": true, "guild_id": data.guild_id.to_string()}),
        ),
        Err(psp_core::error::CoreError::GuildNotFound(_)) => {
            ctx.emitter.emit(
                MessageType::Error,
                &format!("Guild {} not found.", data.guild_id),
            );
        }
        Err(other) => {
            ctx.emitter.emit(
                MessageType::Error,
                &format!("Failed to update lab research: {other}"),
            );
        }
    }
    Ok(())
}

/// delete_guild_handler (guild_handler.py:13-38). No save → `warning`
/// "No save file loaded". Otherwise progress frames (from the domain), then
/// `delete_guild` `{guild_id, origin}`.
pub async fn handle_delete_guild(
    data: DeleteGuildData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Ok(session) = ctx.session.save_mut() else {
        ctx.emitter
            .emit(MessageType::Warning, &"No save file loaded");
        return Ok(());
    };
    let progress = ctx.emitter.progress_sink();
    psp_core::domain::guild::delete_guild_and_players(
        session,
        &ctx.app.game_data,
        data.guild_id,
        &progress,
    )?;
    ctx.emitter.emit(
        MessageType::DeleteGuild,
        &json!({ "guild_id": data.guild_id, "origin": data.origin }),
    );
    Ok(())
}
