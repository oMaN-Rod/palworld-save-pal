//! Desktop-mode `export_preset` / `import_preset`: the native-dialog branches,
//! driven through a queued (fake) dialog provider.

mod common;

use psp_server::desktop_dialogs::QueuedDialogProvider;

async fn add_inventory_preset(socket: &mut common::WsClient, name: &str) -> String {
    common::send_json(
        socket,
        serde_json::json!({"type": "add_preset", "data": {"name": name, "type": "inventory"}}),
    )
    .await;
    let added = common::next_json(socket).await;
    added["data"]["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn export_preset_writes_file_and_confirms() {
    let scratch = tempfile::tempdir().expect("tempdir");
    let export_path = scratch.path().join("Kit.json");

    let server = common::start_desktop_test_server(std::sync::Arc::new(
        QueuedDialogProvider::new_with_saves(vec![], vec![Some(export_path.clone())]),
    ))
    .await;
    let mut socket = common::connect(&server).await;

    let preset_id = add_inventory_preset(&mut socket, "Kit").await;

    common::send_json(
        &mut socket,
        serde_json::json!({"type": "export_preset",
            "data": {"preset_id": preset_id, "preset_type": "inventory", "preset_name": "Kit"}}),
    )
    .await;

    let reply = common::next_json(&mut socket).await;
    assert_eq!(reply["type"], "export_preset");
    assert_eq!(
        reply["data"]["file_path"],
        export_path.to_string_lossy().as_ref()
    );

    let written = std::fs::read_to_string(&export_path).expect("export file written");
    let preset: serde_json::Value = serde_json::from_str(&written).expect("valid json");
    assert_eq!(preset["name"], "Kit");
    assert_eq!(preset["type"], "inventory");

    server.handle.shutdown().await;
}

#[tokio::test]
async fn export_preset_canceled_emits_no_file_selected() {
    let server = common::start_desktop_test_server(std::sync::Arc::new(
        QueuedDialogProvider::new_with_saves(vec![], vec![None]),
    ))
    .await;
    let mut socket = common::connect(&server).await;

    let preset_id = add_inventory_preset(&mut socket, "Kit").await;

    common::send_json(
        &mut socket,
        serde_json::json!({"type": "export_preset",
            "data": {"preset_id": preset_id, "preset_type": "inventory", "preset_name": "Kit"}}),
    )
    .await;

    let reply = common::next_json(&mut socket).await;
    assert_eq!(reply["type"], "no_file_selected");

    server.handle.shutdown().await;
}

#[tokio::test]
async fn import_preset_strips_id_and_adds() {
    let scratch = tempfile::tempdir().expect("tempdir");
    let import_path = scratch.path().join("imported.json");
    std::fs::write(
        &import_path,
        serde_json::json!({"id": "old-fixed-id", "name": "Imported", "type": "inventory"})
            .to_string(),
    )
    .expect("write import fixture");

    let server = common::start_desktop_test_server(std::sync::Arc::new(
        QueuedDialogProvider::new(vec![Some(import_path.clone())]),
    ))
    .await;
    let mut socket = common::connect(&server).await;

    common::send_json(&mut socket, serde_json::json!({"type": "import_preset", "data": null})).await;

    let reply = common::next_json(&mut socket).await;
    assert_eq!(reply["type"], "import_preset");
    let new_id = reply["data"]["preset_id"].as_str().expect("preset_id string");
    assert_ne!(new_id, "old-fixed-id");
    assert_eq!(
        reply["data"]["file_path"],
        import_path.to_string_lossy().as_ref()
    );

    common::send_json(&mut socket, serde_json::json!({"type": "get_presets", "data": null})).await;
    let presets = common::next_json(&mut socket).await;
    let imported = &presets["data"][new_id];
    assert_eq!(imported["name"], "Imported");
    assert_eq!(imported["type"], "inventory");
    assert_eq!(imported["id"], new_id);

    server.handle.shutdown().await;
}

#[tokio::test]
async fn import_preset_canceled_emits_no_file_selected() {
    let server = common::start_desktop_test_server(std::sync::Arc::new(
        QueuedDialogProvider::new(vec![None]),
    ))
    .await;
    let mut socket = common::connect(&server).await;

    common::send_json(&mut socket, serde_json::json!({"type": "import_preset", "data": null})).await;

    let reply = common::next_json(&mut socket).await;
    assert_eq!(reply["type"], "no_file_selected");

    server.handle.shutdown().await;
}
