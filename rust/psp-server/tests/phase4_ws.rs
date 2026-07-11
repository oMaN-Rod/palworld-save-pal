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
