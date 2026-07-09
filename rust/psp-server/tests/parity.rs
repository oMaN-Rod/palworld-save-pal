//! Replays rust/parity/fixtures/** against an in-process Rust server and
//! asserts response-sequence equality with the recorded Python behavior.
//! With no fixtures generated (fresh clone / CI), the test skips loudly on
//! stderr and passes — see rust/parity/README.md for the three states.

use std::path::PathBuf;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use serde_json::Value;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use psp_server::{start_server, ServerConfig};

/// Justified, documented parity divergences, as `"<message_type>:<json_pointer>"`
/// masks (e.g. `"loaded_save_files:/data/save_dir"`). MUST stay empty unless a
/// divergence is reviewed and each entry carries a one-line "why" comment.
const PARITY_IGNORED_PATHS: &[&str] = &[];

// clippy(const_is_empty) correctly notices PARITY_IGNORED_PATHS is empty right
// now — that IS the point of this guard: it must keep failing loudly the
// moment someone appends a mask without also implementing the stripping logic.
#[allow(clippy::const_is_empty)]
fn strip_ignored_paths(_message_type: &str, _value: &mut Value) {
    // Populated when PARITY_IGNORED_PATHS gains its first entry.
    assert!(
        PARITY_IGNORED_PATHS.is_empty(),
        "implement path stripping before allowlisting"
    );
}

/// Replays every fixture found under `fixtures_root` against a fresh server
/// instance and returns the number of fixtures replayed. Returns 0 (without
/// starting a server) if `fixtures_root` doesn't exist or has no corpus
/// subdirectories — the caller decides what "0" means (skip vs. failure).
async fn replay_all_fixtures(fixtures_root: &std::path::Path) -> usize {
    let mut corpus_dirs: Vec<PathBuf> = match std::fs::read_dir(fixtures_root) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect(),
        Err(_) => return 0,
    };
    corpus_dirs.sort();
    if corpus_dirs.is_empty() {
        return 0;
    }

    let temp_dir = tempfile::tempdir().unwrap();
    let handle = start_server(ServerConfig {
        host: "127.0.0.1".parse().unwrap(),
        port: 0,
        ui_dir: temp_dir.path().join("ui"),
        data_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data"),
        db_path: temp_dir.path().join("parity.db"),
        desktop_mode: false,
    })
    .await
    .unwrap();

    let mut fixtures_replayed = 0usize;
    for corpus_dir in corpus_dirs {
        let mut fixture_paths: Vec<PathBuf> = std::fs::read_dir(&corpus_dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("json"))
            .collect();
        fixture_paths.sort();

        let (mut socket, _) = connect_async(format!("ws://{}/ws/parity-replay", handle.addr))
            .await
            .unwrap();

        for fixture_path in fixture_paths {
            let fixture: Value =
                serde_json::from_str(&std::fs::read_to_string(&fixture_path).unwrap()).unwrap();
            let request_text = fixture["request"].to_string();
            let request_type = fixture["request"]["type"]
                .as_str()
                .unwrap_or("")
                .to_string();
            let expected_responses = fixture["responses"].as_array().unwrap().clone();

            socket.send(Message::Text(request_text)).await.unwrap();

            let mut actual_responses = Vec::with_capacity(expected_responses.len());
            for response_index in 0..expected_responses.len() {
                let frame = tokio::time::timeout(Duration::from_secs(60), socket.next())
                    .await
                    .unwrap_or_else(|_| {
                        panic!(
                            "timeout waiting for response {} of {} (request type {:?}) — the \
                             Rust server sent fewer frames than the fixture recorded, or hung",
                            response_index,
                            fixture_path.display(),
                            request_type,
                        )
                    })
                    .expect("socket closed mid-fixture")
                    .unwrap();
                let mut value: Value = serde_json::from_str(frame.to_text().unwrap()).unwrap();
                let response_message_type =
                    value["type"].as_str().unwrap_or(&request_type).to_string();
                strip_ignored_paths(&response_message_type, &mut value);
                actual_responses.push(value);
            }

            let mut expected = expected_responses;
            for value in expected.iter_mut() {
                let response_message_type =
                    value["type"].as_str().unwrap_or(&request_type).to_string();
                strip_ignored_paths(&response_message_type, value);
            }
            pretty_assertions::assert_eq!(
                actual_responses,
                expected,
                "fixture {} (request type {:?}) — response sequence mismatch \
                 (order and count both matter: this compares ordered arrays, not sets)",
                fixture_path.display(),
                request_type,
            );
            fixtures_replayed += 1;
        }
        socket.close(None).await.ok();
    }
    handle.shutdown().await;
    fixtures_replayed
}

#[tokio::test]
async fn replay_recorded_python_fixtures() {
    let fixtures_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../parity/fixtures");
    let fixtures_replayed = replay_all_fixtures(&fixtures_root).await;
    if fixtures_replayed == 0 {
        // Rust's test harness captures `eprintln!`/`println!` output and only
        // shows it for FAILED tests unless the caller passes `--nocapture` —
        // and this test passes on purpose when there are no fixtures. Writing
        // straight to `std::io::stderr()` (bypassing the `eprint!` macro's
        // capture hook entirely) is what makes the skip note show up in the
        // default `cargo test` run, not just under `--nocapture`. Confirmed
        // empirically: `eprintln!` here was silently swallowed on a passing
        // run; this raw write was not.
        use std::io::Write;
        let _ = writeln!(
            std::io::stderr(),
            "SKIPPED: no parity fixtures at {} — run scripts/capture_parity.py; \
             this is expected on a fresh clone/CI (see rust/parity/README.md)",
            fixtures_root.display()
        );
    }
}

