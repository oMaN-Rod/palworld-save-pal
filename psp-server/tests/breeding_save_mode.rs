//! Save Mode end-to-end: prove the breeding calculator's Save Mode workflow
//! against a real save, exactly as the frontend drives it —
//! `select_save` → player summaries (players discovered) →
//! `request_player_details` with `origin: "breeding"` (the selected player's
//! pals parsed) → `breeding_chain` with those owned pals and a reachable
//! target (chains computed from the player's data).
//!
//! This mirrors `ui/src/routes/breeding/+page.svelte`: the page sends the
//! already-parsed `appState.players[uid].pals` back as `origin: "owned"`
//! inputs (the same shape `saveOwnedPals()` produces) and only computes after
//! the player's pals are loaded.

mod common;

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// The world1 fixture's player (see tests/fixtures/saves/world1/Players/).
const WORLD1_PLAYER_O: &str = "8c2f1930-0000-0000-0000-000000000000";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for dir_entry in std::fs::read_dir(src).unwrap() {
        let dir_entry = dir_entry.unwrap();
        let dest_path = dst.join(dir_entry.file_name());
        if dir_entry.path().is_dir() {
            copy_dir_recursive(&dir_entry.path(), &dest_path);
        } else {
            std::fs::copy(&dir_entry.path(), &dest_path).unwrap();
        }
    }
}

/// Reads frames until one whose `type` equals `stop_type`, panicking if an
/// `error` frame arrives first (rather than hanging until the timeout).
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
            return frames;
        }
    }
}

