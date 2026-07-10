mod common;

use psp_core::domain::{containers, pal, world};
use psp_core::gamedata::GameData;

/// `GameData::load` takes the `data/json` directory. From `rust/psp-core`
/// (this crate's `CARGO_MANIFEST_DIR`) that's `../../data/json` -- matching
/// `pal_read.rs`'s own established helper, not the brief's `../data` (fixed
/// per this task's "brief vs. established Phase-1 interface -- interface
/// wins" rule; see this task's report).
fn game_data() -> GameData {
    let json_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/json");
    GameData::load(&json_dir).expect("data dir")
}

/// Real-save coverage that always runs (not gated behind `PSP_TEST_SAVE_DIR`):
/// mutate a real pal from `tests/fixtures/saves/world1`, extract it back
/// through Task 5's real `pal_dto_from_entry`, and assert every field the
/// mutation touched actually round-tripped -- not just "the property is
/// present now", but the exact values a caller would see on the wire.
#[test]
fn apply_dto_round_trips_through_reader_on_a_real_pal() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let entry_index = {
        let entries = world::character_map(&session.level).unwrap();
        entries
            .iter()
            .position(|e| !world::entry_is_player(e))
            .expect("world1 has at least one pal")
    };
    let mut dto = {
        let entries = world::character_map(&session.level).unwrap();
        pal::pal_dto_from_entry(&entries[entry_index], &data).unwrap()
    };
    dto.nickname = Some("Renamed".to_string());
    dto.level = 42;
    dto.rank = 3;
    dto.is_lucky = Some(true);
    dto.friendship_point = 777;
    dto.talent_hp = 90;
    dto.rank_hp = 2;
    dto.learned_skills = vec!["EPalWazaID::Unique_SheepBall_Roll".to_string()];
    dto.active_skills = vec!["EPalWazaID::Unique_Test_Skill".to_string()];
    dto.passive_skills = vec!["SomePassive".to_string()];

    pal::ensure_pal_property_schemas(&mut session.level);
    {
        let entries = world::character_map_mut(&mut session.level).unwrap();
        let save_parameter = world::entry_save_parameter_mut(&mut entries[entry_index]).unwrap();
        pal::apply_pal_dto(save_parameter, &dto, false, &data);
    }

    let entries = world::character_map(&session.level).unwrap();
    let reread = pal::pal_dto_from_entry(&entries[entry_index], &data).unwrap();

    assert_eq!(reread.nickname.as_deref(), Some("Renamed"));
    assert_eq!(reread.level, 42);
    assert_eq!(reread.rank, 3);
    assert_eq!(reread.is_lucky, Some(true));
    assert_eq!(reread.friendship_point, 777);
    assert_eq!(reread.talent_hp, 90);
    assert_eq!(reread.rank_hp, 2);
    assert_eq!(
        reread.learned_skills,
        vec!["EPalWazaID::Unique_SheepBall_Roll".to_string()]
    );
    assert_eq!(
        reread.active_skills,
        vec!["EPalWazaID::Unique_Test_Skill".to_string()]
    );
    assert_eq!(reread.passive_skills, vec!["SomePassive".to_string()]);
    assert_eq!(reread.hp, reread.max_hp, "update_from sets hp = max_hp");
    assert!(!reread.is_sick, "update_from heals non-DPS pals");
    assert_eq!(
        reread.sanity, 100.0,
        "heal() always resets sanity to 100 for non-DPS pals"
    );
    assert!(
        reread.character_id.starts_with("BOSS_"),
        "lucky pals get a BOSS_ prefix"
    );
}

