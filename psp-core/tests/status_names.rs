//! The 18 Palworld 1.0 status-point names, verified against real 1.0 saves.

mod common;

use psp_core::domain::player::{self, STATUS_NAME_MAP};
use psp_core::dto::ordered_map::OrderedMap;
use psp_core::gamedata::GameData;
use psp_core::progress::null_progress;
use uuid::Uuid;

fn game_data() -> GameData {
    let json_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/json");
    GameData::load(&json_dir).expect("data dir")
}

fn first_player_id(session: &mut psp_core::session::SaveSession) -> Uuid {
    *session
        .player_file_refs
        .keys()
        .next()
        .expect("fixture has players")
}

const NEW_IN_1_0: [&str; 12] = [
    "hunger_reduction",
    "swim_speed",
    "food_decay_reduction",
    "jump_power",
    "glider_speed",
    "climb_speed",
    "status_ailment_resist",
    "exp_bonus",
    "rainbow_passive_rate",
    "move_speed",
    "sphere_homing",
    "stamina_reduction",
];

#[test]
fn status_name_map_has_all_18_entries() {
    assert_eq!(STATUS_NAME_MAP.len(), 18);
    for english in NEW_IN_1_0 {
        assert!(
            STATUS_NAME_MAP.iter().any(|(_, en)| *en == english),
            "STATUS_NAME_MAP is missing {english}"
        );
    }
}

/// Collect every english status key the reader produces across all players of a
/// fixture. Proves our Japanese strings match what the game actually writes: a
/// typo would simply never match, and the key would never appear.
fn keys_seen_in(fixture: &str) -> std::collections::BTreeSet<String> {
    let mut session = common::load_fixture_session(fixture);
    let data = game_data();
    let ids: Vec<Uuid> = session.player_file_refs.keys().copied().collect();
    let mut seen = std::collections::BTreeSet::new();
    for id in ids {
        player::get_player_details(&mut session, &data, id, &null_progress())
            .unwrap()
            .unwrap();
        if let Some(dto) = player::build_player_dto(&session, &data, id).unwrap() {
            // `OrderedMap` exposes `iter()`, not `keys()`.
            for (key, _) in dto.status_point_list.iter() {
                seen.insert(key.clone());
            }
        }
    }
    seen
}

#[test]
fn real_1_0_saves_exercise_all_18_status_names() {
    let mut seen = keys_seen_in("v1_relics");
    seen.extend(keys_seen_in("v1_stats"));

    for (_, english) in STATUS_NAME_MAP {
        assert!(
            seen.contains(english),
            "no fixture player exposes {english}; either the Japanese string is wrong \
             or the fixtures no longer cover it. seen = {seen:?}"
        );
    }
}

/// The game creates a `{StatusName, StatusPoint}` row lazily -- it exists only once a
/// rank has been bought, so an absent row means rank 0. Setting a stat the save has no
/// row for must CREATE the row, or the edit would silently do nothing.
#[test]
fn setting_a_stat_with_no_row_appends_one() {
    let mut session = common::load_fixture_session("v1_relics");
    let data = game_data();
    let player_id = first_player_id(&mut session);
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();
    let mut dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();

    // Find a relic stat this player has NO row for. Every fixture player is missing at
    // least one; if not, the fixture changed and this test is no longer meaningful.
    let missing = STATUS_NAME_MAP
        .iter()
        .map(|(_, english)| *english)
        .find(|english| dto.status_point_list.get(*english).is_none())
        .expect("fixture player should be missing at least one status row");

    dto.status_point_list.insert(missing.to_string(), 7);
    let mut modified = OrderedMap::new();
    modified.insert(player_id, dto);
    player::update_players(&mut session, &data, &modified, &null_progress()).unwrap();

    let reread = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        reread.status_point_list.get(missing).copied(),
        Some(7),
        "a stat with no row must be appended, not silently dropped"
    );
}

/// The DTO is untrusted input off the websocket. A negative rank is a value the game
/// never writes, so it must not conjure a row either. Without this, a mutation of the
/// append guard from `> 0` to `!= 0` would append `StatusPoint = -5` and every other
/// test would still pass.
#[test]
fn setting_a_missing_stat_to_a_negative_appends_nothing() {
    let mut session = common::load_fixture_session("v1_relics");
    let data = game_data();
    let player_id = first_player_id(&mut session);
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();
    let mut dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();

    let missing = STATUS_NAME_MAP
        .iter()
        .map(|(_, english)| *english)
        .find(|english| dto.status_point_list.get(*english).is_none())
        .expect("fixture player should be missing at least one status row");

    dto.status_point_list.insert(missing.to_string(), -5);
    let mut modified = OrderedMap::new();
    modified.insert(player_id, dto);
    player::update_players(&mut session, &data, &modified, &null_progress()).unwrap();

    let reread = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    assert!(
        reread.status_point_list.get(missing).is_none(),
        "a negative rank must not create a row"
    );
}

