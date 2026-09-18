//! Wire handlers that read a target's latest verification result and push
//! later results to the subscribing connection.
use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;
use crate::mods_handlers::target_or_refusal;
use crate::services::mods::verify::TargetVerification;
use crate::services::ServerServices;

#[derive(Debug, serde::Deserialize)]
pub struct VerificationGetData {
    pub target_id: String,
}

pub async fn handle_mod_verification_get(
    services: &ServerServices,
    data: VerificationGetData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let message_type = MessageType::ModVerificationGet;
    let context = json!({ "target_id": data.target_id });
    let Some(target) = target_or_refusal(ctx, message_type, &data.target_id, context).await? else {
        return Ok(());
    };
    ctx.emitter.emit(
        message_type,
        &json!({ "target_id": target.id, "verification": services.verification.get(&target.id) }),
    );
    Ok(())
}

pub async fn handle_mod_verification_subscribe(
    services: &ServerServices,
    _data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    ctx.emitter.emit(
        MessageType::ModVerificationSubscribe,
        &json!({ "active": true }),
    );
    let Some(subscribed) = &mut ctx.mod_verification_subscribed else {
        return Ok(());
    };
    if **subscribed {
        return Ok(());
    }
    **subscribed = true;

    let mut rx = services.verification.subscribe();
    let forward = ctx.emitter.clone();
    tokio::spawn(async move {
        let mut sent: BTreeMap<String, TargetVerification> = BTreeMap::new();
        loop {
            let current = rx.borrow_and_update().clone();
            for (target_id, verification) in &current {
                if sent.get(target_id) != Some(verification) {
                    forward.emit(MessageType::ModVerification, verification);
                    sent.insert(target_id.clone(), verification.clone());
                }
            }
            tokio::select! {
                _ = forward.closed() => break,
                changed = rx.changed() => {
                    if changed.is_err() {
                        break;
                    }
                }
            }
        }
    });
    Ok(())
}
