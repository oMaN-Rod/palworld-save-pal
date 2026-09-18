use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, USER_AGENT};
use reqwest::StatusCode;
use serde_json::{json, Value};

use ps_core::nexus::{
    self, Account, Category, DownloadLink, GraphqlResponse, ModFile, ModsPage, RateLimit,
    SearchData, SearchParams,
};

use super::keystore::ApiKey;
use crate::services::mods::frameworks::source::Progress;

pub const API_BASE: &str = "https://api.nexusmods.com";
pub const MAX_DOWNLOAD: u64 = 2 * 1024 * 1024 * 1024;
const MAX_JSON_BYTES: u64 = 16 * 1024 * 1024;
const API_TIMEOUT: Duration = Duration::from_secs(30);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const CHUNK_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone, PartialEq, Eq)]
pub struct NxmAuth {
    pub key: String,
    pub expires: u64,
}

impl std::fmt::Debug for NxmAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NxmAuth")
            .field("key", &"<redacted>")
            .field("expires", &self.expires)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NexusError {
    #[error("Nexus Mods did not accept the API key")]
    InvalidKey,
    #[error("downloading directly needs a Nexus Mods Premium account")]
    PremiumRequired,
    #[error("the download link does not match this file")]
    InvalidLink,
    #[error("the download link has expired")]
    LinkExpired,
    #[error("Nexus Mods has no such mod or file")]
    NotFound,
    #[error("the Nexus Mods request limit is used up")]
    RateLimited { reset: Option<String> },
    #[error("Nexus Mods answered {status}: {message}")]
    Api { status: u16, message: String },
    #[error("could not reach Nexus Mods: {0}")]
    Network(String),
    #[error("downloads are limited to {0} bytes")]
    TooLarge(u64),
    #[error("{0}")]
    Io(String),
}

impl NexusError {
    pub fn code(&self) -> &'static str {
        match self {
            NexusError::InvalidKey => "invalid_key",
            NexusError::PremiumRequired => "premium_required",
            NexusError::InvalidLink => "invalid_link",
            NexusError::LinkExpired => "link_expired",
            NexusError::NotFound => "not_found",
            NexusError::RateLimited { .. } => "rate_limited",
            NexusError::Api { .. } => "nexus_error",
            NexusError::Network(_) => "network",
            NexusError::TooLarge(_) => "download_too_large",
            NexusError::Io(_) => "io",
        }
    }

    pub fn detail(&self) -> Value {
        match self {
            NexusError::RateLimited { reset } => json!({ "reset": reset }),
            NexusError::Api { status, .. } => json!({ "status": status }),
            _ => json!({}),
        }
    }
}

#[async_trait]
pub trait NexusApi: Send + Sync {
    async fn validate(&self, key: &ApiKey) -> Result<Account, NexusError>;
    async fn categories(&self, key: &ApiKey) -> Result<Vec<Category>, NexusError>;
    async fn search(&self, params: &SearchParams) -> Result<ModsPage, NexusError>;
    async fn mod_files(&self, mod_ids: &[u32]) -> Result<BTreeMap<u32, Vec<ModFile>>, NexusError>;
    async fn download_link(
        &self,
        key: &ApiKey,
        mod_id: u32,
        file_id: u32,
        nxm: Option<&NxmAuth>,
    ) -> Result<String, NexusError>;
    async fn download(
        &self,
        url: &str,
        dest: &Path,
        progress: Progress<'_>,
    ) -> Result<u64, NexusError>;
    fn rate_limit(&self) -> Option<RateLimit>;
}

fn app_headers() -> HeaderMap {
    let version = env!("CARGO_PKG_VERSION");
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("application-name"),
        HeaderValue::from_static("PalStudio"),
    );
    headers.insert(
        HeaderName::from_static("application-version"),
        HeaderValue::from_static(version),
    );
    headers.insert(
        USER_AGENT,
        HeaderValue::from_str(&format!("PalStudio/{version}")).expect("valid header value"),
    );
    headers
}

pub struct HttpNexusApi {
    api: reqwest::Client,
    download: reqwest::Client,
    v1_base: String,
    graphql_url: String,
    allow_http: bool,
    max_download: u64,
    rate_limit: Mutex<Option<RateLimit>>,
}

impl HttpNexusApi {
    pub fn production() -> Self {
        Self::new(API_BASE)
    }

