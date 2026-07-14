//! Desktop-mode `unlock_map`: the native-file-dialog branch of
//! `handle_unlock_map`, driven through a queued (fake) dialog provider.

mod common;

#[tokio::test]
async fn canceled_unlock_map_dialog_emits_no_file_selected() {
    let server = common::start_desktop_test_server(std::sync::Arc::new(
        psp_server::desktop_dialogs::QueuedDialogProvider::new(vec![None]),
    ))
    .await;

    let mut socket = common::connect(&server).await;
    common::send_json(
        &mut socket,
        serde_json::json!({"type":"unlock_map","data":{}}),
    )
    .await;

    let reply = common::next_json(&mut socket).await;
    assert_eq!(reply["type"], "no_file_selected");
    assert_eq!(reply["data"], "No file selected");

    server.handle.shutdown().await;
}

#[tokio::test]
async fn unlock_map_rejects_non_local_data_filename() {
    let scratch = tempfile::tempdir().expect("tempdir");
    let picked = scratch.path().join("Level.sav");
    std::fs::write(&picked, b"junk").expect("write file");

    let server = common::start_desktop_test_server(std::sync::Arc::new(
        psp_server::desktop_dialogs::QueuedDialogProvider::new(vec![Some(picked)]),
    ))
    .await;

    let mut socket = common::connect(&server).await;
    common::send_json(
        &mut socket,
        serde_json::json!({"type":"unlock_map","data":{}}),
    )
    .await;

    let reply = common::next_json(&mut socket).await;
    assert_eq!(reply["type"], "error");
    assert_eq!(
        reply["data"]["message"],
        "Selected file Level.sav does not match expected type for local_data save. Please select a valid save file."
    );

    server.handle.shutdown().await;
}
