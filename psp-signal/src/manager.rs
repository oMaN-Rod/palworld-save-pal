//! The Signal manager: owns the config, the token, the poller, and the
//! local API listener. One instance lives for the whole app; the UI starts
//! and stops the *server* while the manager keeps settings and the access
//! token alive across stops.
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use crate::api::{build_router, ApiState};
use crate::model::{SourceKind, SourceStatus};
use crate::poller::{spawn_poller, FeedStateShared, SourceConfig};
use crate::store::{SignalStore, SignalStored};
use crate::{discovery, token as token_module};

/// A consistent view for status renders (UI tab + `/v1/server` + `/v1/live`).
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub status: SourceStatus,
    pub frame: Option<crate::model::LiveFrame>,
    pub source: Option<SourceKind>,
    pub source_url: Option<String>,
    pub source_locked: bool,
    pub password_set: bool,
    pub running: bool,
    pub bind_addr: SocketAddr,
    pub lan_ip: Option<String>,
    pub token: String,
}

/// Runtime shared with the API layer.
pub struct SignalRuntime {
    poller: Mutex<Option<Arc<Mutex<FeedStateShared>>>>,
    token: Mutex<String>,
    allowed_origins: Mutex<Vec<String>>,
    source_url: Mutex<Option<String>>,
    source_locked: Mutex<bool>,
    password_set: Mutex<bool>,
    minimap_claim: Mutex<Option<(String, std::time::Instant)>>,
    /// Set once by `SignalManager::new` after the Arc exists.
    manager: std::sync::Mutex<std::sync::Weak<SignalManagerInner>>,
}

impl SignalRuntime {
    pub async fn token(&self) -> String {
        self.token.lock().await.clone()
    }

    pub async fn allowed_origins(&self) -> Vec<String> {
        self.allowed_origins.lock().await.clone()
    }

    /// One consistent read of everything the endpoints render.
    pub async fn snapshot(&self) -> Snapshot {
        let poller = self.poller.lock().await.clone();
        let (status, frame, kind) = match poller {
            Some(shared) => {
                let mut state = shared.lock().await;
                state.refresh();
                let status = state.status.clone();
                let kind = status.kind;
                (status, state.frame.clone(), kind)
            }
            None => (SourceStatus::default(), None, None),
        };
        let manager = self.manager.lock().expect("runtime manager slot").upgrade();
        let Some(inner) = manager else {
            return Snapshot {
                status,
                frame,
                source: kind,
                source_url: None,
                source_locked: false,
                password_set: false,
                running: false,
                bind_addr: "127.0.0.1:0".parse().unwrap(),
                lan_ip: None,
                token: String::new(),
            };
        };
        let guard = inner.shared.lock().await;
        Snapshot {
            status,
            frame,
            source: kind.or(guard.source_kind),
            source_url: self.source_url.lock().await.clone(),
            source_locked: *self.source_locked.lock().await,
            password_set: *self.password_set.lock().await,
            running: guard.handle.is_some(),
            bind_addr: guard.bound_addr,
            lan_ip: inner.lan_ip.clone(),
            token: self.token.lock().await.clone(),
        }
    }

    /// Handles `POST /v1/server` bodies: `{"url","password"}` connects,
    /// `{"clear":true}` forgets. A blank password keeps the current one.
    pub async fn apply_server_post(
        &self,
        body: &serde_json::Value,
    ) -> Result<Snapshot, String> {
        if *self.source_locked.lock().await {
            return Err("the source is set by the host app and cannot be changed here".into());
        }
        let manager = self.manager.lock().expect("runtime manager slot").upgrade();
        let Some(inner) = manager else {
            return Err("signal is shutting down".into());
        };
        if body.get("clear").and_then(|value| value.as_bool()) == Some(true) {
            inner.set_source(None).await.map_err(|error| error.to_string())?;
            return Ok(self.snapshot().await);
        }
        let url = body
            .get("url")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let Some(normalized) = crate::rest::normalize_base(&url) else {
            return Err("not an http address Signal can call".into());
        };
        let password = body
            .get("password")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let password = if password.is_empty() && *self.password_set.lock().await {
            None // keep the current one
        } else {
            Some(password)
        };
        inner
            .set_source(Some(SourceConfig::Rest {
                base: normalized,
                password,
            }))
            .await
            .map_err(|error| error.to_string())?;
        Ok(self.snapshot().await)
    }

