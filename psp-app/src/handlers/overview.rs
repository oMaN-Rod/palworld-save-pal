//! Overview dashboard WS handler: computes the whole-save statistics
//! (totals, composition, leaderboards, and the illegal-pal report) on demand
//! from the loaded session — the same lazy, compute-per-request pattern as
//! `get_pal_summaries`.

use base64::Engine as _;
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

/// The client picks the file name (it owns the world-name/timestamp format);
/// an empty or absent one falls back rather than writing a nameless file.
#[derive(Debug, serde::Deserialize)]
pub struct ExportOverviewStatsData {
    #[serde(default)]
    pub file_name: Option<String>,
}

/// Desktop writes the report to a native-picked path; the browser cannot be
/// handed a file that way (the webview ignores `<a download>`), so web mode
/// gets the same base64 `[{name, content}]` frame every other export uses.
pub async fn handle_export_overview_stats(
    data: ExportOverviewStatsData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(session) = ctx.session.save.as_ref() else {
        ctx.emitter.emit(
            MessageType::ExportOverviewStats,
            &json!({"error": "No save file loaded"}),
        );
        return Ok(());
    };
    let stats = match overview_stats(session, &ctx.app.game_data) {
        Ok(stats) => stats,
        Err(error) => {
            ctx.emitter.emit(
                MessageType::ExportOverviewStats,
                &json!({"error": format!("Failed to compute overview stats: {error}")}),
            );
            return Ok(());
        }
    };
    let bytes = serde_json::to_vec_pretty(&stats)
        .map_err(|error| HandlerError::Other(format!("overview stats encode failed: {error}")))?;
    let file_name = data
        .file_name
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "overview.json".to_string());

    if ctx.app.config.desktop_mode {
        let request = crate::desktop_dialogs::FileSaveRequest {
            filter_name: "JSON Files",
            filter_extensions: &["json"],
            suggested_file_name: file_name,
            initial_directory: None,
        };
        let Some(path) = ctx.app.dialogs.save_file(request).await else {
            ctx.emitter
                .emit(MessageType::NoFileSelected, &"No file selected");
            return Ok(());
        };
        std::fs::write(&path, &bytes)
            .map_err(|error| HandlerError::Other(format!("Failed to write overview: {error}")))?;
        ctx.emitter.emit(
            MessageType::ExportOverviewStats,
            &json!({
                "message": "Overview exported successfully",
                "file_path": path,
            }),
        );
    } else {
        let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
        ctx.emitter.emit(
            MessageType::ExportOverviewStats,
            &json!([{ "name": file_name, "content": encoded }]),
        );
    }
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

    /// Web mode hands the browser a base64 `[{name, content}]` frame, the same
    /// shape every other export uses, because the desktop webview is the only
    /// place `<a download>` fails -- in the browser it is the working path.
    #[tokio::test]
    async fn web_export_emits_a_named_base64_file() {
        use base64::Engine as _;

        let mut test = TestContext::new(|json_dir| {
            std::fs::write(json_dir.join("pals.json"), r#"{"Alpaca": {"is_pal": true}}"#).unwrap();
        })
        .await;
        test.session.save = Some(psp_core::session::SaveSession::new_for_tests(
            psp_core::session::SaveKind::InMemory,
            minimal_level(),
        ));
        dispatch(
            envelope(
                "export_overview_stats",
                serde_json::json!({"file_name": "overview_MyWorld.json"}),
            ),
            ctx(&mut test),
        )
        .await;

        let frame = test.next_frame_json();
        assert_eq!(frame["type"], "export_overview_stats");
        let files = frame["data"].as_array().expect("web mode emits an array");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0]["name"], "overview_MyWorld.json");
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(files[0]["content"].as_str().expect("content is base64"))
            .expect("content decodes");
        let parsed: serde_json::Value = serde_json::from_slice(&decoded).expect("valid JSON");
        assert_eq!(parsed["totals"]["pals"], 0);
        test.assert_no_more_frames();
    }

    /// A blank name must not produce a nameless download.
    #[tokio::test]
    async fn a_blank_file_name_falls_back() {
        let mut test = TestContext::new(|json_dir| {
            std::fs::write(json_dir.join("pals.json"), r#"{"Alpaca": {"is_pal": true}}"#).unwrap();
        })
        .await;
        test.session.save = Some(psp_core::session::SaveSession::new_for_tests(
            psp_core::session::SaveKind::InMemory,
            minimal_level(),
        ));
        dispatch(
            envelope("export_overview_stats", serde_json::json!({"file_name": "  "})),
            ctx(&mut test),
        )
        .await;
        let frame = test.next_frame_json();
        assert_eq!(frame["data"][0]["name"], "overview.json");
        test.assert_no_more_frames();
    }

    #[tokio::test]
    async fn export_without_a_save_answers_with_an_error_object() {
        let mut test = TestContext::new(|_| {}).await;
        dispatch(
            envelope("export_overview_stats", serde_json::json!({})),
            ctx(&mut test),
        )
        .await;
        let frame = test.next_frame_json();
        assert_eq!(frame["type"], "export_overview_stats");
        assert_eq!(frame["data"]["error"], "No save file loaded");
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
