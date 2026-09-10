mod common;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message as AxumMessage, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use base64::Engine;
use ps_server::signal::crypto::{derive_keys, open, seal};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;
use webrtc::data_channel::{DataChannel, DataChannelEvent};
use webrtc::peer_connection::{
    PeerConnection, PeerConnectionBuilder, PeerConnectionEventHandler, RTCIceCandidateInit,
    RTCPeerConnectionIceEvent, RTCSessionDescription,
};

const PAIRING_URL_PREFIX: &str = "https://palstudio.app/signal#v1.";
const PAIRING_WINDOW_MS: i64 = 5 * 60 * 1000;
const TEST_BROKER_URL: &str = "ws://127.0.0.1:9";
const BROKER_URL_VAR: &str = "PS_SIGNAL_BROKER_URL";
const DEVICE_ACK_TIMEOUT_VAR: &str = "PS_SIGNAL_DEVICE_ACK_TIMEOUT_MS";
const BASE64: base64::engine::general_purpose::GeneralPurpose =
    base64::engine::general_purpose::STANDARD;

static SIGNAL_ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct SignalEnvGuard {
    _lock: tokio::sync::MutexGuard<'static, ()>,
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl SignalEnvGuard {
    async fn acquire(vars: &[(&'static str, &str)]) -> Self {
        let lock = SIGNAL_ENV_LOCK.lock().await;
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

impl Drop for SignalEnvGuard {
    fn drop(&mut self) {
        for (name, prior) in &self.previous {
            match prior {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }
}

fn write_fixture_with_time(path: &std::path::Path, when: chrono::DateTime<chrono::Local>) {
    const FIXTURE: &str = include_str!("../../ps-app/tests/fixtures/live/world_snapshot.json");
    let mut doc: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    doc["Time"] = serde_json::Value::String(when.format("%Y-%m-%d %H:%M:%S").to_string());
    std::fs::write(path, serde_json::to_string(&doc).unwrap()).unwrap();
}

fn group(code: &str) -> String {
    code.as_bytes()
        .chunks(4)
        .map(|chunk| std::str::from_utf8(chunk).unwrap())
        .collect::<Vec<_>>()
        .join("-")
}

fn ungrouped_code(started: &serde_json::Value) -> String {
    let url = started["data"]["url"]
        .as_str()
        .unwrap_or_else(|| panic!("no pairing url: {started}"));
    let code = url
        .strip_prefix(PAIRING_URL_PREFIX)
        .unwrap_or_else(|| panic!("unexpected pairing url: {url}"));
    assert_eq!(
        code.len(),
        26,
        "the url must carry the ungrouped code: {url}"
    );
    assert!(
        code.chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()),
        "the url code must be bare, ungrouped alphabet characters: {url}"
    );
    code.to_string()
}

#[tokio::test]
async fn signal_start_pairing_answers_with_a_code_and_arms_the_window() {
    let _env = SignalEnvGuard::acquire(&[(BROKER_URL_VAR, TEST_BROKER_URL)]).await;
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;

    let before_ms = chrono::Utc::now().timestamp_millis();
    common::send_json(
        &mut ws,
        serde_json::json!({ "type": "signal_start_pairing" }),
    )
    .await;
    let started = common::next_json(&mut ws).await;
    let after_ms = chrono::Utc::now().timestamp_millis();

    assert_eq!(started["type"], "signal_start_pairing");
    assert!(
        started["data"].get("error").is_none(),
        "unexpected error: {started}"
    );
    let code = ungrouped_code(&started);
    assert_eq!(started["data"]["code"].as_str().unwrap(), group(&code));
    assert_eq!(started["data"]["pairing"], "waiting");

    let expires_at_ms = started["data"]["expiresAtMs"]
        .as_i64()
        .unwrap_or_else(|| panic!("no expiresAtMs: {started}"));
    assert!(
        expires_at_ms >= before_ms + PAIRING_WINDOW_MS
            && expires_at_ms <= after_ms + PAIRING_WINDOW_MS,
        "expiresAtMs {expires_at_ms} is not ~5 minutes after {before_ms}..{after_ms}"
    );

    common::send_json(&mut ws, serde_json::json!({ "type": "signal_status" })).await;
    let status = common::next_json(&mut ws).await;
    assert_eq!(status["data"]["pairing"], "waiting");
    assert_eq!(status["data"]["code"].as_str().unwrap(), group(&code));
    assert_eq!(
        status["data"]["expiresAtMs"].as_i64().unwrap(),
        expires_at_ms
    );
    assert_eq!(status["data"]["source"]["health"], "idle");

    common::send_json(
        &mut ws,
        serde_json::json!({ "type": "signal_stop_pairing" }),
    )
    .await;
    let stopped = common::next_json(&mut ws).await;
    assert_eq!(stopped["type"], "signal_stop_pairing");
    assert_eq!(stopped["data"]["pairing"], "off");
    assert!(stopped["data"].get("code").is_none(), "{stopped}");
    assert!(stopped["data"].get("expiresAtMs").is_none(), "{stopped}");

    common::send_json(&mut ws, serde_json::json!({ "type": "signal_status" })).await;
    let status = common::next_json(&mut ws).await;
    assert_eq!(status["data"]["pairing"], "off");
    assert!(status["data"].get("code").is_none(), "{status}");

    server.handle.shutdown().await;
}

#[tokio::test]
async fn signal_start_pairing_accepts_an_optional_turn_payload() {
    let _env = SignalEnvGuard::acquire(&[(BROKER_URL_VAR, TEST_BROKER_URL)]).await;
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;

    common::send_json(
        &mut ws,
        serde_json::json!({
            "type": "signal_start_pairing",
            "data": {
                "turn": {
                    "urls": ["turn:relay.example:3478"],
                    "username": "user",
                    "credential": "secret"
                }
            }
        }),
    )
    .await;
    let started = common::next_json(&mut ws).await;

    assert_eq!(started["type"], "signal_start_pairing");
    assert!(
        started["data"].get("error").is_none(),
        "unexpected error: {started}"
    );
    assert_eq!(started["data"]["pairing"], "waiting");
    ungrouped_code(&started);

    server.handle.shutdown().await;
}

#[tokio::test]
async fn starting_pairing_again_rotates_the_code() {
    let _env = SignalEnvGuard::acquire(&[(BROKER_URL_VAR, TEST_BROKER_URL)]).await;
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;

    common::send_json(
        &mut ws,
        serde_json::json!({ "type": "signal_start_pairing" }),
    )
    .await;
    let first = common::next_json(&mut ws).await;
    let first_code = ungrouped_code(&first);

    common::send_json(
        &mut ws,
        serde_json::json!({ "type": "signal_start_pairing" }),
    )
    .await;
    let second = common::next_json(&mut ws).await;
    let second_code = ungrouped_code(&second);

    assert_ne!(
        first_code, second_code,
        "starting again while waiting must rotate to a new code"
    );

    common::send_json(&mut ws, serde_json::json!({ "type": "signal_status" })).await;
    let status = common::next_json(&mut ws).await;
    assert_eq!(status["data"]["pairing"], "waiting");
    assert_eq!(
        status["data"]["code"].as_str().unwrap(),
        group(&second_code),
        "only the newest code is live"
    );

    server.handle.shutdown().await;
}

#[tokio::test]
async fn signal_set_source_switches_file_source_on_and_off() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;

    common::send_json(&mut ws, serde_json::json!({ "type": "signal_status" })).await;
    let status = common::next_json(&mut ws).await;
    assert_eq!(status["type"], "signal_status");
    assert_eq!(status["data"]["source"]["health"], "idle");
    assert_eq!(status["data"]["pairing"], "off");

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("GameData.json");
    write_fixture_with_time(&path, chrono::Local::now());

    common::send_json(
        &mut ws,
        serde_json::json!({
            "type": "signal_set_source",
            "data": { "kind": "file", "path": path.to_str().unwrap() }
        }),
    )
    .await;
    let ack = common::next_json(&mut ws).await;
    assert_eq!(ack["type"], "signal_set_source");
    assert!(
        ack["data"].get("error").is_none(),
        "unexpected error: {ack}"
    );

    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        common::send_json(&mut ws, serde_json::json!({ "type": "signal_status" })).await;
        let status = common::next_json(&mut ws).await;
        if status["data"]["source"]["health"] == "ok" {
            assert!(status["data"]["source"]["actorCount"].as_u64().unwrap() > 0);
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("source did not report health \"ok\" within 3s: {status}");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let mut post_off_rx = server.handle.app.live_bus.subscribe();
    post_off_rx.borrow_and_update();

    common::send_json(
        &mut ws,
        serde_json::json!({ "type": "signal_set_source", "data": { "kind": "off" } }),
    )
    .await;
    let off_ack = common::next_json(&mut ws).await;
    assert_eq!(off_ack["type"], "signal_set_source");
    assert_eq!(off_ack["data"]["source"]["health"], "idle");

    common::send_json(&mut ws, serde_json::json!({ "type": "signal_status" })).await;
    let status = common::next_json(&mut ws).await;
    assert_eq!(status["data"]["source"]["health"], "idle");

    tokio::time::timeout(Duration::from_secs(1), post_off_rx.changed())
        .await
        .expect("Off must publish None to the bus")
        .unwrap();
    assert!(post_off_rx.borrow().is_none());

    write_fixture_with_time(&path, chrono::Local::now());
    let changed = tokio::time::timeout(Duration::from_millis(1500), post_off_rx.changed()).await;
    assert!(
        changed.is_err(),
        "the cancelled file source must not publish further bus updates"
    );

    server.handle.shutdown().await;
}

async fn next_frame_seq(
    rx: &mut tokio::sync::watch::Receiver<Option<ps_app::live::LiveFrame>>,
) -> u64 {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        tokio::time::timeout_at(deadline, rx.changed())
            .await
            .expect("no live frame within 5s")
            .expect("the live bus closed");
        if let Some(frame) = rx.borrow_and_update().as_ref() {
            return frame.seq;
        }
    }
}

#[tokio::test]
async fn frame_sequence_keeps_climbing_across_a_source_switch() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("GameData.json");
    write_fixture_with_time(&path, chrono::Local::now());

    let mut bus_rx = server.handle.app.live_bus.subscribe();
    bus_rx.borrow_and_update();

    let set_file = serde_json::json!({
        "type": "signal_set_source",
        "data": { "kind": "file", "path": path.to_str().unwrap() }
    });

    common::send_json(&mut ws, set_file.clone()).await;
    let ack = common::next_json(&mut ws).await;
    assert!(
        ack["data"].get("error").is_none(),
        "unexpected error: {ack}"
    );
    let first = next_frame_seq(&mut bus_rx).await;

    common::send_json(
        &mut ws,
        serde_json::json!({ "type": "signal_set_source", "data": { "kind": "off" } }),
    )
    .await;
    common::next_json(&mut ws).await;

    common::send_json(&mut ws, set_file).await;
    let ack = common::next_json(&mut ws).await;
    assert!(
        ack["data"].get("error").is_none(),
        "unexpected error: {ack}"
    );
    let second = next_frame_seq(&mut bus_rx).await;

    assert!(
        second > first,
        "seq went {first} -> {second} across a source switch"
    );

    server.handle.shutdown().await;
}

#[tokio::test]
async fn signal_set_source_refuses_an_unknown_kind_under_its_own_type() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;

    common::send_json(
        &mut ws,
        serde_json::json!({ "type": "signal_set_source", "data": { "kind": "bogus" } }),
    )
    .await;
    let refusal = common::next_json(&mut ws).await;
    assert_eq!(refusal["type"], "signal_set_source");
    assert!(refusal["data"]["error"].as_str().is_some());

    server.handle.shutdown().await;
}

#[derive(Clone)]
struct MockPairingState {
    from_host_tx: mpsc::Sender<String>,
    to_host_tx: broadcast::Sender<String>,
    cancel: CancellationToken,
    live: Arc<AtomicUsize>,
}

async fn pairing_broker_ws_handler(
    upgrade: WebSocketUpgrade,
    State(state): State<MockPairingState>,
) -> Response {
    upgrade.on_upgrade(move |socket| pairing_broker_connection_loop(socket, state))
}

async fn pairing_broker_connection_loop(mut socket: WebSocket, state: MockPairingState) {
    state.live.fetch_add(1, Ordering::Relaxed);
    let mut to_host_rx = state.to_host_tx.subscribe();
    loop {
        tokio::select! {
            _ = state.cancel.cancelled() => break,
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(AxumMessage::Text(text))) => {
                        let _ = state.from_host_tx.send(text.as_str().to_string()).await;
                    }
                    Some(Ok(AxumMessage::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
            outgoing = to_host_rx.recv() => {
                match outgoing {
                    Ok(text) => {
                        if socket.send(AxumMessage::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
    state.live.fetch_sub(1, Ordering::Relaxed);
}

struct MockPairingBroker {
    addr: SocketAddr,
    cancel: CancellationToken,
    serve_task: tokio::task::JoinHandle<()>,
    to_host_tx: broadcast::Sender<String>,
    live: Arc<AtomicUsize>,
}

impl MockPairingBroker {
    async fn start() -> (Self, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (from_host_tx, from_host_rx) = mpsc::channel::<String>(64);
        let (to_host_tx, _) = broadcast::channel::<String>(64);
        let cancel = CancellationToken::new();
        let live = Arc::new(AtomicUsize::new(0));
        let state = MockPairingState {
            from_host_tx,
            to_host_tx: to_host_tx.clone(),
            cancel: cancel.clone(),
            live: live.clone(),
        };
        let router = Router::new()
            .route("/signal/ws", get(pairing_broker_ws_handler))
            .with_state(state);
        let shutdown_cancel = cancel.clone();
        let serve_task = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async move { shutdown_cancel.cancelled().await })
                .await
                .unwrap();
        });
        (
            Self {
                addr,
                cancel,
                serve_task,
                to_host_tx,
                live,
            },
            from_host_rx,
        )
    }

    fn url(&self) -> String {
        format!("ws://{}", self.addr)
    }

    fn post_raw(&self, text: &str) {
        self.to_host_tx
            .send(text.to_string())
            .expect("the host is attached");
    }

    async fn wait_for_a_connection(&self) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while self.live.load(Ordering::Relaxed) == 0 {
            assert!(
                tokio::time::Instant::now() < deadline,
                "timed out waiting for the desktop to dial the pairing room"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    async fn kill(self) {
        self.cancel.cancel();
        tokio::time::timeout(Duration::from_secs(5), self.serve_task)
            .await
            .expect("mock broker did not shut down within 5s")
            .unwrap();
    }
}

fn guest_blob(seal_key: &[u8; 32], envelope: Value) -> String {
    BASE64.encode(seal(seal_key, envelope.to_string().as_bytes(), b"guest"))
}

fn open_host_blob(seal_key: &[u8; 32], blob: &str) -> Option<Value> {
    let sealed = BASE64.decode(blob).ok()?;
    serde_json::from_slice(&open(seal_key, &sealed, b"host").ok()?).ok()
}

struct PairingGuestHandler {
    seal_key: [u8; 32],
    to_host: broadcast::Sender<String>,
}

#[async_trait::async_trait]
impl PeerConnectionEventHandler for PairingGuestHandler {
    async fn on_ice_candidate(&self, event: RTCPeerConnectionIceEvent) {
        let Ok(init) = event.candidate.to_json() else {
            return;
        };
        let Ok(candidate) = serde_json::to_string(&init) else {
            return;
        };
        let blob = guest_blob(&self.seal_key, json!({ "t": "ice", "c": candidate }));
        let _ = self.to_host.send(blob);
    }
}

async fn build_pairing_guest(
    broker: &MockPairingBroker,
    seal_key: [u8; 32],
) -> Arc<dyn PeerConnection> {
    Arc::new(
        PeerConnectionBuilder::new()
            .with_handler(Arc::new(PairingGuestHandler {
                seal_key,
                to_host: broker.to_host_tx.clone(),
            }))
            .with_udp_addrs(vec!["127.0.0.1:0".to_string()])
            .build()
            .await
            .expect("guest peer builds"),
    )
}

async fn pump_host_signaling(
    mut from_host_rx: mpsc::Receiver<String>,
    seal_key: [u8; 32],
    guest: Arc<dyn PeerConnection>,
) {
    while let Some(blob) = from_host_rx.recv().await {
        let Some(envelope) = open_host_blob(&seal_key, &blob) else {
            continue;
        };
        match envelope["t"].as_str() {
            Some("answer") => {
                let sdp = envelope["sdp"].as_str().unwrap_or_default().to_string();
                let answer = RTCSessionDescription::answer(sdp).expect("valid answer sdp");
                guest
                    .set_remote_description(answer)
                    .await
                    .expect("guest set remote description");
            }
            Some("ice") => {
                let candidate = envelope["c"].as_str().unwrap_or_default();
                if let Ok(init) = serde_json::from_str::<RTCIceCandidateInit>(candidate) {
                    let _ = guest.add_ice_candidate(init).await;
                }
            }
            _ => {}
        }
    }
}

type CtlInbox = mpsc::Receiver<String>;

fn spawn_ctl_inbox(channel: Arc<dyn DataChannel>) -> CtlInbox {
    let (tx, rx) = mpsc::channel(64);
    tokio::spawn(async move {
        while let Some(event) = channel.poll().await {
            match event {
                DataChannelEvent::OnMessage(message) => {
                    let text = String::from_utf8_lossy(&message.data).to_string();
                    if tx.send(text).await.is_err() {
                        break;
                    }
                }
                DataChannelEvent::OnClose => break,
                _ => {}
            }
        }
    });
    rx
}

async fn next_ctl_message(inbox: &mut CtlInbox, seconds: u64) -> Value {
    let text = tokio::time::timeout(Duration::from_secs(seconds), inbox.recv())
        .await
        .expect("a ctl message arrives before the deadline")
        .expect("the ctl channel stays open");
    serde_json::from_str(&text).expect("ctl message is JSON")
}

async fn expect_session_end(inbox: &mut CtlInbox, seconds: u64) {
    let goodbye = next_ctl_message(inbox, seconds).await;
    assert_eq!(goodbye["type"], "session_end", "{goodbye}");
    assert_eq!(goodbye["data"], json!({}), "{goodbye}");
}

async fn wait_for_pairing_connected(ws: &mut common::WsClient) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        common::send_json(ws, serde_json::json!({ "type": "signal_status" })).await;
        let status = common::next_json(ws).await;
        if status["data"]["pairing"] == "connected" {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "pairing never reached connected: {status}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

struct PairingHandshake {
    server: common::TestServer,
    #[allow(dead_code)]
    ws: common::WsClient,
    guest: Arc<dyn PeerConnection>,
    ctl: Arc<dyn DataChannel>,
    ctl_inbox: CtlInbox,
    pump: tokio::task::JoinHandle<()>,
    credential: Value,
}

impl PairingHandshake {
    async fn run(broker: &MockPairingBroker, from_host_rx: mpsc::Receiver<String>) -> Self {
        let server = common::start_test_server().await;
        let mut ws = common::connect(&server).await;

        common::send_json(
            &mut ws,
            serde_json::json!({ "type": "signal_start_pairing" }),
        )
        .await;
        let started = common::next_json(&mut ws).await;
        let code = ungrouped_code(&started);
        let seal_key = derive_keys(&code).seal_key;

        broker.wait_for_a_connection().await;

        let guest = build_pairing_guest(broker, seal_key).await;
        let ctl = guest
            .create_data_channel("ctl", None)
            .await
            .expect("ctl channel");
        let mut ctl_inbox = spawn_ctl_inbox(ctl.clone());

        let offer = guest.create_offer(None).await.expect("offer");
        broker.post_raw(&guest_blob(
            &seal_key,
            json!({ "t": "offer", "sdp": offer.sdp }),
        ));
        guest
            .set_local_description(offer)
            .await
            .expect("guest set local description");

        let pump = tokio::spawn(pump_host_signaling(from_host_rx, seal_key, guest.clone()));

        wait_for_pairing_connected(&mut ws).await;
        let credential = next_ctl_message(&mut ctl_inbox, 15).await;
        assert_eq!(credential["type"], "device_credential");

        Self {
            server,
            ws,
            guest,
            ctl,
            ctl_inbox,
            pump,
            credential,
        }
    }

    async fn shutdown(self) {
        self.pump.abort();
        self.guest.close().await.ok();
        self.server.handle.shutdown().await;
    }
}

async fn wait_for_a_device_row(
    driver: &dyn ps_db::DbDriver,
) -> Vec<ps_db::signal_devices::SignalDevice> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let devices = ps_db::signal_devices::list_devices(driver).await.unwrap();
        if !devices.is_empty() {
            return devices;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the device row never appeared"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pairing_mints_a_device_credential_and_persists_it_once_the_guest_acks() {
    let (broker, from_host_rx) = MockPairingBroker::start().await;
    let _env = SignalEnvGuard::acquire(&[(BROKER_URL_VAR, broker.url().as_str())]).await;

    let handshake = PairingHandshake::run(&broker, from_host_rx).await;
    let credential = handshake.credential.clone();
    let device_id = credential["data"]["deviceId"]
        .as_str()
        .expect("deviceId is a string")
        .to_string();
    let device_secret = credential["data"]["deviceSecret"]
        .as_str()
        .expect("deviceSecret is a string")
        .to_string();
    assert_eq!(device_id.len(), 32, "16 bytes, hex");
    assert_eq!(device_secret.len(), 64, "32 bytes, hex");
    assert!(credential["data"]["meetRoom"].as_str().is_some());
    assert!(!credential["data"]["desktopName"]
        .as_str()
        .unwrap()
        .is_empty());

    handshake
        .ctl
        .send_text(
            &json!({ "type": "device_credential", "data": { "name": "My Phone" } }).to_string(),
        )
        .await
        .expect("ctl accepts the ack");

    let devices = wait_for_a_device_row(&*handshake.server.handle.app.driver).await;
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].device_id, device_id);
    assert_eq!(devices[0].secret_hex, device_secret);
    assert_eq!(devices[0].name, "My Phone");

    handshake.shutdown().await;
    broker.kill().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pairing_persists_no_device_when_the_guest_never_acks() {
    let (broker, from_host_rx) = MockPairingBroker::start().await;
    let _env = SignalEnvGuard::acquire(&[
        (BROKER_URL_VAR, broker.url().as_str()),
        (DEVICE_ACK_TIMEOUT_VAR, "200"),
    ])
    .await;

    let handshake = PairingHandshake::run(&broker, from_host_rx).await;

    tokio::time::sleep(Duration::from_millis(800)).await;
    assert!(
        ps_db::signal_devices::list_devices(&*handshake.server.handle.app.driver)
            .await
            .unwrap()
            .is_empty(),
        "an unacked credential must not be persisted"
    );

    handshake.shutdown().await;
    broker.kill().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stopping_a_connected_pairing_session_tells_the_guest_before_cutting_it() {
    let (broker, from_host_rx) = MockPairingBroker::start().await;
    let _env = SignalEnvGuard::acquire(&[(BROKER_URL_VAR, broker.url().as_str())]).await;

    let mut handshake = PairingHandshake::run(&broker, from_host_rx).await;

    let mut ws = common::connect(&handshake.server).await;
    common::send_json(&mut ws, json!({ "type": "signal_stop_pairing" })).await;
    let stopped = common::next_json(&mut ws).await;
    assert_eq!(stopped["data"]["pairing"], "off", "{stopped}");

    expect_session_end(&mut handshake.ctl_inbox, 15).await;

    handshake.shutdown().await;
    broker.kill().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn revoking_a_just_paired_device_cuts_the_pairing_session_serving_it() {
    let (broker, from_host_rx) = MockPairingBroker::start().await;
    let _env = SignalEnvGuard::acquire(&[(BROKER_URL_VAR, broker.url().as_str())]).await;

    let mut handshake = PairingHandshake::run(&broker, from_host_rx).await;
    handshake
        .ctl
        .send_text(
            &json!({ "type": "device_credential", "data": { "name": "My Phone" } }).to_string(),
        )
        .await
        .expect("ctl accepts the ack");
    let devices = wait_for_a_device_row(&*handshake.server.handle.app.driver).await;
    let device_id = devices[0].device_id.clone();

    let mut ws = common::connect(&handshake.server).await;
    common::send_json(&mut ws, json!({ "type": "signal_status" })).await;
    assert_eq!(
        common::next_json(&mut ws).await["data"]["pairing"],
        "connected",
        "the guest is still being served by the pairing session"
    );

    common::send_json(
        &mut ws,
        json!({ "type": "signal_revoke_device", "data": { "device_id": device_id } }),
    )
    .await;
    let revoked = common::next_json(&mut ws).await;
    assert!(revoked["data"].get("error").is_none(), "{revoked}");
    assert!(
        revoked["data"]["devices"].as_array().unwrap().is_empty(),
        "{revoked}"
    );

    expect_session_end(&mut handshake.ctl_inbox, 15).await;

    common::send_json(&mut ws, json!({ "type": "signal_status" })).await;
    let status = common::next_json(&mut ws).await;
    assert_eq!(status["data"]["pairing"], "off", "{status}");

    handshake.shutdown().await;
    broker.kill().await;
}

const ARMED_KEY: &str = "signal_armed";
const IDENTITY_SECRET_KEY: &str = "signal_identity_secret_hex";
const SEEDED_SECRET_HEX: &str = "5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a";

async fn seed_device(
    server: &common::TestServer,
    device_id: &str,
    name: &str,
    created_at_ms: i64,
    last_seen_ms: Option<i64>,
) {
    ps_db::signal_devices::insert_device(
        &*server.handle.app.driver,
        &ps_db::signal_devices::SignalDevice {
            device_id: device_id.to_string(),
            secret_hex: SEEDED_SECRET_HEX.to_string(),
            name: name.to_string(),
            created_at_ms,
            last_seen_ms,
        },
    )
    .await
    .unwrap();
}

async fn meta(server: &common::TestServer, key: &str) -> Option<String> {
    ps_db::meta::get(&*server.handle.app.driver, key)
        .await
        .unwrap()
}

async fn device_names(server: &common::TestServer) -> Vec<String> {
    ps_db::signal_devices::list_devices(&*server.handle.app.driver)
        .await
        .unwrap()
        .into_iter()
        .map(|device| device.name)
        .collect()
}

#[tokio::test]
async fn signal_set_armed_arms_the_desktop_and_persists_the_choice() {
    let _env = SignalEnvGuard::acquire(&[(BROKER_URL_VAR, TEST_BROKER_URL)]).await;
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;

    common::send_json(&mut ws, json!({ "type": "signal_status" })).await;
    let status = common::next_json(&mut ws).await;
    assert_eq!(status["data"]["armed"], false);
    assert_eq!(meta(&server, ARMED_KEY).await, None);

    common::send_json(
        &mut ws,
        json!({ "type": "signal_set_armed", "data": { "armed": true } }),
    )
    .await;
    let armed = common::next_json(&mut ws).await;
    assert_eq!(armed["type"], "signal_set_armed");
    assert!(armed["data"].get("error").is_none(), "{armed}");
    assert_eq!(armed["data"]["armed"], true);
    assert_eq!(armed["data"]["pairing"], "off", "armed is not connected");
    assert_eq!(armed["data"]["source"]["health"], "idle");

    common::send_json(&mut ws, json!({ "type": "signal_status" })).await;
    assert_eq!(common::next_json(&mut ws).await["data"]["armed"], true);
    assert!(server.handle.services.signal.lock().await.armed());
    assert_eq!(meta(&server, ARMED_KEY).await.as_deref(), Some("1"));

    common::send_json(
        &mut ws,
        json!({ "type": "signal_set_armed", "data": { "armed": false } }),
    )
    .await;
    let disarmed = common::next_json(&mut ws).await;
    assert_eq!(disarmed["data"]["armed"], false);

    common::send_json(&mut ws, json!({ "type": "signal_status" })).await;
    assert_eq!(common::next_json(&mut ws).await["data"]["armed"], false);
    assert!(!server.handle.services.signal.lock().await.armed());
    assert_eq!(meta(&server, ARMED_KEY).await.as_deref(), Some("0"));

    server.handle.shutdown().await;
}

#[tokio::test]
async fn signal_set_armed_refuses_a_payload_without_the_flag() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;

    common::send_json(&mut ws, json!({ "type": "signal_set_armed" })).await;
    let refusal = common::next_json(&mut ws).await;
    assert_eq!(refusal["type"], "signal_set_armed");
    assert!(refusal["data"]["error"].as_str().is_some(), "{refusal}");
    assert!(!server.handle.services.signal.lock().await.armed());

    server.handle.shutdown().await;
}

#[tokio::test]
async fn signal_list_devices_shows_every_paired_device_and_never_a_secret() {
    let server = common::start_test_server().await;
    seed_device(&server, "dev-1", "Phone", 1_700_000_000_000, None).await;
    seed_device(
        &server,
        "dev-2",
        "Tablet",
        1_700_000_001_000,
        Some(1_700_000_100_000),
    )
    .await;
    let mut ws = common::connect(&server).await;

    common::send_json(&mut ws, json!({ "type": "signal_list_devices" })).await;
    let listed = common::next_json(&mut ws).await;

    assert_eq!(listed["type"], "signal_list_devices");
    let devices = listed["data"]["devices"]
        .as_array()
        .unwrap_or_else(|| panic!("no device list: {listed}"));
    assert_eq!(devices.len(), 2);
    assert_eq!(devices[0]["deviceId"], "dev-1");
    assert_eq!(devices[0]["name"], "Phone");
    assert_eq!(devices[0]["createdAtMs"], 1_700_000_000_000i64);
    assert_eq!(devices[0]["connected"], false);
    assert!(
        devices[0].get("lastSeenMs").is_none(),
        "a device that never connected has no last-seen time: {listed}"
    );
    assert_eq!(devices[1]["deviceId"], "dev-2");
    assert_eq!(devices[1]["lastSeenMs"], 1_700_000_100_000i64);
    assert!(
        !listed.to_string().contains(SEEDED_SECRET_HEX),
        "a device secret reached the wire: {listed}"
    );

    server.handle.shutdown().await;
}

#[tokio::test]
async fn signal_rename_device_renames_one_device_and_refuses_an_unknown_id() {
    let server = common::start_test_server().await;
    seed_device(&server, "dev-1", "Phone", 1_700_000_000_000, None).await;
    seed_device(&server, "dev-2", "Tablet", 1_700_000_001_000, None).await;
    let mut ws = common::connect(&server).await;

    common::send_json(
        &mut ws,
        json!({
            "type": "signal_rename_device",
            "data": { "device_id": "dev-1", "name": "  Kitchen phone  " }
        }),
    )
    .await;
    let renamed = common::next_json(&mut ws).await;
    assert_eq!(renamed["type"], "signal_rename_device");
    assert!(renamed["data"].get("error").is_none(), "{renamed}");
    let devices = renamed["data"]["devices"].as_array().unwrap();
    assert_eq!(devices[0]["name"], "Kitchen phone", "the name is trimmed");
    assert_eq!(devices[1]["name"], "Tablet");
    assert_eq!(
        device_names(&server).await,
        vec!["Kitchen phone".to_string(), "Tablet".to_string()]
    );

    common::send_json(
        &mut ws,
        json!({
            "type": "signal_rename_device",
            "data": { "device_id": "gone", "name": "Nobody" }
        }),
    )
    .await;
    let refusal = common::next_json(&mut ws).await;
    assert_eq!(refusal["type"], "signal_rename_device");
    assert!(refusal["data"]["error"].as_str().is_some(), "{refusal}");
    assert!(refusal["data"].get("devices").is_none(), "{refusal}");

    common::send_json(
        &mut ws,
        json!({
            "type": "signal_rename_device",
            "data": { "device_id": "dev-1", "name": "   " }
        }),
    )
    .await;
    let empty = common::next_json(&mut ws).await;
    assert!(empty["data"]["error"].as_str().is_some(), "{empty}");
    assert_eq!(
        device_names(&server).await,
        vec!["Kitchen phone".to_string(), "Tablet".to_string()],
        "an empty rename leaves the device named as it was"
    );

    server.handle.shutdown().await;
}

#[tokio::test]
async fn signal_revoke_device_forgets_the_device_and_refuses_an_unknown_id() {
    let server = common::start_test_server().await;
    seed_device(&server, "dev-1", "Phone", 1_700_000_000_000, None).await;
    seed_device(&server, "dev-2", "Tablet", 1_700_000_001_000, None).await;
    let mut ws = common::connect(&server).await;

    common::send_json(
        &mut ws,
        json!({ "type": "signal_revoke_device", "data": { "device_id": "dev-1" } }),
    )
    .await;
    let revoked = common::next_json(&mut ws).await;
    assert_eq!(revoked["type"], "signal_revoke_device");
    assert!(revoked["data"].get("error").is_none(), "{revoked}");
    let devices = revoked["data"]["devices"].as_array().unwrap();
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0]["deviceId"], "dev-2");
    assert_eq!(device_names(&server).await, vec!["Tablet".to_string()]);

    common::send_json(
        &mut ws,
        json!({ "type": "signal_revoke_device", "data": { "device_id": "dev-1" } }),
    )
    .await;
    let refusal = common::next_json(&mut ws).await;
    assert_eq!(refusal["type"], "signal_revoke_device");
    assert!(refusal["data"]["error"].as_str().is_some(), "{refusal}");

    server.handle.shutdown().await;
}

#[tokio::test]
async fn signal_reset_remote_access_rotates_the_identity_and_forgets_every_device() {
    let _env = SignalEnvGuard::acquire(&[(BROKER_URL_VAR, TEST_BROKER_URL)]).await;
    let server = common::start_test_server().await;
    seed_device(&server, "dev-1", "Phone", 1_700_000_000_000, None).await;
    seed_device(&server, "dev-2", "Tablet", 1_700_000_001_000, None).await;
    let mut ws = common::connect(&server).await;

    common::send_json(
        &mut ws,
        json!({ "type": "signal_set_armed", "data": { "armed": true } }),
    )
    .await;
    common::next_json(&mut ws).await;
    let identity = meta(&server, IDENTITY_SECRET_KEY)
        .await
        .expect("arming mints an identity secret");

    common::send_json(&mut ws, json!({ "type": "signal_reset_remote_access" })).await;
    let reset = common::next_json(&mut ws).await;
    assert_eq!(reset["type"], "signal_reset_remote_access");
    assert!(reset["data"].get("error").is_none(), "{reset}");
    assert_eq!(
        reset["data"]["armed"], true,
        "a reset does not disarm a desktop that was armed"
    );
    assert_eq!(reset["data"]["pairing"], "off");

    assert!(device_names(&server).await.is_empty());
    let rotated = meta(&server, IDENTITY_SECRET_KEY)
        .await
        .expect("the identity is replaced, not deleted");
    assert_ne!(rotated, identity, "the meet room must move");
    assert!(server.handle.services.signal.lock().await.armed());

    common::send_json(&mut ws, json!({ "type": "signal_list_devices" })).await;
    let listed = common::next_json(&mut ws).await;
    assert!(listed["data"]["devices"].as_array().unwrap().is_empty());

    server.handle.shutdown().await;
}
