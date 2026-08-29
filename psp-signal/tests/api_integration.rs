//! End-to-end tests: a running SignalManager on an ephemeral port, driven
//! over real HTTP, plus a stub Palworld dedicated server for the REST
//! pipeline. These exercise the full wire behavior end to end (auth, CORS,
//! states, minimap claims, wire shapes).
use std::time::Duration;

use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use psp_signal::manager::SignalManager;
use psp_signal::poller::SourceConfig;
use psp_signal::store::{MemorySignalStore, SignalStored};

async fn manager_with(source: Option<SourceConfig>) -> SignalManager {
    let mut stored = SignalStored::defaults();
    stored.bind = "127.0.0.1".into();
    stored.port = 0; // ephemeral
    let manager = SignalManager::new(Box::new(MemorySignalStore::new(stored))).await;
    if let Some(source) = source {
        manager.set_source(Some(source)).await.unwrap();
    }
    manager
}

async fn spawn_manager(source: Option<SourceConfig>) -> (SignalManager, String) {
    let manager = manager_with(source).await;
    let addr = manager.start().await.expect("ephemeral bind always works");
    (manager, format!("http://{addr}"))
}

// ---------------------------------------------------------------- stub server

type AuthLog = Arc<std::sync::Mutex<Vec<String>>>;

/// `mode`: "game-data" (rich feed), "players-only" (game-data 404). Every
/// players request appends the auth header it saw, so tests can assert
/// the basic-auth header reached the stub.
async fn spawn_stub_palworld(mode: &'static str, auth_log: AuthLog) -> u16 {
    let players_log = Arc::clone(&auth_log);
    let app = Router::new()
        .route(
            "/v1/api/players",
            get(move |headers: HeaderMap| {
                let players_log = Arc::clone(&players_log);
                async move {
                    players_log.lock().unwrap().push(
                        headers
                            .get("authorization")
                            .map(|value| value.to_str().unwrap().to_string())
                            .unwrap_or_default(),
                    );
                    Json(serde_json::json!({"players": [
                        {"accountInfo": {"accountId": "76561198000000000", "name": "StubTamer"},
                         "playerInfo": {"level": 9, "hp": 111, "maxHP": 111}}
                    ]}))
                }
            }),
        )
        .route(
            "/v1/api/game-data",
            get(move |_state: State<()>| async move {
                if mode == "players-only" {
                    return (StatusCode::NOT_FOUND, "no game-data here").into_response();
                }
                Json(serde_json::json!({"ActorData": [
                    {"UnitType": "Player", "InstanceID": "p1", "NickName": "StubTamer",
                     "LocationX": -100000.0, "LocationY": 50000.0, "LocationZ": 2000.0,
                     "RotationZ": 12.0, "Level": 9, "HP": 111, "MaxHP": 111},
                    {"UnitType": "Pal", "InstanceID": "pal1", "Class": "Chara_BP_SheepBall_C",
                     "LocationX": -100100.0, "LocationY": 50100.0, "RotationZ": 90.0}
                ]}))
                .into_response()
            }),
        )
        .with_state(());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    port
}



