use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{CloseFrame, Message as AxumMessage, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use psp_server::signal::broker_client::{run_host_broker_link, BrokerConfig, RoomKind};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
struct GuestSide {
    from_host_tx: mpsc::Sender<String>,
    to_host_tx: broadcast::Sender<String>,
}

#[derive(Clone)]
struct MockState {
    guest: GuestSide,
    cancel: CancellationToken,
    close_with: Option<u16>,
    connections: Arc<AtomicUsize>,
}

async fn ws_handler(upgrade: WebSocketUpgrade, State(state): State<MockState>) -> Response {
    upgrade.on_upgrade(move |socket| connection_loop(socket, state))
}

async fn connection_loop(mut socket: WebSocket, state: MockState) {
    state.connections.fetch_add(1, Ordering::Relaxed);
    if let Some(code) = state.close_with {
        socket
            .send(AxumMessage::Close(Some(CloseFrame {
                code,
                reason: "pairing window expired".into(),
            })))
            .await
            .ok();
        return;
    }
    let mut to_host_rx = state.guest.to_host_tx.subscribe();
    loop {
        tokio::select! {
            _ = state.cancel.cancelled() => break,
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(AxumMessage::Text(text))) => {
                        let _ = state.guest.from_host_tx.send(text.as_str().to_string()).await;
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
}

struct MockBroker {
    cancel: CancellationToken,
    serve_task: tokio::task::JoinHandle<()>,
    connections: Arc<AtomicUsize>,
}

impl MockBroker {
    async fn start(addr: SocketAddr, guest: GuestSide) -> Self {
        Self::start_inner(addr, guest, None).await
    }

    async fn start_closing_with(addr: SocketAddr, guest: GuestSide, code: u16) -> Self {
        Self::start_inner(addr, guest, Some(code)).await
    }

    async fn start_inner(addr: SocketAddr, guest: GuestSide, close_with: Option<u16>) -> Self {
        let listener = bind_retrying(addr).await;
        let cancel = CancellationToken::new();
        let connections = Arc::new(AtomicUsize::new(0));
        let state = MockState {
            guest,
            cancel: cancel.clone(),
            close_with,
            connections: connections.clone(),
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
        Self {
            cancel,
            serve_task,
            connections,
        }
    }

    fn connection_count(&self) -> usize {
        self.connections.load(Ordering::Relaxed)
    }

    async fn kill(self) {
        self.cancel.cancel();
        tokio::time::timeout(Duration::from_secs(2), self.serve_task)
            .await
            .expect("mock broker did not shut down within 2s")
            .unwrap();
    }
}

async fn bind_retrying(addr: SocketAddr) -> TcpListener {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        match TcpListener::bind(addr).await {
            Ok(listener) => return listener,
            Err(err) if tokio::time::Instant::now() < deadline => {
                tracing::debug!(%err, "bind_retrying: retrying");
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(err) => panic!("failed to bind {addr}: {err}"),
        }
    }
}

async fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    listener.local_addr().unwrap().port()
}

#[tokio::test]
async fn frames_flow_both_ways_and_resume_after_the_broker_restarts() {
    let (from_host_tx, mut from_host_rx) = mpsc::channel::<String>(16);
    let (to_host_tx, _) = broadcast::channel::<String>(16);
    let guest = GuestSide {
        from_host_tx,
        to_host_tx: to_host_tx.clone(),
    };

    let port = free_port().await;
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let broker = MockBroker::start(addr, guest.clone()).await;

    let (to_guest_tx, to_guest_rx) = mpsc::channel::<String>(16);
    let (from_guest_tx, mut from_guest_rx) = mpsc::channel::<String>(16);
    let cancel = CancellationToken::new();

    let cfg = BrokerConfig {
        base_ws_url: format!("ws://{addr}"),
    };
    let client_task = tokio::spawn(run_host_broker_link(
        cfg,
        "0123456789abcdef0123456789abcdef".to_string(),
        RoomKind::Pair,
        to_guest_rx,
        from_guest_tx,
        cancel.clone(),
    ));

    to_guest_tx.send("hello-guest".to_string()).await.unwrap();
    let received = tokio::time::timeout(Duration::from_secs(2), from_host_rx.recv())
        .await
        .expect("timed out waiting for the host's frame at the broker")
        .expect("from_host channel closed");
    assert_eq!(received, "hello-guest");

    to_host_tx.send("hello-host".to_string()).unwrap();
    let received = tokio::time::timeout(Duration::from_secs(2), from_guest_rx.recv())
        .await
        .expect("timed out waiting for the guest's frame at the host")
        .expect("from_guest channel closed");
    assert_eq!(received, "hello-host");

    broker.kill().await;
    let broker = MockBroker::start(addr, guest.clone()).await;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut delivered = None;
    while tokio::time::Instant::now() < deadline {
        let _ = to_host_tx.send("post-restart".to_string());
        if let Ok(Some(text)) =
            tokio::time::timeout(Duration::from_millis(200), from_guest_rx.recv()).await
        {
            delivered = Some(text);
            break;
        }
    }
    assert_eq!(
        delivered.as_deref(),
        Some("post-restart"),
        "client did not reconnect and resume within 5s of the broker restarting"
    );

    cancel.cancel();
    tokio::time::timeout(Duration::from_secs(2), client_task)
        .await
        .expect("client task did not stop within 2s of cancellation")
        .unwrap();
    broker.kill().await;
}

#[tokio::test]
async fn a_room_expiry_close_ends_the_link_without_reconnecting() {
    let (from_host_tx, _from_host_rx) = mpsc::channel::<String>(16);
    let (to_host_tx, _) = broadcast::channel::<String>(16);
    let guest = GuestSide {
        from_host_tx,
        to_host_tx,
    };

    let port = free_port().await;
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let broker = MockBroker::start_closing_with(addr, guest, 4001).await;

    let (_to_guest_tx, to_guest_rx) = mpsc::channel::<String>(16);
    let (from_guest_tx, _from_guest_rx) = mpsc::channel::<String>(16);
    let cancel = CancellationToken::new();

    let client_task = tokio::spawn(run_host_broker_link(
        BrokerConfig {
            base_ws_url: format!("ws://{addr}"),
        },
        "0123456789abcdef0123456789abcdef".to_string(),
        RoomKind::Pair,
        to_guest_rx,
        from_guest_tx,
        cancel.clone(),
    ));

    tokio::time::timeout(Duration::from_secs(3), client_task)
        .await
        .expect("run_host_broker_link kept reconnecting after the room expired")
        .unwrap();

    assert!(!cancel.is_cancelled(), "the link ended on its own");
    assert_eq!(
        broker.connection_count(),
        1,
        "the expired room must be dialed exactly once"
    );
    broker.kill().await;
}

#[tokio::test]
async fn cancelling_mid_connect_returns_promptly_against_a_black_holed_socket() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let accept_task = tokio::spawn(async move {
        let _held = listener.accept().await.unwrap();
        std::future::pending::<()>().await;
    });

    let (_to_guest_tx, to_guest_rx) = mpsc::channel::<String>(1);
    let (from_guest_tx, _from_guest_rx) = mpsc::channel::<String>(1);
    let cancel = CancellationToken::new();

    let cfg = BrokerConfig {
        base_ws_url: format!("ws://{addr}"),
    };
    let client_task = tokio::spawn(run_host_broker_link(
        cfg,
        "0123456789abcdef0123456789abcdef".to_string(),
        RoomKind::Pair,
        to_guest_rx,
        from_guest_tx,
        cancel.clone(),
    ));

    tokio::time::sleep(Duration::from_millis(200)).await;
    cancel.cancel();

    tokio::time::timeout(Duration::from_secs(2), client_task)
        .await
        .expect("run_host_broker_link did not return within 2s of cancellation mid-connect")
        .unwrap();

    accept_task.abort();
}
