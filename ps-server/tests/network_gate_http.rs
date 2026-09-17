//! End-to-end tests for PalStudio's own network policy over real router
//! requests: listen-mode enforcement, PIN session flow, and the write
//! allowlist on mutating HTTP routes.
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::connect_info::ConnectInfo;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use ps_network::{AuthScope, ListenMode, NetworkConfig, PinHash};
use ps_server::network::NetworkRuntime;
use ps_server::router::build_router;
use ps_server::{AppConfig, AppState};

fn config(listen: ListenMode, scope: AuthScope, pin: bool) -> NetworkConfig {
    let mut config = NetworkConfig {
        listen,
        ..NetworkConfig::default()
    };
    config.auth.scope = scope;
    if pin {
        config.auth.pin = Some(PinHash::generate("4321"));
    }
    config
}

async fn test_router(config: NetworkConfig) -> axum::Router {
    let temp_dir = tempfile::tempdir().unwrap();
    let ui_dir = temp_dir.path().join("ui");
    std::fs::create_dir_all(&ui_dir).unwrap();
    std::fs::write(ui_dir.join("index.html"), "<html></html>").unwrap();
    // Keep the tree alive for the router's lifetime by leaking — these tests
    // are short-lived processes.
    let root = temp_dir.into_path();
    let ui_dir = root.join("ui");
    let data_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data");
    let db_path = root.join("test.db");
    let db = ps_db::open(&db_path).await.unwrap();
    let game_data = Arc::new(ps_core::gamedata::GameData::load(&data_dir.join("json")).unwrap());
    let (live_connections, _live_rx) = tokio::sync::watch::channel(0usize);
    let (live_bus, _bus_rx) = tokio::sync::watch::channel(None);
    let server_services = Arc::new(ps_server::services::ServerServices::with_docker(Arc::new(
        ps_server::services::docker::mock::MockDocker::default(),
    )));
    build_router(
        Arc::new(AppState {
            config: AppConfig {
                desktop_mode: false,
            },
            game_data,
            driver: Arc::new(ps_db::SqlxSqliteDriver::new(db)),
            dialogs: Arc::new(ps_server::desktop_dialogs::NullDialogProvider),
            live_connections,
            live_bus,
            ext: Arc::new(ps_server::server_ext::ServerExtRouter {
                services: server_services,
            }),
            lsp: Arc::new(ps_app::lsp::NullLspService),
            sessions: std::sync::Mutex::new(ps_server::SessionStore::default()),
            breeding_db: Default::default(),
            plugins: Default::default(),
            network_policy: None,
        }),
        &ui_dir,
        Arc::new(NetworkRuntime::new(config)),
    )
}

fn peer_request(peer: &str, method: &str, uri: &str) -> Request<Body> {
    let request_uri = if peer.parse::<std::net::IpAddr>().unwrap().is_loopback() {
        uri.to_owned()
    } else {
        format!("https://palstudio.test{uri}")
    };
    let mut request = Request::builder()
        .method(method)
        .uri(request_uri)
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(
        format!("{peer}:40000").parse::<SocketAddr>().unwrap(),
    ));
    request
}

fn peer_json_request(peer: &str, method: &str, uri: &str, body: &str) -> Request<Body> {
    let mut request = peer_request(peer, method, uri);
    request.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/json"),
    );
    *request.body_mut() = Body::from(body.to_owned());
    request
}

fn with_cookie(mut request: Request<Body>, cookie: &str) -> Request<Body> {
    request.headers_mut().insert(
        axum::http::header::COOKIE,
        axum::http::HeaderValue::from_str(cookie).unwrap(),
    );
    request
}

