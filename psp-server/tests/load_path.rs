//! Integration tests for the save-load path: select_save, load_zip_file and
//! sync_app_state's save branch (see src/handlers/save_file.rs, system.rs).

use std::io::Write;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

async fn start_test_server() -> (psp_server::ServerHandle, tempfile::TempDir) {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let scratch = tempfile::tempdir().unwrap();
    let config = psp_server::ServerConfig {
        host: "127.0.0.1".parse().unwrap(),
        port: 0,
        ui_dir: repo_root.join("ui"),
        data_dir: repo_root.join("data"),
        db_path: scratch.path().join("psp-rs-test.db"),
        desktop_mode: false,
    };
    let handle = psp_server::start_server(config).await.unwrap();
    (handle, scratch)
}

type WsClient =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn connect(addr: std::net::SocketAddr) -> WsClient {
    let (socket, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/ws/1"))
        .await
        .unwrap();
    socket
}

async fn send_request(socket: &mut WsClient, envelope: serde_json::Value) {
    socket
        .send(Message::Text(envelope.to_string()))
        .await
        .unwrap();
}

async fn receive_json(socket: &mut WsClient) -> serde_json::Value {
    let frame = tokio::time::timeout(Duration::from_secs(10), socket.next())
        .await
        .expect("timed out waiting for a frame")
        .expect("socket closed")
        .unwrap();
    match frame {
        Message::Text(text) => serde_json::from_str(&text).unwrap(),
        Message::Binary(bytes) => serde_json::from_slice(&bytes).unwrap(),
        other => panic!("expected a text/binary frame, got {other:?}"),
    }
}

fn build_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buffer = std::io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut buffer);
        for (name, content) in entries {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(content).unwrap();
        }
        writer.finish().unwrap();
    }
    buffer.into_inner()
}

fn zip_bytes_as_json_ints(zip_bytes: Vec<u8>) -> serde_json::Value {
    serde_json::Value::Array(zip_bytes.into_iter().map(serde_json::Value::from).collect())
}

const PLAYER_UUID: &str = "11111111-1111-1111-1111-111111111111";

#[tokio::test]
async fn test_select_save_missing_directory_errors() {
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;

    send_request(
        &mut socket,
        serde_json::json!({
            "type": "select_save",
            "data": {"type": "steam", "path": "Z:/does/not/exist/Level.sav", "local": false}
        }),
    )
    .await;

    let response = receive_json(&mut socket).await;
    assert_eq!("error", response["type"]);
    assert_eq!(
        "Level.sav file not found in the selected directory.",
        response["data"]["message"]
    );

    // Connection must survive the error.
    handle_survives(&mut socket).await;
    server.shutdown().await;
}

async fn handle_survives(socket: &mut WsClient) {
    send_request(socket, serde_json::json!({"type": "get_version"})).await;
    assert_eq!("get_version", receive_json(socket).await["type"]);
}

#[tokio::test]
async fn test_select_save_missing_players_directory_errors() {
    // Pins validate_steam_save_directory's check ORDER: with Level.sav present
    // but Players/ absent, the Players/ error must be the one reported.
    let (server, _scratch) = start_test_server().await;
    let temp_save_dir = tempfile::tempdir().unwrap();
    std::fs::write(temp_save_dir.path().join("Level.sav"), b"not-a-real-save").unwrap();
    let level_sav_path = temp_save_dir.path().join("Level.sav");

    let mut socket = connect(server.addr).await;
    send_request(
        &mut socket,
        serde_json::json!({
            "type": "select_save",
            "data": {"type": "steam", "path": level_sav_path.to_string_lossy(), "local": false}
        }),
    )
    .await;

    let response = receive_json(&mut socket).await;
    assert_eq!("error", response["type"]);
    assert_eq!(
        "Players directory not found in the selected directory.",
        response["data"]["message"]
    );
    server.shutdown().await;
}

