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

use super::model::{ComboOutcome, ComboType, DirectResult, Gender, GenderProb};
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
    /// `DT_PalCombiUnique.ParentGenderA`; absent/null for the usual
    /// gender-agnostic combo.
    #[serde(default)]
    parent_a_gender: Option<String>,
    #[serde(default)]
    parent_b_gender: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct RawPair {
    parent_a: String,
    parent_b: String,
    #[serde(default)]
    parent_a_gender: Option<String>,
    #[serde(default)]
    parent_b_gender: Option<String>,
}

fn parse_gender(raw: &Option<String>) -> Option<Gender> {
    raw.as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(|s| Gender::coerce(Some(s)))
        .filter(|g| matches!(g, Gender::Male | Gender::Female))
}

/// One way to produce a child: the two parent tribes plus any gender gate.
#[derive(Debug, Clone)]
pub struct ParentPair {
    pub parent_a: String,
    pub parent_b: String,
    pub parent_a_gender: Option<Gender>,
    pub parent_b_gender: Option<Gender>,
    pub combo_type: ComboType,
}

impl ParentPair {
    /// Re-state this pair with `pinned` as `parent_a`, moving its gender gate
    /// with it. Returns `None` when `pinned` is not one of the two parents.
    fn oriented(&self, pinned: &str) -> Option<ParentPair> {
        if self.parent_a == pinned {
            Some(self.clone())
        } else if self.parent_b == pinned {
            Some(ParentPair {
                parent_a: self.parent_b.clone(),
                parent_b: self.parent_a.clone(),
                parent_a_gender: self.parent_b_gender,
                parent_b_gender: self.parent_a_gender,
                combo_type: self.combo_type,
            })
        } else {
            None
        }
    }
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

fn raw_to_pair(raw: &RawPair, combo_type: ComboType) -> ParentPair {
    ParentPair {
        parent_a: raw.parent_a.clone(),
        parent_b: raw.parent_b.clone(),
        parent_a_gender: parse_gender(&raw.parent_a_gender),
        parent_b_gender: parse_gender(&raw.parent_b_gender),
        combo_type,
    }
}

/// Do two pairs impose the same gender gate on the same two tribes? Compared
/// after normalizing parent order, so (A♂,B♀) and (B♀,A♂) are one gate.
fn same_gate(x: &ParentPair, y: &ParentPair) -> bool {
    let norm = |p: &ParentPair| {
        let mut sides = [
            (p.parent_a.clone(), p.parent_a_gender),
            (p.parent_b.clone(), p.parent_b_gender),
        ];
        sides.sort_by(|l, r| {
            l.0.cmp(&r.0)
                .then(format!("{:?}", l.1).cmp(&format!("{:?}", r.1)))
        });
        sides
    };
    norm(x) == norm(y)
}

fn push_pair(bucket: &mut Vec<ParentPair>, pair: ParentPair) {
    if !bucket.iter().any(|p| same_gate(p, &pair)) {
        bucket.push(pair);
    }
}

// ---------------------------------------------------------------------
// BreedingDB
// ---------------------------------------------------------------------
/// Indexed breeding data. Construct via [`BreedingDB::from_game_data`].
pub struct BreedingDB {
    pal_info: HashMap<String, PalInfo>,
    /// `unique_combos` reindexed by child, for `is_unique_combo`.
    child_to_parents_unique: HashMap<String, Vec<RawPair>>,
    display_names: HashMap<String, String>,
    gender_prob: HashMap<String, GenderProb>,
    /// Sorted-(a,b) → the formula child. One entry per pair.
    pair_formula: HashMap<(String, String), String>,
    /// Sorted-(a,b) → every unique combo for that pair. Usually one, but a
    /// gender-gated pair has one entry per gender assignment, so this cannot
    /// collapse to a single child the way a plain map would.
    pair_unique: HashMap<(String, String), Vec<ParentPair2Child>>,
    /// child → deduped parent pairs (unique-first, then formula).
    child_to_parents_merged: HashMap<String, Vec<ParentPair>>,
    min_steps: HashMap<String, HashMap<String, i64>>,
}

/// Internal: a unique combo stored against its sorted pair key.
#[derive(Debug, Clone)]
struct ParentPair2Child {
    pair: ParentPair,
    child: String,
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

        let mut pair_formula: HashMap<(String, String), String> = HashMap::new();
        for (child, pairs) in &breeding_file.child_to_parents_formula {
            for pair in pairs {
                pair_formula.insert(pair_key(&pair.parent_a, &pair.parent_b), child.clone());
            }
        }

