//! Save-out (`.sav` bytes), world rename, and the byte-validity resave gate.
//!
//! The gates here compare against the original file bytes rather than the
//! decompressed GVAS: the game compresses `Level.sav` with the same Oodle
//! Mermaid/Normal settings this crate drives, so decompress -> recompress
//! reproduces the original file exactly.

mod common;

use psp_core::domain::{guild_tail, pal, player};
use psp_core::dto::ordered_map::OrderedMap;
use psp_core::gamedata::GameData;
use psp_core::progress::null_progress;
use psp_core::session::{SaveKind, SaveSession};

fn game_data() -> GameData {
    GameData::load(std::path::Path::new("../data")).expect("data dir")
}

fn fixture_file(relative: &str) -> Vec<u8> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/saves")
        .join(relative);
    std::fs::read(&path).unwrap_or_else(|error| panic!("read fixture {relative}: {error}"))
}

/// A no-edit resave of `Level.sav` must reproduce the original file
/// byte-for-byte: the whole read -> write compressed pipeline is lossless.
/// The fixture is ~112 KB, so this goes red the moment any codec, schema, or
/// the Oodle compressor drifts by a single byte.
#[test]
fn untouched_level_resaves_byte_identical() {
    let session = common::load_fixture_session("world1");
    let original = fixture_file("world1/Level.sav");
    let rewritten = session.level_sav_bytes().expect("write level sav");
    assert!(original.len() > 1000, "fixture must be a real save");
    assert_eq!(
        rewritten, original,
        "no-edit Level.sav resave must be byte-identical"
    );
}

#[test]
fn untouched_corpus_level_resaves_byte_identical() {
    let session = common::load_corpus_session();
    let original = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/saves/v1_relics/Level.sav"),
    )
    .expect("read corpus Level.sav");
    let rewritten = session.level_sav_bytes().expect("write corpus level sav");
    assert_eq!(
        rewritten, original,
        "no-edit corpus Level.sav resave must be byte-identical"
    );
}

#[test]
fn untouched_level_meta_resaves_byte_identical() {
    let session = common::load_fixture_session("world1");
    let original = fixture_file("world1/LevelMeta.sav");
    let rewritten = session
        .level_meta_sav_bytes()
        .expect("write level meta sav")
        .expect("world1 has a LevelMeta.sav");
    assert!(original.len() > 100, "fixture must be a real meta save");
    assert_eq!(
        rewritten, original,
        "no-edit LevelMeta.sav resave must be byte-identical"
    );
}

#[test]
fn edited_pal_reloads_with_edit_applied_and_entry_count_stable() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();

    let (player_id, pal_id, original_level) = {
        let player_ids: Vec<uuid::Uuid> = session.player_summaries.keys().copied().collect();
        let mut found = None;
        for player_id in player_ids {
            player::get_player_details(&mut session, &data, player_id, &null_progress())
                .expect("player details")
                .expect("player exists");
            let details = player::build_player_dto(&session, &data, player_id)
                .expect("build dto")
                .expect("dto present");
            let first = details
                .pals
                .iter()
                .next()
                .map(|(pal_id, source)| (*pal_id, source.level));
            if let Some((pal_id, level)) = first {
                found = Some((player_id, pal_id, level));
                break;
            }
        }
        found.expect("world1 has at least one player-owned pal")
    };

    let target_level = if original_level == 50 { 42 } else { 50 };
    assert_ne!(
        target_level, original_level,
        "edit must change the value or the test is tautological"
    );

    let source = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap()
        .pals
        .get(&pal_id)
        .expect("pal present")
        .clone();
    let mut edited = source;
    edited.level = target_level;
    let mut modified = OrderedMap::new();
    modified.insert(pal_id, edited);
    pal::update_pals(&mut session, &data, &modified, &null_progress()).expect("update pals");

    let entry_count = psp_core::domain::world::character_map(&session.level)
        .unwrap()
        .len();
    let sav_bytes = session.level_sav_bytes().expect("write level sav");
    let reloaded = psp_core::savio::read_sav_bytes(&sav_bytes).expect("re-read level sav");

    assert_eq!(
        psp_core::domain::world::character_map(&reloaded)
            .unwrap()
            .len(),
        entry_count,
        "editing a pal must not change the entry count"
    );
    let reloaded_entry = psp_core::domain::world::character_map(&reloaded)
        .unwrap()
        .iter()
        .find(|entry| psp_core::domain::world::entry_instance_id(entry) == Some(pal_id))
        .expect("edited pal survives the round trip");
    let reread = pal::pal_dto_from_entry(reloaded_entry, &data).expect("re-read pal dto");
    assert_eq!(
        reread.level, target_level,
        "the pal-level edit must survive write→read"
    );
}

