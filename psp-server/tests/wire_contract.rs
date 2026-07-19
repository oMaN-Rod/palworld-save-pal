//! Replays the committed request/response fixtures under `contract/fixtures/`
//! against an in-process server and asserts each response sequence still
//! matches, frame for frame and in order. The fixtures are committed golden
//! snapshots of the wire protocol the Svelte frontend consumes, so this suite
//! is a regression net against ACCIDENTAL protocol drift. It FAILS loudly if
//! the committed corpus is missing rather than skipping, so it can never pass
//! with zero coverage. The comparators and masks it relies on are unit-tested
//! below regardless.
//!
//! Provenance: the snapshots were first captured from a now-retired reference
//! implementation and can no longer be re-captured here, so they are maintained
//! as committed golden inputs. Fixtures that only echoed static `data/json`
//! content (and so broke on every game-data patch without exercising real
//! request/response behaviour) are not part of this corpus.

use std::path::PathBuf;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use serde_json::Value;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use psp_server::{start_server, ServerConfig};

/// The single pass/fail decision for a fixture: same frames, same ORDER, same
/// count. Named (rather than an inline `assert_eq!`) so the ordering rule stays
/// unit-testable without a live multi-frame fixture to replay.
fn compare_responses(
    fixture_name: &str,
    request_type: &str,
    actual: &[Value],
    expected: &[Value],
) -> Result<(), String> {
    if actual == expected {
        return Ok(());
    }
    Err(format!(
        "fixture {fixture_name} (request type {request_type:?}) — response sequence \
         mismatch (order and count both matter: this compares ordered arrays, not sets)\n\
         --- actual ---\n{}\n--- expected ---\n{}",
        serde_json::to_string_pretty(actual).unwrap_or_else(|_| format!("{actual:?}")),
        serde_json::to_string_pretty(expected).unwrap_or_else(|_| format!("{expected:?}")),
    ))
}

/// How long `assert_no_surplus_frame` waits for a frame that should NOT arrive.
/// By the time it runs, the in-process server has already emitted every frame
/// the fixture expects, so 250ms is ample slack for a surplus frame in flight
/// over a loopback socket without slowing the suite down per fixture.
const SURPLUS_FRAME_IDLE_TIMEOUT: Duration = Duration::from_millis(250);

/// The other half of `compare_responses`: after a fixture's expected frames have
/// been read, confirms NO further frame arrives. A handler emitting MORE frames
/// than the fixture recorded would otherwise corrupt the NEXT fixture's
/// comparison mid-corpus, and go entirely unnoticed on a corpus's LAST fixture,
/// where `socket.close`/`handle.shutdown` just discard it.
///
/// Generic over the stream so it can be unit-tested against a
/// `futures::stream::iter` with a surplus frame queued, no live socket needed.
async fn assert_no_surplus_frame<S>(
    socket: &mut S,
    fixture_name: &str,
    request_type: &str,
    expected_frame_count: usize,
    idle_timeout: Duration,
) -> Result<(), String>
where
    S: futures::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    match tokio::time::timeout(idle_timeout, socket.next()).await {
        Ok(Some(Ok(frame))) => Err(format!(
            "fixture {fixture_name} (request type {request_type:?}) — Rust emitted a SURPLUS \
             frame beyond the {expected_frame_count} frame(s) the fixture recorded (Rust sent \
             MORE frames than the fixture recorded for this request)\n--- surplus frame ---\n{frame:?}"
        )),
        _ => Ok(()),
    }
}

/// `(message_type, json_pointer)` pairs whose value is IRREDUCIBLY
/// nondeterministic — generated fresh on the recording run and again on the
/// replay run, so it can never match. Each is masked on BOTH the recorded and
/// the replayed frame before the equality check. This is the ONLY divergence
/// mechanism: any other field that differs is a REAL bug to fix in domain code,
/// never to add here.
///
/// A static pointer can only reach a FIXED path, so array- and map-shaped
/// payloads get dedicated walkers instead (`mask_ups_list_frames`,
/// `mask_gps_response_frame`, `compare_get_presets_equivalent`).
const IGNORED_PATHS: &[(&str, &str)] = &[
    // A freshly-created pal's InstanceId is a fresh uuid4. ONLY that field is
    // masked; every other field of the new pal (character_id, nickname,
    // container id, storage_slot, every stat) is still compared strictly.
    // clone_pal/clone_dps_pal answer on these same response types.
    ("add_pal", "/data/pal/instance_id"),
    ("add_dps_pal", "/data/pal/instance_id"),
    // `name` embeds a wall-clock timestamp, and `content` is a base64 zip whose
    // CONTAINER framing (per-entry DOS timestamps, deflate stream) differs even
    // when the saves inside are byte-identical. Neither is a blind skip: the
    // replay loop shape-checks `name`, and decodes both zips to compare the
    // DECOMPRESSED GVAS of every member — see `compare_download_equivalent`.
    ("download_save_file", "/data/0/name"),
    ("download_save_file", "/data/0/content"),
    // The response echoes a server-generated uuid4 preset id.
    ("add_preset", "/data/id"),
    // Every timestamp below is a wall-clock value written by whichever run
    // produced the frame, and each freshly-persisted pal's `instance_id` is a
    // fresh uuid4. Everything else on these frames (character_id, nickname,
    // level, tags, notes, source_*, counts, names, colors) is compared strictly.
    ("add_ups_pal", "/data/created_at"),
    ("add_ups_pal", "/data/updated_at"),
    ("add_ups_pal", "/data/last_accessed_at"),
    ("add_ups_pal", "/data/instance_id"),
    ("update_ups_pal", "/data/pal/updated_at"),
    ("update_ups_pal", "/data/pal/instance_id"),
    ("clone_ups_pal", "/data/cloned_pal/instance_id"),
    // `storage_size_mb` drifts by a few bytes because the two runs' JSON
    // encoders compact-encode the same pal_data slightly differently
    // (float/whitespace) — a serializer artifact, not a data difference.
    ("get_ups_stats", "/data/stats/last_updated"),
    ("get_ups_stats", "/data/stats/storage_size_mb"),
    ("create_ups_collection", "/data/collection/created_at"),
    ("create_ups_collection", "/data/collection/updated_at"),
    ("update_ups_collection", "/data/collection/updated_at"),
    ("create_ups_tag", "/data/tag/created_at"),
    ("create_ups_tag", "/data/tag/updated_at"),
    ("update_ups_tag", "/data/tag/updated_at"),
    // add_gps_pal mints a fresh uuid4 InstanceId, exactly like add_pal above.
    // handle_clone_gps_pal answers on this same wire type, so it is covered too.
    ("add_gps_pal", "/data/pal/instance_id"),
    // Machine-dependent: this echoes whatever Steam workshop install path the
    // host resolved (or "" if none), which another machine cannot reproduce.
    // Nothing else in the servers corpus is masked — every other handler there
    // answers deterministically against a fresh, empty DB.
    ("detect_workshop_dir", "/data/workshop_dir"), // machine-dependent Steam install location
];

/// The UPS list frames carry an ARRAY of records, so their per-element
/// timestamps / instance ids need a walker rather than a fixed pointer.
/// Deterministic siblings (`total_count`/`offset`/`limit`, names, colors,
/// counts) stay strictly compared.
fn mask_ups_list_frames(message_type: &str, value: &mut Value) {
    let (array_field, keys): (&str, &[&str]) = match message_type {
        "get_ups_pals" => (
            "pals",
            &[
                "created_at",
                "updated_at",
                "last_accessed_at",
                "instance_id",
            ],
        ),
        "get_ups_collections" => ("collections", &["created_at", "updated_at"]),
        "get_ups_tags" => ("tags", &["created_at", "updated_at"]),
        _ => return,
    };
    let pointer = format!("/data/{array_field}");
    if let Some(items) = value.pointer_mut(&pointer).and_then(Value::as_array_mut) {
        for item in items.iter_mut() {
            let Some(object) = item.as_object_mut() else {
                continue;
            };
            for key in keys {
                if let Some(field) = object.get_mut(*key) {
                    *field = Value::String("<masked>".to_string());
                }
            }
        }
    }
}

