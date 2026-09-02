//! The player leaderboard: every known player with the full bundle of
//! ranking metrics (pal count, lucky count, average/max pal level, summed
//! raw power, DPS storage count).
//!
//! Rows come from the guild rosters (name + membership survive even when a
//! player's own save file is absent from the world) joined with the eager
//! player summaries (save-file nicknames for players with saves). All pal
//! metrics come from a tally over the character map — the same source the
//! reference implementation uses — so players without save files still rank.
//! Rows leave here ordered by pal count (the default ranking, ties broken
//! deterministically); the dashboard re-sorts client-side per metric using
//! the same tie-break ladder: primary metric, then pal count, then level,
//! then nickname.

use std::collections::{HashMap, HashSet};

use crate::dto::overview::OverviewPlayerRow;
use crate::session::SaveSession;
use uuid::Uuid;

use super::composition::round1;

/// Per-owner ranking metrics tallied during the character-map pass. `pal_count`
/// includes corrupt-but-owned entries; the level/power fields only advance for
/// entries with a readable `CharacterID`, so `leveled_count` gates the
/// level-derived averages.
#[derive(Debug, Default, Clone)]
pub(crate) struct OwnerMetrics {
    pub(crate) pal_count: i64,
    pub(crate) lucky_count: i64,
    pub(crate) level_sum: i64,
    pub(crate) leveled_count: i64,
    pub(crate) max_pal_level: i64,
    pub(crate) total_power: i64,
}

impl OwnerMetrics {
    /// Adds one owned pal whose `SaveParameter` was fully readable.
    pub(crate) fn note_pal(&mut self, level: i64, lucky: bool, power: i64) {
        self.leveled_count += 1;
        self.level_sum += level;
        self.max_pal_level = self.max_pal_level.max(level);
        if lucky {
            self.lucky_count += 1;
        }
        self.total_power += power;
    }

    /// Mean pal level at one decimal (banker's rounding, like the composition
    /// averages); `None` when no readable pal was tallied.
    pub(crate) fn avg_pal_level(&self) -> Option<f64> {
        (self.leveled_count > 0).then(|| round1(self.level_sum as f64 / self.leveled_count as f64))
    }

    pub(crate) fn max_pal_level(&self) -> Option<i64> {
        (self.leveled_count > 0).then_some(self.max_pal_level)
    }
}

