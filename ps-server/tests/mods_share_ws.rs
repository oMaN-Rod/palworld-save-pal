mod common;

use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use ps_core::mods::{FileRoute, InstallManifest, ModType, RouteKind, SourceHint};
use ps_server::desktop_dialogs::QueuedDialogProvider;
use ps_server::services::mods::share::{
    self, extract_archive, read_profile, SharedEntry, SharedFramework, SharedProfile,
};
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

async fn request(ws: &mut common::WsClient, message_type: &str, data: Value) -> Value {
    common::send_json(ws, json!({ "type": message_type, "data": data })).await;
    let (frame, _) = common::mods_ws::next_of_type(ws, message_type).await;
    frame["data"].clone()
}

async fn add_target(ws: &mut common::WsClient, root: &Path) -> String {
    let reply = request(
        ws,
        "mod_target_add",
        json!({ "root_path": root.to_string_lossy() }),
    )
    .await;
    reply["target"]["id"].as_str().unwrap().to_string()
}

/// Installs `CoolMod/Scripts/main.lua` through `mod_install`, returning
/// `(mod_id, archive_path)`.
async fn install_cool_mod(
    ws: &mut common::WsClient,
    root: &Path,
    target_id: &str,
) -> (String, PathBuf) {
    let archive = root.join("CoolMod-1.0.zip");
    write_zip(&archive, &[("CoolMod/Scripts/main.lua", b"print('cool')")]);
    let reply = request(
        ws,
        "mod_install",
        json!({ "path": archive.to_string_lossy(), "target_id": target_id, "enable": true }),
    )
    .await;
    assert!(reply["error"].is_null(), "{reply:?}");
    (reply["mod_id"].as_str().unwrap().to_string(), archive)
}

/// Stores a UE4SS mod straight into the library with no kept archive.
async fn store_ue4ss_mod(server: &common::TestServer, mod_id: &str, folder: &str) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let rel_path = format!("{folder}/Scripts/main.lua");
    common::mods::write(&scratch.path().join(&rel_path), b"print('other')");
    let manifest = InstallManifest {
        folder_name: folder.to_string(),
        display_name: folder.to_string(),
        mod_type: ModType::Ue4ss,
        version: "1.0".to_string(),
        routes: vec![FileRoute {
            archive_path: rel_path.clone(),
            rel_path,
            kind: RouteKind::Ue4ss,
        }],
        decisions: Vec::new(),
        platform_filtered: None,
        source: SourceHint::default(),
    };
    let stored = library::store(
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
    stored.version.id
}

async fn export(ws: &mut common::WsClient, target_id: &str, include: bool, path: &str) -> Value {
    request(
        ws,
        "profile_export",
        json!({
            "target_id": target_id,
            "profile_id": format!("{target_id}/default"),
            "include_archives": include,
            "path": path,
        }),
    )
    .await
}

