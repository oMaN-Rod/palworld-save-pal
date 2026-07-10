mod common;

use psp_core::gamedata::GameData;

/// `GameData::load` takes the `data/json` directory. From
/// `rust/psp-core`, `../../data/json` is `<repo>/data/json` -- see
/// `pal_read.rs`'s own doc comment for this workspace's established
/// `CARGO_MANIFEST_DIR`-relative convention.
fn game_data() -> GameData {
    let json_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/json");
    GameData::load(&json_dir).expect("data dir")
}

/// Set `PSP_TEST_GPS_SAV` to a real `GlobalPalStorage.sav` to run this
/// module's corpus test; skips cleanly otherwise, matching every other
/// corpus-gated test in this workspace (`common::load_corpus_session`,
/// `session.rs`'s `test_load_real_steam_save`). Unlike the task brief's
/// draft, this does NOT also require a `PSP_TEST_LEVEL_SAV` -- GPS session
/// state (`SaveSession::gps`) has no dependency on which `Level.sav` is
/// loaded (`domain::gps`'s ops only ever touch `session.gps`, never
/// `session.level`), so the always-present, checked-in `world1` fixture
/// (`common::load_fixture_session`) already supplies everything the
/// `SaveSession` half of this test needs.
fn gps_fixture_bytes() -> Option<Vec<u8>> {
    let path = std::env::var("PSP_TEST_GPS_SAV").ok()?;
    Some(std::fs::read(path).expect("PSP_TEST_GPS_SAV must point at a readable file"))
}

/// Exercises `common::load_corpus_session` (which passes `gps_file_path:
/// None` into `SaveSession::load`, same as every save with no
/// `GlobalPalStorage.sav`), proving the plumbing lands where this task wired
/// it: `session.gps.file_path` stays `None`, so `gps_available()`/
/// `gps_pals()` correctly report "nothing to load" before `load_gps` ever
/// runs. Skips cleanly without `PSP_TEST_SAVE_DIR`.
#[test]
fn gps_state_starts_unavailable_for_a_freshly_loaded_corpus_session() {
    let Some(session) = common::load_corpus_session() else {
        return;
    };
    assert!(!session.gps_available());
    assert!(session.gps_pals().is_none());
}

/// Full round trip against a real `GlobalPalStorage.sav`: load, add, clone
/// (`add_gps_pal_from_dto`, which is also what `clone_gps_pal` ports to —
/// `pal_ops.py:266-267`), delete, and confirm the freed slots are reusable
/// and the tree still re-serializes. Mirrors the corpus-gated round trip
/// this workspace already runs for player/DPS pals
/// (`pal_crud.rs`'s `add_and_delete_player_pal_round_trips_across_the_whole_corpus`).
#[test]
fn gps_load_add_clone_delete_round_trips_against_a_real_file() {
    let Some(gps_bytes) = gps_fixture_bytes() else {
        eprintln!("PSP_TEST_GPS_SAV not set, skipping");
        return;
    };
    let mut session = common::load_fixture_session("world1");
    let data = game_data();

    assert!(session.gps_pals().is_none(), "not loaded yet");
    session.load_gps(&gps_bytes, &data).unwrap();
    let initial_count = session.gps_pals().unwrap().len();

    let Some((new_pal, slot)) = session
        .add_gps_pal(&data, "SheepBall", "TestSheep", None)
        .unwrap()
    else {
        eprintln!("no empty GPS slot in this corpus file; nothing further to prove");
        return;
    };
    assert_eq!(new_pal.character_id, "SheepBall");
    assert_eq!(new_pal.nickname.as_deref(), Some("TestSheep"));
    assert_eq!(new_pal.hp, new_pal.max_hp);
    assert_eq!(session.gps_pals().unwrap().len(), initial_count + 1);

    // clone_gps_pal == add_gps_pal_from_dto (pal_ops.py:266-267)
    let Some((clone_slot, clone)) = session.add_gps_pal_from_dto(&data, &new_pal, None).unwrap()
    else {
        eprintln!("no second empty GPS slot in this corpus file; nothing further to prove");
        return;
    };
    assert_ne!(clone_slot, slot);
    assert_ne!(
        clone.instance_id, new_pal.instance_id,
        "clone gets a fresh instance id"
    );

    session.delete_gps_pals(&[slot, clone_slot]);
    assert_eq!(session.gps_pals().unwrap().len(), initial_count);
    // freed slots become reusable
    assert!(session.find_first_empty_gps_slot().is_some());

    session.rebuild_gps_save(&data).unwrap();
    assert_eq!(session.gps_pals().unwrap().len(), initial_count);
    let resaved = session
        .gps_sav_bytes()
        .unwrap()
        .expect("gps save must still be loaded");
    assert!(!resaved.is_empty());
}
