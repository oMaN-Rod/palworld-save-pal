//! An inventory edit must survive `update_save_file` -> `save_modded_save` ->
//! a fresh `select_save` from disk. Runs against a temp copy of the committed
//! `world1` fixture, with `settings.save_dir` pointed at that copy so the write
//! lands there rather than in the machine's real Steam save directory.

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

/// Reads frames until one whose `type` equals `stop_type`, panicking with the
/// payload if an `error` arrives first rather than hanging until the timeout.
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
    let temp_root = tempfile::tempdir().unwrap();
    let world1_copy = temp_root.path().join("world1");
    copy_dir_recursive(
        &repo_root().join("tests/fixtures/saves/world1"),
        &world1_copy,
    );
    let level_sav_path = world1_copy.join("Level.sav").to_string_lossy().into_owned();

    // settings.save_dir decides where the Steam write path puts LevelMeta.sav
    // and Players/*.sav; point it at the temp copy, never the machine's real
    // default save dir.
    let server = common::start_test_server().await;
    let db_path = server._temp_dir.path().join("psp-rs.db");
    let db = psp_db::open(&db_path).await.expect("open test db");
    psp_db::settings::update_save_dir(&db, &world1_copy.to_string_lossy())
        .await
        .expect("set save_dir to temp world1 copy");

    let mut socket = common::connect(&server).await;

    select_save(&mut socket, &level_sav_path).await;

    // Add a new inventory slot to the player's common container.
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

    // update_save_file then save_modded_save (with `data: null`) is exactly the
    // pair the frontend sends when saving a Steam save.
    common::send_json(
        &mut socket,
        json!({"type": "update_save_file",
               "data": {"modified_players": {WORLD1_PLAYER_O: player}}}),
    )
    .await;
    let update_frames = recv_until(&mut socket, "update_save_file").await;
    assert_eq!(update_frames.last().unwrap()["data"], "Changes saved");

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

    // Re-load fresh from the same path on disk: the edit must still be there.
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

/// GUID for WatchTower_11 (`BP_LevelObject_UnlockMapPoint_C`) from
/// `data/json/fast_travel_points.json`. Unlocking it writes into the save's
/// `FastTravelPointUnlockFlag` map; this test proves that flag survives a
/// full write -> reload cycle via `unlocked_fast_travel_points`.
const WATCHTOWER_11_GUID: &str = "0C0AF9F34C0491BCAD80B1BF355B9A98";

#[tokio::test]
async fn watchtower_unlock_survives_update_save_modded_save_and_reload() {
    let temp_root = tempfile::tempdir().unwrap();
    let world1_copy = temp_root.path().join("world1");
    copy_dir_recursive(
        &repo_root().join("tests/fixtures/saves/world1"),
        &world1_copy,
    );
    let level_sav_path = world1_copy.join("Level.sav").to_string_lossy().into_owned();

    let server = common::start_test_server().await;
    let db_path = server._temp_dir.path().join("psp-rs.db");
    let db = psp_db::open(&db_path).await.expect("open test db");
    psp_db::settings::update_save_dir(&db, &world1_copy.to_string_lossy())
        .await
        .expect("set save_dir to temp world1 copy");

    let mut socket = common::connect(&server).await;

    select_save(&mut socket, &level_sav_path).await;

    let mut player = load_player(&mut socket, WORLD1_PLAYER_O).await;
    let existing: Vec<String> = player["unlocked_fast_travel_points"]
        .as_array()
        .map(|points| {
            points
                .iter()
                .map(|point| point.as_str().unwrap_or_default().to_string())
                .collect()
        })
        .unwrap_or_default();
    assert!(
        !existing
            .iter()
            .any(|point| point.eq_ignore_ascii_case(WATCHTOWER_11_GUID)),
        "sanity check failed: world1 fixture player already has watchtower unlocked: {existing:?}"
    );

    let player_object = player.as_object_mut().expect("player is an object");
    let points = player_object
        .entry("unlocked_fast_travel_points")
        .or_insert_with(|| json!([]));
    if points.is_null() {
        *points = json!([]);
    }
    points
        .as_array_mut()
        .expect("unlocked_fast_travel_points is an array")
        .push(json!(WATCHTOWER_11_GUID));

    common::send_json(
        &mut socket,
        json!({"type": "update_save_file",
               "data": {"modified_players": {WORLD1_PLAYER_O: player}}}),
    )
    .await;
    let update_frames = recv_until(&mut socket, "update_save_file").await;
    assert_eq!(update_frames.last().unwrap()["data"], "Changes saved");

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

    // Re-load fresh from the same path on disk: the unlocked watchtower must
    // still be there, proving it round-tripped through FastTravelPointUnlockFlag.
    select_save(&mut socket, &level_sav_path).await;
    let reloaded_player = load_player(&mut socket, WORLD1_PLAYER_O).await;
    let reloaded_points = reloaded_player["unlocked_fast_travel_points"]
        .as_array()
        .expect("reloaded player has unlocked_fast_travel_points");
    let found = reloaded_points.iter().any(|point| {
        point
            .as_str()
            .map(|s| s.to_ascii_uppercase() == WATCHTOWER_11_GUID.to_ascii_uppercase())
            .unwrap_or(false)
    });
    assert!(
        found,
        "watchtower GUID {WATCHTOWER_11_GUID} must survive update_save_file -> \
         save_modded_save -> reload from disk, got points: {reloaded_points:?}"
    );

    server.handle.shutdown().await;
}
