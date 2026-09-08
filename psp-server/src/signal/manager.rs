use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use psp_app::live::{LiveFrame, SignalSourceStatus};
use psp_app::AppState;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc, watch};
use tokio_util::sync::CancellationToken;

use super::broker_client::{
    default_broker_url, fetch_turn_ice_servers, run_host_broker_link, BrokerConfig, RoomKind,
};
use super::crypto::{
    derive_device_seal_key, derive_keys, derive_meet_room, generate_device_id,
    generate_pairing_code, generate_secret32, hex32, open, parse_hex32, PairingKeys,
};
use super::gamedata_source::{default_gamedata_path, run_file_source};
use super::peer::{HostPeer, IceServerConfig, PeerError, PeerEvent};
use super::remote_ctl;
use super::rest_source::{run_rest_source, RestSourceConfig};

const PAIRING_WINDOW: Duration = Duration::from_secs(5 * 60);
const PAIRING_URL_PATH: &str = "/signal#v1.";
const PAIRING_URL_BASE_DEFAULT: &str = "https://palworldsavepal.app";
const PAIRING_URL_BASE_ENV: &str = "PSP_SIGNAL_PAIRING_URL_BASE";
const STATUS_BLIP_GRACE: Duration = Duration::from_secs(5);
const CODE_GROUP_LEN: usize = 4;
const SIGNALING_CAPACITY: usize = 32;
const PEER_EVENT_CAPACITY: usize = 8;
const CTL_CAPACITY: usize = 16;

const IDENTITY_SECRET_KEY: &str = "signal_identity_secret_hex";
const ARMED_KEY: &str = "signal_armed";
const ARMED_FLAG_ON: &str = "1";
const ARMED_FLAG_OFF: &str = "0";
const MEET_RELINK_BACKOFF: Duration = Duration::from_secs(30);
const MEET_RELINK_BACKOFF_ENV: &str = "PSP_SIGNAL_MEET_RELINK_BACKOFF_MS";
const REVOKE_CAPACITY: usize = 8;
const MEET_WRAPPER_VERSION: u8 = 1;
const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(60);
const GUEST_AAD: &[u8] = b"guest";
const BASE64: base64::engine::general_purpose::GeneralPurpose =
    base64::engine::general_purpose::STANDARD;

const DEVICE_CREDENTIAL_ACK_TIMEOUT: Duration = Duration::from_secs(30);
const DEVICE_CREDENTIAL_ACK_TIMEOUT_ENV: &str = "PSP_SIGNAL_DEVICE_ACK_TIMEOUT_MS";
const DEFAULT_DEVICE_NAME: &str = "Paired device";
const FALLBACK_DESKTOP_NAME: &str = "Palworld Save Pal";

const SESSION_END_FLUSH: Duration = Duration::from_millis(200);

