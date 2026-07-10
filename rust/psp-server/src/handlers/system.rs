use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::handlers::save_file::emit_summary_messages;
use crate::handlers::settings::settings_dto_from_row;
use crate::messages::MessageType;

/// Port of app_state_handler.py sync_app_state_handler's dict literal.
#[derive(Debug, serde::Serialize)]
struct SyncLoadedSaveFilesData {
    level: String,
    players: Vec<String>,
    guilds: Vec<String>,
    world_name: String,
    r#type: &'static str,
    size: u64,
    has_gps: bool,
}

/// Port of app_state_handler.py sync_app_state_handler: settings first,
/// then -- only when a save is loaded -- loaded_save_files followed by both
/// summary messages.
pub async fn handle_sync_app_state(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let row = psp_db::settings::get_settings(&ctx.app.db).await?;
    ctx.emitter
        .emit(MessageType::GetSettings, &settings_dto_from_row(row));

    let Some(session) = ctx.session.save.as_ref() else {
        tracing::warn!("no save file loaded");
        return Ok(());
    };

    // Unlike select_save's/load_zip_file's `players` array (filesystem/zip
    // *discovery* order, see save_file.rs), these two fields intentionally
    // keep the BTreeMap's sorted order rather than chasing Python's.
    // Python's `sync_app_state_handler` emits
    // `[str(p) for p in app_state.player_summaries.keys()]` /
    // `.guild_summaries.keys()`, and those dicts' insertion order is not a
    // filesystem-discovery order at all:
    //   - `player_summaries` is built by `_extract_player_summaries`, which
    //     for saves with more than two players dispatches to
    //     `_extract_players_parallel` -- a `ThreadPoolExecutor` whose results
    //     are inserted via `as_completed()`, i.e. whichever worker thread
    //     finishes first. That insertion order is genuinely
    //     non-deterministic across runs of the same save file, so there is
    //     no stable Python order to port here (psp-core's own
    //     `domain/summaries.rs` already documents this same fact and is why
    //     Task 10's parity harness restricts itself to <=2-player saves).
    //   - `guild_summaries` is deterministic per save file, but its order is
    //     `_group_save_data_map`'s (Level.sav's internal GroupSaveDataMap)
    //     iteration order, filtered to guild-type entries -- an artifact of
    //     the save's binary layout, not anything this port currently
    //     threads through as a separate ordered list anywhere.
    // Sorted-by-UUID is therefore not a parity gap here the way it was for
    // select_save's `players` array; it is a reasonable, stable choice where
    // Python itself has no fixed answer.
    let payload = SyncLoadedSaveFilesData {
        level: session.save_id.clone(),
        players: session
            .player_summaries
            .keys()
            .map(|uid| uid.to_string())
            .collect(),
        guilds: session
            .guild_summaries
            .keys()
            .map(|guild_id| guild_id.to_string())
            .collect(),
        world_name: session.world_name.clone(),
        r#type: session.save_type_label,
        size: session.size,
        has_gps: session.has_gps_available(),
    };
    ctx.emitter.emit(MessageType::LoadedSaveFiles, &payload);
    emit_summary_messages(session, ctx.emitter);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestContext;

    #[tokio::test]
    async fn sync_app_state_without_save_emits_only_settings() {
        // sync_app_state is the ONLY path by which settings reach the UI during
        // bootstrap() — so this asserts the full six-field payload, not just
        // `language`. `save_dir` is the most delicate field in it: Python emits
        // `null` on a fresh DB (a deterministic import-order bug — see
        // rust/parity/README.md), Rust correctly emits
        // `default_steam_save_dir()`, and that divergence is deliberately left
        // unmasked (PARITY_IGNORED_PATHS stays empty). Pinning the real default
        // here, rather than merely `is_string()`, is what would catch a
        // regression back to `null`/an empty string.
        let mut test = TestContext::new(|_| {}).await;
        let mut ctx = HandlerCtx {
            session: &mut test.session,
            app: &test.app,
            emitter: &test.emitter,
        };
        handle_sync_app_state(&mut ctx).await.unwrap();
        let frame = test.next_frame_json();
        assert_eq!(frame["type"], "get_settings");
        assert_eq!(frame["data"]["language"], "en");
        assert_eq!(
            frame["data"]["save_dir"],
            psp_db::settings::default_steam_save_dir()
        );
        assert_eq!(frame["data"]["clone_prefix"], "©️");
        assert_eq!(frame["data"]["new_pal_prefix"], "🆕");
        assert_eq!(frame["data"]["debug_mode"], false);
        assert_eq!(frame["data"]["cheat_mode"], false);
        test.assert_no_more_frames();
    }

    /// A `SaveSession` whose only content that matters to
    /// `handle_sync_app_state` is populated -- everything else (the `level`
    /// GVAS tree, the position indexes) is a harmless empty placeholder,
    /// same pattern as psp-core's own `session_with_level_properties` test
    /// helper. All `SaveSession` fields are `pub` (Task 7), so this struct
    /// literal is legal from outside the crate.
    fn fake_loaded_session() -> psp_core::session::SaveSession {
        use psp_core::dto::summary::{GuildSummary, PlayerSummary};
        use psp_core::session::{SaveKind, SaveSession};
        use std::collections::{BTreeMap, HashMap};

        let mut player_summaries = BTreeMap::new();
        let player_uid: uuid::Uuid = "11111111-1111-1111-1111-111111111111".parse().unwrap();
        player_summaries.insert(
            player_uid,
            PlayerSummary {
                uid: player_uid,
                nickname: "Tester".to_string(),
                level: Some(9),
                guild_id: None,
                pal_count: 0,
                last_online_time: None,
                loaded: false,
            },
        );

        let mut guild_summaries = BTreeMap::new();
        let guild_id: uuid::Uuid = "22222222-2222-2222-2222-222222222222".parse().unwrap();
        guild_summaries.insert(
            guild_id,
            GuildSummary {
                id: guild_id,
                name: "The Guild".to_string(),
                admin_player_uid: Some(player_uid),
                player_count: 1,
                base_count: 0,
                level: Some(1),
                pal_count: 0,
                loaded: false,
            },
        );

        SaveSession {
            kind: SaveKind::InMemory,
            world_name: "My World".to_string(),
            level: uesave::Save {
                header: uesave::Header {
                    magic: 0,
                    save_game_version: 0,
                    package_version: uesave::PackageVersion { ue4: 0, ue5: None },
                    engine_version_major: 0,
                    engine_version_minor: 0,
                    engine_version_patch: 0,
                    engine_version_build: 0,
                    engine_version: String::new(),
                    custom_version: None,
                },
                schemas: uesave::PropertySchemas::default(),
                root: uesave::Root {
                    save_game_type: String::new(),
                    properties: uesave::Properties::default(),
                },
                extra: Vec::new(),
            },
            save_id: "C:/saves/world/Level.sav".to_string(),
            save_type_label: "steam",
            size: 12345,
            level_meta: None,
            player_file_refs: BTreeMap::new(),
            player_sav_cache: HashMap::new(),
            player_summaries,
            guild_summaries,
            character_index: HashMap::new(),
            item_container_index: HashMap::new(),
            character_container_index: HashMap::new(),
            group_index: HashMap::new(),
            guild_extra_index: HashMap::new(),
            gps_file_path: None,
            gps_loaded: false,
        }
    }

    #[tokio::test]
    async fn sync_app_state_with_save_emits_full_frame_sequence_in_order() {
        let mut test = TestContext::new(|_| {}).await;
        test.session.save = Some(fake_loaded_session());
        let mut ctx = HandlerCtx {
            session: &mut test.session,
            app: &test.app,
            emitter: &test.emitter,
        };
        handle_sync_app_state(&mut ctx).await.unwrap();

        assert_eq!(test.next_frame_json()["type"], "get_settings");

        let loaded = test.next_frame_json();
        assert_eq!(loaded["type"], "loaded_save_files");
        assert_eq!(loaded["data"]["level"], "C:/saves/world/Level.sav");
        assert_eq!(
            loaded["data"]["players"],
            serde_json::json!(["11111111-1111-1111-1111-111111111111"])
        );
        assert_eq!(
            loaded["data"]["guilds"],
            serde_json::json!(["22222222-2222-2222-2222-222222222222"])
        );
        assert_eq!(loaded["data"]["world_name"], "My World");
        assert_eq!(loaded["data"]["type"], "steam");
        assert_eq!(loaded["data"]["size"], 12345);
        assert_eq!(loaded["data"]["has_gps"], false);

        let player_summaries = test.next_frame_json();
        assert_eq!(player_summaries["type"], "get_player_summaries");
        assert_eq!(
            player_summaries["data"]["11111111-1111-1111-1111-111111111111"]["nickname"],
            "Tester"
        );

        let guild_summaries = test.next_frame_json();
        assert_eq!(guild_summaries["type"], "get_guild_summaries");
        assert_eq!(
            guild_summaries["data"]["22222222-2222-2222-2222-222222222222"]["name"],
            "The Guild"
        );

        test.assert_no_more_frames();
    }
}
