mod common;

use psp_core::domain::world;
use psp_core::ue::{Property, StructValue};

/// Real-save validation of `PalDynamicItem.id.local_id_in_created_world`, the
/// field path `build_dynamic_item_index` keys by. world1 carries ~43 real
/// `DynamicItemSaveData` entries, so this exercises parsed `PalDynamicItem`
/// structs rather than an empty loop.
#[test]
fn dynamic_item_index_resolves_every_real_entry_by_local_id_in_created_world() {
    let session = common::load_fixture_session("world1");

    let entries = world::dynamic_item_values(&session.level).unwrap();
    assert!(
        !entries.is_empty(),
        "world1 is expected to carry DynamicItemSaveData entries; if this \
         ever goes empty (e.g. the fixture is replaced), this test no \
         longer proves anything about the real field path and must be \
         re-pointed at a save that does have dynamic items"
    );

    let index = world::build_dynamic_item_index(&session.level);
    assert_eq!(
        entries.len(),
        index.len(),
        "every DynamicItemSaveData entry's RawData must parse as \
         PalDynamicItem and yield a resolvable local_id_in_created_world -- \
         a mismatch here means the field path is wrong for real save data"
    );

    // The struct at each indexed position must itself report the uuid the
    // index filed it under -- proving the key extraction reads the field it
    // claims to, not just that the counts happen to line up.
    for (&local_id, &position) in &index {
        let StructValue::Struct(item_props) = &entries[position] else {
            panic!("indexed position {position} is not a StructValue::Struct");
        };
        let Some(Property::Struct(StructValue::Game(psp_core::ue::PalStruct::DynamicItem(dynamic_item)))) =
            psp_core::props::get(item_props, &["RawData"])
        else {
            panic!("indexed position {position} has no PalDynamicItem RawData");
        };
        assert_eq!(
            local_id,
            psp_core::props::guid_to_uuid(&dynamic_item.id.local_id_in_created_world),
            "position {position}'s real local_id_in_created_world must match \
             the uuid it was indexed under"
        );
    }
}

#[test]
fn character_index_finds_every_pal_and_player() {
    let session = common::load_corpus_session();
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
    // A complete fixture (every player-flagged map entry has a committed
    // `.sav`), so this can assert exact equality -- catching both under- and
    // over-flagging. The rich `v1_relics` corpus is trimmed (ghost player
    // entries with no `.sav`), where flagged > loaded, so it can't be used here.
    let session = common::load_fixture_session("world1");
    let entries = world::character_map(&session.level).unwrap();
    let player_count = entries.iter().filter(|e| world::entry_is_player(e)).count();
    assert_eq!(player_count, session.player_summaries.len());
}

#[test]
fn invalidation_clears_all_caches() {
    let mut session = common::load_corpus_session();
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

/// Why `WorldCaches` must be invalidated on every character-map mutation: a
/// stale index resolves to a different, still-existing entry, silently
/// editing the wrong pal. Removes position 0 specifically -- with >= 2
/// entries it is provably not the last position, so something always shifts
/// into it and the demonstration can never degrade into "the slot is merely
/// gone".
#[test]
fn stale_character_index_after_removal_would_resolve_the_wrong_entry() {
    let mut session = common::load_corpus_session();
    let entries_before_removal = world::character_map(&session.level).unwrap();
    // A single-entry corpus leaves nothing behind at position 0 to be the
    // "different existing entry" this test is about.
    if entries_before_removal.len() < 2 {
        return;
    }
    let removed_id = world::entry_instance_id(&entries_before_removal[0]);

    let entries = world::character_map_mut(&mut session.level).unwrap();
    entries.remove(0);

    // The stale index still claims `removed_id` lives at position 0, but every
    // entry has shifted left by one, so position 0 now holds a different entry
    // that still exists.
    let entries_after_removal = world::character_map(&session.level).unwrap();
    let resolved_after_removal = entries_after_removal
        .first()
        .and_then(world::entry_instance_id);
    assert!(
        resolved_after_removal.is_some(),
        "position 0 must resolve to a DIFFERENT, EXISTING entry after the \
         removal, not merely go missing -- otherwise this test only proves \
         \"the slot is gone\", not \"the stale index now silently edits the \
         wrong pal\""
    );
    assert_ne!(
        resolved_after_removal, removed_id,
        "position 0 must no longer resolve to the removed entry's InstanceId"
    );

    // A freshly rebuilt index no longer contains the removed id at all.
    if let Some(removed_id) = removed_id {
        let fresh_index = world::build_character_index(&session.level);
        assert!(!fresh_index.contains_key(&removed_id));
    }
}
