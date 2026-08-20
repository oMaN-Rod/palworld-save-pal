//! Breeding-graph reachability helpers.
//!
//! The precomputed `MinBreedingSteps` map (built by palcalc via Floyd-Warshall
//! and shipped in `breeding_distance.json`) answers shortest-path queries in
//! O(1). This module exposes a thin query layer over it plus a fallback for
//! pairs the map doesn't cover (newer pals absent from the palcalc snapshot) —
//! a one-step direct-breed check against the combo table.

use super::data::BreedingDB;

/// Fewest breeding steps from `start` to `target`.
///
/// Same pal → 0. Unknown (not in the distance map) → `None`, **unless** the
/// pair is directly breedable (one step), detected via the combo table so newer
/// pals without a distance row aren't falsely reported unreachable.
pub fn min_steps(db: &BreedingDB, start: &str, target: &str) -> Option<i64> {
    if start == target {
        return Some(0);
    }
    if let Some(row) = db.min_steps_row(start) {
        if let Some(steps) = row.get(target) {
            return Some(*steps);
        }
    }
    // Fallback: maybe `start` breeds directly into `target` despite having no
    // distance row (a pal added after palcalc's snapshot). Only resolves the
    // 1-step case; multi-step paths through such pals stay unknown (None).
    if directly_breeds_into(db, start, target) {
        return Some(1);
    }
    None
}

fn directly_breeds_into(db: &BreedingDB, parent: &str, target: &str) -> bool {
    db.child_to_parents(target)
        .iter()
        .any(|p| p.parent_a == parent || p.parent_b == parent)
}

pub fn can_reach(db: &BreedingDB, start: &str, target: &str, max_steps: i64) -> bool {
    min_steps(db, start, target)
        .map(|steps| steps <= max_steps)
        .unwrap_or(false)
}
