//! Phase 4 WS integration tests (Task 11: gamepass scan/delete/rename).
//! `PSP_GAMEPASS_PACKAGES_ROOT` and `PSP_BACKUPS_ROOT` are process-global env
//! vars, so every assertion that depends on them lives in the single
//! `gamepass_scan_and_management_flow` test below to avoid cross-test races
//! (cargo runs tests in the same process, in parallel threads by default).

mod common;

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
    let _env_guard = common::GamepassEnvGuard::acquire(&[
        (
            "PSP_GAMEPASS_PACKAGES_ROOT",
            gamepass_root.path().to_path_buf(),
        ),
        ("PSP_BACKUPS_ROOT", gamepass_root.path().join("backups")),
    ])
    .await;

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
    let _env_guard = common::GamepassEnvGuard::acquire(&[
        ("PSP_BACKUPS_ROOT", gamepass_root.path().join("backups")),
        (
            "PSP_GAMEPASS_PACKAGES_ROOT",
            gamepass_root.path().to_path_buf(),
        ),
    ])
    .await;

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

    server.handle.shutdown().await;
}

/// Standalone (headless) `convert_save_format`: gamepass -> steam -> gamepass,
/// built entirely from the COMMITTED `tests/fixtures/saves/world1` fixture (no
/// `PSP_PY_TESTDATA` gate), plus the sentinel-path and nothing-loaded error
/// branches. Runs unconditionally in a fresh clone / CI. Only the second leg
/// (steam -> gamepass) writes a backup (`import_steam_dir_to_gamepass`), so
/// `PSP_BACKUPS_ROOT` is pointed at a temp dir for the whole test to keep the
/// process CWD's real `backups/` untouched.
#[tokio::test]
async fn convert_save_format_standalone_round_trip_over_ws() {
    let world1 = repo_root().join("tests/fixtures/saves/world1");
    let level_bytes = std::fs::read(world1.join("Level.sav")).unwrap();
    let meta_bytes = std::fs::read(world1.join("LevelMeta.sav")).unwrap();
    let player_uuid = uuid::Uuid::parse_str("43797F87000000000000000000000000").unwrap();
    let player_bytes =
        std::fs::read(world1.join("Players/43797F87000000000000000000000000.sav")).unwrap();

    let temp = tempfile::tempdir().unwrap();
    let save_id = "ACE00000ACE00000ACE00000ACE00000";
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
    let container_dir = psp_core::gamepass::fixture::build_wgs_tree(temp.path(), &[save]).unwrap();
    let steam_out = temp.path().join("steam-out");

    let _env_guard =
        common::GamepassEnvGuard::acquire(&[("PSP_BACKUPS_ROOT", temp.path().join("backups"))])
            .await;

    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;

    // gamepass → steam (standalone, explicit paths)
    common::send_json(
        &mut ws,
        serde_json::json!({
            "type": "convert_save_format",
            "data": {
                "target_format": "steam",
                "source_path": container_dir.to_string_lossy(),
                "output_path": steam_out.to_string_lossy()
            }
        }),
    )
    .await;
    let messages = recv_until(&mut ws, "convert_save_format").await;
    let response = messages.last().unwrap();
    assert!(response["data"]["message"]
        .as_str()
        .unwrap()
        .starts_with("GamePass saves extracted to Steam format at:"));
    let extracted_level = std::fs::read(steam_out.join(save_id).join("Level.sav")).unwrap();
    assert_eq!(&extracted_level[8..12], b"PlM1");
    assert!(steam_out
        .join(save_id)
        .join("Players")
        .join("43797F87000000000000000000000000.sav")
        .exists());
    assert!(messages.iter().any(|message| {
        message["type"] == "progress_message"
            && message["data"] == format!("Converting Level.sav for {save_id}...")
    }));

    // steam → gamepass (standalone, back into the same container dir)
    common::send_json(
        &mut ws,
        serde_json::json!({
            "type": "convert_save_format",
            "data": {
                "target_format": "gamepass",
                "source_path": steam_out.join(save_id).to_string_lossy(),
                "output_path": container_dir.to_string_lossy()
            }
        }),
    )
    .await;
    let messages = recv_until(&mut ws, "convert_save_format").await;
    let response = messages.last().unwrap();
    let new_save_id = response["data"]["save_id"].as_str().unwrap();
    assert_eq!(new_save_id.len(), 32);
    let saves = psp_core::gamepass::scan::scan_saves(&container_dir).unwrap();
    assert!(saves.get(new_save_id).is_some());

    // sentinel path in web mode → "No file selected."
    common::send_json(
        &mut ws,
        serde_json::json!({
            "type": "convert_save_format",
            "data": {"target_format": "steam", "source_path": "__select__"}
        }),
    )
    .await;
    let response = common::next_json(&mut ws).await;
    assert_eq!(response["data"]["error"], "No file selected.");

    // nothing loaded, no paths → Python's exact error string
    common::send_json(
        &mut ws,
        serde_json::json!({
            "type": "convert_save_format",
            "data": {"target_format": "gamepass"}
        }),
    )
    .await;
    let response = common::next_json(&mut ws).await;
    assert_eq!(
        response["data"]["error"],
        "No save file loaded and no source path provided."
    );

    server.handle.shutdown().await;
}

