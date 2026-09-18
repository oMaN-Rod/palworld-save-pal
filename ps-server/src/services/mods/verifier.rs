//! The background task that keeps `VerificationStore` in sync with whatever
//! Amity instance the bridge is currently connected to.
use std::sync::Arc;

use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::bridge::binding;
use crate::bridge::service::BridgeService;

use super::verify::{
    classify, expected_for_target, parse_loaded, parse_resolution, TargetVerification,
    VerificationStore,
};

const VERIFY_INTERVAL_ENV: &str = "PS_VERIFY_INTERVAL_SECS";
const VERIFY_INTERVAL_DEFAULT_SECS: u64 = 30;
const AMITY_DETECTED_KEYS: &[&str] = &[
    "engineVersion",
    "ue4ssVersion",
    "amityVersion",
    "platform",
    "ue4ssMode",
];

pub fn verify_interval() -> std::time::Duration {
    std::env::var(VERIFY_INTERVAL_ENV)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .map(std::time::Duration::from_secs)
        .unwrap_or(std::time::Duration::from_secs(VERIFY_INTERVAL_DEFAULT_SECS))
}

/// Merges Amity's build info into a target's `detected` JSON under the
/// `amity` key, keeping every other key untouched. `detected` that does not
/// parse as a JSON object (including an empty string) starts from `{}`.
pub fn merge_amity_detected(detected: &str, build_info: &Value, checked_at: &str) -> String {
    let mut object = serde_json::from_str::<Value>(detected)
        .ok()
        .and_then(|value| match value {
            Value::Object(map) => Some(map),
            _ => None,
        })
        .unwrap_or_default();

    let mut amity = serde_json::Map::new();
    for key in AMITY_DETECTED_KEYS {
        if let Some(value) = build_info.get(*key) {
            amity.insert((*key).to_string(), value.clone());
        }
    }
    amity.insert(
        "checked_at".to_string(),
        Value::String(checked_at.to_string()),
    );

    object.insert("amity".to_string(), Value::Object(amity));
    Value::Object(object).to_string()
}

fn without_amity_checked_at(detected: &str) -> Value {
    let mut value = serde_json::from_str::<Value>(detected).unwrap_or(Value::Null);
    if let Some(amity) = value.get_mut("amity").and_then(Value::as_object_mut) {
        amity.remove("checked_at");
    }
    value
}

/// Whether `merged` (the result of `merge_amity_detected` against `current`)
/// carries anything beyond a bumped `amity.checked_at`, so a pass that learns
/// nothing new does not write to the database every tick.
fn detected_needs_update(current: &str, merged: &str) -> bool {
    without_amity_checked_at(current) != without_amity_checked_at(merged)
}

async fn run_pass(
    driver: &dyn ps_db::DbDriver,
    bridge: &BridgeService,
    store: &VerificationStore,
    instance_id: &str,
    last_bound: &mut Option<String>,
) {
    let bound = match binding::bound_target_id(driver, instance_id, binding::process_exe).await {
        Ok(Some(id)) => id,
        _ => {
            if let Some(old) = last_bound.take() {
                store.mark_offline(&old);
            }
            return;
        }
    };

    if last_bound.as_deref() != Some(bound.as_str()) {
        if let Some(old) = last_bound.as_ref() {
            store.mark_offline(old);
        }
    }

    let Ok(Some(target)) = ps_db::mod_targets::get(driver, &bound).await else {
        return;
    };

    let Ok(loaded_value) = bridge
        .request("get_loaded_mods", serde_json::json!({}))
        .await
    else {
        return;
    };

    let Ok(resolution_value) = bridge
        .request("get_resolution_report", serde_json::json!({}))
        .await
    else {
        return;
    };
    let resolution = parse_resolution(&resolution_value);

    let Ok((expected, builtins)) = expected_for_target(driver, &target).await else {
        return;
    };

    let loaded = parse_loaded(&loaded_value);
    let mod_status = classify(&expected, &loaded, &builtins, true);
    let checked_at = chrono::Utc::now().to_rfc3339();

    // Re-checked after the round trips above: a slow reply must not publish a
    // stale "live" result for an instance that has since disconnected or been
    // replaced.
    let current_status = bridge.status_rx().borrow().clone();
    if !current_status.connected || current_status.instance_id.as_deref() != Some(instance_id) {
        return;
    }

    store.publish(TargetVerification {
        target_id: bound.clone(),
        instance_id: instance_id.to_string(),
        live: true,
        checked_at: checked_at.clone(),
        build_info: current_status.build_info.clone(),
        resolution,
        status: mod_status,
    });

    if let Some(build_info) = &current_status.build_info {
        if let Ok(Some(fresh)) = ps_db::mod_targets::get(driver, &bound).await {
            let merged = merge_amity_detected(&fresh.detected, build_info, &checked_at);
            if detected_needs_update(&fresh.detected, &merged) {
                if let Err(error) = ps_db::mod_targets::set_detected(driver, &bound, &merged).await
                {
                    tracing::warn!(%error, target_id = %bound, "failed to persist detected amity build info");
                }
            }
        }
    }

    *last_bound = Some(bound);
}

