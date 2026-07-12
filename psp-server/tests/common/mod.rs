//! Shared WS test-server helper (Task 3B-2). Mirrors the connect/send/recv
//! pattern established in `tests/phase2_ws.rs` and `tests/ws_integration.rs` —
//! `futures::{SinkExt, StreamExt}` (not `futures_util`, which psp-server does
//! not depend on directly) supplies the socket's `send`/`next` methods.

use futures::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

pub struct TestServer {
    pub handle: psp_server::ServerHandle,
    /// Held for RAII (deletes the temp tree on drop) AND read by tests that
    /// need the server's DB file: the SQLite database lives at
    /// `_temp_dir.path().join("psp-rs.db")` (see `start_test_server`).
    pub _temp_dir: tempfile::TempDir,
}

/// `#[allow(dead_code)]`: `tests/common/mod.rs` is compiled fresh into every
/// integration-test binary via `mod common;`; binaries that only exercise
/// desktop mode (e.g. `desktop_config.rs`) don't call this helper.
#[allow(dead_code)]
pub async fn start_test_server() -> TestServer {
    let temp_dir = tempfile::tempdir().unwrap();
    let ui_dir = temp_dir.path().join("ui");
    std::fs::create_dir_all(&ui_dir).unwrap();
    let config = psp_server::ServerConfig {
        host: "127.0.0.1".parse().unwrap(),
        port: 0,
        ui_dir,
        data_dir: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data"),
        db_path: temp_dir.path().join("psp-rs.db"),
        desktop_mode: false,
    };
    let handle = psp_server::start_server(config).await.unwrap();
    TestServer {
        handle,
        _temp_dir: temp_dir,
    }
}

/// Mirrors `start_test_server`, but runs in desktop mode with an
/// injected `FileDialogProvider` (Task P5-2). Reused by Phase 5 tests
/// that exercise desktop-only handler branches (select_save, unlock_map,
/// open_folder) against a `QueuedDialogProvider`.
#[allow(dead_code)]
pub async fn start_desktop_test_server(
    dialogs: std::sync::Arc<dyn psp_server::desktop_dialogs::FileDialogProvider>,
) -> TestServer {
    let temp_dir = tempfile::tempdir().unwrap();
    let ui_dir = temp_dir.path().join("ui");
    std::fs::create_dir_all(&ui_dir).unwrap();
    let config = psp_server::ServerConfig {
        host: "127.0.0.1".parse().unwrap(),
        port: 0,
        ui_dir,
        data_dir: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data"),
        db_path: temp_dir.path().join("psp-rs.db"),
        desktop_mode: true,
    };
    let handle = psp_server::start_server_with(config, dialogs)
        .await
        .unwrap();
    TestServer {
        handle,
        _temp_dir: temp_dir,
    }
}

pub type WsClient =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

pub async fn connect(server: &TestServer) -> WsClient {
    let url = format!("ws://{}/ws/test-client", server.handle.addr);
    let (socket, _) = tokio_tungstenite::connect_async(url).await.unwrap();
    socket
}

pub async fn send_json(socket: &mut WsClient, value: serde_json::Value) {
    socket.send(Message::Text(value.to_string())).await.unwrap();
}

pub async fn next_json(socket: &mut WsClient) -> serde_json::Value {
    loop {
        // 30s (not 10s): a WS flow that (de)compresses a real Level.sav via
        // Oodle can have a single frame delayed well past 10s when the whole
        // `cargo test --workspace` suite saturates the CPU in parallel. The
        // timeout only bounds a genuine hang, so a generous value costs nothing
        // on the happy path but removes a load-induced flake.
        match tokio::time::timeout(std::time::Duration::from_secs(30), socket.next())
            .await
            .expect("timed out waiting for a frame")
            .expect("socket closed")
            .unwrap()
        {
            Message::Text(text) => return serde_json::from_str(&text).unwrap(),
            Message::Ping(_) | Message::Pong(_) => continue,
            other => panic!("unexpected frame: {other:?}"),
        }
    }
}

/// Serializes tests that mutate the PROCESS-GLOBAL gamepass env vars and
/// restores their prior values on Drop (so a panic mid-test cannot leak a
/// temp path into a sibling test). Hold this for the whole test body.
///
/// `#[allow(dead_code)]`: `tests/common/mod.rs` is compiled fresh into every
/// integration-test binary via `mod common;`, and only `phase4_ws.rs` uses
/// this guard today — the other binaries would otherwise fail
/// `-D dead-code`.
#[allow(dead_code)]
pub static GAMEPASS_ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[allow(dead_code)]
pub struct GamepassEnvGuard {
    _lock: tokio::sync::MutexGuard<'static, ()>,
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

#[allow(dead_code)]
impl GamepassEnvGuard {
    /// Acquire the lock, snapshot the current values of the given vars, then
    /// set them. `vars` is a slice of (name, value) pairs; only the named
    /// vars are touched. Restored (or removed if previously unset) on Drop.
    pub async fn acquire(vars: &[(&'static str, std::path::PathBuf)]) -> Self {
        let lock = GAMEPASS_ENV_LOCK.lock().await;
        let mut previous = Vec::new();
        for (name, value) in vars {
            previous.push((*name, std::env::var_os(name)));
            std::env::set_var(name, value);
        }
        Self {
            _lock: lock,
            previous,
        }
    }
}

impl Drop for GamepassEnvGuard {
    fn drop(&mut self) {
        for (name, prior) in &self.previous {
            match prior {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }
}