#[tokio::test]
async fn exporting_writes_concrete_versions_and_order() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let (cool_id, _) = install_cool_mod(&mut ws, install.path(), &target_id).await;
    let pinned_version = store_ue4ss_mod(&server, "othermod-ue4ss", "OtherMod").await;
    let set = request(
        &mut ws,
        "profile_set_mod",
        json!({
            "target_id": target_id,
            "mod_id": "othermod-ue4ss",
            "enabled": false,
            "mod_version_id": pinned_version,
        }),
    )
    .await;
    assert!(set["error"].is_null(), "{set:?}");

    let scratch = tempfile::tempdir().unwrap();
    let dest = scratch.path().join("p.psmods");
    let reply = export(&mut ws, &target_id, false, &dest.to_string_lossy()).await;

    assert!(reply.get("error").is_none(), "{reply:?}");
    assert_eq!(reply["entries"], 2, "{reply:?}");
    assert_eq!(reply["archives_included"], 0, "{reply:?}");
    assert_eq!(reply["missing_archives"], json!([]), "{reply:?}");
    assert_eq!(reply["unresolved"], json!([]), "{reply:?}");
    assert_eq!(Path::new(reply["path"].as_str().unwrap()), dest);

    let db = &*server.handle.app.driver;
    let shared = read_profile(&dest).unwrap();
    assert_eq!(shared.target_platform, "win64");
    assert_eq!(shared.name, "Default");
    let current = ps_db::mod_library::current_version(db, &cool_id)
        .await
        .unwrap()
        .unwrap();
    let profile_mods = ps_db::mod_profiles::mods_of(db, &format!("{target_id}/default"))
        .await
        .unwrap();
    assert_eq!(shared.entries.len(), profile_mods.len());
    for (entry, row) in shared.entries.iter().zip(&profile_mods) {
        assert_eq!(entry.mod_id, row.mod_id);
        assert_eq!(entry.enabled, row.enabled);
        assert_eq!(entry.load_order, row.load_order);
        assert_eq!(entry.archive, None);
    }
    let cool = shared.entries.iter().find(|e| e.mod_id == cool_id).unwrap();
    assert_eq!(cool.version, current.version);
    assert_eq!(cool.mod_version_id, current.id);
    assert!(cool.enabled);
    let other = shared
        .entries
        .iter()
        .find(|e| e.mod_id == "othermod-ue4ss")
        .unwrap();
    assert_eq!(other.mod_version_id, pinned_version);
    assert!(!other.enabled);
}

#[tokio::test]
async fn including_archives_packs_kept_archives_and_names_the_rest() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let (cool_id, archive) = install_cool_mod(&mut ws, install.path(), &target_id).await;
    let bare_version = store_ue4ss_mod(&server, "othermod-ue4ss", "OtherMod").await;
    let set = request(
        &mut ws,
        "profile_set_mod",
        json!({ "target_id": target_id, "mod_id": "othermod-ue4ss", "enabled": true }),
    )
    .await;
    assert!(set["error"].is_null(), "{set:?}");

    let scratch = tempfile::tempdir().unwrap();
    let dest = scratch.path().join("p.psmods");
    let reply = export(&mut ws, &target_id, true, &dest.to_string_lossy()).await;

    assert!(reply.get("error").is_none(), "{reply:?}");
    assert_eq!(reply["entries"], 2, "{reply:?}");
    assert_eq!(reply["archives_included"], 1, "{reply:?}");
    assert_eq!(
        reply["missing_archives"],
        json!([bare_version]),
        "{reply:?}"
    );

    let shared = read_profile(&dest).unwrap();
    let other = shared
        .entries
        .iter()
        .find(|e| e.mod_id == "othermod-ue4ss")
        .unwrap();
    assert_eq!(other.archive, None);
    let cool = shared.entries.iter().find(|e| e.mod_id == cool_id).unwrap();
    let name = cool.archive.clone().unwrap();
    assert!(name.starts_with("archives/"), "{name}");
    let out_dir = scratch.path().join("out");
    std::fs::create_dir_all(&out_dir).unwrap();
    let extracted = extract_archive(&dest, &shared, &name, &out_dir).unwrap();
    assert_eq!(
        std::fs::read(extracted).unwrap(),
        std::fs::read(archive).unwrap()
    );
}

#[tokio::test]
async fn select_opens_a_save_dialog_and_cancel_is_quiet() {
    let scratch = tempfile::tempdir().unwrap();
    let dest = scratch.path().join("picked.psmods");
    let server = common::start_desktop_test_server(std::sync::Arc::new(
        QueuedDialogProvider::new_with_saves(vec![], vec![Some(dest.clone()), None]),
    ))
    .await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;

    let written = export(&mut ws, &target_id, false, "__select__").await;
    assert!(written.get("error").is_none(), "{written:?}");
    assert_eq!(Path::new(written["path"].as_str().unwrap()), dest);
    assert_eq!(written["entries"], 0);
    assert!(read_profile(&dest).unwrap().entries.is_empty());
    std::fs::remove_file(&dest).unwrap();

    let canceled = export(&mut ws, &target_id, false, "__select__").await;
    assert_eq!(canceled["canceled"], true, "{canceled:?}");
    assert!(canceled.get("error").is_none(), "{canceled:?}");
    assert_eq!(canceled["target_id"], target_id.as_str());
    assert_eq!(std::fs::read_dir(scratch.path()).unwrap().count(), 0);
}

