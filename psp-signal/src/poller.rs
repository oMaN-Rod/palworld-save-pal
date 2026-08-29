//! The polling engine: one background task per active source.
//!
//! Failure semantics: a torn/empty answer is a *hiccup*
//! (retried after `HICCUP_RETRY_POLLS`), a hard refusal (auth, transport,
//! unsupported endpoint) is remembered for `REPROBE_POLLS`, and a frame
//! older than `stale_after` reports `stale: true` while the last good
//! actors stay on the wire.
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use crate::model::{Actor, FeedState, InGameTime, LiveFrame, SourceKind, SourceStatus};
use crate::normalize::{
    actors_from_game_data, actors_from_players_feed, unit_for_source,
};
use crate::rest::{RestRead, SignalRestClient};

/// Polls skipped after a hiccup before retrying.
pub const HICCUP_RETRY_POLLS: u32 = 5;
/// Polls skipped after a refusal before a full reprobe.
pub const REPROBE_POLLS: u32 = 60;
/// A frame older than this is stale (the source kept the last actors).
pub const STALE_AFTER: Duration = Duration::from_secs(10);

/// What the poller should read. The REST password lives only here, in
/// memory — it is never persisted.
#[derive(Debug, Clone)]
pub enum SourceConfig {
    /// Synthetic feed; no game or server needed.
    Fake,
    /// The game's `-output-gamedata` bridge file. `None` = auto-discover.
    GameData { path: Option<PathBuf> },
    /// A dedicated server REST base (already normalized) + AdminPassword.
    Rest { base: String, password: Option<String> },
}

/// Everything the API layer needs, updated by the poll task under one lock.
#[derive(Debug)]
pub struct FeedStateShared {
    pub status: SourceStatus,
    pub frame: Option<LiveFrame>,
    last_ok: Option<std::time::Instant>,
    /// REST-mode bookkeeping.
    probed: bool,
    rich_mode: bool,
    skips_left: u32,
    /// Fake-source animation phase.
    tick: u64,
}

impl FeedStateShared {
    pub fn new() -> Self {
        Self::default()
    }

    /// Refreshes `age`/`stale` on demand so readers always see fresh values.
    pub fn refresh(&mut self) {
        if let (Some(frame), Some(last_ok)) = (&mut self.frame, self.last_ok) {
            frame.age = (last_ok.elapsed().as_secs_f64() * 10.0).round() / 10.0;
            frame.stale = last_ok.elapsed() > STALE_AFTER;
            if frame.stale {
                self.status.state = match self.status.state {
                    FeedState::Players | FeedState::World | FeedState::Feeding => FeedState::Stale,
                    other => other,
                };
            }
        }
    }
}

impl Default for FeedStateShared {
    fn default() -> Self {
        Self {
            status: SourceStatus::default(),
            frame: None,
            last_ok: None,
            probed: false,
            rich_mode: true,
            skips_left: 0,
            tick: 0,
        }
    }
}

/// A best-effort in-game clock for the fake source so the map has something
/// plausible to render.
fn fake_ingame_clock(tick: u64) -> InGameTime {
    let minute_of_day = (tick * 10) % (24 * 60);
    InGameTime {
        time: Some(format!(
            "{:02}:{:02}",
            minute_of_day / 60,
            minute_of_day % 60
        )),
        days: Some(1 + (tick / (24 * 6)) as i64),
    }
}

