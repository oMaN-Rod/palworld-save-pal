//! Overview dashboard WS handler: computes the whole-save statistics
//! (totals, composition, leaderboards, and the illegal-pal report) on demand
//! from the loaded session — the same lazy, compute-per-request pattern as
//! `get_pal_summaries`.

use serde_json::json;

use psp_core::domain::overview::overview_stats;

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;

/// With no save loaded, answers under `get_overview_stats` with
/// `{"error": ...}` rather than an `error` frame — the frontend correlates
/// the failure to this request by message type.
pub async fn handle_get_overview_stats(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let Some(session) = ctx.session.save.as_ref() else {
        ctx.emitter.emit(
            MessageType::GetOverviewStats,
            &json!({"error": "No save file loaded"}),
        );
        return Ok(());
    };
    let stats = match overview_stats(session, &ctx.app.game_data) {
        Ok(stats) => stats,
        Err(error) => {
            // Correlated under this message type (not a global `error` frame)
            // so the frontend's overview handler clears its loading state.
            ctx.emitter.emit(
                MessageType::GetOverviewStats,
                &json!({"error": format!("Failed to compute overview stats: {error}")}),
            );
            return Ok(());
        }
    };
    ctx.emitter
        .emit(MessageType::GetOverviewStats, &json!({ "stats": stats }));
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::dispatcher::{dispatch, HandlerCtx};
    use crate::envelope::Envelope;
    use crate::messages::MessageType;
    use crate::test_support::TestContext;

    fn envelope(message_type: &str, data: serde_json::Value) -> Envelope {
        Envelope {
            message_type: message_type.into(),
            data,
        }
    }

    fn ctx<'a>(test: &'a mut TestContext) -> HandlerCtx<'a> {
        HandlerCtx {
            session: &mut test.session,
            app: &test.app,
            emitter: &test.emitter,
            blueprints: &mut test.blueprints,
            attachment: None,
        }
    }

    #[tokio::test]
    async fn no_save_answers_with_an_error_object() {
        let mut test = TestContext::new(|_| {}).await;
        dispatch(
            envelope("get_overview_stats", serde_json::Value::Null),
            ctx(&mut test),
        )
        .await;
        let frame = test.next_frame_json();
        assert_eq!(frame["type"], "get_overview_stats");
        assert_eq!(frame["data"]["error"], "No save file loaded");
        test.assert_no_more_frames();
    }

    #[tokio::test]
    async fn empty_save_emits_zeroed_stats() {
        let mut test = TestContext::new(|json_dir| {
            std::fs::write(
                json_dir.join("pals.json"),
                r#"{"Alpaca": {"is_pal": true}}"#,
            )
            .unwrap();
        })
        .await;
        test.session.save = Some(psp_core::session::SaveSession::new_for_tests(
            psp_core::session::SaveKind::InMemory,
            minimal_level(),
        ));
        dispatch(
            envelope("get_overview_stats", serde_json::Value::Null),
            ctx(&mut test),
        )
        .await;
        let frame = test.next_frame_json();
        assert_eq!(frame["type"], "get_overview_stats");
        assert_eq!(frame["data"]["stats"]["totals"]["players"], 0);
        assert_eq!(frame["data"]["stats"]["totals"]["pals"], 0);
        assert_eq!(frame["data"]["stats"]["anomalies"]["pal_count"], 0);
        test.assert_no_more_frames();
    }

    fn minimal_level() -> psp_core::ue::Save {
        use psp_core::ue::{Properties, Property, StructValue};
        let mut world_save_data = Properties::default();
        world_save_data.insert("CharacterSaveParameterMap", Property::Map(Vec::new()));
        world_save_data.insert("GroupSaveDataMap", Property::Map(Vec::new()));
        world_save_data.insert("ItemContainerSaveData", Property::Map(Vec::new()));
        let mut root_properties = Properties::default();
        root_properties.insert(
            "worldSaveData",
            Property::Struct(StructValue::Struct(world_save_data)),
        );
        psp_core::ue::Save {
            header: psp_core::ue::Header {
                magic: 0,
                save_game_version: 0,
                package_version: psp_core::ue::PackageVersion { ue4: 0, ue5: None },
                engine_version_major: 0,
                engine_version_minor: 0,
                engine_version_patch: 0,
                engine_version_build: 0,
                engine_version: String::new(),
                custom_version: None,
            },
            schemas: psp_core::ue::PropertySchemas::default(),
            root: psp_core::ue::Root {
                save_game_type: String::new(),
                properties: root_properties,
            },
            extra: Vec::new(),
        }
    }

    /// The wire type round-trips: emitted frames carry `get_overview_stats`.
    #[test]
    fn overview_stats_wire_name_round_trips() {
        assert_eq!(
            MessageType::GetOverviewStats.as_wire(),
            "get_overview_stats"
        );
        assert_eq!(
            MessageType::from_wire("get_overview_stats"),
            Some(MessageType::GetOverviewStats)
        );
    }
}
