mod common;

use std::sync::Arc;

use psp_server::messages::MessageType;
use psp_server::signal::framing::{AssemblerOutcome, ChunkAssembler};
use psp_server::signal::remote_ctl;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

const CTL_CAPACITY: usize = 16;

fn spawn_bridge(
    app: Arc<psp_server::AppState>,
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

    ctl_in_tx.send(json!({"type":"list_servers"})).await.unwrap();

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

    impl psp_server::desktop_dialogs::FileDialogProvider for PanicIfCalledDialogProvider {
        fn pick_file(
            &self,
            _request: psp_server::desktop_dialogs::FileDialogRequest,
        ) -> psp_server::desktop_dialogs::DialogFuture {
            panic!("pick_file must not be invoked for a remote select_save call");
        }
        fn save_file(
            &self,
            _request: psp_server::desktop_dialogs::FileSaveRequest,
        ) -> psp_server::desktop_dialogs::DialogFuture {
            panic!("save_file must not be invoked for a remote select_save call");
        }
        fn pick_folder(
            &self,
            _initial_directory: Option<std::path::PathBuf>,
        ) -> psp_server::desktop_dialogs::DialogFuture {
            panic!("pick_folder must not be invoked for a remote select_save call");
        }
        fn pick_files(
            &self,
            _request: psp_server::desktop_dialogs::FileDialogRequest,
        ) -> psp_server::desktop_dialogs::DialogFilesFuture {
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
    let server =
        common::start_desktop_test_server(std::sync::Arc::new(
            psp_server::desktop_dialogs::NullDialogProvider,
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
async fn install_server_mod_is_refused_and_writes_nothing() {
    let server = common::start_test_server().await;

    let scratch = tempfile::tempdir().unwrap();
    let mods_path = scratch.path().join("mods");
    std::fs::create_dir_all(&mods_path).unwrap();

    let new_server = psp_db::servers::NewServer {
        name: "Refusal Test".to_string(),
        container_name: "refusal-test".to_string(),
        image_name: "omanrod/psp-palworld-server".to_string(),
        server_type: "docker".to_string(),
        game_port: 8211,
        query_port: 27015,
        rest_api_port: 8212,
        data_volume_name: "psp-refusal-test-data".to_string(),
        mods_path: mods_path.to_string_lossy().into_owned(),
        admin_password: "admin".to_string(),
        server_name: "PSP Palworld Server".to_string(),
        ..Default::default()
    };
    let record = psp_db::servers::create_server(&*server.handle.app.driver, new_server)
        .await
        .unwrap();

    let (ctl_in_tx, mut ctl_out_rx, cancel) = spawn_bridge(server.handle.app.clone());

    ctl_in_tx
        .send(json!({
            "type": "install_server_mod",
            "data": {
                "server_id": record.id,
                "mod_name": "Sneaky",
                "mod_data": base64_zip_fixture(),
                "mod_type": "ue4ss",
            }
        }))
        .await
        .unwrap();

    let frame = next_frame(&mut ctl_out_rx).await;
    assert_eq!(frame["type"], "install_server_mod");
    let error = frame["data"]["error"]
        .as_str()
        .expect("denylisted reply must carry data.error");
    assert!(
        error.to_lowercase().contains("remote"),
        "expected the refusal to mention remote unavailability, got: {error}"
    );
    assert!(
        std::fs::read_dir(&mods_path).unwrap().next().is_none(),
        "install_server_mod must not touch the filesystem when refused"
    );

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

    let installed = psp_db::plugins::get_all(&*server.handle.app.driver)
        .await
        .unwrap();
    assert!(
        installed.iter().all(|row| row.id != "sneaky"),
        "install_plugin must not install anything when refused"
    );

    cancel.cancel();
    server.handle.shutdown().await;
}

fn base64_lua_fixture() -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(b"function main() end")
}

fn base64_zip_fixture() -> String {
    use base64::Engine;
    let empty_zip: &[u8] = &[
        0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    base64::engine::general_purpose::STANDARD.encode(empty_zip)
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

    assert!(pieces.len() > 1, "a >64 KiB frame must split into more than one piece");

    let mut assembler = ChunkAssembler::default();
    let mut complete = None;
    for piece in &pieces {
        assert!(piece.len() <= 64 * 1024, "chunk {} bytes exceeds the cap", piece.len());
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