/// `get_gps_response`'s `data` is a slot-keyed MAP of pals, so it needs a walker
/// like `mask_ups_list_frames` above. Only a pal added during the run has a
/// fresh `instance_id`; the rest were read from the same on-disk
/// `GlobalPalStorage.sav` on both runs and already match, so masking every slot
/// unconditionally is a no-op for them and keeps this simple. Every other field
/// (character_id, nickname, stats, the slot key itself) stays strictly compared.
fn mask_gps_response_frame(message_type: &str, value: &mut Value) {
    if message_type != "get_gps_response" {
        return;
    }
    let Some(slots) = value.get_mut("data").and_then(Value::as_object_mut) else {
        return;
    };
    for pal in slots.values_mut() {
        let Some(object) = pal.as_object_mut() else {
            // The no-save / no-gps-file shapes put a string or bool under
            // `data` instead of a pal map -- nothing to mask.
            continue;
        };
        if let Some(field) = object.get_mut("instance_id") {
            *field = Value::String("<masked>".to_string());
        }
    }
}

/// Replaces every masked pointer for `message_type` with a fixed sentinel, in
/// place. A pointer that isn't present in `value` is left alone (the frame may
/// legitimately not carry it, e.g. a `warning` instead of an `add_pal`).
fn mask_ignored_paths(message_type: &str, value: &mut Value) {
    for (masked_type, pointer) in IGNORED_PATHS {
        if *masked_type == message_type {
            if let Some(target) = value.pointer_mut(pointer) {
                *target = Value::String("<masked>".to_string());
            }
        }
    }
    mask_ups_list_frames(message_type, value);
    mask_gps_response_frame(message_type, value);
    // `session_id` and `world_option_present` post-date the fixtures, so they are
    // DROPPED rather than masked: a mask can't reconcile a key that is absent on
    // the recorded side.
    if message_type == "loaded_save_files" {
        if let Some(data) = value.get_mut("data").and_then(Value::as_object_mut) {
            data.remove("session_id");
            data.remove("world_option_present");
        }
    }
}

/// Response index 1 of `save_modded_save`'s gamepass burst names a
/// wall-clock-timestamped backup directory, so it can never match. A mask keyed
/// by response `type` is useless here: every frame in the burst shares the
/// generic `progress_message` type, as do countless deterministic progress lines
/// elsewhere. Keying on (request type, response INDEX) keeps it narrow, and the
/// prefix check means a re-ordered progress sequence fails LOUDLY — the frame
/// simply stops being masked — instead of silently masking the wrong field.
fn mask_gamepass_backup_progress_line(
    request_type: &str,
    response_index: usize,
    value: &mut Value,
) {
    if request_type != "save_modded_save" || response_index != 1 {
        return;
    }
    if value["type"] != "progress_message" {
        return;
    }
    let Some(text) = value["data"].as_str() else {
        return;
    };
    if !text.starts_with("Created backup at: ") {
        return;
    }
    value["data"] = Value::String("<masked>".to_string());
}

/// Splits a `download_save_file` filename `<world>_<YYYYMMDD>_<HHMMSS>.zip` into
/// its world-name prefix, or `None` if it doesn't match that shape. A world name
/// may itself contain `_` or spaces, so this anchors on the trailing
/// `_<8 digits>_<6 digits>.zip` rather than splitting on the first `_`.
fn download_world_prefix(name: &str) -> Option<String> {
    let stem = name.strip_suffix(".zip")?;
    let (rest, hms) = stem.rsplit_once('_')?;
    if hms.len() != 6 || !hms.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (world, ymd) = rest.rsplit_once('_')?;
    if ymd.len() != 8 || !ymd.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if world.is_empty() {
        return None;
    }
    Some(world.to_string())
}

/// Decodes a `download_save_file` frame's base64 zip into `{member name -> raw
/// (still-compressed) sav bytes}`. A `BTreeMap`, so differing member order
/// between the two zips never affects the comparison.
fn decode_download_zip_members(response: &Value) -> std::collections::BTreeMap<String, Vec<u8>> {
    use base64::Engine as _;
    let content = response["data"][0]["content"]
        .as_str()
        .expect("download_save_file content is a base64 string");
    let zip_bytes = base64::engine::general_purpose::STANDARD
        .decode(content)
        .expect("download_save_file content is valid base64");
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).expect("download content is a zip");
    let mut members = std::collections::BTreeMap::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).unwrap();
        let name = entry.name().to_string();
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut bytes).unwrap();
        members.insert(name, bytes);
    }
    members
}

/// Decompresses one `.sav` CONTAINER (PlM/Oodle) to its raw GVAS payload. The
/// container framing is what legitimately differs between the two zips; the
/// GVAS INSIDE is what must match.
fn decompress_sav_container(sav_bytes: &[u8]) -> Vec<u8> {
    psp_core::ue::compression::decompress_save(&mut std::io::Cursor::new(sav_bytes))
        .expect("sav container decompresses to GVAS")
}

/// Decompresses a `.sav` CONTAINER, parses its GVAS, drops
/// `worldSaveData.MapObjectSaveData`, then re-serialises canonically.
///
/// `MapObjectSaveData` is dropped because the recorded saves re-encode that one
/// map's opaque `RawData` blobs non-byte-faithfully: an UNEDITED, zero-edit
/// `world1/Level.sav` from the recording run differs from the on-disk original
/// by 356 bytes, every one of them inside that map. uesave keeps those blobs
/// opaque and byte-faithful, so the map is normalised away to compare the parts
/// both sides agree on. Everything the edit sequence actually touches survives:
/// the pals' `CharacterSaveParameterMap` (property ORDER included — uesave uses
/// an order-preserving `IndexMap`), the guild's `GuildExtraSaveDataMap`, and
/// every `Players/*.sav` (which carry no `worldSaveData` at all).
fn normalized_member_gvas(compressed_sav: &[u8]) -> Vec<u8> {
    use psp_core::ue::{Property, PropertyKey, StructValue};
    let mut save = psp_core::savio::read_sav_bytes(compressed_sav).expect("parse sav container");
    if let Some(Property::Struct(StructValue::Struct(world_save_data))) = save
        .root
        .properties
        .0
        .get_mut(&PropertyKey::from("worldSaveData"))
    {
        world_save_data
            .0
            .shift_remove(&PropertyKey::from("MapObjectSaveData"));
    }
    let recompressed = psp_core::savio::write_sav_bytes(&save).expect("re-serialize sav container");
    psp_core::ue::compression::decompress_save(&mut std::io::Cursor::new(recompressed))
        .expect("decompress re-serialized sav container")
}

/// The real `download_save_file` assertion (the `name`/`content` masks only
/// exist so the strict-equality pass doesn't re-fail on them afterwards):
///  1. both filenames have the `<world>_<YYYYMMDD>_<HHMMSS>.zip` shape and the
///     SAME world-name prefix — only the timestamp may differ;
///  2. both zips carry the same member names; and
///  3. every member's DECOMPRESSED GVAS payload is byte-identical.
///
/// Returns `Err` rather than panicking so both branches are unit-testable; the
/// replay loop turns an `Err` into a panic.
fn compare_download_equivalent(
    fixture_name: &str,
    expected: &Value,
    actual: &Value,
) -> Result<(), String> {
    let expected_name = expected["data"][0]["name"]
        .as_str()
        .ok_or_else(|| format!("{fixture_name}: expected download frame has no data[0].name"))?;
    let actual_name = actual["data"][0]["name"]
        .as_str()
        .ok_or_else(|| format!("{fixture_name}: actual download frame has no data[0].name"))?;
    let expected_world = download_world_prefix(expected_name).ok_or_else(|| {
        format!(
            "{fixture_name}: expected download name {expected_name:?} is not \
             <world>_<YYYYMMDD>_<HHMMSS>.zip"
        )
    })?;
    let actual_world = download_world_prefix(actual_name).ok_or_else(|| {
        format!(
            "{fixture_name}: actual download name {actual_name:?} is not \
             <world>_<YYYYMMDD>_<HHMMSS>.zip"
        )
    })?;
    if expected_world != actual_world {
        return Err(format!(
            "{fixture_name}: download filename world-name prefix differs \
             (only the timestamp may): expected {expected_world:?}, got {actual_world:?}"
        ));
    }

    let expected_members = decode_download_zip_members(expected);
    let actual_members = decode_download_zip_members(actual);
    let expected_names: Vec<&String> = expected_members.keys().collect();
    let actual_names: Vec<&String> = actual_members.keys().collect();
    if expected_names != actual_names {
        return Err(format!(
            "{fixture_name}: download zip members differ: expected {expected_names:?}, \
             got {actual_names:?}"
        ));
    }

    for (name, expected_sav) in &expected_members {
        let actual_sav = actual_members
            .get(name)
            .expect("member key present in both (checked above)");
        let expected_gvas = normalized_member_gvas(expected_sav);
        let actual_gvas = normalized_member_gvas(actual_sav);
        if expected_gvas != actual_gvas {
            let first_diff = expected_gvas
                .iter()
                .zip(actual_gvas.iter())
                .position(|(a, b)| a != b);
            return Err(format!(
                "{fixture_name}: normalised GVAS of zip member {name:?} differs between \
                 the recorded fixture (capture) and Rust (replay) — expected {} bytes, got {} \
                 bytes, first differing byte at offset {:?}. (MapObjectSaveData is already \
                 excluded — see normalized_member_gvas.) This is a REAL edit divergence, NOT a \
                 maskable field.",
                expected_gvas.len(),
                actual_gvas.len(),
                first_diff,
            ));
        }
    }
    Ok(())
}

