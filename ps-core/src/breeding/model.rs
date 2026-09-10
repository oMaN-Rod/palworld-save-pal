//! Core data types for the breeding engine: plain immutable types, no I/O and no
//! save deps. Kept tiny so the wire format (serde here, TS in the frontend)
//! mirrors it 1:1.

use std::collections::BTreeSet;
use std::sync::Arc;

use serde_json::Value;

/// `Wildcard` means "could be either" — bred children start there until forced to
/// a concrete gender by a target spec. Wire values are title-case
/// ("Male"/"Female"/"Wildcard"/"Unknown"), matching the frontend `PalGender`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Gender {
    #[serde(rename = "Male")]
    Male,
    #[serde(rename = "Female")]
    Female,
    #[serde(rename = "Wildcard")]
    Wildcard,
    #[serde(rename = "Unknown")]
    Unknown,
}

impl Gender {
    /// The title-case wire string (equals the serde serialization).
    pub fn as_value(self) -> &'static str {
        match self {
            Gender::Male => "Male",
            Gender::Female => "Female",
            Gender::Wildcard => "Wildcard",
            Gender::Unknown => "Unknown",
        }
    }

    /// Normalizes arbitrary input (save enum strings, short codes, empty).
    pub fn coerce(raw: Option<&str>) -> Gender {
        let Some(s) = raw else { return Gender::Unknown };
        let r = s.trim();
        if r.is_empty() {
            return Gender::Unknown;
        }
        let lower = r.to_ascii_lowercase();
        match lower.as_str() {
            "male" | "m" | "epalgendertype::male" => Gender::Male,
            "female" | "f" | "epalgendertype::female" => Gender::Female,
            "wildcard" | "any" | "both" => Gender::Wildcard,
            _ => Gender::Unknown,
        }
    }
}

/// Free-form provenance for owned pals (nickname / level / instance_id), carried
/// through to the UI badge. Unused by the solver; harmless on selected/wild refs.
#[derive(Debug, Clone, Default)]
pub struct Provenance {
    pub instance_id: Option<Value>,
    pub nickname: Option<Value>,
    pub level: Option<Value>,
    pub owner_uid: Option<Value>,
    /// Always present for owned pals: the raw CharacterID before boss-prefix stripping,
    /// kept distinct from `species` so the UI can show the exact save value.
    pub raw_character_id: Option<Value>,
}

/// Immutable (shared via `Arc`) so it can be grouped in the solver’s working set.
/// The `parents` pair makes a `PalRef` a node in a DAG: sources are leaves
/// (`parents == None`), bred children carry two parent refs. `origin` records *why*
/// this pal exists, so source badges need no re-derivation.
#[derive(Debug, Clone)]
pub struct PalRef {
    /// Internal asset/tribe name (e.g. "WeaselDragon").
    pub species: String,
    pub gender: Gender,
    pub passives: BTreeSet<String>,
    /// 0 = source pal, 1 = first bred generation, ...
    pub generation: u32,
    pub parents: Option<(Arc<PalRef>, Arc<PalRef>)>,
    pub origin: Origin,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Owned,
    Selected,
    Wild,
    Bred,
}

impl Origin {
    pub fn as_str(self) -> &'static str {
        match self {
            Origin::Owned => "owned",
            Origin::Selected => "selected",
            Origin::Wild => "wild",
            Origin::Bred => "bred",
        }
    }
}

impl PalRef {
    pub fn is_source(&self) -> bool {
        self.parents.is_none()
    }

    /// Identity used to dedupe refs in `CompositeSource`: species + gender +
    /// *full* passive set. Distinct from the solver's `group_key`, which uses
    /// effective passives.
    pub fn group_key(&self) -> (String, Gender, BTreeSet<String>) {
        (self.species.clone(), self.gender, self.passives.clone())
    }
}

/// One A+B→child edge inside a chain, flattened for serialization.
///
/// `parent_a`/`parent_b` are tribe strings for display. The lineage refs
/// (`*_step` / `*_source` indices) disambiguate *which* occurrence of a tribe is the
/// parent — a species can appear both as a bred node and as a source leaf in one
/// chain. `*_step` indexes `Chain::steps` (parents are bred earlier, so a lower
/// index); `*_source` indexes `Chain::sources`. Exactly one of the two is `Some`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BreedingStep {
    pub parent_a: String,
    pub parent_b: String,
    pub child: String,
    pub inherited_passives: Vec<String>,
    pub gender_feasible: bool,
    pub parent_a_step: Option<usize>,
    pub parent_b_step: Option<usize>,
    pub parent_a_source: Option<usize>,
    pub parent_b_source: Option<usize>,
}

/// `steps` is a flat, topologically-ordered list (parents before children) rather
/// than a nested tree. `sources` lists the leaf pals the chain consumes, by origin.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Chain {
    pub target: String,
    pub generations: u32,
    pub steps: Vec<BreedingStep>,
    pub final_passives: Vec<String>,
    pub sources: Vec<ChainSource>,
    pub gender_feasible: bool,
    pub matched_passives: Vec<String>,
}

/// Shape mirrors the TS `ChainSource`: optional provenance fields are omitted when absent.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ChainSource {
    #[serde(rename = "type")]
    pub source_type: String,
    pub pal: String,
    pub display: String,
    pub gender: String,
    pub passives: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nickname: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_uid: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_character_id: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct BreedingSpec {
    pub target_pal: String,
    /// Must be a subset of the child's passives.
    pub required_passives: Vec<String>,
    pub target_gender: Option<Gender>,
    pub max_generations: u32,
    pub max_results: u32,
}

impl Default for BreedingSpec {
    fn default() -> Self {
        Self {
            target_pal: String::new(),
            required_passives: Vec::new(),
            target_gender: None,
            max_generations: 5,
            max_results: 5,
        }
    }
}

/// `parent_*_gender` is `Some` only for the unique combos the game gates on parent
/// gender (`DT_PalCombiUnique.ParentGenderA/B`) — e.g. CatMage + FoxMage yields a
/// different child depending on which parent is male. Genders are stated relative
/// to this row's own `parent_a`/`parent_b` order.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DirectResult {
    pub parent_a: String,
    pub parent_b: String,
    pub child: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_display: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_gender_prob: Option<GenderProb>,
    pub combo_type: ComboType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_a_gender: Option<Gender>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_b_gender: Option<Gender>,
}

/// A pair usually has exactly one outcome, but a gender-gated unique pair has two,
/// one per gender assignment. Genders are relative to the queried parent order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComboOutcome {
    pub child: String,
    pub parent_a_gender: Option<Gender>,
    pub parent_b_gender: Option<Gender>,
    pub combo_type: ComboType,
}

impl ComboOutcome {
    /// `Wildcard`/`Unknown` count as "could be either", so an unresolved pal keeps
    /// every branch alive.
    pub fn admits(&self, gender_a: Gender, gender_b: Gender) -> bool {
        fn ok(req: Option<Gender>, actual: Gender) -> bool {
            match req {
                None => true,
                Some(want) => matches!(actual, Gender::Wildcard | Gender::Unknown) || actual == want,
            }
        }
        ok(self.parent_a_gender, gender_a) && ok(self.parent_b_gender, gender_b)
    }
}

/// `{"male": p, "female": q}` — serialized as a two-key object.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GenderProb {
    #[serde(default = "default_half")]
    pub male: f64,
    #[serde(default = "default_half")]
    pub female: f64,
}

fn default_half() -> f64 {
    0.5
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ComboType {
    Formula,
    Unique,
}
