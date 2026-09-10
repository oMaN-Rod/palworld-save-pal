//! A rough "raw power" score behind the leaderboard's power ranking: the
//! validator's HP ceiling plus attack and defense estimated with the same
//! shape. It is a ranking heuristic for fun, not the game's stat formula, and
//! never feeds the legality checks.

use crate::props;
use crate::ue::Properties;

use crate::domain::pal::{param, AWAKENING_STATUS_MULTIPLY};

use super::catalogs::{OverviewCatalogs, SpeciesVitals};

/// `hp_ceiling` is the validator's [`super::illegal_pals::validator_max_hp`]
/// for the same pal.
pub(crate) fn pal_power_score(
    save_parameter: &Properties,
    vitals: SpeciesVitals,
    hp_ceiling: i64,
    catalogs: &OverviewCatalogs,
) -> i64 {
    let pal = PalStats::read(save_parameter);
    let hp = hp_ceiling as f64 / 1000.0;
    let attack_bonus = param(save_parameter, "PassiveSkillList")
        .and_then(props::name_values)
        .map_or(0.0, |passives| {
            passives
                .iter()
                .filter_map(|passive| catalogs.passive_attack_fraction(passive))
                .sum::<f64>()
        });
    let attack = pal.estimate(100.0, vitals.scaling_attack, pal.attack_iv, pal.attack_soul)
        * (1.0 + attack_bonus);
    let defense = pal.estimate(
        50.0,
        vitals.scaling_defense,
        pal.defense_iv,
        pal.defense_soul,
    );
    (hp + attack.floor() + defense.floor()) as i64
}

struct PalStats {
    level: f64,
    condenser: f64,
    multiplier: f64,
    attack_iv: f64,
    attack_soul: f64,
    defense_iv: f64,
    defense_soul: f64,
}

impl PalStats {
    fn read(save_parameter: &Properties) -> Self {
        let byte = |key: &str, default: u8| {
            param(save_parameter, key)
                .and_then(props::as_byte_number)
                .unwrap_or(default) as f64
        };
        let flag = |key: &str| {
            param(save_parameter, key)
                .and_then(props::as_bool)
                .unwrap_or(false)
        };
        let lucky = if flag("IsRarePal") { 1.2 } else { 1.0 };
        let awakened = if flag("bIsAwakening") {
            AWAKENING_STATUS_MULTIPLY
        } else {
            1.0
        };
        PalStats {
            level: byte("Level", 1),
            condenser: (byte("Rank", 1) - 1.0).max(0.0) * 0.05,
            multiplier: lucky * awakened,
            attack_iv: byte("Talent_Shot", 0),
            attack_soul: byte("Rank_Attack", 0),
            defense_iv: byte("Talent_Defense", 0),
            defense_soul: byte("Rank_Defence", 0),
        }
    }

    fn estimate(&self, base: f64, scaling: f64, iv: f64, soul: f64) -> f64 {
        let raw = base + 5.0 * self.level + scaling * 0.5 * self.level * (1.0 + iv / 100.0);
        raw * (1.0 + self.condenser) * (1.0 + soul * 0.03) * self.multiplier
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::overview::illegal_pals::validator_max_hp;
    use crate::gamedata::GameData;
    use crate::ue::{Byte, Property};

    fn score(save_parameter: &Properties, catalogs: &OverviewCatalogs) -> i64 {
        pal_power_score(
            save_parameter,
            catalogs.vitals_for("Alpaca").unwrap(),
            validator_max_hp(save_parameter, "Alpaca", catalogs),
            catalogs,
        )
    }

    fn catalogs() -> OverviewCatalogs {
        OverviewCatalogs::from_game_data(
            &GameData::from_entries([(
                "pals".to_string(),
                r#"{"Alpaca": {"is_pal": true, "scaling": {"hp": 90, "attack": 75, "defense": 90}, "friendship_hp": 4.5}}"#
                    .to_string(),
            )])
            .unwrap(),
        )
    }

    fn pal(level: u8) -> Properties {
        let mut save_parameter = Properties::default();
        save_parameter.insert("Level", Property::Byte(Byte::Byte(level)));
        save_parameter
    }

    /// hp 1000 (validator ceiling) + atk floor(100 + 50 + 75·0.5·10) = 525
    /// + def floor(50 + 50 + 90·0.5·10) = 550.
    #[test]
    fn score_matches_the_documented_shape() {
        assert_eq!(score(&pal(10), &catalogs()), 2075);
    }

    #[test]
    fn score_rises_with_level_ivs_souls_and_luck() {
        let catalogs = catalogs();
        let base = score(&pal(10), &catalogs);
        let leveled = score(&pal(50), &catalogs);
        assert!(leveled > base);

        let mut souped = pal(50);
        souped.insert("Talent_Shot", Property::Byte(Byte::Byte(100)));
        souped.insert("Rank_Attack", Property::Byte(Byte::Byte(10)));
        assert!(score(&souped, &catalogs) > leveled);

        let mut lucky = pal(10);
        lucky.insert("IsRarePal", Property::Bool(true));
        assert!(score(&lucky, &catalogs) > base);
    }
}