#[derive(Debug, Clone, PartialEq)]
pub enum SourceSelection {
    Off,
    File {
        path: Option<PathBuf>,
    },
    Server {
        server_id: i64,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum PairingState {
    Off,
    Waiting {
        code: String,
        expires_at_ms: i64,
    },
    Connected,
    Failed,
}

impl PairingState {
    pub fn label(&self) -> &'static str {
        match self {
            PairingState::Off => "off",
            PairingState::Waiting { .. } => "waiting",
            PairingState::Connected => "connected",
            PairingState::Failed => "failed",
        }
    }
}

pub fn group_code(code: &str) -> String {
    code.as_bytes()
        .chunks(CODE_GROUP_LEN)
        .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
        .collect::<Vec<_>>()
        .join("-")
}

pub fn pairing_url(code: &str) -> String {
    format!("{}{PAIRING_URL_PATH}{code}", pairing_url_base())
}

fn pairing_url_base() -> String {
    std::env::var(PAIRING_URL_BASE_ENV)
        .ok()
        .map(|value| value.trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| PAIRING_URL_BASE_DEFAULT.to_string())
}

struct ActiveTask {
    cancel: CancellationToken,
    handle: tokio::task::JoinHandle<()>,
}

struct PairingSession {
    cancel: CancellationToken,
    tasks: Vec<tokio::task::JoinHandle<()>>,
    ctl_out: mpsc::Sender<String>,
    minted_device_rx: watch::Receiver<Option<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectedDevice {
    pub device_id: String,
    pub name: String,
}

#[derive(Serialize, Deserialize)]
struct MeetWrapper {
    v: u8,
    device: String,
    blob: String,
}

#[derive(Debug, PartialEq)]
enum MeetRoute {
    Ignore,
    Forward,
    Redial,
    Open,
}

struct ArmedSession {
    cancel: CancellationToken,
    task: tokio::task::JoinHandle<()>,
    going_away: CancellationToken,
    revoke_tx: broadcast::Sender<String>,
}

#[derive(Clone)]
struct MeetChannels {
    state: Arc<AppState>,
    status_rx: watch::Receiver<SignalSourceStatus>,
    pairing_tx: watch::Sender<PairingState>,
    connected_device_tx: watch::Sender<Option<ConnectedDevice>>,
    revoke_tx: broadcast::Sender<String>,
    going_away: CancellationToken,
}

struct DeviceSession {
    device_id: String,
    seal_key: [u8; 32],
    open_blob: String,
    signaling_tx: mpsc::Sender<String>,
    cancel: CancellationToken,
    task: tokio::task::JoinHandle<()>,
    seq: u64,
}

pub struct SignalManager {
    active: Option<ActiveTask>,
    status_tx: watch::Sender<SignalSourceStatus>,
    status_rx: watch::Receiver<SignalSourceStatus>,
    pairing_tx: watch::Sender<PairingState>,
    pairing_rx: watch::Receiver<PairingState>,
    connected_device_tx: watch::Sender<Option<ConnectedDevice>>,
    connected_device_rx: watch::Receiver<Option<ConnectedDevice>>,
    pairing: Option<PairingSession>,
    armed: Option<ArmedSession>,
    frame_seq: Arc<AtomicU64>,
}

impl SignalManager {
    pub fn new() -> Self {
        let (status_tx, status_rx) = watch::channel(SignalSourceStatus::default());
        let (pairing_tx, pairing_rx) = watch::channel(PairingState::Off);
        let (connected_device_tx, connected_device_rx) = watch::channel(None);
        Self {
            active: None,
            status_tx,
            status_rx,
            pairing_tx,
            pairing_rx,
            connected_device_tx,
            connected_device_rx,
            pairing: None,
            armed: None,
            frame_seq: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn status(&self) -> SignalSourceStatus {
        self.status_rx.borrow().clone()
    }

    pub fn pairing(&self) -> PairingState {
        self.pairing_rx.borrow().clone()
    }

    pub fn connected_device(&self) -> Option<ConnectedDevice> {
        self.connected_device_rx.borrow().clone()
    }

    pub async fn set_source(
        &mut self,
        selection: SourceSelection,
        state: &Arc<AppState>,
    ) -> Result<(), String> {
        match selection {
            SourceSelection::Off => {
                self.cancel_active().await;
                state.live_bus.send_replace(None);
                self.status_tx.send_replace(SignalSourceStatus::default());
                Ok(())
            }
            SourceSelection::File { path } => {
                let path = match path {
                    Some(path) => path,
                    None => default_gamedata_path()
                        .ok_or_else(|| "No default game-data path on this platform".to_string())?,
                };
                self.cancel_active().await;
                let cancel = CancellationToken::new();
                let handle = tokio::spawn(run_file_source(
                    path,
                    state.live_bus.clone(),
                    self.status_tx.clone(),
                    self.frame_seq.clone(),
                    cancel.clone(),
                ));
                self.active = Some(ActiveTask { cancel, handle });
                Ok(())
            }
            SourceSelection::Server { server_id } => {
                let record = psp_db::servers::get_server(&*state.driver, server_id)
                    .await
                    .map_err(|error| error.to_string())?
                    .ok_or_else(|| "Server not found".to_string())?;
                let cfg = RestSourceConfig {
                    host: "127.0.0.1".to_string(),
                    port: record.rest_api_port as u16,
                    admin_password: record.admin_password,
                    endpoint_base: "v1/api".to_string(),
                };
                self.cancel_active().await;
                let cancel = CancellationToken::new();
                let handle = tokio::spawn(run_rest_source(
                    cfg,
                    state.live_bus.clone(),
                    self.status_tx.clone(),
                    self.frame_seq.clone(),
                    cancel.clone(),
                ));
                self.active = Some(ActiveTask { cancel, handle });
                Ok(())
            }
        }
    }

    pub async fn start_pairing(
        &mut self,
        state: &Arc<AppState>,
        ice_servers: Vec<IceServerConfig>,
    ) -> PairingState {
        self.cancel_pairing().await;

        let code = generate_pairing_code();
        let keys = derive_keys(&code);
        let room_id = keys.room_id.clone();
        let cancel = CancellationToken::new();
        let expires_at_ms = now_ms() + PAIRING_WINDOW.as_millis() as i64;
        self.pairing_tx.send_replace(PairingState::Waiting {
            code,
            expires_at_ms,
        });

        let (to_guest_tx, to_guest_rx) = mpsc::channel(SIGNALING_CAPACITY);
        let (from_guest_tx, from_guest_rx) = mpsc::channel(SIGNALING_CAPACITY);
        let (events_tx, events_rx) = mpsc::channel(PEER_EVENT_CAPACITY);
        let (ctl_out_tx, ctl_out_rx) = mpsc::channel(CTL_CAPACITY);
        let (ctl_in_tx, ctl_in_rx) = mpsc::channel(CTL_CAPACITY);

        let link_cancel = cancel.child_token();
        let link = tokio::spawn(run_host_broker_link(
            BrokerConfig {
                base_ws_url: default_broker_url(),
            },
            room_id,
            RoomKind::Pair,
            to_guest_rx,
            from_guest_tx,
            link_cancel.clone(),
        ));

        let mut ice = fetch_turn_ice_servers().await;
        ice.extend(ice_servers);

        let live_rx = state.live_bus.subscribe();
        let status_rx = self.status_rx.clone();
        let peer = tokio::spawn(run_peer_session(
            HostPeer::run(
                keys,
                ice,
                from_guest_rx,
                to_guest_tx,
                live_rx,
                status_rx,
                ctl_out_rx,
                ctl_in_tx,
                events_tx,
                cancel.clone(),
            ),
            self.pairing_tx.clone(),
            cancel.clone(),
        ));

        let pump = tokio::spawn(pump_peer_events(
            events_rx,
            self.pairing_tx.clone(),
            cancel.clone(),
            link_cancel,
        ));
        let window = tokio::spawn(run_pairing_window(
            PAIRING_WINDOW,
            self.pairing_tx.clone(),
            cancel.clone(),
        ));
        let (minted_device_tx, minted_device_rx) = watch::channel(None);
        let mint_driver = state.driver.clone();
        let mint_pairing_rx = self.pairing_tx.subscribe();
        let mint_ctl_out = ctl_out_tx.clone();
        let mint_ack_timeout = device_credential_ack_timeout();
        let mint_cancel = cancel.clone();
        let remote_ctl_app = Arc::clone(state);
        let remote_ctl_ctl_out = ctl_out_tx.clone();
        let remote_ctl_cancel = cancel.clone();
        let credential = tokio::spawn(async move {
            let ctl_in_rx = run_credential_mint(
                mint_driver,
                mint_pairing_rx,
                mint_ctl_out,
                ctl_in_rx,
                mint_ack_timeout,
                minted_device_tx,
                mint_cancel,
            )
            .await;
            remote_ctl::run_ctl_bridge(
                ctl_in_rx,
                remote_ctl_ctl_out,
                remote_ctl_app,
                remote_ctl_cancel,
            )
            .await;
        });

        self.pairing = Some(PairingSession {
            cancel,
            tasks: vec![link, peer, pump, window, credential],
            ctl_out: ctl_out_tx,
            minted_device_rx,
        });
        self.pairing()
    }

    fn pairing_guest_connected(&self) -> bool {
        self.pairing.is_some()
            && matches!(*self.pairing_rx.borrow(), PairingState::Connected)
            && self.connected_device_rx.borrow().is_none()
    }

    pub async fn stop_pairing(&mut self) {
        if self.pairing_guest_connected() {
            if let Some(session) = &self.pairing {
                send_session_end(&session.ctl_out, &session.cancel).await;
            }
        }
        self.cancel_pairing().await;
        let device_connected = self.connected_device_rx.borrow().is_some();
        self.pairing_tx.send_if_modified(|state| {
            if matches!(state, PairingState::Off)
                || (device_connected && matches!(state, PairingState::Connected))
            {
                return false;
            }
            *state = PairingState::Off;
            true
        });
    }

    pub fn armed(&self) -> bool {
        self.armed.is_some()
    }

    pub async fn set_armed(&mut self, armed: bool, state: &Arc<AppState>) -> Result<(), String> {
        let changed = armed != self.armed();
        if changed {
            if armed {
                self.arm(state).await?;
            } else {
                self.cancel_armed().await;
            }
        }
        let flag = if armed { ARMED_FLAG_ON } else { ARMED_FLAG_OFF };
        if let Err(error) = psp_db::meta::set(&*state.driver, ARMED_KEY, flag).await {
            if changed {
                self.revert_arming(armed, state).await;
            }
            return Err(error.to_string());
        }
        Ok(())
    }

    async fn revert_arming(&mut self, attempted: bool, state: &Arc<AppState>) {
        if attempted {
            self.cancel_armed().await;
        } else if let Err(error) = self.arm(state).await {
            tracing::warn!(
                %error,
                "signal: could not restore the meet link after a disarm that was never recorded"
            );
        }
    }

    pub async fn restore_armed(&mut self, state: &Arc<AppState>) -> Result<(), String> {
        let stored = psp_db::meta::get(&*state.driver, ARMED_KEY)
            .await
            .map_err(|error| error.to_string())?;
        if stored.as_deref() != Some(ARMED_FLAG_ON) {
            return Ok(());
        }
        self.arm(state).await
    }

    pub async fn reset_remote_access(&mut self, state: &Arc<AppState>) -> Result<(), String> {
        let armed = self.armed();
        self.cancel_armed().await;
        self.stop_pairing().await;
        psp_db::signal_devices::delete_all_devices(&*state.driver)
            .await
            .map_err(|error| error.to_string())?;
        psp_db::meta::set(
            &*state.driver,
            IDENTITY_SECRET_KEY,
            &hex32(&generate_secret32()),
        )
        .await
        .map_err(|error| error.to_string())?;
        if armed {
            self.arm(state).await?;
        }
        Ok(())
    }

    pub async fn revoke_device_session(&mut self, device_id: &str) {
        if let Some(session) = &self.armed {
            let _ = session.revoke_tx.send(device_id.to_string());
        }
        let minted = self
            .pairing
            .as_ref()
            .and_then(|session| session.minted_device_rx.borrow().clone());
        if minted.as_deref() == Some(device_id) {
            self.stop_pairing().await;
        }
    }

    async fn arm(&mut self, state: &Arc<AppState>) -> Result<(), String> {
        if self.armed() {
            return Ok(());
        }
        let identity = load_or_mint_identity(&*state.driver).await?;
        let room_id = derive_meet_room(&identity);
        let cancel = CancellationToken::new();
        let going_away = CancellationToken::new();
        let (revoke_tx, _) = broadcast::channel(REVOKE_CAPACITY);
        let task = tokio::spawn(run_armed_link(
            room_id,
            MeetChannels {
                state: state.clone(),
                status_rx: self.status_rx.clone(),
                pairing_tx: self.pairing_tx.clone(),
                connected_device_tx: self.connected_device_tx.clone(),
                revoke_tx: revoke_tx.clone(),
                going_away: going_away.clone(),
            },
            meet_relink_backoff(),
            cancel.clone(),
        ));

        self.armed = Some(ArmedSession {
            cancel,
            task,
            going_away,
            revoke_tx,
        });
        Ok(())
    }

    async fn cancel_active(&mut self) {
        if let Some(active) = self.active.take() {
            active.cancel.cancel();
            let _ = active.handle.await;
        }
    }

    async fn cancel_pairing(&mut self) {
        if let Some(session) = self.pairing.take() {
            session.cancel.cancel();
            for task in session.tasks {
                let _ = task.await;
            }
        }
    }

    async fn cancel_armed(&mut self) {
        if let Some(session) = self.armed.take() {
            session.cancel.cancel();
            let _ = session.task.await;
        }
    }

    pub async fn shutdown(&mut self) {
        self.cancel_active().await;
        if let Some(session) = &self.armed {
            session.going_away.cancel();
        }
        self.cancel_armed().await;
        self.stop_pairing().await;
    }
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

async fn send_session_end(ctl_out: &mpsc::Sender<String>, alive: &CancellationToken) {
    let envelope = serde_json::json!({ "type": "session_end", "data": {} }).to_string();
    let queued = tokio::select! {
        _ = alive.cancelled() => return,
        sent = tokio::time::timeout(SESSION_END_FLUSH, ctl_out.send(envelope)) => {
            matches!(sent, Ok(Ok(())))
        }
    };
    if !queued {
        return;
    }
    tokio::select! {
        _ = alive.cancelled() => {}
        _ = tokio::time::sleep(SESSION_END_FLUSH) => {}
    }
}

async fn load_or_mint_identity(db: &dyn psp_db::DbDriver) -> Result<[u8; 32], String> {
    let stored = psp_db::meta::get(db, IDENTITY_SECRET_KEY)
        .await
        .map_err(|error| error.to_string())?;
    if let Some(secret) = stored.as_deref().and_then(parse_hex32) {
        return Ok(secret);
    }
    let secret = generate_secret32();
    psp_db::meta::set(db, IDENTITY_SECRET_KEY, &hex32(&secret))
        .await
        .map_err(|error| error.to_string())?;
    Ok(secret)
}

async fn device_seal_key(db: &dyn psp_db::DbDriver, device_id: &str) -> Option<[u8; 32]> {
    let devices = psp_db::signal_devices::list_devices(db).await.ok()?;
    let device = devices
        .into_iter()
        .find(|device| device.device_id == device_id)?;
    parse_hex32(&device.secret_hex).map(|secret| derive_device_seal_key(&secret))
}

async fn device_name(db: &dyn psp_db::DbDriver, device_id: &str) -> Option<String> {
    let devices = psp_db::signal_devices::list_devices(db).await.ok()?;
    devices
        .into_iter()
        .find(|device| device.device_id == device_id)
        .map(|device| device.name)
}

fn device_credential_ack_timeout() -> Duration {
    std::env::var(DEVICE_CREDENTIAL_ACK_TIMEOUT_ENV)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_millis)
        .unwrap_or(DEVICE_CREDENTIAL_ACK_TIMEOUT)
}

fn desktop_hostname() -> String {
    sysinfo::System::host_name()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| FALLBACK_DESKTOP_NAME.to_string())
}

fn trimmed_or_default(name: &str, default: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        default.to_string()
    } else {
        trimmed.to_string()
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_credential_mint(
    driver: Arc<dyn psp_db::DbDriver>,
    mut pairing_rx: watch::Receiver<PairingState>,
    ctl_out_tx: mpsc::Sender<String>,
    mut ctl_in_rx: mpsc::Receiver<serde_json::Value>,
    ack_timeout: Duration,
    minted_device_tx: watch::Sender<Option<String>>,
    cancel: CancellationToken,
) -> mpsc::Receiver<serde_json::Value> {
    tokio::select! {
        _ = cancel.cancelled() => return ctl_in_rx,
        result = pairing_rx.wait_for(|state| matches!(state, PairingState::Connected)) => {
            if result.is_err() {
                return ctl_in_rx;
            }
        }
    }

    let identity = match load_or_mint_identity(&*driver).await {
        Ok(identity) => identity,
        Err(error) => {
            tracing::warn!(%error, "signal: failed to load or mint the identity secret while minting a device credential");
            return ctl_in_rx;
        }
    };
    let device_id = generate_device_id();
    let device_secret = generate_secret32();
    let envelope = serde_json::json!({
        "type": "device_credential",
        "data": {
            "deviceId": device_id,
            "deviceSecret": hex32(&device_secret),
            "meetRoom": derive_meet_room(&identity),
            "desktopName": desktop_hostname(),
        }
    })
    .to_string();
    tokio::select! {
        _ = cancel.cancelled() => return ctl_in_rx,
        sent = ctl_out_tx.send(envelope) => {
            if sent.is_err() {
                return ctl_in_rx;
            }
        }
    }

    let Some(name) = await_device_credential_ack(&mut ctl_in_rx, ack_timeout, &cancel).await else {
        return ctl_in_rx;
    };

    let device = psp_db::signal_devices::SignalDevice {
        device_id,
        secret_hex: hex32(&device_secret),
        name: trimmed_or_default(&name, DEFAULT_DEVICE_NAME),
        created_at_ms: now_ms(),
        last_seen_ms: None,
    };
    match psp_db::signal_devices::insert_device(&*driver, &device).await {
        Ok(()) => {
            minted_device_tx.send_replace(Some(device.device_id));
        }
        Err(error) => {
            tracing::warn!(%error, "signal: failed to persist a newly paired device")
        }
    }
    ctl_in_rx
}

async fn await_device_credential_ack(
    ctl_in_rx: &mut mpsc::Receiver<serde_json::Value>,
    timeout: Duration,
    cancel: &CancellationToken,
) -> Option<String> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        tokio::select! {
            _ = cancel.cancelled() => return None,
            _ = tokio::time::sleep_until(deadline) => return None,
            envelope = ctl_in_rx.recv() => {
                let envelope = envelope?;
                if envelope.get("type").and_then(|t| t.as_str()) != Some("device_credential") {
                    continue;
                }
                return Some(
                    envelope["data"]["name"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                );
            }
        }
    }
}

fn parse_meet_wrapper(text: &str) -> Option<MeetWrapper> {
    let wrapper: MeetWrapper = serde_json::from_str(text).ok()?;
    (wrapper.v == MEET_WRAPPER_VERSION).then_some(wrapper)
}

fn route_wrapper(active: Option<(&str, &[u8; 32])>, device: &str, blob: &str) -> MeetRoute {
    match active {
        None => MeetRoute::Open,
        Some((active_device, seal_key)) if active_device == device => {
            if opens_as_offer(seal_key, blob) {
                MeetRoute::Redial
            } else {
                MeetRoute::Forward
            }
        }
        Some(_) => MeetRoute::Ignore,
    }
}

fn opens_as_offer(seal_key: &[u8; 32], blob: &str) -> bool {
    let Ok(sealed) = BASE64.decode(blob) else {
        return false;
    };
    let Ok(plaintext) = open(seal_key, &sealed, GUEST_AAD) else {
        return false;
    };
    let Ok(envelope) = serde_json::from_slice::<serde_json::Value>(&plaintext) else {
        return false;
    };
    envelope.get("t").and_then(|t| t.as_str()) == Some("offer")
}

fn meet_relink_backoff() -> Duration {
    std::env::var(MEET_RELINK_BACKOFF_ENV)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_millis)
        .unwrap_or(MEET_RELINK_BACKOFF)
}

async fn run_armed_link(
    room_id: String,
    channels: MeetChannels,
    backoff: Duration,
    cancel: CancellationToken,
) {
    loop {
        let (to_guest_tx, to_guest_rx) = mpsc::channel(SIGNALING_CAPACITY);
        let (from_guest_tx, from_guest_rx) = mpsc::channel(SIGNALING_CAPACITY);

        let attempt = cancel.child_token();
        let mut link = tokio::spawn(run_host_broker_link(
            BrokerConfig {
                base_ws_url: default_broker_url(),
            },
            room_id.clone(),
            RoomKind::Meet,
            to_guest_rx,
            from_guest_tx,
            attempt.child_token(),
        ));
        let mut listener = tokio::spawn(run_meet_listener(
            room_id.clone(),
            from_guest_rx,
            to_guest_tx,
            channels.clone(),
            attempt.child_token(),
        ));

        let link_ended = tokio::select! {
            _ = &mut link => true,
            _ = &mut listener => false,
        };
        attempt.cancel();
        if link_ended {
            let _ = listener.await;
        } else {
            let _ = link.await;
        }
        if cancel.is_cancelled() {
            break;
        }

        tracing::warn!(
            backoff_ms = backoff.as_millis() as u64,
            "signal: the meet link ended while armed; dialing again after a backoff"
        );
        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = tokio::time::sleep(backoff) => {}
        }
    }
}

async fn run_meet_listener(
    room_id: String,
    mut from_guest_rx: mpsc::Receiver<String>,
    to_guest_tx: mpsc::Sender<String>,
    channels: MeetChannels,
    cancel: CancellationToken,
) {
    let (ended_tx, mut ended_rx) = mpsc::channel::<u64>(4);
    let mut revoked_rx = channels.revoke_tx.subscribe();
    let mut active: Option<DeviceSession> = None;
    let mut sessions: u64 = 0;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            revoked = revoked_rx.recv() => {
                let ends_the_session = match revoked {
                    Ok(device_id) => active
                        .as_ref()
                        .is_some_and(|session| session.device_id == device_id),
                    Err(broadcast::error::RecvError::Lagged(_)) => true,
                    Err(broadcast::error::RecvError::Closed) => break,
                };
                if ends_the_session {
                    if let Some(session) = active.take() {
                        session.cancel.cancel();
                        while ended_rx.try_recv().is_ok() {}
                        let _ = session.task.await;
                    }
                }
            }
            ended = ended_rx.recv() => {
                let Some(seq) = ended else { break };
                if active.as_ref().is_some_and(|session| session.seq == seq) {
                    if let Some(session) = active.take() {
                        let _ = session.task.await;
                    }
                }
            }
            incoming = from_guest_rx.recv() => {
                let Some(text) = incoming else { break };
                let Some(wrapper) = parse_meet_wrapper(&text) else { continue };
                if active
                    .as_ref()
                    .is_some_and(|session| session.open_blob == wrapper.blob)
                {
                    continue;
                }
                let route = route_wrapper(
                    active.as_ref().map(|s| (s.device_id.as_str(), &s.seal_key)),
                    &wrapper.device,
                    &wrapper.blob,
                );
                match route {
                    MeetRoute::Ignore => {}
                    MeetRoute::Forward => {
                        let Some(session) = active.as_ref() else { continue };
                        tokio::select! {
                            _ = cancel.cancelled() => break,
                            sent = session.signaling_tx.send(wrapper.blob) => { sent.ok(); }
                        }
                    }
                    MeetRoute::Redial => {
                        let Some(session) = active.take() else { continue };
                        let seal_key = session.seal_key;
                        session.cancel.cancel();
                        while ended_rx.try_recv().is_ok() {}
                        let _ = session.task.await;
                        sessions += 1;
                        active = Some(
                            start_device_session(
                                &room_id,
                                wrapper.device,
                                seal_key,
                                wrapper.blob,
                                &channels,
                                &to_guest_tx,
                                &ended_tx,
                                sessions,
                                &cancel,
                            )
                            .await,
                        );
                    }
                    MeetRoute::Open => {
                        let Some(seal_key) = device_seal_key(&*channels.state.driver, &wrapper.device).await else {
                            continue;
                        };
                        if !opens_as_offer(&seal_key, &wrapper.blob) {
                            continue;
                        }
                        sessions += 1;
                        active = Some(
                            start_device_session(
                                &room_id,
                                wrapper.device,
                                seal_key,
                                wrapper.blob,
                                &channels,
                                &to_guest_tx,
                                &ended_tx,
                                sessions,
                                &cancel,
                            )
                            .await,
                        );
                    }
                }
            }
        }
    }

    if let Some(session) = active.take() {
        session.cancel.cancel();
        while ended_rx.try_recv().is_ok() {}
        let _ = session.task.await;
    }
}

