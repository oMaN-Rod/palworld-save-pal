//! PalStudio's own network policy runtime: config persistence, request
//! enforcement, PIN sessions, and the Network page's REST API.
//!
//! Layering with the pure `ps-network` crate: that one decides (config,
//! peer) → verdict; this one owns the live config, persists it in the `meta`
//! table, evaluates every inbound HTTP/WS request, and applies edits from
//! the UI (including tailscale funnel toggling and optional UPnP mapping).
use std::collections::HashSet;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use axum::extract::{ConnectInfo, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::{Json, Router};
use ps_network::auth::{AuthRateLimiter, SessionRegistry};
use ps_network::{
    AuthScope, ListenMode, NetworkConfig, NetworkTier, PeerAcl as Verdict, MAX_PIN_CHARS,
    MAX_SESSION_TTL_SECS, MIN_PIN_CHARS, MIN_SESSION_TTL_SECS,
};

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
    policy_generation: AtomicU64,
    update_lock: tokio::sync::Mutex<()>,
    auth_limiter: AuthRateLimiter,
    tailnet_peer_ips: RwLock<Option<HashSet<IpAddr>>>,
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
            policy_generation: AtomicU64::new(0),
            update_lock: tokio::sync::Mutex::new(()),
            auth_limiter: AuthRateLimiter::default(),
            tailnet_peer_ips: RwLock::new(None),
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
            NetworkTier::Desktop | NetworkTier::LocalWebapp => {
                self.config().clamped_for_local_webapp()
            }
            NetworkTier::Hosted => self.config(),
        }
    }

    pub fn config(&self) -> NetworkConfig {
        self.config
            .read()
            .expect("network config lock poisoned")
            .clone()
    }

    pub fn set_config(&self, config: NetworkConfig) {
        let changed = {
            let mut current = self.config.write().expect("network config lock poisoned");
            if *current == config {
                false
            } else {
                *current = config;
                true
            }
        };
        if changed {
            self.sessions.revoke_all();
            self.policy_generation.fetch_add(1, Ordering::SeqCst);
        }
    }

    pub(crate) async fn lock_updates(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.update_lock.lock().await
    }

    pub fn policy_generation(&self) -> u64 {
        self.policy_generation.load(Ordering::SeqCst)
    }

    pub fn auth_retry_after(&self, peer: IpAddr) -> Option<Duration> {
        self.auth_limiter.retry_after(peer)
    }

    pub fn record_auth_failure(&self, peer: IpAddr) -> Option<Duration> {
        self.auth_limiter.record_failure(peer)
    }

    pub fn record_auth_success(&self, peer: IpAddr) {
        self.auth_limiter.record_success(peer);
    }

    pub async fn refresh_tailnet_peers(&self) -> anyhow::Result<()> {
        let status = tokio::task::spawn_blocking(ps_network::tailscale::detect)
            .await
            .map_err(|error| anyhow::anyhow!("tailscale detection task failed: {error}"))?;
        if !status.available || !status.logged_in {
            return Err(anyhow::anyhow!(
                "Tailscale is unavailable or this node is not logged in"
            ));
        }
        let peers = status
            .peer_ipv4
            .into_iter()
            .map(ps_network::canonical)
            .collect::<HashSet<_>>();
        *self
            .tailnet_peer_ips
            .write()
            .expect("tailnet peer lock poisoned") = Some(peers);
        Ok(())
    }

    fn tailnet_peer_is_verified(&self, peer: IpAddr) -> bool {
        self.tailnet_peer_ips
            .read()
            .expect("tailnet peer lock poisoned")
            .as_ref()
            .is_some_and(|peers| peers.contains(&ps_network::canonical(peer)))
    }

    pub fn effective_port(&self) -> u16 {
        self.config().port
    }

    pub fn evaluate(&self, peer: IpAddr) -> Verdict {
        let config = self.effective_config();
        let verdict = ps_network::evaluate(&config, peer);
        if verdict.can_connect
            && config.listen == ListenMode::Tailscale
            && ps_network::classify(peer) != ps_network::PeerClass::Loopback
            && !self.tailnet_peer_is_verified(peer)
        {
            return Verdict {
                can_connect: false,
                can_write: false,
                auth_required: false,
            };
        }
        verdict
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
                let mut config = NetworkConfig::from_json_lenient(&raw)?;
                let adjustments = config.normalize_legacy();
                config.validate()?;
                if !adjustments.is_empty() {
                    for note in &adjustments {
                        tracing::warn!("stored network policy adjusted: {note}");
                    }
                    ps_db::meta::set(driver, META_KEY, &config.to_json()).await?;
                }
                let config = if always {
                    apply_env_blocking(config).await?
                } else {
                    config
                };
                Ok(NetworkRuntime::new(config))
            }
            None => {
                let seeded = apply_env_blocking(NetworkConfig::default()).await?;
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
        session_token_from(headers)
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

    fn policy_generation(&self) -> u64 {
        self.policy_generation()
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
    let peer_is_loopback = ps_network::canonical(peer).is_loopback();

    let verdict = runtime.evaluate(peer);
    if !verdict.can_connect {
        tracing::warn!(%peer, %path, "connection refused by network policy");
        return error_response(
            StatusCode::FORBIDDEN,
            "refused: your address is not allowed to connect to this PalStudio instance",
        );
    }
    if !peer_is_loopback && !secure_transport(&request) {
        tracing::warn!(%peer, %path, "refused cleartext network request");
        return error_response(
            StatusCode::UPGRADE_REQUIRED,
            "HTTPS is required for non-loopback connections",
        );
    }
    // These routes are intentionally unauthenticated only after the peer has
    // passed listen mode and the connect allowlist. Otherwise a public client
    // could use this exception as an unrestricted online PIN oracle.
    if path == "/network-unlock" || path == "/api/network/session" {
        return next.run(request).await;
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

fn secure_transport(request: &Request) -> bool {
    request.uri().scheme_str() == Some("https")
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
            "listening on '{}' without a PIN — {} can read only; set a PIN and an explicit \
             write allowlist for network edits",
            config.listen.as_str(),
            ps_network::default_audience(config.listen)
        ));
    }
    if config.auth.scope != AuthScope::Never && config.auth.pin.is_none() {
        warnings.push("auth is enabled but no PIN is set; network peers are refused".into());
    }
    warnings
}

