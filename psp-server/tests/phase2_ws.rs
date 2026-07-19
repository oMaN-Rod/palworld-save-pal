//! WS handler integration tests: lazy player/guild details, pal CRUD,
//! technologies, lab research, deletes, and the save-file handlers
//! (update/download/rename/save_modded_save). Everything runs over a live
//! socket against an in-process server, so the full dispatch + emit path is
//! exercised.
//!
//! Every test runs unconditionally: the "no save loaded" cases need no fixture,
//! and the load-then-edit flows drive the committed
//! `tests/fixtures/saves/world1` save (2 players, 2 guilds).

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

/// select_save the committed world1 fixture and drain to get_guild_summaries,
/// the last frame of a successful load.
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
    // The no-save check runs BEFORE the missing-id branch, and its payload is a
    // bare STRING on a `warning` frame (not the {message, trace} error object).
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

    // This handler emits NO frame at all when no save is loaded.
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

    // This handler's no-save error is an `error` frame carrying a bare STRING —
    // a distinct shape from the dispatcher's {message, trace} object.
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

    // A player is loaded lazily, so progress_message(s) must precede the
    // get_player_details_response.
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

    // The pal box has huge capacity, so a free slot is guaranteed.
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

    // delete_pals answers with exactly get_player_summaries then
    // get_guild_summaries: no delete_pals frame, no progress_message.
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
    // Nothing may trail the pair: the next request's answer must arrive first.
    send(&mut socket, json!({"type": "get_version"})).await;
    assert_eq!(recv(&mut socket).await["type"], "get_version");

    server.shutdown().await;
}

#[tokio::test]
async fn update_save_file_without_save_errors_with_object_payload() {
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;

    // Here the no-save failure surfaces through the dispatcher, so its payload
    // is the {message, trace} OBJECT rather than a bare string.
    send(
        &mut socket,
        json!({"type": "update_save_file", "data": {"modified_pals": {}}}),
    )
    .await;
    let frame = recv(&mut socket).await;
    assert_eq!(frame["type"], "error");
    assert_eq!(frame["data"]["message"], "No save file loaded");
    assert!(frame["data"]["trace"].is_string());
    server.shutdown().await;
}

#[tokio::test]
async fn download_save_file_without_save_errors() {
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;

    send(&mut socket, json!({"type": "download_save_file"})).await;
    let frame = recv(&mut socket).await;
    assert_eq!(frame["type"], "error");
    assert_eq!(frame["data"]["message"], "No save file loaded");
    server.shutdown().await;
}

#[tokio::test]
async fn rename_world_without_save_errors() {
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;

    // data is a BARE string.
    send(
        &mut socket,
        json!({"type": "rename_world", "data": "Whatever"}),
    )
    .await;
    let frame = recv(&mut socket).await;
    assert_eq!(frame["type"], "error");
    assert_eq!(frame["data"]["message"], "No save file loaded");
    server.shutdown().await;
}

#[tokio::test]
async fn save_modded_save_without_save_errors() {
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;

    // data is a BARE world-name string. With no save loaded this errors out
    // before the disk-write path, so it cannot touch any real save.
    send(
        &mut socket,
        json!({"type": "save_modded_save", "data": "MyWorld"}),
    )
    .await;
    let frame = recv(&mut socket).await;
    assert_eq!(frame["type"], "error");
    assert_eq!(frame["data"]["message"], "No save file loaded");
    server.shutdown().await;
}

#[tokio::test]
async fn save_modded_save_accepts_null_data_from_the_steam_write_path() {
    // The frontend's Steam write path sends `data: null` (the world name is
    // GamePass-only), so the payload must deserialize and reach the handler
    // instead of failing as "invalid type: null, expected a string". Landing on
    // the same "No save file loaded" business error proves it parsed.
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;

    send(
        &mut socket,
        json!({"type": "save_modded_save", "data": null}),
    )
    .await;
    let frame = recv(&mut socket).await;
    assert_eq!(frame["type"], "error");
    assert_eq!(frame["data"]["message"], "No save file loaded");
    server.shutdown().await;
}

#[tokio::test]
async fn rename_world_renames_and_reports_old_and_new_name() {
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;

    let load_frames = load_world1(&mut socket).await;
    let loaded = load_frames
        .iter()
        .find(|f| f["type"] == "loaded_save_files")
        .expect("select_save emits loaded_save_files");
    let old_world_name = loaded["data"]["world_name"].as_str().unwrap().to_string();

    send(
        &mut socket,
        json!({"type": "rename_world", "data": "RenamedByTest"}),
    )
    .await;
    let frame = recv(&mut socket).await;
    assert_eq!(frame["type"], "rename_world");
    assert_eq!(
        frame["data"],
        format!("World renamed from '{old_world_name}' to 'RenamedByTest'")
    );
    server.shutdown().await;
}

