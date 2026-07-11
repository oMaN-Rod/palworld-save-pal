mod common;

#[tokio::test]
async fn request_gps_without_save_reports_error_in_gps_response() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    common::send_json(
        &mut ws,
        serde_json::json!({"type": "request_gps", "data": null}),
    )
    .await;
    let response = common::next_json(&mut ws).await;
    assert_eq!(response["type"], "get_gps_response");
    assert_eq!(response["data"]["error"], "No save file loaded");
    server.handle.shutdown().await;
}

#[tokio::test]
async fn add_and_delete_gps_pals_are_silent_without_save() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    common::send_json(
        &mut ws,
        serde_json::json!({"type": "add_gps_pal",
            "data": {"character_id": "SheepBall", "nickname": "X", "storage_slot": null}}),
    )
    .await;
    common::send_json(
        &mut ws,
        serde_json::json!({"type": "delete_gps_pals", "data": {"pal_indexes": [0]}}),
    )
    .await;
    // Neither produces a response; prove the socket is alive and ordered with a follow-up.
    common::send_json(
        &mut ws,
        serde_json::json!({"type": "get_ups_tags", "data": null}),
    )
    .await;
    let response = common::next_json(&mut ws).await;
    assert_eq!(response["type"], "get_ups_tags");
    server.handle.shutdown().await;
}

#[tokio::test]
async fn clone_gps_pal_to_player_without_save_errors() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    common::send_json(
        &mut ws,
        serde_json::json!({"type": "clone_gps_pal_to_player", "data": {
            "pal_ids": ["11111111-1111-1111-1111-111111111111"],
            "destination_type": "pal_box",
            "destination_player_uid": "55555555-5555-5555-5555-555555555555"}}),
    )
    .await;
    let response = common::next_json(&mut ws).await;
    assert_eq!(response["type"], "error");
    assert_eq!(response["data"]["message"], "No save file loaded");
    server.handle.shutdown().await;
}
