//! Phase 4 WS integration tests (Task 11: gamepass scan/delete/rename).
//! `PSP_GAMEPASS_PACKAGES_ROOT` and `PSP_BACKUPS_ROOT` are process-global env
//! vars, so every assertion that depends on them lives in the single
//! `gamepass_scan_and_management_flow` test below to avoid cross-test races
//! (cargo runs tests in the same process, in parallel threads by default).

mod common;

/// Serializes the two tests below, which both mutate the PROCESS-GLOBAL env
/// vars `PSP_GAMEPASS_PACKAGES_ROOT` / `PSP_BACKUPS_ROOT` (cargo runs a test
/// binary's tests in parallel threads, so without this one test could clear an
/// env var mid-flight of the other and its gamepass backup could land under the
/// process CWD's real `backups/`). A `tokio::sync::Mutex` (not `std`) so the
/// guard can be held across the tests' `.await` points without tripping
/// `clippy::await_holding_lock`; the async mutex also releases cleanly if a
/// test panics while holding it, so there is no poison to recover from.
static GAMEPASS_ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Repo root, resolved from this crate's manifest dir (mirrors
/// `phase2_ws.rs`'s `repo_root()`): `rust/psp-server/../..`.
fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// Collects WS frames until (and including) the first whose `type` equals
/// `target_type`. Used to gather the `progress_message`* burst a load or
/// modded-save emits before its terminal response frame.
async fn recv_until(ws: &mut common::WsClient, target_type: &str) -> Vec<serde_json::Value> {
    let mut frames = Vec::new();
    loop {
        let frame = common::next_json(ws).await;
        let reached_target = frame["type"] == target_type;
        frames.push(frame);
        if reached_target {
            return frames;
        }
    }
}

