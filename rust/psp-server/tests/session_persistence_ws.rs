//! WS-level tests for session persistence (SP-T2): reattach_session /
//! eject_session against a temp copy of the committed `world1` fixture.

mod common;

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const WORLD1_PLAYER_O: &str = "8c2f1930-0000-0000-0000-000000000000";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
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

/// Reads frames until one whose `type` equals `stop_type`, collecting them and
/// panicking loudly if an unexpected `error` frame arrives first.
async fn recv_until(socket: &mut common::WsClient, stop_type: &str) -> Vec<Value> {
    let mut frames = Vec::new();
    loop {
        let frame = common::next_json(socket).await;
        let message_type = frame["type"].as_str().unwrap_or_default().to_string();
        frames.push(frame.clone());
        if message_type == "error" && stop_type != "error" {
            panic!("unexpected error frame while awaiting {stop_type}: {frame}");
        }
        if message_type == stop_type {
            break;
        }
    }
    frames
}

/// Loads world1 on `socket` and returns the store `session_id` from
/// `loaded_save_files`.
async fn select_world1(socket: &mut common::WsClient, level_sav_path: &str) -> String {
    common::send_json(
        socket,
        json!({"type": "select_save",
               "data": {"type": "steam", "path": level_sav_path, "local": true}}),
    )
    .await;
    let frames = recv_until(socket, "get_guild_summaries").await;
    let loaded = frames
        .iter()
        .find(|frame| frame["type"] == "loaded_save_files")
        .expect("loaded_save_files frame");
    loaded["data"]["session_id"]
        .as_str()
        .expect("session_id in loaded_save_files")
        .to_string()
}

/// Copies the committed world1 fixture to a temp dir and returns (temp handle,
/// Level.sav path).
fn temp_world1() -> (tempfile::TempDir, String) {
    let temp_root = tempfile::tempdir().unwrap();
    let world1_copy = temp_root.path().join("world1");
    copy_dir_recursive(
        &repo_root().join("tests/fixtures/saves/world1"),
        &world1_copy,
    );
    let level_sav_path = world1_copy.join("Level.sav").to_string_lossy().into_owned();
    (temp_root, level_sav_path)
}

#[tokio::test]
async fn reattach_on_second_connection_restores_and_works() {
    let (_temp_root, level_sav_path) = temp_world1();
    let server = common::start_test_server().await;

    // Connection 1 loads the save and registers a session.
    let mut socket1 = common::connect(&server).await;
    let session_id = select_world1(&mut socket1, &level_sav_path).await;

    // Connection 2 reattaches by id → gets the whole load overview.
    let mut socket2 = common::connect(&server).await;
    common::send_json(
        &mut socket2,
        json!({"type": "reattach_session", "data": {"session_id": session_id}}),
    )
    .await;
    let frames = recv_until(&mut socket2, "get_guild_summaries").await;
    let loaded = frames
        .iter()
        .find(|frame| frame["type"] == "loaded_save_files")
        .expect("loaded_save_files frame on reattach");
    assert_eq!(loaded["data"]["session_id"], session_id);
    assert!(frames.iter().any(|f| f["type"] == "get_player_summaries"));
    assert!(frames.iter().any(|f| f["type"] == "get_guild_summaries"));

    // A subsequent op on connection 2 works against the reattached session.
    common::send_json(
        &mut socket2,
        json!({"type": "request_player_details",
               "data": {"player_id": WORLD1_PLAYER_O, "origin": "edit"}}),
    )
    .await;
    let detail_frames = recv_until(&mut socket2, "get_player_details_response").await;
    let player = &detail_frames.last().unwrap()["data"]["player"];
    assert_eq!(player["uid"], WORLD1_PLAYER_O);

    server.handle.shutdown().await;
}

#[tokio::test]
async fn reattach_unknown_id_replies_session_not_found() {
    let server = common::start_test_server().await;
    let mut socket = common::connect(&server).await;

    let bogus_id = "00000000-0000-0000-0000-000000000000";
    common::send_json(
        &mut socket,
        json!({"type": "reattach_session", "data": {"session_id": bogus_id}}),
    )
    .await;
    let frame = common::next_json(&mut socket).await;
    assert_eq!(frame["type"], "session_not_found");
    assert_eq!(frame["data"], bogus_id);

    server.handle.shutdown().await;
}

