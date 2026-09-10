mod common;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::ws::{CloseFrame, Message as AxumMessage, WebSocket, WebSocketUpgrade};
use axum::extract::{RawQuery, State};
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use base64::Engine;
use ps_server::signal::crypto::{
    derive_device_seal_key, derive_meet_room, open, parse_hex32, seal,
};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;
use webrtc::data_channel::{DataChannel, DataChannelEvent, RTCDataChannelState};
use webrtc::peer_connection::{
    PeerConnection, PeerConnectionBuilder, PeerConnectionEventHandler, RTCIceCandidateInit,
    RTCPeerConnectionIceEvent, RTCSessionDescription,
};

const BASE64: base64::engine::general_purpose::GeneralPurpose =
    base64::engine::general_purpose::STANDARD;
const BROKER_URL_VAR: &str = "PS_SIGNAL_BROKER_URL";
const RELINK_BACKOFF_VAR: &str = "PS_SIGNAL_MEET_RELINK_BACKOFF_MS";
const DISCARD_BROKER_URL: &str = "ws://127.0.0.1:9";
const ROOM_EXPIRED_CLOSE: u16 = 4001;
const IDENTITY_SECRET_KEY: &str = "signal_identity_secret_hex";
const ARMED_KEY: &str = "signal_armed";
const DEVICE: &str = "device-with-a-row";
const OTHER_DEVICE: &str = "the-other-paired-device";
const DEVICE_SECRET: [u8; 32] = [0x5a; 32];
const OTHER_SECRET: [u8; 32] = [0x1c; 32];
const REDIAL_BURST: usize = 5000;

