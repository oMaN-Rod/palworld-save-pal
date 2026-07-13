//! Docker server lifecycle: container spec builders, Engine-API JSON transforms,
//! and the `DockerApi` trait with its bollard implementation.
use psp_db::servers::ServerRecord;
use serde_json::Value;

use super::{python_str, round_to, ServerProcessStatus, ServiceError};

#[derive(Debug, Clone, PartialEq)]
pub struct PortBinding {
    pub container_port: String, // e.g. "8211/udp"
    pub host_port: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContainerSpec {
    pub name: String,
    pub image: String,
    pub env: Vec<String>,
    pub port_bindings: Vec<PortBinding>,
    pub binds: Vec<String>,
    pub restart_policy: String,
    pub stop_signal: String,
}

fn upsert_env(env: &mut Vec<(String, String)>, key: &str, value: String) {
    if let Some(existing) = env.iter_mut().find(|(existing_key, _)| existing_key == key) {
        existing.1 = value;
    } else {
        env.push((key.to_string(), value));
    }
}

/// User-supplied `env_vars` come first; the explicit server fields below always
/// override them.
pub fn build_environment(record: &ServerRecord) -> Vec<String> {
    let mut env: Vec<(String, String)> = record
        .env_vars
        .0
        .iter()
        .map(|(key, value)| (key.clone(), python_str(value)))
        .collect();
    upsert_env(&mut env, "PORT", record.game_port.to_string());
    upsert_env(&mut env, "QUERY_PORT", record.query_port.to_string());
    upsert_env(&mut env, "PLAYERS", record.max_players.to_string());
    upsert_env(&mut env, "SERVER_NAME", record.server_name.clone());
    upsert_env(
        &mut env,
        "SERVER_DESCRIPTION",
        record.server_description.clone(),
    );
    upsert_env(&mut env, "ADMIN_PASSWORD", record.admin_password.clone());
    upsert_env(&mut env, "REST_API_ENABLED", "true".to_string());
    upsert_env(&mut env, "REST_API_PORT", record.rest_api_port.to_string());
    if !record.server_password.is_empty() {
        upsert_env(&mut env, "SERVER_PASSWORD", record.server_password.clone());
    }
    env.into_iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect()
}

pub fn build_port_bindings(record: &ServerRecord) -> Vec<PortBinding> {
    vec![
        PortBinding {
            container_port: format!("{}/udp", record.game_port),
            host_port: record.game_port as u16,
        },
        PortBinding {
            container_port: format!("{}/udp", record.query_port),
            host_port: record.query_port as u16,
        },
        PortBinding {
            container_port: format!("{}/tcp", record.rest_api_port),
            host_port: record.rest_api_port as u16,
        },
    ]
}

pub fn build_binds(record: &ServerRecord) -> Vec<String> {
    vec![
        format!("{}:/palworld/:rw", record.data_volume_name),
        format!("{}:/palworld/Pal/Saved/:rw", record.saves_path),
        format!("{}:/palworld/Pal/Binaries/Win64/Mods/:rw", record.mods_path),
        format!(
            "{}:/palworld/Pal/Content/Paks/LogicMods/:rw",
            record.logicmods_path
        ),
        format!("{}:/palworld/nativemods/:rw", record.nativemods_path),
    ]
}

pub fn container_spec(record: &ServerRecord) -> ContainerSpec {
    ContainerSpec {
        name: record.container_name.clone(),
        image: record.image_name.clone(),
        env: build_environment(record),
        port_bindings: build_port_bindings(record),
        binds: build_binds(record),
        restart_policy: "unless-stopped".to_string(),
        stop_signal: "SIGTERM".to_string(),
    }
}

/// Docker only emits `State.Health` for images that declare a HEALTHCHECK, so an
/// absent block means "unknown", not "unhealthy".
pub fn status_from_inspect(inspect: &Value) -> ServerProcessStatus {
    let state = inspect.get("State").cloned().unwrap_or(Value::Null);
    ServerProcessStatus {
        status: state
            .get("Status")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        running: state
            .get("Running")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        started_at: state
            .get("StartedAt")
            .and_then(Value::as_str)
            .map(str::to_string),
        health: state
            .get("Health")
            .and_then(|health| health.get("Status"))
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

/// CPU percent needs both the current and previous sample; the very first stats
/// sample after a container starts has no `precpu_stats`, hence `None`.
pub fn stats_from_raw(raw_stats: &Value) -> Option<Value> {
    let cpu_total = raw_stats
        .pointer("/cpu_stats/cpu_usage/total_usage")?
        .as_f64()?;
    let precpu_total = raw_stats
        .pointer("/precpu_stats/cpu_usage/total_usage")?
        .as_f64()?;
    let system_usage = raw_stats.pointer("/cpu_stats/system_cpu_usage")?.as_f64()?;
    let presystem_usage = raw_stats
        .pointer("/precpu_stats/system_cpu_usage")?
        .as_f64()?;
    let cpu_delta = cpu_total - precpu_total;
    let system_delta = system_usage - presystem_usage;
    let online_cpus = raw_stats
        .pointer("/cpu_stats/online_cpus")
        .and_then(Value::as_f64)
        .unwrap_or(1.0);
    let cpu_percent = if system_delta > 0.0 {
        (cpu_delta / system_delta) * online_cpus * 100.0
    } else {
        0.0
    };

    let mem_usage = raw_stats
        .pointer("/memory_stats/usage")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let mem_limit = raw_stats
        .pointer("/memory_stats/limit")
        .and_then(Value::as_f64)
        .unwrap_or(1.0);
    let mem_percent = if mem_limit > 0.0 {
        (mem_usage / mem_limit) * 100.0
    } else {
        0.0
    };

    let empty_networks = serde_json::Map::new();
    let networks = raw_stats
        .get("networks")
        .and_then(Value::as_object)
        .unwrap_or(&empty_networks);
    let net_rx: f64 = networks
        .values()
        .filter_map(|iface| iface.get("rx_bytes").and_then(Value::as_f64))
        .sum();
    let net_tx: f64 = networks
        .values()
        .filter_map(|iface| iface.get("tx_bytes").and_then(Value::as_f64))
        .sum();

    let empty_blkio = Vec::new();
    let blkio_entries = raw_stats
        .pointer("/blkio_stats/io_service_bytes_recursive")
        .and_then(Value::as_array)
        .unwrap_or(&empty_blkio);
    let sum_blkio = |operation: &str| -> f64 {
        blkio_entries
            .iter()
            .filter(|entry| entry.get("op").and_then(Value::as_str) == Some(operation))
            .filter_map(|entry| entry.get("value").and_then(Value::as_f64))
            .sum()
    };
    let disk_read = sum_blkio("read");
    let disk_write = sum_blkio("write");

    const MB: f64 = 1024.0 * 1024.0;
    Some(serde_json::json!({
        "cpu_percent": round_to(cpu_percent, 2),
        "mem_usage_mb": round_to(mem_usage / MB, 1),
        "mem_limit_mb": round_to(mem_limit / MB, 1),
        "mem_percent": round_to(mem_percent, 1),
        "net_rx_mb": round_to(net_rx / MB, 2),
        "net_tx_mb": round_to(net_tx / MB, 2),
        "disk_read_mb": round_to(disk_read / MB, 2),
        "disk_write_mb": round_to(disk_write / MB, 2)
    }))
}

#[async_trait::async_trait]
pub trait DockerApi: Send + Sync {
    async fn ensure_image(&self, image_name: &str) -> Result<(), ServiceError>;
    async fn create_and_start_container(&self, spec: ContainerSpec)
        -> Result<String, ServiceError>;
    async fn start_container(&self, container_name: &str) -> Result<(), ServiceError>;
    async fn stop_container(
        &self,
        container_name: &str,
        timeout_seconds: i64,
    ) -> Result<(), ServiceError>;
    async fn remove_container_forced(&self, container_name: &str) -> Result<(), ServiceError>;
    async fn remove_volume(&self, volume_name: &str) -> Result<(), ServiceError>;
    async fn inspect_container(&self, container_name: &str) -> Result<Option<Value>, ServiceError>;
    async fn raw_container_stats(
        &self,
        container_name: &str,
    ) -> Result<Option<Value>, ServiceError>;
}

/// The bind-mount host directories must exist before the container starts, or
/// Docker creates them root-owned.
pub async fn create_server_container(
    api: &dyn DockerApi,
    record: &ServerRecord,
) -> Result<String, ServiceError> {
    api.ensure_image(&record.image_name).await?;
    for host_path in [
        &record.saves_path,
        &record.mods_path,
        &record.logicmods_path,
        &record.nativemods_path,
    ] {
        std::fs::create_dir_all(host_path)?;
    }
    api.create_and_start_container(container_spec(record)).await
}

pub async fn start_server_container(api: &dyn DockerApi, container_name: &str) -> bool {
    api.start_container(container_name).await.is_ok()
}

pub async fn stop_server_container(api: &dyn DockerApi, container_name: &str) -> bool {
    api.stop_container(container_name, 30).await.is_ok()
}

/// `DockerApi::remove_volume` folds "volume not found" into `Ok(())`, so any
/// `Err` here is a genuine removal failure and must report `false` even though
/// the container itself was already removed.
pub async fn remove_server_container(
    api: &dyn DockerApi,
    container_name: &str,
    remove_volumes: bool,
) -> bool {
    match api.remove_container_forced(container_name).await {
        Ok(()) => {
            if remove_volumes {
                api.remove_volume(&format!("psp-{container_name}-data"))
                    .await
                    .is_ok()
            } else {
                true
            }
        }
        Err(_) => false,
    }
}

/// A missing container is a `not_found` status; `None` means Docker itself could
/// not be reached.
pub async fn container_status(
    api: &dyn DockerApi,
    container_name: &str,
) -> Option<ServerProcessStatus> {
    match api.inspect_container(container_name).await {
        Ok(Some(inspect)) => Some(status_from_inspect(&inspect)),
        Ok(None) => Some(ServerProcessStatus::not_found()),
        Err(_) => None,
    }
}

/// Stats are only meaningful for a running container; a stopped one reports zeros.
pub async fn container_stats(api: &dyn DockerApi, container_name: &str) -> Option<Value> {
    let inspect = api.inspect_container(container_name).await.ok()??;
    if status_from_inspect(&inspect).status != "running" {
        return None;
    }
    let raw = api.raw_container_stats(container_name).await.ok()??;
    stats_from_raw(&raw)
}

pub struct BollardDocker {
    docker: bollard::Docker,
}

impl BollardDocker {
    pub fn connect() -> Result<Self, ServiceError> {
        bollard::Docker::connect_with_local_defaults()
            .map(|docker| Self { docker })
            .map_err(|error| ServiceError::Docker(error.to_string()))
    }
}

fn is_not_found(error: &bollard::errors::Error) -> bool {
    matches!(
        error,
        bollard::errors::Error::DockerResponseServerError {
            status_code: 404,
            ..
        }
    )
}

fn docker_err(error: bollard::errors::Error) -> ServiceError {
    ServiceError::Docker(error.to_string())
}

// bollard 0.18 keeps the per-call `*Options` structs in bollard::container /
// ::image / ::volume, and the create-container body is bollard::container::Config
// — newer bollard releases move these to bollard::query_parameters and
// bollard::models::ContainerCreateBody, so upgrading requires rewriting the calls.
#[async_trait::async_trait]
impl DockerApi for BollardDocker {
    async fn ensure_image(&self, image_name: &str) -> Result<(), ServiceError> {
        use futures_util::StreamExt;
        if self.docker.inspect_image(image_name).await.is_ok() {
            return Ok(());
        }
        let options = bollard::image::CreateImageOptions {
            from_image: image_name,
            ..Default::default()
        };
        let mut pull_progress = self.docker.create_image(Some(options), None, None);
        while let Some(step) = pull_progress.next().await {
            step.map_err(docker_err)?;
        }
        Ok(())
    }

    async fn create_and_start_container(
        &self,
        spec: ContainerSpec,
    ) -> Result<String, ServiceError> {
        use std::collections::HashMap;
        let mut port_bindings = HashMap::new();
        let mut exposed_ports = HashMap::new();
        for binding in &spec.port_bindings {
            exposed_ports.insert(binding.container_port.clone(), HashMap::new());
            port_bindings.insert(
                binding.container_port.clone(),
                Some(vec![bollard::models::PortBinding {
                    host_ip: None,
                    host_port: Some(binding.host_port.to_string()),
                }]),
            );
        }
        let config = bollard::container::Config {
            image: Some(spec.image.clone()),
            env: Some(spec.env.clone()),
            stop_signal: Some(spec.stop_signal.clone()),
            exposed_ports: Some(exposed_ports),
            host_config: Some(bollard::models::HostConfig {
                binds: Some(spec.binds.clone()),
                port_bindings: Some(port_bindings),
                restart_policy: Some(bollard::models::RestartPolicy {
                    name: Some(bollard::models::RestartPolicyNameEnum::UNLESS_STOPPED),
                    maximum_retry_count: None,
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let options = bollard::container::CreateContainerOptions {
            name: spec.name.clone(),
            platform: None,
        };
        let created = self
            .docker
            .create_container(Some(options), config)
            .await
            .map_err(docker_err)?;
        self.docker
            .start_container(
                &spec.name,
                None::<bollard::container::StartContainerOptions<String>>,
            )
            .await
            .map_err(docker_err)?;
        Ok(created.id)
    }

    async fn start_container(&self, container_name: &str) -> Result<(), ServiceError> {
        self.docker
            .start_container(
                container_name,
                None::<bollard::container::StartContainerOptions<String>>,
            )
            .await
            .map_err(docker_err)
    }

    async fn stop_container(
        &self,
        container_name: &str,
        timeout_seconds: i64,
    ) -> Result<(), ServiceError> {
        let options = bollard::container::StopContainerOptions { t: timeout_seconds };
        self.docker
            .stop_container(container_name, Some(options))
            .await
            .map_err(docker_err)
    }

    async fn remove_container_forced(&self, container_name: &str) -> Result<(), ServiceError> {
        let options = bollard::container::RemoveContainerOptions {
            force: true,
            ..Default::default()
        };
        self.docker
            .remove_container(container_name, Some(options))
            .await
            .map_err(docker_err)
    }

    async fn remove_volume(&self, volume_name: &str) -> Result<(), ServiceError> {
        match self.docker.remove_volume(volume_name, None).await {
            Ok(()) => Ok(()),
            Err(error) if is_not_found(&error) => Ok(()),
            Err(error) => Err(docker_err(error)),
        }
    }

    async fn inspect_container(&self, container_name: &str) -> Result<Option<Value>, ServiceError> {
        match self
            .docker
            .inspect_container(
                container_name,
                None::<bollard::container::InspectContainerOptions>,
            )
            .await
        {
            Ok(inspect) => {
                Ok(Some(serde_json::to_value(inspect).map_err(|error| {
                    ServiceError::Docker(error.to_string())
                })?))
            }
            Err(error) if is_not_found(&error) => Ok(None),
            Err(error) => Err(docker_err(error)),
        }
    }

    async fn raw_container_stats(
        &self,
        container_name: &str,
    ) -> Result<Option<Value>, ServiceError> {
        use futures_util::StreamExt;
        // one_shot must stay false: dockerd then waits for a real second sample, so
        // cpu_stats/precpu_stats both carry usable deltas. With one_shot=true the
        // single returned sample has an empty precpu_stats and CPU percent is always 0.
        let options = bollard::container::StatsOptions {
            stream: false,
            one_shot: false,
        };
        let mut stats_stream = self.docker.stats(container_name, Some(options));
        match stats_stream.next().await {
            Some(Ok(stats)) => {
                Ok(Some(serde_json::to_value(stats).map_err(|error| {
                    ServiceError::Docker(error.to_string())
                })?))
            }
            Some(Err(error)) if is_not_found(&error) => Ok(None),
            Some(Err(error)) => Err(docker_err(error)),
            None => Ok(None),
        }
    }
}

/// Scripted in-memory DockerApi. Lives outside `cfg(test)` so handler tests in
/// other modules can inject it into AppState.
pub mod mock {
    use std::collections::{HashMap, HashSet};
    use std::sync::Mutex;

    use serde_json::Value;

    use super::{ContainerSpec, DockerApi};
    use crate::services::ServiceError;

    #[derive(Default)]
    pub struct MockDocker {
        /// container_name -> inspect JSON ({"State": {...}})
        pub statuses: Mutex<HashMap<String, Value>>,
        /// container_name -> raw stats JSON
        pub stats: Mutex<HashMap<String, Value>>,
        /// Ordered call log: "ensure_image:x", "create_and_start:x", "start:x", ...
        pub calls: Mutex<Vec<String>>,
        pub fail_start: Mutex<HashSet<String>>,
        pub fail_stop: Mutex<HashSet<String>>,
        pub fail_inspect: Mutex<HashSet<String>>,
        /// Bare volume names (e.g. "psp-alpha-data") whose removal fails with a
        /// non-NotFound error.
        pub fail_remove_volume: Mutex<HashSet<String>>,
    }

    fn running_state() -> Value {
        serde_json::json!({
            "State": {"Status": "running", "Running": true, "StartedAt": "2026-07-09T00:00:00Z"}
        })
    }

    #[async_trait::async_trait]
    impl DockerApi for MockDocker {
        async fn ensure_image(&self, image_name: &str) -> Result<(), ServiceError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("ensure_image:{image_name}"));
            Ok(())
        }

        async fn create_and_start_container(
            &self,
            spec: ContainerSpec,
        ) -> Result<String, ServiceError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("create_and_start:{}", spec.name));
            self.statuses
                .lock()
                .unwrap()
                .insert(spec.name.clone(), running_state());
            Ok(format!("mock-{}", spec.name))
        }

        async fn start_container(&self, container_name: &str) -> Result<(), ServiceError> {
            if self.fail_start.lock().unwrap().contains(container_name) {
                return Err(ServiceError::Docker("mock start failure".to_string()));
            }
            self.calls
                .lock()
                .unwrap()
                .push(format!("start:{container_name}"));
            self.statuses
                .lock()
                .unwrap()
                .insert(container_name.to_string(), running_state());
            Ok(())
        }

        async fn stop_container(
            &self,
            container_name: &str,
            _timeout_seconds: i64,
        ) -> Result<(), ServiceError> {
            if self.fail_stop.lock().unwrap().contains(container_name) {
                return Err(ServiceError::Docker("mock stop failure".to_string()));
            }
            self.calls
                .lock()
                .unwrap()
                .push(format!("stop:{container_name}"));
            self.statuses.lock().unwrap().insert(
                container_name.to_string(),
                serde_json::json!({"State": {"Status": "exited", "Running": false, "StartedAt": null}}),
            );
            Ok(())
        }

        async fn remove_container_forced(&self, container_name: &str) -> Result<(), ServiceError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("remove_container:{container_name}"));
            self.statuses.lock().unwrap().remove(container_name);
            Ok(())
        }

        async fn remove_volume(&self, volume_name: &str) -> Result<(), ServiceError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("remove_volume:{volume_name}"));
            if self
                .fail_remove_volume
                .lock()
                .unwrap()
                .contains(volume_name)
            {
                return Err(ServiceError::Docker(
                    "mock volume removal failure".to_string(),
                ));
            }
            Ok(())
        }

        async fn inspect_container(
            &self,
            container_name: &str,
        ) -> Result<Option<Value>, ServiceError> {
            if self.fail_inspect.lock().unwrap().contains(container_name) {
                return Err(ServiceError::Docker("mock inspect failure".to_string()));
            }
            Ok(self.statuses.lock().unwrap().get(container_name).cloned())
        }

        async fn raw_container_stats(
            &self,
            container_name: &str,
        ) -> Result<Option<Value>, ServiceError> {
            Ok(self.stats.lock().unwrap().get(container_name).cloned())
        }
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use psp_db::servers::ServerRecord;

    pub(crate) fn docker_record() -> ServerRecord {
        let timestamp = "2026-07-09T00:00:00".to_string();
        let mut env_vars = serde_json::Map::new();
        env_vars.insert("EXP_RATE".to_string(), serde_json::json!("2.0"));
        env_vars.insert("MULTITHREADING".to_string(), serde_json::json!(true));
        ServerRecord {
            id: 1,
            name: "My Server".to_string(),
            container_name: "alpha".to_string(),
            image_name: "omanrod/psp-palworld-server".to_string(),
            server_type: "docker".to_string(),
            game_port: 8211,
            query_port: 27015,
            rest_api_port: 8212,
            data_volume_name: "psp-alpha-data".to_string(),
            saves_path: "/srv/alpha/saves".to_string(),
            mods_path: "/srv/alpha/mods".to_string(),
            logicmods_path: "/srv/alpha/logicmods".to_string(),
            nativemods_path: "/srv/alpha/nativemods".to_string(),
            install_path: String::new(),
            steamcmd_path: String::new(),
            pid: None,
            launch_args: String::new(),
            workshop_dir: String::new(),
            server_name: "PSP Palworld Server".to_string(),
            server_description: "desc".to_string(),
            server_password: "pw".to_string(),
            admin_password: "admin".to_string(),
            max_players: 16,
            env_vars: sqlx::types::Json(env_vars),
            created_at: timestamp.clone(),
            updated_at: timestamp,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::ServerProcessStatus;

    #[test]
    fn build_environment_applies_overrides_after_user_env_vars() {
        let record = test_support::docker_record();
        let env = build_environment(&record);
        assert_eq!(env[0], "EXP_RATE=2.0");
        assert_eq!(env[1], "MULTITHREADING=True");
        assert!(env.contains(&"PORT=8211".to_string()));
        assert!(env.contains(&"QUERY_PORT=27015".to_string()));
        assert!(env.contains(&"PLAYERS=16".to_string()));
        assert!(env.contains(&"SERVER_NAME=PSP Palworld Server".to_string()));
        assert!(env.contains(&"SERVER_DESCRIPTION=desc".to_string()));
        assert!(env.contains(&"ADMIN_PASSWORD=admin".to_string()));
        assert!(env.contains(&"REST_API_ENABLED=true".to_string()));
        assert!(env.contains(&"REST_API_PORT=8212".to_string()));
        assert!(env.contains(&"SERVER_PASSWORD=pw".to_string()));
    }

    #[test]
    fn build_environment_omits_empty_server_password() {
        let mut record = test_support::docker_record();
        record.server_password = String::new();
        let env = build_environment(&record);
        assert!(!env
            .iter()
            .any(|entry| entry.starts_with("SERVER_PASSWORD=")));
    }

    #[test]
    fn build_environment_field_overrides_replace_user_env_vars() {
        let mut record = test_support::docker_record();
        record
            .env_vars
            .0
            .insert("PORT".to_string(), serde_json::json!("9999"));
        let env = build_environment(&record);
        let port_entries: Vec<_> = env.iter().filter(|e| e.starts_with("PORT=")).collect();
        assert_eq!(port_entries, vec!["PORT=8211"]);
    }

    #[test]
    fn build_port_bindings_maps_udp_udp_tcp() {
        let bindings = build_port_bindings(&test_support::docker_record());
        assert_eq!(bindings[0].container_port, "8211/udp");
        assert_eq!(bindings[0].host_port, 8211);
        assert_eq!(bindings[1].container_port, "27015/udp");
        assert_eq!(bindings[2].container_port, "8212/tcp");
        assert_eq!(bindings[2].host_port, 8212);
    }

    #[test]
    fn build_binds_covers_volume_and_four_host_paths() {
        let binds = build_binds(&test_support::docker_record());
        assert_eq!(
            binds,
            vec![
                "psp-alpha-data:/palworld/:rw",
                "/srv/alpha/saves:/palworld/Pal/Saved/:rw",
                "/srv/alpha/mods:/palworld/Pal/Binaries/Win64/Mods/:rw",
                "/srv/alpha/logicmods:/palworld/Pal/Content/Paks/LogicMods/:rw",
                "/srv/alpha/nativemods:/palworld/nativemods/:rw",
            ]
        );
    }

    #[test]
    fn container_spec_sets_restart_policy_and_stop_signal() {
        let spec = container_spec(&test_support::docker_record());
        assert_eq!(spec.name, "alpha");
        assert_eq!(spec.image, "omanrod/psp-palworld-server");
        assert_eq!(spec.restart_policy, "unless-stopped");
        assert_eq!(spec.stop_signal, "SIGTERM");
    }

    #[test]
    fn status_from_inspect_extracts_state_fields() {
        let inspect = serde_json::json!({
            "State": {
                "Status": "running",
                "Running": true,
                "StartedAt": "2026-07-09T12:00:00.123456789Z",
                "Health": {"Status": "healthy"}
            }
        });
        assert_eq!(
            status_from_inspect(&inspect),
            ServerProcessStatus {
                status: "running".to_string(),
                running: true,
                started_at: Some("2026-07-09T12:00:00.123456789Z".to_string()),
                health: Some("healthy".to_string()),
            }
        );
    }

    #[test]
    fn status_from_inspect_handles_missing_health() {
        let inspect = serde_json::json!({
            "State": {"Status": "exited", "Running": false, "StartedAt": "x"}
        });
        let status = status_from_inspect(&inspect);
        assert_eq!(status.status, "exited");
        assert!(!status.running);
        assert_eq!(status.health, None);
    }

    #[test]
    fn stats_from_raw_computes_python_stats_dict() {
        let raw = serde_json::json!({
            "cpu_stats": {
                "cpu_usage": {"total_usage": 400_000_000u64},
                "system_cpu_usage": 2_000_000_000u64,
                "online_cpus": 4
            },
            "precpu_stats": {
                "cpu_usage": {"total_usage": 200_000_000u64},
                "system_cpu_usage": 1_000_000_000u64
            },
            "memory_stats": {"usage": 1_073_741_824u64, "limit": 4_294_967_296u64},
            "networks": {"eth0": {"rx_bytes": 1_048_576u64, "tx_bytes": 2_097_152u64}},
            "blkio_stats": {"io_service_bytes_recursive": [
                {"op": "read", "value": 3_145_728u64},
                {"op": "write", "value": 1_048_576u64}
            ]}
        });
        assert_eq!(
            stats_from_raw(&raw).unwrap(),
            serde_json::json!({
                "cpu_percent": 80.0,
                "mem_usage_mb": 1024.0,
                "mem_limit_mb": 4096.0,
                "mem_percent": 25.0,
                "net_rx_mb": 1.0,
                "net_tx_mb": 2.0,
                "disk_read_mb": 3.0,
                "disk_write_mb": 1.0
            })
        );
    }

    #[test]
    fn stats_from_raw_returns_none_when_required_cpu_fields_missing() {
        // The first stats sample after start lacks precpu system_cpu_usage.
        let raw = serde_json::json!({
            "cpu_stats": {"cpu_usage": {"total_usage": 1u64}},
            "precpu_stats": {"cpu_usage": {"total_usage": 0u64}}
        });
        assert!(stats_from_raw(&raw).is_none());
    }

    #[tokio::test]
    async fn create_server_container_pulls_image_makes_dirs_and_starts() {
        let scratch = tempfile::tempdir().unwrap();
        let mut record = test_support::docker_record();
        record.saves_path = scratch.path().join("saves").to_string_lossy().to_string();
        record.mods_path = scratch.path().join("mods").to_string_lossy().to_string();
        record.logicmods_path = scratch
            .path()
            .join("logicmods")
            .to_string_lossy()
            .to_string();
        record.nativemods_path = scratch
            .path()
            .join("nativemods")
            .to_string_lossy()
            .to_string();
        let api = mock::MockDocker::default();
        let container_id = create_server_container(&api, &record).await.unwrap();
        assert_eq!(container_id, "mock-alpha");
        assert!(std::path::Path::new(&record.saves_path).is_dir());
        assert!(std::path::Path::new(&record.nativemods_path).is_dir());
        let calls = api.calls.lock().unwrap().clone();
        assert_eq!(
            calls,
            vec![
                "ensure_image:omanrod/psp-palworld-server".to_string(),
                "create_and_start:alpha".to_string()
            ]
        );
    }

    #[tokio::test]
    async fn container_status_maps_missing_container_to_not_found_and_error_to_none() {
        let api = mock::MockDocker::default();
        assert_eq!(
            container_status(&api, "ghost").await,
            Some(ServerProcessStatus::not_found())
        );
        api.fail_inspect
            .lock()
            .unwrap()
            .insert("broken".to_string());
        assert_eq!(container_status(&api, "broken").await, None);
    }

    #[tokio::test]
    async fn container_stats_requires_running_container() {
        let api = mock::MockDocker::default();
        assert!(container_stats(&api, "ghost").await.is_none());
        // Exited container -> None even when stats exist
        api.statuses.lock().unwrap().insert(
            "sleepy".to_string(),
            serde_json::json!({"State": {"Status": "exited", "Running": false}}),
        );
        api.stats
            .lock()
            .unwrap()
            .insert("sleepy".to_string(), serde_json::json!({}));
        assert!(container_stats(&api, "sleepy").await.is_none());
    }

    #[tokio::test]
    async fn remove_server_container_optionally_removes_volume() {
        let api = mock::MockDocker::default();
        api.statuses.lock().unwrap().insert(
            "alpha".to_string(),
            serde_json::json!({"State": {"Status": "exited", "Running": false}}),
        );
        assert!(remove_server_container(&api, "alpha", true).await);
        let calls = api.calls.lock().unwrap().clone();
        assert!(calls.contains(&"remove_container:alpha".to_string()));
        assert!(calls.contains(&"remove_volume:psp-alpha-data".to_string()));
    }

    #[tokio::test]
    async fn remove_server_container_returns_false_when_volume_removal_fails() {
        let api = mock::MockDocker::default();
        api.statuses.lock().unwrap().insert(
            "alpha".to_string(),
            serde_json::json!({"State": {"Status": "exited", "Running": false}}),
        );
        api.fail_remove_volume
            .lock()
            .unwrap()
            .insert("psp-alpha-data".to_string());
        assert!(!remove_server_container(&api, "alpha", true).await);
        let calls = api.calls.lock().unwrap().clone();
        assert!(calls.contains(&"remove_container:alpha".to_string()));
        assert!(calls.contains(&"remove_volume:psp-alpha-data".to_string()));
    }

    #[tokio::test]
    async fn remove_server_container_ignores_volume_failure_when_not_requested() {
        let api = mock::MockDocker::default();
        api.fail_remove_volume
            .lock()
            .unwrap()
            .insert("psp-alpha-data".to_string());
        assert!(remove_server_container(&api, "alpha", false).await);
        let calls = api.calls.lock().unwrap().clone();
        assert!(!calls.iter().any(|call| call.starts_with("remove_volume")));
    }

    #[tokio::test]
    async fn start_and_stop_return_false_on_api_errors() {
        let api = mock::MockDocker::default();
        api.fail_start.lock().unwrap().insert("alpha".to_string());
        assert!(!start_server_container(&api, "alpha").await);
        api.fail_stop.lock().unwrap().insert("alpha".to_string());
        assert!(!stop_server_container(&api, "alpha").await);
    }

    #[tokio::test]
    async fn bollard_status_for_missing_container_when_docker_available() {
        if std::env::var("PSP_DOCKER_TESTS").as_deref() != Ok("1") {
            eprintln!("skipped: set PSP_DOCKER_TESTS=1 to run Docker integration tests");
            return;
        }
        let api = BollardDocker::connect().expect("docker daemon reachable");
        let status = container_status(&api, "psp-phase6-test-does-not-exist").await;
        assert_eq!(status, Some(ServerProcessStatus::not_found()));
    }
}
