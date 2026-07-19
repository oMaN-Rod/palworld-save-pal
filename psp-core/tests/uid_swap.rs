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

/// A swap between world1's two real players exchanges the
/// `CharacterSaveParameterMap` key `PlayerUId` at each player's own
/// (unchanged) `InstanceId`, and `Level.sav` still re-serializes afterward:
/// the deep swap only overwrites existing property values in place, so it can
/// never introduce a `MissingPropertySchema`.
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

    // Each character keeps its own instance id, but the key's PlayerUId at
    // that instance id now names the other player.
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

/// `props::swap_uuid_values_deep` over the ownership keys must be inert on
/// real save data: every reachable occurrence of those keys sits behind a
/// typed codec struct the walk stops at, so nothing on the wire may change.
/// Guards against `swap_leaf_uuid_property` starting to over-swap a reachable
/// `Str`/`Guid` leaf. Asserted on the whole serialized `Level.sav`, not a
/// hand-picked subset of properties.
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
        "the deep ownership-key swap must be a no-op on real save data"
    );
}

/// The same same-uid rejection against the committed `v1_relics` corpus fixture.
#[test]
fn corpus_swapping_same_uid_is_rejected() {
    let mut session = common::load_corpus_session();
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
