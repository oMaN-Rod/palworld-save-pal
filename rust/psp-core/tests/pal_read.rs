mod common;

use psp_core::domain::{pal, world};
use psp_core::gamedata::GameData;

/// `GameData::load` takes the `data/json` directory itself. From
/// `rust/psp-core` (this crate's `CARGO_MANIFEST_DIR`) that's `../../data/json`
/// -- matching `gamedata.rs`'s own `loads_the_real_repo_data_dir` test, the
/// established Phase 1 interface. The brief's `Path::new("../data")` is
/// neither the manifest-relative path Phase 1 actually uses nor safe against
/// a non-crate-root working directory (a plain relative path depends on
/// `cwd`, which differs between `cargo test` invocations and IDEs); fixed
/// here per this task's "brief vs Python/Phase-1 source -- source wins" rule.
fn game_data() -> GameData {
    let json_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/json");
    GameData::load(&json_dir).expect("data dir")
}

#[test]
fn every_corpus_pal_reads_into_a_dto() {
    let Some(session) = common::load_corpus_session() else {
        return;
    };
    let data = game_data();
    let entries = world::character_map(&session.level).unwrap();
    let mut pal_count = 0;
    for entry in entries {
        if world::entry_is_player(entry) {
            continue;
        }
        let dto = pal::pal_dto_from_entry(entry, &data).expect("pal DTO");
        assert!(!dto.character_id.is_empty());
        assert_eq!(
            dto.character_key,
            psp_core::dto::pal::format_character_key(
                &dto.character_id,
                &pal::known_pal_keys(&data)
            )
        );
        assert!(dto.level >= 1);
        pal_count += 1;
    }
    assert!(pal_count > 0);
}

#[test]
fn pal_summaries_match_python_defaults() {
    let Some(session) = common::load_corpus_session() else {
        return;
    };
    let data = game_data();
    let summaries = pal::pal_summaries(&session, &data).unwrap();
    let entries = world::character_map(&session.level).unwrap();
    let pal_entry_count = entries
        .iter()
        .filter(|e| !world::entry_is_player(e))
        .count();
    assert_eq!(summaries.len(), pal_entry_count);
    for summary in &summaries {
        assert!(summary.level >= 1);
        assert!(
            summary.rank >= 1,
            "summaries default rank to 1 (summaries.py get_pal_summaries)"
        );
    }
}

/// Real-save coverage that always runs (not gated behind `PSP_TEST_SAVE_DIR`):
/// `tests/fixtures/saves/world1` is checked into the repo. Asserts concrete
/// field values, not just "some DTO came back" -- proving the accessors
/// actually read the right save-file properties, not just that they don't
/// panic.
#[test]
fn world1_fixture_pals_have_real_field_values() {
    let session = common::load_fixture_session("world1");
    let data = game_data();
    let entries = world::character_map(&session.level).unwrap();

    let mut pal_count = 0;
    let mut saw_nonzero_hp = false;
    let mut saw_nonempty_character_key = false;
    for entry in entries {
        if world::entry_is_player(entry) {
            continue;
        }
        let dto = pal::pal_dto_from_entry(entry, &data).expect("pal DTO");
        assert!(
            !dto.character_id.is_empty(),
            "every real pal has a CharacterID"
        );
        assert!(dto.level >= 1);
        assert!(dto.max_hp >= 0);
        if dto.hp > 0 {
            saw_nonzero_hp = true;
        }
        if !dto.character_key.is_empty() {
            saw_nonempty_character_key = true;
        }
        pal_count += 1;
    }
    assert!(
        pal_count > 0,
        "world1 fixture must contain at least one pal"
    );
    assert!(
        saw_nonzero_hp,
        "at least one real pal should carry non-zero HP"
    );
    assert!(
        saw_nonempty_character_key,
        "at least one real pal should resolve a non-empty character_key"
    );
}

