//! Source rows → normalized [`Actor`]s.
//!
//! Transforms: world centimeters become map units
//! (÷1000), `yaw` comes from `RotationZ` (not RotationY), a Blueprint class
//! like `...BP_PlantSlime_C` resolves to the bare tribe, unknown unit types
//! are kept (lower-cased) rather than dropped, rows without `InstanceID`
//! get a position-stable id, and `userid`/`ip` are never read, let alone
//! published.
use serde::Deserialize;

use crate::model::{Actor, InGameTime, SourceKind};

/// One `ActorData` row from `/v1/api/game-data` or the local bridge file.
/// Field names/aliases cover both wire spellings (`UnitType`/`unitType`).
#[derive(Debug, Clone, Deserialize)]
pub struct ActorDataRow {
    #[serde(default, alias = "UnitType", alias = "unitType")]
    pub unit_type: Option<String>,
    #[serde(default, alias = "instanceId")]
    pub instance_id: Option<String>,
    #[serde(default, alias = "nickName")]
    pub nick_name: Option<String>,
    #[serde(default, alias = "isActive")]
    pub is_active: Option<bool>,
    #[serde(default, alias = "otomoPal")]
    pub otomo_pal: Option<bool>,
    #[serde(default, alias = "HP")]
    pub hp: Option<i64>,
    #[serde(default, alias = "MaxHP", alias = "maxHP")]
    pub max_hp: Option<i64>,
    #[serde(default, alias = "Level")]
    pub level: Option<i64>,
    #[serde(default, alias = "Stage")]
    pub stage: Option<String>,
    #[serde(default, alias = "Class")]
    pub class: Option<String>,
    #[serde(default, alias = "GuildName", alias = "guildName")]
    pub guild_name: Option<String>,
    #[serde(default, alias = "Name")]
    pub name: Option<String>,
    #[serde(default, alias = "Owner")]
    pub owner: Option<String>,
    #[serde(default, alias = "LocationX")]
    pub location_x: Option<f64>,
    #[serde(default, alias = "LocationY")]
    pub location_y: Option<f64>,
    #[serde(default, alias = "LocationZ")]
    pub location_z: Option<f64>,
    #[serde(default, alias = "RotationZ")]
    pub rotation_z: Option<f64>,
}

/// World centimeters → map units, rounded to 2 decimals the way the map
/// readout prints them.
fn cm_to_units(cm: f64) -> f64 {
    (cm / 1000.0 * 100.0).round() / 100.0
}

/// `Chara_BP_SheepBall_C` → `SheepBall`; anything without a Blueprint body
/// is left alone.
pub fn tribe_from_class(class: &str) -> Option<String> {
    let before = class.strip_suffix("_C")?;
    let idx = before.rfind("BP_")?;
    let tribe = &before[idx + 3..];
    if tribe.is_empty() {
        None
    } else {
        Some(tribe.to_string())
    }
}

/// Maps a wire unit type onto the feed's kind vocabulary: the known kinds
/// stay canonical, anything unknown is kept (lower-cased), never dropped.
pub fn kind_from_unit_type(unit_type: &str) -> String {
    let lowered = unit_type.to_ascii_lowercase();
    match lowered.as_str() {
        "player" => "player".to_string(),
        "otomo" => "otomo".to_string(),
        "wild" => "wild".to_string(),
        "pal" => {
            if lowered == "pal" {
                "pal".to_string()
            } else {
                lowered
            }
        }
        "palbox" | "basecamp" => "palbox".to_string(),
        other => other.to_string(),
    }
}

/// Normalizes one row. Rows with no position and no identity at all are
/// dropped (they cannot be placed or de-duplicated); a row with no position
/// but a name is still kept — e.g. the players-only feed.
pub fn actor_from_row(row: &ActorDataRow, kind_hint: Option<&str>) -> Option<Actor> {
    let unit_type = row.unit_type.as_deref().unwrap_or("");
    let kind = kind_hint
        .map(str::to_string)
        .unwrap_or_else(|| kind_from_unit_type(unit_type));
    let has_position = row.location_x.is_some() && row.location_y.is_some();
    // A row with no position is dropped: the map cannot place it and the
    // feed has no way to keep it stable.
    if !has_position {
        return None;
    }

    let id = row
        .instance_id
        .clone()
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| {
            // Position-stable id in raw centimeters:
            // `palbox@5,6` / `palbox@900000,-40000`.
            format!(
                "{}@{},{}",
                kind,
                row.location_x.unwrap_or_default() as i64,
                row.location_y.unwrap_or_default() as i64
            )
        });

    let (x, y, alt) = (
        cm_to_units(row.location_x.unwrap_or_default()),
        cm_to_units(row.location_y.unwrap_or_default()),
        row.location_z.map(cm_to_units).unwrap_or(0.0),
    );

    let name = row
        .nick_name
        .clone()
        .filter(|name| !name.is_empty())
        .or_else(|| row.name.clone().filter(|name| !name.is_empty()));

    Some(Actor {
        id,
        kind,
        x,
        y,
        alt,
        name,
        level: row.level,
        stage: row.stage.clone().filter(|stage| !stage.is_empty()),
        hp: row.hp,
        max_hp: row.max_hp,
        active: row.is_active,
        cls: row.class.clone().filter(|class| !class.is_empty()),
        yaw: row.rotation_z.map(|yaw| (yaw * 10.0).round() / 10.0),
        owner: row.owner.clone().filter(|owner| !owner.is_empty()),
        tribe: row
            .class
            .as_deref()
            .and_then(tribe_from_class)
            .or_else(|| unit_type_tribe_fallback(unit_type)),
        guild_name: row
            .guild_name
            .clone()
            .filter(|guild| !guild.is_empty()),
    })
}

