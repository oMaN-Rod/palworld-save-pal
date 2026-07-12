//! `swap_player_uids` integration coverage -- port of `game/mixins/
//! player_swap.py`'s `PlayerSwapMixin.swap_player_uids`.
//!
//! Two layers, mirroring `tests/transfer.rs`'s own split:
//! * `world1` fixture (checked in, always runs): same-uid rejection,
//!   unknown-player rejection, and -- since world1 carries two real players
//!   -- a full forward swap that must succeed and leave both uids' character-
//!   map identities exchanged.
//! * `PSP_TEST_LEVEL_SAV` corpus (env-gated, clean-skips when unset): the
//!   same same-uid rejection against an external corpus, matching the
//!   brief's Step-1 test.

mod common;

use psp_core::domain::world;
use psp_core::progress::null_progress;
use psp_core::props;
use psp_core::session::SaveSession;
use psp_core::transfer::TransferError;
use uuid::Uuid;

const OWNERSHIP_KEYS: [&str; 4] = [
    "OwnerPlayerUId",
    "owner_player_uid",
    "build_player_uid",
    "private_lock_player_uid",
];

/// The `InstanceId` of the `CharacterSaveParameterMap` entry belonging to
/// `player_uid`'s own player character (`IsPlayer == true`).
fn player_instance_id(session: &SaveSession, player_uid: Uuid) -> Uuid {
    session
        .character_map()
        .unwrap()
        .iter()
        .find(|entry| {
            world::entry_is_player(entry) && world::entry_player_uid(entry) == Some(player_uid)
        })
        .and_then(world::entry_instance_id)
        .expect("player has a CharacterSaveParameterMap entry")
}

#[test]
fn world1_swapping_same_uid_is_rejected() {
    let mut session = common::load_fixture_session("world1");
    let uid = *session
        .player_summaries
        .keys()
        .next()
        .expect("world1 fixture has at least one player");

    let result = session.swap_player_uids(uid, uid, &null_progress());

    match result {
        Err(TransferError::Rejected(message)) => {
            assert_eq!(message, "Both players are the same.");
        }
        other => panic!("expected rejection, got {other:?}"),
    }
}

#[test]
fn world1_swapping_an_unknown_player_is_rejected() {
    let mut session = common::load_fixture_session("world1");
    let known_uid = *session
        .player_summaries
        .keys()
        .next()
        .expect("world1 fixture has at least one player");
    let unknown_uid = Uuid::new_v4();

    let result = session.swap_player_uids(known_uid, unknown_uid, &null_progress());

    match result {
        Err(TransferError::Rejected(message)) => {
            assert_eq!(message, format!("Player {unknown_uid} not found."));
        }
        other => panic!("expected rejection, got {other:?}"),
    }
}

/// End-to-end: swap between world1's two real players must succeed, leave
/// both summaries present after the cache rebuild, exchange the
/// `CharacterSaveParameterMap` key `PlayerUId` at each player's own
/// (unchanged) `InstanceId`, and re-serialize `Level.sav` afterward without
/// a `MissingPropertySchema` failure (the deep swap only overwrites
/// EXISTING property values in place, so this must never fail on schema
/// grounds).
#[test]
fn world1_swap_between_two_players_exchanges_character_map_identities() {
    let mut session = common::load_fixture_session("world1");
    let uids: Vec<Uuid> = session.player_summaries.keys().copied().collect();
    assert!(
        uids.len() >= 2,
        "world1 fixture must have at least two players for this test"
    );
    let (first_uid, second_uid) = (uids[0], uids[1]);
    let first_instance_id = player_instance_id(&session, first_uid);
    let second_instance_id = player_instance_id(&session, second_uid);

    session
        .swap_player_uids(first_uid, second_uid, &null_progress())
        .expect("swap between two real world1 players succeeds");

    assert!(session.player_summaries.contains_key(&first_uid));
    assert!(session.player_summaries.contains_key(&second_uid));

    // Each character keeps its OWN instance id, but the KEY's PlayerUId at
    // that instance id now names the OTHER player.
    let entry_at_first_instance = session
        .character_map()
        .unwrap()
        .iter()
        .find(|entry| world::entry_instance_id(entry) == Some(first_instance_id))
        .expect("the first player's character entry still exists");
    assert_eq!(
        world::entry_player_uid(entry_at_first_instance),
        Some(second_uid)
    );

    let entry_at_second_instance = session
        .character_map()
        .unwrap()
        .iter()
        .find(|entry| world::entry_instance_id(entry) == Some(second_instance_id))
        .expect("the second player's character entry still exists");
    assert_eq!(
        world::entry_player_uid(entry_at_second_instance),
        Some(first_uid)
    );

    session
        .level_sav_bytes()
        .expect("post-swap Level.sav re-serializes without a schema error");
}

