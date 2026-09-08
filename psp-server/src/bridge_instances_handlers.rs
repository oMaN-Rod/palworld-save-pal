use serde_json::Value;

use psp_db::amity_instances::{self, NewAmityInstance};

use crate::bridge::client;
use crate::bridge::endpoint;
use crate::bridge::registry::{self, ACTIVE_INSTANCE_KEY};
use crate::bridge::service::{BridgeService, BridgeTarget};
use crate::dispatcher::HandlerCtx;
use crate::emitter::Emitter;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;
use crate::services::ServerServices;

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceIdData {
    pub id: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceFieldsData {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub token: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInstanceData {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub token: String,
}

fn refuse(emitter: &Emitter, request: MessageType, code: &str, message: &str) {
    emitter.emit(request, &serde_json::json!({ "code": code, "error": message }));
}

fn saved_row_id(id: &str) -> Option<i64> {
    id.strip_prefix("saved:").and_then(|rest| rest.parse().ok())
}

fn active_row_id(services: &ServerServices) -> Option<i64> {
    services
        .bridge
        .target()
        .and_then(|target| saved_row_id(&target.id))
}

fn discovered_now() -> Vec<endpoint::DiscoveredEndpoint> {
    endpoint::default_endpoint_dir()
        .map(|dir| endpoint::scan_endpoints(&dir, &endpoint::sysinfo_liveness))
        .unwrap_or_default()
}

struct Snapshot {
    discovered: Vec<endpoint::DiscoveredEndpoint>,
    saved: Vec<amity_instances::AmityInstance>,
}

/// A `list_instances` failure (a busy pool, a lock timeout) is not the same
/// fact as "there are no saved instances" -- collapsing the two would make a
/// transient DB hiccup look like every saved instance vanished. On failure
/// this logs and returns `None` so callers skip the tick instead of resolving
/// from an empty list they can't tell apart from a real one.
async fn read_snapshot(driver: &dyn psp_db::DbDriver) -> Option<Snapshot> {
    let discovered = discovered_now();
    match amity_instances::list_instances(driver).await {
        Ok(saved) => Some(Snapshot { discovered, saved }),
        Err(error) => {
            tracing::warn!(
                %error,
                "failed to list saved Amity instances; skipping Amity target resolution"
            );
            None
        }
    }
}

/// Same reasoning as [`read_snapshot`]: a failed read is not "nothing stored".
async fn read_stored_active_id(driver: &dyn psp_db::DbDriver) -> Option<String> {
    match psp_db::meta::get(driver, ACTIVE_INSTANCE_KEY).await {
        Ok(value) => Some(value.unwrap_or_default()),
        Err(error) => {
            tracing::warn!(
                %error,
                "failed to read the persisted active Amity instance; skipping resolution"
            );
            None
        }
    }
}

/// Resolves what the bridge target should be right now: the stored (last
/// explicitly selected) instance if it still exists, otherwise the first
/// discovered instance. Pure with respect to `BridgeService` -- callers decide
/// whether and when to act on the result. Returns `None` both when nothing
/// should be targeted and when a DB read failed; at startup (the only direct
/// caller) both cases mean the same thing -- start with no target, and let
/// the reconciler correct it once the read succeeds.
pub async fn resolve_active_target(driver: &dyn psp_db::DbDriver) -> Option<BridgeTarget> {
    let snapshot = read_snapshot(driver).await?;
    let stored = read_stored_active_id(driver).await?;

    registry::target_for(&stored, &snapshot.discovered, &snapshot.saved)
        .or_else(|| registry::default_target(&snapshot.discovered))
}

/// Leaves a still-resolvable target alone (a saved instance being unreachable
/// is not a reason to switch away from it) and otherwise re-resolves. Called
/// once at startup and repeatedly by a background reconciler so a game that
/// starts after PSP, or restarts under a new pid, gets adopted without
/// requiring a manual reselect.
///
/// A DB read failure at any point aborts the tick without touching the
/// target -- see [`read_snapshot`]. A target set by an explicit user
/// selection landing on the bridge while this tick was awaiting the DB is
/// re-checked for immediately before the final `set_target`, so it always
/// wins over a resolution computed before it happened.
pub async fn reconcile_active_target(driver: &dyn psp_db::DbDriver, bridge: &BridgeService) {
    let Some(snapshot) = read_snapshot(driver).await else {
        return;
    };

    let current = bridge.target();
    let still_valid = current.as_ref().is_some_and(|target| {
        registry::target_for(&target.id, &snapshot.discovered, &snapshot.saved).is_some()
    });
    if still_valid {
        return;
    }

    let Some(stored) = read_stored_active_id(driver).await else {
        return;
    };
    let resolved = registry::target_for(&stored, &snapshot.discovered, &snapshot.saved)
        .or_else(|| registry::default_target(&snapshot.discovered));

    if bridge.target() != current {
        return;
    }
    bridge.set_target(resolved);
}

pub async fn handle_game_instances(
    services: &ServerServices,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let discovered = discovered_now();
    let saved = match amity_instances::list_instances(&*ctx.app.driver).await {
        Ok(saved) => saved,
        Err(error) => {
            refuse(ctx.emitter, MessageType::GameInstances, "db_error", &error.to_string());
            return Ok(());
        }
    };
    let entries = registry::merge_instances(&discovered, &saved);
    let active = services.bridge.target().map(|target| target.id);

    ctx.emitter.emit(
        MessageType::GameInstances,
        &serde_json::json!({ "instances": entries, "activeId": active }),
    );
    Ok(())
}

pub async fn handle_game_add_instance(
    services: &ServerServices,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let payload: InstanceFieldsData = serde_json::from_value(data)?;
    let outcome = amity_instances::insert_instance(
        &*ctx.app.driver,
        &NewAmityInstance {
            name: payload.name,
            host: payload.host,
            port: i64::from(payload.port),
            token: payload.token,
        },
    )
    .await;
    if let Err(error) = outcome {
        refuse(ctx.emitter, MessageType::GameAddInstance, "db_error", &error.to_string());
        return Ok(());
    }
    handle_game_instances(services, ctx).await
}

pub async fn handle_game_update_instance(
    services: &ServerServices,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let payload: UpdateInstanceData = serde_json::from_value(data)?;
    let Some(row) = saved_row_id(&payload.id) else {
        refuse(
            ctx.emitter,
            MessageType::GameUpdateInstance,
            "validation_failed",
            "only saved instances can be edited",
        );
        return Ok(());
    };

    let outcome = amity_instances::update_instance(
        &*ctx.app.driver,
        row,
        &NewAmityInstance {
            name: payload.name,
            host: payload.host,
            port: i64::from(payload.port),
            token: payload.token,
        },
    )
    .await;
    if let Err(error) = outcome {
        refuse(ctx.emitter, MessageType::GameUpdateInstance, "db_error", &error.to_string());
        return Ok(());
    }

    // The row is now correct either way; retargeting a live connection onto
    // it is a best-effort follow-up, not something a hiccup here should turn
    // the (already-successful) edit into a reported failure for.
    if active_row_id(services) == Some(row) {
        let discovered = discovered_now();
        match amity_instances::list_instances(&*ctx.app.driver).await {
            Ok(saved) => {
                if let Some(target) = registry::target_for(&payload.id, &discovered, &saved) {
                    services.bridge.set_target(Some(target));
                }
            }
            Err(error) => {
                tracing::warn!(%error, "failed to re-list Amity instances while retargeting");
            }
        }
    }

    handle_game_instances(services, ctx).await
}

pub async fn handle_game_delete_instance(
    services: &ServerServices,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let payload: InstanceIdData = serde_json::from_value(data)?;
    let Some(row) = saved_row_id(&payload.id) else {
        refuse(
            ctx.emitter,
            MessageType::GameDeleteInstance,
            "validation_failed",
            "only saved instances can be deleted",
        );
        return Ok(());
    };

    if let Err(error) = amity_instances::delete_instance(&*ctx.app.driver, row).await {
        refuse(ctx.emitter, MessageType::GameDeleteInstance, "db_error", &error.to_string());
        return Ok(());
    }

    // The row is gone either way; failing to persist that the active
    // selection was cleared must not report the deletion itself as an error.
    if active_row_id(services) == Some(row) {
        services.bridge.set_target(None);
        if let Err(error) = psp_db::meta::set(&*ctx.app.driver, ACTIVE_INSTANCE_KEY, "").await {
            tracing::warn!(
                %error,
                "failed to clear the persisted active Amity instance after delete"
            );
        }
    }

    handle_game_instances(services, ctx).await
}

pub async fn handle_game_select_instance(
    services: &ServerServices,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let payload: InstanceIdData = serde_json::from_value(data)?;
    let discovered = discovered_now();
    let saved = match amity_instances::list_instances(&*ctx.app.driver).await {
        Ok(saved) => saved,
        Err(error) => {
            refuse(ctx.emitter, MessageType::GameSelectInstance, "db_error", &error.to_string());
            return Ok(());
        }
    };

    let Some(target) = registry::target_for(&payload.id, &discovered, &saved) else {
        refuse(
            ctx.emitter,
            MessageType::GameSelectInstance,
            "validation_failed",
            "no such instance",
        );
        return Ok(());
    };

    services.bridge.set_target(Some(target));
    // The selection already took effect above; a failure to persist it only
    // risks losing the choice across a restart, not the current session.
    let persisted =
        psp_db::meta::set(&*ctx.app.driver, ACTIVE_INSTANCE_KEY, &payload.id).await;
    if let Err(error) = persisted {
        tracing::warn!(%error, "failed to persist the selected Amity instance");
    }
    handle_game_instances(services, ctx).await
}

pub async fn handle_game_test_instance(
    _services: &ServerServices,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let payload: InstanceFieldsData = serde_json::from_value(data)?;
    let target = BridgeTarget {
        id: "test".to_string(),
        name: payload.name,
        host: payload.host,
        port: payload.port,
        token: payload.token,
    };

    let cancel = tokio_util::sync::CancellationToken::new();
    let outcome = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client::connect_and_handshake(&target, &cancel),
    )
    .await;

    let reply = match outcome {
        Ok(Ok(connected)) => {
            serde_json::json!({ "ok": true, "modVersion": connected.mod_version })
        }
        Ok(Err(client::ConnectError::Auth { code })) => {
            serde_json::json!({ "ok": false, "error": code })
        }
        Ok(Err(_)) => serde_json::json!({ "ok": false, "error": "transport" }),
        Err(_) => serde_json::json!({ "ok": false, "error": "timeout" }),
    };

    ctx.emitter.emit(MessageType::GameTestInstance, &reply);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A driver whose every query/execute fails, standing in for a transient
    /// DB error (a busy pool, a lock timeout) rather than a real empty table.
    struct FailingDriver;

    #[async_trait::async_trait]
    impl psp_db::DbDriver for FailingDriver {
        async fn execute(
            &self,
            _sql: &str,
            _params: &[psp_db::DbValue],
        ) -> Result<u64, psp_db::DbError> {
            Err(psp_db::DbError::Other("simulated failure".to_string()))
        }

        async fn query(
            &self,
            _sql: &str,
            _params: &[psp_db::DbValue],
        ) -> Result<Vec<psp_db::DbRow>, psp_db::DbError> {
            Err(psp_db::DbError::Other("simulated failure".to_string()))
        }
    }

    #[tokio::test]
    async fn a_transient_db_failure_does_not_change_an_explicitly_selected_target() {
        let bridge = BridgeService::new();
        let selected = BridgeTarget {
            id: "saved:3".to_string(),
            name: "Remote".to_string(),
            host: "10.0.0.14".to_string(),
            port: 8788,
            token: "s3cr3t".to_string(),
        };
        bridge.set_target(Some(selected.clone()));

        reconcile_active_target(&FailingDriver, &bridge).await;

        assert_eq!(
            bridge.target(),
            Some(selected),
            "a DB read failure must leave an explicit selection untouched, not fall through to \
             a default"
        );
    }

    /// Serializes tests in this module that mutate `PSP_BRIDGE_ENDPOINT_DIR`
    /// (only this file's tests touch it, but the lock costs nothing and
    /// matches the `SignalEnvGuard` convention used elsewhere for the same
    /// reason: this is process-global state).
    static BRIDGE_ENDPOINT_ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    struct BridgeEndpointEnvGuard {
        _lock: tokio::sync::MutexGuard<'static, ()>,
        previous: Option<std::ffi::OsString>,
    }

    impl BridgeEndpointEnvGuard {
        async fn acquire(dir: &std::path::Path) -> Self {
            let lock = BRIDGE_ENDPOINT_ENV_LOCK.lock().await;
            let previous = std::env::var_os("PSP_BRIDGE_ENDPOINT_DIR");
            std::env::set_var("PSP_BRIDGE_ENDPOINT_DIR", dir);
            Self {
                _lock: lock,
                previous,
            }
        }
    }

    impl Drop for BridgeEndpointEnvGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => std::env::set_var("PSP_BRIDGE_ENDPOINT_DIR", value),
                None => std::env::remove_var("PSP_BRIDGE_ENDPOINT_DIR"),
            }
        }
    }

    /// With nothing discovered, `resolve_active_target` returning `None` on a
    /// DB failure is indistinguishable from it returning `None` because there
    /// was genuinely nothing to pick -- `target_for("", [], [])` and
    /// `default_target([])` are both `None` regardless of the DB. To actually
    /// exercise the abort-on-error path, a live instance must be discoverable
    /// so that the *wrong* (pre-fix) behavior has something to wrongly land
    /// on: falling through a swallowed error to `default_target(discovered)`
    /// would auto-select it. The fix aborts before ever calling
    /// `default_target` and must return `None` here instead.
    #[tokio::test]
    async fn a_transient_db_failure_resolves_to_no_target_even_with_a_live_discovered_instance() {
        let dir = tempfile::tempdir().unwrap();
        let pid = std::process::id();
        std::fs::write(
            dir.path().join(format!("{pid}.json")),
            serde_json::json!({
                "protocolVersion": 2,
                "port": 8788,
                "token": "t",
                "name": "Solo",
                "bind": "127.0.0.1",
                "pid": pid,
                "startedAt": "now",
            })
            .to_string(),
        )
        .unwrap();
        let _env = BridgeEndpointEnvGuard::acquire(dir.path()).await;

        assert!(
            resolve_active_target(&FailingDriver).await.is_none(),
            "a DB failure must abort resolution even though a live instance was discovered on \
             disk -- landing on it anyway would mean the error was silently treated as \
             \"nothing saved\" instead of aborting"
        );
    }
}
