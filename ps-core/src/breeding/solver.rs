//! The breeding-chain solver, shared by Selection Mode and Save Mode.
//!
//! Iterative working-set expansion: grow a frontier of reachable `PalRef`s one
//! breeding generation at a time, breeding every compatible pair, keeping only
//! the optimal ref per group. The load-bearing optimizations:
//!
//! 1. **Reachability pruning** via the precomputed `MinBreedingSteps` map
//!    ([`super::data::BreedingDB::reachable`]): a pair is bred only when the
//!    child can still reach the target within the remaining generation budget.
//!    Without this the frontier explodes combinatorially.
//! 2. **Effective-passives grouping**: refs are interchangeable when they share
//!    species, gender, and the required-relevant subset of their passives.
//!    Non-required passives collapse to one sentinel in the group key, so 50
//!    Anubis with 50 different random passive sets occupy ONE group slot.
//! 3. **Optimal-per-group keep**: fewest generations, then most required-passive
//!    matches.
//!
//! Passive inheritance is modeled as *possibility*, not probability: a child
//! "can have" passive P iff at least one parent has P (union, capped at 4
//! slots). Bred children are Wildcard gender; a pair is compatible when at
//! least one male and one female are present (wildcard counts as either).

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use super::data::BreedingDB;
use super::model::{
    BreedingSpec, BreedingStep, Chain, ChainSource, Gender, Origin, PalRef,
};
use super::sources::SourceAdapter;

/// Game hard cap; a child can hold at most 4 passives.
const MAX_PASSIVES: usize = 4;
/// Sentinel marking "one or more non-required passives present". Kept distinct
/// from any real passive name (CamelCase IDs like "Legend", never NUL-prefixed).
const OTHER: &str = "\u{0}other";

type GroupKey = (String, Gender, BTreeSet<String>);

struct WorkingSet {
    /// Insertion order of group keys; makes which equivalent ref survives deterministic.
    order: Vec<GroupKey>,
    map: HashMap<GroupKey, Arc<PalRef>>,
}

impl WorkingSet {
    fn new() -> Self {
        Self {
            order: Vec::new(),
            map: HashMap::new(),
        }
    }

    /// Inserts `ref_pal` when it improves on its group's entry; true when the set changed.
    fn merge(&mut self, ref_pal: Arc<PalRef>, required: &BTreeSet<String>) -> bool {
        let key = group_key(&ref_pal, required);
        match self.map.get(&key) {
            None => {
                self.order.push(key.clone());
                self.map.insert(key, ref_pal);
                true
            }
            Some(existing) => {
                if is_better(&ref_pal, existing, required) {
                    self.map.insert(key, ref_pal);
                    true
                } else {
                    false
                }
            }
        }
    }

    fn frontier(&self) -> Vec<Arc<PalRef>> {
        self.order
            .iter()
            .filter_map(|k| self.map.get(k).cloned())
            .collect()
    }
}

/// Up to `spec.max_results` chains from `source` to `spec.target_pal`. Empty when
/// unreachable within `spec.max_generations` or when no chain satisfies the
/// required-passives / target-gender constraints.
pub fn solve(db: &BreedingDB, source: &dyn SourceAdapter, spec: &BreedingSpec) -> Vec<Chain> {
    let required: BTreeSet<String> = spec.required_passives.iter().cloned().collect();

    let initial: Vec<PalRef> = source
        .initial_refs(db)
        .into_iter()
        .filter(|r| keep_seed(db, r, spec))
        .collect();
    let mut working = WorkingSet::new();
    for ref_pal in initial {
        working.merge(Arc::new(ref_pal), &required);
    }

    let mut target_refs: Vec<Arc<PalRef>> = Vec::new();

    for gen in 0..spec.max_generations {
        let frontier = working.frontier();
        if frontier.is_empty() {
            break;
        }

        let remaining_budget = (spec.max_generations as i64) - ((gen + 1) as i64);
        let mut new_children: Vec<PalRef> = Vec::new();

        // Symmetric product with i<=j dedup — unordered pairs including (a,a) self-breeds.
        let n = frontier.len();
        for i in 0..n {
            for j in i..n {
                let p1 = &frontier[i];
                let p2 = &frontier[j];
                if !gender_compatible(p1, p2) {
                    continue;
                }
                // A pair usually has one outcome, but a gender-gated unique pair has
                // two, so branch on every outcome the parents' genders still admit.
                for outcome in
                    db.forward_gendered(&p1.species, p1.gender, &p2.species, p2.gender)
                {
                    if !db.reachable(&outcome.child, &spec.target_pal, remaining_budget) {
                        continue;
                    }
                    let inherited = inherit_passives(&p1.passives, &p2.passives);
                    new_children.push(PalRef {
                        species: outcome.child,
                        gender: Gender::Wildcard,
                        passives: inherited,
                        generation: gen + 1,
                        parents: Some((Arc::clone(p1), Arc::clone(p2))),
                        origin: Origin::Bred,
                        provenance: Default::default(),
                    });
                }
            }
        }

        if new_children.is_empty() {
            break;
        }

        let mut changed = false;
        for ref_pal in new_children {
            let is_target = ref_pal.species == spec.target_pal;
            let arc = Arc::new(ref_pal);
            if is_target {
                target_refs.push(Arc::clone(&arc));
            }
            if working.merge(arc, &required) {
                changed = true;
            }
        }
        if !changed {
            // Frontier stable: no ref improved. Further generations can't help.
            break;
        }
    }

    build_results(db, target_refs, &working, spec, &required)
}

