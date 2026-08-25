use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::lsp::TierStatus;
use crate::messages::MessageType;

pub async fn handle_get_editor_tier(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let (tier, reason) = match ctx.app.lsp.status() {
        TierStatus::Available => ("full", None),
        TierStatus::Starting => ("starting", None),
        TierStatus::Unavailable { reason } => ("baseline", Some(reason)),
    };
    ctx.emitter.emit(
        MessageType::GetEditorTier,
        &serde_json::json!({ "tier": tier, "reason": reason }),
    );
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct LspRequestData {
    pub plugin_id: String,
    pub frame: serde_json::Value,
}

pub async fn handle_lsp_request(
    data: LspRequestData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    match ctx.app.lsp.request(&data.plugin_id, data.frame).await {
        Ok(frame) => ctx.emitter.emit(
            MessageType::LspRequest,
            &serde_json::json!({ "frame": frame }),
        ),
        Err(error) => ctx.emitter.emit(
            MessageType::LspRequest,
            &serde_json::json!({ "error": error }),
        ),
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct LspNotificationData {
    pub plugin_id: String,
    pub frame: serde_json::Value,
}

pub async fn handle_lsp_notification(
    data: LspNotificationData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if let Err(error) = ctx.app.lsp.notify(&data.plugin_id, data.frame).await {
        tracing::warn!(%error, plugin_id = %data.plugin_id, "lsp notification failed");
        ctx.emitter.emit(
            MessageType::LspNotification,
            &serde_json::json!({ "error": error }),
        );
    }
    Ok(())
}
