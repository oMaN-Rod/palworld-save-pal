mod common;

use serde_json::json;

#[tokio::test]
async fn listing_targets_on_a_fresh_install_is_an_empty_list() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;

    common::send_json(&mut ws, json!({"type": "mod_target_list", "data": null})).await;
    let reply = common::next_json(&mut ws).await;

    assert_eq!(reply["type"], "mod_target_list");
    assert_eq!(reply["data"]["targets"], json!([]));
    server.handle.shutdown().await;
}

#[tokio::test]
async fn adding_a_target_by_path_lists_it_with_its_resolved_layout() {
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
    assert_eq!(added["type"], "mod_target_add");
    assert_eq!(added["data"]["target"]["name"], "Test Install");
    let target_id = added["data"]["target"]["id"].as_str().unwrap().to_string();
    assert!(
        added["data"]["target"]["layout"]["paks_mods_dir"]
            .as_str()
            .unwrap()
            .ends_with("~mods"),
        "the reply carries a resolved layout: {}",
        added["data"]["target"]["layout"]
    );

    common::send_json(&mut ws, json!({"type": "mod_target_list", "data": null})).await;
    let listed = common::next_json(&mut ws).await;
    assert_eq!(listed["data"]["targets"][0]["id"], target_id);
    server.handle.shutdown().await;
}

