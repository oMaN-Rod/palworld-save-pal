mod common;

use std::path::Path;

use serde_json::json;

use ps_core::mods::pak_index::{test_pak, test_pak_version, TestPakVersion};
use ps_db::mod_targets::{ModTarget, NewModTarget};
use ps_server::services::mods::LibraryPaths;

async fn driver_for(server: &common::TestServer) -> ps_db::SqlxSqliteDriver {
    ps_db::SqlxSqliteDriver::new(
        ps_db::open(&server._temp_dir.path().join("ps-rs.db"))
            .await
            .unwrap(),
    )
}

fn library_for(server: &common::TestServer) -> LibraryPaths {
    LibraryPaths::new(server._temp_dir.path())
}

async fn platform_target(
    db: &ps_db::SqlxSqliteDriver,
    id: &str,
    root: &Path,
    platform: &str,
) -> ModTarget {
    common::mods::install_tree(root);
    ps_db::mod_targets::upsert(
        db,
        &NewModTarget {
            id: id.to_string(),
            kind: "client".to_string(),
            name: id.to_string(),
            root_path: root.to_string_lossy().into_owned(),
            platform: platform.to_string(),
            ue4ss_mode: "standard".to_string(),
            layout_overrides: "{}".to_string(),
            detected: "{}".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap()
}

async fn ready_target(
    db: &ps_db::SqlxSqliteDriver,
    id: &str,
    root: &Path,
    platform: &str,
) -> String {
    let target = platform_target(db, id, root, platform).await;
    common::mods::default_profile(db, &target.id).await;
    target.id
}

async fn conflicts(
    ws: &mut common::WsClient,
    target_id: &str,
    profile_id: Option<&str>,
) -> serde_json::Value {
    common::send_json(
        ws,
        json!({"type": "mod_conflicts", "data": {
            "target_id": target_id,
            "profile_id": profile_id,
        }}),
    )
    .await;
    let reply = common::next_json(ws).await;
    assert_eq!(reply["type"], "mod_conflicts", "{reply:?}");
    reply["data"].clone()
}

fn conflicts_of_kind<'a>(data: &'a serde_json::Value, kind: &str) -> Vec<&'a serde_json::Value> {
    data["conflicts"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"] == kind)
        .collect()
}

#[tokio::test]
async fn overlapping_paks_report_one_conflict_that_disabling_a_mod_clears() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let db = driver_for(&server).await;
    let paths = library_for(&server);
    let root = tempfile::tempdir().unwrap();
    let target_id = ready_target(&db, "client-pak", root.path(), "win64").await;

    let pak_a = test_pak("../../../", &["Pal/Content/X.uasset", "Pal/Content/X.uexp"]);
    let pak_b = test_pak("../../../", &["Pal/Content/X.uasset", "Pal/Content/X.uexp"]);
    common::mods::install_fixture_mod(&db, &paths, "moda", "1.0", &[("pak", "A_P.pak", &pak_a)])
        .await;
    common::mods::install_fixture_mod(&db, &paths, "modb", "1.0", &[("pak", "Z_P.pak", &pak_b)])
        .await;
    common::mods::enable(&db, &target_id, "moda", None, true).await;
    common::mods::enable(&db, &target_id, "modb", None, true).await;

    let data = conflicts(&mut ws, &target_id, None).await;
    assert!(data["error"].is_null(), "{data:?}");
    assert_eq!(data["target_id"], target_id);
    assert_eq!(data["profile_id"], format!("{target_id}/default"));
    assert_eq!(data["platform"], "win64");

    let overlaps = conflicts_of_kind(&data, "pak_overlap");
    assert_eq!(overlaps.len(), 1, "{data:?}");
    let overlap = overlaps[0];
    assert_eq!(overlap["asset_count"], 1);
    assert_eq!(overlap["assets"], json!(["pal/content/x.uasset"]));
    assert_eq!(overlap["winner"]["mod_id"], "modb");
    assert_eq!(overlap["winner"]["file"], "Z_P.pak");
    assert_eq!(overlap["winner"]["source"], "library");
    assert!(overlap["winner"]["path"].is_null(), "{overlap:?}");

    common::mods::enable(&db, &target_id, "modb", None, false).await;
    let data = conflicts(&mut ws, &target_id, None).await;
    assert!(
        conflicts_of_kind(&data, "pak_overlap").is_empty(),
        "{data:?}"
    );

    server.handle.shutdown().await;
}

