//! Relic type maps, rank calculation, and `relic_data.json` integrity.

mod common;

use psp_core::domain::player::{self, STATUS_NAME_MAP};
use psp_core::domain::relic::{self, RELIC_TYPE_MAP, RELIC_TYPE_TO_STATUS_NAME};
use psp_core::gamedata::GameData;
use psp_core::progress::null_progress;
use uuid::Uuid;

fn game_data() -> GameData {
    let json_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/json");
    GameData::load(&json_dir).expect("data dir")
}

#[test]
fn relic_type_map_has_13_entries() {
    assert_eq!(RELIC_TYPE_MAP.len(), 13);
    assert_eq!(RELIC_TYPE_TO_STATUS_NAME.len(), 13);
    for (_, key) in RELIC_TYPE_MAP {
        assert!(
            RELIC_TYPE_TO_STATUS_NAME.iter().any(|(k, _)| *k == key),
            "{key} has no status-name mapping"
        );
    }
}

/// Every relic type must grant a stat the status map knows about, or the relic
/// would be unrepresentable in the player DTO.
#[test]
fn every_relic_status_name_is_a_known_status_name() {
    for (relic_key, japanese) in RELIC_TYPE_TO_STATUS_NAME {
        assert!(
            STATUS_NAME_MAP.iter().any(|(jp, _)| *jp == japanese),
            "relic {relic_key} maps to {japanese}, which STATUS_NAME_MAP does not know"
        );
    }
}

/// Data-integrity guard on relic_data.json: per_rank must have exactly max_rank
/// steps and sum to cumulative_max, and effect_rate must have max_rank entries.
#[test]
fn relic_data_json_is_internally_consistent() {
    let data = game_data();
    let relics = data.get("relic_data").expect("relic_data.json present");
    let map = relics.as_object().expect("relic_data.json is an object");

    assert_eq!(map.len(), 13);

    for (key, entry) in map {
        let max_rank = entry["max_rank"].as_i64().expect("max_rank");
        let cumulative_max = entry["cumulative_max"].as_i64().expect("cumulative_max");
        let per_rank: Vec<i64> = entry["per_rank"]
            .as_array()
            .expect("per_rank")
            .iter()
            .map(|v| v.as_i64().expect("per_rank entry"))
            .collect();
        let effect_rate = entry["effect_rate"].as_array().expect("effect_rate");

        assert_eq!(
            per_rank.len() as i64,
            max_rank,
            "{key}: per_rank has {} steps but max_rank is {max_rank}",
            per_rank.len()
        );
        assert_eq!(
            per_rank.iter().sum::<i64>(),
            cumulative_max,
            "{key}: per_rank sums to {} but cumulative_max is {cumulative_max}",
            per_rank.iter().sum::<i64>()
        );
        assert_eq!(
            effect_rate.len() as i64,
            max_rank,
            "{key}: effect_rate has {} entries but max_rank is {max_rank}",
            effect_rate.len()
        );
        assert!(
            RELIC_TYPE_MAP.iter().any(|(_, k)| *k == key.as_str()),
            "relic_data.json has key {key} with no RELIC_TYPE_MAP entry"
        );
    }
}

/// capture_power's per_rank is [1,2,3,4,5,6,7,9,9,...]; cumulative thresholds are
/// 1,3,6,10,15,21,28,37,46,55,64...
#[test]
fn rank_for_count_walks_cumulative_thresholds() {
    let data = game_data();
    assert_eq!(relic::rank_for_count(&data, "capture_power", 0), 0);
    assert_eq!(relic::rank_for_count(&data, "capture_power", 1), 1);
    assert_eq!(relic::rank_for_count(&data, "capture_power", 2), 1);
    assert_eq!(relic::rank_for_count(&data, "capture_power", 3), 2);
    assert_eq!(relic::rank_for_count(&data, "capture_power", 5), 2);
    assert_eq!(relic::rank_for_count(&data, "capture_power", 6), 3);
    assert_eq!(relic::rank_for_count(&data, "capture_power", 10), 4);
    // The real-save case: player E1530496 collected 58 and is rank 10 in-game.
    assert_eq!(relic::rank_for_count(&data, "capture_power", 58), 10);
    // Saturates at max_rank.
    assert_eq!(relic::rank_for_count(&data, "capture_power", 10_000), 15);
}

#[test]
fn rank_for_count_is_zero_for_unknown_relic() {
    let data = game_data();
    assert_eq!(relic::rank_for_count(&data, "not_a_relic", 99), 0);
}

#[test]
fn max_rank_and_effect_for_rank() {
    let data = game_data();
    assert_eq!(relic::max_rank(&data, "swim_speed"), Some(20));
    assert_eq!(relic::max_rank(&data, "sphere_homing"), Some(4));
    assert_eq!(relic::max_rank(&data, "move_speed"), Some(92));
    assert_eq!(relic::max_rank(&data, "capture_power"), Some(15));
    assert_eq!(relic::max_rank(&data, "not_a_relic"), None);

    // swim_speed is 5% per rank.
    assert_eq!(relic::effect_for_rank(&data, "swim_speed", 1), Some(5.0));
    assert_eq!(relic::effect_for_rank(&data, "swim_speed", 11), Some(55.0));
    assert_eq!(relic::effect_for_rank(&data, "swim_speed", 20), Some(100.0));
    // Rank 0 means "no ranks bought": no effect.
    assert_eq!(relic::effect_for_rank(&data, "swim_speed", 0), None);
    // Out of range.
    assert_eq!(relic::effect_for_rank(&data, "swim_speed", 21), None);
    // capture_power grants no percentage at any rank.
    assert_eq!(
        relic::effect_for_rank(&data, "capture_power", 10),
        Some(0.0)
    );
}

