//! Desktop-mode `convert_save_format` with the "__select__" sentinel on
//! `source_path`/`output_path`: the native-file-dialog branch of
//! `handle_convert_save_format` (the Steam -> GamePass import the tools UI's
//! `handleSteamToGamepass` drives), run through a queued (fake) dialog provider.
//! In web mode the same sentinel answers "No file selected." (see `phase4_ws.rs`).

mod common;

use std::sync::Arc;

/// A canceled source dialog answers `convert_save_format` with a `canceled`
/// flag rather than the web-mode "No file selected." error, so the tools UI's
/// `sendAndWait` (which correlates by message type) resolves quietly.
#[tokio::test]
async fn canceled_source_dialog_answers_canceled() {
    let server = common::start_desktop_test_server(Arc::new(
        psp_server::desktop_dialogs::QueuedDialogProvider::new(vec![None]),
    ))
    .await;

    let mut socket = common::connect(&server).await;
    common::send_json(
        &mut socket,
        serde_json::json!({"type": "convert_save_format",
            "data": {"target_format": "gamepass", "source_path": "__select__", "output_path": "__select__"}}),
    )
    .await;

    let reply = common::next_json(&mut socket).await;
    assert_eq!(reply["type"], "convert_save_format");
    assert_eq!(reply["data"]["canceled"], true);
    assert!(reply["data"]["error"].is_null());

    server.handle.shutdown().await;
}

/// A source pick that isn't a `Level.sav` is rejected with the shared
/// validation message, as a soft `convert_save_format` error.
#[tokio::test]
async fn wrong_source_filename_answers_validation_error() {
    let scratch = tempfile::tempdir().expect("tempdir");
    let picked = scratch.path().join("LevelMeta.sav");
    std::fs::write(&picked, b"junk").expect("write file");

    let server = common::start_desktop_test_server(Arc::new(
        psp_server::desktop_dialogs::QueuedDialogProvider::new(vec![Some(picked)]),
    ))
    .await;

    let mut socket = common::connect(&server).await;
    common::send_json(
        &mut socket,
        serde_json::json!({"type": "convert_save_format",
            "data": {"target_format": "gamepass", "source_path": "__select__", "output_path": "__select__"}}),
    )
    .await;

    let reply = common::next_json(&mut socket).await;
    assert_eq!(reply["type"], "convert_save_format");
    assert_eq!(
        reply["data"]["error"],
        "Selected file LevelMeta.sav does not match expected type for steam save. Please select a valid save file."
    );

    server.handle.shutdown().await;
}

/// Both dialogs resolve (Steam `Level.sav` source, GamePass `containers.index`
/// output), so the handler proceeds into the conversion. The junk index makes
/// that step fail with its own soft error -- proof the __select__ pair was
/// resolved to real paths rather than short-circuiting on "No file selected.".
#[tokio::test]
async fn both_dialogs_resolve_and_reach_conversion() {
    let scratch = tempfile::tempdir().expect("tempdir");
    let steam_dir = scratch.path().join("steamsave");
    std::fs::create_dir_all(&steam_dir).expect("mkdir");
    let level_sav = steam_dir.join("Level.sav");
    std::fs::write(&level_sav, b"not a real sav").expect("write level");

    let gp_dir = scratch.path().join("gp");
    std::fs::create_dir_all(&gp_dir).expect("mkdir");
    let index = gp_dir.join("containers.index");
    std::fs::write(&index, b"junk index").expect("write index");

    let server = common::start_desktop_test_server(Arc::new(
        psp_server::desktop_dialogs::QueuedDialogProvider::new(vec![Some(level_sav), Some(index)]),
    ))
    .await;

    let mut socket = common::connect(&server).await;
    common::send_json(
        &mut socket,
        serde_json::json!({"type": "convert_save_format",
            "data": {"target_format": "gamepass", "source_path": "__select__", "output_path": "__select__"}}),
    )
    .await;

    // Drain any progress frames until the terminal convert_save_format payload.
    loop {
        let reply = common::next_json(&mut socket).await;
        if reply["type"] == "convert_save_format" {
            let error = reply["data"]["error"].as_str().unwrap_or_default();
            assert!(
                error.contains("Could not read GamePass container index"),
                "expected downstream index-read error, got: {reply}"
            );
            break;
        }
    }

    server.handle.shutdown().await;
}
