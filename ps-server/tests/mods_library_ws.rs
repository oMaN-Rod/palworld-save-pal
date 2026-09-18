mod common;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::json;

use ps_server::desktop_dialogs::QueuedDialogProvider;
use ps_server::services::mods::deploy::backup::BackupSet;
use ps_server::services::mods::LibraryPaths;

/// Writes a real zip file to disk: these messages take a path, not bytes.
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
        json!({"type": "mod_target_add", "data": {
            "root_path": root.to_string_lossy(),
        }}),
    )
    .await;
    let reply = common::next_json(ws).await;
    assert_eq!(reply["type"], "mod_target_add", "{reply:?}");
    reply["data"]["target"]["id"].as_str().unwrap().to_string()
}

fn library_of(server: &common::TestServer) -> LibraryPaths {
    LibraryPaths::new(server._temp_dir.path())
}

fn library_root_is_empty(library: &LibraryPaths) -> bool {
    match std::fs::read_dir(library.root()) {
        Ok(mut entries) => entries.next().is_none(),
        Err(_) => true,
    }
}

#[tokio::test]
async fn analyzing_an_archive_returns_its_manifest_and_installs_nothing() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;

    let archive = install.path().join("CoolMod-1.0.zip");
    write_zip(&archive, &[("CoolMod/Scripts/main.lua", b"print('hi')")]);

    common::send_json(
        &mut ws,
        json!({"type": "mod_analyze", "data": {
            "path": archive.to_string_lossy(),
            "target_id": target_id,
        }}),
    )
    .await;
    let analyzed = common::next_json(&mut ws).await;
    assert_eq!(analyzed["type"], "mod_analyze");
    assert_eq!(analyzed["data"]["target_id"], target_id);
    assert!(
        !analyzed["data"]["manifest"]["routes"]
            .as_array()
            .unwrap()
            .is_empty(),
        "{analyzed:?}"
    );

    common::send_json(&mut ws, json!({"type": "mod_list", "data": null})).await;
    let listed = common::next_json(&mut ws).await;
    assert_eq!(listed["data"]["mods"], json!([]));
    server.handle.shutdown().await;
}

