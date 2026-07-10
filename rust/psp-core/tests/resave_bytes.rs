//! Phase-2 Task 12 — save-out (`.sav` bytes), world rename, and the
//! byte-validity resave gate.
//!
//! Every test here runs UNCONDITIONALLY off the committed fixture
//! `tests/fixtures/saves/world1` (via `common::load_fixture_session`), never
//! the env-gated private corpus. Two reasons: (1) it discharges the Phase-1
//! deferred item "commit a fixture so the write gate isn't env-gated" — bare
//! CI now has real protection for the resave path; (2) the (C) compression
//! experiment (below) proved this port's Oodle output is byte-identical to
//! the game's original `Level.sav`, so an unconditional original-file gate is
//! legitimate here rather than a GVAS-layer fallback.
//!
//! **(C) compression experiment result (run during implementation, then
//! folded into `untouched_level_resaves_byte_identical`).** Loading
//! `world1/Level.sav` (112 879 compressed bytes → 1 339 023 GVAS bytes) and
//! re-writing it produced output byte-identical at BOTH layers: the
//! compressed `.sav` (112 879 bytes, `compressed_identical=true`) and the
//! decompressed GVAS (1 339 023 bytes, `gvas_identical=true`). The game
//! compressed the original with the same Oodle Mermaid/Normal settings this
//! port drives, so decompress→recompress reproduces the original file. Gate
//! chosen: compare `level_sav_bytes()` against the raw original file bytes
//! (the brief's strong form), NOT the GVAS-layer fallback (C) allows when
//! recompression diverges.

mod common;

use psp_core::domain::{guild_tail, pal, player};
use psp_core::dto::ordered_map::OrderedMap;
use psp_core::gamedata::GameData;
use psp_core::progress::null_progress;
use psp_core::session::{SaveKind, SaveSession};

fn game_data() -> GameData {
    GameData::load(std::path::Path::new("../../data")).expect("data dir")
}

fn fixture_file(relative: &str) -> Vec<u8> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/saves")
        .join(relative);
    std::fs::read(&path).unwrap_or_else(|error| panic!("read fixture {relative}: {error}"))
}

/// The (C) gate, strong form: a no-edit resave of `Level.sav` reproduces the
/// original file byte-for-byte. Proves the whole read→write compressed
/// pipeline (`savio::read_sav_bytes` → `savio::write_sav_bytes`) is
/// lossless. Non-trivial: the fixture is ~112 KB, and this fails the moment
/// any codec, schema, or the Oodle compressor drifts by a single byte.
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

/// The same no-edit resave gate on the PRIVATE corpus named by
/// `PSP_TEST_SAVE_DIR` (skips loudly when unset — see
/// `common::load_corpus_session`). The committed fixture above already runs
/// this gate unconditionally on bare CI; this widens it to whatever
/// larger/other real save a developer points at, catching byte drift the one
/// small fixture might not exercise.
#[test]
fn untouched_corpus_level_resaves_byte_identical() {
    let Some(session) = common::load_corpus_session() else {
        return;
    };
    let original = std::fs::read(
        std::path::Path::new(&std::env::var("PSP_TEST_SAVE_DIR").unwrap()).join("Level.sav"),
    )
    .expect("read corpus Level.sav");
    let rewritten = session.level_sav_bytes().expect("write corpus level sav");
    assert_eq!(
        rewritten, original,
        "no-edit corpus Level.sav resave must be byte-identical"
    );
}

/// Same gate for `LevelMeta.sav` — exercises `level_meta_sav_bytes()`'s
/// `Some` branch and proves the meta write path is lossless too.
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

/// An edited pal survives the full write→read round trip with its edit
/// applied, and editing a pal never changes the character-map entry count.
/// Non-tautological: `target_level` is chosen to DIFFER from the pal's
/// current level, so a resave that silently dropped the edit would read back
/// the original and fail.
#[test]
fn edited_pal_reloads_with_edit_applied_and_entry_count_stable() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();

    // Find a player that actually owns at least one pal.
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

/// Editing one pal's nickname leaves every guild's opaque raw tail
/// (`GroupSaveDataMap` `remaining_data`) byte-identical — a pal stat edit
/// touches `CharacterSaveParameterMap`, never the guild tail blob. Captures
/// the tails before the edit and asserts each is unchanged after, so a
/// regression that accidentally re-encoded a tail would go red.
#[test]
fn edit_one_pal_leaves_guild_tails_byte_identical() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();

    let untouched_tails: Vec<(uuid::Uuid, Vec<u8>)> =
        psp_core::domain::world::group_map(&session.level)
            .unwrap()
            .iter()
            .filter_map(|entry| {
                let guild_id = psp_core::props::as_uuid(&entry.key)?;
                let group = guild_tail::entry_group_data(entry)?;
                Some((guild_id, group.remaining_data.clone()))
            })
            .collect();
    assert!(
        !untouched_tails.is_empty(),
        "world1 must have at least one guild tail to make this test meaningful"
    );

    // Find and edit one player-owned pal's nickname.
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
            group.remaining_data, original.1,
            "a pal stat edit must leave the guild raw tail untouched"
        );
    }
}

