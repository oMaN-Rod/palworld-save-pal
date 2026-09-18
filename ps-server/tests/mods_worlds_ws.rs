mod common;

use serde_json::{json, Value};

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// Builds a synthetic one-save wgs container under `root`, reusing the
/// committed world1 `LevelMeta.sav` so the scanner reads a real world name.
/// Returns the expected world name and the save id.
fn build_one_save_gamepass_tree(root: &std::path::Path) -> (String, String) {
    let meta_bytes =
        std::fs::read(repo_root().join("tests/fixtures/saves/world1/LevelMeta.sav")).unwrap();
    let expected_world_name =
        ps_core::gamepass::scan::world_name_from_level_meta(&meta_bytes).unwrap();
    let save_id = "ABCDEF0123456789ABCDEF0123456789".to_string();
    let save = ps_core::gamepass::fixture::SyntheticSave {
        save_id: save_id.clone(),
        level_sav: b"LEVEL-PLACEHOLDER".to_vec(),
        level_meta: Some(meta_bytes),
        local_data: None,
        world_option: None,
        players: vec![],
    };
    ps_core::gamepass::fixture::build_wgs_tree(root, &[save]).unwrap();
    (expected_world_name, save_id)
}

async fn add_target(ws: &mut common::WsClient, root: &std::path::Path) -> String {
    common::send_json(
        ws,
        json!({"type": "mod_target_add", "data": {"root_path": root.to_string_lossy()}}),
    )
    .await;
    let reply = common::next_json(ws).await;
    assert_eq!(reply["type"], "mod_target_add", "{reply:?}");
    reply["data"]["target"]["id"].as_str().unwrap().to_string()
}

async fn native_server_target(server: &common::TestServer, root: &std::path::Path) -> String {
    let db = &*server.handle.app.driver;
    let record = ps_db::servers::create_server(
        db,
        ps_db::servers::NewServer {
            name: "Native".to_string(),
            container_name: "native".to_string(),
            server_type: "native".to_string(),
            install_path: root.to_string_lossy().into_owned(),
            mods_path: common::mods_ws::ue4ss_mods_dir(root)
                .to_string_lossy()
                .into_owned(),
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
    ps_db::mod_targets::ensure_server_target(db, &record, "unused")
        .await
        .unwrap()
        .unwrap()
}

async fn request(ws: &mut common::WsClient, kind: &str, data: Value) -> Value {
    common::send_json(ws, json!({"type": kind, "data": data})).await;
    common::mods_ws::next_of_type(ws, kind).await.0
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

async fn list_profiles(ws: &mut common::WsClient, target_id: &str) -> Value {
    request(ws, "profile_list", json!({"target_id": target_id})).await
}

fn profile_named<'a>(listed: &'a Value, name: &str) -> &'a Value {
    listed["data"]["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == name)
        .unwrap_or_else(|| panic!("no profile {name} in {listed:?}"))
}

async fn set_world(
    ws: &mut common::WsClient,
    world_key: &str,
    world_name: &str,
    profile_id: Option<&str>,
) -> Value {
    request(
        ws,
        "world_profile_set",
        json!({"world_key": world_key, "world_name": world_name, "profile_id": profile_id}),
    )
    .await
}

#[tokio::test]
async fn linking_a_world_lists_it_on_the_profile() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let hard = created_id(&create(&mut ws, &target_id, "Hard").await);

    let reply = set_world(&mut ws, "C:/saves/abc", "Abc", Some(&hard)).await;
    assert!(reply["data"]["error"].is_null(), "{reply:?}");
    assert_eq!(reply["data"]["world_key"], "C:/saves/abc");
    assert_eq!(reply["data"]["world_name"], "Abc");
    assert_eq!(reply["data"]["profile_id"], hard);
    assert_eq!(reply["data"]["linked"], true);

    let listed = list_profiles(&mut ws, &target_id).await;
    assert_eq!(
        profile_named(&listed, "Hard")["worlds"],
        json!([{ "world_key": "C:/saves/abc", "world_name": "Abc" }])
    );
    assert_eq!(profile_named(&listed, "Default")["worlds"], json!([]));
    server.handle.shutdown().await;
}

#[tokio::test]
async fn relinking_moves_the_world_and_null_unlinks() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let hard = created_id(&create(&mut ws, &target_id, "Hard").await);
    let default_id = format!("{target_id}/default");

    let linked = set_world(&mut ws, "C:/saves/abc", "Abc", Some(&hard)).await;
    assert!(linked["data"]["error"].is_null(), "{linked:?}");

    let moved = set_world(&mut ws, "C:/saves/abc", "Abc", Some(&default_id)).await;
    assert!(moved["data"]["error"].is_null(), "{moved:?}");
    assert_eq!(moved["data"]["linked"], true);

    let listed = list_profiles(&mut ws, &target_id).await;
    assert_eq!(profile_named(&listed, "Hard")["worlds"], json!([]));
    assert_eq!(
        profile_named(&listed, "Default")["worlds"],
        json!([{ "world_key": "C:/saves/abc", "world_name": "Abc" }])
    );

    let unlinked = set_world(&mut ws, "C:/saves/abc", "Abc", None).await;
    assert!(unlinked["data"]["error"].is_null(), "{unlinked:?}");
    assert_eq!(unlinked["data"]["linked"], false);
    assert!(unlinked["data"]["profile_id"].is_null(), "{unlinked:?}");

    let listed = list_profiles(&mut ws, &target_id).await;
    assert_eq!(profile_named(&listed, "Hard")["worlds"], json!([]));
    assert_eq!(profile_named(&listed, "Default")["worlds"], json!([]));
    server.handle.shutdown().await;
}

