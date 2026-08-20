pub mod api_convert;
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
    /// 0 picks a free port (tests).
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
    shutdown_sender: tokio::sync::oneshot::Sender<()>,
    serve_task: tokio::task::JoinHandle<std::io::Result<()>>,
}

impl ServerHandle {
    /// Signals the serve loop to stop and waits for it to exit.
    pub async fn shutdown(self) {
        let _ = self.shutdown_sender.send(());
        let _ = self.serve_task.await;
    }

    /// Blocks until the server exits on its own.
    pub async fn wait(self) {
        let _ = self.serve_task.await;
    }
}

/// Picks the real `RfdDialogProvider` in desktop mode and the inert
/// `NullDialogProvider` otherwise, then defers to `start_server_with`.
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
/// connections by the time the caller sees a `ServerHandle`. `dialogs` lets
/// callers inject a `FileDialogProvider` of their own.
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
        sessions: std::sync::Mutex::new(SessionStore::default()),
        breeding_db: Default::default(),
        plugins: Default::default(),
    });
    psp_app::handlers::plugins::seed_bundled_plugins(&state).await?;

    let listener = tokio::net::TcpListener::bind((config.host, config.port)).await?;
    let addr = listener.local_addr()?;
    tracing::info!(%addr, desktop_mode = config.desktop_mode, "psp-server listening");

    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel::<()>();
    let application = router::build_router(Arc::clone(&state), &config.ui_dir);
    let serve_task = tokio::spawn(async move {
        axum::serve(listener, application)
            .with_graceful_shutdown(async {
                let _ = shutdown_receiver.await;
            })
            .await
    });

    Ok(ServerHandle {
        addr,
        app: state,
        live_connections: live_connections_rx,
        shutdown_sender,
        serve_task,
    })
}
