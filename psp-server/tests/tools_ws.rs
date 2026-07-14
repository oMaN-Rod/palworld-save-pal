mod common;

#[tokio::test]
async fn convert_steam_id_handles_uid_steam_id_and_garbage() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "convert_steam_id", "data": {"steam_input": "76561198000000001"}}),
    )
    .await;
    let from_steam = common::next_json(&mut ws).await;
    assert_eq!(from_steam["type"], "convert_steam_id");
    assert!(from_steam["data"]["palworld_uid"]
        .as_str()
        .unwrap()
        .chars()
        .all(|c| !c.is_ascii_lowercase()));
    assert!(from_steam["data"].get("from_uid").is_none());

    let uid = from_steam["data"]["palworld_uid"]
        .as_str()
        .unwrap()
        .to_string();
    common::send_json(
        &mut ws,
        serde_json::json!({"type": "convert_steam_id", "data": {"steam_input": uid}}),
    )
    .await;
    let from_uid = common::next_json(&mut ws).await;
    assert_eq!(from_uid["data"]["from_uid"], true);
    assert_eq!(
        from_uid["data"]["palworld_uid"],
        from_steam["data"]["palworld_uid"]
    );
    assert_eq!(
        from_uid["data"]["nosteam_uid"],
        from_steam["data"]["nosteam_uid"]
    );

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "convert_steam_id", "data": {"steam_input": "garbage!!"}}),
    )
    .await;
    let garbage = common::next_json(&mut ws).await;
    // The frontend matches on this exact string, so unparseable input must keep
    // reporting it verbatim rather than falling back to a generic message.
    assert_eq!(
        garbage["data"]["error"],
        "invalid literal for int() with base 10: 'garbage!!'"
    );

    server.handle.shutdown().await;
}

#[tokio::test]
async fn swap_player_uids_without_save_errors() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    common::send_json(
        &mut ws,
        serde_json::json!({"type": "swap_player_uids", "data": {
            "old_player_uid": "55555555-5555-5555-5555-555555555555",
            "new_player_uid": "66666666-6666-6666-6666-666666666666"}}),
    )
    .await;
    let response = common::next_json(&mut ws).await;
    assert_eq!(response["type"], "swap_player_uids");
    assert_eq!(response["data"]["error"], "No save file loaded.");
    server.handle.shutdown().await;
}

// get_raw_data echoes uesave's own JSON serialization of the located save
// subtree, so its exact field content is an implementation detail. These tests
// pin only the SHAPE: an empty object when nothing resolves, a non-empty one
// when a target does.
#[tokio::test]
async fn get_raw_data_without_resolvable_target_returns_empty_object() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    common::send_json(
        &mut ws,
        serde_json::json!({"type": "get_raw_data", "data": {
            "guild_id": null, "player_id": null, "pal_id": null, "base_id": null,
            "item_container_id": null, "character_container_id": null, "level": false}}),
    )
    .await;
    let response = common::next_json(&mut ws).await;
    assert_eq!(response["type"], "get_raw_data");
    assert_eq!(response["data"], serde_json::json!({}));

    // `get_guild_raw_data` has no handler and must stay SILENT. Prove it by
    // ordering: send the dead type, then a live probe -- the very next frame
    // answering the probe means the dead type emitted nothing.
    common::send_json(
        &mut ws,
        serde_json::json!({"type": "get_guild_raw_data", "data": null}),
    )
    .await;
    common::send_json(
        &mut ws,
        serde_json::json!({"type": "get_raw_data", "data": {
            "guild_id": null, "player_id": null, "pal_id": null, "base_id": null,
            "item_container_id": null, "character_container_id": null, "level": false}}),
    )
    .await;
    let next_frame = common::next_json(&mut ws).await;
    assert_eq!(next_frame["type"], "get_raw_data");

    server.handle.shutdown().await;
}

/// With a save loaded, `level: true` resolves to the whole GVAS root: a
/// non-empty object carrying a `properties` field. `level` is used rather than
/// a player/pal/guild id because it needs no id harvested from an earlier
/// response burst.
#[tokio::test]
async fn get_raw_data_level_resolves_against_a_loaded_world1_save() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;

    let level_sav = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/saves/world1/Level.sav")
        .to_string_lossy()
        .into_owned();
    common::send_json(
        &mut ws,
        serde_json::json!({"type": "select_save",
            "data": {"type": "steam", "path": level_sav, "local": false}}),
    )
    .await;
    // get_guild_summaries is the last frame of a successful load.
    loop {
        let frame = common::next_json(&mut ws).await;
        if frame["type"] == "get_guild_summaries" {
            break;
        }
    }

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "get_raw_data", "data": {
            "guild_id": null, "player_id": null, "pal_id": null, "base_id": null,
            "item_container_id": null, "character_container_id": null, "level": true}}),
    )
    .await;
    let response = common::next_json(&mut ws).await;
    assert_eq!(response["type"], "get_raw_data");
    let data = response["data"]
        .as_object()
        .expect("get_raw_data's data must be a JSON object once a save is loaded");
    assert!(
        !data.is_empty(),
        "level: true must resolve to a non-empty object once a save is loaded"
    );
    assert!(
        data.contains_key("properties"),
        "expected the serialized GVAS root's properties field, got keys {:?}",
        data.keys().collect::<Vec<_>>()
    );

    server.handle.shutdown().await;
}
