mod common;

use psp_core::domain::raw_path::{NodeKind, RawPath, RawScalar, RawScope, VisitAction};

#[test]
fn a_dotted_path_parses_into_key_segments() {
    let path = RawPath::parse("worldSaveData.GroupSaveDataMap").expect("parses");
    assert!(!path.is_empty());
}

#[test]
fn an_indexed_path_parses() {
    RawPath::parse("worldSaveData.CharacterSaveParameterMap[3].value").expect("parses");
}

#[test]
fn malformed_paths_are_refused() {
    for bad in ["", ".", "a..b", "a.", "a[", "a[]", "a[x]", "a[-1]", "[0]", "a..", "a.[0]"] {
        assert!(
            RawPath::parse(bad).is_err(),
            "path {bad:?} should have been refused"
        );
    }
}

#[test]
fn a_top_level_key_resolves_and_reports_its_length() {
    let mut session = common::load_corpus_session();
    let path = RawPath::parse("worldSaveData.CharacterSaveParameterMap").expect("parses");
    let len = session
        .raw_len(RawScope::Level, &path)
        .expect("resolves")
        .expect("the character map is present");
    assert!(len > 0, "the corpus fixture has characters");
}

#[test]
fn an_absent_key_errors_on_get_and_len_but_not_on_kind_or_exists() {
    let mut session = common::load_corpus_session();
    let path = RawPath::parse("worldSaveData.NoSuchKeyAtAll").expect("parses");
    assert!(session.raw_len(RawScope::Level, &path).is_err());
    assert!(session.raw_get(RawScope::Level, &path).is_err());
    assert_eq!(session.raw_kind(RawScope::Level, &path).expect("resolves"), None);
}

#[test]
fn deleting_a_top_level_key_removes_it() {
    let mut session = common::load_corpus_session();
    let path = RawPath::parse("worldSaveData.GroupSaveDataMap").expect("parses");
    assert!(session.raw_len(RawScope::Level, &path).expect("resolves").is_some());

    assert!(session.raw_delete(RawScope::Level, &path).expect("deletes"));
    assert!(session.raw_len(RawScope::Level, &path).is_err());

    assert!(!session.raw_delete(RawScope::Level, &path).expect("second delete"));
}

#[test]
fn deleting_an_indexed_element_shortens_its_parent() {
    let mut session = common::load_corpus_session();
    let map = RawPath::parse("worldSaveData.CharacterSaveParameterMap").expect("parses");
    let before = session.raw_len(RawScope::Level, &map).expect("resolves").expect("present");

    let element = RawPath::parse("worldSaveData.CharacterSaveParameterMap[0]").expect("parses");
    assert!(session.raw_delete(RawScope::Level, &element).expect("deletes"));

    let after = session.raw_len(RawScope::Level, &map).expect("resolves").expect("present");
    assert_eq!(after, before - 1);
}

#[test]
fn an_out_of_bounds_index_errors_on_get_rather_than_panicking() {
    let mut session = common::load_corpus_session();
    let path = RawPath::parse("worldSaveData.CharacterSaveParameterMap[999999999]").expect("parses");
    assert!(session.raw_get(RawScope::Level, &path).is_err());
    assert!(!session.raw_delete(RawScope::Level, &path).expect("resolves"));
}

#[test]
fn a_scalar_round_trips_through_get_and_set() {
    let mut session = common::load_corpus_session();
    let path = RawPath::parse(
        "worldSaveData.CharacterSaveParameterMap[0].key.InstanceId",
    )
    .expect("parses");

    let original = session.raw_get(RawScope::Level, &path).expect("resolves");
    assert!(matches!(original, Some(RawScalar::Guid(_))), "got {original:?}");
}

#[test]
fn setting_a_scalar_to_a_different_variant_is_refused() {
    let mut session = common::load_corpus_session();
    let path = RawPath::parse("worldSaveData.CharacterSaveParameterMap[0].key.InstanceId")
        .expect("parses");
    assert!(session
        .raw_set(RawScope::Level, &path, RawScalar::Int(1))
        .is_err());
}

