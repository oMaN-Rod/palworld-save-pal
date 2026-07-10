mod common;

use psp_core::domain::{guild_tail, world};

/// Deviation from the brief: the brief's version of this test gated itself
/// on `common::load_corpus_session()` (an optional `PSP_TEST_SAVE_DIR` env
/// var), which is unset in this environment and would silently skip,
/// producing no evidence at all. The task instructions are explicit that the
/// checked-in fixtures at `tests/fixtures/saves/world1` (2 players) and
/// `world2` (1 player) must be used instead, via the always-present
/// `common::load_fixture_session` helper -- this never skips, so a pass here
/// is real evidence the encoder matches real save bytes.
#[test]
fn every_guild_tail_in_fixture_saves_round_trips_byte_identically() {
    let mut guild_count = 0;
    for fixture_name in ["world1", "world2"] {
        let session = common::load_fixture_session(fixture_name);
        let groups = world::group_map(&session.level).unwrap();
        for entry in groups {
            if guild_tail::entry_group_type(entry).as_deref() != Some("EPalGroupType::Guild") {
                continue;
            }
            let group_data = guild_tail::entry_group_data(entry).expect("typed PalGroupData");
            let original_bytes = &group_data.remaining_data;
            let tail = guild_tail::GuildTail::parse(original_bytes)
                .unwrap_or_else(|err| panic!("{fixture_name} guild tail parses: {err}"));
            let re_encoded = tail.to_bytes();
            assert_eq!(
                &re_encoded,
                original_bytes,
                "{fixture_name}: byte-identical rewrite of a {}-byte guild tail",
                original_bytes.len()
            );
            guild_count += 1;
        }
    }
    assert!(
        guild_count > 0,
        "fixture saves (world1, world2) must contain at least one guild"
    );
}

/// Supplementary to the fixture-based test above: when `PSP_TEST_SAVE_DIR`
/// points at a larger corpus save (not set in this environment), also
/// exercise the codec against it, matching the gating convention every
/// other corpus test in this workspace uses (`world_index.rs`).
#[test]
fn every_guild_tail_in_corpus_session_round_trips_byte_identically() {
    let Some(session) = common::load_corpus_session() else {
        return;
    };
    let groups = world::group_map(&session.level).unwrap();
    for entry in groups {
        if guild_tail::entry_group_type(entry).as_deref() != Some("EPalGroupType::Guild") {
            continue;
        }
        let group_data = guild_tail::entry_group_data(entry).expect("typed PalGroupData");
        let original_bytes = &group_data.remaining_data;
        let tail = guild_tail::GuildTail::parse(original_bytes)
            .unwrap_or_else(|err| panic!("corpus guild tail parses: {err}"));
        assert_eq!(
            tail.to_bytes(),
            *original_bytes,
            "byte-identical rewrite of a {}-byte guild tail",
            original_bytes.len()
        );
    }
}

#[test]
fn synthetic_tail_round_trip_with_unicode_name() {
    let tail = guild_tail::GuildTail {
        org_type: 0,
        leading_bytes: [0; 4],
        base_ids: vec![uuid::Uuid::new_v4()],
        unknown_1: 0,
        base_camp_level: 5,
        map_object_instance_ids_base_camp_points: vec![],
        guild_name: "ギルド".to_string(),
        last_guild_name_modifier_player_uid: uuid::Uuid::new_v4(),
        unknown_2: [0; 4],
        admin_player_uid: uuid::Uuid::new_v4(),
        players: vec![guild_tail::GuildPlayerInfo {
            player_uid: uuid::Uuid::new_v4(),
            last_online_real_time: 638_000_000_000_000_000,
            player_name: "プレイヤー".to_string(),
        }],
        trailing_bytes: [0; 4],
    };
    let encoded = tail.to_bytes();
    let reparsed = guild_tail::GuildTail::parse(&encoded).unwrap();
    assert_eq!(reparsed.guild_name, "ギルド");
    assert_eq!(reparsed.players[0].player_name, "プレイヤー");
    assert_eq!(reparsed.base_camp_level, 5);
    assert_eq!(reparsed.base_ids, tail.base_ids);
    assert_eq!(
        reparsed.admin_player_uid, tail.admin_player_uid,
        "guid round-trips through the write/read permutation exactly"
    );
    // encode(decode(encode(x))) must also equal encode(x): a second pass
    // through the codec must not drift.
    assert_eq!(
        guild_tail::GuildTail::parse(&encoded).unwrap().to_bytes(),
        encoded
    );
}

#[test]
fn parse_rejects_truncated_blob_without_panicking() {
    let truncated = [0u8; 3]; // shorter than even org_type + leading_bytes
    let result = guild_tail::GuildTail::parse(&truncated);
    assert!(
        result.is_err(),
        "truncated blob must be a clean Err, not a panic"
    );
}

#[test]
fn parse_rejects_trailing_garbage_without_panicking() {
    let tail = guild_tail::GuildTail {
        org_type: 0,
        leading_bytes: [0; 4],
        base_ids: vec![],
        unknown_1: 0,
        base_camp_level: 1,
        map_object_instance_ids_base_camp_points: vec![],
        guild_name: "G".to_string(),
        last_guild_name_modifier_player_uid: uuid::Uuid::nil(),
        unknown_2: [0; 4],
        admin_player_uid: uuid::Uuid::nil(),
        players: vec![],
        trailing_bytes: [0; 4],
    };
    let mut bytes = tail.to_bytes();
    bytes.push(0xFF); // one extra byte beyond a structurally complete tail
    let result = guild_tail::GuildTail::parse(&bytes);
    assert!(
        result.is_err(),
        "unconsumed trailing bytes must be a clean Err, not silently ignored"
    );
}
