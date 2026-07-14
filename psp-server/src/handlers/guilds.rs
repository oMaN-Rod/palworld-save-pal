//! Guild-management WS handlers: request_guild_details, update_lab_research,
//! delete_guild. (`get_lab_research` lives in `handlers::game_data`.)

use serde::Deserialize;
use serde_json::json;

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;

#[derive(Debug, Deserialize)]
pub struct DeleteGuildData {
    pub guild_id: uuid::Uuid,
    pub origin: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateLabResearchData {
    pub guild_id: uuid::Uuid,
    pub research_updates: Vec<psp_core::dto::guild::GuildLabResearchInfo>,
}

/// `data` is a BARE UUID string. Every outcome answers under
/// `get_guild_details_response`: `{guild, guild_id}` on success, `{error}` when
/// no save is loaded or the guild is unknown — never the dispatcher's
/// `error` frame, which the frontend does not correlate to this request.
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

/// Failures emit `error` frames whose data is a PLAIN STRING, not the
/// dispatcher's `{message, trace}` object — the frontend renders it directly.
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

/// `origin` is echoed back untouched: the frontend routes the response to the
/// view that asked for the delete.
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
