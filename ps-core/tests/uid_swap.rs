mod common;

use ps_core::domain::world;
use ps_core::props;
use ps_core::ue::games::palworld::PalStruct;
use ps_core::ue::{Property, PropertyKey, StructValue};
use ps_core::progress::null_progress;
use ps_core::session::SaveSession;
use ps_core::transfer::TransferError;
use uuid::Uuid;

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

/// The swap overwrites existing property values in place, so it can never
/// introduce a `MissingPropertySchema`.
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

/// Counts, per owner uid, the `CharacterSaveParameterMap` pals that name it in
/// `OwnerPlayerUId` -- the field the game reads to decide whose pal it is.
fn pal_owner_count(session: &SaveSession, owner: Uuid) -> usize {
    session
        .character_map()
        .unwrap()
        .iter()
        .filter(|entry| !world::entry_is_player(entry))
        .filter_map(world::entry_save_parameter)
        .filter(|parameters| {
            props::get(parameters, &["OwnerPlayerUId"]).and_then(props::as_uuid) == Some(owner)
        })
        .count()
}

/// `MapObjectSaveData` structures whose `Model.RawData.build_player_uid` names `builder`.
fn structure_count(session: &SaveSession, builder: Uuid) -> usize {
    let Ok(Some(values)) = world::map_object_values(&session.level) else {
        return 0;
    };
    values
        .iter()
        .filter(|value| {
            let StructValue::Struct(object_props) = value else {
                return false;
            };
            let Some(Property::Struct(StructValue::Game(PalStruct::MapModel(model)))) = object_props
                .0
                .get(&PropertyKey::from("Model"))
                .and_then(props::struct_props)
                .and_then(|model_props| model_props.0.get(&PropertyKey::from("RawData")))
            else {
                return false;
            };
            props::guid_to_uuid(&model.build_player_uid) == builder
        })
        .count()
}

/// Every occupied `CharacterContainerSaveData` slot's `(player_uid, instance_id)`.
fn container_slot_identities(session: &SaveSession) -> Vec<(Uuid, Uuid)> {
    let Ok(entries) = world::character_container_map(&session.level) else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| props::struct_props(&entry.value))
        .filter_map(|value_props| {
            props::get(value_props, &["Slots"]).and_then(props::struct_values)
        })
        .flatten()
        .filter_map(|slot| {
            let StructValue::Struct(slot_props) = slot else {
                return None;
            };
            match slot_props.0.get(&PropertyKey::from("RawData")) {
                Some(Property::Struct(StructValue::Game(PalStruct::CharacterContainer(raw)))) => Some((
                    props::guid_to_uuid(&raw.player_uid),
                    props::guid_to_uuid(&raw.instance_id),
                )),
                _ => None,
            }
        })
        .filter(|(_, instance_id)| *instance_id != Uuid::nil())
        .collect()
}

/// Slots whose identity names no `CharacterSaveParameterMap` key -- pals the game drops
/// from their container, and then deletes, when the world loads.
fn unresolved_container_slots(session: &SaveSession) -> usize {
    let keys: std::collections::HashSet<(Uuid, Uuid)> = session
        .character_map()
        .unwrap()
        .iter()
        .filter_map(|entry| {
            let key = props::struct_props(&entry.key)?;
            Some((
                props::get(key, &["PlayerUId"]).and_then(props::as_uuid)?,
                props::get(key, &["InstanceId"]).and_then(props::as_uuid)?,
            ))
        })
        .collect();
    container_slot_identities(session)
        .iter()
        .filter(|identity| !keys.contains(identity))
        .count()
}

fn container_slots_naming(session: &SaveSession, uid: Uuid) -> usize {
    container_slot_identities(session)
        .iter()
        .filter(|(player_uid, _)| *player_uid == uid)
        .count()
}

/// The two corpus players holding the most and the fewest pals, so every "did it move"
/// assertion below has a non-zero difference to detect.
fn richest_and_poorest(session: &SaveSession) -> (Uuid, Uuid) {
    let mut ranked: Vec<(Uuid, i64)> = session
        .player_summaries
        .iter()
        .map(|(uid, summary)| (*uid, summary.pal_count))
        .collect();
    ranked.sort_by_key(|(uid, count)| (std::cmp::Reverse(*count), *uid));
    (ranked[0].0, ranked.last().unwrap().0)
}

#[test]
fn corpus_swap_moves_pal_ownership() {
    let mut session = common::load_corpus_session();
    let (rich, poor) = richest_and_poorest(&session);
    let (rich_pals, poor_pals) = (pal_owner_count(&session, rich), pal_owner_count(&session, poor));
    assert!(rich_pals > 0, "the corpus player must own pals to begin with");

    session
        .swap_player_uids(rich, poor, &null_progress())
        .expect("swap succeeds");

    assert_eq!(pal_owner_count(&session, poor), rich_pals);
    assert_eq!(pal_owner_count(&session, rich), poor_pals);
}