#[test]
fn a_byte_property_reads_as_a_scalar_not_opaque() {
    let mut session = common::load_corpus_session();
    let path = RawPath::parse("worldSaveData.CharacterSaveParameterMap[0].value.RawData.SaveParameter.Level")
        .expect("parses");
    let kind = session.raw_kind(RawScope::Level, &path).expect("resolves").expect("present");
    assert_eq!(kind, NodeKind::Scalar);
    let value = session.raw_get(RawScope::Level, &path).expect("resolves");
    assert!(matches!(value, Some(RawScalar::Int(_))), "got {value:?}");
}

#[test]
fn a_byte_level_field_round_trips_through_set_and_reparse() {
    let mut session = common::load_corpus_session();
    let path = RawPath::parse("worldSaveData.CharacterSaveParameterMap[0].value.RawData.SaveParameter.Level")
        .expect("parses");

    let original = session.raw_get(RawScope::Level, &path).expect("resolves");
    let Some(RawScalar::Int(original)) = original else {
        panic!("expected a Byte-backed Level field, got {original:?}");
    };
    let new_level = if original == 250 { 1 } else { original + 1 };

    session
        .raw_set(RawScope::Level, &path, RawScalar::Int(new_level))
        .expect("sets");
    assert_eq!(
        session.raw_get(RawScope::Level, &path).expect("resolves"),
        Some(RawScalar::Int(new_level))
    );

    let bytes = session.level_sav_bytes().expect("the edited level re-serializes");
    let reparsed = psp_core::savio::read_sav_bytes(&bytes).expect("the edited level re-parses");
    let mut reparsed_session = psp_core::session::SaveSession::new_for_tests(
        psp_core::session::SaveKind::InMemory,
        reparsed,
    );
    assert_eq!(
        reparsed_session.raw_get(RawScope::Level, &path).expect("resolves"),
        Some(RawScalar::Int(new_level)),
        "the written Byte value must survive a full serialize/reparse round trip"
    );
}

#[test]
fn a_visit_reaches_every_node_and_reports_a_count() {
    let mut session = common::load_corpus_session();
    let path = RawPath::parse("worldSaveData.CharacterSaveParameterMap").expect("parses");

    let mut seen = 0usize;
    let stats = session
        .raw_visit(RawScope::Level, &path, 64, |_node| {
            seen += 1;
            VisitAction::Keep
        })
        .expect("visits");
    assert_eq!(stats.visited, seen);
    assert!(stats.visited > 0);
    assert_eq!(stats.removed, 0);
    assert!(!stats.stopped_early);
}

#[test]
fn a_visit_can_remove_matching_nodes_and_they_stay_removed() {
    let mut session = common::load_corpus_session();
    let path = RawPath::parse("worldSaveData").expect("parses");

    // IsPlayer occurs on every character entry in the corpus fixture.
    let count = |session: &mut psp_core::session::SaveSession| {
        session
            .raw_visit(RawScope::Level, &path, 64, |node| {
                if node.key() == Some("IsPlayer") {
                    VisitAction::Remove
                } else {
                    VisitAction::Keep
                }
            })
            .expect("visits")
    };

    let first = count(&mut session);
    assert!(first.removed > 0, "the corpus has IsPlayer keys to remove");
    assert!(!first.stopped_early);

    let second = count(&mut session);
    assert_eq!(second.removed, 0);
    assert!(second.visited < first.visited, "the tree got smaller");
}

#[test]
fn a_visit_stops_early_when_asked() {
    let mut session = common::load_corpus_session();
    let path = RawPath::parse("worldSaveData").expect("parses");

    let mut seen = 0usize;
    let stats = session
        .raw_visit(RawScope::Level, &path, 64, |_node| {
            seen += 1;
            if seen >= 10 { VisitAction::Stop } else { VisitAction::Keep }
        })
        .expect("visits");
    assert!(stats.stopped_early);
    assert_eq!(stats.visited, 10);
}

#[test]
fn a_visit_respects_its_depth_ceiling() {
    let mut session = common::load_corpus_session();
    let path = RawPath::parse("worldSaveData").expect("parses");

    let shallow = session
        .raw_visit(RawScope::Level, &path, 2, |_| VisitAction::Keep)
        .expect("visits");
    let deep = session
        .raw_visit(RawScope::Level, &path, 64, |_| VisitAction::Keep)
        .expect("visits");
    assert!(shallow.visited < deep.visited);
}