#[tokio::test]
async fn adding_a_path_that_is_not_an_install_is_refused_by_name() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let empty = tempfile::tempdir().unwrap();

    common::send_json(
        &mut ws,
        json!({"type": "mod_target_add", "data": {
            "root_path": empty.path().to_string_lossy(),
        }}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;

    assert_eq!(
        reply["type"], "mod_target_add",
        "a refusal replies under its own type; an error frame would send the UI to /error"
    );
    assert_eq!(reply["data"]["error"]["code"], "not_an_install");
    let message = reply["data"]["error"]["message"].as_str().unwrap();
    assert!(
        message.contains(&empty.path().display().to_string()),
        "the refusal names the path it refused: {message}"
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn an_unknown_target_is_refused_under_the_request_type() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;

    common::send_json(
        &mut ws,
        json!({"type": "mod_target_scan", "data": {"target_id": "client-nope"}}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;

    assert_eq!(reply["type"], "mod_target_scan");
    assert_eq!(reply["data"]["target_id"], "client-nope");
    assert_eq!(reply["data"]["error"]["code"], "target_not_found");
    server.handle.shutdown().await;
}

#[tokio::test]
async fn removing_a_target_removes_it_from_the_list() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();

    common::send_json(
        &mut ws,
        json!({"type": "mod_target_add", "data": {
            "root_path": install.path().to_string_lossy(),
        }}),
    )
    .await;
    let target_id = common::next_json(&mut ws).await["data"]["target"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    common::send_json(
        &mut ws,
        json!({"type": "mod_target_remove", "data": {"target_id": target_id}}),
    )
    .await;
    let removed = common::next_json(&mut ws).await;
    assert_eq!(removed["data"]["removed"], true);

    common::send_json(&mut ws, json!({"type": "mod_target_list", "data": null})).await;
    assert_eq!(
        common::next_json(&mut ws).await["data"]["targets"],
        json!([])
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn removing_a_target_drops_its_verification_entry() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();

    common::send_json(
        &mut ws,
        json!({"type": "mod_target_add", "data": {
            "root_path": install.path().to_string_lossy(),
        }}),
    )
    .await;
    let target_id = common::next_json(&mut ws).await["data"]["target"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    server.handle.services.verification.publish(
        ps_server::services::mods::verify::TargetVerification {
            target_id: target_id.clone(),
            instance_id: "auto:1".to_string(),
            live: true,
            checked_at: "2026-09-15T00:00:00Z".to_string(),
            build_info: None,
            resolution: ps_server::services::mods::verify::Resolution {
                complete: true,
                missing: vec![],
            },
            status: vec![],
        },
    );
    assert!(server
        .handle
        .services
        .verification
        .get(&target_id)
        .is_some());

    common::send_json(
        &mut ws,
        json!({"type": "mod_target_remove", "data": {"target_id": target_id}}),
    )
    .await;
    let removed = common::next_json(&mut ws).await;
    assert_eq!(removed["data"]["removed"], true);

    assert!(server
        .handle
        .services
        .verification
        .get(&target_id)
        .is_none());
    server.handle.shutdown().await;
}

#[tokio::test]
async fn removing_a_server_target_is_refused_by_name() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let driver = &*server.handle.app.driver;

    let record = ps_db::servers::create_server(
        driver,
        ps_db::servers::NewServer {
            name: "Alpha".to_string(),
            container_name: "alpha".to_string(),
            server_type: "docker".to_string(),
            mods_path: "/srv/alpha/mods".to_string(),
            logicmods_path: "/srv/alpha/logicmods".to_string(),
            nativemods_path: "/srv/alpha/nativemods".to_string(),
            paks_path: "/srv/alpha/paks".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let app_root = tempfile::tempdir().unwrap();
    let target_id = ps_server::mod_target_service::ensure_for(driver, &record, app_root.path())
        .await
        .unwrap()
        .expect("a server target is created for a fresh server row");

    common::send_json(
        &mut ws,
        json!({"type": "mod_target_remove", "data": {"target_id": target_id}}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;

    assert_eq!(reply["type"], "mod_target_remove");
    assert_eq!(reply["data"]["error"]["code"], "server_target");
    let message = reply["data"]["error"]["message"].as_str().unwrap();
    assert!(
        message.contains(&target_id),
        "the refusal names the target it refused: {message}"
    );

    let still_there = ps_db::mod_targets::get(driver, &target_id).await.unwrap();
    assert!(
        still_there.is_some(),
        "the server target must survive the refused removal"
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn scanning_a_target_reports_an_unmanaged_folder_as_an_adoption_candidate() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    common::mods_ws::write_ue4ss_mod(install.path(), "CoolMod", "main.lua", b"print hi");

    common::send_json(
        &mut ws,
        json!({"type": "mod_target_add", "data": {
            "root_path": install.path().to_string_lossy(),
        }}),
    )
    .await;
    let target_id = common::next_json(&mut ws).await["data"]["target"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    common::send_json(
        &mut ws,
        json!({"type": "mod_target_scan", "data": {"target_id": target_id}}),
    )
    .await;
    let scanned = common::next_json(&mut ws).await;
    assert_eq!(scanned["type"], "mod_target_scan");
    let candidates = scanned["data"]["candidates"].as_array().unwrap();
    assert_eq!(candidates.len(), 1, "{candidates:?}");
    assert_eq!(candidates[0]["name"], "CoolMod");
    server.handle.shutdown().await;
}

#[tokio::test]
async fn adopting_a_candidate_leaves_the_files_exactly_where_they_were() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    common::mods_ws::write_ue4ss_mod(install.path(), "CoolMod", "main.lua", b"print hi");
    let deployed = common::mods_ws::ue4ss_mods_dir(install.path()).join("CoolMod/main.lua");

    common::send_json(
        &mut ws,
        json!({"type": "mod_target_add", "data": {
            "root_path": install.path().to_string_lossy(),
        }}),
    )
    .await;
    let target_id = common::next_json(&mut ws).await["data"]["target"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    common::send_json(
        &mut ws,
        json!({"type": "mod_adopt", "data": {
            "target_id": target_id, "candidate_name": "CoolMod",
        }}),
    )
    .await;
    let adopted = common::next_json(&mut ws).await;
    assert_eq!(adopted["type"], "mod_adopt");
    assert_eq!(adopted["data"]["files"], 1);
    assert_eq!(
        std::fs::read_to_string(&deployed).unwrap(),
        "print hi",
        "adoption changes nothing the game sees"
    );
    server.handle.shutdown().await;
}
