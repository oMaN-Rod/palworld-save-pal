//! Player-management WS handlers — ports of
//! `palworld_save_pal/ws/handlers/lazy_load_handler.py` (request_player_details),
//! `player_handler.py` (delete_player) and `technologies_handler.py`
//! (set_technology_data).

use serde::Deserialize;
use serde_json::json;

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;

/// ws/messages.py:743-745 (origin defaults to "edit").
#[derive(Debug, Deserialize)]
pub struct RequestPlayerDetailsData {
    pub player_id: uuid::Uuid,
    #[serde(default = "default_origin")]
    pub origin: String,
}

fn default_origin() -> String {
    "edit".to_string()
}

/// ws/messages.py:470-472.
#[derive(Debug, Deserialize)]
pub struct DeletePlayerData {
    pub player_id: uuid::Uuid,
    pub origin: String,
}

/// ws/messages.py:436-440 — camelCase field names copied EXACTLY off the wire.
#[derive(Debug, Deserialize)]
pub struct TechnologyData {
    #[serde(rename = "playerID")]
    pub player_id: uuid::Uuid,
    pub technologies: Vec<String>,
    #[serde(rename = "techPoints")]
    pub tech_points: i64,
    #[serde(rename = "ancientTechPoints")]
    pub ancient_tech_points: i64,
}

/// request_player_details_handler (lazy_load_handler.py:22-64). No save →
/// `get_player_details_response` `{error, origin}` (NOT the dispatcher error
/// frame). Otherwise the domain's on-demand load emits `progress_message`
/// frames first, then the response is `{player, player_id (string), origin}`
/// or `{error, origin}`.
pub async fn handle_request_player_details(
    data: RequestPlayerDetailsData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Ok(session) = ctx.session.save_mut() else {
        ctx.emitter.emit(
            MessageType::GetPlayerDetailsResponse,
            &json!({"error": "No save file loaded", "origin": data.origin}),
        );
        return Ok(());
    };
    let progress = ctx.emitter.progress_sink();
    let details = psp_core::domain::player::get_player_details(
        session,
        &ctx.app.game_data,
        data.player_id,
        &progress,
    )?;
    match details {
        Some(player) => ctx.emitter.emit(
            MessageType::GetPlayerDetailsResponse,
            &json!({
                "player": player,
                "player_id": data.player_id.to_string(),
                "origin": data.origin,
            }),
        ),
        None => ctx.emitter.emit(
            MessageType::GetPlayerDetailsResponse,
            &json!({
                "error": format!("Player {} not found", data.player_id),
                "origin": data.origin,
            }),
        ),
    }
    Ok(())
}

/// delete_player_handler (player_handler.py:14-39). No save → `warning`
/// "No save file loaded". Otherwise progress frames (from the domain), then
/// `delete_player` `{player_id: id-or-null, origin}` — Python emits the id on
/// success, `None` on failure.
pub async fn handle_delete_player(
    data: DeletePlayerData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Ok(session) = ctx.session.save_mut() else {
        ctx.emitter
            .emit(MessageType::Warning, &"No save file loaded");
        return Ok(());
    };
    let progress = ctx.emitter.progress_sink();
    let deleted = psp_core::domain::player::delete_player(
        session,
        &ctx.app.game_data,
        data.player_id,
        &progress,
    )?;
    ctx.emitter.emit(
        MessageType::DeletePlayer,
        &json!({
            "player_id": if deleted { Some(data.player_id) } else { None },
            "origin": data.origin,
        }),
    );
    Ok(())
}

/// set_technology_data_handler (technologies_handler.py:40-56): silently
/// returns (no frame) when no save is loaded; otherwise applies the update and
/// emits `set_technology_data` `{"success": true}`.
pub async fn handle_set_technology_data(
    data: TechnologyData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Ok(session) = ctx.session.save_mut() else {
        return Ok(());
    };
    psp_core::domain::player::update_player_technologies(
        session,
        data.player_id,
        Some(&data.technologies),
        Some(data.tech_points),
        Some(data.ancient_tech_points),
    )?;
    ctx.emitter
        .emit(MessageType::SetTechnologyData, &json!({"success": true}));
    Ok(())
}