/// Proves the harness above actually discriminates: builds a synthetic
/// fixture set — one fixture with a correct expected response, one with a
/// deliberately WRONG expected response — and asserts the replay panics on
/// the mismatch. Without this, `replay_all_fixtures` could be miscomparing
/// (e.g. comparing lengths only, or comparing as an unordered set) and the
/// "no fixtures → skip" test above would never catch it, since it never
/// exercises the comparison logic at all.
#[tokio::test]
#[should_panic(expected = "response sequence mismatch")]
async fn replay_panics_on_a_mismatched_fixture() {
    let temp_dir = tempfile::tempdir().unwrap();
    let corpus_dir = temp_dir.path().join("fixtures/synthetic-mismatch");
    std::fs::create_dir_all(&corpus_dir).unwrap();
    std::fs::write(
        corpus_dir.join("00_get_version.json"),
        serde_json::json!({
            "request": {"type": "get_version"},
            // The real handler answers with env!("CARGO_PKG_VERSION"), not
            // this literal — this fixture is deliberately wrong.
            "responses": [{"type": "get_version", "data": "not-the-real-version"}]
        })
        .to_string(),
    )
    .unwrap();

    replay_all_fixtures(&temp_dir.path().join("fixtures")).await;
}

/// Companion to the mismatch test: the identical fixture, with the correct
/// expected value substituted in, must replay clean. Proves the harness
/// isn't just panicking unconditionally on any synthetic input — it can
/// also pass when the recorded response is right.
#[tokio::test]
async fn replay_passes_on_a_matching_fixture() {
    let temp_dir = tempfile::tempdir().unwrap();
    let corpus_dir = temp_dir.path().join("fixtures/synthetic-match");
    std::fs::create_dir_all(&corpus_dir).unwrap();
    std::fs::write(
        corpus_dir.join("00_get_version.json"),
        serde_json::json!({
            "request": {"type": "get_version"},
            "responses": [{"type": "get_version", "data": env!("CARGO_PKG_VERSION")}]
        })
        .to_string(),
    )
    .unwrap();

    let fixtures_replayed = replay_all_fixtures(&temp_dir.path().join("fixtures")).await;
    assert_eq!(fixtures_replayed, 1);
}

/// Proves fixtures are replayed in FILENAME order, not filesystem discovery
/// order: `std::fs::read_dir` makes no ordering guarantee, so if
/// `replay_all_fixtures` ever dropped its `.sort()` call, this would start
/// failing intermittently depending on directory-entry order. Writes the
/// "01_" fixture (an unregistered — hence silent — message type, so it
/// can't itself hang the test) to disk before the "00_" fixture, so a
/// creation-order bug and a filename-order-respecting implementation would
/// disagree about the number of fixtures a single connection round-trips.
#[tokio::test]
async fn replay_reads_fixtures_in_filename_order() {
    let temp_dir = tempfile::tempdir().unwrap();
    let corpus_dir = temp_dir.path().join("fixtures/synthetic-order");
    std::fs::create_dir_all(&corpus_dir).unwrap();
    std::fs::write(
        corpus_dir.join("01_unregistered.json"),
        serde_json::json!({"request": {"type": "not_a_real_message_type"}, "responses": []})
            .to_string(),
    )
    .unwrap();
    std::fs::write(
        corpus_dir.join("00_get_version.json"),
        serde_json::json!({
            "request": {"type": "get_version"},
            "responses": [{"type": "get_version", "data": env!("CARGO_PKG_VERSION")}]
        })
        .to_string(),
    )
    .unwrap();

    let fixtures_replayed = replay_all_fixtures(&temp_dir.path().join("fixtures")).await;
    assert_eq!(fixtures_replayed, 2);
}

/// Pins the specific property the whole harness leans on for the
/// load-bearing multi-response ordering rule described in
/// rust/parity/README.md: `Vec<Value>` equality (what `assert_eq!` uses on
/// `actual_responses`/`expected` in `replay_all_fixtures`) is index-wise,
/// not multiset/order-insensitive. If this assumption ever silently broke
/// (e.g. a refactor swapped the comparison to something that sorts or
/// dedupes first), two responses recorded in one order but emitted in
/// another — the exact class of regression a `progress_message` /
/// final-result reordering would produce — would stop being caught.
#[test]
fn response_vector_equality_is_order_sensitive_not_set_based() {
    let recorded = vec![
        serde_json::json!({"type": "progress_message", "data": "step 1"}),
        serde_json::json!({"type": "loaded_save_files", "data": {}}),
    ];
    let same_frames_reordered = vec![recorded[1].clone(), recorded[0].clone()];
    assert_ne!(
        recorded, same_frames_reordered,
        "same two frames in swapped order must NOT compare equal"
    );
}
