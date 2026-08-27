//! The Effigies editor's data path: the per-type `RelicPossessNumMap` view on
//! `PlayerDto`, its absolute-count write-back with clamping, and its reconcile
//! rule against the flag-delta counter writes.

mod common;

use psp_core::domain::{player, relic};
use psp_core::dto::ordered_map::OrderedMap;
use psp_core::gamedata::GameData;
use psp_core::progress::null_progress;
use std::collections::BTreeMap;
use uuid::Uuid;

/// `v1_relics` "zBlasters": possess map holds CapturePower alone.
const V1_CAPTURE_POWER_ONLY: &str = "62b176f8-0000-0000-0000-000000000000";
/// `v1_relics` "espat": 12 relic types, most at 0 unspent, ranks bought in
/// nearly all -- the real save proving a 0-valued key is a normal state.
const V1_MANY_RELIC_RANKS: &str = "e1530496-0000-0000-0000-000000000000";

fn game_data() -> GameData {
    let json_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/json");
    GameData::load(&json_dir).expect("data dir")
}

/// The raw save's possess map, as bare keys -- the same projection the DTO
/// promises, derived here through the independent serialized-bytes view.
fn raw_possess_map_bare(sav: &serde_json::Value) -> BTreeMap<String, i64> {
    common::relic_possess_num_map(sav)
        .into_iter()
        .map(|(enum_name, count)| {
            let bare = relic::RELIC_TYPE_MAP
                .iter()
                .find(|(name, _)| *name == enum_name)
                .map(|(_, key)| key.to_string())
                .unwrap_or(enum_name);
            (bare, count)
        })
        .collect()
}

/// The DTO's possess map is exactly the raw save's, for every fixture player,
/// and `None` exactly when the save carries no map at all.
#[test]
fn dto_possess_map_matches_raw_save_for_every_player() {
    let data = game_data();
    let mut session = common::load_fixture_session("v1_relics");
    let ids: Vec<Uuid> = session.player_file_refs.keys().copied().collect();
    let mut checked = 0;
    for id in ids {
        let Some(dto) =
            player::get_player_details(&mut session, &data, id, &null_progress()).unwrap()
        else {
            continue;
        };
        let sav = common::player_sav_json(&session, id);
        if !common::relic_possess_map_present(&sav) {
            assert_eq!(
                dto.relic_possess_num_map, None,
                "{id}: save carries no possess map; the DTO must read None, not empty"
            );
        } else {
            assert_eq!(
                dto.relic_possess_num_map,
                Some(raw_possess_map_bare(&sav)),
                "{id}: DTO possess map must equal the raw save's, as bare keys"
            );
        }
        checked += 1;
    }
    assert!(checked > 0, "no fixture player was loaded");
}

/// The documented format invariant (`apply_relic_counters`): the legacy scalar
/// mirrors CapturePower only. Checked on real saves, not synthetic data.
#[test]
fn scalar_mirrors_capture_power_on_real_saves() {
    let data = game_data();
    let mut session = common::load_fixture_session("v1_relics");
    let ids: Vec<Uuid> = session.player_file_refs.keys().copied().collect();
    let mut checked = 0;
    for id in ids {
        let Some(dto) =
            player::get_player_details(&mut session, &data, id, &null_progress()).unwrap()
        else {
            continue;
        };
        if let Some(map) = &dto.relic_possess_num_map {
            assert_eq!(
                dto.effigy_possess_num,
                map.get("capture_power").copied().unwrap_or(0),
                "{id}: RelicPossessNum must mirror RelicPossessNumMap[CapturePower]"
            );
            checked += 1;
        }
    }
    assert!(checked > 0, "no fixture player carries a possess map");
}

fn loaded_v1_player(
    session: &mut psp_core::session::SaveSession,
    data: &GameData,
    uid: &str,
) -> psp_core::dto::player::PlayerDto {
    let id: Uuid = uid.parse().unwrap();
    player::get_player_details(session, data, id, &null_progress())
        .unwrap()
        .unwrap_or_else(|| panic!("fixture player {uid} must load"))
}

