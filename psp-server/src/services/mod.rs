//! Server-management services: Docker and native Palworld dedicated servers.
pub mod docker;
pub mod docker_mods;
pub mod native_config;
pub mod native_mods;
pub mod native_process;
pub mod palworld_api;

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
pub fn python_isoformat(timestamp: chrono::NaiveDateTime) -> String {
    if timestamp.and_utc().timestamp_subsec_micros() == 0 {
        timestamp.format("%Y-%m-%dT%H:%M:%S").to_string()
    } else {
        timestamp.format("%Y-%m-%dT%H:%M:%S%.6f").to_string()
    }
}

/// Stringifies env-var values the way the server image and PalWorldSettings.ini
/// expect them: bools as `True`/`False`, null as `None`.
pub fn python_str(value: &serde_json::Value) -> String {
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
}

impl ServerServices {
    pub fn real() -> Self {
        Self::with_docker(std::sync::Arc::new(LazyDocker::default()))
    }

    pub fn with_docker(docker: std::sync::Arc<dyn docker::DockerApi>) -> Self {
        Self {
            docker,
            palworld_api: palworld_api::PalworldApiClient::new(),
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
    fn python_isoformat_omits_zero_microseconds() {
        let whole = NaiveDate::from_ymd_opt(2026, 7, 9)
            .unwrap()
            .and_hms_opt(18, 22, 33)
            .unwrap();
        assert_eq!(python_isoformat(whole), "2026-07-09T18:22:33");
        let fractional = whole + chrono::Duration::microseconds(123456);
        assert_eq!(python_isoformat(fractional), "2026-07-09T18:22:33.123456");
    }

    #[test]
    fn python_str_formats_floats_without_trailing_zeros() {
        assert_eq!(python_str(&serde_json::json!("text")), "text");
        assert_eq!(python_str(&serde_json::json!(true)), "True");
        assert_eq!(python_str(&serde_json::json!(false)), "False");
        assert_eq!(python_str(&serde_json::json!(8211)), "8211");
        assert_eq!(python_str(&serde_json::json!(1.5)), "1.5");
        assert_eq!(python_str(&serde_json::Value::Null), "None");
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
        let services = ServerServices::with_docker(mock.clone());
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