fn group_key(ref_pal: &PalRef, required: &BTreeSet<String>) -> GroupKey {
    (
        ref_pal.species.clone(),
        ref_pal.gender,
        effective_passives(&ref_pal.passives, required),
    )
}

fn effective_passives(passives: &BTreeSet<String>, required: &BTreeSet<String>) -> BTreeSet<String> {
    let kept: BTreeSet<String> = passives.intersection(required).cloned().collect();
    let has_extra = passives.difference(required).next().is_some();
    if has_extra {
        let mut out = kept;
        out.insert(OTHER.to_string());
        out
    } else {
        kept
    }
}

fn is_better(a: &PalRef, b: &PalRef, required: &BTreeSet<String>) -> bool {
    if a.generation != b.generation {
        return a.generation < b.generation;
    }
    let a_match = a.passives.intersection(required).count();
    let b_match = b.passives.intersection(required).count();
    if a_match != b_match {
        return a_match > b_match;
    }
    false
}

fn inherit_passives(a: &BTreeSet<String>, b: &BTreeSet<String>) -> BTreeSet<String> {
    // a's passives first, then b's new ones. BTreeSet iterates sorted, so the 4-slot
    // cap is deterministic. Only set membership matters downstream.
    let mut combined: Vec<String> = Vec::with_capacity(a.len() + b.len());
    for p in a {
        combined.push(p.clone());
    }
    for p in b {
        if !a.contains(p) {
            combined.push(p.clone());
        }
    }
    if combined.len() <= MAX_PASSIVES {
        combined.into_iter().collect()
    } else {
        combined.into_iter().take(MAX_PASSIVES).collect()
    }
}

fn gender_compatible(p1: &PalRef, p2: &PalRef) -> bool {
    let (g1, g2) = (p1.gender, p2.gender);
    if matches!(g1, Gender::Wildcard | Gender::Unknown)
        || matches!(g2, Gender::Wildcard | Gender::Unknown)
    {
        return true;
    }
    (g1, g2) == (Gender::Male, Gender::Female) || (g1, g2) == (Gender::Female, Gender::Male)
}

fn gender_feasible(db: &BreedingDB, species: &str, target: Option<Gender>) -> bool {
    let Some(target) = target else {
        return true;
    };
    if matches!(target, Gender::Wildcard | Gender::Unknown) {
        return true;
    }
    let prob = db.gender_probability(species);
    match target {
        Gender::Male => prob.male > 0.0,
        Gender::Female => prob.female > 0.0,
        _ => true,
    }
}

fn keep_seed(db: &BreedingDB, ref_pal: &PalRef, spec: &BreedingSpec) -> bool {
    if ref_pal.species == spec.target_pal {
        return true;
    }
    db.reachable(
        &ref_pal.species,
        &spec.target_pal,
        spec.max_generations as i64,
    )
}

fn build_results(
    db: &BreedingDB,
    bred_targets: Vec<Arc<PalRef>>,
    working: &WorkingSet,
    spec: &BreedingSpec,
    required: &BTreeSet<String>,
) -> Vec<Chain> {
    let mut candidates: Vec<Arc<PalRef>> = bred_targets;
    // Include a target already in the seed pool (e.g. owned target that needs
    // more passives bred onto it, or a 0-gen "you already have it" answer).
    for ref_pal in working.map.values() {
        if ref_pal.species != spec.target_pal {
            continue;
        }
        if ref_pal.origin != Origin::Bred {
            // bred ones already in `bred_targets`.
            candidates.push(Arc::clone(ref_pal));
        }
    }

    let mut qualifying: Vec<Arc<PalRef>> = candidates
        .into_iter()
        .filter(|r| {
            if !required.is_empty() && !required.is_subset(&r.passives) {
                return false;
            }
            if !gender_feasible(db, &r.species, spec.target_gender) {
                return false;
            }
            true
        })
        .collect();

    // Rank: fewest generations, then most required matches, then fewest total.
    qualifying.sort_by(|a, b| {
        a.generation
            .cmp(&b.generation)
            .then_with(|| {
                let am = a.passives.intersection(required).count();
                let bm = b.passives.intersection(required).count();
                bm.cmp(&am)
            })
            .then_with(|| a.passives.len().cmp(&b.passives.len()))
    });

    let mut chains: Vec<Chain> = Vec::new();
    let mut seen_sigs: HashSet<(String, BTreeSet<(String, String, String)>, BTreeSet<String>)> =
        HashSet::new();
    for ref_pal in qualifying {
        let chain = build_chain(db, &ref_pal, spec, required);
        let sig = chain_signature(&chain);
        if !seen_sigs.insert(sig) {
            continue;
        }
        if is_degenerate_self_breed(&chain) {
            continue;
        }
        chains.push(chain);
        if chains.len() >= spec.max_results as usize {
            break;
        }
    }
    chains
}

