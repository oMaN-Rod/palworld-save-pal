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

/// The load-bearing comparison at the heart of the parity harness (see
/// `rust/parity/README.md`): are `actual` and `expected` the SAME sequence,
/// in the SAME order? `replay_all_fixtures` below calls this — and nothing
/// else decides pass/fail for a fixture — so a regression that made this
/// function set-based (sorted/deduped before comparing) instead of
/// order-based would fail the `compare_responses_*` unit tests even with no
/// live multi-frame fixture to exercise it end-to-end.
///
/// `[Value]`/`Vec<Value>` equality is already index-wise (order- and
/// length-sensitive) — this function doesn't add that property, it just
/// makes it a named, directly-testable one rather than an inline
/// `assert_eq!` that only a live replay could ever reach.
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

/// How long `assert_no_surplus_frame` waits for a frame that should NOT
/// arrive before declaring the burst well and truly over. Mirrors the
/// purpose of `scripts/capture_parity.py`'s `IDLE_SECONDS` (2.0) — "this much
/// silence means the response burst ended" — but is much shorter: capture's
/// timeout has to tolerate Python doing real work (game-data loads, DB I/O)
/// before each frame it emits, whereas this check races an in-process Rust
/// server over a loopback socket that has, by construction, ALREADY finished
/// emitting every frame the fixture expects (they were read out one `send`
/// ahead of this check running). 250ms is generous slack for a surplus frame
/// already in flight without materially slowing the suite down per fixture.
const SURPLUS_FRAME_IDLE_TIMEOUT: Duration = Duration::from_millis(250);

/// The other half of `compare_responses`: after a fixture's expected frames
/// have been read, confirms NO further frame arrives within `idle_timeout`.
/// Reading exactly `expected_frame_count` frames and declaring victory (what
/// `replay_all_fixtures` used to do, unconditionally) is blind to a Rust
/// handler that emits MORE frames than Python recorded: mid-corpus, the
/// surplus frame sits in the socket buffer and corrupts the NEXT fixture's
/// comparison (caught, but misattributed to the wrong fixture); on a
/// corpus's LAST fixture, `socket.close`/`handle.shutdown` simply discard it
/// and nothing ever notices.
///
/// Generic over the stream type — rather than the concrete
/// `tokio_tungstenite::WebSocketStream` `replay_all_fixtures` uses — purely so
/// this can be unit-tested against a `futures::stream::iter` with a surplus
/// frame already queued, with no live socket or server required. See
/// `assert_no_surplus_frame_errs_when_a_frame_is_already_queued` below.
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
             MORE frames than Python did for this request)\n--- surplus frame ---\n{frame:?}"
        )),
        _ => Ok(()),
    }
}

/// Justified, documented parity divergences as `(message_type, json_pointer)`
/// pairs whose value is IRREDUCIBLY nondeterministic (generated independently by
/// Python at capture time and Rust at replay time). Each is masked in BOTH the
/// expected and the actual frame before the equality check. See
/// `rust/parity/README.md` ("Determinism policy") for the per-entry
/// justification. This is the ONLY divergence mechanism: any other field that
/// differs is a REAL bug to fix in domain code, never to add here.
///
/// Phases 0–1 kept this EMPTY. Phase 2 is the first phase with genuinely
/// nondeterministic outputs, adding four masks (the two `instance_id` entries
/// plus `download_save_file`'s `name`/`content`); Phase 3 added a fifth,
/// `add_preset:/data/id`, for a total of five. `get_presets`' ids are
/// deliberately NOT masked here — a static JSON pointer can't reach a dynamic
/// dict key, so it uses `compare_get_presets_equivalent` instead.
const PARITY_IGNORED_PATHS: &[(&str, &str)] = &[
    // A freshly-created pal's InstanceId is a `uuid4` minted INDEPENDENTLY by
    // Python (capture) and Rust (replay) — it can never match. ONLY this one
    // field is masked; every other field of the new pal (character_id,
    // nickname, container id, storage_slot, every stat) is still compared
    // strictly. `clone_pal`/`clone_dps_pal` answer on the `add_pal`/
    // `add_dps_pal` response types, so these two entries cover them too.
    ("add_pal", "/data/pal/instance_id"),
    ("add_dps_pal", "/data/pal/instance_id"),
    // download_save_file: `name` embeds a `Local::now()` timestamp, and
    // `content` is a base64 zip whose CONTAINER (per-entry DOS timestamps +
    // Python `zipfile` vs Rust `zip` deflate streams) differs even when the
    // saves inside are byte-identical. Neither is a blind skip: the replay
    // loop additionally (a) shape-checks `name` and asserts its world-name
    // prefix matches, and (b) decodes BOTH zips and asserts the DECOMPRESSED
    // GVAS of `Level.sav` and every `Players/*.sav` member is byte-identical
    // (see `compare_download_equivalent`).
    ("download_save_file", "/data/0/name"),
    ("download_save_file", "/data/0/content"),
    // add_preset: the response echoes the server-generated uuid4 preset id —
    // minted INDEPENDENTLY by Python (capture) and Rust (replay), so it can
    // never match. get_presets (the dict-keyed listing that also carries
    // these ids, as both the dict KEYS and each preset's `id`/`pal_preset_id`/
    // `pal_preset.id`) is NOT handled here: a static JSON pointer can't mask
    // a dynamic dict key, so it gets its own comparator — see
    // `compare_get_presets_equivalent` below.
    ("add_preset", "/data/id"),
    // Task 3C-6 (db-ups scenario). Every timestamp below is a wall-clock value
    // written by whichever backend ran (Python at capture, Rust at replay) and
    // can never match; each freshly-persisted pal's `instance_id` is likewise
    // a `uuid4` minted independently. All are SINGLE-OBJECT frames reachable by
    // a fixed JSON pointer; the ARRAY-shaped list frames (`get_ups_pals`,
    // `get_ups_collections`, `get_ups_tags`) are handled by
    // `mask_ups_list_frames` instead (a static pointer can't reach every
    // element). Everything else on these frames (character_id, nickname, level,
    // tags, notes, source_*, counts, names, colors) is still compared strictly.
    //
    // add_ups_pal echoes the whole persisted record.
    ("add_ups_pal", "/data/created_at"),
    ("add_ups_pal", "/data/updated_at"),
    ("add_ups_pal", "/data/last_accessed_at"),
    ("add_ups_pal", "/data/instance_id"),
    // update_ups_pal / clone_ups_pal wrap the pal under `pal`/`cloned_pal`.
    ("update_ups_pal", "/data/pal/updated_at"),
    ("update_ups_pal", "/data/pal/instance_id"),
    ("clone_ups_pal", "/data/cloned_pal/instance_id"),
    // get_ups_stats: `last_updated` is a timestamp; `storage_size_mb` differs
    // by a few bytes because Python orjson vs Rust serde_json compact-encode
    // the same pal_data JSON slightly differently (float/whitespace) — a
    // documented, sub-kilobyte serializer divergence, not a data difference.
    ("get_ups_stats", "/data/stats/last_updated"),
    ("get_ups_stats", "/data/stats/storage_size_mb"),
    // Collections / tags wrap their record under `collection`/`tag`.
    ("create_ups_collection", "/data/collection/created_at"),
    ("create_ups_collection", "/data/collection/updated_at"),
    ("update_ups_collection", "/data/collection/updated_at"),
    ("create_ups_tag", "/data/tag/created_at"),
    ("create_ups_tag", "/data/tag/updated_at"),
    ("update_ups_tag", "/data/tag/updated_at"),
    // Task 3D-3 (gps scenario). add_gps_pal mints a fresh uuid4 InstanceId,
    // INDEPENDENTLY minted by Python (capture) and Rust (replay) exactly like
    // add_pal/add_dps_pal above -- it can never match. handle_clone_gps_pal
    // (rust/psp-server/src/handlers/gps.rs) answers on this same
    // `add_gps_pal` wire type, so this entry covers that path too. Every
    // other field of the new pal (character_id, nickname, storage_slot,
    // every stat) is still compared strictly. get_gps_response's slot-keyed
    // map of GPS pals is NOT handled here -- a static JSON pointer can't
    // reach a dynamic map value -- see mask_gps_response_frame below.
    ("add_gps_pal", "/data/pal/instance_id"),
    // Task P6-14 (servers scenario). detect_workshop_dir's own response IS
    // machine-dependent: it echoes whatever Steam workshop install path the
    // CAPTURING machine resolved (or "" if none was found), which the
    // REPLAYING machine (a different CI/dev box, almost always with no Steam
    // install at all) can never reproduce. Nothing else in the servers
    // corpus is masked -- list_servers/get_server/get_server_stats/
    // toggle_server_mod all answer fully deterministically (empty list /
    // "Server not found") against the fresh, empty psp.db the corpus is
    // captured against.
    ("detect_workshop_dir", "/data/workshop_dir"), // machine-dependent Steam install location
];