#[tokio::test]
async fn reattach_same_id_does_not_deadlock() {
    let (_temp_root, level_sav_path) = temp_world1();
    let server = common::start_test_server().await;
    let mut socket = common::connect(&server).await;

    // The connection is already attached to this id; reattaching it must read
    // the overview from the held guard, NOT re-lock the same mutex (deadlock).
    let session_id = select_world1(&mut socket, &level_sav_path).await;
    common::send_json(
        &mut socket,
        json!({"type": "reattach_session", "data": {"session_id": session_id}}),
    )
    .await;
    // If the handler re-locked its own arc this would hang until the 30s recv
    // timeout and fail; reaching get_guild_summaries proves it did not.
    let frames = recv_until(&mut socket, "get_guild_summaries").await;
    let loaded = frames
        .iter()
        .find(|frame| frame["type"] == "loaded_save_files")
        .expect("loaded_save_files frame on same-id reattach");
    assert_eq!(loaded["data"]["session_id"], session_id);

    server.handle.shutdown().await;
}

#[tokio::test]
async fn mutual_reattach_across_connections_does_not_deadlock() {
    // Two connections, each attached to its own session (X and Y). They then
    // reattach to EACH OTHER's id at the same time. If a handler held its own
    // per-session guard while locking the target's (same lock order on both
    // tasks: own-arc → target-arc), this forms a cycle and deadlocks forever.
    // The fix locks at most one per-session arc, so both complete.
    let (_temp_x, level_x) = temp_world1();
    let (_temp_y, level_y) = temp_world1();
    let server = common::start_test_server().await;

    let mut socket_x = common::connect(&server).await;
    let mut socket_y = common::connect(&server).await;
    let session_x = select_world1(&mut socket_x, &level_x).await;
    let session_y = select_world1(&mut socket_y, &level_y).await;

    // Concurrently: the X-attached connection reattaches Y, and vice versa.
    let reattach_x_to_y = async {
        common::send_json(
            &mut socket_x,
            json!({"type": "reattach_session", "data": {"session_id": session_y}}),
        )
        .await;
        recv_until(&mut socket_x, "get_guild_summaries").await
    };
    let reattach_y_to_x = async {
        common::send_json(
            &mut socket_y,
            json!({"type": "reattach_session", "data": {"session_id": session_x}}),
        )
        .await;
        recv_until(&mut socket_y, "get_guild_summaries").await
    };

    // Bound the whole exchange: the old own-arc→target-arc code hangs here.
    let (frames_x, frames_y) = tokio::time::timeout(std::time::Duration::from_secs(15), async {
        tokio::join!(reattach_x_to_y, reattach_y_to_x)
    })
    .await
    .expect("mutual reattach deadlocked");

    let loaded_x = frames_x
        .iter()
        .find(|frame| frame["type"] == "loaded_save_files")
        .expect("loaded_save_files on X→Y reattach");
    assert_eq!(loaded_x["data"]["session_id"], session_y);
    let loaded_y = frames_y
        .iter()
        .find(|frame| frame["type"] == "loaded_save_files")
        .expect("loaded_save_files on Y→X reattach");
    assert_eq!(loaded_y["data"]["session_id"], session_x);

    server.handle.shutdown().await;
}

#[tokio::test]
async fn eject_removes_from_store_and_clears_connection() {
    let (_temp_root, level_sav_path) = temp_world1();
    let server = common::start_test_server().await;
    let mut socket = common::connect(&server).await;

    let session_id = select_world1(&mut socket, &level_sav_path).await;

    // Eject → confirmation with the id.
    common::send_json(
        &mut socket,
        json!({"type": "eject_session", "data": {"session_id": session_id}}),
    )
    .await;
    let frame = common::next_json(&mut socket).await;
    assert_eq!(frame["type"], "eject_session");
    assert_eq!(frame["data"], session_id);

    // The id is gone from the store: a later reattach → session_not_found.
    common::send_json(
        &mut socket,
        json!({"type": "reattach_session", "data": {"session_id": session_id}}),
    )
    .await;
    let frame = common::next_json(&mut socket).await;
    assert_eq!(frame["type"], "session_not_found");
    assert_eq!(frame["data"], session_id);

    // The connection's session is cleared: save_modded_save now errors.
    common::send_json(
        &mut socket,
        json!({"type": "save_modded_save", "data": null}),
    )
    .await;
    let frame = common::next_json(&mut socket).await;
    assert_eq!(frame["type"], "error");
    assert_eq!(frame["data"]["message"], "No save file loaded");

    server.handle.shutdown().await;
}
