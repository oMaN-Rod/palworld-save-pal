//! End-to-end reproduction: `update_save_file` (inventory edit) ->
//! `save_modded_save` -> fresh `select_save`, against a temp copy of the
//! committed `world1` fixture, asserting the edit survives the backend's own
//! save->reload cycle. `settings.save_dir` is pointed at the temp copy first
//! so the Steam write's LevelMeta/Players output lands there, not on the real
//! machine's default Steam dir.

mod common;

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const WORLD1_PLAYER_O: &str = "8c2f1930-0000-0000-0000-000000000000";

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

/// Like `phase2_ws.rs`'s `recv_until_type_or_error`: reads frames until one
/// whose `type` equals `stop_type`, panicking loudly (dumping the payload)
/// if an `error` frame arrives first instead of hanging until the receive
/// timeout.
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

async fn select_save(socket: &mut common::WsClient, level_sav_path: &str) {
    common::send_json(
        socket,
        json!({"type": "select_save",
               "data": {"type": "steam", "path": level_sav_path, "local": true}}),
    )
    .await;
    recv_until(socket, "get_guild_summaries").await;
}

async fn load_player(socket: &mut common::WsClient, player_id: &str) -> Value {
    common::send_json(
        socket,
        json!({"type": "request_player_details",
               "data": {"player_id": player_id, "origin": "edit"}}),
    )
    .await;
    let frames = recv_until(socket, "get_player_details_response").await;
    frames.last().unwrap()["data"]["player"].clone()
}

#[tokio::test]
async fn item_edit_survives_update_save_modded_save_and_reload() {
    // 1. Copy the committed world1 fixture to a temp dir.
    let temp_root = tempfile::tempdir().unwrap();
    let world1_copy = temp_root.path().join("world1");
    copy_dir_recursive(
        &repo_root().join("tests/fixtures/saves/world1"),
        &world1_copy,
    );
    let level_sav_path = world1_copy.join("Level.sav").to_string_lossy().into_owned();

    // 2. Start a test server and point settings.save_dir at the temp copy so
    // the Steam write path (LevelMeta.sav + Players/*.sav) targets it instead
    // of the machine's real default Steam save dir.
    let server = common::start_test_server().await;
    let db_path = server._temp_dir.path().join("psp-rs.db");
    let db = psp_db::open(&db_path).await.expect("open test db");
    psp_db::settings::update_save_dir(&db, &world1_copy.to_string_lossy())
        .await
        .expect("set save_dir to temp world1 copy");

    let mut socket = common::connect(&server).await;

    // 3. Load the temp save.
    select_save(&mut socket, &level_sav_path).await;

    // 4. Load the world1 player and add a new inventory slot to their common
    // container -- mirrors psp-core's
    // `update_players_common_container_edit_ignores_forged_dto_id`, which
    // proves this exact edit shape applies correctly in-memory.
    let mut player = load_player(&mut socket, WORLD1_PLAYER_O).await;
    let common_container = player
        .get_mut("common_container")
        .expect("player has a common_container")
        .as_object_mut()
        .expect("common_container is an object");
    let slots = common_container
        .get_mut("slots")
        .expect("common_container has slots")
        .as_array_mut()
        .expect("slots is an array");
    slots.push(json!({
        "dynamic_item": null,
        "slot_index": 9000,
        "count": 3,
        "static_id": "Wood",
        "local_id": null,
    }));

    // 5. update_save_file — the exact WS message the frontend's saveState()
    // sends before writeSave's save_modded_save call.
    common::send_json(
        &mut socket,
        json!({"type": "update_save_file",
               "data": {"modified_players": {WORLD1_PLAYER_O: player}}}),
    )
    .await;
    let update_frames = recv_until(&mut socket, "update_save_file").await;
    assert_eq!(update_frames.last().unwrap()["data"], "Changes saved");

    // 6. save_modded_save — Steam branch, data: null (matches writeSave's
    // saveOperations.svelte.ts).
    common::send_json(
        &mut socket,
        json!({"type": "save_modded_save", "data": null}),
    )
    .await;
    let save_frames = recv_until(&mut socket, "save_modded_save").await;
    assert_eq!(
        save_frames.last().unwrap()["data"],
        "Modded save file saved successfully"
    );

    // 7. RE-LOAD fresh from disk (same path) and check the edit survived the
    // full write+reload cycle.
    select_save(&mut socket, &level_sav_path).await;
    let reloaded_player = load_player(&mut socket, WORLD1_PLAYER_O).await;
    let reloaded_slots = reloaded_player["common_container"]["slots"]
        .as_array()
        .expect("reloaded common_container has slots");
    let added = reloaded_slots
        .iter()
        .find(|slot| slot["slot_index"] == 9000);
    assert!(
        added.is_some(),
        "the added Wood slot must survive update_save_file -> save_modded_save -> reload \
         from disk, got slots: {reloaded_slots:?}"
    );
    let added = added.unwrap();
    assert_eq!(added["static_id"], "Wood");
    assert_eq!(added["count"], 3);

    server.handle.shutdown().await;
}
