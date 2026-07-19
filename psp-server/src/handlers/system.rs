use std::path::{Path, PathBuf};

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
    let row = psp_db::settings::get_settings(&ctx.app.db).await?;
    ctx.emitter
        .emit(MessageType::GetSettings, &settings_dto_from_row(row));

    let Some(session) = ctx.session.save.as_ref() else {
        tracing::warn!("no save file loaded");
        return Ok(());
    };

    // The `players`/`guilds` arrays must follow save-file (GVAS) order, which
    // `extract_summaries` records into `player_summary_order` /
    // `guild_summary_order`. Reading the `BTreeMap`s' `.keys()` instead would
    // silently resort them to `Uuid` order.
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

#[derive(Debug, serde::Deserialize)]
pub struct OpenFolderData {
    pub folder_type: String,
}

/// `app_root` is the writable base dir — the desktop shell exports `PSP_APP_ROOT`
/// pointing at a per-user dir, which is where `backups/` is written.
pub fn folder_path_for(folder_type: &str, app_root: &Path) -> Option<PathBuf> {
    match folder_type {
        "backups" => Some(app_root.join("backups")),
        "steam" => Some(crate::desktop_dialogs::steam_save_root()),
        "gamepass" => Some(crate::desktop_dialogs::gamepass_save_root()),
        "psp_root" => Some(app_root.to_path_buf()),
        _ => None,
    }
}

pub fn browser_url_from(host_and_port: &str) -> String {
    let (host, port) = match host_and_port.rsplit_once(':') {
        Some((host, port)) => (host, port),
        None => (host_and_port, ""),
    };
    let host = if host == "127.0.0.1" {
        "localhost"
    } else {
        host
    };
    format!("http://{host}:{port}")
}

/// Opens with NO response frame on success; a missing folder answers `warning`.
pub async fn handle_open_folder(
    data: OpenFolderData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if !ctx.app.config.desktop_mode {
        // `open_folder` is a desktop-only message: stay silent rather than
        // emit a frame a web client would never see the button for.
        return Ok(());
    }
    let app_root = psp_core::paths::app_root();
    let resolved = folder_path_for(&data.folder_type, &app_root);
    match resolved {
        Some(folder_path) if folder_path.exists() => {
            opener::open(&folder_path).map_err(|open_error| {
                HandlerError::Other(format!(
                    "Failed to open folder {}: {open_error}",
                    folder_path.display()
                ))
            })?;
        }
        Some(folder_path) => {
            ctx.emitter.emit(
                MessageType::Warning,
                &format!("Folder not found: {}", folder_path.display()),
            );
        }
        None => {
            ctx.emitter.emit(
                MessageType::Warning,
                &format!("Folder not found: {}", data.folder_type),
            );
        }
    }
    Ok(())
}

/// Active in BOTH desktop and web mode, unlike `open_folder`.
pub async fn handle_open_in_browser(
    data: String,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let url = browser_url_from(&data);
    opener::open(&url).map_err(|open_error| {
        HandlerError::Other(format!("Failed to open browser: {open_error}"))
    })?;
    ctx.emitter
        .emit(MessageType::OpenInBrowser, &"Browser opened successfully");
    Ok(())
}

/// Only http(s) URLs may be handed to `opener`; anything else (a `file://`
/// path, a `javascript:` payload, an arbitrary scheme) is refused so a WS
/// message can't coax the host into launching an unexpected handler.
fn is_openable_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

/// Opens an external URL in the OS default browser. The Tauri webview drops
/// `<a target="_blank">` navigations, so desktop links route here instead;
/// `opener::open` hands the URL to the host, escaping the webview.
pub async fn handle_open_url(
    data: String,
    _ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let url = data.trim();
    if !is_openable_url(url) {
        return Err(HandlerError::Other(format!(
            "Refusing to open non-http(s) URL: {url}"
        )));
    }
    opener::open(url)
        .map_err(|open_error| HandlerError::Other(format!("Failed to open URL {url}: {open_error}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestContext;

    #[test]
    fn is_openable_url_accepts_only_http_schemes() {
        assert!(is_openable_url("http://localhost:5173"));
        assert!(is_openable_url("https://github.com/oMaN-Rod/palworld-save-pal"));
        assert!(is_openable_url("https://buymeacoffee.com/i_am_o"));

        assert!(!is_openable_url("file:///etc/passwd"));
        assert!(!is_openable_url("javascript:alert(1)"));
        assert!(!is_openable_url("ftp://example.com"));
        assert!(!is_openable_url("github.com"));
        assert!(!is_openable_url(""));
    }

    #[tokio::test]
    async fn handle_open_url_rejects_non_http_scheme() {
        let mut test = TestContext::new(|_| {}).await;
        let mut ctx = HandlerCtx {
            session: &mut test.session,
            app: &test.app,
            emitter: &test.emitter,
            attachment: None,
        };
        let result = handle_open_url("file:///etc/passwd".to_string(), &mut ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn sync_app_state_without_save_emits_only_settings() {
        // sync_app_state is the ONLY path by which settings reach the UI during
        // bootstrap, so assert the full six-field payload. Pinning `save_dir` to
        // the real default (rather than merely `is_string()`) is what catches a
        // regression to `null`/an empty string.
        let mut test = TestContext::new(|_| {}).await;
        let mut ctx = HandlerCtx {
            session: &mut test.session,
            app: &test.app,
            emitter: &test.emitter,
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

    /// A `SaveSession` with only the fields `handle_sync_app_state` reads
    /// populated; everything else is an empty placeholder.
    ///
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

#[cfg(test)]
mod desktop_system_tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn folder_path_resolves_all_four_python_folder_types() {
        let app_root = Path::new("/opt/psp-data");
        assert_eq!(
            folder_path_for("backups", app_root),
            Some(app_root.join("backups"))
        );
        assert_eq!(
            folder_path_for("steam", app_root),
            Some(crate::desktop_dialogs::steam_save_root())
        );
        assert_eq!(
            folder_path_for("gamepass", app_root),
            Some(crate::desktop_dialogs::gamepass_save_root())
        );
        assert_eq!(
            folder_path_for("psp_root", app_root),
            Some(app_root.to_path_buf())
        );
        assert_eq!(folder_path_for("bogus", app_root), None);
    }

    #[test]
    fn browser_url_maps_loopback_to_localhost() {
        assert_eq!(browser_url_from("127.0.0.1:5174"), "http://localhost:5174");
        assert_eq!(browser_url_from("myhost:8080"), "http://myhost:8080");
    }
}
