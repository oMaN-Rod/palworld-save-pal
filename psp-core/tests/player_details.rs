mod common;

use psp_core::domain::{player, world};
use psp_core::gamedata::GameData;
use psp_core::progress::null_progress;
use psp_core::session::{LoadedPlayer, PlayerFileData, SaveKind, SaveSession};
use psp_core::ue::{
    Header, MapEntry, PackageVersion, Properties, Property, PropertySchemas, Root, Save,
    StructValue, ValueVec,
};
use uuid::Uuid;

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

fn guid_property(value: Uuid) -> Property {
    Property::Struct(StructValue::Guid(psp_core::props::uuid_to_guid(value)))
}

fn player_character_entry(
    player_uid: Uuid,
    instance_id: Uuid,
    mut save_parameter: Properties,
) -> MapEntry {
    save_parameter.insert("IsPlayer", Property::Bool(true));

    let mut key_properties = Properties::default();
    key_properties.insert("PlayerUId", guid_property(player_uid));
    key_properties.insert("InstanceId", guid_property(instance_id));

    let mut object = Properties::default();
    object.insert("SaveParameter", struct_property(save_parameter));
    let character_data = psp_core::ue::games::palworld::PalCharacterData {
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

/// A `SaveSession` whose `CharacterSaveParameterMap` is exactly `entries` --
/// enough to drive `build_player_dto` without a save file on disk.
fn session_with_character_map_entries(entries: Vec<MapEntry>) -> SaveSession {
    let mut world_save_data = Properties::default();
    world_save_data.insert("CharacterSaveParameterMap", Property::Map(entries));
    let mut root_properties = Properties::default();
    root_properties.insert("worldSaveData", struct_property(world_save_data));
    SaveSession::new_for_tests(SaveKind::InMemory, minimal_save(root_properties))
}

fn player_sav_save(timestamp_ticks: u64, save_data: Properties) -> Save {
    let mut root_properties = Properties::default();
    root_properties.insert(
        "Timestamp",
        Property::Struct(StructValue::DateTime(timestamp_ticks)),
    );
    // `SaveData` must exist for `save_data_props` to succeed, even if empty.
    root_properties.insert("SaveData", struct_property(save_data));
    minimal_save(root_properties)
}

/// `filtered_nickname` is deliberate: it is populated only for DPS pals and
/// reads "FilteredNickName", a different save property from the "NickName"
/// every pal uses for `nickname`. Writing it through "NickName" here would
/// silently test the wrong field.
fn dps_slot(instance_id: Uuid, character_id: &str, filtered_nickname: Option<&str>) -> StructValue {
    let mut save_parameter = Properties::default();
    save_parameter.insert("CharacterID", Property::Name(character_id.to_string()));
    if let Some(filtered_nickname) = filtered_nickname {
        save_parameter.insert(
            "FilteredNickName",
            Property::Str(filtered_nickname.to_string()),
        );
    }
    let mut inner_instance_id = Properties::default();
    inner_instance_id.insert("InstanceId", guid_property(instance_id));

    let mut slot_properties = Properties::default();
    slot_properties.insert("SaveParameter", struct_property(save_parameter));
    slot_properties.insert("InstanceId", struct_property(inner_instance_id));
    StructValue::Struct(slot_properties)
}

fn dps_save(slots: Vec<StructValue>) -> Save {
    let mut root_properties = Properties::default();
    root_properties.insert(
        "SaveParameterArray",
        Property::Array(ValueVec::Struct(slots)),
    );
    minimal_save(root_properties)
}

#[test]
fn ticks_conversion_is_correct() {
    assert_eq!(
        player::ticks_to_isoformat(638_000_000_000_000_000),
        "2022-09-28T22:13:20"
    );
    // Zero ticks is the year-1 epoch: `last_online_time` has no zero-guard,
    // unlike `PlayerSummary::last_online_time`.
    assert_eq!(player::ticks_to_isoformat(0), "0001-01-01T00:00:00");
}

const WORLD1_PLAYER_O: &str = "8c2f1930-0000-0000-0000-000000000000";
const WORLD1_PLAYER_SKY: &str = "43797f87-0000-0000-0000-000000000000";

/// The expected values below are world1 player `8c2f1930`'s real save data:
/// nickname "O", level 65, 7 technology points, 502 unlocked technologies, a
/// `SFBow_5` weapon and 6 armor dynamic items. Those equipment containers are
/// the only real-save coverage of `read_item_container`'s Weapon/Armor
/// branches (no fixture player has an Egg container).
#[test]
fn player_details_load_and_cache() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let player_id: Uuid = WORLD1_PLAYER_O.parse().unwrap();

    let details = player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .expect("player loads");

    assert_eq!(details.uid, player_id);
    assert_eq!(details.nickname, "O");
    assert_eq!(details.level, 65);
    assert_eq!(
        details.instance_id,
        Some("5669dbff-48ca-5113-6c52-2b939f0fb385".parse().unwrap())
    );
    assert_eq!(details.technology_points, 7);
    assert_eq!(details.technologies.len(), 502);
    assert_eq!(
        details.pal_box_id,
        Some("a6cb3db4-4760-f87f-c404-8b87887c0f29".parse().unwrap())
    );
    assert_eq!(
        details.otomo_container_id,
        Some("65a4f103-471b-3102-7bb3-18bcefee294d".parse().unwrap())
    );
    // Sub-second precision is the point: the fixture's raw tick value
    // (639111766067410000) is shared with `dto::summary::ticks_to_datetime`'s
    // own test, so a lossy ticks/1e7 division anywhere on this path goes red.
    // Compared via the JSON encoding, not `NaiveDateTime`'s `Display`.
    assert_eq!(
        details
            .last_online_time
            .map(|iso| serde_json::to_value(iso).unwrap()),
        Some(serde_json::json!("2026-04-07T16:36:46.740997"))
    );

    assert!(details.common_container.is_some(), "inventory loads");
    assert!(details.pal_box.is_some());
    assert!(details.party.is_some());
    // No fixture player has a `_dps.sav`, so `dps` is `None` (JSON `null`)
    // here rather than an empty map.
    assert!(details.dps.is_none());

    let weapon_container = details
        .weapon_load_out_container
        .as_ref()
        .expect("weapon container loads");
    assert_eq!(weapon_container.slots.len(), 1);
    let weapon_item = weapon_container.slots[0]
        .dynamic_item
        .as_ref()
        .expect("real weapon dynamic item resolves");
    assert_eq!(weapon_item.static_id, Some("SFBow_5".to_string()));
    assert_eq!(weapon_item.r#type, Some("weapon".to_string()));

    let armor_container = details
        .player_equipment_armor_container
        .as_ref()
        .expect("armor container loads");
    assert_eq!(armor_container.slots.len(), 6);
    assert!(
        armor_container
            .slots
            .iter()
            .all(|slot| slot.dynamic_item.is_some()),
        "every one of the 6 real armor slots must resolve its dynamic item"
    );
    assert!(armor_container.slots.iter().any(|slot| slot
        .dynamic_item
        .as_ref()
        .unwrap()
        .static_id
        .as_deref()
        == Some("HeadEquip041")));

    // Cross-check against `player_summaries.pal_count`, which is derived by a
    // separate code path (`pal_owner_counts`), not by this one.
    for (_, pal) in details.pals.iter() {
        assert_eq!(pal.owner_uid, Some(player_id));
    }
    assert_eq!(
        details.pals.len() as i64,
        session.player_summaries[&player_id].pal_count
    );

    assert!(session.loaded_players.contains_key(&player_id));
    assert!(session.player_summaries[&player_id].loaded);
}

#[test]
fn player_details_second_player_real_field_values() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let player_id: Uuid = WORLD1_PLAYER_SKY.parse().unwrap();

    let details = player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .expect("player loads");

    assert_eq!(details.nickname, "sky");
    assert_eq!(details.level, 2);
    assert_eq!(details.technology_points, 7);
    assert_eq!(details.technologies.len(), 7);
}

/// Broad but shallow coverage of the committed `v1_relics` corpus fixture.
#[test]
fn every_corpus_player_loads_without_panicking() {
    let mut session = common::load_corpus_session();
    let data = game_data();
    let player_ids: Vec<Uuid> = session.player_summaries.keys().copied().collect();
    assert!(!player_ids.is_empty());
    for player_id in player_ids {
        let details = player::get_player_details(&mut session, &data, player_id, &null_progress())
            .unwrap()
            .expect("every player_summaries entry must be loadable on demand");
        assert_eq!(details.uid, player_id);
        assert!(details.level >= 1);
        for (_, pal) in details.pals.iter() {
            assert_eq!(pal.owner_uid, Some(player_id));
        }
    }
}

#[test]
fn unknown_player_returns_none() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();

    let result =
        player::get_player_details(&mut session, &data, Uuid::new_v4(), &null_progress()).unwrap();

    assert!(result.is_none());
}

