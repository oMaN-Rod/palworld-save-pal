use std::collections::BTreeMap;

use crate::emitter::Emitter;

pub enum TierStatus {
    Available,
    Starting,
    Unavailable { reason: String },
}

#[async_trait::async_trait]
pub trait LspService: Send + Sync {
    fn status(&self) -> TierStatus;
    /// Directs the frames a language server sends unprompted — diagnostics
    /// above all — at the connection that asked for the session. Without it
    /// they are emitted into a channel nobody reads.
    fn attach_client(&self, emitter: Emitter);
    /// Starts a language server for `plugin_id` if one is not already up, and
    /// returns the `rootUri` of the workspace it indexed. Every document URI
    /// the client sends afterwards has to sit under that root, or the server
    /// answers about files it has never seen.
    async fn open_session(
        &self,
        plugin_id: &str,
        sources: &BTreeMap<String, String>,
    ) -> Result<String, String>;
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

    fn attach_client(&self, _emitter: Emitter) {}

    async fn open_session(
        &self,
        _plugin_id: &str,
        _sources: &BTreeMap<String, String>,
    ) -> Result<String, String> {
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
