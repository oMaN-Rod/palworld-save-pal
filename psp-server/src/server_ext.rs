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