/// The db-ups list frames carry an ARRAY of records whose per-element
/// timestamps / instance ids are independently generated (Python at capture,
/// Rust at replay) — a static `PARITY_IGNORED_PATHS` pointer can only reach a
/// FIXED path, never every element of a variable-length array. Mask those
/// per-element fields IN PLACE, leaving deterministic siblings
/// (`total_count`/`offset`/`limit`, names, colors, counts) strictly compared.
/// Called from `mask_ignored_paths` so it runs on both the expected and the
/// actual frame, exactly like the static masks.
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

/// `get_gps_response`'s `data` is a DYNAMIC slot-keyed object (Global Pal
/// Storage's `BTreeMap<i32, PalDto>`, JSON-encoded as `{"0": {pal...}, "3":
/// {pal...}}` -- a map keyed by storage slot, not an array), so -- like
/// `mask_ups_list_frames` above -- a static `PARITY_IGNORED_PATHS` pointer
/// can't reach every value. After `add_gps_pal`, the newly-added pal's
/// `instance_id` is a fresh `uuid4` minted INDEPENDENTLY by Python (capture)
/// and Rust (replay) and can never match. Every OTHER GPS pal in the map was
/// read from the same on-disk `GlobalPalStorage.sav` by both backends, so its
/// `instance_id` is already stable and IDENTICAL between capture and replay
/// -- masking it too is harmless (a no-op on already-equal values), and doing
/// so unconditionally (rather than trying to distinguish "the one pal that's
/// new" from the rest) keeps this walker as simple as `mask_ups_list_frames`.
/// GPS pals carry no DB timestamps (unlike db-ups' `created_at`/etc.), so
/// `instance_id` is the only field this masks; every other field (
/// `character_id`, `nickname`, stats, the slot key itself) stays strictly
/// compared. The `error`/`available: false` no-save / no-gps-file shapes
/// (see `handle_request_gps` in `rust/psp-server/src/handlers/gps.rs`) have
/// no pal map under `data` at all, so this is a no-op for them.
fn mask_gps_response_frame(message_type: &str, value: &mut Value) {
    if message_type != "get_gps_response" {
        return;
    }
    let Some(slots) = value.get_mut("data").and_then(Value::as_object_mut) else {
        return;
    };
    for pal in slots.values_mut() {
        let Some(object) = pal.as_object_mut() else {
            // The error/`available: false` shapes' `data` values are
            // strings/bools, not pal objects -- nothing to mask.
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
    for (masked_type, pointer) in PARITY_IGNORED_PATHS {
        if *masked_type == message_type {
            if let Some(target) = value.pointer_mut(pointer) {
                *target = Value::String("<masked>".to_string());
            }
        }
    }
    // db-ups list frames need per-element masking a fixed pointer can't do.
    mask_ups_list_frames(message_type, value);
    // get_gps_response's slot-keyed map needs the same per-value treatment.
    mask_gps_response_frame(message_type, value);
}

/// gamepass corpus (Task P4-14): `save_modded_save`'s gamepass branch emits
/// SIX response frames -- five `progress_message` frames, then the final
/// `save_modded_save` frame (`local_file_handler.py:88-132` /
/// `save_file.rs::write_modded_gamepass_containers`). Response index 1 is
/// `f"Created backup at: {backup_path}"` -- a wall-clock-timestamped backup
/// directory name, computed independently by Python at capture time and
/// Rust at replay time, so it can never match. This can't be a
/// `PARITY_IGNORED_PATHS` entry keyed by response `type`: EVERY frame in
/// this sequence (including the four fully-deterministic ones) shares the
/// generic `progress_message` wire type, which is also used by countless
/// deterministic progress lines in every OTHER corpus -- a type-wide mask
/// would blank all of those too. Masking by (request type, response INDEX)
/// instead stays narrow to this one frame; the prefix check makes a future
/// re-ordering of the progress sequence fail LOUDLY (the frame silently
/// stops being masked and the aggregate `compare_responses` catches the raw
/// text mismatch) rather than silently masking the wrong field.
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

/// The gamepass corpus's shared tmp state, written once by
/// `scripts/capture_parity.py`'s `build_gamepass_corpus` and read by every
/// replay run: `wgs/` (the LIVE container tree, mutated by `save_modded_save`
/// each time the corpus replays), `wgs-pristine/` (a snapshot taken right
/// after the corpus was built, before any request touched it),
/// `LocalData.sav` (the working copy `unlock_map` zeroes IN PLACE) and
/// `LocalData.sav.pristine` (an untouched copy -- see
/// `rust/parity/README.md`, "gamepass scenario", for why a synthetic graft
/// is used here instead of a real corpus `LocalData.sav`, which neither
/// `world1` nor `world2` has).
fn gamepass_tmp_root() -> PathBuf {
    // Deliberately built WITHOUT a literal ".." component (unlike e.g.
    // `fixtures_root`'s "../parity/fixtures" elsewhere in this file): this
    // exact PathBuf is never compared against a Python-captured string, but
    // keeping it clean costs nothing and avoids ever having to reason about
    // whether a lexically-unnormalized path behaves the same as a clean one
    // for the plain filesystem calls below.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("psp-server crate dir has a parent (rust/)")
        .join("parity")
        .join("tmp")
        .join("gamepass")
}

/// Recursively copies `source` into `destination`, creating `destination`
/// (and every subdirectory) as needed. Used to restore the gamepass corpus's
/// `wgs/` container tree from its `wgs-pristine/` snapshot before every
/// replay run, undoing whatever `save_modded_save` wrote into it last time.
fn copy_dir_recursive(source: &std::path::Path, destination: &std::path::Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let dest_path = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path);
        } else {
            std::fs::copy(entry.path(), &dest_path).unwrap();
        }
    }
}