/// Message types whose `data` is a raw GVAS serialization: the recorded frames
/// and the replayed ones express the SAME save subtree in two different,
/// non-comparable JSON dialects. These are compared structurally instead of
/// value-exactly — see `compare_raw_data_structural`.
const STRUCTURAL_TYPES: &[&str] = &["get_raw_data"];

/// Sentinel substituted for a structurally-compared `data` field, so the
/// strict-equality pass afterwards doesn't re-fail on the two necessarily
/// different payloads.
const MASKED_STRUCTURAL_DATA: &str = "<structural>";

/// Structural comparator for `STRUCTURAL_TYPES`. Errs when either side's
/// `data` isn't a JSON object, or when the replayed side is empty though the
/// recorded one was not — proof on the wire that a target failed to resolve.
/// Contents beyond that are deliberately UNCHECKED (the two dialects differ).
fn compare_raw_data_structural(
    fixture_name: &str,
    expected: &Value,
    actual: &Value,
) -> Result<(), String> {
    let expected_data = expected["data"].as_object().ok_or_else(|| {
        format!("{fixture_name}: expected get_raw_data frame's data is not a JSON object")
    })?;
    let actual_data = actual["data"].as_object().ok_or_else(|| {
        format!("{fixture_name}: actual get_raw_data frame's data is not a JSON object")
    })?;
    if !expected_data.is_empty() && actual_data.is_empty() {
        return Err(format!(
            "{fixture_name}: expected get_raw_data data was non-empty ({} key(s)) but the \
             actual (Rust) data is empty -- the Rust handler failed to resolve a target \
             the fixture recorded as resolved",
            expected_data.len()
        ));
    }
    Ok(())
}

/// Sentinel substituted for a server-generated preset uuid.
const MASKED_PRESET_ID: &str = "<masked>";

/// Masks the uuids a preset gets minted fresh on each run — its own `id`, its
/// `pal_preset_id`, and the nested `pal_preset.id`. Everything else (`name`,
/// `type`, every container, every other `pal_preset` field) is real content and
/// still compares strictly.
///
/// CRUCIALLY, only a NON-NULL STRING `pal_preset_id` is masked. `null` there
/// means "no pal_preset association" — a real, comparable fact — so masking it
/// too would collapse "has an association" and "has none" into the same
/// sentinel and let that divergence compare equal.
fn mask_preset_ids(preset: &mut Value) {
    let Some(object) = preset.as_object_mut() else {
        return;
    };
    if let Some(id) = object.get_mut("id") {
        if id.is_string() {
            *id = Value::String(MASKED_PRESET_ID.to_string());
        }
    }
    if let Some(pal_preset_id) = object.get_mut("pal_preset_id") {
        if pal_preset_id.is_string() {
            *pal_preset_id = Value::String(MASKED_PRESET_ID.to_string());
        }
    }
    if let Some(pal_preset) = object.get_mut("pal_preset").and_then(Value::as_object_mut) {
        if let Some(nested_id) = pal_preset.get_mut("id") {
            if nested_id.is_string() {
                *nested_id = Value::String(MASKED_PRESET_ID.to_string());
            }
        }
    }
}

/// Ordered, id-masked preset VALUES from a `get_presets` frame. The dict KEYS
/// are dropped: they are the same nondeterministic uuids as the ids inside each
/// preset. Order is meaningful and preserved — `serde_json::Map` is built with
/// `preserve_order`, and presets are always listed `ORDER BY rowid`.
fn masked_preset_values(frame: &Value) -> Result<Vec<Value>, String> {
    let object = frame["data"]
        .as_object()
        .ok_or_else(|| "get_presets frame has no object `data`".to_string())?;
    Ok(object
        .values()
        .cloned()
        .map(|mut preset| {
            mask_preset_ids(&mut preset);
            preset
        })
        .collect())
}

/// `get_presets`' `data` is a dict keyed by the same server-generated uuids its
/// values carry, which no fixed JSON pointer can mask. So: mask every preset's
/// ids, then compare the two ORDERED lists of masked presets, keys ignored.
fn compare_get_presets_equivalent(
    fixture_name: &str,
    expected: &Value,
    actual: &Value,
) -> Result<(), String> {
    let expected_presets = masked_preset_values(expected)
        .map_err(|message| format!("{fixture_name}: expected {message}"))?;
    let actual_presets = masked_preset_values(actual)
        .map_err(|message| format!("{fixture_name}: actual {message}"))?;
    if expected_presets == actual_presets {
        return Ok(());
    }
    Err(format!(
        "fixture {fixture_name} (request type \"get_presets\") — preset list mismatch after \
         masking server-generated ids (dict keys are ignored by design; only the ordered, \
         id-masked preset values are compared)\n\
         --- actual ---\n{}\n--- expected ---\n{}",
        serde_json::to_string_pretty(&actual_presets)
            .unwrap_or_else(|_| format!("{actual_presets:?}")),
        serde_json::to_string_pretty(&expected_presets)
            .unwrap_or_else(|_| format!("{expected_presets:?}")),
    ))
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
        data_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data"),
        db_path: temp_dir.path().join("contract.db"),
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

        let (mut socket, _) = connect_async(format!("ws://{}/ws/contract-replay", handle.addr))
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
            for (response_index, expected_frame) in expected_responses.iter().enumerate() {
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
                // The deep checks below all run on the UNMASKED pair, BEFORE the
                // masking pass blanks the fields they inspect.
                if response_message_type == "download_save_file" {
                    if let Err(message) = compare_download_equivalent(
                        &fixture_path.display().to_string(),
                        expected_frame,
                        &value,
                    ) {
                        panic!("{message}");
                    }
                } else if response_message_type == "get_presets" {
                    // Both sides' `data` is then blanked to a shared sentinel so
                    // the strict-equality pass below doesn't re-fail on the
                    // necessarily different raw dict.
                    if let Err(message) = compare_get_presets_equivalent(
                        &fixture_path.display().to_string(),
                        expected_frame,
                        &value,
                    ) {
                        panic!("{message}");
                    }
                    value["data"] = Value::String(MASKED_PRESET_ID.to_string());
                } else if STRUCTURAL_TYPES.contains(&response_message_type.as_str()) {
                    if let Err(message) = compare_raw_data_structural(
                        &fixture_path.display().to_string(),
                        expected_frame,
                        &value,
                    ) {
                        panic!("{message}");
                    }
                    value["data"] = Value::String(MASKED_STRUCTURAL_DATA.to_string());
                }
                mask_ignored_paths(&response_message_type, &mut value);
                mask_gamepass_backup_progress_line(&request_type, response_index, &mut value);
                actual_responses.push(value);
            }

            if let Err(message) = assert_no_surplus_frame(
                &mut socket,
                &fixture_path.display().to_string(),
                &request_type,
                expected_responses.len(),
                SURPLUS_FRAME_IDLE_TIMEOUT,
            )
            .await
            {
                panic!("{message}");
            }

            let mut expected = expected_responses;
            for (response_index, value) in expected.iter_mut().enumerate() {
                let response_message_type =
                    value["type"].as_str().unwrap_or(&request_type).to_string();
                if response_message_type == "get_presets" {
                    value["data"] = Value::String(MASKED_PRESET_ID.to_string());
                } else if STRUCTURAL_TYPES.contains(&response_message_type.as_str()) {
                    value["data"] = Value::String(MASKED_STRUCTURAL_DATA.to_string());
                }
                mask_ignored_paths(&response_message_type, value);
                mask_gamepass_backup_progress_line(&request_type, response_index, value);
            }
            if let Err(message) = compare_responses(
                &fixture_path.display().to_string(),
                &request_type,
                &actual_responses,
                &expected,
            ) {
                panic!("{message}");
            }
            fixtures_replayed += 1;
        }
        socket.close(None).await.ok();
    }
    handle.shutdown().await;
    fixtures_replayed
}

