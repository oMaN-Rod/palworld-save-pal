use std::panic::AssertUnwindSafe;
use std::sync::Arc;

use futures::FutureExt;
use serde_json::Value;

use psp_core::session::Session;

use crate::emitter::Emitter;
use crate::envelope::Envelope;
use crate::handler_error::HandlerError;
use crate::handlers;
use crate::messages::MessageType;
use crate::AppState;

pub struct HandlerCtx<'a> {
    pub session: &'a mut Session,
    pub app: &'a Arc<AppState>,
    pub emitter: &'a Emitter,
    /// The connection's store attachment: its current session id and the `Arc`
    /// backing `session`, so a load handler can register/replace it in the
    /// store. `None` in unit tests that build a ctx directly and never load.
    pub attachment: Option<SessionAttachment<'a>>,
}

/// Links a `HandlerCtx` to the connection's entry in `AppState::sessions`.
/// `arc` is the connection's OWN arc slot (`&mut`), so `reattach_session` can
/// REPLACE it with the store's arc for a different id — setting `current_id`
/// alone does not reattach.
pub struct SessionAttachment<'a> {
    pub current_id: &'a mut Option<uuid::Uuid>,
    pub arc: &'a mut crate::SharedSession,
}

impl HandlerCtx<'_> {
    /// Registers the connection's session in the store under a FRESH id,
    /// dropping the id this connection already held, and returns it. Load
    /// handlers put the returned id in their `loaded_save_files` response. Only
    /// the outer std map lock is taken, briefly — never across an `.await`.
    pub fn register_current_session(&mut self) -> uuid::Uuid {
        let attachment = self
            .attachment
            .as_mut()
            .expect("register_current_session requires a connection attachment");
        let mut store = self.app.sessions.lock().expect("session store poisoned");
        if let Some(previous_id) = attachment.current_id.take() {
            store.remove(&previous_id);
        }
        let new_id = store.register(std::sync::Arc::clone(&*attachment.arc));
        drop(store);
        *attachment.current_id = Some(new_id);
        new_id
    }
}

/// Routes one envelope to its handler. Wire contract:
/// - unknown or unrouted message type → warn log, nothing sent;
/// - handler Err → `error` message {message, trace};
/// - handler panic → contained, reported as an `error` message.
///
/// Never returns an error: the connection loop and socket always survive.
pub async fn dispatch(envelope: Envelope, mut ctx: HandlerCtx<'_>) {
    let Some(message_type) = MessageType::from_wire(&envelope.message_type) else {
        tracing::warn!(message_type = %envelope.message_type, "invalid message type");
        return;
    };

    let emitter = ctx.emitter;
    let routed = catch_handler_panic(
        route(message_type, envelope.data, &mut ctx),
        message_type.as_wire(),
        emitter,
    )
    .await;
    if let Err(handler_error) = routed {
        tracing::error!(message_type = message_type.as_wire(), %handler_error, "handler failed");
        ctx.emitter
            .emit_error(&handler_error.to_string(), &format!("{handler_error:?}"));
    }
}

/// Runs `handler` to completion, converting any panic it raises into an `error`
/// frame so a bad handler cannot tear down the connection. A separate function
/// (rather than inline in `dispatch`) so the containment path is unit-testable:
/// `route`'s dispatch table is a fixed `match`, so no test can push a panicking
/// handler through `dispatch` itself.
async fn catch_handler_panic<F>(
    handler: F,
    message_type: &str,
    emitter: &Emitter,
) -> Result<(), HandlerError>
where
    F: std::future::Future<Output = Result<(), HandlerError>>,
{
    match AssertUnwindSafe(handler).catch_unwind().await {
        Ok(result) => result,
        Err(panic_payload) => {
            let panic_text = panic_payload
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| panic_payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "handler panicked".to_string());
            tracing::error!(message_type, %panic_text, "handler panicked");
            emitter.emit_error(&panic_text, "handler panicked");
            Ok(())
        }
    }
}