/// A pal stat edit touches `CharacterSaveParameterMap` only, so every guild's tail in
/// `GroupSaveDataMap` must come out byte-identical.
#[test]
fn edit_one_pal_leaves_guild_tails_byte_identical() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();

    let untouched_tails: Vec<(uuid::Uuid, psp_core::ue::games::palworld::PalGroupVariant)> =
        psp_core::domain::world::group_map(&session.level)
            .unwrap()
            .iter()
            .filter_map(|entry| {
                let guild_id = psp_core::props::as_uuid(&entry.key)?;
                let group = guild_tail::entry_group_data(entry)?;
                Some((guild_id, group.data.clone()))
            })
            .collect();
    assert!(
        !untouched_tails.is_empty(),
        "world1 must have at least one guild tail to make this test meaningful"
    );

    let (player_id, pal_id) = {
        let player_ids: Vec<uuid::Uuid> = session.player_summaries.keys().copied().collect();
        let mut found = None;
        for player_id in player_ids {
            player::get_player_details(&mut session, &data, player_id, &null_progress())
                .unwrap()
                .unwrap();
            let details = player::build_player_dto(&session, &data, player_id)
                .unwrap()
                .unwrap();
            let first = details.pals.iter().next().map(|(pal_id, _)| *pal_id);
            if let Some(pal_id) = first {
                found = Some((player_id, pal_id));
                break;
            }
        }
        found.expect("world1 has a player-owned pal")
    };

    let mut edited = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap()
        .pals
        .get(&pal_id)
        .unwrap()
        .clone();
    edited.nickname = Some("ByteCheck".to_string());
    let mut modified = OrderedMap::new();
    modified.insert(pal_id, edited);
    pal::update_pals(&mut session, &data, &modified, &null_progress()).unwrap();

    for entry in psp_core::domain::world::group_map(&session.level).unwrap() {
        let Some(guild_id) = psp_core::props::as_uuid(&entry.key) else {
            continue;
        };
        let Some(group) = guild_tail::entry_group_data(entry) else {
            continue;
        };
        let original = untouched_tails
            .iter()
            .find(|(id, _)| *id == guild_id)
            .expect("guild present before edit");
        assert_eq!(
            group.data, original.1,
            "a pal stat edit must leave the guild's structured data untouched"
        );
    }
}

#[test]
fn rename_world_updates_meta_and_survives_resave() {
    let mut session = common::load_fixture_session("world1");
    assert!(
        session.level_meta.is_some(),
        "world1 fixture must carry a LevelMeta.sav"
    );

    let original_name = session.world_name.clone();
    let new_name = format!("{original_name} Renamed");
    session.set_world_name(&new_name).expect("rename world");
    assert_eq!(session.world_name, new_name);

    let meta_bytes = session
        .level_meta_sav_bytes()
        .expect("write meta")
        .expect("meta present");
    let reloaded_meta = psp_core::savio::read_sav_bytes(&meta_bytes).expect("re-read meta");
    let world_name =
        psp_core::props::get(&reloaded_meta.root.properties, &["SaveData", "WorldName"])
            .and_then(psp_core::props::as_str);
    assert_eq!(world_name, Some(new_name.as_str()));
}

