use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rand::Rng;
use serde_json::Value;
use tokio::sync::{mpsc, oneshot, watch};
use tokio_util::sync::CancellationToken;

use super::client::{self, Command, ConnectError, Connected, ConnectionEnd};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(6);
const DIAL_TIMEOUT: Duration = Duration::from_secs(5);
const DISCOVERY_INTERVAL: Duration = Duration::from_secs(2);
const MIN_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(60);
const COMMAND_CHANNEL_CAPACITY: usize = 32;

#[derive(Debug, Clone, PartialEq)]
pub struct BridgeTarget {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeStatus {
    pub connected: bool,
    pub port: Option<u16>,
    pub mod_version: Option<String>,
    pub last_error: Option<String>,
    pub instance_id: Option<String>,
    pub instance_name: Option<String>,
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
    target_tx: watch::Sender<Option<BridgeTarget>>,
    target_rx: watch::Receiver<Option<BridgeTarget>>,
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
        let (target_tx, target_rx) = watch::channel(None);
        let (command_tx, command_rx) = mpsc::channel(COMMAND_CHANNEL_CAPACITY);
        Arc::new(Self {
            status_tx,
            status_rx,
            target_tx,
            target_rx,
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
            self.target_rx.clone(),
            self.cancel.clone(),
        ));
        *self.supervisor.lock().unwrap() = Some(handle);
    }

    pub fn status_rx(&self) -> watch::Receiver<BridgeStatus> {
        self.status_rx.clone()
    }

    pub fn set_target(&self, target: Option<BridgeTarget>) {
        let _ = self.target_tx.send(target);
    }

