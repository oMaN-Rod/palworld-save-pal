//! The pal legality validator — the "illegal pals checker" ported from
//! PalSavTools' `pal_validator.py`. A pure function over one pal's
//! `SaveParameter` and CharacterID against the game-data catalogs; no I/O, no
//! per-instance state.
//!
//! Design rules (inherited from the reference):
//! * **Low false-positive rate.** Only flag what is *impossible* in a
//!   legitimate save, never merely unusual. A legitimately-caught level-60
//!   pal must never be flagged.
//! * **Machine-readable codes.** Each issue is a stable string code;
//!   the frontend translates codes to localized text + severity. Codes are
//!   namespaced `ILLEGAL_*` (severity danger) or `SUSPICIOUS_*` (warning).
//! * **Graceful degradation.** A missing skill catalog disables its check —
//!   absent data must never fabricate flags.

use crate::props;
use crate::ue::Properties;

use crate::domain::pal::param;

use super::catalogs::OverviewCatalogs;

// ── Legality caps (match the reference implementation exactly) ─────────────

pub const SAFE_LEVEL_MAX: i64 = 80;
pub const SAFE_RANK_MAX: i64 = 5;
pub const SAFE_SOUL_MAX: i64 = 20;
pub const SAFE_IV_MAX: i64 = 100;
pub const SAFE_PASSIVE_SLOTS: usize = 4;

/// Tolerated MaxHP overshoot (fraction of the computed ceiling) before
/// `ILLEGAL_HP` fires. Absorbs cross-version formula drift; cheat-inflated HP
/// sits far above this band.
const HP_TOLERANCE: f64 = 0.05;

// ── Issue codes ─────────────────────────────────────────────────────────────

pub const ILLEGAL_SPECIES: &str = "ILLEGAL_SPECIES";
pub const ILLEGAL_LEVEL: &str = "ILLEGAL_LEVEL";
pub const ILLEGAL_RANK: &str = "ILLEGAL_RANK";
pub const SUSPICIOUS_SOUL_RANK: &str = "SUSPICIOUS_SOUL_RANK";
pub const SUSPICIOUS_TALENT: &str = "SUSPICIOUS_TALENT";
pub const SUSPICIOUS_PASSIVE_SLOTS: &str = "SUSPICIOUS_PASSIVE_SLOTS";
pub const ILLEGAL_PASSIVE: &str = "ILLEGAL_PASSIVE";
pub const ILLEGAL_ACTIVE: &str = "ILLEGAL_ACTIVE";
pub const ILLEGAL_HP: &str = "ILLEGAL_HP";

const DANGER_CODES: [&str; 6] = [
    ILLEGAL_SPECIES,
    ILLEGAL_LEVEL,
    ILLEGAL_RANK,
    ILLEGAL_PASSIVE,
    ILLEGAL_ACTIVE,
    ILLEGAL_HP,
];

/// `"danger"` for hard illegals, `"warning"` for soft suspicions.
pub fn severity_of(code: &str) -> &'static str {
    if DANGER_CODES.contains(&code) {
        "danger"
    } else {
        "warning"
    }
}

/// Every legality issue found on one pal's `SaveParameter`, in check order.
/// Empty for a clean pal; never panics on malformed data (missing fields read
/// as their defaults, and absent catalogs disable their checks).
pub(crate) fn detect_pal_issues(
    save_parameter: &Properties,
    character_id: &str,
    catalogs: &OverviewCatalogs,
) -> Vec<&'static str> {
    let mut issues: Vec<&'static str> = Vec::new();

    if !character_id.is_empty() && !catalogs.species_known(character_id) {
        issues.push(ILLEGAL_SPECIES);
    }

    let level = byte_field(save_parameter, "Level").unwrap_or(1) as i64;
    if !(1..=SAFE_LEVEL_MAX).contains(&level) {
        issues.push(ILLEGAL_LEVEL);
    }

    let rank = byte_field(save_parameter, "Rank").unwrap_or(1) as i64;
    if !(1..=SAFE_RANK_MAX).contains(&rank) {
        issues.push(ILLEGAL_RANK);
    }

    const SOUL_KEYS: [&str; 4] = ["Rank_HP", "Rank_Attack", "Rank_Defence", "Rank_CraftSpeed"];
    if SOUL_KEYS
        .iter()
        .any(|key| byte_field(save_parameter, key).unwrap_or(0) as i64 > SAFE_SOUL_MAX)
    {
        issues.push(SUSPICIOUS_SOUL_RANK);
    }

    const TALENT_KEYS: [&str; 3] = ["Talent_HP", "Talent_Shot", "Talent_Defense"];
    if TALENT_KEYS
        .iter()
        .any(|key| byte_field(save_parameter, key).unwrap_or(0) as i64 > SAFE_IV_MAX)
    {
        issues.push(SUSPICIOUS_TALENT);
    }

    let passives = param(save_parameter, "PassiveSkillList")
        .and_then(props::name_values)
        .cloned()
        .unwrap_or_default();
    if passives.len() > SAFE_PASSIVE_SLOTS {
        issues.push(SUSPICIOUS_PASSIVE_SLOTS);
    }
    if catalogs.passives_loaded() && passives.iter().any(|p| !catalogs.has_passive(p)) {
        issues.push(ILLEGAL_PASSIVE);
    }

    let actives = param(save_parameter, "EquipWaza")
        .and_then(props::enum_values)
        .cloned()
        .unwrap_or_default();
    if catalogs.actives_loaded() && actives.iter().any(|a| !catalogs.has_active(a)) {
        issues.push(ILLEGAL_ACTIVE);
    }

    let stored_max = stored_max_hp(save_parameter);
    if stored_max > 0 {
        let computed = validator_max_hp(save_parameter, character_id, catalogs);
        if computed > 0 && stored_max as f64 > computed as f64 * (1.0 + HP_TOLERANCE) {
            issues.push(ILLEGAL_HP);
        }
    }

    issues
}