#[allow(clippy::too_many_arguments)]
async fn start_device_session(
    room_id: &str,
    device: String,
    seal_key: [u8; 32],
    offer: String,
    channels: &MeetChannels,
    to_guest_tx: &mpsc::Sender<String>,
    ended_tx: &mpsc::Sender<u64>,
    seq: u64,
    cancel: &CancellationToken,
) -> DeviceSession {
    let (signaling_tx, signaling_rx) = mpsc::channel(SIGNALING_CAPACITY);
    let open_blob = offer.clone();
    signaling_tx.send(offer).await.ok();
    let session_cancel = cancel.child_token();
    let task = tokio::spawn(run_device_session(
        PairingKeys {
            room_id: room_id.to_string(),
            seal_key,
        },
        signaling_rx,
        to_guest_tx.clone(),
        channels.state.live_bus.subscribe(),
        channels.status_rx.clone(),
        channels.pairing_tx.clone(),
        ended_tx.clone(),
        seq,
        device.clone(),
        channels.state.driver.clone(),
        Arc::clone(&channels.state),
        channels.connected_device_tx.clone(),
        channels.going_away.clone(),
        session_cancel.clone(),
    ));
    DeviceSession {
        device_id: device,
        seal_key,
        open_blob,
        signaling_tx,
        cancel: session_cancel,
        task,
        seq,
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_device_session(
    keys: PairingKeys,
    signaling_rx: mpsc::Receiver<String>,
    signaling_tx: mpsc::Sender<String>,
    live_rx: watch::Receiver<Option<LiveFrame>>,
    status_rx: watch::Receiver<SignalSourceStatus>,
    pairing_tx: watch::Sender<PairingState>,
    ended_tx: mpsc::Sender<u64>,
    seq: u64,
    device_id: String,
    driver: Arc<dyn psp_db::DbDriver>,
    app: Arc<AppState>,
    connected_device: watch::Sender<Option<ConnectedDevice>>,
    going_away: CancellationToken,
    cancel: CancellationToken,
) {
    let (events_tx, events_rx) = mpsc::channel(PEER_EVENT_CAPACITY);
    let (ctl_out_tx, ctl_out_rx) = mpsc::channel(CTL_CAPACITY);
    let (ctl_in_tx, ctl_in_rx) = mpsc::channel(CTL_CAPACITY);

    let peer_cancel = CancellationToken::new();
    let peer_token = peer_cancel.clone();
    let ice_servers = fetch_turn_ice_servers().await;
    let peer = tokio::spawn(async move {
        if let Err(error) = HostPeer::run(
            keys,
            ice_servers,
            signaling_rx,
            signaling_tx,
            live_rx,
            status_rx,
            ctl_out_rx,
            ctl_in_tx,
            events_tx,
            peer_token,
        )
        .await
        {
            tracing::warn!(%error, "signal device peer ended with an error");
        }
    });
    let remote_ctl_task = tokio::spawn(remote_ctl::run_ctl_bridge(
        ctl_in_rx,
        ctl_out_tx.clone(),
        app,
        cancel.clone(),
    ));

    let connected = watch_device_peer(
        events_rx,
        pairing_tx,
        ATTEMPT_TIMEOUT,
        device_id,
        driver,
        connected_device,
        cancel.clone(),
    )
    .await;
    if connected && cancel.is_cancelled() && !going_away.is_cancelled() {
        send_session_end(&ctl_out_tx, &peer_cancel).await;
    }
    cancel.cancel();
    peer_cancel.cancel();
    let _ = peer.await;
    let _ = remote_ctl_task.await;
    ended_tx.send(seq).await.ok();
}

#[allow(clippy::too_many_arguments)]
async fn watch_device_peer(
    mut events: mpsc::Receiver<PeerEvent>,
    pairing: watch::Sender<PairingState>,
    attempt_timeout: Duration,
    device_id: String,
    driver: Arc<dyn psp_db::DbDriver>,
    connected_device: watch::Sender<Option<ConnectedDevice>>,
    cancel: CancellationToken,
) -> bool {
    let deadline = tokio::time::Instant::now() + attempt_timeout;
    let mut connected = false;
    let mut pending_end_at: Option<tokio::time::Instant> = None;
    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = tokio::time::sleep_until(deadline), if !connected => {
                tracing::debug!("signal: abandoning a device session that never connected");
                break;
            }
            _ = sleep_until_or_forever(pending_end_at) => break,
            event = events.recv() => {
                let Some(event) = event else { break };
                match event {
                    PeerEvent::Connected => {
                        connected = true;
                        pending_end_at = None;
                        pairing.send_replace(PairingState::Connected);
                        if let Err(error) =
                            psp_db::signal_devices::touch_device(&*driver, &device_id, now_ms()).await
                        {
                            tracing::warn!(%error, "signal: failed to record a device's last-seen time");
                        }
                        let name = device_name(&*driver, &device_id).await.unwrap_or_default();
                        connected_device.send_replace(Some(ConnectedDevice {
                            device_id: device_id.clone(),
                            name,
                        }));
                    }
                    PeerEvent::Disconnected if connected => {
                        pending_end_at = Some(tokio::time::Instant::now() + STATUS_BLIP_GRACE);
                    }
                    PeerEvent::Disconnected | PeerEvent::Failed => break,
                }
            }
        }
    }
    if connected {
        pairing.send_if_modified(|state| {
            if matches!(state, PairingState::Connected) {
                *state = PairingState::Off;
                true
            } else {
                false
            }
        });
        connected_device.send_replace(None);
    }
    connected
}

