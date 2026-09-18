//! Wire handler for reporting what is wrong with a target profile's mods on
//! demand: missing frameworks or Workshop dependencies, overlapping paks,
//! shared PalSchema rows, and Game Pass-incompatible legacy paks.
use serde_json::json;

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;
use crate::mods_handlers::{emit_refusal, target_or_refusal};
use crate::mods_profile_handlers::profile_or_refusal;
use crate::services::mods::conflicts;

#[derive(Debug, serde::Deserialize)]
pub struct ModConflictsData {
    pub target_id: String,
    #[serde(default)]
    pub profile_id: Option<String>,
}

pub async fn handle_mod_conflicts(
    data: ModConflictsData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let message_type = MessageType::ModConflicts;
    let context = json!({ "target_id": data.target_id, "profile_id": data.profile_id });
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

    let db = &*ctx.app.driver;
    match conflicts::report(db, &target, &profile.id).await {
        Ok(report) => ctx.emitter.emit(message_type, &report),
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
