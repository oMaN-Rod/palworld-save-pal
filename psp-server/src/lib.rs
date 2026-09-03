pub mod api_convert;
pub mod lsp_service;
#[cfg(feature = "desktop")]
pub mod rfd_dialogs;
pub mod router;
pub mod server_ext;
pub mod servers_handlers;
pub mod services;
pub mod static_files;
pub mod system_native;
pub mod ws;

pub use psp_app::{
    blueprint_registry, desktop_dialogs, dispatcher, emitter, envelope, handler_error, handlers,
    messages, AppConfig, AppState, SessionStore, SharedSession,
};

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;

use psp_core::gamedata::GameData;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Web default 0.0.0.0; desktop 127.0.0.1.
    pub host: IpAddr,
    pub port: u16,
    pub ui_dir: PathBuf,
    /// Directory holding "json/" with the game data.
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    /// Enables native file dialogs and the local folder/browser handlers.
    pub desktop_mode: bool,
}

pub struct ServerHandle {
    pub addr: SocketAddr,
    pub app: Arc<AppState>,
    /// Subscriber on `AppState::live_connections`, seeded at 0 before any
    /// connection is accepted, so tests can await connection teardown instead
    /// of sleeping.
    pub live_connections: tokio::sync::watch::Receiver<usize>,
    /// Fires when a client sends the `shutdown` message (the browser-mode
    /// control panel's Quit button) — the embedding shell watches it to exit
    /// the app while the server drains gracefully.
    pub shutdown_requested: tokio::sync::watch::Receiver<bool>,
    shutdown_sender: tokio::sync::oneshot::Sender<()>,
    serve_task: tokio::task::JoinHandle<std::io::Result<()>>,
}

/// Set once by `start_server_with`; fired by the `shutdown` WS message so a
/// connected UI can ask the whole process to stop. Returns false when no
/// server is running in this process (e.g. unit tests calling the handler).
pub fn request_shutdown() -> bool {
    SHUTDOWN_REQUESTED
        .get()
        .is_some_and(|sender| sender.send(true).is_ok())
}

static SHUTDOWN_REQUESTED: std::sync::OnceLock<tokio::sync::watch::Sender<bool>> =
    std::sync::OnceLock::new();

/// A request to change the Linux launch mode (`set_mode` WS message). The
/// desktop shell registers a listener so it can persist the new mode to
/// `mode.json` and pivot/relaunch; psp-server itself only relays the event and
/// stays Tauri-agnostic. `Send + Sync` via the channel; the shell owns the
/// receiver task.
#[derive(Debug, Clone)]
pub struct ModeEvent {
    /// Wire value: `"desktop"` or `"browser"`.
    pub mode: String,
}

static MODE_LISTENER: std::sync::OnceLock<tokio::sync::mpsc::UnboundedSender<ModeEvent>> =
    std::sync::OnceLock::new();

/// Register the shell's mode-change listener. Only the first registration
/// sticks (a server is started once per process); later ones are dropped.
pub fn set_mode_listener(tx: tokio::sync::mpsc::UnboundedSender<ModeEvent>) {
    let _ = MODE_LISTENER.set(tx);
}

/// Relay a `set_mode` event to the shell's listener. Returns false when no
/// listener is installed (e.g. unit tests, or a non-desktop server).
pub fn emit_mode_event(event: ModeEvent) -> bool {
    MODE_LISTENER
        .get()
        .is_some_and(|tx| tx.send(event).is_ok())
}

impl ServerHandle {
    pub async fn shutdown(self) {
        let _ = self.shutdown_sender.send(());
        let _ = self.serve_task.await;
    }

    pub async fn wait(self) {
        let _ = self.serve_task.await;
    }
}

