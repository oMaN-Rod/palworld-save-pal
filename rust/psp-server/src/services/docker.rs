//! Docker server lifecycle. Mirrors DockerService (docker_service.py).
//! Pure builders + JSON transforms here; the DockerApi trait and bollard
//! implementation live in this file too (Task 5).
use psp_db::servers::ServerRecord;
use serde_json::Value;

use super::{python_str, round_to, ServerProcessStatus};

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

/// build_environment (docker_service.py:383-403): user env_vars first (Python str()
/// semantics), explicit server fields override, SERVER_PASSWORD only when non-empty.
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

/// get_container_status success path (docker_service.py:116-128): container.status is
/// State.Status; health only when a Health block exists.
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

/// get_container_stats math (docker_service.py:148-187). Required fields missing
/// (Python KeyError) → None; .get() fields default like Python.
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
        // First stats sample after start lacks precpu system_cpu_usage — Python's
        // KeyError path returns None.
        let raw = serde_json::json!({
            "cpu_stats": {"cpu_usage": {"total_usage": 1u64}},
            "precpu_stats": {"cpu_usage": {"total_usage": 0u64}}
        });
        assert!(stats_from_raw(&raw).is_none());
    }
}
