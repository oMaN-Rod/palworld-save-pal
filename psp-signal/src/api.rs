//! The local HTTP API.
//!
//! Endpoints and JSON shapes form the wire contract companion map apps
//! consume:
//!
//! | Route | Methods | Auth | Shape |
//! |---|---|---|---|
//! | `/v1/hello` | GET/HEAD | none | identity/health |
//! | `/v1/live` | GET/HEAD | token | full frame, `actors` never null |
//! | `/v1/server` | GET/POST | token | status card / connect-forget |
//! | `/v1/minimap` | POST | token | claim protocol (`supported:false` on desktop Signal) |
//!
//! Auth accepts `Authorization: Bearer <token>` or `?token=` (compared in
//! constant time), or falls back to loopback + an approved `Origin`, the
//! standard DNS-rebinding posture. CORS is deliberately narrow: only
//! loopback origins and the operator-configured allow-list are echoed, and
//! only on preflight.
use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::Response;
use axum::routing::any;
use axum::Router;
use serde_json::json;

use crate::manager::SignalRuntime;
use crate::model::FeedState;
use crate::token;

/// Shared state handed to the router.
pub struct ApiState {
    pub runtime: Arc<SignalRuntime>,
}

pub fn build_router(state: ApiState) -> Router {
    Router::new()
        .route("/v1/hello", any(hello))
        .route("/v1/live", any(live))
        .route("/v1/server", any(server))
        .route("/v1/minimap", any(minimap))
        .fallback(not_found)
        .with_state(Arc::new(state))
}

fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn send_json(status: StatusCode, body: serde_json::Value) -> Response {
    let mut response = Response::new(Body::from(
        serde_json::to_vec(&body).expect("JSON value serializes"),
    ));
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    no_store(response)
}

fn error_json(status: StatusCode, reason: &str) -> Response {
    send_json(status, json!({"ok": false, "error": reason}))
}

async fn not_found() -> Response {
    error_json(StatusCode::NOT_FOUND, "no such endpoint")
}

/// Extracts the bearer token from `Authorization`.
fn bearer(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let rest = value.strip_prefix("Bearer ").or_else(|| value.strip_prefix("bearer "))?;
    let trimmed = rest.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// True when `origin` is loopback on any port (the local UI/dev servers) or
/// an exact entry of the configured allow-list.
pub fn origin_approved(origin: &str, allowed: &[String]) -> bool {
    if let Some(rest) = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
    {
        let host = rest.split(':').next().unwrap_or("");
        if host == "127.0.0.1" || host == "localhost" || host == "[::1]" {
            return true;
        }
    }
    allowed.iter().any(|entry| entry == origin)
}

fn is_loopback(remote: &str) -> bool {
    let ip = remote.rsplit_once(':').map(|(ip, _)| ip).unwrap_or(remote);
    ip == "127.0.0.1" || ip == "::1" || ip == "[::1]" || ip == "localhost"
}

/// The auth rule: a token that matches, or loopback with a non-empty
/// approved Origin. An empty Origin from loopback is denied, so browsers
/// cannot be tricked into unauthenticated reads.
async fn may_read(state: &ApiState, headers: &HeaderMap, query_token: Option<&str>, remote: &str) -> bool {
    let expected = state.runtime.token().await;
    let candidate = query_token
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .or_else(|| bearer(headers));
    if let Some(candidate) = candidate {
        return token::constant_time_eq(&candidate, &expected);
    }
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let allowed = state.runtime.allowed_origins().await;
    is_loopback(remote) && !origin.is_empty() && origin_approved(origin, &allowed)
}

fn query_token(uri: &axum::http::Uri) -> Option<String> {
    uri.query().and_then(|query| {
        query.split('&').find_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            (key == "token").then(|| value.replace('+', " "))
        })
    })
}

async fn cors_preflight(state: &ApiState, headers: &HeaderMap) -> Response {
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let mut response = Response::new(Body::empty());
    *response.status_mut() = StatusCode::NO_CONTENT;
    let out = response.headers_mut();
    out.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    out.insert(header::VARY, HeaderValue::from_static("Origin"));
    let allowed = state.runtime.allowed_origins().await;
    if origin_approved(origin, &allowed) {
        out.insert(
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            HeaderValue::from_str(origin).unwrap(),
        );
        out.insert(
            header::ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_static("GET, POST, OPTIONS"),
        );
        out.insert(
            header::ACCESS_CONTROL_ALLOW_HEADERS,
            HeaderValue::from_static("Authorization, Content-Type"),
        );
        out.insert(
            header::ACCESS_CONTROL_MAX_AGE,
            HeaderValue::from_static("600"),
        );
        if headers
            .get("access-control-request-private-network")
            .and_then(|value| value.to_str().ok())
            == Some("true")
        {
            out.insert(
                header::HeaderName::from_static("access-control-allow-private-network"),
                HeaderValue::from_static("true"),
            );
        }
    }
    response
}

async fn hello(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    method: Method,
) -> Response {
    if method == Method::OPTIONS {
        return cors_preflight(&state, &headers).await;
    }
    if method != Method::GET && method != Method::HEAD {
        return error_json(StatusCode::METHOD_NOT_ALLOWED, "method");
    }
    let snapshot = state.runtime.snapshot().await;
    let feeding = matches!(
        snapshot.status.state,
        FeedState::Players | FeedState::World | FeedState::Feeding
    );
    send_json(
        StatusCode::OK,
        json!({
            "beacon": crate::WIRE_BUILD,
            "feeding": feeding,
            "name": crate::PRODUCT_NAME,
            "needsToken": true,
            "semver": env!("CARGO_PKG_VERSION"),
            "source": snapshot.source.map(|k| k.as_str()).unwrap_or("none"),
            "version": crate::WIRE_BUILD.to_string(),
        }),
    )
}

