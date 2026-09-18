mod common;

use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use ps_server::messages::MessageType;
use ps_server::signal::framing::{AssemblerOutcome, ChunkAssembler};
use ps_server::signal::remote_ctl;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

const CTL_CAPACITY: usize = 16;

fn spawn_bridge(
    app: Arc<ps_server::AppState>,
) -> (
    mpsc::Sender<Value>,
    mpsc::Receiver<String>,
    CancellationToken,
) {
    let (ctl_in_tx, ctl_in_rx) = mpsc::channel(CTL_CAPACITY);
    let (ctl_out_tx, ctl_out_rx) = mpsc::channel(CTL_CAPACITY);
    let cancel = CancellationToken::new();
    tokio::spawn(remote_ctl::run_ctl_bridge(
        ctl_in_rx,
        ctl_out_tx,
        app,
        cancel.clone(),
    ));
    (ctl_in_tx, ctl_out_rx, cancel)
}

async fn next_frame(ctl_out_rx: &mut mpsc::Receiver<String>) -> Value {
    let text = tokio::time::timeout(std::time::Duration::from_secs(5), ctl_out_rx.recv())
        .await
        .expect("timed out waiting for a ctl_out frame")
        .expect("ctl_out closed");
    serde_json::from_str(&text).unwrap()
}

#[tokio::test]
async fn bridge_dispatches_an_allowed_message_and_replies_on_ctl_out() {
    let server = common::start_test_server().await;
    let (ctl_in_tx, mut ctl_out_rx, cancel) = spawn_bridge(server.handle.app.clone());

    ctl_in_tx
        .send(json!({"type":"list_servers"}))
        .await
        .unwrap();

    let frame = next_frame(&mut ctl_out_rx).await;
    assert_eq!(frame["type"], "list_servers");
    assert_eq!(frame["data"]["servers"], json!([]));

    cancel.cancel();
    server.handle.shutdown().await;
}

#[tokio::test]
async fn denylisted_types_are_refused_under_their_own_type() {
    let server = common::start_test_server().await;
    let (ctl_in_tx, mut ctl_out_rx, cancel) = spawn_bridge(server.handle.app.clone());

    ctl_in_tx.send(json!({"type":"unlock_map"})).await.unwrap();

    let frame = next_frame(&mut ctl_out_rx).await;
    assert_eq!(frame["type"], "unlock_map");
    let error = frame["data"]["error"]
        .as_str()
        .expect("denylisted reply must carry data.error");
    assert!(
        error.to_lowercase().contains("remote"),
        "expected the refusal to mention remote unavailability, got: {error}"
    );

    cancel.cancel();
    server.handle.shutdown().await;
}

#[tokio::test]
async fn select_save_over_the_bridge_loads_by_path_without_a_dialog() {
    struct PanicIfCalledDialogProvider;

    impl ps_server::desktop_dialogs::FileDialogProvider for PanicIfCalledDialogProvider {
        fn pick_file(
            &self,
            _request: ps_server::desktop_dialogs::FileDialogRequest,
        ) -> ps_server::desktop_dialogs::DialogFuture {
            panic!("pick_file must not be invoked for a remote select_save call");
        }
        fn save_file(
            &self,
            _request: ps_server::desktop_dialogs::FileSaveRequest,
        ) -> ps_server::desktop_dialogs::DialogFuture {
            panic!("save_file must not be invoked for a remote select_save call");
        }
        fn pick_folder(
            &self,
            _initial_directory: Option<std::path::PathBuf>,
        ) -> ps_server::desktop_dialogs::DialogFuture {
            panic!("pick_folder must not be invoked for a remote select_save call");
        }
        fn pick_files(
            &self,
            _request: ps_server::desktop_dialogs::FileDialogRequest,
        ) -> ps_server::desktop_dialogs::DialogFilesFuture {
            panic!("pick_files must not be invoked for a remote select_save call");
        }
    }

    let server = common::start_desktop_test_server(Arc::new(PanicIfCalledDialogProvider)).await;
    let (ctl_in_tx, mut ctl_out_rx, cancel) = spawn_bridge(server.handle.app.clone());

    let save_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/saves/v1_relics");
    let level_sav_path = save_dir.join("Level.sav");

    ctl_in_tx
        .send(json!({
            "type": "select_save",
            "data": {"type": "steam", "path": level_sav_path.to_string_lossy(), "local": false}
        }))
        .await
        .unwrap();

    let loaded = loop {
        let frame = next_frame(&mut ctl_out_rx).await;
        if frame["type"] == "error" {
            panic!("select_save over the bridge errored: {frame}");
        }
        if frame["type"] == "loaded_save_files" {
            break frame;
        }
    };
    assert!(loaded["data"]["session_id"].is_string());
    assert_eq!(loaded["data"]["type"], "steam");

    cancel.cancel();
    server.handle.shutdown().await;
}