#[tokio::test]
async fn test_select_save_no_player_saves_errors() {
    let (server, _scratch) = start_test_server().await;
    let temp_save_dir = tempfile::tempdir().unwrap();
    std::fs::write(temp_save_dir.path().join("Level.sav"), b"not-a-real-save").unwrap();
    std::fs::create_dir(temp_save_dir.path().join("Players")).unwrap();
    let level_sav_path = temp_save_dir.path().join("Level.sav");

    let mut socket = connect(server.addr).await;
    send_request(
        &mut socket,
        serde_json::json!({
            "type": "select_save",
            "data": {"type": "steam", "path": level_sav_path.to_string_lossy(), "local": false}
        }),
    )
    .await;

    let response = receive_json(&mut socket).await;
    assert_eq!("error", response["type"]);
    assert_eq!(
        "No player save files found in the Players directory.",
        response["data"]["message"]
    );
    server.shutdown().await;
}

#[tokio::test]
async fn test_select_save_garbage_level_sav_errors_cleanly_and_connection_survives() {
    // Passes every validate_steam_save_directory check, but the Level.sav bytes
    // are garbage: the load must surface a normal `error` frame, never a panic,
    // and the connection must still answer the next request.
    let (server, _scratch) = start_test_server().await;
    let temp_save_dir = tempfile::tempdir().unwrap();
    std::fs::write(temp_save_dir.path().join("Level.sav"), b"not-a-real-save").unwrap();
    let players_dir = temp_save_dir.path().join("Players");
    std::fs::create_dir(&players_dir).unwrap();
    std::fs::write(players_dir.join(format!("{PLAYER_UUID}.sav")), b"garbage").unwrap();
    let level_sav_path = temp_save_dir.path().join("Level.sav");

    let mut socket = connect(server.addr).await;
    send_request(
        &mut socket,
        serde_json::json!({
            "type": "select_save",
            "data": {"type": "steam", "path": level_sav_path.to_string_lossy(), "local": false}
        }),
    )
    .await;

    // Zero or more progress_message frames, then an error frame.
    let response = loop {
        let frame = receive_json(&mut socket).await;
        if frame["type"] != "progress_message" {
            break frame;
        }
    };
    assert_eq!("error", response["type"]);
    assert!(response["data"]["message"].is_string());

    handle_survives(&mut socket).await;
    server.shutdown().await;
}

#[tokio::test]
async fn test_select_save_non_steam_type_errors_without_touching_disk() {
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;

    send_request(
        &mut socket,
        serde_json::json!({
            "type": "select_save",
            "data": {"type": "gamepass", "path": "Z:/wherever", "local": false}
        }),
    )
    .await;

    let response = receive_json(&mut socket).await;
    assert_eq!("error", response["type"]);
    server.shutdown().await;
}

#[tokio::test]
async fn test_load_zip_file_without_level_sav_errors() {
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;

    let zip_bytes = build_zip(&[("readme.txt", b"nothing useful")]);

    send_request(
        &mut socket,
        serde_json::json!({"type": "load_zip_file", "data": zip_bytes_as_json_ints(zip_bytes)}),
    )
    .await;

    let response = receive_json(&mut socket).await;
    assert_eq!("error", response["type"]);
    assert_eq!(
        "Zip file does not contain 'Level.sav'",
        response["data"]["message"]
    );
    server.shutdown().await;
}

#[tokio::test]
async fn test_load_zip_file_without_players_folder_errors() {
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;

    let zip_bytes = build_zip(&[("Level.sav", b"garbage")]);

    send_request(
        &mut socket,
        serde_json::json!({"type": "load_zip_file", "data": zip_bytes_as_json_ints(zip_bytes)}),
    )
    .await;

    let response = receive_json(&mut socket).await;
    assert_eq!("error", response["type"]);
    assert_eq!(
        "Zip file does not contain 'Players' folder",
        response["data"]["message"]
    );
    server.shutdown().await;
}

#[tokio::test]
async fn test_load_zip_file_empty_zip_errors() {
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;

    let zip_bytes = build_zip(&[]);

    send_request(
        &mut socket,
        serde_json::json!({"type": "load_zip_file", "data": zip_bytes_as_json_ints(zip_bytes)}),
    )
    .await;

    let response = receive_json(&mut socket).await;
    assert_eq!("error", response["type"]);
    assert_eq!("Zip file is empty", response["data"]["message"]);
    server.shutdown().await;
}

