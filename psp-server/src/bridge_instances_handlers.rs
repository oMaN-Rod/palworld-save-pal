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

/// Resolves what the bridge target should be right now: the stored (last
/// explicitly selected) instance if it still exists, otherwise the first
/// discovered instance. Pure with respect to `BridgeService` -- callers decide
/// whether and when to act on the result.
pub async fn resolve_active_target(driver: &dyn psp_db::DbDriver) -> Option<BridgeTarget> {
    let discovered = discovered_now();
    let saved = amity_instances::list_instances(driver).await.unwrap_or_default();
    let stored = psp_db::meta::get(driver, ACTIVE_INSTANCE_KEY)
        .await
        .ok()
        .flatten()
        .unwrap_or_default();

    registry::target_for(&stored, &discovered, &saved)
        .or_else(|| registry::default_target(&discovered))
}

/// Leaves a still-resolvable target alone (a saved instance being unreachable
/// is not a reason to switch away from it) and otherwise re-resolves via
/// [`resolve_active_target`]. Called once at startup and repeatedly by a
/// background reconciler so a game that starts after PSP, or restarts under a
/// new pid, gets adopted without requiring a manual reselect.
pub async fn reconcile_active_target(driver: &dyn psp_db::DbDriver, bridge: &BridgeService) {
    let discovered = discovered_now();
    let saved = amity_instances::list_instances(driver).await.unwrap_or_default();

    let current = bridge.target();
    let still_valid = current
        .as_ref()
        .is_some_and(|target| registry::target_for(&target.id, &discovered, &saved).is_some());
    if still_valid {
        return;
    }

    bridge.set_target(resolve_active_target(driver).await);
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
        if let Ok(saved) = amity_instances::list_instances(&*ctx.app.driver).await {
            if let Some(target) = registry::target_for(&payload.id, &discovered, &saved) {
                services.bridge.set_target(Some(target));
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
