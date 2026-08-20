use std::path::PathBuf;

fn world1_level_sav() -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/saves/world1/Level.sav");
    std::fs::read(path).expect("read world1 Level.sav")
}

#[test]
fn write_gvas_is_gvas_prefixed_and_round_trips() {
    let sav = world1_level_sav();
    let save = psp_core::savio::read_sav_bytes(&sav).expect("parse .sav");
    let gvas = psp_core::savio::write_gvas_bytes(&save).expect("serialize GVAS");
    assert_eq!(&gvas[0..4], b"GVAS", "write_gvas_bytes must emit raw GVAS");

    let reparsed = psp_core::savio::read_gvas_bytes(&gvas).expect("parse GVAS");
    let regvas = psp_core::savio::write_gvas_bytes(&reparsed).expect("re-serialize");
    assert_eq!(gvas, regvas, "GVAS round-trips byte-for-byte");

    // Golden the JS boundary test reads (written under target/, never committed).
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../target/gvas-goldens");
    std::fs::create_dir_all(&out).expect("create golden dir");
    std::fs::write(out.join("world1-level.gvas"), &gvas).expect("write golden");
}

#[test]
fn gvas_getters_round_trip_and_load_ingests_gvas() {
    let sav = world1_level_sav();
    let session = psp_core::session::SaveSession::load(
        psp_core::session::SaveKind::InMemory,
        "world1".to_string(),
        "steam",
        &sav,
        None,
        None,
        std::collections::BTreeMap::new(),
        None,
        false,
        &psp_core::progress::null_progress(),
    )
    .expect("load from .sav");

    // The save-side getter emits GVAS that read_gvas_bytes accepts.
    let level_gvas = session.level_gvas_bytes().expect("level gvas");
    assert_eq!(&level_gvas[0..4], b"GVAS");
    psp_core::savio::read_gvas_bytes(&level_gvas).expect("level gvas re-parses");

    // SaveSession::load ingests that same raw GVAS (pass-through) into an equivalent session.
    let from_gvas = psp_core::session::SaveSession::load(
        psp_core::session::SaveKind::InMemory,
        "world1".to_string(),
        "steam",
        &level_gvas,
        None,
        None,
        std::collections::BTreeMap::new(),
        None,
        false,
        &psp_core::progress::null_progress(),
    )
    .expect("load from GVAS");
    assert_eq!(
        from_gvas.level_gvas_bytes().expect("re-emit"),
        level_gvas,
        "load(GVAS) reproduces the same level GVAS"
    );
}
