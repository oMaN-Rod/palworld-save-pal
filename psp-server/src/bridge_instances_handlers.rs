use serde_json::Value;

use psp_db::amity_instances::{self, NewAmityInstance};

use crate::bridge::client;
use crate::bridge::endpoint;
use crate::bridge::registry::{self, ACTIVE_INSTANCE_KEY};
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

async fn discovered_now() -> Vec<endpoint::DiscoveredEndpoint> {
    endpoint::default_endpoint_dir()
        .map(|dir| endpoint::scan_endpoints(&dir, &endpoint::sysinfo_liveness))
        .unwrap_or_default()
}

pub async fn handle_game_instances(
    services: &ServerServices,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let discovered = discovered_now().await;
    let saved = amity_instances::list_instances(&*ctx.app.driver).await?;
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
    amity_instances::insert_instance(
        &*ctx.app.driver,
        &NewAmityInstance {
            name: payload.name,
            host: payload.host,
            port: i64::from(payload.port),
            token: payload.token,
        },
    )
    .await?;
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
    amity_instances::update_instance(
        &*ctx.app.driver,
        row,
        &NewAmityInstance {
            name: payload.name,
            host: payload.host,
            port: i64::from(payload.port),
            token: payload.token,
        },
    )
    .await?;
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

    amity_instances::delete_instance(&*ctx.app.driver, row).await?;

    if services.bridge.target().map(|target| target.id).as_deref() == Some(payload.id.as_str()) {
        services.bridge.set_target(None);
        psp_db::meta::set(&*ctx.app.driver, ACTIVE_INSTANCE_KEY, "").await?;
    }

    handle_game_instances(services, ctx).await
}

pub async fn handle_game_select_instance(
    services: &ServerServices,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let payload: InstanceIdData = serde_json::from_value(data)?;
    let discovered = discovered_now().await;
    let saved = amity_instances::list_instances(&*ctx.app.driver).await?;

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
    psp_db::meta::set(&*ctx.app.driver, ACTIVE_INSTANCE_KEY, &payload.id).await?;
    handle_game_instances(services, ctx).await
}

pub async fn handle_game_test_instance(
    _services: &ServerServices,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let payload: InstanceFieldsData = serde_json::from_value(data)?;
    let target = crate::bridge::service::BridgeTarget {
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
