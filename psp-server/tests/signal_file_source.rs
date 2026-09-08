use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use psp_app::live::{SignalSourceStatus, SourceHealth};
use psp_server::signal::gamedata_source::run_file_source;
use tokio_util::sync::CancellationToken;

fn seq() -> Arc<AtomicU64> {
    Arc::new(AtomicU64::new(0))
}

#[tokio::test]
async fn publishes_frames_and_survives_torn_writes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("GameData.json");
    let (bus, mut bus_rx) = tokio::sync::watch::channel(None);
    let (status, status_rx) = tokio::sync::watch::channel(SignalSourceStatus::default());
    let cancel = CancellationToken::new();
    let task = tokio::spawn(run_file_source(
        path.clone(),
        bus,
        status,
        seq(),
        cancel.clone(),
    ));

    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    assert_eq!(status_rx.borrow().health, SourceHealth::Waiting);

    write_fixture_with_time(&path, chrono::Local::now());
    tokio::time::timeout(std::time::Duration::from_secs(3), bus_rx.changed())
        .await
        .unwrap()
        .unwrap();
    let seq1 = bus_rx.borrow().as_ref().unwrap().seq;
    assert_eq!(status_rx.borrow().health, SourceHealth::Ok);

    std::fs::write(&path, "{\"Time\": \"2026").unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    assert_eq!(bus_rx.borrow().as_ref().unwrap().seq, seq1);

    cancel.cancel();
    tokio::time::timeout(std::time::Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn a_leftover_file_reports_stale_not_fresh() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("GameData.json");
    write_fixture_with_time(&path, chrono::Local::now() - chrono::Duration::hours(6));
    let (bus, mut bus_rx) = tokio::sync::watch::channel(None);
    let (status, status_rx) = tokio::sync::watch::channel(SignalSourceStatus::default());
    let cancel = CancellationToken::new();
    let task = tokio::spawn(run_file_source(path, bus, status, seq(), cancel.clone()));

    tokio::time::timeout(std::time::Duration::from_secs(3), bus_rx.changed())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status_rx.borrow().health, SourceHealth::Stale);

    cancel.cancel();
    tokio::time::timeout(std::time::Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn health_ages_from_ok_to_stale_without_the_file_changing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("GameData.json");
    write_fixture_with_time(&path, chrono::Local::now());
    let (bus, mut bus_rx) = tokio::sync::watch::channel(None);
    let (status, mut status_rx) = tokio::sync::watch::channel(SignalSourceStatus::default());
    let cancel = CancellationToken::new();
    let task = tokio::spawn(run_file_source(path, bus, status, seq(), cancel.clone()));

    tokio::time::timeout(std::time::Duration::from_secs(3), bus_rx.changed())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status_rx.borrow().health, SourceHealth::Ok);
    let seq1 = bus_rx.borrow().as_ref().unwrap().seq;

    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        loop {
            status_rx.changed().await.unwrap();
            if status_rx.borrow().health == SourceHealth::Stale {
                break;
            }
        }
    })
    .await
    .unwrap();

    assert_eq!(bus_rx.borrow().as_ref().unwrap().seq, seq1);

    cancel.cancel();
    tokio::time::timeout(std::time::Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
}

fn write_fixture_with_time(path: &std::path::Path, when: chrono::DateTime<chrono::Local>) {
    const FIXTURE: &str = include_str!("../../psp-app/tests/fixtures/live/world_snapshot.json");
    let mut doc: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    doc["Time"] = serde_json::Value::String(when.format("%Y-%m-%d %H:%M:%S").to_string());
    std::fs::write(path, serde_json::to_string(&doc).unwrap()).unwrap();
}
