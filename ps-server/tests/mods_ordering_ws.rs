mod common;

use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use ps_core::mods::{FileRoute, InstallManifest, ModType, RouteKind, SourceHint};
use ps_server::services::mods::{library, LibraryPaths};

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

struct Target {
    id: String,
    palschema_dir: PathBuf,
}

async fn add_target(ws: &mut common::WsClient, root: &Path) -> Target {
    common::send_json(
        ws,
        json!({"type": "mod_target_add", "data": {"root_path": root.to_string_lossy()}}),
    )
    .await;
    let reply = common::next_json(ws).await;
    assert_eq!(reply["type"], "mod_target_add", "{reply:?}");
    let target = &reply["data"]["target"];
    Target {
        id: target["id"].as_str().unwrap().to_string(),
        palschema_dir: PathBuf::from(target["layout"]["palschema_mods_dir"].as_str().unwrap()),
    }
}

async fn store_mod(
    server: &common::TestServer,
    mod_id: &str,
    mod_type: ModType,
    kind: RouteKind,
    rel_path: &str,
    body: &[u8],
) {
    let scratch = tempfile::tempdir().unwrap();
    common::mods::write(&scratch.path().join(rel_path), body);
    let folder = rel_path.split('/').next().unwrap().to_string();
    let manifest = InstallManifest {
        folder_name: folder.clone(),
        display_name: folder,
        mod_type,
        version: "1.0".to_string(),
        routes: vec![FileRoute {
            archive_path: rel_path.to_string(),
            rel_path: rel_path.to_string(),
            kind,
        }],
        decisions: Vec::new(),
        platform_filtered: None,
        source: SourceHint::default(),
    };
    library::store(
        &*server.handle.app.driver,
        &LibraryPaths::new(server._temp_dir.path()),
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

async fn store_ue4ss_mod(server: &common::TestServer, mod_id: &str, rel_path: &str, body: &[u8]) {
    store_mod(
        server,
        mod_id,
        ModType::Ue4ss,
        RouteKind::Ue4ss,
        rel_path,
        body,
    )
    .await;
}

async fn store_palschema_mod(
    server: &common::TestServer,
    mod_id: &str,
    folder: &str,
    file: &str,
    body: &[u8],
) {
    store_mod(
        server,
        mod_id,
        ModType::PalSchema,
        RouteKind::PalSchema,
        &format!("{folder}/{file}"),
        body,
    )
    .await;
}

async fn request(ws: &mut common::WsClient, kind: &str, data: Value) -> Value {
    common::send_json(ws, json!({"type": kind, "data": data})).await;
    common::mods_ws::next_of_type(ws, kind).await.0
}

async fn enable(ws: &mut common::WsClient, target_id: &str, mod_id: &str) {
    let reply = request(
        ws,
        "profile_set_mod",
        json!({"target_id": target_id, "mod_id": mod_id, "enabled": true}),
    )
    .await;
    assert!(reply["data"]["error"].is_null(), "{reply:?}");
    assert!(reply["data"]["apply"]["error"].is_null(), "{reply:?}");
}

async fn reorder(
    ws: &mut common::WsClient,
    target_id: &str,
    kind: &str,
    ordered_mod_ids: &[&str],
) -> Value {
    request(
        ws,
        "profile_reorder",
        json!({"target_id": target_id, "kind": kind, "ordered_mod_ids": ordered_mod_ids}),
    )
    .await
}

async fn default_profile(ws: &mut common::WsClient, target_id: &str) -> Value {
    let listed = request(ws, "profile_list", json!({"target_id": target_id})).await;
    listed["data"]["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["is_default"] == true)
        .cloned()
        .unwrap_or_else(|| panic!("no default profile in {listed:?}"))
}

async fn load_order_of(ws: &mut common::WsClient, target_id: &str, mod_id: &str) -> i64 {
    let profile = default_profile(ws, target_id).await;
    profile["mods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["mod_id"] == mod_id)
        .unwrap_or_else(|| panic!("no {mod_id} in {profile:?}"))["load_order"]
        .as_i64()
        .unwrap()
}

fn mods_txt_lines(root: &Path) -> Vec<String> {
    std::fs::read_to_string(common::mods_ws::ue4ss_mods_dir(root).join("mods.txt"))
        .unwrap()
        .lines()
        .map(|line| line.trim().to_string())
        .collect()
}

fn line_index(lines: &[String], line: &str) -> usize {
    lines
        .iter()
        .position(|l| l == line)
        .unwrap_or_else(|| panic!("no {line:?} in {lines:?}"))
}

#[tokio::test]
async fn reordering_ue4ss_mods_rewrites_the_mods_txt_block() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target = add_target(&mut ws, install.path()).await;
    store_ue4ss_mod(&server, "alpha", "Alpha/Scripts/main.lua", b"alpha").await;
    store_palschema_mod(&server, "middle", "Middle", "m.json", b"{}").await;
    store_ue4ss_mod(&server, "beta", "Beta/Scripts/main.lua", b"beta").await;
    for mod_id in ["alpha", "middle", "beta"] {
        enable(&mut ws, &target.id, mod_id).await;
    }
    let lines = mods_txt_lines(install.path());
    assert!(
        line_index(&lines, "Alpha : 1") < line_index(&lines, "Beta : 1"),
        "{lines:?}"
    );
    let middle_order = load_order_of(&mut ws, &target.id, "middle").await;

    let reply = reorder(&mut ws, &target.id, "ue4ss", &["beta", "alpha"]).await;

    assert!(reply["data"]["error"].is_null(), "{reply:?}");
    assert_eq!(reply["data"]["target_id"], target.id);
    assert_eq!(
        reply["data"]["profile_id"],
        format!("{}/default", target.id)
    );
    assert_eq!(reply["data"]["kind"], "ue4ss");
    assert_eq!(reply["data"]["ordered_mod_ids"], json!(["beta", "alpha"]));
    assert_eq!(reply["data"]["pending"], false, "{reply:?}");
    assert!(reply["data"]["request_id"].is_string(), "{reply:?}");
    assert!(reply["data"]["apply"].is_object(), "{reply:?}");
    assert!(reply["data"]["apply"]["error"].is_null(), "{reply:?}");
    let lines = mods_txt_lines(install.path());
    assert!(
        line_index(&lines, "Beta : 1") < line_index(&lines, "Alpha : 1"),
        "{lines:?}"
    );
    let beta_order = load_order_of(&mut ws, &target.id, "beta").await;
    let alpha_order = load_order_of(&mut ws, &target.id, "alpha").await;
    assert_eq!(
        load_order_of(&mut ws, &target.id, "middle").await,
        middle_order,
        "another kind keeps its position"
    );
    assert!(
        beta_order < middle_order && middle_order < alpha_order,
        "beta {beta_order}, middle {middle_order}, alpha {alpha_order}"
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_partial_or_foreign_order_is_refused() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target = add_target(&mut ws, install.path()).await;
    store_ue4ss_mod(&server, "alpha", "Alpha/Scripts/main.lua", b"alpha").await;
    store_ue4ss_mod(&server, "beta", "Beta/Scripts/main.lua", b"beta").await;
    enable(&mut ws, &target.id, "alpha").await;
    enable(&mut ws, &target.id, "beta").await;
    let before = default_profile(&mut ws, &target.id).await;

    let partial = reorder(&mut ws, &target.id, "ue4ss", &["beta"]).await;
    assert_eq!(
        partial["data"]["error"]["code"], "invalid_order",
        "{partial:?}"
    );
    assert_eq!(
        partial["data"]["error"]["expected"],
        json!(["alpha", "beta"])
    );
    assert_eq!(partial["data"]["target_id"], target.id);
    assert_eq!(partial["data"]["kind"], "ue4ss");

    let foreign = reorder(&mut ws, &target.id, "ue4ss", &["alpha", "nope"]).await;
    assert_eq!(
        foreign["data"]["error"]["code"], "invalid_order",
        "{foreign:?}"
    );

    let repeated = reorder(&mut ws, &target.id, "ue4ss", &["beta", "alpha", "beta"]).await;
    assert_eq!(
        repeated["data"]["error"]["code"], "invalid_order",
        "{repeated:?}"
    );

    let pak = reorder(&mut ws, &target.id, "pak", &["alpha", "beta"]).await;
    assert_eq!(pak["data"]["error"]["code"], "invalid_kind", "{pak:?}");
    assert_eq!(pak["data"]["error"]["kind"], "pak");

    let after = default_profile(&mut ws, &target.id).await;
    assert_eq!(after["mods"], before["mods"]);
    server.handle.shutdown().await;
}

#[tokio::test]
async fn palschema_force_order_moves_folders_and_carries_edits() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target = add_target(&mut ws, install.path()).await;
    store_palschema_mod(&server, "a", "A", "x.json", b"{\"a\": 1}").await;
    store_palschema_mod(&server, "b", "B", "y.json", b"{\"b\": 1}").await;
    enable(&mut ws, &target.id, "a").await;
    enable(&mut ws, &target.id, "b").await;
    assert!(target.palschema_dir.join("A/x.json").is_file());

    let options = request(
        &mut ws,
        "profile_set_options",
        json!({"target_id": target.id, "force_order_palschema": true}),
    )
    .await;
    assert!(options["data"]["error"].is_null(), "{options:?}");
    assert!(options["data"]["apply"]["error"].is_null(), "{options:?}");
    assert!(target.palschema_dir.join("001_A/x.json").is_file());
    assert!(target.palschema_dir.join("002_B/y.json").is_file());
    assert!(!target.palschema_dir.join("A").exists());
    assert!(!target.palschema_dir.join("B").exists());

    std::fs::write(target.palschema_dir.join("001_A/x.json"), "edited").unwrap();
    let reply = reorder(&mut ws, &target.id, "palschema", &["b", "a"]).await;

    assert!(reply["data"]["error"].is_null(), "{reply:?}");
    assert!(reply["data"]["apply"]["error"].is_null(), "{reply:?}");
    assert!(target.palschema_dir.join("001_B/y.json").is_file());
    assert_eq!(
        std::fs::read_to_string(target.palschema_dir.join("002_A/x.json")).unwrap(),
        "edited"
    );
    assert_eq!(reply["data"]["apply"]["counts"]["move"], 2, "{reply:?}");
    assert!(!target.palschema_dir.join("001_A").exists());
    assert!(!target.palschema_dir.join("002_B").exists());

    let off = request(
        &mut ws,
        "profile_set_options",
        json!({"target_id": target.id, "force_order_palschema": false}),
    )
    .await;
    assert!(off["data"]["error"].is_null(), "{off:?}");
    assert!(off["data"]["apply"]["error"].is_null(), "{off:?}");
    assert_eq!(
        std::fs::read_to_string(target.palschema_dir.join("A/x.json")).unwrap(),
        "edited"
    );
    assert!(target.palschema_dir.join("B/y.json").is_file());
    assert!(!target.palschema_dir.join("002_A").exists());
    assert!(!target.palschema_dir.join("001_B").exists());
    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_user_file_keeps_its_folder_and_the_base_outlives_its_last_mod() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target = add_target(&mut ws, install.path()).await;
    store_palschema_mod(&server, "a", "A", "x.json", b"{\"a\": 1}").await;
    enable(&mut ws, &target.id, "a").await;
    std::fs::write(target.palschema_dir.join("A/readme.txt"), "mine").unwrap();

    let options = request(
        &mut ws,
        "profile_set_options",
        json!({"target_id": target.id, "force_order_palschema": true}),
    )
    .await;
    assert!(options["data"]["apply"]["error"].is_null(), "{options:?}");
    assert!(target.palschema_dir.join("001_A/x.json").is_file());
    assert!(!target.palschema_dir.join("A/x.json").exists());
    assert!(target.palschema_dir.join("A/readme.txt").is_file());

    let disabled = request(
        &mut ws,
        "profile_set_mod",
        json!({"target_id": target.id, "mod_id": "a", "enabled": false}),
    )
    .await;
    assert!(disabled["data"]["error"].is_null(), "{disabled:?}");
    assert!(disabled["data"]["apply"]["error"].is_null(), "{disabled:?}");
    assert!(!target.palschema_dir.join("001_A").exists());
    assert!(target.palschema_dir.is_dir(), "the base itself survives");
    assert!(target.palschema_dir.join("A/readme.txt").is_file());
    server.handle.shutdown().await;
}

#[tokio::test]
async fn mods_txt_mode_removes_a_deployed_enabled_txt() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target = add_target(&mut ws, install.path()).await;
    let archive = install.path().join("CoolMod-1.0.zip");
    write_zip(
        &archive,
        &[
            ("CoolMod/enabled.txt", b""),
            ("CoolMod/Scripts/main.lua", b"print('hi')"),
        ],
    );
    let installed = request(
        &mut ws,
        "mod_install",
        json!({"path": archive.to_string_lossy(), "target_id": target.id, "enable": true}),
    )
    .await;
    assert!(installed["data"]["error"].is_null(), "{installed:?}");
    let applied = request(&mut ws, "profile_apply", json!({"target_id": target.id})).await;
    assert!(applied["data"]["error"].is_null(), "{applied:?}");
    let mod_dir = common::mods_ws::ue4ss_mods_dir(install.path()).join("CoolMod");
    assert!(mod_dir.join("enabled.txt").is_file());

    let reply = request(
        &mut ws,
        "profile_set_options",
        json!({"target_id": target.id, "ue4ss_control_mode": "mods_txt"}),
    )
    .await;

    assert!(reply["data"]["error"].is_null(), "{reply:?}");
    assert!(reply["data"]["apply"]["error"].is_null(), "{reply:?}");
    assert_eq!(reply["data"]["profile"]["ue4ss_control_mode"], "mods_txt");
    assert!(!mod_dir.join("enabled.txt").exists());
    assert!(mod_dir.join("Scripts/main.lua").is_file());
    let lines = mods_txt_lines(install.path());
    line_index(&lines, "CoolMod : 1");
    server.handle.shutdown().await;
}

#[tokio::test]
async fn an_unknown_control_mode_is_refused() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target = add_target(&mut ws, install.path()).await;

    let reply = request(
        &mut ws,
        "profile_set_options",
        json!({"target_id": target.id, "ue4ss_control_mode": "sometimes", "force_order_palschema": true}),
    )
    .await;

    assert_eq!(
        reply["data"]["error"]["code"], "invalid_option",
        "{reply:?}"
    );
    assert_eq!(reply["data"]["error"]["ue4ss_control_mode"], "sometimes");
    assert_eq!(reply["data"]["target_id"], target.id);
    let profile = default_profile(&mut ws, &target.id).await;
    assert_eq!(profile["ue4ss_control_mode"], "enabled_txt");
    assert_eq!(profile["force_order_ue4ss"], false);
    assert_eq!(profile["force_order_palschema"], false);
    server.handle.shutdown().await;
}

#[tokio::test]
async fn options_on_an_inactive_profile_apply_nothing() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target = add_target(&mut ws, install.path()).await;
    let created = request(
        &mut ws,
        "profile_create",
        json!({"target_id": target.id, "name": "Other"}),
    )
    .await;
    assert!(created["data"]["error"].is_null(), "{created:?}");
    let other = created["data"]["profile"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let reply = request(
        &mut ws,
        "profile_set_options",
        json!({"target_id": target.id, "profile_id": other, "force_order_palschema": true}),
    )
    .await;

    assert!(reply["data"]["error"].is_null(), "{reply:?}");
    assert_eq!(reply["data"]["target_id"], target.id);
    assert!(reply["data"]["apply"].is_null(), "{reply:?}");
    assert!(reply["data"]["request_id"].is_null(), "{reply:?}");
    assert_eq!(reply["data"]["pending"], false);
    let profile = &reply["data"]["profile"];
    assert_eq!(profile["id"], other);
    assert_eq!(profile["is_active"], false);
    assert_eq!(profile["force_order_palschema"], true);
    assert_eq!(profile["force_order_ue4ss"], false);
    assert_eq!(profile["ue4ss_control_mode"], "enabled_txt");
    let default = default_profile(&mut ws, &target.id).await;
    assert_eq!(default["force_order_palschema"], false);
    server.handle.shutdown().await;
}
