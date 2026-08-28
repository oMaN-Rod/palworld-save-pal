mod common;

use psp_core::domain::world;
use psp_core::props;
use psp_core::ue::games::palworld::PalStruct;
use psp_core::ue::{Property, PropertyKey, StructValue};
use psp_core::progress::null_progress;
use psp_core::session::SaveSession;
use psp_core::transfer::TransferError;
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

/// `CharacterContainerSaveData` slots whose `RawData.player_uid` names `owner`. A pal box
/// travels with the `.sav` that references it, so its slot owners must travel too.
fn container_slot_owner_count(session: &SaveSession, owner: Uuid) -> usize {
    let Ok(entries) = world::character_container_map(&session.level) else {
        return 0;
    };
    entries
        .iter()
        .filter_map(|entry| props::struct_props(&entry.value))
        .filter_map(|value_props| {
            props::get(value_props, &["Slots"]).and_then(props::struct_values)
        })
        .flatten()
        .filter(|slot| {
            let StructValue::Struct(slot_props) = slot else {
                return false;
            };
            matches!(
                slot_props.0.get(&PropertyKey::from("RawData")),
                Some(Property::Struct(StructValue::Game(PalStruct::CharacterContainer(raw))))
                    if props::guid_to_uuid(&raw.player_uid) == owner
            )
        })
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

/// Every `v1_relics` container slot carries the nil owner, so only `v1_stats` can tell a
/// working rewrite from a missing one here.
#[test]
fn v1_stats_swap_moves_character_container_slot_owners() {
    let mut session = common::load_fixture_session("v1_stats");
    let (rich, poor) = richest_and_poorest(&session);
    let rich_slots = container_slot_owner_count(&session, rich);
    let poor_slots = container_slot_owner_count(&session, poor);
    assert!(
        rich_slots > 0 || poor_slots > 0,
        "the fixture must name a player on some container slot"
    );

    session
        .swap_player_uids(rich, poor, &null_progress())
        .expect("swap succeeds");

    assert_eq!(container_slot_owner_count(&session, poor), rich_slots);
    assert_eq!(container_slot_owner_count(&session, rich), poor_slots);
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
        psp_core::domain::player::get_player_details(&mut session, &data, uid, &null_progress())
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
