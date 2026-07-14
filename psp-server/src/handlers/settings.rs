use psp_core::dto::settings::{SettingsDto, SettingsUpdateDto};
use psp_db::settings::{get_settings, update_settings, SettingsRow, SettingsUpdate};

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;

pub fn settings_dto_from_row(row: SettingsRow) -> SettingsDto {
    SettingsDto {
        language: row.language,
        save_dir: row.save_dir,
        clone_prefix: row.clone_prefix,
        new_pal_prefix: row.new_pal_prefix,
        debug_mode: row.debug_mode,
        cheat_mode: row.cheat_mode,
    }
}

pub async fn handle_get_settings(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let row = get_settings(&ctx.app.db).await?;
    ctx.emitter
        .emit(MessageType::GetSettings, &settings_dto_from_row(row));
    Ok(())
}

/// Answers under `get_settings`, NOT `update_settings` — the frontend refreshes
/// its settings store off that message type.
pub async fn handle_update_settings(
    update: SettingsUpdateDto,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let row = update_settings(
        &ctx.app.db,
        &SettingsUpdate {
            language: update.language,
            clone_prefix: update.clone_prefix,
            new_pal_prefix: update.new_pal_prefix,
            debug_mode: update.debug_mode,
            cheat_mode: update.cheat_mode,
        },
    )
    .await?;
    ctx.emitter
        .emit(MessageType::GetSettings, &settings_dto_from_row(row));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatcher::HandlerCtx;
    use crate::test_support::TestContext;

    #[tokio::test]
    async fn get_settings_emits_defaults() {
        let mut test = TestContext::new(|_| {}).await;
        let mut ctx = HandlerCtx {
            session: &mut test.session,
            app: &test.app,
            emitter: &test.emitter,
            attachment: None,
        };
        handle_get_settings(&mut ctx).await.unwrap();

        let frame = test.next_frame_json();
        assert_eq!(frame["type"], "get_settings");
        assert_eq!(frame["data"]["language"], "en");
        assert_eq!(frame["data"]["clone_prefix"], "©️");
        assert_eq!(frame["data"]["new_pal_prefix"], "🆕");
        assert_eq!(frame["data"]["debug_mode"], false);
        assert_eq!(frame["data"]["cheat_mode"], false);
        assert!(frame["data"]["save_dir"].is_string());
        test.assert_no_more_frames();
    }

    #[tokio::test]
    async fn update_settings_responds_with_get_settings_type() {
        let mut test = TestContext::new(|_| {}).await;
        let mut ctx = HandlerCtx {
            session: &mut test.session,
            app: &test.app,
            emitter: &test.emitter,
            attachment: None,
        };
        let update: psp_core::dto::settings::SettingsUpdateDto =
            serde_json::from_value(serde_json::json!({
                "language": "fr", "clone_prefix": "©️", "new_pal_prefix": "🆕",
                "debug_mode": true, "cheat_mode": false,
                "save_dir": "ignored-extra-key"
            }))
            .unwrap();
        handle_update_settings(update, &mut ctx).await.unwrap();

        let frame = test.next_frame_json();
        assert_eq!(frame["type"], "get_settings");
        assert_eq!(frame["data"]["language"], "fr");
        assert_eq!(frame["data"]["debug_mode"], true);
        // `save_dir` is not a settable field: the response must carry the row's
        // persisted default, never the extra key the request smuggled in.
        assert_eq!(
            frame["data"]["save_dir"],
            psp_db::settings::default_steam_save_dir()
        );
        test.assert_no_more_frames();
    }
}