/// `convert_sav_file` round trip over the WS socket: a real committed
/// player `.sav` (`tests/fixtures/saves/world1/Players/...`) -> JSON string ->
/// back to `.sav` (base64), asserting the rebuilt bytes carry the PlM1 magic.
/// Runs unconditionally (no `PSP_PY_TESTDATA` gate).
#[tokio::test]
async fn convert_sav_file_round_trips_over_ws() {
    let world1 = repo_root().join("tests/fixtures/saves/world1");
    let sav_bytes =
        std::fs::read(world1.join("Players/43797F87000000000000000000000000.sav")).unwrap();

    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;

    let file_data: Vec<u64> = sav_bytes.iter().map(|byte| *byte as u64).collect();
    common::send_json(
        &mut ws,
        serde_json::json!({
            "type": "convert_sav_file",
            "data": {"file_data": file_data, "target_type": "json"}
        }),
    )
    .await;
    let response = common::next_json(&mut ws).await;
    assert_eq!(response["type"], "convert_sav_file");
    let json_text = response["data"]
        .as_str()
        .expect("data must be a JSON string");
    assert!(json_text.starts_with('{'));

    let json_as_ints: Vec<u64> = json_text.bytes().map(|byte| byte as u64).collect();
    common::send_json(
        &mut ws,
        serde_json::json!({
            "type": "convert_sav_file",
            "data": {"file_data": json_as_ints, "target_type": "sav"}
        }),
    )
    .await;
    let response = common::next_json(&mut ws).await;
    let encoded = response["data"].as_str().expect("data must be base64");
    use base64::Engine as _;
    let rebuilt = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .unwrap();
    assert_eq!(&rebuilt[8..12], b"PlM1");

    server.handle.shutdown().await;
}

/// Reads `SaveData.WorldMapMaskTextureV4` back out of a `LocalData.sav`-shaped
/// GVAS blob, mirroring `psp_core::localdata`'s own private test helper of the
/// same name (that one is `cfg(test)`-private to `psp-core`, so this WS-level
/// test needs its own copy to inspect the mask before/after the handler runs).
fn mask_bytes(local_data_sav: &[u8]) -> Vec<u8> {
    let save = uesave::Save::read_with_types(
        &mut std::io::Cursor::new(local_data_sav),
        uesave::games::palworld::palworld_types(),
    )
    .unwrap();
    let uesave::Property::Struct(uesave::StructValue::Struct(save_data)) =
        &save.root.properties.0[&uesave::PropertyKey::from("SaveData")]
    else {
        panic!("SaveData missing");
    };
    let uesave::Property::Array(uesave::ValueVec::Byte(uesave::ByteArray::Byte(bytes))) =
        &save_data.0[&uesave::PropertyKey::from("WorldMapMaskTextureV4")]
    else {
        panic!("WorldMapMaskTextureV4 missing or not a byte array");
    };
    bytes.clone()
}

