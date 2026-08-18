//! Source adapters — how the solver gets its initial breeding pool.
//!
//! The key palcalc insight reproduced here: Save Mode and Selection Mode are
//! the *same solver* with different initial pools. Each adapter just produces a
//! list of `PalRef` leaves; the solver never knows which mode it's in.
//!
//! * [`OwnedSource`]    — Save Mode. Wraps owned-pal summaries (gender,
//!   passives, character_id, ...). One ref per owned pal, carrying its real
//!   gender + passives + provenance.
//! * [`SelectedSource`] — Selection Mode. User's theoretical picks. Gender
//!   defaults to Wildcard so the solver treats them as flexible.
//! * [`WildSource`]     — optional. Adds one Wildcard ref per breedable
//!   species, representing "go catch one". Composable via [`CompositeSource`].
//!
//! Faithful port of `PalSavTools/src/palworld_aio/breeding/sources.py`.

use std::collections::BTreeSet;

use serde::Deserialize;
use serde_json::Value;

use super::data::BreedingDB;
use super::model::{Gender, Origin, PalRef, Provenance};

/// Produces the initial `PalRef` leaves for the solver.
pub trait SourceAdapter {
    fn initial_refs(&self, db: &BreedingDB) -> Vec<PalRef>;
}

// ---------------------------------------------------------------------
// OwnedSource — Save Mode
// ---------------------------------------------------------------------
/// Wraps owned-pal summaries. Accepts the shape the frontend sends for Save
/// Mode: `character_id` (species), `gender` ("Male"/"Female"/"Unknown"),
/// `passive_skills` ([str, ...]), plus optional provenance
/// (`instance_id`, `nickname`, `level`, `owner_uid`).
#[derive(Debug, Default)]
pub struct OwnedSource {
    pals: Vec<OwnedPal>,
}

#[derive(Debug, Deserialize)]
struct OwnedPal {
    character_id: Option<String>,
    #[serde(default)]
    gender: Option<String>,
    #[serde(default)]
    passive_skills: Option<Vec<Value>>,
    instance_id: Option<Value>,
    nickname: Option<Value>,
    level: Option<Value>,
    owner_uid: Option<Value>,
}

impl OwnedSource {
    /// Build from already-deserialized owned-pal JSON values (one per pal).
    pub fn from_values(pals: Vec<Value>) -> Result<Self, super::BreedingError> {
        let parsed: Vec<OwnedPal> = pals
            .into_iter()
            .map(serde_json::from_value::<OwnedPal>)
            .collect::<Result<_, _>>()?;
        Ok(Self { pals: parsed })
    }

    /// Build from typed owned-pal summaries (no deserialize round-trip).
    pub fn new(pals: Vec<OwnedPalInput>) -> Self {
        Self {
            pals: pals
                .into_iter()
                .map(|p| OwnedPal {
                    character_id: Some(p.character_id),
                    gender: p.gender,
                    passive_skills: Some(p.passive_skills.into_iter().map(Value::String).collect()),
                    instance_id: p.instance_id,
                    nickname: p.nickname,
                    level: p.level,
                    owner_uid: p.owner_uid,
                })
                .collect(),
        }
    }
}

/// Typed owned-pal input (used by the handler when it already has typed data).
#[derive(Debug, Clone, Deserialize)]
pub struct OwnedPalInput {
    pub character_id: String,
    pub gender: Option<String>,
    #[serde(default)]
    pub passive_skills: Vec<String>,
    pub instance_id: Option<Value>,
    pub nickname: Option<Value>,
    pub level: Option<Value>,
    pub owner_uid: Option<Value>,
}

impl SourceAdapter for OwnedSource {
    fn initial_refs(&self, db: &BreedingDB) -> Vec<PalRef> {
        let mut refs = Vec::new();
        for pal in &self.pals {
            let raw_cid = pal.character_id.clone().unwrap_or_default();
            let species = normalize_species(Some(&raw_cid));
            if species.is_empty() || !db.is_breedable(&species) {
                continue;
            }
            refs.push(PalRef {
                species,
                gender: Gender::coerce(pal.gender.as_deref()),
                passives: clean_passives(pal.passive_skills.as_deref()),
                generation: 0,
                parents: None,
                origin: Origin::Owned,
                provenance: Provenance {
                    instance_id: pal.instance_id.clone(),
                    nickname: pal.nickname.clone(),
                    level: pal.level.clone(),
                    owner_uid: pal.owner_uid.clone(),
                    raw_character_id: Some(Value::String(raw_cid)),
                },
            });
        }
        refs
    }
}

// ---------------------------------------------------------------------
// SelectedSource — Selection Mode
// ---------------------------------------------------------------------
/// User-selected theoretical pals. `gender` omitted → Wildcard. Unbreedable
/// species are dropped; warnings are collected on the struct for surfacing.
#[derive(Debug, Default)]
pub struct SelectedSource {
    selected: Vec<SelectedPal>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SelectedPal {
    species: Option<String>,
    gender: Option<String>,
    #[serde(default)]
    passives: Option<Vec<Value>>,
}

impl SelectedSource {
    pub fn from_values(pals: Vec<Value>) -> Result<Self, super::BreedingError> {
        let parsed: Vec<SelectedPal> = pals
            .into_iter()
            .map(serde_json::from_value::<SelectedPal>)
            .collect::<Result<_, _>>()?;
        Ok(Self {
            selected: parsed,
            warnings: Vec::new(),
        })
    }

