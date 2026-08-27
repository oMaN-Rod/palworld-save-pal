//! Pal composition accumulation: level brackets, gender split, talent
//! averages, and the top-passive/top-active skill tallies.

use crate::dto::overview::{
    OverviewComposition, OverviewGenderSplit, OverviewLevelBracket, OverviewSkillCount,
    OverviewTalentAvg,
};
use crate::props;
use crate::ue::Properties;

use crate::domain::pal::param;

use super::classify::gender_bucket;

/// First-seen-ordered counter: `most_common` in Python, without the
/// nondeterministic HashMap iteration a plain counter would introduce.
pub(crate) struct OrderedCounter {
    counts: std::collections::HashMap<String, i64>,
    order: Vec<String>,
}

impl OrderedCounter {
    pub(crate) fn new() -> Self {
        OrderedCounter {
            counts: std::collections::HashMap::new(),
            order: Vec::new(),
        }
    }

    pub(crate) fn add(&mut self, key: String) {
        if let Some(count) = self.counts.get_mut(&key) {
            *count += 1;
        } else {
            self.order.push(key.clone());
            self.counts.insert(key, 1);
        }
    }

    /// Top `limit` by count descending; first-seen order breaks ties.
    pub(crate) fn top(&self, limit: usize) -> Vec<OverviewSkillCount> {
        let mut ranked: Vec<&String> = self.order.iter().collect();
        ranked.sort_by(|a, b| self.counts[*b].cmp(&self.counts[*a]));
        ranked
            .into_iter()
            .take(limit)
            .map(|key| OverviewSkillCount {
                skill: key.clone(),
                count: self.counts[key],
            })
            .collect()
    }

    pub(crate) fn len(&self) -> usize {
        self.order.len()
    }
}

/// Python `round(x, 1)` (banker's rounding) — the reference averages use it,
/// and half-away-from-zero would drift the displayed tenths on exact `.x5`.
pub(crate) fn round1(value: f64) -> f64 {
    (value * 10.0).round_ties_even() / 10.0
}

fn bracket_index(level: i64) -> usize {
    if level <= 20 {
        0
    } else if level <= 40 {
        1
    } else if level <= 60 {
        2
    } else {
        3
    }
}

/// Running totals for the composition card. `add` is called once for every
/// non-player entry with a readable `SaveParameter` — including ones too
/// corrupt to classify for the totals, matching the reference behavior.
pub(crate) struct CompositionAccumulator {
    count: i64,
    sum_level: i64,
    gender: OverviewGenderSplit,
    brackets: [i64; 4],
    talent_sums: [i64; 3],
    passives: OrderedCounter,
    actives: OrderedCounter,
}

impl CompositionAccumulator {
    pub(crate) fn new() -> Self {
        CompositionAccumulator {
            count: 0,
            sum_level: 0,
            gender: OverviewGenderSplit::default(),
            brackets: [0; 4],
            talent_sums: [0; 3],
            passives: OrderedCounter::new(),
            actives: OrderedCounter::new(),
        }
    }

    pub(crate) fn add(&mut self, save_parameter: &Properties) {
        self.count += 1;
        let level = param(save_parameter, "Level")
            .and_then(props::as_byte_number)
            .unwrap_or(1) as i64;
        self.sum_level += level;
        self.brackets[bracket_index(level)] += 1;

        let entry_gender = gender_bucket(save_parameter);
        self.gender.male += entry_gender.male;
        self.gender.female += entry_gender.female;
        self.gender.unknown += entry_gender.unknown;

        self.talent_sums[0] += param(save_parameter, "Talent_HP")
            .and_then(props::as_byte_number)
            .unwrap_or(0) as i64;
        self.talent_sums[1] += param(save_parameter, "Talent_Shot")
            .and_then(props::as_byte_number)
            .unwrap_or(0) as i64;
        self.talent_sums[2] += param(save_parameter, "Talent_Defense")
            .and_then(props::as_byte_number)
            .unwrap_or(0) as i64;

        for passive in param(save_parameter, "PassiveSkillList")
            .and_then(props::name_values)
            .into_iter()
            .flatten()
        {
            if !passive.is_empty() {
                self.passives.add(passive.clone());
            }
        }
        for active in param(save_parameter, "EquipWaza")
            .and_then(props::enum_values)
            .into_iter()
            .flatten()
        {
            if !active.is_empty() {
                self.actives.add(active.clone());
            }
        }
    }

