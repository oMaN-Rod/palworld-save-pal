mod common;

use psp_core::domain::world;

#[test]
fn character_index_finds_every_pal_and_player() {
    let Some(session) = common::load_corpus_session() else {
        return;
    };
    let index = world::build_character_index(&session.level);
    let entries = world::character_map(&session.level).unwrap();
    assert_eq!(
        index.len(),
        entries.len(),
        "every entry must be indexed by InstanceId"
    );
    for (instance_id, entry_index) in &index {
        let entry = &entries[*entry_index];
        assert_eq!(world::entry_instance_id(entry), Some(*instance_id));
    }
}

#[test]
fn player_entries_are_flagged() {
    let Some(session) = common::load_corpus_session() else {
        return;
    };
    let entries = world::character_map(&session.level).unwrap();
    let player_count = entries.iter().filter(|e| world::entry_is_player(e)).count();
    assert_eq!(player_count, session.player_summaries.len());
}

#[test]
fn invalidation_clears_all_caches() {
    let Some(mut session) = common::load_corpus_session() else {
        return;
    };
    session.caches.character_index = Some(world::build_character_index(&session.level));
    session.caches.item_container_index = Some(world::build_item_container_index(&session.level));
    session.invalidate_performance_caches();
    assert!(session.caches.character_index.is_none());
    assert!(session.caches.item_container_index.is_none());
    assert!(session.caches.character_container_index.is_none());
    assert!(session.caches.dynamic_item_index.is_none());
    assert!(session.caches.pal_owner_counts.is_none());
    assert!(session.caches.player_guild_map.is_none());
}

/// The mandated cache-invalidation-after-mutation proof: builds the
/// character index, mutates the underlying map (removes an entry) WITHOUT
/// invalidating, and shows the stale index would now resolve to a mismatched
/// or out-of-bounds entry -- demonstrating exactly why `WorldCaches` must be
/// invalidated on every character-map mutation (Tasks 9/11's contract).
/// Then re-builds after the mutation and shows the fresh index is correct.
#[test]
fn stale_character_index_after_removal_would_resolve_the_wrong_entry() {
    let Some(mut session) = common::load_corpus_session() else {
        return;
    };
    let stale_index = world::build_character_index(&session.level);
    let Some((&removed_id, &removed_position)) = stale_index.iter().next() else {
        return;
    };
    let entries = world::character_map_mut(&mut session.level).unwrap();
    if removed_position >= entries.len() {
        return;
    }
    entries.remove(removed_position);

    // The stale index still claims `removed_id` lives at `removed_position`,
    // but every entry from that position onward has shifted left by one --
    // so either the slot is gone, or it now holds a different InstanceId.
    let entries_after_removal = world::character_map(&session.level).unwrap();
    let still_matches = entries_after_removal
        .get(removed_position)
        .and_then(world::entry_instance_id)
        == Some(removed_id);
    assert!(
        !still_matches,
        "removed entry's old position must no longer resolve to its InstanceId"
    );

    // A freshly rebuilt index (what every Task-9/11 mutation must trigger
    // via invalidate_performance_caches, then a rebuild-on-next-access) no
    // longer contains the removed id at all.
    let fresh_index = world::build_character_index(&session.level);
    assert!(!fresh_index.contains_key(&removed_id));
}