#[tokio::test]
async fn an_unknown_profile_or_empty_key_is_refused() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;

    let unknown = set_world(
        &mut ws,
        "C:/saves/abc",
        "Abc",
        Some(&format!("{target_id}/nope")),
    )
    .await;
    assert_eq!(
        unknown["data"]["error"]["code"], "profile_not_found",
        "{unknown:?}"
    );
    assert_eq!(unknown["data"]["world_key"], "C:/saves/abc");
    assert_eq!(unknown["data"]["world_name"], "Abc");

    let empty_key = set_world(&mut ws, "  ", "Abc", None).await;
    assert_eq!(
        empty_key["data"]["error"]["code"], "invalid_world",
        "{empty_key:?}"
    );

    let empty_name = set_world(&mut ws, "C:/saves/abc", "  ", None).await;
    assert_eq!(
        empty_name["data"]["error"]["code"], "invalid_world",
        "{empty_name:?}"
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn deleting_a_profile_unlinks_its_worlds() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let hard = created_id(&create(&mut ws, &target_id, "Hard").await);

    let linked = set_world(&mut ws, "C:/saves/abc", "Abc", Some(&hard)).await;
    assert!(linked["data"]["error"].is_null(), "{linked:?}");

    let deleted = request(
        &mut ws,
        "profile_delete",
        json!({"target_id": target_id, "profile_id": hard}),
    )
    .await;
    assert!(deleted["data"]["error"].is_null(), "{deleted:?}");

    let listed = list_profiles(&mut ws, &target_id).await;
    for profile in listed["data"]["profiles"].as_array().unwrap() {
        assert_eq!(profile["worlds"], json!([]), "{profile:?}");
    }
    server.handle.shutdown().await;
}

#[tokio::test]
async fn local_saves_carry_the_link() {
    let server = common::start_desktop_test_server(std::sync::Arc::new(
        ps_server::desktop_dialogs::NullDialogProvider,
    ))
    .await;
    let save_root = server._temp_dir.path().join("SaveGames");
    let save_dir = save_root.join("123").join("ABCDEF");
    std::fs::create_dir_all(&save_dir).unwrap();
    std::fs::write(save_dir.join("Level.sav"), b"data").unwrap();
    ps_db::settings::update_save_dir(&*server.handle.app.driver, &save_root.to_string_lossy())
        .await
        .unwrap();

    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let hard = created_id(&create(&mut ws, &target_id, "Hard").await);

    let listed = request(&mut ws, "list_local_saves", json!({})).await;
    let saves = listed["data"]["saves"].as_array().unwrap();
    let entry = saves
        .iter()
        .find(|save| save["world_key"].as_str().unwrap().ends_with("ABCDEF"))
        .unwrap_or_else(|| panic!("no save ending in ABCDEF in {saves:?}"));
    let world_key = entry["world_key"].as_str().unwrap().to_string();
    assert!(entry["mod_profile"].is_null(), "{entry:?}");

    let linked = set_world(&mut ws, &world_key, "Abc", Some(&hard)).await;
    assert!(linked["data"]["error"].is_null(), "{linked:?}");

    let listed = request(&mut ws, "list_local_saves", json!({})).await;
    let saves = listed["data"]["saves"].as_array().unwrap();
    let entry = saves
        .iter()
        .find(|save| save["world_key"].as_str().unwrap() == world_key)
        .unwrap_or_else(|| panic!("no save with world_key {world_key} in {saves:?}"));
    assert_eq!(entry["mod_profile"]["profile_id"], hard);
    assert_eq!(entry["mod_profile"]["profile_name"], "Hard");
    assert_eq!(entry["mod_profile"]["target_id"], target_id);
    server.handle.shutdown().await;
}

#[tokio::test]
async fn game_launch_without_desktop_mode_is_refused() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;

    let reply = request(
        &mut ws,
        "game_launch",
        json!({"target_id": target_id, "world_key": "C:/saves/abc"}),
    )
    .await;
    assert_eq!(reply["data"]["error"]["code"], "desktop_only", "{reply:?}");
    assert_eq!(reply["data"]["target_id"], target_id);
    assert_eq!(reply["data"]["world_key"], "C:/saves/abc");
    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_server_target_cannot_be_launched() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = native_server_target(&server, install.path()).await;

    let reply = request(&mut ws, "game_launch", json!({"target_id": target_id})).await;
    assert_eq!(
        reply["data"]["error"]["code"], "not_supported_on_target",
        "{reply:?}"
    );
    assert_eq!(reply["data"]["target_id"], target_id);
    assert!(reply["data"]["world_key"].is_null(), "{reply:?}");
    server.handle.shutdown().await;
}

