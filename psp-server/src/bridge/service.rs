use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rand::Rng;
use serde_json::Value;
use tokio::sync::{mpsc, oneshot, watch};
use tokio_util::sync::CancellationToken;

use super::client::{self, Command, ConnectError, Connected, ConnectionEnd};
use super::endpoint::{self, DiscoveredEndpoint};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(6);
const DIAL_TIMEOUT: Duration = Duration::from_secs(5);
const DISCOVERY_INTERVAL: Duration = Duration::from_secs(2);
const MIN_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(60);
const COMMAND_CHANNEL_CAPACITY: usize = 32;

#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeStatus {
    pub connected: bool,
    pub endpoint_present: bool,
    pub port: Option<u16>,
    pub mod_version: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("bridge offline")]
    Offline,
    #[error("bridge request timed out")]
    Timeout,
    #[error("{code}: {message}")]
    Mod { code: String, message: String },
    #[error("bridge transport error")]
    Transport,
}

struct Backoff {
    current: Duration,
}

impl Backoff {
    fn new() -> Self {
        Self { current: MIN_BACKOFF }
    }

    fn on_success(&mut self) {
        self.current = MIN_BACKOFF;
    }

    fn next_delay(&mut self) -> Duration {
        let half = self.current / 2;
        let delay = half + rand::rng().random_range(Duration::ZERO..=half);
        self.current = (self.current * 2).min(MAX_BACKOFF);
        delay
    }
}