/// True when the environment re-applies `PS_LISTEN`/`PS_PORT`/`PS_PIN` on
/// every boot (the Docker posture set by docker-compose.yml).
fn env_mode_always() -> bool {
    std::env::var(ENV_MODE).is_ok_and(|mode| mode.trim().eq_ignore_ascii_case("always"))
}

async fn apply_env_blocking(config: NetworkConfig) -> anyhow::Result<NetworkConfig> {
    tokio::task::spawn_blocking(move || config.apply_env())
        .await
        .map_err(|error| anyhow::anyhow!("network environment task failed: {error}"))?
        .map_err(anyhow::Error::from)
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

/// Validates an API update before it is allowed to reach either the database
/// or an external networking daemon.
fn validate(update: &NetworkConfigUpdate) -> Vec<String> {
    let mut errors = Vec::new();
    if update.port == 0 {
        errors.push("port must be between 1 and 65535".into());
    }
    if !(MIN_SESSION_TTL_SECS..=MAX_SESSION_TTL_SECS).contains(&update.auth.session_ttl_secs) {
        errors.push(format!(
            "session_ttl_secs must be between {MIN_SESSION_TTL_SECS} and {MAX_SESSION_TTL_SECS}"
        ));
    }
    for (label, rules) in [
        ("allow.connect", &update.allow.connect),
        ("allow.write", &update.allow.write),
    ] {
        if rules.len() > 256 {
            errors.push(format!("{label} contains too many entries"));
        }
        for entry in rules {
            if entry.len() > 64
                || entry.chars().any(char::is_control)
                || ps_network::policy::IpNet::parse(entry).is_none()
            {
                errors.push(format!("{label}: invalid entry `{entry}`"));
            }
        }
    }
    if let Some(pin) = update.auth.new_pin.as_deref() {
        if !pin.is_empty() {
            if let Err(error) = ps_network::validate_pin(pin) {
                errors.push(format!("new_pin: {error}"));
            }
        }
    }
    if update.funnel_enabled && update.auth.scope != AuthScope::Always {
        errors.push("Tailscale Funnel requires AuthScope::Always".into());
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

    let errors = validate(&update);
    if !errors.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": errors.join("; ") })),
        )
            .into_response();
    }

    // Serialize the complete update workflow. This prevents two requests
    // from interleaving router/daemon changes and database commits.
    let _update_guard = runtime.lock_updates().await;
    let old = runtime.config();

    // A hand-launched local webapp is a local tool: only the port is
    // editable, everything else stays clamped to localhost. The stored
    // policy is preserved untouched for a later hosted/service run.
    if runtime.tier() == NetworkTier::LocalWebapp {
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
        let port_changed = update.port != old.port;
        let mut merged = old.clone();
        merged.port = update.port;
        if let Err(error) = merged.validate() {
            return error_response(StatusCode::BAD_REQUEST, &error.to_string());
        }
        if let Err(error) = ps_db::meta::set(&*app.driver, META_KEY, &merged.to_json()).await {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("could not persist network config: {error}"),
            );
        }
        runtime.set_config(merged);
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

    let mut merged = NetworkConfig {
        version: old.version,
        listen: update.listen,
        port: update.port,
        allow: update.allow.clone(),
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
        Some(pin) => {
            let pin = pin.to_owned();
            let generated =
                tokio::task::spawn_blocking(move || ps_network::PinHash::generate(&pin))
                    .await
                    .map_err(|error| format!("PIN hashing task failed: {error}"));
            match generated {
                Ok(hash) => merged.auth.pin = Some(hash),
                Err(error) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &error),
            }
        }
        None => {}
    }

    if let Err(error) = merged.validate() {
        return error_response(StatusCode::BAD_REQUEST, &error.to_string());
    }

    if merged.listen == ListenMode::Tailscale {
        if let Err(error) = runtime.refresh_tailnet_peers().await {
            return error_response(
                StatusCode::BAD_GATEWAY,
                &format!("Tailscale peer identity could not be verified: {error}"),
            );
        }
    }

    let mut warnings = security_warnings(&merged);
    if merged.upnp_enabled {
        warnings
            .push("UPnP opens a port on your router — tailscale is the safer remote path".into());
    }

    // Commit the candidate first. External changes are a compensating
    // transaction: if reconciliation fails, restore both the old resources
    // and the old database row before exposing the new runtime state.
    if let Err(error) = ps_db::meta::set(&*app.driver, META_KEY, &merged.to_json()).await {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("could not persist network config: {error}"),
        );
    }

    if let Err(error) = reconcile_network_resources(&old, &merged).await {
        tracing::error!(%error, "network resource transaction failed; rolling back");
        if let Err(rollback_error) = reconcile_network_resources(&merged, &old).await {
            tracing::error!(%rollback_error, "network resource rollback failed");
        }
        if let Err(rollback_error) = ps_db::meta::set(&*app.driver, META_KEY, &old.to_json()).await
        {
            tracing::error!(%rollback_error, "network config database rollback failed");
        }
        return error_response(
            StatusCode::BAD_GATEWAY,
            &format!("network resource update failed; previous configuration restored when possible: {error}"),
        );
    }

    let port_changed = merged.port != old.port;
    let listen_changed = merged.listen != old.listen;
    let new_port = merged.port;
    runtime.set_config(merged.clone());

    if port_changed || listen_changed {
        tracing::info!(
            old = old.port,
            new = new_port,
            listen_changed,
            "network listener configuration changed; requesting rebind"
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
            "restart_required": port_changed || listen_changed,
            "warnings": warnings,
        })),
    )
        .into_response()
}