async fn run_peer_session(
    peer: impl std::future::Future<Output = Result<(), PeerError>>,
    pairing: watch::Sender<PairingState>,
    cancel: CancellationToken,
) {
    if let Err(error) = peer.await {
        tracing::warn!(%error, "signal peer ended with an error");
    }
    if !cancel.is_cancelled() {
        pairing.send_if_modified(|state| {
            if matches!(state, PairingState::Waiting { .. }) {
                *state = PairingState::Failed;
                true
            } else {
                false
            }
        });
    }
    cancel.cancel();
}

async fn sleep_until_or_forever(deadline: Option<tokio::time::Instant>) {
    match deadline {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}

async fn pump_peer_events(
    mut events: mpsc::Receiver<PeerEvent>,
    pairing: watch::Sender<PairingState>,
    cancel: CancellationToken,
    link: CancellationToken,
) {
    let mut pending_off_at: Option<tokio::time::Instant> = None;
    loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => break,
            event = events.recv() => {
                let Some(event) = event else {
                    if pending_off_at.take().is_some() && !cancel.is_cancelled() {
                        pairing.send_replace(PairingState::Off);
                    }
                    break;
                };
                if cancel.is_cancelled() {
                    break;
                }
                match event {
                    PeerEvent::Connected => {
                        link.cancel();
                        pending_off_at = None;
                        pairing.send_replace(PairingState::Connected);
                    }
                    PeerEvent::Disconnected => {
                        pending_off_at = Some(tokio::time::Instant::now() + STATUS_BLIP_GRACE);
                    }
                    PeerEvent::Failed => {
                        pending_off_at = None;
                        pairing.send_replace(PairingState::Failed);
                    }
                }
            }
            _ = sleep_until_or_forever(pending_off_at) => {
                if !cancel.is_cancelled() {
                    pairing.send_replace(PairingState::Off);
                }
                pending_off_at = None;
            }
        }
    }
}

async fn run_pairing_window(
    window: Duration,
    pairing: watch::Sender<PairingState>,
    cancel: CancellationToken,
) {
    tokio::select! {
        _ = cancel.cancelled() => {}
        _ = tokio::time::sleep(window) => {
            let expired = pairing.send_if_modified(|state| {
                if matches!(state, PairingState::Waiting { .. }) {
                    *state = PairingState::Off;
                    true
                } else {
                    false
                }
            });
            if expired {
                cancel.cancel();
            }
        }
    }
}

