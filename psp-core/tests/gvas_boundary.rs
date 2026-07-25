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

    // Golden for the JS boundary test (Task 2 step 4). Written under target/ so it
    // is never committed; the JS test reads it via the same relative path.
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../target/gvas-goldens");
    std::fs::create_dir_all(&out).expect("create golden dir");
    std::fs::write(out.join("world1-level.gvas"), &gvas).expect("write golden");
}
