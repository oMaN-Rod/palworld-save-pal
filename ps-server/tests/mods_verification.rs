mod common;

use common::mods::{
    client_target, db_and_dir, default_profile, enable, install_fixture_mod, install_framework,
};
use common::start_test_server;
use ps_server::services::mods::verify::expected_for_target;
use ps_server::services::mods::LibraryPaths;

#[tokio::test]
async fn expected_for_target_lists_enabled_mods_and_framework_slots() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let target = client_target(&db, &dir.path().join("client")).await;
    default_profile(&db, &target.id).await;

    install_fixture_mod(
        &db,
        &paths,
        "coolmod",
        "1.0",
        &[("ue4ss", "CoolMod/Scripts/main.lua", b"x")],
    )
    .await;
    enable(&db, &target.id, "coolmod", None, true).await;

    install_fixture_mod(
        &db,
        &paths,
        "schema-thing",
        "1.0",
        &[("palschema", "Schema/pals.json", b"{}")],
    )
    .await;
    enable(&db, &target.id, "schema-thing", None, true).await;

    install_fixture_mod(
        &db,
        &paths,
        "off",
        "1.0",
        &[("ue4ss", "Off/Scripts/main.lua", b"x")],
    )
    .await;
    enable(&db, &target.id, "off", None, false).await;

    let ue4ss_framework = install_framework(
        &db,
        &paths,
        "framework-ue4ss",
        "3.0.1",
        &[
            ("ue4ss", "BPModLoaderMod/Scripts/main.lua", b"x"),
            ("ue4ss", "Keybinds/Scripts/main.lua", b"x"),
        ],
    )
    .await;
    ps_db::mod_profiles::set_framework(&db, &target.id, "ue4ss", &ue4ss_framework.version.id)
        .await
        .unwrap();

    let amity_framework = install_framework(
        &db,
        &paths,
        "framework-amity",
        "0.3.0",
        &[("ue4ss", "PSAmity/dlls/main.dll", b"x")],
    )
    .await;
    ps_db::mod_profiles::set_framework(&db, &target.id, "amity", &amity_framework.version.id)
        .await
        .unwrap();

    let (expected, builtins) = expected_for_target(&db, &target).await.unwrap();

    let names: Vec<&str> = expected.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"CoolMod"));
    assert!(names.contains(&"UE4SS"));
    assert!(names.contains(&"PSAmity"));
    assert!(!names.contains(&"Off"));

    let coolmod = expected.iter().find(|e| e.name == "CoolMod").unwrap();
    assert!(coolmod.observable);
    let schema = expected
        .iter()
        .find(|e| e.name != "CoolMod" && e.name != "UE4SS" && e.name != "PSAmity")
        .unwrap();
    assert!(!schema.observable);
    let ue4ss = expected.iter().find(|e| e.name == "UE4SS").unwrap();
    assert!(!ue4ss.observable);
    let amity = expected.iter().find(|e| e.name == "PSAmity").unwrap();
    assert!(amity.observable);

    let mut builtins_sorted = builtins.clone();
    builtins_sorted.sort();
    assert_eq!(builtins_sorted, vec!["BPModLoaderMod", "Keybinds"]);
}

const BUILD_INFO_FIXTURE: &str = include_str!("../../ps-amity/fixtures/build_info.json");
const RESOLUTION_REPORT_FIXTURE: &str =
    include_str!("../../ps-amity/fixtures/resolution_report.json");

fn fixture_data(json: &str) -> serde_json::Value {
    serde_json::from_str::<serde_json::Value>(json).unwrap()["data"].clone()
}