    pub async fn claim_minimap(&self, title: String) {
        *self.minimap_claim.lock().await = Some((title, std::time::Instant::now()));
    }

    pub async fn release_minimap(&self) {
        *self.minimap_claim.lock().await = None;
    }

    pub async fn minimap_active(&self) -> bool {
        const CLAIM_TTL: Duration = Duration::from_secs(60);
        match &*self.minimap_claim.lock().await {
            Some((_, at)) => at.elapsed() < CLAIM_TTL,
            None => false,
        }
    }
}

struct RunningHandle {
    poll_task: tokio::task::JoinHandle<()>,
    serve_task: tokio::task::JoinHandle<std::io::Result<()>>,
    shutdown: tokio::sync::watch::Sender<bool>,
}

struct ManagerShared {
    stored: SignalStored,
    source_kind: Option<SourceKind>,
    source: Option<SourceConfig>,
    bound_addr: SocketAddr,
    handle: Option<RunningHandle>,
}

struct SignalManagerInner {
    store: Box<dyn SignalStore>,
    lan_ip: Option<String>,
    runtime: Arc<SignalRuntime>,
    shared: Mutex<ManagerShared>,
}

#[derive(Debug, thiserror::Error)]
pub enum SignalError {
    #[error("the Signal API is already running on {0}")]
    AlreadyRunning(SocketAddr),
    #[error("could not bind {bind}: {source}")]
    BindFailed {
        bind: String,
        source: std::io::Error,
    },
    #[error("not an http address Signal can call")]
    BadUrl,
}

/// The public manager handle.
#[derive(Clone)]
pub struct SignalManager {
    inner: Arc<SignalManagerInner>,
}

impl SignalManager {
    /// Builds the manager, loading (and seeding) persisted settings. The
    /// token is loaded from the store or generated once and saved back —
    /// restarts keep existing clients valid.
    pub async fn new(store: Box<dyn SignalStore>) -> Self {
        let mut stored = store.load().await;
        if stored.token.is_empty() || !token_module::is_well_formed(&stored.token) {
            stored.token = token_module::generate_token();
            store.save(&stored).await;
        }
        if stored.interval_ms == 0 {
            stored.interval_ms = SignalStored::defaults().interval_ms;
        }
        let lan_ip = lan_ip().await;
        let runtime = Arc::new(SignalRuntime {
            poller: Mutex::new(None),
            token: Mutex::new(stored.token.clone()),
            allowed_origins: Mutex::new(stored.allowed_origins.clone()),
            source_url: Mutex::new(None),
            source_locked: Mutex::new(false),
            password_set: Mutex::new(false),
            minimap_claim: Mutex::new(None),
            manager: std::sync::Mutex::new(std::sync::Weak::new()),
        });
        let inner = Arc::new(SignalManagerInner {
            store,
            lan_ip,
            runtime: Arc::clone(&runtime),
            shared: Mutex::new(ManagerShared {
                source_kind: None,
                source: None,
                bound_addr: "127.0.0.1:0".parse().unwrap(),
                handle: None,
                stored,
            }),
        });
        // The weak back-reference can only be created after the Arc it
        // points at; interior mutability lets it be set through &self.
        *runtime.manager.lock().expect("runtime manager slot") = Arc::downgrade(&inner);
        Self { inner }
    }

    /// Consistent snapshot for the UI tab.
    pub async fn snapshot(&self) -> Snapshot {
        self.inner.runtime.snapshot().await
    }

    /// Current persisted settings (token included — the UI shows it).
    pub async fn stored(&self) -> SignalStored {
        self.inner.shared.lock().await.stored.clone()
    }

