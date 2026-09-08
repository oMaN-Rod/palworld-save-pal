use std::net::SocketAddr;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use psp_app::live::{LiveSourceKind, SignalSourceStatus, SourceHealth};
use psp_server::signal::rest_source::{run_rest_source, RestSourceConfig};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy)]
enum MockMode {
    Ok,
    Unauthorized,
    NotFound,
}

async fn hang_server() -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        if let Ok((socket, _)) = listener.accept().await {
            let _held = socket;
            std::future::pending::<()>().await;
        }
    });
    addr
}

async fn mock_server(mode: MockMode) -> SocketAddr {
    const FIXTURE: &str = include_str!("../../psp-app/tests/fixtures/live/world_snapshot.json");
    let handler = move || async move {
        match mode {
            MockMode::Ok => {
                let mut doc: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
                doc["Time"] = serde_json::Value::String(
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                );
                Json(doc).into_response()
            }
            MockMode::Unauthorized => StatusCode::UNAUTHORIZED.into_response(),
            MockMode::NotFound => StatusCode::NOT_FOUND.into_response(),
        }
    };
    let router = Router::new().route("/v1/api/game-data", get(handler));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    addr
}

fn seq() -> Arc<AtomicU64> {
    Arc::new(AtomicU64::new(0))
}

fn config_for(addr: SocketAddr) -> RestSourceConfig {
    RestSourceConfig {
        host: "127.0.0.1".to_string(),
        port: addr.port(),
        admin_password: "hunter2".to_string(),
        endpoint_base: "v1/api".to_string(),
    }
}

#[tokio::test]
async fn publishes_frames_from_rest() {
    let addr = mock_server(MockMode::Ok).await;
    let (bus, mut bus_rx) = tokio::sync::watch::channel(None);
    let (status, _status_rx) = tokio::sync::watch::channel(SignalSourceStatus::default());
    let cancel = CancellationToken::new();
    let task = tokio::spawn(run_rest_source(
        config_for(addr),
        bus,
        status,
        seq(),
        cancel.clone(),
    ));

    tokio::time::timeout(Duration::from_secs(3), bus_rx.changed())
        .await
        .unwrap()
        .unwrap();
    let frame = bus_rx.borrow().clone().unwrap();
    assert_eq!(frame.source, LiveSourceKind::Rest);

    cancel.cancel();
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn cancelling_mid_probe_stops_the_task_promptly() {
    let addr = hang_server().await;
    let (bus, _bus_rx) = tokio::sync::watch::channel(None);
    let (status, _status_rx) = tokio::sync::watch::channel(SignalSourceStatus::default());
    let cancel = CancellationToken::new();
    let task = tokio::spawn(run_rest_source(
        config_for(addr),
        bus,
        status,
        seq(),
        cancel.clone(),
    ));

    tokio::time::sleep(Duration::from_millis(200)).await;

    let started = tokio::time::Instant::now();
    cancel.cancel();
    tokio::time::timeout(Duration::from_secs(1), task)
        .await
        .expect("the task must not wait out the in-flight probe")
        .unwrap();
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(1),
        "cancellation took {elapsed:?}, expected well under 1s despite a hung connection"
    );
}

#[tokio::test]
async fn wrong_password_reports_auth_state_without_leaking_it() {
    let addr = mock_server(MockMode::Unauthorized).await;
    let (bus, _bus_rx) = tokio::sync::watch::channel(None);
    let (status, mut status_rx) = tokio::sync::watch::channel(SignalSourceStatus::default());
    let cancel = CancellationToken::new();
    let task = tokio::spawn(run_rest_source(
        config_for(addr),
        bus,
        status,
        seq(),
        cancel.clone(),
    ));

    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            status_rx.changed().await.unwrap();
            if status_rx.borrow().health == SourceHealth::Auth {
                break;
            }
        }
    })
    .await
    .unwrap();

    let snapshot = status_rx.borrow().clone();
    let error = snapshot
        .error
        .expect("auth failure must carry an error message");
    assert!(!error.contains("hunter2"));

    cancel.cancel();
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn missing_endpoint_reports_down_with_guidance() {
    let addr = mock_server(MockMode::NotFound).await;
    let (bus, _bus_rx) = tokio::sync::watch::channel(None);
    let (status, mut status_rx) = tokio::sync::watch::channel(SignalSourceStatus::default());
    let cancel = CancellationToken::new();
    let task = tokio::spawn(run_rest_source(
        config_for(addr),
        bus,
        status,
        seq(),
        cancel.clone(),
    ));

    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            status_rx.changed().await.unwrap();
            if status_rx.borrow().health == SourceHealth::Down {
                break;
            }
        }
    })
    .await
    .unwrap();

    let snapshot = status_rx.borrow().clone();
    let error = snapshot
        .error
        .expect("missing endpoint must carry an error message");
    assert!(error.contains("launch argument"));

    cancel.cancel();
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
}