/// Regression test for a real bug found in the brief's own reference
/// implementation: `update_from` (`game/pal.py`) skips `is_lucky` entirely
/// when the DTO value is `None` (`if value is None: continue`) -- it does
/// NOT treat "absent" the same as "false" (which would remove an existing
/// `IsRarePal`). An implementation that force-removes `IsRarePal` whenever
/// `dto.is_lucky` isn't `Some(true)` would fail this test.
#[test]
fn apply_dto_is_lucky_none_leaves_existing_is_rare_pal_untouched() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let entry_index = {
        let entries = world::character_map(&session.level).unwrap();
        entries
            .iter()
            .position(|e| !world::entry_is_player(e))
            .expect("a pal")
    };

    // First, make the pal lucky (a real, present IsRarePal property).
    let mut dto = {
        let entries = world::character_map(&session.level).unwrap();
        pal::pal_dto_from_entry(&entries[entry_index], &data).unwrap()
    };
    dto.is_lucky = Some(true);
    pal::ensure_pal_property_schemas(&mut session.level);
    {
        let entries = world::character_map_mut(&mut session.level).unwrap();
        let save_parameter = world::entry_save_parameter_mut(&mut entries[entry_index]).unwrap();
        pal::apply_pal_dto(save_parameter, &dto, false, &data);
    }
    let after_lucky = {
        let entries = world::character_map(&session.level).unwrap();
        pal::pal_dto_from_entry(&entries[entry_index], &data).unwrap()
    };
    assert_eq!(after_lucky.is_lucky, Some(true));

    // Now apply an update whose is_lucky is None (e.g. a partial edit that
    // never touched luck) -- IsRarePal must survive untouched.
    let mut second_dto = after_lucky.clone();
    second_dto.is_lucky = None;
    second_dto.nickname = Some("StillLucky".to_string());
    {
        let entries = world::character_map_mut(&mut session.level).unwrap();
        let save_parameter = world::entry_save_parameter_mut(&mut entries[entry_index]).unwrap();
        pal::apply_pal_dto(save_parameter, &second_dto, false, &data);
    }
    let final_dto = {
        let entries = world::character_map(&session.level).unwrap();
        pal::pal_dto_from_entry(&entries[entry_index], &data).unwrap()
    };
    assert_eq!(
        final_dto.is_lucky,
        Some(true),
        "is_lucky: None must skip the IsRarePal setter entirely, not remove it"
    );
    assert_eq!(final_dto.nickname.as_deref(), Some("StillLucky"));
}

/// Regression test for a second real bug found in the brief: the boss-prefix
/// decision at the end of `update_from` must be derived from the
/// already-updated `character_id`/`is_lucky` (`self.is_boss or
/// self.is_lucky`, which simplifies to `character_id.upper().startswith
/// ("BOSS_") or is_lucky`), never from the DTO's own (possibly stale)
/// `is_boss` field -- `is_boss` has no setter in Python and is unconditionally
/// skipped by `update_from`'s loop. A stale `is_boss: Some(true)` on a
/// non-boss, non-lucky pal must NOT add a `BOSS_` prefix.
#[test]
fn apply_dto_stale_is_boss_flag_does_not_add_boss_prefix() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let entry_index = {
        let entries = world::character_map(&session.level).unwrap();
        entries
            .iter()
            .position(|e| !world::entry_is_player(e))
            .expect("a pal")
    };
    let mut dto = {
        let entries = world::character_map(&session.level).unwrap();
        pal::pal_dto_from_entry(&entries[entry_index], &data).unwrap()
    };
    assert!(
        !dto.character_id.starts_with("BOSS_"),
        "fixture precondition: pal is not already a boss"
    );
    // A stale is_boss=true that does not match the actual character_id/luck.
    dto.is_boss = Some(true);
    dto.is_lucky = Some(false);

    pal::ensure_pal_property_schemas(&mut session.level);
    {
        let entries = world::character_map_mut(&mut session.level).unwrap();
        let save_parameter = world::entry_save_parameter_mut(&mut entries[entry_index]).unwrap();
        pal::apply_pal_dto(save_parameter, &dto, false, &data);
    }

    let reread = {
        let entries = world::character_map(&session.level).unwrap();
        pal::pal_dto_from_entry(&entries[entry_index], &data).unwrap()
    };
    assert!(
        !reread.character_id.starts_with("BOSS_"),
        "a stale dto.is_boss=true must not add a BOSS_ prefix when character_id/is_lucky don't warrant it"
    );
}

/// `heal_save_parameter` in isolation: removes every sick marker, forces
/// `SanityValue = 100.0` and `FullStomach` to the pal's max.
#[test]
fn heal_save_parameter_clears_sickness_and_resets_sanity_and_stomach() {
    let data = game_data();
    let mut save_parameter = uesave::Properties::default();
    save_parameter.insert(
        "CharacterID",
        uesave::Property::Name("Sheepball".to_string()),
    );
    save_parameter.insert(
        "PalReviveTimer",
        uesave::Property::Float(uesave::Float(30.0)),
    );
    save_parameter.insert("WorkerSick", uesave::Property::Bool(true));
    save_parameter.insert("SanityValue", uesave::Property::Float(uesave::Float(12.0)));

    pal::heal_save_parameter(&mut save_parameter, "Sheepball", &data);

    let dto = pal::read_save_parameter_dto(&save_parameter, uuid::Uuid::nil(), false, &data);
    assert!(!dto.is_sick, "every sick marker must be removed");
    assert_eq!(dto.sanity, 100.0);
    assert_eq!(
        dto.stomach,
        pal::max_stomach_for("Sheepball", &data),
        "FullStomach must be reset to the pal's max, not left at whatever it was"
    );
}

