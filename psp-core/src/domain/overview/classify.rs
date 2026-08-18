//! Entry classification helpers shared by the overview passes: boss-prefix
//! handling, gender bucketing, and the sick/fainted conditions.

use crate::dto::overview::OverviewGenderSplit;
use crate::dto::pal::format_character_key;
use crate::gamedata::GameData;
use crate::props;
use crate::ue::Properties;

use crate::domain::pal::param;
use std::collections::HashSet;

/// Boss/variant prefixes stripped for species classification. The same list
/// the reference implementation uses; `PREDATOR_`/`GYM_`/`RAID_`/`SUMMON_`
/// variants count as their base species.
const BOSS_PREFIXES: [&str; 6] = ["BOSS_", "B_O_S_S_", "PREDATOR_", "GYM_", "RAID_", "SUMMON_"];

/// `PhysicalHealth` value that means "downed".
const DYING: &str = "EPalStatusPhysicalHealthType::Dying";

/// Strips a boss/raid/predator/gym/summon prefix, case-insensitively matched
/// but case-preserving on the remainder.
pub(crate) fn strip_boss_prefix(character_id: &str) -> &str {
    let upper = character_id.to_uppercase();
    for prefix in BOSS_PREFIXES {
        if upper.starts_with(prefix) {
            return &character_id[prefix.len()..];
        }
    }
    character_id
}

/// A boss variant carries a `BOSS_`/`B_O_S_S_` prefix in its CharacterID.
pub(crate) fn is_boss_id(character_id: &str) -> bool {
    let upper = character_id.to_uppercase();
    upper.starts_with("BOSS_") || upper.starts_with("B_O_S_S_")
}

/// `EPalGenderType::Male`/`Female` → bucket, anything else (including an
/// absent property) → unknown. Returns a one-hot split so the caller can add
/// it into the running totals without a matching branch.
pub(crate) fn gender_bucket(save_parameter: &Properties) -> OverviewGenderSplit {
    let mut split = OverviewGenderSplit::default();
    match param(save_parameter, "Gender").and_then(props::as_enum) {
        Some("EPalGenderType::Male") => split.male = 1,
        Some("EPalGenderType::Female") => split.female = 1,
        _ => split.unknown = 1,
    }
    split
}

/// Sick = `WorkerSick` present with a value that isn't the None enum.
pub(crate) fn is_sick(save_parameter: &Properties) -> bool {
    match param(save_parameter, "WorkerSick").and_then(props::as_enum) {
        Some(value) => !value.is_empty() && !value.contains("None"),
        None => false,
    }
}

/// Fainted = `PalReviveTimer` present (any value) or `PhysicalHealth` ==
/// `Dying`. Current `Hp` is deliberately not consulted.
pub(crate) fn is_fainted(save_parameter: &Properties) -> bool {
    if param(save_parameter, "PalReviveTimer").is_some() {
        return true;
    }
    param(save_parameter, "PhysicalHealth").and_then(props::as_enum) == Some(DYING)
}

/// Catchable human-NPC species share a small set of asset prefixes (or the
/// exact id `Human`). Same list as the reference implementation, which derives
/// its human-species set from `characters.json` by this very match.
const HUMAN_ASSET_PREFIXES: [&str; 16] = [
    "male_",
    "female_",
    "hunter",
    "believer",
    "police",
    "scientist",
    "viking",
    "darktrader",
    "ninja",
    "desertpeople",
    "survey",
    "grimgirl",
    "dandeliongirl",
    "badcatgirl",
    "muumage",
    "merchant",
];

/// Lowercased ids that count as human NPCs: `pals.json` keys matching a human
/// asset prefix, plus the exact `Human` id. Built like the reference's
/// characters.json-derived set, so variant ids that never existed in the
/// species catalog (e.g. `BOSS_DarkTrader`, whose base form is not a catalog
/// key) classify as creatures there and here alike.
pub(crate) fn human_species_set(
    known_species: &std::collections::HashSet<String>,
) -> HashSet<String> {
    known_species
        .iter()
        .filter(|key| {
            *key == "human"
                || HUMAN_ASSET_PREFIXES
                    .iter()
                    .any(|prefix| key.starts_with(prefix))
        })
        .cloned()
        .collect()
}

/// Human-NPC classification: a boss-stripped id whose base form is in the
/// catalog-derived human set. Never classifies an unknown id as human.
pub(crate) fn is_human_npc(character_id: &str, human_species: &HashSet<String>) -> bool {
    let base = strip_boss_prefix(character_id);
    !base.is_empty() && human_species.contains(&base.to_lowercase())
}