/// Brings the live router/tailnet exposure in line with `config`, as best
/// the current environment allows. Failures are collected, not fatal: a
/// missing Tailscale CLI or a network without a UPnP gateway degrades the
/// deployment loudly (each caller warns) instead of stopping the server
/// from serving at all. Strict error handling belongs to the interactive
/// config-change path (`reconcile_network_resources`), where the operator
/// is present and a rollback exists.
pub(crate) async fn reconcile_current_resources(config: &NetworkConfig) -> Vec<String> {
    let mut failures = Vec::new();

    if config.funnel_enabled {
        match tokio::task::spawn_blocking(ps_network::tailscale::funnel_probe).await {
            Ok(live) if live.available => {
                let target_matches =
                    live.on && ps_network::tailscale::funnel_owns_local_port(&live, config.port);
                if !live.on || !target_matches {
                    let port = config.port;
                    match tokio::task::spawn_blocking(move || {
                        ps_network::tailscale::set_funnel(true, port)
                    })
                    .await
                    {
                        Ok(Ok(_)) => {}
                        Ok(Err(error)) => {
                            failures.push(format!("Tailscale Funnel could not be enabled: {error}"))
                        }
                        Err(error) => failures.push(format!("funnel update task failed: {error}")),
                    }
                }
            }
            Ok(_) => failures.push(
                "Tailscale Funnel is enabled but the CLI is unavailable".into(),
            ),
            Err(error) => failures.push(format!("funnel probe task failed: {error}")),
        }
    } else {
        match tokio::task::spawn_blocking(ps_network::tailscale::funnel_probe).await {
            Ok(live)
                if live.available && live.on && ps_network::tailscale::funnel_owns_local_targets(&live) =>
            {
                let port = config.port;
                match tokio::task::spawn_blocking(move || {
                    ps_network::tailscale::set_funnel(false, port)
                })
                .await
                {
                    Ok(Ok(_)) => {}
                    Ok(Err(error)) => {
                        failures.push(format!("Tailscale Funnel could not be turned off: {error}"))
                    }
                    Err(error) => failures.push(format!("funnel cleanup task failed: {error}")),
                }
            }
            Ok(_) => {}
            Err(error) => failures.push(format!("funnel cleanup probe task failed: {error}")),
        }
    }

    #[cfg(feature = "upnp")]
    {
        if config.upnp_enabled {
            match tokio::time::timeout(
                Duration::from_secs(20),
                ps_network::upnp::map_port(config.port, "PalStudio"),
            )
            .await
            {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    failures.push(format!("UPnP port mapping could not be created: {error}"))
                }
                Err(_) => failures.push("UPnP port mapping timed out".into()),
            }
            match tokio::time::timeout(
                Duration::from_secs(20),
                ps_network::upnp::remove_owned_mappings_except("PalStudio", Some(config.port)),
            )
            .await
            {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    failures.push(format!("stale UPnP mappings could not be cleaned: {error}"))
                }
                Err(_) => failures.push("stale UPnP mapping cleanup timed out".into()),
            }
        } else {
            // Best-effort cleanup of mappings an earlier run may have left;
            // a network without a UPnP gateway simply has nothing to remove.
            if let Ok(Err(error)) = tokio::time::timeout(
                Duration::from_secs(20),
                ps_network::upnp::remove_owned_mappings("PalStudio"),
            )
            .await
            {
                if !error.contains("no UPnP gateway") {
                    failures.push(format!("stale UPnP mappings could not be cleaned: {error}"));
                }
            }
        }
    }
    #[cfg(not(feature = "upnp"))]
    if config.upnp_enabled {
        failures.push("this build does not include UPnP support".into());
    }

    failures
}