#[tokio::test]
async fn list_local_saves_is_refused_without_desktop_mode() {
    let server = common::start_test_server().await;
    let (ctl_in_tx, mut ctl_out_rx, cancel) = spawn_bridge(server.handle.app.clone());

    ctl_in_tx
        .send(json!({"type":"list_local_saves"}))
        .await
        .unwrap();

    let frame = next_frame(&mut ctl_out_rx).await;
    assert_eq!(frame["type"], "list_local_saves");
    assert!(frame["data"]["error"].is_string());

    cancel.cancel();
    server.handle.shutdown().await;
}

#[tokio::test]
async fn list_local_saves_rides_the_bridge_in_desktop_mode() {
    let server = common::start_desktop_test_server(std::sync::Arc::new(
        ps_server::desktop_dialogs::NullDialogProvider,
    ))
    .await;
    let (ctl_in_tx, mut ctl_out_rx, cancel) = spawn_bridge(server.handle.app.clone());

    ctl_in_tx
        .send(json!({"type":"list_local_saves"}))
        .await
        .unwrap();

    let frame = next_frame(&mut ctl_out_rx).await;
    assert_eq!(frame["type"], "list_local_saves");
    assert!(frame["data"]["saves"].is_array());

    cancel.cancel();
    server.handle.shutdown().await;
}

#[tokio::test]
async fn signal_control_is_refused() {
    let server = common::start_test_server().await;
    let armed_before = server.handle.services.signal.lock().await.armed();
    let (ctl_in_tx, mut ctl_out_rx, cancel) = spawn_bridge(server.handle.app.clone());

    ctl_in_tx
        .send(json!({"type":"signal_set_armed","data":{"armed":false}}))
        .await
        .unwrap();

    let frame = next_frame(&mut ctl_out_rx).await;
    assert_eq!(frame["type"], "signal_set_armed");
    assert!(frame["data"]["error"].is_string());

    let armed_after = server.handle.services.signal.lock().await.armed();
    assert_eq!(armed_before, armed_after, "armed state must not change");

    cancel.cancel();
    server.handle.shutdown().await;
}

#[tokio::test]
async fn install_plugin_is_refused_and_installs_nothing() {
    let server = common::start_test_server().await;
    let (ctl_in_tx, mut ctl_out_rx, cancel) = spawn_bridge(server.handle.app.clone());

    ctl_in_tx
        .send(json!({
            "type": "install_plugin",
            "data": {
                "filename": "sneaky.lua",
                "content": base64_lua_fixture(),
            }
        }))
        .await
        .unwrap();

    let frame = next_frame(&mut ctl_out_rx).await;
    assert_eq!(frame["type"], "install_plugin");
    let error = frame["data"]["error"]
        .as_str()
        .expect("denylisted reply must carry data.error");
    assert!(
        error.to_lowercase().contains("remote"),
        "expected the refusal to mention remote unavailability, got: {error}"
    );

    let installed = ps_db::plugins::get_all(&*server.handle.app.driver)
        .await
        .unwrap();
    assert!(
        installed.iter().all(|row| row.id != "sneaky"),
        "install_plugin must not install anything when refused"
    );

    cancel.cancel();
    server.handle.shutdown().await;
}

#[tokio::test]
async fn mod_analyze_is_refused_over_the_bridge() {
    let server = common::start_test_server().await;
    let (ctl_in_tx, mut ctl_out_rx, cancel) = spawn_bridge(server.handle.app.clone());

    ctl_in_tx
        .send(json!({
            "type": "mod_analyze",
            "data": { "target_id": "client-nope", "path": "/etc/passwd" }
        }))
        .await
        .unwrap();

    let frame = next_frame(&mut ctl_out_rx).await;
    assert_eq!(frame["type"], "mod_analyze");
    assert_remote_denied(&frame);
    assert_eq!(frame["data"]["target_id"], "client-nope");
    assert_eq!(frame["data"]["path"], "/etc/passwd");

    cancel.cancel();
    server.handle.shutdown().await;
}