#[tokio::test]
async fn a_relative_or_wrong_extension_path_is_refused() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let scratch = tempfile::tempdir().unwrap();

    for path in [
        PathBuf::from("p.psmods"),
        scratch.path().join("p.zip"),
        scratch.path().join("missing").join("p.psmods"),
    ] {
        let path = path.to_string_lossy().into_owned();
        let reply = export(&mut ws, &target_id, false, &path).await;
        assert_eq!(reply["error"]["code"], "invalid_path", "{path}: {reply:?}");
        assert_eq!(reply["path"], path.as_str());
        assert_eq!(reply["target_id"], target_id.as_str());
    }
    let reply = export(&mut ws, &target_id, false, "__select__").await;
    assert_eq!(reply["error"]["code"], "desktop_only", "{reply:?}");
    assert_eq!(std::fs::read_dir(scratch.path()).unwrap().count(), 0);
}

async fn import(ws: &mut common::WsClient, data: Value) -> Value {
    request(ws, "profile_import", data).await
}

fn entry_of<'a>(profile: &'a Value, mod_id: &str) -> Option<&'a Value> {
    profile["mods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["mod_id"] == mod_id)
}

fn assert_no_import_scratch(server: &common::TestServer) {
    let imports = server._temp_dir.path().join("downloads").join(".imports");
    if imports.exists() {
        assert_eq!(std::fs::read_dir(&imports).unwrap().count(), 0);
    }
}

fn shared_profile(entries: Vec<SharedEntry>, frameworks: Vec<SharedFramework>) -> SharedProfile {
    SharedProfile {
        format: share::FORMAT.to_string(),
        format_version: share::FORMAT_VERSION,
        name: "Handmade".to_string(),
        exported_at: "2026-09-14T12:00:00+00:00".to_string(),
        target_kind: "client".to_string(),
        target_platform: "win64".to_string(),
        entries,
        frameworks,
    }
}

