mod common;

use std::time::Duration;

use common::{BridgeEnvGuard, MockMod};

async fn send_and_wait(
    client: &mut common::TestClient,
    kind: &str,
    data: serde_json::Value,
) -> serde_json::Value {
    common::send_json(client, serde_json::json!({ "type": kind, "data": data })).await;
    common::next_json(client).await["data"].clone()
}

#[tokio::test]
async fn saved_instances_round_trip_through_list_select_update_and_delete() {
    let _env = BridgeEnvGuard::acquire(&[("PSP_BRIDGE_ENDPOINT_DIR", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("PSP_BRIDGE_ENDPOINT_DIR", dir.path().to_str().unwrap());

    let server = common::start_test_server().await;
    let mut client = common::connect(&server).await;

    let empty = send_and_wait(&mut client, "game_instances", serde_json::json!({})).await;
    assert_eq!(empty["instances"].as_array().unwrap().len(), 0);
    assert!(empty["activeId"].is_null());

    send_and_wait(
        &mut client,
        "game_add_instance",
        serde_json::json!({ "name": "Remote", "host": "10.0.0.14", "port": 8788, "token": "s3cr3t" }),
    )
    .await;

    let listed = send_and_wait(&mut client, "game_instances", serde_json::json!({})).await;
    let instances = listed["instances"].as_array().unwrap();
    assert_eq!(instances.len(), 1);
    assert_eq!(instances[0]["source"], "saved");
    assert_eq!(instances[0]["name"], "Remote");
    assert_eq!(instances[0]["host"], "10.0.0.14");
    assert_eq!(instances[0]["live"], false);
    let id = instances[0]["id"].as_str().unwrap().to_string();
    assert!(listed["activeId"].is_null(), "adding must not select");

    let selected =
        send_and_wait(&mut client, "game_select_instance", serde_json::json!({ "id": id })).await;
    assert_eq!(selected["activeId"], id);

    let renamed = send_and_wait(
        &mut client,
        "game_update_instance",
        serde_json::json!({ "id": id, "name": "Renamed", "host": "10.0.0.14", "port": 8788, "token": "s3cr3t" }),
    )
    .await;
    assert_eq!(renamed["instances"][0]["name"], "Renamed");

    let after =
        send_and_wait(&mut client, "game_delete_instance", serde_json::json!({ "id": id })).await;
    assert_eq!(after["instances"].as_array().unwrap().len(), 0);
    assert!(after["activeId"].is_null(), "deleting the active instance must clear it");

    server.handle.shutdown().await;
}

#[tokio::test]
async fn selecting_an_unknown_instance_is_refused() {
    let _env = BridgeEnvGuard::acquire(&[("PSP_BRIDGE_ENDPOINT_DIR", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("PSP_BRIDGE_ENDPOINT_DIR", dir.path().to_str().unwrap());

    let server = common::start_test_server().await;
    let mut client = common::connect(&server).await;

    common::send_json(
        &mut client,
        serde_json::json!({ "type": "game_select_instance", "data": { "id": "saved:404" } }),
    )
    .await;
    let reply = common::next_json(&mut client).await;
    assert_eq!(
        reply["type"], "game_select_instance",
        "a refusal must be emitted on the request's own type or the client's await never resolves"
    );
    assert_eq!(reply["data"]["code"], "validation_failed");

    server.handle.shutdown().await;
}

#[tokio::test]
async fn updating_the_active_instance_retargets_the_live_connection() {
    let _env = BridgeEnvGuard::acquire(&[("PSP_BRIDGE_ENDPOINT_DIR", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("PSP_BRIDGE_ENDPOINT_DIR", dir.path().to_str().unwrap());

    let server = common::start_test_server().await;
    let mut client = common::connect(&server).await;

    send_and_wait(
        &mut client,
        "game_add_instance",
        serde_json::json!({ "name": "Remote", "host": "10.0.0.14", "port": 8788, "token": "s3cr3t" }),
    )
    .await;
    let listed = send_and_wait(&mut client, "game_instances", serde_json::json!({})).await;
    let id = listed["instances"][0]["id"].as_str().unwrap().to_string();

    send_and_wait(&mut client, "game_select_instance", serde_json::json!({ "id": id })).await;
    assert_eq!(
        server.handle.services.bridge.target().unwrap().port,
        8788,
        "sanity: selecting must set the live target"
    );

    send_and_wait(
        &mut client,
        "game_update_instance",
        serde_json::json!({
            "id": id, "name": "Remote", "host": "10.0.0.14", "port": 9999, "token": "new-token"
        }),
    )
    .await;

    let target = server
        .handle
        .services
        .bridge
        .target()
        .expect("editing the active instance must not clear the target");
    assert_eq!(target.port, 9999, "the live target must pick up the edited port");
    assert_eq!(target.token, "new-token", "the live target must pick up the edited token");

    server.handle.shutdown().await;
}

#[tokio::test]
async fn testing_an_unreachable_instance_reports_failure() {
    let _env = BridgeEnvGuard::acquire(&[("PSP_BRIDGE_ENDPOINT_DIR", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("PSP_BRIDGE_ENDPOINT_DIR", dir.path().to_str().unwrap());

    let server = common::start_test_server().await;
    let mut client = common::connect(&server).await;

    let reply = send_and_wait(
        &mut client,
        "game_test_instance",
        serde_json::json!({ "name": "Dead", "host": "127.0.0.1", "port": 1, "token": "t" }),
    )
    .await;
    assert_eq!(reply["ok"], false);
    assert!(reply["error"].as_str().is_some());

    server.handle.shutdown().await;
}

#[tokio::test]
async fn testing_a_reachable_instance_reports_success() {
    let _env = BridgeEnvGuard::acquire(&[("PSP_BRIDGE_ENDPOINT_DIR", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("PSP_BRIDGE_ENDPOINT_DIR", dir.path().to_str().unwrap());

    let mock = MockMod::new("s3cr3t", serde_json::json!({}), serde_json::json!({}));
    let (addr, mock_cancel, mock_handle) = common::spawn_mock_mod(mock).await;

    let server = common::start_test_server().await;
    let mut client = common::connect(&server).await;

    let reply = send_and_wait(
        &mut client,
        "game_test_instance",
        serde_json::json!({ "name": "Mock", "host": "127.0.0.1", "port": addr.port(), "token": "s3cr3t" }),
    )
    .await;
    assert_eq!(reply["ok"], true);
    assert_eq!(reply["modVersion"], "0.1.0");

    server.handle.shutdown().await;
    let _ = tokio::time::timeout(Duration::from_secs(2), async {
        mock_cancel.cancel();
        let _ = mock_handle.await;
    })
    .await;
}

/// The zero-configuration property users actually notice: launch PSP with no
/// game running yet (or restart the game under a new pid) and, once a live
/// instance appears on disk, the bridge adopts it without a manual reselect.
#[tokio::test]
async fn a_discovery_file_appearing_after_startup_is_adopted() {
    let _env = BridgeEnvGuard::acquire(&[
        ("PSP_BRIDGE_ENDPOINT_DIR", None),
        ("PSP_BRIDGE_RECONCILE_INTERVAL_MS", Some("30")),
    ])
    .await;
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("PSP_BRIDGE_ENDPOINT_DIR", dir.path().to_str().unwrap());

    let mock = MockMod::new("secret-token", serde_json::json!({}), serde_json::json!({}));
    let (addr, mock_cancel, mock_handle) = common::spawn_mock_mod(mock).await;

    let server = common::start_test_server().await;
    let mut client = common::connect(&server).await;

    let before = send_and_wait(&mut client, "game_instances", serde_json::json!({})).await;
    assert_eq!(before["instances"].as_array().unwrap().len(), 0);
    assert!(before["activeId"].is_null());
    assert!(server.handle.services.bridge.target().is_none());

    common::mock_mod::write_endpoint_file(
        dir.path(),
        addr.port(),
        "secret-token",
        std::process::id(),
    );

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if server.handle.services.bridge.target().is_some() {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the bridge never adopted the instance discovered after startup"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let target = server.handle.services.bridge.target().unwrap();
    assert!(target.id.starts_with("auto:"));
    assert_eq!(target.port, addr.port());

    server.handle.shutdown().await;
    let _ = tokio::time::timeout(Duration::from_secs(2), async {
        mock_cancel.cancel();
        let _ = mock_handle.await;
    })
    .await;
}