async fn route(
    message_type: MessageType,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    match message_type {
        MessageType::GetSettings => handlers::settings::handle_get_settings(ctx).await,
        MessageType::UpdateSettings => {
            handlers::settings::handle_update_settings(serde_json::from_value(data)?, ctx).await
        }
        MessageType::GetActiveSkills => handlers::game_data::handle_get_active_skills(ctx).await,
        MessageType::GetPassiveSkills => handlers::game_data::handle_get_passive_skills(ctx).await,
        MessageType::GetTechnologies => handlers::game_data::handle_get_technologies(ctx).await,
        MessageType::GetElements => handlers::game_data::handle_get_elements(ctx).await,
        MessageType::GetItems => handlers::game_data::handle_get_items(ctx).await,
        MessageType::GetMissions => handlers::game_data::handle_get_missions(ctx).await,
        MessageType::GetBuildings => handlers::game_data::handle_get_buildings(ctx).await,
        MessageType::GetWorkSuitability => {
            handlers::game_data::handle_get_work_suitability(ctx).await
        }
        MessageType::GetExpData => handlers::game_data::handle_get_exp_data(ctx).await,
        MessageType::GetRelicData => handlers::game_data::handle_get_relic_data(ctx).await,
        MessageType::GetFriendshipData => {
            handlers::game_data::handle_get_friendship_data(ctx).await
        }
        MessageType::GetMapObjects => handlers::game_data::handle_get_map_objects(ctx).await,
        MessageType::GetBosses => handlers::game_data::handle_get_bosses(ctx).await,
        MessageType::GetRelics => handlers::game_data::handle_get_relics(ctx).await,
        MessageType::GetFastTravelPoints => {
            handlers::game_data::handle_get_fast_travel_points(ctx).await
        }
        MessageType::GetEffigies => handlers::game_data::handle_get_effigies(ctx).await,
        MessageType::GetUiCommon => handlers::game_data::handle_get_ui_common(ctx).await,
        MessageType::GetVersion => handlers::game_data::handle_get_version(ctx).await,
        MessageType::GetPals => handlers::game_data::handle_get_pals(ctx).await,
        MessageType::GetLabResearch => handlers::game_data::handle_get_lab_research(ctx).await,
        MessageType::SyncAppState => handlers::system::handle_sync_app_state(ctx).await,
        MessageType::GetPresets => handlers::presets::handle_get_presets(ctx).await,
        MessageType::AddPreset => handlers::presets::handle_add_preset(data, ctx).await,
        MessageType::UpdatePreset => {
            handlers::presets::handle_update_preset(serde_json::from_value(data)?, ctx).await
        }
        MessageType::DeletePreset => {
            handlers::presets::handle_delete_presets(serde_json::from_value(data)?, ctx).await
        }
        MessageType::NukePresets => handlers::presets::handle_nuke_presets(ctx).await,
        MessageType::ExportPreset => {
            handlers::presets::handle_export_preset(serde_json::from_value(data)?, ctx).await
        }
        MessageType::ExportPresets => {
            handlers::presets::handle_export_presets(serde_json::from_value(data)?, ctx).await
        }
        MessageType::ImportPreset => handlers::presets::handle_import_preset(ctx).await,
        MessageType::SelectSave => {
            handlers::save_file::handle_select_save(serde_json::from_value(data)?, ctx).await
        }
        // `data` is a BARE save-id string, not an object.
        MessageType::SelectGamepassSave => {
            handlers::gamepass::handle_select_gamepass_save(serde_json::from_value(data)?, ctx)
                .await
        }
        MessageType::LoadZipFile => {
            handlers::save_file::handle_load_zip_file(serde_json::from_value(data)?, ctx).await
        }
        MessageType::RequestPlayerDetails => {
            handlers::players::handle_request_player_details(serde_json::from_value(data)?, ctx)
                .await
        }
        MessageType::RequestGuildDetails => {
            handlers::guilds::handle_request_guild_details(serde_json::from_value(data)?, ctx).await
        }
        MessageType::GetPalSummaries => handlers::pals::handle_get_pal_summaries(ctx).await,
        MessageType::AddPal => {
            handlers::pals::handle_add_pal(serde_json::from_value(data)?, ctx).await
        }
        MessageType::AddDpsPal => {
            handlers::pals::handle_add_dps_pal(serde_json::from_value(data)?, ctx).await
        }
        MessageType::ClonePal => {
            handlers::pals::handle_clone_pal(serde_json::from_value(data)?, ctx).await
        }
        MessageType::CloneDpsPal => {
            handlers::pals::handle_clone_dps_pal(serde_json::from_value(data)?, ctx).await
        }
        MessageType::DeletePals => {
            handlers::pals::handle_delete_pals(serde_json::from_value(data)?, ctx).await
        }
        MessageType::DeleteDpsPals => {
            handlers::pals::handle_delete_dps_pals(serde_json::from_value(data)?, ctx).await
        }
        MessageType::MovePal => {
            handlers::pals::handle_move_pal(serde_json::from_value(data)?, ctx).await
        }
        MessageType::HealPals => {
            handlers::pals::handle_heal_pals(serde_json::from_value(data)?, ctx).await
        }
        MessageType::HealAllPals => {
            handlers::pals::handle_heal_all_pals(serde_json::from_value(data)?, ctx).await
        }
        MessageType::SetTechnologyData => {
            handlers::players::handle_set_technology_data(serde_json::from_value(data)?, ctx).await
        }
        MessageType::UpdateLabResearch => {
            handlers::guilds::handle_update_lab_research(serde_json::from_value(data)?, ctx).await
        }
        MessageType::DeletePlayer => {
            handlers::players::handle_delete_player(serde_json::from_value(data)?, ctx).await
        }
        MessageType::DeleteGuild => {
            handlers::guilds::handle_delete_guild(serde_json::from_value(data)?, ctx).await
        }
        MessageType::UpdateSaveFile => {
            handlers::save_file::handle_update_save_file(serde_json::from_value(data)?, ctx).await
        }
        MessageType::DownloadSaveFile => handlers::save_file::handle_download_save_file(ctx).await,
        MessageType::SaveModdedSave => {
            handlers::save_file::handle_save_modded_save(serde_json::from_value(data)?, ctx).await
        }
        MessageType::SaveEditedSav => {
            handlers::save_file::handle_save_edited_sav(serde_json::from_value(data)?, ctx).await
        }
        MessageType::RenameWorld => {
            handlers::save_file::handle_rename_world(serde_json::from_value(data)?, ctx).await
        }
        MessageType::GetUpsPals => {
            handlers::ups::handle_get_ups_pals(serde_json::from_value(data)?, ctx).await
        }
        MessageType::GetUpsAllFilteredIds => {
            handlers::ups::handle_get_ups_all_filtered_ids(serde_json::from_value(data)?, ctx).await
        }
        MessageType::AddUpsPal => {
            handlers::ups::handle_add_ups_pal(serde_json::from_value(data)?, ctx).await
        }
        MessageType::UpdateUpsPal => {
            handlers::ups::handle_update_ups_pal(serde_json::from_value(data)?, ctx).await
        }
        MessageType::DeleteUpsPals => {
            handlers::ups::handle_delete_ups_pals(serde_json::from_value(data)?, ctx).await
        }
        MessageType::CloneUpsPal => {
            handlers::ups::handle_clone_ups_pal(serde_json::from_value(data)?, ctx).await
        }
        MessageType::GetUpsStats => handlers::ups::handle_get_ups_stats(ctx).await,
        MessageType::NukeUpsPals => handlers::ups::handle_nuke_ups_pals(ctx).await,
        MessageType::GetUpsCollections => handlers::ups::handle_get_ups_collections(ctx).await,
        MessageType::CreateUpsCollection => {
            handlers::ups::handle_create_ups_collection(serde_json::from_value(data)?, ctx).await
        }
        MessageType::UpdateUpsCollection => {
            handlers::ups::handle_update_ups_collection(serde_json::from_value(data)?, ctx).await
        }
        MessageType::DeleteUpsCollection => {
            handlers::ups::handle_delete_ups_collection(serde_json::from_value(data)?, ctx).await
        }
        MessageType::GetUpsTags => handlers::ups::handle_get_ups_tags(ctx).await,
        MessageType::CreateUpsTag => {
            handlers::ups::handle_create_ups_tag(serde_json::from_value(data)?, ctx).await
        }
        MessageType::UpdateUpsTag => {
            handlers::ups::handle_update_ups_tag(serde_json::from_value(data)?, ctx).await
        }
        MessageType::DeleteUpsTag => {
            handlers::ups::handle_delete_ups_tag(serde_json::from_value(data)?, ctx).await
        }
        MessageType::CloneToUps => {
            handlers::ups::handle_clone_to_ups(serde_json::from_value(data)?, ctx).await
        }
        MessageType::ImportToUps => {
            handlers::ups::handle_import_to_ups(serde_json::from_value(data)?, ctx).await
        }
        MessageType::ExportUpsPal => {
            handlers::ups::handle_export_ups_pal(serde_json::from_value(data)?, ctx).await
        }
        // GetGpsPals has no arm on purpose: it is a permanently dead wire type.
        MessageType::RequestGps => handlers::gps::handle_request_gps(ctx).await,
        MessageType::AddGpsPal => {
            handlers::gps::handle_add_gps_pal(serde_json::from_value(data)?, ctx).await
        }
        MessageType::CloneGpsPal => {
            handlers::gps::handle_clone_gps_pal(serde_json::from_value(data)?, ctx).await
        }
        MessageType::DeleteGpsPals => {
            handlers::gps::handle_delete_gps_pals(serde_json::from_value(data)?, ctx).await
        }
        MessageType::CloneGpsPalToPlayer => {
            handlers::gps::handle_clone_gps_pal_to_player(serde_json::from_value(data)?, ctx).await
        }
        MessageType::ConvertSteamId => {
            handlers::tools::handle_convert_steam_id(serde_json::from_value(data)?, ctx).await
        }
        MessageType::LoadSourceSave => {
            handlers::tools::handle_load_source_save(serde_json::from_value(data)?, ctx).await
        }
        MessageType::GetSourcePlayers => handlers::tools::handle_get_source_players(ctx).await,
        MessageType::TransferPlayer => {
            handlers::tools::handle_transfer_player(serde_json::from_value(data)?, ctx).await
        }
        MessageType::UnloadSourceSave => handlers::tools::handle_unload_source_save(ctx).await,
        MessageType::SwapPlayerUids => {
            handlers::tools::handle_swap_player_uids(serde_json::from_value(data)?, ctx).await
        }
        // GetGuildRawData has no arm on purpose: it is a permanently dead wire
        // type, pinned by valid_but_unimplemented_type_sends_nothing below.
        MessageType::GetRawData => {
            handlers::tools::handle_get_raw_data(serde_json::from_value(data)?, ctx).await
        }
        MessageType::ScanGamepassSaves => handlers::gamepass::handle_scan_gamepass_saves(ctx).await,
        MessageType::DeleteGamepassSave => {
            handlers::gamepass::handle_delete_gamepass_save(serde_json::from_value(data)?, ctx)
                .await
        }
        MessageType::DeleteGamepassPlayer => {
            handlers::gamepass::handle_delete_gamepass_player(serde_json::from_value(data)?, ctx)
                .await
        }
        MessageType::RenameGamepassWorld => {
            handlers::gamepass::handle_rename_gamepass_world(serde_json::from_value(data)?, ctx)
                .await
        }
        MessageType::ConvertSaveFormat => {
            handlers::gamepass::handle_convert_save_format(serde_json::from_value(data)?, ctx).await
        }
        MessageType::ConvertSavFile => {
            handlers::save_file::handle_convert_sav_file(serde_json::from_value(data)?, ctx).await
        }
        MessageType::UnlockMap => {
            handlers::save_file::handle_unlock_map(serde_json::from_value(data)?, ctx).await
        }
        MessageType::OpenFolder => {
            handlers::system::handle_open_folder(serde_json::from_value(data)?, ctx).await
        }
        MessageType::OpenInBrowser => {
            handlers::system::handle_open_in_browser(serde_json::from_value(data)?, ctx).await
        }
        MessageType::OpenUrl => {
            handlers::system::handle_open_url(serde_json::from_value(data)?, ctx).await
        }
        MessageType::ListServers => handlers::servers::handle_list_servers(data, ctx).await,
        MessageType::GetServer => {
            handlers::servers::handle_get_server(serde_json::from_value(data)?, ctx).await
        }
        MessageType::DetectWorkshopDir => {
            handlers::servers::handle_detect_workshop_dir(data, ctx).await
        }
        MessageType::GetServerStats => {
            handlers::servers::handle_get_server_stats(serde_json::from_value(data)?, ctx).await
        }
        MessageType::CreateServer => {
            handlers::servers::handle_create_server(serde_json::from_value(data)?, ctx).await
        }
        MessageType::ImportServer => {
            handlers::servers::handle_import_server(serde_json::from_value(data)?, ctx).await
        }
        MessageType::UpdateServer => {
            handlers::servers::handle_update_server(serde_json::from_value(data)?, ctx).await
        }
        MessageType::DeleteServer => {
            handlers::servers::handle_delete_server(serde_json::from_value(data)?, ctx).await
        }
        MessageType::StartServer => {
            handlers::servers::handle_start_server(serde_json::from_value(data)?, ctx).await
        }
        MessageType::StopServer => {
            handlers::servers::handle_stop_server(serde_json::from_value(data)?, ctx).await
        }
        MessageType::ServerApiCall => {
            handlers::servers::handle_server_api_call(serde_json::from_value(data)?, ctx).await
        }
        MessageType::ListServerMods => {
            handlers::servers::handle_list_server_mods(serde_json::from_value(data)?, ctx).await
        }
        MessageType::ToggleServerMod => {
            handlers::servers::handle_toggle_server_mod(serde_json::from_value(data)?, ctx).await
        }
        MessageType::InstallServerMod => {
            handlers::servers::handle_install_server_mod(serde_json::from_value(data)?, ctx).await
        }
        // ServerPlayerCount has no arm on purpose: it is a permanently dead
        // wire type.
        MessageType::LoadServerSave => {
            handlers::servers::handle_load_server_save(serde_json::from_value(data)?, ctx).await
        }
        // session_not_found is emit-only, so it has no inbound arm.
        MessageType::ReattachSession => {
            handlers::session::handle_reattach_session(serde_json::from_value(data)?, ctx).await
        }
        MessageType::EjectSession => {
            handlers::session::handle_eject_session(serde_json::from_value(data)?, ctx).await
        }
        MessageType::GetWorldOption => handlers::world_option::handle_get_world_option(ctx).await,
        MessageType::UpdateWorldOption => {
            handlers::world_option::handle_update_world_option(serde_json::from_value(data)?, ctx)
                .await
        }
        other => {
            tracing::warn!(
                message_type = other.as_wire(),
                "handler not implemented yet (Phase 0)"
            );
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::Envelope;
    use crate::test_support::TestContext;

    /// Restores the previous panic hook on drop, so a failing assertion in a test
    /// that silences panic output cannot leak the silent hook into sibling tests
    /// (the harness runs them in one process).
    #[allow(clippy::type_complexity)]
    struct PanicHookGuard(
        Option<Box<dyn Fn(&std::panic::PanicHookInfo<'_>) + Sync + Send + 'static>>,
    );

    impl Drop for PanicHookGuard {
        fn drop(&mut self) {
            if let Some(previous_hook) = self.0.take() {
                std::panic::set_hook(previous_hook);
            }
        }
    }

    fn envelope(message_type: &str, data: serde_json::Value) -> Envelope {
        Envelope {
            message_type: message_type.into(),
            data,
        }
    }

    #[tokio::test]
    async fn unknown_type_sends_nothing() {
        let mut test = TestContext::new(|_| {}).await;
        dispatch(
            envelope("definitely_not_a_type", serde_json::Value::Null),
            HandlerCtx {
                session: &mut test.session,
                app: &test.app,
                emitter: &test.emitter,
                attachment: None,
            },
        )
        .await;
        test.assert_no_more_frames();
    }

    #[tokio::test]
    async fn valid_but_unimplemented_type_sends_nothing() {
        // get_guild_raw_data is a valid MessageType that is never routed.
        let mut test = TestContext::new(|_| {}).await;
        dispatch(
            envelope("get_guild_raw_data", serde_json::Value::Null),
            HandlerCtx {
                session: &mut test.session,
                app: &test.app,
                emitter: &test.emitter,
                attachment: None,
            },
        )
        .await;
        test.assert_no_more_frames();
    }

    #[tokio::test]
    async fn bad_payload_becomes_error_message() {
        let mut test = TestContext::new(|_| {}).await;
        dispatch(
            envelope("update_settings", serde_json::json!(42)),
            HandlerCtx {
                session: &mut test.session,
                app: &test.app,
                emitter: &test.emitter,
                attachment: None,
            },
        )
        .await;
        let frame = test.next_frame_json();
        assert_eq!(frame["type"], "error");
        assert!(frame["data"]["message"]
            .as_str()
            .unwrap()
            .contains("invalid payload"));
        assert!(frame["data"]["trace"].is_string());
        test.assert_no_more_frames();
    }

    #[tokio::test]
    async fn sync_app_state_routes_and_emits_settings() {
        let mut test = TestContext::new(|_| {}).await;
        dispatch(
            envelope("sync_app_state", serde_json::Value::Null),
            HandlerCtx {
                session: &mut test.session,
                app: &test.app,
                emitter: &test.emitter,
                attachment: None,
            },
        )
        .await;
        assert_eq!(test.next_frame_json()["type"], "get_settings");
        test.assert_no_more_frames();
    }

    #[tokio::test]
    async fn get_settings_routes_to_handler() {
        let mut test = TestContext::new(|_| {}).await;
        dispatch(
            envelope("get_settings", serde_json::Value::Null),
            HandlerCtx {
                session: &mut test.session,
                app: &test.app,
                emitter: &test.emitter,
                attachment: None,
            },
        )
        .await;
        assert_eq!(test.next_frame_json()["type"], "get_settings");
    }

    #[tokio::test]
    async fn catch_handler_panic_converts_panics_into_error_frames() {
        // The default panic hook prints to stderr even though catch_unwind
        // catches it. Silence it for the duration so test output stays clean;
        // the guard's Drop restores it.
        let _hook_guard = PanicHookGuard(Some(std::panic::take_hook()));
        std::panic::set_hook(Box::new(|_| {}));

        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let emitter = Emitter::new(sender);

        // `panic!("literal")` with no format args panics with a `&'static str` payload.
        let result = catch_handler_panic(async { panic!("boom") }, "get_settings", &emitter).await;
        assert!(
            result.is_ok(),
            "a caught panic must not propagate out of catch_handler_panic"
        );
        let frame = crate::test_support::next_frame_json_from(&mut receiver);
        assert_eq!(frame["type"], "error");
        assert!(
            frame["data"]["message"].as_str().unwrap().contains("boom"),
            "expected the panic's own text in the error frame, got {frame:?}"
        );

        // `panic!("{}", ...)` goes through the formatting path and panics with an
        // owned `String` payload instead.
        let result =
            catch_handler_panic(async { panic!("boom-{}", 42) }, "get_settings", &emitter).await;
        assert!(result.is_ok());
        let frame = crate::test_support::next_frame_json_from(&mut receiver);
        assert_eq!(frame["type"], "error");
        assert!(
            frame["data"]["message"]
                .as_str()
                .unwrap()
                .contains("boom-42"),
            "expected the formatted panic text in the error frame, got {frame:?}"
        );

        // A payload that is neither `&str` nor `String` must still produce an
        // `error` frame carrying the generic fallback text, not panic again.
        let result = catch_handler_panic(
            async { std::panic::panic_any(42i32) },
            "get_settings",
            &emitter,
        )
        .await;
        assert!(result.is_ok());
        let frame = crate::test_support::next_frame_json_from(&mut receiver);
        assert_eq!(frame["type"], "error");
        assert_eq!(frame["data"]["message"], "handler panicked");

        assert!(
            receiver.try_recv().is_err(),
            "expected exactly three frames, no more"
        );
    }
}
