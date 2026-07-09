use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::handlers::settings::settings_dto_from_row;
use crate::messages::MessageType;

/// Port of app_state_handler.py sync_app_state_handler. Phase 0 implements
/// only the no-save branch; the loaded_save_files/summaries emissions are Phase 1.
pub async fn handle_sync_app_state(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let row = psp_db::settings::get_settings(&ctx.app.db).await?;
    ctx.emitter
        .emit(MessageType::GetSettings, &settings_dto_from_row(row));

    if ctx.session.save.is_none() {
        tracing::warn!("no save file loaded");
        return Ok(());
    }
    // Phase 1: emit loaded_save_files, get_player_summaries, get_guild_summaries.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestContext;

    #[tokio::test]
    async fn sync_app_state_without_save_emits_only_settings() {
        // sync_app_state is the ONLY path by which settings reach the UI during
        // bootstrap() — so this asserts the full six-field payload, not just
        // `language`. `save_dir` is the most delicate field in it: Python emits
        // `null` on a fresh DB (a deterministic import-order bug — see
        // rust/parity/README.md), Rust correctly emits
        // `default_steam_save_dir()`, and that divergence is deliberately left
        // unmasked (PARITY_IGNORED_PATHS stays empty). Pinning the real default
        // here, rather than merely `is_string()`, is what would catch a
        // regression back to `null`/an empty string.
        let mut test = TestContext::new(|_| {}).await;
        let mut ctx = HandlerCtx {
            session: &mut test.session,
            app: &test.app,
            emitter: &test.emitter,
        };
        handle_sync_app_state(&mut ctx).await.unwrap();
        let frame = test.next_frame_json();
        assert_eq!(frame["type"], "get_settings");
        assert_eq!(frame["data"]["language"], "en");
        assert_eq!(
            frame["data"]["save_dir"],
            psp_db::settings::default_steam_save_dir()
        );
        assert_eq!(frame["data"]["clone_prefix"], "©️");
        assert_eq!(frame["data"]["new_pal_prefix"], "🆕");
        assert_eq!(frame["data"]["debug_mode"], false);
        assert_eq!(frame["data"]["cheat_mode"], false);
        test.assert_no_more_frames();
    }
}