    pub fn new(api_base: &str) -> Self {
        let api_base = api_base.trim_end_matches('/');
        let api = reqwest::Client::builder()
            .default_headers(app_headers())
            .timeout(API_TIMEOUT)
            // A cross-host redirect keeps the `apikey` header, so following one
            // would hand the user's key to another host. Treat any 3xx as an
            // ordinary unexpected status instead.
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("nexus api client");
        let download = reqwest::Client::builder()
            .default_headers(app_headers())
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .expect("nexus download client");
        Self {
            api,
            download,
            v1_base: format!("{api_base}/v1"),
            graphql_url: format!("{api_base}/v2/graphql"),
            allow_http: api_base.starts_with("http://"),
            max_download: MAX_DOWNLOAD,
            rate_limit: Mutex::new(None),
        }
    }

    pub fn with_max_download(mut self, max_download: u64) -> Self {
        self.max_download = max_download;
        self
    }

    fn record(&self, headers: &HeaderMap) -> Option<RateLimit> {
        let limit = RateLimit::from_headers(|name| {
            headers.get(name).and_then(|value| value.to_str().ok())
        })?;
        *self
            .rate_limit
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(limit.clone());
        Some(limit)
    }

    async fn v1_get(
        &self,
        key: &ApiKey,
        path: &str,
        query: &[(&str, String)],
        premium_route: bool,
    ) -> Result<Value, NexusError> {
        let mut apikey = HeaderValue::from_str(key.expose()).map_err(|_| NexusError::InvalidKey)?;
        apikey.set_sensitive(true);
        let response = self
            .api
            .get(format!("{}{path}", self.v1_base))
            .header("apikey", apikey)
            .query(query)
            .send()
            .await
            .map_err(network)?;
        let limit = self.record(response.headers());
        let status = response.status();
        let body = read_capped(response, MAX_JSON_BYTES).await?;
        if !status.is_success() {
            return Err(v1_error(status, &body, limit.as_ref(), premium_route));
        }
        serde_json::from_slice(&body).map_err(|error| unexpected(status, error))
    }

    async fn graphql(&self, body: &Value) -> Result<Value, NexusError> {
        let response = self
            .api
            .post(&self.graphql_url)
            .json(body)
            .send()
            .await
            .map_err(network)?;
        let limit = self.record(response.headers());
        let status = response.status();
        let bytes = read_capped(response, MAX_JSON_BYTES).await?;
        if status == StatusCode::TOO_MANY_REQUESTS {
            return Err(rate_limited(limit.as_ref()));
        }
        if !status.is_success() {
            return Err(NexusError::Api {
                status: status.as_u16(),
                message: nexus::error_message(&bytes).unwrap_or_else(|| reason(status)),
            });
        }
        let parsed: GraphqlResponse<Value> =
            serde_json::from_slice(&bytes).map_err(|error| unexpected(status, error))?;
        match parsed.data {
            Some(data) if !data.is_null() => Ok(data),
            _ => Err(NexusError::Api {
                status: status.as_u16(),
                message: parsed
                    .errors
                    .into_iter()
                    .next()
                    .map(|error| error.message)
                    .unwrap_or_else(|| "Nexus Mods returned no data".to_string()),
            }),
        }
    }

    async fn download_into(
        &self,
        url: &str,
        part: &Path,
        progress: Progress<'_>,
    ) -> Result<u64, NexusError> {
        use tokio::io::AsyncWriteExt;

        // A CDN that accepts the connection but never sends response headers
        // would otherwise hang the handler forever: the client only has a
        // connect timeout, not a response timeout.
        let mut response =
            match tokio::time::timeout(CHUNK_TIMEOUT, self.download.get(url).send()).await {
                Ok(Ok(response)) => response,
                Ok(Err(error)) => return Err(network(error)),
                Err(_) => return Err(NexusError::Network("the download stalled".to_string())),
            };
        let status = response.status();
        if !status.is_success() {
            return Err(NexusError::Api {
                status: status.as_u16(),
                message: "the download server refused the file".to_string(),
            });
        }
        let total = response.content_length();
        if total.is_some_and(|length| length > self.max_download) {
            return Err(NexusError::TooLarge(self.max_download));
        }
        let mut file = tokio::fs::File::create(part).await.map_err(io)?;
        let mut received: u64 = 0;
        loop {
            let chunk = match tokio::time::timeout(CHUNK_TIMEOUT, response.chunk()).await {
                Ok(Ok(Some(chunk))) => chunk,
                Ok(Ok(None)) => break,
                Ok(Err(error)) => return Err(network(error)),
                Err(_) => return Err(NexusError::Network("the download stalled".to_string())),
            };
            received += chunk.len() as u64;
            if received > self.max_download {
                return Err(NexusError::TooLarge(self.max_download));
            }
            file.write_all(&chunk).await.map_err(io)?;
            progress(received, total);
        }
        file.flush().await.map_err(io)?;
        drop(file);
        Ok(received)
    }
}

