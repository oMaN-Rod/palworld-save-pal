//! A property at its default value is absent from a Palworld save, and `uesave`
//! schemas only what it read -- so anything this app introduces must be primed or
//! the write fails with `missing property schema for path: ...`.
//!
//! These pin the save shapes that hit that in the wild: no pal at all, no insane
//! pal, and an empty `_dps.sav`.
//!
//! Every one of them asserts the bytes PARSE BACK, never merely that the write
//! returned `Ok`. A primed tag that disagrees with the real one serializes happily
//! and produces a save neither this app nor the game can read -- which is exactly
//! what shipped when these tests only checked that serialization succeeded.

mod common;

use psp_core::domain::{containers, pal, player};
use psp_core::dto::container::DynamicItemDto;
use psp_core::dto::ordered_map::OrderedMap;
use psp_core::dto::pal::PalGender;
use psp_core::gamedata::GameData;
use psp_core::progress::null_progress;
use psp_core::session::SaveSession;

fn game_data() -> GameData {
    let json_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/json");
    GameData::load(&json_dir).expect("data dir")
}

/// Serializes `Level.sav` and reads it straight back. Writing bytes the reader
/// chokes on is the failure this guards -- `level_sav_bytes()` alone cannot see it.
fn written_level_parses_back(session: &SaveSession, what: &str) {
    let bytes = session
        .level_sav_bytes()
        .unwrap_or_else(|e| panic!("level.sav must serialize after {what}: {e}"));
    if let Err(e) = psp_core::savio::read_sav_bytes(&bytes) {
        panic!("level.sav written after {what} does not parse back: {e}");
    }
}

/// Schemas as the file recorded them. The session primes on load, so the gaps these
/// tests pin are only visible on a raw parse.
fn raw_schema_paths(fixture: &str, file: &str) -> Vec<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/saves")
        .join(fixture)
        .join(file);
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let save = psp_core::savio::read_sav_bytes(&bytes).expect("parse fixture save");
    save.schemas.schemas().keys().cloned().collect()
}

fn loaded_player_ids(
    session: &mut psp_core::session::SaveSession,
    data: &GameData,
) -> Vec<uuid::Uuid> {
    let ids: Vec<uuid::Uuid> = session.player_file_refs.keys().copied().collect();
    ids.into_iter()
        .filter(|id| {
            player::get_player_details(session, data, *id, &null_progress())
                .ok()
                .flatten()
                .is_some()
        })
        .collect()
}

/// `world2` holds no pal, so it recorded no `SaveParameter.CharacterID` -- the anchor
/// the primer used to derive its prefix from, bailing out and registering nothing.
/// A player edit then writes `SanityValue` and `Level`, which the save also lacks.
#[test]
fn player_edit_serializes_on_a_save_that_holds_no_pal() {
    let data = game_data();
    let mut session = common::load_fixture_session("world2");
    let id = *loaded_player_ids(&mut session, &data)
        .first()
        .expect("fixture has a loadable player");

    let dto = player::get_player_details(&mut session, &data, id, &null_progress())
        .expect("get details")
        .expect("player loads");
    let mut modified = OrderedMap::default();
    modified.insert(id, dto);
    player::update_players(&mut session, &data, &modified, &null_progress()).expect("update");

    written_level_parses_back(&session, "a player edit");
}

/// Healing re-introduces `SanityValue`, which no save records until a pal actually
/// goes insane. The heal paths primed nothing at all.
#[test]
fn healing_serializes_on_a_save_where_no_pal_is_insane() {
    let data = game_data();
    assert!(
        !raw_schema_paths("world1", "Level.sav")
            .iter()
            .any(|k| k.ends_with("SaveParameter.SanityValue")),
        "fixture precondition: world1's Level.sav must record no SanityValue schema"
    );
    let mut session = common::load_fixture_session("world1");

    for id in loaded_player_ids(&mut session, &data) {
        pal::heal_all_player_pals(&mut session, &data, id).expect("heal");
    }

    written_level_parses_back(&session, "healing");
}

