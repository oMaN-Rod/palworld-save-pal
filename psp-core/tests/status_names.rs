//! The 18 Palworld 1.0 status-point names, verified against real 1.0 saves.

mod common;

use psp_core::domain::player::{self, STATUS_NAME_MAP};
use psp_core::gamedata::GameData;
use psp_core::progress::null_progress;
use uuid::Uuid;

fn game_data() -> GameData {
    let json_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/json");
    GameData::load(&json_dir).expect("data dir")
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