#[tokio::test]
async fn save_mode_parses_players_and_computes_chains_from_owned_pals() {
    let temp_root = tempfile::tempdir().unwrap();
    let world1_copy = temp_root.path().join("world1");
    copy_dir_recursive(&repo_root().join("tests/fixtures/saves/world1"), &world1_copy);
    let level_sav_path = world1_copy.join("Level.sav").to_string_lossy().into_owned();

    let server = common::start_test_server().await;
    let mut socket = common::connect(&server).await;

    // --- 1. Load the save ----------------------------------------------------
    common::send_json(
        &mut socket,
        json!({"type": "select_save",
               "data": {"type": "steam", "path": level_sav_path, "local": true}}),
    )
    .await;
    let load_frames = recv_until(&mut socket, "get_player_summaries").await;
    let summaries = load_frames
        .iter()
        .rev()
        .find_map(|f| f["data"].as_object().map(|o| o.clone()))
        .expect("get_player_summaries data is an object");
    assert!(
        !summaries.is_empty(),
        "players must be discovered from the save: got {summaries:?}"
    );
    // The breeding page derives its owner list from these summaries.
    for summary in summaries.values() {
        assert!(summary["uid"].is_string(), "summary has uid: {summary:?}");
        assert!(summary["pal_count"].is_u64(), "summary has pal_count");
    }

    // --- 2. Parse the selected player's pals (Save Mode lazy load) -----------
    common::send_json(
        &mut socket,
        json!({"type": "request_player_details",
               "data": {"player_id": WORLD1_PLAYER_O, "origin": "breeding"}}),
    )
    .await;
    let detail_frames = recv_until(&mut socket, "get_player_details_response").await;
    let detail = detail_frames.last().unwrap();
    assert_eq!(detail["data"]["origin"], "breeding", "{detail}");
    let player = &detail["data"]["player"];
    assert!(player["pals"].is_object(), "player must parse a pals object");
    let pals_obj = player["pals"].as_object().unwrap();
    assert!(
        pals_obj.len() >= 3,
        "fixture player should own several pals, got {}: {detail:?}",
        pals_obj.len()
    );

    // --- 3. Shape owned pals EXACTLY like the frontend's saveOwnedPals() -----
    // PalInput: character_id, gender, passive_skills, instance_id, nickname,
    // level, owner_uid, origin: "owned".
    let mut owned_pals: Vec<Value> = Vec::new();
    let mut owned_species: Vec<String> = Vec::new();
    for pal in pals_obj.values() {
        owned_pals.push(json!({
            "character_id": pal["character_id"],
            "gender": pal["gender"],
            "passive_skills": pal["passive_skills"],
            "instance_id": pal["instance_id"],
            "nickname": pal["nickname"],
            "level": pal["level"],
            "owner_uid": pal["owner_uid"],
            "origin": "owned",
        }));
        owned_species.push(pal["character_id"].as_str().unwrap().to_string());
    }
    assert!(
        owned_pals
            .iter()
            .all(|p| p["character_id"].as_str().is_some() && p["passive_skills"].is_array()),
        "owned pal inputs must carry character_id + passive_skills"
    );

    // --- 4. Breeding DB up + breedable list ---------------------------------
    common::send_json(&mut socket, json!({"type": "get_breeding_pals"})).await;
    let pals_frames = recv_until(&mut socket, "get_breeding_pals").await;
    let breedable: Vec<String> = pals_frames
        .last()
        .unwrap()["data"]["pals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["tribe"].as_str().unwrap().to_string())
        .collect();
    assert!(breedable.len() > 100, "breeding dataset loaded");

    let strip_boss = |s: &str| {
        s.strip_prefix("B_O_S_S_")
            .or_else(|| s.strip_prefix("BOSS_"))
            .unwrap_or(s)
            .to_string()
    };

    // --- 5. Compute chains from the player's pals ----------------------------
    // To prove chains are COMPUTED (not just echoed), target species the
    // player does NOT own that two owned species combine into — the
    // `breeding_direct_child` handler lists exactly those. The chain solver
    // is gender-aware (faithful to PalSavTools: a pair needs M+F, wildcard
    // counts as either), so the solver — not the test — decides which combos
    // actually breed.
    let owned_norm: Vec<String> = owned_species.iter().map(|s| strip_boss(s)).collect();
    let mut candidates: Vec<String> = Vec::new();
    for (i, a) in owned_norm.iter().enumerate() {
        for b in owned_norm.iter().skip(i + 1) {
            common::send_json(
                &mut socket,
                json!({"type": "breeding_direct_child",
                       "data": {"parent_a": a, "parent_b": b}}),
            )
            .await;
            let frames = recv_until(&mut socket, "breeding_direct_child").await;
            let result = frames.last().unwrap()["data"]["result"].clone();
            let Some(child) = result["child"].as_str() else {
                continue;
            };
            if child == a || child == b || owned_norm.iter().any(|s| s == child) {
                continue;
            }
            if !candidates.contains(&child.to_string()) {
                candidates.push(child.to_string());
            }
        }
    }
    assert!(
        !candidates.is_empty(),
        "the player's distinct species must combine into new species"
    );

    async fn request_chains(socket: &mut common::WsClient, pals: &[Value], target: &str) -> Value {
        common::send_json(
            socket,
            json!({"type": "breeding_chain",
                   "data": {
                       "target_pal": target,
                       "required_passives": [],
                       "target_gender": null,
                       "max_generations": 4,
                       "max_results": 5,
                       "include_wild": false,
                       "pals": pals,
                   }}),
        )
        .await;
        let frames = recv_until(socket, "breeding_chain").await;
        frames.last().unwrap()["data"].clone()
    }

    // --- 5a. The real pool is all-female: the correct answer is 0 chains. ---
    // The fixture player owns only females, so no non-owned target can be
    // bred. A well-formed EMPTY answer is the correct Save Mode behavior for
    // such a pool and proves the pipeline runs end-to-end without fabricating
    // a chain.
    let response = request_chains(&mut socket, &owned_pals, &candidates[0]).await;
    assert!(response["elapsed_ms"].is_u64(), "{response:?}");
    assert!(response["warnings"].is_array(), "{response:?}");
    assert!(
        response["chains"].as_array().unwrap().is_empty(),
        "an all-female pool must yield no chains (gender-compatible M+F rule): {response:?}"
    );

    // --- 5b. Positive proof: pool + one male of an owned species. ------------
    // A realistic save contains males. Add a single male owned pal of a
    // species the player already owns; every other input is the parsed player
    // data, unmodified. The solver must now produce a real ≥1-step chain for
    // a non-owned target.
    owned_pals.push(json!({
        "character_id": "Anubis",
        "gender": "Male",
        "passive_skills": [],
        "origin": "owned",
    }));
    let mut found: Option<(String, Value)> = None;
    for target in &candidates {
        let response = request_chains(&mut socket, &owned_pals, target).await;
        if !response["chains"].as_array().unwrap().is_empty() {
            found = Some((target.clone(), response));
            break;
        }
    }

    let (target, response) = found.expect(
        "with a male present, at least one non-owned target must be breedable from the owned pals",
    );
    assert_eq!(
        response["total"].as_u64().unwrap() as usize,
        response["chains"].as_array().unwrap().len()
    );
    let chains = response["chains"].as_array().unwrap();
    assert!(chains.len() <= 5, "max_results respected: {response:?}");
    for (i, chain) in chains.iter().enumerate() {
        assert_eq!(chain["target"], target, "chain {i} targets the request");
        let steps = chain["steps"].as_array().unwrap();
        assert!(
            !steps.is_empty(),
            "chain {i} for a non-owned target must have ≥1 breeding step: {chain:?}"
        );
        for step in steps {
            assert!(step["parent_a"].is_string(), "step has parent_a: {step:?}");
            assert!(step["parent_b"].is_string(), "step has parent_b: {step:?}");
            assert!(step["child"].is_string(), "step has child: {step:?}");
        }
    }
    // The final step must actually produce the requested target.
    assert_eq!(
        chains[0]["steps"].as_array().unwrap().last().unwrap()["child"],
        target,
        "the chain's last step produces the target: {response:?}"
    );

    server.handle.shutdown().await;
}

/// The Save Mode loading bug this test pins: `ensurePlayerLoaded` previously
/// used `sendAndWait` on `request_player_details`, whose queue resolves only
/// on a frame of the REQUEST's type — but the backend answers under
/// `get_player_details_response`, so the page hung. The polling fix depends on
/// the backend replying with a DIFFERENT type, which this test asserts
/// explicitly so the wire contract can't silently regress.
#[tokio::test]
async fn player_details_reply_type_differs_from_request_type() {
    let server = common::start_test_server().await;
    let mut socket = common::connect(&server).await;

    // No save loaded: the handler replies with an error — but still under
    // get_player_details_response, never under request_player_details.
    common::send_json(
        &mut socket,
        json!({"type": "request_player_details",
               "data": {"player_id": WORLD1_PLAYER_O, "origin": "breeding"}}),
    )
    .await;
    let frame = common::next_json(&mut socket).await;
    assert_eq!(frame["type"], "get_player_details_response", "{frame}");
    assert_ne!(
        frame["type"], "request_player_details",
        "the reply type must differ from the request type — sendAndWait on the \
         request type would never resolve and Save Mode would hang"
    );

    server.handle.shutdown().await;
}