/// `get_player_details` must not re-parse a player's `.sav` on a second call.
#[test]
fn player_details_second_call_reuses_the_cached_loaded_sav_without_reparsing() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let player_id: Uuid = WORLD1_PLAYER_SKY.parse().unwrap();

    let first = player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .expect("player loads");
    assert_eq!(first.nickname, "sky");

    // The on-disk file is untouched, so only a genuine cache hit (no
    // re-read/re-parse) can make the second call observe this mutation.
    let loaded = session.loaded_players.get_mut(&player_id).unwrap();
    let timestamp = loaded
        .sav
        .root
        .properties
        .0
        .get_mut(&psp_core::ue::PropertyKey::from("Timestamp"))
        .unwrap();
    *timestamp = Property::Struct(StructValue::DateTime(0));

    let second = player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .expect("player still loads from cache");
    assert_eq!(
        second
            .last_online_time
            .map(|iso| serde_json::to_value(iso).unwrap()),
        Some(serde_json::json!("0001-01-01T00:00:00")),
        "second call must reflect the in-memory mutation, proving no re-parse happened"
    );
}

/// `get_player_details` is read-only over `session.level`, so every
/// position-keyed index must resolve identically before and after it runs and
/// no cache invalidation is required.
#[test]
fn get_player_details_never_moves_any_world_tree_index_position() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let player_id: Uuid = WORLD1_PLAYER_O.parse().unwrap();

    let character_index_before = world::build_character_index(&session.level);
    let item_container_index_before = world::build_item_container_index(&session.level);
    let character_container_index_before = world::build_character_container_index(&session.level);
    let dynamic_item_index_before = world::build_dynamic_item_index(&session.level);

    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .expect("player loads");

    assert_eq!(
        character_index_before,
        world::build_character_index(&session.level)
    );
    assert_eq!(
        item_container_index_before,
        world::build_item_container_index(&session.level)
    );
    assert_eq!(
        character_container_index_before,
        world::build_character_container_index(&session.level)
    );
    assert_eq!(
        dynamic_item_index_before,
        world::build_dynamic_item_index(&session.level)
    );
}