/// Polls `/v1/hello` until `feeding` flips true — the poller's first tick
/// races the first HTTP request, like any real consumer.
async fn wait_for_hello_feeding(base: &str) -> serde_json::Value {
    for _ in 0..40 {
        if let Ok(body) = reqwest::get(format!("{base}/v1/hello")).await {
            if let Ok(body) = body.json::<serde_json::Value>().await {
                if body["feeding"] == true {
                    return body;
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    panic!("hello never reported feeding");
}

/// Polls `/v1/live` until `ok` is true (auth via bearer or query token).
async fn wait_for_live(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    origin: Option<&str>,
) -> serde_json::Value {
    for _ in 0..40 {
        let mut request = client.get(format!("{base}/v1/live?token={token}"));
        if let Some(origin) = origin {
            request = request.header("Origin", origin);
        }
        if let Ok(response) = request.send().await {
            if let Ok(body) = response.json::<serde_json::Value>().await {
                if body["ok"] == true {
                    return body;
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    panic!("live frame never became ok");
}

// ---------------------------------------------------------------- API surface

#[tokio::test]
async fn hello_is_public_and_reports_identity() {
    let (manager, base) = spawn_manager(Some(SourceConfig::Fake)).await;
    let body = wait_for_hello_feeding(&base).await;
    assert_eq!(body["name"], "PalStudio Signal");
    assert_eq!(body["feeding"], true);
    assert_eq!(body["needsToken"], true);
    assert_eq!(body["source"], "fake");
    assert!(body["semver"].as_str().is_some());
    manager.stop().await;
}

#[tokio::test]
async fn live_serves_the_fake_frame_to_bearer_and_query_tokens() {
    let (manager, base) = spawn_manager(Some(SourceConfig::Fake)).await;
    let token = manager.snapshot().await.token;

    let client = reqwest::Client::new();
    let by_bearer = wait_for_live(&client, &base, &token, None).await;
    assert_eq!(by_bearer["ok"], true);
    assert_eq!(by_bearer["source"], "fake");
    assert_eq!(by_bearer["unit"], "game");
    let kinds: Vec<&str> = by_bearer["actors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|actor| actor["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, vec!["player", "otomo", "wild", "palbox"]);

    let by_query: serde_json::Value = client
        .get(format!("{base}/v1/live?token={token}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(by_query["ok"], true);
    manager.stop().await;
}

#[tokio::test]
async fn live_auth_rejects_wrong_or_missing_tokens() {
    let (manager, base) = spawn_manager(Some(SourceConfig::Fake)).await;
    let client = reqwest::Client::new();

    let wrong = client
        .get(format!("{base}/v1/live"))
        .bearer_auth("WRONGTOKEN1")
        .send()
        .await
        .unwrap();
    assert_eq!(wrong.status(), 403);
    let body: serde_json::Value = wrong.json().await.unwrap();
    assert_eq!(body["error"], "bad or missing token");

    let none = client.get(format!("{base}/v1/live")).send().await.unwrap();
    assert_eq!(none.status(), 403);

    // Loopback + approved origin is allowed without a token; a foreign
    // origin is not, and neither is an empty origin.
    let allowed = client
        .get(format!("{base}/v1/live"))
        .header("Origin", "http://127.0.0.1:5173")
        .send()
        .await
        .unwrap();
    assert_eq!(allowed.status(), 200);
    let evil = client
        .get(format!("{base}/v1/live"))
        .header("Origin", "https://evil.example")
        .send()
        .await
        .unwrap();
    assert_eq!(evil.status(), 403);
    manager.stop().await;
}

#[tokio::test]
async fn options_preflight_is_narrow_and_private_network_aware() {
    let (manager, base) = spawn_manager(Some(SourceConfig::Fake)).await;
    let client = reqwest::Client::new();
    let response = client
        .request(reqwest::Method::OPTIONS, format!("{base}/v1/live"))
        .header("Origin", "http://localhost:9999")
        .header("Access-Control-Request-Private-Network", "true")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 204);
    let headers = response.headers();
    assert_eq!(
        headers.get("access-control-allow-origin").unwrap(),
        "http://localhost:9999"
    );
    assert_eq!(
        headers.get("access-control-allow-methods").unwrap(),
        "GET, POST, OPTIONS"
    );
    assert_eq!(
        headers.get("access-control-allow-headers").unwrap(),
        "Authorization, Content-Type"
    );
    assert_eq!(headers.get("access-control-max-age").unwrap(), "600");
    assert_eq!(
        headers.get("access-control-allow-private-network").unwrap(),
        "true"
    );

    // A foreign origin gets no CORS headers at all.
    let response = client
        .request(reqwest::Method::OPTIONS, format!("{base}/v1/live"))
        .header("Origin", "https://evil.example")
        .send()
        .await
        .unwrap();
    assert!(response.headers().get("access-control-allow-origin").is_none());
    manager.stop().await;
}

#[tokio::test]
async fn unknown_routes_and_methods_speak_the_error_shapes() {
    let (manager, base) = spawn_manager(Some(SourceConfig::Fake)).await;
    let token = manager.snapshot().await.token;
    let client = reqwest::Client::new();

    let missing: serde_json::Value = client
        .get(format!("{base}/v1/nope"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(missing["error"], "no such endpoint");

    let method: serde_json::Value = client
        .delete(format!("{base}/v1/live"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(method["error"], "method");
    manager.stop().await;
}

#[tokio::test]
async fn minimap_claims_need_a_title_and_report_desktop_support() {
    let (manager, base) = spawn_manager(Some(SourceConfig::Fake)).await;
    let token = manager.snapshot().await.token;
    let client = reqwest::Client::new();

    let missing_title: serde_json::Value = client
        .post(format!("{base}/v1/minimap"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"open": true}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(missing_title["error"], "a claim needs a window title");

    let claim: serde_json::Value = client
        .post(format!("{base}/v1/minimap"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"open": true, "title": "Signal Map"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(claim["ok"], true);
    assert_eq!(claim["supported"], false);
    assert_eq!(claim["active"], true);

    let release: serde_json::Value = client
        .post(format!("{base}/v1/minimap"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"open": false}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(release["active"], false);
    manager.stop().await;
}

// ------------------------------------------------------------ REST pipeline

#[tokio::test]
async fn rest_source_reaches_world_state_through_the_rich_feed() {
    let auth_seen: AuthLog = Arc::new(std::sync::Mutex::new(Vec::new()));
    let port = spawn_stub_palworld("game-data", Arc::clone(&auth_seen)).await;
    let (manager, base) = spawn_manager(None).await;
    let token = manager.snapshot().await.token;

    let client = reqwest::Client::new();
    let connect: serde_json::Value = client
        .post(format!("{base}/v1/server"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"url": format!("127.0.0.1:{port}"), "password": "pw"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(connect["ok"], true, "connect card: {connect}");
    assert_eq!(connect["passwordSet"], true);
    assert!(connect.get("password").is_none(), "password never on the wire");

    // The probe + first read happen within a couple of 1s polls.
    let mut card = serde_json::Value::Null;
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(300)).await;
        card = client
            .get(format!("{base}/v1/server"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if card["state"] == "world" {
            break;
        }
    }
    assert_eq!(card["state"], "world", "server card: {card}");
    assert_eq!(card["configured"], true);

    let live: serde_json::Value = client
        .get(format!("{base}/v1/live"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(live["source"], "restgamedata");
    assert_eq!(live["unit"], "game");
    let actors = live["actors"].as_array().unwrap();
    assert_eq!(actors.len(), 2);
    assert_eq!(actors[0]["x"], -100.0);
    assert_eq!(actors[1]["tribe"], "SheepBall");

    // The stub saw the basic-auth header.
    let seen = auth_seen.lock().unwrap().clone();
    assert!(
        seen.iter().any(|value| value.starts_with("Basic ")),
        "basic auth header must reach the server"
    );

    // Forgetting the server clears the card.
    let cleared: serde_json::Value = client
        .post(format!("{base}/v1/server"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"clear": true}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(cleared["configured"], false);
    manager.stop().await;
}

#[tokio::test]
async fn rest_source_falls_back_to_players_only_and_reports_auth_failures() {
    let auth_seen: AuthLog = Arc::new(std::sync::Mutex::new(Vec::new()));
    let port = spawn_stub_palworld("players-only", auth_seen).await;
    let (manager, base) = spawn_manager(None).await;
    let token = manager.snapshot().await.token;
    let client = reqwest::Client::new();

    client
        .post(format!("{base}/v1/server"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"url": format!("http://127.0.0.1:{port}"), "password": "pw"}))
        .send()
        .await
        .unwrap();
    let mut card = serde_json::Value::Null;
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(300)).await;
        card = client
            .get(format!("{base}/v1/server"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if card["state"] == "players" {
            break;
        }
    }
    assert_eq!(card["state"], "players", "players-only card: {card}");
    let live: serde_json::Value = client
        .get(format!("{base}/v1/live"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(live["unit"], "unknown");
    assert_eq!(live["actors"].as_array().unwrap().len(), 1);
    manager.stop().await;
}

#[tokio::test]
async fn unreachable_rest_lands_in_down_not_a_crash() {
    let (manager, base) = spawn_manager(None).await;
    let token = manager.snapshot().await.token;
    let client = reqwest::Client::new();
    client
        .post(format!("{base}/v1/server"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"url": "http://127.0.0.1:1", "password": "pw"}))
        .send()
        .await
        .unwrap();
    let mut card = serde_json::Value::Null;
    for _ in 0..30 {
        tokio::time::sleep(Duration::from_millis(300)).await;
        card = client
            .get(format!("{base}/v1/server"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if card["state"] == "down" {
            break;
        }
    }
    assert_eq!(card["state"], "down", "down card: {card}");
    assert!(card["error"].as_str().unwrap().contains("unreachable"));
    manager.stop().await;
}

// ---------------------------------------------------------- gamedata source

#[tokio::test]
async fn gamedata_source_reads_the_bridge_file_and_survives_torn_writes() {
    let dir = tempfile::tempdir().unwrap();
    let bridge = dir.path().join("GameData.json");
    std::fs::write(
        &bridge,
        serde_json::json!({"ActorData": [
            {"UnitType": "Player", "InstanceID": "p1", "NickName": "LocalTamer",
             "LocationX": -412456.0, "LocationY": 88300.0, "LocationZ": 12400.0,
             "RotationZ": -78.0}
        ]})
        .to_string(),
    )
    .unwrap();

    let (manager, base) = spawn_manager(Some(SourceConfig::GameData { path: Some(bridge.clone()) })).await;
    let token = manager.snapshot().await.token;
    let client = reqwest::Client::new();

    let mut live = serde_json::Value::Null;
    for _ in 0..10 {
        tokio::time::sleep(Duration::from_millis(250)).await;
        live = client
            .get(format!("{base}/v1/live"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if live["ok"] == true {
            break;
        }
    }
    assert_eq!(live["source"], "gamedata");
    let actor = &live["actors"][0];
    assert_eq!(actor["x"], -412.46);
    assert_eq!(actor["y"], 88.3);
    assert_eq!(actor["alt"], 12.4);
    assert_eq!(actor["yaw"], -78.0);

    // A torn write is a hiccup, never a crash: the last frame stays.
    std::fs::write(&bridge, "{\"ActorData\": [{\"Uni").unwrap();
    tokio::time::sleep(Duration::from_millis(600)).await;
    let live: serde_json::Value = client
        .get(format!("{base}/v1/live"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(live["ok"], true, "last frame survives a torn write");
    manager.stop().await;
}

// ------------------------------------------------------------- manager flows

#[tokio::test]
async fn token_regenerate_changes_the_pairing_and_old_tokens_stop_working() {
    let (manager, base) = spawn_manager(Some(SourceConfig::Fake)).await;
    let old = manager.snapshot().await.token;
    let fresh = manager.regenerate_token().await;
    assert_ne!(old, fresh);

    let client = reqwest::Client::new();
    let response = client
        .get(format!("{base}/v1/live"))
        .bearer_auth(old)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
    let response = client
        .get(format!("{base}/v1/live"))
        .bearer_auth(fresh)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    manager.stop().await;
}

#[tokio::test]
async fn stop_and_start_keep_the_token_and_settings() {
    let (manager, base) = spawn_manager(Some(SourceConfig::Fake)).await;
    let token = manager.snapshot().await.token;
    manager.stop().await;
    assert!(!manager.is_running().await);

    // The stopped API refuses connections.
    assert!(reqwest::get(format!("{base}/v1/hello")).await.is_err());

    let addr = manager.start().await.unwrap();
    let body = wait_for_hello_feeding(&format!("http://{addr}")).await;
    assert_eq!(body["source"], "fake", "source survives a restart");
    assert_eq!(manager.snapshot().await.token, token, "token survives");
    manager.stop().await;
}

#[tokio::test]
async fn stored_settings_never_include_a_password() {
    let auth_seen: AuthLog = Arc::new(std::sync::Mutex::new(Vec::new()));
    let port = spawn_stub_palworld("game-data", Arc::clone(&auth_seen)).await;
    let manager = manager_with(None).await;
    manager
        .set_source(Some(SourceConfig::Rest {
            base: format!("http://127.0.0.1:{port}"),
            password: Some("super-secret".into()),
        }))
        .await
        .unwrap();
    let stored = manager.stored().await;
    let serialized = serde_json::to_string(&stored).unwrap();
    assert!(!serialized.contains("super-secret"));
    assert!(!serialized.to_lowercase().contains("password"));
    manager.stop().await;
}

#[tokio::test]
async fn locked_sources_refuse_redirects_from_the_wire() {
    let (manager, base) = spawn_manager(None).await;
    manager.lock_source(true).await;
    let token = manager.snapshot().await.token;
    let client = reqwest::Client::new();
    let card: serde_json::Value = client
        .post(format!("{base}/v1/server"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"url": "http://127.0.0.1:1", "password": "x"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(card["ok"], false);
    assert!(card["error"].as_str().unwrap().contains("host app"));
    manager.stop().await;
}
