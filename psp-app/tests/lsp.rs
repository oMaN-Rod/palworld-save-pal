use std::collections::BTreeMap;
use std::sync::Arc;

use psp_app::dispatcher::{dispatch, HandlerCtx};
use psp_app::envelope::Envelope;
use psp_app::handlers::lsp::*;
use psp_app::lsp::{LspService, TierStatus};
use psp_app::test_support::TestContext;

fn ctx<'a>(test: &'a mut TestContext) -> HandlerCtx<'a> {
    HandlerCtx {
        session: &mut test.session,
        app: &test.app,
        emitter: &test.emitter,
        blueprints: &mut test.blueprints,
        attachment: None,
    }
}

fn envelope(message_type: &str, data: serde_json::Value) -> Envelope {
    Envelope {
        message_type: message_type.into(),
        data,
    }
}

struct FailingLspService;

#[async_trait::async_trait]
impl LspService for FailingLspService {
    fn status(&self) -> TierStatus {
        TierStatus::Unavailable {
            reason: "failing".to_string(),
        }
    }

    async fn ensure_ready(
        &self,
        _plugin_id: &str,
        _sources: &BTreeMap<String, String>,
    ) -> Result<(), String> {
        Err("boom".to_string())
    }

    async fn request(
        &self,
        _plugin_id: &str,
        _frame: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        Err("boom".to_string())
    }

    async fn notify(&self, _plugin_id: &str, _frame: serde_json::Value) -> Result<(), String> {
        Err("notify boom".to_string())
    }

    async fn shutdown(&self, _plugin_id: &str) {}
}

#[tokio::test]
async fn a_deployment_without_a_language_server_reports_the_baseline_tier() {
    let mut test = TestContext::new(|_| {}).await;
    handle_get_editor_tier(&mut ctx(&mut test)).await.unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "get_editor_tier");
    assert_eq!(frame["data"]["tier"], "baseline");
    assert_eq!(
        frame["data"]["reason"], "the language server does not run on this deployment",
        "the baseline tier must say why, so the notice can explain itself"
    );
    test.assert_no_more_frames();
}

#[tokio::test]
async fn an_lsp_request_without_a_service_answers_rather_than_dropping_the_frame() {
    let mut test = TestContext::new(|_| {}).await;
    handle_lsp_request(
        LspRequestData {
            plugin_id: "user.x".to_string(),
            frame: serde_json::json!({ "method": "initialize" }),
        },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();

    let frame = test.next_frame_json();
    assert_eq!(
        frame["type"], "lsp_request",
        "the answer must carry the request's own type; a dropped or differently typed \
         frame leaves the client's sendAndWait promise pending forever"
    );
    assert!(frame["data"]["error"].is_string());
    test.assert_no_more_frames();
}

#[tokio::test]
async fn an_lsp_notification_without_a_service_is_accepted_without_erroring() {
    let mut test = TestContext::new(|_| {}).await;
    let result = handle_lsp_notification(
        LspNotificationData {
            plugin_id: "user.x".to_string(),
            frame: serde_json::json!({ "method": "textDocument/didChange" }),
        },
        &mut ctx(&mut test),
    )
    .await;
    assert!(
        result.is_ok(),
        "a notification nobody is awaiting must not surface an error"
    );
    test.assert_no_more_frames();
}

#[tokio::test]
async fn an_lsp_notification_that_fails_still_answers_under_its_own_type() {
    let mut test = TestContext::new(|_| {}).await;
    Arc::get_mut(&mut test.app).unwrap().lsp = Arc::new(FailingLspService);

    let result = handle_lsp_notification(
        LspNotificationData {
            plugin_id: "user.x".to_string(),
            frame: serde_json::json!({ "method": "textDocument/didChange" }),
        },
        &mut ctx(&mut test),
    )
    .await;
    assert!(result.is_ok());

    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "lsp_notification");
    assert!(frame["data"]["error"].is_string());
    test.assert_no_more_frames();
}

#[tokio::test]
async fn dispatch_answers_get_editor_tier_under_its_own_type() {
    let mut test = TestContext::new(|_| {}).await;
    dispatch(
        envelope("get_editor_tier", serde_json::Value::Null),
        ctx(&mut test),
    )
    .await;

    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "get_editor_tier");
    test.assert_no_more_frames();
}

#[tokio::test]
async fn dispatch_answers_a_well_formed_lsp_request_under_its_own_type() {
    let mut test = TestContext::new(|_| {}).await;
    dispatch(
        envelope(
            "lsp_request",
            serde_json::json!({ "plugin_id": "user.x", "frame": { "method": "initialize" } }),
        ),
        ctx(&mut test),
    )
    .await;

    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "lsp_request");
    assert!(frame["data"]["error"].is_string());
    test.assert_no_more_frames();
}

#[tokio::test]
async fn dispatch_answers_a_malformed_lsp_request_under_lsp_request_not_error() {
    let mut test = TestContext::new(|_| {}).await;
    dispatch(
        envelope("lsp_request", serde_json::json!({ "plugin_id": 7 })),
        ctx(&mut test),
    )
    .await;

    let frame = test.next_frame_json();
    assert_eq!(
        frame["type"], "lsp_request",
        "a malformed payload must not fall through to MessageType::Error — no sendAndWait \
         promise matches that type, so the client would hang forever"
    );
    assert!(frame["data"]["error"].is_string());
    test.assert_no_more_frames();
}

#[tokio::test]
async fn dispatch_answers_a_malformed_lsp_notification_under_lsp_notification_not_error() {
    let mut test = TestContext::new(|_| {}).await;
    dispatch(
        envelope("lsp_notification", serde_json::json!({ "plugin_id": 7 })),
        ctx(&mut test),
    )
    .await;

    let frame = test.next_frame_json();
    assert_eq!(
        frame["type"], "lsp_notification",
        "a malformed payload must not fall through to MessageType::Error"
    );
    assert!(frame["data"]["error"].is_string());
    test.assert_no_more_frames();
}