#[tokio::test]
async fn an_iostore_pak_is_skipped_and_a_garbage_pak_is_reported_unreadable() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let db = driver_for(&server).await;
    let paths = library_for(&server);
    let root = tempfile::tempdir().unwrap();
    let target_id = ready_target(&db, "client-io", root.path(), "win64").await;

    common::mods::install_fixture_mod(
        &db,
        &paths,
        "iomod",
        "1.0",
        &[
            ("pak", "IoMod_P.pak", b"not a real pak body"),
            ("pak", "IoMod_P.utoc", b"utoc"),
            ("pak", "IoMod_P.ucas", b"ucas"),
        ],
    )
    .await;
    common::mods::install_fixture_mod(
        &db,
        &paths,
        "garbagemod",
        "1.0",
        &[("pak", "Garbage_P.pak", b"definitely not a pak")],
    )
    .await;
    common::mods::enable(&db, &target_id, "iomod", None, true).await;
    common::mods::enable(&db, &target_id, "garbagemod", None, true).await;

    let data = conflicts(&mut ws, &target_id, None).await;
    assert!(data["error"].is_null(), "{data:?}");
    let unreadable = data["unreadable"].as_array().unwrap();
    assert!(
        unreadable.iter().any(|u| u["mod_id"] == "iomod"
            && u["file"] == "IoMod_P.pak"
            && u["reason"] == "iostore"
            && u["source"] == "library"),
        "{unreadable:?}"
    );
    assert!(
        unreadable.iter().any(|u| u["mod_id"] == "garbagemod"
            && u["file"] == "Garbage_P.pak"
            && u["reason"] == "not_a_pak"
            && u["source"] == "library"),
        "{unreadable:?}"
    );

    server.handle.shutdown().await;
}

#[tokio::test]
async fn palschema_mods_sharing_a_row_are_reported_and_invalid_json_is_unreadable() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let db = driver_for(&server).await;
    let paths = library_for(&server);
    let root = tempfile::tempdir().unwrap();
    let target_id = ready_target(&db, "client-schema", root.path(), "win64").await;

    let jsonc_with_bom = "\u{FEFF}{\n  // shares DT_A::R1\n  \"DT_A\": {\"R1\": {}}\n}";
    common::mods::install_fixture_mod(
        &db,
        &paths,
        "schemaa",
        "1.0",
        &[(
            "palschema",
            "SchemaA/raw/DT_A.json",
            br#"{"DT_A": {"R1": {}}}"#,
        )],
    )
    .await;
    common::mods::install_fixture_mod(
        &db,
        &paths,
        "schemab",
        "1.0",
        &[(
            "palschema",
            "SchemaB/raw/DT_A.jsonc",
            jsonc_with_bom.as_bytes(),
        )],
    )
    .await;
    common::mods::install_fixture_mod(
        &db,
        &paths,
        "schemac",
        "1.0",
        &[("palschema", "SchemaC/raw/DT_B.json", b"{not json")],
    )
    .await;
    common::mods::enable(&db, &target_id, "schemaa", None, true).await;
    common::mods::enable(&db, &target_id, "schemab", None, true).await;
    common::mods::enable(&db, &target_id, "schemac", None, true).await;

    let data = conflicts(&mut ws, &target_id, None).await;
    assert!(data["error"].is_null(), "{data:?}");

    let rows = conflicts_of_kind(&data, "palschema_row");
    assert_eq!(rows.len(), 1, "{data:?}");
    assert_eq!(rows[0]["key"], "DT_A::R1");
    let mods = rows[0]["mods"].as_array().unwrap();
    assert_eq!(mods.len(), 2, "{mods:?}");
    let mod_ids: Vec<&str> = mods.iter().map(|m| m["mod_id"].as_str().unwrap()).collect();
    assert!(mod_ids.contains(&"schemaa"), "{mod_ids:?}");
    assert!(mod_ids.contains(&"schemab"), "{mod_ids:?}");

    let unreadable = data["unreadable"].as_array().unwrap();
    assert!(
        unreadable.iter().any(|u| u["mod_id"] == "schemac"
            && u["file"] == "SchemaC/raw/DT_B.json"
            && u["reason"] == "invalid_json"),
        "{unreadable:?}"
    );

    server.handle.shutdown().await;
}

