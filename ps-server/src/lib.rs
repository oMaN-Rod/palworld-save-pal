pub mod api_convert;
pub mod bridge;
pub mod bridge_handlers;
pub mod bridge_instances_handlers;
pub mod local_saves_handlers;
pub mod lsp_service;
pub mod network;
#[cfg(feature = "desktop")]
pub mod rfd_dialogs;
pub mod router;
pub mod server_ext;
pub mod servers_handlers;
pub mod service_control;
pub mod services;
pub mod signal;
pub mod signal_handlers;
pub mod static_files;
pub mod system_native;
pub mod ws;

pub use ps_app::{
    blueprint_registry, desktop_dialogs, dispatcher, emitter, envelope, handler_error, handlers,
    messages, network_policy, AppConfig, AppState, SessionStore, SharedSession,
};

use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

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
    /// Web default 0.0.0.0; desktop 127.0.0.1. Listen MODES are enforced
    /// per-peer (see ps-network), so this stays the raw bind address.
    pub host: IpAddr,
    /// Port override (CLI flag, install script, desktop app). `None` defers
    /// to the network policy's configured port so the in-app Network page
    /// can change it without fighting the command line.
    pub port: Option<u16>,
    pub ui_dir: PathBuf,
    /// Directory holding "json/" with the game data.
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    /// Enables native file dialogs and the local folder/browser handlers.
    pub desktop_mode: bool,
    /// A network-facing context (`serve`/`host` verb, service, container):
    /// full network policy. `false` marks a hand-launched local webapp,
    /// which is hard-clamped to localhost with only the port editable.
    pub hosted: bool,
}

pub struct ServerHandle {
    pub addr: SocketAddr,
    pub app: Arc<AppState>,
    pub services: Arc<crate::services::ServerServices>,
    pub network: Arc<crate::network::NetworkRuntime>,
    /// Subscriber on `AppState::live_connections`, seeded at 0 before any
    /// connection is accepted, so tests can await connection teardown instead
    /// of sleeping.
    pub live_connections: tokio::sync::watch::Receiver<usize>,
    _live_bus_keepalive: tokio::sync::watch::Receiver<Option<ps_app::live::LiveFrame>>,
    shutdown_sender: tokio::sync::oneshot::Sender<()>,
    serve_task: tokio::task::JoinHandle<std::io::Result<()>>,
    instance_reconciler_cancel: tokio_util::sync::CancellationToken,
    instance_reconciler_task: tokio::task::JoinHandle<()>,
    network_reconciler_cancel: tokio_util::sync::CancellationToken,
    network_reconciler_task: tokio::task::JoinHandle<()>,
    restart_flag: Arc<std::sync::atomic::AtomicBool>,
    exit_flag: Arc<std::sync::atomic::AtomicBool>,
}

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);

async fn stop_unit_task(mut task: tokio::task::JoinHandle<()>, name: &str) {
    if tokio::time::timeout(SHUTDOWN_TIMEOUT, &mut task)
        .await
        .is_err()
    {
        tracing::warn!(task = name, "background task did not stop; aborting");
        task.abort();
        let _ = task.await;
    }
}

/// Waits for the listener to end on its own — shutdown signal, port-change
/// rebind, or runtime-mode switch. This is the server's lifecycle clock and
/// must carry NO deadline: a healthy server runs indefinitely.
async fn join_serve_task(
    task: tokio::task::JoinHandle<std::io::Result<()>>,
) -> Result<(), String> {
    match task.await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(error.to_string()),
        Err(error) => Err(format!("listener task panicked or was cancelled: {error}")),
    }
}

/// Bounded stop for a listener whose shutdown was already requested; a slow
/// or stuck listener is aborted after the deadline. The join result is
/// consumed exactly once (re-awaiting a completed JoinHandle panics).
async fn stop_serve_task(
    mut task: tokio::task::JoinHandle<std::io::Result<()>>,
) -> Result<(), String> {
    let joined = match tokio::time::timeout(SHUTDOWN_TIMEOUT, &mut task).await {
        Ok(joined) => joined,
        Err(_) => {
            task.abort();
            task.await
        }
    };
    match joined {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(error.to_string()),
        Err(error) => Err(format!("listener task panicked or was cancelled: {error}")),
    }
}

