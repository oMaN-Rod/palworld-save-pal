//! Native-only message routing owned by the transport: server management and
//! shell-open. Kept out of the dispatcher so the message layer stays free of
//! Docker, process, and OS-shell dependencies.
use std::sync::Arc;

use serde_json::Value;

use crate::dispatcher::{ExtRouter, HandlerCtx};
use crate::handler_error::HandlerError;
use crate::handlers;
use crate::messages::MessageType;
use crate::services::ServerServices;

pub struct ServerExtRouter {
    pub services: Arc<ServerServices>,
}

#[async_trait::async_trait]
impl ExtRouter for ServerExtRouter {
    async fn route(
        &self,
        message_type: MessageType,
        data: Value,
        ctx: &mut HandlerCtx<'_>,
    ) -> Option<Result<(), HandlerError>> {
        let services = &self.services;
        // `?` is unavailable here (this returns Option<Result<..>>), so each
        // payload parse spells out its own error conversion.
        Some(match message_type {
            MessageType::OpenFolder => match serde_json::from_value(data) {
                Ok(payload) => handlers::system::handle_open_folder(payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::OpenInBrowser => match serde_json::from_value(data) {
                Ok(payload) => handlers::system::handle_open_in_browser(payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::OpenUrl => match serde_json::from_value(data) {
                Ok(payload) => handlers::system::handle_open_url(payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::ListServers => {
                handlers::servers::handle_list_servers(services, data, ctx).await
            }
            MessageType::GetServer => match serde_json::from_value(data) {
                Ok(payload) => handlers::servers::handle_get_server(services, payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::DetectWorkshopDir => {
                handlers::servers::handle_detect_workshop_dir(data, ctx).await
            }
            MessageType::GetServerStats => match serde_json::from_value(data) {
                Ok(payload) => {
                    handlers::servers::handle_get_server_stats(services, payload, ctx).await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::CreateServer => match serde_json::from_value(data) {
                Ok(payload) => {
                    handlers::servers::handle_create_server(services, payload, ctx).await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::ImportServer => match serde_json::from_value(data) {
                Ok(payload) => handlers::servers::handle_import_server(payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::UpdateServer => match serde_json::from_value(data) {
                Ok(payload) => {
                    handlers::servers::handle_update_server(services, payload, ctx).await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::DeleteServer => match serde_json::from_value(data) {
                Ok(payload) => {
                    handlers::servers::handle_delete_server(services, payload, ctx).await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::StartServer => match serde_json::from_value(data) {
                Ok(payload) => handlers::servers::handle_start_server(services, payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::StopServer => match serde_json::from_value(data) {
                Ok(payload) => handlers::servers::handle_stop_server(services, payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::ServerApiCall => match serde_json::from_value(data) {
                Ok(payload) => {
                    handlers::servers::handle_server_api_call(services, payload, ctx).await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::ListServerMods => match serde_json::from_value(data) {
                Ok(payload) => handlers::servers::handle_list_server_mods(payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::ToggleServerMod => match serde_json::from_value(data) {
                Ok(payload) => handlers::servers::handle_toggle_server_mod(payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::InstallServerMod => match serde_json::from_value(data) {
                Ok(payload) => handlers::servers::handle_install_server_mod(payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::LoadServerSave => match serde_json::from_value(data) {
                Ok(payload) => {
                    handlers::servers::handle_load_server_save(services, payload, ctx).await
                }
                Err(error) => Err(error.into()),
            },
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handlers::servers::test_env::TestEnv;

    /// The 18 wire names `ServerExtRouter` is meant to own.
    const OWNED_WIRE_TYPES: &[&str] = &[
        "open_folder",
        "open_in_browser",
        "open_url",
        "list_servers",
        "get_server",
        "detect_workshop_dir",
        "get_server_stats",
        "create_server",
        "import_server",
        "update_server",
        "delete_server",
        "start_server",
        "stop_server",
        "server_api_call",
        "list_server_mods",
        "toggle_server_mod",
        "install_server_mod",
        "load_server_save",
    ];

    /// Asserts ownership, not behavior: every wire name above must come back
    /// `Some(_)` from `route` (Null payloads mostly fail to deserialize, which
    /// is fine — the point is that this router claims the type at all), and at
    /// least one type it does not own must come back `None`.
    #[tokio::test]
    async fn owns_exactly_the_documented_native_types() {
        let mut env = TestEnv::new().await;
        let router = ServerExtRouter {
            services: env.services.clone(),
        };

        for wire in OWNED_WIRE_TYPES {
            let message_type = MessageType::from_wire(wire)
                .unwrap_or_else(|| panic!("{wire} is not a known MessageType"));
            let mut ctx = env.ctx();
            let result = router.route(message_type, Value::Null, &mut ctx).await;
            assert!(result.is_some(), "{wire} must be owned by ServerExtRouter");
        }

        let mut ctx = env.ctx();
        let result = router
            .route(MessageType::GetSettings, Value::Null, &mut ctx)
            .await;
        assert!(
            result.is_none(),
            "get_settings must NOT be owned by ServerExtRouter"
        );
    }
}
