//! Every pal in `pals.json` must have a matching image asset, or the UI falls
//! back to the generic "unknown" icon instead of the pal's real artwork.
//!
//! Filename derivation mirrors `ui/src/lib/utils/assetLoader.ts`:
//! - `AssetLoader.cleanseCharacterId` lowercases the key and strips the
//!   `predator_`, `_oilrig`, `raid_`, `summon_`, `_max`, trailing `_<digits>`,
//!   `boss_`, `quest_farmer03_`, and `_otomo` tokens.
//! - `AssetLoader.loadPalImage` first tries `<cleansed>.webp`, then falls
//!   back to `t_<cleansed>_icon_normal.webp` (`loadMenuImage`).
//!
//! Only `is_pal == true` entries are checked. For `is_pal == false` entries
//! (humans/NPCs), the UI callers (`PalCard.svelte`, `PalBadge.svelte`,
//! `itemUtils.ts`, etc.) hardcode `character_id = "commonhuman"` before
//! calling into the asset loader, so those entries never attempt a per-key
//! lookup and can't 404 regardless of what images ship.

use std::collections::HashSet;
use std::path::Path;

use psp_core::gamedata::GameData;

fn game_data() -> GameData {
    let json_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/json");
    GameData::load(&json_dir).expect("data dir")
}

/// Mirrors `AssetLoader.cleanseCharacterId` in `ui/src/lib/utils/assetLoader.ts`.
fn cleanse_character_id(key: &str) -> String {
    let mut s = key.to_lowercase();
    s = s.replace("predator_", "");
    s = s.replace("_oilrig", "");
    s = s.replace("raid_", "");
    s = s.replace("summon_", "");
    s = s.replace("_max", "");
    // Strip a trailing `_<digits>` suffix, e.g. "_2".
    if let Some(pos) = s.rfind('_') {
        if s[pos + 1..].chars().all(|c| c.is_ascii_digit()) && pos + 1 < s.len() {
            s.truncate(pos);
        }
    }
    s = s.replace("boss_", "");
    s = s.replace("quest_farmer03_", "");
    s = s.replace("_otomo", "");
    s
}

#[test]
fn every_pal_key_has_an_image_asset() {
    let data = game_data();
    let pals = data.get("pals").expect("pals.json present");
    let map = pals.as_object().expect("pals.json is an object");

    let img_dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui/src/lib/assets/img");
    let existing: HashSet<String> = std::fs::read_dir(&img_dir)
        .expect("img dir present")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().to_lowercase())
        .collect();

    let mut missing: Vec<String> = Vec::new();
    for (key, value) in map.iter() {
        let is_pal = value.get("is_pal").and_then(|v| v.as_bool()).unwrap_or(false);
        if !is_pal {
            // Non-pal entries (humans/NPCs) never reach a per-key image
            // lookup in the UI; they render a shared "commonhuman" icon.
            continue;
        }

        let cleansed = cleanse_character_id(key);
        let direct = format!("{cleansed}.webp");
        let menu_fallback = format!("t_{cleansed}_icon_normal.webp");
        if !existing.contains(&direct) && !existing.contains(&menu_fallback) {
            missing.push(key.clone());
        }
    }

    missing.sort();
    assert!(
        missing.is_empty(),
        "{} pals have no .webp asset: {:?}",
        missing.len(),
        &missing[..missing.len().min(20)]
    );
}