fn commit(
    session: &mut psp_core::session::SaveSession,
    data: &GameData,
    dto: psp_core::dto::player::PlayerDto,
) {
    let mut modified: OrderedMap<Uuid, _> = OrderedMap::new();
    modified.insert(dto.uid, dto);
    player::update_players(session, data, &modified, &null_progress()).unwrap();
}

/// An explicit count edit lands absolutely: the map entry, the CapturePower
/// scalar mirror, and nothing else.
#[test]
fn edited_counts_round_trip() {
    let data = game_data();
    let mut session = common::load_fixture_session("v1_relics");
    let mut dto = loaded_v1_player(&mut session, &data, V1_MANY_RELIC_RANKS);
    let before = dto.relic_possess_num_map.clone().unwrap();

    let mut edited = dto.relic_possess_num_map.clone().unwrap();
    edited.insert("swim_speed".to_string(), 12);
    edited.insert("capture_power".to_string(), 42);
    dto.relic_possess_num_map = Some(edited);
    commit(&mut session, &data, dto);

    let sav = common::player_sav_json(&session, V1_MANY_RELIC_RANKS.parse().unwrap());
    let on_disk = common::relic_possess_num_map(&sav);
    assert_eq!(on_disk.get("EPalRelicType::SwimSpeed"), Some(&12));
    assert_eq!(on_disk.get("EPalRelicType::CapturePower"), Some(&42));
    // The scalar mirrors CapturePower, never the cross-type total.
    assert_eq!(common::relic_possess_num(&sav), 42);
    // Every type not edited keeps exactly what it held.
    for (key, value) in &before {
        if key != "swim_speed" && key != "capture_power" {
            let enum_name = relic::RELIC_TYPE_MAP
                .iter()
                .find(|(_, k)| k == key)
                .map(|(name, _)| *name)
                .unwrap();
            assert_eq!(
                on_disk.get(enum_name),
                Some(value),
                "{key} must be untouched"
            );
        }
    }
}

/// The illegal-value guard: over-cap counts clamp to each type's
/// `cumulative_max` on the way into the save.
#[test]
fn over_cap_counts_clamp_to_cumulative_max() {
    let data = game_data();
    let mut session = common::load_fixture_session("v1_relics");
    let mut dto = loaded_v1_player(&mut session, &data, V1_CAPTURE_POWER_ONLY);
    let mut edited = dto.relic_possess_num_map.clone().unwrap_or_default();
    edited.insert("capture_power".to_string(), 9_999); // cap 100
    edited.insert("move_speed".to_string(), 9_999); // cap 287
    edited.insert("sphere_homing".to_string(), -7); // negative clamps to 0
    dto.relic_possess_num_map = Some(edited);
    commit(&mut session, &data, dto);

    let sav = common::player_sav_json(&session, V1_CAPTURE_POWER_ONLY.parse().unwrap());
    let on_disk = common::relic_possess_num_map(&sav);
    assert_eq!(on_disk.get("EPalRelicType::CapturePower"), Some(&100));
    assert_eq!(on_disk.get("EPalRelicType::MoveSpeed"), Some(&287));
    assert_eq!(on_disk.get("EPalRelicType::SphereHoming"), Some(&0));
    assert_eq!(common::relic_possess_num(&sav), 100);
}

/// The reconcile rule's first half: a DTO map that merely echoes the loaded
/// values must NOT undo the flag-delta counter moves -- the pre-Effigies
/// behavior a flags-only client still relies on.
#[test]
fn unedited_echo_leaves_flag_delta_moves_alone() {
    let data = game_data();
    let mut session = common::load_fixture_session("v1_relics");
    let mut dto = loaded_v1_player(&mut session, &data, V1_CAPTURE_POWER_ONLY);
    let before_scalar = dto.effigy_possess_num;
    let before_map = dto.relic_possess_num_map.clone().unwrap();

    // Collect two more effigies WITHOUT touching the possess map -- the exact
    // payload shape every pre-Effigies client sent.
    let mut effigies = dto.collected_effigies.clone().unwrap_or_default();
    effigies.push("EF_ECHO_1".to_string());
    effigies.push("EF_ECHO_2".to_string());
    dto.collected_effigies = Some(effigies);
    dto.relic_possess_num_map = Some(before_map.clone()); // untouched echo
    commit(&mut session, &data, dto);

    let reread = loaded_v1_player(&mut session, &data, V1_CAPTURE_POWER_ONLY);
    assert_eq!(
        reread.effigy_possess_num,
        before_scalar + 2,
        "collecting 2 new effigies must still grant exactly 2 relics"
    );
    assert_eq!(
        reread
            .relic_possess_num_map
            .as_ref()
            .unwrap()
            .get("capture_power"),
        Some(&(before_map.get("capture_power").copied().unwrap_or(0) + 2)),
        "the map entry moves with the scalar"
    );
}