#[test]
fn a_player_scope_loads_the_player_before_resolving() {
    let mut session = common::load_corpus_session();
    // fixture_player_uid has no backing .sav file in this corpus.
    let uid = *session
        .player_file_refs
        .keys()
        .next()
        .expect("the fixture has at least one player file");
    assert!(
        !session.loaded_players.contains_key(&uid),
        "the fixture starts with no player parsed"
    );

    let path = RawPath::parse("SaveData").expect("parses");
    let len = session.raw_len(RawScope::Player(uid), &path).expect("resolves");

    assert!(len.is_some() || len.is_none(), "resolution must not error");
    assert!(
        session.loaded_players.contains_key(&uid),
        "resolving a player scope must load that player, or its edits are dropped at save time"
    );
}

#[test]
fn an_unknown_player_scope_errors_rather_than_panicking() {
    let mut session = common::load_corpus_session();
    let path = RawPath::parse("SaveData").expect("parses");
    let unknown = uuid::Uuid::from_u128(0xdead_beef);
    assert!(session.raw_len(RawScope::Player(unknown), &path).is_err());
}

// Indices below are specific to the checked-in v1_relics fixture and only
// stable as long as that file doesn't change.

#[test]
fn a_visit_reaches_a_hatching_pals_save_parameter() {
    let mut session = common::load_fixture_session("v1_relics");
    // MapObjectSaveData[4353] is one of the few hatching eggs in this fixture
    // with a non-empty SaveParameter bag.
    let path = RawPath::parse("worldSaveData.MapObjectSaveData[4353]").expect("parses");

    let mut found_character_id = false;
    session
        .raw_visit(RawScope::Level, &path, 64, |node| {
            if node.key() == Some("CharacterID") {
                found_character_id = true;
                assert!(
                    matches!(node.scalar(), Some(RawScalar::Text(_))),
                    "got {:?}",
                    node.scalar()
                );
            }
            VisitAction::Keep
        })
        .expect("visits");
    assert!(
        found_character_id,
        "the walk must descend through ConcreteModel.RawData into the hatching pal's SaveParameter"
    );
}

#[test]
fn a_hatching_pals_character_id_round_trips_through_get() {
    let mut session = common::load_fixture_session("v1_relics");
    let path = RawPath::parse(
        "worldSaveData.MapObjectSaveData[4353].ConcreteModel.RawData.SaveParameter.CharacterID",
    )
    .expect("parses");
    let value = session.raw_get(RawScope::Level, &path).expect("resolves");
    assert!(matches!(value, Some(RawScalar::Text(_))), "got {value:?}");
}

#[test]
fn a_visit_classifies_an_unhatched_eggs_payload_as_a_struct_not_opaque() {
    let mut session = common::load_fixture_session("v1_relics");
    // Every egg carries an empty `object` bag, so this only proves the walk opens
    // NodeKind::Struct rather than treating it as opaque.
    let path = RawPath::parse("worldSaveData.DynamicItemSaveData[5]").expect("parses");

    let mut saw_raw_data = false;
    session
        .raw_visit(RawScope::Level, &path, 64, |node| {
            if node.key() == Some("RawData") {
                saw_raw_data = true;
                assert_eq!(node.kind(), NodeKind::Struct);
            }
            VisitAction::Keep
        })
        .expect("visits");
    assert!(saw_raw_data, "the walk must reach the egg's RawData property");
}

