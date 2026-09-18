mod common;

use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

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

async fn request(ws: &mut common::WsClient, kind: &str, data: Value) -> Value {
    common::send_json(ws, json!({"type": kind, "data": data})).await;
    common::mods_ws::next_of_type(ws, kind).await.0
}

fn profile_named<'a>(listed: &'a Value, name: &str) -> &'a Value {
    listed["data"]["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == name)
        .unwrap_or_else(|| panic!("no profile {name} in {listed:?}"))
}

async fn create(ws: &mut common::WsClient, target_id: &str, name: &str) -> Value {
    request(
        ws,
        "profile_create",
        json!({"target_id": target_id, "name": name}),
    )
    .await
}

fn created_id(reply: &Value) -> String {
    assert!(reply["data"]["error"].is_null(), "{reply:?}");
    reply["data"]["profile"]["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn a_created_profile_is_inactive_with_a_slug_id() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;

    let reply = create(&mut ws, &target_id, "Hard Mode").await;
    assert!(reply["data"]["error"].is_null(), "{reply:?}");
    assert_eq!(reply["data"]["target_id"], target_id);
    let profile = &reply["data"]["profile"];
    assert_eq!(profile["id"], format!("{target_id}/hard-mode"));
    assert_eq!(profile["name"], "Hard Mode");
    assert_eq!(profile["is_active"], false);
    assert_eq!(profile["is_default"], false);
    assert_eq!(profile["mods"], json!([]));
    assert_eq!(profile["worlds"], json!([]));

    let listed = list_profiles(&mut ws, &target_id).await;
    let profiles = listed["data"]["profiles"].as_array().unwrap();
    assert_eq!(profiles.len(), 2, "{profiles:?}");
    assert_eq!(profiles[0]["id"], format!("{target_id}/default"));
    assert_eq!(profiles[0]["worlds"], json!([]));
    assert_eq!(profiles[1]["id"], format!("{target_id}/hard-mode"));
    server.handle.shutdown().await;
}

#[tokio::test]
async fn creating_a_second_profile_with_the_same_slug_gets_a_suffix() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;

    let first = create(&mut ws, &target_id, "Hard Mode").await;
    assert_eq!(created_id(&first), format!("{target_id}/hard-mode"));
    let second = create(&mut ws, &target_id, "hard-mode!").await;
    assert_eq!(created_id(&second), format!("{target_id}/hard-mode-2"));
    let third = create(&mut ws, &target_id, "Default ").await;
    assert_eq!(
        third["data"]["error"]["code"], "name_taken",
        "the default already holds the name: {third:?}"
    );
    let third = create(&mut ws, &target_id, "Default!").await;
    assert_eq!(created_id(&third), format!("{target_id}/default-2"));
    server.handle.shutdown().await;
}

#[tokio::test]
async fn names_are_trimmed_and_checked() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;

    let blank = create(&mut ws, &target_id, "  ").await;
    assert_eq!(blank["data"]["error"]["code"], "invalid_name", "{blank:?}");
    assert_eq!(blank["data"]["target_id"], target_id);
    assert_eq!(blank["data"]["name"], "  ");

    let long = create(&mut ws, &target_id, &"a".repeat(65)).await;
    assert_eq!(long["data"]["error"]["code"], "invalid_name", "{long:?}");
    let longest = create(&mut ws, &target_id, &"a".repeat(64)).await;
    assert!(longest["data"]["error"].is_null(), "{longest:?}");

    let first = create(&mut ws, &target_id, "Hard Mode").await;
    let first_id = created_id(&first);
    let taken = create(&mut ws, &target_id, "HARD MODE").await;
    assert_eq!(taken["data"]["error"]["code"], "name_taken", "{taken:?}");
    assert_eq!(taken["data"]["error"]["profile_id"], first_id);

    let spaced = create(&mut ws, &target_id, "  Spaced  ").await;
    assert!(spaced["data"]["error"].is_null(), "{spaced:?}");
    assert_eq!(spaced["data"]["profile"]["name"], "Spaced");

    let listed = list_profiles(&mut ws, &target_id).await;
    assert_eq!(listed["data"]["profiles"].as_array().unwrap().len(), 4);
    server.handle.shutdown().await;
}

