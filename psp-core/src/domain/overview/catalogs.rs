//! Game-data catalogs the overview resolves against: species validity and
//! classification, skill catalogs for the legality checks, and the HP-formula
//! inputs. Built once per `overview_stats` call — a few thousand small inserts,
//! negligible next to the character-map pass.

use std::collections::{HashMap, HashSet};

use crate::gamedata::GameData;

use super::classify::strip_boss_prefix;

/// Friendship-point → rank ladder used by the HP formula when
/// `friendship.json` is unavailable. Index == rank (0..10).
const FRIENDSHIP_THRESHOLDS: [i64; 11] = [
    0, 6_000, 13_000, 21_000, 30_000, 40_000, 55_000, 80_000, 110_000, 150_000, 200_000,
];

/// Per-species numbers the HP ceiling formula needs, resolved from
/// `pals.json`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SpeciesVitals {
    pub(crate) scaling_hp: f64,
    pub(crate) friendship_hp: f64,
}

pub(crate) struct OverviewCatalogs {
    /// Lowercased `pals.json` keys (every known species, pal or human).
    known_species: HashSet<String>,
    /// Lowercased human-NPC ids: `known_species` members matching the human
    /// asset-prefix rule (plus the exact `Human` id), like the reference's
    /// characters.json-derived set.
    human_species: HashSet<String>,
    /// Lowercased passive asset ids.
    passive_assets: HashSet<String>,
    /// Lowercased active-skill ids in BOTH stored (`epalwazaid::x`) and bare
    /// (`x`) forms, so whichever shape a save carries compares equal.
    active_assets: HashSet<String>,
    /// Lowercased passive id → summed MaxHP% effects as a fraction (0.10).
    passive_hp_fraction: HashMap<String, f64>,
    /// Lowercased species key → HP formula inputs.
    vitals: HashMap<String, SpeciesVitals>,
    /// Ascending friendship-point thresholds; index == rank.
    friendship_thresholds: Vec<i64>,
}

impl OverviewCatalogs {
    pub(crate) fn from_game_data(game_data: &GameData) -> Self {
        let mut catalogs = OverviewCatalogs {
            known_species: HashSet::new(),
            human_species: HashSet::new(),
            passive_assets: HashSet::new(),
            active_assets: HashSet::new(),
            passive_hp_fraction: HashMap::new(),
            vitals: HashMap::new(),
            friendship_thresholds: FRIENDSHIP_THRESHOLDS.to_vec(),
        };

        if let Some(pals) = game_data.get("pals").and_then(|value| value.as_object()) {
            for (key, info) in pals {
                let lower = key.to_lowercase();
                catalogs.known_species.insert(lower.clone());
                let scaling_hp = info
                    .pointer("/scaling/hp")
                    .and_then(|value| value.as_f64())
                    .unwrap_or(0.0);
                if scaling_hp > 0.0 {
                    catalogs.vitals.insert(
                        lower,
                        SpeciesVitals {
                            scaling_hp,
                            friendship_hp: info
                                .get("friendship_hp")
                                .and_then(|value| value.as_f64())
                                .unwrap_or(0.0),
                        },
                    );
                }
            }
            catalogs.human_species =
                crate::domain::overview::classify::human_species_set(&catalogs.known_species);
        }

        if let Some(passives) = game_data
            .get("passive_skills")
            .and_then(|value| value.as_object())
        {
            for (key, info) in passives {
                let lower = key.to_lowercase();
                catalogs.passive_assets.insert(lower.clone());
                let mut bonus_pct = 0.0;
                for effect in info
                    .get("effects")
                    .and_then(|value| value.as_array())
                    .unwrap_or(&Vec::new())
                {
                    let effect_type = effect.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    let target = effect.get("target").and_then(|v| v.as_str()).unwrap_or("");
                    // Trainer-only effects never raise the pal's own HP pool.
                    if target.contains("ToTrainer") && !target.contains("ToSelf") {
                        continue;
                    }
                    if effect_type.contains("MaxHP") {
                        bonus_pct += effect.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    }
                }
                catalogs
                    .passive_hp_fraction
                    .insert(lower, bonus_pct / 100.0);
            }
        }

        if let Some(actives) = game_data
            .get("active_skills")
            .and_then(|value| value.as_object())
        {
            for key in actives.keys() {
                let lower = key.to_lowercase();
                catalogs.active_assets.insert(lower.clone());
                if let Some(bare) = lower.strip_prefix("epalwazaid::") {
                    catalogs.active_assets.insert(bare.to_string());
                }
            }
        }

        if let Some(friendship) = game_data
            .get("friendship")
            .and_then(|value| value.as_object())
        {
            let mut thresholds: Vec<i64> = friendship
                .values()
                .filter_map(|entry| entry.get("required_point").and_then(|v| v.as_i64()))
                .collect();
            if !thresholds.is_empty() {
                thresholds.sort_unstable();
                catalogs.friendship_thresholds = thresholds;
            }
        }

        catalogs
    }

    /// A CharacterID resolves when either it or its boss-stripped base form is
    /// a known species (boss variants that are their own catalog entry win).
    pub(crate) fn species_known(&self, character_id: &str) -> bool {
        self.known_species.contains(&character_id.to_lowercase())
            || self
                .known_species
                .contains(&strip_boss_prefix(character_id).to_lowercase())
    }