/// Replays every committed wire fixture and fails on any deviation: a
/// regression net for the request/response protocol the frontend consumes.
#[tokio::test]
async fn replay_recorded_wire_fixtures() {
    let fixtures_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../contract/fixtures");
    let fixtures_replayed = replay_all_fixtures(&fixtures_root).await;
    // The golden corpus is committed, so zero replayed fixtures means it went
    // missing -- a hard failure, never a silent green with no coverage.
    assert!(
        fixtures_replayed > 0,
        "no committed wire-contract fixtures replayed from {} -- the golden \
         corpus under contract/fixtures/ must be present (see contract/README.md)",
        fixtures_root.display()
    );
}

/// Proves the harness discriminates at all: with no fixtures on disk, the test
/// above never exercises the comparison logic, so a `replay_all_fixtures` that
/// compared lengths only (or compared as an unordered set) would still pass it.
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

/// The same fixture with the correct expected value must replay clean — the
/// harness is not simply panicking on any synthetic input.
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

/// Fixtures must replay in FILENAME order: `std::fs::read_dir` guarantees no
/// ordering, so dropping the `.sort()` would make replays order-dependent. The
/// "01_" fixture is written to disk BEFORE the "00_" one to tell the two apart,
/// and uses an unregistered (hence silent) message type so it cannot hang.
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

/// Identical order must report success. Calls `compare_responses` directly —
/// the exact function that decides pass/fail — so the rule is pinned even with
/// no multi-frame fixture on disk to drive it through a real socket.
#[test]
fn compare_responses_oks_identical_order() {
    let progress = serde_json::json!({"type": "progress_message", "data": "step 1"});
    let result = serde_json::json!({"type": "loaded_save_files", "data": {}});
    let recorded = vec![progress.clone(), result.clone()];
    let replayed = vec![progress, result];

    assert_eq!(
        compare_responses(
            "fixtures/demo/00_select_save.json",
            "select_save",
            &replayed,
            &recorded
        ),
        Ok(())
    );
}

/// The same two frames SWAPPED must be a mismatch, not accepted because both
/// are individually present — a comparison that sorted or deduped first would
/// go red here. The error must also name the offending fixture and say why.
#[test]
fn compare_responses_errs_on_swapped_order() {
    let progress = serde_json::json!({"type": "progress_message", "data": "step 1"});
    let result = serde_json::json!({"type": "loaded_save_files", "data": {}});
    let recorded = vec![progress.clone(), result.clone()];
    let replayed_but_swapped = vec![result, progress];

    let error_message = compare_responses(
        "fixtures/demo/00_select_save.json",
        "select_save",
        &replayed_but_swapped,
        &recorded,
    )
    .expect_err("same two frames in swapped order must NOT compare equal");

    assert!(
        error_message.contains("fixtures/demo/00_select_save.json"),
        "error must name the offending fixture so a developer can find it \
         without re-running with extra flags; got: {error_message}"
    );
    assert!(
        error_message.contains("response sequence mismatch"),
        "error must explain why the fixture failed, not just that it did; \
         got: {error_message}"
    );
}

