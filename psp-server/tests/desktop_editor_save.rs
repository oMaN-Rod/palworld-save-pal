//! Desktop-mode `save_edited_sav`: the JSON-editor Save button. The webview
//! ignores browser `<a download>`, so desktop saves the edited JSON back to a
//! native-picked `.sav`. Driven through a queued (fake) dialog provider.

mod common;

use psp_server::desktop_dialogs::QueuedDialogProvider;

/// A plain-GVAS save turned into the uesave JSON the editor holds in Monaco.
fn editor_json() -> String {
    let sav_bytes = std::fs::read(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/drg-save-test.sav"),
    )
    .expect("vendored tests/fixtures/drg-save-test.sav");
    psp_core::convert::sav_to_json_string(&sav_bytes).expect("sav decodes to json")
}

#[tokio::test]
async fn save_edited_sav_writes_sav_and_confirms() {
    let scratch = tempfile::tempdir().expect("tempdir");
    let out_path = scratch.path().join("edited.sav");
    let json = editor_json();

    let server = common::start_desktop_test_server(std::sync::Arc::new(
        QueuedDialogProvider::new_with_saves(vec![], vec![Some(out_path.clone())]),
    ))
    .await;
    let mut socket = common::connect(&server).await;

    common::send_json(
        &mut socket,
        serde_json::json!({"type": "save_edited_sav",
            "data": {"json": json, "file_name": "edited.sav"}}),
    )
    .await;

    let reply = common::next_json(&mut socket).await;
    assert_eq!(reply["type"], "save_edited_sav");
    assert_eq!(
        reply["data"]["file_path"],
        out_path.to_string_lossy().as_ref()
    );
    assert!(reply["data"]["message"].is_string());

    // The written .sav must decode back to the exact JSON the editor sent.
    let written = std::fs::read(&out_path).expect("edited .sav written");
    let round_trip = psp_core::convert::sav_to_json_string(&written).expect("written sav decodes");
    let sent: serde_json::Value = serde_json::from_str(&editor_json()).unwrap();
    let back: serde_json::Value = serde_json::from_str(&round_trip).unwrap();
    assert_eq!(sent, back);

    server.handle.shutdown().await;
}

#[tokio::test]
async fn save_edited_sav_canceled_answers_on_same_type() {
    let server = common::start_desktop_test_server(std::sync::Arc::new(
        QueuedDialogProvider::new_with_saves(vec![], vec![None]),
    ))
    .await;
    let mut socket = common::connect(&server).await;

    common::send_json(
        &mut socket,
        serde_json::json!({"type": "save_edited_sav",
            "data": {"json": editor_json(), "file_name": "edited.sav"}}),
    )
    .await;

    // Answers on its OWN type (not no_file_selected) so the editor's
    // sendAndWait, which correlates by message type, resolves quietly.
    let reply = common::next_json(&mut socket).await;
    assert_eq!(reply["type"], "save_edited_sav");
    assert_eq!(reply["data"]["canceled"], true);

    server.handle.shutdown().await;
}

#[tokio::test]
async fn save_edited_sav_invalid_json_answers_error_without_dialog() {
    // No save queued: invalid JSON must fail BEFORE the native dialog is ever
    // opened, so the user is not prompted for a location only to fail after.
    let server = common::start_desktop_test_server(std::sync::Arc::new(
        QueuedDialogProvider::new_with_saves(vec![], vec![]),
    ))
    .await;
    let mut socket = common::connect(&server).await;

    common::send_json(
        &mut socket,
        serde_json::json!({"type": "save_edited_sav",
            "data": {"json": "{ not valid save json", "file_name": "edited.sav"}}),
    )
    .await;

    let reply = common::next_json(&mut socket).await;
    assert_eq!(reply["type"], "save_edited_sav");
    assert!(reply["data"]["error"].is_string());

    server.handle.shutdown().await;
}
