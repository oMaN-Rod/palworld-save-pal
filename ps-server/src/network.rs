//! PalStudio's own network policy runtime: config persistence, request
//! enforcement, PIN sessions, and the Network page's REST API.
//!
//! Layering with the pure `ps-network` crate: that one decides (config,
//! peer) → verdict; this one owns the live config, persists it in the `meta`
//! table, evaluates every inbound HTTP/WS request, and applies edits from
//! the UI (including tailscale funnel toggling and optional UPnP mapping).
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use axum::extract::{Request, State};
use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::{Json, Router};
use ps_network::auth::SessionRegistry;
use ps_network::{AuthScope, ListenMode, NetworkConfig, NetworkTier, PeerAcl as Verdict};

use crate::network_policy::{ConnectionAcl, NetworkPolicy};
use crate::AppState;

pub const META_KEY: &str = "network_config";
pub const SESSION_COOKIE: &str = "ps_network_session";

/// How install-time env (`PS_LISTEN`, `PS_PORT`, `PS_PIN`) interacts with
/// the stored config: `firstboot` (default) seeds a fresh install once;
/// `always` re-applies on every boot — the Docker posture, where the
/// container environment is the operator's source of truth.
const ENV_MODE: &str = "PS_NETWORK_ENV";

/// Wrong-PIN delay: PBKDF2 already costs ~200ms, this keeps online guessing
/// uninspiring even if iterations are ever lowered.
const FAILED_PIN_DELAY: Duration = Duration::from_millis(400);

pub struct NetworkRuntime {
    config: RwLock<NetworkConfig>,
    tier: NetworkTier,
    pub sessions: SessionRegistry,
    restart: tokio::sync::Notify,
    restart_flag: Arc<AtomicBool>,
    exit: tokio::sync::Notify,
    exit_flag: Arc<AtomicBool>,
}

impl NetworkRuntime {
    /// Hosted by default — the permissive historical behavior that the
    /// router-level tests were written against.
    pub fn new(config: NetworkConfig) -> Self {
        Self::with_tier(config, NetworkTier::Hosted)
    }

    pub fn with_tier(config: NetworkConfig, tier: NetworkTier) -> Self {
        NetworkRuntime {
            config: RwLock::new(config),
            tier,
            sessions: SessionRegistry::default(),
            restart: tokio::sync::Notify::new(),
            restart_flag: Arc::new(AtomicBool::new(false)),
            exit: tokio::sync::Notify::new(),
            exit_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn tier(&self) -> NetworkTier {
        self.tier
    }

    /// Builder used at boot: the loaded policy keeps its stored bytes, only
    /// the enforcement tier is attached.
    pub fn into_tier(self, tier: NetworkTier) -> Self {
        NetworkRuntime { tier, ..self }
    }

    /// The policy as enforced: a hand-launched local webapp sees the stored
    /// config clamped to localhost-only (the stored one is preserved for a
    /// later hosted run).
    pub fn effective_config(&self) -> NetworkConfig {
        match self.tier {
            NetworkTier::LocalWebapp => self.config().clamped_for_local_webapp(),
            NetworkTier::Desktop | NetworkTier::Hosted => self.config(),
        }
    }

    pub fn config(&self) -> NetworkConfig {
        self.config
            .read()
            .expect("network config lock poisoned")
            .clone()
    }

    pub fn set_config(&self, config: NetworkConfig) {
        *self.config.write().expect("network config lock poisoned") = config;
    }

    pub fn effective_port(&self) -> u16 {
        self.config().port
    }

    pub fn evaluate(&self, peer: IpAddr) -> Verdict {
        ps_network::evaluate(&self.effective_config(), peer)
    }

    /// The flag the server main loops poll after the listener exits; true
    /// means "rebind with the (possibly changed) configured port".
    pub fn restart_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.restart_flag)
    }

