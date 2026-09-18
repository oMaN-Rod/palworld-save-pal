//! A local stand-in for the Nexus Mods API, bound to 127.0.0.1:0.
use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Json;
use serde_json::{json, Value};

pub const PREMIUM_KEY: &str = "stub-premium-key";
pub const FREE_KEY: &str = "stub-free-key";
pub const MOD_ID: u32 = 4821;
pub const FILE_ID: u32 = 99001;
pub const OLD_FILE_ID: u32 = 99000;

pub struct StubState {
    pub base: String,
    pub archive: Vec<u8>,
    pub rate_limited: AtomicBool,
    pub graphql_apikeys: Mutex<Vec<Option<String>>>,
    pub graphql_queries: Mutex<Vec<String>>,
    pub download_link_queries: Mutex<Vec<HashMap<String, String>>>,
    pub cdn_hits: AtomicUsize,
    pub files: Mutex<BTreeMap<u32, Value>>,
}

pub struct NexusStub {
    pub base: String,
    pub state: Arc<StubState>,
}

pub fn stub_archive() -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options: zip::write::FileOptions<'_, ()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    writer
        .start_file("CoolMod/Scripts/main.lua", options)
        .unwrap();
    writer.write_all(b"print('hi')").unwrap();
    writer.finish().unwrap().into_inner()
}

pub fn file_uri(file_id: u32, version: &str) -> String {
    format!(
        "Cool Mod-{MOD_ID}-{}-{file_id}.zip",
        version.replace('.', "-")
    )
}

pub fn file_json(file_id: u32, category: &str, version: &str, size: usize) -> Value {
    json!({
        "fileId": file_id, "name": "Cool Mod", "version": version, "category": category,
        "date": 1_700_000_000, "sizeInBytes": size.to_string(), "uri": file_uri(file_id, version),
        "primary": 1, "description": ""
    })
}

pub async fn spawn_nexus_stub() -> NexusStub {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let archive = stub_archive();
    let files = BTreeMap::from([(
        MOD_ID,
        json!([
            file_json(OLD_FILE_ID, "OLD_VERSION", "1.0.0", archive.len()),
            file_json(FILE_ID, "MAIN", "1.2.0", archive.len()),
        ]),
    )]);
    let state = Arc::new(StubState {
        base: base.clone(),
        archive,
        rate_limited: AtomicBool::new(false),
        graphql_apikeys: Mutex::new(Vec::new()),
        graphql_queries: Mutex::new(Vec::new()),
        download_link_queries: Mutex::new(Vec::new()),
        cdn_hits: AtomicUsize::new(0),
        files: Mutex::new(files),
    });
    let router = axum::Router::new()
        .route("/v1/users/validate.json", get(validate))
        .route("/v1/games/palworld.json", get(game))
        .route(
            "/v1/games/palworld/mods/{mod_id}/files/{file_id}/download_link.json",
            get(download_link),
        )
        .route("/v2/graphql", post(graphql))
        .route("/cdn/{name}", get(cdn))
        .with_state(state.clone());
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    NexusStub { base, state }
}

fn apikey(headers: &HeaderMap) -> Option<String> {
    headers
        .get("apikey")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

fn known_key(headers: &HeaderMap) -> bool {
    matches!(apikey(headers).as_deref(), Some(PREMIUM_KEY | FREE_KEY))
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({ "message": "Please provide a valid API Key" })),
    )
        .into_response()
}

fn rate_limited() -> Response {
    (
        StatusCode::TOO_MANY_REQUESTS,
        [
            ("x-rl-daily-limit", "20000"),
            ("x-rl-daily-remaining", "0"),
            ("x-rl-daily-reset", "2026-09-17T00:00:00+00:00"),
            ("x-rl-hourly-limit", "500"),
            ("x-rl-hourly-remaining", "0"),
            ("x-rl-hourly-reset", "2026-09-16T21:00:00+00:00"),
        ],
        Json(json!({ "message": "Rate limit exceeded" })),
    )
        .into_response()
}

async fn validate(State(state): State<Arc<StubState>>, headers: HeaderMap) -> Response {
    if state.rate_limited.load(Ordering::SeqCst) {
        return rate_limited();
    }
    let premium = match apikey(&headers).as_deref() {
        Some(PREMIUM_KEY) => true,
        Some(FREE_KEY) => false,
        _ => return unauthorized(),
    };
    Json(json!({
        "user_id": 7, "key": apikey(&headers), "name": "Tester",
        "is_premium?": premium, "is_premium": premium, "is_supporter": premium,
        "email": "tester@example.invalid", "profile_url": "https://example.invalid/users/7"
    }))
    .into_response()
}

async fn game(headers: HeaderMap) -> Response {
    if !known_key(&headers) {
        return unauthorized();
    }
    Json(
        json!({ "id": 6063, "name": "Palworld", "domain_name": "palworld", "categories": [
        { "category_id": 1, "name": "Palworld", "parent_category": false },
        { "category_id": 10, "name": "Pals", "parent_category": 1 }
    ] }),
    )
    .into_response()
}

async fn download_link(
    State(state): State<Arc<StubState>>,
    Path((_mod_id, file_id)): Path<(u32, u32)>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    state
        .download_link_queries
        .lock()
        .unwrap()
        .push(query.clone());
    if !known_key(&headers) {
        return unauthorized();
    }
    match (query.get("key"), query.get("expires")) {
        (Some(key), Some(_)) if key == "expired" => {
            return (StatusCode::GONE, Json(json!({ "message": "expired" }))).into_response()
        }
        (Some(_), None) | (None, Some(_)) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "message": "mismatch" })),
            )
                .into_response()
        }
        (None, None) if apikey(&headers).as_deref() == Some(FREE_KEY) => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({ "message": "premium only" })),
            )
                .into_response()
        }
        _ => {}
    }
    Json(json!([{ "name": "Stub CDN", "short_name": "stub",
                  "URI": format!("{}/cdn/file-{file_id}.zip", state.base) }]))
    .into_response()
}

async fn graphql(
    State(state): State<Arc<StubState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    state.graphql_apikeys.lock().unwrap().push(apikey(&headers));
    let query = body["query"].as_str().unwrap_or_default().to_string();
    state.graphql_queries.lock().unwrap().push(query.clone());
    if query.contains("mods(") {
        return Json(json!({ "data": { "mods": { "totalCount": 1, "nodes": [{
            "modId": MOD_ID, "name": "Cool Mod", "summary": "A stub", "version": "1.2.0",
            "author": "Stubber", "uploader": { "name": "Stubber" }, "pictureUrl": null,
            "thumbnailUrl": null, "endorsements": 3, "downloads": 40, "fileSize": 4096,
            "adultContent": false,
            "createdAt": "2026-01-01T00:00:00Z", "updatedAt": "2026-02-01T00:00:00Z", "category": "Pals"
        }] } } }))
        .into_response();
    }
    let files = state.files.lock().unwrap();
    let mut data = serde_json::Map::new();
    for piece in query.split("modFiles(modId: \"").skip(1) {
        if let Some(id) = piece
            .split('"')
            .next()
            .and_then(|id| id.parse::<u32>().ok())
        {
            data.insert(
                format!("m{id}"),
                files.get(&id).cloned().unwrap_or_else(|| json!([])),
            );
        }
    }
    Json(json!({ "data": data })).into_response()
}

async fn cdn(State(state): State<Arc<StubState>>) -> Response {
    state.cdn_hits.fetch_add(1, Ordering::SeqCst);
    state.archive.clone().into_response()
}
