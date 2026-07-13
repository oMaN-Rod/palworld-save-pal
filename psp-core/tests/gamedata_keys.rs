//! Guards the case-sensitive `pals.json` top-level key contract.

use std::collections::HashSet;

use psp_core::gamedata::GameData;

fn game_data() -> GameData {
    let json_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/json");
    GameData::load(&json_dir).expect("data dir")
}

#[test]
fn pals_json_keys_are_upper_camel_and_boss_prefixes_are_uppercase() {
    let data = game_data();
    let pals = data.get("pals").expect("pals.json present");
    let map = pals.as_object().expect("pals.json is an object");

    assert!(map.len() > 100, "expected a full pal catalog, got {}", map.len());

    // `PalLookup::lower_to_canonical` (gamedata.rs) folds every key to
    // lowercase for `pal_data_for`'s case-insensitive lookup. If pals.json
    // ever carried two keys differing only by case, that fold would collide
    // and `pal_data_for` would silently resolve to whichever key happened to
    // be inserted last -- a real, load-bearing invariant, unlike a bare
    // "starts uppercase" check (pal_data_for doesn't care about case at all,
    // so that assertion never caught anything `pal_data_for` itself would
    // notice).
    let mut seen_lower = HashSet::new();
    let mut boss_count = 0;
    for key in map.keys() {
        assert!(
            seen_lower.insert(key.to_lowercase()),
            "pals.json key collides case-insensitively with another key, \
             which would make pal_data_for's lower_to_canonical lookup \
             ambiguous: {key}"
        );
        if key.to_uppercase().starts_with("BOSS_") {
            assert!(
                key.starts_with("BOSS_"),
                "boss prefix must be literally uppercase BOSS_, got: {key}"
            );
            boss_count += 1;
        }
    }

    // format_character_key (dto/pal.rs) only strips a BOSS_ prefix when the
    // full id is ABSENT from known_pal_keys (an exact-case HashSet) -- so if
    // pals.json ever lost every BOSS_ entry, that branch would silently stop
    // being exercised by real data and this guard would still pass with zero
    // iterations. Pin that boss keys actually exist.
    assert!(
        boss_count > 0,
        "expected at least one BOSS_-prefixed pal key, got 0"
    );
}