    pub fn target(&self) -> Option<BridgeTarget> {
        self.target_rx.borrow().clone()
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

fn publish(status_tx: &watch::Sender<BridgeStatus>, status: BridgeStatus) {
    let _ = status_tx.send(status);
}

async fn wait(
    duration: Duration,
    cancel: &CancellationToken,
    command_rx: &mut mpsc::Receiver<Command>,
    target_rx: &mut watch::Receiver<Option<BridgeTarget>>,
) -> bool {
    let sleep = tokio::time::sleep(duration);
    tokio::pin!(sleep);
    loop {
        tokio::select! {
            _ = cancel.cancelled() => return true,
            _ = &mut sleep => return false,
            changed = target_rx.changed() => return changed.is_err(),
            Some(command) = command_rx.recv() => command.fail_offline(),
        }
    }
}

async fn dial_and_drain(
    target: &BridgeTarget,
    cancel: &CancellationToken,
    command_rx: &mut mpsc::Receiver<Command>,
) -> Result<Connected, ConnectError> {
    let dial = tokio::time::timeout(DIAL_TIMEOUT, client::connect_and_handshake(target, cancel));
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
    mut target_rx: watch::Receiver<Option<BridgeTarget>>,
    cancel: CancellationToken,
) {
    let mut backoff = Backoff::new();

    loop {
        if cancel.is_cancelled() {
            break;
        }

        let Some(target) = target_rx.borrow().clone() else {
            publish(&status_tx, BridgeStatus::default());
            tokio::select! {
                _ = cancel.cancelled() => break,
                changed = target_rx.changed() => {
                    if changed.is_err() { break; }
                }
                Some(command) = command_rx.recv() => command.fail_offline(),
            }
            continue;
        };

        let identity = |connected: bool, last_error: Option<String>| BridgeStatus {
            connected,
            port: Some(target.port),
            mod_version: None,
            last_error,
            instance_id: Some(target.id.clone()),
            instance_name: Some(target.name.clone()),
        };

        match dial_and_drain(&target, &cancel, &mut command_rx).await {
            Ok(connected) => {
                backoff.on_success();
                let mut status = identity(true, None);
                status.mod_version = Some(connected.mod_version);
                publish(&status_tx, status);

                let end = client::pump(
                    connected.write, connected.read, &cancel, &mut command_rx, &mut target_rx,
                )
                .await;
                publish(&status_tx, identity(false, None));

                match end {
                    ConnectionEnd::Cancelled => break,
                    ConnectionEnd::Retarget => continue,
                    ConnectionEnd::Disconnected => {
                        if wait(DISCOVERY_INTERVAL, &cancel, &mut command_rx, &mut target_rx).await {
                            break;
                        }
                    }
                }
            }
            Err(ConnectError::Cancelled) => break,
            Err(ConnectError::Transport) => {
                publish(&status_tx, identity(false, Some("bridge transport error".to_string())));
                if wait(backoff.next_delay(), &cancel, &mut command_rx, &mut target_rx).await {
                    break;
                }
            }
            Err(ConnectError::Auth { code }) => {
                publish(&status_tx, identity(false, Some(code)));
                if wait(backoff.next_delay(), &cancel, &mut command_rx, &mut target_rx).await {
                    break;
                }
            }
        }
    }

    while let Ok(command) = command_rx.try_recv() {
        command.fail_offline();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(id: &str, port: u16) -> BridgeTarget {
        BridgeTarget {
            id: id.to_string(),
            name: format!("instance {id}"),
            host: "127.0.0.1".to_string(),
            port,
            token: "s3cr3t".to_string(),
        }
    }

    #[tokio::test]
    async fn a_new_service_has_no_target() {
        let service = BridgeService::new();
        assert!(service.target().is_none());
    }

    #[tokio::test]
    async fn set_target_is_readable_back() {
        let service = BridgeService::new();
        service.set_target(Some(target("saved:1", 8788)));
        assert_eq!(service.target().unwrap().id, "saved:1");
        service.set_target(None);
        assert!(service.target().is_none());
    }

    #[tokio::test]
    async fn requests_fail_offline_with_no_target() {
        let service = BridgeService::new();
        service.start();
        let error = service.request("get_status", serde_json::json!({})).await.unwrap_err();
        assert!(matches!(error, BridgeError::Offline));
        service.shutdown().await;
    }

    #[tokio::test]
    async fn status_reports_the_target_identity_while_disconnected() {
        let service = BridgeService::new();
        service.start();
        service.set_target(Some(target("auto:42", 1)));

        let mut status_rx = service.status_rx();
        // Port 1 never accepts, so this settles on disconnected-with-identity.
        let settled = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if status_rx.changed().await.is_err() {
                    return None;
                }
                let status = status_rx.borrow().clone();
                if status.instance_id.as_deref() == Some("auto:42") {
                    return Some(status);
                }
            }
        })
        .await
        .expect("status should settle");

        let status = settled.expect("status channel stayed open");
        assert!(!status.connected);
        assert_eq!(status.instance_name.as_deref(), Some("instance auto:42"));
        service.shutdown().await;
    }

    #[tokio::test]
    async fn a_target_switch_during_backoff_is_acted_on_without_waiting_out_the_backoff() {
        let service = BridgeService::new();
        service.start();
        service.set_target(Some(target("dead:1", 1)));

        let mut status_rx = service.status_rx();
        let mut failed_dials = 0;
        // Port 1 never accepts, so every dial fails and the supervisor backs off. By the
        // 4th failure the backoff wait's floor is MIN_BACKOFF * 2^2 = 4s -- comfortably
        // above the 2.5s window below, so an unfixed `wait()` (one that doesn't race the
        // target change) would still be sleeping when that window elapses.
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                if status_rx.changed().await.is_err() {
                    panic!("status channel closed while backoff was building up");
                }
                let status = status_rx.borrow().clone();
                if status.instance_id.as_deref() == Some("dead:1") && status.last_error.is_some() {
                    failed_dials += 1;
                    if failed_dials >= 4 {
                        return;
                    }
                }
            }
        })
        .await
        .expect("the target should have failed to dial repeatedly by now");

        service.set_target(Some(target("dead:2", 1)));

        tokio::time::timeout(Duration::from_millis(2_500), async {
            loop {
                if status_rx.changed().await.is_err() {
                    panic!("status channel closed while waiting for the switch to take effect");
                }
                if status_rx.borrow().instance_id.as_deref() == Some("dead:2") {
                    return;
                }
            }
        })
        .await
        .expect(
            "a target switch during backoff must be picked up well inside the backoff wait, \
             not after it elapses",
        );

        service.shutdown().await;
    }
}
