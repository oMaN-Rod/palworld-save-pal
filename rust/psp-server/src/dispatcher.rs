use std::panic::AssertUnwindSafe;
use std::sync::Arc;

use futures::FutureExt;
use serde_json::Value;

use psp_core::session::Session;

use crate::emitter::Emitter;
use crate::envelope::Envelope;
use crate::handler_error::HandlerError;
use crate::handlers;
use crate::messages::MessageType;
use crate::AppState;

pub struct HandlerCtx<'a> {
    pub session: &'a mut Session,
    pub app: &'a Arc<AppState>,
    pub emitter: &'a Emitter,
}

/// Routes one envelope to its handler. Behavior (matches the Python backend, see
/// plan "Contract deviations" 1-2):
/// - unknown wire string → warn log, nothing sent;
/// - registered type without a Phase-0 handler → warn log, nothing sent;
/// - handler Err → `error` message {message, trace};
/// - handler panic → contained, reported as an `error` message.
///
/// Never returns an error: the connection loop and socket always survive.
pub async fn dispatch(envelope: Envelope, ctx: HandlerCtx<'_>) {
    let mut ctx = ctx;
    let Some(message_type) = MessageType::from_wire(&envelope.message_type) else {
        tracing::warn!(message_type = %envelope.message_type, "invalid message type");
        return;
    };

    let routed = AssertUnwindSafe(route(message_type, envelope.data, &mut ctx))
        .catch_unwind()
        .await;
    match routed {
        Ok(Ok(())) => {}
        Ok(Err(handler_error)) => {
            tracing::error!(message_type = message_type.as_wire(), %handler_error, "handler failed");
            ctx.emitter
                .emit_error(&handler_error.to_string(), &format!("{handler_error:?}"));
        }
        Err(panic_payload) => {
            let panic_text = panic_payload
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| panic_payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "handler panicked".to_string());
            tracing::error!(message_type = message_type.as_wire(), %panic_text, "handler panicked");
            ctx.emitter.emit_error(&panic_text, "handler panicked");
        }
    }
}

async fn route(
    message_type: MessageType,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    match message_type {
        MessageType::GetSettings => handlers::settings::handle_get_settings(ctx).await,
        MessageType::UpdateSettings => {
            handlers::settings::handle_update_settings(serde_json::from_value(data)?, ctx).await
        }
        // Remaining arms are added by Tasks 9-10 and by Phases 1-6.
        other => {
            tracing::warn!(
                message_type = other.as_wire(),
                "handler not implemented yet (Phase 0)"
            );
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::Envelope;
    use crate::test_support::TestContext;

    fn envelope(message_type: &str, data: serde_json::Value) -> Envelope {
        Envelope {
            message_type: message_type.into(),
            data,
        }
    }

    #[tokio::test]
    async fn unknown_type_sends_nothing() {
        // ws/manager.py discards the dispatcher's {"error": ...} return value,
        // so Python sends NOTHING for unknown types. We match that.
        let mut test = TestContext::new(|_| {}).await;
        dispatch(
            envelope("definitely_not_a_type", serde_json::Value::Null),
            HandlerCtx {
                session: &mut test.session,
                app: &test.app,
                emitter: &test.emitter,
            },
        )
        .await;
        test.assert_no_more_frames();
    }

    #[tokio::test]
    async fn valid_but_unimplemented_type_sends_nothing() {
        let mut test = TestContext::new(|_| {}).await;
        dispatch(
            envelope(
                "get_ups_pals",
                serde_json::json!({"offset": 0, "limit": 30}),
            ),
            HandlerCtx {
                session: &mut test.session,
                app: &test.app,
                emitter: &test.emitter,
            },
        )
        .await;
        test.assert_no_more_frames();
    }

    #[tokio::test]
    async fn bad_payload_becomes_error_message() {
        let mut test = TestContext::new(|_| {}).await;
        dispatch(
            envelope("update_settings", serde_json::json!(42)),
            HandlerCtx {
                session: &mut test.session,
                app: &test.app,
                emitter: &test.emitter,
            },
        )
        .await;
        let frame = test.next_frame_json();
        assert_eq!(frame["type"], "error");
        assert!(frame["data"]["message"]
            .as_str()
            .unwrap()
            .contains("invalid payload"));
        assert!(frame["data"]["trace"].is_string());
        test.assert_no_more_frames();
    }

    #[tokio::test]
    async fn get_settings_routes_to_handler() {
        let mut test = TestContext::new(|_| {}).await;
        dispatch(
            envelope("get_settings", serde_json::Value::Null),
            HandlerCtx {
                session: &mut test.session,
                app: &test.app,
                emitter: &test.emitter,
            },
        )
        .await;
        assert_eq!(test.next_frame_json()["type"], "get_settings");
    }
}
