//! The per-pal "raw power" score summed into the leaderboard's total-power
//! ranking. HP uses the game-accurate [`super::illegal_pals::validator_max_hp`]
//! ceiling; attack and defense are estimates built on the community-datamined
//! stat structure (base 100 attack / 50 defense at level 0, `+5/level`, plus
//! `scaling × 0.5 × level × (1 + IV/100)`), with the same condenser / soul /
//! awakening multipliers the HP formula applies. A lucky pal's 1.2× alpha is
//! applied here too; boss-prefixed species are not boosted because a boss
//! variant that matters resolves to its own higher `pals.json` scaling. The
//! score only orders players — it never gates anything — so cross-version
//! drift in the estimate shifts every roster equally and is acceptable.

use crate::props;
use crate::ue::Properties;

use crate::domain::pal::{AWAKENING_STATUS_MULTIPLY, param};

use super::catalogs::OverviewCatalogs;
use super::illegal_pals::validator_max_hp;

/// One pal's combat score in whole stat points: estimated max HP + attack +
/// defense. Unknown species (no `pals.json` vitals) score 0.
pub(crate) fn pal_power_score(
    save_parameter: &Properties,
    character_id: &str,
    catalogs: &OverviewCatalogs,
) -> i64 {
    let Some(vitals) = catalogs.vitals_for(character_id) else {
        return 0;
    };

    let hp = validator_max_hp(save_parameter, character_id, catalogs) as f64 / 1000.0;
    let attack = stat_estimate(
        100.0,
        vitals.scaling_attack,
        save_parameter,
        "Talent_Shot",
        "Rank_Attack",
        catalogs,
        true,
    );
    let defense = stat_estimate(
        50.0,
        vitals.scaling_defense,
        save_parameter,
        "Talent_Defense",
        "Rank_Defence",
        catalogs,
        false,
    );
    (hp + attack + defense) as i64
}

/// The estimated attack or defense stat. `with_passive_bonus` folds in summed
/// Attack% passives (defense passives are rare and skipped — this is a
/// ranking heuristic, not a simulation).
#[allow(clippy::too_many_arguments)]
fn stat_estimate(
    base: f64,
    scaling: f64,
    save_parameter: &Properties,
    talent_key: &str,
    soul_key: &str,
    catalogs: &OverviewCatalogs,
    with_passive_bonus: bool,
) -> f64 {
    let level = byte_or(save_parameter, "Level", 1) as f64;
    let rank = byte_or(save_parameter, "Rank", 1) as f64;
    let iv = byte_or(save_parameter, talent_key, 0) as f64;
    let soul = byte_or(save_parameter, soul_key, 0) as f64;
    let is_lucky = param(save_parameter, "IsRarePal")
        .and_then(props::as_bool)
        .unwrap_or(false);
    let awakened = param(save_parameter, "bIsAwakening")
        .and_then(props::as_bool)
        .unwrap_or(false);

    let passive_bonus = if with_passive_bonus {
        param(save_parameter, "PassiveSkillList")
            .and_then(props::name_values)
            .map(|passives| {
                passives
                    .iter()
                    .filter_map(|passive| catalogs.passive_attack_fraction(passive))
                    .sum::<f64>()
            })
            .unwrap_or(0.0)
    } else {
        0.0
    };

    let raw = base + 5.0 * level + scaling * 0.5 * level * (1.0 + iv / 100.0);
    let condenser = (rank - 1.0).max(0.0) * 0.05;
    let alpha = if is_lucky { 1.2 } else { 1.0 };
    let awakening = if awakened {
        AWAKENING_STATUS_MULTIPLY
    } else {
        1.0
    };
    (raw * (1.0 + condenser) * (1.0 + soul * 0.03) * alpha * awakening * (1.0 + passive_bonus))
        .floor()
}

fn byte_or(save_parameter: &Properties, key: &str, default: u8) -> u8 {
    param(save_parameter, key)
        .and_then(props::as_byte_number)
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gamedata::GameData;
    use crate::ue::{Byte, Property};

    fn catalogs() -> OverviewCatalogs {
        OverviewCatalogs::from_game_data(&GameData::from_entries([(
            "pals".to_string(),
            r#"{
                "Alpaca": {"is_pal": true, "scaling": {"hp": 90, "attack": 75, "defense": 90}, "friendship_hp": 4.5}
            }"#
            .to_string(),
        )])
        .unwrap())
    }

    fn pal(level: u8) -> Properties {
        let mut save_parameter = Properties::default();
        save_parameter.insert("Level", Property::Byte(Byte::Byte(level)));
        save_parameter
    }

    #[test]
    fn unknown_species_scores_zero() {
        assert_eq!(pal_power_score(&pal(10), "NotAPal", &catalogs()), 0);
    }

    #[test]
    fn score_rises_with_level_ivs_and_souls() {
        let catalogs = catalogs();
        let base = pal_power_score(&pal(10), "Alpaca", &catalogs);

        let leveled = pal_power_score(&pal(50), "Alpaca", &catalogs);
        assert!(leveled > base);

        let mut souped = pal(50);
        souped.insert("Talent_Shot", Property::Byte(Byte::Byte(100)));
        souped.insert("Rank_Attack", Property::Byte(Byte::Byte(10)));
        let souped_score = pal_power_score(&souped, "Alpaca", &catalogs);
        assert!(souped_score > leveled);
        assert!(souped_score > 0);
    }

    /// Locks the estimate's arithmetic: level 10 Alpaca (hp 90, atk 75, def 90),
    /// rank 1, no souls/IVs/passives →
    /// hp  = floor(500 + 50 + 90·0.5·10) = 1000
    /// atk = floor(100 + 50 + 75·0.5·10) = 525
    /// def = floor(50 + 50 + 90·0.5·10)  = 550
    /// total 2075.
    #[test]
    fn score_matches_the_documented_formula() {
        assert_eq!(pal_power_score(&pal(10), "Alpaca", &catalogs()), 2075);
    }

    #[test]
    fn lucky_pals_score_higher() {
        let catalogs = catalogs();
        let mut lucky = pal(10);
        lucky.insert("IsRarePal", Property::Bool(true));
        assert!(
            pal_power_score(&lucky, "Alpaca", &catalogs)
                > pal_power_score(&pal(10), "Alpaca", &catalogs)
        );
    }
}