#[tokio::test]
async fn gamepass_scan_and_management_flow() {
    let _env_guard = GAMEPASS_ENV_LOCK.lock().await;
    // Seed the synthetic wgs tree with the COMMITTED, always-present
    // `tests/fixtures/saves/world1/LevelMeta.sav` (a real PlM1/Oodle save
    // that `savio::read_sav_bytes` + `scan` parse for the world name) rather
    // than the `PSP_PY_TESTDATA`-gated fixture — so this test RUNS
    // unconditionally in a fresh clone / CI, exercising the brand-new
    // scan/delete/rename WS handlers with real assertions. (The gamepass
    // `backups/gamepass/` corpus is gitignored/local-only and is NOT a
    // CI-safe fixture source.)
    let meta_bytes =
        std::fs::read(repo_root().join("tests/fixtures/saves/world1/LevelMeta.sav")).unwrap();

    // The world name scan should extract from those committed fixture bytes.
    let expected_world_name =
        psp_core::gamepass::scan::world_name_from_level_meta(&meta_bytes).unwrap();
    assert!(
        !expected_world_name.is_empty(),
        "committed LevelMeta fixture must yield a non-empty world name"
    );

    let gamepass_root = tempfile::tempdir().unwrap();
    let player_id = uuid::Uuid::new_v4();
    let player_hex = player_id.as_simple().to_string().to_uppercase();
    let save_id = "0123456789ABCDEF0123456789ABCDEF";
    let save = psp_core::gamepass::fixture::SyntheticSave {
        save_id: save_id.to_string(),
        level_sav: b"LEVEL-PLACEHOLDER".to_vec(),
        level_meta: Some(meta_bytes),
        local_data: None,
        world_option: None,
        players: vec![psp_core::gamepass::fixture::SyntheticPlayer {
            id: player_id,
            sav: b"P".to_vec(),
            dps: Some(b"D".to_vec()),
        }],
    };

    psp_core::gamepass::fixture::build_wgs_tree(gamepass_root.path(), &[save]).unwrap();
    std::env::set_var("PSP_GAMEPASS_PACKAGES_ROOT", gamepass_root.path());
    std::env::set_var("PSP_BACKUPS_ROOT", gamepass_root.path().join("backups"));

    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;

    // scan
    common::send_json(
        &mut ws,
        serde_json::json!({"type": "scan_gamepass_saves", "data": null}),
    )
    .await;
    let response = common::next_json(&mut ws).await;
    assert_eq!(response["type"], "scan_gamepass_saves");
    assert!(response["data"]["container_path"].is_string());
    assert_eq!(
        response["data"]["saves"][save_id]["world_name"],
        expected_world_name
    );
    assert_eq!(response["data"]["saves"][save_id]["player_count"], 1);

    // rename world
    common::send_json(
        &mut ws,
        serde_json::json!({
            "type": "rename_gamepass_world",
            "data": {"save_id": save_id, "new_name": "Renamed World"}
        }),
    )
    .await;
    let response = common::next_json(&mut ws).await;
    assert_eq!(response["type"], "rename_gamepass_world");
    assert_eq!(
        response["data"]["message"],
        "World renamed to 'Renamed World'"
    );

    common::send_json(
        &mut ws,
        serde_json::json!({"type": "scan_gamepass_saves", "data": null}),
    )
    .await;
    let response = common::next_json(&mut ws).await;
    assert_eq!(
        response["data"]["saves"][save_id]["world_name"],
        "Renamed World"
    );

    // delete player
    common::send_json(
        &mut ws,
        serde_json::json!({
            "type": "delete_gamepass_player",
            "data": {"save_id": save_id, "player_id": player_hex}
        }),
    )
    .await;
    let response = common::next_json(&mut ws).await;
    assert_eq!(response["type"], "delete_gamepass_player");
    assert_eq!(
        response["data"]["message"],
        format!("Deleted player {player_hex}")
    );

    // delete save
    common::send_json(
        &mut ws,
        serde_json::json!({
            "type": "delete_gamepass_save",
            "data": {"save_id": save_id}
        }),
    )
    .await;
    let response = common::next_json(&mut ws).await;
    assert_eq!(response["type"], "delete_gamepass_save");
    assert!(response["data"]["message"]
        .as_str()
        .unwrap()
        .starts_with("Deleted save with "));

    // deleting again reports the Python error payload
    common::send_json(
        &mut ws,
        serde_json::json!({
            "type": "delete_gamepass_save",
            "data": {"save_id": save_id}
        }),
    )
    .await;
    let response = common::next_json(&mut ws).await;
    assert_eq!(
        response["data"]["error"],
        format!("No containers found for save: {save_id}")
    );

    std::env::remove_var("PSP_GAMEPASS_PACKAGES_ROOT");
    std::env::remove_var("PSP_BACKUPS_ROOT");
    server.handle.shutdown().await;
}