#[tokio::test]
async fn list_local_saves_without_include_gamepass_is_steam_only() {
    let server = common::start_desktop_test_server(std::sync::Arc::new(
        ps_server::desktop_dialogs::NullDialogProvider,
    ))
    .await;
    let mut ws = common::connect(&server).await;

    common::send_json(&mut ws, json!({"type": "list_local_saves"})).await;
    let reply = common::next_json(&mut ws).await;
    assert_eq!(reply["type"], "list_local_saves");
    assert!(reply["data"]["error"].is_null(), "{reply:?}");
    let saves = reply["data"]["saves"].as_array().unwrap();
    assert!(saves.iter().all(|save| save["save_type"] != "gamepass"));

    let reply = request(&mut ws, "list_local_saves", json!({})).await;
    assert!(reply["data"]["error"].is_null(), "{reply:?}");
    let saves = reply["data"]["saves"].as_array().unwrap();
    assert!(saves.iter().all(|save| save["save_type"] != "gamepass"));

    server.handle.shutdown().await;
}

#[tokio::test]
async fn list_local_saves_include_gamepass_adds_the_gamepass_entry() {
    let gamepass_root = tempfile::tempdir().unwrap();
    let (expected_world_name, save_id) = build_one_save_gamepass_tree(gamepass_root.path());
    let _env_guard = common::GamepassEnvGuard::acquire(&[(
        "PS_GAMEPASS_PACKAGES_ROOT",
        gamepass_root.path().to_path_buf(),
    )])
    .await;

    let server = common::start_desktop_test_server(std::sync::Arc::new(
        ps_server::desktop_dialogs::NullDialogProvider,
    ))
    .await;
    let mut ws = common::connect(&server).await;

    let reply = request(
        &mut ws,
        "list_local_saves",
        json!({"include_gamepass": true}),
    )
    .await;
    assert!(reply["data"]["error"].is_null(), "{reply:?}");
    let saves = reply["data"]["saves"].as_array().unwrap();
    let gamepass_entries: Vec<&Value> = saves
        .iter()
        .filter(|save| save["save_type"] == "gamepass")
        .collect();
    assert_eq!(gamepass_entries.len(), 1, "{saves:?}");
    let entry = gamepass_entries[0];
    assert_eq!(entry["world_key"], format!("gamepass:{save_id}"));
    assert_eq!(entry["name"], expected_world_name);
    assert!(entry["mod_profile"].is_null(), "{entry:?}");

    server.handle.shutdown().await;
}

#[tokio::test]
async fn list_local_saves_include_gamepass_shows_the_linked_profile() {
    let gamepass_root = tempfile::tempdir().unwrap();
    let (expected_world_name, save_id) = build_one_save_gamepass_tree(gamepass_root.path());
    let _env_guard = common::GamepassEnvGuard::acquire(&[(
        "PS_GAMEPASS_PACKAGES_ROOT",
        gamepass_root.path().to_path_buf(),
    )])
    .await;

    let server = common::start_desktop_test_server(std::sync::Arc::new(
        ps_server::desktop_dialogs::NullDialogProvider,
    ))
    .await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let hard = created_id(&create(&mut ws, &target_id, "Hard").await);

    let world_key = format!("gamepass:{save_id}");
    let linked = set_world(&mut ws, &world_key, &expected_world_name, Some(&hard)).await;
    assert!(linked["data"]["error"].is_null(), "{linked:?}");

    let reply = request(
        &mut ws,
        "list_local_saves",
        json!({"include_gamepass": true}),
    )
    .await;
    let saves = reply["data"]["saves"].as_array().unwrap();
    let entry = saves
        .iter()
        .find(|save| save["world_key"] == world_key)
        .unwrap_or_else(|| panic!("no save with world_key {world_key} in {saves:?}"));
    assert_eq!(entry["mod_profile"]["profile_id"], hard);
    assert_eq!(entry["mod_profile"]["profile_name"], "Hard");
    assert_eq!(entry["mod_profile"]["target_id"], target_id);

    server.handle.shutdown().await;
}

#[tokio::test]
async fn list_local_saves_include_gamepass_with_no_install_is_steam_only() {
    let empty_root = tempfile::tempdir().unwrap();
    let _env_guard = common::GamepassEnvGuard::acquire(&[(
        "PS_GAMEPASS_PACKAGES_ROOT",
        empty_root.path().to_path_buf(),
    )])
    .await;

    let server = common::start_desktop_test_server(std::sync::Arc::new(
        ps_server::desktop_dialogs::NullDialogProvider,
    ))
    .await;
    let mut ws = common::connect(&server).await;

    let reply = request(
        &mut ws,
        "list_local_saves",
        json!({"include_gamepass": true}),
    )
    .await;
    assert!(reply["data"]["error"].is_null(), "{reply:?}");
    let saves = reply["data"]["saves"].as_array().unwrap();
    assert!(saves.iter().all(|save| save["save_type"] != "gamepass"));

    server.handle.shutdown().await;
}
