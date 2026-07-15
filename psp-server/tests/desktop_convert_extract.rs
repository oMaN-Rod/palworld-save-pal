//! Desktop-mode `convert_save_format` paths that write a NEW Steam tree via a
//! native OUTPUT-DIRECTORY dialog (`pick_folder`):
//!   1. a named GamePass save extracted to Steam (`save_id` + "steam"), and
//!   2. the loaded (GamePass) save converted to Steam (`convert_loaded_save`).
//! Both answered "Desktop mode required." before the dialog was wired. Driven
//! through a queued (fake) dialog provider; the synthetic wgs tree mirrors
//! `phase4_ws.rs`.

mod common;

use std::path::PathBuf;
use std::sync::Arc;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// Builds a one-save synthetic wgs tree from the committed world1 fixture and
/// returns `(gamepass_root_tempdir, container_dir, save_id)`.
fn build_gamepass_fixture() -> (tempfile::TempDir, PathBuf, String) {
    let world1 = repo_root().join("tests/fixtures/saves/world1");
    let level_bytes = std::fs::read(world1.join("Level.sav")).unwrap();
    let meta_bytes = std::fs::read(world1.join("LevelMeta.sav")).unwrap();
    let player_uuid = uuid::Uuid::parse_str("43797F87000000000000000000000000").unwrap();
    let player_bytes =
        std::fs::read(world1.join("Players/43797F87000000000000000000000000.sav")).unwrap();

    let gamepass_root = tempfile::tempdir().unwrap();
    let save_id = "FEED0000FEED0000FEED0000FEED0000".to_string();
    let save = psp_core::gamepass::fixture::SyntheticSave {
        save_id: save_id.clone(),
        level_sav: level_bytes,
        level_meta: Some(meta_bytes),
        local_data: None,
        world_option: None,
        players: vec![psp_core::gamepass::fixture::SyntheticPlayer {
            id: player_uuid,
            sav: player_bytes,
            dps: None,
        }],
    };
    let container_dir =
        psp_core::gamepass::fixture::build_wgs_tree(gamepass_root.path(), &[save]).unwrap();
    (gamepass_root, container_dir, save_id)
}

async fn next_convert_frame(socket: &mut common::WsClient) -> serde_json::Value {
    loop {
        let reply = common::next_json(socket).await;
        if reply["type"] == "convert_save_format" {
            return reply;
        }
    }
}

/// A canceled output-dir dialog answers `canceled` rather than the old
/// "Desktop mode required." error -- and short-circuits before touching the
/// install, so no fixture is needed.
#[tokio::test]
async fn named_extract_canceled_dialog_answers_canceled() {
    let server = common::start_desktop_test_server(Arc::new(
        psp_server::desktop_dialogs::QueuedDialogProvider::new_with_folders(vec![None]),
    ))
    .await;

    let mut socket = common::connect(&server).await;
    common::send_json(
        &mut socket,
        serde_json::json!({"type": "convert_save_format",
            "data": {"target_format": "steam", "save_id": "FEED0000FEED0000FEED0000FEED0000"}}),
    )
    .await;

    let reply = next_convert_frame(&mut socket).await;
    assert_eq!(reply["data"]["canceled"], true);
    assert!(reply["data"]["error"].is_null());

    server.handle.shutdown().await;
}

/// A named GamePass save extracts to the picked output directory as a real
/// Steam tree under `<output>/<save_id>/`.
#[tokio::test]
async fn named_extract_writes_steam_tree_to_picked_dir() {
    let (gamepass_root, _container_dir, save_id) = build_gamepass_fixture();
    let steam_out = gamepass_root.path().join("steam-out");

    let _env = common::GamepassEnvGuard::acquire(&[
        (
            "PSP_GAMEPASS_PACKAGES_ROOT",
            gamepass_root.path().to_path_buf(),
        ),
        ("PSP_BACKUPS_ROOT", gamepass_root.path().join("backups")),
    ])
    .await;

    let server = common::start_desktop_test_server(Arc::new(
        psp_server::desktop_dialogs::QueuedDialogProvider::new_with_folders(vec![Some(
            steam_out.clone(),
        )]),
    ))
    .await;

    let mut socket = common::connect(&server).await;
    common::send_json(
        &mut socket,
        serde_json::json!({"type": "convert_save_format",
            "data": {"target_format": "steam", "save_id": save_id}}),
    )
    .await;

    let reply = next_convert_frame(&mut socket).await;
    assert!(
        reply["data"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("extracted to Steam format"),
        "unexpected reply: {reply}"
    );
    assert!(
        steam_out.join(&save_id).join("Level.sav").exists(),
        "extracted Level.sav missing under {}",
        steam_out.join(&save_id).display()
    );

    server.handle.shutdown().await;
}

/// The loaded GamePass save is converted to Steam by writing a full Steam tree
/// (Level.sav + LevelMeta.sav + Players/*.sav) into the picked directory. The
/// load itself runs through the same queued dialog (a `pick_file` for the
/// container index), then the convert draws the `pick_folder` answer.
#[tokio::test]
async fn loaded_gamepass_save_converts_to_steam_dir() {
    let (gamepass_root, container_dir, save_id) = build_gamepass_fixture();
    let steam_out = gamepass_root.path().join("steam-out");

    let _env = common::GamepassEnvGuard::acquire(&[
        (
            "PSP_GAMEPASS_PACKAGES_ROOT",
            gamepass_root.path().to_path_buf(),
        ),
        ("PSP_BACKUPS_ROOT", gamepass_root.path().join("backups")),
    ])
    .await;

    let server = common::start_desktop_test_server(Arc::new(
        psp_server::desktop_dialogs::QueuedDialogProvider::new_with_all(
            vec![Some(container_dir.join("containers.index"))],
            Vec::new(),
            vec![Some(steam_out.clone())],
        ),
    ))
    .await;

    let mut socket = common::connect(&server).await;

    // Load the gamepass save: select_save picks the container index (populating
    // the save cache), then select_gamepass_save loads the named save.
    common::send_json(
        &mut socket,
        serde_json::json!({"type": "select_save", "data": {"type": "gamepass", "local": false}}),
    )
    .await;
    loop {
        if common::next_json(&mut socket).await["type"] == "select_gamepass_save" {
            break;
        }
    }
    common::send_json(
        &mut socket,
        serde_json::json!({"type": "select_gamepass_save", "data": save_id}),
    )
    .await;
    loop {
        if common::next_json(&mut socket).await["type"] == "get_guild_summaries" {
            break;
        }
    }

    // Convert the loaded (gamepass) save to steam -> picks the output folder.
    common::send_json(
        &mut socket,
        serde_json::json!({"type": "convert_save_format", "data": {"target_format": "steam"}}),
    )
    .await;

    let reply = next_convert_frame(&mut socket).await;
    assert!(
        reply["data"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("converted to Steam format"),
        "unexpected reply: {reply}"
    );
    assert!(steam_out.join("Level.sav").exists(), "Level.sav missing");
    assert!(
        steam_out.join("LevelMeta.sav").exists(),
        "LevelMeta.sav missing"
    );
    assert!(
        steam_out.join("Players").is_dir(),
        "Players dir missing"
    );

    server.handle.shutdown().await;
}