/// Hermetic `unlock_map` test: grafts a synthetic `WorldMapMaskTextureV4`
/// mask `[1, 2, 3, 0, 4]` into the COMMITTED `tests/fixtures/saves/world1`
/// `LevelMeta.sav`'s `SaveData` struct (the same graft `psp_core::localdata`'s
/// own hermetic test uses, ported to a real committed fixture instead of the
/// gamepass backup corpus), writes the result to a TEMP `LocalData.sav`, and
/// drives the `unlock_map` WS handler against that temp path — the real
/// `tests/fixtures` tree is never written to.
///
/// SAFETY: `unlock_map` rewrites its target file IN PLACE, so this test only
/// ever points the handler at a path under `tempfile::tempdir()`.
#[tokio::test]
async fn unlock_map_zeroes_mask_and_backs_up() {
    let world1 = repo_root().join("tests/fixtures/saves/world1");
    let meta_bytes = std::fs::read(world1.join("LevelMeta.sav")).unwrap();

    let mut save = psp_core::savio::read_sav_bytes(&meta_bytes).unwrap();
    {
        let save_data = psp_core::props::get_mut(&mut save.root.properties, &["SaveData"])
            .expect("world1 LevelMeta.sav must carry a SaveData struct");
        let save_data =
            psp_core::props::struct_props_mut(save_data).expect("SaveData must be a struct");
        save_data.insert(
            "WorldMapMaskTextureV4",
            uesave::Property::Array(uesave::ValueVec::Byte(uesave::ByteArray::Byte(vec![
                1, 2, 3, 0, 4,
            ]))),
        );
    }
    psp_core::props::ensure_schema(
        &mut save,
        "SaveData.WorldMapMaskTextureV4".to_string(),
        uesave::PropertyTagPartial {
            id: None,
            data: uesave::PropertyTagDataPartial::Array(Box::new(
                uesave::PropertyTagDataPartial::Byte(None),
            )),
        },
    );
    let grafted_local_data = psp_core::savio::write_sav_bytes(&save).unwrap();
    let original_mask = mask_bytes(&grafted_local_data);
    assert_eq!(original_mask, vec![1, 2, 3, 0, 4]); // sanity: the graft took

    let temp = tempfile::tempdir().unwrap();
    let local_data_path = temp.path().join("LocalData.sav");
    std::fs::write(&local_data_path, &grafted_local_data).unwrap();

    let server = common::start_test_server().await;
    let mut ws = common::connect(&server).await;

    common::send_json(
        &mut ws,
        serde_json::json!({
            "type": "unlock_map",
            "data": {"path": local_data_path.to_string_lossy()}
        }),
    )
    .await;
    let response = common::next_json(&mut ws).await;
    assert_eq!(response["type"], "unlock_map");
    assert_eq!(response["data"]["success"], true);
    assert_eq!(
        response["data"]["message"],
        "Map unlocked successfully! Restart the game to see changes."
    );

    let backup_path = temp.path().join("LocalData.sav.backup");
    assert!(backup_path.exists());
    assert_eq!(
        mask_bytes(&std::fs::read(&backup_path).unwrap()),
        original_mask
    );

    let rewritten = std::fs::read(&local_data_path).unwrap();
    assert_eq!(&rewritten[8..12], b"PlM1");
    assert_eq!(mask_bytes(&rewritten), vec![0, 0, 0, 0, 0]);

    // Wrong file name → handler-internal error message (NOT a dispatcher error).
    let wrong_path = temp.path().join("Level.sav");
    std::fs::write(&wrong_path, b"x").unwrap();
    common::send_json(
        &mut ws,
        serde_json::json!({
            "type": "unlock_map",
            "data": {"path": wrong_path.to_string_lossy()}
        }),
    )
    .await;
    let response = common::next_json(&mut ws).await;
    assert_eq!(response["type"], "error");
    assert_eq!(
        response["data"]["message"],
        "Failed to unlock map: Please select the LocalData.sav file."
    );

    // Missing path (web mode) → "No file path provided"
    common::send_json(
        &mut ws,
        serde_json::json!({"type": "unlock_map", "data": {}}),
    )
    .await;
    let response = common::next_json(&mut ws).await;
    assert_eq!(response["type"], "error");
    assert_eq!(
        response["data"]["message"],
        "Failed to unlock map: No file path provided"
    );

    server.handle.shutdown().await;
}
