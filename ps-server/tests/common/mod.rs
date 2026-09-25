//! Shared WS test-server helpers. This module is compiled fresh into every
//! integration-test binary via `mod common;`, so helpers a given binary
//! doesn't call need `#[allow(dead_code)]`.

use futures::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

#[allow(dead_code)]
pub mod mock_mod;

#[allow(unused_imports)]
pub use mock_mod::{spawn_mock_mod, MockMod};

pub struct TestServer {
    pub handle: ps_server::ServerHandle,
    /// Deletes the temp tree on drop; also read by tests that need the
    /// server's SQLite file at `_temp_dir.path().join("ps-rs.db")`.
    pub _temp_dir: tempfile::TempDir,
}

/// Startup and the background reconciler both read `PS_BRIDGE_ENDPOINT_DIR`
/// (falling back to the real per-user Palworld directory when unset). A test
/// that cares about bridge discovery already sets this var itself (guarded by
/// `BridgeEnvGuard`, held for the whole test); this fills in an isolated,
/// always-empty directory for the many tests that never touch the bridge at
/// all, so they can't end up scanning -- and auto-connecting to -- a real
/// running game on the developer's machine.
///
/// Set once per process and never unset. Several such tests can run
/// concurrently in one binary without a `BridgeEnvGuard` of their own (they
/// have no reason to take one); if teardown of any single test's server
/// removed the var, it could pull it out from under another test's
/// still-running server, whose background reconciler keeps re-reading the
/// var for the server's entire lifetime -- turning a one-time startup read
/// into a sustained window onto the real directory. A `OnceLock` shared by
/// the whole process sidesteps that: every such test converges on the same
/// directory, and nothing ever un-sets it.
static HERMETIC_BRIDGE_ENDPOINT_DIR: std::sync::OnceLock<tempfile::TempDir> =
    std::sync::OnceLock::new();

fn ensure_hermetic_bridge_endpoint_dir() {
    if std::env::var_os("PS_BRIDGE_ENDPOINT_DIR").is_some() {
        return;
    }
    let dir = HERMETIC_BRIDGE_ENDPOINT_DIR.get_or_init(|| tempfile::tempdir().unwrap());
    std::env::set_var("PS_BRIDGE_ENDPOINT_DIR", dir.path());
}

/// Starts a web-mode server on an ephemeral port (`port: 0`).
#[allow(dead_code)]
pub async fn start_test_server() -> TestServer {
    ensure_hermetic_bridge_endpoint_dir();
    let temp_dir = tempfile::tempdir().unwrap();
    let ui_dir = temp_dir.path().join("ui");
    std::fs::create_dir_all(&ui_dir).unwrap();
    let config = ps_server::ServerConfig {
        host: "127.0.0.1".parse().unwrap(),
        port: Some(0),
        ui_dir,
        data_dir: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data"),
        db_path: temp_dir.path().join("ps-rs.db"),
        desktop_mode: false,
        hosted: false,
        websuite: false,
        allow_network_edits: true,
    };
    let handle = ps_server::start_server(config).await.unwrap();
    TestServer {
        handle,
        _temp_dir: temp_dir,
    }
}

/// Same, but in desktop mode with an injected `FileDialogProvider`, so
/// desktop-only handler branches can run headless.
#[allow(dead_code)]
pub async fn start_desktop_test_server(
    dialogs: std::sync::Arc<dyn ps_server::desktop_dialogs::FileDialogProvider>,
) -> TestServer {
    ensure_hermetic_bridge_endpoint_dir();
    let temp_dir = tempfile::tempdir().unwrap();
    let ui_dir = temp_dir.path().join("ui");
    std::fs::create_dir_all(&ui_dir).unwrap();
    let config = ps_server::ServerConfig {
        host: "127.0.0.1".parse().unwrap(),
        port: Some(0),
        ui_dir,
        data_dir: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data"),
        db_path: temp_dir.path().join("ps-rs.db"),
        desktop_mode: true,
        hosted: false,
        websuite: false,
        allow_network_edits: true,
    };
    let handle = ps_server::start_server_with(config, dialogs).await.unwrap();
    TestServer {
        handle,
        _temp_dir: temp_dir,
    }
}

#[allow(dead_code)]
pub type WsClient =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

#[allow(dead_code)]
pub type TestClient = WsClient;

#[allow(dead_code)]
pub async fn connect(server: &TestServer) -> WsClient {
    let url = format!("ws://{}/ws/test-client", server.handle.addr);
    let (socket, _) = tokio_tungstenite::connect_async(url).await.unwrap();
    socket
}

#[allow(dead_code)]
pub async fn send_json(socket: &mut WsClient, value: serde_json::Value) {
    socket.send(Message::Text(value.to_string())).await.unwrap();
}

#[allow(dead_code)]
pub async fn next_json(socket: &mut WsClient) -> serde_json::Value {
    loop {
        // 30s: a flow that (de)compresses a real Level.sav via Oodle can stall
        // a frame well past 10s when the suite saturates the CPU in parallel.
        // The timeout only bounds a genuine hang.
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
/// restores their prior values on Drop, so a panic mid-test cannot leak a temp
/// path into a sibling test. Hold this for the whole test body.
#[allow(dead_code)]
pub static GAMEPASS_ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[allow(dead_code)]
pub struct GamepassEnvGuard {
    _lock: tokio::sync::MutexGuard<'static, ()>,
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

#[allow(dead_code)]
impl GamepassEnvGuard {
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

/// Serializes tests that mutate the PROCESS-GLOBAL bridge env vars (e.g.
/// `PS_BRIDGE_ENDPOINT_DIR`) and restores their prior values on Drop.
#[allow(dead_code)]
pub static BRIDGE_ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[allow(dead_code)]
pub struct BridgeEnvGuard {
    _lock: tokio::sync::MutexGuard<'static, ()>,
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

#[allow(dead_code)]
impl BridgeEnvGuard {
    pub async fn acquire(vars: &[(&'static str, Option<&str>)]) -> Self {
        let lock = BRIDGE_ENV_LOCK.lock().await;
        let mut previous = Vec::new();
        for (name, value) in vars {
            previous.push((*name, std::env::var_os(name)));
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
        Self {
            _lock: lock,
            previous,
        }
    }
}

impl Drop for BridgeEnvGuard {
    fn drop(&mut self) {
        for (name, prior) in &self.previous {
            match prior {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }
}

#[allow(dead_code)]
pub async fn kill_mock(
    cancel: tokio_util::sync::CancellationToken,
    handle: tokio::task::JoinHandle<()>,
) {
    cancel.cancel();
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), handle).await;
}
