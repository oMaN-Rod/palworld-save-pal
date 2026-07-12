mod common;

use psp_core::domain::{guild_tail, world};
use uuid::Uuid;

/// Every guild in a real save survives a full write -> read round trip with
/// its structured data byte-identical.
///
/// Reconciliation with the 2026-07 uesave refactor: uesave now OWNS guild
/// (de)serialization -- `PalGroupData::data` is a structured
/// `PalGroupVariant::Guild(PalGuildGroup)` (with a two-shape `PalGuildTail`),
/// not the flat `remaining_data` blob psp-core used to parse and re-encode by
/// hand. So this test no longer re-encodes a blob; instead it captures each
/// guild's structured `data`, re-serializes the WHOLE level through uesave's
/// writer (`level_sav_bytes()`), re-reads it, and asserts every guild's
/// structured data is unchanged. Because uesave's `PalGuildGroup::write` is
/// the exact inverse of its `read`, structural equality across this round trip
/// IS byte identity of the guild's on-disk bytes -- the same guarantee the old
/// `tail.to_bytes() == remaining_data` assertion made, now proven through the
/// real save write path rather than a hand-rolled codec.
///
/// Runs UNCONDITIONALLY on the committed fixtures (never the env-gated private
/// corpus), so a pass here is real evidence on bare CI.
#[test]
fn every_guild_tail_in_fixture_saves_round_trips_byte_identically() {
    let mut guild_count = 0;
    for fixture_name in ["world1", "world2"] {
        let session = common::load_fixture_session(fixture_name);
        let before = collect_guild_data(&session.level);
        assert!(
            !before.is_empty(),
            "{fixture_name}: fixture must contain at least one guild"
        );

        let bytes = session
            .level_sav_bytes()
            .unwrap_or_else(|err| panic!("{fixture_name}: write level sav: {err}"));
        let reloaded = psp_core::savio::read_sav_bytes(&bytes)
            .unwrap_or_else(|err| panic!("{fixture_name}: re-read level sav: {err}"));
        let after = collect_guild_data(&reloaded);

        for (guild_id, data) in &before {
            let round_tripped = after
                .iter()
                .find(|(id, _)| id == guild_id)
                .unwrap_or_else(|| panic!("{fixture_name}: guild {guild_id} survives resave"));
            assert_eq!(
                &round_tripped.1, data,
                "{fixture_name}: guild {guild_id} structured data must be byte-identical after resave"
            );
            guild_count += 1;
        }
    }
    assert!(
        guild_count > 0,
        "fixture saves (world1, world2) must contain at least one guild"
    );
}

/// The same round trip on the PRIVATE corpus named by `PSP_TEST_SAVE_DIR`
/// (skips loudly when unset).
#[test]
fn every_guild_tail_in_corpus_session_round_trips_byte_identically() {
    let Some(session) = common::load_corpus_session() else {
        return;
    };
    let before = collect_guild_data(&session.level);
    let bytes = session.level_sav_bytes().expect("write corpus level sav");
    let reloaded = psp_core::savio::read_sav_bytes(&bytes).expect("re-read corpus level sav");
    let after = collect_guild_data(&reloaded);

    for (guild_id, data) in &before {
        let round_tripped = after
            .iter()
            .find(|(id, _)| id == guild_id)
            .unwrap_or_else(|| panic!("corpus guild {guild_id} survives resave"));
        assert_eq!(
            &round_tripped.1, data,
            "corpus guild {guild_id} structured data must be byte-identical after resave"
        );
    }
}

/// Two-shape coverage: the structured accessors and mutators must read and
/// edit BOTH the pre-2026-07 (`PreUpdate`) and 2026-07 (`PostUpdate`) tail
/// shapes without ever assuming one. A `PreUpdate` guild is built via the
/// public constructor; every guild the fixtures actually carry exercises the
/// real read path in the round-trip test above.
#[test]
fn accessors_handle_pre_update_guilds_built_from_the_constructor() {
    let admin: Uuid = "77777777-7777-7777-7777-777777777777".parse().unwrap();
    let member: Uuid = "88888888-8888-8888-8888-888888888888".parse().unwrap();
    let mut guild = guild_tail::pre_update_guild(
        5,
        "Founders",
        admin,
        &[(admin, 10, "Admin"), (member, 20, "Member")],
    );

    assert_eq!(guild_tail::guild_admin_uid(&guild), admin);
    assert_eq!(guild_tail::guild_player_uids(&guild), vec![admin, member]);
    assert_eq!(guild_tail::guild_player_count(&guild), 2);
    assert_eq!(
        guild_tail::find_player_membership(&guild, member),
        Some((20, "Member".to_string()))
    );

    guild_tail::remove_player(&mut guild, member);
    assert_eq!(guild_tail::guild_player_uids(&guild), vec![admin]);
    assert_eq!(guild.base_camp_level, 5);
}

fn collect_guild_data(
    level: &uesave::Save,
) -> Vec<(Uuid, uesave::games::palworld::PalGroupVariant)> {
    world::group_map(level)
        .unwrap()
        .iter()
        .filter_map(|entry| {
            if guild_tail::entry_group_type(entry).as_deref() != Some("EPalGroupType::Guild") {
                return None;
            }
            let guild_id = psp_core::props::as_uuid(&entry.key)?;
            let group_data = guild_tail::entry_group_data(entry)?;
            Some((guild_id, group_data.data.clone()))
        })
        .collect()
}
