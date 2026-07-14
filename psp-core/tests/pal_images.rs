//! Content-completeness check: every `is_pal == true` entry in `pals.json`
//! should have a matching `.webp` asset, so that newly-added pals ship with
//! real artwork instead of silently falling back to art-less rendering.
//!
//! This is NOT a crash guard. `ui/src/lib/utils/assetLoader.ts` (`AssetLoader`)
//! falls back to a shared `unknownIcon` (`img/unknown.webp`) whenever a
//! per-key lookup misses, so a missing asset degrades gracefully to a
//! generic icon rather than a broken image / 404. The value of this test is
//! purely to flag the gap early so someone remembers to add the art.
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
//! lookup and are out of scope for this check.

use std::collections::HashSet;
use std::path::Path;

use psp_core::gamedata::GameData;

/// Pal keys that are known to have no `.webp` asset, and are deliberately
/// exempted rather than fixed.
///
/// All eight entries are internal body-part/phase sub-entities of the
/// `RAID_YakushimaBoss002` raid boss (its two hand hitboxes, its head
/// hitbox, and their "_2" phase variants). A player can never own or
/// display one of these as a pal, upstream ships no art for them either
/// (we cherry-picked their image commits verbatim), and at runtime the UI
/// simply falls back to `unknown.webp` for them. If upstream ever adds art
/// for these, the "still missing" check below will start failing and this
/// list should be pruned.
///
/// If this list grows for a *new* reason, add a comment explaining why that
/// entry is legitimately un-ownable/internal rather than just silencing the
/// test.
const KNOWN_MISSING_ART: &[&str] = &[
    "RAID_YakushimaBoss002",
    "RAID_YakushimaBoss002_2",
    "RAID_YakushimaBoss002_Hand_Left",
    "RAID_YakushimaBoss002_Hand_Left_2",
    "RAID_YakushimaBoss002_Hand_Right",
    "RAID_YakushimaBoss002_Hand_Right_2",
    "RAID_YakushimaBoss002_Head",
    "RAID_YakushimaBoss002_Head_2",
];

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

/// Returns true if neither `<cleansed>.webp` nor the menu-icon fallback
/// exists in `existing` for the given pal `key`.
fn is_missing_art(key: &str, existing: &HashSet<String>) -> bool {
    let cleansed = cleanse_character_id(key);
    let direct = format!("{cleansed}.webp");
    let menu_fallback = format!("t_{cleansed}_icon_normal.webp");
    !existing.contains(&direct) && !existing.contains(&menu_fallback)
}

fn existing_assets() -> HashSet<String> {
    let img_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui/src/lib/assets/img");
    std::fs::read_dir(&img_dir)
        .expect("img dir present")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().to_lowercase())
        .collect()
}

#[test]
fn every_pal_key_has_an_image_asset() {
    let data = game_data();
    let pals = data.get("pals").expect("pals.json present");
    let map = pals.as_object().expect("pals.json is an object");

    let existing = existing_assets();
    let allow_list: HashSet<&str> = KNOWN_MISSING_ART.iter().copied().collect();

    let mut missing: Vec<String> = Vec::new();
    for (key, value) in map.iter() {
        let is_pal = value
            .get("is_pal")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !is_pal {
            // Non-pal entries (humans/NPCs) never reach a per-key image
            // lookup in the UI; they render a shared "commonhuman" icon.
            continue;
        }

        if allow_list.contains(key.as_str()) {
            continue;
        }

        if is_missing_art(key, &existing) {
            missing.push(key.clone());
        }
    }

    missing.sort();
    assert!(
        missing.is_empty(),
        "{} pal(s) have no .webp asset and are not in KNOWN_MISSING_ART: {:?}\n\
         Either add the missing asset(s) under ui/src/lib/assets/img, or if the \
         pal is a genuinely un-ownable internal entity (like a raid-boss body \
         part), deliberately add it to KNOWN_MISSING_ART in psp-core/tests/pal_images.rs \
         with a comment explaining why.",
        missing.len(),
        &missing[..missing.len().min(20)]
    );

    // Keep KNOWN_MISSING_ART honest: if upstream ever ships art for one of
    // these entries, fail here so the entry gets pruned instead of the
    // allow-list silently rotting.
    let still_missing: Vec<&str> = KNOWN_MISSING_ART
        .iter()
        .copied()
        .filter(|key| is_missing_art(key, &existing))
        .collect();
    let stale: Vec<&str> = KNOWN_MISSING_ART
        .iter()
        .copied()
        .filter(|key| !still_missing.contains(key))
        .collect();
    assert!(
        stale.is_empty(),
        "these KNOWN_MISSING_ART entries now have art and should be removed \
         from the allow-list in psp-core/tests/pal_images.rs: {stale:?}"
    );
}
