//! Desktop-mode `select_save`: the native-file-dialog branch of
//! `handle_select_save`, driven through a queued (fake) dialog provider.

mod common;

#[tokio::test]
async fn canceled_dialog_emits_no_file_selected() {
    let server = common::start_desktop_test_server(std::sync::Arc::new(
        psp_server::desktop_dialogs::QueuedDialogProvider::new(vec![None]),
    ))
    .await;

    let mut socket = common::connect(&server).await;
    common::send_json(
        &mut socket,
        serde_json::json!({"type":"select_save","data":{"type":"steam","local":true}}),
    )
    .await;

    let reply = common::next_json(&mut socket).await;
    assert_eq!(reply["type"], "no_file_selected");
    assert_eq!(reply["data"], "No file selected");

    server.handle.shutdown().await;
}

#[tokio::test]
async fn wrong_filename_emits_error_with_exact_message() {
    let scratch = tempfile::tempdir().expect("tempdir");
    let picked = scratch.path().join("LevelMeta.sav");
    std::fs::write(&picked, b"junk").expect("write file");

    let server = common::start_desktop_test_server(std::sync::Arc::new(
        psp_server::desktop_dialogs::QueuedDialogProvider::new(vec![Some(picked)]),
    ))
    .await;

    let mut socket = common::connect(&server).await;
    common::send_json(
        &mut socket,
        serde_json::json!({"type":"select_save","data":{"type":"steam","local":true}}),
    )
    .await;

    let reply = common::next_json(&mut socket).await;
    assert_eq!(reply["type"], "error");
    assert_eq!(
        reply["data"]["message"],
        "Selected file LevelMeta.sav does not match expected type for steam save. Please select a valid save file."
    );

    server.handle.shutdown().await;
}

#[tokio::test]
async fn valid_pick_persists_save_dir_before_loading() {
    let scratch = tempfile::tempdir().expect("tempdir");
    let save_dir = scratch.path().join("world");
    std::fs::create_dir_all(&save_dir).expect("mkdir");
    let level_sav = save_dir.join("Level.sav");
    std::fs::write(&level_sav, b"not a real sav").expect("write file");

    let server = common::start_desktop_test_server(std::sync::Arc::new(
        psp_server::desktop_dialogs::QueuedDialogProvider::new(vec![Some(level_sav)]),
    ))
    .await;
    let db_path = server._temp_dir.path().join("psp-rs.db");

    let mut socket = common::connect(&server).await;
    common::send_json(
        &mut socket,
        serde_json::json!({"type":"select_save","data":{"type":"steam","local":true}}),
    )
    .await;

    // The junk Level.sav fails to parse -> an `error` frame; drain progress
    // frames until it arrives. save_dir must already be persisted by then: it
    // is written BEFORE the load attempt, so a failed load still remembers the
    // directory the user picked.
    loop {
        let reply = common::next_json(&mut socket).await;
        if reply["type"] == "error" {
            break;
        }
    }

    let pool = psp_db::open(&db_path).await.expect("open db");
    let persisted = psp_db::settings::saved_save_dir(&pool).await.expect("read");
    assert_eq!(persisted, Some(save_dir.to_string_lossy().into_owned()));

    server.handle.shutdown().await;
}