#[tokio::test]
async fn copy_from_copies_entries_and_pins() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let (mod_id, version_id) =
        install_cool_mod(&mut ws, install.path(), &target_id, b"print('hi')", true).await;
    let pinned = set_mod(
        &mut ws,
        json!({"target_id": target_id, "mod_id": mod_id, "enabled": true, "mod_version_id": version_id}),
    )
    .await;
    assert!(pinned["data"]["error"].is_null(), "{pinned:?}");

    let reply = request(
        &mut ws,
        "profile_create",
        json!({"target_id": target_id, "name": "Copy", "copy_from": format!("{target_id}/default")}),
    )
    .await;
    assert!(reply["data"]["error"].is_null(), "{reply:?}");
    let copy_id = format!("{target_id}/copy");
    let mods = reply["data"]["profile"]["mods"].as_array().unwrap();
    assert_eq!(mods.len(), 1, "{reply:?}");
    let listed = list_profiles(&mut ws, &target_id).await;
    let source = &profile_named(&listed, "Default")["mods"][0];
    assert_eq!(mods[0]["profile_id"], copy_id);
    for field in ["mod_id", "enabled", "load_order", "mod_version_id"] {
        assert_eq!(mods[0][field], source[field], "{field}: {reply:?}");
    }
    assert_eq!(mods[0]["mod_version_id"], version_id);
    assert_eq!(mods[0]["enabled"], true);

    let other_install = common::mods_ws::fake_windows_install();
    let other_id = add_target(&mut ws, other_install.path()).await;
    let foreign = request(
        &mut ws,
        "profile_create",
        json!({"target_id": target_id, "name": "Foreign", "copy_from": format!("{other_id}/default")}),
    )
    .await;
    assert_eq!(
        foreign["data"]["error"]["code"], "profile_not_found",
        "{foreign:?}"
    );
    assert_eq!(
        foreign["data"]["error"]["profile_id"],
        format!("{other_id}/default")
    );
    let listed = list_profiles(&mut ws, &target_id).await;
    assert_eq!(listed["data"]["profiles"].as_array().unwrap().len(), 2);
    server.handle.shutdown().await;
}

#[tokio::test]
async fn rename_replies_with_the_profile_and_refuses_a_taken_name() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let hard = created_id(&create(&mut ws, &target_id, "Hard Mode").await);
    let easy = created_id(&create(&mut ws, &target_id, "Easy").await);

    let renamed = request(
        &mut ws,
        "profile_rename",
        json!({"target_id": target_id, "profile_id": hard, "name": " Brutal "}),
    )
    .await;
    assert!(renamed["data"]["error"].is_null(), "{renamed:?}");
    assert_eq!(renamed["data"]["target_id"], target_id);
    assert_eq!(renamed["data"]["profile"]["id"], hard, "the id is kept");
    assert_eq!(renamed["data"]["profile"]["name"], "Brutal");

    let recased = request(
        &mut ws,
        "profile_rename",
        json!({"target_id": target_id, "profile_id": hard, "name": "BRUTAL"}),
    )
    .await;
    assert!(
        recased["data"]["error"].is_null(),
        "a profile may take its own name: {recased:?}"
    );

    let taken = request(
        &mut ws,
        "profile_rename",
        json!({"target_id": target_id, "profile_id": hard, "name": "easy"}),
    )
    .await;
    assert_eq!(taken["data"]["error"]["code"], "name_taken", "{taken:?}");
    assert_eq!(taken["data"]["error"]["profile_id"], easy);
    assert_eq!(taken["data"]["profile_id"], hard);

    let missing = request(
        &mut ws,
        "profile_rename",
        json!({"target_id": target_id, "profile_id": format!("{target_id}/nope"), "name": "X"}),
    )
    .await;
    assert_eq!(
        missing["data"]["error"]["code"], "profile_not_found",
        "{missing:?}"
    );

    let listed = list_profiles(&mut ws, &target_id).await;
    assert_eq!(profile_named(&listed, "BRUTAL")["id"], hard);
    server.handle.shutdown().await;
}