#[tokio::test]
async fn test_load_zip_file_corrupt_archive_errors_cleanly_and_connection_survives() {
    // Not a zip at all: the handler must fail cleanly and the socket must still
    // answer the next request. Whether the `zip` crate errors or panics is
    // immaterial -- dispatcher::catch_handler_panic converts an escaping panic
    // into this same `error` frame shape.
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;

    let garbage = zip_bytes_as_json_ints(vec![0u8; 64]);
    send_request(
        &mut socket,
        serde_json::json!({"type": "load_zip_file", "data": garbage}),
    )
    .await;

    let response = receive_json(&mut socket).await;
    assert_eq!("error", response["type"]);
    assert!(response["data"]["message"].is_string());

    handle_survives(&mut socket).await;
    server.shutdown().await;
}

#[tokio::test]
async fn test_load_zip_file_with_traversal_style_entry_names_does_not_crash_and_stays_contained() {
    // The zip's top-level "folder" name is a traversal string, and save_id is
    // derived from it -- so a malicious name reaches the GPS-temp-file write
    // path. Request handling must neither panic nor hang (containment itself is
    // proven by zip_gps_temp_path's unit tests). The garbage Level.sav means
    // this ends in a parse error: HOW it fails is the point, not that it fails.
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;

    let evil = "../../evil";
    let zip_bytes = build_zip(&[
        (&format!("{evil}/Level.sav"), b"garbage"),
        (
            &format!("{evil}/Players/{PLAYER_UUID}.sav"),
            b"also garbage",
        ),
        (&format!("{evil}/GlobalPalStorage.sav"), b"gps garbage"),
    ]);

    send_request(
        &mut socket,
        serde_json::json!({"type": "load_zip_file", "data": zip_bytes_as_json_ints(zip_bytes)}),
    )
    .await;

    let response = loop {
        let frame = receive_json(&mut socket).await;
        if frame["type"] != "progress_message" {
            break frame;
        }
    };
    assert_eq!("error", response["type"]);

    handle_survives(&mut socket).await;
    server.shutdown().await;
}

#[tokio::test]
async fn test_sync_app_state_without_save_emits_only_settings() {
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;

    send_request(&mut socket, serde_json::json!({"type": "sync_app_state"})).await;
    let first = receive_json(&mut socket).await;
    assert_eq!("get_settings", first["type"]);

    // Prove nothing else follows: the next request's first response arrives next.
    send_request(&mut socket, serde_json::json!({"type": "sync_app_state"})).await;
    let second = receive_json(&mut socket).await;
    assert_eq!("get_settings", second["type"]);
    server.shutdown().await;
}

#[tokio::test]
async fn test_no_file_selected_produces_no_response() {
    // `no_file_selected` is a SERVER-to-client message; a client sending it must
    // fall through the dispatcher's "valid type, no handler" catch-all in
    // silence, leaving the connection usable.
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;

    send_request(&mut socket, serde_json::json!({"type": "no_file_selected"})).await;
    handle_survives(&mut socket).await;
    server.shutdown().await;
}

