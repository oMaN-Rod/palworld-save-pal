//! Standalone conversion utilities (Task 3E-1), ported from
//! `ws/handlers/steam_id_handler.py`. Unlike the other handler modules this
//! one does not touch `ctx.session`/`ctx.app` at all -- `convert_steam_id` is
//! a pure input->output tool available with no save file loaded.

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
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
            Err(_) => serde_json::json!({
                "error": "Invalid input. Enter a numeric Steam ID, profile URL, or Palworld UID."
            }),
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
            Err(error) => {
                let message = error.to_string();
                let message = if message.is_empty() {
                    "Invalid input. Enter a numeric Steam ID, profile URL, or Palworld UID."
                        .to_string()
                } else {
                    message
                };
                serde_json::json!({ "error": message })
            }
        }
    };
    ctx.emitter.emit(MessageType::ConvertSteamId, &payload);
    Ok(())
}
