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
    // These suites exercise listen/auth/write verdicts, not transport, so
    // they run in the relaxed cleartext posture (what the asset-transport
    // tests below cover is the default's refusal).
    config.asset_transport = ps_network::AssetTransport::HttpsHttp;
    config.auth.scope = scope;
    if pin {
        config.auth.pin = Some(PinHash::generate("4321"));
    }
    config
}

async fn test_router(config: NetworkConfig) -> axum::Router {
    test_router_with_runtime(config).await.0
}

async fn test_router_with_runtime(config: NetworkConfig) -> (axum::Router, Arc<NetworkRuntime>) {
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
    let runtime = Arc::new(NetworkRuntime::new(config));
    let router = build_router(
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
            network_policy: Some(
                Arc::clone(&runtime) as Arc<dyn ps_app::network_policy::NetworkPolicy>
            ),
        }),
        &ui_dir,
        Arc::clone(&runtime),
    );
    (router, runtime)
}

/// A request shaped the way a browser sends it: origin-form URI (path only,
/// no scheme) regardless of peer. The scheme never appears on the wire for
/// plain HTTP/1.1 traffic, so the gate must not assume one.
fn peer_request(peer: &str, method: &str, uri: &str) -> Request<Body> {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(
        format!("{peer}:40000").parse::<SocketAddr>().unwrap(),
    ));
    request
}

