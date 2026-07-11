//! Task 3C-6: the three UPS <-> save-session interop handlers
//! (clone_to_ups / import_to_ups / export_ups_pal) all require a loaded save.
//! With no save loaded, each emits the `error` `{"message": "No save file
//! loaded"}` frame (ups_handler.py:262-267,399-403,553-557). The full
//! save-loaded paths are corpus/parity territory, not unit-tested here.

mod common;

#[tokio::test]
async fn session_dependent_ups_handlers_require_a_loaded_save() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "clone_to_ups", "data": {
            "pal_ids": ["11111111-1111-1111-1111-111111111111"],
            "source_type": "pal_box", "source_player_uid": null,
            "collection_id": null, "tags": null, "notes": null}}),
    )
    .await;
    let clone_response = common::next_json(&mut ws).await;
    assert_eq!(clone_response["type"], "error");
    assert_eq!(clone_response["data"]["message"], "No save file loaded");

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "import_to_ups", "data": {
            "source_type": "pal_box", "source_pal_id": null, "source_slot": null,
            "source_player_uid": null, "collection_id": null, "tags": null, "notes": null}}),
    )
    .await;
    let import_response = common::next_json(&mut ws).await;
    assert_eq!(import_response["type"], "error");
    assert_eq!(import_response["data"]["message"], "No save file loaded");

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "export_ups_pal", "data": {
            "pal_id": 1, "destination_type": "pal_box",
            "destination_player_uid": null, "destination_slot": null}}),
    )
    .await;
    let export_response = common::next_json(&mut ws).await;
    assert_eq!(export_response["type"], "error");
    assert_eq!(export_response["data"]["message"], "No save file loaded");

    server.handle.shutdown().await;
}
