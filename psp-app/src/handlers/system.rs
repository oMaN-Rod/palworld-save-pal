use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::handlers::save_file::emit_summary_messages;
use crate::handlers::settings::settings_dto_from_row;
use crate::messages::MessageType;

#[derive(Debug, serde::Serialize)]
struct SyncLoadedSaveFilesData {
    level: String,
    players: Vec<String>,
    guilds: Vec<String>,
    world_name: String,
    r#type: &'static str,
    size: u64,
    has_gps: bool,
    /// The single fact the WorldOption button gates on, across all three platforms.
    world_option_present: bool,
}

/// Frame order is the contract: `get_settings` first, then — only when a save
/// is loaded — `loaded_save_files` followed by both summary messages.
pub async fn handle_sync_app_state(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let row = psp_db::settings::get_settings(&*ctx.app.driver).await?;
    ctx.emitter
        .emit(MessageType::GetSettings, &settings_dto_from_row(row));

    let Some(session) = ctx.session.save.as_ref() else {
        tracing::warn!("no save file loaded");
        return Ok(());
    };

    // Must follow save-file (GVAS) order, recorded by `extract_summaries` into
    // `player_summary_order` / `guild_summary_order`. Reading the `BTreeMap`s'
    // `.keys()` instead would silently resort them to `Uuid` order.
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
        has_gps: session.gps_available(),
        world_option_present: session.world_option.is_some(),
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
        // Pinning `save_dir` to the real default, not merely `is_string()`, is
        // what catches a regression to `null`/an empty string.
        let mut test = TestContext::new(|_| {}).await;
        let mut ctx = HandlerCtx {
            session: &mut test.session,
            app: &test.app,
            emitter: &test.emitter,
            blueprints: &mut test.blueprints,
            attachment: None,
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

    /// The two players and two guilds are inserted in HIGH-then-LOW `Uuid`
    /// order — the opposite of `Uuid`'s `Ord`. That is what lets the test below
    /// discriminate: reading `player_summaries.keys()` instead of
    /// `player_summary_order` would emit them LOW-then-HIGH and fail.
    fn fake_loaded_session() -> psp_core::session::SaveSession {
        use psp_core::dto::summary::{GuildSummary, PlayerSummary};
        use psp_core::session::{SaveKind, SaveSession};
        use std::collections::BTreeMap;

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

        let level = psp_core::ue::Save {
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
                properties: psp_core::ue::Properties::default(),
            },
            extra: Vec::new(),
        };

        let mut session = SaveSession::new_for_tests(SaveKind::InMemory, level);
        session.world_name = "My World".to_string();
        session.save_id = "C:/saves/world/Level.sav".to_string();
        session.size = 12345;
        session.player_summaries = player_summaries;
        session.guild_summaries = guild_summaries;
        session.player_summary_order = vec![high_player, low_player];
        session.guild_summary_order = vec![high_guild, low_guild];
        session
    }

    #[tokio::test]
    async fn sync_app_state_with_save_emits_full_frame_sequence_in_order() {
        let mut test = TestContext::new(|_| {}).await;
        test.session.save = Some(fake_loaded_session());
        let mut ctx = HandlerCtx {
            session: &mut test.session,
            app: &test.app,
            emitter: &test.emitter,
            blueprints: &mut test.blueprints,
            attachment: None,
        };
        handle_sync_app_state(&mut ctx).await.unwrap();

        assert_eq!(test.next_frame_json()["type"], "get_settings");

        let loaded = test.next_frame_json();
        assert_eq!(loaded["type"], "loaded_save_files");
        assert_eq!(loaded["data"]["level"], "C:/saves/world/Level.sav");
        // HIGH-then-LOW: `*_summary_order`, NOT the BTreeMaps' sorted order.
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
