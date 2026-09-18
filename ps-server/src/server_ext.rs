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
use crate::mods_conflict_handlers;
use crate::mods_framework_handlers;
use crate::mods_handlers;
use crate::mods_iostore_handlers;
use crate::mods_profile_entry_handlers;
use crate::mods_profile_handlers;
use crate::mods_share_handlers;
use crate::mods_upload_handlers;
use crate::mods_verification_handlers;
use crate::nexus_handlers;
use crate::servers_handlers as servers;
use crate::services::mods::uploads::{UploadStore, EXTENSIONS, PROFILE_EXTENSION};
use crate::services::mods::LibraryPaths;
use crate::services::ServerServices;
use crate::signal_handlers;
use crate::system_native;

pub struct ServerExtRouter {
    pub services: Arc<ServerServices>,
    pub library: LibraryPaths,
    pub uploads: Arc<UploadStore>,
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
            MessageType::ListServers => {
                servers::handle_list_servers(services, &self.library, data, ctx).await
            }
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
                Ok(payload) => servers::handle_import_server(services, payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::UpdateServer => match serde_json::from_value(data) {
                Ok(payload) => {
                    servers::handle_update_server(services, &self.library, payload, ctx).await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::EnsureGamedataLaunchArg => match serde_json::from_value(data) {
                Ok(payload) => {
                    servers::handle_ensure_gamedata_launch_arg(
                        services,
                        &self.library,
                        payload,
                        ctx,
                    )
                    .await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::DeleteServer => match serde_json::from_value(data) {
                Ok(payload) => {
                    servers::handle_delete_server(services, &self.library, payload, ctx).await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::StartServer => match serde_json::from_value(data) {
                Ok(payload) => {
                    servers::handle_start_server(services, &self.library, payload, ctx).await
                }
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
            MessageType::ListLocalSaves => {
                local_saves_handlers::handle_list_local_saves(data, ctx).await
            }
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
            MessageType::GameGuild => bridge_handlers::handle_game_guild(services, data, ctx).await,
            MessageType::GameGuilds => bridge_handlers::handle_game_guilds(services, ctx).await,
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
            MessageType::GameInstanceSetTarget => {
                bridge_instances_handlers::handle_game_instance_set_target(services, data, ctx)
                    .await
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
            MessageType::ModTargetList => mods_handlers::handle_mod_target_list(data, ctx).await,
            MessageType::ModTargetDetect => {
                mods_handlers::handle_mod_target_detect(data, ctx).await
            }
            MessageType::ModTargetAdd => match serde_json::from_value(data) {
                Ok(payload) => mods_handlers::handle_mod_target_add(payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::ModTargetRemove => match serde_json::from_value(data) {
                Ok(payload) => {
                    mods_handlers::handle_mod_target_remove(services, &self.library, payload, ctx)
                        .await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::ModTargetScan => match serde_json::from_value(data) {
                Ok(payload) => mods_handlers::handle_mod_target_scan(payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::ModAdopt => match serde_json::from_value(data) {
                Ok(payload) => mods_handlers::handle_mod_adopt(&self.library, payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::ModAnalyze => match serde_json::from_value(data) {
                Ok(payload) => mods_handlers::handle_mod_analyze(payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::ModInstall => match serde_json::from_value(data) {
                Ok(payload) => mods_handlers::handle_mod_install(&self.library, payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::ModList => mods_handlers::handle_mod_list(data, ctx).await,
            MessageType::ModRemove => match serde_json::from_value(data) {
                Ok(payload) => mods_handlers::handle_mod_remove(&self.library, payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::ModVersionSetCurrent => match serde_json::from_value(data) {
                Ok(payload) => mods_handlers::handle_mod_version_set_current(payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::ModVersionDelete => match serde_json::from_value(data) {
                Ok(payload) => mods_handlers::handle_mod_version_delete(payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::ModBackupList => match serde_json::from_value(data) {
                Ok(payload) => {
                    mods_handlers::handle_mod_backup_list(&self.library, payload, ctx).await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::ModBackupRestore => match serde_json::from_value(data) {
                Ok(payload) => {
                    mods_handlers::handle_mod_backup_restore(
                        &self.services,
                        &self.library,
                        payload,
                        ctx,
                    )
                    .await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::ModBackupDelete => match serde_json::from_value(data) {
                Ok(payload) => {
                    mods_handlers::handle_mod_backup_delete(&self.library, payload, ctx).await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::ProfileList => match serde_json::from_value(data) {
                Ok(payload) => mods_handlers::handle_profile_list(payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::ProfileSetMod => match serde_json::from_value(data) {
                Ok(payload) => {
                    mods_handlers::handle_profile_set_mod(
                        &self.services,
                        &self.library,
                        payload,
                        ctx,
                    )
                    .await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::ProfilePlan => match serde_json::from_value(data) {
                Ok(payload) => mods_handlers::handle_profile_plan(payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::ProfileApply => match serde_json::from_value(data) {
                Ok(payload) => {
                    mods_handlers::handle_profile_apply(&self.services, &self.library, payload, ctx)
                        .await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::ProfileCreate => match serde_json::from_value(data) {
                Ok(payload) => mods_profile_handlers::handle_profile_create(payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::ProfileRename => match serde_json::from_value(data) {
                Ok(payload) => mods_profile_handlers::handle_profile_rename(payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::ProfileDelete => match serde_json::from_value(data) {
                Ok(payload) => {
                    mods_profile_handlers::handle_profile_delete(
                        &self.services,
                        &self.library,
                        payload,
                        ctx,
                    )
                    .await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::ProfileActivate => match serde_json::from_value(data) {
                Ok(payload) => {
                    mods_profile_handlers::handle_profile_activate(
                        &self.services,
                        &self.library,
                        payload,
                        ctx,
                    )
                    .await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::ProfileReorder => match serde_json::from_value(data) {
                Ok(payload) => {
                    mods_profile_handlers::handle_profile_reorder(
                        &self.services,
                        &self.library,
                        payload,
                        ctx,
                    )
                    .await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::ProfileSetOptions => match serde_json::from_value(data) {
                Ok(payload) => {
                    mods_profile_handlers::handle_profile_set_options(
                        &self.services,
                        &self.library,
                        payload,
                        ctx,
                    )
                    .await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::WorldProfileSet => match serde_json::from_value(data) {
                Ok(payload) => mods_profile_handlers::handle_world_profile_set(payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::GameLaunch => match serde_json::from_value(data) {
                Ok(payload) => {
                    mods_profile_handlers::handle_game_launch(
                        &self.services,
                        &self.library,
                        payload,
                        ctx,
                    )
                    .await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::ModUploadBegin => match serde_json::from_value(data) {
                Ok(payload) => {
                    mods_upload_handlers::handle_mod_upload_begin(&self.uploads, payload, ctx).await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::ModUploadChunk => match serde_json::from_value(data) {
                Ok(payload) => {
                    mods_upload_handlers::handle_mod_upload_chunk(&self.uploads, payload, ctx).await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::ModUploadEnd => match serde_json::from_value(data) {
                Ok(payload) => {
                    mods_upload_handlers::handle_mod_upload_end(&self.uploads, payload, ctx).await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::ProfileExport => match serde_json::from_value(data) {
                Ok(payload) => mods_share_handlers::handle_profile_export(payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::ProfileImport => match serde_json::from_value(data) {
                Ok(payload) => {
                    mods_share_handlers::handle_profile_import(
                        &self.library,
                        &self.uploads,
                        payload,
                        ctx,
                    )
                    .await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::FrameworkStatus => match serde_json::from_value(data) {
                Ok(payload) => {
                    mods_framework_handlers::handle_framework_status(services, payload, ctx).await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::FrameworkInstall => match serde_json::from_value(data) {
                Ok(payload) => {
                    mods_framework_handlers::handle_framework_install(
                        services,
                        &self.library,
                        payload,
                        ctx,
                    )
                    .await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::FrameworkRemove => match serde_json::from_value(data) {
                Ok(payload) => {
                    mods_framework_handlers::handle_framework_remove(
                        services,
                        &self.library,
                        payload,
                        ctx,
                    )
                    .await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::FrameworkHazardRemove => match serde_json::from_value(data) {
                Ok(payload) => {
                    mods_framework_handlers::handle_framework_hazard_remove(
                        services,
                        &self.library,
                        payload,
                        ctx,
                    )
                    .await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::ModVerificationGet => match serde_json::from_value(data) {
                Ok(payload) => {
                    mods_verification_handlers::handle_mod_verification_get(services, payload, ctx)
                        .await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::ModVerificationSubscribe => {
                mods_verification_handlers::handle_mod_verification_subscribe(services, data, ctx)
                    .await
            }
            MessageType::ModConflicts => match serde_json::from_value(data) {
                Ok(payload) => mods_conflict_handlers::handle_mod_conflicts(payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::ModIostoreConvert => match serde_json::from_value(data) {
                Ok(payload) => {
                    mods_iostore_handlers::handle_mod_iostore_convert(
                        services,
                        &self.library,
                        payload,
                        ctx,
                    )
                    .await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::ProfileRemoveMod => match serde_json::from_value(data) {
                Ok(payload) => {
                    mods_profile_entry_handlers::handle_profile_remove_mod(payload, ctx).await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::ModReleaseProfiles => match serde_json::from_value(data) {
                Ok(payload) => {
                    mods_profile_entry_handlers::handle_mod_release_profiles(payload, ctx).await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::NexusAccountGet => {
                nexus_handlers::handle_nexus_account_get(services, data, ctx).await
            }
            MessageType::NexusKeySet => {
                nexus_handlers::handle_nexus_key_set(services, data, ctx).await
            }
            MessageType::NexusKeyClear => {
                nexus_handlers::handle_nexus_key_clear(services, data, ctx).await
            }
            MessageType::NexusCategories => {
                nexus_handlers::handle_nexus_categories(services, data, ctx).await
            }
            MessageType::NexusSearch => match serde_json::from_value(data) {
                Ok(payload) => nexus_handlers::handle_nexus_search(services, payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::NexusModFiles => match serde_json::from_value(data) {
                Ok(payload) => nexus_handlers::handle_nexus_mod_files(services, payload, ctx).await,
                Err(error) => Err(error.into()),
            },
            MessageType::NexusDownload => match serde_json::from_value(data) {
                Ok(payload) => {
                    nexus_handlers::handle_nexus_download(
                        services,
                        &self.library,
                        &self.uploads,
                        payload,
                        ctx,
                    )
                    .await
                }
                Err(error) => Err(error.into()),
            },
            MessageType::NexusLinkSubscribe => {
                nexus_handlers::handle_nexus_link_subscribe(services, data, ctx).await
            }
            MessageType::NexusHandlerStatus => {
                nexus_handlers::handle_nexus_handler_status(services, data, ctx).await
            }
            MessageType::NexusHandlerRegister => {
                nexus_handlers::handle_nexus_handler_register(services, data, ctx).await
            }
            MessageType::ModUpdateCheck => {
                nexus_handlers::handle_mod_update_check(services, data, ctx).await
            }
            MessageType::ModUpdateIgnore => match serde_json::from_value(data) {
                Ok(payload) => {
                    nexus_handlers::handle_mod_update_ignore(services, payload, ctx).await
                }
                Err(error) => Err(error.into()),
            },
            _ => return None,
        })
    }

    fn remote_allows(&self, message_type: MessageType, data: &Value) -> bool {
        let Some(path) = data.get("path").and_then(Value::as_str) else {
            return false;
        };
        let extension = std::path::Path::new(path)
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase);
        let extension = extension.as_deref();
        let fits = match message_type {
            MessageType::ProfileImport => extension == Some(PROFILE_EXTENSION),
            MessageType::ModAnalyze | MessageType::ModInstall => {
                extension.is_some_and(|extension| {
                    extension != PROFILE_EXTENSION && EXTENSIONS.contains(&extension)
                })
            }
            _ => false,
        };
        fits && self.uploads.is_upload_path(path)
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
        "game_instance_set_target",
        "game_delete_instance",
        "game_select_instance",
        "game_test_instance",
        "mod_target_list",
        "mod_target_detect",
        "mod_target_add",
        "mod_target_remove",
        "mod_target_scan",
        "mod_adopt",
        "mod_analyze",
        "mod_install",
        "mod_list",
        "mod_remove",
        "mod_version_set_current",
        "mod_version_delete",
        "mod_backup_list",
        "mod_backup_restore",
        "mod_backup_delete",
        "profile_list",
        "profile_set_mod",
        "profile_plan",
        "profile_apply",
        "profile_create",
        "profile_rename",
        "profile_delete",
        "profile_activate",
        "profile_reorder",
        "profile_set_options",
        "world_profile_set",
        "game_launch",
        "mod_upload_begin",
        "mod_upload_chunk",
        "mod_upload_end",
        "profile_export",
        "profile_import",
        "framework_status",
        "framework_install",
        "framework_remove",
        "framework_hazard_remove",
        "mod_verification_get",
        "mod_verification_subscribe",
        "mod_conflicts",
        "mod_iostore_convert",
        "profile_remove_mod",
        "mod_release_profiles",
        "nexus_account_get",
        "nexus_key_set",
        "nexus_key_clear",
        "nexus_categories",
        "nexus_search",
        "nexus_mod_files",
        "nexus_download",
        "nexus_link_subscribe",
        "nexus_handler_status",
        "nexus_handler_register",
        "mod_update_check",
        "mod_update_ignore",
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
            library: LibraryPaths::new(env._scratch.path()),
            uploads: Arc::new(UploadStore::new(env._scratch.path())),
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
