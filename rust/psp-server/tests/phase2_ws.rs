//! Phase-2 WS handler integration tests (Task 13): lazy details, pal CRUD,
//! technologies, lab research, deletes. Boots the real in-process server
//! (mirroring `tests/load_path.rs`) and drives everything over a live
//! WebSocket, so every assertion exercises the full dispatch + emit path.
//!
//! Most tests are UNCONDITIONAL: the "no save loaded" cases need no fixture,
//! and the load-then-edit flow drives the committed `tests/fixtures/saves/world1`
//! save (2 players, 2 guilds — always present, never `PSP_TEST_SAVE_DIR`-gated).

use std::path::PathBuf;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

type WsClient =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

async fn start_test_server() -> (psp_server::ServerHandle, tempfile::TempDir) {
    let root = repo_root();
    let scratch = tempfile::tempdir().unwrap();
    let config = psp_server::ServerConfig {
        host: "127.0.0.1".parse().unwrap(),
        port: 0,
        ui_dir: root.join("ui"),
        data_dir: root.join("data"),
        db_path: scratch.path().join("phase2-ws-test.db"),
        desktop_mode: false,
    };
    let handle = psp_server::start_server(config).await.unwrap();
    (handle, scratch)
}

async fn connect(addr: std::net::SocketAddr) -> WsClient {
    let (socket, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/ws/1"))
        .await
        .unwrap();
    socket
}

async fn send(socket: &mut WsClient, envelope: Value) {
    socket
        .send(Message::Text(envelope.to_string()))
        .await
        .unwrap();
}

async fn recv(socket: &mut WsClient) -> Value {
    let frame = tokio::time::timeout(Duration::from_secs(15), socket.next())
        .await
        .expect("timed out waiting for a frame")
        .expect("socket closed")
        .unwrap();
    match frame {
        Message::Text(text) => serde_json::from_str(&text).unwrap(),
        Message::Binary(bytes) => serde_json::from_slice(&bytes).unwrap(),
        other => panic!("expected text/binary frame, got {other:?}"),
    }
}

/// Reads frames until one whose `type` equals `stop_type`, returning every
/// frame read (including the stop frame).
async fn recv_until(socket: &mut WsClient, stop_type: &str) -> Vec<Value> {
    let mut frames = Vec::new();
    loop {
        let frame = recv(socket).await;
        let message_type = frame["type"].as_str().unwrap_or_default().to_string();
        frames.push(frame);
        if message_type == stop_type {
            break;
        }
    }
    frames
}

/// select_save the committed world1 fixture and drain to get_guild_summaries.
/// Returns the collected frames.
async fn load_world1(socket: &mut WsClient) -> Vec<Value> {
    let level_sav = repo_root()
        .join("tests/fixtures/saves/world1/Level.sav")
        .to_string_lossy()
        .into_owned();
    send(
        socket,
        json!({"type": "select_save",
               "data": {"type": "steam", "path": level_sav, "local": false}}),
    )
    .await;
    recv_until(socket, "get_guild_summaries").await
}

fn first_key(payload: &Value) -> String {
    payload
        .as_object()
        .expect("summary payload is an object")
        .keys()
        .next()
        .expect("at least one summary")
        .clone()
}

// ---------------------------------------------------------------------------
// No-save-loaded cases (unconditional, no fixture needed).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn add_pal_without_save_warns() {
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;

    send(
        &mut socket,
        json!({"type": "add_pal", "data": {"character_id": "SheepBall", "nickname": "x"}}),
    )
    .await;
    let frame = recv(&mut socket).await;
    // pal_handler.py checks "no save" BEFORE the missing-id branch, and the
    // warning payload is a bare STRING (not the {message, trace} object).
    assert_eq!(frame["type"], "warning");
    assert_eq!(frame["data"], "No save file loaded");
    server.shutdown().await;
}

#[tokio::test]
async fn get_pal_summaries_without_save_errors_with_object_payload() {
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;

    send(&mut socket, json!({"type": "get_pal_summaries"})).await;
    let frame = recv(&mut socket).await;
    // Handler-local {"error": ...} payload on its OWN response type — NOT the
    // dispatcher's `error`/{message, trace} shape.
    assert_eq!(frame["type"], "get_pal_summaries");
    assert_eq!(frame["data"], json!({"error": "No save file loaded"}));
    server.shutdown().await;
}

#[tokio::test]
async fn set_technology_data_without_save_is_silent() {
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;

    // technologies_handler.py returns with NO frame when no save is loaded.
    send(
        &mut socket,
        json!({"type": "set_technology_data",
               "data": {"playerID": "11111111-1111-1111-1111-111111111111",
                        "technologies": [], "techPoints": 0, "ancientTechPoints": 0}}),
    )
    .await;
    // The only way to prove silence is that the NEXT request answers first.
    send(&mut socket, json!({"type": "get_version"})).await;
    assert_eq!(recv(&mut socket).await["type"], "get_version");
    server.shutdown().await;
}