#[tokio::test]
async fn installing_an_archive_lists_it_and_enables_it_on_the_active_profile() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;

    let archive = install.path().join("CoolMod-1.0.zip");
    write_zip(&archive, &[("CoolMod/Scripts/main.lua", b"print('hi')")]);

    common::send_json(
        &mut ws,
        json!({"type": "mod_install", "data": {
            "path": archive.to_string_lossy(),
            "target_id": target_id,
        }}),
    )
    .await;
    let installed = common::next_json(&mut ws).await;
    assert_eq!(installed["type"], "mod_install", "{installed:?}");
    assert_eq!(installed["data"]["target_id"], target_id);
    assert_eq!(
        PathBuf::from(installed["data"]["path"].as_str().unwrap()),
        archive
    );
    assert!(installed["data"]["enable_error"].is_null(), "{installed:?}");
    let mod_id = installed["data"]["mod_id"].as_str().unwrap().to_string();
    let version_id = installed["data"]["version_id"]
        .as_str()
        .unwrap()
        .to_string();

    common::send_json(&mut ws, json!({"type": "mod_list", "data": null})).await;
    let listed = common::next_json(&mut ws).await;
    let mods = listed["data"]["mods"].as_array().unwrap();
    assert_eq!(mods.len(), 1, "{mods:?}");
    assert_eq!(mods[0]["current_version_id"], version_id);

    let driver = &*server.handle.app.driver;
    let profile = ps_db::mod_profiles::active_for_target(driver, &target_id)
        .await
        .unwrap()
        .unwrap();
    let entries = ps_db::mod_profiles::mods_of(driver, &profile.id)
        .await
        .unwrap();
    assert!(
        entries.iter().any(|e| e.mod_id == mod_id && e.enabled),
        "{entries:?}"
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn installing_with_enable_false_leaves_the_profile_untouched() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;

    let archive = install.path().join("CoolMod-1.0.zip");
    write_zip(&archive, &[("CoolMod/Scripts/main.lua", b"print('hi')")]);

    common::send_json(
        &mut ws,
        json!({"type": "mod_install", "data": {
            "path": archive.to_string_lossy(),
            "target_id": target_id,
            "enable": false,
        }}),
    )
    .await;
    let installed = common::next_json(&mut ws).await;
    assert_eq!(installed["type"], "mod_install", "{installed:?}");
    let mod_id = installed["data"]["mod_id"].as_str().unwrap().to_string();

    let driver = &*server.handle.app.driver;
    let profile = ps_db::mod_profiles::active_for_target(driver, &target_id)
        .await
        .unwrap()
        .unwrap();
    let entries = ps_db::mod_profiles::mods_of(driver, &profile.id)
        .await
        .unwrap();
    assert!(
        entries.iter().all(|e| e.mod_id != mod_id),
        "enable: false must not touch the profile: {entries:?}"
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_relative_path_is_refused_by_name() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;

    common::send_json(
        &mut ws,
        json!({"type": "mod_install", "data": {
            "path": "CoolMod-1.0.zip",
            "target_id": target_id,
        }}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;
    assert_eq!(reply["type"], "mod_install");
    assert_eq!(reply["data"]["error"]["code"], "invalid_path");
    assert_eq!(reply["data"]["target_id"], target_id);
    server.handle.shutdown().await;
}

#[tokio::test]
async fn selecting_an_archive_through_the_dialog_analyzes_it() {
    let install = common::mods_ws::fake_windows_install();
    let archive = install.path().join("CoolMod-1.0.zip");
    write_zip(&archive, &[("CoolMod/Scripts/main.lua", b"print('hi')")]);

    let server =
        common::start_desktop_test_server(Arc::new(QueuedDialogProvider::new(vec![Some(
            archive.clone(),
        )])))
        .await;
    let mut ws = common::connect(&server).await;
    let target_id = add_target(&mut ws, install.path()).await;

    common::send_json(
        &mut ws,
        json!({"type": "mod_analyze", "data": {
            "path": "__select__",
            "target_id": target_id,
        }}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;
    assert_eq!(reply["type"], "mod_analyze");
    assert_eq!(
        PathBuf::from(reply["data"]["path"].as_str().unwrap()),
        archive
    );
    assert!(
        !reply["data"]["manifest"]["routes"]
            .as_array()
            .unwrap()
            .is_empty(),
        "{reply:?}"
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_cancelled_dialog_replies_canceled_and_installs_nothing() {
    let install = common::mods_ws::fake_windows_install();
    let server =
        common::start_desktop_test_server(Arc::new(QueuedDialogProvider::new(vec![None]))).await;
    let mut ws = common::connect(&server).await;
    let target_id = add_target(&mut ws, install.path()).await;

    common::send_json(
        &mut ws,
        json!({"type": "mod_install", "data": {
            "path": "__select__",
            "target_id": target_id,
        }}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;
    assert_eq!(reply["type"], "mod_install");
    assert_eq!(reply["data"]["canceled"], true);
    assert_eq!(reply["data"]["target_id"], target_id);

    common::send_json(&mut ws, json!({"type": "mod_list", "data": null})).await;
    let listed = common::next_json(&mut ws).await;
    assert_eq!(listed["data"]["mods"], json!([]));
    server.handle.shutdown().await;
}

#[tokio::test]
async fn selecting_is_refused_outside_desktop_mode() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;

    common::send_json(
        &mut ws,
        json!({"type": "mod_analyze", "data": {
            "path": "__select__",
            "target_id": target_id,
        }}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;
    assert_eq!(reply["type"], "mod_analyze");
    assert_eq!(reply["data"]["error"]["code"], "desktop_only");
    assert_eq!(reply["data"]["target_id"], target_id);
    server.handle.shutdown().await;
}

#[tokio::test]
async fn adding_a_target_through_the_folder_dialog() {
    let install = common::mods_ws::fake_windows_install();
    let server =
        common::start_desktop_test_server(Arc::new(QueuedDialogProvider::new_with_folders(vec![
            Some(install.path().to_path_buf()),
        ])))
        .await;
    let mut ws = common::connect(&server).await;

    common::send_json(
        &mut ws,
        json!({"type": "mod_target_add", "data": {
            "root_path": "__select__",
        }}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;
    assert_eq!(reply["type"], "mod_target_add", "{reply:?}");
    assert_eq!(
        PathBuf::from(reply["data"]["target"]["root_path"].as_str().unwrap()),
        install.path()
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn removing_a_mod_that_a_profile_holds_is_refused_with_its_usage() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;

    let archive = install.path().join("CoolMod-1.0.zip");
    write_zip(&archive, &[("CoolMod/Scripts/main.lua", b"print('hi')")]);

    common::send_json(
        &mut ws,
        json!({"type": "mod_install", "data": {
            "path": archive.to_string_lossy(),
            "target_id": target_id,
            "enable": true,
        }}),
    )
    .await;
    let installed = common::next_json(&mut ws).await;
    let mod_id = installed["data"]["mod_id"].as_str().unwrap().to_string();

    common::send_json(
        &mut ws,
        json!({"type": "mod_remove", "data": {"mod_id": mod_id}}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;
    assert_eq!(reply["type"], "mod_remove");
    assert_eq!(reply["data"]["error"]["code"], "version_in_use");
    assert!(
        reply["data"]["error"]["targets"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == &json!(target_id)),
        "{reply:?}"
    );
    assert!(
        !reply["data"]["error"]["profiles"]
            .as_array()
            .unwrap()
            .is_empty(),
        "{reply:?}"
    );

    common::send_json(&mut ws, json!({"type": "mod_list", "data": null})).await;
    let listed = common::next_json(&mut ws).await;
    let mods = listed["data"]["mods"].as_array().unwrap();
    assert_eq!(
        mods.len(),
        1,
        "a refused removal must not touch the mod: {mods:?}"
    );
    let library_dir = mods[0]["versions"][0]["library_dir"].as_str().unwrap();
    assert!(
        Path::new(library_dir).exists(),
        "the version's library directory must survive a refused removal"
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn deleting_the_current_version_is_refused() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;

    let archive_v1 = install.path().join("CoolMod-1.0.zip");
    write_zip(
        &archive_v1,
        &[
            ("CoolMod/Scripts/main.lua", b"print('v1')"),
            ("CoolMod/modinfo.json", br#"{"version":"1.0"}"#),
        ],
    );
    let archive_v2 = install.path().join("CoolMod-2.0.zip");
    write_zip(
        &archive_v2,
        &[
            ("CoolMod/Scripts/main.lua", b"print('v2')"),
            ("CoolMod/modinfo.json", br#"{"version":"2.0"}"#),
        ],
    );

    common::send_json(
        &mut ws,
        json!({"type": "mod_install", "data": {
            "path": archive_v1.to_string_lossy(),
            "target_id": target_id,
            "enable": false,
        }}),
    )
    .await;
    let installed_v1 = common::next_json(&mut ws).await;
    assert_eq!(installed_v1["type"], "mod_install", "{installed_v1:?}");
    let mod_id = installed_v1["data"]["mod_id"].as_str().unwrap().to_string();
    let version_v1 = installed_v1["data"]["version_id"]
        .as_str()
        .unwrap()
        .to_string();

    common::send_json(
        &mut ws,
        json!({"type": "mod_install", "data": {
            "path": archive_v2.to_string_lossy(),
            "target_id": target_id,
            "enable": false,
        }}),
    )
    .await;
    let installed_v2 = common::next_json(&mut ws).await;
    assert_eq!(installed_v2["type"], "mod_install", "{installed_v2:?}");
    let version_v2 = installed_v2["data"]["version_id"]
        .as_str()
        .unwrap()
        .to_string();

    common::send_json(&mut ws, json!({"type": "mod_list", "data": null})).await;
    let listed = common::next_json(&mut ws).await;
    let versions = listed["data"]["mods"][0]["versions"].as_array().unwrap();
    let library_dir_v1 = versions.iter().find(|v| v["id"] == version_v1).unwrap()["library_dir"]
        .as_str()
        .unwrap()
        .to_string();

    common::send_json(
        &mut ws,
        json!({"type": "mod_version_delete", "data": {"version_id": version_v1}}),
    )
    .await;
    let refused = common::next_json(&mut ws).await;
    assert_eq!(refused["type"], "mod_version_delete");
    assert_eq!(refused["data"]["error"]["code"], "version_in_use");
    assert_eq!(refused["data"]["error"]["is_current"], true, "{refused:?}");

    common::send_json(
        &mut ws,
        json!({"type": "mod_version_set_current", "data": {
            "mod_id": mod_id, "version_id": version_v2,
        }}),
    )
    .await;
    let set_current = common::next_json(&mut ws).await;
    assert_eq!(set_current["type"], "mod_version_set_current");
    assert!(set_current["data"]["error"].is_null(), "{set_current:?}");

    common::send_json(
        &mut ws,
        json!({"type": "mod_version_delete", "data": {"version_id": version_v1}}),
    )
    .await;
    let removed = common::next_json(&mut ws).await;
    assert_eq!(removed["type"], "mod_version_delete");
    assert_eq!(removed["data"]["removed"], true, "{removed:?}");
    assert!(
        !Path::new(&library_dir_v1).exists(),
        "the deleted version's library directory must be gone"
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn an_archive_with_open_decisions_is_not_installed_until_defaults_are_accepted() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;

    let archive = install.path().join("Bundle.zip");
    write_zip(
        &archive,
        &[
            ("Bundle/ModA/Scripts/main.lua", b"print('a')"),
            ("Bundle/ModB/Scripts/main.lua", b"print('b')"),
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
    let first = common::next_json(&mut ws).await;
    assert_eq!(first["type"], "mod_install", "{first:?}");
    assert!(
        !first["data"]["needs_decisions"]
            .as_array()
            .unwrap()
            .is_empty(),
        "{first:?}"
    );
    assert!(first["data"]["mod_id"].is_null());
    assert_eq!(first["data"]["target_id"], target_id);

    common::send_json(&mut ws, json!({"type": "mod_list", "data": null})).await;
    let listed = common::next_json(&mut ws).await;
    assert_eq!(listed["data"]["mods"], json!([]));
    assert!(
        library_root_is_empty(&library_of(&server)),
        "the decisions gate must leave no mod directory on disk, not only an empty mod_list"
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
    let second = common::next_json(&mut ws).await;
    assert_eq!(second["type"], "mod_install", "{second:?}");
    assert!(second["data"]["mod_id"].is_string(), "{second:?}");

    common::send_json(&mut ws, json!({"type": "mod_list", "data": null})).await;
    let listed_again = common::next_json(&mut ws).await;
    assert_eq!(listed_again["data"]["mods"].as_array().unwrap().len(), 1);
    server.handle.shutdown().await;
}

#[tokio::test]
async fn reinstalling_keeps_the_profile_entrys_load_order_and_pinned_version() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;

    let archive_v1 = install.path().join("CoolMod-1.0.zip");
    write_zip(
        &archive_v1,
        &[
            ("CoolMod/Scripts/main.lua", b"print('v1')"),
            ("CoolMod/modinfo.json", br#"{"version":"1.0"}"#),
        ],
    );
    common::send_json(
        &mut ws,
        json!({"type": "mod_install", "data": {
            "path": archive_v1.to_string_lossy(),
            "target_id": target_id,
            "enable": true,
        }}),
    )
    .await;
    let installed_v1 = common::next_json(&mut ws).await;
    let mod_id = installed_v1["data"]["mod_id"].as_str().unwrap().to_string();
    let version_v1 = installed_v1["data"]["version_id"]
        .as_str()
        .unwrap()
        .to_string();

    // Simulate a user customization the reinstall below must not disturb.
    let driver = &*server.handle.app.driver;
    let profile = ps_db::mod_profiles::active_for_target(driver, &target_id)
        .await
        .unwrap()
        .unwrap();
    ps_db::mod_profiles::set_mod(
        driver,
        &ps_db::mod_profiles::ProfileModRow {
            profile_id: profile.id.clone(),
            mod_id: mod_id.clone(),
            mod_version_id: Some(version_v1.clone()),
            enabled: true,
            load_order: 7,
        },
    )
    .await
    .unwrap();

    let archive_v2 = install.path().join("CoolMod-2.0.zip");
    write_zip(
        &archive_v2,
        &[
            ("CoolMod/Scripts/main.lua", b"print('v2')"),
            ("CoolMod/modinfo.json", br#"{"version":"2.0"}"#),
        ],
    );
    common::send_json(
        &mut ws,
        json!({"type": "mod_install", "data": {
            "path": archive_v2.to_string_lossy(),
            "target_id": target_id,
            "enable": true,
        }}),
    )
    .await;
    let installed_v2 = common::next_json(&mut ws).await;
    assert_eq!(installed_v2["type"], "mod_install", "{installed_v2:?}");
    assert!(
        installed_v2["data"]["error"].is_null(),
        "v2 must actually install, not be refused under the same reply type: {installed_v2:?}"
    );
    let version_v2 = installed_v2["data"]["version_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_ne!(version_v2, version_v1);

    let entries = ps_db::mod_profiles::mods_of(driver, &profile.id)
        .await
        .unwrap();
    let entry = entries.iter().find(|e| e.mod_id == mod_id).unwrap();
    assert_eq!(
        entry.load_order, 7,
        "reinstalling must not reorder: {entry:?}"
    );
    assert_eq!(
        entry.mod_version_id.as_deref(),
        Some(version_v1.as_str()),
        "reinstalling must not unpin the user's chosen version: {entry:?}"
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_mod_held_only_by_a_non_active_profile_is_refused_by_mod_remove() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;

    let archive = install.path().join("CoolMod-1.0.zip");
    write_zip(&archive, &[("CoolMod/Scripts/main.lua", b"print('hi')")]);
    common::send_json(
        &mut ws,
        json!({"type": "mod_install", "data": {
            "path": archive.to_string_lossy(),
            "target_id": target_id,
            "enable": false,
        }}),
    )
    .await;
    let installed = common::next_json(&mut ws).await;
    let mod_id = installed["data"]["mod_id"].as_str().unwrap().to_string();

    let driver = &*server.handle.app.driver;
    // A second profile on the same target is not active by construction (the
    // first, default profile already is).
    let second_profile = ps_db::mod_profiles::create(
        driver,
        &ps_db::mod_profiles::NewProfile {
            id: format!("{target_id}/second"),
            target_id: target_id.clone(),
            name: "Second".to_string(),
            is_default: false,
        },
    )
    .await
    .unwrap();
    assert!(!second_profile.is_active, "{second_profile:?}");
    ps_db::mod_profiles::set_mod(
        driver,
        &ps_db::mod_profiles::ProfileModRow {
            profile_id: second_profile.id.clone(),
            mod_id: mod_id.clone(),
            mod_version_id: None,
            enabled: true,
            load_order: 0,
        },
    )
    .await
    .unwrap();

    common::send_json(
        &mut ws,
        json!({"type": "mod_remove", "data": {"mod_id": mod_id}}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;
    assert_eq!(reply["type"], "mod_remove");
    assert_eq!(
        reply["data"]["error"]["code"], "version_in_use",
        "{reply:?}"
    );
    assert!(
        reply["data"]["error"]["profiles"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == &json!(second_profile.id)),
        "{reply:?}"
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_backup_set_is_listed_with_its_entries_and_size() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let library = library_of(&server);

    let original_dir = tempfile::tempdir().unwrap();
    let original = original_dir.path().join("main.lua");
    std::fs::write(&original, b"print('hi')").unwrap();

    let mut set = BackupSet::open(&library, &target_id, "20260101-000000", "abc123").unwrap();
    set.copy_in(&original).unwrap();
    set.write_index().unwrap();

    common::send_json(
        &mut ws,
        json!({"type": "mod_backup_list", "data": {"target_id": target_id}}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;
    assert_eq!(reply["type"], "mod_backup_list");
    let sets = reply["data"]["sets"].as_array().unwrap();
    assert_eq!(sets.len(), 1, "{sets:?}");
    assert_eq!(sets[0]["name"], "20260101-000000-abc123");
    assert!(sets[0]["size_bytes"].as_u64().unwrap() > 0);
    assert_eq!(sets[0]["entries"].as_array().unwrap().len(), 1);
    server.handle.shutdown().await;
}

#[tokio::test]
async fn restoring_puts_a_file_back_only_where_nothing_is() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let library = library_of(&server);

    // Under the target's own root, so restore's containment check accepts it.
    let original = install.path().join("Pal/Binaries/Win64/main.lua");
    std::fs::write(&original, b"print('original')").unwrap();

    let mut set = BackupSet::open(&library, &target_id, "20260101-000000", "abc123").unwrap();
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
    let restored = common::next_json(&mut ws).await;
    assert_eq!(restored["type"], "mod_backup_restore");
    let restored_paths = restored["data"]["restored"].as_array().unwrap();
    assert_eq!(restored_paths.len(), 1, "{restored:?}");
    assert_eq!(PathBuf::from(restored_paths[0].as_str().unwrap()), original);
    assert_eq!(restored["data"]["skipped"], json!([]));
    assert_eq!(
        std::fs::read_to_string(&original).unwrap(),
        "print('original')"
    );

    common::send_json(
        &mut ws,
        json!({"type": "mod_backup_restore", "data": {
            "target_id": target_id, "set": "20260101-000000-abc123",
        }}),
    )
    .await;
    let skipped = common::next_json(&mut ws).await;
    assert_eq!(skipped["data"]["restored"], json!([]));
    let skipped_list = skipped["data"]["skipped"].as_array().unwrap();
    assert_eq!(skipped_list.len(), 1);
    assert_eq!(skipped_list[0]["reason"], "occupied");
    assert_eq!(
        std::fs::read_to_string(&original).unwrap(),
        "print('original')",
        "an occupied destination must not be overwritten"
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn restoring_skips_a_blob_whose_hash_no_longer_matches() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let library = library_of(&server);

    let original = install.path().join("Pal/Binaries/Win64/main.lua");
    std::fs::write(&original, b"print('original')").unwrap();

    let mut set = BackupSet::open(&library, &target_id, "20260101-000000", "abc123").unwrap();
    let entry = set.copy_in(&original).unwrap();
    set.write_index().unwrap();
    std::fs::remove_file(&original).unwrap();

    // Corrupt the stored blob after the index recorded its hash.
    std::fs::write(set.dir().join(&entry.backup_key), b"tampered").unwrap();

    common::send_json(
        &mut ws,
        json!({"type": "mod_backup_restore", "data": {
            "target_id": target_id, "set": "20260101-000000-abc123",
        }}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;
    assert_eq!(reply["data"]["restored"], json!([]));
    let skipped = reply["data"]["skipped"].as_array().unwrap();
    assert_eq!(skipped.len(), 1, "{reply:?}");
    assert_eq!(skipped[0]["reason"], "changed");
    assert!(!original.exists());
    server.handle.shutdown().await;
}

#[tokio::test]
async fn restoring_with_a_paths_filter_restores_only_the_named_paths() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let library = library_of(&server);

    let first = install.path().join("Pal/Binaries/Win64/first.lua");
    let second = install.path().join("Pal/Binaries/Win64/second.lua");
    std::fs::write(&first, b"one").unwrap();
    std::fs::write(&second, b"two").unwrap();

    let mut set = BackupSet::open(&library, &target_id, "20260101-000000", "abc123").unwrap();
    set.copy_in(&first).unwrap();
    set.copy_in(&second).unwrap();
    set.write_index().unwrap();
    std::fs::remove_file(&first).unwrap();
    std::fs::remove_file(&second).unwrap();

    common::send_json(
        &mut ws,
        json!({"type": "mod_backup_restore", "data": {
            "target_id": target_id,
            "set": "20260101-000000-abc123",
            "paths": [first.to_string_lossy()],
        }}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;
    let restored = reply["data"]["restored"].as_array().unwrap();
    assert_eq!(restored.len(), 1, "{reply:?}");
    assert_eq!(PathBuf::from(restored[0].as_str().unwrap()), first);
    assert!(first.exists());
    assert!(
        !second.exists(),
        "the paths filter must leave paths not on the list untouched"
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn an_index_entry_pointing_outside_the_target_is_skipped() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let library = library_of(&server);

    // Entirely unrelated to the target's root or any of its layout dirs.
    let outside_dir = tempfile::tempdir().unwrap();
    let outside = outside_dir.path().join("evil.lua");
    std::fs::write(&outside, b"not yours").unwrap();

    let mut set = BackupSet::open(&library, &target_id, "20260101-000000", "abc123").unwrap();
    set.copy_in(&outside).unwrap();
    set.write_index().unwrap();
    std::fs::remove_file(&outside).unwrap();

    common::send_json(
        &mut ws,
        json!({"type": "mod_backup_restore", "data": {
            "target_id": target_id, "set": "20260101-000000-abc123",
        }}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;
    assert_eq!(reply["data"]["restored"], json!([]));
    let skipped = reply["data"]["skipped"].as_array().unwrap();
    assert_eq!(skipped.len(), 1, "{reply:?}");
    assert_eq!(skipped[0]["reason"], "outside_target");
    assert!(
        !outside.exists(),
        "an entry outside the target must never be written back"
    );
    server.handle.shutdown().await;
}

fn install_tree_at(root: &Path) {
    let binaries = root.join("Pal/Binaries/Win64");
    std::fs::create_dir_all(&binaries).unwrap();
    std::fs::write(binaries.join("Palworld-Win64-Shipping.exe"), b"stub").unwrap();
    std::fs::write(binaries.join("dwmapi.dll"), b"stub").unwrap();
    std::fs::create_dir_all(root.join("Pal/Content/Paks")).unwrap();
}

#[tokio::test]
async fn a_dot_dot_in_original_path_is_skipped_not_lexically_contained() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let library = library_of(&server);

    // A dedicated outer directory, so the resolved-outside location is one
    // this test fully controls and can assert nothing was written there.
    let outer = tempfile::tempdir().unwrap();
    let root = outer.path().join("game");
    install_tree_at(&root);
    let target_id = add_target(&mut ws, &root).await;

    // Lexically starts with `root` (an extra `ParentDir` component after a
    // real prefix match), but resolves to a sibling of it.
    let sneaky_original = root.join("..").join("outside").join("evil.txt");
    let resolved_outside = outer.path().join("outside").join("evil.txt");
    assert!(
        sneaky_original.starts_with(&root),
        "the attack depends on Path::starts_with being lexical, not resolving"
    );

    let set_dir = library
        .backups_dir(&target_id)
        .join("20260101-000000-abc123");
    std::fs::create_dir_all(set_dir.join("0000")).unwrap();
    let payload = b"malicious payload";
    std::fs::write(set_dir.join("0000/evil.txt"), payload).unwrap();
    let index = json!([{
        "original_path": sneaky_original.to_string_lossy(),
        "backup_key": "0000/evil.txt",
        "hash": ps_server::services::mods::digest::hash_bytes(payload),
    }]);
    std::fs::write(
        set_dir.join("index.json"),
        serde_json::to_string(&index).unwrap(),
    )
    .unwrap();

    common::send_json(
        &mut ws,
        json!({"type": "mod_backup_restore", "data": {
            "target_id": target_id, "set": "20260101-000000-abc123",
        }}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;
    assert_eq!(reply["data"]["restored"], json!([]), "{reply:?}");
    let skipped = reply["data"]["skipped"].as_array().unwrap();
    assert_eq!(skipped.len(), 1, "{reply:?}");
    assert_eq!(skipped[0]["reason"], "outside_target");
    assert!(
        !resolved_outside.exists(),
        "a `..` component must never let restore write outside the target root"
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_backup_key_that_escapes_its_set_is_refused() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let library = library_of(&server);

    let target_path = install.path().join("Pal/Binaries/Win64/main.lua");
    let set_dir = library
        .backups_dir(&target_id)
        .join("20260101-000000-abc123");
    std::fs::create_dir_all(&set_dir).unwrap();
    let malicious = json!([{
        "original_path": target_path.to_string_lossy(),
        "backup_key": "../evil",
        "hash": "0000000000000000000000000000000000000000000000000000000000000000",
    }]);
    std::fs::write(
        set_dir.join("index.json"),
        serde_json::to_string(&malicious).unwrap(),
    )
    .unwrap();

    common::send_json(
        &mut ws,
        json!({"type": "mod_backup_restore", "data": {
            "target_id": target_id, "set": "20260101-000000-abc123",
        }}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;
    assert_eq!(reply["data"]["restored"], json!([]));
    let skipped = reply["data"]["skipped"].as_array().unwrap();
    assert_eq!(skipped.len(), 1, "{reply:?}");
    assert_eq!(skipped[0]["reason"], "invalid_backup_key");
    assert!(!target_path.exists());
    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_missing_index_is_refused_not_reported_as_an_empty_restore() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let library = library_of(&server);
    std::fs::create_dir_all(
        library
            .backups_dir(&target_id)
            .join("20260101-000000-abc123"),
    )
    .unwrap();

    common::send_json(
        &mut ws,
        json!({"type": "mod_backup_restore", "data": {
            "target_id": target_id, "set": "20260101-000000-abc123",
        }}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;
    assert_eq!(reply["type"], "mod_backup_restore");
    assert_eq!(
        reply["data"]["error"]["code"], "index_unreadable",
        "{reply:?}"
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_set_name_that_escapes_the_backup_directory_is_refused() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;

    common::send_json(
        &mut ws,
        json!({"type": "mod_backup_restore", "data": {
            "target_id": target_id, "set": "../x",
        }}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;
    assert_eq!(reply["type"], "mod_backup_restore");
    assert_eq!(reply["data"]["error"]["code"], "invalid_set");
    server.handle.shutdown().await;
}

#[tokio::test]
async fn every_windows_trailing_dot_set_name_variant_is_refused_and_deletes_nothing() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let library = library_of(&server);

    let survivor = install.path().join("Pal/Binaries/Win64/main.lua");
    std::fs::write(&survivor, b"x").unwrap();
    let mut set = BackupSet::open(&library, &target_id, "20260101-000000", "abc123").unwrap();
    set.copy_in(&survivor).unwrap();
    set.write_index().unwrap();

    for bad in [
        "...", ".. ", "x.", "x ", "a\\b", "C:x", "/abs", "C:\\abs", "..",
    ] {
        common::send_json(
            &mut ws,
            json!({"type": "mod_backup_delete", "data": {
                "target_id": target_id, "set": bad,
            }}),
        )
        .await;
        let reply = common::next_json(&mut ws).await;
        assert_eq!(reply["type"], "mod_backup_delete");
        assert_eq!(
            reply["data"]["error"]["code"], "invalid_set",
            "set {bad:?} should have been refused: {reply:?}"
        );
    }

    assert!(
        set.dir().exists(),
        "none of the rejected names may have deleted the real set"
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn deleting_a_set_removes_exactly_that_set() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let library = library_of(&server);

    let file_a = install.path().join("Pal/Binaries/Win64/a.lua");
    let file_b = install.path().join("Pal/Binaries/Win64/b.lua");
    std::fs::write(&file_a, b"a").unwrap();
    std::fs::write(&file_b, b"b").unwrap();

    let mut set_a = BackupSet::open(&library, &target_id, "20260101-000000", "aaaaaaaa").unwrap();
    set_a.copy_in(&file_a).unwrap();
    set_a.write_index().unwrap();
    let mut set_b = BackupSet::open(&library, &target_id, "20260102-000000", "bbbbbbbb").unwrap();
    set_b.copy_in(&file_b).unwrap();
    set_b.write_index().unwrap();

    common::send_json(
        &mut ws,
        json!({"type": "mod_backup_delete", "data": {
            "target_id": target_id, "set": "20260101-000000-aaaaaaaa",
        }}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;
    assert_eq!(reply["type"], "mod_backup_delete");
    assert_eq!(reply["data"]["removed"], true, "{reply:?}");
    assert!(!set_a.dir().exists());
    assert!(
        set_b.dir().exists(),
        "deleting one set must not touch its sibling"
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn deleting_a_set_that_does_not_exist_is_refused() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;

    common::send_json(
        &mut ws,
        json!({"type": "mod_backup_delete", "data": {
            "target_id": target_id, "set": "20260101-000000-nosuchset",
        }}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;
    assert_eq!(reply["type"], "mod_backup_delete");
    assert_eq!(reply["data"]["error"]["code"], "set_not_found", "{reply:?}");
    server.handle.shutdown().await;
}

#[tokio::test]
async fn deleting_a_set_while_a_journal_is_open_is_refused() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let library = library_of(&server);
    let driver = &*server.handle.app.driver;

    let mut set = BackupSet::open(&library, &target_id, "20260101-000000", "abc123").unwrap();
    let original_dir = tempfile::tempdir().unwrap();
    let original = original_dir.path().join("main.lua");
    std::fs::write(&original, b"x").unwrap();
    set.copy_in(&original).unwrap();
    set.write_index().unwrap();

    ps_db::mod_deployments::open_journal(driver, &target_id, "abc123", "{}")
        .await
        .unwrap();

    common::send_json(
        &mut ws,
        json!({"type": "mod_backup_delete", "data": {
            "target_id": target_id, "set": "20260101-000000-abc123",
        }}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;
    assert_eq!(reply["type"], "mod_backup_delete");
    assert_eq!(reply["data"]["error"]["code"], "journal_open");
    assert!(
        set.dir().exists(),
        "the refused delete must leave the set on disk"
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn an_absolute_target_id_is_refused_and_touches_nothing() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;

    // An absolute `target_id` makes `PathBuf::join` discard the library root
    // entirely, so the real attack lands at `<target_id>/<set>`. Plant the
    // marker exactly there and prove it survives.
    let outer = tempfile::tempdir().unwrap();
    let set_name = "20260101-000000-abc123";
    let planted_dir = outer.path().join(set_name);
    std::fs::create_dir_all(&planted_dir).unwrap();
    let marker = planted_dir.join("marker.txt");
    std::fs::write(&marker, b"do not delete me").unwrap();

    common::send_json(
        &mut ws,
        json!({"type": "mod_backup_delete", "data": {
            "target_id": outer.path().to_string_lossy(), "set": set_name,
        }}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;
    assert_eq!(reply["type"], "mod_backup_delete");
    assert_eq!(
        reply["data"]["error"]["code"], "invalid_target",
        "{reply:?}"
    );
    assert!(
        marker.exists(),
        "an absolute target_id must never reach the location it would have attacked"
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_dot_dot_target_id_is_refused() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;

    common::send_json(
        &mut ws,
        json!({"type": "mod_backup_list", "data": {"target_id": ".."}}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;
    assert_eq!(reply["type"], "mod_backup_list");
    assert_eq!(
        reply["data"]["error"]["code"], "invalid_target",
        "{reply:?}"
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn an_unregistered_target_id_is_refused_and_touches_nothing() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let library = library_of(&server);

    // A well-formed but never-registered target id: plant a real set under
    // it directly (bypassing the WS layer, as a leftover from a removed
    // target might be) and prove the refusal leaves it alone.
    let set_dir = library
        .backups_dir("client-nosuchtarget")
        .join("20260101-000000-abc123");
    std::fs::create_dir_all(&set_dir).unwrap();
    let marker = set_dir.join("marker.txt");
    std::fs::write(&marker, b"do not delete me").unwrap();

    common::send_json(
        &mut ws,
        json!({"type": "mod_backup_delete", "data": {
            "target_id": "client-nosuchtarget", "set": "20260101-000000-abc123",
        }}),
    )
    .await;
    let reply = common::next_json(&mut ws).await;
    assert_eq!(reply["type"], "mod_backup_delete");
    assert_eq!(
        reply["data"]["error"]["code"], "target_not_found",
        "{reply:?}"
    );
    assert!(
        marker.exists(),
        "an unregistered target's existing backup set must survive the refusal"
    );
    server.handle.shutdown().await;
}

#[tokio::test]
async fn backup_and_target_removal_are_refused_while_an_apply_holds_the_target() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let library = library_of(&server);
    let driver = &*server.handle.app.driver;

    let original = install.path().join("Pal/Binaries/Win64/main.lua");
    std::fs::write(&original, b"print('original')").unwrap();
    let mut set = BackupSet::open(&library, &target_id, "20260101-000000", "abc123").unwrap();
    set.copy_in(&original).unwrap();
    set.write_index().unwrap();
    std::fs::remove_file(&original).unwrap();

    let target = ps_db::mod_targets::get(driver, &target_id)
        .await
        .unwrap()
        .unwrap();
    let guard = ps_server::services::mods::deploy::try_lock_target(&library, &target)
        .expect("no apply is running");

    let set_data = json!({"target_id": target_id, "set": "20260101-000000-abc123"});
    for (message_type, data) in [
        ("mod_backup_restore", set_data.clone()),
        ("mod_backup_delete", set_data.clone()),
        ("mod_target_remove", json!({"target_id": target_id})),
    ] {
        common::send_json(&mut ws, json!({"type": message_type, "data": data})).await;
        let reply = common::next_json(&mut ws).await;
        assert_eq!(reply["type"], message_type, "{reply:?}");
        assert_eq!(reply["data"]["error"]["code"], "apply_in_progress", "{reply:?}");
        assert_eq!(reply["data"]["error"]["target_id"], target_id.as_str());
    }
    assert!(!original.exists(), "a refused restore writes nothing");
    assert!(set.dir().exists(), "a refused delete keeps the set");
    assert!(
        ps_db::mod_targets::get(driver, &target_id)
            .await
            .unwrap()
            .is_some(),
        "a refused removal keeps the target"
    );

    drop(guard);
    common::send_json(&mut ws, json!({"type": "mod_backup_restore", "data": set_data})).await;
    let restored = common::next_json(&mut ws).await;
    assert_eq!(
        restored["data"]["restored"].as_array().unwrap().len(),
        1,
        "{restored:?}"
    );
    server.handle.shutdown().await;
}