#[tokio::test]
async fn mod_install_is_refused_and_installs_nothing() {
    let server = common::start_test_server().await;
    let (ctl_in_tx, mut ctl_out_rx, cancel) = spawn_bridge(server.handle.app.clone());

    ctl_in_tx
        .send(json!({
            "type": "mod_install",
            "data": {
                "path": "/etc/passwd",
                "target_id": "client-nope",
            }
        }))
        .await
        .unwrap();

    let frame = next_frame(&mut ctl_out_rx).await;
    assert_eq!(frame["type"], "mod_install");
    assert_remote_denied(&frame);
    assert_eq!(frame["data"]["target_id"], "client-nope");
    assert_eq!(frame["data"]["path"], "/etc/passwd");

    let mods = ps_db::mod_library::list_mods(&*server.handle.app.driver)
        .await
        .unwrap();
    assert!(
        mods.is_empty(),
        "mod_install must not touch the library when refused"
    );

    cancel.cancel();
    server.handle.shutdown().await;
}

#[tokio::test]
async fn mod_target_add_is_refused_and_registers_nothing() {
    let server = common::start_test_server().await;
    let (ctl_in_tx, mut ctl_out_rx, cancel) = spawn_bridge(server.handle.app.clone());
    let install = common::mods_ws::fake_windows_install();

    ctl_in_tx
        .send(json!({
            "type": "mod_target_add",
            "data": { "root_path": install.path().to_string_lossy() }
        }))
        .await
        .unwrap();

    let frame = next_frame(&mut ctl_out_rx).await;
    assert_eq!(frame["type"], "mod_target_add");
    assert_remote_denied(&frame);
    assert_eq!(
        frame["data"]["root_path"],
        install.path().to_string_lossy().as_ref()
    );

    let targets = ps_db::mod_targets::list(&*server.handle.app.driver)
        .await
        .unwrap();
    assert!(
        targets.is_empty(),
        "mod_target_add must not register a target when refused: {targets:?}"
    );

    cancel.cancel();
    server.handle.shutdown().await;
}

#[tokio::test]
async fn mod_backup_list_is_refused_over_the_bridge() {
    let server = common::start_test_server().await;
    let (ctl_in_tx, mut ctl_out_rx, cancel) = spawn_bridge(server.handle.app.clone());

    ctl_in_tx
        .send(json!({
            "type": "mod_backup_list",
            "data": { "target_id": "client-nope" }
        }))
        .await
        .unwrap();

    let frame = next_frame(&mut ctl_out_rx).await;
    assert_eq!(frame["type"], "mod_backup_list");
    assert_remote_denied(&frame);
    assert_eq!(frame["data"]["target_id"], "client-nope");

    cancel.cancel();
    server.handle.shutdown().await;
}

#[tokio::test]
async fn mod_backup_restore_is_refused_over_the_bridge() {
    let server = common::start_test_server().await;
    let (ctl_in_tx, mut ctl_out_rx, cancel) = spawn_bridge(server.handle.app.clone());

    ctl_in_tx
        .send(json!({
            "type": "mod_backup_restore",
            "data": { "target_id": "client-nope", "set": "20260101-000000-abc123" }
        }))
        .await
        .unwrap();

    let frame = next_frame(&mut ctl_out_rx).await;
    assert_eq!(frame["type"], "mod_backup_restore");
    assert_remote_denied(&frame);
    assert_eq!(frame["data"]["target_id"], "client-nope");
    assert_eq!(frame["data"]["set"], "20260101-000000-abc123");

    cancel.cancel();
    server.handle.shutdown().await;
}

