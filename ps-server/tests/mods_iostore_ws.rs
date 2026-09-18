mod common;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::json;

use ps_db::mod_targets::{ModTarget, NewModTarget};
use ps_server::desktop_dialogs::NullDialogProvider;
use ps_server::services::mods::frameworks::install::scratch_root;
use ps_server::services::mods::frameworks::source::Progress;
use ps_server::services::mods::iostore::{ConvertError, ConvertedPak, IoStoreConverter};
use ps_server::services::mods::LibraryPaths;
use ps_server::services::ServerServices;

/// Writes `<stem>.pak`/`.utoc`/`.ucas` stub bytes into `out_dir` and records
/// every pak it was asked to convert. `fail` makes every call refuse instead,
/// so a test can exercise a conversion failure without a real `retoc`.
#[derive(Default)]
struct FakeConverter {
    calls: Mutex<Vec<PathBuf>>,
    fail: AtomicBool,
}

#[async_trait::async_trait]
impl IoStoreConverter for FakeConverter {
    async fn convert(
        &self,
        pak: &Path,
        out_dir: &Path,
        _progress: Progress<'_>,
    ) -> Result<ConvertedPak, ConvertError> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(ConvertError::ConversionFailed(
                "fake conversion failure".to_string(),
            ));
        }
        self.calls.lock().unwrap().push(pak.to_path_buf());
        std::fs::create_dir_all(out_dir).unwrap();
        let stem = pak.file_stem().unwrap().to_string_lossy().into_owned();
        let pak_out = out_dir.join(format!("{stem}.pak"));
        let utoc_out = out_dir.join(format!("{stem}.utoc"));
        let ucas_out = out_dir.join(format!("{stem}.ucas"));
        std::fs::write(&pak_out, b"converted pak bytes").unwrap();
        std::fs::write(&utoc_out, b"utoc stub").unwrap();
        std::fs::write(&ucas_out, b"ucas stub").unwrap();
        Ok(ConvertedPak {
            pak: pak_out,
            utoc: utoc_out,
            ucas: ucas_out,
        })
    }
}

async fn start_iostore_server(fail: bool) -> (common::TestServer, Arc<FakeConverter>) {
    let converter = Arc::new(FakeConverter {
        fail: AtomicBool::new(fail),
        ..Default::default()
    });
    let docker = Arc::new(ps_server::services::docker::mock::MockDocker::default());
    let unused_app_root = std::env::temp_dir().join("ps-iostore-ws-test-unused-app-root");
    let mut services = ServerServices::with_docker(docker, unused_app_root);
    services.iostore = converter.clone();
    let dialogs: Arc<dyn ps_server::desktop_dialogs::FileDialogProvider> =
        Arc::new(NullDialogProvider);
    let server = common::start_desktop_test_server_with_services(dialogs, services).await;
    (server, converter)
}

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

