mod common;

use std::time::Duration;

use serde_json::json;

use ps_server::bridge::service::BridgeTarget;
use ps_server::services::mods::verify::{
    ModVerification, Resolution, TargetVerification, VerifyStatus,
};

/// A target row bound-able from this test process's own executable path,
/// without touching the real cargo target directory `client_target` writes
/// an install tree into.
async fn bindable_target(driver: &ps_db::SqlxSqliteDriver, id: &str, up: u32) -> String {
    let mut root = std::env::current_exe().unwrap();
    for _ in 0..up {
        root = root.parent().unwrap().to_path_buf();
    }
    let target = ps_db::mod_targets::upsert(
        driver,
        &ps_db::mod_targets::NewModTarget {
            id: id.to_string(),
            kind: "client".to_string(),
            name: id.to_string(),
            root_path: root.to_string_lossy().into_owned(),
            platform: "win64".to_string(),
            ue4ss_mode: "standard".to_string(),
            layout_overrides: "{}".to_string(),
            detected: "{}".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    common::mods::default_profile(driver, &target.id).await;
    target.id
}

async fn add_target(ws: &mut common::WsClient, install: &tempfile::TempDir) -> String {
    common::send_json(
        ws,
        json!({"type": "mod_target_add", "data": {
            "root_path": install.path().to_string_lossy(),
            "name": "Test Install",
        }}),
    )
    .await;
    let added = common::next_json(ws).await;
    assert_eq!(added["type"], "mod_target_add");
    added["data"]["target"]["id"].as_str().unwrap().to_string()
}

fn verification(target_id: &str, live: bool) -> TargetVerification {
    TargetVerification {
        target_id: target_id.to_string(),
        instance_id: "auto:1".to_string(),
        live,
        checked_at: "2026-09-15T00:00:00Z".to_string(),
        build_info: None,
        resolution: Resolution {
            complete: true,
            missing: vec![],
        },
        status: vec![ModVerification {
            mod_id: Some("coolmod-ue4ss".to_string()),
            name: "CoolMod".to_string(),
            kind: "ue4ss".to_string(),
            status: VerifyStatus::Verified,
        }],
    }
}

#[tokio::test]
async fn get_before_any_result_replies_null() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, &install).await;

    common::send_json(
        &mut ws,
        json!({"type": "mod_verification_get", "data": {"target_id": target_id}}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;
    assert_eq!(reply["type"], "mod_verification_get");
    assert_eq!(reply["data"]["target_id"], target_id);
    assert_eq!(reply["data"]["verification"], json!(null));

    server.handle.shutdown().await;
}

#[tokio::test]
async fn get_for_an_unknown_target_is_refused() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;

    common::send_json(
        &mut ws,
        json!({"type": "mod_verification_get", "data": {"target_id": "client-nope"}}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;
    assert_eq!(reply["type"], "mod_verification_get");
    assert_eq!(reply["data"]["error"]["code"], "target_not_found");

    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_subscribed_client_is_pushed_results_from_the_real_verifier() {
    let _env = common::BridgeEnvGuard::acquire(&[
        ("PS_BRIDGE_ENDPOINT_DIR", None),
        ("PS_VERIFY_INTERVAL_SECS", Some("1")),
    ])
    .await;

    let mock = common::mock_mod::MockMod::new("secret-token", json!({}), json!([]))
        .with_build_info(json!({ "amityVersion": "0.3.0" }))
        .with_loaded_mods(json!({ "ue4ss": [] }))
        .with_resolution_report(json!({ "ok": [], "missing": [], "complete": true }));
    let (addr, mock_cancel, mock_handle) = common::spawn_mock_mod(mock).await;

    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;

    let db_path = server._temp_dir.path().join("ps-rs.db");
    let pool = ps_db::open(&db_path).await.unwrap();
    let driver = ps_db::SqlxSqliteDriver::new(pool);
    let target_id = bindable_target(&driver, "client-target", 2).await;

    common::send_json(
        &mut ws,
        json!({"type": "mod_verification_subscribe", "data": {}}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;
    assert_eq!(reply["data"]["active"], true);

    server.handle.services.bridge.set_target(Some(BridgeTarget {
        id: format!("auto:{}", std::process::id()),
        name: "Mock".to_string(),
        host: addr.ip().to_string(),
        port: addr.port(),
        token: "secret-token".to_string(),
    }));

    let (push, _skipped) = common::mods_ws::next_of_type(&mut ws, "mod_verification").await;
    assert_eq!(push["data"]["target_id"], target_id);
    assert_eq!(push["data"]["live"], true);

    common::kill_mock(mock_cancel, mock_handle).await;

    let push = loop {
        let (push, _skipped) = common::mods_ws::next_of_type(&mut ws, "mod_verification").await;
        if push["data"]["live"] == false {
            break push;
        }
    };
    assert_eq!(push["data"]["target_id"], target_id);

    server.handle.shutdown().await;
}

#[tokio::test]
async fn subscribing_pushes_the_published_result() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, &install).await;

    server
        .handle
        .services
        .verification
        .publish(verification(&target_id, true));

    common::send_json(
        &mut ws,
        json!({"type": "mod_verification_subscribe", "data": {}}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;
    assert_eq!(reply["type"], "mod_verification_subscribe");
    assert_eq!(reply["data"]["active"], true);

    let (push, _skipped) = common::mods_ws::next_of_type(&mut ws, "mod_verification").await;
    assert_eq!(push["data"]["target_id"], target_id);
    assert_eq!(push["data"]["status"][0]["status"], "verified");
    assert_eq!(push["data"]["live"], true);

    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_changed_publish_pushes_again_but_an_identical_one_does_not() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, &install).await;

    server
        .handle
        .services
        .verification
        .publish(verification(&target_id, true));

    common::send_json(
        &mut ws,
        json!({"type": "mod_verification_subscribe", "data": {}}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;
    assert_eq!(reply["data"]["active"], true);
    let (_push, _skipped) = common::mods_ws::next_of_type(&mut ws, "mod_verification").await;

    server
        .handle
        .services
        .verification
        .publish(verification(&target_id, false));
    let (push, _skipped) = common::mods_ws::next_of_type(&mut ws, "mod_verification").await;
    assert_eq!(push["data"]["target_id"], target_id);
    assert_eq!(push["data"]["live"], false);

    server
        .handle
        .services
        .verification
        .publish(verification(&target_id, false));
    let nothing =
        tokio::time::timeout(Duration::from_millis(300), common::next_json(&mut ws)).await;
    assert!(
        nothing.is_err(),
        "an identical publish must not produce another push"
    );

    server.handle.shutdown().await;
}

#[tokio::test]
async fn subscribing_twice_does_not_spawn_a_second_forwarder() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, &install).await;

    server
        .handle
        .services
        .verification
        .publish(verification(&target_id, true));

    common::send_json(
        &mut ws,
        json!({"type": "mod_verification_subscribe", "data": {}}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;
    assert_eq!(reply["data"]["active"], true);
    let (_push, _skipped) = common::mods_ws::next_of_type(&mut ws, "mod_verification").await;

    common::send_json(
        &mut ws,
        json!({"type": "mod_verification_subscribe", "data": {}}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;
    assert_eq!(reply["type"], "mod_verification_subscribe");
    assert_eq!(reply["data"]["active"], true);

    server
        .handle
        .services
        .verification
        .publish(verification(&target_id, false));
    let (push, skipped) = common::mods_ws::next_of_type(&mut ws, "mod_verification").await;
    assert_eq!(push["data"]["live"], false);
    assert!(
        skipped.is_empty(),
        "a second subscribe must not spawn a duplicate forwarder: {skipped:?}"
    );

    let nothing =
        tokio::time::timeout(Duration::from_millis(300), common::next_json(&mut ws)).await;
    assert!(nothing.is_err(), "exactly one push per change");

    server.handle.shutdown().await;
}

#[tokio::test]
async fn get_after_a_result_returns_the_stored_entry() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, &install).await;

    server
        .handle
        .services
        .verification
        .publish(verification(&target_id, true));

    common::send_json(
        &mut ws,
        json!({"type": "mod_verification_get", "data": {"target_id": target_id}}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;
    assert_eq!(reply["type"], "mod_verification_get");
    assert_eq!(reply["data"]["verification"]["target_id"], target_id);
    assert_eq!(reply["data"]["verification"]["live"], true);
    assert_eq!(
        reply["data"]["verification"]["status"][0]["status"],
        "verified"
    );

    server.handle.shutdown().await;
}

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) {
    std::fs::create_dir_all(dst).unwrap();
    for dir_entry in std::fs::read_dir(src).unwrap() {
        let dir_entry = dir_entry.unwrap();
        let entry_path = dir_entry.path();
        let dest_path = dst.join(dir_entry.file_name());
        if entry_path.is_dir() {
            copy_dir_recursive(&entry_path, &dest_path);
        } else {
            std::fs::copy(&entry_path, &dest_path).unwrap();
        }
    }
}

/// Copies the committed world1 fixture to a temp dir and returns (temp handle,
/// Level.sav path), so a test can register a real session via `select_save`.
fn temp_world1() -> (tempfile::TempDir, String) {
    let temp_root = tempfile::tempdir().unwrap();
    let world1_copy = temp_root.path().join("world1");
    copy_dir_recursive(
        &repo_root().join("tests/fixtures/saves/world1"),
        &world1_copy,
    );
    let level_sav_path = world1_copy.join("Level.sav").to_string_lossy().into_owned();
    (temp_root, level_sav_path)
}

async fn recv_until(socket: &mut common::WsClient, stop_type: &str) -> Vec<serde_json::Value> {
    let mut frames = Vec::new();
    loop {
        let frame = common::next_json(socket).await;
        let message_type = frame["type"].as_str().unwrap_or_default().to_string();
        frames.push(frame.clone());
        if message_type == "error" && stop_type != "error" {
            panic!("unexpected error frame while awaiting {stop_type}: {frame}");
        }
        if message_type == stop_type {
            break;
        }
    }
    frames
}

async fn select_world1(socket: &mut common::WsClient, level_sav_path: &str) -> String {
    common::send_json(
        socket,
        json!({"type": "select_save",
               "data": {"type": "steam", "path": level_sav_path, "local": true}}),
    )
    .await;
    let frames = recv_until(socket, "get_guild_summaries").await;
    let loaded = frames
        .iter()
        .find(|frame| frame["type"] == "loaded_save_files")
        .expect("loaded_save_files frame");
    loaded["data"]["session_id"]
        .as_str()
        .expect("session_id in loaded_save_files")
        .to_string()
}

#[tokio::test]
async fn reattaching_a_subscribed_session_still_subscribes_the_new_connection() {
    let (_temp_root, level_sav_path) = temp_world1();
    let server = common::start_test_server().await;
    let install = common::mods_ws::fake_windows_install();

    let mut ws1 = common::connect(&server).await;
    let target_id = add_target(&mut ws1, &install).await;
    let session_id = select_world1(&mut ws1, &level_sav_path).await;

    common::send_json(
        &mut ws1,
        json!({"type": "mod_verification_subscribe", "data": {}}),
    )
    .await;
    let reply = common::next_json(&mut ws1).await;
    assert_eq!(reply["data"]["active"], true);

    server
        .handle
        .services
        .verification
        .publish(verification(&target_id, true));
    let (push1, _skipped) = common::mods_ws::next_of_type(&mut ws1, "mod_verification").await;
    assert_eq!(push1["data"]["target_id"], target_id);

    // Connection 2 adopts session 1's session via reattach. With the flag
    // tracked on `Session` instead of the connection, this session would
    // already read "subscribed" from connection 1 and connection 2 would
    // never get its own forwarder.
    let mut ws2 = common::connect(&server).await;
    common::send_json(
        &mut ws2,
        json!({"type": "reattach_session", "data": {"session_id": session_id}}),
    )
    .await;
    recv_until(&mut ws2, "get_guild_summaries").await;

    common::send_json(
        &mut ws2,
        json!({"type": "mod_verification_subscribe", "data": {}}),
    )
    .await;
    let reply2 = common::next_json(&mut ws2).await;
    assert_eq!(reply2["type"], "mod_verification_subscribe");
    assert_eq!(reply2["data"]["active"], true);

    // ws2's forwarder flushes the store's current entry (still `live: true`
    // from connection 1's publish) as soon as it subscribes.
    let (initial, _skipped) = common::mods_ws::next_of_type(&mut ws2, "mod_verification").await;
    assert_eq!(initial["data"]["target_id"], target_id);
    assert_eq!(initial["data"]["live"], true);

    server
        .handle
        .services
        .verification
        .publish(verification(&target_id, false));
    let (push2, _skipped) = common::mods_ws::next_of_type(&mut ws2, "mod_verification").await;
    assert_eq!(push2["data"]["target_id"], target_id);
    assert_eq!(push2["data"]["live"], false);

    // A second subscribe on ws2 itself must still not duplicate its forwarder.
    common::send_json(
        &mut ws2,
        json!({"type": "mod_verification_subscribe", "data": {}}),
    )
    .await;
    let reply3 = common::next_json(&mut ws2).await;
    assert_eq!(reply3["data"]["active"], true);

    server
        .handle
        .services
        .verification
        .publish(verification(&target_id, true));
    let (push3, skipped) = common::mods_ws::next_of_type(&mut ws2, "mod_verification").await;
    assert_eq!(push3["data"]["live"], true);
    assert!(
        skipped.is_empty(),
        "a duplicate forwarder would double-push: {skipped:?}"
    );

    let nothing =
        tokio::time::timeout(Duration::from_millis(300), common::next_json(&mut ws2)).await;
    assert!(nothing.is_err(), "exactly one push per change on ws2");

    server.handle.shutdown().await;
}