#[tokio::test]
async fn update_then_download_save_file_round_trip() {
    let (server, _scratch) = start_test_server().await;
    let mut socket = connect(server.addr).await;

    let load_frames = load_world1(&mut socket).await;
    let player_summaries = load_frames
        .iter()
        .find(|f| f["type"] == "get_player_summaries")
        .expect("select_save emits get_player_summaries");
    let player_ids: Vec<String> = player_summaries["data"]
        .as_object()
        .expect("player summaries is an object")
        .keys()
        .cloned()
        .collect();

    // Edit an EXISTING, save-resident pal: it is already serializable, so
    // Level.sav stays byte-valid (a freshly-added pal would not). Loading the
    // player is also what puts an entry in `player_sav_bytes` for the download.
    let mut chosen: Option<(String, String, Value)> = None;
    let mut loaded_player_count: usize = 0;
    for player_id in &player_ids {
        send(
            &mut socket,
            json!({"type": "request_player_details",
                   "data": {"player_id": player_id, "origin": "edit"}}),
        )
        .await;
        let detail_frames =
            recv_until_type_or_error(&mut socket, "get_player_details_response").await;
        loaded_player_count += 1;
        let details = detail_frames.last().unwrap();
        if let Some((pal_id, pal_dto)) = details["data"]["player"]["pals"]
            .as_object()
            .and_then(|pals| pals.iter().next())
        {
            chosen = Some((player_id.clone(), pal_id.clone(), pal_dto.clone()));
            break;
        }
    }
    let (player_id, pal_id, pal_dto) =
        chosen.expect("at least one world1 player carries an editable pal");
    let mut edited_pal = pal_dto.clone();
    edited_pal["nickname"] = json!("RoundTripRenamed");

    // Progress messages must precede the terminal update_save_file frame.
    send(
        &mut socket,
        json!({"type": "update_save_file",
               "data": {"modified_pals": {pal_id: edited_pal}}}),
    )
    .await;
    let update_frames = recv_until_type_or_error(&mut socket, "update_save_file").await;
    let update_response = update_frames.last().unwrap();
    let update_index = update_frames.len() - 1;
    assert!(
        update_frames[..update_index]
            .iter()
            .any(|f| f["type"] == "progress_message"),
        "a progress_message must precede update_save_file, got {update_frames:?}"
    );
    assert_eq!(update_response["data"], "Changes saved");

    // The download's zip must carry Level.sav plus the loaded player's
    // LOWERCASE-named .sav (the on-disk saves use uppercase hex names).
    send(&mut socket, json!({"type": "download_save_file"})).await;
    let download_frames = recv_until_type_or_error(&mut socket, "download_save_file").await;
    let progress_texts: Vec<String> = download_frames
        .iter()
        .filter(|f| f["type"] == "progress_message")
        .map(|f| f["data"].as_str().unwrap_or_default().to_string())
        .collect();
    assert_eq!(
        vec![
            "Generating save files in memory... 💾".to_string(),
            "Creating ZIP archive... 🤏".to_string(),
            format!(
                "Archive created with Level.sav and {loaded_player_count} player(s) data. Encoding..."
            ),
            "Sending ZIP file to client... 🚀".to_string(),
        ],
        progress_texts,
        "download progress strings and order are a frontend contract and must not drift"
    );

    let download_response = download_frames.last().unwrap();
    let entries = download_response["data"]
        .as_array()
        .expect("download data is an ARRAY");
    assert_eq!(1, entries.len());
    let name = entries[0]["name"].as_str().unwrap();
    assert!(
        name.ends_with(".zip"),
        "download name must end .zip, got {name}"
    );
    let content_b64 = entries[0]["content"].as_str().unwrap();

    use base64::Engine as _;
    let zip_bytes = base64::engine::general_purpose::STANDARD
        .decode(content_b64)
        .expect("content is valid base64");
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).expect("valid zip");
    let names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().to_string())
        .collect();
    assert!(
        names.iter().any(|n| n == "Level.sav"),
        "zip must contain Level.sav, got {names:?}"
    );
    let lower_stem = player_id.replace('-', "").to_lowercase();
    assert!(
        names
            .iter()
            .any(|n| n == &format!("Players/{lower_stem}.sav")),
        "zip must contain the loaded player's lowercase-named .sav, got {names:?}"
    );

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