#[tokio::test]
async fn activating_applies_the_new_selection() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    install_cool_mod(&mut ws, install.path(), &target_id, b"print('hi')", true).await;
    let (applied, _) = apply(&mut ws, json!({"target_id": target_id})).await;
    assert!(applied["data"]["error"].is_null(), "{applied:?}");
    assert!(deployed_path(install.path()).is_file());
    let empty = created_id(&create(&mut ws, &target_id, "Empty").await);

    let reply = request(
        &mut ws,
        "profile_activate",
        json!({"target_id": target_id, "profile_id": empty}),
    )
    .await;
    assert!(reply["data"]["error"].is_null(), "{reply:?}");
    assert_eq!(reply["data"]["target_id"], target_id);
    assert_eq!(reply["data"]["profile_id"], empty);
    assert_eq!(reply["data"]["pending"], false, "{reply:?}");
    assert!(reply["data"]["request_id"].is_string(), "{reply:?}");
    assert!(reply["data"]["apply"].is_object(), "{reply:?}");
    assert!(reply["data"]["apply"]["error"].is_null(), "{reply:?}");
    assert!(!deployed_path(install.path()).exists());

    let listed = list_profiles(&mut ws, &target_id).await;
    assert_eq!(profile_named(&listed, "Empty")["is_active"], true);
    assert_eq!(profile_named(&listed, "Default")["is_active"], false);

    let back = request(
        &mut ws,
        "profile_activate",
        json!({"target_id": target_id, "profile_id": format!("{target_id}/default")}),
    )
    .await;
    assert!(back["data"]["apply"]["error"].is_null(), "{back:?}");
    assert_eq!(
        std::fs::read_to_string(deployed_path(install.path())).unwrap(),
        "print('hi')"
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn deleting_the_active_profile_falls_back_to_default_and_applies() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    install_cool_mod(&mut ws, install.path(), &target_id, b"print('hi')", true).await;
    let empty = created_id(&create(&mut ws, &target_id, "Empty").await);
    let activated = request(
        &mut ws,
        "profile_activate",
        json!({"target_id": target_id, "profile_id": empty}),
    )
    .await;
    assert!(
        activated["data"]["apply"]["error"].is_null(),
        "{activated:?}"
    );
    assert!(!deployed_path(install.path()).exists());

    let reply = request(
        &mut ws,
        "profile_delete",
        json!({"target_id": target_id, "profile_id": empty}),
    )
    .await;
    assert!(reply["data"]["error"].is_null(), "{reply:?}");
    assert_eq!(reply["data"]["target_id"], target_id);
    assert_eq!(reply["data"]["profile_id"], empty);
    assert_eq!(reply["data"]["deleted"], true);
    assert_eq!(
        reply["data"]["active_profile_id"],
        format!("{target_id}/default")
    );
    assert_eq!(reply["data"]["pending"], false, "{reply:?}");
    assert!(reply["data"]["request_id"].is_string(), "{reply:?}");
    assert!(reply["data"]["apply"]["error"].is_null(), "{reply:?}");
    assert!(deployed_path(install.path()).is_file());

    let listed = list_profiles(&mut ws, &target_id).await;
    let profiles = listed["data"]["profiles"].as_array().unwrap();
    assert_eq!(profiles.len(), 1, "{profiles:?}");
    assert_eq!(profiles[0]["is_active"], true);
    server.handle.shutdown().await;
}

#[tokio::test]
async fn deleting_an_inactive_profile_applies_nothing() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let spare = created_id(&create(&mut ws, &target_id, "Spare").await);

    let reply = request(
        &mut ws,
        "profile_delete",
        json!({"target_id": target_id, "profile_id": spare}),
    )
    .await;
    assert!(reply["data"]["error"].is_null(), "{reply:?}");
    assert_eq!(reply["data"]["deleted"], true);
    assert_eq!(
        reply["data"]["active_profile_id"],
        format!("{target_id}/default")
    );
    assert!(reply["data"]["request_id"].is_null(), "{reply:?}");
    assert!(reply["data"]["apply"].is_null(), "{reply:?}");
    assert_eq!(reply["data"]["pending"], false);

    let again = request(
        &mut ws,
        "profile_delete",
        json!({"target_id": target_id, "profile_id": spare}),
    )
    .await;
    assert_eq!(
        again["data"]["error"]["code"], "profile_not_found",
        "{again:?}"
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn the_default_profile_cannot_be_deleted() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let default_id = format!("{target_id}/default");

    let reply = request(
        &mut ws,
        "profile_delete",
        json!({"target_id": target_id, "profile_id": default_id}),
    )
    .await;
    assert_eq!(
        reply["data"]["error"]["code"], "default_profile",
        "{reply:?}"
    );
    assert_eq!(reply["data"]["target_id"], target_id);
    assert_eq!(reply["data"]["profile_id"], default_id);
    assert!(reply["data"]["deleted"].is_null(), "{reply:?}");
    let listed = list_profiles(&mut ws, &target_id).await;
    assert_eq!(listed["data"]["profiles"].as_array().unwrap().len(), 1);
    server.handle.shutdown().await;
}