#[tokio::test]
async fn mod_backup_delete_is_refused_and_deletes_nothing() {
    let server = common::start_test_server().await;
    let library = ps_server::services::mods::LibraryPaths::new(server._temp_dir.path());
    let install = common::mods_ws::fake_windows_install();
    let target = ps_db::mod_targets::upsert(
        &*server.handle.app.driver,
        &ps_db::mod_targets::NewModTarget {
            id: "client-bridge-test".to_string(),
            kind: "client".to_string(),
            name: "Bridge Test".to_string(),
            root_path: install.path().to_string_lossy().into_owned(),
            platform: "win64".to_string(),
            ue4ss_mode: "standard".to_string(),
            layout_overrides: "{}".to_string(),
            detected: "{}".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let original = install.path().join("Pal/Binaries/Win64/main.lua");
    std::fs::write(&original, b"x").unwrap();
    let mut set = ps_server::services::mods::deploy::backup::BackupSet::open(
        &library,
        &target.id,
        "20260101-000000",
        "abc123",
    )
    .unwrap();
    set.copy_in(&original).unwrap();
    set.write_index().unwrap();

    let (ctl_in_tx, mut ctl_out_rx, cancel) = spawn_bridge(server.handle.app.clone());
    ctl_in_tx
        .send(json!({
            "type": "mod_backup_delete",
            "data": { "target_id": target.id, "set": "20260101-000000-abc123" }
        }))
        .await
        .unwrap();

    let frame = next_frame(&mut ctl_out_rx).await;
    assert_eq!(frame["type"], "mod_backup_delete");
    assert_remote_denied(&frame);
    assert_eq!(frame["data"]["target_id"], "client-bridge-test");
    assert_eq!(frame["data"]["set"], "20260101-000000-abc123");
    assert!(
        set.dir().exists(),
        "mod_backup_delete must not remove the set when refused"
    );

    cancel.cancel();
    server.handle.shutdown().await;
}

async fn reply_of_type(ctl_out_rx: &mut mpsc::Receiver<String>, message_type: &str) -> Value {
    loop {
        let frame = next_frame(ctl_out_rx).await;
        assert_ne!(frame["type"], "error", "{frame}");
        if frame["type"] == message_type {
            return frame["data"].clone();
        }
    }
}

async fn bridge_request(
    ctl_in_tx: &mpsc::Sender<Value>,
    ctl_out_rx: &mut mpsc::Receiver<String>,
    message_type: &str,
    data: Value,
) -> Value {
    ctl_in_tx
        .send(json!({ "type": message_type, "data": data }))
        .await
        .unwrap();
    reply_of_type(ctl_out_rx, message_type).await
}

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

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    format!("{:x}", sha2::Sha256::digest(bytes))
}

async fn begin_upload_over_bridge(
    ctl_in_tx: &mpsc::Sender<Value>,
    ctl_out_rx: &mut mpsc::Receiver<String>,
    name: &str,
    bytes: &[u8],
) -> String {
    let begun = bridge_request(
        ctl_in_tx,
        ctl_out_rx,
        "mod_upload_begin",
        json!({ "name": name, "size": bytes.len(), "sha256": sha256_hex(bytes) }),
    )
    .await;
    assert!(begun.get("error").is_none(), "{begun:?}");
    begun["upload_id"].as_str().unwrap().to_string()
}

async fn upload_over_bridge(
    ctl_in_tx: &mpsc::Sender<Value>,
    ctl_out_rx: &mut mpsc::Receiver<String>,
    name: &str,
    bytes: &[u8],
) -> String {
    use base64::Engine;
    let upload_id = begin_upload_over_bridge(ctl_in_tx, ctl_out_rx, name, bytes).await;
    let chunk = bridge_request(
        ctl_in_tx,
        ctl_out_rx,
        "mod_upload_chunk",
        json!({
            "upload_id": upload_id,
            "seq": 0,
            "data_b64": base64::engine::general_purpose::STANDARD.encode(bytes),
        }),
    )
    .await;
    assert!(chunk.get("error").is_none(), "{chunk:?}");
    let ended = bridge_request(
        ctl_in_tx,
        ctl_out_rx,
        "mod_upload_end",
        json!({ "upload_id": upload_id }),
    )
    .await;
    assert!(ended.get("error").is_none(), "{ended:?}");
    ended["path"].as_str().unwrap().to_string()
}

async fn register_client_target(server: &common::TestServer, root: &Path) -> String {
    let db = &*server.handle.app.driver;
    let target = ps_db::mod_targets::upsert(
        db,
        &ps_db::mod_targets::NewModTarget {
            id: "client-bridge-test".to_string(),
            kind: "client".to_string(),
            name: "Bridge Test".to_string(),
            root_path: root.to_string_lossy().into_owned(),
            platform: "win64".to_string(),
            ue4ss_mode: "standard".to_string(),
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

#[tokio::test]
async fn game_launch_and_profile_export_are_refused_over_the_bridge() {
    let server = common::start_test_server().await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = register_client_target(&server, install.path()).await;
    let (ctl_in_tx, mut ctl_out_rx, cancel) = spawn_bridge(server.handle.app.clone());

    let launched = bridge_request(
        &ctl_in_tx,
        &mut ctl_out_rx,
        "game_launch",
        json!({ "target_id": target_id, "world_key": "C:/saves/abc" }),
    )
    .await;
    assert_remote_denied(&json!({ "data": launched }));
    assert_eq!(launched["target_id"], target_id.as_str());
    assert_eq!(launched["world_key"], "C:/saves/abc");

    let scratch = tempfile::tempdir().unwrap();
    let file = scratch.path().join("p.psmods");
    let profile_id = format!("{target_id}/default");
    let exported = bridge_request(
        &ctl_in_tx,
        &mut ctl_out_rx,
        "profile_export",
        json!({
            "target_id": target_id,
            "profile_id": profile_id,
            "include_archives": false,
            "path": file.to_string_lossy(),
        }),
    )
    .await;
    assert_remote_denied(&json!({ "data": exported }));
    assert_eq!(exported["target_id"], target_id.as_str());
    assert_eq!(exported["profile_id"], profile_id.as_str());
    assert_eq!(exported["path"], file.to_string_lossy().as_ref());
    assert!(!file.exists(), "profile_export must not write when refused");

    cancel.cancel();
    server.handle.shutdown().await;
}

#[tokio::test]
async fn an_uploaded_archive_can_be_installed_over_the_bridge() {
    let server = common::start_test_server().await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = register_client_target(&server, install.path()).await;
    let (ctl_in_tx, mut ctl_out_rx, cancel) = spawn_bridge(server.handle.app.clone());

    let scratch = tempfile::tempdir().unwrap();
    let zip_path = scratch.path().join("CoolMod-1.0.zip");
    write_zip(&zip_path, &[("CoolMod/Scripts/main.lua", b"print('hi')")]);
    let archive = std::fs::read(&zip_path).unwrap();
    let path = upload_over_bridge(&ctl_in_tx, &mut ctl_out_rx, "CoolMod-1.0.zip", &archive).await;

    let installed = bridge_request(
        &ctl_in_tx,
        &mut ctl_out_rx,
        "mod_install",
        json!({ "path": path, "target_id": target_id }),
    )
    .await;
    assert!(installed.get("error").is_none(), "{installed:?}");
    let mod_id = installed["mod_id"].as_str().unwrap().to_string();

    let listed = bridge_request(&ctl_in_tx, &mut ctl_out_rx, "mod_list", Value::Null).await;
    let mods = listed["mods"].as_array().unwrap();
    assert!(
        mods.iter().any(|row| row["id"] == mod_id.as_str()
            && row["versions"][0]["manifest"]["folder_name"] == "CoolMod"),
        "{mods:?}"
    );

    cancel.cancel();
    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_path_outside_downloads_is_still_refused() {
    let server = common::start_test_server().await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = register_client_target(&server, install.path()).await;
    let (ctl_in_tx, mut ctl_out_rx, cancel) = spawn_bridge(server.handle.app.clone());

    let scratch = tempfile::tempdir().unwrap();
    let outside = scratch.path().join("CoolMod-1.0.zip");
    write_zip(&outside, &[("CoolMod/Scripts/main.lua", b"print('hi')")]);
    let archive = std::fs::read(&outside).unwrap();
    let downloads = server._temp_dir.path().join("downloads");
    let upload_id =
        begin_upload_over_bridge(&ctl_in_tx, &mut ctl_out_rx, "CoolMod-1.0.zip", &archive).await;
    let pending = downloads.join(".uploads").join(&upload_id);
    assert!(pending.is_file(), "{}", pending.display());

    let cases = [
        ("mod_install", outside.to_string_lossy().into_owned()),
        (
            "mod_analyze",
            downloads
                .join("..")
                .join("mods")
                .join("x.zip")
                .to_string_lossy()
                .into_owned(),
        ),
        ("mod_install", "__select__".to_string()),
        ("mod_install", pending.to_string_lossy().into_owned()),
    ];
    for (message_type, path) in cases {
        let reply = bridge_request(
            &ctl_in_tx,
            &mut ctl_out_rx,
            message_type,
            json!({ "path": path, "target_id": target_id }),
        )
        .await;
        assert_remote_denied(&json!({ "data": reply }));
        assert_eq!(reply["path"], path.as_str());
        assert_eq!(reply["target_id"], target_id.as_str());
    }

    let mods = ps_db::mod_library::list_mods(&*server.handle.app.driver)
        .await
        .unwrap();
    assert!(mods.is_empty(), "{mods:?}");

    cancel.cancel();
    server.handle.shutdown().await;
}

#[tokio::test]
async fn an_upload_of_the_wrong_file_type_is_still_refused() {
    let server = common::start_test_server().await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = register_client_target(&server, install.path()).await;
    let (ctl_in_tx, mut ctl_out_rx, cancel) = spawn_bridge(server.handle.app.clone());

    let scratch = tempfile::tempdir().unwrap();
    let zip_path = scratch.path().join("CoolMod-1.0.zip");
    write_zip(&zip_path, &[("CoolMod/Scripts/main.lua", b"print('hi')")]);
    let archive = std::fs::read(&zip_path).unwrap();
    let zip = upload_over_bridge(&ctl_in_tx, &mut ctl_out_rx, "CoolMod-1.0.zip", &archive).await;
    let psmods =
        upload_over_bridge(&ctl_in_tx, &mut ctl_out_rx, "CoolMod-1.0.psmods", &archive).await;

    for (message_type, path) in [
        ("mod_install", &psmods),
        ("mod_analyze", &psmods),
        ("profile_import", &zip),
    ] {
        let reply = bridge_request(
            &ctl_in_tx,
            &mut ctl_out_rx,
            message_type,
            json!({ "path": path, "target_id": target_id }),
        )
        .await;
        assert_remote_denied(&json!({ "data": reply }));
        assert_eq!(reply["path"], path.as_str());
    }

    let mods = ps_db::mod_library::list_mods(&*server.handle.app.driver)
        .await
        .unwrap();
    assert!(mods.is_empty(), "{mods:?}");

    cancel.cancel();
    server.handle.shutdown().await;
}

#[tokio::test]
async fn profile_import_of_an_upload_is_allowed() {
    use ps_server::services::mods::share::{self, SharedProfile};

    let server = common::start_test_server().await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = register_client_target(&server, install.path()).await;
    let (ctl_in_tx, mut ctl_out_rx, cancel) = spawn_bridge(server.handle.app.clone());

    let scratch = tempfile::tempdir().unwrap();
    let file = scratch.path().join("Handmade.psmods");
    let profile = SharedProfile {
        format: share::FORMAT.to_string(),
        format_version: share::FORMAT_VERSION,
        name: "Handmade".to_string(),
        exported_at: "2026-09-14T12:00:00+00:00".to_string(),
        target_kind: "client".to_string(),
        target_platform: "win64".to_string(),
        entries: vec![],
        frameworks: vec![],
    };
    share::write_psmods(&file, &profile, &[]).unwrap();
    let bytes = std::fs::read(&file).unwrap();
    let path = upload_over_bridge(&ctl_in_tx, &mut ctl_out_rx, "Handmade.psmods", &bytes).await;

    let imported = bridge_request(
        &ctl_in_tx,
        &mut ctl_out_rx,
        "profile_import",
        json!({ "path": path, "target_id": target_id }),
    )
    .await;
    assert!(imported.get("error").is_none(), "{imported:?}");
    assert_eq!(imported["profile"]["name"], "Handmade", "{imported:?}");

    cancel.cancel();
    server.handle.shutdown().await;
}

#[tokio::test]
async fn world_links_and_profile_messages_ride_the_bridge() {
    let server = common::start_test_server().await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = register_client_target(&server, install.path()).await;
    let (ctl_in_tx, mut ctl_out_rx, cancel) = spawn_bridge(server.handle.app.clone());

    let created = bridge_request(
        &ctl_in_tx,
        &mut ctl_out_rx,
        "profile_create",
        json!({ "target_id": target_id, "name": "Hard" }),
    )
    .await;
    assert!(created.get("error").is_none(), "{created:?}");
    let profile_id = created["profile"]["id"].as_str().unwrap().to_string();

    let linked = bridge_request(
        &ctl_in_tx,
        &mut ctl_out_rx,
        "world_profile_set",
        json!({ "world_key": "C:/saves/abc", "world_name": "Abc", "profile_id": profile_id }),
    )
    .await;
    assert!(linked.get("error").is_none(), "{linked:?}");
    assert_eq!(linked["linked"], true, "{linked:?}");

    cancel.cancel();
    server.handle.shutdown().await;
}

#[tokio::test]
async fn profile_entry_messages_ride_the_bridge() {
    let server = common::start_test_server().await;
    let install = common::mods_ws::fake_windows_install();
    let target_id = register_client_target(&server, install.path()).await;
    let (ctl_in_tx, mut ctl_out_rx, cancel) = spawn_bridge(server.handle.app.clone());

    let removed = bridge_request(
        &ctl_in_tx,
        &mut ctl_out_rx,
        "profile_remove_mod",
        json!({ "target_id": target_id, "mod_id": "nothing-here" }),
    )
    .await;
    assert_eq!(
        removed["error"]["code"], "mod_not_in_profile",
        "{removed:?}"
    );

    let released = bridge_request(
        &ctl_in_tx,
        &mut ctl_out_rx,
        "mod_release_profiles",
        json!({ "mod_id": "nothing-here" }),
    )
    .await;
    assert_eq!(
        released,
        json!({ "mod_id": "nothing-here", "removed": [], "enabled": [] })
    );

    cancel.cancel();
    server.handle.shutdown().await;
}

fn assert_remote_denied(frame: &Value) {
    assert_eq!(
        frame["data"]["error"]["code"], "remote_denied",
        "got: {frame}"
    );
    let message = frame["data"]["error"]["message"]
        .as_str()
        .expect("a mods refusal must carry error.message");
    assert!(
        message.to_lowercase().contains("remote"),
        "expected the refusal to mention remote unavailability, got: {message}"
    );
}

fn base64_lua_fixture() -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(b"function main() end")
}

#[tokio::test]
async fn oversized_responses_leave_as_chunks_that_reassemble() {
    let (emit_tx, emit_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let (ctl_out_tx, mut ctl_out_rx) = mpsc::channel::<String>(64);
    let drain = tokio::spawn(remote_ctl::drain_emitter(emit_rx, ctl_out_tx));

    let payload = "x".repeat(200 * 1024);
    let frame = json!({"type":"get_version","data":payload}).to_string();
    emit_tx.send(frame.clone()).unwrap();
    drop(emit_tx);

    let mut pieces = Vec::new();
    while let Some(piece) = ctl_out_rx.recv().await {
        pieces.push(piece);
    }
    let _ = drain.await;

    assert!(
        pieces.len() > 1,
        "a >64 KiB frame must split into more than one piece"
    );

    let mut assembler = ChunkAssembler::default();
    let mut complete = None;
    for piece in &pieces {
        assert!(
            piece.len() <= 64 * 1024,
            "chunk {} bytes exceeds the cap",
            piece.len()
        );
        let value: Value = serde_json::from_str(piece).unwrap();
        assert_eq!(value["type"], "chunk");
        match assembler.accept(value) {
            AssemblerOutcome::Complete(text) => complete = Some(text),
            AssemblerOutcome::Pending => {}
            other => panic!("unexpected assembler outcome: {other:?}"),
        }
    }
    assert_eq!(complete, Some(frame));
}

#[tokio::test]
async fn inbound_chunks_reassemble_before_dispatch() {
    let server = common::start_test_server().await;
    let (ctl_in_tx, mut ctl_out_rx, cancel) = spawn_bridge(server.handle.app.clone());

    let frame = r#"{"type":"list_servers"}"#;
    let (first, second) = frame.split_at(frame.len() / 2);
    ctl_in_tx
        .send(json!({"type":"chunk","data":{"id":7,"part":0,"parts":2,"data":first}}))
        .await
        .unwrap();
    ctl_in_tx
        .send(json!({"type":"chunk","data":{"id":7,"part":1,"parts":2,"data":second}}))
        .await
        .unwrap();

    let reply = next_frame(&mut ctl_out_rx).await;
    assert_eq!(reply["type"], "list_servers");
    assert_eq!(reply["data"]["servers"], json!([]));

    cancel.cancel();
    server.handle.shutdown().await;
}

#[tokio::test]
async fn mod_iostore_convert_is_refused_over_the_bridge() {
    let server = common::start_test_server().await;
    let (ctl_in_tx, mut ctl_out_rx, cancel) = spawn_bridge(server.handle.app.clone());

    ctl_in_tx
        .send(json!({
            "type": "mod_iostore_convert",
            "data": { "target_id": "client-nope", "mod_id": "cool-mod" }
        }))
        .await
        .unwrap();

    let frame = next_frame(&mut ctl_out_rx).await;
    assert_eq!(frame["type"], "mod_iostore_convert");
    assert_remote_denied(&frame);
    assert_eq!(frame["data"]["target_id"], "client-nope");
    assert_eq!(frame["data"]["mod_id"], "cool-mod");

    cancel.cancel();
    server.handle.shutdown().await;
}

#[tokio::test]
async fn reattach_with_unknown_id_reports_session_not_found() {
    let server = common::start_test_server().await;
    let (ctl_in_tx, mut ctl_out_rx, cancel) = spawn_bridge(server.handle.app.clone());

    let unknown_id = uuid::Uuid::new_v4().to_string();
    ctl_in_tx
        .send(json!({"type":"reattach_session","data":{"session_id":unknown_id}}))
        .await
        .unwrap();

    let frame = next_frame(&mut ctl_out_rx).await;
    assert_eq!(frame["type"], "session_not_found");
    assert_eq!(frame["data"], unknown_id);

    cancel.cancel();
    server.handle.shutdown().await;
}

#[test]
fn the_denylist_matches_the_vocabulary() {
    for entry in remote_ctl::REMOTE_DENYLIST {
        assert_eq!(
            MessageType::from_wire(entry.as_wire()),
            Some(*entry),
            "REMOTE_DENYLIST entry {entry:?} no longer round-trips through the wire vocabulary"
        );
    }
}

#[tokio::test]
async fn nexus_key_set_is_refused_over_the_bridge_without_echoing_the_key() {
    let server = common::start_test_server().await;
    let (ctl_in_tx, mut ctl_out_rx, cancel) = spawn_bridge(server.handle.app.clone());

    ctl_in_tx
        .send(json!({ "type": "nexus_key_set", "data": { "key": "secret-remote-key" } }))
        .await
        .unwrap();

    let frame = next_frame(&mut ctl_out_rx).await;
    assert_eq!(frame["type"], "nexus_key_set");
    assert_remote_denied(&frame);
    assert!(!frame.to_string().contains("secret-remote-key"), "{frame}");

    cancel.cancel();
    server.handle.shutdown().await;
}

#[tokio::test]
async fn nexus_download_is_refused_over_the_bridge_echoing_its_ids() {
    let server = common::start_test_server().await;
    let (ctl_in_tx, mut ctl_out_rx, cancel) = spawn_bridge(server.handle.app.clone());

    ctl_in_tx
        .send(json!({
            "type": "nexus_download",
            "data": {
                "target_id": "client-nope", "mod_id": 4821, "file_id": 99001,
                "key": "k", "expires": 5
            }
        }))
        .await
        .unwrap();

    let frame = next_frame(&mut ctl_out_rx).await;
    assert_eq!(frame["type"], "nexus_download");
    assert_remote_denied(&frame);
    assert_eq!(frame["data"]["target_id"], "client-nope");
    assert_eq!(frame["data"]["mod_id"], 4821);
    assert_eq!(frame["data"]["file_id"], 99001);
    assert!(frame["data"].get("key").is_none());

    cancel.cancel();
    server.handle.shutdown().await;
}

#[test]
fn every_nexus_and_update_message_is_denylisted() {
    let names: Vec<MessageType> = MessageType::ALL
        .iter()
        .copied()
        .filter(|message_type| {
            let wire = message_type.as_wire();
            wire.starts_with("nexus_") || wire.starts_with("mod_update_")
        })
        .collect();
    assert_eq!(names.len(), 13, "{names:?}");
    for message_type in names {
        assert!(
            remote_ctl::REMOTE_DENYLIST.contains(&message_type),
            "{message_type:?} must be denied over a remote session"
        );
    }
}
