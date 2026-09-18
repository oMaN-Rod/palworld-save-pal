mod common;

use serde_json::json;

#[tokio::test]
async fn framework_status_reports_ue4ss_and_the_amity_legacy_hazard() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    common::mods_ws::write_ue4ss_mod(install.path(), "PSPAmity", "marker.txt", b"legacy");

    common::send_json(
        &mut ws,
        json!({"type": "mod_target_add", "data": {
            "root_path": install.path().to_string_lossy(),
            "name": "Test Install",
        }}),
    )
    .await;
    let added = common::next_json(&mut ws).await;
    assert_eq!(added["type"], "mod_target_add");
    let target_id = added["data"]["target"]["id"].as_str().unwrap().to_string();

    common::send_json(
        &mut ws,
        json!({"type": "framework_status", "data": { "target_id": target_id }}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;

    assert_eq!(reply["type"], "framework_status");
    assert!(reply["data"]["error"].is_null(), "{reply:?}");
    let frameworks = reply["data"]["frameworks"].as_array().unwrap();
    let keys: Vec<&str> = frameworks
        .iter()
        .map(|entry| entry["key"].as_str().unwrap())
        .collect();
    assert_eq!(keys, ["ue4ss", "palschema", "amity"]);

    let ue4ss = &frameworks[0];
    assert_eq!(ue4ss["installed"]["present"], true, "{ue4ss:?}");
    for entry in frameworks {
        assert!(entry["latest"].is_null(), "{entry:?}");
    }

    let hazards = reply["data"]["hazards"].as_array().unwrap();
    assert!(
        hazards
            .iter()
            .any(|hazard| hazard["code"] == "amity_legacy_folder"),
        "{hazards:?}"
    );

    server.handle.shutdown().await;
}

#[tokio::test]
async fn installing_a_library_framework_deploys_it() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();

    common::send_json(
        &mut ws,
        json!({"type": "mod_target_add", "data": {
            "root_path": install.path().to_string_lossy(),
            "name": "Test Install",
        }}),
    )
    .await;
    let added = common::next_json(&mut ws).await;
    let target_id = added["data"]["target"]["id"].as_str().unwrap().to_string();

    let db = ps_db::SqlxSqliteDriver::new(
        ps_db::open(&server._temp_dir.path().join("ps-rs.db"))
            .await
            .unwrap(),
    );
    let library = ps_server::services::mods::LibraryPaths::new(server._temp_dir.path());
    common::mods::install_framework(
        &db,
        &library,
        "framework-amity",
        "0.2.0",
        &[
            ("ue4ss", "PSAmity/dlls/main.dll", b"dll"),
            ("ue4ss", "PSAmity/enabled.txt", b""),
        ],
    )
    .await;

    common::send_json(
        &mut ws,
        json!({"type": "framework_install", "data": {
            "target_id": target_id,
            "key": "amity",
            "mod_version_id": "framework-amity@0.2.0",
        }}),
    )
    .await;
    let (reply, _) = common::mods_ws::next_of_type(&mut ws, "framework_install").await;

    assert!(reply["data"]["error"].is_null(), "{reply:?}");
    assert_eq!(reply["data"]["installed_new"], false, "{reply:?}");
    let bytes = std::fs::read(
        install
            .path()
            .join("Pal/Binaries/Win64/ue4ss/Mods/PSAmity/dlls/main.dll"),
    )
    .unwrap();
    assert_eq!(bytes, b"dll");

    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_framework_cannot_be_enabled_as_a_profile_mod_and_can_be_removed() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();

    common::send_json(
        &mut ws,
        json!({"type": "mod_target_add", "data": {
            "root_path": install.path().to_string_lossy(),
            "name": "Test Install",
        }}),
    )
    .await;
    let added = common::next_json(&mut ws).await;
    let target_id = added["data"]["target"]["id"].as_str().unwrap().to_string();

    let db = ps_db::SqlxSqliteDriver::new(
        ps_db::open(&server._temp_dir.path().join("ps-rs.db"))
            .await
            .unwrap(),
    );
    let library = ps_server::services::mods::LibraryPaths::new(server._temp_dir.path());
    common::mods::install_framework(
        &db,
        &library,
        "framework-amity",
        "0.2.0",
        &[
            ("ue4ss", "PSAmity/dlls/main.dll", b"dll"),
            ("ue4ss", "PSAmity/enabled.txt", b""),
        ],
    )
    .await;

    common::send_json(
        &mut ws,
        json!({"type": "framework_install", "data": {
            "target_id": target_id,
            "key": "amity",
            "mod_version_id": "framework-amity@0.2.0",
        }}),
    )
    .await;
    let (install_reply, _) = common::mods_ws::next_of_type(&mut ws, "framework_install").await;
    assert!(
        install_reply["data"]["error"].is_null(),
        "{install_reply:?}"
    );

    common::send_json(
        &mut ws,
        json!({"type": "profile_set_mod", "data": {
            "target_id": target_id,
            "mod_id": "framework-amity",
            "enabled": true,
        }}),
    )
    .await;
    let (set_mod_reply, _) = common::mods_ws::next_of_type(&mut ws, "profile_set_mod").await;
    assert_eq!(
        set_mod_reply["data"]["error"]["code"], "framework_mod",
        "{set_mod_reply:?}"
    );

    common::send_json(
        &mut ws,
        json!({"type": "framework_remove", "data": {
            "target_id": target_id,
            "key": "amity",
        }}),
    )
    .await;
    let (remove_reply, _) = common::mods_ws::next_of_type(&mut ws, "framework_remove").await;

    assert!(remove_reply["data"]["error"].is_null(), "{remove_reply:?}");
    assert_eq!(remove_reply["data"]["removed"], true, "{remove_reply:?}");
    assert!(!install
        .path()
        .join("Pal/Binaries/Win64/ue4ss/Mods/PSAmity/dlls/main.dll")
        .exists());

    server.handle.shutdown().await;
}

#[tokio::test]
async fn an_unknown_framework_key_is_refused() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();

    common::send_json(
        &mut ws,
        json!({"type": "mod_target_add", "data": {
            "root_path": install.path().to_string_lossy(),
            "name": "Test Install",
        }}),
    )
    .await;
    let added = common::next_json(&mut ws).await;
    let target_id = added["data"]["target"]["id"].as_str().unwrap().to_string();

    common::send_json(
        &mut ws,
        json!({"type": "framework_install", "data": {
            "target_id": target_id,
            "key": "nope",
        }}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;

    assert_eq!(
        reply["data"]["error"]["code"], "invalid_framework",
        "{reply:?}"
    );

    server.handle.shutdown().await;
}