/// Stored MaxHP (×1000). `MaxHP` is a FixedPoint64 `{Value}`; a bare Int64 is
/// accepted defensively for odd saves. Absent → 0 (ceiling check skipped).
fn stored_max_hp(save_parameter: &Properties) -> i64 {
    match param(save_parameter, "MaxHP") {
        Some(property) => props::fixed_point64(property).or_else(|| props::as_i64(property)),
        None => None,
    }
    .unwrap_or(0)
}

fn byte_field(save_parameter: &Properties, name: &str) -> Option<u8> {
    param(save_parameter, name).and_then(props::as_byte_number)
}

/// The game-accurate MaxHP ceiling (×1000) the `ILLEGAL_HP` check compares
/// against. A faithful port of the reference formula:
///
/// ```text
/// base        = floor(500 + 5*lvl + hp_scaling*0.5*lvl*(1 + hp_iv))
/// base_wc     = floor(base * (1 + condenser_bonus))
/// trust       = round(lvl * friendship_rank * friendship_hp * 0.65
///                     * (1 + condenser_bonus))
/// awake       = floor(hp_scaling * lvl * 0.065 * (1 + condenser_bonus))
///                   when bIsAwakening, else 0
/// final       = floor((base_wc + trust + awake) * (1 + soul_bonus)
///                     * (1 + passive_hp_bonus))
/// ```
///
/// Returns 0 when the species has no HP scaling data — the caller then skips
/// the ceiling check rather than flagging against a fabricated number.
pub(crate) fn validator_max_hp(
    save_parameter: &Properties,
    character_id: &str,
    catalogs: &OverviewCatalogs,
) -> i64 {
    // Raw id first so a boss variant resolves to its own (higher) scaling.
    let Some(vitals) = catalogs.vitals_for(character_id) else {
        return 0;
    };

    let lvl = byte_field(save_parameter, "Level").unwrap_or(1) as f64;
    let rank = byte_field(save_parameter, "Rank").unwrap_or(1) as i64;
    let talent_hp = byte_field(save_parameter, "Talent_HP").unwrap_or(0) as f64;
    let rank_hp = byte_field(save_parameter, "Rank_HP").unwrap_or(0) as f64;
    let friendship_point = param(save_parameter, "FriendshipPoint")
        .and_then(props::as_i32)
        .unwrap_or(0) as i64;
    let is_awake = param(save_parameter, "bIsAwakening")
        .and_then(props::as_bool)
        .unwrap_or(false);

    let condenser_bonus = (rank - 1).max(0) as f64 * 0.05;
    let hp_iv = talent_hp * 0.3 / 100.0;
    let soul_bonus = rank_hp * 0.03;
    let friendship_rank = catalogs.friendship_rank(friendship_point);
    let passive_bonus = param(save_parameter, "PassiveSkillList")
        .and_then(props::name_values)
        .map(|passives| {
            passives
                .iter()
                .filter_map(|passive| catalogs.passive_hp_fraction(passive))
                .sum::<f64>()
        })
        .unwrap_or(0.0);

    let base = (500.0 + 5.0 * lvl + vitals.scaling_hp * 0.5 * lvl * (1.0 + hp_iv)).floor();
    let base_wc = (base * (1.0 + condenser_bonus)).floor();
    let trust_bonus = (lvl
        * friendship_rank as f64
        * vitals.friendship_hp
        * 0.65
        * (1.0 + condenser_bonus)
        + 0.5) as i64;
    let awake_bonus = if is_awake {
        (vitals.scaling_hp * lvl * 0.065 * (1.0 + condenser_bonus)).floor()
    } else {
        0.0
    };
    let subtotal = base_wc + trust_bonus as f64 + awake_bonus;
    let final_hp = (subtotal * (1.0 + soul_bonus) * (1.0 + passive_bonus)).floor();
    final_hp as i64 * 1000
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gamedata::GameData;
    use crate::ue::{Byte, Property, ValueVec};

    fn game_data() -> GameData {
        GameData::from_entries([
            (
                "pals".to_string(),
                r#"{
                    "Alpaca": {"is_pal": true, "scaling": {"hp": 90}, "friendship_hp": 4.5},
                    "BOSS_Believer_CrossBow": {"is_pal": false, "scaling": {"hp": 100}, "friendship_hp": 1.0},
                    "Sheepball": {"is_pal": true, "scaling": {"hp": 80}, "friendship_hp": 4.5}
                }"#.to_string(),
            ),
            (
                "passive_skills".to_string(),
                r#"{
                    "HP_ACC_up1": {"effects": [{"type": "MaxHP", "value": 10.0, "target": "ToSelf"}]},
                    "HP_ACC_up3": {"effects": [{"type": "MaxHP", "value": 30.0, "target": "ToSelf"}]},
                    "TrainerStamina": {"effects": [{"type": "TrainerStamina", "value": 50.0, "target": "ToTrainer"}]},
                    "Legend": {"effects": [
                        {"type": "MaxHP", "value": 20.0, "target": "ToSelf"},
                        {"type": "Attack", "value": 20.0, "target": "ToSelf"}
                    ]}
                }"#.to_string(),
            ),
            (
                "active_skills".to_string(),
                r#"{
                    "EPalWazaID::AirCanon": {"power": 40},
                    "EPalWazaID::SandBlast": {"power": 40}
                }"#.to_string(),
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

    fn catalogs() -> OverviewCatalogs {
        OverviewCatalogs::from_game_data(&game_data())
    }

    fn byte_property(value: u8) -> Property {
        Property::Byte(Byte::Byte(value))
    }

    fn name_array(values: &[&str]) -> Property {
        Property::Array(ValueVec::Name(
            values.iter().map(|value| value.to_string()).collect(),
        ))
    }

    fn enum_array(values: &[&str]) -> Property {
        Property::Array(ValueVec::Enum(
            values.iter().map(|value| value.to_string()).collect(),
        ))
    }

    fn clean_pal() -> Properties {
        let mut save_parameter = Properties::default();
        save_parameter.insert("Level", byte_property(12));
        save_parameter.insert("Rank", byte_property(1));
        save_parameter
    }

    #[test]
    fn severity_splits_danger_from_warning() {
        assert_eq!(severity_of(ILLEGAL_HP), "danger");
        assert_eq!(severity_of(ILLEGAL_SPECIES), "danger");
        assert_eq!(severity_of(SUSPICIOUS_TALENT), "warning");
        assert_eq!(severity_of(SUSPICIOUS_PASSIVE_SLOTS), "warning");
    }

    #[test]
    fn clean_pal_has_no_issues() {
        assert!(detect_pal_issues(&clean_pal(), "Alpaca", &catalogs()).is_empty());
    }

    #[test]
    fn unknown_species_is_flagged() {
        let issues = detect_pal_issues(&clean_pal(), "NotAPal", &catalogs());
        assert_eq!(issues, vec![ILLEGAL_SPECIES]);
        // A boss-prefixed known id still resolves through its base form.
        let pal = clean_pal();
        assert!(detect_pal_issues(&pal, "BOSS_Alpaca", &catalogs()).is_empty());
    }

    #[test]
    fn level_and_rank_checks_fire() {
        let catalogs = catalogs();
        let mut save_parameter = Properties::default();
        save_parameter.insert("Level", byte_property(99));
        save_parameter.insert("Rank", byte_property(9));
        let issues = detect_pal_issues(&save_parameter, "Alpaca", &catalogs);
        assert!(issues.contains(&ILLEGAL_LEVEL));
        assert!(issues.contains(&ILLEGAL_RANK));
    }

    #[test]
    fn soul_talent_and_slot_checks_fire() {
        let catalogs = catalogs();

        let mut save_parameter = Properties::default();
        save_parameter.insert("Rank_HP", byte_property(21));
        assert_eq!(
            detect_pal_issues(&save_parameter, "Alpaca", &catalogs),
            vec![SUSPICIOUS_SOUL_RANK]
        );

        let mut save_parameter = Properties::default();
        save_parameter.insert("Talent_Shot", byte_property(101));
        assert_eq!(
            detect_pal_issues(&save_parameter, "Alpaca", &catalogs),
            vec![SUSPICIOUS_TALENT]
        );

        let mut save_parameter = Properties::default();
        save_parameter.insert(
            "PassiveSkillList",
            name_array(&[
                "Legend",
                "HP_ACC_up1",
                "HP_ACC_up3",
                "TrainerStamina",
                "Legend",
            ]),
        );
        assert_eq!(
            detect_pal_issues(&save_parameter, "Alpaca", &catalogs),
            vec![SUSPICIOUS_PASSIVE_SLOTS]
        );
    }

    #[test]
    fn skill_catalog_checks_fire_and_tolerate_enum_forms() {
        let catalogs = catalogs();

        let mut save_parameter = Properties::default();
        save_parameter.insert("PassiveSkillList", name_array(&["NotAPassive"]));
        assert_eq!(
            detect_pal_issues(&save_parameter, "Alpaca", &catalogs),
            vec![ILLEGAL_PASSIVE]
        );

        // The stored EPalWazaID:: form and the bare form both validate.
        let mut save_parameter = Properties::default();
        save_parameter.insert(
            "EquipWaza",
            enum_array(&["EPalWazaID::AirCanon", "SandBlast"]),
        );
        assert!(detect_pal_issues(&save_parameter, "Alpaca", &catalogs).is_empty());

        let mut save_parameter = Properties::default();
        save_parameter.insert("EquipWaza", enum_array(&["EPalWazaID::HackSkill"]));
        assert_eq!(
            detect_pal_issues(&save_parameter, "Alpaca", &catalogs),
            vec![ILLEGAL_ACTIVE]
        );
    }

    #[test]
    fn absent_catalogs_disable_skill_checks() {
        let empty = GameData::from_entries([("empty".to_string(), "{}".to_string())]).unwrap();
        let catalogs = OverviewCatalogs::from_game_data(&empty);
        let mut save_parameter = Properties::default();
        save_parameter.insert("PassiveSkillList", name_array(&["NotAPassive"]));
        save_parameter.insert("EquipWaza", enum_array(&["EPalWazaID::HackSkill"]));
        // Like the reference, a missing species catalog still flags the
        // species itself; only the skill checks degrade to no-ops.
        assert_eq!(
            detect_pal_issues(&save_parameter, "Alpaca", &catalogs),
            vec![ILLEGAL_SPECIES]
        );
    }

    /// Reference value verified by running the original Python
    /// `_compute_max_hp` over PalSavTools' own characters.json/skills.json
    /// with these inputs: Alpaca (hp 90, friendship_hp 4.5), level 12, rank 2,
    /// Talent_HP 40, Rank_HP 3, friendship 7500 (rank 1), HP_ACC_up1 (+10%)
    /// → base 1164, base_wc 1222, trust 37, final floor(1259·1.09·1.10)=1509.
    #[test]
    fn validator_max_hp_matches_the_reference_formula() {
        let catalogs = catalogs();
        let mut save_parameter = Properties::default();
        save_parameter.insert("Level", byte_property(12));
        save_parameter.insert("Rank", byte_property(2));
        save_parameter.insert("Talent_HP", byte_property(40));
        save_parameter.insert("Rank_HP", byte_property(3));
        save_parameter.insert("FriendshipPoint", Property::Int(7_500));
        save_parameter.insert("PassiveSkillList", name_array(&["HP_ACC_up1"]));
        assert_eq!(
            validator_max_hp(&save_parameter, "Alpaca", &catalogs),
            1_509_000
        );
    }

    #[test]
    fn hp_ceiling_check_flags_only_beyond_tolerance() {
        let catalogs = catalogs();
        let mut save_parameter = Properties::default();
        save_parameter.insert("Level", byte_property(12));
        save_parameter.insert("Talent_HP", byte_property(30));
        save_parameter.insert("MaxHP", props::fixed_point64_property(300_000));
        // Far below the ceiling: clean.
        assert!(!detect_pal_issues(&save_parameter, "Alpaca", &catalogs).contains(&ILLEGAL_HP));
        save_parameter.insert("MaxHP", props::fixed_point64_property(3_000_000));
        assert!(detect_pal_issues(&save_parameter, "Alpaca", &catalogs).contains(&ILLEGAL_HP));
    }

    #[test]
    fn boss_ids_resolve_their_own_scaling_first() {
        let catalogs = catalogs();
        let mut save_parameter = Properties::default();
        save_parameter.insert("Level", byte_property(10));
        // BOSS_Believer_CrossBow is its own catalog entry with its own scaling.
        assert!(validator_max_hp(&save_parameter, "BOSS_Believer_CrossBow", &catalogs) > 0);
        assert_eq!(validator_max_hp(&save_parameter, "NotAPal", &catalogs), 0);
    }
}