#[test]
fn set_world_name_without_level_meta_errors_with_python_message() {
    let level = read_level_only();
    let mut session = SaveSession::new_for_tests(SaveKind::InMemory, level);
    assert!(session.level_meta.is_none());

    let error = session.set_world_name("Whatever").unwrap_err();
    assert_eq!(error.to_string(), "No LevelMeta GvasFile has been loaded.");
}

/// `apply_player_dto` writes `SaveData.bossTechnologyPoint` unconditionally, but a
/// player `.sav` that never carried the property has no write schema for it.
#[test]
fn edited_player_save_out_succeeds_after_boss_technology_point_fix() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let player_id = *session.player_summaries.keys().next().unwrap();
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();
    let dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    let mut modified = OrderedMap::new();
    modified.insert(player_id, dto);
    player::update_players(&mut session, &data, &modified, &null_progress())
        .expect("update players");

    let player_files = session
        .player_sav_bytes()
        .expect("player save-out must succeed with bossTechnologyPoint schema registered");
    let (sav_bytes, _dps) = player_files.get(&player_id).expect("edited player emitted");
    assert!(
        sav_bytes.len() > 100,
        "written player .sav must be non-trivial"
    );

    psp_core::savio::read_sav_bytes(sav_bytes).expect("edited player .sav re-reads cleanly");
}

/// `apply_player_dto` writes `SanityValue` into the player's own
/// `CharacterSaveParameterMap` entry on every edit, which has no write schema for
/// that path. If `level_sav_bytes()` fails here, the edit is reported as saved while
/// not one byte reached disk -- silent data loss.
#[test]
fn edited_player_level_sav_bytes_succeeds_and_edit_round_trips() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let player_id = *session.player_summaries.keys().next().unwrap();
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();
    let mut dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    dto.level = 60;
    let mut modified = OrderedMap::new();
    modified.insert(player_id, dto);
    player::update_players(&mut session, &data, &modified, &null_progress())
        .expect("update players");

    let level_bytes = session
        .level_sav_bytes()
        .expect("Level.sav re-serialization must succeed after a player edit (SanityValue schema)");

    let reloaded = SaveSession::new_for_tests(
        SaveKind::InMemory,
        psp_core::savio::read_sav_bytes(&level_bytes).expect("edited Level.sav re-reads cleanly"),
    );
    let entries = psp_core::domain::world::character_map(&reloaded.level).unwrap();
    let reloaded_entry = entries
        .iter()
        .find(|entry| {
            psp_core::domain::world::entry_is_player(entry)
                && psp_core::domain::world::entry_player_uid(entry) == Some(player_id)
        })
        .expect("edited player still present in the reloaded Level.sav");
    let save_parameter = psp_core::domain::world::entry_save_parameter(reloaded_entry)
        .expect("reloaded player entry has a save parameter");
    let level_after = save_parameter
        .0
        .get(&psp_core::ue::PropertyKey::from("Level"))
        .expect("Level property present");
    assert_eq!(
        level_after,
        &psp_core::props::byte_property(60),
        "the edited level must survive the Level.sav write+reread, not just the in-memory apply"
    );
}