/// Canonical `pals.json` casing for a (possibly boss-prefixed, possibly
/// unknown) CharacterID, for frontend icon/name lookups. Falls back to the
/// formatted key when the species isn't in the catalog.
pub(crate) fn canonical_character_key(character_id: &str, game_data: &GameData) -> String {
    let formatted = format_character_key(character_id, &game_data.pal_lookup().keys);
    game_data
        .pal_lookup()
        .lower_to_canonical
        .get(&formatted)
        .cloned()
        .unwrap_or(formatted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ue::Property;

    fn game_data() -> GameData {
        GameData::from_entries([(
            "pals".to_string(),
            r#"{"Alpaca": {"is_pal": true, "scaling": {"hp": 90}}}"#.to_string(),
        )])
        .unwrap()
    }

    #[test]
    fn strip_boss_prefix_matches_all_prefix_families() {
        assert_eq!(strip_boss_prefix("BOSS_Anubis"), "Anubis");
        assert_eq!(strip_boss_prefix("boss_anubis"), "anubis");
        assert_eq!(strip_boss_prefix("B_O_S_S_Lamball"), "Lamball");
        assert_eq!(strip_boss_prefix("PREDATOR_Anubis"), "Anubis");
        assert_eq!(strip_boss_prefix("GYM_Lily"), "Lily");
        assert_eq!(strip_boss_prefix("RAID_XXX"), "XXX");
        assert_eq!(strip_boss_prefix("SUMMON_XXX"), "XXX");
        assert_eq!(strip_boss_prefix("Lamball"), "Lamball");
    }

    #[test]
    fn only_boss_prefixes_make_a_boss() {
        assert!(is_boss_id("BOSS_Alpaca"));
        assert!(is_boss_id("B_O_S_S_Alpaca"));
        assert!(!is_boss_id("PREDATOR_Alpaca"));
        assert!(!is_boss_id("Alpaca"));
    }

    #[test]
    fn gender_buckets_only_known_enum_values() {
        for (raw, expected) in [
            ("EPalGenderType::Male", (1, 0, 0)),
            ("EPalGenderType::Female", (0, 1, 0)),
            ("EPalGenderType::Whatever", (0, 0, 1)),
        ] {
            let mut save_parameter = Properties::default();
            save_parameter.insert("Gender", Property::Enum(raw.to_string()));
            let split = gender_bucket(&save_parameter);
            assert_eq!(
                (split.male, split.female, split.unknown),
                expected,
                "gender {raw}"
            );
        }
        // Absent property → unknown.
        let split = gender_bucket(&Properties::default());
        assert_eq!((split.male, split.female, split.unknown), (0, 0, 1));
    }

    #[test]
    fn sick_requires_a_non_none_worker_sick_value() {
        for (raw, expected) in [
            ("EPalStatusSickType::Cold", true),
            ("EPalStatusSickType::None", false),
            ("", false),
        ] {
            let mut save_parameter = Properties::default();
            save_parameter.insert("WorkerSick", Property::Enum(raw.to_string()));
            assert_eq!(is_sick(&save_parameter), expected, "WorkerSick {raw:?}");
        }
        assert!(!is_sick(&Properties::default()));
    }

    #[test]
    fn fainted_is_revive_timer_presence_or_dying_health() {
        let mut timer_set = Properties::default();
        timer_set.insert("PalReviveTimer", Property::Int(0));
        assert!(is_fainted(&timer_set), "timer presence alone faints");

        let mut dying = Properties::default();
        dying.insert("PhysicalHealth", Property::Enum(DYING.to_string()));
        assert!(is_fainted(&dying));

        let mut healthy = Properties::default();
        healthy.insert(
            "PhysicalHealth",
            Property::Enum("EPalStatusPhysicalHealthType::Normal".to_string()),
        );
        assert!(!is_fainted(&healthy));
        assert!(!is_fainted(&Properties::default()));
    }

    #[test]
    fn human_npc_prefix_rule_matches_the_reference() {
        use std::collections::HashSet;
        let known: HashSet<String> = [
            "Alpaca",
            "Anubis",
            "BadCatgirl",
            "Believer_CrossBow",
            "Boss_Alpaca",
            "Boss_DarkTrader",
            "Female_Soldier01",
            "GrimGirl",
            "GrimGirl_01",
            "Human",
            "Male_DarkTrader01_02",
            "Police_HawkBird",
            "Sheepball",
        ]
        .iter()
        .map(|key| key.to_lowercase())
        .collect();
        let human = human_species_set(&known);
        // Prefix-matched human species — including ones marked as pals in the
        // catalog (BadCatgirl, GrimGirl) and boss variants of them.
        for id in [
            "Human",
            "BadCatgirl",
            "BOSS_BadCatgirl",
            "GrimGirl",
            "BOSS_GrimGirl",
            "Female_Soldier01",
            "Believer_CrossBow",
            "Police_HawkBird",
        ] {
            assert!(is_human_npc(id, &human), "{id} should be human");
        }
        // Base forms absent from the catalog never classify as human — the
        // same variant quirk the reference set has (BOSS_DarkTrader is not a
        // known catalog base, so it is a creature there too).
        assert!(!is_human_npc("BOSS_DarkTrader", &human));
        // Non-human NPCs and creature pals never classify as human.
        for id in [
            "MobuCitizen",
            "PalDealer",
            "GrassBoss",
            "Guard_Rifle",
            "Reward_Paldex",
            "Reward_PalDisplay_A_01",
            "Alpaca",
            "BOSS_Alpaca",
            "Anubis",
            "",
        ] {
            assert!(!is_human_npc(id, &human), "{id} should not be human");
        }
        // The derived set itself: catalog members only.
        assert_eq!(human.len(), 8);
    }

    #[test]
    fn canonical_character_key_prefers_catalog_casing() {
        let game_data = game_data();
        assert_eq!(canonical_character_key("BOSS_Alpaca", &game_data), "Alpaca");
        assert_eq!(canonical_character_key("NotAPal", &game_data), "notapal");
    }
}