/// Resets every piece of the gamepass corpus's shared on-disk state that a
/// PREVIOUS replay run (or the capture run itself) may have mutated, so this
/// replay starts from the exact snapshot `build_gamepass_corpus` captured.
/// Returns `false` (and resets nothing) when `rust/parity/tmp/gamepass`
/// doesn't exist -- the normal state on a fresh clone/CI, or before the
/// gamepass corpus has ever been captured locally; the caller treats that
/// exactly like the "no fixtures" skip every other corpus already has.
fn reset_gamepass_corpus_filesystem_state() -> bool {
    let tmp_root = gamepass_tmp_root();
    if !tmp_root.exists() {
        return false;
    }

    let container_dir = tmp_root
        .join("wgs")
        .join("0009000000000000_00000000000000000000000000000000");
    if container_dir.exists() {
        std::fs::remove_dir_all(&container_dir).unwrap();
    }
    let pristine_wgs = tmp_root.join("wgs-pristine");
    if pristine_wgs.exists() {
        copy_dir_recursive(&pristine_wgs, &container_dir);
    }

    // convert_save_format (standalone gamepass->steam) writes fresh output
    // here every replay; clear it so a stale run from a previous replay
    // can't linger (the WS response never echoes these files' bytes, but a
    // clean directory keeps local state honest with what capture produced).
    let steam_out = tmp_root.join("steam-out");
    if steam_out.exists() {
        std::fs::remove_dir_all(&steam_out).unwrap();
    }

    // unlock_map mutates LocalData.sav IN PLACE; restore the untouched graft
    // from the `.pristine` copy `build_gamepass_corpus` wrote alongside it.
    let pristine_local_data = tmp_root.join("LocalData.sav.pristine");
    if pristine_local_data.exists() {
        std::fs::copy(&pristine_local_data, tmp_root.join("LocalData.sav")).unwrap();
    }

    true
}

/// Extracts the gamepass container directory from the FIRST fixture's
/// captured `select_save` request (`data.path` = "<container_dir>/containers.index"),
/// rather than independently re-resolving it on the Rust side. Python's own
/// absolute path -- baked into the fixture at capture time by
/// `build_gamepass_corpus` -- is the ONE string `loaded_save_files.level`
/// (fixture 001, `select_gamepass_save`) must match byte-for-byte, since both
/// backends build that field from `settings.save_dir` joined with a
/// container UUID; recomputing the directory independently here would risk a
/// spurious drive-letter-case or path-separator mismatch on Windows (the
/// exact failure mode the `phase2` scenario's README section warns about for
/// mixed-separator `--save-dir` input). Reading it out of the fixture instead
/// guarantees Rust's `settings.save_dir` is set to the LITERAL string Python
/// used for its own.
fn gamepass_save_dir_from_first_fixture(fixture_paths: &[PathBuf]) -> Option<String> {
    let first = fixture_paths.first()?;
    let fixture: Value = serde_json::from_str(&std::fs::read_to_string(first).ok()?).ok()?;
    let path = fixture["request"]["data"]["path"].as_str()?;
    let (container_dir, _file_name) = path.rsplit_once(['/', '\\'])?;
    Some(container_dir.to_string())
}

/// Splits a `download_save_file` filename `<world>_<YYYYMMDD>_<HHMMSS>.zip`
/// into its world-name prefix, or `None` if it doesn't match that shape. The
/// world name itself may contain `_` or spaces (e.g. `"Parity World"`), so we
/// anchor on the trailing `_<8 digits>_<6 digits>.zip` rather than splitting
/// on the first `_`.
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
/// (still-compressed) sav bytes}`. A `BTreeMap` so member order (which differs
/// between Python's insertion order and Rust's `BTreeMap` iteration) never
/// affects the comparison.
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
/// container framing is exactly what legitimately differs between the two
/// backends' zips; the GVAS INSIDE is what must match.
fn decompress_sav_container(sav_bytes: &[u8]) -> Vec<u8> {
    uesave::compression::decompress_save(&mut std::io::Cursor::new(sav_bytes))
        .expect("sav container decompresses to GVAS")
}