    pub(crate) fn exit_flag_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.exit_flag)
    }

    /// Fires the graceful-shutdown path of the running listener so callers
    /// can rebind on the new port.
    pub fn request_restart(&self) {
        self.restart_flag.store(true, Ordering::SeqCst);
        self.restart.notify_waiters();
        self.restart.notify_one();
    }

    /// Ends the whole process loop (runtime-mode switch): the listener stops
    /// and the main loop exits instead of rebinding.
    pub fn request_exit(&self) {
        self.exit_flag.store(true, Ordering::SeqCst);
        self.exit.notify_waiters();
        self.exit.notify_one();
    }

    pub fn exit_requested(&self) -> bool {
        self.exit_flag.load(Ordering::SeqCst)
    }

    pub(crate) async fn restart_wait(&self) {
        self.restart.notified().await;
    }

    pub(crate) async fn exit_wait(&self) {
        self.exit.notified().await;
    }

    /// Loads the policy from the meta table, seeding defaults (and
    /// install-time env, per `PS_NETWORK_ENV`) on first boot.
    pub async fn load(driver: &dyn ps_db::DbDriver) -> anyhow::Result<Self> {
        let stored = ps_db::meta::get(driver, META_KEY).await?;
        let always = env_mode_always();
        match stored {
            Some(raw) => {
                let config = NetworkConfig::from_json(&raw)?;
                let config = if always { config.apply_env() } else { config };
                Ok(NetworkRuntime::new(config))
            }
            None => {
                let seeded = NetworkConfig::default().apply_env();
                ps_db::meta::set(driver, META_KEY, &seeded.to_json()).await?;
                Ok(NetworkRuntime::new(seeded))
            }
        }
    }

    pub async fn save(&self, driver: &dyn ps_db::DbDriver) -> anyhow::Result<()> {
        ps_db::meta::set(driver, META_KEY, &self.config().to_json()).await?;
        Ok(())
    }

    fn cookie_token(&self, headers: &axum::http::HeaderMap) -> Option<String> {
        let cookie = headers.get(header::COOKIE)?.to_str().ok()?;
        cookie.split(';').find_map(|part| {
            let part = part.trim();
            part.strip_prefix(SESSION_COOKIE)
                .map(|rest| rest.trim_start_matches('=').trim().to_owned())
        })
    }

    pub fn session_valid(&self, headers: &axum::http::HeaderMap) -> bool {
        self.cookie_token(headers)
            .is_some_and(|token| self.sessions.is_valid(&token))
    }
}

impl NetworkPolicy for NetworkRuntime {
    fn acl_for(&self, peer: IpAddr) -> ConnectionAcl {
        let verdict = self.evaluate(peer);
        ConnectionAcl {
            can_connect: verdict.can_connect,
            can_write: verdict.can_write,
            auth_required: verdict.auth_required,
        }
    }

    fn has_valid_session(&self, token: &str) -> bool {
        self.sessions.is_valid(token)
    }
}

/// The outermost request gate: listen mode, allowlists, PIN session, and
/// write permission for mutating HTTP routes. Websocket write permission is
/// additionally stamped per-connection in `ws.rs` and enforced in the
/// dispatcher; this layer still guards the upgrade itself.
pub async fn network_gate(
    State(runtime): State<Arc<NetworkRuntime>>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_owned();
    let method = request.method().clone();

    // The unlock page and the session endpoint must work while locked,
    // otherwise nobody could ever present the PIN.
    if path == "/network-unlock" || path == "/api/network/session" {
        return next.run(request).await;
    }

    let peer = request
        .extensions()
        .get::<axum::extract::ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip());
    let Some(peer) = peer else {
        // No ConnectInfo means no real socket peer (unit-level plumbing);
        // refuse rather than guess.
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "client address unavailable",
        );
    };

    let verdict = runtime.evaluate(peer);
    if !verdict.can_connect {
        tracing::warn!(%peer, %path, "connection refused by network policy");
        return error_response(
            StatusCode::FORBIDDEN,
            "refused: your address is not allowed to connect to this PalStudio instance",
        );
    }
    if verdict.auth_required && !runtime.session_valid(request.headers()) {
        tracing::info!(%peer, %path, "PIN session required");
        // Machine clients (SPA fetches, websocket upgrades) get the 401 they
        // can detect; human navigations are bounced to the self-contained
        // unlock page so no SPA bytes reach a locked browser.
        let machine_client = path.starts_with("/api/") || path.starts_with("/ws");
        if machine_client || !matches!(method, Method::GET | Method::HEAD) {
            return error_response(StatusCode::UNAUTHORIZED, "pin required");
        }
        let next = request
            .uri()
            .path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or("/");
        return Redirect::to(&format!(
            "/network-unlock?next={}",
            encode_query_component(next)
        ))
        .into_response();
    }
    let is_write_method = !matches!(method, Method::GET | Method::HEAD | Method::OPTIONS);
    if is_write_method && path.starts_with("/api/") && !verdict.can_write {
        tracing::warn!(%peer, %method, %path, "write refused by network policy");
        return error_response(
            StatusCode::FORBIDDEN,
            "read-only access: your address may view but not modify this PalStudio instance",
        );
    }
    next.run(request).await
}

pub(crate) fn error_response(status: StatusCode, message: &str) -> Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}