/// The reconcile rule's second half: an explicit edit wins even when flag
/// deltas pulled the other way in the same save.
#[test]
fn explicit_edit_wins_over_flag_delta() {
    let data = game_data();
    let mut session = common::load_fixture_session("v1_relics");
    let mut dto = loaded_v1_player(&mut session, &data, V1_CAPTURE_POWER_ONLY);
    let before = dto
        .relic_possess_num_map
        .clone()
        .unwrap()
        .get("capture_power")
        .copied()
        .unwrap_or(0);

    // Collect one more effigy AND explicitly set the count lower: the Effigies
    // editor's absolute value is the intent, the flag delta is not.
    let mut effigies = dto.collected_effigies.clone().unwrap_or_default();
    effigies.push("EF_EXPLICIT_1".to_string());
    dto.collected_effigies = Some(effigies);
    let mut edited = dto.relic_possess_num_map.clone().unwrap();
    edited.insert("capture_power".to_string(), before.saturating_sub(1).max(0));
    dto.relic_possess_num_map = Some(edited);
    commit(&mut session, &data, dto);

    let reread = loaded_v1_player(&mut session, &data, V1_CAPTURE_POWER_ONLY);
    let expected = before.saturating_sub(1).max(0);
    assert_eq!(
        reread.effigy_possess_num, expected,
        "the explicit edit wins"
    );
    assert_eq!(
        reread
            .relic_possess_num_map
            .as_ref()
            .unwrap()
            .get("capture_power"),
        Some(&expected)
    );
}

/// The Effigies Apply flow: counts land in the possess map and ranks derived
/// from those counts land in `GotStatusPointList` -- including appending a row
/// for a stat the save had no row for, and NOT inventing one at rank 0.
#[test]
fn apply_flow_syncs_ranks_into_status_point_list() {
    let data = game_data();
    let mut session = common::load_fixture_session("v1_relics");
    let mut dto = loaded_v1_player(&mut session, &data, V1_MANY_RELIC_RANKS);

    let mut edited = dto.relic_possess_num_map.clone().unwrap_or_default();
    edited.insert("swim_speed".to_string(), 12); // rank 12 of 20
    edited.insert("exp_bonus".to_string(), 3); // rank 3 of 4
    edited.insert("move_speed".to_string(), 0); // rank 0 -> no row
    dto.relic_possess_num_map = Some(edited);
    // The frontend's Apply derives ranks from the staged counts.
    dto.status_point_list.insert(
        "swim_speed".to_string(),
        relic::rank_for_count(&data, "swim_speed", 12),
    );
    dto.status_point_list.insert(
        "exp_bonus".to_string(),
        relic::rank_for_count(&data, "exp_bonus", 3),
    );
    dto.status_point_list.insert("move_speed".to_string(), 0);
    commit(&mut session, &data, dto);

    let reread = loaded_v1_player(&mut session, &data, V1_MANY_RELIC_RANKS);
    assert_eq!(reread.status_point_list.get("swim_speed"), Some(&12));
    assert_eq!(reread.status_point_list.get("exp_bonus"), Some(&3));
    // Rank 0 never appends a row; a pre-existing row reads back as written 0.
    let move_speed = reread
        .status_point_list
        .get("move_speed")
        .copied()
        .unwrap_or(0);
    assert_eq!(move_speed, 0);
    assert_eq!(
        reread
            .relic_possess_num_map
            .as_ref()
            .unwrap()
            .get("swim_speed"),
        Some(&12)
    );
}