fn unit_type_tribe_fallback(unit_type: &str) -> Option<String> {
    // `Pal::SheepBall`-style unit types carry the tribe after the separator.
    unit_type
        .split_once("::")
        .map(|(_, tribe)| tribe.to_string())
        .filter(|tribe| !tribe.is_empty())
}

/// Parses a rich game-data body (`{"ActorData": [...]}`) into actors plus an
/// optional in-game clock. Torn bodies are the caller's problem (the REST
/// client already classifies them).
pub fn actors_from_game_data(body: &serde_json::Value) -> (Vec<Actor>, Option<InGameTime>) {
    let empty = Vec::new();
    let rows = body
        .get("ActorData")
        .and_then(|value| value.as_array())
        .unwrap_or(&empty);
    let actors: Vec<Actor> = rows
        .iter()
        .filter_map(|row| {
            serde_json::from_value::<ActorDataRow>(row.clone())
                .ok()
                .and_then(|row| actor_from_row(&row, None))
        })
        .collect();
    let ingame = body.get("worldDateTime").and_then(|value| {
        value
            .as_str()
            .map(|time| InGameTime {
                time: Some(time.to_string()),
                days: None,
            })
    });
    (actors, ingame)
}

/// One entry of the players-only REST feed.
#[derive(Debug, Clone, Deserialize)]
pub struct PlayersFeedEntry {
    #[serde(default, alias = "accountInfo")]
    pub account_info: Option<AccountInfo>,
    #[serde(default, alias = "playerInfo")]
    pub player_info: Option<PlayerInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AccountInfo {
    #[serde(default, alias = "accountId")]
    pub account_id: Option<String>,
    #[serde(default, alias = "Name")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlayerInfo {
    #[serde(default)]
    pub level: Option<i64>,
    #[serde(default, alias = "HP")]
    pub hp: Option<i64>,
    #[serde(default, alias = "maxHP")]
    pub max_hp: Option<i64>,
}

/// Players-feed entries → actors. This feed never carries positions, so the
/// map guesses placements and the frame reports `unit: "unknown"`.
pub fn actors_from_players_feed(body: &serde_json::Value) -> Vec<Actor> {
    let empty = Vec::new();
    let entries = body
        .get("players")
        .and_then(|value| value.as_array())
        .unwrap_or(&empty);
    entries
        .iter()
        .filter_map(|entry| {
            let entry: PlayersFeedEntry = serde_json::from_value(entry.clone()).ok()?;
            let account = entry.account_info.as_ref()?;
            let name = account.name.clone().filter(|name| !name.is_empty())?;
            let id = account
                .account_id
                .clone()
                .filter(|id| !id.is_empty())
                .unwrap_or_else(|| name.clone());
            let info = entry.player_info.clone().unwrap_or_default();
            Some(Actor {
                id,
                kind: "player".to_string(),
                x: 0.0,
                y: 0.0,
                alt: 0.0,
                name: Some(name),
                level: info.level,
                stage: None,
                hp: info.hp,
                max_hp: info.max_hp,
                active: None,
                cls: None,
                yaw: None,
                owner: None,
                tribe: None,
                guild_name: None,
            })
        })
        .collect()
}

/// The unit vocabulary for a frame from this source kind.
pub fn unit_for_source(kind: SourceKind) -> &'static str {
    if kind.declares_unit() {
        "game"
    } else {
        "unknown"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn row(value: serde_json::Value) -> ActorDataRow {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn world_cm_convert_to_the_ingame_readout() {
        assert_eq!(cm_to_units(-412_456.0), -412.46);
        assert_eq!(cm_to_units(88_300.0), 88.3);
        assert_eq!(cm_to_units(12_400.0), 12.4);
    }

    #[test]
    fn unit_type_maps_to_the_maps_vocabulary() {
        assert_eq!(kind_from_unit_type("Player"), "player");
        assert_eq!(kind_from_unit_type("BaseCamp"), "palbox");
        assert_eq!(kind_from_unit_type("FutureThing"), "futurething");
    }

    #[test]
    fn blueprint_class_resolves_to_a_bare_tribe() {
        assert_eq!(
            tribe_from_class("Chara_BP_PlantSlime_C").as_deref(),
            Some("PlantSlime")
        );
        assert_eq!(
            tribe_from_class("Chara_BP_SheepBall_C").as_deref(),
            Some("SheepBall")
        );
        assert_eq!(tribe_from_class("Player_Female"), None);
        assert_eq!(tribe_from_class("BP__C"), None);
    }

    #[test]
    fn rows_normalize_with_yaw_from_rotation_z() {
        let actor = actor_from_row(
            &row(json!({
                "UnitType": "Player", "InstanceID": "p1", "NickName": "MockTamer",
                "LocationX": -123456.0, "LocationY": 98765.0, "LocationZ": 2500.0,
                "RotationY": 999.0, "RotationZ": 45.0,
                "Level": 25, "HP": 300, "MaxHP": 300, "GuildName": "MockGuild"
            })),
            None,
        )
        .unwrap();
        assert_eq!(actor.x, -123.46);
        assert_eq!(actor.y, 98.77);
        assert_eq!(actor.alt, 2.5);
        assert_eq!(actor.yaw, Some(45.0));
        assert_eq!(actor.guild_name.as_deref(), Some("MockGuild"));
        assert_eq!(actor.kind, "player");
        assert_eq!(actor.max_hp, Some(300));
    }

    #[test]
    fn withheld_fields_never_reach_the_actor() {
        let actor = actor_from_row(
            &row(json!({
                "UnitType": "Player", "InstanceID": "p1",
                "LocationX": 1.0, "LocationY": 2.0,
                "userid": "SECRET", "ip": "1.2.3.4"
            })),
            None,
        )
        .unwrap();
        let wire = serde_json::to_string(&actor).unwrap();
        assert!(!wire.contains("SECRET"));
        assert!(!wire.contains("1.2.3.4"));
        assert!(!wire.contains("userid"));
        assert!(!wire.contains("\"ip\""));
    }

    #[test]
    fn unknown_unit_type_is_kept_not_dropped() {
        let actor = actor_from_row(
            &row(json!({
                "UnitType": "FutureThing", "InstanceID": "ft1",
                "LocationX": 100.0, "LocationY": 200.0, "LocationZ": 3000.0
            })),
            None,
        )
        .unwrap();
        assert_eq!(actor.kind, "futurething");
        assert_eq!(actor.alt, 3.0);
    }

    #[test]
    fn rows_without_identity_get_position_stable_ids() {
        let a = actor_from_row(
            &row(json!({"LocationX": 5000.0, "LocationY": 6000.0})),
            Some("palbox"),
        )
        .unwrap();
        assert_eq!(a.id, "palbox@5000,6000");
        let b = actor_from_row(
            &row(json!({"LocationX": 900_000.0, "LocationY": -40_000.0})),
            Some("palbox"),
        )
        .unwrap();
        assert_eq!(b.id, "palbox@900000,-40000");
        assert_ne!(a.id, b.id);
    }

    #[test]
    fn a_row_with_no_position_and_no_identity_is_dropped() {
        assert!(actor_from_row(&row(json!({"HP": 10})), None).is_none());
    }

    #[test]
    fn game_data_bodies_parse_actors_and_clock() {
        let body = json!({"ActorData": [
            {"UnitType": "Player", "InstanceID": "p1", "LocationX": -1.0, "LocationY": 2.0},
            {"UnitType": "Pal", "InstanceID": "pal1", "Class": "Chara_BP_SheepBall_C",
             "LocationX": -1000.0, "LocationY": 2000.0, "RotationZ": 90.0}
        ]});
        let (actors, ingame) = actors_from_game_data(&body);
        assert_eq!(actors.len(), 2);
        assert_eq!(actors[1].tribe.as_deref(), Some("SheepBall"));
        assert_eq!(actors[1].yaw, Some(90.0));
        assert!(ingame.is_none());
    }

    #[test]
    fn players_feed_maps_to_positionless_player_actors() {
        let body = json!({"players": [
            {"accountInfo": {"accountId": "76561198000000000", "name": "MockTamer"},
             "playerInfo": {"level": 25, "hp": 300, "maxHP": 300}},
            {"accountInfo": {"accountId": "76561198000000001", "name": "Second"}}
        ]});
        let actors = actors_from_players_feed(&body);
        assert_eq!(actors.len(), 2);
        assert_eq!(actors[0].id, "76561198000000000");
        assert_eq!(actors[0].name.as_deref(), Some("MockTamer"));
        assert_eq!(actors[0].kind, "player");
        assert_eq!(actors[1].level, None);
        assert_eq!(unit_for_source(SourceKind::Rest), "unknown");
        assert_eq!(unit_for_source(SourceKind::RestGameData), "game");
    }
}