    /// Human-NPC classification against the catalog-derived set (see
    /// [`super::classify::human_species_set`]).
    pub(crate) fn is_human_npc(&self, character_id: &str) -> bool {
        crate::domain::overview::classify::is_human_npc(character_id, &self.human_species)
    }

    /// Raw id first (boss variant scaling), boss-stripped base second.
    pub(crate) fn vitals_for(&self, character_id: &str) -> Option<SpeciesVitals> {
        let stripped = strip_boss_prefix(character_id).to_lowercase();
        self.vitals
            .get(&character_id.to_lowercase())
            .or_else(|| self.vitals.get(&stripped))
            .copied()
    }

    /// Highest rank whose threshold ≤ `point`, from the data ladder when
    /// `friendship.json` is present and the builtin one otherwise.
    pub(crate) fn friendship_rank(&self, point: i64) -> i64 {
        let mut rank = 0;
        for (index, threshold) in self.friendship_thresholds.iter().enumerate() {
            if point >= *threshold {
                rank = index as i64;
            } else {
                break;
            }
        }
        rank
    }

    pub(crate) fn passive_hp_fraction(&self, passive: &str) -> Option<f64> {
        self.passive_hp_fraction
            .get(&passive.to_lowercase())
            .copied()
    }

    pub(crate) fn has_passive(&self, passive: &str) -> bool {
        self.passive_assets.contains(&passive.to_lowercase())
    }

    pub(crate) fn passives_loaded(&self) -> bool {
        !self.passive_assets.is_empty()
    }

    pub(crate) fn has_active(&self, active: &str) -> bool {
        self.active_assets.contains(&active.to_lowercase())
    }

    pub(crate) fn actives_loaded(&self) -> bool {
        !self.active_assets.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game_data() -> GameData {
        GameData::from_entries([
            (
                "pals".to_string(),
                r#"{
                    "Alpaca": {"is_pal": true, "scaling": {"hp": 90}, "friendship_hp": 4.5},
                    "Human": {"is_pal": false, "scaling": {"hp": 70}, "friendship_hp": 1.0}
                }"#.to_string(),
            ),
            (
                "passive_skills".to_string(),
                r#"{"Legend": {"effects": [{"type": "MaxHP", "value": 20.0, "target": "ToSelf"}]}}"#.to_string(),
            ),
            (
                "active_skills".to_string(),
                r#"{"EPalWazaID::AirCanon": {"power": 40}}"#.to_string(),
            ),
            (
                "friendship".to_string(),
                r#"{
                    "Friendship_Rank_0": {"rank": 0, "required_point": 0},
                    "Friendship_Rank_1": {"rank": 1, "required_point": 6000},
                    "Friendship_Rank_2": {"rank": 2, "required_point": 13000}
                }"#.to_string(),
            ),
        ])
        .unwrap()
    }

    #[test]
    fn species_resolution_accepts_boss_and_base_forms() {
        let catalogs = OverviewCatalogs::from_game_data(&game_data());
        assert!(catalogs.species_known("Alpaca"));
        assert!(catalogs.species_known("alpaca"));
        assert!(catalogs.species_known("BOSS_Alpaca"));
        assert!(!catalogs.species_known("NotAPal"));
        assert!(!catalogs.species_known(""));
    }

    #[test]
    fn vitals_prefer_the_exact_variant_over_the_base_form() {
        let game_data = GameData::from_entries([(
            "pals".to_string(),
            r#"{
                "Alpaca": {"is_pal": true, "scaling": {"hp": 90}, "friendship_hp": 4.5},
                "BOSS_Alpaca": {"is_pal": true, "scaling": {"hp": 500}, "friendship_hp": 4.5}
            }"#
            .to_string(),
        )])
        .unwrap();
        let catalogs = OverviewCatalogs::from_game_data(&game_data);
        assert_eq!(
            catalogs.vitals_for("BOSS_Alpaca").unwrap().scaling_hp,
            500.0
        );
        assert_eq!(catalogs.vitals_for("Alpaca").unwrap().scaling_hp, 90.0);
        assert!(catalogs.vitals_for("NotAPal").is_none());
    }

    #[test]
    fn friendship_rank_uses_the_data_ladder() {
        let catalogs = OverviewCatalogs::from_game_data(&game_data());
        assert_eq!(catalogs.friendship_rank(0), 0);
        assert_eq!(catalogs.friendship_rank(5_999), 0);
        assert_eq!(catalogs.friendship_rank(6_000), 1);
        assert_eq!(catalogs.friendship_rank(500_000), 2);
    }

    #[test]
    fn friendship_rank_falls_back_to_the_builtin_ladder_without_data() {
        let empty = GameData::from_entries([("empty".to_string(), "{}".to_string())]).unwrap();
        let catalogs = OverviewCatalogs::from_game_data(&empty);
        assert_eq!(catalogs.friendship_rank(80_000), 7);
    }

    #[test]
    fn skill_membership_is_case_insensitive_and_form_tolerant() {
        let catalogs = OverviewCatalogs::from_game_data(&game_data());
        assert!(catalogs.has_passive("legend"));
        assert!(catalogs.has_active("EPalWazaID::AirCanon"));
        assert!(catalogs.has_active("AirCanon"));
        assert!(!catalogs.has_active("HackSkill"));
        assert_eq!(catalogs.passive_hp_fraction("Legend"), Some(0.20));
    }
}
