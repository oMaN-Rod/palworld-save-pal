mod common;

use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use ps_core::mods::{FileRoute, InstallManifest, ModType, RouteKind, SourceHint};
use ps_server::services::mods::{digest, library, LibraryPaths};

fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let file = std::fs::File::create(path).unwrap();
    let mut writer = zip::ZipWriter::new(file);
    let options: zip::write::FileOptions<'_, ()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, body) in entries {
        writer.start_file(*name, options).unwrap();
        writer.write_all(body).unwrap();
    }
    writer.finish().unwrap();
}

fn library_of(server: &common::TestServer) -> LibraryPaths {
    LibraryPaths::new(server._temp_dir.path())
}

async fn add_target(ws: &mut common::WsClient, root: &Path) -> String {
    common::send_json(
        ws,
        json!({"type": "mod_target_add", "data": {"root_path": root.to_string_lossy()}}),
    )
    .await;
    let reply = common::next_json(ws).await;
    assert_eq!(reply["type"], "mod_target_add", "{reply:?}");
    reply["data"]["target"]["id"].as_str().unwrap().to_string()
}

/// Installs `CoolMod/Scripts/main.lua` with `body` through `mod_install`,
/// returning `(mod_id, version_id)`.
async fn install_cool_mod(
    ws: &mut common::WsClient,
    root: &Path,
    target_id: &str,
    body: &[u8],
    enable: bool,
) -> (String, String) {
    let archive = root.join("CoolMod-1.0.zip");
    write_zip(&archive, &[("CoolMod/Scripts/main.lua", body)]);
    common::send_json(
        ws,
        json!({"type": "mod_install", "data": {
            "path": archive.to_string_lossy(),
            "target_id": target_id,
            "enable": enable,
        }}),
    )
    .await;
    let reply = common::next_json(ws).await;
    assert_eq!(reply["type"], "mod_install", "{reply:?}");
    assert!(reply["data"]["error"].is_null(), "{reply:?}");
    (
        reply["data"]["mod_id"].as_str().unwrap().to_string(),
        reply["data"]["version_id"].as_str().unwrap().to_string(),
    )
}

/// Stores a UE4SS mod straight into the library under an explicit id, so two
/// mods can route the very same destination.
async fn store_ue4ss_mod(
    server: &common::TestServer,
    mod_id: &str,
    rel_path: &str,
    body: &[u8],
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    common::mods::write(&scratch.path().join(rel_path), body);
    let folder = rel_path.split('/').next().unwrap().to_string();
    let manifest = InstallManifest {
        folder_name: folder.clone(),
        display_name: folder,
        mod_type: ModType::Ue4ss,
        version: "1.0".to_string(),
        routes: vec![FileRoute {
            archive_path: rel_path.to_string(),
            rel_path: rel_path.to_string(),
            kind: RouteKind::Ue4ss,
        }],
        decisions: Vec::new(),
        platform_filtered: None,
        source: SourceHint::default(),
    };
    let stored = library::store(
        &*server.handle.app.driver,
        &library_of(server),
        &library::StoreRequest {
            mod_id,
            manifest: &manifest,
            extracted_root: scratch.path(),
            archive: None,
            source_kind: "local",
            source_ref: "{}",
            custom_name: None,
        },
    )
    .await
    .unwrap();
    stored.version.id
}

fn deployed_path(root: &Path) -> PathBuf {
    common::mods_ws::ue4ss_mods_dir(root).join("CoolMod/Scripts/main.lua")
}

async fn set_mod(ws: &mut common::WsClient, data: Value) -> Value {
    common::send_json(ws, json!({"type": "profile_set_mod", "data": data})).await;
    common::mods_ws::next_of_type(ws, "profile_set_mod").await.0
}

async fn apply(ws: &mut common::WsClient, data: Value) -> (Value, Vec<Value>) {
    common::send_json(ws, json!({"type": "profile_apply", "data": data})).await;
    common::mods_ws::next_of_type(ws, "profile_apply").await
}

async fn list_profiles(ws: &mut common::WsClient, target_id: &str) -> Value {
    common::send_json(
        ws,
        json!({"type": "profile_list", "data": {"target_id": target_id}}),
    )
    .await;
    common::mods_ws::next_of_type(ws, "profile_list").await.0
}

fn entry_for<'a>(listed: &'a Value, mod_id: &str) -> Option<&'a Value> {
    listed["data"]["profiles"][0]["mods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["mod_id"] == mod_id)
}

fn only_keep(counts: &Value) -> bool {
    counts.as_object().unwrap().iter().all(|(op, n)| {
        if op == "keep" {
            n.as_u64().unwrap() >= 1
        } else {
            n.as_u64().unwrap() == 0
        }
    })
}

async fn backup_entries(ws: &mut common::WsClient, target_id: &str) -> Vec<Value> {
    common::send_json(
        ws,
        json!({"type": "mod_backup_list", "data": {"target_id": target_id}}),
    )
    .await;
    let reply = common::mods_ws::next_of_type(ws, "mod_backup_list").await.0;
    reply["data"]["sets"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|set| set["entries"].as_array().unwrap().clone())
        .collect()
}

/// A native server target whose recorded pid is this test process, so the real
/// running check reports it running.
async fn running_server_target(server: &common::TestServer, root: &Path) -> String {
    let db = &*server.handle.app.driver;
    let mods_dir = common::mods_ws::ue4ss_mods_dir(root);
    std::fs::create_dir_all(&mods_dir).unwrap();
    let record = ps_db::servers::create_server(
        db,
        ps_db::servers::NewServer {
            name: "Running".to_string(),
            container_name: "running".to_string(),
            server_type: "native".to_string(),
            install_path: root.to_string_lossy().into_owned(),
            mods_path: mods_dir.to_string_lossy().into_owned(),
            logicmods_path: root
                .join("Pal/Content/Paks/LogicMods")
                .to_string_lossy()
                .into_owned(),
            nativemods_path: root
                .join("Pal/Binaries/Win64/NativeMods")
                .to_string_lossy()
                .into_owned(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let mut updates = serde_json::Map::new();
    updates.insert("pid".to_string(), json!(std::process::id() as i64));
    let record = ps_db::servers::update_server(db, record.id, &updates)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.pid, Some(std::process::id() as i64));
    ps_db::mod_targets::ensure_server_target(db, &record, "unused")
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn a_fresh_target_lists_its_default_profile_as_active_and_empty() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;

    let listed = list_profiles(&mut ws, &target_id).await;
    assert!(listed["data"]["error"].is_null(), "{listed:?}");
    assert_eq!(listed["data"]["target_id"], target_id);
    let profiles = listed["data"]["profiles"].as_array().unwrap();
    assert_eq!(profiles.len(), 1, "{profiles:?}");
    assert_eq!(profiles[0]["is_active"], true);
    assert_eq!(profiles[0]["is_default"], true);
    assert_eq!(profiles[0]["mods"], json!([]));
    server.handle.shutdown().await;
}

#[tokio::test]
async fn setting_a_mod_enabled_shows_in_the_profile_list() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let (mod_id, _) =
        install_cool_mod(&mut ws, install.path(), &target_id, b"print('hi')", false).await;

    let reply = set_mod(
        &mut ws,
        json!({"target_id": target_id, "mod_id": mod_id, "enabled": true}),
    )
    .await;
    assert!(reply["data"]["error"].is_null(), "{reply:?}");
    assert_eq!(reply["data"]["enabled"], true);
    assert_eq!(reply["data"]["pending"], false);
    assert!(reply["data"]["apply"]["error"].is_null(), "{reply:?}");

    let listed = list_profiles(&mut ws, &target_id).await;
    let entry = entry_for(&listed, &mod_id).expect("the mod is in the profile");
    assert_eq!(entry["enabled"], true);
    server.handle.shutdown().await;
}

#[tokio::test]
async fn an_omitted_version_keeps_the_pin_and_an_explicit_null_clears_it() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let (mod_id, version_id) =
        install_cool_mod(&mut ws, install.path(), &target_id, b"print('hi')", false).await;

    let pinned = set_mod(
        &mut ws,
        json!({"target_id": target_id, "mod_id": mod_id, "enabled": true, "mod_version_id": version_id}),
    )
    .await;
    assert_eq!(pinned["data"]["mod_version_id"], version_id, "{pinned:?}");

    let toggled = set_mod(
        &mut ws,
        json!({"target_id": target_id, "mod_id": mod_id, "enabled": false}),
    )
    .await;
    assert_eq!(toggled["data"]["mod_version_id"], version_id, "{toggled:?}");
    let listed = list_profiles(&mut ws, &target_id).await;
    assert_eq!(
        entry_for(&listed, &mod_id).unwrap()["mod_version_id"],
        version_id
    );

    let unpinned = set_mod(
        &mut ws,
        json!({"target_id": target_id, "mod_id": mod_id, "enabled": true, "mod_version_id": null}),
    )
    .await;
    assert!(unpinned["data"]["mod_version_id"].is_null(), "{unpinned:?}");
    let listed = list_profiles(&mut ws, &target_id).await;
    let entry = entry_for(&listed, &mod_id).unwrap();
    assert!(entry["mod_version_id"].is_null(), "{entry:?}");
    assert_eq!(entry["enabled"], true);
    server.handle.shutdown().await;
}

