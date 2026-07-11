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
            // Near-unreachable: `is_palworld_uid` already validated `raw` as
            // 32-hex / dashed-hex, and every such string parses as a UUID, so
            // Python never actually hits `parse_palworld_uid`'s error path
            // either. Kept only so the branch is total; the emitted text is not
            // load-bearing (no Python fixture exercises it).
            Err(error) => serde_json::json!({ "error": error.to_string() }),
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
            // Emit the error's own message verbatim. For a non-numeric input
            // this is Python's `int()` text ("invalid literal for int() with
            // base 10: '<processed>'"); for a vanity URL it is the distinct
            // VanityUrl message. steam_id_handler.py:39-42's `str(e) if str(e)
            // else <generic>` fallback is dead code in real Python (`int()`'s
            // ValueError message is never empty), so no generic remap here.
            Err(error) => serde_json::json!({ "error": error.to_string() }),
        }
    };
    ctx.emitter.emit(MessageType::ConvertSteamId, &payload);
    Ok(())
}
