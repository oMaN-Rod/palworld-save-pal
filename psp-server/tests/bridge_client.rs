mod common;

use std::time::Duration;

use common::mock_mod::{spawn_mock_mod, spawn_mock_mod_on, write_endpoint_file, MockMod};
use psp_server::bridge::service::{BridgeError, BridgeService, BridgeStatus};
use tokio_util::sync::CancellationToken;

const STATUS_FIXTURE: &str = include_str!("../../psp-amity/fixtures/status.json");
const PLAYERS_FIXTURE: &str = include_str!("../../psp-amity/fixtures/players.json");
const COMMAND_RESULT_HEAL_FIXTURE: &str =
    include_str!("../../psp-amity/fixtures/command_result_heal.json");
const CAPABILITIES_FIXTURE: &str = include_str!("../../psp-amity/fixtures/capabilities.json");

fn fixture_data(json: &str) -> serde_json::Value {
    serde_json::from_str::<serde_json::Value>(json).unwrap()["data"].clone()
}

static BRIDGE_ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct BridgeEnvGuard {
    _lock: tokio::sync::MutexGuard<'static, ()>,
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl BridgeEnvGuard {
    async fn acquire(vars: &[(&'static str, Option<&str>)]) -> Self {
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

async fn kill_mock(cancel: CancellationToken, handle: tokio::task::JoinHandle<()>) {
    cancel.cancel();
    let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
}

async fn wait_for_status<F: Fn(&BridgeStatus) -> bool>(
    mut rx: tokio::sync::watch::Receiver<BridgeStatus>,
    timeout: Duration,
    what: &str,
    predicate: F,
) -> BridgeStatus {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        {
            let status = rx.borrow();
            if predicate(&status) {
                return status.clone();
            }
        }
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            panic!(
                "timed out after {timeout:?} waiting for status to {what}; last status = {:?}",
                rx.borrow()
            );
        }
        let _ = tokio::time::timeout(remaining, rx.changed()).await;
    }
}

#[tokio::test]
async fn discovery_connects_and_reports_status() {
    let _env = BridgeEnvGuard::acquire(&[("PSP_BRIDGE_ENDPOINT_PATH", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    );
    let (addr, mock_cancel, mock_handle) = spawn_mock_mod(mock).await;
    let endpoint_path = write_endpoint_file(dir.path(), addr.port(), "secret-token", std::process::id());
    std::env::set_var(
        "PSP_BRIDGE_ENDPOINT_PATH",
        endpoint_path.to_str().unwrap(),
    );

    let service = BridgeService::new();
    service.start();

    let status = wait_for_status(service.status_rx(), Duration::from_secs(5), "connect", |s| {
        s.connected
    })
    .await;
    assert!(status.connected);
    assert_eq!(status.mod_version.as_deref(), Some("0.1.0"));

    service.shutdown().await;
    kill_mock(mock_cancel, mock_handle).await;
}

#[tokio::test]
async fn get_status_returns_the_configured_payload() {
    let _env = BridgeEnvGuard::acquire(&[("PSP_BRIDGE_ENDPOINT_PATH", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    );
    let (addr, mock_cancel, mock_handle) = spawn_mock_mod(mock).await;
    let endpoint_path = write_endpoint_file(dir.path(), addr.port(), "secret-token", std::process::id());
    std::env::set_var(
        "PSP_BRIDGE_ENDPOINT_PATH",
        endpoint_path.to_str().unwrap(),
    );

    let service = BridgeService::new();
    service.start();
    wait_for_status(service.status_rx(), Duration::from_secs(5), "connect", |s| {
        s.connected
    })
    .await;

    let payload = service
        .request("get_status", serde_json::json!({}))
        .await
        .expect("get_status should succeed");
    assert_eq!(payload["mode"], "coop_host");
    assert_eq!(payload["authoritative"], true);

    service.shutdown().await;
    kill_mock(mock_cancel, mock_handle).await;
}

#[tokio::test]
async fn get_players_returns_the_configured_payload() {
    let _env = BridgeEnvGuard::acquire(&[("PSP_BRIDGE_ENDPOINT_PATH", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    );
    let (addr, mock_cancel, mock_handle) = spawn_mock_mod(mock).await;
    let endpoint_path = write_endpoint_file(dir.path(), addr.port(), "secret-token", std::process::id());
    std::env::set_var(
        "PSP_BRIDGE_ENDPOINT_PATH",
        endpoint_path.to_str().unwrap(),
    );

    let service = BridgeService::new();
    service.start();
    wait_for_status(service.status_rx(), Duration::from_secs(5), "connect", |s| {
        s.connected
    })
    .await;

    let payload = service
        .request("get_players", serde_json::json!({}))
        .await
        .expect("get_players should succeed");
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["players"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        payload["players"][0]["uid"],
        "00000000-0000-0000-0000-000000000001"
    );

    service.shutdown().await;
    kill_mock(mock_cancel, mock_handle).await;
}

#[tokio::test]
async fn mod_error_passes_through_as_typed_error() {
    let _env = BridgeEnvGuard::acquire(&[("PSP_BRIDGE_ENDPOINT_PATH", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    );
    let (addr, mock_cancel, mock_handle) = spawn_mock_mod(mock).await;
    let endpoint_path = write_endpoint_file(dir.path(), addr.port(), "secret-token", std::process::id());
    std::env::set_var(
        "PSP_BRIDGE_ENDPOINT_PATH",
        endpoint_path.to_str().unwrap(),
    );

    let service = BridgeService::new();
    service.start();
    wait_for_status(service.status_rx(), Duration::from_secs(5), "connect", |s| {
        s.connected
    })
    .await;

    let error = service
        .request("get_guilds", serde_json::json!({}))
        .await
        .expect_err("an unsupported request type should error");
    match error {
        BridgeError::Mod { code, .. } => assert_eq!(code, "capability_unavailable"),
        other => panic!("expected BridgeError::Mod, got {other:?}"),
    }

    service.shutdown().await;
    kill_mock(mock_cancel, mock_handle).await;
}

#[tokio::test]
async fn dead_pid_never_dials_the_endpoint() {
    let _env = BridgeEnvGuard::acquire(&[("PSP_BRIDGE_ENDPOINT_PATH", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    );
    let connections = mock.connections.clone();
    let (addr, mock_cancel, mock_handle) = spawn_mock_mod(mock).await;
    let endpoint_path = write_endpoint_file(dir.path(), addr.port(), "secret-token", 4_294_000_000);
    std::env::set_var(
        "PSP_BRIDGE_ENDPOINT_PATH",
        endpoint_path.to_str().unwrap(),
    );

    let service = BridgeService::new();
    service.start();

    tokio::time::sleep(Duration::from_secs(3)).await;

    let status = service.status_rx().borrow().clone();
    assert!(!status.connected);
    assert!(status.endpoint_present);
    assert_eq!(
        connections.load(std::sync::atomic::Ordering::Relaxed),
        0,
        "a dead pid must never be dialed"
    );

    service.shutdown().await;
    kill_mock(mock_cancel, mock_handle).await;
}

#[tokio::test]
async fn wrong_token_backs_off_and_records_unauthorized() {
    let _env = BridgeEnvGuard::acquire(&[("PSP_BRIDGE_ENDPOINT_PATH", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let mock = MockMod::new(
        "right-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    );
    let connections = mock.connections.clone();
    let (addr, mock_cancel, mock_handle) = spawn_mock_mod(mock).await;
    let endpoint_path = write_endpoint_file(dir.path(), addr.port(), "wrong-token", std::process::id());
    std::env::set_var(
        "PSP_BRIDGE_ENDPOINT_PATH",
        endpoint_path.to_str().unwrap(),
    );

    let service = BridgeService::new();
    service.start();

    let status = wait_for_status(
        service.status_rx(),
        Duration::from_secs(5),
        "record an unauthorized last_error",
        |s| s.last_error.is_some(),
    )
    .await;
    assert!(!status.connected);
    assert!(status
        .last_error
        .as_deref()
        .is_some_and(|e| e.contains("unauthorized")));
    assert!(
        connections.load(std::sync::atomic::Ordering::Relaxed) >= 1,
        "at least one connection attempt should have reached the mock"
    );

    service.shutdown().await;
    kill_mock(mock_cancel, mock_handle).await;
}

#[tokio::test]
async fn reconnects_after_the_mod_restarts_on_the_same_port() {
    let _env = BridgeEnvGuard::acquire(&[("PSP_BRIDGE_ENDPOINT_PATH", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    );
    let (addr, mock_cancel, mock_handle) = spawn_mock_mod(mock.clone()).await;
    let endpoint_path = write_endpoint_file(dir.path(), addr.port(), "secret-token", std::process::id());
    std::env::set_var(
        "PSP_BRIDGE_ENDPOINT_PATH",
        endpoint_path.to_str().unwrap(),
    );

    let service = BridgeService::new();
    service.start();
    wait_for_status(service.status_rx(), Duration::from_secs(5), "connect", |s| {
        s.connected
    })
    .await;

    kill_mock(mock_cancel, mock_handle).await;
    wait_for_status(
        service.status_rx(),
        Duration::from_secs(5),
        "disconnect after the mod dies",
        |s| !s.connected,
    )
    .await;

    let (_addr2, mock_cancel2, mock_handle2) = spawn_mock_mod_on(addr, mock).await;
    write_endpoint_file(dir.path(), addr.port(), "secret-token", std::process::id());

    let status = wait_for_status(
        service.status_rx(),
        Duration::from_secs(5),
        "reconnect",
        |s| s.connected,
    )
    .await;
    assert!(status.connected);

    service.shutdown().await;
    kill_mock(mock_cancel2, mock_handle2).await;
}

#[tokio::test]
async fn shutdown_completes_quickly_with_a_request_in_flight() {
    let _env = BridgeEnvGuard::acquire(&[("PSP_BRIDGE_ENDPOINT_PATH", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    )
    .with_status_delay(Duration::from_secs(10));
    let (addr, mock_cancel, mock_handle) = spawn_mock_mod(mock).await;
    let endpoint_path = write_endpoint_file(dir.path(), addr.port(), "secret-token", std::process::id());
    std::env::set_var(
        "PSP_BRIDGE_ENDPOINT_PATH",
        endpoint_path.to_str().unwrap(),
    );

    let service = BridgeService::new();
    service.start();
    wait_for_status(service.status_rx(), Duration::from_secs(5), "connect", |s| {
        s.connected
    })
    .await;

    let request_service = service.clone();
    let request_task = tokio::spawn(async move {
        request_service
            .request("get_status", serde_json::json!({}))
            .await
    });
    tokio::time::sleep(Duration::from_millis(200)).await;

    let started = tokio::time::Instant::now();
    service.shutdown().await;
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(2),
        "shutdown took {elapsed:?}, expected under 2s even with a slow reply in flight"
    );

    let result = tokio::time::timeout(Duration::from_secs(2), request_task)
        .await
        .expect("the in-flight request task did not finish")
        .expect("request task panicked");
    match result {
        Err(BridgeError::Offline) => {}
        other => panic!("expected Err(BridgeError::Offline), got {other:?}"),
    }

    kill_mock(mock_cancel, mock_handle).await;
}

#[tokio::test]
async fn a_stalled_dial_fails_pending_requests_offline_and_then_records_a_transport_error() {
    let _env = BridgeEnvGuard::acquire(&[("PSP_BRIDGE_ENDPOINT_PATH", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    )
    .with_hello_delay(Duration::from_secs(30));
    let (addr, mock_cancel, mock_handle) = spawn_mock_mod(mock).await;
    let endpoint_path = write_endpoint_file(dir.path(), addr.port(), "secret-token", std::process::id());
    std::env::set_var(
        "PSP_BRIDGE_ENDPOINT_PATH",
        endpoint_path.to_str().unwrap(),
    );

    let service = BridgeService::new();
    service.start();
    tokio::time::sleep(Duration::from_millis(200)).await;

    let started = tokio::time::Instant::now();
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        service.request("get_status", serde_json::json!({})),
    )
    .await
    .expect("a request issued mid-dial must resolve well under the 6s request timeout");
    let elapsed = started.elapsed();
    assert!(
        matches!(result, Err(BridgeError::Offline)),
        "expected Err(BridgeError::Offline) while mid-dial, got {result:?}"
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "took {elapsed:?} to resolve, expected well under 2s"
    );

    let status = wait_for_status(
        service.status_rx(),
        Duration::from_secs(8),
        "record a transport error once the stalled dial times out",
        |s| s.last_error.is_some(),
    )
    .await;
    assert!(!status.connected);
    assert!(status.last_error.as_deref().is_some_and(|e| e.contains("transport")));

    service.shutdown().await;
    kill_mock(mock_cancel, mock_handle).await;
}

#[tokio::test]
async fn never_started_service_fails_requests_offline_immediately() {
    let service = BridgeService::new();

    let started = tokio::time::Instant::now();
    let result = service.request("get_status", serde_json::json!({})).await;
    let elapsed = started.elapsed();

    assert!(
        matches!(result, Err(BridgeError::Offline)),
        "expected Err(BridgeError::Offline) from a never-started service, got {result:?}"
    );
    assert!(
        elapsed < Duration::from_secs(1),
        "took {elapsed:?}, expected well under 1s"
    );
}

#[tokio::test]
async fn default_endpoint_path_reads_from_the_env_override() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("custom-endpoint.json");
    let _env = BridgeEnvGuard::acquire(&[(
        "PSP_BRIDGE_ENDPOINT_PATH",
        Some(path.to_str().unwrap()),
    )])
    .await;
    assert_eq!(
        psp_server::bridge::endpoint::default_endpoint_path(),
        Some(path)
    );
}

#[tokio::test]
async fn command_round_trips_the_mod_result_verbatim() {
    let _env = BridgeEnvGuard::acquire(&[("PSP_BRIDGE_ENDPOINT_PATH", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let expected = fixture_data(COMMAND_RESULT_HEAL_FIXTURE);
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    )
    .with_command_result("pal.heal", expected.clone());
    let (addr, mock_cancel, mock_handle) = spawn_mock_mod(mock).await;
    let endpoint_path = write_endpoint_file(dir.path(), addr.port(), "secret-token", std::process::id());
    std::env::set_var("PSP_BRIDGE_ENDPOINT_PATH", endpoint_path.to_str().unwrap());

    let service = BridgeService::new();
    service.start();
    wait_for_status(service.status_rx(), Duration::from_secs(5), "connect", |s| {
        s.connected
    })
    .await;

    let result = service
        .command(
            "pal.heal",
            "heal-cmd-1",
            serde_json::json!({ "playerUid": "11111111-2222-3333-4444-555555555555", "page": 0, "slotIndex": 3 }),
        )
        .await
        .expect("command should succeed");
    assert_eq!(result, expected);

    service.shutdown().await;
    kill_mock(mock_cancel, mock_handle).await;
}

#[tokio::test]
async fn command_mod_error_passes_through_as_typed_error() {
    let _env = BridgeEnvGuard::acquire(&[("PSP_BRIDGE_ENDPOINT_PATH", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    )
    .with_command_error("pal.heal", "not_authoritative", "server is not authoritative for this world");
    let (addr, mock_cancel, mock_handle) = spawn_mock_mod(mock).await;
    let endpoint_path = write_endpoint_file(dir.path(), addr.port(), "secret-token", std::process::id());
    std::env::set_var("PSP_BRIDGE_ENDPOINT_PATH", endpoint_path.to_str().unwrap());

    let service = BridgeService::new();
    service.start();
    wait_for_status(service.status_rx(), Duration::from_secs(5), "connect", |s| {
        s.connected
    })
    .await;

    let error = service
        .command("pal.heal", "heal-cmd-1", serde_json::json!({}))
        .await
        .expect_err("a mod error envelope should surface as BridgeError::Mod");
    match error {
        BridgeError::Mod { code, .. } => assert_eq!(code, "not_authoritative"),
        other => panic!("expected BridgeError::Mod, got {other:?}"),
    }

    service.shutdown().await;
    kill_mock(mock_cancel, mock_handle).await;
}

#[tokio::test]
async fn get_capabilities_round_trips_the_mod_payload() {
    let _env = BridgeEnvGuard::acquire(&[("PSP_BRIDGE_ENDPOINT_PATH", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let expected = fixture_data(CAPABILITIES_FIXTURE);
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    )
    .with_capabilities(expected.clone());
    let (addr, mock_cancel, mock_handle) = spawn_mock_mod(mock).await;
    let endpoint_path = write_endpoint_file(dir.path(), addr.port(), "secret-token", std::process::id());
    std::env::set_var("PSP_BRIDGE_ENDPOINT_PATH", endpoint_path.to_str().unwrap());

    let service = BridgeService::new();
    service.start();
    wait_for_status(service.status_rx(), Duration::from_secs(5), "connect", |s| {
        s.connected
    })
    .await;

    let result = service
        .get_capabilities()
        .await
        .expect("get_capabilities should succeed");
    assert_eq!(result, expected);

    service.shutdown().await;
    kill_mock(mock_cancel, mock_handle).await;
}

#[tokio::test]
async fn unsolicited_capabilities_push_is_dropped_without_disturbing_pending_requests() {
    let _env = BridgeEnvGuard::acquire(&[("PSP_BRIDGE_ENDPOINT_PATH", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let heal_result = fixture_data(COMMAND_RESULT_HEAL_FIXTURE);
    let capabilities = fixture_data(CAPABILITIES_FIXTURE);
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    )
    .with_command_result("pal.heal", heal_result.clone())
    .with_capabilities(capabilities.clone())
    .with_capabilities_push(capabilities.clone());
    let (addr, mock_cancel, mock_handle) = spawn_mock_mod(mock).await;
    let endpoint_path = write_endpoint_file(dir.path(), addr.port(), "secret-token", std::process::id());
    std::env::set_var("PSP_BRIDGE_ENDPOINT_PATH", endpoint_path.to_str().unwrap());

    let service = BridgeService::new();
    service.start();
    wait_for_status(service.status_rx(), Duration::from_secs(5), "connect", |s| {
        s.connected
    })
    .await;

    tokio::time::sleep(Duration::from_millis(200)).await;

    let command_result = service
        .command("pal.heal", "heal-cmd-1", serde_json::json!({}))
        .await
        .expect("command sent after the push should still round-trip correctly");
    assert_eq!(command_result, heal_result);

    let capabilities_result = service
        .get_capabilities()
        .await
        .expect("get_capabilities sent after the push should still round-trip correctly");
    assert_eq!(capabilities_result, capabilities);

    service.shutdown().await;
    kill_mock(mock_cancel, mock_handle).await;
}
