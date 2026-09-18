//! Server-management services: Docker and native Palworld dedicated servers.
pub mod docker;
pub mod language_server;
pub mod lsp_process;
pub mod lsp_workspace;
pub mod mods;
pub mod native_config;
pub mod native_process;
pub mod nexus;
pub mod palworld_api;
pub mod steam_news;

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("docker error: {0}")]
    Docker(String),
    #[error("http error: {0}")]
    Http(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

impl From<ServiceError> for crate::handler_error::HandlerError {
    fn from(err: ServiceError) -> Self {
        crate::handler_error::HandlerError::Other(err.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ServerProcessStatus {
    pub status: String,
    pub running: bool,
    pub started_at: Option<String>,
    pub health: Option<String>,
}

impl ServerProcessStatus {
    pub fn exited() -> Self {
        Self {
            status: "exited".to_string(),
            running: false,
            started_at: None,
            health: None,
        }
    }
    pub fn not_found() -> Self {
        Self {
            status: "not_found".to_string(),
            running: false,
            started_at: None,
            health: None,
        }
    }
}

/// Rounds stats values half-to-even (banker's rounding), the tie rule the
/// server-stats wire format is specified with — not `f64::round()`'s half-away.
pub fn round_to(value: f64, decimals: u32) -> f64 {
    let factor = 10f64.powi(decimals as i32);
    (value * factor).round_ties_even() / factor
}

/// ISO-8601 timestamp for the wire format: "T" separator, microseconds omitted
/// when zero.
pub fn iso_timestamp(timestamp: chrono::NaiveDateTime) -> String {
    if timestamp.and_utc().timestamp_subsec_micros() == 0 {
        timestamp.format("%Y-%m-%dT%H:%M:%S").to_string()
    } else {
        timestamp.format("%Y-%m-%dT%H:%M:%S%.6f").to_string()
    }
}

/// Stringifies env-var values the way the server image and PalWorldSettings.ini
/// expect them: bools as `True`/`False`, null as `None`.
pub fn env_value_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Bool(true) => "True".to_string(),
        serde_json::Value::Bool(false) => "False".to_string(),
        serde_json::Value::Null => "None".to_string(),
        other => other.to_string(),
    }
}

/// Held once in `AppState` behind an `Arc` so handlers share one Docker API
/// (real bollard in production, `mock::MockDocker` in tests) and REST client.
pub struct ServerServices {
    pub docker: std::sync::Arc<dyn docker::DockerApi>,
    pub palworld_api: palworld_api::PalworldApiClient,
    pub latest_version: steam_news::LatestVersion,
    pub signal: std::sync::Arc<tokio::sync::Mutex<crate::signal::manager::SignalManager>>,
    pub bridge: std::sync::Arc<crate::bridge::service::BridgeService>,
    /// Where Docker servers' default host folders live.
    pub app_root: std::path::PathBuf,
    pub relocation_retries: RelocationRetries,
    pub server_locks: ServerLocks,
    pub launcher: std::sync::Arc<dyn mods::launch::GameLauncher>,
    pub iostore: std::sync::Arc<dyn mods::iostore::IoStoreConverter>,
    /// Answers every running check in place of the process scan when set.
    pub running_override: Option<bool>,
    pub frameworks: std::sync::Arc<dyn mods::frameworks::source::FrameworkSource>,
    pub verification: std::sync::Arc<mods::verify::VerificationStore>,
    pub nexus_keys: std::sync::Arc<dyn nexus::keystore::NexusKeyStore>,
    pub nexus: std::sync::Arc<dyn nexus::api::NexusApi>,
    pub nexus_links: std::sync::Arc<nexus::links::NexusLinks>,
    pub protocol_registry: std::sync::Arc<dyn nexus::protocol::ProtocolRegistry>,
}

/// One async mutex per server, held across anything that removes, creates or
/// starts its container or moves its mods, so two of those never interleave.
#[derive(Default)]
pub struct ServerLocks {
    locks: std::sync::Mutex<std::collections::HashMap<i64, std::sync::Arc<tokio::sync::Mutex<()>>>>,
}

impl ServerLocks {
    pub fn of(&self, server_id: i64) -> std::sync::Arc<tokio::sync::Mutex<()>> {
        self.locks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry(server_id)
            .or_default()
            .clone()
    }
}

type Clock = Box<dyn Fn() -> std::time::Instant + Send + Sync>;

/// Rate-limits the relocation attempts a polled server listing makes, per
/// server, across every connection.
pub struct RelocationRetries {
    last_attempt: std::sync::Mutex<std::collections::HashMap<i64, std::time::Instant>>,
    clock: Clock,
}

impl RelocationRetries {
    pub const INTERVAL: std::time::Duration = std::time::Duration::from_secs(300);

    pub fn with_clock(clock: Clock) -> Self {
        Self {
            last_attempt: Default::default(),
            clock,
        }
    }

    /// Stamps the attempt before it runs, so a long move cannot let the next poll
    /// start a second one. False when an attempt was stamped within the interval.
    pub fn begin_attempt(&self, server_id: i64) -> bool {
        let now = (self.clock)();
        let mut last_attempt = self
            .last_attempt
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(previous) = last_attempt.get(&server_id) {
            if now.saturating_duration_since(*previous) < Self::INTERVAL {
                return false;
            }
        }
        last_attempt.insert(server_id, now);
        true
    }
}

impl Default for RelocationRetries {
    fn default() -> Self {
        Self::with_clock(Box::new(std::time::Instant::now))
    }
}

impl ServerServices {
    pub fn real() -> Self {
        Self::with_docker(
            std::sync::Arc::new(LazyDocker::default()),
            ps_core::paths::app_root(),
        )
    }

    pub fn with_docker(
        docker: std::sync::Arc<dyn docker::DockerApi>,
        app_root: impl Into<std::path::PathBuf>,
    ) -> Self {
        let app_root = app_root.into();
        Self {
            iostore: std::sync::Arc::new(mods::iostore::RetocConverter::github(&app_root)),
            app_root,
            relocation_retries: RelocationRetries::default(),
            server_locks: ServerLocks::default(),
            launcher: std::sync::Arc::new(mods::launch::SystemLauncher),
            running_override: None,
            frameworks: std::sync::Arc::new(mods::frameworks::github::ReleaseSources::github(
                std::env::var_os("PS_AMITY_BUNDLE_DIR").map(std::path::PathBuf::from),
            )),
            verification: std::sync::Arc::new(mods::verify::VerificationStore::new()),
            nexus_keys: nexus::keystore::system_key_store(),
            nexus: std::sync::Arc::new(nexus::api::HttpNexusApi::production()),
            nexus_links: std::sync::Arc::new(nexus::links::NexusLinks::default()),
            protocol_registry: nexus::protocol::system_registry(),
            docker,
            palworld_api: palworld_api::PalworldApiClient::new(),
            latest_version: steam_news::LatestVersion::new(),
            signal: std::sync::Arc::new(tokio::sync::Mutex::new(
                crate::signal::manager::SignalManager::new(),
            )),
            bridge: crate::bridge::service::BridgeService::new(),
        }
    }
}

/// Connects to Docker on first use; every method reports `ServiceError::Docker`
/// when the daemon is unreachable (server startup never fails on missing Docker).
#[derive(Default)]
pub struct LazyDocker {
    inner: tokio::sync::OnceCell<docker::BollardDocker>,
}

impl LazyDocker {
    async fn api(&self) -> Result<&docker::BollardDocker, ServiceError> {
        self.inner
            .get_or_try_init(|| async { docker::BollardDocker::connect() })
            .await
    }
}

#[async_trait::async_trait]
impl docker::DockerApi for LazyDocker {
    async fn ensure_image(&self, image_name: &str) -> Result<(), ServiceError> {
        self.api().await?.ensure_image(image_name).await
    }

    async fn create_and_start_container(
        &self,
        spec: docker::ContainerSpec,
    ) -> Result<String, ServiceError> {
        self.api().await?.create_and_start_container(spec).await
    }

    async fn create_container(&self, spec: docker::ContainerSpec) -> Result<String, ServiceError> {
        self.api().await?.create_container(spec).await
    }

    async fn start_container(&self, container_name: &str) -> Result<(), ServiceError> {
        self.api().await?.start_container(container_name).await
    }

    async fn stop_container(
        &self,
        container_name: &str,
        timeout_seconds: i64,
    ) -> Result<(), ServiceError> {
        self.api()
            .await?
            .stop_container(container_name, timeout_seconds)
            .await
    }

    async fn remove_container_forced(&self, container_name: &str) -> Result<(), ServiceError> {
        self.api()
            .await?
            .remove_container_forced(container_name)
            .await
    }

    async fn remove_volume(&self, volume_name: &str) -> Result<(), ServiceError> {
        self.api().await?.remove_volume(volume_name).await
    }

    async fn inspect_container(
        &self,
        container_name: &str,
    ) -> Result<Option<serde_json::Value>, ServiceError> {
        self.api().await?.inspect_container(container_name).await
    }

    async fn raw_container_stats(
        &self,
        container_name: &str,
    ) -> Result<Option<serde_json::Value>, ServiceError> {
        self.api().await?.raw_container_stats(container_name).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn round_to_uses_banker_rounding_for_stats_values() {
        assert_eq!(round_to(80.004, 2), 80.0);
        assert_eq!(round_to(1024.04, 1), 1024.0);
        // Exactly-representable halves (eighths) must round to even: 0.125 -> 0.12,
        // 0.375 -> 0.38. `f64::round()` (half-away) would give 0.13 for the first.
        assert_eq!(round_to(0.125, 2), 0.12);
        assert_eq!(round_to(0.375, 2), 0.38);
    }

    #[test]
    fn iso_timestamp_omits_zero_microseconds() {
        let whole = NaiveDate::from_ymd_opt(2026, 7, 9)
            .unwrap()
            .and_hms_opt(18, 22, 33)
            .unwrap();
        assert_eq!(iso_timestamp(whole), "2026-07-09T18:22:33");
        let fractional = whole + chrono::Duration::microseconds(123456);
        assert_eq!(iso_timestamp(fractional), "2026-07-09T18:22:33.123456");
    }

    #[test]
    fn env_value_text_renders_bools_and_null_the_way_the_server_image_reads_them() {
        assert_eq!(env_value_text(&serde_json::json!("text")), "text");
        assert_eq!(env_value_text(&serde_json::json!(true)), "True");
        assert_eq!(env_value_text(&serde_json::json!(false)), "False");
        assert_eq!(env_value_text(&serde_json::json!(8211)), "8211");
        assert_eq!(env_value_text(&serde_json::json!(1.5)), "1.5");
        assert_eq!(env_value_text(&serde_json::Value::Null), "None");
    }

    #[test]
    fn server_process_status_constructors() {
        assert_eq!(
            serde_json::to_value(ServerProcessStatus::not_found()).unwrap(),
            serde_json::json!({"status": "not_found", "running": false, "started_at": null, "health": null})
        );
        assert_eq!(
            serde_json::to_value(ServerProcessStatus::exited()).unwrap(),
            serde_json::json!({"status": "exited", "running": false, "started_at": null, "health": null})
        );
    }

    #[test]
    fn server_services_real_constructs_without_a_docker_daemon() {
        // Constructing ServerServices::real() must succeed with no Docker daemon
        // present. strong_count == 1 pins that each call yields a fresh, unshared
        // lazy client — it would exceed 1 if construction stashed it anywhere.
        let services = ServerServices::real();
        assert_eq!(std::sync::Arc::strong_count(&services.docker), 1);
    }

    #[tokio::test]
    async fn server_services_with_docker_delegates_to_injected_api() {
        let mock = std::sync::Arc::new(docker::mock::MockDocker::default());
        let services = ServerServices::with_docker(mock.clone(), "unused");
        services
            .docker
            .ensure_image("omanrod/psp-palworld-server")
            .await
            .unwrap();
        assert_eq!(
            mock.calls.lock().unwrap().clone(),
            vec!["ensure_image:omanrod/psp-palworld-server".to_string()]
        );
    }
}