pub async fn start_server(config: ServerConfig) -> anyhow::Result<ServerHandle> {
    // rfd only exists under the `desktop` feature; the headless server/Docker
    // build always uses the inert NullDialogProvider.
    #[cfg(feature = "desktop")]
    let dialogs: Arc<dyn crate::desktop_dialogs::FileDialogProvider> = if config.desktop_mode {
        Arc::new(crate::rfd_dialogs::RfdDialogProvider)
    } else {
        Arc::new(crate::desktop_dialogs::NullDialogProvider)
    };
    #[cfg(not(feature = "desktop"))]
    let dialogs: Arc<dyn crate::desktop_dialogs::FileDialogProvider> =
        Arc::new(crate::desktop_dialogs::NullDialogProvider);
    start_server_with(config, dialogs).await
}

/// Binds the listener before returning, so the port is already accepting
/// connections by the time the caller sees a `ServerHandle`.
pub async fn start_server_with(
    config: ServerConfig,
    dialogs: Arc<dyn crate::desktop_dialogs::FileDialogProvider>,
) -> anyhow::Result<ServerHandle> {
    let game_data = Arc::new(GameData::load(&config.data_dir.join("json"))?);
    let db = psp_db::open(&config.db_path).await?;
    let legacy_db_path = config
        .db_path
        .parent()
        .map(|dir| dir.join("psp.db"))
        .unwrap_or_else(|| std::path::PathBuf::from("psp.db"));
    let pal_data_validator = |value: &serde_json::Value| -> Result<serde_json::Value, String> {
        let dto =
            psp_core::dto::pal::PalDto::from_json_lenient(value).map_err(|e| e.to_string())?;
        serde_json::to_value(&dto).map_err(|e| e.to_string())
    };
    match psp_db::import_legacy::import_legacy_if_needed(&db, &legacy_db_path, &pal_data_validator)
        .await
    {
        Ok(Some(report)) => tracing::info!(?report, "legacy psp.db imported"),
        Ok(None) => {}
        Err(error) => {
            tracing::error!(%error, "legacy psp.db import failed; continuing with new DB")
        }
    }
    let (live_connections, live_connections_rx) = tokio::sync::watch::channel(0usize);
    // Both roots sit beside the database, the one directory the deployment
    // already guarantees is writable.
    let app_dir = config
        .db_path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let state = Arc::new(AppState {
        config: AppConfig {
            desktop_mode: config.desktop_mode,
        },
        game_data,
        driver: Arc::new(psp_db::SqlxSqliteDriver::new(db)),
        dialogs,
        live_connections,
        ext: Arc::new(crate::server_ext::ServerExtRouter {
            services: Arc::new(crate::services::ServerServices::real()),
        }),
        lsp: Arc::new(crate::lsp_service::ServerLspService::new(
            app_dir.join("lua-language-server"),
            app_dir.join("plugin-workspaces"),
        )),
        sessions: std::sync::Mutex::new(SessionStore::default()),
        breeding_db: Default::default(),
        plugins: Default::default(),
    });
    psp_app::handlers::plugins::seed_bundled_plugins(&state).await?;

    let listener = tokio::net::TcpListener::bind((config.host, config.port)).await?;
    let addr = listener.local_addr()?;
    tracing::info!(%addr, desktop_mode = config.desktop_mode, "psp-server listening");

    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel::<()>();
    let (shutdown_request_tx, mut shutdown_request_rx) = tokio::sync::watch::channel(false);
    let shutdown_requested = shutdown_request_tx.subscribe();
    // The first server in the process owns the shutdown message's channel;
    // later servers (parallel test servers) keep their own sender ALIVE — a
    // dropped sender resolves `changed()` with an error, which the graceful
    // select below treats as a stop request and would kill the server at
    // once. Deliberate leak with process-lifetime ownership semantics.
    if let Err(sender) = SHUTDOWN_REQUESTED.set(shutdown_request_tx) {
        std::mem::forget(sender);
    }
    let application = router::build_router(Arc::clone(&state), &config.ui_dir);
    let serve_task = tokio::spawn(async move {
        axum::serve(listener, application)
            .with_graceful_shutdown(async move {
                tokio::select! {
                    _ = shutdown_receiver => {},
                    _ = shutdown_request_rx.changed() => {},
                }
            })
            .await
    });

    Ok(ServerHandle {
        addr,
        app: state,
        live_connections: live_connections_rx,
        shutdown_requested,
        shutdown_sender,
        serve_task,
    })
}
