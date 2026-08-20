//! Native-only message routing owned by the transport: server management and
//! shell-open. Kept out of the dispatcher so the message layer stays free of
//! Docker, process, and OS-shell dependencies.
use std::sync::Arc;

use serde_json::Value;

use crate::dispatcher::{ExtRouter, HandlerCtx};
use crate::handler_error::HandlerError;
use crate::messages::MessageType;
use crate::servers_handlers as servers;
use crate::services::ServerServices;
use crate::system_native;

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
                Ok(payload) => system_native::handle_open_folder(payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::OpenInBrowser => match serde_json::from_value(data) {
                Ok(payload) => system_native::handle_open_in_browser(payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::OpenUrl => match serde_json::from_value(data) {
                Ok(payload) => system_native::handle_open_url(payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::ListServers => servers::handle_list_servers(services, data, ctx).await,
            MessageType::GetServer => match serde_json::from_value(data) {
                Ok(payload) => servers::handle_get_server(services, payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::DetectWorkshopDir => servers::handle_detect_workshop_dir(data, ctx).await,
            MessageType::GetServerStats => match serde_json::from_value(data) {
                Ok(payload) => servers::handle_get_server_stats(services, payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::CreateServer => match serde_json::from_value(data) {
                Ok(payload) => servers::handle_create_server(services, payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::ImportServer => match serde_json::from_value(data) {
                Ok(payload) => servers::handle_import_server(payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::UpdateServer => match serde_json::from_value(data) {
                Ok(payload) => servers::handle_update_server(services, payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::DeleteServer => match serde_json::from_value(data) {
                Ok(payload) => servers::handle_delete_server(services, payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::StartServer => match serde_json::from_value(data) {
                Ok(payload) => servers::handle_start_server(services, payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::StopServer => match serde_json::from_value(data) {
                Ok(payload) => servers::handle_stop_server(services, payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::ServerApiCall => match serde_json::from_value(data) {
                Ok(payload) => servers::handle_server_api_call(services, payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::ListServerMods => match serde_json::from_value(data) {
                Ok(payload) => servers::handle_list_server_mods(payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::ToggleServerMod => match serde_json::from_value(data) {
                Ok(payload) => servers::handle_toggle_server_mod(payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::InstallServerMod => match serde_json::from_value(data) {
                Ok(payload) => servers::handle_install_server_mod(payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::LoadServerSave => match serde_json::from_value(data) {
                Ok(payload) => servers::handle_load_server_save(services, payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::servers_handlers::test_env::TestEnv;

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
    /// `Some(_)` from `route`, every other `MessageType` must come back `None`.
    /// Iterating `MessageType::ALL` means a new arm added to `route` without a
    /// matching entry in `OWNED_WIRE_TYPES` fails this test.
    #[tokio::test]
    async fn owns_exactly_the_documented_native_types() {
        let mut env = TestEnv::new().await;
        let router = ServerExtRouter {
            services: env.services.clone(),
        };

        for message_type in MessageType::ALL {
            let wire = message_type.as_wire();
            let mut ctx = env.ctx();
            let result = router.route(*message_type, Value::Null, &mut ctx).await;
            if OWNED_WIRE_TYPES.contains(&wire) {
                assert!(result.is_some(), "{wire} must be owned by ServerExtRouter");
            } else {
                assert!(
                    result.is_none(),
                    "{wire} must NOT be claimed by ServerExtRouter"
                );
            }
        }
    }
}
