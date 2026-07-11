mod common;

/// Error-path and unload coverage for the player-transfer WS surface (Task
/// 3E-3). Hits ONLY the error paths -- no loaded target/source -- so no disk
/// write is ever triggered (the standalone-target auto-save path is a real
/// filesystem write and deliberately NOT exercised by this test; see the
/// task report for how it's covered instead).
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

/// A non-existent Steam save directory takes the "no target loaded" branch
/// off the table via a different route than the type-check above: this
/// proves `load_steam_save_for_transfer`'s failure path (bad directory)
/// responds with `{"error": ...}` too, not the hard WS `error` frame, and
/// leaves `ctx.session.source` unset (so a subsequent `get_source_players`
/// still reports `{}`).
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