/// Every known player ranked by owned-pal count (the default view).
///
/// Candidates are the union of guild-roster members (in guild-tail order,
/// deduplicated) and players with eager summaries (in summary order),
/// mirroring the reference implementation's join of guild rosters with
/// precomputed counts/levels. The wire carries all of them — the frontend
/// truncates per view — sorted by pal count, then level, then nickname.
pub(crate) fn top_players(
    session: &SaveSession,
    roster: &[(Uuid, String)],
    owner_metrics: &HashMap<Uuid, OwnerMetrics>,
    player_levels: &HashMap<Uuid, i64>,
    dps_pal_counts: &HashMap<Uuid, i64>,
) -> Vec<OverviewPlayerRow> {
    let mut candidates: Vec<(Uuid, String)> = Vec::new();
    let mut seen: HashSet<Uuid> = HashSet::new();
    for (uid, name) in roster {
        if seen.insert(*uid) {
            candidates.push((*uid, name.clone()));
        }
    }
    for uid in &session.player_summary_order {
        if seen.insert(*uid) {
            let nickname = session
                .player_summaries
                .get(uid)
                .map(|summary| summary.nickname.clone())
                .unwrap_or_default();
            candidates.push((*uid, nickname));
        }
    }

    let mut rows: Vec<OverviewPlayerRow> = candidates
        .into_iter()
        .map(|(uid, roster_name)| {
            let summary = session.player_summaries.get(&uid);
            let metrics = owner_metrics.get(&uid);
            let nickname = summary
                .map(|summary| summary.nickname.clone())
                .unwrap_or(roster_name);
            OverviewPlayerRow {
                uid,
                nickname: nickname.clone(),
                level: player_levels
                    .get(&uid)
                    .copied()
                    .or_else(|| summary.and_then(|summary| summary.level)),
                pal_count: metrics
                    .map(|metrics| metrics.pal_count)
                    .unwrap_or_else(|| summary.map(|summary| summary.pal_count).unwrap_or(0)),
                lucky_count: metrics.map(|metrics| metrics.lucky_count).unwrap_or(0),
                avg_pal_level: metrics.and_then(|metrics| metrics.avg_pal_level()),
                max_pal_level: metrics.and_then(|metrics| metrics.max_pal_level()),
                total_power: metrics.map(|metrics| metrics.total_power).unwrap_or(0),
                dps_pal_count: dps_pal_counts.get(&uid).copied().unwrap_or(0),
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        b.pal_count
            .cmp(&a.pal_count)
            .then_with(|| b.level.unwrap_or(0).cmp(&a.level.unwrap_or(0)))
            .then_with(|| a.nickname.to_lowercase().cmp(&b.nickname.to_lowercase()))
    });
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::summary::PlayerSummary;
    use crate::session::{SaveKind, SaveSession};
    use crate::ue::{Properties, Property, StructValue};

    fn summary(uid: &str, nickname: &str, level: i64, pal_count: i64) -> PlayerSummary {
        PlayerSummary {
            uid: uid.parse().unwrap(),
            nickname: nickname.to_string(),
            level: Some(level),
            guild_id: None,
            pal_count,
            last_online_time: None,
            loaded: false,
        }
    }

    fn session_with_players(order: Vec<(&str, PlayerSummary)>) -> SaveSession {
        let mut root_properties = Properties::default();
        root_properties.insert(
            "worldSaveData",
            Property::Struct(StructValue::Struct(Properties::default())),
        );
        let level = crate::ue::Save {
            header: crate::ue::Header {
                magic: 0,
                save_game_version: 0,
                package_version: crate::ue::PackageVersion { ue4: 0, ue5: None },
                engine_version_major: 0,
                engine_version_minor: 0,
                engine_version_patch: 0,
                engine_version_build: 0,
                engine_version: String::new(),
                custom_version: None,
            },
            schemas: crate::ue::PropertySchemas::default(),
            root: crate::ue::Root {
                save_game_type: String::new(),
                properties: root_properties,
            },
            extra: Vec::new(),
        };
        let mut session = SaveSession::new_for_tests(SaveKind::InMemory, level);
        for (uid, player_summary) in order {
            session.player_summary_order.push(uid.parse().unwrap());
            session
                .player_summaries
                .insert(uid.parse().unwrap(), player_summary);
        }
        session
    }

    fn uid(text: &str) -> Uuid {
        text.parse().unwrap()
    }

    #[test]
    fn ranks_by_pal_count_with_deterministic_ties_and_carries_metrics() {
        let session = session_with_players(vec![
            (
                "22222222-2222-2222-2222-222222222222",
                summary("22222222-2222-2222-2222-222222222222", "Second", 40, 12),
            ),
            (
                "11111111-1111-1111-1111-111111111111",
                summary("11111111-1111-1111-1111-111111111111", "First", 10, 3),
            ),
            (
                "55555555-5555-5555-5555-555555555555",
                summary("55555555-5555-5555-5555-555555555555", "Fifth", 20, 7),
            ),
            (
                "88888888-8888-8888-8888-888888888888",
                summary("88888888-8888-8888-8888-888888888888", "AlsoTie", 42, 3),
            ),
        ]);
        let roster: Vec<(Uuid, String)> = vec![
            (
                uid("33333333-3333-3333-3333-333333333333"),
                "RosterOnly".into(),
            ),
            (uid("22222222-2222-2222-2222-222222222222"), "Second".into()),
            (uid("11111111-1111-1111-1111-111111111111"), "First".into()),
            (uid("99999999-9999-9999-9999-999999999999"), "TieA".into()),
            (
                uid("77777777-7777-7777-7777-777777777777"),
                "ZeroPal".into(),
            ),
        ];
        let mut owner_metrics = HashMap::new();
        let mut second = OwnerMetrics {
            pal_count: 12,
            ..OwnerMetrics::default()
        };
        second.note_pal(50, true, 2075);
        second.note_pal(30, false, 1000);
        owner_metrics.insert(uid("22222222-2222-2222-2222-222222222222"), second);
        let mut third = OwnerMetrics {
            pal_count: 6,
            ..OwnerMetrics::default()
        };
        third.note_pal(12, false, 900);
        owner_metrics.insert(uid("33333333-3333-3333-3333-333333333333"), third);
        let mut tie_a = OwnerMetrics {
            pal_count: 5,
            ..OwnerMetrics::default()
        };
        tie_a.note_pal(7, false, 400);
        owner_metrics.insert(uid("99999999-9999-9999-9999-999999999999"), tie_a);
        let mut also_tie = OwnerMetrics {
            pal_count: 5,
            ..OwnerMetrics::default()
        };
        also_tie.note_pal(9, false, 500);
        owner_metrics.insert(uid("88888888-8888-8888-8888-888888888888"), also_tie);
        let first_metrics = OwnerMetrics {
            pal_count: 3,
            ..OwnerMetrics::default()
        };
        owner_metrics.insert(uid("11111111-1111-1111-1111-111111111111"), first_metrics);
        let mut player_levels = HashMap::new();
        player_levels.insert(uid("33333333-3333-3333-3333-333333333333"), 33);
        player_levels.insert(uid("88888888-8888-8888-8888-888888888888"), 42);
        let mut dps_pal_counts = HashMap::new();
        dps_pal_counts.insert(uid("22222222-2222-2222-2222-222222222222"), 4);

        let top = top_players(
            &session,
            &roster,
            &owner_metrics,
            &player_levels,
            &dps_pal_counts,
        );
        // No truncation: every candidate ranks, including ZeroPal at the tail.
        let nicknames: Vec<&str> = top.iter().map(|row| row.nickname.as_str()).collect();
        // Second (12), Fifth (7, summary fallback), RosterOnly (6, map level
        // 33), then the 5-tie broken by level: AlsoTie (map level 42) before
        // roster-only TieA (no level), then First (3), ZeroPal (0).
        assert_eq!(
            nicknames,
            vec![
                "Second",
                "Fifth",
                "RosterOnly",
                "AlsoTie",
                "TieA",
                "First",
                "ZeroPal"
            ]
        );

        let second = &top[0];
        assert_eq!(second.level, Some(40));
        assert_eq!(second.pal_count, 12);
        assert_eq!(second.lucky_count, 1);
        assert_eq!(second.avg_pal_level, Some(40.0));
        assert_eq!(second.max_pal_level, Some(50));
        assert_eq!(second.total_power, 3075);
        assert_eq!(second.dps_pal_count, 4);

        // No readable pals → level metrics are None, counts fall back to the
        // summary's pal_count.
        let first = top.iter().find(|row| row.nickname == "First").unwrap();
        assert_eq!(first.pal_count, 3);
        assert_eq!(first.avg_pal_level, None);
        assert_eq!(first.max_pal_level, None);
        assert_eq!(first.lucky_count, 0);
        assert_eq!(first.total_power, 0);

        let roster_only = top.iter().find(|row| row.nickname == "RosterOnly").unwrap();
        assert_eq!(roster_only.level, Some(33));
        assert_eq!(roster_only.avg_pal_level, Some(12.0));
    }

    #[test]
    fn avg_pal_level_rounds_to_one_decimal() {
        let mut metrics = OwnerMetrics::default();
        metrics.note_pal(30, false, 0);
        metrics.note_pal(50, false, 0);
        metrics.note_pal(1, false, 0);
        // 81 / 3 = 27 exactly.
        assert_eq!(metrics.avg_pal_level(), Some(27.0));
        metrics.note_pal(2, false, 0);
        // 83 / 4 = 20.75 → 20.8 under round-ties-even? 20.75*10=207.5 → 208 → 20.8.
        assert_eq!(metrics.avg_pal_level(), Some(20.8));
        assert_eq!(metrics.max_pal_level(), Some(50));
    }

    #[test]
    fn empty_roster_falls_back_to_summary_players() {
        let session = session_with_players(vec![(
            "11111111-1111-1111-1111-111111111111",
            summary("11111111-1111-1111-1111-111111111111", "Solo", 9, 4),
        )]);
        let top = top_players(
            &session,
            &[],
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
        );
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].nickname, "Solo");
        assert_eq!(top[0].pal_count, 4);
        assert_eq!(top[0].level, Some(9));
        assert_eq!(top[0].dps_pal_count, 0);
    }

    #[test]
    fn empty_sessions_yield_an_empty_leaderboard() {
        let session = session_with_players(vec![]);
        assert!(
            top_players(
                &session,
                &[],
                &HashMap::new(),
                &HashMap::new(),
                &HashMap::new()
            )
            .is_empty()
        );
    }
}