/// Locks in the deep-swap step's real-save behavior: running
/// `props::swap_uuid_values_deep` over world1's actual `Level.sav` root
/// properties for two real player uids changes NOTHING on the wire. This is
/// the parity-correct outcome -- Python's `_deep_swap_uids` is likewise a
/// no-op on real saves (all four ownership keys are `UUID` objects / typed
/// Guid structs, never the `str` its guard requires; the reachable ones are
/// behind typed codec structs this walk stops at). The test guards against a
/// future regression where `swap_leaf_uuid_property` starts over-swapping a
/// reachable `Str`/`Guid` leaf and diverges from Python. Asserted on the
/// serialized `Level.sav` bytes (a total, structural equality check), not on
/// a hand-picked subset of properties.
#[test]
fn deep_swap_over_real_level_sav_properties_changes_nothing() {
    let mut session = common::load_fixture_session("world1");
    let uids: Vec<Uuid> = session.player_summaries.keys().copied().collect();
    assert!(
        uids.len() >= 2,
        "world1 fixture must have at least two players for this test"
    );
    let (first_uid, second_uid) = (uids[0], uids[1]);

    let before = session
        .level_sav_bytes()
        .expect("world1 Level.sav serializes before the deep swap");

    props::swap_uuid_values_deep(
        session.level_properties_mut(),
        &OWNERSHIP_KEYS,
        first_uid,
        second_uid,
    );

    let after = session
        .level_sav_bytes()
        .expect("world1 Level.sav serializes after the deep swap");

    assert_eq!(
        before, after,
        "the deep ownership-key swap must be a no-op on real save data \
         (parity with Python's real-save-inert _deep_swap_uids)"
    );
}

/// Brief Step-1 test, adapted to the real API (`common::load_corpus_session`
/// instead of the fictional `session::load_steam_save`, `player_summaries`
/// field instead of a method). Runs only with `PSP_TEST_LEVEL_SAV` set.
#[test]
fn corpus_swapping_same_uid_is_rejected() {
    let level_path = match std::env::var("PSP_TEST_LEVEL_SAV") {
        Ok(path) => path,
        Err(_) => {
            eprintln!("skipping: PSP_TEST_LEVEL_SAV not set");
            return;
        }
    };
    // `PSP_TEST_LEVEL_SAV` names a Level.sav; `load_corpus_session` reads
    // `PSP_TEST_SAVE_DIR`. Point the corpus loader at the Level.sav's
    // directory when only PSP_TEST_LEVEL_SAV is provided.
    if std::env::var("PSP_TEST_SAVE_DIR").is_err() {
        if let Some(parent) = std::path::Path::new(&level_path).parent() {
            std::env::set_var("PSP_TEST_SAVE_DIR", parent);
        }
    }
    let Some(mut session) = common::load_corpus_session() else {
        return;
    };
    let uid = *session
        .player_summaries
        .keys()
        .next()
        .expect("corpus save has at least one player");

    let result = session.swap_player_uids(uid, uid, &null_progress());

    match result {
        Err(TransferError::Rejected(message)) => {
            assert_eq!(message, "Both players are the same.");
        }
        other => panic!("expected rejection, got {other:?}"),
    }
}
