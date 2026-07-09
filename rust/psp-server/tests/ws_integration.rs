use std::path::PathBuf;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use serde_json::Value;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use psp_server::{start_server, ServerConfig, ServerHandle};

async fn start_test_server(temp_dir: &tempfile::TempDir) -> ServerHandle {
    start_server(ServerConfig {
        host: "127.0.0.1".parse().unwrap(),
        port: 0,
        ui_dir: temp_dir.path().join("ui"),
        data_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data"),
        db_path: temp_dir.path().join("test.db"),
        desktop_mode: false,
    })
    .await
    .unwrap()
}

type WsClient =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn connect(handle: &ServerHandle) -> WsClient {
    let (socket, _) = connect_async(format!("ws://{}/ws/1751000000000", handle.addr))
        .await
        .unwrap();
    socket
}

async fn next_json(socket: &mut WsClient) -> Value {
    let frame = tokio::time::timeout(Duration::from_secs(10), socket.next())
        .await
        .expect("timed out waiting for a frame")
        .expect("socket closed")
        .unwrap();
    serde_json::from_str(frame.to_text().unwrap()).unwrap()
}

#[tokio::test]
async fn get_version_round_trip() {
    let temp_dir = tempfile::tempdir().unwrap();
    let handle = start_test_server(&temp_dir).await;
    let mut socket = connect(&handle).await;

    socket
        .send(Message::Text(r#"{"type":"get_version"}"#.into()))
        .await
        .unwrap();
    let response = next_json(&mut socket).await;
    assert_eq!(response["type"], "get_version");
    assert_eq!(response["data"], env!("CARGO_PKG_VERSION"));
    handle.shutdown().await;
}

#[tokio::test]
async fn malformed_json_yields_string_error_and_socket_survives() {
    let temp_dir = tempfile::tempdir().unwrap();
    let handle = start_test_server(&temp_dir).await;
    let mut socket = connect(&handle).await;

    socket
        .send(Message::Text("{ not json".into()))
        .await
        .unwrap();
    let error_frame = next_json(&mut socket).await;
    assert_eq!(error_frame["type"], "error");
    // ws/manager.py sends a STRING payload for JSON decode errors — this is
    // Stage 1 (JSON syntax) and must stay distinguishable from Stage 2
    // (envelope shape) below, which sends an OBJECT payload instead.
    assert!(
        error_frame["data"].is_string(),
        "Stage 1 (JSON syntax) errors must carry a STRING payload: {error_frame}"
    );
    assert!(error_frame["data"]
        .as_str()
        .unwrap()
        .starts_with("Invalid JSON received:"));

    // Connection must survive the error.
    socket
        .send(Message::Text(r#"{"type":"get_version"}"#.into()))
        .await
        .unwrap();
    assert_eq!(next_json(&mut socket).await["type"], "get_version");
    handle.shutdown().await;
}

#[tokio::test]
async fn envelope_shape_error_yields_object_error_with_message_and_trace() {
    let temp_dir = tempfile::tempdir().unwrap();
    let handle = start_test_server(&temp_dir).await;
    let mut socket = connect(&handle).await;

    // Syntactically valid JSON, but missing the required "type" field: this
    // fails `Envelope` deserialization (Stage 2), not JSON parsing (Stage 1).
    // ws/manager.py's matching branch (the generic `except Exception`, lines
    // 43-51) sends {"message": ..., "trace": ...} — an OBJECT payload, not the
    // plain string Stage 1 uses above.
    socket
        .send(Message::Text(r#"{"data":1}"#.into()))
        .await
        .unwrap();
    let error_frame = next_json(&mut socket).await;
    assert_eq!(error_frame["type"], "error");
    assert!(
        error_frame["data"].is_object(),
        "Stage 2 (envelope shape) errors must carry an OBJECT payload: {error_frame}"
    );
    assert!(error_frame["data"]["message"].is_string());
    assert!(error_frame["data"]["trace"].is_string());

    // Connection must survive the error, same as Stage 1.
    socket
        .send(Message::Text(r#"{"type":"get_version"}"#.into()))
        .await
        .unwrap();
    assert_eq!(next_json(&mut socket).await["type"], "get_version");
    handle.shutdown().await;
}

#[tokio::test]
async fn connection_teardown_decrements_live_connection_gauge() {
    // Proves the reader loop / writer task actually terminate on disconnect,
    // sleep-free. `ServerHandle::shutdown` cannot prove this on its own: axum's
    // `WebSocketUpgrade::on_upgrade` hands the socket to an independently
    // `tokio::spawn`ed task, decoupled from the HTTP connection future that
    // graceful shutdown waits on — that future resolves once the HTTP/1 upgrade
    // handshake completes, not when `connection_loop`/the writer task exit.
    let temp_dir = tempfile::tempdir().unwrap();
    let mut handle = start_test_server(&temp_dir).await;
    let mut socket = connect(&handle).await;

    socket
        .send(Message::Text(r#"{"type":"get_version"}"#.into()))
        .await
        .unwrap();
    assert_eq!(next_json(&mut socket).await["type"], "get_version");

    // By the time the response above was received, connection_loop must have
    // already incremented the gauge (the increment happens before the reader
    // loop that produced the response even starts reading).
    assert_eq!(*handle.live_connections.borrow(), 1);

    drop(socket);

    // Event-driven wait for the gauge to drop back to 0 — no polling, no sleep.
    // The outer timeout is purely a hang-guard, same role as next_json's.
    tokio::time::timeout(Duration::from_secs(10), async {
        while *handle.live_connections.borrow() != 0 {
            handle.live_connections.changed().await.unwrap();
        }
    })
    .await
    .expect("timed out waiting for connection teardown to decrement live_connections");

    handle.shutdown().await;
}

#[tokio::test]
async fn unknown_type_is_silent() {
    let temp_dir = tempfile::tempdir().unwrap();
    let handle = start_test_server(&temp_dir).await;
    let mut socket = connect(&handle).await;

    socket
        .send(Message::Text(r#"{"type":"bogus_type"}"#.into()))
        .await
        .unwrap();
    // No response for the unknown type: the NEXT response answers get_version.
    socket
        .send(Message::Text(r#"{"type":"get_version"}"#.into()))
        .await
        .unwrap();
    assert_eq!(next_json(&mut socket).await["type"], "get_version");
    handle.shutdown().await;
}

#[tokio::test]
async fn registered_but_unimplemented_type_is_silent() {
    // Deliberate strengthening beyond the brief: `unknown_type_is_silent` above
    // only proves silence for a wire string with no MessageType variant at all
    // (dispatch's `from_wire` returns None). Silence must also hold for a wire
    // string that DOES have a variant but no Phase-0 handler arm (`route`'s
    // catch-all `other =>` branch) — a distinct code path in dispatcher.rs.
    // `get_ups_pals` is registered in MessageType but unrouted in Phase 0.
    let temp_dir = tempfile::tempdir().unwrap();
    let handle = start_test_server(&temp_dir).await;
    let mut socket = connect(&handle).await;

    socket
        .send(Message::Text(
            r#"{"type":"get_ups_pals","data":{"offset":0,"limit":30}}"#.into(),
        ))
        .await
        .unwrap();
    socket
        .send(Message::Text(r#"{"type":"get_version"}"#.into()))
        .await
        .unwrap();
    assert_eq!(next_json(&mut socket).await["type"], "get_version");
    handle.shutdown().await;
}

#[tokio::test]
async fn update_settings_persists_and_answers_with_get_settings() {
    let temp_dir = tempfile::tempdir().unwrap();
    let handle = start_test_server(&temp_dir).await;
    let mut socket = connect(&handle).await;

    let update = r#"{"type":"update_settings","data":{"language":"fr","clone_prefix":"©️",
        "new_pal_prefix":"🆕","debug_mode":false,"cheat_mode":false}}"#;
    socket.send(Message::Text(update.into())).await.unwrap();
    let response = next_json(&mut socket).await;
    assert_eq!(response["type"], "get_settings");
    assert_eq!(response["data"]["language"], "fr");

    // A second connection gets its own Session but shares the DB.
    let mut second_socket = connect(&handle).await;
    second_socket
        .send(Message::Text(r#"{"type":"get_settings"}"#.into()))
        .await
        .unwrap();
    assert_eq!(
        next_json(&mut second_socket).await["data"]["language"],
        "fr"
    );
    handle.shutdown().await;
}