/// The new character-map entry and the container slot pointing at it are both built
/// from scratch, in a save that recorded a schema for neither.
#[test]
fn adding_the_first_pal_to_a_world_that_has_none_serializes() {
    let data = game_data();
    let mut session = common::load_fixture_session("world2");
    let id = *loaded_player_ids(&mut session, &data)
        .first()
        .expect("fixture has a loadable player");

    let dto = player::get_player_details(&mut session, &data, id, &null_progress())
        .expect("get details")
        .expect("player loads");
    let pal_box = dto.pal_box.as_ref().expect("player has a pal box").id;

    pal::add_player_pal(&mut session, &data, id, "Lamball", "Fluffy", pal_box, None)
        .expect("add pal");

    // The new pal's character-container slot is a `PalCharacterContainer` RawData.
    // A world with no pal has empty `Slots`, so no tag for it was ever recorded and
    // the primer supplies one -- a wrong tag here writes a save the game silently
    // drops the pal from, and that this app then cannot reopen.
    written_level_parses_back(&session, "adding the first pal");
}

/// An egg carries a pal's `SaveParameter` under `DynamicItemSaveData.RawData`. A save
/// with no egg records no schema for that `SaveParameter` struct *node* (only its
/// children are primed), so writing the first egg failed with
/// `missing property schema for path: worldSaveData.DynamicItemSaveData.RawData.SaveParameter`.
#[test]
fn adding_an_egg_serializes_on_a_save_that_holds_no_egg() {
    assert!(
        !raw_schema_paths("world2", "Level.sav")
            .iter()
            .any(|k| k == "worldSaveData.DynamicItemSaveData.RawData.SaveParameter"),
        "fixture precondition: world2 must record no egg SaveParameter node schema"
    );
    let mut session = common::load_fixture_session("world2");

    let egg = DynamicItemDto {
        local_id: uuid::Uuid::from_u128(0x0123_4567_89ab_cdef),
        modified: true,
        character_id: Some("Lamball".to_string()),
        character_key: None,
        durability: None,
        passive_skill_list: None,
        remaining_bullets: None,
        r#type: Some("egg".to_string()),
        static_id: Some("PalEgg_Normal_01".to_string()),
        gender: Some(PalGender::Female),
        active_skills: Some(vec![]),
        learned_skills: Some(vec![]),
        passive_skills: Some(vec![]),
        talent_hp: Some(50),
        talent_shot: Some(50),
        talent_defense: Some(50),
    };
    containers::upsert_dynamic_item(&mut session, &egg, "PalEgg_Normal_01").expect("add egg");

    written_level_parses_back(&session, "adding an egg");
}

/// A `_dps.sav`'s slots are empty until a pal is stored, so its `GotStatusPointList`
/// has no element and no element schema. Adding the first DPS pal writes
/// `{StatusName, StatusPoint}` rows into it, and nothing primed the DPS save.
#[test]
fn adding_a_dps_pal_serializes_when_the_dps_slots_are_empty() {
    let data = game_data();
    assert!(
        !raw_schema_paths(
            "v1_stats",
            "Players/00000000000000000000000000000001_dps.sav"
        )
        .iter()
        .any(|k| k.ends_with("GotStatusPointList.StatusName")),
        "fixture precondition: the _dps.sav must record no status-point element schema"
    );

    let mut session = common::load_fixture_session("v1_stats");
    let id = loaded_player_ids(&mut session, &data)
        .into_iter()
        .find(|id| {
            session
                .loaded_players
                .get(id)
                .is_some_and(|p| p.dps.is_some())
        })
        .expect("fixture has a player with a _dps.sav");

    pal::add_player_dps_pal(&mut session, &data, id, "Lamball", "Fluffy", None)
        .expect("add dps pal");

    let files = session
        .player_sav_bytes()
        .expect("_dps.sav must serialize after adding a pal");
    let (_, dps) = files.get(&id).expect("player is loaded");
    let dps = dps.as_ref().expect("player has a _dps.sav");
    psp_core::savio::read_sav_bytes(dps).expect("_dps.sav must parse back after adding a pal");
}