/// End-to-end gamepass LOAD + modded-save WRITE flow, exercised over the WS
/// socket against a synthetic wgs container tree built from the COMMITTED
/// `tests/fixtures/saves/world1` save (real PlM/Oodle bytes, always present in
/// a fresh clone / CI — no `PSP_PY_TESTDATA` gate). Mirrors Python's
/// `select_save` (gamepass branch) -> `select_gamepass_save` ->
/// `save_modded_save` sequence in `local_file_handler.py`.
#[tokio::test]
async fn select_gamepass_save_loads_and_saves_modded_copy() {
    let _env_guard = GAMEPASS_ENV_LOCK.lock().await;

    let world1 = repo_root().join("tests/fixtures/saves/world1");
    let level_bytes = std::fs::read(world1.join("Level.sav")).unwrap();
    let meta_bytes = std::fs::read(world1.join("LevelMeta.sav")).unwrap();
    // world1's players are named with UPPERCASE simple-hex UUIDs (Steam on-disk
    // form); parse one back to a Uuid so the synthetic gamepass container name
    // (`Players-<HEX>`) and the wire `players` array (dashed lowercase) agree.
    let player_uuid = uuid::Uuid::parse_str("43797F87000000000000000000000000").unwrap();
    let player_bytes =
        std::fs::read(world1.join("Players/43797F87000000000000000000000000.sav")).unwrap();

    let gamepass_root = tempfile::tempdir().unwrap();
    let save_id = "FEED0000FEED0000FEED0000FEED0000";
    let save = psp_core::gamepass::fixture::SyntheticSave {
        save_id: save_id.to_string(),
        level_sav: level_bytes,
        level_meta: Some(meta_bytes),
        local_data: None,
        world_option: None,
        players: vec![psp_core::gamepass::fixture::SyntheticPlayer {
            id: player_uuid,
            sav: player_bytes,
            dps: None,
        }],
    };
    let container_dir =
        psp_core::gamepass::fixture::build_wgs_tree(gamepass_root.path(), &[save]).unwrap();

    // Backups must NOT touch the process CWD's real `backups/`.
    std::env::set_var("PSP_BACKUPS_ROOT", gamepass_root.path().join("backups"));
    std::env::set_var("PSP_GAMEPASS_PACKAGES_ROOT", gamepass_root.path());

    let server = common::start_test_server().await;

    // The gamepass load reads the container dir from settings.save_dir (set by
    // the desktop dialog in production); seed it directly in the SAME SQLite
    // file the server opened (`<temp>/psp-rs.db`), then drop our pool so no
    // extra connection lingers against it.
    let pool = psp_db::open(&server._temp_dir.path().join("psp-rs.db"))
        .await
        .unwrap();
    psp_db::settings::update_save_dir(&pool, &container_dir.to_string_lossy())
        .await
        .unwrap();
    pool.close().await;

    let mut ws = common::connect(&server).await;

    // select_save (gamepass branch) -> select_gamepass_save with the saves map.
    common::send_json(
        &mut ws,
        serde_json::json!({
            "type": "select_save",
            "data": {
                "type": "gamepass",
                "path": container_dir.join("containers.index").to_string_lossy(),
                "local": false
            }
        }),
    )
    .await;
    let response = common::next_json(&mut ws).await;
    assert_eq!(response["type"], "select_gamepass_save");
    assert!(response["data"][save_id]["world_name"].is_string());

    // select_gamepass_save -> progress* -> loaded_save_files -> summaries.
    common::send_json(
        &mut ws,
        serde_json::json!({"type": "select_gamepass_save", "data": save_id}),
    )
    .await;
    let messages = recv_until(&mut ws, "get_guild_summaries").await;
    let loaded = messages
        .iter()
        .find(|message| message["type"] == "loaded_save_files")
        .expect("loaded_save_files not emitted");
    assert_eq!(loaded["data"]["type"], "gamepass");
    assert_eq!(loaded["data"]["has_gps"], false);
    let players = loaded["data"]["players"].as_array().unwrap();
    assert!(
        players.contains(&serde_json::json!("43797f87-0000-0000-0000-000000000000")),
        "players array {players:?} missing the loaded player uuid"
    );
    assert!(messages
        .iter()
        .any(|message| message["type"] == "get_player_summaries"));

    // save_modded_save (gamepass branch) -> progress* -> save_modded_save.
    common::send_json(
        &mut ws,
        serde_json::json!({"type": "save_modded_save", "data": "Modded World"}),
    )
    .await;
    let messages = recv_until(&mut ws, "save_modded_save").await;
    assert_eq!(messages.last().unwrap()["data"], "Created modded save");
    let progress: Vec<&str> = messages
        .iter()
        .filter(|m| m["type"] == "progress_message")
        .filter_map(|m| m["data"].as_str())
        .collect();
    assert!(progress.contains(&"Creating backup of container path..."));
    assert!(progress.contains(&"Converting modified save to SAV format..."));
    assert!(progress.contains(&"Creating new containers for modified save..."));
    assert!(progress.contains(&"Modded save created"));

    // A new save id now sits beside the original, carrying the new world name.
    let saves = psp_core::gamepass::scan::scan_saves(&container_dir).unwrap();
    assert_eq!(saves.len(), 2, "expected original + modded save");
    let modded = saves
        .iter()
        .map(|(_, data)| data)
        .find(|data| data.save_id != save_id)
        .expect("modded save missing");
    assert_eq!(modded.world_name, "Modded World");

    std::env::remove_var("PSP_BACKUPS_ROOT");
    std::env::remove_var("PSP_GAMEPASS_PACKAGES_ROOT");
    server.handle.shutdown().await;
}
