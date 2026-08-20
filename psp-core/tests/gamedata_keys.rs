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

    assert!(
        map.len() > 100,
        "expected a full pal catalog, got {}",
        map.len()
    );

    // `PalLookup::lower_to_canonical` folds every key to lowercase for `pal_data_for`'s
    // case-insensitive lookup; two keys differing only by case would collide there.
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

    // `format_character_key` only strips a BOSS_ prefix when the full id is absent
    // from `known_pal_keys`; pin that boss keys actually exist to exercise that branch.
    assert!(
        boss_count > 0,
        "expected at least one BOSS_-prefixed pal key, got 0"
    );
}

/// `GameData` keys files by their on-disk path, but four l10n directories are mixed-case
/// (`es-MX`, `pt-BR`, `zh-Hans`, `zh-Hant`) while the app sends lowercase locale codes; an
/// exact-case lookup resolves those four to nothing, for every table.
///
/// The codes below are `SupportedLanguage` in `ui/src/lib/types/settings.ts`.
#[test]
fn every_supported_language_resolves_its_l10n_tables() {
    const APP_LOCALES: [&str; 16] = [
        "de", "en", "es", "es-mx", "fr", "it", "id-id", "ko", "pl", "pt-br", "ru", "th", "tr",
        "vi", "zh-hans", "zh-hant",
    ];
    let data = game_data();
    let mut broken = Vec::new();
    for locale in APP_LOCALES {
        for table in ["pals", "items", "relics"] {
            if data.get(&format!("l10n/{locale}/{table}")).is_none() {
                broken.push(format!("l10n/{locale}/{table}"));
            }
        }
    }
    assert!(
        broken.is_empty(),
        "these l10n tables do not resolve for a language the app can be set to, so those \
         users see raw code names: {broken:?}"
    );
}