#[async_trait]
impl NexusApi for HttpNexusApi {
    async fn validate(&self, key: &ApiKey) -> Result<Account, NexusError> {
        let value = self.v1_get(key, "/users/validate.json", &[], false).await?;
        nexus::parse_account(value).map_err(|error| unexpected(StatusCode::OK, error))
    }

    async fn categories(&self, key: &ApiKey) -> Result<Vec<Category>, NexusError> {
        let path = format!("/games/{}.json", nexus::GAME_DOMAIN);
        let value = self.v1_get(key, &path, &[], false).await?;
        nexus::categories_from_game(value).map_err(|error| unexpected(StatusCode::OK, error))
    }

    async fn search(&self, params: &SearchParams) -> Result<ModsPage, NexusError> {
        let data = self.graphql(&nexus::search_body(params)).await?;
        serde_json::from_value::<SearchData>(data)
            .map(|data| data.mods)
            .map_err(|error| unexpected(StatusCode::OK, error))
    }

    async fn mod_files(&self, mod_ids: &[u32]) -> Result<BTreeMap<u32, Vec<ModFile>>, NexusError> {
        let ids: Vec<u32> = mod_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        let data = self.graphql(&nexus::mod_files_body(&ids)).await?;
        nexus::mod_files_from_data(data, &ids).map_err(|error| unexpected(StatusCode::OK, error))
    }

    async fn download_link(
        &self,
        key: &ApiKey,
        mod_id: u32,
        file_id: u32,
        nxm: Option<&NxmAuth>,
    ) -> Result<String, NexusError> {
        let mut query = Vec::new();
        if let Some(nxm) = nxm {
            query.push(("key", nxm.key.clone()));
            query.push(("expires", nxm.expires.to_string()));
        }
        let path = format!(
            "/games/{}/mods/{mod_id}/files/{file_id}/download_link.json",
            nexus::GAME_DOMAIN
        );
        let value = self.v1_get(key, &path, &query, true).await?;
        let links: Vec<DownloadLink> =
            serde_json::from_value(value).map_err(|error| unexpected(StatusCode::OK, error))?;
        links
            .into_iter()
            .map(|link| link.uri)
            .find(|uri| !uri.is_empty())
            .ok_or(NexusError::NotFound)
    }

    async fn download(
        &self,
        url: &str,
        dest: &Path,
        progress: Progress<'_>,
    ) -> Result<u64, NexusError> {
        let allowed =
            url.starts_with("https://") || (self.allow_http && url.starts_with("http://"));
        if !allowed {
            return Err(NexusError::Network(
                "refusing a download link that is not https".to_string(),
            ));
        }
        let part = part_path(dest);
        match self.download_into(url, &part, progress).await {
            Ok(total) => match tokio::fs::rename(&part, dest).await {
                Ok(()) => Ok(total),
                Err(error) => {
                    let _ = tokio::fs::remove_file(&part).await;
                    Err(io(error))
                }
            },
            Err(error) => {
                let _ = tokio::fs::remove_file(&part).await;
                Err(error)
            }
        }
    }