/// Percent-encodes a path (with optional query) for use as a query-string
/// value; '/' stays readable, everything outside the unreserved set escapes.
fn encode_query_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Standing posture warnings for a config: things that are legal to save but
/// leave the instance exposed or broken for non-loopback peers. Surfaced at
/// boot (log), on save (response warnings), and mirrored client-side by the
/// Network page's banner.
pub(crate) fn security_warnings(config: &NetworkConfig) -> Vec<String> {
    let mut warnings = Vec::new();
    if config.listen != ListenMode::Localhost && config.auth.scope == AuthScope::Never {
        warnings.push(format!(
            "listening on '{}' without a PIN — {} can connect AND edit saves; \
             set a PIN (PS_PIN or the Network page)",
            config.listen.as_str(),
            ps_network::default_audience(config.listen)
        ));
    }
    if config.auth.scope != AuthScope::Never && config.auth.pin.is_none() {
        warnings.push(
            "auth is on but no PIN is set — non-loopback peers are refused until one is".into(),
        );
    }
    warnings
}

/// True when the environment re-applies `PS_LISTEN`/`PS_PORT`/`PS_PIN` on
/// every boot (the Docker posture set by docker-compose.yml).
fn env_mode_always() -> bool {
    std::env::var(ENV_MODE).is_ok_and(|mode| mode.trim().eq_ignore_ascii_case("always"))
}

// ---------------------------------------------------------------------------
// REST API
// ---------------------------------------------------------------------------

/// Redacted view for the UI: shape mirrors NetworkConfig but the PIN hash
/// never leaves the process — only whether one is set.
#[derive(serde::Serialize)]
pub struct NetworkConfigDto {
    pub listen: ListenMode,
    pub port: u16,
    pub allow: ps_network::AllowRules,
    pub auth: AuthDto,
    pub upnp_enabled: bool,
    pub funnel_enabled: bool,
}

#[derive(serde::Serialize)]
pub struct AuthDto {
    pub scope: AuthScope,
    pub pin_set: bool,
    pub session_ttl_secs: u64,
}

/// Edit payload from the Network page. Omitted pins keep the stored hash;
/// `Some("")` clears it.
#[derive(serde::Deserialize)]
pub struct NetworkConfigUpdate {
    pub listen: ListenMode,
    pub port: u16,
    #[serde(default)]
    pub allow: ps_network::AllowRules,
    pub auth: AuthUpdate,
    #[serde(default)]
    pub upnp_enabled: bool,
    #[serde(default)]
    pub funnel_enabled: bool,
}

#[derive(serde::Deserialize)]
pub struct AuthUpdate {
    pub scope: AuthScope,
    #[serde(default)]
    pub new_pin: Option<String>,
    #[serde(default = "default_session_ttl")]
    pub session_ttl_secs: u64,
}

fn default_session_ttl() -> u64 {
    12 * 60 * 60
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/config", get(get_config).put(put_config).post(put_config))
        .route(
            "/session",
            axum::routing::post(create_session).delete(delete_session),
        )
        .route("/status", get(get_status))
}

async fn get_config(axum::Extension(runtime): axum::Extension<Arc<NetworkRuntime>>) -> Response {
    (
        StatusCode::OK,
        Json(
            serde_json::to_value(&redact(&runtime.effective_config()))
                .map(|mut value| {
                    value["tier"] = serde_json::json!(runtime.tier().as_str());
                    value
                })
                .unwrap_or_else(|_| serde_json::json!({ "error": "serialization failed" })),
        ),
    )
        .into_response()
}

fn redact(config: &NetworkConfig) -> NetworkConfigDto {
    NetworkConfigDto {
        listen: config.listen,
        port: config.port,
        allow: config.allow.clone(),
        auth: AuthDto {
            scope: config.auth.scope,
            pin_set: config.auth.pin.is_some(),
            session_ttl_secs: config.auth.session_ttl_secs,
        },
        upnp_enabled: config.upnp_enabled,
        funnel_enabled: config.funnel_enabled,
    }
}

/// Validates a candidate config beyond serde: port range and parseable
/// CIDR entries in both allowlists.
fn validate(update: &NetworkConfigUpdate) -> Vec<String> {
    let mut errors = Vec::new();
    if update.port == 0 {
        errors.push("port must be between 1 and 65535".into());
    }
    for (label, rules) in [
        ("allow.connect", &update.allow.connect),
        ("allow.write", &update.allow.write),
    ] {
        for entry in rules {
            if ps_network::policy::IpNet::parse(entry).is_none() {
                errors.push(format!(
                    "{label}: '{entry}' is not a valid IP or CIDR range"
                ));
            }
        }
    }
    if let Some(pin) = update.auth.new_pin.as_deref() {
        if pin.chars().count() < 4 && !pin.is_empty() {
            errors.push("new_pin: use at least 4 characters (or empty to clear)".into());
        }
    }
    errors
}

