//! Palworld 1.0 relic types. A relic grants ranks in one status stat; the number of
//! relics of a type spent determines its rank, via cumulative thresholds in
//! `data/json/relic_data.json`, and each rank carries a known effect rate.
//!
//! A player's `StatusPoint` for a relic-backed stat IS its rank -- verified against
//! real saves. Nothing here recomputes it; these lookups exist so the UI can clamp
//! edits to `max_rank` and show the resulting bonus.
//!
//! Not to be confused with effigies, which the save format also calls "relics"
//! (`RelicObtainForInstanceFlag`, `RelicPossessNum`) for historical reasons.

use crate::gamedata::GameData;

/// The `EPalRelicType::*` enum value as written in the save, to our key.
pub const RELIC_TYPE_MAP: [(&str, &str); 13] = [
    ("EPalRelicType::CapturePower", "capture_power"),
    ("EPalRelicType::HungerReduction", "hunger_reduction"),
    ("EPalRelicType::SwimSpeed", "swim_speed"),
    ("EPalRelicType::FoodDecayReduction", "food_decay_reduction"),
    ("EPalRelicType::JumpPower", "jump_power"),
    ("EPalRelicType::GliderSpeed", "glider_speed"),
    ("EPalRelicType::ClimbSpeed", "climb_speed"),
    (
        "EPalRelicType::StatusAilmentResist",
        "status_ailment_resist",
    ),
    ("EPalRelicType::StaminaReduction", "stamina_reduction"),
    ("EPalRelicType::SphereHoming", "sphere_homing"),
    ("EPalRelicType::ExpBonus", "exp_bonus"),
    ("EPalRelicType::RainbowPassiveRate", "rainbow_passive_rate"),
    ("EPalRelicType::MoveSpeed", "move_speed"),
];

/// The status stat each relic type grants, as the Japanese `StatusName` the save
/// stores. Every value here is a key of `player::STATUS_NAME_MAP`.
pub const RELIC_TYPE_TO_STATUS_NAME: [(&str, &str); 13] = [
    ("capture_power", "捕獲率"),
    ("hunger_reduction", "空腹率低減"),
    ("swim_speed", "泳ぎ速度"),
    ("food_decay_reduction", "食料腐敗低減"),
    ("jump_power", "ジャンプ力"),
    ("glider_speed", "滑空速度"),
    ("climb_speed", "崖登り速度"),
    ("status_ailment_resist", "状態異常耐性"),
    ("stamina_reduction", "スタミナ消費軽減"),
    ("sphere_homing", "パルスフィアホーミング"),
    ("exp_bonus", "経験値ボーナス"),
    ("rainbow_passive_rate", "虹パッシブ率"),
    ("move_speed", "移動速度アップ"),
];

fn entry<'a>(data: &'a GameData, relic_key: &str) -> Option<&'a serde_json::Value> {
    data.get("relic_data").and_then(|v| v.get(relic_key))
}

/// Rank earned for having spent `count` relics of `relic_key`, by walking the
/// cumulative `per_rank` thresholds. `0` for an unknown key or a count below the
/// first threshold; saturates at `max_rank`.
pub fn rank_for_count(data: &GameData, relic_key: &str, count: i64) -> i64 {
    let Some(per_rank) = entry(data, relic_key)
        .and_then(|e| e.get("per_rank"))
        .and_then(|v| v.as_array())
    else {
        return 0;
    };
    let mut rank = 0;
    let mut cumulative = 0;
    for step in per_rank {
        let Some(step) = step.as_i64() else { break };
        cumulative += step;
        if count >= cumulative {
            rank += 1;
        } else {
            break;
        }
    }
    rank
}

/// The highest rank this stat can reach. `None` for a stat with no relic backing
/// (`max_hp`, `attack`, ...), which is how callers tell the two kinds apart.
pub fn max_rank(data: &GameData, relic_key: &str) -> Option<i64> {
    entry(data, relic_key)?.get("max_rank")?.as_i64()
}

/// The bonus granted at `rank`, e.g. swim_speed rank 11 -> 55.0 (percent). `None`
/// for rank 0 (no ranks bought), an out-of-range rank, or an unknown key.
pub fn effect_for_rank(data: &GameData, relic_key: &str, rank: i64) -> Option<f64> {
    if rank < 1 {
        return None;
    }
    entry(data, relic_key)?
        .get("effect_rate")?
        .as_array()?
        .get((rank - 1) as usize)?
        .as_f64()
}