pub async fn run_verifier(
    driver: Arc<dyn ps_db::DbDriver>,
    bridge: Arc<BridgeService>,
    store: Arc<VerificationStore>,
    cancel: CancellationToken,
) {
    let mut status_rx = bridge.status_rx();
    let mut last_bound: Option<String> = None;

    loop {
        let status = status_rx.borrow().clone();
        match (status.connected, status.instance_id.clone()) {
            (true, Some(instance_id)) => {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = run_pass(&*driver, &bridge, &store, &instance_id, &mut last_bound) => {}
                }
            }
            _ => {
                if let Some(id) = last_bound.take() {
                    store.mark_offline(&id);
                }
            }
        }

        tokio::select! {
            _ = cancel.cancelled() => break,
            changed = status_rx.changed() => {
                if changed.is_err() {
                    break;
                }
            }
            _ = tokio::time::sleep(verify_interval()) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static VERIFY_INTERVAL_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct EnvGuard {
        _lock: std::sync::MutexGuard<'static, ()>,
        previous: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn set(value: &str) -> Self {
            let lock = VERIFY_INTERVAL_ENV_LOCK
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let previous = std::env::var_os(VERIFY_INTERVAL_ENV);
            std::env::set_var(VERIFY_INTERVAL_ENV, value);
            Self {
                _lock: lock,
                previous,
            }
        }

        fn unset() -> Self {
            let lock = VERIFY_INTERVAL_ENV_LOCK
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let previous = std::env::var_os(VERIFY_INTERVAL_ENV);
            std::env::remove_var(VERIFY_INTERVAL_ENV);
            Self {
                _lock: lock,
                previous,
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => std::env::set_var(VERIFY_INTERVAL_ENV, value),
                None => std::env::remove_var(VERIFY_INTERVAL_ENV),
            }
        }
    }

    #[test]
    fn defaults_to_thirty_seconds() {
        let _env = EnvGuard::unset();
        assert_eq!(verify_interval(), std::time::Duration::from_secs(30));
    }

    #[test]
    fn an_env_override_is_parsed() {
        let _env = EnvGuard::set("5");
        assert_eq!(verify_interval(), std::time::Duration::from_secs(5));
    }

    #[test]
    fn zero_is_ignored_in_favor_of_the_default() {
        let _env = EnvGuard::set("0");
        assert_eq!(verify_interval(), std::time::Duration::from_secs(30));
    }

    #[test]
    fn merge_keeps_other_keys_and_replaces_an_older_amity_block() {
        let detected = r#"{"hazards":["x"],"amity":{"amityVersion":"0.1.0"}}"#;
        let build_info = serde_json::json!({
            "engineVersion": "5.1",
            "ue4ssVersion": "3.0.1",
            "amityVersion": "0.3.0",
            "platform": "win64",
            "ue4ssMode": "standard",
            "gameVersion": null,
        });
        let merged = merge_amity_detected(detected, &build_info, "2026-09-15T00:00:00Z");
        let value: Value = serde_json::from_str(&merged).unwrap();
        assert_eq!(value["hazards"], serde_json::json!(["x"]));
        assert_eq!(value["amity"]["amityVersion"], "0.3.0");
        assert_eq!(value["amity"]["engineVersion"], "5.1");
        assert_eq!(value["amity"]["checked_at"], "2026-09-15T00:00:00Z");
        assert!(value["amity"].get("gameVersion").is_none());
    }

    #[test]
    fn invalid_json_is_treated_as_an_empty_object() {
        let build_info = serde_json::json!({ "amityVersion": "0.3.0" });
        let merged = merge_amity_detected("not json", &build_info, "2026-09-15T00:00:00Z");
        let value: Value = serde_json::from_str(&merged).unwrap();
        assert_eq!(value.as_object().unwrap().len(), 1);
        assert_eq!(value["amity"]["amityVersion"], "0.3.0");
    }

    #[test]
    fn only_checked_at_changing_needs_no_update() {
        let current = r#"{"hazards":["x"],"amity":{"amityVersion":"0.3.0","checked_at":"t0"}}"#;
        let build_info = serde_json::json!({ "amityVersion": "0.3.0" });
        let merged = merge_amity_detected(current, &build_info, "t1");
        assert!(!detected_needs_update(current, &merged));
    }

    #[test]
    fn a_changed_amity_value_needs_an_update() {
        let current = r#"{"amity":{"amityVersion":"0.2.0","checked_at":"t0"}}"#;
        let build_info = serde_json::json!({ "amityVersion": "0.3.0" });
        let merged = merge_amity_detected(current, &build_info, "t1");
        assert!(detected_needs_update(current, &merged));
    }

    #[test]
    fn adding_amity_for_the_first_time_needs_an_update() {
        let current = r#"{"hazards":[]}"#;
        let build_info = serde_json::json!({ "amityVersion": "0.3.0" });
        let merged = merge_amity_detected(current, &build_info, "t1");
        assert!(detected_needs_update(current, &merged));
    }
}
