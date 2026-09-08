use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::ws::{Message as AxumMessage, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
#[allow(dead_code)]
pub struct MockMod {
    pub token: String,
    pub status: serde_json::Value,
    pub players: serde_json::Value,
    pub connections: Arc<AtomicUsize>,
    pub requests: Arc<Mutex<Vec<serde_json::Value>>>,
    status_delay: Option<Duration>,
    hello_delay: Option<Duration>,
    players_error: Option<(String, String)>,
    players_delay: Option<Duration>,
    pals: Option<serde_json::Value>,
    pal_detail: Option<serde_json::Value>,
    inventory: Option<serde_json::Value>,
    commands: HashMap<String, serde_json::Value>,
    command_errors: HashMap<String, (String, String)>,
    command_errors_at: HashMap<String, (usize, String, String)>,
    capabilities: Option<serde_json::Value>,
    capabilities_push: Option<serde_json::Value>,
}

#[allow(dead_code)]
impl MockMod {
    pub fn new(
        token: impl Into<String>,
        status: serde_json::Value,
        players: serde_json::Value,
    ) -> Self {
        Self {
            token: token.into(),
            status,
            players,
            connections: Arc::new(AtomicUsize::new(0)),
            requests: Arc::new(Mutex::new(Vec::new())),
            status_delay: None,
            hello_delay: None,
            players_error: None,
            players_delay: None,
            pals: None,
            pal_detail: None,
            inventory: None,
            commands: HashMap::new(),
            command_errors: HashMap::new(),
            command_errors_at: HashMap::new(),
            capabilities: None,
            capabilities_push: None,
        }
    }

    pub fn with_pals(mut self, pals: serde_json::Value) -> Self {
        self.pals = Some(pals);
        self
    }

    pub fn with_pal_detail(mut self, pal_detail: serde_json::Value) -> Self {
        self.pal_detail = Some(pal_detail);
        self
    }

    pub fn with_inventory(mut self, inventory: serde_json::Value) -> Self {
        self.inventory = Some(inventory);
        self
    }

    pub fn with_status_delay(mut self, delay: Duration) -> Self {
        self.status_delay = Some(delay);
        self
    }

    pub fn with_hello_delay(mut self, delay: Duration) -> Self {
        self.hello_delay = Some(delay);
        self
    }

    pub fn with_players_error(
        mut self,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        self.players_error = Some((code.into(), message.into()));
        self
    }

    pub fn with_players_delay(mut self, delay: Duration) -> Self {
        self.players_delay = Some(delay);
        self
    }

    pub fn with_command_result(mut self, op: impl Into<String>, result: serde_json::Value) -> Self {
        self.commands.insert(op.into(), result);
        self
    }

    pub fn with_command_error(
        mut self,
        op: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        self.command_errors
            .insert(op.into(), (code.into(), message.into()));
        self
    }

    pub fn with_command_error_at(
        mut self,
        op: impl Into<String>,
        call_index: usize,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        self.command_errors_at
            .insert(op.into(), (call_index, code.into(), message.into()));
        self
    }

    pub fn with_capabilities(mut self, capabilities: serde_json::Value) -> Self {
        self.capabilities = Some(capabilities);
        self
    }

    pub fn with_capabilities_push(mut self, capabilities: serde_json::Value) -> Self {
        self.capabilities_push = Some(capabilities);
        self
    }
}

#[derive(Clone)]
struct MockState {
    mock: MockMod,
    cancel: CancellationToken,
}

async fn ws_handler(upgrade: WebSocketUpgrade, State(state): State<MockState>) -> Response {
    upgrade.on_upgrade(move |socket| connection_loop(socket, state))
}

fn envelope_id(value: &serde_json::Value) -> String {
    value
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string()
}

async fn connection_loop(mut socket: WebSocket, state: MockState) {
    state.mock.connections.fetch_add(1, Ordering::Relaxed);

    if let Some(delay) = state.mock.hello_delay {
        tokio::select! {
            _ = state.cancel.cancelled() => {}
            _ = tokio::time::sleep(delay) => {}
        }
        return;
    }

    const MOCK_NONCE: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

    let Some(Ok(AxumMessage::Text(text))) = socket.recv().await else {
        return;
    };
    let Ok(hello) = serde_json::from_str::<serde_json::Value>(&text) else {
        return;
    };
    let hello_ok = serde_json::json!({
        "id": envelope_id(&hello),
        "type": "hello_ok",
        "data": {
            "mod": "PSPAmity", "protocolVersion": 2, "version": "0.1.0",
            "name": "Mock", "nonce": MOCK_NONCE,
        },
    });
    if socket
        .send(AxumMessage::Text(hello_ok.to_string().into()))
        .await
        .is_err()
    {
        return;
    }

    let Some(Ok(AxumMessage::Text(text))) = socket.recv().await else {
        return;
    };
    let Ok(auth) = serde_json::from_str::<serde_json::Value>(&text) else {
        return;
    };
    let auth_id = envelope_id(&auth);
    let proof = auth
        .get("data")
        .and_then(|data| data.get("proof"))
        .and_then(|proof| proof.as_str())
        .unwrap_or_default();
    if proof != psp_server::bridge::client::compute_proof(&state.mock.token, MOCK_NONCE) {
        let error = serde_json::json!({
            "id": auth_id,
            "type": "error",
            "data": { "code": "unauthorized", "message": "invalid token" },
        });
        let _ = socket
            .send(AxumMessage::Text(error.to_string().into()))
            .await;
        let _ = socket.send(AxumMessage::Close(None)).await;
        return;
    }
    let auth_ok = serde_json::json!({ "id": auth_id, "type": "auth_ok", "data": {} });
    if socket
        .send(AxumMessage::Text(auth_ok.to_string().into()))
        .await
        .is_err()
    {
        return;
    }

    if let Some(capabilities) = &state.mock.capabilities_push {
        let push =
            serde_json::json!({ "id": "push", "type": "capabilities", "data": capabilities });
        if socket
            .send(AxumMessage::Text(push.to_string().into()))
            .await
            .is_err()
        {
            return;
        }
    }

    loop {
        tokio::select! {
            _ = state.cancel.cancelled() => break,
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(AxumMessage::Text(text))) => {
                        let Ok(request) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
                        let id = envelope_id(&request);
                        let kind = request.get("type").and_then(|v| v.as_str()).unwrap_or_default();
                        state.mock.requests.lock().unwrap().push(request.clone());
                        let reply = match kind {
                            "get_status" => {
                                if let Some(delay) = state.mock.status_delay {
                                    tokio::time::sleep(delay).await;
                                }
                                serde_json::json!({ "id": id, "type": "status", "data": state.mock.status })
                            }
                            "get_players" => {
                                if let Some(delay) = state.mock.players_delay {
                                    tokio::time::sleep(delay).await;
                                }
                                match &state.mock.players_error {
                                    Some((code, message)) => serde_json::json!({
                                        "id": id,
                                        "type": "error",
                                        "data": { "code": code, "message": message },
                                    }),
                                    None => serde_json::json!({ "id": id, "type": "players", "data": state.mock.players }),
                                }
                            }
                            "get_pals" => match &state.mock.pals {
                                Some(pals) => serde_json::json!({ "id": id, "type": "pals", "data": pals }),
                                None => serde_json::json!({
                                    "id": id,
                                    "type": "error",
                                    "data": { "code": "capability_unavailable", "message": "get_pals not configured" },
                                }),
                            },
                            "get_pal_detail" => match &state.mock.pal_detail {
                                Some(detail) => serde_json::json!({ "id": id, "type": "pal_detail", "data": detail }),
                                None => serde_json::json!({
                                    "id": id,
                                    "type": "error",
                                    "data": { "code": "capability_unavailable", "message": "get_pal_detail not configured" },
                                }),
                            },
                            "get_inventory" => match &state.mock.inventory {
                                Some(inventory) => serde_json::json!({ "id": id, "type": "inventory", "data": inventory }),
                                None => serde_json::json!({
                                    "id": id,
                                    "type": "error",
                                    "data": { "code": "capability_unavailable", "message": "get_inventory not configured" },
                                }),
                            },
                            "command" => {
                                let op = request
                                    .get("data")
                                    .and_then(|data| data.get("op"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default();
                                let call_index = state
                                    .mock
                                    .requests
                                    .lock()
                                    .unwrap()
                                    .iter()
                                    .filter(|logged| {
                                        logged.get("type").and_then(serde_json::Value::as_str)
                                            == Some("command")
                                            && logged
                                                .get("data")
                                                .and_then(|data| data.get("op"))
                                                .and_then(serde_json::Value::as_str)
                                                == Some(op)
                                    })
                                    .count()
                                    - 1;
                                if let Some((code, message)) = state.mock.command_errors.get(op) {
                                    serde_json::json!({
                                        "id": id,
                                        "type": "error",
                                        "data": { "code": code, "message": message },
                                    })
                                } else if state
                                    .mock
                                    .command_errors_at
                                    .get(op)
                                    .is_some_and(|(fail_at, _, _)| *fail_at == call_index)
                                {
                                    let (_, code, message) = &state.mock.command_errors_at[op];
                                    serde_json::json!({
                                        "id": id,
                                        "type": "error",
                                        "data": { "code": code, "message": message },
                                    })
                                } else if let Some(result) = state.mock.commands.get(op) {
                                    serde_json::json!({ "id": id, "type": "command_result", "data": result })
                                } else {
                                    serde_json::json!({
                                        "id": id,
                                        "type": "error",
                                        "data": { "code": "capability_unavailable", "message": format!("command op {op} not configured") },
                                    })
                                }
                            }
                            "get_capabilities" => match &state.mock.capabilities {
                                Some(capabilities) => {
                                    serde_json::json!({ "id": id, "type": "capabilities", "data": capabilities })
                                }
                                None => serde_json::json!({
                                    "id": id,
                                    "type": "error",
                                    "data": { "code": "capability_unavailable", "message": "get_capabilities not configured" },
                                }),
                            },
                            other => serde_json::json!({
                                "id": id,
                                "type": "error",
                                "data": { "code": "capability_unavailable", "message": format!("unsupported request type {other}") },
                            }),
                        };
                        if socket.send(AxumMessage::Text(reply.to_string().into())).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(AxumMessage::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
        }
    }
}

async fn serve(
    listener: TcpListener,
    mock: MockMod,
) -> (SocketAddr, CancellationToken, tokio::task::JoinHandle<()>) {
    let addr = listener.local_addr().unwrap();
    let cancel = CancellationToken::new();
    let state = MockState {
        mock,
        cancel: cancel.clone(),
    };
    let router = Router::new().route("/", get(ws_handler)).with_state(state);
    let shutdown_cancel = cancel.clone();
    let handle = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async move { shutdown_cancel.cancelled().await })
            .await
            .unwrap();
    });
    (addr, cancel, handle)
}

#[allow(dead_code)]
pub async fn spawn_mock_mod(
    mock: MockMod,
) -> (SocketAddr, CancellationToken, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    serve(listener, mock).await
}

#[allow(dead_code)]
pub async fn spawn_mock_mod_on(
    addr: SocketAddr,
    mock: MockMod,
) -> (SocketAddr, CancellationToken, tokio::task::JoinHandle<()>) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    let listener = loop {
        match TcpListener::bind(addr).await {
            Ok(listener) => break listener,
            Err(_) if tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(err) => panic!("failed to rebind {addr}: {err}"),
        }
    };
    serve(listener, mock).await
}

#[allow(dead_code)]
pub fn write_endpoint_file(
    dir: &std::path::Path,
    port: u16,
    token: &str,
    pid: u32,
) -> std::path::PathBuf {
    let path = dir.join(format!("{pid}.json"));
    let json = serde_json::json!({
        "protocolVersion": 2,
        "port": port,
        "token": token,
        "name": "Mock",
        "bind": "127.0.0.1",
        "pid": pid,
        "startedAt": "2026-09-03T00:00:00Z",
    });
    std::fs::write(&path, json.to_string()).unwrap();
    path
}
