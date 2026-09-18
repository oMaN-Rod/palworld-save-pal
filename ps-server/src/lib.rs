pub mod api_convert;
pub mod bridge;
pub mod bridge_handlers;
pub mod bridge_instances_handlers;
pub mod local_saves_handlers;
pub mod lsp_service;
pub mod mod_target_service;
pub mod mods_conflict_handlers;
pub mod mods_framework_handlers;
pub mod mods_handlers;
pub mod mods_iostore_handlers;
pub mod mods_profile_entry_handlers;
pub mod mods_profile_handlers;
pub mod mods_share_handlers;
pub mod mods_upload_handlers;
pub mod mods_verification_handlers;
pub mod nexus_handlers;
#[cfg(feature = "desktop")]
pub mod rfd_dialogs;
pub mod router;
pub mod server_ext;
pub mod servers_handlers;
pub mod services;
pub mod signal;
pub mod signal_handlers;
pub mod static_files;
pub mod system_native;
pub mod ws;

pub use ps_app::{
    blueprint_registry, desktop_dialogs, dispatcher, emitter, envelope, handler_error, handlers,
    messages, AppConfig, AppState, SessionStore, SharedSession,
};

use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ps_core::gamedata::GameData;

const DB_FILE: &str = "ps-rs.db";
const LEGACY_DB_FILE: &str = "psp-rs.db";

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Takes over the database under its previous file name, WAL and shared-memory
/// siblings included, so no committed-but-uncheckpointed pages are left behind.
fn adopt_legacy_db_file(db_path: &Path) {
    if db_path.file_name() != Some(DB_FILE.as_ref()) || db_path.exists() {
        return;
    }
    let legacy = db_path.with_file_name(LEGACY_DB_FILE);
    if !legacy.is_file() {
        return;
    }
    for suffix in ["-wal", "-shm", ""] {
        let from = with_suffix(&legacy, suffix);
        if !from.exists() {
            continue;
        }
        if let Err(error) = std::fs::rename(&from, with_suffix(db_path, suffix)) {
            tracing::warn!(%error, "could not rename {}", from.display());
            return;
        }
    }
    tracing::info!("renamed {} to {}", legacy.display(), db_path.display());
}

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
    pub services: Arc<crate::services::ServerServices>,
    /// Subscriber on `AppState::live_connections`, seeded at 0 before any
    /// connection is accepted, so tests can await connection teardown instead
    /// of sleeping.
    pub live_connections: tokio::sync::watch::Receiver<usize>,
    _live_bus_keepalive: tokio::sync::watch::Receiver<Option<ps_app::live::LiveFrame>>,
    shutdown_sender: tokio::sync::oneshot::Sender<()>,
    serve_task: tokio::task::JoinHandle<std::io::Result<()>>,
    instance_reconciler_cancel: tokio_util::sync::CancellationToken,
    instance_reconciler_task: tokio::task::JoinHandle<()>,
    verifier_cancel: tokio_util::sync::CancellationToken,
    verifier_task: tokio::task::JoinHandle<()>,
}

impl ServerHandle {
    pub async fn shutdown(self) {
        // Stop the reconciler and verifier before the bridge, so neither can
        // call into a bridge that has already shut down.
        self.instance_reconciler_cancel.cancel();
        let _ = self.instance_reconciler_task.await;
        self.verifier_cancel.cancel();
        let _ = self.verifier_task.await;
        self.services.bridge.shutdown().await;
        self.services.signal.lock().await.shutdown().await;
        let _ = self.shutdown_sender.send(());
        let _ = self.serve_task.await;
    }

    pub async fn wait(self) {
        let _ = self.serve_task.await;
    }
}

const INSTANCE_RECONCILE_INTERVAL_ENV: &str = "PS_BRIDGE_RECONCILE_INTERVAL_MS";
const INSTANCE_RECONCILE_INTERVAL_DEFAULT: std::time::Duration = std::time::Duration::from_secs(3);

fn instance_reconcile_interval() -> std::time::Duration {
    std::env::var(INSTANCE_RECONCILE_INTERVAL_ENV)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(std::time::Duration::from_millis)
        .unwrap_or(INSTANCE_RECONCILE_INTERVAL_DEFAULT)
}

async fn run_instance_reconciler(
    driver: Arc<dyn ps_db::DbDriver>,
    bridge: Arc<crate::bridge::service::BridgeService>,
    cancel: tokio_util::sync::CancellationToken,
) {
    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = tokio::time::sleep(instance_reconcile_interval()) => {}
        }
        crate::bridge_instances_handlers::reconcile_active_target(&*driver, &bridge).await;
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
    start_server_with_services(
        config,
        dialogs,
        Arc::new(crate::services::ServerServices::real()),
    )
    .await
}

