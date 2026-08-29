pub mod api_convert;
pub mod lsp_service;
#[cfg(feature = "desktop")]
pub mod rfd_dialogs;
pub mod router;
pub mod server_ext;
pub mod servers_handlers;
pub mod signal_handlers;
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
    shutdown_sender: tokio::sync::oneshot::Sender<()>,
    serve_task: tokio::task::JoinHandle<std::io::Result<()>>,
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
    let driver = Arc::new(psp_db::SqlxSqliteDriver::new(db));
    let signal_manager = crate::signal_setup(Arc::clone(&driver)).await;
    let state = Arc::new(AppState {
        config: AppConfig {
            desktop_mode: config.desktop_mode,
        },
        game_data,
        driver,
        dialogs,
        live_connections,
        ext: Arc::new(crate::server_ext::ServerExtRouter {
            services: Arc::new(crate::services::ServerServices::real()),
            signal: signal_manager,
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

/// A SignalManager over an in-memory store (port 0, not started) — for
/// tests and embeddings that don't need persistence.
pub async fn memory_signal_manager() -> Arc<psp_signal::manager::SignalManager> {
    let stored = psp_signal::store::SignalStored {
        port: 0,
        ..psp_signal::store::SignalStored::defaults()
    };
    Arc::new(
        psp_signal::manager::SignalManager::new(Box::new(
            psp_signal::store::MemorySignalStore::new(stored),
        ))
        .await,
    )
}

/// Builds the Signal manager from persisted settings, restoring the source
/// and auto-starting when it was enabled at shutdown. The REST password is
/// never restored (never stored) — the tab asks for it again.
async fn signal_setup(driver: Arc<psp_db::SqlxSqliteDriver>) -> Arc<psp_signal::manager::SignalManager> {
    struct DbStore(Arc<psp_db::SqlxSqliteDriver>);

    #[async_trait::async_trait]
    impl psp_signal::store::SignalStore for DbStore {
        async fn load(&self) -> psp_signal::store::SignalStored {
            psp_db::signal::get_signal_config(&*self.0)
                .await
                .map(|row| psp_signal::store::SignalStored {
                    enabled: row.enabled,
                    bind: row.bind,
                    port: row.port,
                    interval_ms: row.interval_ms,
                    allowed_origins: row.allowed_origins,
                    source_type: row.source_type,
                    source_url: row.source_url,
                    gamedata_path: row.gamedata_path,
                    token: row.token,
                })
                .unwrap_or_else(|error| {
                    tracing::warn!(%error, "signal config load failed; using defaults");
                    psp_signal::store::SignalStored::defaults()
                })
        }

        async fn save(&self, stored: &psp_signal::store::SignalStored) {
            let row = psp_db::signal::SignalConfigRow {
                enabled: stored.enabled,
                bind: stored.bind.clone(),
                port: stored.port,
                interval_ms: stored.interval_ms,
                allowed_origins: stored.allowed_origins.clone(),
                source_type: stored.source_type.clone(),
                source_url: stored.source_url.clone(),
                gamedata_path: stored.gamedata_path.clone(),
                token: stored.token.clone(),
            };
            if let Err(error) = psp_db::signal::save_signal_config(&*self.0, &row).await {
                tracing::warn!(%error, "signal config save failed");
            }
        }
    }

    let manager = psp_signal::manager::SignalManager::new(Box::new(DbStore(driver))).await;
    if let Some(source) = crate::signal_handlers::source_from_stored(&manager.stored().await) {
        if let Err(error) = manager.set_source(Some(source)).await {
            tracing::warn!(%error, "signal source restore failed");
        }
    }
    if manager.stored().await.enabled {
        if let Err(error) = manager.start().await {
            tracing::warn!(%error, "signal autostart failed");
        }
    }
    Arc::new(manager)
}