#[tokio::test]
async fn the_verifier_publishes_and_marks_offline_end_to_end() {
    let _env = common::BridgeEnvGuard::acquire(&[
        ("PS_BRIDGE_ENDPOINT_DIR", None),
        ("PS_VERIFY_INTERVAL_SECS", Some("1")),
    ])
    .await;

    let mock = common::mock_mod::MockMod::new(
        "secret-token",
        serde_json::json!({}),
        serde_json::json!([]),
    )
    .with_build_info(fixture_data(BUILD_INFO_FIXTURE))
    .with_loaded_mods(serde_json::json!({
        "ue4ss": [
            { "name": "CoolMod", "enabled": true, "hasLua": true, "hasDll": false },
            { "name": "Stray", "enabled": true, "hasLua": true, "hasDll": false },
        ],
    }))
    .with_resolution_report(fixture_data(RESOLUTION_REPORT_FIXTURE));
    let (addr, mock_cancel, mock_handle) = common::spawn_mock_mod(mock).await;

    let server = start_test_server().await;

    let root = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let db_path = server._temp_dir.path().join("ps-rs.db");
    let pool = ps_db::open(&db_path).await.unwrap();
    let driver = ps_db::SqlxSqliteDriver::new(pool);
    // Not `common::mods::client_target`: that also writes a fake install tree,
    // and `root` here is the real cargo target directory (the test binary's
    // grandparent), which nothing in this suite may write into.
    let target = ps_db::mod_targets::upsert(
        &driver,
        &ps_db::mod_targets::NewModTarget {
            id: "client-target".to_string(),
            kind: "client".to_string(),
            name: "Client".to_string(),
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
    common::mods::default_profile(&driver, &target.id).await;
    let paths = LibraryPaths::new(server._temp_dir.path());
    install_fixture_mod(
        &driver,
        &paths,
        "coolmod",
        "1.0",
        &[("ue4ss", "CoolMod/Scripts/main.lua", b"x")],
    )
    .await;
    enable(&driver, &target.id, "coolmod", None, true).await;

    let target_id = target.id.clone();

    server
        .handle
        .services
        .bridge
        .set_target(Some(ps_server::bridge::service::BridgeTarget {
            id: format!("auto:{}", std::process::id()),
            name: "Mock".to_string(),
            host: addr.ip().to_string(),
            port: addr.port(),
            token: "secret-token".to_string(),
        }));

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    // Waits for a pass that ran after the build-info probe landed: an earlier
    // pass can publish a result with `build_info: None` while the probe is
    // still in flight, and that pass never merges `detected.amity`.
    let verification = loop {
        if let Some(v) = server.handle.services.verification.get(&target_id) {
            if v.live && !v.status.is_empty() && v.build_info.is_some() {
                break v;
            }
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("timed out waiting for a verification result with build info");
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    };

    assert!(verification.live);
    let coolmod = verification
        .status
        .iter()
        .find(|m| m.name == "CoolMod")
        .unwrap();
    assert_eq!(
        coolmod.status,
        ps_server::services::mods::verify::VerifyStatus::Verified
    );
    let stray = verification
        .status
        .iter()
        .find(|m| m.name == "Stray")
        .unwrap();
    assert_eq!(
        stray.status,
        ps_server::services::mods::verify::VerifyStatus::Unexpected
    );
    assert!(verification.resolution.complete);

    // `set_detected` runs after `publish` within the same pass, so the write
    // can still be in flight when this test observes the published result.
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let stored_target = ps_db::mod_targets::get(&driver, &target_id)
            .await
            .unwrap()
            .unwrap();
        let detected: serde_json::Value = serde_json::from_str(&stored_target.detected).unwrap();
        if detected["amity"]["amityVersion"] == "0.3.0" {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("timed out waiting for the target's detected amity info to be persisted");
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    common::kill_mock(mock_cancel, mock_handle).await;

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Some(v) = server.handle.services.verification.get(&target_id) {
            if !v.live {
                break;
            }
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("timed out waiting for the entry to go offline");
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    server.handle.shutdown().await;
}

#[tokio::test]
async fn shutdown_does_not_wait_on_an_in_flight_verifier_pass() {
    let _env = common::BridgeEnvGuard::acquire(&[
        ("PS_BRIDGE_ENDPOINT_DIR", None),
        ("PS_VERIFY_INTERVAL_SECS", Some("1")),
    ])
    .await;

    let mock = common::mock_mod::MockMod::new(
        "secret-token",
        serde_json::json!({}),
        serde_json::json!([]),
    )
    .with_silent("get_loaded_mods");
    let requests_log = mock.requests.clone();
    let (addr, mock_cancel, mock_handle) = common::spawn_mock_mod(mock).await;

    let server = start_test_server().await;

    let root = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let db_path = server._temp_dir.path().join("ps-rs.db");
    let pool = ps_db::open(&db_path).await.unwrap();
    let driver = ps_db::SqlxSqliteDriver::new(pool);
    let target = ps_db::mod_targets::upsert(
        &driver,
        &ps_db::mod_targets::NewModTarget {
            id: "client-target".to_string(),
            kind: "client".to_string(),
            name: "Client".to_string(),
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
    common::mods::default_profile(&driver, &target.id).await;

    server
        .handle
        .services
        .bridge
        .set_target(Some(ps_server::bridge::service::BridgeTarget {
            id: format!("auto:{}", std::process::id()),
            name: "Mock".to_string(),
            host: addr.ip().to_string(),
            port: addr.port(),
            token: "secret-token".to_string(),
        }));

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if requests_log
            .lock()
            .unwrap()
            .iter()
            .any(|request| request["type"] == "get_loaded_mods")
        {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("timed out waiting for the verifier to start a pass");
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    let started = tokio::time::Instant::now();
    server.handle.shutdown().await;
    let elapsed = started.elapsed();
    assert!(
        elapsed < std::time::Duration::from_secs(3),
        "shutdown waited on the in-flight pass: {elapsed:?}"
    );

    common::kill_mock(mock_cancel, mock_handle).await;
}

#[tokio::test]
async fn a_concurrent_detect_refresh_mid_pass_is_not_clobbered() {
    let _env = common::BridgeEnvGuard::acquire(&[
        ("PS_BRIDGE_ENDPOINT_DIR", None),
        ("PS_VERIFY_INTERVAL_SECS", Some("1")),
    ])
    .await;

    let mock = common::mock_mod::MockMod::new(
        "secret-token",
        serde_json::json!({}),
        serde_json::json!([]),
    )
    .with_build_info(fixture_data(BUILD_INFO_FIXTURE))
    .with_loaded_mods(serde_json::json!({ "ue4ss": [] }))
    .with_loaded_mods_delay(std::time::Duration::from_millis(300))
    .with_resolution_report(fixture_data(RESOLUTION_REPORT_FIXTURE));
    let requests_log = mock.requests.clone();
    let (addr, mock_cancel, mock_handle) = common::spawn_mock_mod(mock).await;

    let server = start_test_server().await;

    let root = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let db_path = server._temp_dir.path().join("ps-rs.db");
    let pool = ps_db::open(&db_path).await.unwrap();
    let driver = ps_db::SqlxSqliteDriver::new(pool);
    let target = ps_db::mod_targets::upsert(
        &driver,
        &ps_db::mod_targets::NewModTarget {
            id: "client-target".to_string(),
            kind: "client".to_string(),
            name: "Client".to_string(),
            root_path: root.to_string_lossy().into_owned(),
            platform: "win64".to_string(),
            ue4ss_mode: "standard".to_string(),
            layout_overrides: "{}".to_string(),
            detected: r#"{"source":"steam","platform":"win64","ue4ss_mode":"standard","hazards":["orig"]}"#.to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    common::mods::default_profile(&driver, &target.id).await;
    let target_id = target.id.clone();

    server
        .handle
        .services
        .bridge
        .set_target(Some(ps_server::bridge::service::BridgeTarget {
            id: format!("auto:{}", std::process::id()),
            name: "Mock".to_string(),
            host: addr.ip().to_string(),
            port: addr.port(),
            token: "secret-token".to_string(),
        }));

    // The mock holds its `get_loaded_mods` reply for 300ms, opening a window
    // between run_pass's initial target fetch and its final merge.
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if requests_log
            .lock()
            .unwrap()
            .iter()
            .any(|request| request["type"] == "get_loaded_mods")
        {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("timed out waiting for the pass to start");
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    ps_db::mod_targets::set_detected(
        &driver,
        &target_id,
        r#"{"source":"steam","platform":"win64","ue4ss_mode":"standard","hazards":["updated"]}"#,
    )
    .await
    .unwrap();

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    let detected = loop {
        let stored_target = ps_db::mod_targets::get(&driver, &target_id)
            .await
            .unwrap()
            .unwrap();
        let detected: serde_json::Value = serde_json::from_str(&stored_target.detected).unwrap();
        if detected.get("amity").is_some() {
            break detected;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("timed out waiting for the pass to merge amity in");
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    };

    assert_eq!(detected["hazards"], serde_json::json!(["updated"]));
    assert_eq!(detected["amity"]["amityVersion"], "0.3.0");

    common::kill_mock(mock_cancel, mock_handle).await;
    server.handle.shutdown().await;
}

/// A target row bound-able from this test process's own executable path,
/// `up` parent directories above it, without writing an install tree into the
/// real cargo target directory.
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

#[tokio::test]
async fn retargeting_to_a_more_specific_root_marks_the_old_target_offline() {
    let _env = common::BridgeEnvGuard::acquire(&[
        ("PS_BRIDGE_ENDPOINT_DIR", None),
        ("PS_VERIFY_INTERVAL_SECS", Some("1")),
    ])
    .await;

    let mock = common::mock_mod::MockMod::new(
        "secret-token",
        serde_json::json!({}),
        serde_json::json!([]),
    )
    .with_build_info(fixture_data(BUILD_INFO_FIXTURE))
    .with_loaded_mods(serde_json::json!({ "ue4ss": [] }))
    .with_resolution_report(fixture_data(RESOLUTION_REPORT_FIXTURE));
    let (addr, mock_cancel, mock_handle) = common::spawn_mock_mod(mock).await;

    let server = start_test_server().await;
    let db_path = server._temp_dir.path().join("ps-rs.db");
    let pool = ps_db::open(&db_path).await.unwrap();
    let driver = ps_db::SqlxSqliteDriver::new(pool);

    // Two parents up from the running test exe: the outer, less specific root.
    let outer_id = bindable_target(&driver, "outer-target", 2).await;

    server
        .handle
        .services
        .bridge
        .set_target(Some(ps_server::bridge::service::BridgeTarget {
            id: format!("auto:{}", std::process::id()),
            name: "Mock".to_string(),
            host: addr.ip().to_string(),
            port: addr.port(),
            token: "secret-token".to_string(),
        }));

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Some(v) = server.handle.services.verification.get(&outer_id) {
            if v.live {
                break;
            }
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("timed out waiting for the outer target to go live");
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    // One parent up: a longer, more specific root that also contains the exe.
    // The next pass's binding must prefer this one over the outer target.
    let inner_id = bindable_target(&driver, "inner-target", 1).await;

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let outer_offline = server
            .handle
            .services
            .verification
            .get(&outer_id)
            .is_some_and(|v| !v.live);
        let inner_live = server
            .handle
            .services
            .verification
            .get(&inner_id)
            .is_some_and(|v| v.live);
        if outer_offline && inner_live {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("timed out waiting for the retarget: outer_offline={outer_offline} inner_live={inner_live}");
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    common::kill_mock(mock_cancel, mock_handle).await;
    server.handle.shutdown().await;
}