        // Unique combos are collected per pair rather than overwriting, so a
        // gender-gated pair keeps both of its children.
        let mut pair_unique: HashMap<(String, String), Vec<ParentPair2Child>> = HashMap::new();
        for combo in &breeding_file.unique_combos {
            let entry = ParentPair2Child {
                pair: ParentPair {
                    parent_a: combo.parent_a.clone(),
                    parent_b: combo.parent_b.clone(),
                    parent_a_gender: parse_gender(&combo.parent_a_gender),
                    parent_b_gender: parse_gender(&combo.parent_b_gender),
                    combo_type: ComboType::Unique,
                },
                child: combo.child.clone(),
            };
            let bucket = pair_unique
                .entry(pair_key(&combo.parent_a, &combo.parent_b))
                .or_default();
            if !bucket
                .iter()
                .any(|e| e.child == entry.child && same_gate(&e.pair, &entry.pair))
            {
                bucket.push(entry);
            }
        }

        // child_to_parents_merged: unique-first, symmetric dedupe. The gender
        // gate is part of the identity, so both CatMage/FoxMage rows survive.
        let mut merged: HashMap<String, Vec<ParentPair>> = HashMap::new();
        for (child, pairs) in &breeding_file.child_to_parents_unique {
            let bucket = merged.entry(child.clone()).or_default();
            for pair in pairs {
                push_pair(bucket, raw_to_pair(pair, ComboType::Unique));
            }
        }
        for (child, pairs) in &breeding_file.child_to_parents_formula {
            let bucket = merged.entry(child.clone()).or_default();
            for pair in pairs {
                push_pair(bucket, raw_to_pair(pair, ComboType::Formula));
            }
        }

        Ok(Self {
            pal_info: breeding_file.pal_info,
            child_to_parents_unique: breeding_file.child_to_parents_unique,
            display_names: meta_file.display_names,
            gender_prob: meta_file.gender_prob,
            pair_formula,
            pair_unique,
            child_to_parents_merged: merged,
            min_steps,
        })
    }

    /// Every child a pair can produce, with the gender gate (if any) that
    /// selects it. Genders are stated relative to the queried `parent_a`/
    /// `parent_b` order.
    ///
    /// Precedence mirrors the game:
    /// 1. **Same species breeds true.** Two pals of one species always yield
    ///    that species. This is not in the combo tables — the rank formula
    ///    would otherwise hand back a rank-neighbour for the elemental
    ///    variants, and nothing at all for `IgnoreCombi` legendaries.
    /// 2. `DT_PalCombiUnique` entries for the pair.
    /// 3. The rank formula.
    pub fn forward_all(&self, parent_a: &str, parent_b: &str) -> Vec<ComboOutcome> {
        if parent_a == parent_b {
            return vec![ComboOutcome {
                child: parent_a.to_string(),
                parent_a_gender: None,
                parent_b_gender: None,
                combo_type: ComboType::Formula,
            }];
        }
        let key = pair_key(parent_a, parent_b);
        if let Some(entries) = self.pair_unique.get(&key) {
            let mut out = Vec::with_capacity(entries.len());
            for entry in entries {
                let Some(oriented) = entry.pair.oriented(parent_a) else {
                    continue;
                };
                out.push(ComboOutcome {
                    child: entry.child.clone(),
                    parent_a_gender: oriented.parent_a_gender,
                    parent_b_gender: oriented.parent_b_gender,
                    combo_type: ComboType::Unique,
                });
            }
            if !out.is_empty() {
                return out;
            }
        }
        self.pair_formula
            .get(&key)
            .map(|child| {
                vec![ComboOutcome {
                    child: child.clone(),
                    parent_a_gender: None,
                    parent_b_gender: None,
                    combo_type: ComboType::Formula,
                }]
            })
            .unwrap_or_default()
    }

    /// Outcomes reachable given concrete parent genders. `Wildcard`/`Unknown`
    /// keeps every branch — an unresolved pal could still turn out either way.
    pub fn forward_gendered(
        &self,
        parent_a: &str,
        gender_a: Gender,
        parent_b: &str,
        gender_b: Gender,
    ) -> Vec<ComboOutcome> {
        self.forward_all(parent_a, parent_b)
            .into_iter()
            .filter(|o| o.admits(gender_a, gender_b))
            .collect()
    }

    /// A + B → child tribe, ignoring gender gates. Returns the first outcome,
    /// so a gender-gated pair reports only one of its two children — prefer
    /// [`Self::forward_all`] anywhere both matter.
    pub fn forward(&self, parent_a: &str, parent_b: &str) -> Option<String> {
        self.forward_all(parent_a, parent_b)
            .into_iter()
            .next()
            .map(|o| o.child)
    }