/// A real save must never carry a StatusPoint above the stat's max rank. This pins
/// the clamp the UI enforces, against actual game output.
#[test]
fn no_fixture_player_exceeds_max_rank() {
    let data = game_data();
    let mut ranks_checked = 0;
    for fixture in ["v1_relics", "v1_stats"] {
        let mut session = common::load_fixture_session(fixture);
        let ids: Vec<Uuid> = session.player_file_refs.keys().copied().collect();
        for id in ids {
            player::get_player_details(&mut session, &data, id, &null_progress())
                .unwrap()
                .unwrap();
            let Some(dto) = player::build_player_dto(&session, &data, id).unwrap() else {
                continue;
            };
            for (stat, points) in dto.status_point_list.iter() {
                let Some(max) = relic::max_rank(&data, relic_key_for_stat(stat)) else {
                    continue; // not a relic-backed stat (max_hp, attack, ...)
                };
                ranks_checked += 1;
                assert!(
                    *points <= max,
                    "{fixture}/{id}: {stat} rank {points} exceeds max_rank {max}"
                );
            }
        }
    }
    // Without this, the test passes vacuously if the fixtures ever stop carrying
    // relic-backed stats -- and it is the only thing validating our max ranks
    // against real game output.
    assert!(
        ranks_checked > 0,
        "checked no relic-backed ranks at all: the fixtures no longer exercise this"
    );
}

/// The DTO's english stat key equals the relic key for every relic-backed stat
/// except capture_rate, whose relic is called capture_power.
fn relic_key_for_stat(stat: &str) -> &str {
    match stat {
        "capture_rate" => "capture_power",
        other => other,
    }
}

/// Every language must carry every relic key. Guards a game patch that renumbers or
/// renames the BUILDUP_PLAYER_STATUS rows: the l10n files are joined to relic keys BY
/// INDEX, so a renumbering would silently mislabel every stat rather than fail loudly.
#[test]
fn relic_l10n_covers_every_language_and_key() {
    // The directories `relics.json` actually lives in -- the same ones every other l10n
    // file uses. Indonesian is `id-id`, NOT `id`: a stale `id/` directory also exists and
    // nothing reads it, so writing there would silently leave Indonesian users with raw keys.
    //
    // Note these are the ON-DISK directory names. Four of them (`es-MX`, `pt-BR`, `zh-Hans`,
    // `zh-Hant`) do not match the lowercase locale codes the app sends (`es-mx`, ...), and
    // `GameData`'s lookup is exact-case -- so those languages currently resolve to nothing for
    // EVERY l10n table, not just this one. That is a pre-existing, repo-wide bug; this test
    // asserts the files exist where the rest of the l10n lives, and does not paper over it.
    const LANGS: [&str; 16] = [
        "de", "en", "es", "es-MX", "fr", "id-id", "it", "ko", "pl", "pt-BR", "ru", "th", "tr",
        "vi", "zh-Hans", "zh-Hant",
    ];
    let data = game_data();
    for lang in LANGS {
        let table = data
            .get(&format!("l10n/{lang}/relics"))
            .unwrap_or_else(|| panic!("missing data/json/l10n/{lang}/relics.json"));
        let map = table
            .as_object()
            .unwrap_or_else(|| panic!("l10n/{lang}/relics is not an object"));
        assert_eq!(
            map.len(),
            13,
            "{lang}: expected 13 relics, got {}",
            map.len()
        );

        for (_, key) in RELIC_TYPE_MAP {
            let entry = map
                .get(key)
                .unwrap_or_else(|| panic!("{lang}: missing relic key {key}"));
            let name = entry["localized_name"].as_str().unwrap_or("");
            assert!(!name.is_empty(), "{lang}/{key}: empty localized_name");
        }
    }
}

/// The names really do differ from our internal keys -- if this ever passes trivially,
/// the l10n merge has silently fallen back to echoing the key.
#[test]
fn relic_l10n_names_are_not_just_the_keys() {
    let data = game_data();
    let en = data.get("l10n/en/relics").expect("en relics");
    assert_eq!(en["hunger_reduction"]["localized_name"], "Satiety Duration");
    assert_eq!(en["stamina_reduction"]["localized_name"], "Endurance");
    assert_eq!(en["sphere_homing"]["localized_name"], "Sphere Tracking");
}

/// Every relic type needs an icon, or its row renders the generic unknown.webp.
#[test]
fn every_relic_has_an_icon_asset() {
    let img_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui/src/lib/assets/img");
    let mut missing = Vec::new();
    for (_, key) in RELIC_TYPE_MAP {
        if !img_dir.join(format!("relic_{key}.webp")).exists() {
            missing.push(key);
        }
    }
    assert!(missing.is_empty(), "relic icons missing: {missing:?}");
}
