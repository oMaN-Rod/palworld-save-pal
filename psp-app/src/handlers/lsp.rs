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
pub struct OpenLspSessionData {
    pub plugin_id: String,
}

pub async fn handle_open_lsp_session(
    data: OpenLspSessionData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let row = match psp_db::plugins::get(&*ctx.app.driver, &data.plugin_id).await {
        Ok(Some(row)) => row,
        Ok(None) => return open_session_error(ctx, format!("plugin {} not found", data.plugin_id)),
        Err(error) => return open_session_error(ctx, error.to_string()),
    };
    let sources: std::collections::BTreeMap<String, String> =
        serde_json::from_str(&row.sources).unwrap_or_default();

    // Before the session opens, not after: a language server publishes its
    // first diagnostics as soon as it has indexed the workspace, and the
    // frames it sends before a client is attached are gone for good.
    ctx.app.lsp.attach_client(ctx.emitter.clone());

    match ctx.app.lsp.open_session(&data.plugin_id, &sources).await {
        Ok(root_uri) => {
            ctx.emitter.emit(
                MessageType::OpenLspSession,
                &serde_json::json!({ "root_uri": root_uri }),
            );
            Ok(())
        }
        Err(error) => open_session_error(ctx, error),
    }
}

fn open_session_error(
    ctx: &mut HandlerCtx<'_>,
    error: impl Into<String>,
) -> Result<(), HandlerError> {
    ctx.emitter.emit(
        MessageType::OpenLspSession,
        &serde_json::json!({ "root_uri": serde_json::Value::Null, "error": error.into() }),
    );
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct LspRequestData {
    pub plugin_id: String,
    /// Client-generated and echoed back untouched. Two LSP requests are in
    /// flight at once as a matter of course — Monaco asks for a hover while a
    /// references search is still running — and nothing else in the answer
    /// tells the client which one it is looking at.
    pub request_id: String,
    pub frame: serde_json::Value,
}

pub async fn handle_lsp_request(
    data: LspRequestData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    match ctx.app.lsp.request(&data.plugin_id, data.frame).await {
        Ok(frame) => ctx.emitter.emit(
            MessageType::LspRequest,
            &serde_json::json!({ "request_id": data.request_id, "frame": frame }),
        ),
        Err(error) => ctx.emitter.emit(
            MessageType::LspRequest,
            &serde_json::json!({ "request_id": data.request_id, "error": error }),
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
