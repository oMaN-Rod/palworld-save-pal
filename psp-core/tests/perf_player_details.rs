//! Performance-diagnosis harness for `get_player_details` (a single player
//! can take ~18s debug / ~3-4s release on a large save). Not a correctness
//! test -- `#[ignore]`d and gated behind `PSP_PERF_SAVE` (a Steam save
//! directory containing `Level.sav` + `Players/`) so it never runs in normal
//! `cargo test`/CI. Run with:
//!
//!   cargo test --release -p psp-core --test perf_player_details -- --ignored --nocapture
//!
//! with `PSP_PERF_SAVE` set to the save directory (the one holding
//! `Level.sav`, NOT the `Players` subdirectory).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use psp_core::domain::{pal, player, world};
use psp_core::gamedata::GameData;
use psp_core::progress::null_progress;
use psp_core::session::{PlayerFileData, SaveKind, SaveSession};
use uuid::Uuid;

fn game_data() -> GameData {
    let json_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/json");
    GameData::load(&json_dir).expect("data dir")
}

/// Deliberately duplicates `common::load_corpus_session` so this harness has
/// its own `PSP_PERF_SAVE` gate rather than that helper's `PSP_TEST_SAVE_DIR`.
fn load_perf_session(save_dir: &Path) -> SaveSession {
    let level_sav_bytes = std::fs::read(save_dir.join("Level.sav")).expect("read Level.sav");
    let level_meta_bytes = std::fs::read(save_dir.join("LevelMeta.sav")).ok();

    let mut player_file_refs: BTreeMap<Uuid, PlayerFileData> = BTreeMap::new();
    if let Ok(entries) = std::fs::read_dir(save_dir.join("Players")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "sav") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let (uid_part, is_dps) = match stem.strip_suffix("_dps") {
                Some(base) => (base, true),
                None => (stem, false),
            };
            let Ok(uid) = uid_part.parse::<Uuid>() else {
                continue;
            };
            let file_ref = player_file_refs
                .entry(uid)
                .or_insert(PlayerFileData::Paths {
                    sav: None,
                    dps: None,
                });
            if let PlayerFileData::Paths { sav, dps } = file_ref {
                if is_dps {
                    *dps = Some(path);
                } else {
                    *sav = Some(path);
                }
            }
        }
    }

    SaveSession::load(
        SaveKind::Steam {
            level_path: save_dir.join("Level.sav"),
        },
        save_dir.to_string_lossy().into_owned(),
        "steam",
        &level_sav_bytes,
        level_meta_bytes.as_deref(),
        None,
        player_file_refs,
        None,
        true,
        &null_progress(),
    )
    .expect("load perf session")
}

/// Coarse section timing for `get_player_details`'s dominant loop (the
/// owned-pals scan over the whole `CharacterSaveParameterMap`), plus a
/// breakdown of what inside `pal_dto_from_entry` costs the most.
#[test]
#[ignore]
fn profile_get_player_details_for_real_save() {
    let Ok(save_dir) = std::env::var("PSP_PERF_SAVE") else {
        eprintln!("PSP_PERF_SAVE not set; skipping perf harness");
        return;
    };
    let save_dir = PathBuf::from(save_dir);
    let player_id: Uuid = "00000000-0000-0000-0000-000000000001".parse().unwrap();

    let load_start = Instant::now();
    let mut session = load_perf_session(&save_dir);
    println!("[perf] session load: {:?}", load_start.elapsed());

    let data = game_data();

    let entries = world::character_map(&session.level).expect("character map");
    let total_entries = entries.len();
    let non_player_entries = entries
        .iter()
        .filter(|entry| !world::entry_is_player(entry))
        .count();
    println!(
        "[perf] CharacterSaveParameterMap: {total_entries} total entries, {non_player_entries} non-player"
    );

    // Owner-uid-only prefilter cost vs full DTO-build cost, over the same
    // entries `get_player_details`'s own pals loop scans.
    let mut owned_count = 0usize;
    let owner_check_start = Instant::now();
    for entry in entries {
        if world::entry_is_player(entry) {
            continue;
        }
        let Some(save_parameter) = world::entry_save_parameter(entry) else {
            continue;
        };
        let owner_uid = psp_core::props::get(save_parameter, &["OwnerPlayerUId"])
            .and_then(psp_core::props::as_uuid);
        if owner_uid == Some(player_id) {
            owned_count += 1;
        }
    }
    let owner_check_elapsed = owner_check_start.elapsed();
    println!(
        "[perf] owner-uid-only prefilter over {non_player_entries} entries: {owner_check_elapsed:?} ({owned_count} belong to this player)"
    );

    let full_dto_start = Instant::now();
    let mut built = 0usize;
    for entry in entries {
        if world::entry_is_player(entry) {
            continue;
        }
        if pal::pal_dto_from_entry(entry, &data).is_some() {
            built += 1;
        }
    }
    let full_dto_elapsed = full_dto_start.elapsed();
    println!(
        "[perf] full pal_dto_from_entry over {non_player_entries} entries: {full_dto_elapsed:?} ({built} built)"
    );

    // `known_pal_keys()`'s per-call cost, isolated: it is called at least
    // twice per pal by `read_save_parameter_dto`/`max_hp_for`.
    let known_keys_start = Instant::now();
    for _ in 0..non_player_entries {
        std::hint::black_box(pal::known_pal_keys(&data));
    }
    let known_keys_elapsed = known_keys_start.elapsed();
    println!(
        "[perf] known_pal_keys() called {non_player_entries} times (simulating the current per-pal rebuild): {known_keys_elapsed:?}"
    );
    let known_keys_once_start = Instant::now();
    let keys = pal::known_pal_keys(&data);
    let known_keys_once_elapsed = known_keys_once_start.elapsed();
    println!(
        "[perf] known_pal_keys() called ONCE: {known_keys_once_elapsed:?} ({} keys)",
        keys.len()
    );

    // End to end, cold: this call also parses the player .sav.
    let e2e_start = Instant::now();
    let details = player::get_player_details(&mut session, &data, player_id, &null_progress())
        .expect("no error")
        .expect("player exists in this save");
    let e2e_elapsed = e2e_start.elapsed();
    println!(
        "[perf] get_player_details (cold, includes .sav parse): {e2e_elapsed:?}, {} pals returned",
        details.pals.len()
    );

    // Warm: player + guild already cached, so this isolates what a repeat open
    // of the same player pays (the pals loop, inventory reads, containers).
    let e2e_warm_start = Instant::now();
    let _details_warm =
        player::get_player_details(&mut session, &data, player_id, &null_progress())
            .expect("no error")
            .expect("player exists in this save");
    let e2e_warm_elapsed = e2e_warm_start.elapsed();
    println!("[perf] get_player_details (warm, .sav already cached): {e2e_warm_elapsed:?}");
}
