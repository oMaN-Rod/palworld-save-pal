mod common;

use psp_core::gamedata::GameData;

fn game_data() -> GameData {
    let json_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/json");
    GameData::load(&json_dir).expect("data dir")
}

/// The committed `GlobalPalStorage.sav` fixture bytes. No `Level.sav` is needed
/// alongside it: GPS ops only ever touch `session.gps`, so any loaded session
/// (here: `world1`) will do.
fn gps_fixture_bytes() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/saves/GlobalPalStorage.sav");
    std::fs::read(path).expect("read committed GlobalPalStorage.sav fixture")
}

/// A save with no `GlobalPalStorage.sav` leaves `session.gps.file_path`
/// `None`, so GPS state must report "nothing to load" before `load_gps` runs.
#[test]
fn gps_state_starts_unavailable_for_a_freshly_loaded_corpus_session() {
    let session = common::load_corpus_session();
    assert!(!session.gps_available());
    assert!(session.gps_pals().is_none());
}

/// Full round trip against a real `GlobalPalStorage.sav`: load, add, clone,
/// delete, and confirm the freed slots are reusable and the tree still
/// re-serializes.
#[test]
fn gps_load_add_clone_delete_round_trips_against_a_real_file() {
    let gps_bytes = gps_fixture_bytes();
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

    // `clone_gps_pal` is `add_gps_pal_from_dto`.
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
    assert!(session.find_first_empty_gps_slot().is_some());

    session.rebuild_gps_save(&data).unwrap();
    assert_eq!(session.gps_pals().unwrap().len(), initial_count);
    let resaved = session
        .gps_sav_bytes()
        .unwrap()
        .expect("gps save must still be loaded");
    assert!(!resaved.is_empty());
}