/// Decompresses a `.sav` CONTAINER, parses its GVAS with uesave, drops
/// `worldSaveData.MapObjectSaveData`, then re-serialises canonically and
/// returns the resulting GVAS bytes.
///
/// Why drop `MapObjectSaveData`: Python's `palworld_save_tools` re-encodes that
/// one map's opaque `RawData` blobs NON-byte-faithfully. Proven empirically:
/// an UNEDITED `world1/Level.sav` downloaded from the real Python backend
/// differs from the on-disk original by 356 bytes with ZERO edits, and EVERY
/// differing byte lies inside `MapObjectSaveData` — remove that one map from
/// both and the entire rest of the GVAS is byte-identical. Rust (uesave) keeps
/// those blobs opaque and byte-faithful to the game file (Phase-1 Task 12's
/// resave gate proves `read -> write` is byte-identical), so this is a Python
/// serializer quirk we must normalise away to compare the parts both backends
/// DO agree on — it is NOT a wire-field mask, and Rust is the correct side.
/// See `rust/parity/README.md`, "download_save_file deep check".
///
/// Everything else the edit sequence touches is preserved and still compared
/// byte-for-byte: the pals' `CharacterSaveParameterMap` (property ORDER
/// included — uesave parses into an order-preserving `IndexMap`, so the Task-15
/// `GotWorkSuitabilityAddRankList` reordering fix is covered here), the
/// guild's `GuildExtraSaveDataMap` lab research, and every `Players/*.sav`
/// (which carry no `worldSaveData`, so the removal is a no-op there).
///
/// Caveat: because BOTH sides are re-serialised through uesave's writer here,
/// a hypothetical Python↔Rust divergence that uesave canonicalises to the same
/// bytes would be invisible to this check. That risk is low and deliberately
/// accepted: Task 12 proves uesave's writer is order-preserving and
/// byte-faithful (unedited `read -> write` reproduces the game file exactly),
/// so it does not silently absorb real content differences.
fn normalized_member_gvas(compressed_sav: &[u8]) -> Vec<u8> {
    use uesave::{Property, PropertyKey, StructValue};
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
    uesave::compression::decompress_save(&mut std::io::Cursor::new(recompressed))
        .expect("decompress re-serialized sav container")
}

/// The deep, non-masked half of the `download_save_file` check (the `content`
/// mask is only for the strict-equality pass; the real assertion lives here).
/// Verifies, for the captured (`expected`) vs replayed (`actual`) download
/// frames:
///  1. both filenames have the `<world>_<YYYYMMDD>_<HHMMSS>.zip` shape and the
///     SAME world-name prefix (only the timestamp may differ);
///  2. both zips carry the same set of member names; and
///  3. every member's DECOMPRESSED GVAS payload is byte-identical.
///
/// Returns `Err` (rather than panicking) so both the pass and the fail branch
/// are directly unit-testable, mirroring `compare_responses`. The replay loop
/// turns an `Err` into a panic.
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
                 Python (capture) and Rust (replay) — expected {} bytes, got {} bytes, first \
                 differing byte at offset {:?}. (MapObjectSaveData is already excluded — see \
                 normalized_member_gvas.) This is a REAL edit-parity divergence, NOT a \
                 maskable field.",
                expected_gvas.len(),
                actual_gvas.len(),
                first_diff,
            ));
        }
    }
    Ok(())
}

/// Message types whose `data` payload is legitimately dialect-divergent
/// between the two backends (Contract deviation 6, `rust/parity/README.md`):
/// `get_raw_data` echoes uesave's own JSON serialization of the located save
/// subtree on the Rust side (`psp_core::domain::raw::SaveSession::
/// raw_json_for`), and palworld-save-tools' GVAS-dict form on the Python side
/// (`debug_handler.py`'s `guild.save_data`/`player.save_data`/etc.) — two
/// different, non-comparable JSON shapes for the SAME underlying save data.
/// For a type in this list, `replay_all_fixtures` skips value-exact
/// comparison entirely and instead only asserts (a) same message `type`
/// (implicit — both sides are read from the SAME response index), (b) both
/// `data` are JSON *objects*, and (c) the actual `data` is non-empty whenever
/// the expected (Python-captured) `data` was non-empty — see
/// `compare_raw_data_structural`. Content beyond that is deliberately
/// UNCHECKED.
const PARITY_STRUCTURAL_TYPES: &[&str] = &["get_raw_data"];

/// Sentinel value substituted for a structurally-compared `data` field (see
/// `PARITY_STRUCTURAL_TYPES`), matching `MASKED_PRESET_ID`'s convention —
/// used so the aggregate `compare_responses` strict-equality pass (which
/// only knows plain equality) doesn't re-fail on the two, necessarily
/// different, raw `data` payloads once the real structural check below has
/// already run.
const MASKED_STRUCTURAL_DATA: &str = "<structural>";

/// The structural (not value-exact) comparator for `PARITY_STRUCTURAL_TYPES`.
/// Returns `Err` when either side's `data` isn't a JSON object, or when the
/// actual side is empty while the expected side wasn't (on-the-wire proof
/// that the Rust handler failed to resolve a target Python resolved).
/// Deliberately does NOT compare the two objects' contents — Python and Rust
/// emit different, non-comparable JSON dialects for the same underlying save
/// data (see `PARITY_STRUCTURAL_TYPES`'s own doc comment).
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
             Python resolved",
            expected_data.len()
        ));
    }
    Ok(())
}

/// Sentinel value substituted for a server-generated preset uuid, matching
/// `mask_ignored_paths`'s convention.
const MASKED_PRESET_ID: &str = "<masked>";

/// Masks the server-generated uuid fields inside ONE preset object, in place:
/// the preset's own `id`, its `pal_preset_id`, and (when present) the nested
/// `pal_preset.id`. These are the only uuid-shaped fields
/// `psp_db::presets::add` mints independently on each backend (see
/// `rust/psp-db/src/presets.rs`) — everything else in a preset (`name`,
/// `type`, every container, every other `pal_preset` field) is real content
/// and must still compare strictly.
///
/// CRUCIALLY, only a NON-NULL STRING `pal_preset_id` is masked: `get_all`
/// emits `pal_preset_id: null` when a preset has NO pal_preset association, so
/// masking `null` too would collapse "association present (a real uuid)" and
/// "association absent (null)" to the same sentinel — a genuine cross-backend
/// divergence in whether a preset even HAS a pal_preset would then falsely
/// compare equal. `id` is always a non-null string (a preset's primary key),
/// and a nested `pal_preset.id` only exists when the `pal_preset` object does
/// (where it is likewise always a string), so both are masked unconditionally.
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
        // A null here means "no pal_preset association" — a real, comparable
        // fact, NOT a nondeterministic uuid. Only mask an actual uuid string.
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