    pub(crate) fn finish(self) -> OverviewComposition {
        let mut composition = OverviewComposition {
            avg_level: 0.0,
            gender: self.gender,
            level_brackets: vec![
                OverviewLevelBracket {
                    label: "1-20",
                    count: self.brackets[0],
                },
                OverviewLevelBracket {
                    label: "21-40",
                    count: self.brackets[1],
                },
                OverviewLevelBracket {
                    label: "41-60",
                    count: self.brackets[2],
                },
                OverviewLevelBracket {
                    label: "61-80",
                    count: self.brackets[3],
                },
            ],
            talent_avg: OverviewTalentAvg::default(),
            top_passives: self.passives.top(10),
            top_actives: self.actives.top(10),
        };
        if self.count > 0 {
            let count = self.count as f64;
            composition.avg_level = round1(self.sum_level as f64 / count);
            composition.talent_avg = OverviewTalentAvg {
                hp: round1(self.talent_sums[0] as f64 / count),
                attack: round1(self.talent_sums[1] as f64 / count),
                defense: round1(self.talent_sums[2] as f64 / count),
            };
        }
        composition
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ue::{Byte, Property, ValueVec};

    fn byte_property(value: u8) -> Property {
        Property::Byte(Byte::Byte(value))
    }

    #[test]
    fn ordered_counter_ranks_stably() {
        let mut counter = OrderedCounter::new();
        for key in ["a", "b", "a", "c", "b", "a"] {
            counter.add(key.to_string());
        }
        let top = counter.top(2);
        assert_eq!(top[0].skill, "a");
        assert_eq!(top[0].count, 3);
        assert_eq!(top[1].skill, "b");
        assert_eq!(top[1].count, 2);
        assert_eq!(counter.len(), 3);
    }

    #[test]
    fn round1_uses_bankers_rounding_like_the_reference() {
        assert_eq!(round1(66.25), 66.2);
        assert_eq!(round1(70.25), 70.2);
        assert_eq!(round1(66.35), 66.4);
        assert_eq!(round1(0.0), 0.0);
    }

    #[test]
    fn accumulator_buckets_levels_genders_and_skills() {
        let mut accumulator = CompositionAccumulator::new();

        let mut pal = Properties::default();
        pal.insert("Level", byte_property(30));
        pal.insert("Gender", Property::Enum("EPalGenderType::Male".into()));
        pal.insert("Talent_HP", byte_property(10));
        pal.insert(
            "PassiveSkillList",
            Property::Array(ValueVec::Name(vec!["Legend".to_string()])),
        );
        pal.insert(
            "EquipWaza",
            Property::Array(ValueVec::Enum(vec!["EPalWazaID::AirCanon".to_string()])),
        );
        accumulator.add(&pal);

        let mut pal = Properties::default();
        pal.insert("Level", byte_property(50));
        pal.insert("Gender", Property::Enum("EPalGenderType::Female".into()));
        accumulator.add(&pal);

        let mut pal = Properties::default();
        pal.insert("Level", byte_property(200));
        pal.insert("Talent_HP", byte_property(255));
        accumulator.add(&pal);

        let composition = accumulator.finish();
        assert_eq!(composition.gender.male, 1);
        assert_eq!(composition.gender.female, 1);
        assert_eq!(composition.gender.unknown, 1);
        assert_eq!(
            composition
                .level_brackets
                .iter()
                .map(|bracket| (bracket.label, bracket.count))
                .collect::<Vec<_>>(),
            vec![("1-20", 0), ("21-40", 1), ("41-60", 1), ("61-80", 1)]
        );
        // (10 + 0 + 255) / 3 = 88.33… → 88.3.
        assert_eq!(composition.talent_avg.hp, 88.3);
        // (30 + 50 + 200) / 3 = 93.33… → 93.3.
        assert_eq!(composition.avg_level, 93.3);
        assert_eq!(composition.top_passives[0].skill, "Legend");
        assert_eq!(composition.top_actives[0].skill, "EPalWazaID::AirCanon");
    }

    #[test]
    fn empty_accumulator_finishes_with_zeroed_averages() {
        let composition = CompositionAccumulator::new().finish();
        assert_eq!(composition.avg_level, 0.0);
        assert_eq!(composition.talent_avg.hp, 0.0);
        assert!(composition.top_passives.is_empty());
        assert!(composition.level_brackets.iter().all(|b| b.count == 0));
    }
}
