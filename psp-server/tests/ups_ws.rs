//! UPS pal CRUD over a live WebSocket: add/get/get_ids/update/clone/stats/
//! delete/nuke, against the real dispatcher and psp-db UPS module.

mod common;

fn sample_pal_dto() -> serde_json::Value {
    serde_json::json!({
        "instance_id": "11111111-1111-1111-1111-111111111111",
        "owner_uid": null, "character_id": "SheepBall", "is_lucky": false, "is_boss": false,
        "gender": "Female", "rank_hp": 1, "rank_attack": 2, "rank_defense": 3,
        "rank_craftspeed": 4, "talent_hp": 50, "talent_shot": 60, "talent_defense": 70,
        "rank": 1, "level": 12, "exp": 3450, "nickname": "Fluffy", "is_tower": false,
        "storage_id": "22222222-2222-2222-2222-222222222222", "stomach": 100.0,
        "storage_slot": 3, "learned_skills": [], "active_skills": [], "passive_skills": [],
        "hp": 5000, "max_hp": 5000, "group_id": null, "sanity": 100.0,
        "work_suitability": {"Handcraft": 1}, "is_sick": false, "friendship_point": 0
    })
}

#[tokio::test]
async fn ups_pal_lifecycle_over_websocket() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "add_ups_pal", "data": {
            "pal_dto": sample_pal_dto(),
            "source_save_file": "MyWorld",
            "source_player_uid": "55555555-5555-5555-5555-555555555555",
            "source_player_name": "Omar",
            "source_storage_type": "pal_box",
            "source_storage_slot": 3,
            "collection_id": null,
            "tags": ["shiny"],
            "notes": null
        }}),
    )
    .await;
    let added = common::next_json(&mut ws).await;
    assert_eq!(added["type"], "add_ups_pal");
    assert_eq!(added["data"]["character_id"], "SheepBall");
    assert_eq!(added["data"]["nickname"], "Fluffy");
    assert_eq!(added["data"]["level"], 12);
    let pal_id = added["data"]["id"].as_i64().unwrap();

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "get_ups_pals", "data": {
            "offset": 0, "limit": 30, "search_query": "fluf", "character_id_filter": null,
            "collection_id": null, "tags": null, "element_types": null, "pal_types": null,
            "sort_by": "created_at", "sort_order": "desc"
        }}),
    )
    .await;
    let page = common::next_json(&mut ws).await;
    assert_eq!(page["type"], "get_ups_pals");
    assert_eq!(page["data"]["total_count"], 1);
    assert_eq!(page["data"]["offset"], 0);
    assert_eq!(page["data"]["limit"], 30);
    assert_eq!(page["data"]["pals"][0]["character_key"], "sheepball");
    assert!(page["data"]["pals"][0]["pal_data"].is_object());

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "get_ups_all_filtered_ids", "data": {
            "search_query": null, "character_id_filter": null, "collection_id": null,
            "tags": ["shiny"], "element_types": null, "pal_types": null
        }}),
    )
    .await;
    let ids = common::next_json(&mut ws).await;
    assert_eq!(ids["data"]["pal_ids"], serde_json::json!([pal_id]));
    assert_eq!(ids["data"]["total_count"], 1);

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "update_ups_pal",
            "data": {"pal_id": pal_id, "updates": {"nickname": "Rex"}}}),
    )
    .await;
    let updated = common::next_json(&mut ws).await;
    assert_eq!(updated["data"]["pal"]["nickname"], "Rex");

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "update_ups_pal", "data": {"pal_id": 9999, "updates": {}}}),
    )
    .await;
    let not_found = common::next_json(&mut ws).await;
    assert_eq!(not_found["type"], "error");
    assert_eq!(
        not_found["data"]["message"],
        "UPS Pal with ID 9999 not found"
    );

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "clone_ups_pal", "data": {"pal_id": pal_id}}),
    )
    .await;
    let cloned = common::next_json(&mut ws).await;
    assert_eq!(cloned["data"]["original_pal_id"], pal_id);
    assert_eq!(cloned["data"]["cloned_pal"]["nickname"], "Rex (Clone)");

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "get_ups_stats", "data": null}),
    )
    .await;
    let stats = common::next_json(&mut ws).await;
    assert_eq!(stats["data"]["stats"]["total_pals"], 2);
    assert!(stats["data"]["stats"]["element_distribution"].is_string());

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "delete_ups_pals", "data": {"pal_ids": [pal_id, 9999]}}),
    )
    .await;
    let deleted = common::next_json(&mut ws).await;
    assert_eq!(deleted["data"]["deleted_count"], 1);
    assert_eq!(deleted["data"]["requested_count"], 2);

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "nuke_ups_pals", "data": null}),
    )
    .await;
    let nuked = common::next_json(&mut ws).await;
    assert_eq!(nuked["data"]["success"], true);
    assert_eq!(nuked["data"]["deleted_count"], 1);
    assert_eq!(
        nuked["data"]["message"],
        "Successfully deleted 1 pals from UPS"
    );

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "nuke_ups_pals", "data": null}),
    )
    .await;
    let empty_nuke = common::next_json(&mut ws).await;
    assert_eq!(empty_nuke["data"]["message"], "UPS is already empty");

    server.handle.shutdown().await;
}