/// `set_world_name` updates both `self.world_name` and the LevelMeta
/// `SaveData.WorldName` property, and the rename survives a
/// `level_meta_sav_bytes()` write→read round trip.
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

/// `set_world_name` with no LevelMeta loaded errors with EXACTLY Python's
/// message (`save_manager.py:193`: `raise ValueError("No LevelMeta GvasFile
/// has been loaded.")`). Unconditional (synthetic session).
#[test]
fn set_world_name_without_level_meta_errors_with_python_message() {
    let level = read_level_only();
    let mut session = SaveSession::new_for_tests(SaveKind::InMemory, level);
    assert!(session.level_meta.is_none());

    let error = session.set_world_name("Whatever").unwrap_err();
    assert_eq!(error.to_string(), "No LevelMeta GvasFile has been loaded.");
}

/// Pins the `bossTechnologyPoint` schema fix (Task 10 → Task 12
/// carry-forward). `apply_player_dto` writes `SaveData.bossTechnologyPoint`
/// unconditionally, but the world1 fixture player's `.sav` has no schema for
/// it — so before the fix (`ensure_boss_technology_point_schema`), writing an
/// edited player `.sav` through `player_sav_bytes()` failed with
/// `missing property schema for path: SaveData.bossTechnologyPoint`. This
/// test edits a player, saves out, and asserts the write succeeds. It fails
/// (Err on the resave) if the schema fix is removed.
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

    // And it must round-trip back to a parseable save.
    psp_core::savio::read_sav_bytes(sav_bytes).expect("edited player .sav re-reads cleanly");
}

/// Pins the `SlotID` write-schema fix (Task 14b). `new_pal_entry` inserts the
/// new pal's slot struct under the all-caps key `SlotID` (Python's
/// `PalObjects.PalCharacterSlotId`), but every pal already on disk spells it
/// `SlotId`, so uesave recorded a write-schema only for the `SlotId` paths.
/// Before the fix, adding a pal then re-serializing `Level.sav` failed with
/// `missing property schema for path: worldSaveData.CharacterSaveParameterMap.
/// RawData.SaveParameter.SlotID`. This adds a pal via `add_player_pal`, asserts
/// `level_sav_bytes()` succeeds, and confirms the new pal survives a read-back
/// (its `InstanceId` is present in the reloaded `CharacterSaveParameterMap`).
#[test]
fn add_player_pal_then_resave_succeeds_and_pal_round_trips() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();

    // Find a player whose pal box exists and has room for one more pal.
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
        "SheepBall",
        "slotid-fix",
        pal_box_id,
        None,
    )
    .unwrap()
    .expect("world1's pal box has room for one more pal");

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

/// The guild-add sibling of the above, on world1's real founding guild + base
/// (see `pal_crud.rs`'s `WORLD1_GUILD_WITH_BASE`/`WORLD1_BASE_ID`). A base pal
/// built by `add_guild_pal` also carries the all-caps `SlotID`, so the same
/// schema gap broke `level_sav_bytes()` for guild adds until Task 14b.
#[test]
fn add_guild_pal_then_resave_succeeds_and_pal_round_trips() {
    // world1's founding guild + its one real base (empty worker container).
    const WORLD1_GUILD_WITH_BASE: &str = "54491484-4e6c-7327-70b2-868f350929f6";
    const WORLD1_BASE_ID: &str = "4bb24de8-4965-af19-f596-e296089e8ab0";

    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let guild_id: uuid::Uuid = WORLD1_GUILD_WITH_BASE.parse().unwrap();
    let base_id: uuid::Uuid = WORLD1_BASE_ID.parse().unwrap();

    // add_guild_pal requires the guild to be loaded this session.
    psp_core::domain::guild::get_guild_details(&mut session, &data, guild_id)
        .unwrap()
        .expect("guild loads");

    let new_pal = pal::add_guild_pal(
        &mut session,
        &data,
        guild_id,
        base_id,
        "SheepBall",
        "slotid-fix-guild",
        None,
    )
    .unwrap()
    .expect("world1's base worker container has room");

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

/// Builds a `SaveSession` from just `world1/Level.sav`, with no LevelMeta —
/// the state `set_world_name` must reject.
fn read_level_only() -> uesave::Save {
    let level_bytes = fixture_file("world1/Level.sav");
    psp_core::savio::read_sav_bytes(&level_bytes).expect("parse level")
}