async fn put_config(
    State(app): State<Arc<AppState>>,
    axum::Extension(runtime): axum::Extension<Arc<NetworkRuntime>>,
    Json(update): Json<NetworkConfigUpdate>,
) -> Response {
    if app.config.desktop_mode {
        return error_response(
            StatusCode::FORBIDDEN,
            "the desktop app always runs localhost-only; network settings live in the server/webapp editions",
        );
    }

    // A hand-launched local webapp is a local tool: only the port is
    // editable, everything else stays clamped to localhost. The stored
    // policy is preserved untouched for a later hosted/service run.
    if runtime.tier() == NetworkTier::LocalWebapp {
        let stored = runtime.config();
        if update.listen != ListenMode::Localhost
            || !update.allow.connect.is_empty()
            || !update.allow.write.is_empty()
            || update.auth.scope != AuthScope::Never
            || update.auth.new_pin.is_some_and(|pin| !pin.is_empty())
            || update.upnp_enabled
            || update.funnel_enabled
        {
            return error_response(
                StatusCode::FORBIDDEN,
                "this instance runs as a local webapp (localhost only); only the port is editable — \
                 run it as a background service or `palstudio host` for full network settings",
            );
        }
        let port_changed = update.port != stored.port;
        let mut merged = stored;
        merged.port = update.port;
        runtime.set_config(merged);
        if let Err(error) = runtime.save(&*app.driver).await {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("could not persist network config: {error}"),
            );
        }
        if port_changed {
            tracing::info!(
                new = update.port,
                "local webapp port changed; requesting rebind"
            );
            runtime.request_restart();
        }
        let mut dto = serde_json::to_value(&redact(&runtime.effective_config()))
            .unwrap_or_else(|_| serde_json::json!({}));
        dto["tier"] = serde_json::json!(runtime.tier().as_str());
        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "config": dto,
                "restart_required": port_changed,
                "warnings": Vec::<String>::new(),
            })),
        )
            .into_response();
    }

    let errors = validate(&update);
    if !errors.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": errors.join("; ") })),
        )
            .into_response();
    }

    let old = runtime.config();
    let mut merged = NetworkConfig {
        version: old.version,
        listen: update.listen,
        port: update.port,
        allow: update.allow,
        auth: ps_network::AuthConfig {
            scope: update.auth.scope,
            pin: old.auth.pin.clone(),
            session_ttl_secs: update.auth.session_ttl_secs,
        },
        upnp_enabled: update.upnp_enabled,
        funnel_enabled: update.funnel_enabled,
    };
    match update.auth.new_pin.as_deref() {
        Some("") => merged.auth.pin = None,
        Some(pin) => merged.auth.pin = Some(ps_network::PinHash::generate(pin)),
        None => {}
    }

    let mut warnings = security_warnings(&merged);
    if merged.funnel_enabled && merged.auth.scope == AuthScope::Never {
        warnings.push(
            "tailscale funnel forwards arrive via the local tailscale proxy and cannot be \
             IP-filtered; set a PIN before exposing funnel"
                .into(),
        );
    }
    if merged.upnp_enabled {
        warnings
            .push("UPnP opens a port on your router — tailscale is the safer remote path".into());
    }

    // Side effects first; only persist what actually took effect. The
    // decision compares against the LIVE funnel state, not just the stored
    // one: when reality drifted (funnel toggled outside the tool, or a prior
    // save's CLI call failed), saving the same toggle again must still
    // reconcile instead of being a no-op. Without tailscale the probe fails
    // fast and reports "off", which only matters when funnel was requested.
    let live = tokio::task::spawn_blocking(ps_network::tailscale::funnel_probe)
        .await
        .unwrap_or_default();
    let port_suffix = format!(":{}", merged.port);
    let pointing_elsewhere =
        merged.funnel_enabled && !live.targets.iter().any(|t| t.ends_with(&port_suffix));
    if live.on != merged.funnel_enabled || pointing_elsewhere {
        let enable = merged.funnel_enabled;
        let port = merged.port;
        let outcome =
            tokio::task::spawn_blocking(move || ps_network::tailscale::set_funnel(enable, port))
                .await
                .unwrap_or_else(|error| Err(format!("funnel task failed: {error}")));
        if let Err(error) = outcome {
            warnings.push(format!("tailscale funnel could not be updated: {error}"));
            merged.funnel_enabled = old.funnel_enabled;
        }
    }
    if merged.upnp_enabled && !old.upnp_enabled {
        let port = merged.port;
        #[cfg(feature = "upnp")]
        let outcome = ps_network::upnp::map_port(port, "PalStudio").await.err();
        #[cfg(not(feature = "upnp"))]
        let outcome = Some("this build does not include UPnP support".to_owned());
        if let Some(error) = outcome {
            warnings.push(format!("UPnP mapping failed: {error}"));
            merged.upnp_enabled = false;
        }
    } else if !merged.upnp_enabled && old.upnp_enabled {
        #[cfg(feature = "upnp")]
        if let Err(error) = ps_network::upnp::unmap_port(old.port).await {
            warnings.push(format!("could not remove the UPnP mapping: {error}"));
        }
    }

    let port_changed = merged.port != old.port;
    let new_port = merged.port;
    runtime.set_config(merged);
    if let Err(error) = runtime.save(&*app.driver).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("could not persist network config: {error}") })),
        )
            .into_response();
    }

    if port_changed {
        tracing::info!(
            old = old.port,
            new = new_port,
            "network port changed; requesting rebind"
        );
        if env_mode_always() {
            // The container posture: the published port mapping (docker -p /
            // compose ports:) still forwards to the OLD port, so the UI would
            // strand until the mapping matches. PS_PORT in the environment
            // would also re-apply the old port on the next boot.
            warnings.push(
                "port changed in a container-style deployment (PS_NETWORK_ENV=always): update \
                 the published port mapping to match, or set PS_PORT in the environment"
                    .into(),
            );
        }
        runtime.request_restart();
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "config": redact(&runtime.config()),
            "restart_required": port_changed,
            "warnings": warnings,
        })),
    )
        .into_response()
}

