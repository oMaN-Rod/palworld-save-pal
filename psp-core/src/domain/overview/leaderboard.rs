//! The player leaderboard: top players ranked by how many pals they own.
//!
//! Rows come from the guild rosters (name + membership survive even when a
//! player's own save file is absent from the world) joined with the eager
//! player summaries (save-file nicknames for players with saves). Pal counts
//! and levels come from a tally over the character map — the same source the
//! reference implementation uses — so players without save files still rank.

use std::collections::{HashMap, HashSet};

use crate::dto::overview::OverviewPlayerRow;
use crate::session::SaveSession;
use uuid::Uuid;

/// How many players the overview leaderboard previews.
pub(crate) const LEADERBOARD_SIZE: usize = 6;

/// The top [`LEADERBOARD_SIZE`] players by owned-pal count.
///
/// Candidates are the union of guild-roster members (in guild-tail order,
/// deduplicated) and players with eager summaries (in summary order),
/// mirroring the reference implementation's join of guild rosters with
/// precomputed counts/levels. A stable sort by pal count keeps equal counts
/// in that first-seen order, like the reference's stable `sorted(..., reverse)`
/// over roster rows.
pub(crate) fn top_players(
    session: &SaveSession,
    roster: &[(Uuid, String)],
    owner_counts: &HashMap<Uuid, i64>,
    player_levels: &HashMap<Uuid, i64>,
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
            OverviewPlayerRow {
                uid,
                nickname: summary
                    .map(|summary| summary.nickname.clone())
                    .unwrap_or(roster_name),
                level: player_levels
                    .get(&uid)
                    .copied()
                    .or_else(|| summary.and_then(|summary| summary.level)),
                pal_count: owner_counts
                    .get(&uid)
                    .copied()
                    .unwrap_or_else(|| summary.map(|summary| summary.pal_count).unwrap_or(0)),
            }
        })
        .collect();
    rows.sort_by_key(|row| std::cmp::Reverse(row.pal_count));
    rows.truncate(LEADERBOARD_SIZE);
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
    fn ranks_by_pal_count_and_caps_at_leaderboard_size() {
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
        // Roster order deliberately differs from summary order; RosterOnly and
        // TieA have no save files (names come from the roster).
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
        let mut owner_counts = HashMap::new();
        owner_counts.insert(uid("33333333-3333-3333-3333-333333333333"), 6);
        owner_counts.insert(uid("22222222-2222-2222-2222-222222222222"), 12);
        owner_counts.insert(uid("11111111-1111-1111-1111-111111111111"), 3);
        owner_counts.insert(uid("55555555-5555-5555-5555-555555555555"), 7);
        owner_counts.insert(uid("99999999-9999-9999-9999-999999999999"), 5);
        owner_counts.insert(uid("88888888-8888-8888-8888-888888888888"), 5);
        owner_counts.insert(uid("77777777-7777-7777-7777-777777777777"), 0);
        let mut player_levels = HashMap::new();
        player_levels.insert(uid("33333333-3333-3333-3333-333333333333"), 33);

        let top = top_players(&session, &roster, &owner_counts, &player_levels);
        assert_eq!(top.len(), LEADERBOARD_SIZE);
        let nicknames: Vec<&str> = top.iter().map(|row| row.nickname.as_str()).collect();
        // Second (12), Fifth (7), RosterOnly (6, roster name + map level),
        // then the 5-tie: roster member TieA before summary-only AlsoTie
        // (which uses the map tally, not its summary count of 3), then First (3).
        // ZeroPal (0) falls outside the top 6.
        assert_eq!(
            nicknames,
            vec!["Second", "Fifth", "RosterOnly", "TieA", "AlsoTie", "First"]
        );
        assert_eq!(top[2].level, Some(33));
        assert_eq!(top[4].pal_count, 5);
        assert_eq!(top[3].nickname, "TieA");
    }

    #[test]
    fn empty_roster_falls_back_to_summary_players() {
        let session = session_with_players(vec![(
            "11111111-1111-1111-1111-111111111111",
            summary("11111111-1111-1111-1111-111111111111", "Solo", 9, 4),
        )]);
        let top = top_players(&session, &[], &HashMap::new(), &HashMap::new());
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].nickname, "Solo");
        assert_eq!(top[0].pal_count, 4);
        assert_eq!(top[0].level, Some(9));
    }

    #[test]
    fn empty_sessions_yield_an_empty_leaderboard() {
        let session = session_with_players(vec![]);
        assert!(top_players(&session, &[], &HashMap::new(), &HashMap::new()).is_empty());
    }
}