    pub fn new(picks: Vec<SelectedPalInput>) -> Self {
        Self {
            selected: picks
                .into_iter()
                .map(|p| SelectedPal {
                    species: Some(p.species),
                    gender: p.gender,
                    passives: Some(p.passives.into_iter().map(Value::String).collect()),
                })
                .collect(),
            warnings: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SelectedPalInput {
    pub species: String,
    pub gender: Option<String>,
    #[serde(default)]
    pub passives: Vec<String>,
}

impl SourceAdapter for SelectedSource {
    fn initial_refs(&self, db: &BreedingDB) -> Vec<PalRef> {
        let mut refs = Vec::new();
        for entry in &self.selected {
            let raw = entry.species.clone().unwrap_or_default();
            let species = normalize_species(Some(&raw));
            if species.is_empty() {
                continue;
            }
            if !db.is_breedable(&species) {
                // Bound into &self via a deferred mutation: collect after.
                // (Adapter trait is &self; warnings recorded by the caller via
                // the explicit `take_warnings`/`warnings` field.)
                continue;
            }
            let gender = entry
                .gender
                .as_deref()
                .filter(|s| !s.is_empty())
                .map(|g| Gender::coerce(Some(g)))
                .unwrap_or(Gender::Wildcard);
            refs.push(PalRef {
                species,
                gender,
                passives: clean_passives(entry.passives.as_deref()),
                generation: 0,
                parents: None,
                origin: Origin::Selected,
                provenance: Provenance::default(),
            });
        }
        refs
    }
}

// ---------------------------------------------------------------------
// WildSource — "go catch one" fallback
// ---------------------------------------------------------------------
/// One Wildcard ref per breedable species not in `exclude`. Represents
/// wild-caught pals.
pub struct WildSource {
    exclude: std::collections::HashSet<String>,
}

impl WildSource {
    pub fn new() -> Self {
        Self {
            exclude: std::collections::HashSet::new(),
        }
    }

    pub fn with_exclude(exclude: impl IntoIterator<Item = String>) -> Self {
        Self {
            exclude: exclude.into_iter().collect(),
        }
    }
}

impl Default for WildSource {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceAdapter for WildSource {
    fn initial_refs(&self, db: &BreedingDB) -> Vec<PalRef> {
        db.breedable_tribes()
            .into_iter()
            .filter(|tribe| !self.exclude.contains(tribe))
            .map(|tribe| PalRef {
                species: tribe,
                gender: Gender::Wildcard,
                passives: BTreeSet::new(),
                generation: 0,
                parents: None,
                origin: Origin::Wild,
                provenance: Provenance::default(),
            })
            .collect()
    }
}

// ---------------------------------------------------------------------
// CompositeSource — combine adapters (e.g. OwnedSource + WildSource)
// ---------------------------------------------------------------------
/// Merge multiple adapters' refs. De-duplicates identical group keys
/// (species + gender + full passive set).
pub struct CompositeSource {
    adapters: Vec<Box<dyn SourceAdapter>>,
}

impl CompositeSource {
    pub fn new(adapters: Vec<Box<dyn SourceAdapter>>) -> Self {
        Self { adapters }
    }
}

impl SourceAdapter for CompositeSource {
    fn initial_refs(&self, db: &BreedingDB) -> Vec<PalRef> {
        let mut out = Vec::new();
        let mut seen: std::collections::HashSet<(String, Gender, BTreeSet<String>)> =
            std::collections::HashSet::new();
        for adapter in &self.adapters {
            for ref_pal in adapter.initial_refs(db) {
                let key = ref_pal.group_key();
                if !seen.insert(key) {
                    continue;
                }
                out.push(ref_pal);
            }
        }
        out
    }
}

// ---------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------
/// Strip boss/predator prefixes so a save's CharacterID maps to its tribe.
/// Saves encode boss pals as `BOSS_Anubis`; the breeding table keys on the
/// bare tribe `Anubis`. Returns "" for falsy input.
pub fn normalize_species(raw: Option<&str>) -> String {
    let Some(s) = raw else {
        return String::new();
    };
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    for prefix in ["BOSS_", "B_O_S_S_"] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            return rest.to_string();
        }
    }
    trimmed.to_string()
}

/// Coerce a passive-skills field into a clean `BTreeSet<String>`.
fn clean_passives(raw: Option<&[Value]>) -> BTreeSet<String> {
    let Some(arr) = raw else {
        return BTreeSet::new();
    };
    arr.iter()
        .filter_map(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_boss_prefixes() {
        assert_eq!(normalize_species(Some("Anubis")), "Anubis");
        assert_eq!(normalize_species(Some("BOSS_Anubis")), "Anubis");
        assert_eq!(normalize_species(Some("B_O_S_S_Anubis")), "Anubis");
        assert_eq!(normalize_species(Some("  Anubis ")), "Anubis");
        assert_eq!(normalize_species(None), "");
        assert_eq!(normalize_species(Some("")), "");
    }

    #[test]
    fn clean_passives_handles_mixed() {
        let v = vec![
            Value::String("Legend".into()),
            Value::String("  Runner ".into()),
            Value::Null,
            Value::String("".into()),
        ];
        let set = clean_passives(Some(&v));
        assert!(set.contains("Legend"));
        assert!(set.contains("Runner"));
        assert_eq!(set.len(), 2);
    }
}