#[derive(serde::Deserialize)]
struct SessionRequest {
    pin: String,
}

async fn create_session(
    axum::Extension(runtime): axum::Extension<Arc<NetworkRuntime>>,
    Json(request): Json<SessionRequest>,
) -> Response {
    let config = runtime.config();
    let Some(hash) = config.auth.pin.clone() else {
        return error_response(StatusCode::CONFLICT, "no PIN is configured");
    };
    let ttl = Duration::from_secs(config.auth.session_ttl_secs);
    if !hash.matches(&request.pin) {
        tokio::time::sleep(FAILED_PIN_DELAY).await;
        return error_response(StatusCode::UNAUTHORIZED, "wrong PIN");
    }
    let token = runtime.sessions.issue(ttl);
    let cookie = format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
        config.auth.session_ttl_secs
    );
    let mut response = (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).expect("session cookie is header-safe"),
    );
    response
}

async fn delete_session(
    State(_app): State<Arc<AppState>>,
    axum::Extension(runtime): axum::Extension<Arc<NetworkRuntime>>,
    request: Request,
) -> Response {
    if let Some(token) = cookie_token_from(request.headers()) {
        runtime.sessions.revoke(&token);
    }
    // Clear the cookie regardless so the browser drops it.
    let mut response = (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_static("ps_network_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"),
    );
    response
}

fn cookie_token_from(headers: &axum::http::HeaderMap) -> Option<String> {
    let cookie = headers.get(header::COOKIE)?.to_str().ok()?;
    cookie.split(';').find_map(|part| {
        let part = part.trim();
        part.strip_prefix(SESSION_COOKIE)
            .map(|rest| rest.trim_start_matches('=').trim().to_owned())
    })
}

async fn get_status(
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    let tailscale = tokio::task::spawn_blocking(ps_network::tailscale::detect)
        .await
        .unwrap_or_default();
    let funnel = tokio::task::spawn_blocking(ps_network::tailscale::funnel_probe)
        .await
        .unwrap_or_default();
    let upnp_probe = params.contains_key("probe_upnp");
    #[cfg(feature = "upnp")]
    let upnp_available = if upnp_probe {
        tokio::time::timeout(Duration::from_secs(4), ps_network::upnp::probe())
            .await
            .ok()
            .unwrap_or(false)
    } else {
        false
    };
    #[cfg(not(feature = "upnp"))]
    let upnp_available = false;

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "tailscale": {
                "available": tailscale.available,
                "logged_in": tailscale.logged_in,
                "ipv4": tailscale.ipv4,
            },
            "funnel": {
                "on": funnel.on,
                "urls": funnel.urls,
                "targets": funnel.targets,
                "text": funnel.text,
            },
            "upnp": { "compiled": cfg!(feature = "upnp"), "available": upnp_available },
        })),
    )
        .into_response()
}

