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
        QueuedDialogProvider::new_with_pick_files(vec![Some(vec![import_path.clone()])]),
    ))
    .await;
    let mut socket = common::connect(&server).await;

    common::send_json(&mut socket, serde_json::json!({"type": "import_preset", "data": null})).await;

    let reply = common::next_json(&mut socket).await;
    assert_eq!(reply["type"], "import_preset");
    assert_eq!(reply["data"]["count"], 1);

    common::send_json(&mut socket, serde_json::json!({"type": "get_presets", "data": null})).await;
    let presets = common::next_json(&mut socket).await;
    let imported = presets["data"]
        .as_object()
        .unwrap()
        .values()
        .find(|p| p["name"] == "Imported")
        .expect("imported preset present");
    assert_eq!(imported["type"], "inventory");
    assert_ne!(imported["id"], "old-fixed-id");

    server.handle.shutdown().await;
}

#[tokio::test]
async fn import_preset_reads_zip_and_json_array() {
    use std::io::Write;
    let scratch = tempfile::tempdir().expect("tempdir");

    // A zip with two preset entries.
    let zip_path = scratch.path().join("bundle.zip");
    {
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("Zed.json", options).unwrap();
        writer
            .write_all(
                serde_json::json!({"name": "Zed", "type": "inventory"})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap();
        writer.start_file("Yan.json", options).unwrap();
        writer
            .write_all(
                serde_json::json!({"name": "Yan", "type": "inventory"})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap();
        writer.finish().unwrap();
    }

    // A json file holding an array of two presets.
    let array_path = scratch.path().join("array.json");
    std::fs::write(
        &array_path,
        serde_json::json!([
            {"name": "Arr1", "type": "inventory"},
            {"name": "Arr2", "type": "inventory"},
        ])
        .to_string(),
    )
    .unwrap();

    let server = common::start_desktop_test_server(std::sync::Arc::new(
        QueuedDialogProvider::new_with_pick_files(vec![Some(vec![zip_path, array_path])]),
    ))
    .await;
    let mut socket = common::connect(&server).await;

    common::send_json(&mut socket, serde_json::json!({"type": "import_preset", "data": null})).await;
    let reply = common::next_json(&mut socket).await;
    assert_eq!(reply["type"], "import_preset");
    assert_eq!(reply["data"]["count"], 4);

    common::send_json(&mut socket, serde_json::json!({"type": "get_presets", "data": null})).await;
    let presets = common::next_json(&mut socket).await;
    assert_eq!(presets["data"].as_object().unwrap().len(), 4);

    server.handle.shutdown().await;
}

#[tokio::test]
async fn import_preset_canceled_emits_no_file_selected() {
    let server = common::start_desktop_test_server(std::sync::Arc::new(
        QueuedDialogProvider::new_with_pick_files(vec![None]),
    ))
    .await;
    let mut socket = common::connect(&server).await;

    common::send_json(&mut socket, serde_json::json!({"type": "import_preset", "data": null})).await;

    let reply = common::next_json(&mut socket).await;
    assert_eq!(reply["type"], "no_file_selected");

    server.handle.shutdown().await;
}
