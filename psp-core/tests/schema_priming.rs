//! A property at its default value is absent from a Palworld save, and `uesave`
//! schemas only what it read -- so anything this app introduces must be primed or
//! the write fails with `missing property schema for path: ...`.
//!
//! These pin the save shapes that hit that in the wild: no pal at all, no insane
//! pal, and an empty `_dps.sav`.

mod common;

use psp_core::domain::{pal, player};
use psp_core::dto::ordered_map::OrderedMap;
use psp_core::gamedata::GameData;
use psp_core::progress::null_progress;

fn game_data() -> GameData {
    let json_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/json");
    GameData::load(&json_dir).expect("data dir")
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

    session
        .level_sav_bytes()
        .expect("level.sav must serialize after a player edit");
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

    session
        .level_sav_bytes()
        .expect("level.sav must serialize after healing");
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

    session
        .level_sav_bytes()
        .expect("level.sav must serialize after adding the first pal");
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

    session
        .player_sav_bytes()
        .expect("_dps.sav must serialize after adding a pal");
}