#[tokio::test]
async fn set_mod_on_an_inactive_profile_commits_without_applying() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let (mod_id, _) =
        install_cool_mod(&mut ws, install.path(), &target_id, b"print('hi')", false).await;
    let other = created_id(&create(&mut ws, &target_id, "Other").await);

    let reply = set_mod(
        &mut ws,
        json!({"target_id": target_id, "profile_id": other, "mod_id": mod_id, "enabled": true}),
    )
    .await;
    assert!(reply["data"]["error"].is_null(), "{reply:?}");
    assert_eq!(reply["data"]["profile_id"], other);
    assert_eq!(reply["data"]["enabled"], true);
    assert!(reply["data"]["mod_version_id"].is_null(), "{reply:?}");
    assert!(reply["data"]["apply"].is_null(), "{reply:?}");
    assert!(reply["data"]["request_id"].is_null(), "{reply:?}");
    assert_eq!(reply["data"]["pending"], false);
    assert!(!deployed_path(install.path()).exists());

    let listed = list_profiles(&mut ws, &target_id).await;
    let other_mods = profile_named(&listed, "Other")["mods"].as_array().unwrap();
    assert_eq!(other_mods.len(), 1, "{listed:?}");
    assert_eq!(other_mods[0]["mod_id"], mod_id);
    assert_eq!(other_mods[0]["enabled"], true);
    assert!(
        profile_named(&listed, "Default")["mods"]
            .as_array()
            .unwrap()
            .iter()
            .all(|entry| entry["enabled"] == false),
        "{listed:?}"
    );

    let missing = set_mod(
        &mut ws,
        json!({"target_id": target_id, "profile_id": format!("{target_id}/nope"),
               "mod_id": mod_id, "enabled": true}),
    )
    .await;
    assert_eq!(
        missing["data"]["error"]["code"], "profile_not_found",
        "{missing:?}"
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn disabling_a_mod_leaves_no_empty_folder_behind() {
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

    assert!(reply["data"]["error"].is_null(), "{reply:?}");
    assert!(reply["data"]["apply"]["error"].is_null(), "{reply:?}");
    let mods_dir = common::mods_ws::ue4ss_mods_dir(install.path());
    assert!(!mods_dir.join("CoolMod").exists());
    assert!(mods_dir.is_dir());
    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_profile_of_another_target_is_not_found() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install_a = common::mods_ws::fake_windows_install();
    let install_b = common::mods_ws::fake_windows_install();
    let target_a = add_target(&mut ws, install_a.path()).await;
    let target_b = add_target(&mut ws, install_b.path()).await;
    let foreign = format!("{target_b}/default");

    let reply = request(
        &mut ws,
        "profile_activate",
        json!({"target_id": target_a, "profile_id": foreign}),
    )
    .await;
    assert_eq!(
        reply["data"]["error"]["code"], "profile_not_found",
        "{reply:?}"
    );
    assert_eq!(reply["data"]["error"]["profile_id"], foreign);
    assert_eq!(reply["data"]["target_id"], target_a);

    let unknown = request(
        &mut ws,
        "profile_activate",
        json!({"target_id": "client-nope", "profile_id": foreign}),
    )
    .await;
    assert_eq!(
        unknown["data"]["error"]["code"], "target_not_found",
        "{unknown:?}"
    );

    let listed = list_profiles(&mut ws, &target_b).await;
    assert_eq!(profile_named(&listed, "Default")["is_active"], true);
    server.handle.shutdown().await;
}
