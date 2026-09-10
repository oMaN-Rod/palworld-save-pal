use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{Local, NaiveDateTime, TimeZone};
use ps_app::live::{
    normalize_world_json, LiveFrame, LiveSourceKind, SignalSourceStatus, SourceHealth,
};
use serde_json::Value;
use tokio::sync::watch;
use tokio::time::{self, MissedTickBehavior};
use tokio_util::sync::CancellationToken;

const STALE_THRESHOLD_MS: u64 = 10_000;

pub fn default_gamedata_path() -> Option<PathBuf> {
    if !cfg!(target_os = "windows") {
        return None;
    }
    let local_app_data = std::env::var_os("LOCALAPPDATA")?;
    Some(
        PathBuf::from(local_app_data)
            .join("Pal")
            .join("Saved")
            .join("PalGameDataBridge")
            .join("GameData.json"),
    )
}

pub async fn run_file_source(
    path: PathBuf,
    bus: watch::Sender<Option<LiveFrame>>,
    status: watch::Sender<SignalSourceStatus>,
    seq: Arc<AtomicU64>,
    cancel: CancellationToken,
) {
    let mut ticker = time::interval(Duration::from_secs(1));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut last_seen: Option<(SystemTime, u64)> = None;
    let mut last_captured_at_ms: Option<u64> = None;
    let mut last_actor_count: usize = 0;
    let mut last_frame_observed_at_ms: Option<u64> = None;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = ticker.tick() => {
                let meta = match std::fs::metadata(&path) {
                    Ok(meta) => meta,
                    Err(_) => {
                        last_seen = None;
                        last_captured_at_ms = None;
                        last_actor_count = 0;
                        last_frame_observed_at_ms = None;
                        status.send_replace(SignalSourceStatus {
                            kind: Some(LiveSourceKind::File),
                            health: SourceHealth::Waiting,
                            error: None,
                            last_frame_ms: None,
                            actor_count: 0,
                        });
                        continue;
                    }
                };

                let mtime = meta.modified().unwrap_or(UNIX_EPOCH);
                let signature = (mtime, meta.len());
                if last_seen != Some(signature) {
                    last_seen = Some(signature);

                    if let Ok(raw) = std::fs::read_to_string(&path) {
                        if let Ok(doc) = serde_json::from_str::<Value>(&raw) {
                            let captured_at_ms =
                                captured_at_ms_from(&doc).unwrap_or_else(|| to_millis(mtime));
                            last_captured_at_ms = Some(captured_at_ms);
                            let observed_at_ms = to_millis(SystemTime::now());

                            let frame_seq = seq.fetch_add(1, Ordering::Relaxed) + 1;
                            if let Ok(frame) = normalize_world_json(
                                &raw,
                                LiveSourceKind::File,
                                frame_seq,
                                captured_at_ms,
                                observed_at_ms,
                            ) {
                                last_actor_count = frame.actors.len();
                                last_frame_observed_at_ms = Some(observed_at_ms);
                                bus.send_replace(Some(frame));
                            }
                        }
                    }
                }

                let observed_at_ms = to_millis(SystemTime::now());
                let health = match last_captured_at_ms {
                    Some(captured_at_ms)
                        if observed_at_ms.saturating_sub(captured_at_ms) <= STALE_THRESHOLD_MS =>
                    {
                        SourceHealth::Ok
                    }
                    Some(_) => SourceHealth::Stale,
                    None => SourceHealth::Waiting,
                };
                status.send_replace(SignalSourceStatus {
                    kind: Some(LiveSourceKind::File),
                    health,
                    error: None,
                    last_frame_ms: last_frame_observed_at_ms,
                    actor_count: last_actor_count,
                });
            }
        }
    }
}

fn captured_at_ms_from(doc: &Value) -> Option<u64> {
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