fn chain_signature(chain: &Chain) -> (String, BTreeSet<(String, String, String)>, BTreeSet<String>) {
    let edges: BTreeSet<(String, String, String)> = chain
        .steps
        .iter()
        .map(|s| (s.parent_a.clone(), s.parent_b.clone(), s.child.clone()))
        .collect();
    let final_passives: BTreeSet<String> = chain.final_passives.iter().cloned().collect();
    (chain.target.clone(), edges, final_passives)
}

fn is_degenerate_self_breed(chain: &Chain) -> bool {
    if chain.steps.is_empty() {
        return false;
    }
    chain.steps.iter().all(|s| {
        s.parent_a == chain.target && s.parent_b == chain.target && s.child == chain.target
    })
}

fn build_chain(
    db: &BreedingDB,
    final_ref: &Arc<PalRef>,
    spec: &BreedingSpec,
    required: &BTreeSet<String>,
) -> Chain {
    let mut steps: Vec<BreedingStep> = Vec::new();
    let mut sources: Vec<ChainSource> = Vec::new();
    let mut visited: HashSet<usize> = HashSet::new();
    // Arc-ptr → lineage index, so steps can reference their exact parents.
    let mut source_idx: HashMap<usize, usize> = HashMap::new();
    let mut step_idx: HashMap<usize, usize> = HashMap::new();
    flatten(
        db,
        final_ref,
        &mut steps,
        &mut sources,
        &mut visited,
        &mut source_idx,
        &mut step_idx,
    );

    let matched: Vec<String> = final_ref
        .passives
        .intersection(required)
        .cloned()
        .collect();

    Chain {
        target: spec.target_pal.clone(),
        generations: final_ref.generation,
        steps,
        final_passives: final_ref.passives.iter().cloned().collect(),
        sources,
        gender_feasible: gender_feasible(db, &final_ref.species, spec.target_gender),
        matched_passives: matched,
    }
}