#[tokio::test]
async fn update_lab_research_without_save_errors_with_plain_string() {
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;

    // lab_research_handler.py emits MessageType.ERROR with a bare STRING (the
    // second, distinct error shape) — not the dispatcher's {message, trace}.
    send(
        &mut socket,
        json!({"type": "update_lab_research",
               "data": {"guild_id": "11111111-1111-1111-1111-111111111111",
                        "research_updates": []}}),
    )
    .await;
    let frame = recv(&mut socket).await;
    assert_eq!(frame["type"], "error");
    assert!(
        frame["data"].is_string(),
        "update_lab_research no-save error must be a plain string, got {frame}"
    );
    assert_eq!(frame["data"], "No save file loaded.");
    server.shutdown().await;
}

#[tokio::test]
async fn request_guild_details_without_save_errors() {
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;

    // data is a BARE uuid string (not an object).
    send(
        &mut socket,
        json!({"type": "request_guild_details",
               "data": "11111111-1111-1111-1111-111111111111"}),
    )
    .await;
    let frame = recv(&mut socket).await;
    assert_eq!(frame["type"], "get_guild_details_response");
    assert_eq!(frame["data"], json!({"error": "No save file loaded"}));
    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// Committed-fixture (world1) flows — always run.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn player_details_then_add_then_delete_flow() {
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;

    let load_frames = load_world1(&mut socket).await;
    let player_summaries = load_frames
        .iter()
        .find(|f| f["type"] == "get_player_summaries")
        .expect("select_save emits get_player_summaries");
    let player_id = first_key(&player_summaries["data"]);

    // --- request_player_details: progress_message(s) must precede the
    //     get_player_details_response (the player is lazily loaded on demand).
    send(
        &mut socket,
        json!({"type": "request_player_details",
               "data": {"player_id": player_id, "origin": "edit"}}),
    )
    .await;
    let detail_frames = recv_until(&mut socket, "get_player_details_response").await;
    let response = detail_frames.last().unwrap();
    let response_index = detail_frames.len() - 1;
    assert!(
        detail_frames[..response_index]
            .iter()
            .any(|f| f["type"] == "progress_message"),
        "a progress_message must precede get_player_details_response, got {detail_frames:?}"
    );
    assert_eq!(response["data"]["player_id"], player_id);
    assert_eq!(response["data"]["origin"], "edit");
    assert!(
        response["data"]["player"]["pals"].is_object(),
        "player.pals must be an object, got {}",
        response["data"]["player"]["pals"]
    );
    let pal_box_id = response["data"]["player"]["pal_box_id"]
        .as_str()
        .expect("player has a pal_box_id")
        .to_string();

    // --- add_pal into the pal box (huge capacity → always has a free slot).
    send(
        &mut socket,
        json!({"type": "add_pal",
               "data": {"player_id": player_id, "character_id": "SheepBall",
                        "nickname": "WsTest", "container_id": pal_box_id}}),
    )
    .await;
    let add_frame = recv(&mut socket).await;
    assert_eq!(add_frame["type"], "add_pal");
    assert_eq!(add_frame["data"]["player_id"], player_id);
    let new_pal_id = add_frame["data"]["pal"]["instance_id"]
        .as_str()
        .expect("added pal has an instance_id")
        .to_string();
    assert_eq!(add_frame["data"]["pal"]["character_id"], "SheepBall");

    // --- delete_pals: NO delete_pals frame, exactly get_player_summaries then
    //     get_guild_summaries, and NO spurious progress_message.
    send(
        &mut socket,
        json!({"type": "delete_pals",
               "data": {"player_id": player_id, "pal_ids": [new_pal_id]}}),
    )
    .await;
    let first = recv(&mut socket).await;
    assert_eq!(
        first["type"], "get_player_summaries",
        "delete_pals must emit get_player_summaries first, got {first}"
    );
    let second = recv(&mut socket).await;
    assert_eq!(
        second["type"], "get_guild_summaries",
        "delete_pals must emit get_guild_summaries second, got {second}"
    );
    // Prove nothing else (no delete_pals frame, no progress) trails the pair:
    // the next request's answer must arrive immediately.
    send(&mut socket, json!({"type": "get_version"})).await;
    assert_eq!(recv(&mut socket).await["type"], "get_version");

    server.shutdown().await;
}

#[tokio::test]
async fn request_guild_details_returns_guild() {
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;

    let load_frames = load_world1(&mut socket).await;
    let guild_summaries = load_frames
        .iter()
        .find(|f| f["type"] == "get_guild_summaries")
        .expect("select_save emits get_guild_summaries");
    let guild_id = first_key(&guild_summaries["data"]);

    // data is a BARE uuid string.
    send(
        &mut socket,
        json!({"type": "request_guild_details", "data": guild_id}),
    )
    .await;
    let frame = recv_until(&mut socket, "get_guild_details_response").await;
    let response = frame.last().unwrap();
    assert_eq!(response["data"]["guild_id"], guild_id);
    assert!(
        response["data"]["guild"].is_object(),
        "guild details payload must include the guild object, got {response}"
    );

    server.shutdown().await;
}