#[tokio::test]
async fn an_enabled_ue4ss_mod_is_missing_until_ue4ss_is_present() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let db = driver_for(&server).await;
    let paths = library_for(&server);
    let root = tempfile::tempdir().unwrap();
    let target_id = ready_target(&db, "client-ue4ss", root.path(), "win64").await;

    common::mods::install_fixture_mod(
        &db,
        &paths,
        "coolmod",
        "1.0",
        &[("ue4ss", "CoolMod/Scripts/main.lua", b"return {}")],
    )
    .await;
    common::mods::enable(&db, &target_id, "coolmod", None, true).await;

    let data = conflicts(&mut ws, &target_id, None).await;
    assert!(data["error"].is_null(), "{data:?}");
    let missing = conflicts_of_kind(&data, "missing_dependency");
    assert!(
        missing.iter().any(|c| c["mod_id"] == "coolmod"
            && c["dependency"] == "UE4SS"
            && c["source"] == "mod_type"),
        "{missing:?}"
    );

    common::mods::write(
        &root.path().join("Pal/Binaries/Win64/ue4ss/UE4SS.dll"),
        b"x",
    );
    let data = conflicts(&mut ws, &target_id, None).await;
    let missing = conflicts_of_kind(&data, "missing_dependency");
    assert!(
        !missing.iter().any(|c| c["mod_id"] == "coolmod"),
        "{missing:?}"
    );

    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_workshop_mods_unmet_package_dependency_is_reported() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let db = driver_for(&server).await;
    let paths = library_for(&server);
    let root = tempfile::tempdir().unwrap();
    let target_id = ready_target(&db, "client-wsdep", root.path(), "win64").await;

    let info = br#"{"PackageName":"WorkshopPkg","ModName":"W","Dependencies":["OtherPkg"]}"#;
    common::mods::install_fixture_mod(
        &db,
        &paths,
        "wsmod",
        "1.0",
        &[("workshop", "WorkshopPkg/Info.json", info)],
    )
    .await;
    common::mods::enable(&db, &target_id, "wsmod", None, true).await;

    let data = conflicts(&mut ws, &target_id, None).await;
    assert!(data["error"].is_null(), "{data:?}");
    let missing = conflicts_of_kind(&data, "missing_dependency");
    assert!(
        missing.iter().any(|c| c["mod_id"] == "wsmod"
            && c["dependency"] == "OtherPkg"
            && c["source"] == "workshop_info"),
        "{missing:?}"
    );

    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_workshop_mods_invalid_info_json_is_unreadable_and_others_still_report() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let db = driver_for(&server).await;
    let paths = library_for(&server);
    let root = tempfile::tempdir().unwrap();
    let target_id = ready_target(&db, "client-wsjson", root.path(), "win64").await;

    common::mods::install_fixture_mod(
        &db,
        &paths,
        "brokenws",
        "1.0",
        &[("workshop", "BrokenPkg/Info.json", b"{not json")],
    )
    .await;
    let info = br#"{"PackageName":"WorkshopPkg","ModName":"W","Dependencies":["OtherPkg"]}"#;
    common::mods::install_fixture_mod(
        &db,
        &paths,
        "wsmod",
        "1.0",
        &[("workshop", "WorkshopPkg/Info.json", info)],
    )
    .await;
    common::mods::enable(&db, &target_id, "brokenws", None, true).await;
    common::mods::enable(&db, &target_id, "wsmod", None, true).await;

    let data = conflicts(&mut ws, &target_id, None).await;
    assert!(data["error"].is_null(), "{data:?}");

    let unreadable = data["unreadable"].as_array().unwrap();
    assert!(
        unreadable.iter().any(|u| u["mod_id"] == "brokenws"
            && u["file"] == "Info.json"
            && u["reason"] == "invalid_json"),
        "{unreadable:?}"
    );

    let missing = conflicts_of_kind(&data, "missing_dependency");
    assert!(
        missing.iter().any(|c| c["mod_id"] == "wsmod"
            && c["dependency"] == "OtherPkg"
            && c["source"] == "workshop_info"),
        "{missing:?}"
    );

    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_legacy_pak_is_gamepass_incompatible_only_on_wingdk() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let db = driver_for(&server).await;
    let paths = library_for(&server);
    let gdk_root = tempfile::tempdir().unwrap();
    let win_root = tempfile::tempdir().unwrap();
    let gdk_id = ready_target(&db, "client-gdk", gdk_root.path(), "wingdk").await;
    let win_id = ready_target(&db, "client-win64", win_root.path(), "win64").await;

    let pak = test_pak("../../../", &["Pal/Content/Y.uasset"]);
    common::mods::install_fixture_mod(
        &db,
        &paths,
        "legacypak",
        "1.0",
        &[("pak", "Legacy_P.pak", &pak)],
    )
    .await;
    common::mods::enable(&db, &gdk_id, "legacypak", None, true).await;
    common::mods::enable(&db, &win_id, "legacypak", None, true).await;

    let gdk_data = conflicts(&mut ws, &gdk_id, None).await;
    assert!(gdk_data["error"].is_null(), "{gdk_data:?}");
    let gamepass = conflicts_of_kind(&gdk_data, "gamepass_pak_incompatible");
    assert_eq!(gamepass.len(), 1, "{gdk_data:?}");
    assert_eq!(gamepass[0]["mod_id"], "legacypak");
    assert_eq!(gamepass[0]["files"], json!(["Legacy_P.pak"]));

    let win_data = conflicts(&mut ws, &win_id, None).await;
    assert!(win_data["error"].is_null(), "{win_data:?}");
    assert!(
        conflicts_of_kind(&win_data, "gamepass_pak_incompatible").is_empty(),
        "{win_data:?}"
    );

    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_game_tree_archive_pak_is_gamepass_incompatible() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let db = driver_for(&server).await;
    let paths = library_for(&server);
    let root = tempfile::tempdir().unwrap();
    let target_id = ready_target(&db, "client-gametree", root.path(), "wingdk").await;

    common::mods::install_fixture_mod(
        &db,
        &paths,
        "gametreepak",
        "1.0",
        &[(
            "passthrough",
            "Pal/Content/Paks/~mods/Cool_P.pak",
            b"legacy pak bytes",
        )],
    )
    .await;
    common::mods::enable(&db, &target_id, "gametreepak", None, true).await;

    let data = conflicts(&mut ws, &target_id, None).await;
    assert!(data["error"].is_null(), "{data:?}");
    let gamepass = conflicts_of_kind(&data, "gamepass_pak_incompatible");
    assert_eq!(gamepass.len(), 1, "{data:?}");
    assert_eq!(gamepass[0]["mod_id"], "gametreepak");
    assert_eq!(gamepass[0]["files"], json!(["Cool_P.pak"]));

    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_pinned_mod_is_scanned_on_its_pinned_version_not_the_current_one() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let db = driver_for(&server).await;
    let paths = library_for(&server);
    let root = tempfile::tempdir().unwrap();
    let target_id = ready_target(&db, "client-pin", root.path(), "wingdk").await;

    let pak = test_pak("../../../", &["Pal/Content/Pinned.uasset"]);
    let legacy = common::mods::install_fixture_mod(
        &db,
        &paths,
        "pinnedmod",
        "1.0",
        &[("pak", "Pinned_P.pak", &pak)],
    )
    .await;
    let iostore = common::mods::install_fixture_mod(
        &db,
        &paths,
        "pinnedmod",
        "2.0",
        &[
            ("pak", "Pinned_P.pak", &pak),
            ("pak", "Pinned_P.utoc", b"utoc"),
            ("pak", "Pinned_P.ucas", b"ucas"),
        ],
    )
    .await;
    ps_db::mod_library::set_current_version(&db, "pinnedmod", &iostore.version.id)
        .await
        .unwrap();
    common::mods::enable(&db, &target_id, "pinnedmod", Some(&legacy.version.id), true).await;

    let data = conflicts(&mut ws, &target_id, None).await;
    assert!(data["error"].is_null(), "{data:?}");
    let gamepass = conflicts_of_kind(&data, "gamepass_pak_incompatible");
    assert_eq!(gamepass.len(), 1, "{data:?}");
    assert_eq!(gamepass[0]["mod_id"], "pinnedmod");
    assert!(
        data["unreadable"].as_array().unwrap().is_empty(),
        "the pinned legacy version has no iostore sibling: {data:?}"
    );

    server.handle.shutdown().await;
}

