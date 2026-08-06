//! Direct Mode — the breeding tab's two sub-modes, as pure functions.
//!
//! * **Forward** — Parent A + Parent B → resulting child. One lookup.
//! * **Reverse** — Parent A + target child → all candidate Parent B pals.
//! * **Parents** — target child → all parent pairs.
//!
//! Gender is *not* a filter here (the combo table is gender-agnostic); we
//! surface the child's gender probability for display only. The gender-aware
//! logic lives in the chain solver, where it gates feasibility.
//!
//! Faithful port of `PalSavTools/src/palworld_aio/breeding/direct.py`.

use super::data::BreedingDB;
use super::model::{ComboType, DirectResult};

/// A + B → child. `None` if the pair has no known child.
pub fn direct_child(db: &BreedingDB, parent_a: &str, parent_b: &str) -> Option<DirectResult> {
    let child = db.forward(parent_a, parent_b)?;
    let combo_type = if db.is_unique_combo(parent_a, parent_b, child) {
        ComboType::Unique
    } else {
        ComboType::Formula
    };
    Some(db.direct_result(parent_a, parent_b, child, combo_type))
}

/// A + target → candidate B pals, sorted by partner display name.
pub fn direct_partners(db: &BreedingDB, parent_a: &str, target_child: &str) -> Vec<DirectResult> {
    let mut partners: Vec<DirectResult> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for b in db.reverse(parent_a, target_child) {
        if !seen.insert(b.clone()) {
            continue;
        }
        let combo_type = if db.is_unique_combo(parent_a, &b, target_child) {
            ComboType::Unique
        } else {
            ComboType::Formula
        };
        partners.push(db.direct_result(parent_a, &b, target_child, combo_type));
    }
    partners.sort_by(|a, b| {
        db.display_name(&a.parent_b)
            .to_lowercase()
            .cmp(&db.display_name(&b.parent_b).to_lowercase())
    });
    partners
}

/// target → ALL parent pairs (no Parent A pinned). Unique combos first then
/// formula, de-duplicated symmetrically.
pub fn direct_parents(db: &BreedingDB, target_child: &str) -> Vec<DirectResult> {
    let mut out: Vec<DirectResult> = Vec::new();
    let mut seen: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    for (a, b) in db.child_to_parents(target_child) {
        let key = if a <= b {
            (a.clone(), b.clone())
        } else {
            (b.clone(), a.clone())
        };
        if !seen.insert(key) {
            continue;
        }
        let combo_type = if db.is_unique_combo(a, b, target_child) {
            ComboType::Unique
        } else {
            ComboType::Formula
        };
        out.push(db.direct_result(a, b, target_child, combo_type));
    }
    out
}
