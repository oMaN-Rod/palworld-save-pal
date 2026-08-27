mod common;

use psp_core::domain::{pal, player, world};
use psp_core::progress::null_progress;
use psp_core::props;
use psp_core::session::SaveSession;
use psp_core::transfer::{transfer_player, TransferError, TransferOptions};
use uuid::Uuid;

fn all_options() -> TransferOptions {
    TransferOptions {
        transfer_character: true,
        transfer_inventory: true,
        transfer_pals: true,
        transfer_tech: true,
        transfer_appearance: true,
    }
}

/// The first player whose level is >= 2 -- the minimum `transfer_player`
/// accepts.
fn level_two_player(session: &SaveSession) -> Option<Uuid> {
    session
        .player_summaries
        .iter()
        .find(|(_, summary)| summary.level.unwrap_or(1) >= 2)
        .map(|(uid, _)| *uid)
}

fn assert_unknown_source_rejected(source: &mut SaveSession, target: &mut SaveSession) {
    let unknown_uid = Uuid::new_v4();
    let rejected = transfer_player(
        source,
        target,
        unknown_uid,
        None,
        &all_options(),
        &null_progress(),
    );
    match rejected {
        Err(TransferError::Rejected(message)) => {
            assert_eq!(message, format!("Source player {unknown_uid} not found."));
        }
        other => panic!("expected soft rejection, got {other:?}"),
    }
}

#[test]
fn world1_unknown_source_is_soft_rejected() {
    let mut source = common::load_fixture_session("world1");
    let mut target = common::load_fixture_session("world1");
    assert_unknown_source_rejected(&mut source, &mut target);
}

#[test]
fn world1_spawn_mode_transfer_keeps_player_present() {
    let mut source = common::load_fixture_session("world1");
    let mut target = common::load_fixture_session("world1");

    let Some(source_uid) = level_two_player(&source) else {
        eprintln!("world1 fixture has no level>=2 player; skipping spawn assertion");
        return;
    };

    transfer_player(
        &mut source,
        &mut target,
        source_uid,
        Some(source_uid),
        &all_options(),
        &null_progress(),
    )
    .expect("spawn-mode transfer succeeds");
    assert!(target.player_summaries.contains_key(&source_uid));
}

/// True spawn mode (`target_player_uid = None`): the only path that clones the source
/// GVAS into the target's `loaded_players`/`player_file_refs`.
#[test]
fn world1_true_spawn_mode_inserts_cloned_player() {
    let mut source = common::load_fixture_session("world1");
    let mut target = common::load_fixture_session("world1");

    let Some(source_uid) = level_two_player(&source) else {
        eprintln!("world1 fixture has no level>=2 player; skipping spawn assertion");
        return;
    };

    assert!(
        !target.loaded_players.contains_key(&source_uid),
        "precondition: target has not loaded this player before the spawn"
    );

    transfer_player(
        &mut source,
        &mut target,
        source_uid,
        None, // <-- true spawn mode (target_player_uid == None)
        &all_options(),
        &null_progress(),
    )
    .expect("true spawn-mode transfer succeeds");

    assert!(
        target.loaded_players.contains_key(&source_uid),
        "spawn branch must insert the cloned player GVAS into the target"
    );
    assert!(
        target.player_file_refs.contains_key(&source_uid),
        "spawn branch must insert the player's file reference into the target"
    );
    assert!(
        target.player_summaries.contains_key(&source_uid),
        "spawned player has a summary after the cache rebuild"
    );
}

#[test]
fn corpus_spawn_mode_transfer_copies_player_into_target() {
    let mut source = common::load_corpus_session();
    let mut target = common::load_corpus_session();

    assert_unknown_source_rejected(&mut source, &mut target);

    let source_uid = *source
        .player_summaries
        .keys()
        .next()
        .expect("corpus save has at least one player");
    transfer_player(
        &mut source,
        &mut target,
        source_uid,
        Some(source_uid),
        &all_options(),
        &null_progress(),
    )
    .expect("spawn-mode transfer succeeds");
    assert!(target.player_summaries.contains_key(&source_uid));
}

/// A pal created by this app spells its slot key `SlotID` (`new_pal_entry` writes it
/// unconditionally). A real transfer between two distinct players must repoint that
/// pal's `SlotID.ContainerId.ID` from the source player's pal box to the target's --
/// not merely leave the transfer erroring or silently no-op.
#[test]
fn corpus_transfer_repoints_slotid_pal_container_to_target() {
    let data = common::game_data();
    let mut source = common::load_corpus_session();
    let mut target = common::load_corpus_session();

    let source_uid = level_two_player(&source).expect("corpus has a level>=2 player");
    let target_uid = *target
        .player_summaries
        .keys()
        .find(|&&uid| uid != source_uid)
        .expect("corpus has a second, distinct player");

    player::get_player_details(&mut source, &data, source_uid, &null_progress())
        .unwrap()
        .expect("source player loads");
    let source_pal_box_id = player::build_player_dto(&source, &data, source_uid)
        .unwrap()
        .unwrap()
        .pal_box_id
        .expect("source pal box exists");

    player::get_player_details(&mut target, &data, target_uid, &null_progress())
        .unwrap()
        .expect("target player loads");
    let target_pal_box_id = player::build_player_dto(&target, &data, target_uid)
        .unwrap()
        .unwrap()
        .pal_box_id
        .expect("target pal box exists");
    assert_ne!(
        source_pal_box_id, target_pal_box_id,
        "precondition: distinct players must have distinct pal boxes for this bug to bite"
    );

    let new_pal = pal::add_player_pal(
        &mut source,
        &data,
        source_uid,
        "Sheepball",
        "slotid-fixture",
        source_pal_box_id,
        None,
    )
    .unwrap()
    .expect("source pal box has room for one more pal");

    // `new_pal_entry` writes the slot key as `SlotID`, not `SlotId`.
    let entry = world::character_map(&source.level)
        .unwrap()
        .iter()
        .find(|entry| world::entry_instance_id(entry) == Some(new_pal.instance_id))
        .expect("just-added pal is present");
    let save_parameter = world::entry_save_parameter(entry).expect("save parameter present");
    assert!(
        props::get(save_parameter, &["SlotID", "ContainerId", "ID"]).is_some(),
        "precondition: the seeded pal's slot key must be spelled \"SlotID\""
    );

    transfer_player(
        &mut source,
        &mut target,
        source_uid,
        Some(target_uid),
        &all_options(),
        &null_progress(),
    )
    .expect("transfer succeeds");

    let transferred = world::character_map(&target.level)
        .unwrap()
        .iter()
        .find(|entry| world::entry_instance_id(entry) == Some(new_pal.instance_id))
        .expect("transferred pal is present in the target save");
    let save_parameter = world::entry_save_parameter(transferred).expect("save parameter present");
    let container_id = props::get(save_parameter, &["SlotID", "ContainerId", "ID"])
        .and_then(props::as_uuid)
        .expect("SlotID.ContainerId.ID present after transfer");

    assert_eq!(
        container_id, target_pal_box_id,
        "transferred pal's SlotID.ContainerId.ID must point at the target's pal box, not the source's"
    );
}
