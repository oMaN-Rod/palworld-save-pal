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
}

/// Routes one envelope to its handler. Behavior (matches the Python backend, see
/// plan "Contract deviations" 1-2):
/// - unknown wire string → warn log, nothing sent;
/// - registered type without a Phase-0 handler → warn log, nothing sent;
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

/// Runs `handler` to completion, catching any panic it raises and converting it
/// into an `error` frame via `emitter.emit_error` (spec §5: a handler panic must
/// be contained, not tear down the connection). On a non-panicking completion the
/// handler's own `Result` is passed straight through unchanged.
///
/// Extracted out of `dispatch` as its own function so the panic-containment path
/// is directly unit-testable: `route`'s dispatch table is a fixed `match` over
/// `MessageType`, so no test can register an arbitrary panicking handler through
/// `dispatch` itself. `message_type` is only used for the log line — it plays no
/// part in what gets emitted.
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
        MessageType::GetFriendshipData => {
            handlers::game_data::handle_get_friendship_data(ctx).await
        }
        MessageType::GetMapObjects => handlers::game_data::handle_get_map_objects(ctx).await,
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
        MessageType::ImportPreset => handlers::presets::handle_import_preset(ctx).await,
        MessageType::SelectSave => {
            handlers::save_file::handle_select_save(serde_json::from_value(data)?, ctx).await
        }
        MessageType::LoadZipFile => {
            handlers::save_file::handle_load_zip_file(serde_json::from_value(data)?, ctx).await
        }
        // Phase 2 (Task 13): lazy details, pal CRUD, technologies, lab
        // research, deletes. get_pals / get_lab_research are already
        // registered above (game_data) and are NOT re-added here.
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
        // Phase 2 (Task 14): save-file handlers. None of these four was
        // previously registered — they all fell through to the catch-all.
        MessageType::UpdateSaveFile => {
            handlers::save_file::handle_update_save_file(serde_json::from_value(data)?, ctx).await
        }
        MessageType::DownloadSaveFile => handlers::save_file::handle_download_save_file(ctx).await,
        MessageType::SaveModdedSave => {
            handlers::save_file::handle_save_modded_save(serde_json::from_value(data)?, ctx).await
        }
        MessageType::RenameWorld => {
            handlers::save_file::handle_rename_world(serde_json::from_value(data)?, ctx).await
        }
        // Task 3C-4: UPS pal CRUD.
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
        // Task 3C-5: UPS collection and tag handlers.
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
        // Task 3C-6: UPS <-> save-session interop.
        MessageType::CloneToUps => {
            handlers::ups::handle_clone_to_ups(serde_json::from_value(data)?, ctx).await
        }
        MessageType::ImportToUps => {
            handlers::ups::handle_import_to_ups(serde_json::from_value(data)?, ctx).await
        }
        MessageType::ExportUpsPal => {
            handlers::ups::handle_export_ups_pal(serde_json::from_value(data)?, ctx).await
        }
        // Remaining arms are added by Phases 1-6.
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
        // ws/manager.py discards the dispatcher's {"error": ...} return value,
        // so Python sends NOTHING for unknown types. We match that.
        let mut test = TestContext::new(|_| {}).await;
        dispatch(
            envelope("definitely_not_a_type", serde_json::Value::Null),
            HandlerCtx {
                session: &mut test.session,
                app: &test.app,
                emitter: &test.emitter,
            },
        )
        .await;
        test.assert_no_more_frames();
    }

    #[tokio::test]
    async fn valid_but_unimplemented_type_sends_nothing() {
        // get_guild_raw_data is a valid MessageType but is never routed — a
        // permanently-dead wire type by design. This makes the silence test
        // durable across all phases.
        let mut test = TestContext::new(|_| {}).await;
        dispatch(
            envelope("get_guild_raw_data", serde_json::Value::Null),
            HandlerCtx {
                session: &mut test.session,
                app: &test.app,
                emitter: &test.emitter,
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
            },
        )
        .await;
        assert_eq!(test.next_frame_json()["type"], "get_settings");
    }

    // The dispatch table (`route`'s `match`) is fixed, so `dispatch` itself can
    // never be driven through a panicking handler from a test. `catch_handler_panic`
    // is the extracted seam: exercise it directly with futures that panic in the
    // exact ways a real handler could.
    #[tokio::test]
    async fn catch_handler_panic_converts_panics_into_error_frames() {
        // The default panic hook prints to stderr even though catch_unwind
        // catches it. Silence it for the duration so test output stays
        // pristine, and always restore it afterward via the guard's Drop impl.
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
