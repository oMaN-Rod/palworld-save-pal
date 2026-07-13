//! Desktop-mode `open_folder`. Only the "folder type not resolved" branch is
//! exercised: it never calls `opener::open`, so the test stays headless-safe.

mod common;

#[tokio::test]
async fn unknown_folder_type_emits_warning() {
    let server = common::start_desktop_test_server(std::sync::Arc::new(
        psp_server::desktop_dialogs::NullDialogProvider,
    ))
    .await;

    let mut socket = common::connect(&server).await;
    common::send_json(
        &mut socket,
        serde_json::json!({"type":"open_folder","data":{"folder_type":"bogus"}}),
    )
    .await;

    let reply = common::next_json(&mut socket).await;
    assert_eq!(reply["type"], "warning");
    assert_eq!(reply["data"], "Folder not found: bogus");

    server.handle.shutdown().await;
}