/// Full flow against the committed `v1_relics` Steam save fixture (Level.sav +
/// Players/). Never skips.
#[tokio::test]
async fn test_select_save_full_emission_order() {
    let save_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/saves/v1_relics");
    let level_sav_path = save_dir.join("Level.sav");

    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;

    send_request(
        &mut socket,
        serde_json::json!({
            "type": "select_save",
            "data": {"type": "steam", "path": level_sav_path.to_string_lossy(), "local": false}
        }),
    )
    .await;

    let mut received_types = Vec::new();
    let mut loaded_payload = serde_json::Value::Null;
    loop {
        let response = receive_json(&mut socket).await;
        let message_type = response["type"].as_str().unwrap().to_string();
        if message_type == "loaded_save_files" {
            loaded_payload = response["data"].clone();
        }
        received_types.push(message_type.clone());
        if message_type == "get_guild_summaries" {
            break;
        }
    }

    assert_eq!("progress_message", received_types[0]);
    let type_count = received_types.len();
    assert_eq!("loaded_save_files", received_types[type_count - 3]);
    assert_eq!("get_player_summaries", received_types[type_count - 2]);
    assert_eq!("get_guild_summaries", received_types[type_count - 1]);
    assert!(received_types[..type_count - 3]
        .iter()
        .all(|message_type| message_type == "progress_message"));

    assert_eq!("steam", loaded_payload["type"]);
    assert!(loaded_payload["size"].as_u64().unwrap() > 0);
    assert!(!loaded_payload["players"].as_array().unwrap().is_empty());
    assert!(loaded_payload["world_name"].is_string());
    assert!(loaded_payload["has_gps"].is_boolean());
    assert_eq!(
        level_sav_path.to_string_lossy(),
        loaded_payload["level"].as_str().unwrap()
    );

    // sync_app_state must now report the loaded save too.
    send_request(&mut socket, serde_json::json!({"type": "sync_app_state"})).await;
    assert_eq!("get_settings", receive_json(&mut socket).await["type"]);
    let sync_loaded = receive_json(&mut socket).await;
    assert_eq!("loaded_save_files", sync_loaded["type"]);
    assert_eq!("steam", sync_loaded["data"]["type"]);
    assert_eq!(
        "get_player_summaries",
        receive_json(&mut socket).await["type"]
    );
    assert_eq!(
        "get_guild_summaries",
        receive_json(&mut socket).await["type"]
    );

    server.shutdown().await;
}

/// Full flow for load_zip_file against the committed `v1_relics` save fixture,
/// zipped up on the fly. Never skips.
#[tokio::test]
async fn test_load_zip_file_full_emission_order() {
    let save_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/saves/v1_relics");
    let level_sav = std::fs::read(save_dir.join("Level.sav")).unwrap();
    let mut entries: Vec<(String, Vec<u8>)> = vec![("Level.sav".to_string(), level_sav)];
    let players_dir = save_dir.join("Players");
    let mut has_player = false;
    for dir_entry in std::fs::read_dir(&players_dir).unwrap() {
        let path = dir_entry.unwrap().path();
        if path.extension().and_then(|extension| extension.to_str()) == Some("sav") {
            let file_name = path.file_name().unwrap().to_string_lossy().into_owned();
            entries.push((
                format!("Players/{file_name}"),
                std::fs::read(&path).unwrap(),
            ));
            has_player = true;
        }
    }
    assert!(has_player, "v1_relics fixture must contain a player .sav");

    let entry_refs: Vec<(&str, &[u8])> = entries
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
        .collect();
    let zip_bytes = build_zip(&entry_refs);

    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;
    send_request(
        &mut socket,
        serde_json::json!({"type": "load_zip_file", "data": zip_bytes_as_json_ints(zip_bytes)}),
    )
    .await;

    let mut received_types = Vec::new();
    loop {
        let response = receive_json(&mut socket).await;
        let message_type = response["type"].as_str().unwrap().to_string();
        received_types.push(message_type.clone());
        if message_type == "get_guild_summaries" {
            break;
        }
        if message_type == "error" {
            panic!("load_zip_file failed: {response}");
        }
    }
    let type_count = received_types.len();
    assert_eq!("loaded_save_files", received_types[type_count - 3]);
    assert_eq!("get_player_summaries", received_types[type_count - 2]);
    assert_eq!("get_guild_summaries", received_types[type_count - 1]);
    assert_eq!("progress_message", received_types[type_count - 4]);

    // The receive loop stops at get_guild_summaries, so a surplus frame emitted
    // after the sequence would go unnoticed; a follow-up request whose answer
    // arrives first proves nothing trails it.
    send_request(&mut socket, serde_json::json!({"type": "sync_app_state"})).await;
    assert_eq!("get_settings", receive_json(&mut socket).await["type"]);

    server.shutdown().await;
}