static BROKER_ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct BrokerEnvGuard {
    _lock: tokio::sync::MutexGuard<'static, ()>,
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl BrokerEnvGuard {
    async fn acquire(url: &str) -> Self {
        Self::acquire_vars(&[(BROKER_URL_VAR, url)]).await
    }

    async fn acquire_vars(vars: &[(&'static str, &str)]) -> Self {
        let lock = BROKER_ENV_LOCK.lock().await;
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

impl Drop for BrokerEnvGuard {
    fn drop(&mut self) {
        for (name, prior) in &self.previous {
            match prior {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }
}

#[derive(Clone)]
struct MockState {
    from_host_tx: mpsc::Sender<String>,
    to_host_tx: broadcast::Sender<String>,
    cancel: CancellationToken,
    upgrades: Arc<Mutex<Vec<String>>>,
    connections: Arc<AtomicUsize>,
    live: Arc<AtomicUsize>,
    expire_next: Arc<AtomicBool>,
}

async fn ws_handler(
    upgrade: WebSocketUpgrade,
    RawQuery(query): RawQuery,
    State(state): State<MockState>,
) -> Response {
    state
        .upgrades
        .lock()
        .expect("upgrades mutex is never poisoned")
        .push(query.unwrap_or_default());
    upgrade.on_upgrade(move |socket| connection_loop(socket, state))
}

async fn connection_loop(mut socket: WebSocket, state: MockState) {
    state.connections.fetch_add(1, Ordering::Relaxed);
    state.live.fetch_add(1, Ordering::Relaxed);
    if state.expire_next.swap(false, Ordering::Relaxed) {
        let _ = socket
            .send(AxumMessage::Close(Some(CloseFrame {
                code: ROOM_EXPIRED_CLOSE,
                reason: "room expired".into(),
            })))
            .await;
        state.live.fetch_sub(1, Ordering::Relaxed);
        return;
    }
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

struct MockBroker {
    addr: SocketAddr,
    cancel: CancellationToken,
    serve_task: tokio::task::JoinHandle<()>,
    to_host_tx: broadcast::Sender<String>,
    upgrades: Arc<Mutex<Vec<String>>>,
    connections: Arc<AtomicUsize>,
    live: Arc<AtomicUsize>,
    expire_next: Arc<AtomicBool>,
}

impl MockBroker {
    async fn start() -> (Self, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (from_host_tx, from_host_rx) = mpsc::channel::<String>(64);
        let (to_host_tx, _) = broadcast::channel::<String>(64);
        let cancel = CancellationToken::new();
        let upgrades = Arc::new(Mutex::new(Vec::new()));
        let connections = Arc::new(AtomicUsize::new(0));
        let live = Arc::new(AtomicUsize::new(0));
        let expire_next = Arc::new(AtomicBool::new(false));
        let state = MockState {
            from_host_tx,
            to_host_tx: to_host_tx.clone(),
            cancel: cancel.clone(),
            upgrades: upgrades.clone(),
            connections: connections.clone(),
            live: live.clone(),
            expire_next: expire_next.clone(),
        };
        let router = Router::new()
            .route("/signal/ws", get(ws_handler))
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
                upgrades,
                connections,
                live,
                expire_next,
            },
            from_host_rx,
        )
    }

    fn expire_next_connection(&self) {
        self.expire_next.store(true, Ordering::Relaxed);
    }

    fn url(&self) -> String {
        format!("ws://{}", self.addr)
    }

    fn upgrades(&self) -> Vec<String> {
        self.upgrades
            .lock()
            .expect("upgrades mutex is never poisoned")
            .clone()
    }

    fn post(&self, device: &str, blob: String) {
        let wrapper = json!({ "v": 1, "device": device, "blob": blob }).to_string();
        self.to_host_tx.send(wrapper).expect("the host is attached");
    }

    fn post_raw(&self, text: &str) {
        self.to_host_tx
            .send(text.to_string())
            .expect("the host is attached");
    }

    async fn wait_for_a_connection(&self) {
        wait_until(
            Duration::from_secs(5),
            "the desktop dials the meet room",
            || self.live.load(Ordering::Relaxed) > 0,
        )
        .await;
    }

    async fn kill(self) {
        self.cancel.cancel();
        tokio::time::timeout(Duration::from_secs(5), self.serve_task)
            .await
            .expect("mock broker did not shut down within 5s")
            .unwrap();
    }
}

async fn wait_until(within: Duration, what: &str, mut done: impl FnMut() -> bool) {
    let deadline = tokio::time::Instant::now() + within;
    while !done() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for {what}"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

fn guest_blob(seal_key: &[u8; 32], envelope: Value) -> String {
    BASE64.encode(seal(seal_key, envelope.to_string().as_bytes(), b"guest"))
}

fn open_host_blob(seal_key: &[u8; 32], blob: &str) -> Option<Value> {
    let sealed = BASE64.decode(blob).ok()?;
    serde_json::from_slice(&open(seal_key, &sealed, b"host").ok()?).ok()
}

struct GuestHandler {
    seal_key: [u8; 32],
    device: String,
    to_host: broadcast::Sender<String>,
}

#[async_trait::async_trait]
impl PeerConnectionEventHandler for GuestHandler {
    async fn on_ice_candidate(&self, event: RTCPeerConnectionIceEvent) {
        let Ok(init) = event.candidate.to_json() else {
            return;
        };
        let Ok(candidate) = serde_json::to_string(&init) else {
            return;
        };
        let blob = guest_blob(&self.seal_key, json!({ "t": "ice", "c": candidate }));
        let wrapper = json!({ "v": 1, "device": self.device, "blob": blob }).to_string();
        let _ = self.to_host.send(wrapper);
    }
}

async fn build_guest(
    broker: &MockBroker,
    device: &str,
    seal_key: [u8; 32],
) -> Arc<dyn PeerConnection> {
    Arc::new(
        PeerConnectionBuilder::new()
            .with_handler(Arc::new(GuestHandler {
                seal_key,
                device: device.to_string(),
                to_host: broker.to_host_tx.clone(),
            }))
            .with_udp_addrs(vec!["127.0.0.1:0".to_string()])
            .build()
            .await
            .expect("guest peer builds"),
    )
}

struct ConnectedGuest {
    guest: Arc<dyn PeerConnection>,
    pump: tokio::task::JoinHandle<()>,
    seen_rx: mpsc::UnboundedReceiver<String>,
    ctl_rx: mpsc::Receiver<String>,
    ctl: Arc<dyn DataChannel>,
    open_blob: String,
}

fn spawn_ctl_inbox(channel: Arc<dyn DataChannel>) -> mpsc::Receiver<String> {
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

async fn next_ctl_message(inbox: &mut mpsc::Receiver<String>, seconds: u64) -> Value {
    let text = tokio::time::timeout(Duration::from_secs(seconds), inbox.recv())
        .await
        .expect("a ctl message arrives before the deadline")
        .expect("the ctl channel stays open");
    serde_json::from_str(&text).expect("ctl message is JSON")
}

async fn expect_session_end(inbox: &mut mpsc::Receiver<String>, seconds: u64) {
    let goodbye = next_ctl_message(inbox, seconds).await;
    assert_eq!(goodbye["type"], "session_end", "{goodbye}");
    assert_eq!(goodbye["data"], json!({}), "{goodbye}");
}

async fn connect_a_device(
    broker: &MockBroker,
    from_host_rx: mpsc::Receiver<String>,
    server: &common::TestServer,
    device: &str,
    secret: &[u8; 32],
) -> ConnectedGuest {
    let seal_key = derive_device_seal_key(secret);
    let guest = build_guest(broker, device, seal_key).await;
    let ctl = guest
        .create_data_channel("ctl", None)
        .await
        .expect("ctl channel");
    let mut ctl_rx = spawn_ctl_inbox(ctl.clone());
    let offer = guest.create_offer(None).await.expect("offer");
    let open_blob = guest_blob(&seal_key, json!({ "t": "offer", "sdp": offer.sdp }));
    broker.post(device, open_blob.clone());
    guest
        .set_local_description(offer)
        .await
        .expect("guest set local description");

    let (seen_tx, seen_rx) = mpsc::unbounded_channel::<String>();
    let mut from_host_rx = from_host_rx;
    let answering = guest.clone();
    let pump = tokio::spawn(async move {
        while let Some(blob) = from_host_rx.recv().await {
            seen_tx.send(blob.clone()).ok();
            let Some(envelope) = open_host_blob(&seal_key, &blob) else {
                continue;
            };
            match envelope["t"].as_str() {
                Some("answer") => {
                    let sdp = envelope["sdp"].as_str().unwrap_or_default().to_string();
                    let answer = RTCSessionDescription::answer(sdp).expect("valid answer sdp");
                    answering
                        .set_remote_description(answer)
                        .await
                        .expect("guest set remote description");
                }
                Some("ice") => {
                    let candidate = envelope["c"].as_str().unwrap_or_default();
                    if let Ok(init) = serde_json::from_str::<RTCIceCandidateInit>(candidate) {
                        let _ = answering.add_ice_candidate(init).await;
                    }
                }
                _ => {}
            }
        }
    });

    wait_for_pairing(server, "connected").await;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    while !matches!(ctl.ready_state().await, Ok(RTCDataChannelState::Open)) {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the guest's ctl channel never opened"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    ctl.send_text(&json!({ "type": "ping" }).to_string())
        .await
        .expect("ctl accepts a ping");
    assert_eq!(next_ctl_message(&mut ctl_rx, 15).await["type"], "pong");

    ConnectedGuest {
        guest,
        pump,
        seen_rx,
        ctl_rx,
        ctl,
        open_blob,
    }
}

async fn seed_device(server: &common::TestServer, device_id: &str, secret: &[u8; 32]) {
    ps_db::signal_devices::insert_device(
        &*server.handle.app.driver,
        &ps_db::signal_devices::SignalDevice {
            device_id: device_id.to_string(),
            secret_hex: ps_server::signal::crypto::hex32(secret),
            name: "Test phone".to_string(),
            created_at_ms: 1_700_000_000_000,
            last_seen_ms: None,
        },
    )
    .await
    .unwrap();
}

async fn set_armed(server: &common::TestServer, armed: bool) {
    server
        .handle
        .services
        .signal
        .lock()
        .await
        .set_armed(armed, &server.handle.app)
        .await
        .expect("arming is accepted");
}

async fn pairing_label(server: &common::TestServer) -> String {
    server
        .handle
        .services
        .signal
        .lock()
        .await
        .pairing()
        .label()
        .to_string()
}

async fn wait_for_pairing(server: &common::TestServer, label: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    while pairing_label(server).await != label {
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for pairing {label:?}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_armed_desktop_serves_a_paired_device_and_ignores_the_rest() {
    let (broker, from_host_rx) = MockBroker::start().await;
    let _env = BrokerEnvGuard::acquire(&broker.url()).await;
    let server = common::start_test_server().await;
    seed_device(&server, DEVICE, &DEVICE_SECRET).await;
    seed_device(&server, OTHER_DEVICE, &OTHER_SECRET).await;

    set_armed(&server, true).await;
    broker.wait_for_a_connection().await;

    let identity = ps_db::meta::get(&*server.handle.app.driver, IDENTITY_SECRET_KEY)
        .await
        .unwrap()
        .and_then(|hex| parse_hex32(&hex))
        .expect("arming mints an identity secret");
    let upgrades = broker.upgrades();
    assert_eq!(upgrades.len(), 1, "one standing link: {upgrades:?}");
    assert_eq!(
        upgrades[0],
        format!("room={}&role=host&kind=meet", derive_meet_room(&identity))
    );
    assert_eq!(
        ps_db::meta::get(&*server.handle.app.driver, ARMED_KEY)
            .await
            .unwrap()
            .as_deref(),
        Some("1")
    );
    assert_eq!(
        pairing_label(&server).await,
        "off",
        "armed is not connected"
    );

    let ConnectedGuest {
        guest,
        pump,
        mut seen_rx,
        mut ctl_rx,
        ..
    } = connect_a_device(&broker, from_host_rx, &server, DEVICE, &DEVICE_SECRET).await;

    let other_key = derive_device_seal_key(&OTHER_SECRET);
    let intruder = build_guest(&broker, OTHER_DEVICE, other_key).await;
    intruder
        .create_data_channel("ctl", None)
        .await
        .expect("ctl channel");
    let other_offer = intruder.create_offer(None).await.expect("offer");
    broker.post(
        OTHER_DEVICE,
        guest_blob(&other_key, json!({ "t": "offer", "sdp": other_offer.sdp })),
    );
    tokio::time::sleep(Duration::from_secs(1)).await;
    let mut seen = Vec::new();
    while let Ok(blob) = seen_rx.try_recv() {
        seen.push(blob);
    }
    assert!(
        !seen.is_empty(),
        "the desktop answered the first device, so it posted something"
    );
    assert!(
        seen.iter()
            .all(|blob| open_host_blob(&other_key, blob).is_none()),
        "the desktop answered a second device while one was already connected"
    );
    assert_eq!(pairing_label(&server).await, "connected");

    set_armed(&server, false).await;
    expect_session_end(&mut ctl_rx, 15).await;
    wait_until(Duration::from_secs(5), "the meet link to close", || {
        broker.live.load(Ordering::Relaxed) == 0
    })
    .await;
    let dialed = broker.connections.load(Ordering::Relaxed);
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert_eq!(
        broker.connections.load(Ordering::Relaxed),
        dialed,
        "a disarmed desktop must not dial the meet room again"
    );
    assert_eq!(pairing_label(&server).await, "off");
    assert_eq!(
        ps_db::meta::get(&*server.handle.app.driver, ARMED_KEY)
            .await
            .unwrap()
            .as_deref(),
        Some("0")
    );

    guest.close().await.ok();
    intruder.close().await.ok();
    pump.abort();
    server.handle.shutdown().await;
    broker.kill().await;
}

fn dtls_fingerprint(sdp: &str) -> String {
    sdp.lines()
        .find_map(|line| line.trim().strip_prefix("a=fingerprint:"))
        .expect("an answer always carries a DTLS fingerprint")
        .to_string()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_redial_from_the_connected_device_gets_a_new_session() {
    let (broker, mut from_host_rx) = MockBroker::start().await;
    let _env = BrokerEnvGuard::acquire(&broker.url()).await;
    let server = common::start_test_server().await;
    seed_device(&server, DEVICE, &DEVICE_SECRET).await;

    set_armed(&server, true).await;
    broker.wait_for_a_connection().await;

    let seal_key = derive_device_seal_key(&DEVICE_SECRET);
    let first = build_guest(&broker, DEVICE, seal_key).await;
    first
        .create_data_channel("ctl", None)
        .await
        .expect("ctl channel");

    let current: Arc<Mutex<Arc<dyn PeerConnection>>> = Arc::new(Mutex::new(first.clone()));
    let (seen_tx, mut seen_rx) = mpsc::unbounded_channel::<String>();
    let pumping = current.clone();
    let pump = tokio::spawn(async move {
        while let Some(blob) = from_host_rx.recv().await {
            seen_tx.send(blob.clone()).ok();
            let Some(envelope) = open_host_blob(&seal_key, &blob) else {
                continue;
            };
            let guest = pumping
                .lock()
                .expect("the current-guest mutex is never poisoned")
                .clone();
            match envelope["t"].as_str() {
                Some("answer") => {
                    let sdp = envelope["sdp"].as_str().unwrap_or_default().to_string();
                    let answer = RTCSessionDescription::answer(sdp).expect("valid answer sdp");
                    let _ = guest.set_remote_description(answer).await;
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
    });

    let offer = first.create_offer(None).await.expect("offer");
    broker.post(
        DEVICE,
        guest_blob(&seal_key, json!({ "t": "offer", "sdp": offer.sdp })),
    );
    first
        .set_local_description(offer)
        .await
        .expect("guest set local description");
    wait_for_pairing(&server, "connected").await;

    let first_fingerprint = next_answer_fingerprint(&mut seen_rx, &seal_key).await;

    let reloaded = build_guest(&broker, DEVICE, seal_key).await;
    let ctl = reloaded
        .create_data_channel("ctl", None)
        .await
        .expect("ctl channel");
    *current
        .lock()
        .expect("the current-guest mutex is never poisoned") = reloaded.clone();
    let redial = reloaded.create_offer(None).await.expect("offer");
    broker.post(
        DEVICE,
        guest_blob(&seal_key, json!({ "t": "offer", "sdp": redial.sdp })),
    );
    reloaded
        .set_local_description(redial)
        .await
        .expect("guest set local description");

    let second_fingerprint = next_answer_fingerprint(&mut seen_rx, &seal_key).await;
    assert_ne!(
        first_fingerprint, second_fingerprint,
        "the re-dial was answered by the peer the old tab was already holding"
    );

    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    while !matches!(ctl.ready_state().await, Ok(RTCDataChannelState::Open)) {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the re-dialed tab never got a working session"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    wait_for_pairing(&server, "connected").await;

    set_armed(&server, false).await;
    first.close().await.ok();
    reloaded.close().await.ok();
    pump.abort();
    server.handle.shutdown().await;
    broker.kill().await;
}

fn vary_sdp(template: &str, unique: u64) -> String {
    let marker = template
        .lines()
        .find_map(|line| line.strip_prefix("o=- "))
        .and_then(|rest| rest.split_whitespace().next())
        .expect("a webrtc offer always carries an origin session id");
    template.replacen(marker, &unique.to_string(), 1)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_burst_of_rapid_redials_does_not_deadlock_the_listener() {
    let (broker, from_host_rx) = MockBroker::start().await;
    let _env = BrokerEnvGuard::acquire(&broker.url()).await;
    let server = common::start_test_server().await;
    seed_device(&server, DEVICE, &DEVICE_SECRET).await;

    set_armed(&server, true).await;
    broker.wait_for_a_connection().await;

    let seal_key = derive_device_seal_key(&DEVICE_SECRET);
    let template_guest = build_guest(&broker, DEVICE, seal_key).await;
    let template_sdp = template_guest.create_offer(None).await.expect("offer").sdp;
    template_guest.close().await.ok();
    let blobs: Vec<String> = (0..REDIAL_BURST)
        .map(|i| {
            let sdp = vary_sdp(&template_sdp, i as u64 + 1);
            guest_blob(&seal_key, json!({ "t": "offer", "sdp": sdp }))
        })
        .collect();
    for blob in blobs {
        broker.post(DEVICE, blob);
    }

    tokio::time::timeout(Duration::from_secs(30), async {
        let connected =
            connect_a_device(&broker, from_host_rx, &server, DEVICE, &DEVICE_SECRET).await;
        connected.guest.close().await.ok();
        connected.pump.abort();
    })
    .await
    .expect("the listener answered the re-dial after the burst instead of deadlocking on ended_tx");

    set_armed(&server, false).await;
    server.handle.shutdown().await;
    broker.kill().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn replaying_the_open_blob_does_not_disturb_the_live_session() {
    let (broker, from_host_rx) = MockBroker::start().await;
    let _env = BrokerEnvGuard::acquire(&broker.url()).await;
    let server = common::start_test_server().await;
    seed_device(&server, DEVICE, &DEVICE_SECRET).await;

    set_armed(&server, true).await;
    broker.wait_for_a_connection().await;
    let mut connected =
        connect_a_device(&broker, from_host_rx, &server, DEVICE, &DEVICE_SECRET).await;

    broker.post(DEVICE, connected.open_blob.clone());

    let untouched = tokio::time::timeout(Duration::from_secs(3), connected.ctl_rx.recv()).await;
    assert!(
        untouched.is_err(),
        "a replayed open blob must not end the live session: {:?}",
        untouched.ok().flatten()
    );
    connected
        .ctl
        .send_text(&json!({ "type": "ping" }).to_string())
        .await
        .expect("the live session's ctl still accepts a ping");
    assert_eq!(
        next_ctl_message(&mut connected.ctl_rx, 15).await["type"],
        "pong"
    );

    set_armed(&server, false).await;
    connected.guest.close().await.ok();
    connected.pump.abort();
    server.handle.shutdown().await;
    broker.kill().await;
}

async fn next_answer_fingerprint(
    seen_rx: &mut mpsc::UnboundedReceiver<String>,
    seal_key: &[u8; 32],
) -> String {
    tokio::time::timeout(Duration::from_secs(30), async {
        while let Some(blob) = seen_rx.recv().await {
            let Some(envelope) = open_host_blob(seal_key, &blob) else {
                continue;
            };
            if envelope["t"] == "answer" {
                return dtls_fingerprint(envelope["sdp"].as_str().unwrap_or_default());
            }
        }
        panic!("the desktop stopped posting before it answered");
    })
    .await
    .expect("the desktop answers within the deadline")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn noise_in_the_meet_room_is_dropped_without_a_reply() {
    let (broker, mut from_host_rx) = MockBroker::start().await;
    let _env = BrokerEnvGuard::acquire(&broker.url()).await;
    let server = common::start_test_server().await;
    seed_device(&server, DEVICE, &DEVICE_SECRET).await;
    seed_device(&server, OTHER_DEVICE, &OTHER_SECRET).await;

    set_armed(&server, true).await;
    broker.wait_for_a_connection().await;

    let seal_key = derive_device_seal_key(&DEVICE_SECRET);
    let stranger_key = derive_device_seal_key(&[0x77u8; 32]);
    let offer_envelope = json!({ "t": "offer", "sdp": "v=0\r\n" });

    broker.post_raw("hello?");
    broker.post_raw(&guest_blob(&seal_key, offer_envelope.clone()));
    broker.post_raw(
        &json!({
            "v": 2,
            "device": DEVICE,
            "blob": guest_blob(&seal_key, offer_envelope.clone())
        })
        .to_string(),
    );
    broker.post(
        "never-paired",
        guest_blob(&stranger_key, offer_envelope.clone()),
    );
    broker.post(DEVICE, guest_blob(&stranger_key, offer_envelope.clone()));
    broker.post(DEVICE, BASE64.encode("not an envelope at all"));
    broker.post(DEVICE, "?not even base64?".to_string());
    broker.post(
        DEVICE,
        guest_blob(&seal_key, json!({ "t": "ice", "c": "{}" })),
    );

    let quiet = tokio::time::timeout(Duration::from_secs(2), from_host_rx.recv()).await;
    assert!(
        quiet.is_err(),
        "the desktop replied to noise: {:?}",
        quiet.ok().flatten()
    );
    assert_eq!(pairing_label(&server).await, "off");

    let other_key = derive_device_seal_key(&OTHER_SECRET);
    let guest = build_guest(&broker, OTHER_DEVICE, other_key).await;
    guest
        .create_data_channel("ctl", None)
        .await
        .expect("ctl channel");
    let offer = guest.create_offer(None).await.expect("offer");
    broker.post(
        OTHER_DEVICE,
        guest_blob(&other_key, json!({ "t": "offer", "sdp": offer.sdp })),
    );
    guest
        .set_local_description(offer)
        .await
        .expect("guest set local description");

    let answered = tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(blob) = from_host_rx.recv().await {
            if let Some(envelope) = open_host_blob(&other_key, &blob) {
                if envelope["t"] == "answer" {
                    return true;
                }
            }
        }
        false
    })
    .await;
    assert_eq!(
        answered.ok(),
        Some(true),
        "a real offer after the noise was never answered"
    );

    set_armed(&server, false).await;
    guest.close().await.ok();
    server.handle.shutdown().await;
    broker.kill().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn revoking_the_connected_device_ends_its_session() {
    let (broker, from_host_rx) = MockBroker::start().await;
    let _env = BrokerEnvGuard::acquire(&broker.url()).await;
    let server = common::start_test_server().await;
    seed_device(&server, DEVICE, &DEVICE_SECRET).await;

    set_armed(&server, true).await;
    broker.wait_for_a_connection().await;
    let mut connected =
        connect_a_device(&broker, from_host_rx, &server, DEVICE, &DEVICE_SECRET).await;

    let mut ws = common::connect(&server).await;
    common::send_json(&mut ws, json!({ "type": "signal_stop_pairing" })).await;
    let stopped = common::next_json(&mut ws).await;
    assert_eq!(
        stopped["data"]["pairing"], "connected",
        "a live device session is not the pairing controls' state to clear: {stopped}"
    );
    assert_eq!(stopped["data"]["connectedDevice"]["deviceId"], DEVICE);
    assert_eq!(stopped["data"]["armed"], true);

    common::send_json(
        &mut ws,
        json!({ "type": "signal_revoke_device", "data": { "device_id": DEVICE } }),
    )
    .await;
    let revoked = common::next_json(&mut ws).await;
    assert!(revoked["data"].get("error").is_none(), "{revoked}");
    assert!(revoked["data"]["devices"].as_array().unwrap().is_empty());

    expect_session_end(&mut connected.ctl_rx, 15).await;

    wait_for_pairing(&server, "off").await;
    common::send_json(&mut ws, json!({ "type": "signal_status" })).await;
    let status = common::next_json(&mut ws).await;
    assert!(
        status["data"].get("connectedDevice").is_none(),
        "a revoked device is no longer connected: {status}"
    );
    assert_eq!(
        status["data"]["armed"], true,
        "revoking one device does not disarm the desktop"
    );
    assert!(
        broker.live.load(Ordering::Relaxed) > 0,
        "the standing meet link outlives the session it was serving"
    );

    set_armed(&server, false).await;
    connected.guest.close().await.ok();
    connected.pump.abort();
    server.handle.shutdown().await;
    broker.kill().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_closing_desktop_leaves_the_device_free_to_ladder() {
    let (broker, from_host_rx) = MockBroker::start().await;
    let _env = BrokerEnvGuard::acquire(&broker.url()).await;
    let server = common::start_test_server().await;
    seed_device(&server, DEVICE, &DEVICE_SECRET).await;

    set_armed(&server, true).await;
    broker.wait_for_a_connection().await;
    let mut connected =
        connect_a_device(&broker, from_host_rx, &server, DEVICE, &DEVICE_SECRET).await;

    server.handle.shutdown().await;

    let heard = tokio::time::timeout(Duration::from_secs(2), connected.ctl_rx.recv()).await;
    if let Ok(Some(text)) = &heard {
        assert!(
            !text.contains("session_end"),
            "a closing desktop must not end the device's session for it: {text}"
        );
    }

    connected.guest.close().await.ok();
    connected.pump.abort();
    broker.kill().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_meet_link_that_ends_while_armed_is_dialed_again() {
    let (broker, _from_host_rx) = MockBroker::start().await;
    let _env = BrokerEnvGuard::acquire_vars(&[
        (BROKER_URL_VAR, broker.url().as_str()),
        (RELINK_BACKOFF_VAR, "250"),
    ])
    .await;
    let server = common::start_test_server().await;

    broker.expire_next_connection();
    set_armed(&server, true).await;

    wait_until(
        Duration::from_secs(15),
        "the desktop to dial the meet room again after its link ended",
        || broker.connections.load(Ordering::Relaxed) >= 2,
    )
    .await;
    let upgrades = broker.upgrades();
    assert_eq!(
        upgrades[0], upgrades[1],
        "the same room, dialed again: {upgrades:?}"
    );
    assert!(
        server.handle.services.signal.lock().await.armed(),
        "a desktop whose link died is still armed"
    );

    wait_until(Duration::from_secs(5), "the new link to attach", || {
        broker.live.load(Ordering::Relaxed) > 0
    })
    .await;

    set_armed(&server, false).await;
    server.handle.shutdown().await;
    broker.kill().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_desktop_left_armed_arms_itself_at_startup() {
    let (broker, _from_host_rx) = MockBroker::start().await;
    let _env = BrokerEnvGuard::acquire(&broker.url()).await;

    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("ps-rs.db");
    let identity = [0x2fu8; 32];
    {
        let driver = ps_db::SqlxSqliteDriver::new(ps_db::open(&db_path).await.unwrap());
        ps_db::meta::set(
            &driver,
            IDENTITY_SECRET_KEY,
            &ps_server::signal::crypto::hex32(&identity),
        )
        .await
        .unwrap();
        ps_db::meta::set(&driver, ARMED_KEY, "1").await.unwrap();
    }
    let ui_dir = temp_dir.path().join("ui");
    std::fs::create_dir_all(&ui_dir).unwrap();

    let handle = ps_server::start_server(ps_server::ServerConfig {
        host: "127.0.0.1".parse().unwrap(),
        port: 0,
        ui_dir,
        data_dir: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data"),
        db_path,
        desktop_mode: false,
    })
    .await
    .unwrap();

    broker.wait_for_a_connection().await;
    assert!(handle.services.signal.lock().await.armed());
    assert_eq!(
        broker.upgrades()[0],
        format!("room={}&role=host&kind=meet", derive_meet_room(&identity)),
        "the remembered identity is the room it dials"
    );

    handle.shutdown().await;
    broker.kill().await;
}

async fn block_meta_writes(db: &dyn ps_db::DbDriver) {
    for statement in [
        "CREATE TRIGGER meta_insert_fails BEFORE INSERT ON meta \
         BEGIN SELECT RAISE(ABORT, 'meta is read only'); END",
        "CREATE TRIGGER meta_update_fails BEFORE UPDATE ON meta \
         BEGIN SELECT RAISE(ABORT, 'meta is read only'); END",
    ] {
        db.execute(statement, &[]).await.unwrap();
    }
}

async fn allow_meta_writes(db: &dyn ps_db::DbDriver) {
    for statement in [
        "DROP TRIGGER meta_insert_fails",
        "DROP TRIGGER meta_update_fails",
    ] {
        db.execute(statement, &[]).await.unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn arming_that_cannot_be_recorded_is_rolled_back() {
    let _env = BrokerEnvGuard::acquire(DISCARD_BROKER_URL).await;
    let server = common::start_test_server().await;
    let driver = server.handle.app.driver.clone();
    let signal = server.handle.services.signal.clone();

    set_armed(&server, true).await;
    block_meta_writes(&*driver).await;

    let disarm = signal
        .lock()
        .await
        .set_armed(false, &server.handle.app)
        .await;
    assert!(disarm.is_err(), "a flag that was not written is an error");
    assert!(
        signal.lock().await.armed(),
        "a disarm nobody recorded must not leave a desktop that arms itself again at startup"
    );

    allow_meta_writes(&*driver).await;
    set_armed(&server, false).await;
    block_meta_writes(&*driver).await;

    let arm = signal
        .lock()
        .await
        .set_armed(true, &server.handle.app)
        .await;
    assert!(arm.is_err(), "a flag that was not written is an error");
    assert!(
        !signal.lock().await.armed(),
        "an arm nobody recorded must not leave the desktop listening"
    );

    allow_meta_writes(&*driver).await;
    server.handle.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn restoring_an_already_armed_desktop_leaves_one_link() {
    let (broker, _from_host_rx) = MockBroker::start().await;
    let _env = BrokerEnvGuard::acquire(&broker.url()).await;
    let server = common::start_test_server().await;
    ps_db::meta::set(&*server.handle.app.driver, ARMED_KEY, "1")
        .await
        .unwrap();

    let signal = server.handle.services.signal.clone();
    signal
        .lock()
        .await
        .restore_armed(&server.handle.app)
        .await
        .unwrap();
    broker.wait_for_a_connection().await;
    signal
        .lock()
        .await
        .restore_armed(&server.handle.app)
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(
        broker.connections.load(Ordering::Relaxed),
        1,
        "arming an armed desktop again must not dial a second link"
    );

    set_armed(&server, false).await;
    wait_until(Duration::from_secs(5), "every meet link to close", || {
        broker.live.load(Ordering::Relaxed) == 0
    })
    .await;

    server.handle.shutdown().await;
    broker.kill().await;
}