/// Self-contained PIN entry page — inline CSS/JS, no SPA assets, so it can
/// be served (and pass the gate) while everything else is locked.
pub async fn unlock_page() -> Html<&'static str> {
    Html(UNLOCK_PAGE_HTML)
}

const UNLOCK_PAGE_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>PalStudio — unlock</title>
<script>
// Match the tool's palette before first paint. The SPA persists the theme as a
// JSON string ('ps-theme'), so unwrap it; anything unknown falls back to the
// OS preference so the page never flashes a mismatched color.
(function () {
  var t = null;
  try {
    var raw = localStorage.getItem('ps-theme');
    if (raw) t = JSON.parse(raw);
  } catch (e) { /* private mode / storage blocked — fall through */ }
  var known = ['dark', 'frontier', 'light', 'grizzbolt', 'sakurajima', 'wildlands', 'ancient', 'lamball'];
  if (known.indexOf(t) === -1) {
    t = (window.matchMedia && window.matchMedia('(prefers-color-scheme: light)').matches) ? 'light' : 'dark';
  }
  document.documentElement.setAttribute('data-theme', t);
})();
</script>
<style>
  :root { color-scheme: dark; }
  /* Surface/muted/primary tokens mirrored from ps-ui/src/themes/*.css so the
     lock screen reads as the same app, whichever palette is persisted. */
  [data-theme="dark"] { color-scheme: dark; --bg: rgb(17,17,17); --card: rgb(34,34,34);
    --card-border: rgba(102,102,102,.35); --fg: rgb(255,255,255); --muted: rgb(102,102,102);
    --primary: rgb(1,112,243); --primary-hover: rgb(3,97,210); --ring: rgba(43,135,245,.35);
    --on-primary: rgb(211,229,255); --error: rgb(243,63,51); --shadow: rgba(0,0,0,.35); }
  [data-theme="frontier"] { color-scheme: dark; --bg: rgb(6,11,17); --card: rgb(11,19,26);
    --card-border: rgba(54,76,106,.35); --fg: rgb(232,240,248); --muted: rgb(54,76,106);
    --primary: rgb(0,150,185); --primary-hover: rgb(0,118,148); --ring: rgba(0,180,220,.35);
    --on-primary: rgb(224,248,255); --error: rgb(235,30,30); --shadow: rgba(0,0,0,.35); }
  [data-theme="light"] { color-scheme: light; --bg: rgb(248,250,252); --card: rgb(241,245,249);
    --card-border: rgba(100,116,139,.35); --fg: rgb(2,6,23); --muted: rgb(100,116,139);
    --primary: rgb(1,112,243); --primary-hover: rgb(3,97,210); --ring: rgba(43,135,245,.35);
    --on-primary: rgb(211,229,255); --error: rgb(243,63,51); --shadow: rgba(15,23,42,.12); }
  [data-theme="grizzbolt"] { color-scheme: dark; --bg: rgb(45,44,38); --card: rgb(57,55,49);
    --card-border: rgba(104,102,92,.35); --fg: rgb(228,227,222); --muted: rgb(104,102,92);
    --primary: rgb(250,214,21); --primary-hover: rgb(224,192,18); --ring: rgba(251,221,54,.4);
    --on-primary: rgb(254,247,185); --error: rgb(240,68,56); --shadow: rgba(0,0,0,.35); }
  [data-theme="sakurajima"] { color-scheme: dark; --bg: rgb(49,40,48); --card: rgb(62,51,60);
    --card-border: rgba(112,96,110,.35); --fg: rgb(229,223,228); --muted: rgb(112,96,110);
    --primary: rgb(244,114,182); --primary-hover: rgb(220,100,163); --ring: rgba(246,133,192,.35);
    --on-primary: rgb(253,211,234); --error: rgb(235,87,87); --shadow: rgba(0,0,0,.35); }
  [data-theme="wildlands"] { color-scheme: dark; --bg: rgb(40,47,41); --card: rgb(51,59,53);
    --card-border: rgba(96,108,98,.35); --fg: rgb(223,229,224); --muted: rgb(96,108,98);
    --primary: rgb(52,211,153); --primary-hover: rgb(45,190,137); --ring: rgba(79,218,167,.35);
    --on-primary: rgb(185,244,222); --error: rgb(225,72,72); --shadow: rgba(0,0,0,.35); }
  [data-theme="ancient"] { color-scheme: dark; --bg: rgb(44,40,50); --card: rgb(56,51,64);
    --card-border: rgba(104,96,116,.35); --fg: rgb(226,224,232); --muted: rgb(104,96,116);
    --primary: rgb(150,68,222); --primary-hover: rgb(134,60,200); --ring: rgba(165,94,228,.35);
    --on-primary: rgb(225,199,250); --error: rgb(244,63,94); --shadow: rgba(0,0,0,.4); }
  [data-theme="lamball"] { color-scheme: light; --bg: rgb(234,231,226); --card: rgb(213,209,203);
    --card-border: rgba(128,120,108,.35); --fg: rgb(60,55,48); --muted: rgb(128,120,108);
    --primary: rgb(14,130,216); --primary-hover: rgb(12,117,194); --ring: rgba(47,148,223,.35);
    --on-primary: rgb(181,220,249); --error: rgb(220,38,38); --shadow: rgba(15,23,42,.15); }
  body { margin: 0; min-height: 100vh; display: grid; place-items: center; padding: 24px;
         background: var(--bg); color: var(--fg);
         font: 16px/1.5 'Inter', 'Montserrat', system-ui, sans-serif; }
  .card { box-sizing: border-box; width: min(360px, 100%); display: flex; flex-direction: column;
          gap: 16px; padding: 32px 28px 28px; border-radius: 14px; border: 1px solid var(--card-border);
          background: var(--card); box-shadow: 0 20px 60px var(--shadow); }
  .badge { width: 56px; height: 56px; margin: 0 auto; display: grid; place-items: center;
           border-radius: 50%; background: var(--primary); color: var(--on-primary); }
  h1 { margin: 0; font-size: 1.25rem; font-weight: 600; text-align: center; }
  .sub { margin: 0; font-size: .9rem; color: var(--muted); text-align: center; }
  input { box-sizing: border-box; width: 100%; padding: 11px 13px; font-size: 1rem;
          border-radius: 9px; border: 1px solid var(--card-border); background: var(--bg);
          color: inherit; }
  input::placeholder { color: var(--muted); opacity: .8; }
  input:focus { outline: none; border-color: var(--primary); box-shadow: 0 0 0 3px var(--ring); }
  button { padding: 11px 13px; font-size: 1rem; font-weight: 600; border-radius: 9px; border: 0;
           background: var(--primary); color: var(--on-primary); cursor: pointer;
           transition: background .15s ease; }
  button:hover { background: var(--primary-hover); }
  button:disabled { opacity: .6; cursor: wait; }
  #msg { margin: 0; min-height: 1.2em; font-size: .9rem; color: var(--error); text-align: center; }
