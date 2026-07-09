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
        test.assert_no_more_frames();
    }
}
