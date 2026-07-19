mod common;

use psp_core::domain::{pal, world};
use psp_core::gamedata::GameData;
use psp_core::session::{SaveKind, SaveSession};
use psp_core::ue::games::palworld::PalCharacterData;
use psp_core::ue::{
    Header, MapEntry, PackageVersion, Properties, Property, PropertySchemas, Root, Save,
    StructValue,
};

fn game_data() -> GameData {
    let json_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/json");
    GameData::load(&json_dir).expect("data dir")
}

fn minimal_save(properties: Properties) -> Save {
    Save {
        header: Header {
            magic: 0,
            save_game_version: 0,
            package_version: PackageVersion { ue4: 0, ue5: None },
            engine_version_major: 0,
            engine_version_minor: 0,
            engine_version_patch: 0,
            engine_version_build: 0,
            engine_version: String::new(),
            custom_version: None,
        },
        schemas: PropertySchemas::default(),
        root: Root {
            save_game_type: String::new(),
            properties,
        },
        extra: Vec::new(),
    }
}

fn struct_property(properties: Properties) -> Property {
    Property::Struct(StructValue::Struct(properties))
}

fn guid_property(text: &str) -> Property {
    let guid: psp_core::ue::FGuid =
        serde_json::from_value(serde_json::Value::String(text.to_string())).unwrap();
    Property::Struct(StructValue::Guid(guid))
}

/// A well-formed non-player `CharacterSaveParameterMap` entry, built by hand
/// so a synthetic `save_parameter` (one missing `Rank`, say) can be fed
/// straight into `pal_summaries`.
fn pal_character_entry(instance_id: &str, save_parameter: Properties) -> MapEntry {
    let mut key_properties = Properties::default();
    key_properties.insert(
        "PlayerUId",
        guid_property("00000000-0000-0000-0000-000000000000"),
    );
    key_properties.insert("InstanceId", guid_property(instance_id));

    let mut object = Properties::default();
    object.insert("SaveParameter", struct_property(save_parameter));
    let character_data = PalCharacterData {
        object,
        unknown_bytes: [0; 4],
        group_id: psp_core::ue::FGuid::nil(),
        trailing_bytes: [0; 4],
    };
    let mut value_properties = Properties::default();
    value_properties.insert(
        "RawData",
        Property::Struct(StructValue::Game(psp_core::ue::PalStruct::CharacterData(character_data))),
    );

    MapEntry {
        key: struct_property(key_properties),
        value: struct_property(value_properties),
    }
}

/// A `SaveSession` whose `CharacterSaveParameterMap` is exactly `entries` and
/// which has no base camp map -- enough to run `pal_summaries` end to end
/// without a save file on disk.
fn session_with_character_map_entries(entries: Vec<MapEntry>) -> SaveSession {
    let mut world_save_data = Properties::default();
    world_save_data.insert("CharacterSaveParameterMap", Property::Map(entries));
    let mut root_properties = Properties::default();
    root_properties.insert("worldSaveData", struct_property(world_save_data));
    SaveSession::new_for_tests(SaveKind::InMemory, minimal_save(root_properties))
}

#[test]
fn every_corpus_pal_reads_into_a_dto() {
    let session = common::load_corpus_session();
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
            psp_core::dto::pal::format_character_key(&dto.character_id, pal::known_pal_keys(&data))
        );
        assert!(dto.level >= 1);
        pal_count += 1;
    }
    assert!(pal_count > 0);
}

#[test]
fn pal_summaries_match_python_defaults() {
    let session = common::load_corpus_session();
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
        // Every real pal already carries an explicit `Rank`, so this only
        // shows no corpus pal has `Rank == 0`; the "Rank absent -> 1" default
        // is covered by `pal_summaries_defaults_rank_to_one_when_rank_is_absent`.
        assert!(
            summary.rank >= 1,
            "no corpus pal has Rank == 0 (summaries.py get_pal_summaries)"
        );
    }
}

/// An absent `Rank` defaults to `1` in `PalSummary` but to `0` in the full
/// `PalDto` dump -- both are deliberate. No real save can prove the summary
/// side (every real pal carries an explicit `Rank`), so this feeds a
/// `SaveParameter` that genuinely has none.
#[test]
fn pal_summaries_defaults_rank_to_one_when_rank_is_absent() {
    let mut save_parameter = Properties::default();
    save_parameter.insert("CharacterID", Property::Name("SheepBall".to_string()));
    let entry = pal_character_entry("11111111-2222-3333-4444-555555555555", save_parameter);
    let session = session_with_character_map_entries(vec![entry]);
    let data = game_data();

    let summaries = pal::pal_summaries(&session, &data).unwrap();

    assert_eq!(1, summaries.len());
    assert_eq!(
        1, summaries[0].rank,
        "PalSummary defaults rank to 1 when Rank is absent (summaries.py get_pal_summaries)"
    );
}

/// An empty `Gender` string leaves `PalSummary.gender` as `None`, unlike the
/// full `PalDto` dump, which runs any present value through `from_prefixed`
/// and so defaults even an empty string to `Female`.
#[test]
fn pal_summaries_treats_an_empty_gender_string_as_absent() {
    let mut save_parameter = Properties::default();
    save_parameter.insert("CharacterID", Property::Name("SheepBall".to_string()));
    save_parameter.insert("Gender", Property::Enum(String::new()));
    let entry = pal_character_entry("11111111-2222-3333-4444-555555555555", save_parameter);
    let session = session_with_character_map_entries(vec![entry]);
    let data = game_data();

    let summaries = pal::pal_summaries(&session, &data).unwrap();

    assert_eq!(1, summaries.len());
    assert!(
        summaries[0].gender.is_none(),
        "an empty Gender string must resolve to None, not fall through to \
         PalGender::from_prefixed's Female default"
    );
}

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

/// `pal_summaries` derives `guild_id`/`base_id` by decoding the raw
/// `BaseCampSaveData`/`WorkerDirector` byte blob, so this pins the decode
/// against world1's known base: guild `54491484-...`, worker container
/// `a77f85ca-...`. Every one of world1's 11 pals sits in a party/pal-box
/// container rather than the base's worker container, so no pal in this
/// fixture ever resolves a `base_id`.
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

    let data = game_data();
    let summaries = pal::pal_summaries(&session, &data).unwrap();
    assert!(!summaries.is_empty());
    assert!(summaries.iter().all(|summary| summary.base_id.is_none()));
}

/// Save data is untrusted: an entry that isn't shaped like a pal at all must
/// be skipped, never panic.
#[test]
fn pal_dto_from_entry_returns_none_for_a_malformed_entry() {
    let malformed = psp_core::ue::MapEntry {
        key: psp_core::ue::Property::Bool(true),
        value: psp_core::ue::Property::Bool(true),
    };
    let data = game_data();

    assert!(pal::pal_dto_from_entry(&malformed, &data).is_none());
}

/// An entry whose `SaveParameter` is empty must still produce a `PalDto`:
/// every field in `read_save_parameter_dto` has a default for "key absent".
#[test]
fn read_save_parameter_dto_applies_every_default_for_an_empty_save_parameter() {
    let empty_save_parameter = psp_core::ue::Properties::default();
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
