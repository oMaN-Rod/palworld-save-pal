mod common;

use std::time::Duration;

use common::mock_mod::{spawn_mock_mod, write_endpoint_file, MockMod};
use common::{connect, kill_mock, next_json, send_json, start_test_server, BridgeEnvGuard};
use ps_server::bridge::service::{BridgeStatus, BridgeTarget};
use ps_server::messages::MessageType;
use ps_server::signal::remote_ctl::REMOTE_DENYLIST;

const STATUS_FIXTURE: &str = include_str!("../../ps-amity/fixtures/status.json");
const PLAYERS_FIXTURE: &str = include_str!("../../ps-amity/fixtures/players.json");
const PALS_FIXTURE: &str = include_str!("../../ps-amity/fixtures/pals.json");
const PAL_DETAIL_FIXTURE: &str = include_str!("../../ps-amity/fixtures/pal_detail.json");
const INVENTORY_FIXTURE: &str = include_str!("../../ps-amity/fixtures/inventory.json");
const CAPABILITIES_FIXTURE: &str = include_str!("../../ps-amity/fixtures/capabilities.json");
const COMMAND_RESULT_HEAL_FIXTURE: &str =
    include_str!("../../ps-amity/fixtures/command_result_heal.json");
const COMMAND_RESULT_SET_ITEM_SLOT_FIXTURE: &str =
    include_str!("../../ps-amity/fixtures/command_result_set_item_slot.json");
const BUILD_INFO_FIXTURE: &str = include_str!("../../ps-amity/fixtures/build_info.json");

fn fixture_data(json: &str) -> serde_json::Value {
    serde_json::from_str::<serde_json::Value>(json).unwrap()["data"].clone()
}

/// What the probe stores after sanitizing `build_info.json`'s fixture data:
/// `gameVersion` is `null`, not a string, so it is dropped.
fn sanitized_build_info_fixture() -> serde_json::Value {
    let mut data = fixture_data(BUILD_INFO_FIXTURE);
    data.as_object_mut().unwrap().remove("gameVersion");
    data
}

async fn wait_for_connected(mut rx: tokio::sync::watch::Receiver<BridgeStatus>) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if rx.borrow().connected {
            return;
        }
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            panic!("timed out waiting for the bridge to connect");
        }
        let _ = tokio::time::timeout(remaining, rx.changed()).await;
    }
}

async fn start_server_connected_to(
    dir: &std::path::Path,
    mock: MockMod,
) -> (
    common::TestServer,
    std::net::SocketAddr,
    tokio_util::sync::CancellationToken,
    tokio::task::JoinHandle<()>,
) {
    let (addr, mock_cancel, mock_handle) = spawn_mock_mod(mock).await;
    write_endpoint_file(dir, addr.port(), "secret-token", std::process::id());
    std::env::set_var("PS_BRIDGE_ENDPOINT_DIR", dir.to_str().unwrap());

    let server = start_test_server().await;
    server.handle.services.bridge.set_target(Some(BridgeTarget {
        id: "test".to_string(),
        name: "Mock".to_string(),
        host: addr.ip().to_string(),
        port: addr.port(),
        token: "secret-token".to_string(),
    }));
    wait_for_connected(server.handle.services.bridge.status_rx()).await;
    (server, addr, mock_cancel, mock_handle)
}

