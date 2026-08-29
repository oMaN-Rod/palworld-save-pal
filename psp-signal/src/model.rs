//! The internal data model and its wire (JSON) shapes.
//!
//! Field names on [`Actor`] and [`LiveFrame`] are the wire format
//! (`maxHp`, `guildName`, ...) so companion map apps can consume a
//! Signal feed unchanged.
use serde::{Deserialize, Serialize};

/// One normalized world object: a player, pal, base camp, or anything else
/// the source reported. Optional fields are omitted rather than nulled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Actor {
    pub id: String,
    pub kind: String,
    pub x: f64,
    pub y: f64,
    pub alt: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hp: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_hp: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cls: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yaw: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tribe: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_name: Option<String>,
}

/// In-game clock state, when the source reports one.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InGameTime {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub days: Option<i64>,
}

/// A full snapshot of the world at one poll, as served by `/v1/live`.
#[derive(Debug, Clone, Serialize)]
pub struct LiveFrame {
    pub ok: bool,
    /// Wire build number; serialized under the legacy `beacon` key so
    /// existing feed clients keep working.
    #[serde(rename = "beacon")]
    pub wire_build: i64,
    pub source: String,
    pub age: f64,
    pub stale: bool,
    pub time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fps: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ingame: Option<InGameTime>,
    pub unit: &'static str,
    pub actors: Vec<Actor>,
}

impl LiveFrame {
    /// `actors` is never null on the wire, even for a not-ok frame.
    pub fn empty(ok: bool, source: &str, unit: &'static str) -> Self {
        Self {
            ok,
            wire_build: crate::WIRE_BUILD,
            source: source.to_string(),
            age: 0.0,
            stale: false,
            time: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            fps: None,
            ingame: None,
            unit,
            actors: Vec::new(),
        }
    }
}

/// Which source feeds the poller. Source vocabulary: `rest` (players-only
/// REST), `restgamedata` (rich REST), `gamedata` (local bridge file), `fake`
/// (synthetic).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    Rest,
    RestGameData,
    GameData,
    Fake,
}

impl SourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SourceKind::Rest => "rest",
            SourceKind::RestGameData => "restgamedata",
            SourceKind::GameData => "gamedata",
            SourceKind::Fake => "fake",
        }
    }

    /// Whether actors from this source declare their own unit type. Only the
    /// players-only REST feed leaves the guessing to the map.
    pub fn declares_unit(self) -> bool {
        !matches!(self, SourceKind::Rest)
    }
}

/// Connection lifecycle for a dedicated-server source, as surfaced on
/// `/v1/server` and the Signal tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FeedState {
    /// No source configured.
    Idle,
    /// Configured, first probe in flight (or password still needed).
    Waiting,
    /// Server answered 401 — the AdminPassword is wrong or missing.
    Auth,
    /// Transport failure — unreachable, timeout, TLS.
    Down,
    /// Connected, but the last good frame is old.
    Stale,
    /// Connected through the players-only REST feed.
    Players,
    /// Connected through the rich game-data feed.
    World,
    /// Local (file or fake) source produced a fresh frame.
    Feeding,
}

impl FeedState {
    pub fn as_str(self) -> &'static str {
        match self {
            FeedState::Idle => "idle",
            FeedState::Waiting => "waiting",
            FeedState::Auth => "auth",
            FeedState::Down => "down",
            FeedState::Stale => "stale",
            FeedState::Players => "players",
            FeedState::World => "world",
            FeedState::Feeding => "feeding",
        }
    }
}

/// The poller's published condition, updated after every read.
#[derive(Debug, Clone)]
pub struct SourceStatus {
    pub kind: Option<SourceKind>,
    pub state: FeedState,
    /// Human-readable detail for the UI; never contains a password.
    pub error: Option<String>,
    /// Seconds since the last successful frame, if there ever was one.
    pub last_ok_age: Option<f64>,
    pub actor_count: usize,
}

impl Default for SourceStatus {
    fn default() -> Self {
        Self {
            kind: None,
            state: FeedState::Idle,
            error: None,
            last_ok_age: None,
            actor_count: 0,
        }
    }
}