impl Default for SignalManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signal::crypto::{normalize_code, seal};
    use crate::signal::test_support::SignalEnvGuard;

    const DEVICE_SECRET: [u8; 32] = [0x5au8; 32];

    async fn test_driver() -> psp_db::SqlxSqliteDriver {
        let dir = tempfile::tempdir().unwrap();
        let pool = psp_db::open(&dir.path().join("psp.db")).await.unwrap();
        std::mem::forget(dir);
        psp_db::SqlxSqliteDriver::new(pool)
    }

    async fn test_driver_arc() -> Arc<dyn psp_db::DbDriver> {
        Arc::new(test_driver().await)
    }

    fn device_row(device_id: &str, secret: &[u8; 32]) -> psp_db::signal_devices::SignalDevice {
        psp_db::signal_devices::SignalDevice {
            device_id: device_id.to_string(),
            secret_hex: hex32(secret),
            name: "Phone".to_string(),
            created_at_ms: 1_700_000_000_000,
            last_seen_ms: None,
        }
    }

    fn wrapper(device: &str, blob: &str) -> String {
        serde_json::json!({ "v": 1, "device": device, "blob": blob }).to_string()
    }

    fn guest_blob(seal_key: &[u8; 32], envelope: serde_json::Value) -> String {
        BASE64.encode(seal(seal_key, envelope.to_string().as_bytes(), GUEST_AAD))
    }

    fn waiting() -> PairingState {
        PairingState::Waiting {
            code: generate_pairing_code(),
            expires_at_ms: now_ms(),
        }
    }

    #[test]
    fn group_code_hyphenates_every_four_characters() {
        assert_eq!(
            group_code("ABCDEFGHJKMNPQRSTUVWXY2345"),
            "ABCD-EFGH-JKMN-PQRS-TUVW-XY23-45"
        );
    }

    #[test]
    fn grouping_is_reversible_by_the_guest_normalizer() {
        let code = generate_pairing_code();
        assert_eq!(normalize_code(&group_code(&code)), code);
    }

    #[test]
    fn the_pairing_url_carries_the_ungrouped_code_in_the_fragment() {
        let code = generate_pairing_code();
        let url = pairing_url(&code);
        assert_eq!(url, format!("https://palworldsavepal.app/signal#v1.{code}"));
        assert!(url.ends_with(&code) && !code.contains('-'));
    }

    #[tokio::test]
    async fn the_window_expires_an_unclaimed_code_and_cancels_the_session() {
        let (pairing, _rx) = watch::channel(waiting());
        let cancel = CancellationToken::new();
        run_pairing_window(Duration::from_millis(1), pairing.clone(), cancel.clone()).await;
        assert_eq!(*pairing.borrow(), PairingState::Off);
        assert!(cancel.is_cancelled());
    }

    #[tokio::test]
    async fn the_window_leaves_a_connected_session_running() {
        let (pairing, _rx) = watch::channel(PairingState::Connected);
        let cancel = CancellationToken::new();
        run_pairing_window(Duration::from_millis(1), pairing.clone(), cancel.clone()).await;
        assert_eq!(*pairing.borrow(), PairingState::Connected);
        assert!(!cancel.is_cancelled());
    }

    #[tokio::test]
    async fn a_cancelled_window_expires_nothing() {
        let (pairing, _rx) = watch::channel(waiting());
        let before = pairing.borrow().clone();
        let cancel = CancellationToken::new();
        cancel.cancel();
        run_pairing_window(Duration::from_secs(300), pairing.clone(), cancel).await;
        assert_eq!(*pairing.borrow(), before);
    }

    #[tokio::test]
    async fn a_peer_that_ends_on_its_own_fails_the_pairing() {
        for outcome in [
            Ok(()),
            Err(PeerError::WebRtc(webrtc::error::Error::ErrBufferClosed)),
        ] {
            let (pairing, _rx) = watch::channel(waiting());
            let cancel = CancellationToken::new();
            run_peer_session(async { outcome }, pairing.clone(), cancel.clone()).await;
            assert_eq!(*pairing.borrow(), PairingState::Failed);
            assert!(cancel.is_cancelled());
        }
    }

    #[tokio::test]
    async fn a_cancelled_peer_leaves_the_state_to_whoever_cancelled_it() {
        let (pairing, _rx) = watch::channel(waiting());
        let before = pairing.borrow().clone();
        let cancel = CancellationToken::new();
        cancel.cancel();
        run_peer_session(async { Ok(()) }, pairing.clone(), cancel).await;
        assert_eq!(*pairing.borrow(), before);
    }

    #[tokio::test]
    async fn a_peer_ending_after_connecting_does_not_report_a_failure() {
        let (pairing, _rx) = watch::channel(PairingState::Off);
        let cancel = CancellationToken::new();
        run_peer_session(async { Ok(()) }, pairing.clone(), cancel).await;
        assert_eq!(*pairing.borrow(), PairingState::Off);
    }

    #[tokio::test]
    async fn the_event_pump_maps_peer_events_onto_the_pairing_state() {
        let (pairing, _rx) = watch::channel(waiting());
        let (events, events_rx) = mpsc::channel(4);
        let cancel = CancellationToken::new();
        let link = cancel.child_token();
        let pump = tokio::spawn(pump_peer_events(events_rx, pairing.clone(), cancel, link));

        events.send(PeerEvent::Connected).await.unwrap();
        pairing
            .subscribe()
            .wait_for(|s| *s == PairingState::Connected)
            .await
            .unwrap();
        events.send(PeerEvent::Failed).await.unwrap();
        pairing
            .subscribe()
            .wait_for(|s| *s == PairingState::Failed)
            .await
            .unwrap();

        drop(events);
        pump.await.unwrap();
    }

    #[tokio::test]
    async fn the_event_pump_ignores_events_after_cancellation() {
        let (pairing, _rx) = watch::channel(PairingState::Off);
        let (events, events_rx) = mpsc::channel(4);
        let cancel = CancellationToken::new();
        cancel.cancel();
        let link = cancel.child_token();
        let pump = tokio::spawn(pump_peer_events(events_rx, pairing.clone(), cancel, link));

        events.send(PeerEvent::Connected).await.unwrap();
        pump.await.unwrap();
        assert_eq!(*pairing.borrow(), PairingState::Off);
    }

    #[tokio::test]
    async fn connecting_drops_the_broker_link_and_leaves_the_session_running() {
        let (pairing, _rx) = watch::channel(waiting());
        let (events, events_rx) = mpsc::channel(4);
        let cancel = CancellationToken::new();
        let link = cancel.child_token();
        let pump = tokio::spawn(pump_peer_events(
            events_rx,
            pairing.clone(),
            cancel.clone(),
            link.clone(),
        ));

        events.send(PeerEvent::Connected).await.unwrap();
        pairing
            .subscribe()
            .wait_for(|s| *s == PairingState::Connected)
            .await
            .unwrap();

        assert!(
            link.is_cancelled(),
            "the broker link must not outlive the handshake"
        );
        assert!(!cancel.is_cancelled(), "the session itself keeps running");

        drop(events);
        pump.await.unwrap();
    }

    #[tokio::test]
    async fn cancelling_the_session_still_cancels_the_broker_link() {
        let cancel = CancellationToken::new();
        let link = cancel.child_token();
        cancel.cancel();
        assert!(link.is_cancelled());
    }

    #[test]
    fn a_well_formed_wrapper_names_its_device_and_carries_the_blob() {
        let parsed = parse_meet_wrapper(&wrapper("dev-1", "c2VhbGVk")).expect("wrapper parses");
        assert_eq!(parsed.device, "dev-1");
        assert_eq!(parsed.blob, "c2VhbGVk");
    }

    #[test]
    fn anything_but_this_versions_wrapper_is_dropped() {
        for text in [
            "",
            "not json at all",
            "{",
            "[]",
            "42",
            r#""a bare string""#,
            "c2VhbGVkLWJsb2I=",
            r#"{"v":2,"device":"dev-1","blob":"c2VhbGVk"}"#,
            r#"{"v":0,"device":"dev-1","blob":"c2VhbGVk"}"#,
            r#"{"v":1,"device":"dev-1"}"#,
            r#"{"v":1,"blob":"c2VhbGVk"}"#,
            r#"{"v":1,"device":7,"blob":"c2VhbGVk"}"#,
        ] {
            assert!(
                parse_meet_wrapper(text).is_none(),
                "must not parse as a wrapper: {text}"
            );
        }
    }

    #[test]
    fn a_wrapper_routes_by_whether_its_device_is_the_one_being_served() {
        let key = derive_device_seal_key(&DEVICE_SECRET);
        let candidate = guest_blob(&key, serde_json::json!({ "t": "ice", "c": "{}" }));

        assert_eq!(route_wrapper(None, "dev-1", &candidate), MeetRoute::Open);
        assert_eq!(
            route_wrapper(Some(("dev-1", &key)), "dev-1", &candidate),
            MeetRoute::Forward
        );
        assert_eq!(
            route_wrapper(Some(("dev-1", &key)), "dev-2", &candidate),
            MeetRoute::Ignore
        );
    }

    #[test]
    fn a_fresh_offer_from_the_served_device_is_a_redial_not_signaling() {
        let key = derive_device_seal_key(&DEVICE_SECRET);
        let offer = guest_blob(&key, serde_json::json!({ "t": "offer", "sdp": "v=0" }));

        assert_eq!(
            route_wrapper(Some(("dev-1", &key)), "dev-1", &offer),
            MeetRoute::Redial
        );

        let other = derive_device_seal_key(&[0x11u8; 32]);
        assert_eq!(
            route_wrapper(
                Some(("dev-1", &key)),
                "dev-1",
                &guest_blob(&other, serde_json::json!({ "t": "offer", "sdp": "v=0" }))
            ),
            MeetRoute::Forward
        );
        assert_eq!(
            route_wrapper(Some(("dev-1", &key)), "dev-2", &offer),
            MeetRoute::Ignore
        );
    }

    #[test]
    fn only_a_real_offer_from_the_devices_key_opens_a_session() {
        let key = derive_device_seal_key(&DEVICE_SECRET);
        let other = derive_device_seal_key(&[0x11u8; 32]);

        assert!(opens_as_offer(
            &key,
            &guest_blob(&key, serde_json::json!({ "t": "offer", "sdp": "v=0" }))
        ));

        assert!(!opens_as_offer(
            &key,
            &guest_blob(&other, serde_json::json!({ "t": "offer", "sdp": "v=0" }))
        ));
        assert!(!opens_as_offer(
            &key,
            &guest_blob(&key, serde_json::json!({ "t": "answer", "sdp": "v=0" }))
        ));
        assert!(!opens_as_offer(
            &key,
            &guest_blob(&key, serde_json::json!({ "t": "ice", "c": "{}" }))
        ));
        assert!(!opens_as_offer(&key, &BASE64.encode("not an envelope")));
        assert!(!opens_as_offer(&key, "?not base64?"));
        assert!(!opens_as_offer(&key, ""));
        assert!(!opens_as_offer(
            &key,
            &BASE64.encode(seal(&key, br#"{"t":"offer","sdp":"v=0"}"#, b"host"))
        ));
    }

    #[tokio::test]
    async fn a_device_key_is_found_only_while_the_device_is_paired() {
        let db = test_driver().await;
        psp_db::signal_devices::insert_device(&db, &device_row("dev-1", &DEVICE_SECRET))
            .await
            .unwrap();

        assert_eq!(
            device_seal_key(&db, "dev-1").await,
            Some(derive_device_seal_key(&DEVICE_SECRET))
        );
        assert_eq!(device_seal_key(&db, "dev-2").await, None);

        psp_db::signal_devices::delete_device(&db, "dev-1")
            .await
            .unwrap();
        assert_eq!(device_seal_key(&db, "dev-1").await, None);
    }

    #[tokio::test]
    async fn the_identity_secret_is_minted_once_and_kept() {
        let db = test_driver().await;
        let first = load_or_mint_identity(&db).await.unwrap();
        let second = load_or_mint_identity(&db).await.unwrap();

        assert_eq!(first, second, "the meet room must not move on its own");
        assert_eq!(
            psp_db::meta::get(&db, IDENTITY_SECRET_KEY).await.unwrap(),
            Some(hex32(&first))
        );
    }

    #[tokio::test]
    async fn a_device_session_that_never_connects_is_abandoned() {
        let (pairing, _rx) = watch::channel(PairingState::Off);
        let (_events, events_rx) = mpsc::channel(4);
        let (connected_device, _connected_device_rx) = watch::channel(None);
        let cancel = CancellationToken::new();

        tokio::time::timeout(
            Duration::from_secs(5),
            watch_device_peer(
                events_rx,
                pairing.clone(),
                Duration::from_millis(20),
                "dev-1".to_string(),
                test_driver_arc().await,
                connected_device,
                cancel.clone(),
            ),
        )
        .await
        .expect("the attempt timeout must end a session that never connects");

        assert_eq!(*pairing.borrow(), PairingState::Off);
    }

    #[tokio::test]
    async fn a_connected_device_session_outlives_the_attempt_timeout() {
        let (pairing, _rx) = watch::channel(PairingState::Off);
        let (events, events_rx) = mpsc::channel(4);
        let (connected_device, _connected_device_rx) = watch::channel(None);
        let cancel = CancellationToken::new();
        let watcher = tokio::spawn(watch_device_peer(
            events_rx,
            pairing.clone(),
            Duration::from_millis(20),
            "dev-1".to_string(),
            test_driver_arc().await,
            connected_device,
            cancel,
        ));

        events.send(PeerEvent::Connected).await.unwrap();
        pairing
            .subscribe()
            .wait_for(|state| *state == PairingState::Connected)
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(120)).await;
        assert!(!watcher.is_finished());
        assert_eq!(*pairing.borrow(), PairingState::Connected);

        drop(events);
        tokio::time::timeout(Duration::from_secs(5), watcher)
            .await
            .expect("the session ends when the peer does")
            .unwrap();
        assert_eq!(
            *pairing.borrow(),
            PairingState::Off,
            "an armed desktop with nobody connected is idle"
        );
    }

    #[tokio::test]
    async fn a_cancelled_device_session_stops_reporting_a_connection() {
        let (pairing, _rx) = watch::channel(PairingState::Off);
        let (events, events_rx) = mpsc::channel(4);
        let (connected_device, _connected_device_rx) = watch::channel(None);
        let cancel = CancellationToken::new();
        let watcher = tokio::spawn(watch_device_peer(
            events_rx,
            pairing.clone(),
            Duration::from_secs(60),
            "dev-1".to_string(),
            test_driver_arc().await,
            connected_device,
            cancel.clone(),
        ));

        events.send(PeerEvent::Connected).await.unwrap();
        pairing
            .subscribe()
            .wait_for(|state| *state == PairingState::Connected)
            .await
            .unwrap();

        cancel.cancel();
        tokio::time::timeout(Duration::from_secs(5), watcher)
            .await
            .expect("cancellation ends the session")
            .unwrap();
        assert_eq!(*pairing.borrow(), PairingState::Off);
    }

    #[tokio::test]
    async fn a_device_session_ending_leaves_a_waiting_pairing_alone() {
        let (pairing, _rx) = watch::channel(PairingState::Off);
        let (events, events_rx) = mpsc::channel(4);
        let (connected_device, _connected_device_rx) = watch::channel(None);
        let cancel = CancellationToken::new();
        let watcher = tokio::spawn(watch_device_peer(
            events_rx,
            pairing.clone(),
            Duration::from_secs(60),
            "dev-1".to_string(),
            test_driver_arc().await,
            connected_device,
            cancel,
        ));

        events.send(PeerEvent::Connected).await.unwrap();
        pairing
            .subscribe()
            .wait_for(|state| *state == PairingState::Connected)
            .await
            .unwrap();
        let waiting = waiting();
        pairing.send_replace(waiting.clone());

        drop(events);
        tokio::time::timeout(Duration::from_secs(5), watcher)
            .await
            .expect("the session ends when the peer does")
            .unwrap();
        assert_eq!(*pairing.borrow(), waiting);
    }

    #[test]
    fn a_new_manager_is_not_armed() {
        assert!(!SignalManager::new().armed());
    }

    #[tokio::test]
    async fn stopping_pairing_leaves_a_live_device_sessions_state_alone() {
        let mut manager = SignalManager::new();
        manager
            .connected_device_tx
            .send_replace(Some(ConnectedDevice {
                device_id: "dev-1".to_string(),
                name: "Phone".to_string(),
            }));
        manager.pairing_tx.send_replace(PairingState::Connected);

        manager.stop_pairing().await;

        assert_eq!(manager.pairing(), PairingState::Connected);
        assert!(manager.connected_device().is_some());
    }

    #[tokio::test]
    async fn stopping_pairing_clears_a_pairing_sessions_own_connection() {
        let mut manager = SignalManager::new();
        manager.pairing_tx.send_replace(PairingState::Connected);

        manager.stop_pairing().await;

        assert_eq!(manager.pairing(), PairingState::Off);
    }

    #[tokio::test]
    async fn stopping_pairing_closes_a_window_that_is_still_waiting() {
        let mut manager = SignalManager::new();
        manager.pairing_tx.send_replace(waiting());

        manager.stop_pairing().await;

        assert_eq!(manager.pairing(), PairingState::Off);
    }

    static RELINK_BACKOFF_ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    #[tokio::test]
    async fn meet_relink_backoff_honors_the_env_override() {
        let _lock = RELINK_BACKOFF_ENV_LOCK.lock().await;
        let previous = std::env::var_os(MEET_RELINK_BACKOFF_ENV);

        std::env::set_var(MEET_RELINK_BACKOFF_ENV, "250");
        assert_eq!(meet_relink_backoff(), Duration::from_millis(250));

        std::env::remove_var(MEET_RELINK_BACKOFF_ENV);
        assert_eq!(meet_relink_backoff(), MEET_RELINK_BACKOFF);

        match previous {
            Some(value) => std::env::set_var(MEET_RELINK_BACKOFF_ENV, value),
            None => std::env::remove_var(MEET_RELINK_BACKOFF_ENV),
        }
    }

    #[tokio::test]
    async fn credential_mint_sends_a_credential_and_persists_the_device_on_ack() {
        let driver = test_driver_arc().await;
        let (pairing, _rx) = watch::channel(PairingState::Connected);
        let (ctl_out_tx, mut ctl_out_rx) = mpsc::channel(4);
        let (ctl_in_tx, ctl_in_rx) = mpsc::channel(4);
        let (minted_device_tx, minted_device_rx) = watch::channel(None);
        let cancel = CancellationToken::new();

        let mint = tokio::spawn(run_credential_mint(
            driver.clone(),
            pairing.subscribe(),
            ctl_out_tx,
            ctl_in_rx,
            Duration::from_secs(5),
            minted_device_tx.clone(),
            cancel,
        ));

        let envelope: serde_json::Value = serde_json::from_str(
            &tokio::time::timeout(Duration::from_secs(2), ctl_out_rx.recv())
                .await
                .expect("a credential is sent once connected")
                .expect("the mint task is still running"),
        )
        .unwrap();
        assert_eq!(envelope["type"], "device_credential");
        let device_id = envelope["data"]["deviceId"]
            .as_str()
            .expect("deviceId is a string")
            .to_string();
        let device_secret = envelope["data"]["deviceSecret"]
            .as_str()
            .expect("deviceSecret is a string")
            .to_string();
        assert_eq!(device_id.len(), 32, "16 bytes, hex");
        assert!(parse_hex32(&device_secret).is_some());
        assert!(envelope["data"]["meetRoom"].as_str().is_some());
        assert!(envelope["data"]["desktopName"].as_str().is_some());

        ctl_in_tx
            .send(serde_json::json!({
                "type": "device_credential",
                "data": { "name": "  My Phone  " }
            }))
            .await
            .unwrap();

        tokio::time::timeout(Duration::from_secs(5), mint)
            .await
            .expect("the mint task ends once the ack is handled")
            .unwrap();

        let devices = psp_db::signal_devices::list_devices(&*driver)
            .await
            .unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].device_id, device_id);
        assert_eq!(devices[0].secret_hex, device_secret);
        assert_eq!(devices[0].name, "My Phone", "the name is trimmed");
        assert_eq!(
            *minted_device_rx.borrow(),
            Some(device_id),
            "the session has to know which device it is serving, or a revocation cannot reach it"
        );
    }

    #[tokio::test]
    async fn credential_mint_defaults_an_all_whitespace_ack_name() {
        let driver = test_driver_arc().await;
        let (pairing, _rx) = watch::channel(PairingState::Connected);
        let (ctl_out_tx, mut ctl_out_rx) = mpsc::channel(4);
        let (ctl_in_tx, ctl_in_rx) = mpsc::channel(4);
        let (minted_device_tx, _minted_device_rx) = watch::channel(None);
        let cancel = CancellationToken::new();

        let mint = tokio::spawn(run_credential_mint(
            driver.clone(),
            pairing.subscribe(),
            ctl_out_tx,
            ctl_in_rx,
            Duration::from_secs(5),
            minted_device_tx.clone(),
            cancel,
        ));
        ctl_out_rx.recv().await.expect("a credential is sent");
        ctl_in_tx
            .send(serde_json::json!({ "type": "device_credential", "data": { "name": "   " } }))
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), mint)
            .await
            .unwrap()
            .unwrap();

        let devices = psp_db::signal_devices::list_devices(&*driver)
            .await
            .unwrap();
        assert_eq!(devices[0].name, "Paired device");
    }

    #[tokio::test]
    async fn credential_mint_persists_nothing_without_an_ack() {
        let driver = test_driver_arc().await;
        let (pairing, _rx) = watch::channel(PairingState::Connected);
        let (ctl_out_tx, mut ctl_out_rx) = mpsc::channel(4);
        let (_ctl_in_tx, ctl_in_rx) = mpsc::channel(4);
        let (minted_device_tx, _minted_device_rx) = watch::channel(None);
        let cancel = CancellationToken::new();

        let mint = tokio::spawn(run_credential_mint(
            driver.clone(),
            pairing.subscribe(),
            ctl_out_tx,
            ctl_in_rx,
            Duration::from_millis(50),
            minted_device_tx.clone(),
            cancel,
        ));
        ctl_out_rx.recv().await.expect("a credential is sent");

        tokio::time::timeout(Duration::from_secs(5), mint)
            .await
            .expect("the mint task gives up once the ack window closes")
            .unwrap();

        assert!(psp_db::signal_devices::list_devices(&*driver)
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn credential_mint_does_nothing_if_cancelled_before_the_session_connects() {
        let driver = test_driver_arc().await;
        let (pairing, _rx) = watch::channel(waiting());
        let (ctl_out_tx, mut ctl_out_rx) = mpsc::channel(4);
        let (_ctl_in_tx, ctl_in_rx) = mpsc::channel(4);
        let (minted_device_tx, _minted_device_rx) = watch::channel(None);
        let cancel = CancellationToken::new();
        cancel.cancel();

        tokio::time::timeout(
            Duration::from_secs(5),
            run_credential_mint(
                driver.clone(),
                pairing.subscribe(),
                ctl_out_tx,
                ctl_in_rx,
                Duration::from_secs(30),
                minted_device_tx.clone(),
                cancel,
            ),
        )
        .await
        .expect("a cancelled mint returns promptly");

        assert!(ctl_out_rx.try_recv().is_err(), "nothing was ever sent");
        assert!(psp_db::signal_devices::list_devices(&*driver)
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn credential_mint_persists_nothing_if_cancelled_while_awaiting_the_ack() {
        let driver = test_driver_arc().await;
        let (pairing, _rx) = watch::channel(PairingState::Connected);
        let (ctl_out_tx, mut ctl_out_rx) = mpsc::channel(4);
        let (_ctl_in_tx, ctl_in_rx) = mpsc::channel(4);
        let (minted_device_tx, _minted_device_rx) = watch::channel(None);
        let cancel = CancellationToken::new();

        let mint = tokio::spawn(run_credential_mint(
            driver.clone(),
            pairing.subscribe(),
            ctl_out_tx,
            ctl_in_rx,
            Duration::from_secs(30),
            minted_device_tx.clone(),
            cancel.clone(),
        ));
        ctl_out_rx.recv().await.expect("a credential is sent");
        cancel.cancel();

        tokio::time::timeout(Duration::from_secs(5), mint)
            .await
            .expect("a cancelled mint returns promptly")
            .unwrap();

        assert!(psp_db::signal_devices::list_devices(&*driver)
            .await
            .unwrap()
            .is_empty());
    }

    static DEVICE_ACK_TIMEOUT_ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    #[tokio::test]
    async fn device_credential_ack_timeout_honors_the_env_override() {
        let _lock = DEVICE_ACK_TIMEOUT_ENV_LOCK.lock().await;
        let previous = std::env::var_os(DEVICE_CREDENTIAL_ACK_TIMEOUT_ENV);

        std::env::set_var(DEVICE_CREDENTIAL_ACK_TIMEOUT_ENV, "150");
        assert_eq!(device_credential_ack_timeout(), Duration::from_millis(150));

        std::env::remove_var(DEVICE_CREDENTIAL_ACK_TIMEOUT_ENV);
        assert_eq!(
            device_credential_ack_timeout(),
            DEVICE_CREDENTIAL_ACK_TIMEOUT
        );

        match previous {
            Some(value) => std::env::set_var(DEVICE_CREDENTIAL_ACK_TIMEOUT_ENV, value),
            None => std::env::remove_var(DEVICE_CREDENTIAL_ACK_TIMEOUT_ENV),
        }
    }

    #[test]
    fn trimmed_or_default_falls_back_only_when_empty_after_trimming() {
        assert_eq!(
            trimmed_or_default("  My Phone  ", "Paired device"),
            "My Phone"
        );
        assert_eq!(trimmed_or_default("   ", "Paired device"), "Paired device");
        assert_eq!(trimmed_or_default("", "Paired device"), "Paired device");
    }

    #[test]
    fn desktop_hostname_is_never_empty() {
        assert!(!desktop_hostname().trim().is_empty());
    }

    #[tokio::test]
    async fn a_connected_device_session_touches_last_seen_and_publishes_the_connected_device() {
        let db = test_driver().await;
        psp_db::signal_devices::insert_device(&db, &device_row("dev-1", &DEVICE_SECRET))
            .await
            .unwrap();
        let driver: Arc<dyn psp_db::DbDriver> = Arc::new(db);
        let (pairing, _rx) = watch::channel(PairingState::Off);
        let (events, events_rx) = mpsc::channel(4);
        let (connected_device, mut connected_device_rx) = watch::channel(None);
        let cancel = CancellationToken::new();
        let before_ms = now_ms();

        let watcher = tokio::spawn(watch_device_peer(
            events_rx,
            pairing.clone(),
            Duration::from_secs(60),
            "dev-1".to_string(),
            driver.clone(),
            connected_device.clone(),
            cancel.clone(),
        ));

        events.send(PeerEvent::Connected).await.unwrap();
        connected_device_rx
            .wait_for(|value| value.is_some())
            .await
            .unwrap();
        assert_eq!(
            *connected_device_rx.borrow(),
            Some(ConnectedDevice {
                device_id: "dev-1".to_string(),
                name: "Phone".to_string(),
            })
        );

        let devices = psp_db::signal_devices::list_devices(&*driver)
            .await
            .unwrap();
        assert!(devices[0].last_seen_ms.expect("touched on connect") >= before_ms);

        drop(events);
        tokio::time::timeout(Duration::from_secs(5), watcher)
            .await
            .expect("the session ends when the peer does")
            .unwrap();
        assert_eq!(
            *connected_device_rx.borrow(),
            None,
            "an ended session is no longer connected"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_reconnect_within_the_pumps_grace_window_leaves_status_untouched() {
        let (pairing, _rx) = watch::channel(PairingState::Connected);
        let (events, events_rx) = mpsc::channel(4);
        let cancel = CancellationToken::new();
        let link = cancel.child_token();
        let pump = tokio::spawn(pump_peer_events(
            events_rx,
            pairing.clone(),
            cancel.clone(),
            link,
        ));

        events.send(PeerEvent::Disconnected).await.unwrap();
        tokio::time::sleep(Duration::from_secs(2)).await;
        assert_eq!(
            *pairing.borrow(),
            PairingState::Connected,
            "still inside the grace"
        );

        events.send(PeerEvent::Connected).await.unwrap();
        tokio::time::sleep(STATUS_BLIP_GRACE + Duration::from_secs(1)).await;
        assert_eq!(
            *pairing.borrow(),
            PairingState::Connected,
            "the reconnect must have cancelled the pending flip"
        );

        drop(events);
        pump.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn a_disconnect_with_no_reconnect_flips_the_pumps_status_off_after_the_grace() {
        let (pairing, _rx) = watch::channel(PairingState::Connected);
        let (events, events_rx) = mpsc::channel(4);
        let cancel = CancellationToken::new();
        let link = cancel.child_token();
        let pump = tokio::spawn(pump_peer_events(
            events_rx,
            pairing.clone(),
            cancel.clone(),
            link,
        ));

        events.send(PeerEvent::Disconnected).await.unwrap();
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert_eq!(
            *pairing.borrow(),
            PairingState::Connected,
            "still inside the grace"
        );

        tokio::time::sleep(STATUS_BLIP_GRACE + Duration::from_millis(1)).await;
        assert_eq!(*pairing.borrow(), PairingState::Off);

        drop(events);
        pump.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn a_peer_that_truly_ends_skips_the_pumps_grace() {
        let (pairing, _rx) = watch::channel(PairingState::Connected);
        let (events, events_rx) = mpsc::channel(4);
        let cancel = CancellationToken::new();
        let link = cancel.child_token();
        let pump = tokio::spawn(pump_peer_events(events_rx, pairing.clone(), cancel, link));

        events.send(PeerEvent::Disconnected).await.unwrap();
        drop(events);

        tokio::time::timeout(Duration::from_secs(1), pump)
            .await
            .expect("the post-exit path must not wait out the grace")
            .unwrap();
        assert_eq!(*pairing.borrow(), PairingState::Off);
    }

    #[tokio::test]
    async fn a_device_sessions_reconnect_within_the_grace_window_stays_connected() {
        let (pairing, _rx) = watch::channel(PairingState::Off);
        let (events, events_rx) = mpsc::channel(4);
        let (connected_device, _connected_device_rx) = watch::channel(None);
        let cancel = CancellationToken::new();
        let watcher = tokio::spawn(watch_device_peer(
            events_rx,
            pairing.clone(),
            Duration::from_secs(60),
            "dev-1".to_string(),
            test_driver_arc().await,
            connected_device,
            cancel.clone(),
        ));

        events.send(PeerEvent::Connected).await.unwrap();
        pairing
            .subscribe()
            .wait_for(|state| *state == PairingState::Connected)
            .await
            .unwrap();

        events.send(PeerEvent::Disconnected).await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(
            *pairing.borrow(),
            PairingState::Connected,
            "still inside the grace"
        );
        assert!(!watcher.is_finished());

        events.send(PeerEvent::Connected).await.unwrap();
        tokio::time::sleep(STATUS_BLIP_GRACE + Duration::from_millis(500)).await;
        assert_eq!(
            *pairing.borrow(),
            PairingState::Connected,
            "the reconnect must have cancelled the pending end"
        );
        assert!(!watcher.is_finished());

        drop(events);
        tokio::time::timeout(Duration::from_secs(5), watcher)
            .await
            .expect("the session ends when the peer does")
            .unwrap();
        assert_eq!(*pairing.borrow(), PairingState::Off);
    }

    #[tokio::test]
    async fn a_device_sessions_disconnect_ends_the_session_after_the_grace() {
        let (pairing, _rx) = watch::channel(PairingState::Off);
        let (events, events_rx) = mpsc::channel(4);
        let (connected_device, _connected_device_rx) = watch::channel(None);
        let cancel = CancellationToken::new();
        let watcher = tokio::spawn(watch_device_peer(
            events_rx,
            pairing.clone(),
            Duration::from_secs(60),
            "dev-1".to_string(),
            test_driver_arc().await,
            connected_device,
            cancel,
        ));

        events.send(PeerEvent::Connected).await.unwrap();
        pairing
            .subscribe()
            .wait_for(|state| *state == PairingState::Connected)
            .await
            .unwrap();

        events.send(PeerEvent::Disconnected).await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(!watcher.is_finished(), "still inside the grace");

        tokio::time::timeout(Duration::from_secs(10), watcher)
            .await
            .expect("the grace must eventually end the session")
            .unwrap();
        assert_eq!(*pairing.borrow(), PairingState::Off);
    }

    #[tokio::test]
    async fn a_device_sessions_peer_that_truly_ends_skips_the_grace() {
        let (pairing, _rx) = watch::channel(PairingState::Off);
        let (events, events_rx) = mpsc::channel(4);
        let (connected_device, _connected_device_rx) = watch::channel(None);
        let cancel = CancellationToken::new();
        let watcher = tokio::spawn(watch_device_peer(
            events_rx,
            pairing.clone(),
            Duration::from_secs(60),
            "dev-1".to_string(),
            test_driver_arc().await,
            connected_device,
            cancel,
        ));

        events.send(PeerEvent::Connected).await.unwrap();
        pairing
            .subscribe()
            .wait_for(|state| *state == PairingState::Connected)
            .await
            .unwrap();

        events.send(PeerEvent::Disconnected).await.unwrap();
        drop(events);

        tokio::time::timeout(Duration::from_secs(5), watcher)
            .await
            .expect("the post-exit path must not wait out the grace")
            .unwrap();
        assert_eq!(*pairing.borrow(), PairingState::Off);
    }

    #[tokio::test]
    async fn pairing_url_honors_the_base_env_override_and_trims_trailing_slashes() {
        let _env_guard =
            SignalEnvGuard::acquire(&[(PAIRING_URL_BASE_ENV, Some("https://example.test/"))])
                .await;
        let code = generate_pairing_code();

        assert_eq!(
            pairing_url(&code),
            format!("https://example.test{PAIRING_URL_PATH}{code}")
        );

        std::env::remove_var(PAIRING_URL_BASE_ENV);
        assert_eq!(
            pairing_url(&code),
            format!("{PAIRING_URL_BASE_DEFAULT}{PAIRING_URL_PATH}{code}")
        );
    }

    #[tokio::test(start_paused = true)]
    async fn cancelling_a_pump_with_a_pending_grace_leaves_a_live_devices_status_alone() {
        let (pairing, _rx) = watch::channel(PairingState::Connected);
        let (events, events_rx) = mpsc::channel(4);
        let cancel = CancellationToken::new();
        let link = cancel.child_token();
        let pump = tokio::spawn(pump_peer_events(
            events_rx,
            pairing.clone(),
            cancel.clone(),
            link,
        ));

        events.send(PeerEvent::Disconnected).await.unwrap();
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert_eq!(
            *pairing.borrow(),
            PairingState::Connected,
            "still inside the grace"
        );

        cancel.cancel();
        tokio::time::timeout(Duration::from_secs(1), pump)
            .await
            .expect("a cancelled pump must not linger")
            .unwrap();

        tokio::time::sleep(STATUS_BLIP_GRACE * 2).await;
        assert_eq!(
            *pairing.borrow(),
            PairingState::Connected,
            "a cancelled pump must never clobber a live device's status"
        );
    }
}