async fn reconcile_network_resources(
    old: &NetworkConfig,
    new: &NetworkConfig,
) -> Result<(), String> {
    if old.funnel_enabled != new.funnel_enabled
        || old.port != new.port
        || (new.funnel_enabled && old.funnel_enabled)
    {
        let live = tokio::task::spawn_blocking(ps_network::tailscale::funnel_probe)
            .await
            .map_err(|error| format!("funnel probe task failed: {error}"))?;
        if !live.available && (old.funnel_enabled || new.funnel_enabled) {
            return Err("Tailscale CLI is unavailable while Funnel state must change".into());
        }
        let pointing_at_new_port =
            live.on && ps_network::tailscale::funnel_owns_local_port(&live, new.port);
        if live.on != new.funnel_enabled || (new.funnel_enabled && !pointing_at_new_port) {
            let enabled = new.funnel_enabled;
            let port = new.port;
            tokio::task::spawn_blocking(move || ps_network::tailscale::set_funnel(enabled, port))
                .await
                .map_err(|error| format!("funnel update task failed: {error}"))??;
        }
    }

    #[cfg(feature = "upnp")]
    {
        if new.upnp_enabled {
            tokio::time::timeout(
                Duration::from_secs(20),
                ps_network::upnp::map_port(new.port, "PalStudio"),
            )
            .await
            .map_err(|_| "UPnP mapping timed out".to_owned())??;
            tokio::time::timeout(
                Duration::from_secs(20),
                ps_network::upnp::remove_owned_mappings_except("PalStudio", Some(new.port)),
            )
            .await
            .map_err(|_| "stale UPnP mapping cleanup timed out".to_owned())??;
        } else if old.upnp_enabled {
            tokio::time::timeout(
                Duration::from_secs(20),
                ps_network::upnp::remove_owned_mappings("PalStudio"),
            )
            .await
            .map_err(|_| "UPnP unmapping timed out".to_owned())??;
        }
    }
    #[cfg(not(feature = "upnp"))]
    if new.upnp_enabled {
        return Err("this build does not include UPnP support".into());
    }

    Ok(())
}