/// A proxied request whose URI carries the https scheme (absolute form) —
/// the only way `secure_transport` can observe TLS upstream of us.
fn secure_peer_request(peer: &str, method: &str, uri: &str) -> Request<Body> {
    let mut request = Request::builder()
        .method(method)
        .uri(format!("https://palstudio.test{uri}"))
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

/// Marks a request as forwarded by the trusted local proxy (loopback socket
/// + Funnel on is the trust condition the gate applies).
fn with_forwarded_for(mut request: Request<Body>, client: &str) -> Request<Body> {
    request.headers_mut().insert(
        "x-forwarded-for",
        axum::http::HeaderValue::from_str(client).unwrap(),
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
async fn asset_transport_governs_remote_streaming() {
    // Default (HTTPS only): a browser-shaped cleartext request from an
    // admitted LAN peer is refused — even though the listen mode admits it,
    // because no HTTPS endpoint is hosted.
    let strict = NetworkConfig {
        listen: ListenMode::Lan,
        ..NetworkConfig::default()
    };
    let router = test_router(strict).await;
    let refused = router
        .clone()
        .oneshot(peer_request("192.168.1.50", "GET", "/"))
        .await
        .unwrap();
    assert_eq!(refused.status(), StatusCode::UPGRADE_REQUIRED);
    assert!(body_text(refused).await.contains("asset streaming"));

    // Relaxed (HTTPS or HTTP): the same cleartext request serves the SPA.
    let mut relaxed = NetworkConfig {
        listen: ListenMode::Lan,
        ..NetworkConfig::default()
    };
    relaxed.asset_transport = ps_network::AssetTransport::HttpsHttp;
    let router = test_router(relaxed).await;
    let served = router
        .oneshot(peer_request("192.168.1.50", "GET", "/"))
        .await
        .unwrap();
    assert_eq!(served.status(), StatusCode::OK);

    // Native HTTPS hosting: every request that reached us came through our
    // own TLS listener, so origin-form (scheme-less) requests pass the
    // HTTPS requirement.
    let mut hosted = NetworkConfig {
        listen: ListenMode::Lan,
        ..NetworkConfig::default()
    };
    hosted.https_enabled = true;
    let router = test_router(hosted).await;
    let served = router
        .oneshot(peer_request("192.168.1.50", "GET", "/"))
        .await
        .unwrap();
    assert_eq!(served.status(), StatusCode::OK);

    // Loopback only: remote peers are refused regardless of transport…
    let mut local_only = NetworkConfig {
        listen: ListenMode::Wan,
        ..NetworkConfig::default()
    };
    local_only.asset_transport = ps_network::AssetTransport::Loopback;
    let router = test_router(local_only).await;
    let refused = router
        .clone()
        .oneshot(peer_request("192.168.1.50", "GET", "/"))
        .await
        .unwrap();
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
    assert!(body_text(refused).await.contains("limited to this machine"));
    let refused_https = router
        .clone()
        .oneshot(secure_peer_request("192.168.1.50", "GET", "/"))
        .await
        .unwrap();
    assert_eq!(refused_https.status(), StatusCode::FORBIDDEN);

    // …while the local operator seat (direct loopback) always streams…
    let local = router
        .clone()
        .oneshot(peer_request("127.0.0.1", "GET", "/"))
        .await
        .unwrap();
    assert_eq!(local.status(), StatusCode::OK);

    // …and a Funnel forward is its remote client, not the loopback seat,
    // so loopback-only streaming refuses it too.
    let mut funnel_local_only = NetworkConfig {
        listen: ListenMode::Lan,
        ..NetworkConfig::default()
    };
    funnel_local_only.funnel_enabled = true;
    funnel_local_only.auth.scope = AuthScope::Always;
    funnel_local_only.auth.pin = Some(PinHash::generate("4321"));
    funnel_local_only.allow.connect = vec!["74.133.65.35".into()];
    funnel_local_only.asset_transport = ps_network::AssetTransport::Loopback;
    let router = test_router(funnel_local_only).await;
    let refused = router
        .oneshot(with_forwarded_for(
            peer_request("127.0.0.1", "GET", "/api/network/config"),
            "74.133.65.35",
        ))
        .await
        .unwrap();
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);

    // Under the default transport, the same Funnel forward IS HTTPS (tailscale
    // terminated TLS upstream) and passes.
    let mut funnel_https = NetworkConfig {
        listen: ListenMode::Lan,
        ..NetworkConfig::default()
    };
    funnel_https.funnel_enabled = true;
    funnel_https.auth.scope = AuthScope::Always;
    funnel_https.auth.pin = Some(PinHash::generate("4321"));
    funnel_https.allow.connect = vec!["74.133.65.35".into()];
    let router = test_router(funnel_https).await;
    let served = router
        .oneshot(with_forwarded_for(
            peer_request("127.0.0.1", "GET", "/api/network/config"),
            "74.133.65.35",
        ))
        .await
        .unwrap();
    assert_eq!(served.status(), StatusCode::UNAUTHORIZED); // PIN stands; transport does not 426
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
async fn strict_allow_mode_denies_unlisted_peers_entirely() {
    // "Nobody — list every address": with empty lists, only localhost gets
    // in; a LAN peer is refused before any handler runs.
    let mut policy = config(ListenMode::Lan, AuthScope::Never, false);
    policy.allow.mode = ps_network::AllowMode::Strict;
    let router = test_router(policy).await;

    let refused = router
        .oneshot(peer_request("192.168.1.50", "GET", "/api/network/config"))
        .await
        .unwrap();
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);

    // Listing the address in the connect allowlist restores view access…
    let mut listed = config(ListenMode::Lan, AuthScope::Never, false);
    listed.allow.mode = ps_network::AllowMode::Strict;
    listed.allow.connect = vec!["192.168.1.50".into()];
    let router = test_router(listed).await;
    let view = router
        .clone()
        .oneshot(peer_request("192.168.1.50", "GET", "/api/network/config"))
        .await
        .unwrap();
    assert_eq!(view.status(), StatusCode::OK);

    // …but edits stay blocked until the write allowlist names the peer too.
    let edit = router
        .oneshot(peer_json_request(
            "192.168.1.50",
            "PUT",
            "/api/network/config",
            "{}",
        ))
        .await
        .unwrap();
    assert_eq!(edit.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn open_allow_mode_lets_admitted_peers_edit_with_empty_lists() {
    let mut policy = config(ListenMode::Lan, AuthScope::Never, false);
    policy.allow.mode = ps_network::AllowMode::Open;
    let router = test_router(policy).await;

    let edit = router
        .oneshot(peer_json_request(
            "192.168.1.50",
            "PUT",
            "/api/network/config",
            "{}",
        ))
        .await
        .unwrap();
    // Reaches the handler (422 for the field-less body, NOT the gate's 403):
    // with empty lists in open mode, an admitted peer may also write.
    assert_eq!(edit.status(), StatusCode::UNPROCESSABLE_ENTITY);
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
async fn a_listed_tailnet_ip_connects_under_lan_listen_mode() {
    // The reported trap: a tailscale CGNAT address listed in the connect
    // allowlist was refused because the lan mode gate ran first. Listing an
    // address is operator intent and must admit it regardless of the mode's
    // default audience.
    let mut policy = config(ListenMode::Lan, AuthScope::Never, false);
    policy.allow.mode = ps_network::AllowMode::Strict;
    policy.allow.connect = vec!["100.115.95.115".into()];
    let router = test_router(policy).await;

    let view = router
        .clone()
        .oneshot(peer_request("100.115.95.115", "GET", "/api/network/config"))
        .await
        .unwrap();
    assert_eq!(view.status(), StatusCode::OK);

    // Listed for connect only: still read-only.
    let edit = router
        .clone()
        .oneshot(peer_json_request(
            "100.115.95.115",
            "PUT",
            "/api/network/config",
            "{}",
        ))
        .await
        .unwrap();
    assert_eq!(edit.status(), StatusCode::FORBIDDEN);
    assert!(body_text(edit).await.contains("read-only"));
}

#[tokio::test]
async fn removing_an_address_takes_effect_without_a_restart() {
    // The live runtime is the source of truth for every request: saving a
    // policy without the peer's address must refuse it on the NEXT request,
    // with no server restart in between.
    let mut policy = config(ListenMode::Lan, AuthScope::Never, false);
    policy.allow.mode = ps_network::AllowMode::Strict;
    policy.allow.connect = vec!["192.168.1.50".into()];
    let (router, runtime) = test_router_with_runtime(policy).await;

    let admitted = router
        .clone()
        .oneshot(peer_request("192.168.1.50", "GET", "/api/network/config"))
        .await
        .unwrap();
    assert_eq!(admitted.status(), StatusCode::OK);

    // `put_config` persists + set_config()s in one step; set_config alone is
    // the in-memory half of that save.
    let mut updated = runtime.config();
    updated.allow.connect = Vec::new();
    runtime.set_config(updated);

    let refused = router
        .oneshot(peer_request("192.168.1.50", "GET", "/api/network/config"))
        .await
        .unwrap();
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
    assert!(body_text(refused).await.contains("not allowed to connect"));
}

#[tokio::test]
async fn native_https_marks_the_session_cookie_secure_even_on_loopback() {
    // The operator unlocking over their own TLS listener (https://localhost)
    // must not receive a cookie the browser would happily resend over
    // cleartext after a later config flip.
    let mut policy = config(ListenMode::Lan, AuthScope::NetworkOnly, true);
    policy.https_enabled = true;
    let router = test_router(policy).await;
    let unlock = router
        .oneshot(peer_json_request(
            "127.0.0.1",
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
        .expect("session cookie");
    assert!(cookie.contains("Secure"), "cookie was {cookie}");

    // Without native HTTPS (and no funnel/forwarding), loopback stays on the
    // plain-HTTP dev path: no Secure attribute, or curl-on-http drops it.
    let plain = test_router(config(ListenMode::Lan, AuthScope::NetworkOnly, true)).await;
    let unlock = plain
        .oneshot(peer_json_request(
            "127.0.0.1",
            "POST",
            "/api/network/session",
            r#"{"pin":"4321"}"#,
        ))
        .await
        .unwrap();
    let cookie = unlock
        .headers()
        .get("set-cookie")
        .and_then(|v| v.to_str().ok())
        .expect("session cookie");
    assert!(!cookie.contains("Secure"), "cookie was {cookie}");
}

#[tokio::test]
async fn native_https_and_funnel_cannot_be_saved_together() {
    // Both on would strand Funnel: it forwards to this port over plain
    // HTTP and a TLS listener declines the handshake. The save is refused
    // before anything (db, tailscale) changes.
    let router = test_router(config(ListenMode::Lan, AuthScope::Always, false)).await;
    let conflict = router
        .oneshot(peer_json_request(
            "127.0.0.1",
            "PUT",
            "/api/network/config",
            r#"{"listen":"lan","port":5174,"auth":{"scope":"always"},"allow":{},"funnel_enabled":true,"https_enabled":true}"#,
        ))
        .await
        .unwrap();
    assert_eq!(conflict.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(conflict).await.contains("mutually exclusive"));
}

#[tokio::test]
async fn funnel_forwards_are_filtered_by_the_forwarded_client_address() {
    // Funnel terminates TLS and proxies over loopback; the allowlist must
    // apply to the X-Forwarded-For address, not to the proxy's loopback.
    let mut policy = config(ListenMode::Lan, AuthScope::Never, false);
    policy.funnel_enabled = true;
    policy.allow.mode = ps_network::AllowMode::Strict;
    policy.allow.connect = vec!["74.133.65.35".into()];
    let router = test_router(policy).await;

    // Listed funnel client: admitted (auth off here to isolate the IP layer).
    let admitted = router
        .clone()
        .oneshot(with_forwarded_for(
            peer_request("127.0.0.1", "GET", "/api/network/config"),
            "74.133.65.35",
        ))
        .await
        .unwrap();
    assert_eq!(admitted.status(), StatusCode::OK);

    // The reported behavior: an address that was REMOVED keeps connecting.
    // With the forwarded address enforced, an unlisted client is refused
    // even though the socket peer is the trusted loopback proxy.
    let mut removed = config(ListenMode::Lan, AuthScope::Never, false);
    removed.funnel_enabled = true;
    removed.allow.mode = ps_network::AllowMode::Strict;
    let router = test_router(removed).await;
    let refused = router
        .oneshot(with_forwarded_for(
            peer_request("127.0.0.1", "GET", "/api/network/config"),
            "74.133.65.35",
        ))
        .await
        .unwrap();
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
    assert!(body_text(refused).await.contains("not allowed to connect"));
}

#[tokio::test]
async fn forwarded_peers_do_not_inherit_the_loopback_seat() {
    // Loopback with NetworkOnly needs no PIN; the same socket presenting a
    // forwarded address must be judged as that network peer instead.
    let mut policy = config(ListenMode::Lan, AuthScope::NetworkOnly, true);
    policy.funnel_enabled = true;
    policy.allow.connect = vec!["74.133.65.35".into()];
    let router = test_router(policy).await;

    let pin_required = router
        .clone()
        .oneshot(with_forwarded_for(
            peer_request("127.0.0.1", "GET", "/api/network/config"),
            "74.133.65.35",
        ))
        .await
        .unwrap();
    assert_eq!(pin_required.status(), StatusCode::UNAUTHORIZED);

    // A forwarded claim of loopback is not the operator.
    let spoof = router
        .clone()
        .oneshot(with_forwarded_for(
            peer_request("127.0.0.1", "GET", "/api/network/config"),
            "127.0.0.1",
        ))
        .await
        .unwrap();
    assert_eq!(spoof.status(), StatusCode::FORBIDDEN);

    // Unparsable forwarding header on a trusted proxy path: fail closed.
    let garbage = router
        .clone()
        .oneshot(with_forwarded_for(
            peer_request("127.0.0.1", "GET", "/api/network/config"),
            "not-an-ip",
        ))
        .await
        .unwrap();
    assert_eq!(garbage.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn forwarding_headers_are_ignored_without_funnel_or_from_remote_sockets() {
    // Funnel off: XFF is client-controlled noise, the loopback operator
    // stays the trusted seat (dev proxies rely on this).
    let mut policy = config(ListenMode::Lan, AuthScope::Never, false);
    policy.allow.mode = ps_network::AllowMode::Strict;
    let router = test_router(policy).await;
    let trusted = router
        .clone()
        .oneshot(with_forwarded_for(
            peer_request("127.0.0.1", "GET", "/api/network/config"),
            "8.8.8.8",
        ))
        .await
        .unwrap();
    assert_eq!(trusted.status(), StatusCode::OK);

    // Funnel on but the socket is not loopback: the direct peer decides.
    let mut funnel = config(ListenMode::Lan, AuthScope::Never, false);
    funnel.funnel_enabled = true;
    funnel.allow.mode = ps_network::AllowMode::Strict;
    funnel.allow.connect = vec!["74.133.65.35".into()];
    let router = test_router(funnel).await;
    let direct = router
        .oneshot(with_forwarded_for(
            peer_request("192.168.1.50", "GET", "/api/network/config"),
            "74.133.65.35",
        ))
        .await
        .unwrap();
    assert_eq!(direct.status(), StatusCode::FORBIDDEN);
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

    // The HTTPS/transport knobs are exposure features too: a local webapp
    // cannot host TLS or relax/lock asset streaming.
    let refused = router
        .clone()
        .oneshot(peer_json_request(
            "127.0.0.1",
            "PUT",
            "/api/network/config",
            r#"{"listen":"localhost","port":9000,"auth":{"scope":"never"},"allow":{},"https_enabled":true}"#,
        ))
        .await
        .unwrap();
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
    assert!(body_text(refused).await.contains("local webapp"));

    let refused = router
        .clone()
        .oneshot(peer_json_request(
            "127.0.0.1",
            "PUT",
            "/api/network/config",
            r#"{"listen":"localhost","port":9000,"auth":{"scope":"never"},"allow":{},"asset_transport":"https-http"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);

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
    build_router_for(runtime).await
}

async fn build_router_for(runtime: NetworkRuntime) -> axum::Router {
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

/// A public websuite run without --allow-network is a read-only deployment:
/// the effective policy is clamped to a public audience, the DTO says
/// `edits_locked`, and every network-settings mutation — config saves and
/// runtime-mode switches alike — is refused even for the loopback operator.
#[tokio::test]
async fn websuite_locked_run_is_public_and_refuses_all_network_edits() {
    // Stored policy says localhost; the websuite clamp widens it to Wan for
    // the run, without mutating what a later hosted run would load.
    let stored = config(ListenMode::Localhost, AuthScope::Never, false);
    let runtime = NetworkRuntime::with_tier_and_edits(
        stored,
        ps_network::NetworkTier::WebSuite,
        false,
    );
    // Minted before the runtime moves into the router: the loopback operator
    // seat that the runtime-switch test needs.
    let session_token = runtime.sessions.issue(std::time::Duration::from_secs(60));
    let router = build_router_for(runtime).await;

    let view = router
        .clone()
        .oneshot(peer_request("127.0.0.1", "GET", "/api/network/config"))
        .await
        .unwrap();
    assert_eq!(view.status(), StatusCode::OK);
    let text = body_text(view).await;
    assert!(text.contains("\"tier\":\"websuite\""));
    assert!(text.contains("\"listen\":\"wan\""), "public clamp: {text}");
    assert!(text.contains("\"edits_locked\":true"));

    // A remote visitor is admitted (this is, by construction, a public
    // server) and sees the same locked posture.
    let remote = router
        .clone()
        .oneshot(peer_request("203.0.113.9", "GET", "/api/network/config"))
        .await
        .unwrap();
    assert_eq!(remote.status(), StatusCode::OK);
    assert!(body_text(remote).await.contains("\"edits_locked\":true"));

    // The loopback operator cannot edit the frozen policy either — not the
    // stored values, and not a relaxation.
    for payload in [
        // Identity save: still an edit.
        r#"{"listen":"wan","port":5174,"auth":{"scope":"never"},"allow":{}}"#,
        // Port change.
        r#"{"listen":"wan","port":9000,"auth":{"scope":"never"},"allow":{}}"#,
        // Listen-mode drop back to localhost (locking yourself out is
        // still an edit on a locked run).
        r#"{"listen":"localhost","port":5174,"auth":{"scope":"never"},"allow":{}}"#,
    ] {
        let refused = router
            .clone()
            .oneshot(peer_json_request("127.0.0.1", "PUT", "/api/network/config", payload))
            .await
            .unwrap();
        assert_eq!(refused.status(), StatusCode::FORBIDDEN);
        let message = body_text(refused).await;
        assert!(message.contains("locked"), "got: {message}");
        assert!(message.contains("--allow-network"), "got: {message}");
    }

    // Runtime mode is a network setting: switching is refused too. The
    // service-control plane demands its local session first — the same lock
    // then stops the switch even for an authenticated local operator.
    let refused = router
        .oneshot(with_cookie(
            peer_json_request(
                "127.0.0.1",
                "POST",
                "/api/network/runtime",
                r#"{"mode":"standalone"}"#,
            ),
            &format!("{}={}", ps_server::network::SESSION_COOKIE, session_token),
        ))
        .await
        .unwrap();
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
    assert!(body_text(refused).await.contains("locked"));
}

/// With --allow-network, the websuite tier keeps the full hosted surface:
/// the stored policy applies unclamped and edits work normally.
#[tokio::test]
async fn websuite_with_allow_network_stays_fully_editable() {
    let stored = config(ListenMode::Lan, AuthScope::Never, false);
    let runtime = NetworkRuntime::with_tier_and_edits(
        stored,
        ps_network::NetworkTier::WebSuite,
        true,
    );
    let router = build_router_for(runtime).await;

    let view = router
        .clone()
        .oneshot(peer_request("127.0.0.1", "GET", "/api/network/config"))
        .await
        .unwrap();
    assert_eq!(view.status(), StatusCode::OK);
    let text = body_text(view).await;
    assert!(text.contains("\"tier\":\"websuite\""));
    assert!(text.contains("\"listen\":\"lan\""), "stored policy: {text}");
    assert!(text.contains("\"edits_locked\":false"));

    // A listen-mode + port change goes through exactly like on `serve`.
    let edit = router
        .oneshot(peer_json_request(
            "127.0.0.1",
            "PUT",
            "/api/network/config",
            r#"{"listen":"tailscale","port":9100,"auth":{"scope":"never"},"allow":{}}"#,
        ))
        .await
        .unwrap();
    assert_eq!(edit.status(), StatusCode::OK);
    let text = body_text(edit).await;
    assert!(text.contains("\"listen\":\"tailscale\""));
    assert!(text.contains("\"restart_required\":true"));
}