async fn live(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    uri: axum::http::Uri,
    method: Method,
    remote: axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Response {
    if method == Method::OPTIONS {
        return cors_preflight(&state, &headers).await;
    }
    if method != Method::GET && method != Method::HEAD {
        return error_json(StatusCode::METHOD_NOT_ALLOWED, "method");
    }
    let remote = remote.0.to_string();
    if !may_read(&state, &headers, query_token(&uri).as_deref(), &remote).await {
        return error_json(StatusCode::FORBIDDEN, "bad or missing token");
    }
    let snapshot = state.runtime.snapshot().await;
    let frame = match snapshot.frame.clone() {
        Some(frame) => frame,
        None => crate::model::LiveFrame::empty(
            false,
            snapshot
                .status
                .kind
                .map(|kind| kind.as_str())
                .unwrap_or("none"),
            "unknown",
        ),
    };
    send_json(StatusCode::OK, serde_json::to_value(&frame).unwrap())
}

async fn server(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    uri: axum::http::Uri,
    method: Method,
    remote: axum::extract::ConnectInfo<std::net::SocketAddr>,
    body: Option<axum::Json<serde_json::Value>>,
) -> Response {
    if method == Method::OPTIONS {
        return cors_preflight(&state, &headers).await;
    }
    let remote = remote.0.to_string();
    if !may_read(&state, &headers, query_token(&uri).as_deref(), &remote).await {
        return error_json(StatusCode::FORBIDDEN, "bad or missing token");
    }
    match method {
        Method::GET | Method::HEAD => {
            let snapshot = state.runtime.snapshot().await;
            send_json(StatusCode::OK, server_card(&snapshot))
        }
        Method::POST => {
            let Some(axum::Json(body)) = body else {
                return error_json(StatusCode::BAD_REQUEST, "body");
            };
            match state.runtime.apply_server_post(&body).await {
                Ok(snapshot) => send_json(StatusCode::OK, server_card(&snapshot)),
                Err(reason) => {
                    let snapshot = state.runtime.snapshot().await;
                    let mut card = server_card(&snapshot);
                    card["ok"] = json!(false);
                    card["error"] = json!(reason);
                    send_json(StatusCode::OK, card)
                }
            }
        }
        _ => error_json(StatusCode::METHOD_NOT_ALLOWED, "method"),
    }
}

/// The `/v1/server` card. The AdminPassword is never on this wire.
fn server_card(snapshot: &crate::manager::Snapshot) -> serde_json::Value {
    json!({
        "ok": true,
        "actors": snapshot.status.actor_count,
        "age": snapshot.status.last_ok_age.unwrap_or(0.0),
        "configured": snapshot.source.is_some(),
        "error": snapshot.status.error,
        "locked": snapshot.source_locked,
        "passwordSet": snapshot.password_set,
        "source": snapshot.status.kind.map(|k| k.as_str()).unwrap_or(""),
        "stale": snapshot.status.state == FeedState::Stale,
        "state": snapshot.status.state.as_str(),
        "url": snapshot.source_url.clone().unwrap_or_default(),
    })
}

async fn minimap(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    uri: axum::http::Uri,
    method: Method,
    remote: axum::extract::ConnectInfo<std::net::SocketAddr>,
    body: Option<axum::Json<serde_json::Value>>,
) -> Response {
    if method == Method::OPTIONS {
        return cors_preflight(&state, &headers).await;
    }
    let remote = remote.0.to_string();
    if !may_read(&state, &headers, query_token(&uri).as_deref(), &remote).await {
        return error_json(StatusCode::FORBIDDEN, "bad or missing token");
    }
    if method != Method::POST {
        return error_json(StatusCode::METHOD_NOT_ALLOWED, "method");
    }
    let Some(axum::Json(body)) = body else {
        return error_json(StatusCode::BAD_REQUEST, "body");
    };
    let open = body.get("open").and_then(|value| value.as_bool());
    let Some(open) = open else {
        return error_json(StatusCode::BAD_REQUEST, "body");
    };
    let title = body
        .get("title")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if open && title.is_empty() {
        return error_json(StatusCode::BAD_REQUEST, "a claim needs a window title");
    }
    if open {
        state.runtime.claim_minimap(title).await;
    } else {
        state.runtime.release_minimap().await;
    }
    let active = state.runtime.minimap_active().await;
    send_json(
        StatusCode::OK,
        json!({"ok": true, "supported": false, "active": active}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_origins_on_any_port_are_approved_others_are_not() {
        let allowed = vec!["https://maps.example.com".to_string()];
        assert!(origin_approved("http://127.0.0.1:5173", &allowed));
        assert!(origin_approved("http://localhost:5173", &allowed));
        assert!(origin_approved("https://maps.example.com", &allowed));
        assert!(!origin_approved("https://evil.example", &allowed));
        assert!(!origin_approved("", &allowed));
        assert!(!origin_approved("http://192.168.1.10:8788", &allowed));
    }

    #[test]
    fn loopback_detection_handles_ipv4_ipv6_and_port_suffixes() {
        assert!(is_loopback("127.0.0.1:51734"));
        assert!(is_loopback("[::1]:51734"));
        assert!(!is_loopback("192.168.1.10:51734"));
    }
}