async fn body_text(response: axum::response::Response) -> String {
    String::from_utf8(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap()
}

#[tokio::test]
async fn localhost_mode_admits_loopback_and_refuses_lan() {
    let router = test_router(config(ListenMode::Localhost, AuthScope::Never, false)).await;

    let ok = router
        .clone()
        .oneshot(peer_request("127.0.0.1", "GET", "/"))
        .await
        .unwrap();
    assert_eq!(ok.status(), StatusCode::OK);

    let refused = router
        .oneshot(peer_request("192.168.1.50", "GET", "/"))
        .await
        .unwrap();
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
    assert!(body_text(refused).await.contains("not allowed to connect"));
}

#[tokio::test]
async fn pin_flow_unlocks_network_peers_and_unlock_page_stays_reachable() {
    let router = test_router(config(ListenMode::Lan, AuthScope::NetworkOnly, true)).await;

    // Locked out without a session: human navigations are redirected to the
    // unlock page (no SPA bytes served)…
    let locked = router
        .clone()
        .oneshot(peer_request("192.168.1.50", "GET", "/"))
        .await
        .unwrap();
    assert_eq!(locked.status(), StatusCode::SEE_OTHER);
    let location = locked
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .expect("unlock redirect location");
    assert!(
        location.starts_with("/network-unlock?next="),
        "got {location}"
    );

    // …machine clients (API/websocket) still get the detectable 401…
    let api = router
        .clone()
        .oneshot(peer_request("192.168.1.50", "GET", "/api/network/config"))
        .await
        .unwrap();
    assert_eq!(api.status(), StatusCode::UNAUTHORIZED);

    // …but the unlock page and the session endpoint are exempt…
    let page = router
        .clone()
        .oneshot(peer_request("192.168.1.50", "GET", "/network-unlock"))
        .await
        .unwrap();
    assert_eq!(page.status(), StatusCode::OK);
    assert!(body_text(page).await.contains("PIN-protected"));

    let unlock = router
        .clone()
        .oneshot(peer_json_request(
            "192.168.1.50",
            "POST",
            "/api/network/session",
            r#"{"pin":"4321"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(unlock.status(), StatusCode::OK);
    let cookie = unlock
        .headers()
        .get("set-cookie")
        .and_then(|v| v.to_str().ok())
        .expect("session cookie")
        .split(';')
        .next()
        .unwrap()
        .to_owned();

    // …and the cookie grants access.
    let admitted = router
        .clone()
        .oneshot(with_cookie(
            peer_request("192.168.1.50", "GET", "/"),
            &cookie,
        ))
        .await
        .unwrap();
    assert_eq!(admitted.status(), StatusCode::OK);

    // Loopback never needed the PIN.
    let local = router
        .oneshot(peer_request("127.0.0.1", "GET", "/"))
        .await
        .unwrap();
    assert_eq!(local.status(), StatusCode::OK);
}

#[tokio::test]
async fn wrong_pin_is_rejected_with_delay() {
    let router = test_router(config(ListenMode::Lan, AuthScope::NetworkOnly, true)).await;
    let started = std::time::Instant::now();
    let wrong = router
        .oneshot(peer_json_request(
            "192.168.1.50",
            "POST",
            "/api/network/session",
            r#"{"pin":"0000"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);
    assert!(
        started.elapsed() >= std::time::Duration::from_millis(350),
        "wrong PIN must be slowed, took {:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn write_allowlist_blocks_mutating_http_for_view_only_peers() {
    let mut policy = config(ListenMode::Lan, AuthScope::Never, false);
    policy.allow.write = vec!["192.168.1.7".into()];
    let router = test_router(policy).await;

    // Reads pass for any admitted LAN peer…
    let reader_get = router
        .clone()
        .oneshot(peer_request("192.168.1.50", "GET", "/api/network/config"))
        .await
        .unwrap();
    assert_eq!(reader_get.status(), StatusCode::OK);

    // …but the config PUT (and any mutation) is reserved for write peers.
    let reader_put = router
        .clone()
        .oneshot(peer_json_request(
            "192.168.1.50",
            "PUT",
            "/api/network/config",
            "{}",
        ))
        .await
        .unwrap();
    assert_eq!(reader_put.status(), StatusCode::FORBIDDEN);
    assert!(body_text(reader_put).await.contains("read-only"));

    let writer_put = router
        .oneshot(peer_json_request(
            "192.168.1.7",
            "PUT",
            "/api/network/config",
            "{}",
        ))
        .await
        .unwrap();
    // Reaches the handler (422 for the field-less body, NOT the gate's 403).
    assert_eq!(writer_put.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn fail_closed_when_auth_is_demanded_without_a_pin() {
    let router = test_router(config(ListenMode::Lan, AuthScope::NetworkOnly, false)).await;
    let refused = router
        .oneshot(peer_request("192.168.1.50", "GET", "/"))
        .await
        .unwrap();
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn config_endpoint_never_leaks_the_pin_hash() {
    let router = test_router(config(ListenMode::Lan, AuthScope::NetworkOnly, true)).await;
    let body = router
        .oneshot(peer_request("127.0.0.1", "GET", "/api/network/config"))
        .await
        .unwrap();
    assert_eq!(body.status(), StatusCode::OK);
    let text = body_text(body).await;
    assert!(text.contains("\"pin_set\":true"));
    assert!(!text.contains("\"hash\""));
    assert!(!text.contains("\"salt\""));
}

#[tokio::test]
async fn local_webapp_tier_is_clamped_to_localhost_and_port_only() {
    // A hand-launched webapp tier over a stored "wide open" policy: the
    // enforcement and the visible config clamp to localhost; only the port
    // is editable; everything else is refused with guidance.
    let mut stored = config(ListenMode::Wan, AuthScope::Never, false);
    stored.allow.connect = vec!["0.0.0.0/0".into()];
    let runtime = NetworkRuntime::with_tier(stored, ps_network::NetworkTier::LocalWebapp);
    let router = build_router_local_webapp(runtime).await;

    let body = router
        .clone()
        .oneshot(peer_request("127.0.0.1", "GET", "/api/network/config"))
        .await
        .unwrap();
    assert_eq!(body.status(), StatusCode::OK);
    let text = body_text(body).await;
    assert!(text.contains("\"tier\":\"localwebapp\""));
    assert!(text.contains("\"listen\":\"localhost\""));

    // Trying to open the instance up is refused.
    let refused = router
        .clone()
        .oneshot(peer_json_request(
            "127.0.0.1",
            "PUT",
            "/api/network/config",
            r#"{"listen":"lan","port":9000,"auth":{"scope":"never"},"allow":{}}"#,
        ))
        .await
        .unwrap();
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
    assert!(body_text(refused).await.contains("local webapp"));

    // A pure port change goes through.
    let port_edit = router
        .clone()
        .oneshot(peer_json_request(
            "127.0.0.1",
            "PUT",
            "/api/network/config",
            r#"{"listen":"localhost","port":9001,"auth":{"scope":"never"},"allow":{}}"#,
        ))
        .await
        .unwrap();
    assert_eq!(port_edit.status(), StatusCode::OK);
    assert!(body_text(port_edit)
        .await
        .contains("\"restart_required\":true"));

    // Non-loopback peers are refused even though the STORED policy said wan.
    let denied = router
        .oneshot(peer_request("192.168.1.9", "GET", "/api/network/config"))
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
}

async fn build_router_local_webapp(runtime: NetworkRuntime) -> axum::Router {
    let temp_dir = tempfile::tempdir().unwrap();
    let ui_dir = temp_dir.path().join("ui");
    std::fs::create_dir_all(&ui_dir).unwrap();
    std::fs::write(ui_dir.join("index.html"), "<html></html>").unwrap();
    let root = temp_dir.into_path();
    let ui_dir = root.join("ui");
    let data_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data");
    let db = ps_db::open(&root.join("test.db")).await.unwrap();
    let game_data = Arc::new(ps_core::gamedata::GameData::load(&data_dir.join("json")).unwrap());
    let (live_connections, _live_rx) = tokio::sync::watch::channel(0usize);
    let (live_bus, _bus_rx) = tokio::sync::watch::channel(None);
    let server_services = Arc::new(ps_server::services::ServerServices::with_docker(Arc::new(
        ps_server::services::docker::mock::MockDocker::default(),
    )));
    build_router(
        Arc::new(AppState {
            config: AppConfig {
                desktop_mode: false,
            },
            game_data,
            driver: Arc::new(ps_db::SqlxSqliteDriver::new(db)),
            dialogs: Arc::new(ps_server::desktop_dialogs::NullDialogProvider),
            live_connections,
            live_bus,
            ext: Arc::new(ps_server::server_ext::ServerExtRouter {
                services: server_services,
            }),
            lsp: Arc::new(ps_app::lsp::NullLspService),
            sessions: std::sync::Mutex::new(ps_server::SessionStore::default()),
            breeding_db: Default::default(),
            plugins: Default::default(),
            network_policy: None,
        }),
        &ui_dir,
        Arc::new(runtime),
    )
}