fn shared_entry(mod_id: &str, load_order: i64, archive: Option<String>) -> SharedEntry {
    SharedEntry {
        mod_id: mod_id.to_string(),
        name: mod_id.to_string(),
        mod_type: "ue4ss".to_string(),
        source_kind: "local".to_string(),
        version: "1.0".to_string(),
        mod_version_id: format!("{mod_id}@1.0"),
        enabled: true,
        load_order,
        archive,
    }
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

async fn store_workshop_pack(server: &common::TestServer, mod_id: &str, package: &str) {
    let scratch = tempfile::tempdir().unwrap();
    let rel_path = format!("{package}/Info.json");
    common::mods::write(&scratch.path().join(&rel_path), b"{}");
    let manifest = InstallManifest {
        folder_name: package.to_string(),
        display_name: package.to_string(),
        mod_type: ModType::Workshop,
        version: "1.0".to_string(),
        routes: vec![FileRoute {
            archive_path: rel_path.clone(),
            rel_path,
            kind: RouteKind::Workshop,
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

#[tokio::test]
async fn importing_with_archives_installs_and_pins() {
    let source = common::start_test_server().await;
    let mut source_ws = common::connect(&source).await;
    let source_install = common::mods_ws::fake_windows_install();
    let source_target = add_target(&mut source_ws, source_install.path()).await;
    let (cool_id, _) =
        install_cool_mod(&mut source_ws, source_install.path(), &source_target).await;
    let renamed = request(
        &mut source_ws,
        "profile_rename",
        json!({
            "target_id": source_target,
            "profile_id": format!("{source_target}/default"),
            "name": "Co-op",
        }),
    )
    .await;
    assert!(renamed["error"].is_null(), "{renamed:?}");
    let scratch = tempfile::tempdir().unwrap();
    let file = scratch.path().join("p.psmods");
    let exported = export(
        &mut source_ws,
        &source_target,
        true,
        &file.to_string_lossy(),
    )
    .await;
    assert_eq!(exported["archives_included"], 1, "{exported:?}");
    let version_id = read_profile(&file).unwrap().entries[0]
        .mod_version_id
        .clone();

    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;

    let reply = import(
        &mut ws,
        json!({ "target_id": target_id, "path": file.to_string_lossy() }),
    )
    .await;

    assert!(reply.get("error").is_none(), "{reply:?}");
    assert_eq!(reply["target_id"], target_id.as_str());
    assert_eq!(Path::new(reply["path"].as_str().unwrap()), file);
    assert_eq!(
        reply["installed"],
        json!([{ "mod_id": cool_id, "version_id": version_id }]),
        "{reply:?}"
    );
    assert_eq!(reply["pinned"], json!([cool_id]), "{reply:?}");
    assert_eq!(reply["following_current"], json!([]), "{reply:?}");
    assert_eq!(reply["missing"], json!([]), "{reply:?}");
    assert_eq!(reply["disabled"], json!([]), "{reply:?}");
    let profile = &reply["profile"];
    assert_eq!(profile["is_active"], false, "{reply:?}");
    assert_eq!(profile["is_default"], false, "{reply:?}");
    assert_eq!(profile["name"], "Co-op", "{reply:?}");
    let entry = entry_of(profile, &cool_id).expect("the entry is written");
    assert_eq!(entry["enabled"], true);
    assert_eq!(entry["mod_version_id"], version_id.as_str());

    let db = &*server.handle.app.driver;
    let active = ps_db::mod_profiles::active_for_target(db, &target_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(active.id, format!("{target_id}/default"));
    assert!(!common::mods_ws::ue4ss_mods_dir(install.path())
        .join("CoolMod")
        .exists());
    assert_no_import_scratch(&server);
}

#[tokio::test]
async fn importing_into_the_same_library_pins_without_installing() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let (cool_id, _) = install_cool_mod(&mut ws, install.path(), &target_id).await;
    let scratch = tempfile::tempdir().unwrap();
    let file = scratch.path().join("p.psmods");
    export(&mut ws, &target_id, true, &file.to_string_lossy()).await;
    let version_id = read_profile(&file).unwrap().entries[0]
        .mod_version_id
        .clone();

    let reply = import(
        &mut ws,
        json!({ "target_id": target_id, "path": file.to_string_lossy() }),
    )
    .await;

    assert!(reply.get("error").is_none(), "{reply:?}");
    assert_eq!(reply["installed"], json!([]), "{reply:?}");
    assert_eq!(reply["pinned"], json!([cool_id]), "{reply:?}");
    assert_eq!(reply["missing"], json!([]), "{reply:?}");
    assert_eq!(reply["profile"]["name"], "Default (2)", "{reply:?}");
    assert_eq!(reply["profile"]["is_active"], false, "{reply:?}");
    let entry = entry_of(&reply["profile"], &cool_id).expect("the entry is written");
    assert_eq!(entry["mod_version_id"], version_id.as_str());
    assert_no_import_scratch(&server);
}

#[tokio::test]
async fn mods_neither_present_nor_included_are_reported_missing() {
    let source = common::start_test_server().await;
    let mut source_ws = common::connect(&source).await;
    let source_install = common::mods_ws::fake_windows_install();
    let source_target = add_target(&mut source_ws, source_install.path()).await;
    install_cool_mod(&mut source_ws, source_install.path(), &source_target).await;
    let scratch = tempfile::tempdir().unwrap();
    let file = scratch.path().join("p.psmods");
    export(
        &mut source_ws,
        &source_target,
        false,
        &file.to_string_lossy(),
    )
    .await;
    let shared = read_profile(&file).unwrap();

    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;

    let reply = import(
        &mut ws,
        json!({ "target_id": target_id, "path": file.to_string_lossy() }),
    )
    .await;

    assert!(reply.get("error").is_none(), "{reply:?}");
    let entry = &shared.entries[0];
    assert_eq!(
        reply["missing"],
        json!([{
            "mod_id": entry.mod_id,
            "name": entry.name,
            "version": entry.version,
            "reason": "not_in_library",
        }]),
        "{reply:?}"
    );
    assert_eq!(reply["installed"], json!([]), "{reply:?}");
    assert_eq!(reply["pinned"], json!([]), "{reply:?}");
    assert_eq!(reply["profile"]["mods"], json!([]), "{reply:?}");
    assert!(reply["profile"]["id"].is_string(), "{reply:?}");
}

#[tokio::test]
async fn a_present_mod_at_another_version_follows_current() {
    let source = common::start_test_server().await;
    let mut source_ws = common::connect(&source).await;
    let source_install = common::mods_ws::fake_windows_install();
    let source_target = add_target(&mut source_ws, source_install.path()).await;
    let (cool_id, _) =
        install_cool_mod(&mut source_ws, source_install.path(), &source_target).await;
    let scratch = tempfile::tempdir().unwrap();
    let file = scratch.path().join("p.psmods");
    export(
        &mut source_ws,
        &source_target,
        false,
        &file.to_string_lossy(),
    )
    .await;
    let exported_version = read_profile(&file).unwrap().entries[0]
        .mod_version_id
        .clone();

    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let archive = install.path().join("CoolMod-2.0.zip");
    write_zip(
        &archive,
        &[
            ("modinfo.json", br#"{"version":"2.0"}"#),
            ("CoolMod/Scripts/main.lua", b"print('cooler')"),
        ],
    );
    let installed = request(
        &mut ws,
        "mod_install",
        json!({ "path": archive.to_string_lossy(), "target_id": target_id, "enable": false }),
    )
    .await;
    assert!(installed["error"].is_null(), "{installed:?}");
    assert_eq!(installed["mod_id"], cool_id.as_str());
    let current = ps_db::mod_library::current_version(&*server.handle.app.driver, &cool_id)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(current.id, exported_version);

    let reply = import(
        &mut ws,
        json!({ "target_id": target_id, "path": file.to_string_lossy() }),
    )
    .await;

    assert!(reply.get("error").is_none(), "{reply:?}");
    assert_eq!(reply["following_current"], json!([cool_id]), "{reply:?}");
    assert_eq!(reply["pinned"], json!([]), "{reply:?}");
    assert_eq!(reply["installed"], json!([]), "{reply:?}");
    assert_eq!(reply["missing"], json!([]), "{reply:?}");
    let entry = entry_of(&reply["profile"], &cool_id).expect("the entry is written");
    assert!(entry["mod_version_id"].is_null(), "{reply:?}");
    assert_eq!(entry["enabled"], true);
}

#[tokio::test]
async fn a_given_name_is_used() {
    let scratch = tempfile::tempdir().unwrap();
    let file = scratch.path().join("p.psmods");
    share::write_psmods(&file, &shared_profile(vec![], vec![]), &[]).unwrap();
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let path = file.to_string_lossy().into_owned();

    let reply = import(
        &mut ws,
        json!({ "target_id": target_id, "path": path, "name": "Shared" }),
    )
    .await;
    assert!(reply.get("error").is_none(), "{reply:?}");
    assert_eq!(reply["profile"]["name"], "Shared", "{reply:?}");

    let long = "S".repeat(64);
    let mut names = Vec::new();
    for _ in 0..2 {
        let reply = import(
            &mut ws,
            json!({ "target_id": target_id, "path": path, "name": long }),
        )
        .await;
        assert!(reply.get("error").is_none(), "{reply:?}");
        names.push(reply["profile"]["name"].as_str().unwrap().to_string());
    }
    assert_eq!(names, vec![long.clone(), format!("{} (2)", "S".repeat(60))]);

    for name in ["   ".to_string(), "S".repeat(65)] {
        let reply = import(
            &mut ws,
            json!({ "target_id": target_id, "path": path, "name": name }),
        )
        .await;
        assert_eq!(reply["error"]["code"], "invalid_name", "{reply:?}");
        assert_eq!(reply["target_id"], target_id.as_str());
        assert_eq!(reply["path"], path.as_str());
    }
    let profiles = ps_db::mod_profiles::for_target(&*server.handle.app.driver, &target_id)
        .await
        .unwrap();
    assert_eq!(profiles.len(), 4);
}

#[tokio::test]
async fn select_uses_the_file_picker() {
    let scratch = tempfile::tempdir().unwrap();
    let file = scratch.path().join("picked.psmods");
    share::write_psmods(&file, &shared_profile(vec![], vec![]), &[]).unwrap();
    let server =
        common::start_desktop_test_server(std::sync::Arc::new(QueuedDialogProvider::new(vec![
            Some(file.clone()),
            None,
        ])))
        .await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;

    let imported = import(
        &mut ws,
        json!({ "target_id": target_id, "path": "__select__" }),
    )
    .await;
    assert!(imported.get("error").is_none(), "{imported:?}");
    assert_eq!(Path::new(imported["path"].as_str().unwrap()), file);
    assert_eq!(imported["profile"]["name"], "Handmade", "{imported:?}");

    let canceled = import(
        &mut ws,
        json!({ "target_id": target_id, "path": "__select__" }),
    )
    .await;
    assert_eq!(canceled["canceled"], true, "{canceled:?}");
    assert!(canceled.get("error").is_none(), "{canceled:?}");
    assert!(canceled.get("profile").is_none(), "{canceled:?}");
    assert_eq!(canceled["target_id"], target_id.as_str());
}

#[tokio::test]
async fn a_foreign_zip_is_refused() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let scratch = tempfile::tempdir().unwrap();

    let foreign = scratch.path().join("foreign.psmods");
    write_zip(&foreign, &[("readme.txt", b"hi")]);
    let foreign_path = foreign.to_string_lossy().into_owned();
    let reply = import(
        &mut ws,
        json!({ "target_id": target_id, "path": foreign_path }),
    )
    .await;
    assert_eq!(reply["error"]["code"], "invalid_archive", "{reply:?}");
    assert_eq!(reply["path"], foreign_path.as_str());
    assert_eq!(reply["target_id"], target_id.as_str());

    let other = scratch.path().join("other.psmods");
    write_zip(
        &other,
        &[("profile.json", br#"{"format":"x","format_version":1}"#)],
    );
    let reply = import(
        &mut ws,
        json!({ "target_id": target_id, "path": other.to_string_lossy() }),
    )
    .await;
    assert_eq!(reply["error"]["code"], "unsupported_format", "{reply:?}");
    assert_eq!(reply["error"]["format"], "x", "{reply:?}");
    assert_eq!(reply["error"]["format_version"], 1, "{reply:?}");

    for path in [
        "p.psmods".to_string(),
        scratch
            .path()
            .join("absent.psmods")
            .to_string_lossy()
            .into_owned(),
    ] {
        let reply = import(&mut ws, json!({ "target_id": target_id, "path": path })).await;
        assert_eq!(reply["error"]["code"], "invalid_path", "{path}: {reply:?}");
    }
    let reply = import(
        &mut ws,
        json!({ "target_id": target_id, "path": "__select__" }),
    )
    .await;
    assert_eq!(reply["error"]["code"], "desktop_only", "{reply:?}");
    let reply = import(
        &mut ws,
        json!({ "target_id": "nope", "path": foreign_path }),
    )
    .await;
    assert_eq!(reply["error"]["code"], "target_not_found", "{reply:?}");

    let profiles = ps_db::mod_profiles::for_target(&*server.handle.app.driver, &target_id)
        .await
        .unwrap();
    assert_eq!(profiles.len(), 1);
}

#[tokio::test]
async fn frameworks_are_reported_not_applied() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let scratch = tempfile::tempdir().unwrap();
    let file = scratch.path().join("fw.psmods");
    let framework = SharedFramework {
        framework: "ue4ss".to_string(),
        mod_id: "ue4ss".to_string(),
        version: "3.0".to_string(),
        mod_version_id: "ue4ss@3.0".to_string(),
    };
    share::write_psmods(&file, &shared_profile(vec![], vec![framework]), &[]).unwrap();

    let reply = import(
        &mut ws,
        json!({ "target_id": target_id, "path": file.to_string_lossy() }),
    )
    .await;

    assert!(reply.get("error").is_none(), "{reply:?}");
    assert_eq!(
        reply["frameworks"],
        json!([{ "framework": "ue4ss", "mod_id": "ue4ss", "version": "3.0", "in_library": false }]),
        "{reply:?}"
    );
    let frameworks = ps_db::mod_profiles::frameworks_of(&*server.handle.app.driver, &target_id)
        .await
        .unwrap();
    assert!(frameworks.is_empty(), "{frameworks:?}");
}

#[tokio::test]
async fn an_entry_the_target_cannot_run_joins_disabled() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let client_id = add_target(&mut ws, install.path()).await;
    store_workshop_pack(&server, "coolpack", "CoolPack").await;
    ps_db::mod_profiles::set_mod(
        &*server.handle.app.driver,
        &ps_db::mod_profiles::ProfileModRow {
            profile_id: format!("{client_id}/default"),
            mod_id: "coolpack".to_string(),
            mod_version_id: None,
            enabled: true,
            load_order: 0,
        },
    )
    .await
    .unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let file = scratch.path().join("p.psmods");
    let exported = export(&mut ws, &client_id, false, &file.to_string_lossy()).await;
    assert_eq!(exported["entries"], 1, "{exported:?}");
    let root = tempfile::tempdir().unwrap();
    let docker_id = docker_target(&server, root.path()).await;

    let reply = import(
        &mut ws,
        json!({ "target_id": docker_id, "path": file.to_string_lossy() }),
    )
    .await;

    assert!(reply.get("error").is_none(), "{reply:?}");
    assert_eq!(
        reply["disabled"],
        json!([{ "mod_id": "coolpack", "code": "not_supported_on_target" }]),
        "{reply:?}"
    );
    assert_eq!(reply["pinned"], json!(["coolpack"]), "{reply:?}");
    let entry = entry_of(&reply["profile"], "coolpack").expect("the entry is written");
    assert_eq!(entry["enabled"], false, "{reply:?}");
}

#[tokio::test]
async fn included_archives_that_do_not_match_or_install_are_missing() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let scratch = tempfile::tempdir().unwrap();
    let cool = scratch.path().join("CoolMod-1.0.zip");
    write_zip(&cool, &[("CoolMod/Scripts/main.lua", b"print('cool')")]);
    let broken = scratch.path().join("Broken.zip");
    std::fs::write(&broken, b"not a zip").unwrap();
    let renamed_archive = share::archive_entry_name("renamed-ue4ss@1.0", "CoolMod-1.0.zip");
    let broken_archive = share::archive_entry_name("broken-ue4ss@1.0", "Broken.zip");
    let absent_archive = share::archive_entry_name("absent-ue4ss@1.0", "Absent.zip");
    let profile = shared_profile(
        vec![
            shared_entry("absent-ue4ss", 2, Some(absent_archive)),
            shared_entry("broken-ue4ss", 1, Some(broken_archive.clone())),
            shared_entry("renamed-ue4ss", 0, Some(renamed_archive.clone())),
        ],
        vec![],
    );
    let file = scratch.path().join("p.psmods");
    share::write_psmods(
        &file,
        &profile,
        &[(renamed_archive, cool), (broken_archive, broken)],
    )
    .unwrap();

    let reply = import(
        &mut ws,
        json!({ "target_id": target_id, "path": file.to_string_lossy() }),
    )
    .await;

    assert!(reply.get("error").is_none(), "{reply:?}");
    let reasons: Vec<(&str, &str)> = reply["missing"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| (m["mod_id"].as_str().unwrap(), m["reason"].as_str().unwrap()))
        .collect();
    assert_eq!(
        reasons,
        vec![
            ("renamed-ue4ss", "id_mismatch"),
            ("broken-ue4ss", "nothing_routed"),
            ("absent-ue4ss", "extract_failed"),
        ],
        "{reply:?}"
    );
    assert_eq!(reply["installed"], json!([]), "{reply:?}");
    assert_eq!(reply["profile"]["mods"], json!([]), "{reply:?}");
    let db = &*server.handle.app.driver;
    assert!(ps_db::mod_library::get_mod(db, "coolmod-ue4ss")
        .await
        .unwrap()
        .is_some());
    assert_no_import_scratch(&server);
}

#[tokio::test]
async fn an_archive_named_by_several_entries_is_installed_once() {
    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = add_target(&mut ws, install.path()).await;
    let scratch = tempfile::tempdir().unwrap();
    let cool = scratch.path().join("CoolMod-1.0.zip");
    write_zip(&cool, &[("CoolMod/Scripts/main.lua", b"print('cool')")]);
    let archive = share::archive_entry_name("coolmod-ue4ss@unversioned", "CoolMod-1.0.zip");
    let mut matching = shared_entry("coolmod-ue4ss", 0, Some(archive.clone()));
    matching.mod_version_id = "coolmod-ue4ss@unversioned".to_string();
    let profile = shared_profile(
        vec![
            matching,
            shared_entry("other-ue4ss", 1, Some(archive.clone())),
        ],
        vec![],
    );
    let file = scratch.path().join("p.psmods");
    share::write_psmods(&file, &profile, &[(archive, cool)]).unwrap();

    let reply = import(
        &mut ws,
        json!({ "target_id": target_id, "path": file.to_string_lossy() }),
    )
    .await;

    assert!(reply.get("error").is_none(), "{reply:?}");
    assert_eq!(
        reply["installed"],
        json!([{ "mod_id": "coolmod-ue4ss", "version_id": "coolmod-ue4ss@unversioned" }]),
        "{reply:?}"
    );
    assert_eq!(reply["pinned"], json!(["coolmod-ue4ss"]), "{reply:?}");
    assert_eq!(
        reply["missing"],
        json!([{
            "mod_id": "other-ue4ss",
            "name": "other-ue4ss",
            "version": "1.0",
            "reason": "id_mismatch",
        }]),
        "{reply:?}"
    );
    let db = &*server.handle.app.driver;
    let versions = ps_db::mod_library::versions_of(db, "coolmod-ue4ss")
        .await
        .unwrap();
    assert_eq!(versions.len(), 1, "{versions:?}");
    let kept = versions[0]
        .archive_path
        .as_deref()
        .expect("the archive is kept");
    assert!(Path::new(kept).is_file(), "{kept}");
    assert!(ps_db::mod_library::get_mod(db, "other-ue4ss")
        .await
        .unwrap()
        .is_none());
    assert_no_import_scratch(&server);
}