/// Ordered, id-masked preset VALUES extracted from a `get_presets` frame's
/// `data` (an object keyed by server-generated uuid). The dict KEYS
/// themselves are intentionally dropped here — they are just as
/// nondeterministic as the ids inside each preset, and only the masked
/// VALUES, in order, are what `compare_get_presets_equivalent` compares.
/// `serde_json::Map` (built with the `preserve_order` feature — see
/// `rust/Cargo.toml`) preserves insertion order, and both backends insert in
/// the same logical order: `ORDER BY rowid` — seed presets from
/// `presets.json` in array order, then any added preset appended
/// (`psp_db::presets::get_all`, `db/ctx/presets.py::get_all_presets`).
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

/// Custom equivalence comparator for `get_presets` frames, mirroring the
/// `compare_download_equivalent` pattern: `get_presets`' `data` is a DICT
/// keyed by server-generated uuids, with each preset's own `id`/
/// `pal_preset_id`/`pal_preset.id` also being those same random uuids. A
/// static `PARITY_IGNORED_PATHS` JSON pointer can only mask a fixed path, not
/// a dynamic dict key, so `get_presets` gets its own comparator instead:
/// mask every preset's ids, then compare the two ORDERED lists of masked
/// preset objects (dict keys ignored, insertion order preserved).
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

        // gamepass corpus (Task P4-14): reset the shared on-disk container
        // tree to the pristine snapshot capture took, and point THIS
        // server's settings.save_dir at the exact directory the fixtures'
        // own captured select_save request already carries — the Rust twin
        // of scripts/capture_parity.py's prepare_gamepass_corpus pre-step
        // (Python does the equivalent write into ITS psp.db before its
        // backend process starts; here it happens once per corpus, right
        // before the corpus's own socket connects). `settings.save_dir` is
        // GLOBAL, shared-server state across every corpus this ONE
        // `start_server` instance replays (unlike the gamepass corpus's own
        // wgs/LocalData.sav files, which are scoped to its own tmp dir) — so
        // `previous_save_dir` remembers what it was before mutating it, and
        // is restored right after this corpus's fixtures finish, below,
        // so it doesn't leak into `static-data`'s (or any other corpus's)
        // `get_settings`/`sync_app_state` fixtures replayed afterward.
        let mut previous_save_dir: Option<String> = None;
        if corpus_dir.file_name().and_then(|name| name.to_str()) == Some("gamepass")
            && reset_gamepass_corpus_filesystem_state()
        {
            if let Some(save_dir) = gamepass_save_dir_from_first_fixture(&fixture_paths) {
                let gamepass_db_pool = psp_db::open(&temp_dir.path().join("parity.db"))
                    .await
                    .expect("open replay server's own sqlite db for gamepass save_dir setup");
                previous_save_dir = Some(
                    psp_db::settings::get_settings(&gamepass_db_pool)
                        .await
                        .expect("read settings.save_dir before overwriting it")
                        .save_dir,
                );
                psp_db::settings::update_save_dir(&gamepass_db_pool, &save_dir)
                    .await
                    .expect("persist gamepass container dir into settings.save_dir");
                gamepass_db_pool.close().await;
            }
        }

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
                // download_save_file's `content`/`name` masks hide only the
                // nondeterministic zip container + timestamp; the REAL check is
                // this deep comparison of the decompressed inner saves, run on
                // the UNMASKED pair before masking blanks those fields out.
                if response_message_type == "download_save_file" {
                    if let Err(message) = compare_download_equivalent(
                        &fixture_path.display().to_string(),
                        expected_frame,
                        &value,
                    ) {
                        panic!("{message}");
                    }
                } else if response_message_type == "get_presets" {
                    // get_presets' `data` dict is keyed by server-generated
                    // uuids that no static PARITY_IGNORED_PATHS pointer can
                    // mask (see compare_get_presets_equivalent). Run the real
                    // check on the UNMASKED pair now, then blank the whole
                    // `data` field on both sides to a shared sentinel so the
                    // aggregate compare_responses below — which only knows
                    // how to do plain equality — doesn't re-fail on the
                    // (necessarily different) raw dict.
                    if let Err(message) = compare_get_presets_equivalent(
                        &fixture_path.display().to_string(),
                        expected_frame,
                        &value,
                    ) {
                        panic!("{message}");
                    }
                    value["data"] = Value::String(MASKED_PRESET_ID.to_string());
                } else if PARITY_STRUCTURAL_TYPES.contains(&response_message_type.as_str()) {
                    // get_raw_data's data is a deliberately non-comparable
                    // JSON dialect between Python and Rust (Contract
                    // deviation 6). Run the real (shape-only) check on the
                    // UNMASKED pair now, then blank both sides to a shared
                    // sentinel so the aggregate compare_responses below
                    // doesn't re-fail on the (necessarily different) raw
                    // objects.
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
                } else if PARITY_STRUCTURAL_TYPES.contains(&response_message_type.as_str()) {
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

        // Restore settings.save_dir (see the comment above where it was
        // overwritten) so the NEXT corpus in this loop sees the same
        // shared-server state it would if the gamepass corpus had never run.
        if let Some(save_dir) = previous_save_dir {
            let restore_pool = psp_db::open(&temp_dir.path().join("parity.db"))
                .await
                .expect("open replay server's own sqlite db to restore settings.save_dir");
            psp_db::settings::update_save_dir(&restore_pool, &save_dir)
                .await
                .expect("restore settings.save_dir after the gamepass corpus");
            restore_pool.close().await;
        }
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

/// Pins the load-bearing multi-response ordering rule described in
/// rust/parity/README.md by calling `compare_responses` directly — the exact
/// function `replay_all_fixtures` uses to decide pass/fail — rather than
/// re-deriving the `Vec<T>: PartialEq` guarantee in isolation. Identical
/// order must report success.
///
/// No live Phase-0 handler emits more than one frame per request (so there's
/// no live multi-frame fixture to replay yet), which is why this test calls
/// the extracted comparison function directly instead of driving it through
/// a real WebSocket round-trip: it pins the same property a live multi-frame
/// test would, against the same code path, without fabricating a handler
/// that doesn't exist.
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

/// Companion to the test above: the identical two frames, swapped in the
/// REPLAYED sequence, must be reported as a mismatch — not silently accepted
/// because both frames are individually present. Asserts on the error
/// content, not merely that an `Err` came back: the message must name the
/// offending fixture (so a developer can find it without `--nocapture`) and
/// explain that this is an ordering/count mismatch, not a vacuous "something
/// differs". If a future refactor made the comparison sort or dedupe before
/// comparing, this test goes red — proved by temporarily inserting a `.sort()`
/// into `compare_responses` during development (see task-14-report.md).
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

/// Proves `assert_no_surplus_frame` actually discriminates: a stream with one
/// more frame already queued past what the fixture expected must be reported
/// as an error, and the error must name the fixture (so a developer can find
/// it without extra flags) and include the surplus frame's own content — not
/// a vacuous "something extra arrived".
///
/// This is the direct unit-test seam for the parity-harness hole described in
/// `replay_all_fixtures`'s doc comment: before `assert_no_surplus_frame`
/// existed, `replay_all_fixtures` read exactly `expected_responses.len()`
/// frames and moved on, so a Rust handler emitting one extra frame for the
/// LAST fixture of a corpus was silently discarded by
/// `socket.close`/`handle.shutdown` and never failed any test. There is no
/// live Phase-0 handler that emits a surplus frame to drive that scenario
/// through a real socket, which is why this test constructs the surplus
/// directly with `futures::stream::iter` instead.
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

/// Companion to the test above: an exhausted stream (nothing queued beyond
/// what was already read) must be reported clean — proves the check isn't
/// just failing unconditionally on any stream it's handed.
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

// ---------------------------------------------------------------------------
// get_raw_data structural comparator (Task 3E-5). Proves
// compare_raw_data_structural discriminates: a Rust-side failure to resolve
// a target Python resolved must be caught, while two non-empty (but
// CONTENT-different — different JSON dialects, see PARITY_STRUCTURAL_TYPES's
// own doc comment) objects must NOT be flagged.
// ---------------------------------------------------------------------------

/// Expected non-empty, actual empty -> the discriminating failure case: Rust
/// failed to resolve a target Python resolved.
#[test]
fn compare_raw_data_structural_errs_when_actual_is_empty_but_expected_was_not() {
    let expected = serde_json::json!({"type": "get_raw_data", "data": {"key": "SaveData.Guild"}});
    let actual = serde_json::json!({"type": "get_raw_data", "data": {}});

    let error =
        compare_raw_data_structural("fixtures/tools/005_get_raw_data.json", &expected, &actual)
            .expect_err("an unresolved Rust target when Python resolved one must be reported");
    assert!(
        error.contains("fixtures/tools/005_get_raw_data.json"),
        "error must name the offending fixture; got: {error}"
    );
    assert!(
        error.contains("failed to resolve"),
        "error must explain why it failed; got: {error}"
    );
}

/// Both sides empty (neither backend resolved a target — e.g. no save
/// loaded, or none of the seven fields set) -> compares clean.
#[test]
fn compare_raw_data_structural_oks_both_sides_empty() {
    let expected = serde_json::json!({"type": "get_raw_data", "data": {}});
    let actual = serde_json::json!({"type": "get_raw_data", "data": {}});

    assert_eq!(
        compare_raw_data_structural("fixtures/tools/005_get_raw_data.json", &expected, &actual),
        Ok(())
    );
}

/// Both sides non-empty but with COMPLETELY DIFFERENT content (Python's
/// GVAS-dict dialect vs. Rust's uesave-serde dialect for the very same
/// underlying save subtree) -> still compares clean. This is the whole point
/// of the structural comparator: a naive `actual == expected` (what
/// `compare_responses` does for every other message type) would fail here on
/// content alone, proving that path is NOT what decides `get_raw_data`
/// fixtures.
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

// ---------------------------------------------------------------------------
// Phase-2 masking + download deep-comparator unit tests.
// ---------------------------------------------------------------------------

/// Proves `mask_ignored_paths` replaces EXACTLY the listed pointers for a
/// message type and touches nothing else. If a future edit widened a mask
/// (e.g. blanked the whole `pal` object, or masked `character_id`), the
/// "unchanged" assertions here go red — the mask must never swallow more than
/// the single nondeterministic field it names.
#[test]
fn mask_ignored_paths_masks_only_the_listed_pointers() {
    // add_pal: only /data/pal/instance_id is masked; every sibling stays.
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

    // add_gps_pal: only /data/pal/instance_id is masked; /data/index (a
    // deterministic slot number, unlike a generated uuid) and every other
    // pal field stay strictly compared.
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

    // A message type with no mask entry is left completely untouched.
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

    // download_save_file: only name + content in data[0] are masked.
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

/// `mask_gamepass_backup_progress_line` masks EXACTLY response index 1 of a
/// `save_modded_save` request's progress burst, and touches nothing else —
/// in particular, it must NOT blank other `progress_message` frames sharing
/// the same wire `type`, since `PARITY_IGNORED_PATHS` (which this sits
/// beside, not inside) has no way to express "only index 1" and a type-wide
/// mask would defeat every other deterministic progress line in every other
/// corpus.
#[test]
fn mask_gamepass_backup_progress_line_masks_only_index_one_of_save_modded_save() {
    // Index 1 of a save_modded_save burst, matching the real text: masked.
    let mut backup_line = serde_json::json!({
        "type": "progress_message",
        "data": "Created backup at: backups/gamepass/foo_20260711120000"
    });
    mask_gamepass_backup_progress_line("save_modded_save", 1, &mut backup_line);
    assert_eq!(backup_line["data"], "<masked>");

    // Index 0 of the SAME request (a different, fully-deterministic
    // progress_message frame): untouched.
    let mut creating_backup = serde_json::json!({
        "type": "progress_message",
        "data": "Creating backup of container path..."
    });
    mask_gamepass_backup_progress_line("save_modded_save", 0, &mut creating_backup);
    assert_eq!(
        creating_backup["data"], "Creating backup of container path...",
        "only response index 1 of save_modded_save may be masked"
    );

    // Index 1 of a DIFFERENT request type that also happens to emit a
    // progress_message at index 1: untouched (the mask is keyed to
    // save_modded_save specifically, not "index 1 of any request").
    let mut other_request = serde_json::json!({
        "type": "progress_message",
        "data": "Loading Level.sav..."
    });
    mask_gamepass_backup_progress_line("select_save", 1, &mut other_request);
    assert_eq!(
        other_request["data"], "Loading Level.sav...",
        "the mask must not fire for any request type other than save_modded_save"
    );

    // Index 1 of save_modded_save whose text does NOT match the expected
    // "Created backup at: " prefix (e.g. the sequence was reordered):
    // untouched, so a reordering fails loudly via compare_responses instead
    // of being silently swallowed by an over-eager index-only mask.
    let mut reordered = serde_json::json!({
        "type": "progress_message",
        "data": "Converting modified save to SAV format..."
    });
    mask_gamepass_backup_progress_line("save_modded_save", 1, &mut reordered);
    assert_eq!(
        reordered["data"], "Converting modified save to SAV format...",
        "a non-matching text at index 1 must not be masked, so re-ordering is caught, not hidden"
    );

    // The final save_modded_save response frame itself (type
    // "save_modded_save", not "progress_message") at whatever index it
    // lands on: untouched, since the type check excludes it.
    let mut final_frame = serde_json::json!({
        "type": "save_modded_save",
        "data": "Created modded save"
    });
    mask_gamepass_backup_progress_line("save_modded_save", 5, &mut final_frame);
    assert_eq!(final_frame["data"], "Created modded save");
}

/// Builds a `download_save_file`-shaped frame from `(member name, raw sav
/// bytes)` pairs, with a chosen filename and compression method — so the deep
/// comparator can be exercised against zips that legitimately differ in
/// container framing (compression) and filename timestamp while carrying
/// identical inner saves.
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
        .join("../../tests/fixtures/saves/world1")
        .join(file_name);
    std::fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

/// The deep download check PASSES when the inner saves are identical even
/// though the zip container (compression method) AND the filename timestamp
/// both differ — proving `content`/`name` are legitimately masked while the
/// decompressed GVAS is what actually gets compared. Uses the real committed
/// `world1/Level.sav` container so `decompress_save` runs a genuine PlM/Oodle
/// decode, not a synthetic stand-in.
#[test]
fn compare_download_equivalent_oks_identical_inner_saves() {
    let level = world1_sav("Level.sav");
    let expected = make_download_frame(
        "Parity World_20260101_000000.zip",
        &[("Level.sav", &level)],
        zip::CompressionMethod::Deflated,
    );
    // Different timestamp AND a different (Stored) container — nothing about the
    // inner Level.sav changed.
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

/// The deep download check FAILS when a zip member's DECOMPRESSED GVAS differs
/// by even one byte. Realised here by giving the two zips a same-named
/// `Level.sav` member backed by two genuinely different sav containers
/// (`world1/Level.sav` vs `world1/LevelMeta.sav`) — both decode successfully,
/// so the failure is proven to come from the GVAS byte comparison, not from a
/// decode error or a member-name mismatch. The error must name the differing
/// member and flag it as a real divergence (not a maskable field).
#[test]
fn compare_download_equivalent_errs_on_a_differing_inner_save() {
    let level = world1_sav("Level.sav");
    let level_meta = world1_sav("LevelMeta.sav");
    // Sanity: the two containers really do decompress to different GVAS, so
    // this test can't pass vacuously.
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

/// A zip whose member SET differs (an extra `Players/*.sav` on one side) is
/// also a failure — the download must carry the same members on both backends.
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

// ---------------------------------------------------------------------------
// get_presets custom comparator (Task 3B-3): the dict is keyed by
// server-generated uuids, and each preset's own `id`/`pal_preset_id`/
// `pal_preset.id` are those same random uuids — no static
// PARITY_IGNORED_PATHS pointer can mask a dynamic dict key, so this gets its
// own equivalence check.
// ---------------------------------------------------------------------------

/// Builds a `get_presets`-shaped frame from `(dict key, preset object)`
/// pairs, preserving the given order (the `preserve_order` `serde_json`
/// feature keeps `Map` insertion order, mirroring both backends' real
/// `ORDER BY rowid` insertion order).
#[cfg(test)]
fn make_get_presets_frame(entries: &[(&str, Value)]) -> Value {
    let mut data = serde_json::Map::new();
    for (key, preset) in entries {
        data.insert((*key).to_string(), preset.clone());
    }
    serde_json::json!({ "type": "get_presets", "data": Value::Object(data) })
}

/// `mask_preset_ids` replaces EXACTLY `id`, `pal_preset_id`, and (nested)
/// `pal_preset.id` and touches nothing else — mirrors
/// `mask_ignored_paths_masks_only_the_listed_pointers`'s shape for the same
/// reason: a future edit that widened the mask (e.g. blanked `pal_preset`
/// wholesale, or masked `name`) must turn this red.
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

    // A preset with no pal_preset relationship at all: its own `id` is still
    // masked, but a NULL `pal_preset_id` must be PRESERVED as null (not
    // collapsed to the sentinel) — "no association" is a real, comparable
    // fact, and masking it would let a present-vs-absent divergence slip
    // through (see mask_preset_ids's doc comment and the discriminating test
    // `compare_get_presets_equivalent_errs_when_pal_preset_association_differs`).
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

/// (a) Two `get_presets` dicts with COMPLETELY DIFFERENT uuid dict keys and
/// different `id`/`pal_preset_id`/`pal_preset.id` values, but otherwise
/// identical preset content in the same order, must compare EQUAL. This is
/// the whole point of the custom comparator: a naive `actual == expected` on
/// the raw dicts (as `compare_responses` does for every other message type)
/// would fail here on the keys alone — proving that path is NOT what decides
/// `get_presets` fixtures.
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
    // Same logical presets, same order, but every uuid (dict key, `id`,
    // `pal_preset_id`, nested `pal_preset.id`) is a DIFFERENT random value —
    // exactly what independently-minted uuid4s from two separate backend
    // runs look like.
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

/// Mutation check for the NESTED mask specifically: two presets whose ONLY
/// difference is `pal_preset.id` (every other field, including the outer
/// `id`/`pal_preset_id`, is IDENTICAL) must still compare equal. If
/// `mask_preset_ids` ever forgot the `pal_preset.id` masking step, this is
/// the test that goes red — `compare_get_presets_equivalent_errs_on_a_real_field_difference`
/// below would NOT catch that regression, since it exercises a different
/// (non-id) field.
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

/// (b) Two `get_presets` dicts that differ in a REAL field (not an id) — here
/// a container's item count — must NOT compare equal, even with matching
/// (masked) ids. Proves the comparator isn't vacuously permissive: it can
/// still fail on genuine content divergence, not just report success no
/// matter what.
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
            // Real divergence: 998 vs 999.
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

/// A preset list mismatch in COUNT (one backend returned an extra preset)
/// must also fail — not silently truncate to the shorter list.
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

/// The discriminating test for the `pal_preset_id`-null bug: two `get_presets`
/// dicts identical in EVERY other field, differing ONLY in `pal_preset_id` —
/// `null` (no pal_preset association) on one side, a real uuid on the other.
/// A genuine cross-backend divergence in whether the preset even HAS a
/// pal_preset. This must be reported as NOT equal.
///
/// This is a true MUTATION CHECK for the fix, precisely because the divergence
/// is confined to `pal_preset_id` and NOTHING else (deliberately no `pal_preset`
/// object on either side, which would give the old code a second, unrelated
/// difference to catch and mask the regression):
///  - OLD `mask_preset_ids` (masking `pal_preset_id` UNCONDITIONALLY): the
///    `null` and the uuid BOTH collapse to `"<masked>"`, the two presets become
///    byte-identical, and the comparator FALSELY reports `Ok(())` — the exact
///    hole the reviewer found.
///  - FIXED `mask_preset_ids` (null preserved): the expected side keeps
///    `pal_preset_id: null`, the actual masks its uuid to `"<masked>"`, the two
///    differ, and the comparator correctly returns `Err`.
///
/// Verified RED→GREEN by hand: temporarily reverting `mask_preset_ids` to the
/// unconditional form makes THIS test fail (`Ok` where `Err` was expected)
/// while every other parity test still passes; the fix turns it green. See
/// task-3B-3-report.md for the captured evidence.
#[test]
fn compare_get_presets_equivalent_errs_when_pal_preset_association_differs() {
    // No association: pal_preset_id is null (no pal_preset object).
    let without_association = make_get_presets_frame(&[(
        "shared-key",
        serde_json::json!({
            "id": "shared-key",
            "name": "Kit",
            "type": "inventory",
            "pal_preset_id": Value::Null
        }),
    )]);
    // Has an association: pal_preset_id is a real uuid. The ONLY difference
    // from `without_association` is this field's value (null vs uuid).
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

// ---------------------------------------------------------------------------
// db-ups masking (Task 3C-6). The db-ups fixtures are gitignored (local-only,
// captured against a fresh psp.db) so the live replay path loud-SKIPs them in
// CI — these synthetic unit tests are the standing proof that the masking is
// correct, mirroring the get_presets comparator's own unit tests above.
// ---------------------------------------------------------------------------

/// The SINGLE-OBJECT db-ups frames mask EXACTLY their nondeterministic
/// timestamp / instance-id fields and nothing else. A future edit that widened
/// any of these (e.g. blanked the whole `pal`/`collection`, or masked
/// `character_id`/`level`) turns this red.
#[test]
fn mask_ignored_paths_masks_ups_single_object_frames() {
    // add_ups_pal echoes the whole record at /data.
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

    // update_ups_pal / clone_ups_pal nest the pal under pal/cloned_pal.
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

    // get_ups_stats: only last_updated + storage_size_mb.
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

    // create_ups_collection / create_ups_tag nest under collection/tag.
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

/// The ARRAY-shaped list frames mask the per-element nondeterministic fields in
/// EVERY element while leaving deterministic siblings — both inside each
/// element (character_id, nickname, pal_data, name, color) AND alongside the
/// array (total_count/offset/limit) — untouched. This is the case a static
/// `PARITY_IGNORED_PATHS` pointer provably cannot handle.
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

    // collections + tags carry only created_at/updated_at per element.
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

/// End-to-end through `compare_responses` (the actual pass/fail decision):
/// two get_ups_pals frames whose ONLY differences are the masked per-element
/// timestamps/instance-ids must compare EQUAL after masking, while a real
/// content difference (a nickname) must still FAIL. Proves the mask is neither
/// too weak (letting timestamps fail replay) nor too strong (swallowing real
/// divergences).
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

    // Same content, different (masked) instance id + timestamps → equal.
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

    // Real content divergence (nickname) survives the mask and must fail.
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

// ---------------------------------------------------------------------------
// get_gps_response masking (Task 3D-3, gps scenario). The gps fixtures need a
// corpus save with a GlobalPalStorage.sav that does not exist in this
// checkout, so the live replay path loud-SKIPs this corpus entirely -- these
// synthetic unit tests are the standing proof the masking is correct,
// mirroring the db-ups list-frame tests above (mask_ups_list_frames_masks_
// every_element_only / ups_pal_list_masking_is_neither_too_weak_nor_too_strong).
// ---------------------------------------------------------------------------

/// `mask_gps_response_frame` masks EXACTLY `instance_id` inside every pal
/// value of `get_gps_response`'s slot-keyed map, and leaves everything else
/// untouched: other pal fields, the slot keys themselves, the no-save/
/// no-gps-file `error`/`available` shapes (which have no pal map to walk),
/// and any OTHER message type entirely.
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

    // The no-save-loaded error shape has no pal map under `data` -- must be a
    // complete no-op, not a panic.
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

    // The no-gps-file-available shape is likewise untouched.
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

    // A different message type must be left completely untouched, even
    // though its own shape (a `pal` object with an `instance_id`) looks
    // superficially similar.
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

/// End-to-end through `mask_ignored_paths` (the actual function the replay
/// loop calls, not `mask_gps_response_frame` directly): two `get_gps_response`
/// maps with DIFFERENT slot keys ("0"/"3" swapped for "1"/"2") and DIFFERENT
/// instance_ids, but identical other pal content in each slot, compare EQUAL
/// after masking -- proving the walker is actually wired into the real
/// masking entry point the replay loop uses, not just directly callable.
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

/// Mutation check: a REAL pal-field difference (nickname) surviving the mask
/// must still fail comparison, through `compare_responses` -- the actual
/// pass/fail decision the replay loop uses, exactly mirroring
/// `ups_pal_list_masking_is_neither_too_weak_nor_too_strong`. Proves the
/// walker is neither too weak (letting a genuinely different GPS pal pass)
/// nor too strong (swallowing real content divergence along with the id).
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

    // Same content, different (masked) instance_id -> equal.
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

    // Real content divergence (nickname) survives the mask and must fail --
    // this is the mutation check: if mask_gps_response_frame ever regressed
    // to blank the WHOLE pal object (not just instance_id), this assertion
    // would go red because "NotParitySheep" would never reach the compare at
    // all.
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
