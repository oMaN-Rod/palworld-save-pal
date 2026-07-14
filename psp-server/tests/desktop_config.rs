mod common;

#[tokio::test]
async fn desktop_mode_server_starts_and_answers_get_version() {
    let server = common::start_desktop_test_server(std::sync::Arc::new(
        psp_server::desktop_dialogs::NullDialogProvider,
    ))
    .await;

    let mut socket = common::connect(&server).await;
    common::send_json(
        &mut socket,
        serde_json::json!({"type":"get_version","data":null}),
    )
    .await;

    let reply = common::next_json(&mut socket).await;
    assert_eq!(reply["type"], "get_version");
    assert_eq!(reply["data"], env!("CARGO_PKG_VERSION"));

    server.handle.shutdown().await;
}