/// `new_pal_entry` writes the new pal's slot struct under the all-caps key `SlotID`,
/// but every pal already on disk spells it `SlotId`, so uesave has recorded a write
/// schema only for the `SlotId` paths.
#[test]
fn add_player_pal_then_resave_succeeds_and_pal_round_trips() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();

    let (player_id, pal_box_id) = {
        let mut found = None;
        for candidate in session.player_summaries.keys().copied().collect::<Vec<_>>() {
            player::get_player_details(&mut session, &data, candidate, &null_progress())
                .unwrap()
                .unwrap();
            let details = player::build_player_dto(&session, &data, candidate)
                .unwrap()
                .unwrap();
            if let Some(pal_box_id) = details.pal_box_id {
                found = Some((candidate, pal_box_id));
                break;
            }
        }
        found.expect("world1 has a player with a pal box")
    };

    let new_pal = pal::add_player_pal(
        &mut session,
        &data,
        player_id,
        "Sheepball",
        "slotid-fix",
        pal_box_id,
        None,
    )
    .unwrap()
    .expect("world1's pal box has room for one more pal");

    // A freshly added pal's wire `hp` is the fixed 545000 placeholder written into its
    // `HP` property: the reader looks at `Hp`, which the new entry never sets.
    assert_eq!(
        new_pal.hp, 545_000,
        "a newly added pal must report Python's placeholder HP, not the computed max_hp"
    );

    let sav_bytes = session
        .level_sav_bytes()
        .expect("re-serializing Level.sav after adding a pal must succeed (SlotID schema)");

    let reloaded = psp_core::savio::read_sav_bytes(&sav_bytes).expect("re-read level sav");
    let survived = psp_core::domain::world::character_map(&reloaded)
        .unwrap()
        .iter()
        .any(|entry| {
            psp_core::domain::world::entry_instance_id(entry) == Some(new_pal.instance_id)
        });
    assert!(
        survived,
        "the added pal's InstanceId must be present after the write→read round trip"
    );
}

/// A base pal built by `add_guild_pal` carries the same all-caps `SlotID`.
#[test]
fn add_guild_pal_then_resave_succeeds_and_pal_round_trips() {
    // world1's founding guild + its one base (empty worker container).
    const WORLD1_GUILD_WITH_BASE: &str = "54491484-4e6c-7327-70b2-868f350929f6";
    const WORLD1_BASE_ID: &str = "4bb24de8-4965-af19-f596-e296089e8ab0";

    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let guild_id: uuid::Uuid = WORLD1_GUILD_WITH_BASE.parse().unwrap();
    let base_id: uuid::Uuid = WORLD1_BASE_ID.parse().unwrap();

    // `add_guild_pal` requires the guild to be loaded this session.
    psp_core::domain::guild::get_guild_details(&mut session, &data, guild_id)
        .unwrap()
        .expect("guild loads");

    let new_pal = pal::add_guild_pal(
        &mut session,
        &data,
        guild_id,
        base_id,
        "Sheepball",
        "slotid-fix-guild",
        None,
    )
    .unwrap()
    .expect("world1's base worker container has room");

    assert_eq!(
        new_pal.hp, 545_000,
        "a newly added guild pal must report Python's placeholder HP, not the computed max_hp"
    );

    let sav_bytes = session
        .level_sav_bytes()
        .expect("re-serializing Level.sav after adding a guild pal must succeed (SlotID schema)");

    let reloaded = psp_core::savio::read_sav_bytes(&sav_bytes).expect("re-read level sav");
    let survived = psp_core::domain::world::character_map(&reloaded)
        .unwrap()
        .iter()
        .any(|entry| {
            psp_core::domain::world::entry_instance_id(entry) == Some(new_pal.instance_id)
        });
    assert!(
        survived,
        "the added guild pal's InstanceId must be present after the write→read round trip"
    );
}

fn read_level_only() -> psp_core::ue::Save {
    let level_bytes = fixture_file("world1/Level.sav");
    psp_core::savio::read_sav_bytes(&level_bytes).expect("parse level")
}

/// `uesave` writes from a borrow rather than mutating the property tree, so no
/// deep-copy guard is needed between writes.
#[test]
fn writing_the_same_session_twice_is_byte_identical() {
    let session = common::load_fixture_session("world1");
    let first = session.level_sav_bytes().expect("first write");
    let second = session.level_sav_bytes().expect("second write");
    assert_eq!(
        first, second,
        "a second write of an unmodified session must be byte-identical"
    );
}

