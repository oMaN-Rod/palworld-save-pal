//! Direct Mode — the breeding tab's two sub-modes, as pure functions.
//!
//! * **Forward** — Parent A + Parent B → resulting child(ren). One lookup.
//! * **Reverse** — Parent A + target child → all candidate Parent B pals.
//! * **Parents** — target child → all parent pairs.
//!
//! Gender is not a *filter* here (the combo table is overwhelmingly
//! gender-agnostic), but it is surfaced: the few `DT_PalCombiUnique` rows that
//! gate on parent gender produce more than one child for the same pair, and
//! each result carries the gender assignment that selects it. The child's own
//! gender probability is display-only. The gender-aware *feasibility* logic
//! lives in the chain solver.

use super::data::BreedingDB;
use super::model::DirectResult;

/// A + B → every child the pair can produce.
///
/// Usually one row. A gender-gated unique pair (CatMage + FoxMage) returns two,
/// each tagged with the parent genders that select it.
pub fn direct_child(db: &BreedingDB, parent_a: &str, parent_b: &str) -> Vec<DirectResult> {
    db.forward_all(parent_a, parent_b)
        .into_iter()
        .map(|o| {
            db.direct_result(
                parent_a,
                parent_b,
                &o.child,
                o.combo_type,
                o.parent_a_gender,
                o.parent_b_gender,
            )
        })
        .collect()
}

/// A + target → candidate B pals, sorted by partner display name.
pub fn direct_partners(db: &BreedingDB, parent_a: &str, target_child: &str) -> Vec<DirectResult> {
    let mut partners: Vec<DirectResult> = Vec::new();
    let mut seen: std::collections::HashSet<(String, String, String)> =
        std::collections::HashSet::new();
    for pair in db.reverse(parent_a, target_child) {
        // Gender gates are part of the identity — the same partner can appear
        // twice with different gates and different children.
        let key = (
            pair.parent_b.clone(),
            format!("{:?}", pair.parent_a_gender),
            format!("{:?}", pair.parent_b_gender),
        );
        if !seen.insert(key) {
            continue;
        }
        partners.push(db.direct_result(
            parent_a,
            &pair.parent_b,
            target_child,
            pair.combo_type,
            pair.parent_a_gender,
            pair.parent_b_gender,
        ));
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
    db.child_to_parents(target_child)
        .iter()
        .map(|pair| {
            db.direct_result(
                &pair.parent_a,
                &pair.parent_b,
                target_child,
                pair.combo_type,
                pair.parent_a_gender,
                pair.parent_b_gender,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gamedata::GameData;

    fn load_repo_db() -> BreedingDB {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/json");
        let gd = GameData::load(&dir).expect("game data loads");
        BreedingDB::from_game_data(&gd).expect("breeding db builds")
    }

    #[test]
    fn direct_child_returns_single_row_for_plain_pair() {
        let db = load_repo_db();
        let rows = direct_child(&db, "LazyDragon", "ElecCat");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].child, "LazyDragon_Electric");
        assert!(rows[0].parent_a_gender.is_none());
    }

    #[test]
    fn direct_child_returns_both_rows_for_gender_gated_pair() {
        let db = load_repo_db();
        let rows = direct_child(&db, "CatMage", "FoxMage");
        assert_eq!(rows.len(), 2, "gender-gated pair has two outcomes");
        for row in &rows {
            assert!(
                row.parent_a_gender.is_some() && row.parent_b_gender.is_some(),
                "each gated row states both parent genders"
            );
        }
        let mut kids: Vec<&str> = rows.iter().map(|r| r.child.as_str()).collect();
        kids.sort_unstable();
        assert_eq!(kids, ["CatMage_Fire", "FoxMage_Dark"]);
    }

    #[test]
    fn direct_child_self_pair_breeds_true() {
        let db = load_repo_db();
        let rows = direct_child(&db, "BluePlatypus_Fire", "BluePlatypus_Fire");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].child, "BluePlatypus_Fire");
    }

    #[test]
    fn direct_parents_lists_both_gender_gates_for_a_gated_child() {
        let db = load_repo_db();
        let rows = direct_parents(&db, "FoxMage_Dark");
        let gated: Vec<_> = rows
            .iter()
            .filter(|r| r.parent_a_gender.is_some() || r.parent_b_gender.is_some())
            .collect();
        assert!(
            !gated.is_empty(),
            "FoxMage_Dark is produced by a gender-gated pair"
        );
    }
}