/// `start_server_with` taking an already-built `ServerServices` rather than
/// always constructing the real one, so a test can start a hermetic server
/// around a fake service (an `IoStoreConverter` that writes stub bytes, for
/// instance) instead of the production implementation.
pub async fn start_server_with_services(
    config: ServerConfig,
    dialogs: Arc<dyn crate::desktop_dialogs::FileDialogProvider>,
    services: Arc<crate::services::ServerServices>,
) -> anyhow::Result<ServerHandle> {
    let game_data = Arc::new(GameData::load(&config.data_dir.join("json"))?);
    adopt_legacy_db_file(&config.db_path);
    let db = ps_db::open(&config.db_path).await?;
    let legacy_db_path = config
        .db_path
        .parent()
        .map(|dir| dir.join("psp.db"))
        .unwrap_or_else(|| std::path::PathBuf::from("psp.db"));
    let pal_data_validator = |value: &serde_json::Value| -> Result<serde_json::Value, String> {
        let dto = ps_core::dto::pal::PalDto::from_json_lenient(value).map_err(|e| e.to_string())?;
        serde_json::to_value(&dto).map_err(|e| e.to_string())
    };
    match ps_db::import_legacy::import_legacy_if_needed(&db, &legacy_db_path, &pal_data_validator)
        .await
    {
        Ok(Some(report)) => tracing::info!(?report, "legacy psp.db imported"),
        Ok(None) => {}
        Err(error) => {
            tracing::error!(%error, "legacy psp.db import failed; continuing with new DB")
        }
    }
    let driver = ps_db::SqlxSqliteDriver::new(db.clone());
    match crate::mod_target_service::ensure_all(&driver, &services.app_root).await {
        Ok(created) if !created.is_empty() => {
            tracing::info!(?created, "created mod targets for existing servers")
        }
        Ok(_) => {}
        Err(error) => tracing::error!(%error, "mod target reconciliation failed; continuing"),
    }
    let (live_connections, live_connections_rx) = tokio::sync::watch::channel(0usize);
    let (live_bus, live_bus_keepalive) = tokio::sync::watch::channel(None);
    // Both roots sit beside the database, the one directory the deployment
    // already guarantees is writable.
    let app_dir = config
        .db_path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let uploads = Arc::new(crate::services::mods::uploads::UploadStore::new(&app_dir));
    let framework_scratch = crate::services::mods::frameworks::install::scratch_root(
        &crate::services::mods::LibraryPaths::new(&app_dir),
    );
    match std::fs::remove_dir_all(&framework_scratch) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            tracing::warn!(%error, ?framework_scratch, "failed to clear framework install scratch directory")
        }
    }
    tokio::spawn(crate::services::mods::uploads::sweep_while_alive(
        Arc::downgrade(&uploads),
    ));
    let state = Arc::new(AppState {
        config: AppConfig {
            desktop_mode: config.desktop_mode,
        },
        game_data,
        driver: Arc::new(ps_db::SqlxSqliteDriver::new(db)),
        dialogs,
        live_connections,
        live_bus,
        ext: Arc::new(crate::server_ext::ServerExtRouter {
            services: Arc::clone(&services),
            library: crate::services::mods::LibraryPaths::new(&app_dir),
            uploads,
        }),
        lsp: Arc::new(crate::lsp_service::ServerLspService::new(
            app_dir.join("lua-language-server"),
            app_dir.join("plugin-workspaces"),
        )),
        sessions: std::sync::Mutex::new(SessionStore::default()),
        breeding_db: Default::default(),
        plugins: Default::default(),
    });
    ps_app::handlers::plugins::seed_bundled_plugins(&state).await?;
    if let Err(error) = services.signal.lock().await.restore_armed(&state).await {
        tracing::warn!(%error, "signal: remote access was left armed but could not be restored");
    }
    services.bridge.start();

    let initial_target =
        crate::bridge_instances_handlers::resolve_active_target(&*state.driver).await;
    services.bridge.set_target(initial_target);

    let listener = tokio::net::TcpListener::bind((config.host, config.port)).await?;
    let addr = listener.local_addr()?;
    tracing::info!(%addr, desktop_mode = config.desktop_mode, "ps-server listening");

    let instance_reconciler_cancel = tokio_util::sync::CancellationToken::new();
    let instance_reconciler_task = tokio::spawn(run_instance_reconciler(
        Arc::clone(&state.driver),
        Arc::clone(&services.bridge),
        instance_reconciler_cancel.clone(),
    ));

    let verifier_cancel = tokio_util::sync::CancellationToken::new();
    let verifier_task = tokio::spawn(crate::services::mods::verifier::run_verifier(
        Arc::clone(&state.driver),
        Arc::clone(&services.bridge),
        Arc::clone(&services.verification),
        verifier_cancel.clone(),
    ));

    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel::<()>();
    let application = router::build_router(Arc::clone(&state), &config.ui_dir);
    let serve_task = tokio::spawn(async move {
        axum::serve(
            listener,
            application.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async {
            let _ = shutdown_receiver.await;
        })
        .await
    });

    Ok(ServerHandle {
        addr,
        app: state,
        services,
        live_connections: live_connections_rx,
        _live_bus_keepalive: live_bus_keepalive,
        shutdown_sender,
        serve_task,
        instance_reconciler_cancel,
        instance_reconciler_task,
        verifier_cancel,
        verifier_task,
    })
}

#[cfg(test)]
mod tests {
    use super::{adopt_legacy_db_file, DB_FILE, LEGACY_DB_FILE};

    #[test]
    fn adopts_the_legacy_db_with_its_wal() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(LEGACY_DB_FILE), b"main").unwrap();
        std::fs::write(dir.path().join(format!("{LEGACY_DB_FILE}-wal")), b"wal").unwrap();
        let db_path = dir.path().join(DB_FILE);

        adopt_legacy_db_file(&db_path);

        assert_eq!(std::fs::read(&db_path).unwrap(), b"main");
        assert_eq!(
            std::fs::read(dir.path().join(format!("{DB_FILE}-wal"))).unwrap(),
            b"wal"
        );
        assert!(!dir.path().join(LEGACY_DB_FILE).exists());
    }

    #[test]
    fn keeps_an_existing_db_and_ignores_custom_names() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(LEGACY_DB_FILE), b"old").unwrap();
        std::fs::write(dir.path().join(DB_FILE), b"new").unwrap();

        adopt_legacy_db_file(&dir.path().join(DB_FILE));
        adopt_legacy_db_file(&dir.path().join("custom.db"));

        assert_eq!(std::fs::read(dir.path().join(DB_FILE)).unwrap(), b"new");
        assert!(dir.path().join(LEGACY_DB_FILE).exists());
        assert!(!dir.path().join("custom.db").exists());
    }
}