    /// Updates persisted settings (bind/port/interval/allow-list/autostart)
    /// without touching the source or token. Takes effect on next start.
    #[allow(clippy::too_many_arguments)]
    pub async fn update_settings(
        &self,
        enabled: Option<bool>,
        bind: Option<String>,
        port: Option<u16>,
        interval_ms: Option<u64>,
        allowed_origins: Option<Vec<String>>,
    ) -> SignalStored {
        let mut shared = self.inner.shared.lock().await;
        if let Some(enabled) = enabled {
            shared.stored.enabled = enabled;
        }
        if let Some(bind) = bind {
            let bind = bind.trim().to_string();
            if !bind.is_empty() {
                shared.stored.bind = bind;
            }
        }
        if let Some(port) = port {
            shared.stored.port = port;
        }
        if let Some(interval_ms) = interval_ms {
            shared.stored.interval_ms = interval_ms.clamp(250, 60_000);
        }
        if let Some(allowed) = allowed_origins {
            shared.stored.allowed_origins = allowed;
        }
        let stored = shared.stored.clone();
        drop(shared);
        *self.inner.runtime.allowed_origins.lock().await = stored.allowed_origins.clone();
        self.inner.store.save(&stored).await;
        stored
    }

    /// Sets the active source (None = idle). Restarts the poller when the
    /// API is running; persists everything except the password.
    pub async fn set_source(&self, source: Option<SourceConfig>) -> Result<(), SignalError> {
        self.inner.set_source(source).await
    }

    /// Locks (or unlocks) the source so `POST /v1/server` cannot redirect
    /// it — used when the host app owns the source configuration.
    pub async fn lock_source(&self, locked: bool) {
        *self.inner.runtime.source_locked.lock().await = locked;
    }

    /// Replaces the access token (old clients stop working immediately).
    pub async fn regenerate_token(&self) -> String {
        let fresh = token_module::generate_token();
        *self.inner.runtime.token.lock().await = fresh.clone();
        let mut shared = self.inner.shared.lock().await;
        shared.stored.token = fresh.clone();
        let stored = shared.stored.clone();
        drop(shared);
        self.inner.store.save(&stored).await;
        fresh
    }

    /// Starts the poller + local API. Binding `0` picks a free port (the
    /// chosen port is persisted so restarts keep client URLs stable).
    pub async fn start(&self) -> Result<SocketAddr, SignalError> {
        let mut shared = self.inner.shared.lock().await;
        if shared.handle.is_some() {
            return Err(SignalError::AlreadyRunning(shared.bound_addr));
        }
        let bind: SocketAddr = format!("{}:{}", shared.stored.bind, shared.stored.port)
            .parse()
            .map_err(|_| SignalError::BadUrl)?;
        let listener = tokio::net::TcpListener::bind(bind)
            .await
            .map_err(|source| SignalError::BindFailed {
                bind: bind.to_string(),
                source,
            })?;
        let addr = listener.local_addr().expect("bound listener has a local addr");
        shared.bound_addr = addr;
        if shared.stored.port == 0 {
            shared.stored.port = addr.port();
            let stored = shared.stored.clone();
            self.inner.store.save(&stored).await;
        }

        // The poller starts with the configured source, if any; the serve
        // task owns nothing — teardown aborts handles stored on the manager.
        let mut poll_task = None;
        if let Some(source) = shared.source.clone() {
            let (state, task) =
                spawn_poller(source, Duration::from_millis(shared.stored.interval_ms)).await;
            *self.inner.runtime.poller.lock().await = Some(state);
            poll_task = Some(task);
        }
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);
        let router = build_router(ApiState {
            runtime: Arc::clone(&self.inner.runtime),
        });
        let into_service = router.into_make_service_with_connect_info::<SocketAddr>();
        let serve_task = tokio::spawn(async move {
            axum::serve(listener, into_service)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.changed().await;
                })
                .await
        });
        shared.handle = Some(RunningHandle {
            poll_task: poll_task.unwrap_or_else(|| tokio::spawn(async {})),
            serve_task,
            shutdown: shutdown_tx,
        });
        let addr = shared.bound_addr;
        drop(shared);
        // Keep the stored enabled flag in sync with reality.
        self.update_settings(Some(true), None, None, None, None).await;
        Ok(addr)
    }

    /// Stops the API + poller. Settings, token, and source persist.
    pub async fn stop(&self) {
        let mut shared = self.inner.shared.lock().await;
        if let Some(handle) = shared.handle.take() {
            let _ = handle.shutdown.send(true);
            handle.serve_task.abort();
            handle.poll_task.abort();
        }
        *self.inner.runtime.poller.lock().await = None;
        shared.stored.enabled = false;
        let stored = shared.stored.clone();
        drop(shared);
        self.inner.store.save(&stored).await;
    }

    pub async fn is_running(&self) -> bool {
        self.inner.shared.lock().await.handle.is_some()
    }

    /// Local game-data discovery for the UI's source picker.
    pub fn discover_game_data(&self) -> Vec<discovery::GameDataCandidate> {
        discovery::game_data_candidates()
    }

    pub fn lan_ip(&self) -> Option<&str> {
        self.inner.lan_ip.as_deref()
    }
}