/// A stream with one frame queued past what the fixture expected must be
/// reported, naming the fixture and quoting the surplus frame. No live handler
/// emits a surplus frame, so the stream is constructed directly.
#[tokio::test]
async fn assert_no_surplus_frame_errs_when_a_frame_is_already_queued() {
    let surplus = Message::Text(r#"{"type":"get_settings","data":{}}"#.into());
    let mut stream = futures::stream::iter(vec![Ok(surplus)]);

    let error_message = assert_no_surplus_frame(
        &mut stream,
        "fixtures/demo/00_get_settings.json",
        "get_settings",
        0,
        Duration::from_millis(50),
    )
    .await
    .expect_err("a queued surplus frame must be reported, not silently drained");

    assert!(
        error_message.contains("fixtures/demo/00_get_settings.json"),
        "error must name the offending fixture; got: {error_message}"
    );
    assert!(
        error_message.contains("get_settings"),
        "error must include the surplus frame's own content, not just a \
         generic message; got: {error_message}"
    );
}

/// An exhausted stream must be reported clean — the check does not simply fail
/// on any stream it is handed.
#[tokio::test]
async fn assert_no_surplus_frame_oks_an_exhausted_stream() {
    let mut stream =
        futures::stream::iter(Vec::<Result<Message, tokio_tungstenite::tungstenite::Error>>::new());

    let result = assert_no_surplus_frame(
        &mut stream,
        "fixtures/demo/00_get_settings.json",
        "get_settings",
        1,
        Duration::from_millis(50),
    )
    .await;

    assert_eq!(result, Ok(()));
}

/// Recorded non-empty, replayed empty: the handler failed to resolve a target
/// that once resolved.
#[test]
fn compare_raw_data_structural_errs_when_actual_is_empty_but_expected_was_not() {
    let expected = serde_json::json!({"type": "get_raw_data", "data": {"key": "SaveData.Guild"}});
    let actual = serde_json::json!({"type": "get_raw_data", "data": {}});

    let error =
        compare_raw_data_structural("fixtures/tools/005_get_raw_data.json", &expected, &actual)
            .expect_err("an unresolved Rust target the fixture recorded as resolved must be reported");
    assert!(
        error.contains("fixtures/tools/005_get_raw_data.json"),
        "error must name the offending fixture; got: {error}"
    );
    assert!(
        error.contains("failed to resolve"),
        "error must explain why it failed; got: {error}"
    );
}

/// Both sides empty (no save loaded, or no target field set) compares clean.
#[test]
fn compare_raw_data_structural_oks_both_sides_empty() {
    let expected = serde_json::json!({"type": "get_raw_data", "data": {}});
    let actual = serde_json::json!({"type": "get_raw_data", "data": {}});

    assert_eq!(
        compare_raw_data_structural("fixtures/tools/005_get_raw_data.json", &expected, &actual),
        Ok(())
    );
}

/// Both sides non-empty but in COMPLETELY DIFFERENT dialects for the same save
/// subtree still compares clean — this is the whole point of the structural
/// comparator, since plain equality would fail here on content alone.
#[test]
fn compare_raw_data_structural_oks_non_empty_sides_with_differing_content() {
    let expected = serde_json::json!({
        "type": "get_raw_data",
        "data": {"GroupType": "EPalGroupType::Guild", "group_name": "The Guild"}
    });
    let actual = serde_json::json!({
        "type": "get_raw_data",
        "data": {"key": {"struct": {}}, "value": {"struct": {"RawData": {"struct": {}}}}}
    });

    assert_eq!(
        compare_raw_data_structural("fixtures/tools/005_get_raw_data.json", &expected, &actual),
        Ok(())
    );
}

/// Either side's `data` not even being a JSON object (a malformed fixture,
/// or a response shape this comparator was never meant to see) is reported,
/// not silently treated as "empty".
#[test]
fn compare_raw_data_structural_errs_when_data_is_not_an_object() {
    let expected = serde_json::json!({"type": "get_raw_data", "data": "not-an-object"});
    let actual = serde_json::json!({"type": "get_raw_data", "data": {}});
    let error =
        compare_raw_data_structural("fixtures/tools/005_get_raw_data.json", &expected, &actual)
            .expect_err("a non-object expected data must be reported");
    assert!(error.contains("not a JSON object"), "got: {error}");

    let expected = serde_json::json!({"type": "get_raw_data", "data": {}});
    let actual = serde_json::json!({"type": "get_raw_data", "data": [1, 2, 3]});
    let error =
        compare_raw_data_structural("fixtures/tools/005_get_raw_data.json", &expected, &actual)
            .expect_err("a non-object actual data must be reported");
    assert!(error.contains("not a JSON object"), "got: {error}");
}

/// `mask_ignored_paths` must replace EXACTLY the listed pointers and nothing
/// else: a mask that widened (blanking the whole `pal`, say, or `character_id`)
/// would swallow real divergences, so the "unchanged" assertions here guard it.
#[test]
fn mask_ignored_paths_masks_only_the_listed_pointers() {
    let mut add_pal = serde_json::json!({
        "type": "add_pal",
        "data": {
            "player_id": "11111111-1111-1111-1111-111111111111",
            "pal": {
                "instance_id": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
                "character_id": "SheepBall",
                "nickname": "parity",
                "storage_slot": 3,
                "hp": 12345,
                "owner_uid": "11111111-1111-1111-1111-111111111111"
            }
        }
    });
    mask_ignored_paths("add_pal", &mut add_pal);
    assert_eq!(add_pal["data"]["pal"]["instance_id"], "<masked>");
    assert_eq!(add_pal["data"]["pal"]["character_id"], "SheepBall");
    assert_eq!(add_pal["data"]["pal"]["nickname"], "parity");
    assert_eq!(add_pal["data"]["pal"]["storage_slot"], 3);
    assert_eq!(add_pal["data"]["pal"]["hp"], 12345);
    assert_eq!(
        add_pal["data"]["pal"]["owner_uid"],
        "11111111-1111-1111-1111-111111111111"
    );
    assert_eq!(
        add_pal["data"]["player_id"], "11111111-1111-1111-1111-111111111111",
        "player_id is deterministic and must NOT be masked"
    );

    let mut add_gps_pal = serde_json::json!({
        "type": "add_gps_pal",
        "data": {
            "pal": {
                "instance_id": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
                "character_id": "SheepBall",
                "nickname": "ParitySheep",
                "storage_slot": 0
            },
            "index": 0
        }
    });
    mask_ignored_paths("add_gps_pal", &mut add_gps_pal);
    assert_eq!(add_gps_pal["data"]["pal"]["instance_id"], "<masked>");
    assert_eq!(add_gps_pal["data"]["pal"]["character_id"], "SheepBall");
    assert_eq!(add_gps_pal["data"]["pal"]["nickname"], "ParitySheep");
    assert_eq!(add_gps_pal["data"]["pal"]["storage_slot"], 0);
    assert_eq!(
        add_gps_pal["data"]["index"], 0,
        "index is a deterministic slot number and must NOT be masked"
    );

    let original_get_pals = serde_json::json!({
        "type": "get_pals",
        "data": {"instance_id": "should-not-be-masked"}
    });
    let mut get_pals = original_get_pals.clone();
    mask_ignored_paths("get_pals", &mut get_pals);
    assert_eq!(
        get_pals, original_get_pals,
        "a type with no mask entry must be untouched"
    );

    let mut download = serde_json::json!({
        "type": "download_save_file",
        "data": [{"name": "Parity World_20260101_000000.zip",
                  "content": "QUJD",
                  "extra": "keep-me"}]
    });
    mask_ignored_paths("download_save_file", &mut download);
    assert_eq!(download["data"][0]["name"], "<masked>");
    assert_eq!(download["data"][0]["content"], "<masked>");
    assert_eq!(
        download["data"][0]["extra"], "keep-me",
        "download mask must touch only name + content"
    );
}

/// The backup-line mask must fire ONLY on response index 1 of a
/// `save_modded_save` burst: every other `progress_message` frame — in this
/// burst and in every other corpus — carries deterministic text that must stay
/// compared.
#[test]
fn mask_gamepass_backup_progress_line_masks_only_index_one_of_save_modded_save() {
    let mut backup_line = serde_json::json!({
        "type": "progress_message",
        "data": "Created backup at: backups/gamepass/foo_20260711120000"
    });
    mask_gamepass_backup_progress_line("save_modded_save", 1, &mut backup_line);
    assert_eq!(backup_line["data"], "<masked>");

    let mut creating_backup = serde_json::json!({
        "type": "progress_message",
        "data": "Creating backup of container path..."
    });
    mask_gamepass_backup_progress_line("save_modded_save", 0, &mut creating_backup);
    assert_eq!(
        creating_backup["data"], "Creating backup of container path...",
        "only response index 1 of save_modded_save may be masked"
    );

    // Index 1 of a DIFFERENT request that also emits a progress_message there.
    let mut other_request = serde_json::json!({
        "type": "progress_message",
        "data": "Loading Level.sav..."
    });
    mask_gamepass_backup_progress_line("select_save", 1, &mut other_request);
    assert_eq!(
        other_request["data"], "Loading Level.sav...",
        "the mask must not fire for any request type other than save_modded_save"
    );

    // Index 1 whose text does NOT match the "Created backup at: " prefix, i.e.
    // the burst was reordered: it must stay unmasked so the reordering fails.
    let mut reordered = serde_json::json!({
        "type": "progress_message",
        "data": "Converting modified save to SAV format..."
    });
    mask_gamepass_backup_progress_line("save_modded_save", 1, &mut reordered);
    assert_eq!(
        reordered["data"], "Converting modified save to SAV format...",
        "a non-matching text at index 1 must not be masked, so re-ordering is caught, not hidden"
    );

    // The terminal frame (type "save_modded_save", not "progress_message").
    let mut final_frame = serde_json::json!({
        "type": "save_modded_save",
        "data": "Created modded save"
    });
    mask_gamepass_backup_progress_line("save_modded_save", 5, &mut final_frame);
    assert_eq!(final_frame["data"], "Created modded save");
}

/// Builds a `download_save_file`-shaped frame with a chosen filename and
/// compression method, so the deep comparator can be driven against zips that
/// differ in container framing and timestamp while carrying the same saves.
#[cfg(test)]
fn make_download_frame(
    filename: &str,
    members: &[(&str, &[u8])],
    compression: zip::CompressionMethod,
) -> Value {
    use base64::Engine as _;
    use std::io::Write as _;
    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut cursor);
        let options = zip::write::SimpleFileOptions::default().compression_method(compression);
        for (name, bytes) in members {
            writer.start_file(*name, options).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
    }
    let content = base64::engine::general_purpose::STANDARD.encode(cursor.into_inner());
    serde_json::json!({
        "type": "download_save_file",
        "data": [{"name": filename, "content": content}]
    })
}