#[tokio::test]
async fn planning_an_enabled_mod_reports_adds_and_writes_nothing() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    install_cool_mod(&mut ws, install.path(), &target_id, b"print('hi')", true).await;

    common::send_json(
        &mut ws,
        json!({"type": "profile_plan", "data": {"target_id": target_id}}),
    )
    .await;
    let plan = common::next_json(&mut ws).await;
    assert_eq!(plan["type"], "profile_plan", "{plan:?}");
    assert!(plan["data"]["error"].is_null(), "{plan:?}");
    assert!(
        plan["data"]["counts"]["add"].as_u64().unwrap() >= 1,
        "{plan:?}"
    );
    assert!(!plan["data"]["entries"].as_array().unwrap().is_empty());
    assert!(!deployed_path(install.path()).exists());
    server.handle.shutdown().await;
}

#[tokio::test]
async fn applying_writes_the_files_and_a_second_apply_is_all_keep() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    install_cool_mod(&mut ws, install.path(), &target_id, b"print('hi')", true).await;

    let (first, _) = apply(&mut ws, json!({"target_id": target_id})).await;
    assert_eq!(first["data"]["mid_apply"], false, "{first:?}");
    assert!(first["data"]["error"].is_null(), "{first:?}");
    assert_eq!(
        std::fs::read_to_string(deployed_path(install.path())).unwrap(),
        "print('hi')"
    );

    let (second, _) = apply(&mut ws, json!({"target_id": target_id})).await;
    assert_eq!(second["data"]["mid_apply"], false, "{second:?}");
    assert!(second["data"]["error"].is_null(), "{second:?}");
    assert!(only_keep(&second["data"]["counts"]), "{second:?}");
    server.handle.shutdown().await;
}

#[tokio::test]
async fn an_apply_pushes_progress_before_its_result() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    install_cool_mod(&mut ws, install.path(), &target_id, b"print('hi')", true).await;

    let (reply, before) = apply(&mut ws, json!({"target_id": target_id})).await;
    assert!(reply["data"]["op_id"].is_null(), "{reply:?}");
    let request_id = reply["data"]["request_id"].as_str().unwrap();
    let progress: Vec<&Value> = before
        .iter()
        .filter(|f| f["type"] == "mod_progress" && f["data"]["request_id"] == request_id)
        .collect();
    assert!(!progress.is_empty(), "{before:?}");
    assert!(progress.iter().all(|f| f["data"]["target_id"] == target_id));
    assert!(
        progress.iter().any(|f| f["data"]["stage"] == "done"),
        "{before:?}"
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn an_unmanaged_file_in_the_way_is_reported_as_a_coded_refusal() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    install_cool_mod(&mut ws, install.path(), &target_id, b"print('hi')", true).await;
    let destination = deployed_path(install.path());
    common::mods::write(&destination, b"someone else's file");

    let (reply, before) = apply(&mut ws, json!({"target_id": target_id})).await;
    assert_eq!(reply["data"]["mid_apply"], false, "{reply:?}");
    assert_eq!(
        reply["data"]["error"]["code"], "unmanaged_occupant",
        "{reply:?}"
    );
    assert!(reply["data"]["backup_dir"].is_null(), "{reply:?}");
    assert!(
        reply["data"]["counts"]
            .as_object()
            .unwrap()
            .values()
            .all(|n| n == 0),
        "{reply:?}"
    );
    let request_id = reply["data"]["request_id"].as_str().unwrap();
    assert!(
        before.iter().any(|f| f["type"] == "mod_progress"
            && f["data"]["request_id"] == request_id
            && f["data"]["stage"] == "done"),
        "a refused apply still reports done before its reply: {before:?}"
    );
    let paths = reply["data"]["error"]["paths"].as_array().unwrap();
    assert!(
        paths
            .iter()
            .any(|p| Path::new(p.as_str().unwrap()) == destination.as_path()),
        "{reply:?}"
    );
    assert_eq!(std::fs::read(&destination).unwrap(), b"someone else's file");
    server.handle.shutdown().await;
}

#[tokio::test]
async fn disabling_a_mod_and_applying_removes_its_files() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let (mod_id, _) =
        install_cool_mod(&mut ws, install.path(), &target_id, b"print('hi')", true).await;
    let (applied, _) = apply(&mut ws, json!({"target_id": target_id})).await;
    assert!(applied["data"]["error"].is_null(), "{applied:?}");
    assert!(deployed_path(install.path()).is_file());

    let reply = set_mod(
        &mut ws,
        json!({"target_id": target_id, "mod_id": mod_id, "enabled": false}),
    )
    .await;
    assert_eq!(reply["data"]["pending"], false, "{reply:?}");
    assert!(reply["data"]["apply"]["error"].is_null(), "{reply:?}");
    assert!(
        reply["data"]["apply"]["counts"]["remove"].as_u64().unwrap() >= 1,
        "{reply:?}"
    );
    assert!(!deployed_path(install.path()).exists());

    let (again, _) = apply(&mut ws, json!({"target_id": target_id})).await;
    assert!(again["data"]["error"].is_null(), "{again:?}");
    assert!(!deployed_path(install.path()).exists());
    server.handle.shutdown().await;
}