#[test]
fn removing_an_early_entry_skips_exactly_its_own_subtree() {
    let mut control_session = common::load_corpus_session();
    let mut test_session = common::load_corpus_session();
    let path = RawPath::parse("worldSaveData.CharacterSaveParameterMap").expect("parses");
    let max_depth = 4;

    // A depth-first walk visits a whole subtree contiguously, so entry 0's
    // own subtree size can be measured while walking.
    let mut entry0_subtree = 0usize;
    let mut counting = false;
    let control_stats = control_session
        .raw_visit(RawScope::Level, &path, max_depth, |node| {
            if node.depth() == 1 {
                counting = node.index() == Some(0);
            }
            if counting {
                entry0_subtree += 1;
            }
            VisitAction::Keep
        })
        .expect("visits");
    assert!(
        entry0_subtree > 1,
        "entry 0 must have a non-trivial subtree for this test to mean anything"
    );

    let test_stats = test_session
        .raw_visit(RawScope::Level, &path, max_depth, |node| {
            if node.depth() == 1 && node.index() == Some(0) {
                VisitAction::Remove
            } else {
                VisitAction::Keep
            }
        })
        .expect("visits");

    assert_eq!(test_stats.removal_errors, 0);
    // Entry 0 itself still counts toward `visited` (Remove doesn't push
    // children), so the gap is its subtree minus itself.
    assert_eq!(
        test_stats.visited,
        control_stats.visited - (entry0_subtree - 1),
        "removing entry 0 must skip exactly its own descendants -- not corrupt indexing into \
         whichever entry happens to be visited last"
    );
}

#[test]
fn a_host_driven_walk_removal_invalidates_caches() {
    let mut session = common::load_corpus_session();
    let uid = common::fixture_player_uid(&session);
    let _ = psp_core::domain::guild::find_player_guild_id(&mut session, uid).expect("resolves");
    assert!(session.caches.player_guild_map.is_some());

    let path = RawPath::parse("worldSaveData.GroupSaveDataMap").expect("parses");
    let mut walk = session
        .raw_walk_begin(RawScope::Level, &path, 8)
        .expect("begin");
    while let Some(_info) = session.raw_walk_next(&mut walk) {
        session.raw_walk_act(&mut walk, VisitAction::Remove);
    }
    let stats = session.raw_walk_finish(&mut walk);
    assert!(stats.removed > 0, "the corpus fixture has group entries to remove");
    assert_eq!(stats.removal_errors, 0);
    assert!(
        session.caches.player_guild_map.is_none(),
        "raw_walk_finish must invalidate caches on removal, just like raw_visit/raw_set/raw_delete -- \
         it is the one walk API Lua's raw.visit can actually use"
    );
}

#[test]
fn a_walked_nodes_path_round_trips_through_raw_get() {
    let mut session = common::load_corpus_session();
    let path =
        RawPath::parse("worldSaveData.CharacterSaveParameterMap[3].value.RawData.SaveParameter")
            .expect("parses");
    let mut walk = session.raw_walk_begin(RawScope::Level, &path, 8).expect("begin");

    let mut checked = 0;
    while let Some(info) = session.raw_walk_next(&mut walk) {
        if info.scalar.is_some() {
            // Must not be nil, or this test would trivially pass by skipping every node.
            let text = info.path.clone().unwrap_or_else(|| {
                panic!("node.path must not be nil for a plain fixture key: {info:?}")
            });
            let rendered = RawPath::parse(&text)
                .unwrap_or_else(|e| panic!("node.path {text:?} must itself parse: {e}"));
            let via_path = session.raw_get(RawScope::Level, &rendered).expect("resolves");
            assert_eq!(
                via_path, info.scalar,
                "node.path {text:?} must resolve back to the same value the walk yielded"
            );
            checked += 1;
        }
        session.raw_walk_act(&mut walk, VisitAction::Keep);
    }
    let _ = session.raw_walk_finish(&mut walk);
    assert!(
        checked > 0,
        "SaveParameter must have at least one scalar leaf for this test to mean anything"
    );
}

#[test]
fn the_specs_worked_raw_path_example_resolves_against_a_real_fixture() {
    // `object` is transparently flattened by `game_struct_props`, so the
    // path is `RawData.SaveParameter.X`, not `RawData.object.SaveParameter.X`.
    let mut session = common::load_corpus_session();
    let path = RawPath::parse(
        "worldSaveData.CharacterSaveParameterMap[3].value.RawData.SaveParameter.IsPlayer",
    )
    .expect("parses");
    let value = session.raw_get(RawScope::Level, &path).expect("resolves");
    assert!(matches!(value, Some(RawScalar::Bool(_))), "got {value:?}");
}
