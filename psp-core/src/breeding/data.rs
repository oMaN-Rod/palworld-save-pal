//! Loads and indexes the breeding data files.
//!
//! Reads the three breeding JSONs (sourced into `data/json/` and surfaced via
//! `GameData`) and builds the lookup indexes the rest of the engine needs.
//! Pure data access — no solver logic.
//!
//! Faithful port of `PalSavTools/src/palworld_aio/breeding/data.py`.
//!
//! Design note on the forward index: `child_to_parents_formula` is keyed
//! child→[parent pairs]. We invert it once into `pair_to_child` (a sorted
//! (a,b) → child map) for O(1) "A+B → ?" lookups, which both Direct Mode and
//! the solver need constantly. `child_to_parents` stays as-is for the reverse
//! Direct-Mode lookup. Unique combos overwrite formula results (a given pair
//! may appear in both; unique wins, mirroring the game's `DT_PalCombiUnique`
//! precedence).

use std::collections::HashMap;

use serde::Deserialize;

use crate::gamedata::GameData;

use super::model::{ComboType, DirectResult, GenderProb};
use super::BreedingError;

// ---------------------------------------------------------------------
// raw JSON shapes
// ---------------------------------------------------------------------
#[derive(Debug, Deserialize)]
struct BreedingFile {
    pal_info: HashMap<String, PalInfo>,
    unique_combos: Vec<RawCombo>,
    child_to_parents_formula: HashMap<String, Vec<RawPair>>,
    #[serde(default)]
    child_to_parents_unique: HashMap<String, Vec<RawPair>>,
}