#[tokio::test]
async fn profile_set_mod_while_running_commits_and_reports_pending() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = running_server_target(&server, install.path()).await;
    let mod_id = "coolmod-ue4ss";
    store_ue4ss_mod(&server, mod_id, "CoolMod/Scripts/main.lua", b"print('hi')").await;

    let reply = set_mod(
        &mut ws,
        json!({"target_id": target_id, "mod_id": mod_id, "enabled": true}),
    )
    .await;
    assert!(reply["data"]["error"].is_null(), "{reply:?}");
    assert_eq!(reply["data"]["pending"], true, "{reply:?}");
    assert!(reply["data"]["apply"].is_null(), "{reply:?}");
    assert!(reply["data"]["request_id"].is_string(), "{reply:?}");

    let listed = list_profiles(&mut ws, &target_id).await;
    assert_eq!(entry_for(&listed, mod_id).unwrap()["enabled"], true);
    assert!(!deployed_path(install.path()).exists());
    let rows = ps_db::mod_deployments::files_of(&*server.handle.app.driver, &target_id)
        .await
        .unwrap();
    assert!(rows.is_empty(), "{rows:?}");
    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_selection_that_would_collide_is_rejected_and_nothing_is_written() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let rel = "CoolMod/Scripts/main.lua";
    store_ue4ss_mod(&server, "alpha-ue4ss", rel, b"alpha").await;
    store_ue4ss_mod(&server, "beta-ue4ss", rel, b"beta").await;
    store_ue4ss_mod(&server, "gamma-ue4ss", rel, b"gamma").await;

    let alpha = set_mod(
        &mut ws,
        json!({"target_id": target_id, "mod_id": "alpha-ue4ss", "enabled": true}),
    )
    .await;
    assert!(alpha["data"]["apply"]["error"].is_null(), "{alpha:?}");
    let beta_off = set_mod(
        &mut ws,
        json!({"target_id": target_id, "mod_id": "beta-ue4ss", "enabled": false}),
    )
    .await;
    assert!(beta_off["data"]["error"].is_null(), "{beta_off:?}");
    let destination = deployed_path(install.path());
    assert_eq!(std::fs::read(&destination).unwrap(), b"alpha");

    let beta_on = set_mod(
        &mut ws,
        json!({"target_id": target_id, "mod_id": "beta-ue4ss", "enabled": true}),
    )
    .await;
    assert_eq!(
        beta_on["data"]["error"]["code"], "destination_conflict",
        "{beta_on:?}"
    );
    assert!(
        Path::new(beta_on["data"]["error"]["path"].as_str().unwrap()) == destination.as_path(),
        "{beta_on:?}"
    );
    assert_eq!(
        beta_on["data"]["error"]["mod_version_ids"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let gamma_on = set_mod(
        &mut ws,
        json!({"target_id": target_id, "mod_id": "gamma-ue4ss", "enabled": true}),
    )
    .await;
    assert_eq!(
        gamma_on["data"]["error"]["code"], "destination_conflict",
        "{gamma_on:?}"
    );

    let listed = list_profiles(&mut ws, &target_id).await;
    assert_eq!(entry_for(&listed, "beta-ue4ss").unwrap()["enabled"], false);
    assert!(entry_for(&listed, "gamma-ue4ss").is_none(), "{listed:?}");
    assert_eq!(std::fs::read(&destination).unwrap(), b"alpha");
    server.handle.shutdown().await;
}

#[tokio::test]
async fn replacing_an_occupant_backs_it_up_and_applies() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    install_cool_mod(&mut ws, install.path(), &target_id, b"print('hi')", true).await;
    let destination = deployed_path(install.path());
    common::mods::write(&destination, b"someone else's file");

    let (reply, _) = apply(
        &mut ws,
        json!({"target_id": target_id, "replace_occupants": [destination.to_string_lossy()]}),
    )
    .await;
    assert!(reply["data"]["error"].is_null(), "{reply:?}");
    assert_eq!(reply["data"]["mid_apply"], false);
    assert!(reply["data"]["backup_dir"].is_string(), "{reply:?}");
    assert_eq!(std::fs::read(&destination).unwrap(), b"print('hi')");

    let entries = backup_entries(&mut ws, &target_id).await;
    let taken = entries
        .iter()
        .find(|e| Path::new(e["original_path"].as_str().unwrap()) == destination.as_path())
        .unwrap_or_else(|| panic!("no backup entry names the occupant: {entries:?}"));
    assert_eq!(taken["hash"], digest::hash_bytes(b"someone else's file"));
    server.handle.shutdown().await;
}

