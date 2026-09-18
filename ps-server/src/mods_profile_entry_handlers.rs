//! Wire handlers that take a mod out of profiles: one entry of one profile, or
//! every disabled entry the mod still has on any target. Neither applies; the
//! next plan or apply removes the files.
use serde_json::{json, Value};

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;
use crate::mods_handlers::{emit_refusal, holding_entries, target_or_refusal};
use crate::mods_profile_handlers::profile_or_refusal;

#[derive(Debug, serde::Deserialize)]
pub struct ProfileRemoveModData {
    pub target_id: String,
    pub mod_id: String,
    #[serde(default)]
    pub profile_id: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ModReleaseProfilesData {
    pub mod_id: String,
}

pub async fn handle_profile_remove_mod(
    data: ProfileRemoveModData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let message_type = MessageType::ProfileRemoveMod;
    let mut context = json!({
        "target_id": data.target_id,
        "profile_id": data.profile_id,
        "mod_id": data.mod_id,
    });
    let Some(target) =
        target_or_refusal(ctx, message_type, &data.target_id, context.clone()).await?
    else {
        return Ok(());
    };
    let Some(profile) = profile_or_refusal(
        ctx,
        message_type,
        context.clone(),
        &target.id,
        data.profile_id.as_deref(),
    )
    .await
    else {
        return Ok(());
    };
    context["profile_id"] = json!(profile.id);

    match ps_db::mod_profiles::unset_mod(&*ctx.app.driver, &profile.id, &data.mod_id).await {
        Ok(true) => ctx.emitter.emit(
            message_type,
            &json!({
                "target_id": target.id,
                "profile_id": profile.id,
                "mod_id": data.mod_id,
                "removed": true,
            }),
        ),
        Ok(false) => emit_refusal(
            ctx,
            message_type,
            context,
            "mod_not_in_profile",
            format!("{} is not in profile {}", data.mod_id, profile.id),
            json!({}),
        ),
        Err(error) => emit_refusal(
            ctx,
            message_type,
            context,
            "db",
            error.to_string(),
            json!({}),
        ),
    }
    Ok(())
}

/// Deletes each disabled entry conditionally, so an entry re-enabled between
/// `holding_entries`' read and this delete is reported under `enabled`
/// instead of being removed out from under the user who just turned it on.
async fn release_disabled(
    db: &dyn ps_db::DbDriver,
    mod_id: &str,
) -> Result<(Vec<Value>, Vec<Value>), ps_db::DbError> {
    let mut removed = Vec::new();
    let mut enabled = Vec::new();
    for held in holding_entries(db, mod_id).await? {
        let pair = json!({ "target_id": held.target.id, "profile_id": held.profile.id });
        if held.entry.enabled {
            enabled.push(pair);
        } else if ps_db::mod_profiles::unset_if_disabled(db, &held.profile.id, mod_id).await? {
            removed.push(pair);
        } else {
            enabled.push(pair);
        }
    }
    Ok((removed, enabled))
}

pub async fn handle_mod_release_profiles(
    data: ModReleaseProfilesData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let message_type = MessageType::ModReleaseProfiles;
    let context = json!({ "mod_id": data.mod_id });
    match release_disabled(&*ctx.app.driver, &data.mod_id).await {
        Ok((removed, enabled)) => ctx.emitter.emit(
            message_type,
            &json!({ "mod_id": data.mod_id, "removed": removed, "enabled": enabled }),
        ),
        Err(error) => emit_refusal(
            ctx,
            message_type,
            context,
            "db",
            error.to_string(),
            json!({}),
        ),
    }
    Ok(())
}