async fn stop_services(services: &Arc<crate::services::ServerServices>) {
    if tokio::time::timeout(SHUTDOWN_TIMEOUT, services.bridge.shutdown())
        .await
        .is_err()
    {
        tracing::warn!("bridge service did not stop within the shutdown deadline");
    }
    let signal = Arc::clone(&services.signal);
    if tokio::time::timeout(SHUTDOWN_TIMEOUT, async move {
        signal.lock().await.shutdown().await;
    })
    .await
    .is_err()
    {
        tracing::warn!("signal service did not stop within the shutdown deadline");
    }
}

impl ServerHandle {
    pub async fn shutdown(self) {
        let _ = self.shutdown_sender.send(());
        self.instance_reconciler_cancel.cancel();
        self.network_reconciler_cancel.cancel();
        stop_unit_task(self.instance_reconciler_task, "instance reconciler").await;
        stop_unit_task(self.network_reconciler_task, "network reconciler").await;
        if let Err(error) = stop_serve_task(self.serve_task).await {
            tracing::warn!(%error, "server listener did not stop cleanly");
        }
        stop_services(&self.services).await;
    }

    pub async fn wait(self) {
        let _ = self.wait_or_restart().await;
    }

    /// True when the network policy changed the port and the caller should
    /// rebuild the listener (in-process "restart").
    pub fn restart_requested(&self) -> bool {
        self.restart_flag.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Waits for the listener to end, then tears down services. The reason
    /// tells main loops whether to rebind (port change), exit (runtime-mode
    /// switch), or simply finish (external shutdown).
    pub async fn wait_or_restart(self) -> ListenerExit {
        let ServerHandle {
            app: _,
            addr: _,
            services,
            network: _,
            live_connections: _,
            _live_bus_keepalive: _,
            shutdown_sender,
            serve_task,
            instance_reconciler_cancel,
            instance_reconciler_task,
            network_reconciler_cancel,
            network_reconciler_task,
            restart_flag,
            exit_flag,
        } = self;
        // The listener's task is the lifecycle clock: it ends on graceful
        // shutdown, a Network-page port change, or a runtime-mode switch.
        let listener_result = join_serve_task(serve_task).await;
        let reason = if let Err(error) = listener_result {
            tracing::error!(%error, "PalStudio listener failed");
            ListenerExit::Failed(error)
        } else if exit_flag.load(std::sync::atomic::Ordering::SeqCst) {
            ListenerExit::ExitRequested
        } else if restart_flag.load(std::sync::atomic::Ordering::SeqCst) {
            ListenerExit::RebindRequested
        } else {
            ListenerExit::Stopped
        };
        instance_reconciler_cancel.cancel();
        network_reconciler_cancel.cancel();
        stop_unit_task(instance_reconciler_task, "instance reconciler").await;
        stop_unit_task(network_reconciler_task, "network reconciler").await;
        let _ = shutdown_sender.send(());
        stop_services(&services).await;
        reason
    }
}

/// Why the listener ended — see [`ServerHandle::wait_or_restart`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListenerExit {
    /// External shutdown (ServerHandle::shutdown or signal): stop cleanly.
    Stopped,
    /// The Network page changed the port: rebuild the listener.
    RebindRequested,
    /// A runtime-mode switch (service_control): exit without rebinding.
    ExitRequested,
    /// The listener returned an I/O error or could not shut down cleanly.
    Failed(String),
}

const INSTANCE_RECONCILE_INTERVAL_ENV: &str = "PS_BRIDGE_RECONCILE_INTERVAL_MS";
const INSTANCE_RECONCILE_INTERVAL_DEFAULT: std::time::Duration = std::time::Duration::from_secs(3);
const NETWORK_RECONCILE_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

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

