//! Server-management services (Phase 6). Mirrors palworld_save_pal/services/*.
pub mod docker;
pub mod palworld_api;
// pub mod docker_mods;     // Task 6
// pub mod native_config;   // Task 7
// pub mod native_mods;     // Task 8
// pub mod native_process;  // Task 9

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

/// Matches the Python status dict shape from DockerService.get_container_status /
/// NativeServerService.get_process_status.
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

/// Mirrors Python's builtin `round(value, decimals)`, which rounds halves to
/// even (banker's rounding) — not away from zero. Stats values are fed straight
/// to `round(...)` in docker_service.py / native_server_service.py, so the tie
/// rule is wire-visible; `f64::round()` (half-away) would diverge on exact ties
/// like `round(0.125, 2)` (Python → 0.12, half-away → 0.13).
pub fn round_to(value: f64, decimals: u32) -> f64 {
    let factor = 10f64.powi(decimals as i32);
    (value * factor).round_ties_even() / factor
}

/// Python datetime.isoformat(): "T" separator, microseconds omitted when zero.
pub fn python_isoformat(timestamp: chrono::NaiveDateTime) -> String {
    if timestamp.and_utc().timestamp_subsec_micros() == 0 {
        timestamp.format("%Y-%m-%dT%H:%M:%S").to_string()
    } else {
        timestamp.format("%Y-%m-%dT%H:%M:%S%.6f").to_string()
    }
}

/// Python str() semantics for env-var values (build_environment uses str(v)).
pub fn python_str(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Bool(true) => "True".to_string(),
        serde_json::Value::Bool(false) => "False".to_string(),
        serde_json::Value::Null => "None".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn round_to_matches_python_round_for_stats_values() {
        assert_eq!(round_to(80.004, 2), 80.0);
        assert_eq!(round_to(1024.04, 1), 1024.0);
        // Exactly-representable halves (eighths) round to even, matching
        // Python's builtin round(): round(0.125, 2) == 0.12 (2 is even),
        // round(0.375, 2) == 0.38 (8 is even). `f64::round()` (half-away)
        // would give 0.13 / 0.38 and diverge on the first.
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
    fn python_str_matches_python_str_builtin() {
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
}