#[tokio::test]
async fn an_unknown_target_is_refused() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;

    let data = conflicts(&mut ws, "client-nope", None).await;
    assert_eq!(data["error"]["code"], "target_not_found", "{data:?}");

    server.handle.shutdown().await;
}

#[tokio::test]
async fn another_targets_profile_id_is_refused_as_not_found() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let db = driver_for(&server).await;
    let root_a = tempfile::tempdir().unwrap();
    let root_b = tempfile::tempdir().unwrap();
    let target_a = ready_target(&db, "client-a", root_a.path(), "win64").await;
    let target_b = ready_target(&db, "client-b", root_b.path(), "win64").await;

    let data = conflicts(&mut ws, &target_a, Some(&format!("{target_b}/default"))).await;
    assert_eq!(data["error"]["code"], "profile_not_found", "{data:?}");

    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_target_with_no_active_profile_is_refused() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let db = driver_for(&server).await;
    let root = tempfile::tempdir().unwrap();
    let target = common::mods::client_target(&db, root.path()).await;

    let data = conflicts(&mut ws, &target.id, None).await;
    assert_eq!(data["error"]["code"], "no_active_profile", "{data:?}");

    server.handle.shutdown().await;
}

fn paks(root: &Path) -> std::path::PathBuf {
    root.join("Pal/Content/Paks")
}