    fn rate_limit(&self) -> Option<RateLimit> {
        self.rate_limit
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

fn network(error: reqwest::Error) -> NexusError {
    NexusError::Network(error.without_url().to_string())
}

fn io(error: std::io::Error) -> NexusError {
    NexusError::Io(error.to_string())
}

fn reason(status: StatusCode) -> String {
    status
        .canonical_reason()
        .unwrap_or("unexpected status")
        .to_string()
}

fn unexpected(status: StatusCode, error: serde_json::Error) -> NexusError {
    NexusError::Api {
        status: status.as_u16(),
        message: format!("unexpected response: {error}"),
    }
}

fn rate_limited(limit: Option<&RateLimit>) -> NexusError {
    NexusError::RateLimited {
        reset: limit
            .and_then(RateLimit::reset_after_exhaustion)
            .map(str::to_string),
    }
}

fn v1_error(
    status: StatusCode,
    body: &[u8],
    limit: Option<&RateLimit>,
    premium_route: bool,
) -> NexusError {
    match status {
        StatusCode::UNAUTHORIZED => NexusError::InvalidKey,
        StatusCode::FORBIDDEN if premium_route => NexusError::PremiumRequired,
        StatusCode::BAD_REQUEST => NexusError::InvalidLink,
        StatusCode::GONE => NexusError::LinkExpired,
        StatusCode::NOT_FOUND => NexusError::NotFound,
        StatusCode::TOO_MANY_REQUESTS => rate_limited(limit),
        other => NexusError::Api {
            status: other.as_u16(),
            message: nexus::error_message(body).unwrap_or_else(|| reason(other)),
        },
    }
}

async fn read_capped(mut response: reqwest::Response, max: u64) -> Result<Vec<u8>, NexusError> {
    if response.content_length().is_some_and(|length| length > max) {
        return Err(NexusError::TooLarge(max));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(network)? {
        if body.len() as u64 + chunk.len() as u64 > max {
            return Err(NexusError::TooLarge(max));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}.part", uuid::Uuid::new_v4().simple()));
    dest.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::{Path as AxumPath, Query, State};
    use axum::http::{HeaderMap as AxumHeaders, StatusCode as AxumStatus};
    use axum::response::IntoResponse;
    use axum::routing::{get, post};
    use axum::Json;
    use std::collections::HashMap;
    use std::sync::Arc;

    async fn spawn(router: axum::Router) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        format!("http://{addr}")
    }

    fn key(text: &str) -> ApiKey {
        ApiKey::parse(text).unwrap()
    }

    type Seen = Arc<std::sync::Mutex<Vec<AxumHeaders>>>;

    const RATE_HEADERS: [(&str, &str); 6] = [
        ("x-rl-hourly-limit", "500"),
        ("x-rl-hourly-remaining", "0"),
        ("x-rl-hourly-reset", "2026-09-16T21:00:00+00:00"),
        ("x-rl-daily-limit", "20000"),
        ("x-rl-daily-remaining", "19999"),
        ("x-rl-daily-reset", "2026-09-17T00:00:00+00:00"),
    ];

    #[tokio::test]
    async fn validate_sends_the_key_and_app_headers_and_records_the_rate_limit() {
        let seen: Seen = Default::default();
        let router = axum::Router::new()
            .route(
                "/v1/users/validate.json",
                get(|State(seen): State<Seen>, headers: AxumHeaders| async move {
                    seen.lock().unwrap().push(headers);
                    (
                        RATE_HEADERS,
                        Json(serde_json::json!({
                            "user_id": 7, "key": "k-123", "name": "Tester", "is_premium?": false,
                            "is_premium": true, "is_supporter": true,
                            "email": "t@example.invalid", "profile_url": "https://example.invalid/u/7"
                        })),
                    )
                }),
            )
            .with_state(seen.clone());
        let api = HttpNexusApi::new(&spawn(router).await);

        let account = api.validate(&key("k-123")).await.unwrap();
        assert_eq!(
            (account.user_id, account.name.as_str(), account.is_premium),
            (7, "Tester", true)
        );
        let headers = seen.lock().unwrap()[0].clone();
        assert_eq!(headers["apikey"], "k-123");
        assert_eq!(headers["application-name"], "PalStudio");
        assert_eq!(headers["application-version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(
            headers["user-agent"].to_str().unwrap(),
            format!("PalStudio/{}", env!("CARGO_PKG_VERSION"))
        );
        assert_eq!(api.rate_limit().unwrap().daily_remaining, Some(19999));
    }

    #[tokio::test]
    async fn a_rejected_key_is_invalid_key() {
        let router = axum::Router::new().route(
            "/v1/users/validate.json",
            get(|| async {
                (
                    AxumStatus::UNAUTHORIZED,
                    Json(serde_json::json!({ "message": "Please provide a valid API Key" })),
                )
            }),
        );
        let api = HttpNexusApi::new(&spawn(router).await);
        assert_eq!(
            api.validate(&key("nope")).await.unwrap_err(),
            NexusError::InvalidKey
        );
    }

    #[tokio::test]
    async fn a_403_outside_download_link_is_not_premium_required() {
        let router = axum::Router::new().route(
            "/v1/users/validate.json",
            get(|| async {
                (
                    AxumStatus::FORBIDDEN,
                    Json(serde_json::json!({ "message": "forbidden" })),
                )
            }),
        );
        let api = HttpNexusApi::new(&spawn(router).await);
        let error = api.validate(&key("k")).await.unwrap_err();
        assert_eq!(error.code(), "nexus_error", "{error}");
    }

    #[tokio::test]
    async fn categories_come_from_the_game_record() {
        let router = axum::Router::new().route(
            "/v1/games/palworld.json",
            get(|| async {
                Json(serde_json::json!({ "categories": [
                    { "category_id": 1, "name": "Palworld", "parent_category": false },
                    { "category_id": 10, "name": "Pals", "parent_category": 1 }
                ] }))
            }),
        );
        let api = HttpNexusApi::new(&spawn(router).await);
        let categories = api.categories(&key("k")).await.unwrap();
        assert_eq!(categories.len(), 2);
        assert_eq!(categories[1].parent_category, Some(1));
    }

    #[tokio::test]
    async fn graphql_calls_never_carry_the_key_and_batch_mod_files() {
        type SeenGraphql = Arc<std::sync::Mutex<Vec<(AxumHeaders, serde_json::Value)>>>;
        let seen: SeenGraphql = Default::default();
        let router = axum::Router::new()
            .route(
                "/v2/graphql",
                post(
                    |State(seen): State<SeenGraphql>,
                     headers: AxumHeaders,
                     Json(body): Json<serde_json::Value>| async move {
                        let query = body["query"].as_str().unwrap_or_default().to_string();
                        seen.lock().unwrap().push((headers, body));
                        if query.contains("mods(") {
                            return Json(serde_json::json!({ "data": { "mods": {
                                "totalCount": 1, "nodes": [{ "modId": 4821, "name": "Cool Mod" }]
                            } } }));
                        }
                        Json(serde_json::json!({ "data": {
                            "m4821": [{ "fileId": 99001, "name": "Cool Mod", "version": "1.2.0",
                                        "category": "MAIN", "sizeInBytes": "12", "uri": "c.zip", "primary": 1 }],
                            "m7": []
                        } }))
                    },
                ),
            )
            .with_state(seen.clone());
        let api = HttpNexusApi::new(&spawn(router).await);

        let page = api
            .search(&SearchParams {
                query: Some("cool".to_string()),
                category: None,
                sort: None,
                offset: 0,
                count: 10,
                include_adult: false,
            })
            .await
            .unwrap();
        assert_eq!(page.nodes[0].mod_id, 4821);
        let files = api.mod_files(&[4821, 7, 4821]).await.unwrap();
        assert_eq!(files[&4821][0].file_id, 99001);
        assert!(files[&7].is_empty());
        assert!(api.mod_files(&[]).await.unwrap().is_empty());

        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2, "an empty id list makes no request");
        for (headers, _) in seen.iter() {
            assert!(headers.get("apikey").is_none());
            assert_eq!(headers["application-name"], "PalStudio");
        }
        assert_eq!(
            seen[0].1["variables"]["filter"]["gameId"][0]["value"],
            "6063"
        );
    }

    #[tokio::test]
    async fn graphql_errors_without_data_are_nexus_errors() {
        let router = axum::Router::new().route(
            "/v2/graphql",
            post(|| async {
                Json(
                    serde_json::json!({ "errors": [{ "message": "Field 'bogus' doesn't exist" }] }),
                )
            }),
        );
        let api = HttpNexusApi::new(&spawn(router).await);
        let error = api.mod_files(&[1]).await.unwrap_err();
        assert_eq!(error.code(), "nexus_error");
        assert!(error.to_string().contains("bogus"), "{error}");
    }

    #[tokio::test]
    async fn download_link_statuses_map_to_codes() {
        type SeenQueries = Arc<std::sync::Mutex<Vec<HashMap<String, String>>>>;
        let queries: SeenQueries = Default::default();
        let router = axum::Router::new()
            .route(
                "/v1/games/palworld/mods/{mod_id}/files/{file_id}/download_link.json",
                get(
                    |State(queries): State<SeenQueries>,
                     AxumPath((mod_id, file_id)): AxumPath<(u16, u32)>,
                     Query(query): Query<HashMap<String, String>>| async move {
                        queries.lock().unwrap().push(query);
                        let body = Json(serde_json::json!({ "message": "no" }));
                        match mod_id {
                            1 => Json(serde_json::json!([{ "name": "CDN", "short_name": "CDN",
                                "URI": format!("https://cdn.example.invalid/{file_id}.zip") }]))
                            .into_response(),
                            429 => {
                                (AxumStatus::TOO_MANY_REQUESTS, RATE_HEADERS, body).into_response()
                            }
                            status => (AxumStatus::from_u16(status).unwrap(), body).into_response(),
                        }
                    },
                ),
            )
            .with_state(queries.clone());
        let api = HttpNexusApi::new(&spawn(router).await);
        let nxm = NxmAuth {
            key: "nxm-key".to_string(),
            expires: 4_102_444_800,
        };

        assert_eq!(
            api.download_link(&key("k"), 1, 55, Some(&nxm))
                .await
                .unwrap(),
            "https://cdn.example.invalid/55.zip"
        );
        let first = queries.lock().unwrap()[0].clone();
        assert_eq!(first.get("key").map(String::as_str), Some("nxm-key"));
        assert_eq!(first.get("expires").map(String::as_str), Some("4102444800"));
        api.download_link(&key("k"), 1, 55, None).await.unwrap();
        assert!(queries.lock().unwrap()[1].is_empty());

        for (mod_id, code) in [
            (401, "invalid_key"),
            (403, "premium_required"),
            (400, "invalid_link"),
            (410, "link_expired"),
            (404, "not_found"),
            (502, "nexus_error"),
        ] {
            let error = api
                .download_link(&key("k"), mod_id, 1, None)
                .await
                .unwrap_err();
            assert_eq!(error.code(), code, "{mod_id}");
        }
        let limited = api
            .download_link(&key("k"), 429, 1, None)
            .await
            .unwrap_err();
        assert_eq!(
            limited,
            NexusError::RateLimited {
                reset: Some("2026-09-16T21:00:00+00:00".to_string())
            }
        );
        assert_eq!(limited.detail()["reset"], "2026-09-16T21:00:00+00:00");
    }

    #[tokio::test]
    async fn downloads_stream_to_the_destination_with_progress() {
        let body: Vec<u8> = (0..70_000u32).map(|n| (n % 251) as u8).collect();
        let served = body.clone();
        let router = axum::Router::new().route("/cdn/a.zip", get(move || async move { served }));
        let base = spawn(router).await;
        let api = HttpNexusApi::new(&base);
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("a.zip");
        let seen = std::sync::Mutex::new(Vec::new());
        let progress = |got: u64, total: Option<u64>| seen.lock().unwrap().push((got, total));

        let written = api
            .download(&format!("{base}/cdn/a.zip"), &dest, &progress)
            .await
            .unwrap();

        assert_eq!(written, body.len() as u64);
        assert_eq!(std::fs::read(&dest).unwrap(), body);
        assert_eq!(
            seen.lock().unwrap().last().copied(),
            Some((body.len() as u64, Some(body.len() as u64)))
        );
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            1,
            "no .part left behind"
        );
    }

    #[tokio::test]
    async fn an_oversized_or_insecure_download_leaves_nothing() {
        let router = axum::Router::new().route("/cdn/big.zip", get(|| async { vec![7u8; 64] }));
        let base = spawn(router).await;
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("big.zip");
        let capped = HttpNexusApi::new(&base).with_max_download(10);
        let error = capped
            .download(&format!("{base}/cdn/big.zip"), &dest, &|_, _| {})
            .await
            .unwrap_err();
        assert_eq!(error, NexusError::TooLarge(10));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);

        let production_like = HttpNexusApi::new("https://api.example.invalid");
        let refused = production_like
            .download(&format!("{base}/cdn/big.zip"), &dest, &|_, _| {})
            .await
            .unwrap_err();
        assert_eq!(refused.code(), "network");
        assert!(!dest.exists());
    }

    #[tokio::test]
    async fn a_redirect_never_leaks_the_key_to_another_host() {
        let leaked: Seen = Default::default();
        let decoy = axum::Router::new()
            .route(
                "/v1/users/validate.json",
                get(|State(seen): State<Seen>, headers: AxumHeaders| async move {
                    seen.lock().unwrap().push(headers);
                    Json(serde_json::json!({ "user_id": 1, "name": "Decoy", "is_premium": false }))
                }),
            )
            .with_state(leaked.clone());
        let decoy_base = spawn(decoy).await;

        let target = format!("{decoy_base}/v1/users/validate.json");
        let router = axum::Router::new().route(
            "/v1/users/validate.json",
            get(move || {
                let target = target.clone();
                async move {
                    (AxumStatus::FOUND, [(axum::http::header::LOCATION, target)]).into_response()
                }
            }),
        );
        let api = HttpNexusApi::new(&spawn(router).await);

        let error = api.validate(&key("k-123")).await.unwrap_err();
        assert_eq!(error.code(), "nexus_error");
        assert!(
            leaked.lock().unwrap().is_empty(),
            "the redirect must not be followed"
        );
    }
}
