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
    assert_eq!(
        garbage["data"]["error"],
        "Invalid input. Enter a numeric Steam ID, profile URL, or Palworld UID."
    );

    server.handle.shutdown().await;
}