fn unreadable_of<'a>(data: &'a serde_json::Value, file: &str) -> Vec<&'a serde_json::Value> {
    data["unreadable"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|u| u["file"] == file)
        .collect()
}

#[tokio::test]
async fn an_untracked_logicmods_pak_overlaps_a_library_pak() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let db = driver_for(&server).await;
    let paths = library_for(&server);
    let root = tempfile::tempdir().unwrap();
    let target_id = ready_target(&db, "client-disk", root.path(), "win64").await;

    let shared = test_pak("../../../", &["Pal/Content/X.uasset"]);
    common::mods::install_fixture_mod(&db, &paths, "moda", "1.0", &[("pak", "A_P.pak", &shared)])
        .await;
    common::mods::enable(&db, &target_id, "moda", None, true).await;
    common::mods::write(&paks(root.path()).join("LogicMods/Stray_P.pak"), &shared);

    let data = conflicts(&mut ws, &target_id, None).await;
    assert!(data["error"].is_null(), "{data:?}");
    let overlaps = conflicts_of_kind(&data, "pak_overlap");
    assert_eq!(overlaps.len(), 1, "{data:?}");
    assert_eq!(
        overlaps[0]["winner"],
        json!({"mod_id": null, "file": "Stray_P.pak", "source": "disk", "path": "LogicMods/Stray_P.pak"})
    );
    assert!(overlaps[0]["paks"].as_array().unwrap().contains(
        &json!({"mod_id": "moda", "file": "A_P.pak", "source": "library", "path": null})
    ));

    server.handle.shutdown().await;
}

