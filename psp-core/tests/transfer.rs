mod common;

use psp_core::progress::null_progress;
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

    // Spawning into the same uid in an identical world must succeed and leave
    // the player's summary in place after the cache rebuild.
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

/// True spawn mode (`target_player_uid = None` with a valid source uid): the
/// only path that clones the source GVAS and inserts it into the target's
/// `loaded_players`/`player_file_refs`.
#[test]
fn world1_true_spawn_mode_inserts_cloned_player() {
    let mut source = common::load_fixture_session("world1");
    let mut target = common::load_fixture_session("world1");

    let Some(source_uid) = level_two_player(&source) else {
        eprintln!("world1 fixture has no level>=2 player; skipping spawn assertion");
        return;
    };

    // A freshly loaded target has parsed no player GVAS yet, so the spawn
    // branch is the only thing that can populate `loaded_players` for this uid.
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

/// The same assertions against the committed `v1_relics` corpus fixture.
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
