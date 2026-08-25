use std::collections::BTreeMap;

pub enum TierStatus {
    Available,
    Starting,
    Unavailable { reason: String },
}

#[async_trait::async_trait]
pub trait LspService: Send + Sync {
    fn status(&self) -> TierStatus;
    async fn ensure_ready(
        &self,
        plugin_id: &str,
        sources: &BTreeMap<String, String>,
    ) -> Result<(), String>;
    async fn request(
        &self,
        plugin_id: &str,
        frame: serde_json::Value,
    ) -> Result<serde_json::Value, String>;
    async fn notify(&self, plugin_id: &str, frame: serde_json::Value) -> Result<(), String>;
    async fn shutdown(&self, plugin_id: &str);
}

pub struct NullLspService;

const NOT_AVAILABLE_REASON: &str = "the language server does not run on this deployment";

#[async_trait::async_trait]
impl LspService for NullLspService {
    fn status(&self) -> TierStatus {
        TierStatus::Unavailable {
            reason: NOT_AVAILABLE_REASON.to_string(),
        }
    }

    async fn ensure_ready(
        &self,
        _plugin_id: &str,
        _sources: &BTreeMap<String, String>,
    ) -> Result<(), String> {
        Err(NOT_AVAILABLE_REASON.to_string())
    }

    async fn request(
        &self,
        _plugin_id: &str,
        _frame: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        Err(NOT_AVAILABLE_REASON.to_string())
    }

    async fn notify(&self, _plugin_id: &str, _frame: serde_json::Value) -> Result<(), String> {
        Ok(())
    }

    async fn shutdown(&self, _plugin_id: &str) {}
}