#[derive(Debug, Deserialize)]
struct PalInfo {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    combi_rank: Option<i64>,
    #[serde(default)]
    rarity: Option<i64>,
    #[allow(dead_code)]
    #[serde(default)]
    ignore_combi: bool,
    #[serde(default)]
    icon: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawCombo {
    parent_a: String,
    parent_b: String,
    child: String,
}

#[derive(Debug, Deserialize, Clone)]
struct RawPair {
    parent_a: String,
    parent_b: String,
}

#[derive(Debug, Deserialize, Default)]
struct MetaFile {
    #[serde(default)]
    gender_prob: HashMap<String, GenderProb>,
    #[allow(dead_code)]
    #[serde(default)]
    breedable_genders: HashMap<String, String>,
    #[serde(default)]
    display_names: HashMap<String, String>,
}

impl Default for GenderProb {
    fn default() -> Self {
        Self {
            male: 0.5,
            female: 0.5,
        }
    }
}

/// Sorted (min, max) pair key — order-independent, `Hash + Eq`.
fn pair_key(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

// ---------------------------------------------------------------------
// BreedingDB
// ---------------------------------------------------------------------
/// Indexed breeding data. Construct via [`BreedingDB::from_game_data`].
pub struct BreedingDB {
    pal_info: HashMap<String, PalInfo>,
    /// `unique_combos` reindexed by child, for `_is_unique_combo`.
    child_to_parents_unique: HashMap<String, Vec<RawPair>>,
    display_names: HashMap<String, String>,
    gender_prob: HashMap<String, GenderProb>,
    /// Sorted-(a,b) → child. Unique combos override formula results.
    pair_to_child: HashMap<(String, String), String>,
    /// child → deduped, sorted parent pairs (unique-first, then formula).
    child_to_parents_merged: HashMap<String, Vec<(String, String)>>,
    min_steps: HashMap<String, HashMap<String, i64>>,
}

impl BreedingDB {
    /// Build the indexes from the three breeding JSONs as carried by
    /// `GameData`. Keys: `breeding`, `breeding_meta`, `breeding_distance`.
    pub fn from_game_data(game_data: &GameData) -> Result<Self, super::BreedingError> {
        let breeding = game_data
            .get("breeding")
            .ok_or(BreedingError::MissingData("breeding".to_string()))?;
        let meta = game_data
            .get("breeding_meta")
            .ok_or(BreedingError::MissingData("breeding_meta".to_string()))?;
        let distance = game_data
            .get("breeding_distance")
            .ok_or(BreedingError::MissingData("breeding_distance".to_string()))?;

        let breeding_file: BreedingFile = serde_json::from_value(breeding.clone())?;
        let meta_file: MetaFile = serde_json::from_value(meta.clone())?;
        let min_steps: HashMap<String, HashMap<String, i64>> =
            serde_json::from_value(distance.clone())?;

        // pair_to_child: formula first, then unique overwrites (game precedence).
        let mut pair_to_child: HashMap<(String, String), String> = HashMap::new();
        for (child, pairs) in &breeding_file.child_to_parents_formula {
            for pair in pairs {
                pair_to_child.insert(pair_key(&pair.parent_a, &pair.parent_b), child.clone());
            }
        }
        for combo in &breeding_file.unique_combos {
            pair_to_child.insert(
                pair_key(&combo.parent_a, &combo.parent_b),
                combo.child.clone(),
            );
        }

        // child_to_parents_merged: unique-first, symmetric dedupe.
        let mut merged: HashMap<String, Vec<(String, String)>> = HashMap::new();
        let mut seen: HashMap<String, std::collections::HashSet<(String, String)>> =
            HashMap::new();
        for src in [
            &breeding_file.child_to_parents_unique,
            &breeding_file.child_to_parents_formula,
        ] {
            for (child, pairs) in src {
                let bucket_seen = seen.entry(child.clone()).or_default();
                let bucket = merged.entry(child.clone()).or_default();
                for pair in pairs {
                    let key = pair_key(&pair.parent_a, &pair.parent_b);
                    if bucket_seen.contains(&key) {
                        continue;
                    }
                    bucket_seen.insert(key.clone());
                    bucket.push(key);
                }
            }
        }

        Ok(Self {
            pal_info: breeding_file.pal_info,
            child_to_parents_unique: breeding_file.child_to_parents_unique,
            display_names: meta_file.display_names,
            gender_prob: meta_file.gender_prob,
            pair_to_child,
            child_to_parents_merged: merged,
            min_steps,
        })
    }

    /// A + B → child tribe, or `None` if the pair has no known child.
    /// Order-independent. Same-species pairs resolve to themselves when present
    /// (Alpaca+Alpaca→Alpaca).
    pub fn forward(&self, parent_a: &str, parent_b: &str) -> Option<&str> {
        self.pair_to_child
            .get(&pair_key(parent_a, parent_b))
            .map(String::as_str)
    }

    /// child → all parent pairs (unique + formula), deduped symmetrically.
    pub fn child_to_parents(&self, child: &str) -> &[(String, String)] {
        self.child_to_parents_merged
            .get(child)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Given Parent A + target child, return candidate Parent B tribes.
    pub fn reverse(&self, parent_a: &str, target_child: &str) -> Vec<String> {
        let mut out = Vec::new();
        for (a, b) in self.child_to_parents(target_child) {
            if a == parent_a {
                out.push(b.clone());
            } else if b == parent_a {
                out.push(a.clone());
            }
        }
        out
    }

    /// Raw distance row for `start` (palcalc `MinBreedingSteps`), or `None`.
    pub fn min_steps_row(&self, start: &str) -> Option<&HashMap<String, i64>> {
        self.min_steps.get(start)
    }

    /// True if `start_pal` can reach `target_pal` in ≤ `budget` breeds.
    ///
    /// Same pal → 0 steps (always reachable with budget ≥ 0). Unknown pairs
    /// (absent from the distance map) are treated as unreachable unless
    /// start == target. The `10000` unreachable sentinel naturally fails the
    /// `<= budget` check for any sane budget.
    pub fn reachable(&self, start_pal: &str, target_pal: &str, budget: i64) -> bool {
        if start_pal == target_pal {
            return budget >= 0;
        }
        let Some(row) = self.min_steps.get(start_pal) else {
            return false;
        };
        match row.get(target_pal) {
            Some(steps) => *steps <= budget,
            None => false,
        }
    }

    /// Localized display name, falling back to pal_info name then the tribe.
    pub fn display_name(&self, tribe: &str) -> String {
        if let Some(d) = self.display_names.get(tribe) {
            return d.clone();
        }
        if let Some(info) = self.pal_info.get(tribe) {
            if let Some(name) = &info.name {
                return name.clone();
            }
        }
        tribe.to_string()
    }

    pub fn icon_path(&self, tribe: &str) -> Option<String> {
        self.pal_info.get(tribe).and_then(|i| i.icon.clone())
    }

    /// Breeding power value (used by the breedable-pal list). `None` if absent.
    pub fn combi_rank(&self, tribe: &str) -> Option<i64> {
        self.pal_info.get(tribe).and_then(|i| i.combi_rank)
    }

    /// Rarity (1-7). `None` if absent.
    pub fn rarity(&self, tribe: &str) -> Option<i64> {
        self.pal_info.get(tribe).and_then(|i| i.rarity)
    }

    /// `{"male": p, "female": q}`; defaults to 50/50 when unknown.
    pub fn gender_probability(&self, tribe: &str) -> GenderProb {
        self.gender_prob
            .get(tribe)
            .cloned()
            .unwrap_or_default()
    }

    /// A pal is breedable if it appears in the combo table at all.
    pub fn is_breedable(&self, tribe: &str) -> bool {
        self.pal_info.contains_key(tribe)
    }

    /// All tribes the UI picker should offer (sorted by display name).
    pub fn breedable_tribes(&self) -> Vec<String> {
        let mut tribes: Vec<String> = self.pal_info.keys().cloned().collect();
        tribes.sort_by(|a, b| {
            self.display_name(a)
                .to_lowercase()
                .cmp(&self.display_name(b).to_lowercase())
        });
        tribes
    }

    /// Is a given (parent_a, parent_b, child) triple a "unique" combo?
    /// Checked by membership in `child_to_parents_unique`.
    pub fn is_unique_combo(&self, parent_a: &str, parent_b: &str, child: &str) -> bool {
        let Some(unique_pairs) = self.child_to_parents_unique.get(child) else {
            return false;
        };
        let key = pair_key(parent_a, parent_b);
        unique_pairs
            .iter()
            .any(|p| pair_key(&p.parent_a, &p.parent_b) == key)
    }

    /// `DirectResult` factory — shared by the forward + reverse Direct-Mode
    /// helpers.
    pub fn direct_result(
        &self,
        parent_a: &str,
        parent_b: &str,
        child: &str,
        combo_type: ComboType,
    ) -> DirectResult {
        DirectResult {
            parent_a: parent_a.to_string(),
            parent_b: parent_b.to_string(),
            child: child.to_string(),
            child_display: Some(self.display_name(child)),
            child_icon: self.icon_path(child),
            child_gender_prob: Some(self.gender_probability(child)),
            combo_type,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_repo_db() -> BreedingDB {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/json");
        let gd = GameData::load(&dir).expect("game data loads");
        BreedingDB::from_game_data(&gd).expect("breeding db builds")
    }

    #[test]
    fn forward_resolves_formula_and_unique_combos() {
        let db = load_repo_db();
        // Formula pair (Alpaca self-breed).
        assert_eq!(db.forward("Alpaca", "Alpaca"), Some("Alpaca"));
        // Unique combo: LazyDragon + ElecCat → LazyDragon_Electric.
        assert_eq!(
            db.forward("LazyDragon", "ElecCat"),
            Some("LazyDragon_Electric")
        );
        // Order-independent.
        assert_eq!(
            db.forward("ElecCat", "LazyDragon"),
            Some("LazyDragon_Electric")
        );
        // Unknown pair → None.
        assert_eq!(db.forward("Alpaca", "DoesNotExist"), None);
    }

    #[test]
    fn unique_combo_override_takes_precedence() {
        let db = load_repo_db();
        // Where a pair is both a formula and unique combo, the unique child
        // wins. LazyDragon_Electric is a unique child of LazyDragon+ElecCat.
        let child = db.forward("LazyDragon", "ElecCat").unwrap();
        assert_eq!(child, "LazyDragon_Electric");
        assert!(db.is_unique_combo("LazyDragon", "ElecCat", child));
    }

    #[test]
    fn breedable_tribes_sorted_and_nonempty() {
        let db = load_repo_db();
        let tribes = db.breedable_tribes();
        assert!(!tribes.is_empty());
        // Sorted by display name.
        let names: Vec<String> = tribes.iter().map(|t| db.display_name(t)).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn reachable_handles_sentinel_and_self() {
        let db = load_repo_db();
        // Self → reachable at budget 0.
        assert!(db.reachable("Alpaca", "Alpaca", 0));
        // A real 1-step neighbour reachable at budget 1 (Anubis breeds into
        // many pals at distance 1 — pick any child it forward-resolves to).
        if let Some(child) = db.forward("Alpaca", "Deer") {
            assert!(
                db.reachable("Alpaca", child, 5),
                "Alpaca should reach its own child {child}"
            );
        }
    }
}
