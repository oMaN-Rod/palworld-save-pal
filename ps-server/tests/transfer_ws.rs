mod common;

/// Error-path and unload coverage for the player-transfer surface. Nothing is
/// ever loaded, so the standalone-target auto-save path -- a real filesystem
/// write -- is never reached.
#[tokio::test]
async fn transfer_suite_error_paths_and_unload() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "load_source_save",
            "data": {"type": "gamepass", "path": "C:/whatever", "role": "source"}}),
    )
    .await;
    let unsupported = common::next_json(&mut ws).await;
    assert_eq!(unsupported["type"], "load_source_save");
    assert_eq!(
        unsupported["data"]["error"],
        "Only Steam saves are supported."
    );

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "load_source_save",
            "data": {"type": "steam", "path": "__select__", "role": "source"}}),
    )
    .await;
    let needs_desktop = common::next_json(&mut ws).await;
    assert_eq!(needs_desktop["type"], "load_source_save");
    assert_eq!(
        needs_desktop["data"]["error"],
        "Desktop mode required for file selection."
    );

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "get_source_players", "data": null}),
    )
    .await;
    let players = common::next_json(&mut ws).await;
    assert_eq!(players["type"], "get_source_players");
    assert_eq!(
        players["data"],
        serde_json::json!({"source": {}, "target": {}})
    );

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "transfer_player", "data": {
            "source_player_uid": "55555555-5555-5555-5555-555555555555",
            "target_player_uid": null}}),
    )
    .await;
    let no_target = common::next_json(&mut ws).await;
    assert_eq!(no_target["type"], "transfer_player");
    assert_eq!(no_target["data"]["error"], "No target save loaded.");

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "unload_source_save", "data": null}),
    )
    .await;
    let unloaded = common::next_json(&mut ws).await;
    assert_eq!(unloaded["type"], "unload_source_save");
    assert_eq!(unloaded["data"], serde_json::json!({"success": true}));

    server.handle.shutdown().await;
}

/// A bad source directory must answer with a soft `{"error": ...}` payload
/// rather than a hard `error` frame, and must leave the session's source unset
/// -- so a following `get_source_players` still reports `{}`.
#[tokio::test]
async fn load_source_save_bad_directory_is_a_soft_error_and_does_not_populate_source() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "load_source_save",
            "data": {"type": "steam", "path": "C:/definitely/not/a/real/save/dir", "role": "source"}}),
    )
    .await;
    let response = common::next_json(&mut ws).await;
    assert_eq!(response["type"], "load_source_save");
    assert!(response["data"]["error"].is_string());

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "get_source_players", "data": null}),
    )
    .await;
    let players = common::next_json(&mut ws).await;
    assert_eq!(
        players["data"],
        serde_json::json!({"source": {}, "target": {}})
    );

    server.handle.shutdown().await;
}
