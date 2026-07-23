//! WS handler integration tests for the blueprint capture/place vocabulary:
//! everything runs over a live socket against an in-process server, driving
//! the committed `tests/fixtures/saves/world1` save.

use std::path::PathBuf;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

type WsClient =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

async fn start_test_server() -> (psp_server::ServerHandle, tempfile::TempDir) {
    let root = repo_root();
    let scratch = tempfile::tempdir().unwrap();
    let config = psp_server::ServerConfig {
        host: "127.0.0.1".parse().unwrap(),
        port: 0,
        ui_dir: root.join("ui"),
        data_dir: root.join("data"),
        db_path: scratch.path().join("blueprint-ws-test.db"),
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

/// Like `recv_until` but also stops (with a panic dumping the payload) on an
/// `error` frame — so a handler failure surfaces its message instead of
/// hanging the test until the receive timeout.
async fn recv_until_type_or_error(socket: &mut WsClient, stop_type: &str) -> Vec<Value> {
    let mut frames = Vec::new();
    loop {
        let frame = recv(socket).await;
        let message_type = frame["type"].as_str().unwrap_or_default().to_string();
        frames.push(frame.clone());
        if message_type == "error" && stop_type != "error" {
            panic!("unexpected error frame while awaiting {stop_type}: {frame}");
        }
        if message_type == stop_type {
            break;
        }
    }
    frames
}

/// Loads world1, finds a guild with a base, and returns (guild_id, base_id).
///
/// `request_guild_details`'s `data` is a BARE guild-id string (confirmed
/// against `psp-server/src/handlers/guilds.rs`), and its response nests the
/// `bases` map under `data.guild.bases` (the `GuildDto` shape), not directly
/// under `data.bases`.
async fn load_and_find_base(socket: &mut WsClient) -> (String, String) {
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
    let load_frames = recv_until(socket, "get_guild_summaries").await;
    let summaries = load_frames.last().unwrap();
    let guild_ids: Vec<String> = summaries["data"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();

    for guild_id in guild_ids {
        send(
            socket,
            json!({"type": "request_guild_details", "data": guild_id}),
        )
        .await;
        let detail_frames = recv_until_type_or_error(socket, "get_guild_details_response").await;
        let details = detail_frames.last().unwrap();
        let bases = details["data"]["guild"]["bases"]
            .as_object()
            .expect("guild has a bases map");
        if let Some(base_id) = bases.keys().next() {
            return (guild_id, base_id.clone());
        }
    }
    panic!("no guild in world1 owns a base");
}

#[tokio::test]
async fn capture_returns_a_handle_and_a_stamped_header() {
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;
    let (_guild_id, base_id) = load_and_find_base(&mut socket).await;

    send(
        &mut socket,
        json!({"type": "capture_base_blueprint", "data": {
            "base_id": base_id,
            "options": {"production_config": true, "structure_condition": false,
                        "container_contents": false, "worker_pals": false, "housed_pals": false,
                        "production_progress": false, "access_config": false, "base_identity": true},
            "name": "Home"
        }}),
    )
    .await;
    let frames = recv_until_type_or_error(&mut socket, "capture_base_blueprint").await;
    let frame = frames.last().unwrap();

    assert!(
        frame["data"]["handle"].as_str().is_some(),
        "response carries a handle"
    );
    let header = &frame["data"]["header"];
    assert_eq!(header["name"], "Home");
    assert!(
        header["structure_count"].as_u64().unwrap() >= 1,
        "world1 base has structures"
    );
    assert!(
        header["created_at"].as_i64().unwrap() > 0,
        "created_at is stamped, not left 0"
    );
    assert!(
        !header["game_data_version"].as_str().unwrap().is_empty(),
        "game_data_version is stamped, not left empty"
    );

    server.shutdown().await;
}

#[tokio::test]
async fn store_then_list_shows_the_row() {
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;
    let (_guild_id, base_id) = load_and_find_base(&mut socket).await;

    send(&mut socket, json!({"type": "capture_base_blueprint", "data": {
        "base_id": base_id,
        "options": {"production_config": true, "structure_condition": false,
                    "container_contents": false, "worker_pals": false, "housed_pals": false,
                    "production_progress": false, "access_config": false, "base_identity": true},
        "name": "Library Home"
    }})).await;
    let capture = recv_until_type_or_error(&mut socket, "capture_base_blueprint").await;
    let handle = capture.last().unwrap()["data"]["handle"].as_str().unwrap().to_string();

    send(&mut socket, json!({"type": "store_blueprint", "data": {"handle": handle}})).await;
    let store = recv_until_type_or_error(&mut socket, "store_blueprint").await;
    let row_id = store.last().unwrap()["data"]["id"].as_str().unwrap().to_string();
    assert!(!row_id.is_empty());

    send(&mut socket, json!({"type": "list_blueprints", "data": null})).await;
    let list = recv_until_type_or_error(&mut socket, "list_blueprints").await;
    let blueprints = list.last().unwrap()["data"]["blueprints"].as_array().unwrap();
    assert_eq!(blueprints.len(), 1, "the stored blueprint is listed");
    let row = &blueprints[0];
    assert_eq!(row["id"], row_id);
    assert_eq!(row["name"], "Library Home");
    assert!(row["structure_count"].as_u64().unwrap() >= 1);
    assert!(row.get("payload").is_none(), "list never carries the payload blob");
}

#[tokio::test]
async fn load_from_the_library_by_id_returns_a_fresh_handle() {
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;
    let (_guild_id, base_id) = load_and_find_base(&mut socket).await;

    send(&mut socket, json!({"type": "capture_base_blueprint", "data": {
        "base_id": base_id,
        "options": {"production_config": true, "structure_condition": false,
                    "container_contents": false, "worker_pals": false, "housed_pals": false,
                    "production_progress": false, "access_config": false, "base_identity": true},
        "name": "Roundtrip"
    }})).await;
    let capture = recv_until_type_or_error(&mut socket, "capture_base_blueprint").await;
    let handle = capture.last().unwrap()["data"]["handle"].as_str().unwrap().to_string();

    send(&mut socket, json!({"type": "store_blueprint", "data": {"handle": handle}})).await;
    let store = recv_until_type_or_error(&mut socket, "store_blueprint").await;
    let row_id = store.last().unwrap()["data"]["id"].as_str().unwrap().to_string();

    send(&mut socket, json!({"type": "load_blueprint", "data": {"id": row_id}})).await;
    let load = recv_until_type_or_error(&mut socket, "load_blueprint").await;
    let loaded = load.last().unwrap();
    assert!(loaded["data"]["handle"].as_str().is_some(), "load returns a handle");
    assert_eq!(loaded["data"]["header"]["name"], "Roundtrip");
    assert!(loaded["data"]["header"]["structure_count"].as_u64().unwrap() >= 1);
}

#[tokio::test]
async fn load_of_an_unknown_id_is_an_error_frame() {
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;
    send(&mut socket, json!({"type": "load_blueprint",
        "data": {"id": "00000000-0000-0000-0000-000000000000"}})).await;
    let frame = recv(&mut socket).await;
    assert_eq!(frame["type"], "error");
}

#[tokio::test]
async fn export_returns_bytes_that_decode_back_to_the_blueprint() {
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;
    let (_guild_id, base_id) = load_and_find_base(&mut socket).await;

    send(&mut socket, json!({"type": "capture_base_blueprint", "data": {
        "base_id": base_id,
        "options": {"production_config": true, "structure_condition": false,
                    "container_contents": false, "worker_pals": false, "housed_pals": false,
                    "production_progress": false, "access_config": false, "base_identity": true},
        "name": "Exported"
    }})).await;
    let capture = recv_until_type_or_error(&mut socket, "capture_base_blueprint").await;
    let handle = capture.last().unwrap()["data"]["handle"].as_str().unwrap().to_string();

    for format in ["psp", "json"] {
        send(&mut socket, json!({"type": "export_blueprint_file",
            "data": {"handle": handle, "format": format}})).await;
        let export = recv_until_type_or_error(&mut socket, "export_blueprint_file").await;
        let payload = export.last().unwrap()["data"].as_array().unwrap();
        let entry = &payload[0];
        let name = entry["name"].as_str().unwrap();
        assert!(name.ends_with(&format!(".{format}")), "filename carries the format extension");
        let content = entry["content"].as_str().unwrap();
        assert!(!content.is_empty(), "{format} export has bytes");
    }
}

#[tokio::test]
async fn validate_reports_findings_and_a_blocking_flag() {
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;
    let (guild_id, base_id) = load_and_find_base(&mut socket).await;

    send(&mut socket, json!({"type": "capture_base_blueprint", "data": {
        "base_id": base_id,
        "options": {"production_config": true, "structure_condition": false,
                    "container_contents": false, "worker_pals": false, "housed_pals": false,
                    "production_progress": false, "access_config": false, "base_identity": true},
        "name": "Validate"
    }})).await;
    let capture = recv_until_type_or_error(&mut socket, "capture_base_blueprint").await;
    let handle = capture.last().unwrap()["data"]["handle"].as_str().unwrap().to_string();

    send(&mut socket, json!({"type": "validate_blueprint_placement", "data": {
        "handle": handle,
        "anchor": {"x": 100000.0, "y": 100000.0, "z": 500.0, "yaw": 0.0},
        "mode": "new_base",
        "target_guild": guild_id
    }})).await;
    let frames = recv_until_type_or_error(&mut socket, "validate_blueprint_placement").await;
    let data = &frames.last().unwrap()["data"];
    assert!(data["findings"].is_array(), "findings is a list");
    assert!(data["has_blocking"].is_boolean(), "has_blocking is a bool");
    // Each finding is the {severity, code, message} shape.
    if let Some(first) = data["findings"].as_array().unwrap().first() {
        assert!(first["severity"].as_str().is_some());
        assert!(first["code"].as_str().is_some());
    }
}

#[tokio::test]
async fn place_adds_a_base_and_reports_the_count() {
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;
    let (guild_id, base_id) = load_and_find_base(&mut socket).await;

    // A target player: reuse the guild id's owning player is not available here,
    // so send the nil uuid — placement rebinds ownership to whatever is given,
    // and the wire path does not require the player to pre-exist for this assertion.
    send(&mut socket, json!({"type": "capture_base_blueprint", "data": {
        "base_id": base_id,
        "options": {"production_config": true, "structure_condition": false,
                    "container_contents": false, "worker_pals": false, "housed_pals": false,
                    "production_progress": false, "access_config": false, "base_identity": true},
        "name": "Placed"
    }})).await;
    let capture = recv_until_type_or_error(&mut socket, "capture_base_blueprint").await;
    let handle = capture.last().unwrap()["data"]["handle"].as_str().unwrap().to_string();

    send(&mut socket, json!({"type": "place_blueprint", "data": {
        "handle": handle,
        "anchor": {"x": 300000.0, "y": 300000.0, "z": 500.0, "yaw": 0.0},
        "mode": "new_base",
        "target_player": "00000000-0000-0000-0000-000000000000",
        "target_guild": guild_id,
        "override_warnings": true
    }})).await;
    let frames = recv_until_type_or_error(&mut socket, "place_blueprint").await;
    let data = &frames.last().unwrap()["data"];
    assert!(data["base_id"].as_str().is_some(), "placement founds a base");
    assert!(data["structures_placed"].as_u64().unwrap() >= 1, "structures were placed");
}

#[tokio::test]
async fn place_with_a_missing_target_guild_is_an_error() {
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;
    let (_guild_id, base_id) = load_and_find_base(&mut socket).await;

    send(&mut socket, json!({"type": "capture_base_blueprint", "data": {
        "base_id": base_id,
        "options": {"production_config": true, "structure_condition": false,
                    "container_contents": false, "worker_pals": false, "housed_pals": false,
                    "production_progress": false, "access_config": false, "base_identity": true},
        "name": "NoGuild"
    }})).await;
    let capture = recv_until_type_or_error(&mut socket, "capture_base_blueprint").await;
    let handle = capture.last().unwrap()["data"]["handle"].as_str().unwrap().to_string();

    send(&mut socket, json!({"type": "place_blueprint", "data": {
        "handle": handle,
        "anchor": {"x": 300000.0, "y": 300000.0, "z": 500.0, "yaw": 0.0},
        "mode": "new_base",
        "target_player": "00000000-0000-0000-0000-000000000000",
        "override_warnings": true
    }})).await;
    let frame = recv(&mut socket).await;
    assert_eq!(frame["type"], "error", "new_base without target_guild is refused");
}

#[tokio::test]
async fn load_from_uploaded_content_returns_a_handle() {
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;
    let (_guild_id, base_id) = load_and_find_base(&mut socket).await;

    send(&mut socket, json!({"type": "capture_base_blueprint", "data": {
        "base_id": base_id,
        "options": {"production_config": true, "structure_condition": false,
                    "container_contents": false, "worker_pals": false, "housed_pals": false,
                    "production_progress": false, "access_config": false, "base_identity": true},
        "name": "Uploaded"
    }})).await;
    let capture = recv_until_type_or_error(&mut socket, "capture_base_blueprint").await;
    let handle = capture.last().unwrap()["data"]["handle"].as_str().unwrap().to_string();

    // Export to get the browser base64 body, then feed that body straight back
    // through the upload path — no DB id involved, so the round-trip proves the
    // content branch decodes what the export branch encoded.
    for format in ["psp", "json"] {
        send(&mut socket, json!({"type": "export_blueprint_file",
            "data": {"handle": handle, "format": format}})).await;
        let export = recv_until_type_or_error(&mut socket, "export_blueprint_file").await;
        let content = export.last().unwrap()["data"][0]["content"].as_str().unwrap().to_string();

        send(&mut socket, json!({"type": "load_blueprint",
            "data": {"content": content, "format": format}})).await;
        let load = recv_until_type_or_error(&mut socket, "load_blueprint").await;
        let loaded = load.last().unwrap();
        assert!(loaded["data"]["handle"].as_str().is_some(), "{format} upload returns a handle");
        assert_eq!(loaded["data"]["header"]["name"], "Uploaded",
            "{format} upload decoded the header back");
        assert!(loaded["data"]["header"]["structure_count"].as_u64().unwrap() >= 1);
    }
}

#[tokio::test]
async fn load_of_malformed_base64_content_is_an_error_frame() {
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;
    send(&mut socket, json!({"type": "load_blueprint",
        "data": {"content": "!!!! not base64 !!!!", "format": "psp"}})).await;
    let frame = recv(&mut socket).await;
    assert_eq!(frame["type"], "error", "undecodable content is refused, not decoded to garbage");
}
