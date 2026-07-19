//! Preset CRUD over a live WebSocket: get/add/update/delete/nuke, plus the
//! export/import paths that need a desktop file dialog.

mod common;

#[tokio::test]
async fn preset_crud_over_websocket() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;

    // On an empty table, get_presets seeds itself from data/json/presets.json —
    // hence the non-zero baseline count the later assertions compare against.
    common::send_json(
        &mut ws,
        serde_json::json!({"type": "get_presets", "data": null}),
    )
    .await;
    let seeded = common::next_json(&mut ws).await;
    assert_eq!(seeded["type"], "get_presets");
    assert!(seeded["data"].is_object());
    let seeded_count = seeded["data"].as_object().unwrap().len();

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "add_preset", "data": {"name": "Kit", "type": "inventory",
            "common_container": [{"static_id": "Wood", "count": 999, "slot_index": 0}]}}),
    )
    .await;
    let added = common::next_json(&mut ws).await;
    assert_eq!(added["type"], "add_preset");
    assert_eq!(added["data"]["message"], "Preset added successfully");
    let new_id = added["data"]["id"].as_str().unwrap().to_string();

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "update_preset", "data": {"id": new_id, "name": "Kit2"}}),
    )
    .await;
    let updated = common::next_json(&mut ws).await;
    assert_eq!(updated["type"], "update_preset");
    assert_eq!(updated["data"], "Kit2 updated successfully");

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "delete_preset", "data": [new_id]}),
    )
    .await;
    let deleted = common::next_json(&mut ws).await;
    assert_eq!(deleted["type"], "delete_preset");
    assert_eq!(deleted["data"], "Presets deleted successfully");

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "get_presets", "data": null}),
    )
    .await;
    let after_delete = common::next_json(&mut ws).await;
    assert_eq!(
        after_delete["data"].as_object().unwrap().len(),
        seeded_count
    );

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "nuke_presets", "data": null}),
    )
    .await;
    let nuked = common::next_json(&mut ws).await;
    assert_eq!(nuked["type"], "nuke_presets");
    assert_eq!(nuked["data"], "Presets nuked successfully");

    server.handle.shutdown().await;
}

#[tokio::test]
async fn export_and_import_preset_require_desktop_dialog() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;

    // An unknown preset id is rejected BEFORE the dialog-availability check.
    common::send_json(
        &mut ws,
        serde_json::json!({"type": "export_preset",
            "data": {"preset_id": "missing", "preset_type": "inventory", "preset_name": "X"}}),
    )
    .await;
    let missing = common::next_json(&mut ws).await;
    assert_eq!(missing["type"], "error");
    assert_eq!(missing["data"], "Preset missing not found");

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "add_preset", "data": {"name": "Kit", "type": "inventory"}}),
    )
    .await;
    let added = common::next_json(&mut ws).await;
    let preset_id = added["data"]["id"].as_str().unwrap().to_string();

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "export_preset",
            "data": {"preset_id": preset_id, "preset_type": "inventory", "preset_name": "Kit"}}),
    )
    .await;
    let export_response = common::next_json(&mut ws).await;
    assert_eq!(export_response["type"], "error");
    assert_eq!(export_response["data"], "File dialog not available");

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "import_preset", "data": null}),
    )
    .await;
    let import_response = common::next_json(&mut ws).await;
    assert_eq!(import_response["type"], "error");
    assert_eq!(import_response["data"], "File dialog not available");

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "export_presets", "data": [
            {"preset_id": preset_id, "preset_type": "inventory", "preset_name": "Kit"}
        ]}),
    )
    .await;
    let bulk_export_response = common::next_json(&mut ws).await;
    assert_eq!(bulk_export_response["type"], "error");
    assert_eq!(bulk_export_response["data"], "File dialog not available");

    server.handle.shutdown().await;
}