/// The synthetic world: a walking tamer, their otomo, a wild pal orbiting
/// nearby, and one base with a Pal Box. Coordinates animate a little each
/// tick so consumers can see the feed is live.
fn fake_actors(tick: u64) -> Vec<Actor> {
    let phase = tick as f64;
    let walk = (phase * 0.35).sin() * 12.0;
    let base_x = -385.55;
    let base_y = 59.06;
    let yaw = (190.1 + phase * 2.0) % 360.0;
    vec![
        Actor {
            id: "fake-player".into(),
            kind: "player".into(),
            x: ((base_x + walk) * 100.0).round() / 100.0,
            y: ((base_y - walk * 0.5) * 100.0).round() / 100.0,
            alt: 12.0,
            name: Some("Test Pal Tamer".into()),
            level: Some(42),
            stage: Some("None".into()),
            hp: Some(670),
            max_hp: Some(670),
            active: Some(true),
            cls: Some("Player_Female".into()),
            yaw: Some((yaw * 10.0).round() / 10.0),
            owner: None,
            tribe: None,
            guild_name: None,
        },
        Actor {
            id: "fake-otomo".into(),
            kind: "otomo".into(),
            x: ((base_x + walk - 1.2) * 100.0).round() / 100.0,
            y: ((base_y - walk * 0.5 - 0.9) * 100.0).round() / 100.0,
            alt: 12.0,
            name: Some("Lamball".into()),
            level: Some(40),
            stage: Some("None".into()),
            hp: Some(500),
            max_hp: Some(500),
            active: Some(true),
            cls: None,
            yaw: Some(yaw),
            owner: Some("Test Pal Tamer".into()),
            tribe: Some("SheepBall".into()),
            guild_name: None,
        },
        Actor {
            id: "fake-wild".into(),
            kind: "wild".into(),
            x: ((base_x - 22.0 + (phase * 0.2).cos() * 6.0) * 100.0).round() / 100.0,
            y: ((base_y - 9.0 + (phase * 0.2).sin() * 6.0) * 100.0).round() / 100.0,
            alt: 12.0,
            name: Some("Gumoss".into()),
            level: Some(3),
            stage: Some("None".into()),
            hp: Some(635),
            max_hp: Some(635),
            active: Some(true),
            cls: None,
            yaw: Some((230.1 + phase) % 360.0),
            owner: None,
            tribe: Some("PlantSlime".into()),
            guild_name: None,
        },
        Actor {
            id: "palbox@-375000,0".into(),
            kind: "palbox".into(),
            x: -375.0,
            y: 0.0,
            alt: 12.0,
            name: Some("Test Base".into()),
            level: None,
            stage: None,
            hp: None,
            max_hp: None,
            active: None,
            cls: Some("BuildObject_PalBoxV2".into()),
            yaw: None,
            owner: None,
            tribe: None,
            guild_name: None,
        },
    ]
}

/// Spawns the poll loop for `source`, replacing any previous task. The
/// returned shared handle is what the HTTP API reads; dropping the
/// `JoinHandle`-bearing guard stops the loop.
pub async fn spawn_poller(
    source: SourceConfig,
    interval: Duration,
) -> (Arc<Mutex<FeedStateShared>>, tokio::task::JoinHandle<()>) {
    let shared = Arc::new(Mutex::new(FeedStateShared::new()));
    let task_shared = Arc::clone(&shared);
    let interval = interval.max(Duration::from_millis(250));
    let handle = tokio::spawn(async move {
        let client = SignalRestClient::new();
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            let mut state = task_shared.lock().await;
            state.tick += 1;
            poll_once(&source, &client, &mut state).await;
        }
    });
    (shared, handle)
}

