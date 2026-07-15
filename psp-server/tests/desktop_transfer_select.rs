//! Desktop-mode `load_source_save` with the "__select__" sentinel: the
//! native-file-dialog branch of `handle_load_source_save`, driven through a
//! queued (fake) dialog provider. In web mode the same sentinel answers
//! "Desktop mode required for file selection." (see `transfer_ws.rs`); here the
//! dialog is actually available, so the handler must drive it instead.

mod common;

/// A canceled dialog must answer under the `load_source_save` type (so the
/// transfer UI's `sendAndWait`, which correlates by message type, resolves)
/// with a `canceled` flag rather than the web-mode "Desktop mode required"
/// error.
#[tokio::test]
async fn canceled_dialog_answers_canceled_not_desktop_required() {
    let server = common::start_desktop_test_server(std::sync::Arc::new(
        psp_server::desktop_dialogs::QueuedDialogProvider::new(vec![None]),
    ))
    .await;

    let mut socket = common::connect(&server).await;
    common::send_json(
        &mut socket,
        serde_json::json!({"type": "load_source_save",
            "data": {"type": "steam", "path": "__select__", "role": "source"}}),
    )
    .await;

    let reply = common::next_json(&mut socket).await;
    assert_eq!(reply["type"], "load_source_save");
    assert_eq!(reply["data"]["canceled"], true);
    assert!(reply["data"]["error"].is_null());

    server.handle.shutdown().await;
}

/// A pick that isn't a `Level.sav` is rejected with the same validation message
/// `select_save` uses, as a soft `load_source_save` error (never a hard `error`
/// frame).
#[tokio::test]
async fn wrong_filename_answers_soft_validation_error() {
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
        serde_json::json!({"type": "load_source_save",
            "data": {"type": "steam", "path": "__select__", "role": "source"}}),
    )
    .await;

    let reply = common::next_json(&mut socket).await;
    assert_eq!(reply["type"], "load_source_save");
    assert_eq!(
        reply["data"]["error"],
        "Selected file LevelMeta.sav does not match expected type for steam save. Please select a valid save file."
    );

    server.handle.shutdown().await;
}
