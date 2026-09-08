use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{Local, NaiveDateTime, TimeZone};
use psp_app::live::{
    normalize_world_json, LiveFrame, LiveSourceKind, SignalSourceStatus, SourceHealth,
};
use serde_json::Value;
use tokio::sync::watch;
use tokio::time::{self, MissedTickBehavior};
use tokio_util::sync::CancellationToken;

const STALE_THRESHOLD_MS: u64 = 10_000;
const SHORT_BACKOFF_TICKS: u32 = 4;
const LONG_BACKOFF_TICKS: u32 = 59;

pub struct RestSourceConfig {
    pub host: String,
    pub port: u16,
    pub admin_password: String,
    pub endpoint_base: String,
}

enum Backoff {
    None,
    Short(u32),
    Long(u32),
}

enum ProbeOutcome {
    Ok(String),
    Unauthorized,
    NotFound,
    TransportError,
}

pub async fn run_rest_source(
    cfg: RestSourceConfig,
    bus: watch::Sender<Option<LiveFrame>>,
    status: watch::Sender<SignalSourceStatus>,
    seq: Arc<AtomicU64>,
    cancel: CancellationToken,
) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .expect("building reqwest client cannot fail with static config");
    let url = format!(
        "http://{}:{}/{}/game-data",
        cfg.host, cfg.port, cfg.endpoint_base
    );

    let mut ticker = time::interval(Duration::from_secs(1));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut backoff = Backoff::None;
    let mut health = SourceHealth::Waiting;
    let mut error: Option<String> = None;
    let mut last_actor_count: usize = 0;
    let mut last_frame_observed_at_ms: Option<u64> = None;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = ticker.tick() => {
                let should_probe = match &mut backoff {
                    Backoff::None => true,
                    Backoff::Short(remaining) | Backoff::Long(remaining) => {
                        if *remaining == 0 {
                            true
                        } else {
                            *remaining -= 1;
                            false
                        }
                    }
                };

                if should_probe {
                    let outcome = tokio::select! {
                        _ = cancel.cancelled() => break,
                        outcome = probe(&client, &url, &cfg.admin_password) => outcome,
                    };
                    match outcome {
                        ProbeOutcome::Ok(raw) => {
                            backoff = Backoff::None;
                            error = None;

                            let observed_at_ms = to_millis(SystemTime::now());
                            let captured_at_ms =
                                captured_at_ms_from(&raw).unwrap_or(observed_at_ms);

                            let frame_seq = seq.fetch_add(1, Ordering::Relaxed) + 1;
                            if let Ok(frame) = normalize_world_json(
                                &raw,
                                LiveSourceKind::Rest,
                                frame_seq,
                                captured_at_ms,
                                observed_at_ms,
                            ) {
                                last_actor_count = frame.actors.len();
                                last_frame_observed_at_ms = Some(observed_at_ms);
                                bus.send_replace(Some(frame));
                            }

                            health = if observed_at_ms.saturating_sub(captured_at_ms)
                                <= STALE_THRESHOLD_MS
                            {
                                SourceHealth::Ok
                            } else {
                                SourceHealth::Stale
                            };
                        }
                        ProbeOutcome::Unauthorized => {
                            backoff = Backoff::Long(LONG_BACKOFF_TICKS);
                            health = SourceHealth::Auth;
                            error = Some("The server rejected the admin password".to_string());
                        }
                        ProbeOutcome::NotFound => {
                            backoff = Backoff::Long(LONG_BACKOFF_TICKS);
                            health = SourceHealth::Down;
                            error = Some(
                                "World data endpoint unavailable - the server needs the launch argument"
                                    .to_string(),
                            );
                        }
                        ProbeOutcome::TransportError => {
                            backoff = Backoff::Short(SHORT_BACKOFF_TICKS);
                            health = SourceHealth::Down;
                            error = Some("Could not reach the dedicated server".to_string());
                        }
                    }
                }

                status.send_replace(SignalSourceStatus {
                    kind: Some(LiveSourceKind::Rest),
                    health,
                    error: error.clone(),
                    last_frame_ms: last_frame_observed_at_ms,
                    actor_count: last_actor_count,
                });
            }
        }
    }
}

async fn probe(client: &reqwest::Client, url: &str, admin_password: &str) -> ProbeOutcome {
    let response = match client
        .get(url)
        .basic_auth("admin", Some(admin_password))
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => return ProbeOutcome::TransportError,
    };
    match response.status() {
        reqwest::StatusCode::OK => match response.text().await {
            Ok(raw) => ProbeOutcome::Ok(raw),
            Err(_) => ProbeOutcome::TransportError,
        },
        reqwest::StatusCode::UNAUTHORIZED => ProbeOutcome::Unauthorized,
        reqwest::StatusCode::NOT_FOUND => ProbeOutcome::NotFound,
        _ => ProbeOutcome::TransportError,
    }
}

fn captured_at_ms_from(raw: &str) -> Option<u64> {
    let doc: Value = serde_json::from_str(raw).ok()?;
    let raw_time = doc.get("Time")?.as_str()?;
    let naive = NaiveDateTime::parse_from_str(raw_time, "%Y-%m-%d %H:%M:%S").ok()?;
    let local = Local.from_local_datetime(&naive).single()?;
    Some(local.timestamp_millis() as u64)
}

fn to_millis(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