/// A pre-1.0 save carries no possess map: it reads as `None`, an all-zero
/// round-trip invents nothing, and only a positive count creates the map.
#[test]
fn pre_1_0_save_reads_none_and_invents_nothing_on_zero_edits() {
    let data = game_data();
    let mut session = common::load_fixture_session("world1");
    let ids: Vec<Uuid> = session.player_file_refs.keys().copied().collect();
    let mut checked = 0;
    for id in ids {
        let Some(dto) =
            player::get_player_details(&mut session, &data, id, &null_progress()).unwrap()
        else {
            continue;
        };
        assert_eq!(
            dto.relic_possess_num_map, None,
            "{id}: a pre-1.0 save has no possess map; it must read None"
        );
        // The frontend round-trips what it loaded -- None -- but a defensive
        // client sending explicit zeros must not conjure the property either.
        let mut round_tripped = dto.clone();
        let mut zeros = BTreeMap::new();
        zeros.insert("capture_power".to_string(), 0);
        round_tripped.relic_possess_num_map = Some(zeros);
        commit(&mut session, &data, round_tripped);

        let sav = common::player_sav_json(&session, id);
        assert!(
            common::relic_possess_num_map(&sav).is_empty(),
            "{id}: zero counts must not create a possess map"
        );
        checked += 1;
    }
    assert!(checked > 0, "no fixture player was loaded from world1");
}

/// A positive count on a save that never carried the map creates it -- the
/// PalSavTools behavior of schema-gated creation, which PSP matches for the
/// possess map specifically.
#[test]
fn positive_count_creates_the_map_on_a_save_without_one() {
    let data = game_data();
    let mut session = common::load_fixture_session("world1");
    let ids: Vec<Uuid> = session.player_file_refs.keys().copied().collect();
    let id = ids[0];
    let mut dto = player::get_player_details(&mut session, &data, id, &null_progress())
        .unwrap()
        .unwrap();
    assert_eq!(
        dto.relic_possess_num_map, None,
        "fixture sanity: no map yet"
    );

    let mut counts = BTreeMap::new();
    counts.insert("capture_power".to_string(), 5);
    dto.relic_possess_num_map = Some(counts);
    commit(&mut session, &data, dto);

    let sav = common::player_sav_json(&session, id);
    assert_eq!(
        common::relic_possess_num_map(&sav).get("EPalRelicType::CapturePower"),
        Some(&5)
    );
    assert_eq!(common::relic_possess_num(&sav), 5);
}

/// Unknown DTO keys are dropped, and a type the save carries that the game
/// data does not know survives an Effigies edit untouched.
#[test]
fn unknown_keys_drop_and_unknown_save_entries_survive() {
    let data = game_data();
    let mut session = common::load_fixture_session("v1_relics");
    let mut dto = loaded_v1_player(&mut session, &data, V1_MANY_RELIC_RANKS);
    let before = dto.relic_possess_num_map.clone().unwrap();

    let mut edited = before.clone();
    edited.insert("bogus_future_type".to_string(), 77);
    edited.insert("swim_speed".to_string(), 9);
    dto.relic_possess_num_map = Some(edited);
    commit(&mut session, &data, dto);

    let sav = common::player_sav_json(&session, V1_MANY_RELIC_RANKS.parse().unwrap());
    let on_disk = common::relic_possess_num_map(&sav);
    assert!(
        !on_disk.keys().any(|key| key.contains("bogus")),
        "an unknown DTO key must be dropped, not written"
    );
    assert_eq!(on_disk.get("EPalRelicType::SwimSpeed"), Some(&9));
}

/// An unchanged round-trip is a strict no-op for the possess map: same values,
/// same on-disk entry order.
#[test]
fn unchanged_round_trip_leaves_possess_map_byte_stable() {
    let data = game_data();
    let mut session = common::load_fixture_session("v1_relics");
    let id: Uuid = V1_MANY_RELIC_RANKS.parse().unwrap();
    let dto = loaded_v1_player(&mut session, &data, V1_MANY_RELIC_RANKS);

    let before_ordered =
        common::relic_possess_num_map_ordered(&common::player_sav_json(&session, id));
    commit(&mut session, &data, dto);
    let after_ordered =
        common::relic_possess_num_map_ordered(&common::player_sav_json(&session, id));
    assert_eq!(
        before_ordered, after_ordered,
        "an unedited round-trip must not reorder or rewrite possess-map entries"
    );
}