    /// child → all parent pairs (unique + formula), deduped symmetrically.
    pub fn child_to_parents(&self, child: &str) -> &[ParentPair] {
        self.child_to_parents_merged
            .get(child)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Given Parent A + target child, return the candidate pairs re-stated with
    /// `parent_a` pinned to the first slot (so any gender gate travels with it).
    pub fn reverse(&self, parent_a: &str, target_child: &str) -> Vec<ParentPair> {
        self.child_to_parents(target_child)
            .iter()
            .filter_map(|p| p.oriented(parent_a))
            .collect()
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
        self.gender_prob.get(tribe).cloned().unwrap_or_default()
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
    /// helpers. `gender_a`/`gender_b` carry a unique combo's gender gate, if any.
    pub fn direct_result(
        &self,
        parent_a: &str,
        parent_b: &str,
        child: &str,
        combo_type: ComboType,
        gender_a: Option<Gender>,
        gender_b: Option<Gender>,
    ) -> DirectResult {
        DirectResult {
            parent_a: parent_a.to_string(),
            parent_b: parent_b.to_string(),
            child: child.to_string(),
            child_display: Some(self.display_name(child)),
            child_icon: self.icon_path(child),
            child_gender_prob: Some(self.gender_probability(child)),
            combo_type,
            parent_a_gender: gender_a,
            parent_b_gender: gender_b,
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
        assert_eq!(db.forward("Alpaca", "Alpaca").as_deref(), Some("Alpaca"));
        // Unique combo: LazyDragon + ElecCat → LazyDragon_Electric.
        assert_eq!(
            db.forward("LazyDragon", "ElecCat").as_deref(),
            Some("LazyDragon_Electric")
        );
        // Order-independent.
        assert_eq!(
            db.forward("ElecCat", "LazyDragon").as_deref(),
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
        assert!(db.is_unique_combo("LazyDragon", "ElecCat", &child));
    }

    /// Two pals of the same species always yield that species. The combo tables
    /// do not encode this for the elemental variants (excluded from the formula
    /// result pool) or for `IgnoreCombi` legendaries, so the engine supplies it.
    #[test]
    fn same_species_breeds_true() {
        let db = load_repo_db();
        for tribe in db.breedable_tribes() {
            assert_eq!(
                db.forward(&tribe, &tribe).as_deref(),
                Some(tribe.as_str()),
                "{tribe} + {tribe} must yield {tribe}"
            );
        }
    }

    /// `DT_PalCombiUnique` gates CatMage + FoxMage on parent gender: the male
    /// CatMage line yields FoxMage_Dark, the female one CatMage_Fire. Both must
    /// survive — a pair-keyed map would drop one.
    #[test]
    fn gender_gated_unique_combo_keeps_both_children() {
        let db = load_repo_db();
        let all = db.forward_all("CatMage", "FoxMage");
        let mut kids: Vec<&str> = all.iter().map(|o| o.child.as_str()).collect();
        kids.sort_unstable();
        assert_eq!(kids, ["CatMage_Fire", "FoxMage_Dark"]);

        // Pinning genders selects exactly one.
        let male_cat = db.forward_gendered("CatMage", Gender::Male, "FoxMage", Gender::Female);
        assert_eq!(male_cat.len(), 1);
        assert_eq!(male_cat[0].child, "FoxMage_Dark");

        let female_cat = db.forward_gendered("CatMage", Gender::Female, "FoxMage", Gender::Male);
        assert_eq!(female_cat.len(), 1);
        assert_eq!(female_cat[0].child, "CatMage_Fire");

        // Reversing the query order moves the gate to the other slot.
        let flipped = db.forward_gendered("FoxMage", Gender::Female, "CatMage", Gender::Male);
        assert_eq!(flipped.len(), 1);
        assert_eq!(flipped[0].child, "FoxMage_Dark");

        // An unresolved gender keeps both branches alive.
        assert_eq!(
            db.forward_gendered("CatMage", Gender::Wildcard, "FoxMage", Gender::Wildcard)
                .len(),
            2
        );
    }

    /// Regression: the generator once read tribe names from the `EPalTribeID`
    /// enum, which spells Fuack "Blueplatypus" while its CharacterID (and every
    /// save file) uses "BluePlatypus". Save Mode silently dropped every Fuack.
    #[test]
    fn species_keys_use_character_id_casing() {
        let db = load_repo_db();
        assert!(db.is_breedable("BluePlatypus"));
        assert!(!db.is_breedable("Blueplatypus"));
        assert!(db.min_steps_row("BluePlatypus").is_some());
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
                db.reachable("Alpaca", &child, 5),
                "Alpaca should reach its own child {child}"
            );
        }
    }
}