/// `pal_summaries` derives `guild_id`/`base_id` via
/// `domain::guild::base_guild_and_container`, which decodes the real
/// `BaseCampSaveData`/`WorkerDirector` byte blob (`palbin::
/// worker_director_container_id`) rather than reading a typed struct field
/// that doesn't exist in uesave-rs (see this task's report). world1's single
/// base camp has no pals actually slotted into its worker container (both of
/// its 11 pals sit in party/pal-box containers instead), so `pal_summaries`
/// itself has no real-save case where a `base_id` resolves to `Some` in this
/// particular fixture -- asserting that would test the fixture's data, not
/// this code. What *is* real-save-verified here is the base entry's own
/// decode, checked directly against ground truth independently confirmed
/// via `.venv` Python (`GvasFile.read` on the same fixture): base key
/// `4bb24de8-4965-af19-f596-e296089e8ab0`, `group_id_belong_to`
/// `54491484-4e6c-7327-70b2-868f350929f6`, WorkerDirector `container_id`
/// `a77f85ca-4037-97d8-acef-fcb73f1d931b`.
#[test]
fn world1_fixture_base_camp_worker_director_decodes_to_known_real_values() {
    let session = common::load_fixture_session("world1");
    let bases = session.base_camp_map().expect("world1 has a base camp");
    assert_eq!(1, bases.len());

    let (guild_id, container_id) =
        psp_core::domain::guild::base_guild_and_container(&bases[0]).expect("decodes cleanly");

    assert_eq!("54491484-4e6c-7327-70b2-868f350929f6", guild_id.to_string());
    assert_eq!(
        "a77f85ca-4037-97d8-acef-fcb73f1d931b",
        container_id.to_string()
    );

    // pal_summaries itself must still run cleanly end to end over this save
    // (guild_id/base_id simply stay None for every pal in this fixture).
    let data = game_data();
    let summaries = pal::pal_summaries(&session, &data).unwrap();
    assert!(!summaries.is_empty());
    assert!(summaries.iter().all(|summary| summary.base_id.is_none()));
}

/// A `CharacterSaveParameterMap` entry that isn't shaped like a pal at all
/// (untrusted save data: a wrong-typed value, no `RawData`) must be skipped
/// by `pal_dto_from_entry`, never panic -- matching Python's `PalObjects.
/// get_nested` returning `None` through a broken chain rather than raising.
#[test]
fn pal_dto_from_entry_returns_none_for_a_malformed_entry() {
    let malformed = uesave::MapEntry {
        key: uesave::Property::Bool(true),
        value: uesave::Property::Bool(true),
    };
    let data = game_data();

    assert!(pal::pal_dto_from_entry(&malformed, &data).is_none());
}

/// A well-formed character entry whose `SaveParameter` is simply empty (no
/// properties at all) must still produce a `PalDto` -- every field in
/// `read_save_parameter_dto` has a Python-matching default for "key absent".
#[test]
fn read_save_parameter_dto_applies_every_default_for_an_empty_save_parameter() {
    let empty_save_parameter = uesave::Properties::default();
    let data = game_data();
    let instance_id = uuid::Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap();

    let dto = pal::read_save_parameter_dto(&empty_save_parameter, instance_id, false, &data);

    assert_eq!(dto.instance_id, instance_id);
    assert_eq!(dto.character_id, "");
    assert_eq!(dto.level, 1, "Level defaults to 1 (game/pal.py Pal.level)");
    assert_eq!(
        dto.rank, 0,
        "Rank defaults to 0 in the full Pal dump (game/pal.py Pal.rank)"
    );
    assert_eq!(
        dto.stomach, 150.0,
        "FullStomach defaults to 150.0 (game/pal.py Pal.stomach)"
    );
    assert_eq!(
        dto.sanity, 100.0,
        "SanityValue defaults to 100.0 (game/pal.py Pal.sanity)"
    );
    assert_eq!(dto.hp, 0);
    assert_eq!(dto.exp, 0);
    assert!(!dto.is_sick);
    assert!(dto.nickname.is_none());
    assert!(dto.owner_uid.is_none());
    assert_eq!(dto.storage_id, psp_core::props::EMPTY_UUID);
    assert_eq!(dto.storage_slot, 0);
    assert!(dto.learned_skills.is_empty());
    assert!(dto.active_skills.is_empty());
    assert!(dto.passive_skills.is_empty());
    assert!(dto.work_suitability.is_empty());
}