async fn run_network_reconciler(
    runtime: Arc<crate::network::NetworkRuntime>,
    cancel: tokio_util::sync::CancellationToken,
) {
    let mut first_run = true;
    loop {
        let delay = if first_run {
            Duration::from_secs(1)
        } else {
            NETWORK_RECONCILE_INTERVAL
        };
        first_run = false;
        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = tokio::time::sleep(delay) => {}
        }
        let config = runtime.effective_config();
        for failure in crate::network::reconcile_current_resources(&config).await {
            tracing::error!("network resource renewal failed; retaining current policy: {failure}");
        }
    }
}

async fn listener_bind_ip(
    config: &ServerConfig,
    network: &crate::network::NetworkRuntime,
) -> anyhow::Result<IpAddr> {
    match network.effective_config().listen {
        ps_network::ListenMode::Localhost => Ok(if config.host.is_ipv6() {
            IpAddr::from([0, 0, 0, 0, 0, 0, 0, 1])
        } else {
            IpAddr::from([127, 0, 0, 1])
        }),
        ps_network::ListenMode::Tailscale => {
            let status = tokio::task::spawn_blocking(ps_network::tailscale::detect)
                .await
                .map_err(|error| anyhow::anyhow!("Tailscale detection task failed: {error}"))?;
            anyhow::ensure!(
                status.available && status.logged_in,
                "Tailscale listen mode requires an available, logged-in Tailscale node"
            );
            status.ipv4.into_iter().next().ok_or_else(|| {
                anyhow::anyhow!("Tailscale listen mode requires an active Tailscale IPv4 address")
            })
        }
        ps_network::ListenMode::Lan | ps_network::ListenMode::Wan => {
            if config.host.is_loopback() || config.host.is_unspecified() {
                Ok(if config.host.is_ipv6() {
                    IpAddr::from([0, 0, 0, 0, 0, 0, 0, 0])
                } else {
                    IpAddr::from([0, 0, 0, 0])
                })
            } else {
                Ok(config.host)
            }
        }
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
    let (live_connections, live_connections_rx) = tokio::sync::watch::channel(0usize);
    let (live_bus, live_bus_keepalive) = tokio::sync::watch::channel(None);
    // Both roots sit beside the database, the one directory the deployment
    // already guarantees is writable.
    let app_dir = config
        .db_path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let driver = Arc::new(ps_db::SqlxSqliteDriver::new(db));
    // PalStudio's own network policy (listen mode/allowlists/PIN) loads
    // before the bind so the configured port is honored; listen MODES are
    // enforced per-request, so only a port edit needs a rebind.
    let tier = if config.desktop_mode {
        ps_network::NetworkTier::Desktop
    } else if config.hosted {
        ps_network::NetworkTier::Hosted
    } else {
        ps_network::NetworkTier::LocalWebapp
    };
    let network = Arc::new(
        crate::network::NetworkRuntime::load(&*driver)
            .await
            .map_err(|error| anyhow::anyhow!("could not load network config: {error}"))?
            .into_tier(tier),
    );
    if network.effective_config().listen == ps_network::ListenMode::Tailscale {
        network.refresh_tailnet_peers().await?;
    }
    // The desktop app is localhost by construction; every other context gets
    // a loud reminder when the stored policy leaves it exposed or locked out.
    if tier != ps_network::NetworkTier::Desktop {
        for warning in crate::network::security_warnings(&network.effective_config()) {
            tracing::warn!("network policy: {warning}");
        }
    }
    let effective_port = config
        .port
        .inspect(|override_port| {
            if *override_port != network.effective_port() {
                tracing::info!(
                    override_port,
                    configured_port = network.effective_port(),
                    "port override active; the Network page's port setting is ignored while it is set"
                );
            }
        })
        .unwrap_or_else(|| network.effective_port());
    let services = Arc::new(crate::services::ServerServices::real());
    let state = Arc::new(AppState {
        config: AppConfig {
            desktop_mode: config.desktop_mode,
        },
        game_data,
        driver,
        dialogs,
        live_connections,
        live_bus,
        ext: Arc::new(crate::server_ext::ServerExtRouter {
            services: Arc::clone(&services),
        }),
        lsp: Arc::new(crate::lsp_service::ServerLspService::new(
            app_dir.join("lua-language-server"),
            app_dir.join("plugin-workspaces"),
        )),
        sessions: std::sync::Mutex::new(SessionStore::default()),
        breeding_db: Default::default(),
        plugins: Default::default(),
        network_policy: Some(Arc::clone(&network) as Arc<dyn ps_app::network_policy::NetworkPolicy>),
    });
    ps_app::handlers::plugins::seed_bundled_plugins(&state).await?;
    // The port type is u16, so the only special value is 0: ask the OS for a
    // free ephemeral port (tests rely on it); the bound address reported
    // back always carries a concrete port.
    let bind_ip = listener_bind_ip(&config, &network).await?;
    let listener = tokio::net::TcpListener::bind((bind_ip, effective_port)).await?;
    let addr = listener.local_addr()?;
    tracing::info!(%addr, desktop_mode = config.desktop_mode, "ps-server listening");

    // Reconcile configured router/tailnet exposure after the socket is bound,
    // but before any background service is started. The environment not
    // matching the policy (no Tailscale CLI, no UPnP gateway) degrades the
    // deployment loudly rather than stopping the server from serving; the
    // listen policy itself still gates every peer.
    if network.tier() == ps_network::NetworkTier::Hosted {
        for failure in crate::network::reconcile_current_resources(&network.effective_config())
            .await
        {
            tracing::warn!("network exposure not fully established: {failure}");
        }
    }

    if let Err(error) = services.signal.lock().await.restore_armed(&state).await {
        tracing::warn!(%error, "signal: remote access was left armed but could not be restored");
    }
    services.bridge.start();

    let initial_target =
        crate::bridge_instances_handlers::resolve_active_target(&*state.driver).await;
    services.bridge.set_target(initial_target);

    let instance_reconciler_cancel = tokio_util::sync::CancellationToken::new();
    let instance_reconciler_task = tokio::spawn(run_instance_reconciler(
        Arc::clone(&state.driver),
        Arc::clone(&services.bridge),
        instance_reconciler_cancel.clone(),
    ));
    let network_reconciler_cancel = tokio_util::sync::CancellationToken::new();
    let network_reconciler_task = tokio::spawn(run_network_reconciler(
        Arc::clone(&network),
        network_reconciler_cancel.clone(),
    ));

    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel::<()>();
    let application =
        router::build_router(Arc::clone(&state), &config.ui_dir, Arc::clone(&network));
    let restart_notify = Arc::clone(&network);
    let exit_notify = Arc::clone(&network);
    let restart_flag = network.restart_flag();
    let exit_flag = network.exit_flag_handle();
    let serve_task = tokio::spawn(async move {
        axum::serve(
            listener,
            application.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async move {
            tokio::select! {
                _ = shutdown_receiver => {}
                // A port change from the Network page: end this listener so
                // the main loop rebinds with the new configuration.
                () = restart_notify.restart_wait() => {}
                // A runtime-mode switch (service_control): end the listener
                // AND let the main loop exit rather than rebind.
                () = exit_notify.exit_wait() => {}
            }
        })
        .await
    });

    Ok(ServerHandle {
        addr,
        app: state,
        services,
        network,
        live_connections: live_connections_rx,
        _live_bus_keepalive: live_bus_keepalive,
        shutdown_sender,
        serve_task,
        instance_reconciler_cancel,
        instance_reconciler_task,
        network_reconciler_cancel,
        network_reconciler_task,
        restart_flag,
        exit_flag,
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