/// `max_stomach_for`: a recognized pal with a `pals.json` `max_full_stomach`
/// entry uses it; an unrecognized character_id falls back to the flat 300.0
/// default. ("Alpaca" -> 225.0 verified against real `data/json/pals.json`,
/// same fixture value `domain::pal`'s own read-side NaN-guard test uses.)
#[test]
fn max_stomach_for_uses_pals_json_when_recognized_else_the_flat_default() {
    let data = game_data();
    assert_eq!(pal::max_stomach_for("Alpaca", &data), 225.0);
    assert_eq!(pal::max_stomach_for("TotallyMadeUpCreature", &data), 300.0);
}

/// `new_pal_entry` builds a complete, independently readable
/// `CharacterSaveParameterMap` entry -- proven by round-tripping it through
/// Task 5's real `pal_dto_from_entry`, not by inspecting the raw property
/// tree.
#[test]
fn new_pal_entry_reads_back() {
    let data = game_data();
    let instance_id = uuid::Uuid::new_v4();
    let owner = uuid::Uuid::new_v4();
    let container = uuid::Uuid::new_v4();
    let entry = pal::new_pal_entry("Sheepball", instance_id, owner, container, 4, None, "wooly");

    let dto = pal::pal_dto_from_entry(&entry, &data).expect("readable");
    assert_eq!(dto.instance_id, instance_id);
    assert_eq!(dto.character_id, "Sheepball");
    assert_eq!(dto.nickname.as_deref(), Some("wooly"));
    assert_eq!(dto.owner_uid, Some(owner));
    assert_eq!(dto.storage_id, container);
    assert_eq!(dto.storage_slot, 4);
    assert_eq!(dto.level, 1);
    assert_eq!(dto.exp, 0);
    assert_eq!(dto.talent_hp, 50);
    assert_eq!(dto.talent_shot, 50);
    assert_eq!(dto.talent_defense, 50);
    assert_eq!(dto.stomach, 300.0);
    assert!(dto.learned_skills.is_empty());
    assert!(dto.active_skills.is_empty());
    assert!(dto.passive_skills.is_empty());
    assert!(
        dto.group_id.is_none(),
        "None group_id -> nil -> not surfaced"
    );
}

/// `new_pal_entry`'s `group_id` parameter, when `Some`, is readable back
/// through the same `PalCharacterData.group_id` path `pal_dto_from_entry`
/// already decodes (not exercised by the test above, which passes `None`).
#[test]
fn new_pal_entry_surfaces_a_real_group_id() {
    let data = game_data();
    let group_id = uuid::Uuid::new_v4();
    let entry = pal::new_pal_entry(
        "Sheepball",
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4(),
        0,
        Some(group_id),
        "wooly",
    );

    let dto = pal::pal_dto_from_entry(&entry, &data).expect("readable");
    assert_eq!(dto.group_id, Some(group_id));
}

/// Strong proof that `ensure_pal_property_schemas` is not merely
/// compileable but functionally sufficient: mutate a real world1 pal that
/// (confirmed via a since-deleted debug test, see this task's report)
/// carries NEITHER `IsRarePal` NOR `MasteredWaza` NOR `SanityValue` to begin
/// with, apply a DTO that introduces all three fresh, then actually
/// serialize the WHOLE session to bytes through `uesave::Save::write`.
/// `uesave` refuses to write any property whose path has no recorded schema
/// (`Error::MissingPropertySchema`) -- if `ensure_pal_property_schemas`
/// were missing an entry this mutation needs, this test would fail with
/// that error, not just an assertion.
#[test]
fn ensure_pal_property_schemas_makes_a_freshly_introduced_property_set_actually_serializable() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let entry_index = {
        let entries = world::character_map(&session.level).unwrap();
        entries
            .iter()
            .position(|e| !world::entry_is_player(e))
            .expect("a pal")
    };

    let mut dto = {
        let entries = world::character_map(&session.level).unwrap();
        pal::pal_dto_from_entry(&entries[entry_index], &data).unwrap()
    };
    // Precondition: none of these three are present on the chosen pal yet.
    {
        let entries = world::character_map(&session.level).unwrap();
        let save_parameter = world::entry_save_parameter(&entries[entry_index]).unwrap();
        for absent in ["IsRarePal", "MasteredWaza", "SanityValue"] {
            assert!(
                psp_core::props::get(save_parameter, &[absent]).is_none(),
                "fixture precondition: {absent} must be absent from the chosen pal"
            );
        }
    }

    dto.is_lucky = Some(true); // introduces IsRarePal
    dto.learned_skills = vec!["EPalWazaID::Unique_SheepBall_Roll".to_string()]; // introduces MasteredWaza

    pal::ensure_pal_property_schemas(&mut session.level);
    {
        let entries = world::character_map_mut(&mut session.level).unwrap();
        let save_parameter = world::entry_save_parameter_mut(&mut entries[entry_index]).unwrap();
        pal::apply_pal_dto(save_parameter, &dto, false, &data);
    }

    let mut buffer = Vec::new();
    session
        .level
        .write(&mut buffer)
        .expect("every freshly introduced property must have a registered write schema");
    assert!(!buffer.is_empty());
}

