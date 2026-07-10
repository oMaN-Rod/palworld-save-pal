//! Task 3B-2: preset CRUD WS integration test — drives get/add/update/delete/
//! nuke over a live WebSocket against the real dispatcher + psp-db presets
//! module (Task 3B-1), matching wire shapes from `preset_handler.py`.

mod common;

#[tokio::test]
async fn preset_crud_over_websocket() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;

    // get_presets seeds from data/json/presets.json when empty (preset_handler.py:41)
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
