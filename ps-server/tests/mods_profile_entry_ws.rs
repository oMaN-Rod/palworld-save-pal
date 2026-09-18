mod common;

use std::path::Path;

use serde_json::{json, Value};

use ps_db::mod_targets::NewModTarget;
use ps_server::services::mods::LibraryPaths;

async fn driver_for(server: &common::TestServer) -> ps_db::SqlxSqliteDriver {
    ps_db::SqlxSqliteDriver::new(
        ps_db::open(&server._temp_dir.path().join("ps-rs.db"))
            .await
            .unwrap(),
    )
}

/// `profile_mods.mod_id` is a live foreign key, so every held id needs a library row.
async fn library_mod(server: &common::TestServer, db: &ps_db::SqlxSqliteDriver, mod_id: &str) {
    let paths = LibraryPaths::new(server._temp_dir.path());
    common::mods::install_fixture_mod(
        db,
        &paths,
        mod_id,
        "1.0",
        &[("ue4ss", "CoolMod/Scripts/main.lua", b"return {}")],
    )
    .await;
}

async fn target(db: &dyn ps_db::DbDriver, id: &str, name: &str, root: &Path) -> String {
    common::mods::install_tree(root);
    ps_db::mod_targets::upsert(
        db,
        &NewModTarget {
            id: id.to_string(),
            kind: "client".to_string(),
            name: name.to_string(),
            root_path: root.to_string_lossy().into_owned(),
            platform: "win64".to_string(),
            ue4ss_mode: "none".to_string(),
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

async fn hold(db: &dyn ps_db::DbDriver, profile_id: &str, mod_id: &str, enabled: bool) {
    ps_db::mod_profiles::set_mod(
        db,
        &ps_db::mod_profiles::ProfileModRow {
            profile_id: profile_id.to_string(),
            mod_id: mod_id.to_string(),
            mod_version_id: None,
            enabled,
            load_order: 0,
        },
    )
    .await
    .unwrap();
}

async fn holds(db: &dyn ps_db::DbDriver, profile_id: &str, mod_id: &str) -> bool {
    ps_db::mod_profiles::mods_of(db, profile_id)
        .await
        .unwrap()
        .iter()
        .any(|entry| entry.mod_id == mod_id)
}

async fn request(ws: &mut common::WsClient, message_type: &str, data: Value) -> Value {
    common::send_json(ws, json!({ "type": message_type, "data": data })).await;
    let reply = common::next_json(ws).await;
    assert_eq!(reply["type"], message_type, "{reply:?}");
    reply["data"].clone()
}

#[tokio::test]
async fn profile_remove_mod_removes_a_disabled_entry_from_the_active_profile() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let db = &driver_for(&server).await;
    let root = tempfile::tempdir().unwrap();
    let target_id = target(db, "client-a", "Palworld", root.path()).await;
    library_mod(&server, db, "coolmod").await;
    hold(db, "client-a/default", "coolmod", false).await;

    let data = request(
        &mut ws,
        "profile_remove_mod",
        json!({ "target_id": target_id, "mod_id": "coolmod" }),
    )
    .await;
    assert_eq!(
        data,
        json!({ "target_id": "client-a", "profile_id": "client-a/default", "mod_id": "coolmod", "removed": true })
    );
    assert!(!holds(db, "client-a/default", "coolmod").await);

    server.handle.shutdown().await;
}

#[tokio::test]
async fn profile_remove_mod_removes_an_enabled_entry_from_a_named_profile_only() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let db = &driver_for(&server).await;
    let root = tempfile::tempdir().unwrap();
    let target_id = target(db, "client-a", "Palworld", root.path()).await;
    library_mod(&server, db, "coolmod").await;
    ps_db::mod_profiles::create(
        db,
        &ps_db::mod_profiles::NewProfile {
            id: "client-a/second".to_string(),
            target_id: target_id.clone(),
            name: "Second".to_string(),
            is_default: false,
        },
    )
    .await
    .unwrap();
    hold(db, "client-a/default", "coolmod", true).await;
    hold(db, "client-a/second", "coolmod", true).await;

    let data = request(
        &mut ws,
        "profile_remove_mod",
        json!({ "target_id": target_id, "profile_id": "client-a/second", "mod_id": "coolmod" }),
    )
    .await;
    assert_eq!(data["removed"], true, "{data:?}");
    assert!(!holds(db, "client-a/second", "coolmod").await);
    assert!(holds(db, "client-a/default", "coolmod").await);

    server.handle.shutdown().await;
}

#[tokio::test]
async fn profile_remove_mod_refusals() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let db = &driver_for(&server).await;
    let root_a = tempfile::tempdir().unwrap();
    let root_b = tempfile::tempdir().unwrap();
    let target_a = target(db, "client-a", "Palworld", root_a.path()).await;
    let target_b = target(db, "client-b", "Other", root_b.path()).await;

    let unknown = request(
        &mut ws,
        "profile_remove_mod",
        json!({ "target_id": "client-nope", "mod_id": "coolmod" }),
    )
    .await;
    assert_eq!(unknown["error"]["code"], "target_not_found", "{unknown:?}");

    let other = request(
        &mut ws,
        "profile_remove_mod",
        json!({ "target_id": target_a, "profile_id": format!("{target_b}/default"), "mod_id": "coolmod" }),
    )
    .await;
    assert_eq!(other["error"]["code"], "profile_not_found", "{other:?}");

    let missing = request(
        &mut ws,
        "profile_remove_mod",
        json!({ "target_id": target_a, "mod_id": "coolmod" }),
    )
    .await;
    assert_eq!(
        missing["error"]["code"], "mod_not_in_profile",
        "{missing:?}"
    );
    assert_eq!(missing["mod_id"], "coolmod");
    assert_eq!(missing["profile_id"], "client-a/default");

    let root_c = tempfile::tempdir().unwrap();
    common::mods::install_tree(root_c.path());
    ps_db::mod_targets::upsert(
        db,
        &NewModTarget {
            id: "client-c".to_string(),
            kind: "client".to_string(),
            name: "No profile".to_string(),
            root_path: root_c.path().to_string_lossy().into_owned(),
            platform: "win64".to_string(),
            ue4ss_mode: "none".to_string(),
            layout_overrides: "{}".to_string(),
            detected: "{}".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let no_profile = request(
        &mut ws,
        "profile_remove_mod",
        json!({ "target_id": "client-c", "mod_id": "coolmod" }),
    )
    .await;
    assert_eq!(
        no_profile["error"]["code"], "no_active_profile",
        "{no_profile:?}"
    );

    server.handle.shutdown().await;
}

#[tokio::test]
async fn mod_release_profiles_removes_only_disabled_entries_everywhere() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let db = &driver_for(&server).await;
    let root_a = tempfile::tempdir().unwrap();
    let root_b = tempfile::tempdir().unwrap();
    target(db, "client-a", "Palworld", root_a.path()).await;
    target(db, "client-b", "ModsTest", root_b.path()).await;
    library_mod(&server, db, "coolmod").await;
    library_mod(&server, db, "othermod").await;
    ps_db::mod_profiles::create(
        db,
        &ps_db::mod_profiles::NewProfile {
            id: "client-b/second".to_string(),
            target_id: "client-b".to_string(),
            name: "Second".to_string(),
            is_default: false,
        },
    )
    .await
    .unwrap();
    hold(db, "client-a/default", "coolmod", false).await;
    hold(db, "client-b/default", "coolmod", true).await;
    hold(db, "client-b/second", "coolmod", false).await;
    hold(db, "client-b/second", "othermod", false).await;

    let data = request(
        &mut ws,
        "mod_release_profiles",
        json!({ "mod_id": "coolmod" }),
    )
    .await;
    assert_eq!(
        data,
        json!({
            "mod_id": "coolmod",
            "removed": [
                { "target_id": "client-a", "profile_id": "client-a/default" },
                { "target_id": "client-b", "profile_id": "client-b/second" }
            ],
            "enabled": [{ "target_id": "client-b", "profile_id": "client-b/default" }]
        })
    );
    assert!(!holds(db, "client-a/default", "coolmod").await);
    assert!(holds(db, "client-b/default", "coolmod").await);
    assert!(!holds(db, "client-b/second", "coolmod").await);
    assert!(holds(db, "client-b/second", "othermod").await);

    let none = request(
        &mut ws,
        "mod_release_profiles",
        json!({ "mod_id": "nobody" }),
    )
    .await;
    assert_eq!(
        none,
        json!({ "mod_id": "nobody", "removed": [], "enabled": [] })
    );

    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_mod_held_only_by_disabled_entries_can_be_released_applied_and_removed() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let db = ps_db::SqlxSqliteDriver::new(
        ps_db::open(&server._temp_dir.path().join("ps-rs.db"))
            .await
            .unwrap(),
    );
    let paths = LibraryPaths::new(server._temp_dir.path());
    let root_a = tempfile::tempdir().unwrap();
    let root_b = tempfile::tempdir().unwrap();
    target(&db, "client-a", "Palworld", root_a.path()).await;
    target(&db, "client-b", "ModsTest", root_b.path()).await;
    let stored = common::mods::install_fixture_mod(
        &db,
        &paths,
        "creativemenu_p-pak",
        "1.0",
        &[("pak", "CreativeMenu_P.pak", b"pak")],
    )
    .await;
    hold(&db, "client-a/default", "creativemenu_p-pak", false).await;
    hold(&db, "client-b/default", "creativemenu_p-pak", false).await;
    let deployed_path = root_b
        .path()
        .join("Pal/Content/Paks/~mods/CreativeMenu_P.pak")
        .to_string_lossy()
        .into_owned();
    ps_db::mod_deployments::record(
        &db,
        &[ps_db::mod_deployments::NewDeploymentFile {
            target_id: "client-b".to_string(),
            path: deployed_path.clone(),
            mod_version_id: Some(stored.version.id.clone()),
            hash: "x".to_string(),
            role: "file".to_string(),
            rel_path: Some("CreativeMenu_P.pak".to_string()),
        }],
    )
    .await
    .unwrap();

    let refused = request(
        &mut ws,
        "mod_remove",
        json!({ "mod_id": "creativemenu_p-pak" }),
    )
    .await;
    assert_eq!(refused["error"]["code"], "version_in_use", "{refused:?}");
    assert_eq!(refused["error"]["targets"], json!(["client-a", "client-b"]));
    assert_eq!(
        refused["error"]["profiles"],
        json!(["client-a/default", "client-b/default"])
    );
    assert_eq!(
        refused["error"]["holders"],
        json!([
            { "target_id": "client-a", "target_name": "Palworld", "profile_id": "client-a/default", "profile_name": "Default", "enabled": false },
            { "target_id": "client-b", "target_name": "ModsTest", "profile_id": "client-b/default", "profile_name": "Default", "enabled": false }
        ])
    );
    assert_eq!(
        refused["error"]["deployed"],
        json!([{ "target_id": "client-b", "target_name": "ModsTest" }])
    );

    let released = request(
        &mut ws,
        "mod_release_profiles",
        json!({ "mod_id": "creativemenu_p-pak" }),
    )
    .await;
    assert_eq!(
        released["removed"].as_array().unwrap().len(),
        2,
        "{released:?}"
    );

    let still = request(
        &mut ws,
        "mod_remove",
        json!({ "mod_id": "creativemenu_p-pak" }),
    )
    .await;
    assert_eq!(still["error"]["code"], "version_in_use", "{still:?}");
    assert_eq!(still["error"]["holders"], json!([]));
    assert_eq!(
        still["error"]["deployed"],
        json!([{ "target_id": "client-b", "target_name": "ModsTest" }])
    );

    ps_db::mod_deployments::forget(&db, "client-b", &[deployed_path.as_str()])
        .await
        .unwrap();
    let removed = request(
        &mut ws,
        "mod_remove",
        json!({ "mod_id": "creativemenu_p-pak" }),
    )
    .await;
    assert_eq!(
        removed,
        json!({ "mod_id": "creativemenu_p-pak", "removed": true })
    );

    server.handle.shutdown().await;
}