async fn poll_once(source: &SourceConfig, client: &SignalRestClient, state: &mut FeedStateShared) {
    if state.skips_left > 0 {
        state.skips_left -= 1;
        return;
    }
    match source {
        SourceConfig::Fake => {
            let actors = fake_actors(state.tick);
            let count = actors.len();
            let frame = LiveFrame {
                ok: true,
                wire_build: crate::WIRE_BUILD,
                source: SourceKind::Fake.as_str().to_string(),
                age: 0.0,
                stale: false,
                time: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                fps: Some(60.0),
                ingame: Some(fake_ingame_clock(state.tick)),
                unit: unit_for_source(SourceKind::Fake),
                actors,
            };
            state.frame = Some(frame);
            state.last_ok = Some(std::time::Instant::now());
            state.status = SourceStatus {
                kind: Some(SourceKind::Fake),
                state: FeedState::Feeding,
                error: None,
                last_ok_age: Some(0.0),
                actor_count: count,
            };
        }
        SourceConfig::GameData { path } => {
            let path = path
                .clone()
                .or_else(crate::discovery::find_game_data);
            let Some(path) = path else {
                state.status = SourceStatus {
                    kind: Some(SourceKind::GameData),
                    state: FeedState::Waiting,
                    error: Some(
                        "waiting for the game - is it running with -output-gamedata?".to_string(),
                    ),
                    last_ok_age: state.status.last_ok_age,
                    actor_count: 0,
                };
                return;
            };
            match tokio::fs::read_to_string(&path).await {
                Ok(text) => {
                    let trimmed = text.trim_start_matches('\u{feff}').trim();
                    if trimmed.is_empty() {
                        hiccup(state, SourceKind::GameData, "bridge file is empty");
                        return;
                    }
                    match serde_json::from_str::<serde_json::Value>(trimmed) {
                        Ok(body) => {
                            let (actors, ingame) = actors_from_game_data(&body);
                            let count = actors.len();
                            state.frame = Some(LiveFrame {
                                ok: true,
                                wire_build: crate::WIRE_BUILD,
                                source: SourceKind::GameData.as_str().to_string(),
                                age: 0.0,
                                stale: false,
                                time: chrono::Local::now()
                                    .format("%Y-%m-%d %H:%M:%S")
                                    .to_string(),
                                fps: body.get("fps").and_then(|v| v.as_f64()),
                                ingame,
                                unit: unit_for_source(SourceKind::GameData),
                                actors,
                            });
                            state.last_ok = Some(std::time::Instant::now());
                            state.status = SourceStatus {
                                kind: Some(SourceKind::GameData),
                                state: FeedState::Feeding,
                                error: None,
                                last_ok_age: Some(0.0),
                                actor_count: count,
                            };
                        }
                        Err(_) => {
                            // A half-written file is a soft error, not a crash.
                            hiccup(state, SourceKind::GameData, "partial write (torn JSON)");
                        }
                    }
                }
                Err(_) => {
                    state.status = SourceStatus {
                        kind: Some(SourceKind::GameData),
                        state: FeedState::Waiting,
                        error: Some("bridge file not found yet".to_string()),
                        last_ok_age: state.status.last_ok_age,
                        actor_count: 0,
                    };
                }
            }
        }
        SourceConfig::Rest { base, password } => {
            let Some(password) = password.clone() else {
                state.status = SourceStatus {
                    kind: Some(SourceKind::Rest),
                    state: FeedState::Waiting,
                    error: Some("AdminPassword needed - enter it in the Signal tab".to_string()),
                    last_ok_age: state.status.last_ok_age,
                    actor_count: 0,
                };
                return;
            };
            if !state.probed {
                match client.players(base, &password).await {
                    RestRead::Unauthorized => {
                        refuse(state, SourceKind::Rest, FeedState::Auth,
                            "REST 401 (wrong AdminPassword?)");
                    }
                    RestRead::Transport => {
                        refuse(state, SourceKind::Rest, FeedState::Down,
                            "REST unreachable - check the address and RESTAPIEnabled=True");
                    }
                    RestRead::Ok(_) => {
                        state.probed = true;
                        state.rich_mode = true;
                        state.skips_left = 0;
                        state.status = SourceStatus {
                            kind: Some(SourceKind::Rest),
                            state: FeedState::Waiting,
                            error: None,
                            last_ok_age: state.status.last_ok_age,
                            actor_count: 0,
                        };
                    }
                    RestRead::Torn => hiccup(state, SourceKind::Rest, "torn probe answer"),
                    RestRead::NotSupported | RestRead::Status(_) => {
                        // Old server: fall back to the players-only feed.
                        state.probed = true;
                        state.rich_mode = false;
                    }
                }
                return;
            }
            if state.rich_mode {
                match client.game_data(base, &password).await {
                    RestRead::Ok(body) => {
                        let (actors, ingame) = actors_from_game_data(&body);
                        publish_rest_frame(state, SourceKind::RestGameData,
                            FeedState::World, actors, ingame, None);
                    }
                    RestRead::Unauthorized => {
                        refuse(state, SourceKind::RestGameData, FeedState::Auth,
                            "REST 401 (wrong AdminPassword?)");
                    }
                    RestRead::Transport => {
                        refuse(state, SourceKind::RestGameData, FeedState::Down,
                            "REST unreachable - the server stopped answering");
                    }
                    RestRead::NotSupported => {
                        // The probe passed but the rich feed is absent: old server.
                        state.rich_mode = false;
                    }
                    RestRead::Torn => hiccup(state, SourceKind::RestGameData, "torn or empty answer"),
                    RestRead::Status(status) => {
                        refuse(state, SourceKind::RestGameData, FeedState::Down,
                            &format!("REST {status}"));
                    }
                }
            } else {
                match client.players(base, &password).await {
                    RestRead::Ok(body) => {
                        let actors = actors_from_players_feed(&body);
                        publish_rest_frame(state, SourceKind::Rest,
                            FeedState::Players, actors, None, Some("no positions - players only"));
                    }
                    RestRead::Unauthorized => {
                        refuse(state, SourceKind::Rest, FeedState::Auth,
                            "REST 401 (wrong AdminPassword?)");
                    }
                    RestRead::Transport => {
                        refuse(state, SourceKind::Rest, FeedState::Down,
                            "REST unreachable - the server stopped answering");
                    }
                    RestRead::Torn => hiccup(state, SourceKind::Rest, "torn or empty answer"),
                    RestRead::NotSupported | RestRead::Status(_) => {
                        refuse(state, SourceKind::Rest, FeedState::Down, "players feed unavailable");
                    }
                }
            }
        }
    }
}