/// A rank-0 stat has no row in a real save. The UI sends all 13 relic keys, so an
/// unedited save carries 0 for every stat the player never unlocked -- appending those
/// would bloat the save with rows the game never wrote.
#[test]
fn setting_a_missing_stat_to_zero_appends_nothing() {
    let mut session = common::load_fixture_session("v1_relics");
    let data = game_data();
    let player_id = first_player_id(&mut session);
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();
    let mut dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();

    let missing = STATUS_NAME_MAP
        .iter()
        .map(|(_, english)| *english)
        .find(|english| dto.status_point_list.get(*english).is_none())
        .expect("fixture player should be missing at least one status row");

    dto.status_point_list.insert(missing.to_string(), 0);
    let mut modified = OrderedMap::new();
    modified.insert(player_id, dto);
    player::update_players(&mut session, &data, &modified, &null_progress()).unwrap();

    let reread = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    assert!(
        reread.status_point_list.get(missing).is_none(),
        "a rank-0 stat must not create a row: the game has none, and appending one on \
         every save would bloat the file"
    );
}

/// Writes the fixture player back through the real writer, first letting `edit`
/// mutate the DTO, and returns the resulting `Level.sav` bytes -- status-point
/// rows live in the character map inside `Level.sav`, never in the player
/// `.sav`, so that is the only file a status-point-row assertion can trust.
fn resave_level_sav(edit: impl FnOnce(&mut psp_core::dto::player::PlayerDto)) -> Vec<u8> {
    let mut session = common::load_fixture_session("v1_relics");
    let data = game_data();
    let player_id = first_player_id(&mut session);
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();
    let mut dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    edit(&mut dto);
    let mut modified = OrderedMap::new();
    modified.insert(player_id, dto);
    player::update_players(&mut session, &data, &modified, &null_progress()).unwrap();
    session.level_sav_bytes().expect("serialize Level.sav")
}

/// Pins the anti-bloat rule end to end. The UI sends every relic key on save, 0 for the
/// ones the player never unlocked; that must produce the exact same bytes as a save that
/// sent only the keys the file already had rows for -- not one byte of new row data.
///
/// Compared against another write rather than the on-disk bytes on purpose: some fields
/// `update_players` touches are not byte-identical to the source file for unrelated
/// reasons. Diffing two writes isolates the zeros, which is what this pins.
#[test]
fn resaving_with_all_relic_keys_at_zero_is_byte_identical() {
    // Anti-vacuity guard: if the fixture player already had a row for every status
    // stat, the edit closures below would insert nothing and both writes would match
    // trivially, proving nothing about the anti-bloat rule.
    let missing_count = {
        let mut session = common::load_fixture_session("v1_relics");
        let data = game_data();
        let player_id = first_player_id(&mut session);
        player::get_player_details(&mut session, &data, player_id, &null_progress())
            .unwrap()
            .unwrap();
        let dto = player::build_player_dto(&session, &data, player_id)
            .unwrap()
            .unwrap();
        STATUS_NAME_MAP
            .iter()
            .filter(|(_, english)| dto.status_point_list.get(*english).is_none())
            .count()
    };
    assert!(
        missing_count > 0,
        "fixture player has a status-point row for every stat; this test needs at \
         least one missing row to zero-fill or it passes without exercising anything"
    );

    let untouched = resave_level_sav(|_| {});
    let all_keys_zeroed = resave_level_sav(|dto| {
        for (_, english) in STATUS_NAME_MAP {
            if dto.status_point_list.get(english).is_none() {
                dto.status_point_list.insert(english.to_string(), 0);
            }
        }
    });
    assert_eq!(
        untouched, all_keys_zeroed,
        "sending every relic key at 0 must not change a single byte"
    );
}

/// `setting_a_stat_with_no_row_appends_one` only proves the row exists in the
/// in-memory DTO -- `build_player_dto` reads back in-memory state, never bytes.
/// This proves an appended row actually reaches the serialized `Level.sav`: a
/// write with one status stat freshly set to a positive value must differ from
/// an untouched write, byte for byte.
#[test]
fn appended_status_row_survives_level_sav_serialization() {
    let untouched = resave_level_sav(|_| {});
    let with_new_row = resave_level_sav(|dto| {
        let missing = STATUS_NAME_MAP
            .iter()
            .map(|(_, english)| *english)
            .find(|english| dto.status_point_list.get(*english).is_none())
            .expect("fixture player should be missing at least one status row");
        dto.status_point_list.insert(missing.to_string(), 7);
    });
    assert_ne!(
        untouched, with_new_row,
        "an appended status-point row must change the serialized Level.sav bytes"
    );
}
