//! Guards the case-sensitive `pals.json` top-level key contract.

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

    for key in map.keys() {
        assert!(
            !key.is_empty() && key.chars().next().unwrap().is_ascii_uppercase(),
            "pal key must start uppercase (pal_lookup is case-sensitive): {key}"
        );
        if key.to_uppercase().starts_with("BOSS_") {
            assert!(
                key.starts_with("BOSS_"),
                "boss prefix must be literally uppercase BOSS_, got: {key}"
            );
        }
    }
}
