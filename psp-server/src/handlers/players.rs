//! Player-management WS handlers: request_player_details, delete_player,
//! set_technology_data.

use serde::Deserialize;
use serde_json::json;

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;

#[derive(Debug, Deserialize)]
pub struct RequestPlayerDetailsData {
    pub player_id: uuid::Uuid,
    #[serde(default = "default_origin")]
    pub origin: String,
}

fn default_origin() -> String {
    "edit".to_string()
}

#[derive(Debug, Deserialize)]
pub struct DeletePlayerData {
    pub player_id: uuid::Uuid,
    pub origin: String,
}

/// The frontend sends these three fields in camelCase; the renames are the
/// wire contract, not a style choice.
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

/// Every outcome answers under `get_player_details_response`, and `origin` is
/// echoed on the error payload too — the frontend routes the response (success
/// or failure) back to the view that asked for it.
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

/// `player_id` on the response is the deleted id, or `null` when nothing was
/// deleted — that null is how the frontend detects a no-op delete.
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

/// Emits nothing at all when no save is loaded.
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
