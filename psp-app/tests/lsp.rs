use std::collections::BTreeMap;
use std::sync::Arc;

use psp_app::dispatcher::{dispatch, HandlerCtx};
use psp_app::emitter::Emitter;
use psp_app::envelope::Envelope;
use psp_app::handlers::lsp::*;
use psp_app::lsp::{LspService, TierStatus};
use psp_app::messages::MessageType;
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

    fn attach_client(&self, _emitter: Emitter) {}

    async fn open_session(
        &self,
        _plugin_id: &str,
        _sources: &BTreeMap<String, String>,
    ) -> Result<String, String> {
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

/// Stands in for a language server that is up and answering: it publishes
/// diagnostics the moment it has indexed a workspace — a frame that only goes
/// anywhere if a client was attached before the session opened — and replies
/// to requests.
#[derive(Default)]
struct LiveLspService {
    client: std::sync::Mutex<Option<Emitter>>,
}

#[async_trait::async_trait]
impl LspService for LiveLspService {
    fn status(&self) -> TierStatus {
        TierStatus::Available
    }

    fn attach_client(&self, emitter: Emitter) {
        *self.client.lock().unwrap() = Some(emitter);
    }

    async fn open_session(
        &self,
        plugin_id: &str,
        _sources: &BTreeMap<String, String>,
    ) -> Result<String, String> {
        if let Some(emitter) = self.client.lock().unwrap().as_ref() {
            emitter.emit(
                MessageType::LspNotification,
                &serde_json::json!({
                    "plugin_id": plugin_id,
                    "frame": { "method": "textDocument/publishDiagnostics" },
                }),
            );
        }
        Ok("file:///workspaces/user.demo".to_string())
    }

    async fn request(
        &self,
        _plugin_id: &str,
        _frame: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        Ok(serde_json::json!({ "jsonrpc": "2.0", "id": 1, "result": "hovered" }))
    }

    async fn notify(&self, _plugin_id: &str, _frame: serde_json::Value) -> Result<(), String> {
        Ok(())
    }

    async fn shutdown(&self, _plugin_id: &str) {}
}

async fn store_plugin(test: &TestContext, id: &str) {
    psp_db::plugins::upsert(
        &*test.app.driver,
        &psp_db::plugins::NewPlugin {
            id,
            manifest: &serde_json::json!({
                "id": id,
                "name": id,
                "version": "1.0.0",
                "entry": "main.lua",
            })
            .to_string(),
            sources: &serde_json::json!({ "main.lua": "return {}" }).to_string(),
            granted_capabilities: "[]",
            bundled: false,
        },
    )
    .await
    .expect("a stored plugin");
}

#[tokio::test]
async fn opening_a_session_returns_the_workspace_uri_the_server_indexed() {
    let mut test = TestContext::new(|_| {}).await;
    store_plugin(&test, "user.demo").await;
    Arc::get_mut(&mut test.app).unwrap().lsp = Arc::new(LiveLspService::default());

    handle_open_lsp_session(
        OpenLspSessionData {
            plugin_id: "user.demo".to_string(),
        },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();

    let notification = test.next_frame_json();
    assert_eq!(
        notification["type"], "lsp_notification",
        "a client attached before the session opens receives the diagnostics the \
         language server publishes as soon as it has indexed the workspace"
    );
    assert_eq!(notification["data"]["plugin_id"], "user.demo");

    let answer = test.next_frame_json();
    assert_eq!(answer["type"], "open_lsp_session");
    assert_eq!(
        answer["data"]["root_uri"], "file:///workspaces/user.demo",
        "the client can only name documents the language server indexed if it is told \
         the workspace root"
    );
    assert!(answer["data"]["error"].is_null());
    test.assert_no_more_frames();
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
            request_id: "r-1".to_string(),
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
            serde_json::json!({
                "plugin_id": "user.x",
                "request_id": "r-1",
                "frame": { "method": "initialize" },
            }),
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
        envelope(
            "lsp_request",
            serde_json::json!({ "plugin_id": 7, "request_id": "r-1" }),
        ),
        ctx(&mut test),
    )
    .await;

    let frame = test.next_frame_json();
    assert_eq!(
        frame["type"], "lsp_request",
        "a malformed payload must not fall through to MessageType::Error — no handler \
         matches that type, so the client would hang forever"
    );
    assert!(frame["data"]["error"].is_string());
    assert_eq!(
        frame["data"]["request_id"], "r-1",
        "the client matches an answer to a request by id alone, so a refusal without \
         the id leaves that request pending forever"
    );
    test.assert_no_more_frames();
}

#[tokio::test]
async fn open_lsp_session_without_a_service_answers_under_its_own_type() {
    let mut test = TestContext::new(|_| {}).await;
    store_plugin(&test, "user.x").await;

    handle_open_lsp_session(
        OpenLspSessionData {
            plugin_id: "user.x".to_string(),
        },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "open_lsp_session");
    assert_eq!(
        frame["data"]["error"], "the language server does not run on this deployment",
        "the refusal must come from the service, not from the plugin lookup in front of it"
    );
    assert!(frame["data"]["root_uri"].is_null());
    test.assert_no_more_frames();
}

#[tokio::test]
async fn open_lsp_session_for_a_plugin_that_is_not_installed_answers_under_its_own_type() {
    let mut test = TestContext::new(|_| {}).await;
    handle_open_lsp_session(
        OpenLspSessionData {
            plugin_id: "user.x".to_string(),
        },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "open_lsp_session");
    assert_eq!(frame["data"]["error"], "plugin user.x not found");
    assert!(frame["data"]["root_uri"].is_null());
    test.assert_no_more_frames();
}

#[tokio::test]
async fn an_lsp_request_answer_echoes_the_request_id() {
    let mut test = TestContext::new(|_| {}).await;
    handle_lsp_request(
        LspRequestData {
            plugin_id: "user.x".to_string(),
            request_id: "abc-123".to_string(),
            frame: serde_json::json!({ "method": "textDocument/hover" }),
        },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "lsp_request");
    assert_eq!(
        frame["data"]["request_id"], "abc-123",
        "without the echo the client cannot tell two concurrent replies apart"
    );
    test.assert_no_more_frames();
}

#[tokio::test]
async fn an_answered_lsp_request_echoes_the_request_id_as_well() {
    let mut test = TestContext::new(|_| {}).await;
    Arc::get_mut(&mut test.app).unwrap().lsp = Arc::new(LiveLspService::default());

    handle_lsp_request(
        LspRequestData {
            plugin_id: "user.demo".to_string(),
            request_id: "abc-123".to_string(),
            frame: serde_json::json!({ "method": "textDocument/hover" }),
        },
        &mut ctx(&mut test),
    )
    .await
    .unwrap();

    let frame = test.next_frame_json();
    assert_eq!(frame["type"], "lsp_request");
    assert_eq!(frame["data"]["frame"]["result"], "hovered");
    assert_eq!(
        frame["data"]["request_id"], "abc-123",
        "a reply that arrives is no more identifiable than a refusal without the id"
    );
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