// The DPS branch has no real-save coverage: neither world1 nor world2
// contains a single `*_dps.sav`. The synthetic saves below are its only
// proof, hand-built to the real `SaveParameterArray` shape.

fn player_id_and_instance() -> (Uuid, Uuid) {
    (
        "11111111-2222-3333-4444-555555555555".parse().unwrap(),
        "66666666-7777-8888-9999-aaaaaaaaaaaa".parse().unwrap(),
    )
}

fn minimal_player_save_parameter() -> Properties {
    let mut save_parameter = Properties::default();
    save_parameter.insert("NickName", Property::Str("Tester".to_string()));
    save_parameter.insert("Level", psp_core::props::byte_property(10));
    save_parameter
}

#[test]
fn build_player_dto_populates_dps_pals_and_filters_the_none_placeholder() {
    let (player_id, instance_id) = player_id_and_instance();
    let entry = player_character_entry(player_id, instance_id, minimal_player_save_parameter());
    let mut session = session_with_character_map_entries(vec![entry]);

    let dps_pal_id = Uuid::parse_str("aaaaaaaa-0000-0000-0000-000000000000").unwrap();
    let dps = dps_save(vec![
        dps_slot(dps_pal_id, "SheepBall", Some("DPS Sheep")),
        // An unused arena slot: `character_id == "None"` must be filtered out.
        dps_slot(Uuid::nil(), "None", None),
    ]);
    session.loaded_players.insert(
        player_id,
        LoadedPlayer {
            uid: player_id,
            sav: player_sav_save(0, Properties::default()),
            dps: Some(dps),
        },
    );

    let data = game_data();
    let details = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .expect("player dto builds");

    let dps_map = details.dps.expect("dps file was present -> Some, not null");
    assert_eq!(
        dps_map.len(),
        1,
        "the 'None' placeholder slot must be filtered out"
    );
    let (index, dps_pal) = dps_map.iter().next().unwrap();
    assert_eq!(*index, 0);
    assert_eq!(dps_pal.instance_id, dps_pal_id);
    assert_eq!(dps_pal.character_id, "SheepBall");
    assert_eq!(dps_pal.filtered_nickname, Some("DPS Sheep".to_string()));
}

/// With no `_dps.sav` loaded, `PlayerDto.dps` must serialize as `null`, not
/// `{}` -- a distinction the frontend depends on.
#[test]
fn build_player_dto_dps_is_none_when_no_dps_file_was_loaded() {
    let (player_id, instance_id) = player_id_and_instance();
    let entry = player_character_entry(player_id, instance_id, minimal_player_save_parameter());
    let mut session = session_with_character_map_entries(vec![entry]);
    session.loaded_players.insert(
        player_id,
        LoadedPlayer {
            uid: player_id,
            sav: player_sav_save(0, Properties::default()),
            dps: None,
        },
    );

    let data = game_data();
    let details = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .expect("player dto builds");

    assert!(details.dps.is_none());
    let value = serde_json::to_value(&details).unwrap();
    assert!(value["dps"].is_null());
}