impl SignalManagerInner {
    async fn set_source(&self, source: Option<SourceConfig>) -> Result<(), SignalError> {
        let mut shared = self.shared.lock().await;
        let kind = match &source {
            None => None,
            Some(SourceConfig::Fake) => Some(SourceKind::Fake),
            Some(SourceConfig::GameData { .. }) => Some(SourceKind::GameData),
            Some(SourceConfig::Rest { .. }) => Some(SourceKind::Rest),
        };
        // Persist everything except the password.
        match &source {
            None => {
                shared.stored.source_type = None;
                shared.stored.source_url = None;
                // Keep gamedata_path: a reconnect after "clear" of a REST
                // source should not forget an explicit bridge path.
            }
            Some(SourceConfig::Fake) => shared.stored.source_type = Some("fake".into()),
            Some(SourceConfig::GameData { path }) => {
                shared.stored.source_type = Some("gamedata".into());
                shared.stored.gamedata_path = path
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned());
            }
            Some(SourceConfig::Rest { base, .. }) => {
                shared.stored.source_type = Some("rest".into());
                shared.stored.source_url = Some(base.clone());
            }
        }
        *self.runtime.source_url.lock().await = match &source {
            Some(SourceConfig::Rest { base, .. }) => Some(base.clone()),
            _ => None,
        };
        *self.runtime.password_set.lock().await = match &source {
            Some(SourceConfig::Rest { password, .. }) => password.is_some(),
            _ => false,
        };
        shared.source = source;
        shared.source_kind = kind;
        let stored = shared.stored.clone();
        let interval = Duration::from_millis(stored.interval_ms);
        let running = shared.handle.is_some();
        let source = shared.source.clone();
        drop(shared);
        self.store.save(&stored).await;

        if running {
            match source {
                Some(source) => {
                    let (state, task) = spawn_poller(source, interval).await;
                    let mut poller = self.runtime.poller.lock().await;
                    *poller = Some(state);
                    // Attach the new poll task to the running handle so
                    // teardown aborts it too.
                    drop(poller);
                    let mut shared = self.shared.lock().await;
                    if let Some(handle) = &mut shared.handle {
                        handle.poll_task.abort();
                        handle.poll_task = task;
                    }
                }
                None => {
                    *self.runtime.poller.lock().await = None;
                }
            }
        }
        Ok(())
    }
}

/// The LAN IP other devices would use to reach this machine, discovered with
/// a UDP `connect` to a non-routable address (no packets are sent) and asking
/// the socket which local interface it picked.
pub async fn lan_ip() -> Option<String> {
    let socket = tokio::net::UdpSocket::bind("0.0.0.0:0").await.ok()?;
    socket.connect("10.255.255.255:1").await.ok()?;
    Some(socket.local_addr().ok()?.ip().to_string())
}
