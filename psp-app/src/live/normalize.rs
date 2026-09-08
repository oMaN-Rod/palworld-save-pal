use super::model::*;
use serde::Deserialize;

#[derive(Debug)]
pub enum NormalizeError {
    Empty,
    Parse(String),
}

// Input-side struct declares ONLY consumed fields; `userid`/`ip` are never
// declared, so they cannot survive into any output.
#[derive(Deserialize)]
struct RawWorld {
    #[serde(rename = "FPS")]
    fps: Option<f64>,
    #[serde(rename = "InGameTime")]
    ingame_time: Option<String>,
    #[serde(rename = "InGameDays")]
    ingame_days: Option<i64>,
    #[serde(rename = "ActorData", default)]
    actors: Vec<RawActor>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawActor {
    #[serde(rename = "Type")]
    ty: String,
    #[serde(rename = "UnitType")]
    unit_type: String,
    #[serde(rename = "InstanceID")]
    instance_id: String,
    #[serde(rename = "NickName")]
    nick_name: String,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "TrainerInstanceID")]
    trainer_instance_id: String,
    #[serde(rename = "Class")]
    class: String,
    level: Option<i64>,
    #[serde(rename = "HP")]
    hp: Option<i64>,
    #[serde(rename = "MaxHP")]
    max_hp: Option<i64>,
    #[serde(rename = "GuildName")]
    guild_name: String,
    #[serde(rename = "LocationX")]
    x: Option<f64>,
    #[serde(rename = "LocationY")]
    y: Option<f64>,
    #[serde(rename = "LocationZ")]
    z: Option<f64>,
    #[serde(rename = "RotationZ")]
    yaw: Option<f64>,
    #[serde(rename = "IsActive")]
    is_active: String,
}

pub fn normalize_world_json(
    raw: &str,
    source: LiveSourceKind,
    seq: u64,
    captured_at_ms: u64,
    observed_at_ms: u64,
) -> Result<LiveFrame, NormalizeError> {
    let raw = raw.trim_start_matches('\u{feff}');
    if raw.trim().is_empty() {
        return Err(NormalizeError::Empty);
    }
    let world: RawWorld =
        serde_json::from_str(raw).map_err(|e| NormalizeError::Parse(e.to_string()))?;
    let actors = world.actors.iter().filter_map(to_actor).collect();
    Ok(LiveFrame {
        seq,
        source,
        captured_at_ms,
        observed_at_ms,
        fps: world.fps,
        ingame_time: world.ingame_time,
        ingame_days: world.ingame_days,
        actors,
    })
}

fn to_actor(r: &RawActor) -> Option<LiveActor> {
    let (x, y, z) = (r.x?, r.y?, r.z?);
    let kind = match (r.ty.as_str(), r.unit_type.as_str()) {
        ("PalBox", _) => "palbox".to_string(),
        (_, "Player") => "player".to_string(),
        (_, "OtomoPal") => "otomo".to_string(),
        (_, "BaseCampPal") => "basepal".to_string(),
        (_, "WildPal") => "wild".to_string(),
        (_, "NPC") => "npc".to_string(),
        (_, other) if !other.is_empty() => other.to_lowercase(),
        _ => r.ty.to_lowercase(),
    };
    let id = if r.instance_id.is_empty() {
        format!("{kind}@{x:.0},{y:.0}")
    } else {
        r.instance_id.clone()
    };
    let name = if !r.nick_name.is_empty() {
        Some(r.nick_name.clone())
    } else if !r.name.is_empty() {
        Some(r.name.clone())
    } else {
        None
    };
    Some(LiveActor {
        id,
        kind,
        x,
        y,
        z,
        yaw: r.yaw,
        name,
        level: r.level.filter(|l| *l > 0),
        hp: r.hp,
        max_hp: r.max_hp,
        guild: (!r.guild_name.is_empty()).then(|| r.guild_name.clone()),
        owner: (!r.trainer_instance_id.is_empty()).then(|| r.trainer_instance_id.clone()),
        species: species_of(&r.class),
        active: match r.is_active.as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
    })
}

fn species_of(class: &str) -> Option<String> {
    class
        .strip_prefix("BP_")
        .and_then(|s| s.strip_suffix("_C"))
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}