#[test]
fn get_player_details_returns_none_when_player_has_no_file_ref() {
    let (player_id, instance_id) = player_id_and_instance();
    let entry = player_character_entry(player_id, instance_id, minimal_player_save_parameter());
    let mut session = session_with_character_map_entries(vec![entry]);
    // `player_file_refs` is deliberately left empty.
    assert!(!session.player_file_refs.contains_key(&player_id));

    let data = game_data();
    let result =
        player::get_player_details(&mut session, &data, player_id, &null_progress()).unwrap();

    assert!(result.is_none());
}

/// A file ref that resolves to no `sav` bytes (an orphaned `_dps.sav`-only
/// entry) must return `None`, not error.
#[test]
fn get_player_details_returns_none_when_file_ref_has_no_sav_bytes() {
    let (player_id, instance_id) = player_id_and_instance();
    let entry = player_character_entry(player_id, instance_id, minimal_player_save_parameter());
    let mut session = session_with_character_map_entries(vec![entry]);
    session.player_file_refs.insert(
        player_id,
        PlayerFileData::Bytes {
            sav: None,
            dps: None,
        },
    );

    let data = game_data();
    let result =
        player::get_player_details(&mut session, &data, player_id, &null_progress()).unwrap();

    assert!(result.is_none());
}

#[test]
fn build_player_dto_returns_none_for_a_player_id_never_loaded() {
    let session = session_with_character_map_entries(vec![]);
    let data = game_data();

    let result = player::build_player_dto(&session, &data, Uuid::new_v4()).unwrap();

    assert!(result.is_none());
}

/// `PlayerDto`'s absent-nickname fallback is the ninja-emoji pattern,
/// distinct from `PlayerSummary`'s "Player (xxxxxxxx)" -- not interchangeable.
#[test]
fn build_player_dto_falls_back_to_the_ninja_emoji_nickname_when_nickname_is_absent() {
    let (player_id, instance_id) = player_id_and_instance();
    let mut save_parameter = Properties::default();
    save_parameter.insert("Level", psp_core::props::byte_property(1));
    let entry = player_character_entry(player_id, instance_id, save_parameter);
    let mut session = session_with_character_map_entries(vec![entry]);
    session.loaded_players.insert(
        player_id,
        LoadedPlayer {
            uid: player_id,
            sav: player_sav_save(0, Properties::default()),
            dps: None,
        },
    );

    let data = game_data();
    let details = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .expect("player dto builds");

    let expected_suffix = format!("({})", player_id.to_string().split('-').next().unwrap());
    assert!(
        details.nickname.ends_with(&expected_suffix),
        "got nickname {:?}",
        details.nickname
    );
    assert!(details.nickname.starts_with('\u{1f977}'));
}

/// A player the frontend has not opened yet is present in `player_summaries`
/// (eagerly populated) but absent from `loaded_players` (lazily populated), so
/// `build_player_dto` alone returns `None` for them. WS handlers must gate on
/// `player_summaries`, force-load via `ensure_player_loaded`, and only then
/// call `build_player_dto` -- otherwise a valid player reads as "not found".
#[test]
fn ensure_player_loaded_resolves_a_real_but_unloaded_player_for_build_player_dto() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let player_id: Uuid = WORLD1_PLAYER_SKY.parse().unwrap();

    assert!(
        session.player_summaries.contains_key(&player_id),
        "fixture player must be a real, eagerly-known player"
    );
    assert!(
        !session.loaded_players.contains_key(&player_id),
        "a freshly loaded session must not have lazily loaded any player yet"
    );

    assert!(
        player::build_player_dto(&session, &data, player_id)
            .unwrap()
            .is_none(),
        "build_player_dto must return None before the player is loaded"
    );

    session
        .ensure_player_loaded(player_id)
        .expect("a real, file-ref-backed player must load without error");
    assert!(session.loaded_players.contains_key(&player_id));

    let details = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .expect("build_player_dto must resolve the player once loaded");
    assert_eq!(details.uid, player_id);
    assert_eq!(details.nickname, "sky");
}

/// The force-load above must not resurrect a nonexistent player:
/// `ensure_player_loaded` is a no-op for a uid with no file reference, so a
/// bogus uid still resolves to `None` ("Player not found").
#[test]
fn ensure_player_loaded_is_a_no_op_for_an_unknown_player_and_stays_unresolved() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let bogus_uid = Uuid::new_v4();

    assert!(!session.player_summaries.contains_key(&bogus_uid));
    assert!(!session.player_file_refs.contains_key(&bogus_uid));

    session
        .ensure_player_loaded(bogus_uid)
        .expect("a uid with no file reference is a documented no-op, not an error");

    assert!(
        !session.loaded_players.contains_key(&bogus_uid),
        "a bogus uid must never be inserted into loaded_players"
    );
    assert!(
        player::build_player_dto(&session, &data, bogus_uid)
            .unwrap()
            .is_none(),
        "a genuinely nonexistent player must still resolve to None (\"Player not found\")"
    );
}
