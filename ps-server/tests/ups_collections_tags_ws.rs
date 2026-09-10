mod common;

#[tokio::test]
async fn collections_and_tags_crud() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "create_ups_collection",
            "data": {"name": "Favs", "description": "best", "color": "#f00"}}),
    )
    .await;
    let created = common::next_json(&mut ws).await;
    assert_eq!(created["type"], "create_ups_collection");
    assert_eq!(created["data"]["collection"]["name"], "Favs");
    assert_eq!(created["data"]["collection"]["pal_count"], 0);
    let collection_id = created["data"]["collection"]["id"].as_i64().unwrap();

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "get_ups_collections", "data": null}),
    )
    .await;
    let listed = common::next_json(&mut ws).await;
    assert_eq!(listed["data"]["collections"].as_array().unwrap().len(), 1);

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "update_ups_collection",
            "data": {"collection_id": collection_id, "updates": {"is_favorite": true}}}),
    )
    .await;
    let updated = common::next_json(&mut ws).await;
    assert_eq!(updated["data"]["collection"]["is_favorite"], true);

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "update_ups_collection",
            "data": {"collection_id": 999, "updates": {}}}),
    )
    .await;
    let missing = common::next_json(&mut ws).await;
    assert_eq!(missing["type"], "error");
    assert_eq!(
        missing["data"]["message"],
        "Collection with ID 999 not found"
    );

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "delete_ups_collection", "data": {"collection_id": collection_id}}),
    )
    .await;
    let deleted = common::next_json(&mut ws).await;
    assert_eq!(
        deleted["data"],
        serde_json::json!({"success": true, "collection_id": collection_id})
    );

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "create_ups_tag", "data": {"name": "shiny", "description": null, "color": "#0f0"}}),
    )
    .await;
    let tag_created = common::next_json(&mut ws).await;
    assert_eq!(tag_created["data"]["tag"]["name"], "shiny");
    let tag_id = tag_created["data"]["tag"]["id"].as_i64().unwrap();

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "get_ups_tags", "data": null}),
    )
    .await;
    let tags = common::next_json(&mut ws).await;
    assert_eq!(tags["data"]["tags"].as_array().unwrap().len(), 1);

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "update_ups_tag",
            "data": {"tag_id": tag_id, "updates": {"name": "sparkly"}}}),
    )
    .await;
    let tag_updated = common::next_json(&mut ws).await;
    assert_eq!(tag_updated["data"]["tag"]["name"], "sparkly");

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "delete_ups_tag", "data": {"tag_id": tag_id}}),
    )
    .await;
    let tag_deleted = common::next_json(&mut ws).await;
    assert_eq!(
        tag_deleted["data"],
        serde_json::json!({"success": true, "tag_id": tag_id})
    );

    server.handle.shutdown().await;
}