/// A raw `assert_eq!` on two multi-kilobyte byte vectors prints both in full and
/// names no offset, which says nothing about how far the drift goes.
fn describe_byte_drift(label: &str, written: &[u8], original: &[u8]) -> Option<String> {
    if written == original {
        return None;
    }
    let first_diff = written
        .iter()
        .zip(original.iter())
        .position(|(a, b)| a != b);
    let differing = written
        .iter()
        .zip(original.iter())
        .filter(|(a, b)| a != b)
        .count();
    let window = first_diff.map(|at| {
        let end = (at + 16).min(written.len().min(original.len()));
        (written[at..end].to_vec(), original[at..end].to_vec())
    });
    // A `PlM` container opens with u32 uncompressed_len, u32 compressed_len: the pair
    // separates a payload that really changed from one only the compressor spelled
    // differently.
    let plm_lengths = |bytes: &[u8]| -> Option<(u32, u32)> {
        Some((
            u32::from_le_bytes(bytes.get(0..4)?.try_into().ok()?),
            u32::from_le_bytes(bytes.get(4..8)?.try_into().ok()?),
        ))
    };
    Some(format!(
        "{label}: written {} bytes, original {} bytes ({:+}); \
         PlM (uncompressed_len, compressed_len) written {:?}, original {:?}; \
         first differing offset {:?}; {differing} differing bytes in the common prefix; \
         window written {:?}; window original {:?}",
        written.len(),
        original.len(),
        written.len() as i64 - original.len() as i64,
        plm_lengths(written),
        plm_lengths(original),
        first_diff,
        window.as_ref().map(|(w, _)| w),
        window.as_ref().map(|(_, o)| o),
    ))
}

/// A no-edit resave of a player's own `.sav` must reproduce the original file
/// byte-for-byte, the same way `untouched_level_resaves_byte_identical` pins
/// `Level.sav`. Nothing else in the repo covers the player files, and any caller
/// that force-loads a player's GVAS to read it -- without editing anything --
/// makes every subsequent save-out rewrite that file from this round trip.
#[test]
fn untouched_player_sav_resaves_byte_identical() {
    let mut session = common::load_corpus_session();
    let refs: Vec<(uuid::Uuid, Option<std::path::PathBuf>, Option<std::path::PathBuf>)> = session
        .player_file_refs
        .iter()
        .map(|(uid, data)| match data {
            psp_core::session::PlayerFileData::Paths { sav, dps } => {
                (*uid, sav.clone(), dps.clone())
            }
            _ => (*uid, None, None),
        })
        .collect();
    assert!(
        refs.iter().filter(|(_, sav, _)| sav.is_some()).count() > 1,
        "the corpus fixture must carry several player .sav files"
    );
    assert!(
        refs.iter().any(|(_, _, dps)| dps.is_some()),
        "the corpus fixture must carry at least one _dps.sav"
    );

    for (uid, _, _) in &refs {
        session
            .ensure_player_loaded(*uid)
            .unwrap_or_else(|error| panic!("force-load player {uid}: {error}"));
    }

    let written = session.player_sav_bytes().expect("write player savs");
    let mut compared = 0;
    // Every file is compared before reporting: stopping at the first would hide whether
    // the drift is one bad file or all of them.
    let mut drift = Vec::new();
    for (uid, sav_path, dps_path) in &refs {
        let Some((sav_bytes, dps_bytes)) = written.get(uid) else {
            panic!("player {uid} was force-loaded but emitted no bytes");
        };
        if let Some(path) = sav_path {
            let original = std::fs::read(path).expect("read fixture player .sav");
            assert!(original.len() > 100, "fixture player .sav must be a real save");
            drift.extend(describe_byte_drift(
                &format!("player {uid}'s .sav"),
                sav_bytes,
                &original,
            ));
            compared += 1;
        }
        if let Some(path) = dps_path {
            let original = std::fs::read(path).expect("read fixture player _dps.sav");
            let rewritten = dps_bytes.as_ref().expect("a loaded dps must emit bytes");
            drift.extend(describe_byte_drift(
                &format!("player {uid}'s _dps.sav"),
                rewritten,
                &original,
            ));
            compared += 1;
        }
    }
    assert!(compared > 1, "the comparison must have actually run");
    assert!(
        drift.is_empty(),
        "{} of {compared} player file(s) did not resave byte-identically:\n{}",
        drift.len(),
        drift.join("\n")
    );
}