#[tokio::test]
async fn replace_never_touches_a_managed_file() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    install_cool_mod(&mut ws, install.path(), &target_id, b"print('hi')", true).await;
    let (first, _) = apply(&mut ws, json!({"target_id": target_id})).await;
    assert!(first["data"]["error"].is_null(), "{first:?}");

    let destination = deployed_path(install.path());
    std::fs::write(&destination, b"the user's edit").unwrap();

    let (reply, _) = apply(
        &mut ws,
        json!({"target_id": target_id, "replace_occupants": [destination.to_string_lossy()]}),
    )
    .await;
    assert!(reply["data"]["error"].is_null(), "{reply:?}");
    assert_eq!(reply["data"]["counts"]["preserve"], 1, "{reply:?}");
    assert!(
        reply["data"]["preserved"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| Path::new(p.as_str().unwrap()) == destination.as_path()),
        "{reply:?}"
    );
    let mut copy_name = destination.file_name().unwrap().to_os_string();
    copy_name.push(".new");
    let copy = destination.with_file_name(copy_name);
    assert!(
        reply["data"]["new_copies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| Path::new(p.as_str().unwrap()) == copy.as_path()),
        "{reply:?}"
    );
    assert_eq!(reply["data"]["skipped_new_copies"], json!([]), "{reply:?}");
    assert_eq!(std::fs::read(&destination).unwrap(), b"the user's edit");
    let entries = backup_entries(&mut ws, &target_id).await;
    assert!(
        entries
            .iter()
            .all(|e| Path::new(e["original_path"].as_str().unwrap()) != destination.as_path()),
        "{entries:?}"
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_path_outside_the_desired_set_is_never_replaced() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    install_cool_mod(&mut ws, install.path(), &target_id, b"print('hi')", true).await;
    let destination = deployed_path(install.path());
    common::mods::write(&destination, b"someone else's file");
    let bystander = install.path().join("Pal/Binaries/Win64/bystander.dll");
    std::fs::write(&bystander, b"not a destination").unwrap();
    let dotted =
        common::mods_ws::ue4ss_mods_dir(install.path()).join("CoolMod/Scripts/../Scripts/main.lua");

    let (reply, _) = apply(
        &mut ws,
        json!({"target_id": target_id, "replace_occupants": [
            bystander.to_string_lossy(),
            dotted.to_string_lossy(),
            "CoolMod/Scripts/main.lua",
        ]}),
    )
    .await;
    assert_eq!(
        reply["data"]["error"]["code"], "unmanaged_occupant",
        "{reply:?}"
    );
    assert_eq!(std::fs::read(&bystander).unwrap(), b"not a destination");
    assert_eq!(std::fs::read(&destination).unwrap(), b"someone else's file");
    assert!(backup_entries(&mut ws, &target_id).await.is_empty());
    server.handle.shutdown().await;
}

#[tokio::test]
async fn restoring_a_backup_is_refused_while_the_target_is_running() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = running_server_target(&server, install.path()).await;

    let original = install.path().join("Pal/Binaries/Win64/main.lua");
    std::fs::write(&original, b"print('original')").unwrap();
    let mut set = ps_server::services::mods::deploy::backup::BackupSet::open(
        &library_of(&server),
        &target_id,
        "20260101-000000",
        "abc123",
    )
    .unwrap();
    set.copy_in(&original).unwrap();
    set.write_index().unwrap();
    std::fs::remove_file(&original).unwrap();

    common::send_json(
        &mut ws,
        json!({"type": "mod_backup_restore", "data": {
            "target_id": target_id, "set": "20260101-000000-abc123",
        }}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;
    assert_eq!(reply["type"], "mod_backup_restore");
    assert_eq!(reply["data"]["error"]["code"], "target_locked", "{reply:?}");
    assert!(!original.exists());
    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_collision_is_rejected_even_with_an_unmanaged_file_at_the_destination() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let rel = "CoolMod/Scripts/main.lua";
    store_ue4ss_mod(&server, "alpha-ue4ss", rel, b"alpha").await;
    store_ue4ss_mod(&server, "beta-ue4ss", rel, b"beta").await;
    let destination = deployed_path(install.path());
    common::mods::write(&destination, b"hand-installed");

    let alpha = set_mod(
        &mut ws,
        json!({"target_id": target_id, "mod_id": "alpha-ue4ss", "enabled": true}),
    )
    .await;
    assert!(alpha["data"]["error"].is_null(), "{alpha:?}");
    assert_eq!(
        alpha["data"]["apply"]["error"]["code"], "unmanaged_occupant",
        "{alpha:?}"
    );
    let beta_off = set_mod(
        &mut ws,
        json!({"target_id": target_id, "mod_id": "beta-ue4ss", "enabled": false}),
    )
    .await;
    assert!(beta_off["data"]["error"].is_null(), "{beta_off:?}");

    let beta_on = set_mod(
        &mut ws,
        json!({"target_id": target_id, "mod_id": "beta-ue4ss", "enabled": true}),
    )
    .await;
    assert_eq!(
        beta_on["data"]["error"]["code"], "destination_conflict",
        "{beta_on:?}"
    );

    let listed = list_profiles(&mut ws, &target_id).await;
    assert_eq!(entry_for(&listed, "beta-ue4ss").unwrap()["enabled"], false);
    assert_eq!(std::fs::read(&destination).unwrap(), b"hand-installed");
    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_rejected_collision_restores_a_pinned_version_and_its_load_order() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let rel = "CoolMod/Scripts/main.lua";
    store_ue4ss_mod(&server, "alpha-ue4ss", rel, b"alpha").await;
    let beta_version = store_ue4ss_mod(&server, "beta-ue4ss", rel, b"beta").await;

    set_mod(
        &mut ws,
        json!({"target_id": target_id, "mod_id": "alpha-ue4ss", "enabled": true}),
    )
    .await;
    let pinned = set_mod(
        &mut ws,
        json!({"target_id": target_id, "mod_id": "beta-ue4ss", "enabled": false,
               "mod_version_id": beta_version}),
    )
    .await;
    assert!(pinned["data"]["error"].is_null(), "{pinned:?}");
    let before = list_profiles(&mut ws, &target_id).await;
    let before_entry = entry_for(&before, "beta-ue4ss").unwrap().clone();
    assert_eq!(before_entry["mod_version_id"], beta_version);
    assert_eq!(before_entry["load_order"], 1, "{before_entry:?}");

    let rejected = set_mod(
        &mut ws,
        json!({"target_id": target_id, "mod_id": "beta-ue4ss", "enabled": true,
               "mod_version_id": null}),
    )
    .await;
    assert_eq!(
        rejected["data"]["error"]["code"], "destination_conflict",
        "{rejected:?}"
    );

    let after = list_profiles(&mut ws, &target_id).await;
    assert_eq!(entry_for(&after, "beta-ue4ss").unwrap(), &before_entry);
    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_replace_the_apply_would_still_refuse_moves_nothing() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;

    let archive = install.path().join("CoolMod-1.0.zip");
    write_zip(
        &archive,
        &[
            ("CoolMod/Scripts/main.lua", b"print('hi')"),
            ("CoolMod/Scripts/other.lua", b"print('other')"),
        ],
    );
    common::send_json(
        &mut ws,
        json!({"type": "mod_install", "data": {
            "path": archive.to_string_lossy(),
            "target_id": target_id,
        }}),
    )
    .await;
    let installed = common::next_json(&mut ws).await;
    assert!(installed["data"]["error"].is_null(), "{installed:?}");

    let listed = deployed_path(install.path());
    let unlisted =
        common::mods_ws::ue4ss_mods_dir(install.path()).join("CoolMod/Scripts/other.lua");
    common::mods::write(&listed, b"listed occupant");
    common::mods::write(&unlisted, b"unlisted occupant");

    let (reply, _) = apply(
        &mut ws,
        json!({"target_id": target_id, "replace_occupants": [listed.to_string_lossy()]}),
    )
    .await;
    assert_eq!(
        reply["data"]["error"]["code"], "unmanaged_occupant",
        "{reply:?}"
    );
    assert_eq!(std::fs::read(&listed).unwrap(), b"listed occupant");
    assert_eq!(std::fs::read(&unlisted).unwrap(), b"unlisted occupant");
    assert!(
        !library_of(&server).backups_dir(&target_id).exists(),
        "no backup set may exist for a replace that moved nothing"
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_desired_destination_spelled_with_dot_dot_is_never_replaced() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let dotted_dir = install
        .path()
        .join("Pal/Binaries/Win64/ue4ss/../ue4ss/Mods");
    ps_db::mod_targets::set_layout_overrides(
        &*server.handle.app.driver,
        &target_id,
        &json!({ "ue4ss_mods_dir": dotted_dir.to_string_lossy() }).to_string(),
    )
    .await
    .unwrap();
    install_cool_mod(&mut ws, install.path(), &target_id, b"print('hi')", true).await;

    let dotted = dotted_dir.join("CoolMod/Scripts/main.lua");
    common::mods::write(&deployed_path(install.path()), b"someone else's file");

    common::send_json(
        &mut ws,
        json!({"type": "profile_plan", "data": {"target_id": target_id}}),
    )
    .await;
    let plan = common::next_json(&mut ws).await;
    let occupied = plan["data"]["error"]["paths"].as_array().unwrap();
    assert!(
        occupied
            .iter()
            .any(|p| Path::new(p.as_str().unwrap()) == dotted.as_path()),
        "the desired destination itself carries the `..` spelling: {plan:?}"
    );

    let (reply, _) = apply(
        &mut ws,
        json!({"target_id": target_id, "replace_occupants": [dotted.to_string_lossy()]}),
    )
    .await;
    assert_eq!(
        reply["data"]["error"]["code"], "unmanaged_occupant",
        "{reply:?}"
    );
    assert_eq!(
        std::fs::read(deployed_path(install.path())).unwrap(),
        b"someone else's file"
    );
    assert!(!library_of(&server).backups_dir(&target_id).exists());
    server.handle.shutdown().await;
}

/// A Docker server target whose server record does not exist, so no container is
/// ever inspected.
async fn docker_target(server: &common::TestServer, root: &Path) -> String {
    let db = &*server.handle.app.driver;
    let dir = |name: &str| root.join(name).to_string_lossy().into_owned();
    let target = ps_db::mod_targets::upsert(
        db,
        &ps_db::mod_targets::NewModTarget {
            id: "server-docker".to_string(),
            kind: "server".to_string(),
            name: "Docker".to_string(),
            root_path: root.to_string_lossy().into_owned(),
            platform: "linux".to_string(),
            ue4ss_mode: "none".to_string(),
            layout_overrides: json!({
                "ue4ss_mods_dir": dir("mods"),
                "logicmods_dir": dir("logicmods"),
                "nativemods_dir": dir("nativemods"),
                "paks_mods_dir": dir("paks"),
            })
            .to_string(),
            detected: "{}".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    ps_db::mod_profiles::create(
        db,
        &ps_db::mod_profiles::NewProfile {
            id: format!("{}/default", target.id),
            target_id: target.id.clone(),
            name: "Default".to_string(),
            is_default: true,
        },
    )
    .await
    .unwrap();
    target.id
}

/// A Workshop package in the library. Without files it stands for a
/// Steam-subscribed package, which only `ActiveModList` activates.
async fn store_workshop_pack(
    server: &common::TestServer,
    mod_id: &str,
    package: &str,
    with_files: bool,
) {
    let scratch = tempfile::tempdir().unwrap();
    let rel = format!("{package}/Info.json");
    let routes = if with_files {
        common::mods::write(&scratch.path().join(&rel), b"{}");
        vec![FileRoute {
            archive_path: rel.clone(),
            rel_path: rel,
            kind: RouteKind::Workshop,
        }]
    } else {
        Vec::new()
    };
    let manifest = InstallManifest {
        folder_name: package.to_string(),
        display_name: package.to_string(),
        mod_type: ModType::Workshop,
        version: "1.0".to_string(),
        routes,
        decisions: Vec::new(),
        platform_filtered: None,
        source: SourceHint::default(),
    };
    library::store(
        &*server.handle.app.driver,
        &library_of(server),
        &library::StoreRequest {
            mod_id,
            manifest: &manifest,
            extracted_root: scratch.path(),
            archive: None,
            source_kind: "local",
            source_ref: "{}",
            custom_name: None,
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn enabling_a_workshop_mod_on_a_docker_server_is_refused_and_writes_nothing() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let root = tempfile::tempdir().unwrap();
    let target_id = docker_target(&server, root.path()).await;
    store_workshop_pack(&server, "coolpack", "CoolPack", true).await;
    store_workshop_pack(&server, "steampack", "SteamPack", false).await;

    for mod_id in ["coolpack", "steampack"] {
        let reply = set_mod(
            &mut ws,
            json!({"target_id": target_id, "mod_id": mod_id, "enabled": true}),
        )
        .await;
        assert_eq!(
            reply["data"]["error"]["code"], "not_supported_on_target",
            "{reply:?}"
        );
        assert_eq!(reply["data"]["error"]["kind"], "workshop", "{reply:?}");
        assert_eq!(reply["data"]["mod_id"], mod_id, "{reply:?}");
    }
    let listed = list_profiles(&mut ws, &target_id).await;
    assert!(entry_for(&listed, "coolpack").is_none(), "{listed:?}");
    assert!(entry_for(&listed, "steampack").is_none(), "{listed:?}");
    assert!(!root.path().join("Mods/PalModSettings.ini").exists());
    server.handle.shutdown().await;
}

#[tokio::test]
async fn disabling_a_workshop_mod_on_a_docker_server_is_allowed() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let root = tempfile::tempdir().unwrap();
    let target_id = docker_target(&server, root.path()).await;
    store_workshop_pack(&server, "coolpack", "CoolPack", true).await;

    let reply = set_mod(
        &mut ws,
        json!({"target_id": target_id, "mod_id": "coolpack", "enabled": false}),
    )
    .await;
    assert!(reply["data"]["error"].is_null(), "{reply:?}");
    let listed = list_profiles(&mut ws, &target_id).await;
    assert_eq!(
        entry_for(&listed, "coolpack").expect("the entry is written")["enabled"],
        false
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_profile_already_holding_a_workshop_mod_refuses_the_apply_on_a_docker_server() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let root = tempfile::tempdir().unwrap();
    let target_id = docker_target(&server, root.path()).await;
    store_workshop_pack(&server, "steampack", "SteamPack", false).await;
    ps_db::mod_profiles::set_mod(
        &*server.handle.app.driver,
        &ps_db::mod_profiles::ProfileModRow {
            profile_id: format!("{target_id}/default"),
            mod_id: "steampack".to_string(),
            mod_version_id: None,
            enabled: true,
            load_order: 0,
        },
    )
    .await
    .unwrap();

    let (reply, _) = apply(&mut ws, json!({"target_id": target_id})).await;
    assert_eq!(
        reply["data"]["error"]["code"], "not_supported_on_target",
        "a package that could never activate is refused, not reported applied: {reply:?}"
    );
    assert_eq!(reply["data"]["error"]["kind"], "workshop", "{reply:?}");
    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_subscribed_package_is_adopted_toggled_and_removed_without_touching_steam() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let db = &*server.handle.app.driver;
    let scratch = tempfile::tempdir().unwrap();
    let library_root = scratch.path().join("SteamLibrary");
    let root = library_root.join("steamapps/common/Palworld");
    let content = library_root.join("steamapps/workshop/content/1623730");
    common::mods::write(
        &content.join("3300000001/Info.json"),
        br#"{"PackageName":"SteamPack"}"#,
    );
    common::mods::write(&content.join("3300000001/SteamPack_P.pak"), b"steam bytes");
    let ini = root.join("Mods/PalModSettings.ini");
    common::mods::write(
        &ini,
        b"[PalModSettings]\nbGlobalEnableMod=True\nActiveModList=SteamThing\nActiveModList=SteamPack\n",
    );
    let target_id = ps_db::mod_targets::upsert(
        db,
        &ps_db::mod_targets::NewModTarget {
            id: "client-steam".to_string(),
            kind: "client".to_string(),
            name: "Steam".to_string(),
            root_path: root.to_string_lossy().into_owned(),
            platform: "win64".to_string(),
            ue4ss_mode: "none".to_string(),
            layout_overrides: "{}".to_string(),
            detected: "{}".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .id;
    ps_db::mod_profiles::create(
        db,
        &ps_db::mod_profiles::NewProfile {
            id: format!("{target_id}/default"),
            target_id: target_id.clone(),
            name: "Default".to_string(),
            is_default: true,
        },
    )
    .await
    .unwrap();
    let steam_before = common::mods::tree_snapshot(&content);
    let active = || {
        ps_core::mods::PalModSettings::parse(&std::fs::read_to_string(&ini).unwrap()).active_mods
    };

    common::send_json(
        &mut ws,
        json!({"type": "mod_target_scan", "data": {"target_id": target_id}}),
    )
    .await;
    let scanned = common::mods_ws::next_of_type(&mut ws, "mod_target_scan").await.0;
    let candidate = scanned["data"]["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["source"] == "steam_subscribed")
        .unwrap_or_else(|| panic!("{scanned:?}"));
    assert_eq!(candidate["name"], "SteamPack");
    assert_eq!(candidate["enabled"], true);

    common::send_json(
        &mut ws,
        json!({"type": "mod_adopt", "data": {
            "target_id": target_id,
            "candidate_name": "SteamPack",
            "source": "steam_subscribed",
        }}),
    )
    .await;
    let adopted = common::mods_ws::next_of_type(&mut ws, "mod_adopt").await.0;
    assert!(adopted["data"]["error"].is_null(), "{adopted:?}");
    assert_eq!(adopted["data"]["candidate_name"], "SteamPack");
    assert_eq!(adopted["data"]["source"], "steam_subscribed");
    let mod_id = adopted["data"]["mod_id"].as_str().unwrap().to_string();
    let version_id = adopted["data"]["version_id"].as_str().unwrap().to_string();

    let reply = set_mod(
        &mut ws,
        json!({"target_id": target_id, "mod_id": mod_id, "enabled": false}),
    )
    .await;
    assert!(reply["data"]["error"].is_null(), "{reply:?}");
    assert!(reply["data"]["apply"]["error"].is_null(), "{reply:?}");
    assert_eq!(active(), vec!["SteamThing"]);
    let reply = set_mod(
        &mut ws,
        json!({"target_id": target_id, "mod_id": mod_id, "enabled": true}),
    )
    .await;
    assert!(reply["data"]["apply"]["error"].is_null(), "{reply:?}");
    assert_eq!(active(), vec!["SteamThing", "SteamPack"]);

    common::send_json(
        &mut ws,
        json!({"type": "mod_version_delete", "data": {"version_id": version_id}}),
    )
    .await;
    let refused = common::mods_ws::next_of_type(&mut ws, "mod_version_delete").await.0;
    assert_eq!(refused["data"]["error"]["code"], "subscribed_package", "{refused:?}");
    assert_eq!(refused["data"]["version_id"], version_id.as_str());
    common::send_json(
        &mut ws,
        json!({"type": "mod_version_set_current", "data": {
            "mod_id": mod_id, "version_id": version_id,
        }}),
    )
    .await;
    let refused = common::mods_ws::next_of_type(&mut ws, "mod_version_set_current").await.0;
    assert_eq!(refused["data"]["error"]["code"], "subscribed_package", "{refused:?}");

    common::send_json(&mut ws, json!({"type": "mod_remove", "data": {"mod_id": mod_id}})).await;
    let removed = common::mods_ws::next_of_type(&mut ws, "mod_remove").await.0;
    assert_eq!(removed["data"]["removed"], true, "{removed:?}");
    assert_eq!(active(), vec!["SteamThing", "SteamPack"], "removal itself writes nothing");

    let (applied, _) = apply(&mut ws, json!({"target_id": target_id})).await;
    assert!(applied["data"]["error"].is_null(), "{applied:?}");
    assert_eq!(
        active(),
        vec!["SteamThing"],
        "the next apply drops the removed package's line and keeps Steam's own"
    );
    assert_eq!(
        common::mods::tree_snapshot(&content),
        steam_before,
        "Steam's files are untouched by adoption, toggling and removal"
    );
    common::send_json(&mut ws, json!({"type": "mod_list", "data": null})).await;
    let listed = common::mods_ws::next_of_type(&mut ws, "mod_list").await.0;
    assert!(
        listed["data"]["mods"].as_array().unwrap().is_empty(),
        "{listed:?}"
    );
    let entries = ps_db::mod_profiles::mods_of(db, &format!("{target_id}/default"))
        .await
        .unwrap();
    assert!(entries.is_empty(), "{entries:?}");
    server.handle.shutdown().await;
}

async fn client_row(
    server: &common::TestServer,
    id: &str,
    root: &Path,
    ue4ss_mode: &str,
) -> String {
    let db = &*server.handle.app.driver;
    ps_db::mod_targets::upsert(
        db,
        &ps_db::mod_targets::NewModTarget {
            id: id.to_string(),
            kind: "client".to_string(),
            name: id.to_string(),
            root_path: root.to_string_lossy().into_owned(),
            platform: "win64".to_string(),
            ue4ss_mode: ue4ss_mode.to_string(),
            layout_overrides: "{}".to_string(),
            detected: "{}".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    ps_db::mod_profiles::create(
        db,
        &ps_db::mod_profiles::NewProfile {
            id: format!("{id}/default"),
            target_id: id.to_string(),
            name: "Default".to_string(),
            is_default: true,
        },
    )
    .await
    .unwrap();
    id.to_string()
}

/// `client-steam` in a Steam library whose Workshop content holds `SteamPack` as
/// item 3300000001, enabled in the settings file.
async fn steam_library_target(
    server: &common::TestServer,
    scratch: &Path,
    ue4ss_mode: &str,
) -> (String, PathBuf) {
    let library_root = scratch.join("SteamLibrary");
    let root = library_root.join("steamapps/common/Palworld");
    common::mods::write(
        &library_root.join("steamapps/workshop/content/1623730/3300000001/Info.json"),
        br#"{"PackageName":"SteamPack"}"#,
    );
    common::mods::write(
        &root.join("Mods/PalModSettings.ini"),
        b"[PalModSettings]\nbGlobalEnableMod=True\nActiveModList=SteamPack\n",
    );
    let target_id = client_row(server, "client-steam", &root, ue4ss_mode).await;
    (target_id, root)
}

async fn adopt_candidate(
    ws: &mut common::WsClient,
    target_id: &str,
    name: &str,
    source: &str,
) -> Value {
    common::send_json(
        ws,
        json!({"type": "mod_adopt", "data": {
            "target_id": target_id,
            "candidate_name": name,
            "source": source,
        }}),
    )
    .await;
    common::mods_ws::next_of_type(ws, "mod_adopt").await.0
}

async fn assert_next_reply_is_mod_list(ws: &mut common::WsClient) -> Value {
    common::send_json(ws, json!({"type": "mod_list", "data": null})).await;
    let next = common::next_json(ws).await;
    assert_eq!(next["type"], "mod_list", "a refusal is one reply: {next:?}");
    next
}

#[tokio::test]
async fn mod_remove_is_refused_while_an_apply_holds_a_holding_targets_lock() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let db = &*server.handle.app.driver;
    let scratch = tempfile::tempdir().unwrap();
    let (target_id, _root) = steam_library_target(&server, scratch.path(), "none").await;
    let adopted = adopt_candidate(&mut ws, &target_id, "SteamPack", "steam_subscribed").await;
    assert!(adopted["data"]["error"].is_null(), "{adopted:?}");
    let mod_id = adopted["data"]["mod_id"].as_str().unwrap().to_string();
    let target = ps_db::mod_targets::get(db, &target_id)
        .await
        .unwrap()
        .unwrap();
    let guard = ps_server::services::mods::deploy::try_lock_target(&library_of(&server), &target)
        .expect("no apply is running");

    common::send_json(&mut ws, json!({"type": "mod_remove", "data": {"mod_id": mod_id}})).await;
    let refused = common::mods_ws::next_of_type(&mut ws, "mod_remove").await.0;
    assert_eq!(refused["data"]["error"]["code"], "apply_in_progress", "{refused:?}");
    assert_eq!(refused["data"]["error"]["target_id"], target_id.as_str());
    assert_eq!(refused["data"]["mod_id"], mod_id.as_str());
    assert!(
        library::released_packages(db, &target_id)
            .await
            .unwrap()
            .is_empty(),
        "nothing is released by a refused removal"
    );
    let listed = assert_next_reply_is_mod_list(&mut ws).await;
    assert_eq!(listed["data"]["mods"].as_array().unwrap().len(), 1);

    drop(guard);
    common::send_json(&mut ws, json!({"type": "mod_remove", "data": {"mod_id": mod_id}})).await;
    let removed = common::mods_ws::next_of_type(&mut ws, "mod_remove").await.0;
    assert_eq!(removed["data"]["removed"], true, "{removed:?}");
    assert_eq!(
        library::released_packages(db, &target_id).await.unwrap(),
        vec!["SteamPack"]
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn enabling_a_subscribed_mod_where_steam_lacks_its_item_is_refused() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let scratch = tempfile::tempdir().unwrap();
    let (target_id, _root) = steam_library_target(&server, scratch.path(), "none").await;
    let adopted = adopt_candidate(&mut ws, &target_id, "SteamPack", "steam_subscribed").await;
    let mod_id = adopted["data"]["mod_id"].as_str().unwrap().to_string();

    let other_root = scratch.path().join("Other/Palworld");
    let empty_steam = scratch.path().join("empty-steam/1623730");
    std::fs::create_dir_all(&empty_steam).unwrap();
    common::mods::write(
        &other_root.join("Mods/PalModSettings.ini"),
        format!("[PalModSettings]\nWorkshopRootDir={}\n", empty_steam.display()).as_bytes(),
    );
    let other_id = client_row(&server, "client-other", &other_root, "none").await;

    let reply = set_mod(
        &mut ws,
        json!({"target_id": other_id, "mod_id": mod_id, "enabled": true}),
    )
    .await;
    assert_eq!(
        reply["data"]["error"]["code"], "not_subscribed_on_target",
        "{reply:?}"
    );
    assert_eq!(reply["data"]["error"]["workshop_id"], "3300000001");
    let listed = list_profiles(&mut ws, &other_id).await;
    assert!(entry_for(&listed, &mod_id).is_none(), "{listed:?}");

    let reply = set_mod(
        &mut ws,
        json!({"target_id": other_id, "mod_id": mod_id, "enabled": false}),
    )
    .await;
    assert!(reply["data"]["error"].is_null(), "{reply:?}");
    server.handle.shutdown().await;
}

#[tokio::test]
async fn adopt_refusals_reply_once_under_mod_adopt() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let scratch = tempfile::tempdir().unwrap();
    let (target_id, _root) = steam_library_target(&server, scratch.path(), "none").await;

    let refused = adopt_candidate(&mut ws, &target_id, "SteamPack", "local").await;
    assert_eq!(refused["data"]["error"]["code"], "not_a_candidate", "{refused:?}");
    assert_eq!(refused["data"]["candidate_name"], "SteamPack");
    assert_next_reply_is_mod_list(&mut ws).await;

    let manifest = InstallManifest {
        folder_name: "OldName".to_string(),
        display_name: "OldName".to_string(),
        mod_type: ModType::Workshop,
        version: "steam-subscribed".to_string(),
        routes: Vec::new(),
        decisions: Vec::new(),
        platform_filtered: None,
        source: SourceHint::default(),
    };
    library::store(
        &*server.handle.app.driver,
        &library_of(&server),
        &library::StoreRequest {
            mod_id: "workshop-3300000001",
            manifest: &manifest,
            extracted_root: Path::new(""),
            archive: None,
            source_kind: "workshop",
            source_ref: r#"{"kind":"workshop","package":"OldName","workshop_id":"3300000001"}"#,
            custom_name: None,
        },
    )
    .await
    .unwrap();
    let refused = adopt_candidate(&mut ws, &target_id, "SteamPack", "steam_subscribed").await;
    assert_eq!(refused["data"]["error"]["code"], "already_managed", "{refused:?}");
    let listed = assert_next_reply_is_mod_list(&mut ws).await;
    assert_eq!(listed["data"]["mods"].as_array().unwrap().len(), 1);
    server.handle.shutdown().await;
}

#[tokio::test]
async fn adopt_replies_echo_the_candidate_source_and_root() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let scratch = tempfile::tempdir().unwrap();
    let (target_id, _root) = steam_library_target(&server, scratch.path(), "none").await;

    for (target, code) in [
        (target_id.as_str(), "not_a_candidate"),
        ("client-missing", "target_not_found"),
    ] {
        common::send_json(
            &mut ws,
            json!({"type": "mod_adopt", "data": {
                "target_id": target,
                "candidate_name": "SteamPack",
                "source": "local",
                "root": "Mods/SteamPack",
            }}),
        )
        .await;
        let refused = common::mods_ws::next_of_type(&mut ws, "mod_adopt").await.0;
        assert_eq!(refused["data"]["error"]["code"], code, "{refused:?}");
        assert_eq!(refused["data"]["target_id"], target);
        assert_eq!(refused["data"]["candidate_name"], "SteamPack");
        assert_eq!(refused["data"]["source"], "local");
        assert_eq!(refused["data"]["root"], "Mods/SteamPack");
    }
    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_local_mod_and_a_subscribed_package_sharing_a_name_are_told_apart_by_source() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let scratch = tempfile::tempdir().unwrap();
    let (target_id, root) = steam_library_target(&server, scratch.path(), "standard").await;
    common::mods_ws::write_ue4ss_mod(&root, "SteamPack", "main.lua", b"print('local')");

    let subscribed = adopt_candidate(&mut ws, &target_id, "SteamPack", "steam_subscribed").await;
    assert!(subscribed["data"]["error"].is_null(), "{subscribed:?}");
    assert_eq!(subscribed["data"]["mod_id"], "workshop-3300000001");
    let local = adopt_candidate(&mut ws, &target_id, "SteamPack", "local").await;
    assert!(local["data"]["error"].is_null(), "{local:?}");
    assert_eq!(local["data"]["mod_id"], "steampack-ue4ss");
    assert_eq!(local["data"]["files"], 1);
    server.handle.shutdown().await;
}

#[tokio::test]
async fn removing_a_client_target_drops_its_released_packages() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let db = &*server.handle.app.driver;
    let scratch = tempfile::tempdir().unwrap();
    let target_id = client_row(&server, "client-steam", &scratch.path().join("Palworld"), "none").await;
    store_workshop_pack(&server, "coolpack", "CoolPack", false).await;
    library::release_workshop_packages(db, "coolpack", std::slice::from_ref(&target_id))
        .await
        .unwrap();
    assert_eq!(
        library::released_packages(db, &target_id).await.unwrap(),
        vec!["CoolPack"]
    );

    common::send_json(
        &mut ws,
        json!({"type": "mod_target_remove", "data": {"target_id": target_id}}),
    )
    .await;
    let removed = common::mods_ws::next_of_type(&mut ws, "mod_target_remove").await.0;
    assert_eq!(removed["data"]["removed"], true, "{removed:?}");
    assert!(library::released_packages(db, &target_id)
        .await
        .unwrap()
        .is_empty());
    server.handle.shutdown().await;
}

#[tokio::test]
async fn installing_a_workshop_archive_on_a_docker_server_stores_it_without_enabling_it() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let root = tempfile::tempdir().unwrap();
    let target_id = docker_target(&server, root.path()).await;
    let archive = root.path().join("pack.zip");
    write_zip(
        &archive,
        &[
            (
                "Info.json",
                br#"{"PackageName":"CoolPack","Version":"1.0","InstallRules":[]}"#,
            ),
            ("CoolPack_P.pak", b"pakbytes"),
        ],
    );

    common::send_json(
        &mut ws,
        json!({"type": "mod_install", "data": {
            "path": archive.to_string_lossy(),
            "target_id": target_id,
            "accept_defaults": true,
        }}),
    )
    .await;
    let reply = common::mods_ws::next_of_type(&mut ws, "mod_install").await.0;
    assert!(reply["data"]["error"].is_null(), "{reply:?}");
    assert_eq!(
        reply["data"]["enable_error"]["code"], "not_supported_on_target",
        "{reply:?}"
    );
    assert_eq!(reply["data"]["enable_error"]["kind"], "workshop", "{reply:?}");
    let mod_id = reply["data"]["mod_id"].as_str().unwrap().to_string();

    let listed = list_profiles(&mut ws, &target_id).await;
    assert!(entry_for(&listed, &mod_id).is_none(), "{listed:?}");
    let mods = assert_next_reply_is_mod_list(&mut ws).await;
    assert!(
        mods["data"]["mods"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["id"] == mod_id.as_str()),
        "the version stays stored: {mods:?}"
    );

    let (applied, _) = apply(&mut ws, json!({"target_id": target_id})).await;
    assert!(applied["data"]["error"].is_null(), "{applied:?}");
    server.handle.shutdown().await;
}

#[tokio::test]
async fn profile_set_mod_reports_pending_while_another_apply_holds_the_target() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let mod_id = "coolmod-ue4ss";
    store_ue4ss_mod(&server, mod_id, "CoolMod/Scripts/main.lua", b"print('hi')").await;
    let guard = lock_of(&server, &target_id).await.expect("no apply is running");

    let reply = set_mod(
        &mut ws,
        json!({"target_id": target_id, "mod_id": mod_id, "enabled": true}),
    )
    .await;
    assert!(reply["data"]["error"].is_null(), "{reply:?}");
    assert_eq!(reply["data"]["pending"], true, "{reply:?}");
    assert!(reply["data"]["apply"].is_null(), "{reply:?}");

    let listed = list_profiles(&mut ws, &target_id).await;
    assert_eq!(entry_for(&listed, mod_id).unwrap()["enabled"], true);
    assert!(!deployed_path(install.path()).exists());
    drop(guard);
    server.handle.shutdown().await;
}

#[tokio::test]
async fn pinning_a_version_of_another_mod_is_refused_and_writes_nothing() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    store_ue4ss_mod(&server, "coolmod-ue4ss", "CoolMod/Scripts/main.lua", b"print('hi')").await;
    let other_version =
        store_ue4ss_mod(&server, "othermod-ue4ss", "OtherMod/Scripts/main.lua", b"print('o')")
            .await;

    let reply = set_mod(
        &mut ws,
        json!({
            "target_id": target_id,
            "mod_id": "coolmod-ue4ss",
            "mod_version_id": other_version,
            "enabled": true,
        }),
    )
    .await;
    assert_eq!(reply["data"]["error"]["code"], "version_not_of_mod", "{reply:?}");
    assert_eq!(reply["data"]["error"]["mod_version_id"], other_version.as_str());
    let listed = list_profiles(&mut ws, &target_id).await;
    assert!(entry_for(&listed, "coolmod-ue4ss").is_none(), "{listed:?}");
    server.handle.shutdown().await;
}

#[tokio::test]
async fn adopting_by_root_takes_that_candidate_when_two_share_a_name() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    common::mods::write(&install.path().join("Pal/Content/Paks/~mods/Foo.pak"), b"pak");
    common::mods::write(&install.path().join("Pal/Content/Paks/LogicMods/Foo.pak"), b"logic");

    common::send_json(
        &mut ws,
        json!({"type": "mod_target_scan", "data": {"target_id": target_id, "candidates_only": true}}),
    )
    .await;
    let scanned = common::mods_ws::next_of_type(&mut ws, "mod_target_scan").await.0;
    let same_name: Vec<Value> = scanned["data"]["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["name"] == "Foo.pak")
        .cloned()
        .collect();
    assert_eq!(same_name.len(), 2, "{scanned:?}");
    assert_ne!(same_name[0]["kind"], same_name[1]["kind"]);
    let wanted = &same_name[1];

    common::send_json(
        &mut ws,
        json!({"type": "mod_adopt", "data": {
            "target_id": target_id,
            "candidate_name": "Foo.pak",
            "source": "local",
            "root": wanted["root"],
        }}),
    )
    .await;
    let adopted = common::mods_ws::next_of_type(&mut ws, "mod_adopt").await.0;
    assert!(adopted["data"]["error"].is_null(), "{adopted:?}");
    let version = ps_db::mod_library::current_version(
        &*server.handle.app.driver,
        adopted["data"]["mod_id"].as_str().unwrap(),
    )
    .await
    .unwrap()
    .unwrap();
    let manifest: InstallManifest = serde_json::from_str(&version.manifest).unwrap();
    assert_eq!(json!(manifest.routes[0].kind), wanted["kind"], "{manifest:?}");
    server.handle.shutdown().await;
}

async fn store_subscribed_pack(
    server: &common::TestServer,
    workshop_id: &str,
    package: &str,
) -> String {
    let manifest = InstallManifest {
        folder_name: package.to_string(),
        display_name: package.to_string(),
        mod_type: ModType::Workshop,
        version: "steam-subscribed".to_string(),
        routes: Vec::new(),
        decisions: Vec::new(),
        platform_filtered: None,
        source: SourceHint::default(),
    };
    let mod_id = format!("workshop-{workshop_id}");
    let source_ref =
        json!({"kind": "workshop", "package": package, "workshop_id": workshop_id}).to_string();
    library::store(
        &*server.handle.app.driver,
        &library_of(server),
        &library::StoreRequest {
            mod_id: &mod_id,
            manifest: &manifest,
            extracted_root: Path::new(""),
            archive: None,
            source_kind: "workshop",
            source_ref: &source_ref,
            custom_name: None,
        },
    )
    .await
    .unwrap();
    mod_id
}

async fn hold_in_profile(
    server: &common::TestServer,
    target_id: &str,
    mod_id: &str,
    enabled: bool,
) {
    ps_db::mod_profiles::set_mod(
        &*server.handle.app.driver,
        &ps_db::mod_profiles::ProfileModRow {
            profile_id: format!("{target_id}/default"),
            mod_id: mod_id.to_string(),
            mod_version_id: None,
            enabled,
            load_order: 0,
        },
    )
    .await
    .unwrap();
}

async fn lock_of(
    server: &common::TestServer,
    target_id: &str,
) -> Option<tokio::sync::OwnedMutexGuard<()>> {
    let target = ps_db::mod_targets::get(&*server.handle.app.driver, target_id)
        .await
        .unwrap()
        .unwrap();
    ps_server::services::mods::deploy::try_lock_target(&library_of(server), &target)
}

#[tokio::test]
async fn mod_remove_locks_a_target_whose_profile_holds_the_mod_disabled() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let scratch = tempfile::tempdir().unwrap();
    let target_id =
        client_row(&server, "client-a", &scratch.path().join("A/Palworld"), "none").await;
    let mod_id = store_subscribed_pack(&server, "3300000001", "SteamPack").await;
    hold_in_profile(&server, &target_id, &mod_id, false).await;
    let guard = lock_of(&server, &target_id).await.expect("no apply is running");

    common::send_json(&mut ws, json!({"type": "mod_remove", "data": {"mod_id": mod_id}})).await;
    let refused = common::mods_ws::next_of_type(&mut ws, "mod_remove").await.0;
    assert_eq!(refused["data"]["error"]["code"], "apply_in_progress", "{refused:?}");
    assert_eq!(refused["data"]["error"]["target_id"], target_id.as_str());
    let listed = assert_next_reply_is_mod_list(&mut ws).await;
    assert_eq!(listed["data"]["mods"].as_array().unwrap().len(), 1);

    drop(guard);
    common::send_json(&mut ws, json!({"type": "mod_remove", "data": {"mod_id": mod_id}})).await;
    let removed = common::mods_ws::next_of_type(&mut ws, "mod_remove").await.0;
    assert_eq!(removed["data"]["removed"], true, "{removed:?}");
    server.handle.shutdown().await;
}

#[tokio::test]
async fn mod_remove_locks_every_holding_target_and_releases_them_all() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let db = &*server.handle.app.driver;
    let scratch = tempfile::tempdir().unwrap();
    let first = client_row(&server, "client-a", &scratch.path().join("A/Palworld"), "none").await;
    let second = client_row(&server, "client-b", &scratch.path().join("B/Palworld"), "none").await;
    let mod_id = store_subscribed_pack(&server, "3300000001", "SteamPack").await;
    hold_in_profile(&server, &first, &mod_id, true).await;
    hold_in_profile(&server, &second, &mod_id, true).await;

    let guard = lock_of(&server, &second).await.expect("no apply is running");
    common::send_json(&mut ws, json!({"type": "mod_remove", "data": {"mod_id": mod_id}})).await;
    let refused = common::mods_ws::next_of_type(&mut ws, "mod_remove").await.0;
    assert_eq!(refused["data"]["error"]["code"], "apply_in_progress", "{refused:?}");
    assert_eq!(refused["data"]["error"]["target_id"], second.as_str());
    assert!(
        lock_of(&server, &first).await.is_some(),
        "a refused removal lets go of the locks it already took"
    );
    drop(guard);

    common::send_json(&mut ws, json!({"type": "mod_remove", "data": {"mod_id": mod_id}})).await;
    let removed = common::mods_ws::next_of_type(&mut ws, "mod_remove").await.0;
    assert_eq!(removed["data"]["removed"], true, "{removed:?}");
    for target_id in [&first, &second] {
        assert_eq!(
            library::released_packages(db, target_id).await.unwrap(),
            vec!["SteamPack"],
            "{target_id}"
        );
        assert!(
            lock_of(&server, target_id).await.is_some(),
            "{target_id} is still locked after the removal"
        );
    }
    server.handle.shutdown().await;
}
