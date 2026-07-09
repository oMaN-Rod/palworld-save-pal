use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;

/// PHASE-0 STOPGAP: the UI's bootstrap() awaits get_presets before anything
/// else, so it must get *a* response. Python populates presets from
/// data/json/presets.json into the DB (preset_handler.py:40-46) — that full
/// implementation, and its parity fixture, land in Phase 3.
pub async fn handle_get_presets(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    ctx.emitter
        .emit(MessageType::GetPresets, &Vec::<serde_json::Value>::new());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestContext;

    #[tokio::test]
    async fn get_presets_stopgap_returns_empty_list() {
        let mut test = TestContext::new(|_| {}).await;
        let mut ctx = HandlerCtx {
            session: &mut test.session,
            app: &test.app,
            emitter: &test.emitter,
        };
        handle_get_presets(&mut ctx).await.unwrap();
        let frame = test.next_frame_json();
        assert_eq!(frame["type"], "get_presets");
        assert_eq!(frame["data"], serde_json::json!([]));
    }
}
