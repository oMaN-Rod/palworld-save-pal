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

    // Like select_save's/load_zip_file's `players` array (filesystem/zip
    // *discovery* order, see save_file.rs), these two fields must follow
    // Python's actual dict-insertion order -- NOT the BTreeMap's sorted
    // order. Python's `sync_app_state_handler` emits
    // `[str(p) for p in app_state.player_summaries.keys()]` /
    // `.guild_summaries.keys()`, and both dicts preserve GVAS-file
    // insertion order:
    //   - `player_summaries` is built by `_extract_player_summaries`, which
    //     dispatches to `_extract_players_parallel` (a `ThreadPoolExecutor`
    //     whose results land via `as_completed()`, genuinely
    //     nondeterministic) only when the save has MORE than two players. At
    //     two players or fewer it takes `_extract_players_sequential`
    //     instead, which inserts in `players_data` order --
    //     `CharacterSaveParameterMap` iteration order -- deterministically.
    //     This is exactly the threshold `rust/parity/README.md`'s load_path
    //     corpus rule ("at most 2 players") exists to stay under.
    //   - `guild_summaries` (`_extract_guild_summaries`) is ALWAYS
    //     sequential -- no thread pool involved at any size -- so its order
    //     is unconditionally `GroupSaveDataMap` iteration order, filtered to
    //     guild-type entries with a non-nil guild id.
    // `psp_core::domain::summaries::extract_summaries` walks both maps in
    // that same save-file order and records it verbatim into
    // `session.player_summary_order` / `session.guild_summary_order`
    // alongside the (still `Uuid`-sorted) `BTreeMap`s -- see that function's
    // and `SaveSession`'s own doc comments. Reading `.keys()` here instead
    // would silently resort to `Uuid` order whenever GVAS order isn't
    // already ascending.
    let payload = SyncLoadedSaveFilesData {
        level: session.save_id.clone(),
        players: session
            .player_summary_order
            .iter()
            .map(|uid| uid.to_string())
            .collect(),
        guilds: session
            .guild_summary_order
            .iter()
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
    ///
    /// TWO players and TWO guilds, deliberately inserted (both into the
    /// `BTreeMap`s and into `player_summary_order`/`guild_summary_order`) in
    /// HIGH-then-LOW `Uuid` order -- the opposite of `Uuid`'s `Ord` impl.
    /// This is what lets `sync_app_state_with_save_emits_full_frame_sequence_in_order`
    /// below actually discriminate the fix: if `handle_sync_app_state` ever
    /// regressed to `session.player_summaries.keys()` /
    /// `.guild_summaries.keys()` (sorted order) instead of reading the
    /// `*_order` fields, the emitted `players`/`guilds` arrays would come
    /// back LOW-then-HIGH and the test would fail.
    fn fake_loaded_session() -> psp_core::session::SaveSession {
        use psp_core::dto::summary::{GuildSummary, PlayerSummary};
        use psp_core::session::{SaveKind, SaveSession};
        use std::collections::{BTreeMap, HashMap};

        let low_player: uuid::Uuid = "11111111-1111-1111-1111-111111111111".parse().unwrap();
        let high_player: uuid::Uuid = "ffffffff-ffff-ffff-ffff-ffffffffffff".parse().unwrap();
        let mut player_summaries = BTreeMap::new();
        player_summaries.insert(
            low_player,
            PlayerSummary {
                uid: low_player,
                nickname: "Tester".to_string(),
                level: Some(9),
                guild_id: None,
                pal_count: 0,
                last_online_time: None,
                loaded: false,
            },
        );
        player_summaries.insert(
            high_player,
            PlayerSummary {
                uid: high_player,
                nickname: "High".to_string(),
                level: Some(3),
                guild_id: None,
                pal_count: 0,
                last_online_time: None,
                loaded: false,
            },
        );

        let low_guild: uuid::Uuid = "22222222-2222-2222-2222-222222222222".parse().unwrap();
        let high_guild: uuid::Uuid = "eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee".parse().unwrap();
        let mut guild_summaries = BTreeMap::new();
        guild_summaries.insert(
            low_guild,
            GuildSummary {
                id: low_guild,
                name: "The Guild".to_string(),
                admin_player_uid: Some(low_player),
                player_count: 1,
                base_count: 0,
                level: Some(1),
                pal_count: 0,
                loaded: false,
            },
        );
        guild_summaries.insert(
            high_guild,
            GuildSummary {
                id: high_guild,
                name: "High Guild".to_string(),
                admin_player_uid: Some(high_player),
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
            player_summary_order: vec![high_player, low_player],
            guild_summary_order: vec![high_guild, low_guild],
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
        // HIGH-then-LOW: `player_summary_order`/`guild_summary_order`, NOT
        // the BTreeMaps' sorted (LOW-then-HIGH) iteration order. This is the
        // assertion that actually discriminates the fix -- see
        // `fake_loaded_session`'s doc comment.
        assert_eq!(
            loaded["data"]["players"],
            serde_json::json!([
                "ffffffff-ffff-ffff-ffff-ffffffffffff",
                "11111111-1111-1111-1111-111111111111"
            ])
        );
        assert_eq!(
            loaded["data"]["guilds"],
            serde_json::json!([
                "eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee",
                "22222222-2222-2222-2222-222222222222"
            ])
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
        assert_eq!(
            player_summaries["data"]["ffffffff-ffff-ffff-ffff-ffffffffffff"]["nickname"],
            "High"
        );

        let guild_summaries = test.next_frame_json();
        assert_eq!(guild_summaries["type"], "get_guild_summaries");
        assert_eq!(
            guild_summaries["data"]["22222222-2222-2222-2222-222222222222"]["name"],
            "The Guild"
        );
        assert_eq!(
            guild_summaries["data"]["eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee"]["name"],
            "High Guild"
        );

        test.assert_no_more_frames();
    }
}
