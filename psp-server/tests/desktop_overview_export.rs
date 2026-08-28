//! Desktop-mode `export_overview_stats`: the Overview page's Export JSON
//! button. The webview ignores browser `<a download>`, so desktop writes the
//! report to a native-picked path instead. Driven through a queued (fake)
//! dialog provider.

mod common;

use psp_server::desktop_dialogs::QueuedDialogProvider;

fn level_sav_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/saves/world1/Level.sav")
}

/// Drains frames until one of `message_type` arrives, so the load's progress
/// and summary frames do not have to be enumerated here.
async fn recv_until(socket: &mut common::WsClient, message_type: &str) -> serde_json::Value {
    for _ in 0..200 {
        let frame = common::next_json(socket).await;
        if frame["type"] == message_type {
            return frame;
        }
    }
    panic!("never saw a {message_type} frame");
}

#[tokio::test]
async fn export_writes_the_report_to_the_picked_path() {
    let scratch = tempfile::tempdir().expect("tempdir");
    let out_path = scratch.path().join("overview.json");

    let server = common::start_desktop_test_server(std::sync::Arc::new(
        QueuedDialogProvider::new_with_saves(
            vec![Some(level_sav_path())],
            vec![Some(out_path.clone())],
        ),
    ))
    .await;
    let mut socket = common::connect(&server).await;

    common::send_json(
        &mut socket,
        serde_json::json!({"type": "select_save",
            "data": {"type": "steam", "local": true}}),
    )
    .await;
    recv_until(&mut socket, "get_guild_summaries").await;

    common::send_json(
        &mut socket,
        serde_json::json!({"type": "export_overview_stats",
            "data": {"file_name": "overview_world1.json"}}),
    )
    .await;
    let reply = recv_until(&mut socket, "export_overview_stats").await;

    assert_eq!(
        reply["data"]["file_path"],
        out_path.to_string_lossy().as_ref(),
        "desktop answers with the written path, not a base64 payload"
    );
    assert!(reply["data"]["message"].is_string());

    let written = std::fs::read(&out_path).expect("overview written to the picked path");
    let parsed: serde_json::Value = serde_json::from_slice(&written).expect("written file is JSON");
    assert!(
        parsed["totals"]["pals"].is_number(),
        "the report carries real stats: {parsed}"
    );

    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_canceled_dialog_emits_no_file_selected() {
    let server = common::start_desktop_test_server(std::sync::Arc::new(
        QueuedDialogProvider::new_with_saves(vec![Some(level_sav_path())], vec![None]),
    ))
    .await;
    let mut socket = common::connect(&server).await;

    common::send_json(
        &mut socket,
        serde_json::json!({"type": "select_save",
            "data": {"type": "steam", "local": true}}),
    )
    .await;
    recv_until(&mut socket, "get_guild_summaries").await;

    common::send_json(
        &mut socket,
        serde_json::json!({"type": "export_overview_stats", "data": {}}),
    )
    .await;
    let reply = recv_until(&mut socket, "no_file_selected").await;
    assert_eq!(reply["data"], "No file selected");

    server.handle.shutdown().await;
}