fn publish_rest_frame(
    state: &mut FeedStateShared,
    kind: SourceKind,
    feed_state: FeedState,
    actors: Vec<Actor>,
    ingame: Option<InGameTime>,
    warning: Option<&str>,
) {
    let count = actors.len();
    state.frame = Some(LiveFrame {
        ok: true,
        wire_build: crate::WIRE_BUILD,
        source: kind.as_str().to_string(),
        age: 0.0,
        stale: false,
        time: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        fps: None,
        ingame,
        unit: unit_for_source(kind),
        actors,
    });
    state.last_ok = Some(std::time::Instant::now());
    state.status = SourceStatus {
        kind: Some(kind),
        state: feed_state,
        error: warning.map(str::to_string),
        last_ok_age: Some(0.0),
        actor_count: count,
    };
}

fn hiccup(state: &mut FeedStateShared, kind: SourceKind, why: &str) {
    state.skips_left = HICCUP_RETRY_POLLS;
    state.status = SourceStatus {
        kind: Some(kind),
        state: FeedState::Waiting,
        error: Some(format!("{why} - retrying")),
        last_ok_age: state.status.last_ok_age,
        actor_count: state.status.actor_count,
    };
}

fn refuse(state: &mut FeedStateShared, kind: SourceKind, feed_state: FeedState, why: &str) {
    state.skips_left = REPROBE_POLLS;
    state.probed = false;
    state.status = SourceStatus {
        kind: Some(kind),
        state: feed_state,
        error: Some(why.to_string()),
        last_ok_age: state.status.last_ok_age,
        actor_count: state.status.actor_count,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fake_source_publishes_all_four_kinds_and_advances() {
        let (shared, handle) = spawn_poller(SourceConfig::Fake, Duration::from_millis(300)).await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        let state = shared.lock().await;
        let frame = state.frame.as_ref().expect("frame published");
        let kinds: Vec<&str> = frame.actors.iter().map(|a| a.kind.as_str()).collect();
        assert_eq!(kinds, vec!["player", "otomo", "wild", "palbox"]);
        assert_eq!(frame.source, "fake");
        assert_eq!(frame.unit, "game");
        assert_eq!(state.status.state, FeedState::Feeding);
        drop(state);
        handle.abort();
    }
}