#[cfg(test)]
fn world1_sav(file_name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/saves/world1")
        .join(file_name);
    std::fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

/// Identical inner saves must PASS despite a different compression method AND a
/// different filename timestamp — the decompressed GVAS is what is compared.
/// Uses the real committed `world1/Level.sav`, so this runs a genuine PlM/Oodle
/// decode rather than a synthetic stand-in.
#[test]
fn compare_download_equivalent_oks_identical_inner_saves() {
    let level = world1_sav("Level.sav");
    let expected = make_download_frame(
        "Parity World_20260101_000000.zip",
        &[("Level.sav", &level)],
        zip::CompressionMethod::Deflated,
    );
    let actual = make_download_frame(
        "Parity World_20991231_235959.zip",
        &[("Level.sav", &level)],
        zip::CompressionMethod::Stored,
    );

    assert_eq!(
        compare_download_equivalent("fixtures/phase2/00_download.json", &expected, &actual),
        Ok(())
    );
}

/// A member whose DECOMPRESSED GVAS differs by even one byte must FAIL. The two
/// zips carry a same-named `Level.sav` backed by two different, both-decodable
/// containers, so the failure can only come from the GVAS byte comparison — not
/// from a decode error or a member-name mismatch.
#[test]
fn compare_download_equivalent_errs_on_a_differing_inner_save() {
    let level = world1_sav("Level.sav");
    let level_meta = world1_sav("LevelMeta.sav");
    // Guards against a vacuous pass: the two really must decode differently.
    assert_ne!(
        decompress_sav_container(&level),
        decompress_sav_container(&level_meta),
        "fixture precondition: Level.sav and LevelMeta.sav must differ"
    );

    let expected = make_download_frame(
        "Parity World_20260101_000000.zip",
        &[("Level.sav", &level)],
        zip::CompressionMethod::Deflated,
    );
    let actual = make_download_frame(
        "Parity World_20260101_000000.zip",
        &[("Level.sav", &level_meta)],
        zip::CompressionMethod::Deflated,
    );

    let error = compare_download_equivalent("fixtures/phase2/00_download.json", &expected, &actual)
        .expect_err("differing inner Level.sav GVAS must be reported, not tolerated");
    assert!(
        error.contains("Level.sav"),
        "error must name the differing member; got: {error}"
    );
    assert!(
        error.contains("REAL"),
        "error must flag the divergence as real (not maskable); got: {error}"
    );
}

/// A differing member SET (an extra `Players/*.sav` on one side) must also fail.
#[test]
fn compare_download_equivalent_errs_on_a_member_set_mismatch() {
    let level = world1_sav("Level.sav");
    let expected = make_download_frame(
        "Parity World_20260101_000000.zip",
        &[("Level.sav", &level)],
        zip::CompressionMethod::Deflated,
    );
    let actual = make_download_frame(
        "Parity World_20260101_000000.zip",
        &[("Level.sav", &level), ("Players/abc.sav", &level)],
        zip::CompressionMethod::Deflated,
    );

    let error = compare_download_equivalent("fixtures/phase2/00_download.json", &expected, &actual)
        .expect_err("a differing member set must be reported");
    assert!(
        error.contains("members differ"),
        "error must explain the member-set mismatch; got: {error}"
    );
}

/// `download_world_prefix` must accept a world name containing spaces/
/// underscores and reject anything not ending in `_<8 digits>_<6 digits>.zip`.
#[test]
fn download_world_prefix_parses_the_timestamped_shape() {
    assert_eq!(
        download_world_prefix("Parity World_20260710_143012.zip").as_deref(),
        Some("Parity World")
    );
    assert_eq!(
        download_world_prefix("My_Cool_World_20260710_143012.zip").as_deref(),
        Some("My_Cool_World")
    );
    assert_eq!(
        download_world_prefix("PSP_20260710_143012.zip").as_deref(),
        Some("PSP")
    );
    // Bad shapes.
    assert_eq!(download_world_prefix("noextension_20260710_143012"), None);
    assert_eq!(download_world_prefix("World_2026_143012.zip"), None); // 4-digit date
    assert_eq!(download_world_prefix("World_20260710_1430.zip"), None); // 4-digit time
    assert_eq!(download_world_prefix("_20260710_143012.zip"), None); // empty world
}

/// Builds a `get_presets`-shaped frame from `(dict key, preset)` pairs in the
/// given order (`serde_json`'s `preserve_order` feature keeps `Map` insertion
/// order, matching the real `ORDER BY rowid` listing).
#[cfg(test)]
fn make_get_presets_frame(entries: &[(&str, Value)]) -> Value {
    let mut data = serde_json::Map::new();
    for (key, preset) in entries {
        data.insert((*key).to_string(), preset.clone());
    }
    serde_json::json!({ "type": "get_presets", "data": Value::Object(data) })
}

/// `mask_preset_ids` must replace EXACTLY `id`, `pal_preset_id` and the nested
/// `pal_preset.id`, and nothing else: a widened mask (blanking `pal_preset`
/// wholesale, or masking `name`) has to turn this red.
#[test]
fn mask_preset_ids_masks_only_id_fields() {
    let mut preset = serde_json::json!({
        "id": "11111111-1111-1111-1111-111111111111",
        "name": "Melee Kit",
        "type": "inventory",
        "common_container": [{"static_id": "Wood", "count": 999, "slot_index": 0}],
        "pal_preset_id": "22222222-2222-2222-2222-222222222222",
        "pal_preset": {
            "id": "22222222-2222-2222-2222-222222222222",
            "lock": true,
            "character_id": "SheepBall"
        }
    });
    mask_preset_ids(&mut preset);
    assert_eq!(preset["id"], MASKED_PRESET_ID);
    assert_eq!(preset["pal_preset_id"], MASKED_PRESET_ID);
    assert_eq!(preset["pal_preset"]["id"], MASKED_PRESET_ID);
    assert_eq!(preset["name"], "Melee Kit");
    assert_eq!(preset["type"], "inventory");
    assert_eq!(
        preset["common_container"],
        serde_json::json!([{"static_id": "Wood", "count": 999, "slot_index": 0}])
    );
    assert_eq!(preset["pal_preset"]["lock"], true);
    assert_eq!(preset["pal_preset"]["character_id"], "SheepBall");

    // A preset with no pal_preset relationship: its `id` is still masked, but a
    // NULL `pal_preset_id` must survive as null (see mask_preset_ids).
    let mut bare = serde_json::json!({
        "id": "33333333-3333-3333-3333-333333333333",
        "name": "Bare",
        "type": "inventory",
        "pal_preset_id": Value::Null
    });
    mask_preset_ids(&mut bare);
    assert_eq!(bare["id"], MASKED_PRESET_ID);
    assert_eq!(
        bare["pal_preset_id"],
        Value::Null,
        "a null pal_preset_id (no association) must be preserved, not masked"
    );
    assert_eq!(bare["name"], "Bare");
}

/// Different uuid dict keys and different id values, but identical preset
/// content in the same order, must compare EQUAL — plain equality on the raw
/// dicts would fail on the keys alone.
#[test]
fn compare_get_presets_equivalent_oks_different_uuids_same_content() {
    let expected = make_get_presets_frame(&[
        (
            "aaaaaaaa-0000-0000-0000-000000000000",
            serde_json::json!({
                "id": "aaaaaaaa-0000-0000-0000-000000000000",
                "name": "Kit",
                "type": "inventory",
                "pal_preset_id": Value::Null
            }),
        ),
        (
            "bbbbbbbb-0000-0000-0000-000000000000",
            serde_json::json!({
                "id": "bbbbbbbb-0000-0000-0000-000000000000",
                "name": "Melee",
                "type": "pal",
                "pal_preset_id": "cccccccc-0000-0000-0000-000000000000",
                "pal_preset": {
                    "id": "cccccccc-0000-0000-0000-000000000000",
                    "lock": true,
                    "character_id": "SheepBall"
                }
            }),
        ),
    ]);
    // Same presets, same order, but every uuid is a different random value.
    let actual = make_get_presets_frame(&[
        (
            "11111111-9999-9999-9999-999999999999",
            serde_json::json!({
                "id": "11111111-9999-9999-9999-999999999999",
                "name": "Kit",
                "type": "inventory",
                "pal_preset_id": Value::Null
            }),
        ),
        (
            "22222222-9999-9999-9999-999999999999",
            serde_json::json!({
                "id": "22222222-9999-9999-9999-999999999999",
                "name": "Melee",
                "type": "pal",
                "pal_preset_id": "33333333-9999-9999-9999-999999999999",
                "pal_preset": {
                    "id": "33333333-9999-9999-9999-999999999999",
                    "lock": true,
                    "character_id": "SheepBall"
                }
            }),
        ),
    ]);

    assert_eq!(
        compare_get_presets_equivalent(
            "fixtures/db-presets/002_get_presets.json",
            &expected,
            &actual
        ),
        Ok(())
    );
}

/// Guards the NESTED mask specifically: two presets differing ONLY in
/// `pal_preset.id` must still compare equal. Nothing else here would catch
/// `mask_preset_ids` dropping that step.
#[test]
fn compare_get_presets_equivalent_oks_when_only_nested_pal_preset_id_differs() {
    let expected = make_get_presets_frame(&[(
        "shared-key",
        serde_json::json!({
            "id": "shared-key",
            "name": "Melee",
            "type": "pal",
            "pal_preset_id": "pp-expected",
            "pal_preset": {"id": "pp-expected", "lock": true, "character_id": "SheepBall"}
        }),
    )]);
    let actual = make_get_presets_frame(&[(
        "shared-key",
        serde_json::json!({
            "id": "shared-key",
            "name": "Melee",
            "type": "pal",
            "pal_preset_id": "pp-actual",
            "pal_preset": {"id": "pp-actual", "lock": true, "character_id": "SheepBall"}
        }),
    )]);

    assert_eq!(
        compare_get_presets_equivalent(
            "fixtures/db-presets/002_get_presets.json",
            &expected,
            &actual
        ),
        Ok(())
    );
}

/// A REAL field difference (here a container's item count) must NOT compare
/// equal, even with matching masked ids — the comparator is not vacuously
/// permissive.
#[test]
fn compare_get_presets_equivalent_errs_on_a_real_field_difference() {
    let expected = make_get_presets_frame(&[(
        "shared-key",
        serde_json::json!({
            "id": "shared-key",
            "name": "Kit",
            "type": "inventory",
            "pal_preset_id": Value::Null,
            "common_container": [{"static_id": "Wood", "count": 999, "slot_index": 0}]
        }),
    )]);
    let actual = make_get_presets_frame(&[(
        "shared-key",
        serde_json::json!({
            "id": "shared-key",
            "name": "Kit",
            "type": "inventory",
            "pal_preset_id": Value::Null,
            "common_container": [{"static_id": "Wood", "count": 998, "slot_index": 0}]
        }),
    )]);

    let error = compare_get_presets_equivalent(
        "fixtures/db-presets/002_get_presets.json",
        &expected,
        &actual,
    )
    .expect_err("a real (non-id) content divergence must be reported, not tolerated");
    assert!(
        error.contains("fixtures/db-presets/002_get_presets.json"),
        "error must name the offending fixture; got: {error}"
    );
    assert!(
        error.contains("preset list mismatch"),
        "error must explain the mismatch; got: {error}"
    );
}

/// A COUNT mismatch (one side has an extra preset) must fail, not silently
/// truncate to the shorter list.
#[test]
fn compare_get_presets_equivalent_errs_on_a_count_mismatch() {
    let one_preset = make_get_presets_frame(&[(
        "key-a",
        serde_json::json!({"id": "key-a", "name": "Kit", "type": "inventory", "pal_preset_id": Value::Null}),
    )]);
    let two_presets = make_get_presets_frame(&[
        (
            "key-a",
            serde_json::json!({"id": "key-a", "name": "Kit", "type": "inventory", "pal_preset_id": Value::Null}),
        ),
        (
            "key-b",
            serde_json::json!({"id": "key-b", "name": "Extra", "type": "inventory", "pal_preset_id": Value::Null}),
        ),
    ]);

    assert!(compare_get_presets_equivalent(
        "fixtures/db-presets/002_get_presets.json",
        &one_preset,
        &two_presets,
    )
    .is_err());
}

/// Two presets identical in every field except `pal_preset_id` — `null` (no
/// association) on one side, a real uuid on the other — must NOT compare equal.
/// The divergence is deliberately confined to that one field: masking
/// `pal_preset_id` unconditionally would collapse both to the same sentinel and
/// make the two presets identical, so this is the only test that catches it.
#[test]
fn compare_get_presets_equivalent_errs_when_pal_preset_association_differs() {
    let without_association = make_get_presets_frame(&[(
        "shared-key",
        serde_json::json!({
            "id": "shared-key",
            "name": "Kit",
            "type": "inventory",
            "pal_preset_id": Value::Null
        }),
    )]);
    let with_association = make_get_presets_frame(&[(
        "shared-key",
        serde_json::json!({
            "id": "shared-key",
            "name": "Kit",
            "type": "inventory",
            "pal_preset_id": "pp-real-uuid"
        }),
    )]);

    let error = compare_get_presets_equivalent(
        "fixtures/db-presets/002_get_presets.json",
        &without_association,
        &with_association,
    )
    .expect_err(
        "a preset with NO pal_preset association (pal_preset_id: null) vs one WITH \
         an association (a real uuid) is a real divergence and must NOT compare equal",
    );
    assert!(
        error.contains("preset list mismatch"),
        "error must explain the mismatch; got: {error}"
    );
}

/// The single-object UPS frames must mask EXACTLY their timestamp /
/// instance-id fields and nothing else. The committed replay corpus does not
/// cover UPS, so these synthetic checks are the standing proof the masking is
/// right.
#[test]
fn mask_ignored_paths_masks_ups_single_object_frames() {
    let mut add = serde_json::json!({
        "type": "add_ups_pal",
        "data": {
            "id": 1, "instance_id": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "character_id": "SheepBall", "nickname": "Fluffy", "level": 12,
            "created_at": "2026-07-10T00:00:00Z", "updated_at": "2026-07-10T00:00:00Z",
            "last_accessed_at": "2026-07-10T00:00:00Z", "tags": ["parity-tag"]
        }
    });
    mask_ignored_paths("add_ups_pal", &mut add);
    assert_eq!(add["data"]["instance_id"], "<masked>");
    assert_eq!(add["data"]["created_at"], "<masked>");
    assert_eq!(add["data"]["updated_at"], "<masked>");
    assert_eq!(add["data"]["last_accessed_at"], "<masked>");
    assert_eq!(add["data"]["character_id"], "SheepBall");
    assert_eq!(add["data"]["nickname"], "Fluffy");
    assert_eq!(add["data"]["level"], 12);
    assert_eq!(add["data"]["tags"], serde_json::json!(["parity-tag"]));
    assert_eq!(add["data"]["id"], 1, "the DB id is deterministic (rowid)");

    let mut update = serde_json::json!({
        "type": "update_ups_pal",
        "data": {"pal": {"id": 1, "instance_id": "bbbb", "nickname": "Rex",
                         "updated_at": "2026-07-10T00:00:00Z"}}
    });
    mask_ignored_paths("update_ups_pal", &mut update);
    assert_eq!(update["data"]["pal"]["instance_id"], "<masked>");
    assert_eq!(update["data"]["pal"]["updated_at"], "<masked>");
    assert_eq!(update["data"]["pal"]["nickname"], "Rex");

    let mut clone = serde_json::json!({
        "type": "clone_ups_pal",
        "data": {"original_pal_id": 1, "cloned_pal": {"id": 2, "instance_id": "cccc",
                 "nickname": "Rex (Clone)"}}
    });
    mask_ignored_paths("clone_ups_pal", &mut clone);
    assert_eq!(clone["data"]["cloned_pal"]["instance_id"], "<masked>");
    assert_eq!(clone["data"]["cloned_pal"]["nickname"], "Rex (Clone)");
    assert_eq!(clone["data"]["original_pal_id"], 1);

    let mut stats = serde_json::json!({
        "type": "get_ups_stats",
        "data": {"stats": {"total_pals": 2, "storage_size_mb": 0.01,
                 "last_updated": "2026-07-10T00:00:00Z"}}
    });
    mask_ignored_paths("get_ups_stats", &mut stats);
    assert_eq!(stats["data"]["stats"]["last_updated"], "<masked>");
    assert_eq!(stats["data"]["stats"]["storage_size_mb"], "<masked>");
    assert_eq!(
        stats["data"]["stats"]["total_pals"], 2,
        "total_pals is a deterministic count and must NOT be masked"
    );

    let mut collection = serde_json::json!({
        "type": "create_ups_collection",
        "data": {"collection": {"id": 1, "name": "Parity Favs",
                 "created_at": "t", "updated_at": "t"}}
    });
    mask_ignored_paths("create_ups_collection", &mut collection);
    assert_eq!(collection["data"]["collection"]["created_at"], "<masked>");
    assert_eq!(collection["data"]["collection"]["updated_at"], "<masked>");
    assert_eq!(collection["data"]["collection"]["name"], "Parity Favs");

    let mut tag = serde_json::json!({
        "type": "create_ups_tag",
        "data": {"tag": {"id": 1, "name": "parity-tag", "created_at": "t", "updated_at": "t"}}
    });
    mask_ignored_paths("create_ups_tag", &mut tag);
    assert_eq!(tag["data"]["tag"]["created_at"], "<masked>");
    assert_eq!(tag["data"]["tag"]["updated_at"], "<masked>");
    assert_eq!(tag["data"]["tag"]["name"], "parity-tag");
}

/// The array-shaped list frames must mask EVERY element's nondeterministic
/// fields while leaving deterministic siblings untouched — both inside each
/// element (character_id, nickname, pal_data, name, color) and beside the array
/// (total_count/offset/limit).
#[test]
fn mask_ups_list_frames_masks_every_element_only() {
    let mut pals = serde_json::json!({
        "type": "get_ups_pals",
        "data": {
            "total_count": 2, "offset": 0, "limit": 30,
            "pals": [
                {"id": 1, "instance_id": "a1", "character_id": "SheepBall",
                 "nickname": "Fluffy", "pal_data": {"x": 1},
                 "created_at": "t", "updated_at": "t", "last_accessed_at": "t"},
                {"id": 2, "instance_id": "a2", "character_id": "Lamball",
                 "nickname": "Woolly", "pal_data": {"y": 2},
                 "created_at": "t2", "updated_at": "t2", "last_accessed_at": "t2"}
            ]
        }
    });
    mask_ignored_paths("get_ups_pals", &mut pals);
    for index in 0..2 {
        assert_eq!(pals["data"]["pals"][index]["instance_id"], "<masked>");
        assert_eq!(pals["data"]["pals"][index]["created_at"], "<masked>");
        assert_eq!(pals["data"]["pals"][index]["updated_at"], "<masked>");
        assert_eq!(pals["data"]["pals"][index]["last_accessed_at"], "<masked>");
    }
    assert_eq!(pals["data"]["pals"][0]["character_id"], "SheepBall");
    assert_eq!(pals["data"]["pals"][1]["character_id"], "Lamball");
    assert_eq!(pals["data"]["pals"][0]["nickname"], "Fluffy");
    assert_eq!(
        pals["data"]["pals"][0]["pal_data"],
        serde_json::json!({"x": 1})
    );
    assert_eq!(pals["data"]["pals"][0]["id"], 1);
    assert_eq!(
        pals["data"]["total_count"], 2,
        "total_count is deterministic and must survive the per-element mask"
    );
    assert_eq!(pals["data"]["offset"], 0);
    assert_eq!(pals["data"]["limit"], 30);

    let mut collections = serde_json::json!({
        "type": "get_ups_collections",
        "data": {"collections": [
            {"id": 1, "name": "Favs", "color": "#f00", "created_at": "t", "updated_at": "t"}
        ]}
    });
    mask_ignored_paths("get_ups_collections", &mut collections);
    assert_eq!(
        collections["data"]["collections"][0]["created_at"],
        "<masked>"
    );
    assert_eq!(
        collections["data"]["collections"][0]["updated_at"],
        "<masked>"
    );
    assert_eq!(collections["data"]["collections"][0]["name"], "Favs");
    assert_eq!(collections["data"]["collections"][0]["color"], "#f00");

    let mut tags = serde_json::json!({
        "type": "get_ups_tags",
        "data": {"tags": [{"id": 1, "name": "t1", "created_at": "t", "updated_at": "t"}]}
    });
    mask_ignored_paths("get_ups_tags", &mut tags);
    assert_eq!(tags["data"]["tags"][0]["created_at"], "<masked>");
    assert_eq!(tags["data"]["tags"][0]["updated_at"], "<masked>");
    assert_eq!(tags["data"]["tags"][0]["name"], "t1");
}

/// Driven through `compare_responses`, the actual pass/fail decision: frames
/// differing ONLY in masked timestamps/instance-ids compare EQUAL, while a real
/// nickname difference still FAILS. The mask must be neither too weak (letting
/// timestamps fail a replay) nor too strong (swallowing real divergences).
#[test]
fn ups_pal_list_masking_is_neither_too_weak_nor_too_strong() {
    let make = |instance: &str, timestamp: &str, nickname: &str| {
        let mut frame = serde_json::json!({
            "type": "get_ups_pals",
            "data": {"total_count": 1, "offset": 0, "limit": 30, "pals": [
                {"id": 1, "instance_id": instance, "character_id": "SheepBall",
                 "nickname": nickname, "created_at": timestamp, "updated_at": timestamp,
                 "last_accessed_at": timestamp}
            ]}
        });
        mask_ignored_paths("get_ups_pals", &mut frame);
        frame
    };

    let captured = vec![make("py-uuid", "2026-07-10T00:00:00Z", "Fluffy")];
    let replayed = vec![make("rs-uuid", "2026-07-10T09:59:59Z", "Fluffy")];
    assert_eq!(
        compare_responses(
            "fixtures/db-ups/00_get_ups_pals.json",
            "get_ups_pals",
            &replayed,
            &captured
        ),
        Ok(())
    );

    let divergent = vec![make("rs-uuid", "2026-07-10T09:59:59Z", "Rex")];
    assert!(
        compare_responses(
            "fixtures/db-ups/00_get_ups_pals.json",
            "get_ups_pals",
            &divergent,
            &captured
        )
        .is_err(),
        "a real nickname difference must NOT be masked away"
    );
}

/// `mask_gps_response_frame` must mask EXACTLY `instance_id` in every pal of the
/// slot-keyed map and leave everything else alone: other pal fields, the slot
/// keys, the no-save / no-gps-file shapes (no pal map to walk), and any other
/// message type. The committed replay corpus does not cover GPS, so these
/// synthetic checks stand in for it.
#[test]
fn mask_gps_response_frame_masks_only_instance_id_per_slot() {
    let mut response = serde_json::json!({
        "type": "get_gps_response",
        "data": {
            "0": {"instance_id": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
                  "character_id": "SheepBall", "nickname": "Fluffy", "level": 12},
            "3": {"instance_id": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
                  "character_id": "Lamball", "nickname": "Woolly", "level": 5}
        }
    });
    mask_gps_response_frame("get_gps_response", &mut response);
    assert_eq!(response["data"]["0"]["instance_id"], "<masked>");
    assert_eq!(response["data"]["3"]["instance_id"], "<masked>");
    assert_eq!(response["data"]["0"]["character_id"], "SheepBall");
    assert_eq!(response["data"]["0"]["nickname"], "Fluffy");
    assert_eq!(response["data"]["0"]["level"], 12);
    assert_eq!(response["data"]["3"]["character_id"], "Lamball");
    assert_eq!(response["data"]["3"]["nickname"], "Woolly");

    // The no-save-loaded shape has no pal map: a no-op, not a panic.
    let mut no_save = serde_json::json!({
        "type": "get_gps_response",
        "data": {"error": "No save file loaded"}
    });
    let original_no_save = no_save.clone();
    mask_gps_response_frame("get_gps_response", &mut no_save);
    assert_eq!(
        no_save, original_no_save,
        "the no-save error shape has no pal map to mask"
    );

    let mut unavailable = serde_json::json!({
        "type": "get_gps_response",
        "data": {"available": false, "message": "No GPS file available for this save"}
    });
    let original_unavailable = unavailable.clone();
    mask_gps_response_frame("get_gps_response", &mut unavailable);
    assert_eq!(
        unavailable, original_unavailable,
        "the unavailable shape has no pal map to mask"
    );

    // A different type whose shape (a `pal` with an `instance_id`) looks
    // superficially similar must be left completely untouched.
    let mut other = serde_json::json!({
        "type": "add_gps_pal",
        "data": {"pal": {"instance_id": "should-not-be-masked"}, "index": 0}
    });
    let original_other = other.clone();
    mask_gps_response_frame("add_gps_pal", &mut other);
    assert_eq!(
        other, original_other,
        "mask_gps_response_frame must only touch get_gps_response frames"
    );
}

/// Driven through `mask_ignored_paths` — the entry point the replay loop calls,
/// not `mask_gps_response_frame` directly — proving the walker is actually
/// wired in and not merely callable.
#[test]
fn mask_ignored_paths_masks_gps_response_map_by_slot() {
    let mut captured = serde_json::json!({
        "type": "get_gps_response",
        "data": {
            "0": {"instance_id": "py-uuid-0", "character_id": "SheepBall", "nickname": "Fluffy"},
            "3": {"instance_id": "py-uuid-3", "character_id": "Lamball", "nickname": "Woolly"}
        }
    });
    let mut replayed = serde_json::json!({
        "type": "get_gps_response",
        "data": {
            "0": {"instance_id": "rs-uuid-0", "character_id": "SheepBall", "nickname": "Fluffy"},
            "3": {"instance_id": "rs-uuid-3", "character_id": "Lamball", "nickname": "Woolly"}
        }
    });
    mask_ignored_paths("get_gps_response", &mut captured);
    mask_ignored_paths("get_gps_response", &mut replayed);
    assert_eq!(
        captured, replayed,
        "identical GPS content at each slot must compare equal once each \
         slot's instance_id is masked"
    );
}

/// The GPS walker must be neither too weak (letting a genuinely different pal
/// pass) nor too strong (blanking the whole pal object along with its id), so a
/// real nickname difference must still fail through `compare_responses`.
#[test]
fn gps_response_masking_is_neither_too_weak_nor_too_strong() {
    let make = |slot_zero_instance_id: &str, nickname: &str| {
        let mut frame = serde_json::json!({
            "type": "get_gps_response",
            "data": {
                "0": {"instance_id": slot_zero_instance_id, "character_id": "SheepBall",
                      "nickname": nickname}
            }
        });
        mask_ignored_paths("get_gps_response", &mut frame);
        frame
    };

    let captured = vec![make("py-uuid", "ParitySheep")];
    let replayed = vec![make("rs-uuid", "ParitySheep")];
    assert_eq!(
        compare_responses(
            "fixtures/gps/005_request_gps.json",
            "get_gps_response",
            &replayed,
            &captured
        ),
        Ok(())
    );

    let divergent = vec![make("rs-uuid", "NotParitySheep")];
    assert!(
        compare_responses(
            "fixtures/gps/005_request_gps.json",
            "get_gps_response",
            &divergent,
            &captured
        )
        .is_err(),
        "a real nickname difference must NOT be masked away"
    );
}
