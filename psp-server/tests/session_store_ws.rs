//! A `select_save` load registers its session in `AppState::sessions` and
//! returns the id in `loaded_save_files`; two connections loading two saves
//! stay isolated in distinct store entries.

mod common;

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use uuid::Uuid;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for dir_entry in std::fs::read_dir(src).unwrap() {
        let dir_entry = dir_entry.unwrap();
        let entry_path = dir_entry.path();
        let dest_path = dst.join(dir_entry.file_name());
        if entry_path.is_dir() {
            copy_dir_recursive(&entry_path, &dest_path);
        } else {
            std::fs::copy(&entry_path, &dest_path).unwrap();
        }
    }
}

fn temp_world1_level_sav() -> (tempfile::TempDir, String) {
    let temp_root = tempfile::tempdir().unwrap();
    let world1_copy = temp_root.path().join("world1");
    copy_dir_recursive(
        &repo_root().join("tests/fixtures/saves/world1"),
        &world1_copy,
    );
    let level_sav_path = world1_copy.join("Level.sav").to_string_lossy().into_owned();
    (temp_root, level_sav_path)
}

/// Drives a `select_save` and returns the `session_id` from its
/// `loaded_save_files` frame.
async fn select_save_get_session_id(socket: &mut common::WsClient, level_sav_path: &str) -> String {
    common::send_json(
        socket,
        json!({"type": "select_save",
               "data": {"type": "steam", "path": level_sav_path, "local": true}}),
    )
    .await;
    let mut loaded: Option<Value> = None;
    loop {
        let frame = common::next_json(socket).await;
        let message_type = frame["type"].as_str().unwrap_or_default().to_string();
        if message_type == "error" {
            panic!("unexpected error frame during select_save: {frame}");
        }
        if message_type == "loaded_save_files" {
            loaded = Some(frame.clone());
        }
        if message_type == "get_guild_summaries" {
            break;
        }
    }
    loaded.expect("select_save emits loaded_save_files")["data"]["session_id"]
        .as_str()
        .expect("loaded_save_files carries a session_id string")
        .to_string()
}

#[tokio::test]
async fn load_registers_session_and_returns_findable_id() {
    let (_temp, level_sav_path) = temp_world1_level_sav();
    let server = common::start_test_server().await;
    let mut socket = common::connect(&server).await;

    let session_id = select_save_get_session_id(&mut socket, &level_sav_path).await;
    let parsed = Uuid::parse_str(&session_id).expect("session_id is a uuid");

    // Scope the std map lock so it's dropped before the per-session await below.
    let stored = {
        let store = server.handle.app.sessions.lock().unwrap();
        store
            .get(&parsed)
            .expect("loaded session is registered under the returned id")
    };
    assert!(
        stored.lock().await.save.is_some(),
        "the registered session holds the parsed save"
    );

    server.handle.shutdown().await;
}

#[tokio::test]
async fn two_connections_get_distinct_ids_and_entries() {
    let (_temp_a, level_sav_a) = temp_world1_level_sav();
    let (_temp_b, level_sav_b) = temp_world1_level_sav();
    let server = common::start_test_server().await;

    let mut socket_a = common::connect(&server).await;
    let mut socket_b = common::connect(&server).await;

    let id_a = select_save_get_session_id(&mut socket_a, &level_sav_a).await;
    let id_b = select_save_get_session_id(&mut socket_b, &level_sav_b).await;
    assert_ne!(id_a, id_b, "two loads produce distinct session ids");

    {
        let store = server.handle.app.sessions.lock().unwrap();
        let entry_a = store.get(&Uuid::parse_str(&id_a).unwrap());
        let entry_b = store.get(&Uuid::parse_str(&id_b).unwrap());
        assert!(entry_a.is_some() && entry_b.is_some(), "both ids present");
        assert!(
            !std::sync::Arc::ptr_eq(&entry_a.unwrap(), &entry_b.unwrap()),
            "the two connections' sessions are distinct store entries"
        );
        assert_eq!(store.len(), 2, "exactly two registered sessions");
    }

    server.handle.shutdown().await;
}
