//! `defeated_bosses`: `NormalBossDefeatFlag` + `TowerBossDefeatFlag` merged from
//! `RecordData`, read-only (see `psp-core/src/domain/player.rs` `unlock_flag_keys`).

mod common;

use psp_core::domain::player;
use psp_core::gamedata::GameData;
use psp_core::progress::null_progress;
use uuid::Uuid;

/// v1_relics "espat": carries both boss-defeat flag maps with `true` entries.
const V1_PLAYER_MANY_DEFEATS: &str = "e1530496-0000-0000-0000-000000000000";

fn game_data() -> GameData {
    let json_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/json");
    GameData::load(&json_dir).expect("data dir")
}

#[test]
fn defeated_bosses_reads_normal_and_tower_flags_from_a_real_save() {
    let mut session = common::load_fixture_session("v1_relics");
    let data = game_data();
    let player_id: Uuid = V1_PLAYER_MANY_DEFEATS.parse().unwrap();
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();

    let dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();

    let defeated = dto
        .defeated_bosses
        .expect("fixture sanity: this player must carry defeated bosses");
    assert!(
        defeated.contains(&"BOSS_Hunter_Rifle".to_string()),
        "must include a true NormalBossDefeatFlag key: {defeated:?}"
    );
    assert!(
        defeated.contains(&"BOSS_BATTLE_NAME_GrassBoss".to_string()),
        "must include a true TowerBossDefeatFlag key: {defeated:?}"
    );
    assert_eq!(
        defeated.len(),
        59 + 5,
        "fixture sanity: 59 normal + 5 tower true flags on this player"
    );
}

/// world1's players have no boss-defeat flags at all -- a legitimately
/// key-less save. Must read as an empty list, not error or `None`.
#[test]
fn defeated_bosses_is_empty_list_when_record_data_has_no_flags() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let player_id: Uuid = "8c2f1930-0000-0000-0000-000000000000".parse().unwrap();
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();

    let dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    assert_eq!(dto.defeated_bosses, Some(vec![]));
}

/// No write path exists for boss-defeat flags. An unrelated resave (editing
/// `collected_effigies`, which does write) must leave `defeated_bosses`
/// exactly where it started, proving the flags round-trip untouched.
#[test]
fn defeated_bosses_survives_an_unrelated_resave_untouched() {
    use psp_core::dto::ordered_map::OrderedMap;

    let mut session = common::load_fixture_session("v1_relics");
    let data = game_data();
    let player_id: Uuid = V1_PLAYER_MANY_DEFEATS.parse().unwrap();
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();

    let mut dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    let before = dto.defeated_bosses.clone();

    dto.collected_effigies = Some(vec!["SomeNewEffigy".to_string()]);
    let mut modified = OrderedMap::new();
    modified.insert(player_id, dto);
    player::update_players(&mut session, &data, &modified, &null_progress()).unwrap();

    let reread = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        reread.defeated_bosses, before,
        "an unrelated resave must not alter NormalBossDefeatFlag/TowerBossDefeatFlag"
    );
}