fn flatten(
    db: &BreedingDB,
    ref_pal: &Arc<PalRef>,
    steps: &mut Vec<BreedingStep>,
    sources: &mut Vec<ChainSource>,
    visited: &mut HashSet<usize>,
    source_idx: &mut HashMap<usize, usize>,
    step_idx: &mut HashMap<usize, usize>,
) {
    // Identity by shared allocation (Arc pointer).
    let id = Arc::as_ptr(ref_pal) as *const PalRef as usize;
    if !visited.insert(id) {
        return;
    }

    if ref_pal.is_source() {
        let p = &ref_pal.provenance;
        let this_idx = sources.len();
        sources.push(ChainSource {
            source_type: ref_pal.origin.as_str().to_string(),
            pal: ref_pal.species.clone(),
            display: db.display_name(&ref_pal.species),
            gender: ref_pal.gender.as_value().to_string(),
            passives: ref_pal.passives.iter().cloned().collect(),
            instance_id: p.instance_id.clone(),
            nickname: p.nickname.clone(),
            level: p.level.clone(),
            owner_uid: p.owner_uid.clone(),
            raw_character_id: p.raw_character_id.clone(),
        });
        source_idx.insert(id, this_idx);
        return;
    }

    let (p1, p2) = ref_pal.parents.as_ref().expect("bred ref has parents");
    flatten(db, p1, steps, sources, visited, source_idx, step_idx);
    flatten(db, p2, steps, sources, visited, source_idx, step_idx);

    // A parent is either a prior bred step (already appended — post-order guarantees
    // the earlier index) or a source leaf.
    let parent_ref = |p: &Arc<PalRef>| {
        let pid = Arc::as_ptr(p) as *const PalRef as usize;
        if let Some(i) = step_idx.get(&pid) {
            return (Some(*i), None);
        }
        if let Some(i) = source_idx.get(&pid) {
            return (None, Some(*i));
        }
        (None, None)
    };
    let (a_step, a_source) = parent_ref(p1);
    let (b_step, b_source) = parent_ref(p2);

    let this_idx = steps.len();
    steps.push(BreedingStep {
        parent_a: p1.species.clone(),
        parent_b: p2.species.clone(),
        child: ref_pal.species.clone(),
        inherited_passives: ref_pal.passives.iter().cloned().collect(),
        // Per-step feasibility uses target=None (always true) — the real
        // feasibility check is on the final ref in build_results.
        gender_feasible: true,
        parent_a_step: a_step,
        parent_b_step: b_step,
        parent_a_source: a_source,
        parent_b_source: b_source,
    });
    step_idx.insert(id, this_idx);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::breeding::data::BreedingDB;
    use crate::breeding::sources::{OwnedSource, SelectedSource};
    use crate::gamedata::GameData;

    fn load_repo_db() -> BreedingDB {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/json");
        let gd = GameData::load(&dir).expect("game data loads");
        BreedingDB::from_game_data(&gd).expect("breeding db builds")
    }

    #[test]
    fn solve_selection_finds_direct_child_chain() {
        let db = load_repo_db();
        // Alpaca + Alpaca → Alpaca, so targeting Alpaca with 0 required passives
        // must yield at least a 1-gen chain.
        let source = SelectedSource::new(vec![crate::breeding::sources::SelectedPalInput {
            species: "Alpaca".to_string(),
            gender: None,
            passives: vec![],
        }]);
        let spec = BreedingSpec {
            target_pal: "Alpaca".to_string(),
            required_passives: vec![],
            target_gender: None,
            max_generations: 2,
            max_results: 5,
        };
        let chains = solve(&db, &source, &spec);
        assert!(!chains.is_empty(), "should find at least the self-breed chain");
    }

    #[test]
    fn solve_owned_source_empty_when_no_pals() {
        let db = load_repo_db();
        let source = OwnedSource::new(vec![]);
        let spec = BreedingSpec {
            target_pal: "Anubis".to_string(),
            ..Default::default()
        };
        let chains = solve(&db, &source, &spec);
        assert!(chains.is_empty(), "no owned pals → no chains");
    }

    #[test]
    fn inherit_passives_caps_at_four() {
        let a: BTreeSet<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
        let b: BTreeSet<String> = ["d", "e"].iter().map(|s| s.to_string()).collect();
        let out = inherit_passives(&a, &b);
        assert_eq!(out.len(), MAX_PASSIVES);
    }

    /// Exactly one of *step/*source is set per parent, step refs point at an earlier
    /// step, source refs are in bounds. The frontend dendrogram relies on this to
    /// render correct lineage (a tribe can be both bred and a source in one chain).
    #[test]
    fn chain_steps_have_resolvable_parent_lineage() {
        let db = load_repo_db();
        // A pool of common, mutually-breeding species ensures multi-gen chains exist;
        // the distance map is a graph metric and can't be reached by self-breeding.
        let species = ["Anubis", "Foxparks", "Alpaca", "Chikipi"];
        let source = SelectedSource::new(
            species
                .iter()
                .map(|s| crate::breeding::sources::SelectedPalInput {
                    species: s.to_string(),
                    gender: None,
                    passives: vec![],
                })
                .collect(),
        );

        // Scan targets for one solved with a ≥2-generation chain, which exercises
        // bred-parent lineage refs.
        let mut checked_chains: Option<Vec<Chain>> = None;
        let mut saw_bred_parent = false;
        for tribe in db.breedable_tribes() {
            let spec = BreedingSpec {
                target_pal: tribe.clone(),
                required_passives: vec![],
                target_gender: None,
                max_generations: 5,
                max_results: 1,
            };
            let chains = solve(&db, &source, &spec);
            if chains.is_empty() || chains[0].generations < 2 {
                continue;
            }
            let chain = &chains[0];
            let mut local_bred = false;
            for (i, step) in chain.steps.iter().enumerate() {
                for (step_ref, source_ref) in [
                    (step.parent_a_step, step.parent_a_source),
                    (step.parent_b_step, step.parent_b_source),
                ] {
                    assert_eq!(
                        step_ref.is_some() as u8 + source_ref.is_some() as u8,
                        1,
                        "exactly one lineage ref per parent (step {i})"
                    );
                    if let Some(si) = step_ref {
                        assert!(
                            si < i,
                            "bred parent must be an earlier step (step {i} → {si})"
                        );
                        local_bred = true;
                    }
                    if let Some(si) = source_ref {
                        assert!(
                            si < chain.sources.len(),
                            "source ref in bounds (step {i} → {si})"
                        );
                    }
                }
            }
            checked_chains = Some(chains);
            saw_bred_parent = local_bred;
            if saw_bred_parent {
                break;
            }
        }
        assert!(
            checked_chains.is_some(),
            "expected some target to yield a multi-generational chain"
        );
        assert!(saw_bred_parent, "a multi-gen chain must contain a bred parent ref");
    }
}