pub struct BridgeService {
    status_tx: watch::Sender<BridgeStatus>,
    status_rx: watch::Receiver<BridgeStatus>,
    command_tx: mpsc::Sender<Command>,
    command_rx: Mutex<Option<mpsc::Receiver<Command>>>,
    cancel: CancellationToken,
    next_request_id: AtomicU64,
    started: AtomicBool,
    supervisor: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl BridgeService {
    pub fn new() -> Arc<Self> {
        let (status_tx, status_rx) = watch::channel(BridgeStatus::default());
        let (command_tx, command_rx) = mpsc::channel(COMMAND_CHANNEL_CAPACITY);
        Arc::new(Self {
            status_tx,
            status_rx,
            command_tx,
            command_rx: Mutex::new(Some(command_rx)),
            cancel: CancellationToken::new(),
            next_request_id: AtomicU64::new(1),
            started: AtomicBool::new(false),
            supervisor: Mutex::new(None),
        })
    }

    pub fn start(self: &Arc<Self>) {
        if self.started.swap(true, Ordering::SeqCst) {
            return;
        }
        let command_rx = self
            .command_rx
            .lock()
            .unwrap()
            .take()
            .expect("command_rx already taken; start() raced with itself");
        let handle = tokio::spawn(run_supervisor(
            self.status_tx.clone(),
            command_rx,
            self.cancel.clone(),
        ));
        *self.supervisor.lock().unwrap() = Some(handle);
    }

    pub fn status_rx(&self) -> watch::Receiver<BridgeStatus> {
        self.status_rx.clone()
    }

    pub async fn request(&self, kind: &str, data: Value) -> Result<Value, BridgeError> {
        if !self.started.load(Ordering::SeqCst) {
            return Err(BridgeError::Offline);
        }
        let id = format!("r{}", self.next_request_id.fetch_add(1, Ordering::Relaxed));
        let (reply_tx, reply_rx) = oneshot::channel();
        self.command_tx
            .send(Command::Request {
                id: id.clone(),
                kind: kind.to_string(),
                data,
                reply: reply_tx,
            })
            .await
            .map_err(|_| BridgeError::Offline)?;

        match tokio::time::timeout(REQUEST_TIMEOUT, reply_rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(BridgeError::Offline),
            Err(_) => {
                let _ = self.command_tx.send(Command::CancelRequest { id }).await;
                Err(BridgeError::Timeout)
            }
        }
    }

    pub async fn command(&self, op: &str, command_id: &str, args: Value) -> Result<Value, BridgeError> {
        self.request(
            "command",
            serde_json::json!({
                "commandId": command_id, "op": op, "args": args,
            }),
        )
        .await
    }

    pub async fn get_capabilities(&self) -> Result<Value, BridgeError> {
        self.request("get_capabilities", serde_json::json!({})).await
    }

    pub async fn shutdown(&self) {
        self.cancel.cancel();
        let handle = self.supervisor.lock().unwrap().take();
        if let Some(handle) = handle {
            let _ = handle.await;
        }
    }
}

fn publish(
    status_tx: &watch::Sender<BridgeStatus>,
    connected: bool,
    endpoint_present: bool,
    port: Option<u16>,
    mod_version: Option<String>,
    last_error: Option<String>,
) {
    let _ = status_tx.send(BridgeStatus {
        connected,
        endpoint_present,
        port,
        mod_version,
        last_error,
    });
}

async fn wait(
    duration: Duration,
    cancel: &CancellationToken,
    command_rx: &mut mpsc::Receiver<Command>,
) -> bool {
    let sleep = tokio::time::sleep(duration);
    tokio::pin!(sleep);
    loop {
        tokio::select! {
            _ = cancel.cancelled() => return true,
            _ = &mut sleep => return false,
            Some(command) = command_rx.recv() => command.fail_offline(),
        }
    }
}

async fn dial_and_drain(
    endpoint: &DiscoveredEndpoint,
    cancel: &CancellationToken,
    command_rx: &mut mpsc::Receiver<Command>,
) -> Result<Connected, ConnectError> {
    let dial = tokio::time::timeout(DIAL_TIMEOUT, client::connect_and_handshake(endpoint, cancel));
    tokio::pin!(dial);
    loop {
        tokio::select! {
            outcome = &mut dial => {
                return outcome.unwrap_or(Err(ConnectError::Transport));
            }
            Some(command) = command_rx.recv() => command.fail_offline(),
        }
    }
}

async fn run_supervisor(
    status_tx: watch::Sender<BridgeStatus>,
    mut command_rx: mpsc::Receiver<Command>,
    cancel: CancellationToken,
) {
    let mut backoff = Backoff::new();

    loop {
        if cancel.is_cancelled() {
            break;
        }

        let discovered = endpoint::default_endpoint_dir()
            .map(|dir| endpoint::scan_endpoints(&dir, &endpoint::sysinfo_liveness))
            .unwrap_or_default();
        let present = !discovered.is_empty();
        let Some(discovered) = discovered.into_iter().next() else {
            publish(&status_tx, false, present, None, None, None);
            if wait(DISCOVERY_INTERVAL, &cancel, &mut command_rx).await {
                break;
            }
            continue;
        };

        match dial_and_drain(&discovered, &cancel, &mut command_rx).await {
            Ok(connected) => {
                backoff.on_success();
                publish(
                    &status_tx,
                    true,
                    true,
                    Some(discovered.port),
                    Some(connected.mod_version),
                    None,
                );
                let end = client::pump(connected.write, connected.read, &cancel, &mut command_rx)
                    .await;
                publish(&status_tx, false, true, None, None, None);
                if matches!(end, ConnectionEnd::Cancelled) {
                    break;
                }
                if wait(DISCOVERY_INTERVAL, &cancel, &mut command_rx).await {
                    break;
                }
            }
            Err(ConnectError::Cancelled) => break,
            Err(ConnectError::Transport) => {
                publish(
                    &status_tx,
                    false,
                    true,
                    None,
                    None,
                    Some("bridge transport error".to_string()),
                );
                if wait(backoff.next_delay(), &cancel, &mut command_rx).await {
                    break;
                }
            }
            Err(ConnectError::Auth { code }) => {
                publish(&status_tx, false, true, None, None, Some(code));
                if wait(backoff.next_delay(), &cancel, &mut command_rx).await {
                    break;
                }
            }
        }
    }

    while let Ok(command) = command_rx.try_recv() {
        command.fail_offline();
    }
}