</style>
</head>
<body>
<form id="f" class="card">
  <span class="badge" aria-hidden="true">
    <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>
  </span>
  <h1>PalStudio is PIN-protected</h1>
  <p class="sub">Network access on this device stays locked until the PIN is verified.</p>
  <input id="pin" type="password" autocomplete="current-password" placeholder="PIN" autofocus>
  <button>Unlock</button>
  <p id="msg"></p>
</form>
<script>
const next = (() => {
  try {
    const n = new URLSearchParams(location.search).get('next') || '/';
    // Same-origin app paths only: absolute or protocol-relative URLs are
    // refused so the redirect cannot be pointed elsewhere.
    return (n.startsWith('/') && !n.startsWith('//')) ? n : '/';
  } catch { return '/'; }
})();
const form = document.getElementById('f'), pin = document.getElementById('pin'), msg = document.getElementById('msg');
form.addEventListener('submit', async (e) => {
  e.preventDefault();
  msg.textContent = '';
  const button = form.querySelector('button');
  button.disabled = true;
  try {
    const resp = await fetch('/api/network/session', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ pin: pin.value }),
    });
    if (resp.ok) { location.replace(next); return; }
    msg.textContent = resp.status === 401 ? 'Wrong PIN' : 'Unlock failed (' + resp.status + ')';
  } catch (err) {
    msg.textContent = 'Network error';
  } finally {
    button.disabled = false;
    pin.select();
  }
});
</script>
</body>
</html>"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with_pin(scope: AuthScope) -> NetworkConfig {
        let mut config = NetworkConfig {
            listen: ListenMode::Lan,
            ..NetworkConfig::default()
        };
        config.auth.scope = scope;
        config.auth.pin = Some(ps_network::PinHash::generate("12345"));
        config
    }

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn redacted_config_never_contains_the_pin_hash() {
        let raw = config_with_pin(AuthScope::NetworkOnly).to_json();
        let redacted =
            serde_json::to_string(&redact(&config_with_pin(AuthScope::NetworkOnly))).unwrap();
        let hash = raw
            .split("\"hash\":\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        assert!(!redacted.contains(hash));
        assert!(redacted.contains("\"pin_set\":true"));
    }

    #[test]
    fn validate_rejects_bad_cidrs_ports_and_short_pins() {
        let update = NetworkConfigUpdate {
            listen: ListenMode::Wan,
            port: 0,
            allow: ps_network::AllowRules {
                connect: vec!["192.168.o.0/24".into()],
                write: vec!["10.0.0.999".into()],
            },
            auth: AuthUpdate {
                scope: AuthScope::NetworkOnly,
                new_pin: Some("12".into()),
                session_ttl_secs: 60,
            },
            upnp_enabled: true,
            funnel_enabled: false,
        };
        let errors = validate(&update);
        assert!(errors.iter().any(|e| e.contains("port")));
        assert!(errors.iter().any(|e| e.contains("192.168.o.0/24")));
        assert!(errors.iter().any(|e| e.contains("10.0.0.999")));
        assert!(errors.iter().any(|e| e.contains("new_pin")));
    }

    #[test]
    fn validate_accepts_a_sane_update() {
        let update = NetworkConfigUpdate {
            listen: ListenMode::Tailscale,
            port: 9000,
            allow: ps_network::AllowRules::default(),
            auth: AuthUpdate {
                scope: AuthScope::NetworkOnly,
                new_pin: None,
                session_ttl_secs: 60,
            },
            upnp_enabled: false,
            funnel_enabled: false,
        };
        assert!(validate(&update).is_empty());
    }

    #[tokio::test]
    async fn runtime_seeds_saves_and_reloads_config() {
        let dir = tempfile::tempdir().unwrap();
        let pool = ps_db::open(&dir.path().join("net.db")).await.unwrap();
        let driver = Arc::new(ps_db::SqlxSqliteDriver::new(pool));

        let runtime = NetworkRuntime::load(&*driver).await.unwrap();
        assert_eq!(runtime.config().listen, ListenMode::Localhost);

        let mut config = runtime.config();
        config.listen = ListenMode::Tailscale;
        config.port = 9001;
        runtime.set_config(config);
        runtime.save(&*driver).await.unwrap();

        let reloaded = NetworkRuntime::load(&*driver).await.unwrap();
        assert_eq!(reloaded.config().listen, ListenMode::Tailscale);
        assert_eq!(reloaded.effective_port(), 9001);
    }

    #[test]
    fn gate_verdicts_come_from_the_policy() {
        // Spot-check the runtime↔policy wiring through the trait object the
        // WS layer consumes.
        let runtime = NetworkRuntime::new(config_with_pin(AuthScope::NetworkOnly));
        let policy: Arc<dyn NetworkPolicy> = Arc::new(runtime);
        let lan_peer = policy.acl_for(ip("192.168.1.30"));
        assert!(lan_peer.can_connect && lan_peer.auth_required);
        let local = policy.acl_for(ip("127.0.0.1"));
        assert!(local.can_connect && local.can_write && !local.auth_required);
        let public = policy.acl_for(ip("8.8.8.8"));
        assert!(!public.can_connect);
    }

    #[test]
    fn session_cookie_round_trip() {
        let runtime = NetworkRuntime::new(config_with_pin(AuthScope::NetworkOnly));
        let token = runtime.sessions.issue(Duration::from_secs(60));
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("{SESSION_COOKIE}={token}")).unwrap(),
        );
        assert!(runtime.session_valid(&headers));
        runtime.sessions.revoke(&token);
        assert!(!runtime.session_valid(&headers));
    }

    #[test]
    fn restart_flag_round_trip() {
        let runtime = NetworkRuntime::new(NetworkConfig::default());
        let flag = runtime.restart_flag();
        assert!(!flag.load(Ordering::SeqCst));
        runtime.request_restart();
        assert!(flag.load(Ordering::SeqCst));
    }

    #[test]
    fn security_warnings_flag_exposure_and_missing_pins() {
        // The shipped default (localhost, no PIN) is quiet.
        assert!(security_warnings(&NetworkConfig::default()).is_empty());

        // The Docker default posture: LAN exposure without a PIN.
        let exposed = NetworkConfig {
            listen: ListenMode::Lan,
            ..NetworkConfig::default()
        };
        let warnings = security_warnings(&exposed);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("without a PIN"));

        // A PIN neutralizes the exposure warning.
        let protected = config_with_pin(AuthScope::NetworkOnly);
        assert!(security_warnings(&protected).is_empty());

        // Auth-on-without-PIN is called out even on localhost.
        let mut broken = NetworkConfig::default();
        broken.auth.scope = AuthScope::NetworkOnly;
        let warnings = security_warnings(&broken);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("no PIN is set"));
    }
}
