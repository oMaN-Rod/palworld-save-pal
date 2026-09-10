//! Native-only message routing owned by the transport: server management and
//! shell-open. Kept out of the dispatcher so the message layer stays free of
//! Docker, process, and OS-shell dependencies.
use std::sync::Arc;

use serde_json::Value;

use crate::bridge_handlers;
use crate::bridge_instances_handlers;
use crate::dispatcher::{ExtRouter, HandlerCtx};
use crate::handler_error::HandlerError;
use crate::local_saves_handlers;
use crate::messages::MessageType;
use crate::servers_handlers as servers;
use crate::services::ServerServices;
use crate::signal_handlers;
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
            MessageType::EnsureGamedataLaunchArg => match serde_json::from_value(data) {
                Ok(payload) => {
                    servers::handle_ensure_gamedata_launch_arg(services, payload, ctx).await
                }
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
            MessageType::SubscribeLive => signal_handlers::handle_subscribe_live(data, ctx).await,
            MessageType::SignalStatus => {
                signal_handlers::handle_signal_status(services, data, ctx).await
            }
            MessageType::SignalSetSource => match serde_json::from_value(data) {
                Ok(payload) => {
                    signal_handlers::handle_signal_set_source(services, payload, ctx).await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::SignalStartPairing => {
                signal_handlers::handle_signal_start_pairing(services, data, ctx).await
            }
            MessageType::SignalStopPairing => {
                signal_handlers::handle_signal_stop_pairing(services, data, ctx).await
            }
            MessageType::SignalSetArmed => {
                signal_handlers::handle_signal_set_armed(services, data, ctx).await
            }
            MessageType::SignalListDevices => {
                signal_handlers::handle_signal_list_devices(services, data, ctx).await
            }
            MessageType::SignalRenameDevice => {
                signal_handlers::handle_signal_rename_device(services, data, ctx).await
            }
            MessageType::SignalRevokeDevice => {
                signal_handlers::handle_signal_revoke_device(services, data, ctx).await
            }
            MessageType::SignalResetRemoteAccess => {
                signal_handlers::handle_signal_reset_remote_access(services, data, ctx).await
            }
            MessageType::ListLocalSaves => local_saves_handlers::handle_list_local_saves(ctx).await,
            MessageType::BrowseDirectory => match serde_json::from_value(data) {
                Ok(payload) => local_saves_handlers::handle_browse_directory(payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::GameStatus => bridge_handlers::handle_game_status(services, ctx).await,
            MessageType::GamePlayers => bridge_handlers::handle_game_players(services, ctx).await,
            MessageType::GamePals => bridge_handlers::handle_game_pals(services, data, ctx).await,
            MessageType::GamePalDetail => {
                bridge_handlers::handle_game_pal_detail(services, data, ctx).await
            }
            MessageType::GameInventory => {
                bridge_handlers::handle_game_inventory(services, data, ctx).await
            }
            MessageType::GameGuild => {
                bridge_handlers::handle_game_guild(services, data, ctx).await
            }
            MessageType::GameGuilds => {
                bridge_handlers::handle_game_guilds(services, ctx).await
            }
            MessageType::GameBasePals => {
                bridge_handlers::handle_game_base_pals(services, data, ctx).await
            }
            MessageType::GameGuildContainers => {
                bridge_handlers::handle_game_guild_containers(services, data, ctx).await
            }
            MessageType::GameEditGuild => {
                bridge_handlers::handle_game_edit_guild(services, data, ctx).await
            }
            MessageType::GameSetGuildRole => {
                bridge_handlers::handle_game_set_guild_role(services, data, ctx).await
            }
            MessageType::GameInstances => {
                bridge_instances_handlers::handle_game_instances(services, ctx).await
            }
            MessageType::GameAddInstance => {
                bridge_instances_handlers::handle_game_add_instance(services, data, ctx).await
            }
            MessageType::GameUpdateInstance => {
                bridge_instances_handlers::handle_game_update_instance(services, data, ctx).await
            }
            MessageType::GameDeleteInstance => {
                bridge_instances_handlers::handle_game_delete_instance(services, data, ctx).await
            }
            MessageType::GameSelectInstance => {
                bridge_instances_handlers::handle_game_select_instance(services, data, ctx).await
            }
            MessageType::GameTestInstance => {
                bridge_instances_handlers::handle_game_test_instance(services, data, ctx).await
            }
            MessageType::GameCapabilities => {
                bridge_handlers::handle_game_capabilities(services, ctx).await
            }
            MessageType::GameHealPals => {
                bridge_handlers::handle_game_heal_pals(services, data, ctx).await
            }
            MessageType::GameSetItemSlot => {
                bridge_handlers::handle_game_set_item_slot(services, data, ctx).await
            }
            MessageType::GameRemovePal => {
                bridge_handlers::handle_game_remove_pal(services, data, ctx).await
            }
            MessageType::GameMovePal => {
                bridge_handlers::handle_game_move_pal(services, data, ctx).await
            }
            MessageType::GameAddPal => {
                bridge_handlers::handle_game_add_pal(services, data, ctx).await
            }
            MessageType::GameEditPal => {
                bridge_handlers::handle_game_edit_pal(services, data, ctx).await
            }
            MessageType::GameEditPlayer => {
                bridge_handlers::handle_game_edit_player(services, data, ctx).await
            }
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
        "ensure_gamedata_launch_arg",
        "delete_server",
        "start_server",
        "stop_server",
        "server_api_call",
        "list_server_mods",
        "toggle_server_mod",
        "install_server_mod",
        "load_server_save",
        "subscribe_live",
        "signal_status",
        "signal_set_source",
        "signal_start_pairing",
        "signal_stop_pairing",
        "signal_set_armed",
        "signal_list_devices",
        "signal_rename_device",
        "signal_revoke_device",
        "signal_reset_remote_access",
        "list_local_saves",
        "browse_directory",
        "game_status",
        "game_players",
        "game_pals",
        "game_pal_detail",
        "game_inventory",
        "game_guild",
        "game_guilds",
        "game_base_pals",
        "game_guild_containers",
        "game_edit_guild",
        "game_set_guild_role",
        "game_capabilities",
        "game_heal_pals",
        "game_set_item_slot",
        "game_remove_pal",
        "game_move_pal",
        "game_add_pal",
        "game_edit_pal",
        "game_edit_player",
        "game_instances",
        "game_add_instance",
        "game_update_instance",
        "game_delete_instance",
        "game_select_instance",
        "game_test_instance",
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