#[tokio::test]
async fn game_status_returns_the_bridge_status() {
    let _env = BridgeEnvGuard::acquire(&[("PS_BRIDGE_ENDPOINT_DIR", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    );
    let (server, _addr, mock_cancel, mock_handle) =
        start_server_connected_to(dir.path(), mock).await;

    let mut client = connect(&server).await;
    send_json(
        &mut client,
        serde_json::json!({ "type": "game_status", "data": {} }),
    )
    .await;
    let reply = next_json(&mut client).await;
    assert_eq!(reply["type"], "game_status");
    assert_eq!(reply["data"], fixture_data(STATUS_FIXTURE));

    server.handle.shutdown().await;
    kill_mock(mock_cancel, mock_handle).await;
}

#[tokio::test]
async fn a_connected_bridge_carries_the_build_info() {
    let _env = BridgeEnvGuard::acquire(&[("PS_BRIDGE_ENDPOINT_DIR", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    )
    .with_build_info(fixture_data(BUILD_INFO_FIXTURE));
    let (server, _addr, mock_cancel, mock_handle) =
        start_server_connected_to(dir.path(), mock).await;

    let mut status_rx = server.handle.services.bridge.status_rx();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let build_info = loop {
        if let Some(build_info) = status_rx.borrow().build_info.clone() {
            break build_info;
        }
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            panic!("timed out waiting for build_info to be populated");
        }
        let _ = tokio::time::timeout(remaining, status_rx.changed()).await;
    };
    assert_eq!(build_info, sanitized_build_info_fixture());

    server.handle.shutdown().await;
    kill_mock(mock_cancel, mock_handle).await;
}

#[tokio::test]
async fn an_amity_without_build_info_still_connects() {
    let _env = BridgeEnvGuard::acquire(&[("PS_BRIDGE_ENDPOINT_DIR", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    );
    let (server, _addr, mock_cancel, mock_handle) =
        start_server_connected_to(dir.path(), mock).await;

    let status_rx = server.handle.services.bridge.status_rx();
    assert!(status_rx.borrow().connected);

    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(status_rx.borrow().build_info.is_none());

    server.handle.shutdown().await;
    kill_mock(mock_cancel, mock_handle).await;
}

#[tokio::test]
async fn a_probe_retries_past_an_unanswered_first_request() {
    let _env = BridgeEnvGuard::acquire(&[
        ("PS_BRIDGE_ENDPOINT_DIR", None),
        ("PS_BRIDGE_PROBE_TIMEOUT_MS", Some("30")),
    ])
    .await;
    let dir = tempfile::tempdir().unwrap();
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    )
    .with_build_info_after(1, fixture_data(BUILD_INFO_FIXTURE));
    let (server, _addr, mock_cancel, mock_handle) =
        start_server_connected_to(dir.path(), mock).await;

    let mut status_rx = server.handle.services.bridge.status_rx();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let build_info = loop {
        if let Some(build_info) = status_rx.borrow().build_info.clone() {
            break build_info;
        }
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            panic!("timed out waiting for a retried probe to populate build_info");
        }
        let _ = tokio::time::timeout(remaining, status_rx.changed()).await;
    };
    assert_eq!(build_info, sanitized_build_info_fixture());

    server.handle.shutdown().await;
    kill_mock(mock_cancel, mock_handle).await;
}

#[tokio::test]
async fn game_players_returns_the_bridge_players() {
    let _env = BridgeEnvGuard::acquire(&[("PS_BRIDGE_ENDPOINT_DIR", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    );
    let (server, _addr, mock_cancel, mock_handle) =
        start_server_connected_to(dir.path(), mock).await;

    let mut client = connect(&server).await;
    send_json(
        &mut client,
        serde_json::json!({ "type": "game_players", "data": {} }),
    )
    .await;
    let reply = next_json(&mut client).await;
    assert_eq!(reply["type"], "game_players");
    assert_eq!(reply["data"], fixture_data(PLAYERS_FIXTURE));

    server.handle.shutdown().await;
    kill_mock(mock_cancel, mock_handle).await;
}

#[tokio::test]
async fn game_pals_forwards_camel_case_and_returns_the_fixture_verbatim() {
    let _env = BridgeEnvGuard::acquire(&[("PS_BRIDGE_ENDPOINT_DIR", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    )
    .with_pals(fixture_data(PALS_FIXTURE));
    let requests = mock.requests.clone();
    let (server, _addr, mock_cancel, mock_handle) =
        start_server_connected_to(dir.path(), mock).await;

    let mut client = connect(&server).await;
    send_json(
        &mut client,
        serde_json::json!({
            "type": "game_pals",
            "data": { "player_uid": "00000000-0000-0000-0000-000000000001", "page": 0 },
        }),
    )
    .await;
    let reply = next_json(&mut client).await;
    assert_eq!(reply["type"], "game_pals");
    assert_eq!(reply["data"], fixture_data(PALS_FIXTURE));

    let logged = requests.lock().unwrap();
    let forwarded = logged
        .iter()
        .find(|request| request["type"] == "get_pals")
        .expect("get_pals should have been forwarded to the mock");
    assert_eq!(
        forwarded["data"],
        serde_json::json!({ "playerUid": "00000000-0000-0000-0000-000000000001", "page": 0 })
    );
    drop(logged);

    server.handle.shutdown().await;
    kill_mock(mock_cancel, mock_handle).await;
}

#[tokio::test]
async fn game_pal_detail_forwards_camel_case_and_returns_the_fixture_verbatim() {
    let _env = BridgeEnvGuard::acquire(&[("PS_BRIDGE_ENDPOINT_DIR", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    )
    .with_pal_detail(fixture_data(PAL_DETAIL_FIXTURE));
    let requests = mock.requests.clone();
    let (server, _addr, mock_cancel, mock_handle) =
        start_server_connected_to(dir.path(), mock).await;

    let mut client = connect(&server).await;
    send_json(
        &mut client,
        serde_json::json!({
            "type": "game_pal_detail",
            "data": {
                "player_uid": "00000000-0000-0000-0000-000000000001",
                "slot_index": 0,
            },
        }),
    )
    .await;
    let reply = next_json(&mut client).await;
    assert_eq!(reply["type"], "game_pal_detail");
    assert_eq!(reply["data"], fixture_data(PAL_DETAIL_FIXTURE));

    let logged = requests.lock().unwrap();
    let forwarded = logged
        .iter()
        .find(|request| request["type"] == "get_pal_detail")
        .expect("get_pal_detail should have been forwarded to the mock");
    assert_eq!(
        forwarded["data"],
        serde_json::json!({
            "playerUid": "00000000-0000-0000-0000-000000000001",
            "slotIndex": 0,
            "instanceId": null,
        })
    );
    drop(logged);

    server.handle.shutdown().await;
    kill_mock(mock_cancel, mock_handle).await;
}

#[tokio::test]
async fn game_inventory_forwards_camel_case_and_returns_the_fixture_verbatim() {
    let _env = BridgeEnvGuard::acquire(&[("PS_BRIDGE_ENDPOINT_DIR", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    )
    .with_inventory(fixture_data(INVENTORY_FIXTURE));
    let requests = mock.requests.clone();
    let (server, _addr, mock_cancel, mock_handle) =
        start_server_connected_to(dir.path(), mock).await;

    let mut client = connect(&server).await;
    send_json(
        &mut client,
        serde_json::json!({
            "type": "game_inventory",
            "data": { "player_uid": "00000000-0000-0000-0000-000000000001" },
        }),
    )
    .await;
    let reply = next_json(&mut client).await;
    assert_eq!(reply["type"], "game_inventory");
    assert_eq!(reply["data"], fixture_data(INVENTORY_FIXTURE));

    let logged = requests.lock().unwrap();
    let forwarded = logged
        .iter()
        .find(|request| request["type"] == "get_inventory")
        .expect("get_inventory should have been forwarded to the mock");
    assert_eq!(
        forwarded["data"],
        serde_json::json!({ "playerUid": "00000000-0000-0000-0000-000000000001" })
    );
    drop(logged);

    server.handle.shutdown().await;
    kill_mock(mock_cancel, mock_handle).await;
}

#[tokio::test]
async fn game_capabilities_returns_the_bridge_capabilities_verbatim() {
    let _env = BridgeEnvGuard::acquire(&[("PS_BRIDGE_ENDPOINT_DIR", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    )
    .with_capabilities(fixture_data(CAPABILITIES_FIXTURE));
    let (server, _addr, mock_cancel, mock_handle) =
        start_server_connected_to(dir.path(), mock).await;

    let mut client = connect(&server).await;
    send_json(
        &mut client,
        serde_json::json!({ "type": "game_capabilities", "data": {} }),
    )
    .await;
    let reply = next_json(&mut client).await;
    assert_eq!(reply["type"], "game_capabilities");
    assert_eq!(reply["data"], fixture_data(CAPABILITIES_FIXTURE));

    server.handle.shutdown().await;
    kill_mock(mock_cancel, mock_handle).await;
}

#[tokio::test]
async fn game_edit_player_forwards_camel_case_and_returns_the_command_result_verbatim() {
    let _env = BridgeEnvGuard::acquire(&[("PS_BRIDGE_ENDPOINT_DIR", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let expected = serde_json::json!({
        "commandId": "edit-player-cmd-1",
        "op": "player.edit",
        "applied": true,
        "verified": true,
        "retrySafe": true,
        "data": { "level": 81, "exp": 2454280 },
    });
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    )
    .with_command_result("player.edit", expected.clone());
    let requests = mock.requests.clone();
    let (server, _addr, mock_cancel, mock_handle) =
        start_server_connected_to(dir.path(), mock).await;

    let mut client = connect(&server).await;
    send_json(
        &mut client,
        serde_json::json!({
            "type": "game_edit_player",
            "data": {
                "player_uid": "00000000-0000-0000-0000-000000000001",
                "command_id": "edit-player-cmd-1",
                "level": 81,
                "exp": 2454280,
            },
        }),
    )
    .await;
    let reply = next_json(&mut client).await;
    assert_eq!(reply["type"], "game_edit_player");
    assert_eq!(reply["data"], expected);

    let logged = requests.lock().unwrap();
    let forwarded = logged
        .iter()
        .find(|request| request["type"] == "command")
        .expect("command should have been forwarded to the mock");
    assert_eq!(forwarded["data"]["op"], "player.edit");
    assert_eq!(forwarded["data"]["commandId"], "edit-player-cmd-1");
    assert_eq!(
        forwarded["data"]["args"],
        serde_json::json!({
            "playerUid": "00000000-0000-0000-0000-000000000001",
            "level": 81,
            "exp": 2454280,
        })
    );
    drop(logged);

    server.handle.shutdown().await;
    kill_mock(mock_cancel, mock_handle).await;
}

#[tokio::test]
async fn game_write_command_ids_are_minted_when_absent_and_forwarded_when_given() {
    let _env = BridgeEnvGuard::acquire(&[("PS_BRIDGE_ENDPOINT_DIR", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    )
    .with_command_result(
        "item.setSlot",
        fixture_data(COMMAND_RESULT_SET_ITEM_SLOT_FIXTURE),
    );
    let requests = mock.requests.clone();
    let (server, _addr, mock_cancel, mock_handle) =
        start_server_connected_to(dir.path(), mock).await;

    let mut client = connect(&server).await;
    let base = serde_json::json!({
        "player_uid": "00000000-0000-0000-0000-000000000001",
        "container_id": "66666666-7777-8888-9999-000000000000",
        "slot_index": 4,
        "static_item_id": "Wood",
        "count": 20,
    });
    send_json(
        &mut client,
        serde_json::json!({ "type": "game_set_item_slot", "data": base }),
    )
    .await;
    next_json(&mut client).await;

    let mut replay = base.clone();
    replay["command_id"] = serde_json::json!("set-slot-cmd-1");
    for _ in 0..2 {
        send_json(
            &mut client,
            serde_json::json!({ "type": "game_set_item_slot", "data": replay }),
        )
        .await;
        next_json(&mut client).await;
    }

    let logged = requests.lock().unwrap();
    let command_ids: Vec<&str> = logged
        .iter()
        .filter(|request| request["type"] == "command")
        .map(|request| request["data"]["commandId"].as_str().unwrap())
        .collect();
    assert_eq!(command_ids.len(), 3);
    assert!(
        uuid::Uuid::parse_str(command_ids[0]).is_ok(),
        "a minted command id must be a uuid, got {}",
        command_ids[0]
    );
    assert_eq!(command_ids[1], "set-slot-cmd-1");
    assert_eq!(command_ids[2], "set-slot-cmd-1");
    drop(logged);

    server.handle.shutdown().await;
    kill_mock(mock_cancel, mock_handle).await;
}

#[tokio::test]
async fn game_set_item_slot_passes_a_capability_unavailable_refusal_through() {
    let _env = BridgeEnvGuard::acquire(&[("PS_BRIDGE_ENDPOINT_DIR", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    )
    .with_command_error(
        "item.setSlot",
        "capability_unavailable",
        "item database not loaded",
    );
    let (server, _addr, mock_cancel, mock_handle) =
        start_server_connected_to(dir.path(), mock).await;

    let mut client = connect(&server).await;
    send_json(
        &mut client,
        serde_json::json!({
            "type": "game_set_item_slot",
            "data": {
                "player_uid": "00000000-0000-0000-0000-000000000001",
                "container_id": "66666666-7777-8888-9999-000000000000",
                "slot_index": 4,
                "static_item_id": "Wood",
                "count": 20,
            },
        }),
    )
    .await;
    let reply = next_json(&mut client).await;
    assert_eq!(reply["type"], "game_set_item_slot");
    assert_eq!(reply["data"]["code"], "capability_unavailable");
    assert_eq!(reply["data"]["error"], "item database not loaded");

    server.handle.shutdown().await;
    kill_mock(mock_cancel, mock_handle).await;
}

#[tokio::test]
async fn game_heal_pals_fans_out_one_command_per_target_keyed_by_slot() {
    let _env = BridgeEnvGuard::acquire(&[("PS_BRIDGE_ENDPOINT_DIR", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    )
    .with_command_result("pal.heal", fixture_data(COMMAND_RESULT_HEAL_FIXTURE));
    let requests = mock.requests.clone();
    let (server, _addr, mock_cancel, mock_handle) =
        start_server_connected_to(dir.path(), mock).await;

    let targets = serde_json::json!([
        { "slot_index": 1 },
        { "slot_index": 2 },
        { "slot_index": 30 },
    ]);
    let mut client = connect(&server).await;
    send_json(
        &mut client,
        serde_json::json!({
            "type": "game_heal_pals",
            "data": { "player_uid": "00000000-0000-0000-0000-000000000001", "targets": targets },
        }),
    )
    .await;
    let reply = next_json(&mut client).await;
    assert_eq!(reply["type"], "game_heal_pals");
    let expected_result = fixture_data(COMMAND_RESULT_HEAL_FIXTURE);
    assert_eq!(
        reply["data"],
        serde_json::json!({
            "results": [
                { "slot_index": 1, "ok": true, "result": expected_result, "error": null },
                { "slot_index": 2, "ok": true, "result": expected_result, "error": null },
                { "slot_index": 30, "ok": true, "result": expected_result, "error": null },
            ]
        })
    );

    let logged = requests.lock().unwrap();
    let heal_requests: Vec<&serde_json::Value> = logged
        .iter()
        .filter(|request| request["type"] == "command" && request["data"]["op"] == "pal.heal")
        .collect();
    assert_eq!(
        heal_requests.len(),
        3,
        "expected exactly one command per target"
    );
    let command_ids: Vec<&str> = heal_requests
        .iter()
        .map(|request| request["data"]["commandId"].as_str().unwrap())
        .collect();
    let base = command_ids[0]
        .rsplit_once(':')
        .expect("fan-out command id must be \"{base}:{slot_index}\"")
        .0;
    let expected_targets = [1, 2, 30];
    for (id, slot_index) in command_ids.iter().zip(expected_targets) {
        assert_eq!(*id, format!("{base}:{slot_index}"));
    }
    assert_eq!(
        heal_requests[1]["data"]["args"],
        serde_json::json!({
            "playerUid": "00000000-0000-0000-0000-000000000001",
            "slotIndex": 2,
        })
    );
    drop(logged);

    server.handle.shutdown().await;
    kill_mock(mock_cancel, mock_handle).await;
}

#[tokio::test]
async fn game_heal_pals_command_ids_are_order_independent() {
    let _env = BridgeEnvGuard::acquire(&[("PS_BRIDGE_ENDPOINT_DIR", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    )
    .with_command_result("pal.heal", fixture_data(COMMAND_RESULT_HEAL_FIXTURE));
    let requests = mock.requests.clone();
    let (server, _addr, mock_cancel, mock_handle) =
        start_server_connected_to(dir.path(), mock).await;

    let mut client = connect(&server).await;

    send_json(
        &mut client,
        serde_json::json!({
            "type": "game_heal_pals",
            "data": {
                "player_uid": "00000000-0000-0000-0000-000000000001",
                "command_id": "fixed-base",
                "targets": [
                    { "slot_index": 1 },
                    { "slot_index": 2 },
                    { "slot_index": 30 },
                ],
            },
        }),
    )
    .await;
    next_json(&mut client).await;

    send_json(
        &mut client,
        serde_json::json!({
            "type": "game_heal_pals",
            "data": {
                "player_uid": "00000000-0000-0000-0000-000000000001",
                "command_id": "fixed-base",
                "targets": [
                    { "slot_index": 30 },
                    { "slot_index": 2 },
                    { "slot_index": 1 },
                ],
            },
        }),
    )
    .await;
    next_json(&mut client).await;

    let logged = requests.lock().unwrap();
    let heal_requests: Vec<&serde_json::Value> = logged
        .iter()
        .filter(|request| request["type"] == "command" && request["data"]["op"] == "pal.heal")
        .collect();
    assert_eq!(
        heal_requests.len(),
        6,
        "expected three commands per submission"
    );

    let id_for = |slot_index: i64, requests: &[&serde_json::Value]| {
        requests
            .iter()
            .find(|request| request["data"]["args"]["slotIndex"] == slot_index)
            .map(|request| request["data"]["commandId"].as_str().unwrap().to_string())
            .unwrap_or_else(|| panic!("no command logged for slot {slot_index}"))
    };

    let (first_batch, second_batch) = heal_requests.split_at(3);
    for slot_index in [1, 2, 30] {
        assert_eq!(
            id_for(slot_index, first_batch),
            id_for(slot_index, second_batch),
            "the same target must get the same command id regardless of its position in the list"
        );
    }
    drop(logged);

    server.handle.shutdown().await;
    kill_mock(mock_cancel, mock_handle).await;
}

#[tokio::test]
async fn game_heal_pals_partial_failure_mid_list_still_emits_the_full_results_array() {
    let _env = BridgeEnvGuard::acquire(&[("PS_BRIDGE_ENDPOINT_DIR", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    )
    .with_command_result("pal.heal", fixture_data(COMMAND_RESULT_HEAL_FIXTURE))
    .with_command_error_at("pal.heal", 1, "invalid_target", "pal slot is empty");
    let (server, _addr, mock_cancel, mock_handle) =
        start_server_connected_to(dir.path(), mock).await;

    let targets = serde_json::json!([
        { "slot_index": 1 },
        { "slot_index": 2 },
        { "slot_index": 30 },
    ]);
    let mut client = connect(&server).await;
    send_json(
        &mut client,
        serde_json::json!({
            "type": "game_heal_pals",
            "data": { "player_uid": "00000000-0000-0000-0000-000000000001", "targets": targets },
        }),
    )
    .await;
    let reply = next_json(&mut client).await;
    assert_eq!(reply["type"], "game_heal_pals");
    let expected_result = fixture_data(COMMAND_RESULT_HEAL_FIXTURE);
    assert_eq!(
        reply["data"],
        serde_json::json!({
            "results": [
                { "slot_index": 1, "ok": true, "result": expected_result, "error": null },
                {
                    "slot_index": 2, "ok": false, "result": null,
                    "error": { "code": "invalid_target", "message": "pal slot is empty" },
                },
                { "slot_index": 30, "ok": true, "result": expected_result, "error": null },
            ]
        }),
        "a mid-list failure must not stop the remaining targets from being attempted and reported"
    );

    server.handle.shutdown().await;
    kill_mock(mock_cancel, mock_handle).await;
}

#[tokio::test]
async fn game_heal_pals_rejects_an_out_of_range_target_count() {
    let _env = BridgeEnvGuard::acquire(&[("PS_BRIDGE_ENDPOINT_DIR", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("PS_BRIDGE_ENDPOINT_DIR", dir.path().to_str().unwrap());
    let server = start_test_server().await;
    let mut client = connect(&server).await;

    for targets in [
        serde_json::json!([]),
        serde_json::json!((0..31)
            .map(|i| serde_json::json!({ "slot_index": i }))
            .collect::<Vec<_>>()),
    ] {
        send_json(
            &mut client,
            serde_json::json!({
                "type": "game_heal_pals",
                "data": { "player_uid": "00000000-0000-0000-0000-000000000001", "targets": targets },
            }),
        )
        .await;
        let reply = next_json(&mut client).await;
        assert_eq!(reply["type"], "game_heal_pals");
        assert_eq!(reply["data"]["code"], "validation_failed");
        assert!(reply["data"]["error"].as_str().is_some());
    }

    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_malformed_game_payload_is_refused_inline_on_its_own_request_type() {
    let _env = BridgeEnvGuard::acquire(&[("PS_BRIDGE_ENDPOINT_DIR", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("PS_BRIDGE_ENDPOINT_DIR", dir.path().to_str().unwrap());
    let server = start_test_server().await;
    let mut client = connect(&server).await;

    for (message_type, data) in [
        (
            "game_edit_player",
            serde_json::json!({ "player_uid": "00000000-0000-0000-0000-000000000001", "exp": 1.5 }),
        ),
        (
            "game_set_item_slot",
            serde_json::json!({
                "player_uid": "00000000-0000-0000-0000-000000000001",
                "container_id": "66666666-7777-8888-9999-000000000000",
                "slot_index": 4,
                "static_item_id": "Wood",
                "count": 2.5,
            }),
        ),
        (
            "game_heal_pals",
            serde_json::json!({
                "player_uid": "00000000-0000-0000-0000-000000000001",
                "targets": [{ "slot_index": 0.5 }],
            }),
        ),
        (
            "game_pals",
            serde_json::json!({ "player_uid": "00000000-0000-0000-0000-000000000001", "page": 1.5 }),
        ),
        (
            "game_pal_detail",
            serde_json::json!({ "player_uid": "00000000-0000-0000-0000-000000000001" }),
        ),
        ("game_inventory", serde_json::json!({})),
    ] {
        send_json(
            &mut client,
            serde_json::json!({ "type": message_type, "data": data }),
        )
        .await;
        let reply = next_json(&mut client).await;
        assert_eq!(reply["type"], message_type);
        assert_eq!(reply["data"]["code"], "validation_failed");
        assert!(reply["data"]["error"].as_str().is_some());
    }

    server.handle.shutdown().await;
}

#[tokio::test]
async fn game_write_offline_requests_reply_with_the_bridge_offline_code() {
    let _env = BridgeEnvGuard::acquire(&[("PS_BRIDGE_ENDPOINT_DIR", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("PS_BRIDGE_ENDPOINT_DIR", dir.path().to_str().unwrap());
    let server = start_test_server().await;
    let mut client = connect(&server).await;

    for (message_type, data) in [
        ("game_capabilities", serde_json::json!({})),
        (
            "game_heal_pals",
            serde_json::json!({
                "player_uid": "00000000-0000-0000-0000-000000000001",
                "targets": [{ "slot_index": 0 }],
            }),
        ),
        (
            "game_edit_player",
            serde_json::json!({
                "player_uid": "00000000-0000-0000-0000-000000000001",
                "level": 81,
                "exp": 2454280,
            }),
        ),
        (
            "game_set_item_slot",
            serde_json::json!({
                "player_uid": "00000000-0000-0000-0000-000000000001",
                "container_id": "66666666-7777-8888-9999-000000000000",
                "slot_index": 4,
                "static_item_id": "Wood",
                "count": 5,
            }),
        ),
    ] {
        send_json(
            &mut client,
            serde_json::json!({ "type": message_type, "data": data }),
        )
        .await;
        let reply = next_json(&mut client).await;
        assert_eq!(reply["type"], message_type);
        assert_eq!(reply["data"]["code"], "bridge_offline");
        assert!(reply["data"]["error"].as_str().is_some());
    }

    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_bridge_offline_request_replies_with_the_bridge_offline_code() {
    let _env = BridgeEnvGuard::acquire(&[("PS_BRIDGE_ENDPOINT_DIR", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("PS_BRIDGE_ENDPOINT_DIR", dir.path().to_str().unwrap());

    let server = start_test_server().await;
    let mut client = connect(&server).await;
    send_json(
        &mut client,
        serde_json::json!({ "type": "game_status", "data": {} }),
    )
    .await;
    let reply = next_json(&mut client).await;
    assert_eq!(reply["type"], "game_status");
    assert_eq!(reply["data"]["code"], "bridge_offline");
    assert!(reply["data"]["error"].as_str().is_some());

    server.handle.shutdown().await;
}

#[tokio::test]
async fn game_set_item_slot_forwards_camel_case_and_returns_the_command_result_verbatim() {
    let _env = BridgeEnvGuard::acquire(&[("PS_BRIDGE_ENDPOINT_DIR", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    )
    .with_command_result(
        "item.setSlot",
        fixture_data(COMMAND_RESULT_SET_ITEM_SLOT_FIXTURE),
    );
    let requests = mock.requests.clone();
    let (server, _addr, mock_cancel, mock_handle) =
        start_server_connected_to(dir.path(), mock).await;

    let mut client = connect(&server).await;
    send_json(
        &mut client,
        serde_json::json!({
            "type": "game_set_item_slot",
            "data": {
                "player_uid": "00000000-0000-0000-0000-000000000001",
                "container_id": "66666666-7777-8888-9999-000000000000",
                "slot_index": 4,
                "static_item_id": "Wood",
                "count": 20,
            },
        }),
    )
    .await;
    let reply = next_json(&mut client).await;
    assert_eq!(reply["type"], "game_set_item_slot");
    assert_eq!(
        reply["data"],
        fixture_data(COMMAND_RESULT_SET_ITEM_SLOT_FIXTURE)
    );

    let logged = requests.lock().unwrap();
    let forwarded = logged
        .iter()
        .find(|request| request["type"] == "command")
        .expect("command should have been forwarded to the mock");
    assert_eq!(forwarded["data"]["op"], "item.setSlot");
    assert_eq!(
        forwarded["data"]["args"],
        serde_json::json!({
            "playerUid": "00000000-0000-0000-0000-000000000001",
            "containerId": "66666666-7777-8888-9999-000000000000",
            "slotIndex": 4,
            "staticItemId": "Wood",
            "count": 20,
        })
    );
    drop(logged);

    server.handle.shutdown().await;
    kill_mock(mock_cancel, mock_handle).await;
}

#[tokio::test]
async fn game_set_item_slot_forwards_nulls_when_clearing_a_slot() {
    let _env = BridgeEnvGuard::acquire(&[("PS_BRIDGE_ENDPOINT_DIR", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    )
    .with_command_result(
        "item.setSlot",
        fixture_data(COMMAND_RESULT_SET_ITEM_SLOT_FIXTURE),
    );
    let requests = mock.requests.clone();
    let (server, _addr, mock_cancel, mock_handle) =
        start_server_connected_to(dir.path(), mock).await;

    let mut client = connect(&server).await;
    send_json(
        &mut client,
        serde_json::json!({
            "type": "game_set_item_slot",
            "data": {
                "player_uid": "00000000-0000-0000-0000-000000000001",
                "container_id": "66666666-7777-8888-9999-000000000000",
                "slot_index": 4,
            },
        }),
    )
    .await;
    let reply = next_json(&mut client).await;
    assert_eq!(reply["type"], "game_set_item_slot");

    let logged = requests.lock().unwrap();
    let forwarded = logged
        .iter()
        .find(|request| request["type"] == "command")
        .expect("command should have been forwarded to the mock");
    assert_eq!(
        forwarded["data"]["args"]["staticItemId"],
        serde_json::Value::Null
    );
    assert_eq!(forwarded["data"]["args"]["count"], serde_json::Value::Null);
    drop(logged);

    server.handle.shutdown().await;
    kill_mock(mock_cancel, mock_handle).await;
}

#[tokio::test]
async fn game_remove_pal_forwards_camel_case() {
    let _env = BridgeEnvGuard::acquire(&[("PS_BRIDGE_ENDPOINT_DIR", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    )
    .with_command_result("pal.remove", fixture_data(COMMAND_RESULT_HEAL_FIXTURE));
    let requests = mock.requests.clone();
    let (server, _addr, mock_cancel, mock_handle) =
        start_server_connected_to(dir.path(), mock).await;

    let mut client = connect(&server).await;
    send_json(
        &mut client,
        serde_json::json!({
            "type": "game_remove_pal",
            "data": {
                "player_uid": "00000000-0000-0000-0000-000000000001",
                "slot_index": 37,
            },
        }),
    )
    .await;
    let reply = next_json(&mut client).await;
    assert_eq!(reply["type"], "game_remove_pal");

    let logged = requests.lock().unwrap();
    let forwarded = logged
        .iter()
        .find(|request| request["type"] == "command")
        .expect("command should have been forwarded to the mock");
    assert_eq!(forwarded["data"]["op"], "pal.remove");
    assert_eq!(
        forwarded["data"]["args"],
        serde_json::json!({
            "playerUid": "00000000-0000-0000-0000-000000000001",
            "slotIndex": 37,
        })
    );
    drop(logged);

    server.handle.shutdown().await;
    kill_mock(mock_cancel, mock_handle).await;
}

#[tokio::test]
async fn game_move_pal_forwards_both_ends() {
    let _env = BridgeEnvGuard::acquire(&[("PS_BRIDGE_ENDPOINT_DIR", None)]).await;
    let dir = tempfile::tempdir().unwrap();
    let mock = MockMod::new(
        "secret-token",
        fixture_data(STATUS_FIXTURE),
        fixture_data(PLAYERS_FIXTURE),
    )
    .with_command_result("pal.move", fixture_data(COMMAND_RESULT_HEAL_FIXTURE));
    let requests = mock.requests.clone();
    let (server, _addr, mock_cancel, mock_handle) =
        start_server_connected_to(dir.path(), mock).await;

    let mut client = connect(&server).await;
    send_json(
        &mut client,
        serde_json::json!({
            "type": "game_move_pal",
            "data": {
                "player_uid": "00000000-0000-0000-0000-000000000001",
                "from_slot_index": 4,
                "to_slot_index": 61,
            },
        }),
    )
    .await;
    let reply = next_json(&mut client).await;
    assert_eq!(reply["type"], "game_move_pal");

    let logged = requests.lock().unwrap();
    let forwarded = logged
        .iter()
        .find(|request| request["type"] == "command")
        .expect("command should have been forwarded to the mock");
    assert_eq!(forwarded["data"]["op"], "pal.move");
    assert_eq!(
        forwarded["data"]["args"],
        serde_json::json!({
            "playerUid": "00000000-0000-0000-0000-000000000001",
            "fromSlotIndex": 4,
            "toSlotIndex": 61,
        })
    );
    drop(logged);

    server.handle.shutdown().await;
    kill_mock(mock_cancel, mock_handle).await;
}

#[test]
fn game_messages_are_split_between_remote_reachable_and_denylisted() {
    let game_messages: Vec<MessageType> = MessageType::ALL
        .iter()
        .copied()
        .filter(|message_type| message_type.as_wire().starts_with("game_"))
        .collect();

    let reachable = [
        MessageType::GameStatus,
        MessageType::GamePlayers,
        MessageType::GamePals,
        MessageType::GamePalDetail,
        MessageType::GameInventory,
        MessageType::GameGuild,
        MessageType::GameGuilds,
        MessageType::GameBasePals,
        MessageType::GameGuildContainers,
        MessageType::GameEditGuild,
        MessageType::GameSetGuildRole,
        MessageType::GameCapabilities,
        MessageType::GameHealPals,
        MessageType::GameSetItemSlot,
        MessageType::GameRemovePal,
        MessageType::GameMovePal,
        MessageType::GameAddPal,
        MessageType::GameEditPal,
        MessageType::GameEditPlayer,
    ];
    // Instance management (list/add/update/delete/select) exposes the user's LAN
    // topology and saved credentials to a remote guest, and a test probe makes
    // the host dial an arbitrary host:port on the guest's behalf -- all denied.
    let denylisted = [
        MessageType::GameInstances,
        MessageType::GameAddInstance,
        MessageType::GameUpdateInstance,
        MessageType::GameDeleteInstance,
        MessageType::GameSelectInstance,
        MessageType::GameTestInstance,
        MessageType::GameInstanceSetTarget,
        MessageType::GameLaunch,
    ];

    for expected in reachable.iter().chain(denylisted.iter()) {
        assert!(
            game_messages.contains(expected),
            "{expected:?} should be picked up by the game_ prefix"
        );
    }
    assert_eq!(
        game_messages.len(),
        reachable.len() + denylisted.len(),
        "a game_ message missing from both arrays would otherwise pass this test silently"
    );
    for message_type in game_messages {
        assert_eq!(
            REMOTE_DENYLIST.contains(&message_type),
            denylisted.contains(&message_type),
            "{message_type:?} denylist membership does not match the expected split"
        );
    }
}