#[test]
fn character_container_add_and_remove_on_a_real_container() {
    let mut session = common::load_fixture_session("world1");
    let container_index = world::build_character_container_index(&session.level);
    let (_, entry_index) = container_index
        .iter()
        .next()
        .expect("world1 has a container");
    let entry_index = *entry_index;

    let before = containers::read_character_container(&session.level, entry_index).unwrap();
    let pal_id = uuid::Uuid::new_v4();
    let slot =
        containers::character_container_add_pal(&mut session.level, entry_index, pal_id, None)
            .unwrap();
    if before.slots.len() as i32 >= before.size {
        assert!(slot.is_none(), "a full container returns None");
        return;
    }
    let assigned_slot = slot.expect("assigned");
    let after = containers::read_character_container(&session.level, entry_index).unwrap();
    assert_eq!(after.slots.len(), before.slots.len() + 1);
    assert!(after
        .slots
        .iter()
        .any(|s| s.pal_id == Some(pal_id) && s.slot_index == assigned_slot));

    containers::character_container_remove_pal(&mut session.level, entry_index, pal_id).unwrap();
    let restored = containers::read_character_container(&session.level, entry_index).unwrap();
    assert_eq!(restored.slots.len(), before.slots.len());
}

/// `character_container_add_pal`/`remove_pal` mutate a `Slots` array nested
/// *inside* an already-positioned `CharacterContainerSaveData` entry -- they
/// never insert or remove a map entry, so the container's own position (what
/// `build_character_container_index`/`SaveSession::character_container_index`
/// cache) must never move. Direct, real-save proof that no cache
/// invalidation call is needed anywhere in this task -- see this task's
/// report for the full reasoning.
#[test]
fn container_mutation_never_moves_any_containers_index_position() {
    let mut session = common::load_fixture_session("world1");
    let before = world::build_character_container_index(&session.level);
    assert!(
        before.len() > 1,
        "world1 must have more than one container to prove position stability"
    );

    let (&container_id, &entry_index) = before
        .iter()
        .find(|(_, &idx)| idx != 0)
        .unwrap_or_else(|| before.iter().next().unwrap());
    let pal_id = uuid::Uuid::new_v4();
    let _ = containers::character_container_add_pal(&mut session.level, entry_index, pal_id, None);
    let _ = containers::character_container_remove_pal(&mut session.level, entry_index, pal_id);

    let after = world::build_character_container_index(&session.level);
    assert_eq!(
        before, after,
        "no container's position may change from a slot-level mutation"
    );
    assert_eq!(after.get(&container_id), Some(&entry_index));
}

/// Corpus-gated variant of `apply_dto_round_trips_through_reader_on_a_real_pal`
/// (skips when `PSP_TEST_SAVE_DIR` is unset, per this workspace's
/// established convention for larger, non-committed save corpora): the same
/// mutate/reread/assert shape, run against every pal in whatever save the
/// environment points at, rather than just the first one in the committed
/// `world1` fixture. Keeping this alongside the always-run fixture tests
/// above also keeps `common::load_corpus_session` from going unused in this
/// binary (`cargo clippy -D warnings` flags an unused `pub fn` per test
/// binary, since each integration test file is compiled as its own crate).
#[test]
fn apply_dto_round_trips_through_reader_across_the_whole_corpus() {
    let Some(mut session) = common::load_corpus_session() else {
        return;
    };
    let data = game_data();
    pal::ensure_pal_property_schemas(&mut session.level);

    let pal_positions: Vec<usize> = {
        let entries = world::character_map(&session.level).unwrap();
        entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| !world::entry_is_player(entry))
            .map(|(position, _)| position)
            .collect()
    };
    assert!(!pal_positions.is_empty());

    for entry_index in pal_positions {
        let mut dto = {
            let entries = world::character_map(&session.level).unwrap();
            let Some(dto) = pal::pal_dto_from_entry(&entries[entry_index], &data) else {
                continue;
            };
            dto
        };
        dto.friendship_point += 1;
        {
            let entries = world::character_map_mut(&mut session.level).unwrap();
            let save_parameter =
                world::entry_save_parameter_mut(&mut entries[entry_index]).unwrap();
            pal::apply_pal_dto(save_parameter, &dto, false, &data);
        }
        let entries = world::character_map(&session.level).unwrap();
        let reread = pal::pal_dto_from_entry(&entries[entry_index], &data).unwrap();
        assert_eq!(reread.friendship_point, dto.friendship_point);
        assert_eq!(reread.hp, reread.max_hp);
    }
}