async fn wingdk_target(db: &ps_db::SqlxSqliteDriver, id: &str, root: &Path) -> ModTarget {
    common::mods::install_tree(root);
    ps_db::mod_targets::upsert(
        db,
        &NewModTarget {
            id: id.to_string(),
            kind: "client".to_string(),
            name: id.to_string(),
            root_path: root.to_string_lossy().into_owned(),
            platform: "wingdk".to_string(),
            ue4ss_mode: "standard".to_string(),
            layout_overrides: "{}".to_string(),
            detected: "{}".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap()
}

async fn ready_wingdk_target(db: &ps_db::SqlxSqliteDriver, id: &str, root: &Path) -> String {
    let target = wingdk_target(db, id, root).await;
    common::mods::default_profile(db, &target.id).await;
    target.id
}

/// Sends `mod_iostore_convert` and waits for its reply, collecting the stage
/// names of every `mod_progress` frame seen along the way (the apply this
/// request triggers emits its own, under a different `request_id`).
async fn convert_and_wait(
    ws: &mut common::WsClient,
    target_id: &str,
    mod_id: &str,
) -> (serde_json::Value, Vec<String>) {
    common::send_json(
        ws,
        json!({
            "type": "mod_iostore_convert",
            "data": { "target_id": target_id, "mod_id": mod_id },
        }),
    )
    .await;
    let mut stages = Vec::new();
    loop {
        let frame = common::next_json(ws).await;
        if frame["type"] == "mod_progress" {
            if let Some(stage) = frame["data"]["stage"].as_str() {
                stages.push(stage.to_string());
            }
            continue;
        }
        assert_eq!(frame["type"], "mod_iostore_convert", "{frame:?}");
        return (frame["data"].clone(), stages);
    }
}

async fn refusal(ws: &mut common::WsClient, target_id: &str, mod_id: &str) -> serde_json::Value {
    convert_and_wait(ws, target_id, mod_id).await.0
}

async fn conflicts(ws: &mut common::WsClient, target_id: &str) -> serde_json::Value {
    common::send_json(
        ws,
        json!({"type": "mod_conflicts", "data": {
            "target_id": target_id,
            "profile_id": null,
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
async fn a_legacy_pak_mod_converts_and_deploys_iostore_files() {
    let (server, converter) = start_iostore_server(false).await;
    let mut ws = common::connect(&server).await;
    let db = driver_for(&server).await;
    let library = library_for(&server);
    let root = tempfile::tempdir().unwrap();
    let target_id = ready_wingdk_target(&db, "client-gamepass", root.path()).await;

    common::mods::install_fixture_mod(
        &db,
        &library,
        "cool-mod",
        "1.0",
        &[("pak", "CoolMod_P.pak", b"legacy pak bytes")],
    )
    .await;
    common::mods::enable(&db, &target_id, "cool-mod", None, true).await;

    let (data, stages) = convert_and_wait(&mut ws, &target_id, "cool-mod").await;
    assert!(data["error"].is_null(), "{data:?}");
    assert_eq!(data["target_id"], target_id);
    assert_eq!(data["mod_id"], "cool-mod");
    assert_eq!(data["version"], "1.0+iostore");
    assert_eq!(data["converted"], json!(["CoolMod_P.pak"]));
    assert_eq!(data["reused"], false);
    assert!(stages.contains(&"converting".to_string()), "{stages:?}");
    assert!(stages.contains(&"storing".to_string()), "{stages:?}");

    let new_version_id = data["mod_version_id"].as_str().unwrap().to_string();
    assert_eq!(new_version_id, "cool-mod@1.0+iostore");

    let new_version = ps_db::mod_library::get_version(&db, &new_version_id)
        .await
        .unwrap()
        .unwrap();
    let version_dir = Path::new(&new_version.library_dir);
    assert!(version_dir.join("pak/CoolMod_P.pak").is_file());
    assert!(version_dir.join("pak/CoolMod_P.utoc").is_file());
    assert!(version_dir.join("pak/CoolMod_P.ucas").is_file());

    let current = ps_db::mod_library::current_version(&db, "cool-mod")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        current.version, "1.0",
        "the legacy version must stay current so Steam targets keep it"
    );

    let entries = ps_db::mod_profiles::mods_of(&db, &format!("{target_id}/default"))
        .await
        .unwrap();
    let entry = entries.iter().find(|e| e.mod_id == "cool-mod").unwrap();
    assert_eq!(
        entry.mod_version_id.as_deref(),
        Some(new_version_id.as_str())
    );

    let mods_dir = root.path().join("Pal/Content/Paks/~mods");
    assert!(mods_dir.join("CoolMod_P.pak").is_file());
    assert!(mods_dir.join("CoolMod_P.utoc").is_file());
    assert!(mods_dir.join("CoolMod_P.ucas").is_file());

    assert_eq!(converter.calls.lock().unwrap().len(), 1);

    server.handle.shutdown().await;
}

#[tokio::test]
async fn converting_the_same_version_again_is_reused_without_a_second_call() {
    let (server, converter) = start_iostore_server(false).await;
    let mut ws = common::connect(&server).await;
    let db = driver_for(&server).await;
    let library = library_for(&server);
    let root = tempfile::tempdir().unwrap();
    let target_id = ready_wingdk_target(&db, "client-gamepass", root.path()).await;

    let original = common::mods::install_fixture_mod(
        &db,
        &library,
        "cool-mod",
        "1.0",
        &[("pak", "CoolMod_P.pak", b"legacy pak bytes")],
    )
    .await;
    common::mods::enable(&db, &target_id, "cool-mod", None, true).await;

    let (first, _) = convert_and_wait(&mut ws, &target_id, "cool-mod").await;
    assert!(first["error"].is_null(), "{first:?}");
    assert_eq!(first["reused"], false);
    assert_eq!(converter.calls.lock().unwrap().len(), 1);

    // Re-pin the profile to the original legacy version, as if the target had
    // never been converted, so the second request resolves the same source
    // version and finds the `+iostore` version already in the library.
    common::mods::enable(
        &db,
        &target_id,
        "cool-mod",
        Some(&original.version.id),
        true,
    )
    .await;

    let (second, _) = convert_and_wait(&mut ws, &target_id, "cool-mod").await;
    assert!(second["error"].is_null(), "{second:?}");
    assert_eq!(second["reused"], true);
    assert_eq!(
        second["converted"],
        json!([]),
        "nothing is converted on a reused version"
    );
    assert_eq!(second["mod_version_id"], first["mod_version_id"]);
    assert_eq!(
        converter.calls.lock().unwrap().len(),
        1,
        "reusing an already-converted version must not call the converter again"
    );

    let entries = ps_db::mod_profiles::mods_of(&db, &format!("{target_id}/default"))
        .await
        .unwrap();
    let entry = entries.iter().find(|e| e.mod_id == "cool-mod").unwrap();
    assert_eq!(
        entry.mod_version_id.as_deref(),
        Some(first["mod_version_id"].as_str().unwrap()),
        "the reused path must re-pin the profile to the +iostore version"
    );

    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_mixed_mod_converts_only_its_legacy_pak() {
    let (server, converter) = start_iostore_server(false).await;
    let mut ws = common::connect(&server).await;
    let db = driver_for(&server).await;
    let library = library_for(&server);
    let root = tempfile::tempdir().unwrap();
    let target_id = ready_wingdk_target(&db, "client-gamepass", root.path()).await;

    common::mods::install_fixture_mod(
        &db,
        &library,
        "mixed-mod",
        "1.0",
        &[
            ("pak", "A_P.pak", b"already iostore pak bytes"),
            ("pak", "A_P.utoc", b"already iostore utoc bytes"),
            ("pak", "A_P.ucas", b"already iostore ucas bytes"),
            ("pak", "B_P.pak", b"legacy pak bytes"),
        ],
    )
    .await;
    common::mods::enable(&db, &target_id, "mixed-mod", None, true).await;

    let (data, _) = convert_and_wait(&mut ws, &target_id, "mixed-mod").await;
    assert!(data["error"].is_null(), "{data:?}");
    assert_eq!(data["converted"], json!(["B_P.pak"]));

    {
        let calls = converter.calls.lock().unwrap();
        assert_eq!(calls.len(), 1, "{calls:?}");
        assert_eq!(calls[0].file_name().unwrap().to_str().unwrap(), "B_P.pak");
    }

    let new_version_id = data["mod_version_id"].as_str().unwrap().to_string();
    let new_version = ps_db::mod_library::get_version(&db, &new_version_id)
        .await
        .unwrap()
        .unwrap();
    let version_dir = Path::new(&new_version.library_dir);
    assert_eq!(
        std::fs::read(version_dir.join("pak/A_P.pak")).unwrap(),
        b"already iostore pak bytes"
    );
    assert_eq!(
        std::fs::read(version_dir.join("pak/A_P.utoc")).unwrap(),
        b"already iostore utoc bytes"
    );
    assert_eq!(
        std::fs::read(version_dir.join("pak/A_P.ucas")).unwrap(),
        b"already iostore ucas bytes"
    );
    assert_eq!(
        std::fs::read(version_dir.join("pak/B_P.pak")).unwrap(),
        b"converted pak bytes"
    );
    assert_eq!(
        std::fs::read(version_dir.join("pak/B_P.utoc")).unwrap(),
        b"utoc stub"
    );
    assert_eq!(
        std::fs::read(version_dir.join("pak/B_P.ucas")).unwrap(),
        b"ucas stub"
    );

    let manifest: ps_core::mods::InstallManifest =
        serde_json::from_str(&new_version.manifest).unwrap();
    let mut seen = std::collections::HashSet::new();
    for route in &manifest.routes {
        assert!(
            seen.insert((route.kind, route.rel_path.clone())),
            "duplicate route {route:?}"
        );
    }
    assert_eq!(manifest.routes.len(), 6, "{:?}", manifest.routes);

    server.handle.shutdown().await;
}

#[tokio::test]
async fn an_orphan_ucas_does_not_duplicate_the_converted_route() {
    let (server, converter) = start_iostore_server(false).await;
    let mut ws = common::connect(&server).await;
    let db = driver_for(&server).await;
    let library = library_for(&server);
    let root = tempfile::tempdir().unwrap();
    let target_id = ready_wingdk_target(&db, "client-gamepass", root.path()).await;

    common::mods::install_fixture_mod(
        &db,
        &library,
        "orphan-mod",
        "1.0",
        &[
            ("pak", "X_P.pak", b"legacy pak bytes"),
            ("pak", "X_P.ucas", b"orphan ucas bytes"),
        ],
    )
    .await;
    common::mods::enable(&db, &target_id, "orphan-mod", None, true).await;

    let (data, _) = convert_and_wait(&mut ws, &target_id, "orphan-mod").await;
    assert!(data["error"].is_null(), "{data:?}");
    assert_eq!(converter.calls.lock().unwrap().len(), 1);

    let new_version_id = data["mod_version_id"].as_str().unwrap().to_string();
    let new_version = ps_db::mod_library::get_version(&db, &new_version_id)
        .await
        .unwrap()
        .unwrap();

    let manifest: ps_core::mods::InstallManifest =
        serde_json::from_str(&new_version.manifest).unwrap();
    let mut seen = std::collections::HashSet::new();
    for route in &manifest.routes {
        assert!(
            seen.insert((route.kind, route.rel_path.clone())),
            "duplicate route {route:?}"
        );
    }
    assert_eq!(manifest.routes.len(), 3, "{:?}", manifest.routes);

    let version_dir = Path::new(&new_version.library_dir);
    assert_eq!(
        std::fs::read(version_dir.join("pak/X_P.ucas")).unwrap(),
        b"ucas stub",
        "the fresh converter output must win over the orphan carried-through file"
    );

    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_win64_target_is_not_supported() {
    let (server, _converter) = start_iostore_server(false).await;
    let mut ws = common::connect(&server).await;
    let db = driver_for(&server).await;
    let library = library_for(&server);
    let root = tempfile::tempdir().unwrap();
    let target = common::mods::client_target(&db, root.path()).await;
    common::mods::default_profile(&db, &target.id).await;

    common::mods::install_fixture_mod(
        &db,
        &library,
        "cool-mod",
        "1.0",
        &[("pak", "CoolMod_P.pak", b"legacy pak bytes")],
    )
    .await;
    common::mods::enable(&db, &target.id, "cool-mod", None, true).await;

    let data = refusal(&mut ws, &target.id, "cool-mod").await;
    assert_eq!(data["error"]["code"], "not_supported_on_target", "{data:?}");

    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_mod_absent_from_the_profile_is_refused() {
    let (server, _converter) = start_iostore_server(false).await;
    let mut ws = common::connect(&server).await;
    let db = driver_for(&server).await;
    let library = library_for(&server);
    let root = tempfile::tempdir().unwrap();
    let target_id = ready_wingdk_target(&db, "client-gamepass", root.path()).await;

    common::mods::install_fixture_mod(
        &db,
        &library,
        "cool-mod",
        "1.0",
        &[("pak", "CoolMod_P.pak", b"legacy pak bytes")],
    )
    .await;

    let data = refusal(&mut ws, &target_id, "cool-mod").await;
    assert_eq!(data["error"]["code"], "mod_not_in_profile", "{data:?}");

    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_ue4ss_mod_is_not_convertible() {
    let (server, _converter) = start_iostore_server(false).await;
    let mut ws = common::connect(&server).await;
    let db = driver_for(&server).await;
    let library = library_for(&server);
    let root = tempfile::tempdir().unwrap();
    let target_id = ready_wingdk_target(&db, "client-gamepass", root.path()).await;

    common::mods::install_fixture_mod(
        &db,
        &library,
        "cool-mod",
        "1.0",
        &[("ue4ss", "CoolMod/Scripts/main.lua", b"print('hi')")],
    )
    .await;
    common::mods::enable(&db, &target_id, "cool-mod", None, true).await;

    let data = refusal(&mut ws, &target_id, "cool-mod").await;
    assert_eq!(data["error"]["code"], "not_convertible", "{data:?}");

    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_version_already_shipping_utoc_is_already_iostore() {
    let (server, _converter) = start_iostore_server(false).await;
    let mut ws = common::connect(&server).await;
    let db = driver_for(&server).await;
    let library = library_for(&server);
    let root = tempfile::tempdir().unwrap();
    let target_id = ready_wingdk_target(&db, "client-gamepass", root.path()).await;

    common::mods::install_fixture_mod(
        &db,
        &library,
        "cool-mod",
        "1.0",
        &[
            ("pak", "Cool_P.pak", b"pak bytes"),
            ("pak", "Cool_P.utoc", b"utoc bytes"),
            ("pak", "Cool_P.ucas", b"ucas bytes"),
        ],
    )
    .await;
    common::mods::enable(&db, &target_id, "cool-mod", None, true).await;

    let data = refusal(&mut ws, &target_id, "cool-mod").await;
    assert_eq!(data["error"]["code"], "already_iostore", "{data:?}");

    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_failing_converter_leaves_no_new_version_and_no_scratch() {
    let (server, _converter) = start_iostore_server(true).await;
    let mut ws = common::connect(&server).await;
    let db = driver_for(&server).await;
    let library = library_for(&server);
    let root = tempfile::tempdir().unwrap();
    let target_id = ready_wingdk_target(&db, "client-gamepass", root.path()).await;

    common::mods::install_fixture_mod(
        &db,
        &library,
        "cool-mod",
        "1.0",
        &[("pak", "CoolMod_P.pak", b"legacy pak bytes")],
    )
    .await;
    common::mods::enable(&db, &target_id, "cool-mod", None, true).await;

    let data = refusal(&mut ws, &target_id, "cool-mod").await;
    assert_eq!(data["error"]["code"], "conversion_failed", "{data:?}");

    let new_version = ps_db::mod_library::get_version(&db, "cool-mod@1.0+iostore")
        .await
        .unwrap();
    assert!(
        new_version.is_none(),
        "a failed conversion must not store a version"
    );

    let scratch = scratch_root(&library);
    let remaining = if scratch.exists() {
        std::fs::read_dir(&scratch).unwrap().count()
    } else {
        0
    };
    assert_eq!(
        remaining, 0,
        "a failed conversion must leave no scratch behind"
    );

    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_legacy_pak_round_trips_through_conflicts_and_convert() {
    let (server, _converter) = start_iostore_server(false).await;
    let mut ws = common::connect(&server).await;
    let db = driver_for(&server).await;
    let library = library_for(&server);
    let root = tempfile::tempdir().unwrap();
    let target_id = ready_wingdk_target(&db, "client-gamepass", root.path()).await;

    common::mods::install_fixture_mod(
        &db,
        &library,
        "cool-mod",
        "1.0",
        &[("pak", "CoolMod_P.pak", b"legacy pak bytes")],
    )
    .await;
    common::mods::enable(&db, &target_id, "cool-mod", None, true).await;

    let before = conflicts(&mut ws, &target_id).await;
    assert!(before["error"].is_null(), "{before:?}");
    assert_eq!(
        conflicts_of_kind(&before, "gamepass_pak_incompatible").len(),
        1,
        "{before:?}"
    );

    let (data, _) = convert_and_wait(&mut ws, &target_id, "cool-mod").await;
    assert!(data["error"].is_null(), "{data:?}");

    let after = conflicts(&mut ws, &target_id).await;
    assert!(after["error"].is_null(), "{after:?}");
    assert!(
        conflicts_of_kind(&after, "gamepass_pak_incompatible").is_empty(),
        "{after:?}"
    );
    let unreadable = after["unreadable"].as_array().unwrap();
    assert_eq!(unreadable.len(), 1, "{unreadable:?}");
    assert_eq!(unreadable[0]["reason"], "iostore");
    assert_eq!(unreadable[0]["file"], "CoolMod_P.pak");

    let current = ps_db::mod_library::current_version(&db, "cool-mod")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        current.version, "1.0",
        "the legacy version must stay current"
    );

    server.handle.shutdown().await;
}

#[tokio::test]
async fn a_game_tree_archive_pak_round_trips_through_conflicts_and_convert() {
    let (server, _converter) = start_iostore_server(false).await;
    let mut ws = common::connect(&server).await;
    let db = driver_for(&server).await;
    let library = library_for(&server);
    let root = tempfile::tempdir().unwrap();
    let target_id = ready_wingdk_target(&db, "client-gamepass", root.path()).await;

    common::mods::install_fixture_mod(
        &db,
        &library,
        "gametree-mod",
        "1.0",
        &[(
            "passthrough",
            "Pal/Content/Paks/~mods/X_P.pak",
            b"legacy pak bytes",
        )],
    )
    .await;
    common::mods::enable(&db, &target_id, "gametree-mod", None, true).await;

    let before = conflicts(&mut ws, &target_id).await;
    assert!(before["error"].is_null(), "{before:?}");
    assert_eq!(
        conflicts_of_kind(&before, "gamepass_pak_incompatible").len(),
        1,
        "{before:?}"
    );

    let (data, _) = convert_and_wait(&mut ws, &target_id, "gametree-mod").await;
    assert!(data["error"].is_null(), "{data:?}");
    assert_eq!(data["converted"], json!(["Pal/Content/Paks/~mods/X_P.pak"]));

    let after = conflicts(&mut ws, &target_id).await;
    assert!(after["error"].is_null(), "{after:?}");
    assert!(
        conflicts_of_kind(&after, "gamepass_pak_incompatible").is_empty(),
        "{after:?}"
    );
    let unreadable = after["unreadable"].as_array().unwrap();
    assert_eq!(unreadable.len(), 1, "{unreadable:?}");
    assert_eq!(unreadable[0]["reason"], "iostore");
    assert_eq!(unreadable[0]["file"], "X_P.pak");

    let mods_dir = root.path().join("Pal/Content/Paks/~mods");
    assert!(mods_dir.join("X_P.pak").is_file());
    assert!(mods_dir.join("X_P.utoc").is_file());
    assert!(mods_dir.join("X_P.ucas").is_file());

    server.handle.shutdown().await;
}