/// `PlayerSummary::pal_count` is derived from `OwnerPlayerUId`, so it is what the UI shows
/// straight after a swap -- the number the issue reporter saw stay put.
#[test]
fn corpus_swap_moves_the_reported_pal_count() {
    let mut session = common::load_corpus_session();
    let (rich, poor) = richest_and_poorest(&session);
    let rich_count = session.player_summaries[&rich].pal_count;
    let poor_count = session.player_summaries[&poor].pal_count;

    session
        .swap_player_uids(rich, poor, &null_progress())
        .expect("swap succeeds");

    assert_eq!(session.player_summaries[&poor].pal_count, rich_count);
    assert_eq!(session.player_summaries[&rich].pal_count, poor_count);
}

#[test]
fn corpus_swap_moves_structure_ownership() {
    let mut session = common::load_corpus_session();
    let (rich, poor) = richest_and_poorest(&session);
    let (rich_built, poor_built) = (structure_count(&session, rich), structure_count(&session, poor));
    assert!(rich_built > 0, "the corpus player must have built structures");

    session
        .swap_player_uids(rich, poor, &null_progress())
        .expect("swap succeeds");

    assert_eq!(structure_count(&session, poor), rich_built);
    assert_eq!(structure_count(&session, rich), poor_built);
}

/// A slot's `player_uid` is half of the pal's instance id, not its owner. `v1_stats` is a
/// co-op save, where 168 slots carry the host uid to match their pals' map keys; a host
/// swap that rewrites them orphans every pal in every container.
#[test]
fn v1_stats_host_swap_keeps_container_slots_resolving_to_their_pals() {
    let mut session = common::load_fixture_session("v1_stats");
    let host: Uuid = "00000000-0000-0000-0000-000000000001".parse().unwrap();
    let guest: Uuid = "5de00645-0000-0000-0000-000000000000".parse().unwrap();
    assert_eq!(unresolved_container_slots(&session), 0);
    assert_eq!(container_slots_naming(&session, host), 168);

    session
        .swap_player_uids(host, guest, &null_progress())
        .expect("swap succeeds");

    assert_eq!(unresolved_container_slots(&session), 0);
    assert_eq!(container_slots_naming(&session, host), 168);
    assert_eq!(container_slots_naming(&session, guest), 0);
}

/// The swap is bidirectional, so applying it twice must restore the save byte for byte.
/// Catches a one-directional rewrite that would otherwise collapse both players onto one uid.
#[test]
fn corpus_swapping_twice_restores_the_original_save() {
    let mut session = common::load_corpus_session();
    let (rich, poor) = richest_and_poorest(&session);
    let before = session.level_sav_bytes().expect("corpus Level.sav serializes");

    session.swap_player_uids(rich, poor, &null_progress()).expect("first swap");
    let once = session.level_sav_bytes().expect("Level.sav serializes after one swap");
    assert_ne!(before, once, "one swap must actually change the save");

    session.swap_player_uids(rich, poor, &null_progress()).expect("second swap");
    let twice = session.level_sav_bytes().expect("Level.sav serializes after two swaps");
    assert_eq!(before, twice, "swapping the same pair twice must be a no-op");
}

/// The pals a player keeps in dimensional storage live in a `_dps.sav` beside their
/// `.sav`, counted by neither `pal_count` nor anything in `Level.sav`. Only `v1_relics`
/// ships a fixture player who has one.
fn dps_pals_owned_by(session: &SaveSession, uid: Uuid) -> usize {
    let Some(dps) = session
        .loaded_players
        .get(&uid)
        .and_then(|loaded| loaded.dps.as_ref())
    else {
        return 0;
    };
    let Some(slots) = dps
        .root
        .properties
        .0
        .get(&PropertyKey::from("SaveParameterArray"))
        .and_then(props::struct_values)
    else {
        return 0;
    };
    slots
        .iter()
        .filter_map(|slot| match slot {
            StructValue::Struct(slot_props) => slot_props
                .0
                .get(&PropertyKey::from("SaveParameter"))
                .and_then(props::struct_props),
            _ => None,
        })
        .filter(|save_parameter| {
            props::get(save_parameter, &["OwnerPlayerUId"]).and_then(props::as_uuid) == Some(uid)
        })
        .count()
}

#[test]
fn corpus_swap_moves_dimensional_storage_pals() {
    let mut session = common::load_corpus_session();
    let with_dps: Uuid = "b38a3ab1-0000-0000-0000-000000000000".parse().unwrap();
    let (rich, poor) = richest_and_poorest(&session);
    let other = if rich == with_dps { poor } else { rich };

    let data = common::game_data();
    for uid in [with_dps, other] {
        ps_core::domain::player::get_player_details(&mut session, &data, uid, &null_progress())
            .expect("player loads");
    }
    let stored = dps_pals_owned_by(&session, with_dps);
    assert!(stored > 0, "the fixture player must have pals in dimensional storage");
    assert_eq!(dps_pals_owned_by(&session, other), 0);

    session
        .swap_player_uids(with_dps, other, &null_progress())
        .expect("swap succeeds");

    assert_eq!(dps_pals_owned_by(&session, other), stored);
    assert_eq!(dps_pals_owned_by(&session, with_dps), 0);
}