#[tokio::test]
async fn only_the_documented_pak_locations_and_depths_are_scanned() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let db = driver_for(&server).await;
    let root = tempfile::tempdir().unwrap();
    let target_id = ready_target(&db, "client-depth", root.path(), "win64").await;
    let base = paks(root.path());
    for rel in [
        "~mods/Top_P.pak",
        "~mods/Sub/Deep_P.pak",
        "~mods/Sub/Deeper/TooDeep_P.pak",
        "~mods/pakchunk99-Windows_P.pak",
        "~mods/Pal-Windows.pak",
        "~mods/readme.txt",
        "LogicMods/Logic_P.pak",
        "LogicMods/Nested/NotScanned_P.pak",
        "~WorkshopMods/Loose_P.pak",
        "~WorkshopMods/Folder/InFolder_P.pak",
        "~WorkshopMods/Folder/Sub/TooDeep2_P.pak",
        "Loose_Top_P.pak",
    ] {
        common::mods::write(&base.join(rel), b"not a pak");
    }

    let data = conflicts(&mut ws, &target_id, None).await;
    assert!(data["error"].is_null(), "{data:?}");
    let mut listed: Vec<&str> = data["unreadable"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| u["path"].as_str().unwrap())
        .collect();
    listed.sort();
    assert_eq!(
        listed,
        vec![
            "LogicMods/Logic_P.pak",
            "~WorkshopMods/Folder/InFolder_P.pak",
            "~mods/Sub/Deep_P.pak",
            "~mods/Top_P.pak",
        ]
    );
    for entry in data["unreadable"].as_array().unwrap() {
        assert_eq!(entry["mod_id"], serde_json::Value::Null, "{entry:?}");
        assert_eq!(entry["source"], "disk", "{entry:?}");
        assert_eq!(entry["reason"], "not_a_pak", "{entry:?}");
    }

    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_pak_recorded_in_deployment_files_is_skipped() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let db = driver_for(&server).await;
    let paths = library_for(&server);
    let root = tempfile::tempdir().unwrap();
    let target_id = ready_target(&db, "client-deployed", root.path(), "win64").await;

    let stored = common::mods::install_fixture_mod(
        &db,
        &paths,
        "deployedmod",
        "1.0",
        &[("pak", "Deployed_P.pak", b"not a pak")],
    )
    .await;
    let on_disk = paks(root.path()).join("~mods/Deployed_P.pak");
    common::mods::write(&on_disk, b"not a pak");
    common::mods::write(&paks(root.path()).join("~mods/Other_P.pak"), b"not a pak");
    ps_db::mod_deployments::record(
        &db,
        &[ps_db::mod_deployments::NewDeploymentFile {
            target_id: target_id.clone(),
            path: on_disk.to_string_lossy().into_owned(),
            mod_version_id: Some(stored.version.id.clone()),
            hash: "x".to_string(),
            role: "file".to_string(),
            rel_path: Some("Deployed_P.pak".to_string()),
        }],
    )
    .await
    .unwrap();

    let data = conflicts(&mut ws, &target_id, None).await;
    assert!(data["error"].is_null(), "{data:?}");
    assert!(
        unreadable_of(&data, "Deployed_P.pak").is_empty(),
        "{data:?}"
    );
    let other = unreadable_of(&data, "Other_P.pak");
    assert_eq!(other.len(), 1, "{data:?}");
    assert_eq!(other[0]["path"], "~mods/Other_P.pak");

    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_workshopmods_folder_is_attributed_only_to_an_enabled_workshop_mod() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let db = driver_for(&server).await;
    let paths = library_for(&server);
    let root = tempfile::tempdir().unwrap();
    let target_id = ready_target(&db, "client-wsfolders", root.path(), "win64").await;

    common::mods::install_fixture_mod(
        &db,
        &paths,
        "workshop-111",
        "steam-subscribed",
        &[(
            "workshop",
            "PerfectPlacement/Info.json",
            br#"{"PackageName":"PerfectPlacement"}"#,
        )],
    )
    .await;
    common::mods::install_fixture_mod(
        &db,
        &paths,
        "workshop-222",
        "steam-subscribed",
        &[(
            "workshop",
            "CreativeMenu/Info.json",
            br#"{"PackageName":"CreativeMenu"}"#,
        )],
    )
    .await;
    common::mods::enable(&db, &target_id, "workshop-111", None, true).await;
    common::mods::enable(&db, &target_id, "workshop-222", None, false).await;
    let base = paks(root.path()).join("~WorkshopMods");
    common::mods::write(&base.join("perfectplacement/PP_P.pak"), b"not a pak");
    common::mods::write(&base.join("CreativeMenu/CM_P.pak"), b"not a pak");
    common::mods::write(&base.join("Unknown/U_P.pak"), b"not a pak");

    let data = conflicts(&mut ws, &target_id, None).await;
    assert!(data["error"].is_null(), "{data:?}");
    let pp = unreadable_of(&data, "PP_P.pak");
    assert_eq!(pp.len(), 1, "{data:?}");
    assert_eq!(pp[0]["mod_id"], "workshop-111");
    assert_eq!(pp[0]["source"], "disk");
    assert_eq!(pp[0]["path"], "~WorkshopMods/perfectplacement/PP_P.pak");
    assert!(unreadable_of(&data, "CM_P.pak").is_empty(), "{data:?}");
    let unknown = unreadable_of(&data, "U_P.pak");
    assert_eq!(unknown.len(), 1, "{data:?}");
    assert!(unknown[0]["mod_id"].is_null());

    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_legacy_v3_disk_pak_is_read_and_overlaps() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let db = driver_for(&server).await;
    let paths = library_for(&server);
    let root = tempfile::tempdir().unwrap();
    let target_id = ready_target(&db, "client-legacy", root.path(), "win64").await;

    let modern = test_pak("../../../", &["Pal/Content/Y.uasset"]);
    common::mods::install_fixture_mod(&db, &paths, "modern", "1.0", &[("pak", "A_P.pak", &modern)])
        .await;
    common::mods::enable(&db, &target_id, "modern", None, true).await;
    let legacy = test_pak_version(TestPakVersion::V3, "../../../", &["Pal/Content/Y.uexp"]);
    common::mods::write(
        &paks(root.path()).join("~WorkshopMods/Old/Legacy_P.pak"),
        &legacy,
    );

    let data = conflicts(&mut ws, &target_id, None).await;
    assert!(data["error"].is_null(), "{data:?}");
    assert!(
        data["unreadable"].as_array().unwrap().is_empty(),
        "{data:?}"
    );
    let overlaps = conflicts_of_kind(&data, "pak_overlap");
    assert_eq!(overlaps.len(), 1, "{data:?}");
    assert_eq!(overlaps[0]["assets"], json!(["pal/content/y.uasset"]));

    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_disk_pak_with_iostore_siblings_is_unreadable_as_iostore() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let db = driver_for(&server).await;
    let root = tempfile::tempdir().unwrap();
    let target_id = ready_target(&db, "client-diskio", root.path(), "win64").await;
    let base = paks(root.path()).join("~mods");
    for name in ["Io_P.pak", "Io_P.utoc", "Io_P.ucas"] {
        common::mods::write(&base.join(name), b"container");
    }

    let data = conflicts(&mut ws, &target_id, None).await;
    let io = unreadable_of(&data, "Io_P.pak");
    assert_eq!(io.len(), 1, "{data:?}");
    assert_eq!(io[0]["reason"], "iostore");
    assert!(io[0]["mod_id"].is_null());

    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_docker_target_scans_no_disk_paks() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let db = driver_for(&server).await;
    let root = tempfile::tempdir().unwrap();
    let target = ps_db::mod_targets::upsert(
        &db,
        &NewModTarget {
            id: "server-docker".to_string(),
            kind: "server".to_string(),
            name: "Docker".to_string(),
            root_path: root.path().to_string_lossy().into_owned(),
            platform: "linux".to_string(),
            ue4ss_mode: "none".to_string(),
            layout_overrides: "{}".to_string(),
            detected: "{}".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    common::mods::default_profile(&db, &target.id).await;
    common::mods::write(&root.path().join("paks/Stray_P.pak"), b"not a pak");
    common::mods::write(&root.path().join("logicmods/Stray2_P.pak"), b"not a pak");

    let data = conflicts(&mut ws, &target.id, None).await;
    assert!(data["error"].is_null(), "{data:?}");
    assert!(
        data["unreadable"].as_array().unwrap().is_empty(),
        "{data:?}"
    );

    server.handle.shutdown().await;
}

#[tokio::test]
async fn an_untracked_legacy_pak_is_not_a_gamepass_conflict() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let db = driver_for(&server).await;
    let root = tempfile::tempdir().unwrap();
    let target_id = ready_target(&db, "client-gdk-disk", root.path(), "wingdk").await;
    let pak = test_pak("../../../", &["Pal/Content/Z.uasset"]);
    common::mods::write(&paks(root.path()).join("~mods/Stray_P.pak"), &pak);
    common::mods::write(&paks(root.path()).join("LogicMods/Other_P.pak"), &pak);

    let data = conflicts(&mut ws, &target_id, None).await;
    assert!(data["error"].is_null(), "{data:?}");
    assert!(
        conflicts_of_kind(&data, "gamepass_pak_incompatible").is_empty(),
        "{data:?}"
    );
    let overlaps = conflicts_of_kind(&data, "pak_overlap");
    assert_eq!(overlaps.len(), 1, "{data:?}");
    assert!(overlaps[0]["paks"].as_array().unwrap().contains(
        &json!({"mod_id": null, "file": "Stray_P.pak", "source": "disk", "path": "~mods/Stray_P.pak"})
    ));

    server.handle.shutdown().await;
}