#[derive(serde::Deserialize)]
struct SessionRequest {
    pin: String,
}

async fn create_session(
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    axum::Extension(runtime): axum::Extension<Arc<NetworkRuntime>>,
    Json(request): Json<SessionRequest>,
) -> Response {
    if let Some(retry_after) = runtime.auth_retry_after(peer.ip()) {
        let mut response = error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "too many failed PIN attempts; try again later",
        );
        if let Ok(value) = HeaderValue::from_str(&retry_after.as_secs().max(1).to_string()) {
            response.headers_mut().insert(header::RETRY_AFTER, value);
        }
        return response;
    }

    let config = runtime.effective_config();
    let Some(hash) = config.auth.pin.clone() else {
        return error_response(StatusCode::CONFLICT, "no PIN is configured");
    };
    let ttl = Duration::from_secs(config.auth.session_ttl_secs);
    let pin = request.pin;
    let pin_shape_ok = (MIN_PIN_CHARS..=MAX_PIN_CHARS).contains(&pin.chars().count())
        && !pin.chars().any(char::is_control);
    let matches = tokio::task::spawn_blocking(move || pin_shape_ok && hash.matches(&pin))
        .await
        .unwrap_or(false);
    if !matches {
        let retry_after = runtime.record_auth_failure(peer.ip());
        tokio::time::sleep(FAILED_PIN_DELAY).await;
        let mut response = error_response(StatusCode::UNAUTHORIZED, "wrong PIN");
        if let Some(retry_after) = retry_after {
            if let Ok(value) = HeaderValue::from_str(&retry_after.as_secs().max(1).to_string()) {
                response.headers_mut().insert(header::RETRY_AFTER, value);
            }
        }
        return response;
    }
    runtime.record_auth_success(peer.ip());
    let token = runtime.sessions.issue(ttl);
    let secure = config.funnel_enabled
        || !ps_network::canonical(peer.ip()).is_loopback()
        || secure_transport_from_headers(&headers);
    let mut cookie = format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}",
        config.auth.session_ttl_secs
    );
    if secure {
        cookie.push_str("; Secure");
    }
    let mut response = (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).expect("session cookie is header-safe"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn delete_session(
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    axum::Extension(runtime): axum::Extension<Arc<NetworkRuntime>>,
) -> Response {
    if let Some(token) = session_token_from(&headers) {
        runtime.sessions.revoke(&token);
    }
    // Clear the cookie regardless so the browser drops it.
    let secure = runtime.effective_config().funnel_enabled
        || !ps_network::canonical(peer.ip()).is_loopback()
        || secure_transport_from_headers(&headers);
    let suffix = if secure { "; Secure" } else { "" };
    let cookie = format!("{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0{suffix}");
    let mut response = (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).expect("session cookie is header-safe"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

pub(crate) fn session_token_from(headers: &axum::http::HeaderMap) -> Option<String> {
    let cookie = headers.get(header::COOKIE)?.to_str().ok()?;
    cookie.split(';').find_map(|part| {
        let part = part.trim();
        let (name, value) = part.split_once('=')?;
        (name.trim() == SESSION_COOKIE && !value.trim().is_empty()).then(|| value.trim().to_owned())
    })
}

fn secure_transport_from_headers(headers: &HeaderMap) -> bool {
    headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("https"))
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
pub async fn unlock_page() -> Response {
    let mut response = Html(UNLOCK_PAGE_HTML).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
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
            ..Default::default()
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
