# Parity fixtures

Capture/replay contract harness (spec §2). Fixtures record the PYTHON
backend's exact wire behavior; `psp-server/tests/parity.rs` replays them
against the Rust server and diffs every response.

## Layout

    fixtures/<corpus>/<nnn>_<message_type>.json

`<nnn>` is the zero-padded (3 digits, so a corpus past 99 requests still
sorts lexicographically) scenario index — replay order is filename order,
over ONE WebSocket connection per corpus.

## Fixture format

    {
      "request":   { "type": "select_save", "data": { ... } },
      "responses": [
        { "type": "progress_message", "data": "Loading Level.sav..." },
        { "type": "loaded_save_files", "data": { ... } }
      ]
    }

- `responses` is ordered; the replay reads exactly `responses.len()` frames
  and asserts JSON equality frame-by-frame (floats included —
  serde_json float_roundtrip matches Python's repr round-trip).
- `progress_message` frames are asserted like everything else: same
  sequence, same strings.
- Tolerated divergences must be listed in `PARITY_IGNORED_PATHS` in
  parity.rs with a justifying comment. The list starts empty.

## Generating (Phase 0: static-data corpus)

1. In the palworld-save-pal repo, make sure `psp.db` is FRESH (back it up
   and delete it), then **warm it before capturing** — see "Known Python
   quirks affecting capture" below for why a straight fresh-DB-then-capture
   run reliably records a broken fixture:
   1. Start the backend once and let it finish creating tables:
      `uv run python psp.py --port 5174`
   2. Stop it (Ctrl+C).
   3. Start it again for the actual capture — same command:
      `uv run python psp.py --port 5174`
2. Do NOT load a save.
3. `uv run --with websockets scripts/capture_parity.py --corpus static-data`
4. Stop the Python backend.
5. `cd rust` then `cargo test -p psp-server --test parity`

Fixtures embed machine-local values (e.g. the default `save_dir`) —
they are gitignored; regenerate locally, never commit.

Later phases add corpora that first `select_save` a corpus save file.

## Known Python quirks affecting capture

**`save_dir: null` on a fresh `psp.db` is a real, 100%-reproducible Python
import-order bug — not a race, and not something Rust should ever be made
to match.**

`palworld_save_pal/state.py:181` constructs the module-level `app_state =
AppState()` at *import time*; `state.py:27`'s field `settings: Settings =
Field(default_factory=Settings)` runs `Settings.__init__` — and therefore
`editor/settings.py:95`'s `_load_settings()` — as part of that same import.
`psp.py:10` (`from palworld_save_pal.ws.manager import ConnectionManager`)
imports `ws/manager.py`, which at `ws/manager.py:14` calls
`create_dispatcher()`, which imports every handler module, which imports
`state.py`. All of that happens at module load, **before** `psp.py`'s
`__main__` block calls `create_db_and_tables()` at `psp.py:76`. So on a
genuinely fresh `psp.db`, `_load_settings`'s `get_settings()` call hits
`sqlite3.OperationalError: no such table: settingsmodel`; the bare
`except Exception` at `editor/settings.py:106` logs a warning and swallows
it; `_save_dir` stays at its `PrivateAttr(default=None)` for the rest of
that process's life. Every `get_settings` / `sync_app_state` response for
that run reports `save_dir: null` — deterministically, every time, on a
fresh DB.

**Symptom:** `"save_dir": null` inside a captured `*_get_settings.json` or
`*_sync_app_state.json` fixture (both wire types share the same
`get_settings`-shaped response payload).

**Fix:** warm the database first (the two-start procedure above) so the
`settingsmodel` table already exists by the time `_load_settings` runs. On
that second start, Python returns its real default save directory — the
same value it returns on every subsequent run against a `psp.db` that
already exists.

**If you see `save_dir: null` in a captured fixture, do not add a
`PARITY_IGNORED_PATHS` mask for it.** `psp_db::settings::default_steam_save_dir()`
is the intended, documented default (ported from `STEAM_ROOT` in
`utils/file_manager.py`) and Rust's value is correct; the `null` is a
Python-side capture-time artifact of an unwarmed database, not a
legitimate behavioral divergence between the two backends. Masking it
would permanently hide a real field instead of fixing the five-minute
capture-procedure problem that produces it.
`scripts/capture_parity.py` also refuses to write a fixture containing a
`save_dir: null` response and exits non-zero with a pointer back to this
section — if you hit that check, warm the DB and re-capture; don't work
around it.

## Three states of `cargo test -p psp-server --test parity`

- **No fixtures generated** (default state — `rust/parity/fixtures/`
  doesn't exist or has no corpus subdirectories, since fixtures are
  gitignored and CI never has them): the test prints
  `SKIPPED: no parity fixtures at <path> — run scripts/capture_parity.py; ...`
  and returns without asserting anything. Cargo still reports the test as
  `ok` (a real 0-fixture run is a legitimate local state, not a failure),
  but the `SKIPPED:` line is visible in a plain `cargo test` run with no
  extra flags — it is written straight to `std::io::stderr()` rather than
  via `eprintln!`, because Rust's test harness captures `eprintln!`/`println!`
  output and only shows it for *failed* tests unless you pass `--nocapture`
  (confirmed empirically: an `eprintln!` version of this note was silently
  swallowed on a passing run; the raw write is not).
- **Fixtures present and matching**: every recorded response, in order,
  round-trips through the live Rust server as identical JSON; the test
  passes with no output.
- **Fixtures present and mismatched**: the extracted `compare_responses`
  function (see below) returns an `Err` naming the offending fixture and
  request type, with the actual and expected response arrays pretty-printed;
  `replay_all_fixtures` turns that into a panic, failing the test.

## `compare_responses` — the actual pass/fail decision

`replay_all_fixtures` delegates every fixture's pass/fail decision to a
single function, `compare_responses(fixture_name, request_type, actual,
expected) -> Result<(), String>`, defined in `parity.rs`. It does a plain
`actual == expected` slice comparison — `[Value]` equality is index-wise
(order- and length-sensitive), not multiset/sorted — so two responses
recorded in one order but replayed in another fail the comparison. Two unit
tests in `parity.rs` call this function directly (not just through a live
replay) to pin that property: `compare_responses_oks_identical_order` and
`compare_responses_errs_on_swapped_order` (the latter asserts on the error
message content — that it names the fixture and explains the mismatch — not
merely that an `Err` came back). If a future change ever made this
comparison sort or dedupe before comparing, these two tests would go red
immediately, independent of whether any live fixture happens to have a
multi-frame response (no Phase-0 handler emits more than one frame per
request, so there is currently no live fixture that could catch this any
other way).

## load_path scenario (Phase 1)

Captures: `select_save` (steam) -> `sync_app_state` -> `load_zip_file`, against
a real corpus save directory (`Level.sav`, optional `LevelMeta.sav`,
`Players/*.sav`).

`scripts/capture_parity.py` now distinguishes the *scenario* (which request
sequence to send -- `--scenario`) from the *corpus* (the output subdirectory
under `rust/parity/fixtures/` -- `--corpus`, defaults to the scenario name).
`load_path` also needs `--save-dir <path>`, the save's directory (i.e.
`Level.sav`'s parent) -- pass an **absolute** path, since it is embedded
verbatim in the captured `select_save` request and read from disk again by
the Rust replay test, which may run from a different working directory than
this script.

1. Start the Python backend against a warmed `psp.db` (see "Known Python
   quirks affecting capture" above -- the two-start procedure applies here
   too, since `sync_app_state` also emits `get_settings`).
2. `uv run --with websockets scripts/capture_parity.py --scenario load_path --save-dir <absolute-path-to-corpus-save-dir> --corpus <corpus-name>`
3. Stop the Python backend.
4. `cd rust` then `cargo test -p psp-server --test parity`

Example, from a checkout with `tests/fixtures/saves/world2/` present:

    uv run --with websockets scripts/capture_parity.py \
        --scenario load_path \
        --save-dir "$(pwd)/tests/fixtures/saves/world2" \
        --corpus steam-1p

- **Use a SMALL corpus save with AT MOST 2 players.** With more than 2
  players, `_extract_players_parallel` (`palworld_save_pal/game/...`,
  dispatched from `_extract_player_summaries`) runs on a `ThreadPoolExecutor`
  and inserts results via `as_completed()` -- so the key/array order of
  `player_summaries`, and therefore `sync_app_state`'s and
  `load_zip_file`'s wire `players`-shaped arrays derived from it, is
  genuinely nondeterministic run-to-run in Python itself. A fixture captured
  from such a save can fail replay even against a second run of the SAME
  Python backend, independent of anything Rust does. `tests/fixtures/saves/world2/`
  (exactly 1 player) is the primary corpus for this reason; `world1` (2
  players) sits exactly at the documented threshold and was not used, to
  keep the corpus unambiguous.
- Do not capture error flows: Python `error` messages carry a `trace` string
  (a formatted traceback) that can never match Rust's. If an error fixture
  is ever genuinely needed, the fix is a narrow
  `"error:/data/trace"`-style `PARITY_IGNORED_PATHS` entry with a one-line
  justification -- not a blanket mask.
- Fixtures derived from personal saves stay untracked (do not commit) --
  same rule as every other corpus.

## phase2 scenario (Phase 2 — edit core)

A single deterministic edit sequence driven over ONE connection against a
FRESH backend. Unlike the fixed/load_path scenarios, it is DYNAMIC: it reads
ids out of the live responses (a non-admin-if-possible player, a guild, an
editable pal's `instance_id`, the player's container ids) and builds each
later request from them — all in `capture_parity.py::capture_phase2`.

Fixture order (000..018), all on one socket:
`select_save` → `get_pals` → `get_pal_summaries` →
`request_player_details` (probes players until one has a pal; the first
with-a-pal player is chosen) → `request_guild_details` → `get_lab_research` →
`heal_pals` → `heal_all_pals` → `set_technology_data` → `update_lab_research`
→ `update_save_file` (edit an EXISTING pal) → `request_player_details`
(edit-then-reread) → `move_pal` → `rename_world` → `download_save_file` →
`add_pal` → `delete_player` → `delete_guild` (deletes LAST).

`download_save_file` is captured BEFORE `add_pal` on purpose: `add_pal` mints
a fresh `uuid4` `InstanceId` independently in Python and Rust, which would
diverge the Level.sav bytes and defeat the download deep-check (below).
Downloading first captures the maximal DETERMINISTIC edited state.

    # Backend must be FRESH (no save/player pre-loaded in Python's GLOBAL
    # app_state) — request_player_details is a lazy first-load whose
    # progress frames must match Rust's per-connection first-load.
    uv run --with websockets scripts/capture_parity.py \
        --scenario phase2 \
        --save-dir "D:\...\tests\fixtures\saves\world1" \
        --corpus phase2

Pass a **native** absolute path (backslashes on Windows). A mixed-separator
`--save-dir` (e.g. from `$(pwd)` in Git Bash) makes Python echo the input
path style in `loaded_save_files.level` while Rust normalises to backslashes
— a spurious `level` mismatch. This is a capture-input hygiene issue, NOT a
mask.

**psp.db safety.** The backend opens the CWD-relative `psp.db` (repo root) at
startup. Before capturing, BACK IT UP (copy `psp.db` → `psp.db.parity-backup`,
record its hash) and RESTORE it byte-identical afterward — a developer's real
`psp.db` (custom `save_dir`) would otherwise be touched. The phase2 sequence
emits NO `get_settings`/`sync_app_state` frame, so the "`save_dir: null` on a
fresh/unwarmed `psp.db`" quirk (see "Known Python quirks affecting capture"
above) does NOT affect phase2 fixtures — no warming is needed for this
scenario, but the backup/restore discipline still is. **The phase2 capture is
READ-ONLY w.r.t. save files** (no `save_modded_save`; `update_save_file`/
`download_save_file`/deletes mutate only in memory), so the committed
`world1` fixture and any real `save_dir` are never written.

### Determinism policy — the ONLY sanctioned masks

Phases 0–1 kept `PARITY_IGNORED_PATHS` EMPTY. Phase 2 introduces the first
irreducibly nondeterministic outputs. `parity.rs` masks EXACTLY these
`(message_type, json_pointer)` pairs and no more (each masked in BOTH the
captured and the replayed frame before the equality check):

- `add_pal:/data/pal/instance_id` and `add_dps_pal:/data/pal/instance_id` —
  a newly-created pal's `InstanceId` is a `uuid4` generated INDEPENDENTLY by
  Python (capture) and Rust (replay); it can never match. ONLY this one field
  is masked; every other field of the new pal (`character_id`, `nickname`,
  container id, `storage_slot`, every stat incl. `hp`/`stomach`) is compared
  strictly. `clone_pal`/`clone_dps_pal` answer on the `add_pal`/`add_dps_pal`
  types, so these two entries cover them too.
- `download_save_file:/data/0/name` — the filename embeds a `Local::now()`
  timestamp. NOT a blind skip: the replay ALSO asserts `name` matches
  `^<world>_\d{8}_\d{6}\.zip$` and that its world-name PREFIX equals the
  capture's (only the timestamp may differ).
- `download_save_file:/data/0/content` — the base64 zip CONTAINER (per-entry
  DOS timestamps + Python `zipfile` vs Rust `zip` deflate streams) differs
  even when the saves inside are identical. NOT a blind skip: see the deep
  check below.

Any OTHER field that differs is a REAL bug, fixed in domain code — never by
widening a mask. The Task-15 live replay caught four such bugs this way
(container-slot `local_id` nil-uuid-vs-null; a `bossTechnologyPoint` write
schema gap in `set_technology_data`; a `GotWorkSuitabilityAddRankList`
property-reorder in `apply_pal_dto`; and a new-pal `hp`/`stomach` divergence).

### download_save_file deep check

For the masked `content`, the replay decodes BOTH base64 zips, confirms the
same member set, and for EVERY member (`Level.sav` + each `Players/*.sav`)
asserts the DECOMPRESSED GVAS is byte-identical — after one normalisation:
`worldSaveData.MapObjectSaveData` is removed from both sides first
(`normalized_member_gvas`).

**Why `MapObjectSaveData` is excluded (a documented allowance beyond the
mask set, per the Task-15 policy).** Python's `palworld_save_tools`
re-encodes that one map's opaque `RawData` blobs NON-byte-faithfully. Proven
empirically: an UNEDITED `world1/Level.sav` downloaded from the real Python
backend differs from the on-disk original by 356 bytes with ZERO edits, and
EVERY differing byte lies inside `MapObjectSaveData`; remove that map from
both and the entire rest of the GVAS is byte-identical. Rust (uesave) keeps
those blobs opaque and byte-faithful to the game file (Phase-1 Task 12's
resave gate proves `read → write` is byte-identical). So this is a Python
serializer quirk that Rust is CORRECT not to reproduce (reproducing it would
mean corrupting the save), and byte-identity of a full Level.sav against
Python is impossible for any corpus containing such a map — world1 has a
`DamagableRock`. Normalising both sides through uesave (parse → drop that map
→ re-serialise) leaves EVERY edited structure still compared byte-for-byte:
the pals' `CharacterSaveParameterMap` (property ORDER included — uesave uses
an order-preserving `IndexMap`), the guild's `GuildExtraSaveDataMap` lab
research, and every `Players/*.sav` (which have no `worldSaveData`, so the
removal is a no-op there). This is the ONE allowance beyond the four masks;
it is not a wire-field mask.

## `PARITY_IGNORED_PATHS`

Was empty through Phase 1; Phase 2 added exactly the four masks enumerated
under "Determinism policy" above. Phase 3 (Task 3B-3) adds one more:
`add_preset:/data/id` — the response echoes the server-generated preset uuid,
minted INDEPENDENTLY by Python (capture) and Rust (replay), same rationale as
the `add_pal`/`add_dps_pal` `instance_id` masks. Task 3C-6 adds the db-ups
single-object masks (see "db-ups scenario" below). Task 3D-3 adds
`add_gps_pal:/data/pal/instance_id` — same rationale, for a freshly-added GPS
pal's uuid4 (also covers `clone_gps_pal`, which answers on this same wire
type). Any FUTURE entry must be a narrow, enumerated `(message_type,
json_pointer)` mask, never a whole-payload wildcard, and must carry a
one-line comment in `parity.rs` explaining why the Python and Rust values are
expected to legitimately differ (timestamps, generated uuids,
Python-serializer quirks). A mask that swallows more than the specific field
it names turns a passing test into a lie.

`get_raw_data` (Task 3E-5) is NOT handled by a `PARITY_IGNORED_PATHS` entry
either — like `get_presets`, its `data` needs a dedicated comparator
(`compare_raw_data_structural`, gated by `PARITY_STRUCTURAL_TYPES`) rather
than a fixed-pointer mask; see "tools scenario" below for why.

## db-presets scenario (Phase 3, Task 3B-3)

Exercises the presets CRUD surface against a FRESH presets table:
`get_presets` (seeds from `data/json/presets.json`) → `add_preset` →
`get_presets` → `nuke_presets` → `get_presets` → `export_preset` (an unknown
id, to capture the pre-dialog validation error).

**Why `get_presets` needs its own comparator, not a `PARITY_IGNORED_PATHS`
mask.** `get_presets`' `data` is a DICT keyed by server-generated uuids
(`psp_db::presets::get_all` / `db/ctx/presets.py::get_all_presets`), and each
preset's own `id`, `pal_preset_id`, and (when present) nested `pal_preset.id`
are those same random uuids. A `PARITY_IGNORED_PATHS` entry masks one FIXED
JSON pointer — it cannot reach into a map whose *keys themselves* differ
between the two captures. `parity.rs`'s `compare_get_presets_equivalent`
handles this instead: it extracts the preset VALUES from `data` in insertion
order (both backends insert `ORDER BY rowid` — seed presets from
`presets.json` in array order, then any added preset appended), masks each
preset's `id`/`pal_preset_id`/`pal_preset.id` to a shared sentinel, and
compares the two ordered lists for equality — the dict KEYS are intentionally
never compared. `add_preset`'s own uuid (echoed directly in that response,
not nested in a dict) still uses the ordinary `add_preset:/data/id`
`PARITY_IGNORED_PATHS` mask above.

### Safe capture procedure

**The backend's `psp.db` lives at the repo root and is whatever the
developer running this is currently using — it may hold real, hand-curated
presets and settings. This scenario needs an EMPTY presets table, which means
Python must start from a FRESH `psp.db`. Never delete the real one to get
there.**

1. From the repo root, back up and move the existing `psp.db` ASIDE (don't
   delete it):
   `mv psp.db psp.db.presets-parity-backup` (or, on Windows PowerShell,
   `Move-Item psp.db psp.db.presets-parity-backup`).
2. Start the Python backend on a port your real client isn't using — the
   default 5174 may already be held by a running desktop build
   (`PSP.exe`)/dev server. For example:
   `uv run python psp.py --port 5199`
   This creates a brand-new, empty `psp.db` at the repo root and its
   `presets` table starts with zero rows.
3. From the repo root, capture against that same port:
   `uv run --with websockets scripts/capture_parity.py --scenario db-presets --url ws://127.0.0.1:5199/ws/999999999`
   Fixtures land in `rust/parity/fixtures/db-presets/`.
4. Stop the Python backend (Ctrl+C).
5. Delete the FRESH `psp.db` this capture run created, then restore the
   developer's original:
   `rm psp.db` then `mv psp.db.presets-parity-backup psp.db`
   (PowerShell: `Remove-Item psp.db` then
   `Move-Item psp.db.presets-parity-backup psp.db`).
6. From `rust/`: `cargo test -p psp-server --test parity` — replays
   `db-presets` (and every other captured corpus) against a fresh Rust temp
   DB, same as the capture run.

Fixtures are gitignored (`rust/.gitignore`'s `/parity/fixtures/`); this
corpus, like every other, is local-only — regenerate, never commit.

## db-ups scenario (Phase 3, Task 3C-6)

Exercises the UPS DATABASE surface against a FRESH psp.db (no loaded save):
`get_ups_stats` → `get_ups_collections` → `create_ups_collection` →
`get_ups_tags` → `create_ups_tag` → `add_ups_pal` → `get_ups_pals` →
`get_ups_all_filtered_ids` → `update_ups_pal` → `clone_ups_pal` →
`get_ups_stats` → `delete_ups_pals` → `nuke_ups_pals` →
`update_ups_collection` → `update_ups_tag` → `delete_ups_tag` →
`delete_ups_collection`. `scripts/capture_parity.py`'s `SAMPLE_PAL_DTO` is the
same 30-field dict as `psp-server/tests/ups_ws.rs::sample_pal_dto`.

The three session-level interop handlers (`clone_to_ups`/`import_to_ups`/
`export_ups_pal`) are NOT in this corpus: they need a loaded save, and the
add-from-DTO destinations mint fresh `uuid4` instance ids that would diverge
the save bytes. Their no-save error paths are covered by
`psp-server/tests/ups_session_ws.rs`.

**Why db-ups needs masks, not a custom comparator.** Unlike `get_presets`
(a uuid-KEYED dict, which no fixed pointer can reach), every db-ups frame is
addressable: single-object frames (`add_ups_pal` echoes the whole record;
`update_ups_pal`/`clone_ups_pal` nest under `pal`/`cloned_pal`;
`get_ups_stats` under `stats`; collections/tags under `collection`/`tag`) use
ordinary `PARITY_IGNORED_PATHS` entries; the ARRAY-shaped list frames
(`get_ups_pals` → `data.pals[]`, `get_ups_collections` →
`data.collections[]`, `get_ups_tags` → `data.tags[]`) are masked per-element
by `mask_ups_list_frames` (called from `mask_ignored_paths`), which blanks
each element's `created_at`/`updated_at`/`last_accessed_at`/`instance_id`
while leaving `total_count`/`offset`/`limit`, names, colors, and every real
field strictly compared. Both mechanisms are unit-tested synthetically
(`mask_ignored_paths_masks_ups_single_object_frames`,
`mask_ups_list_frames_masks_every_element_only`,
`ups_pal_list_masking_is_neither_too_weak_nor_too_strong`) since the fixtures
themselves are gitignored and loud-SKIP in CI. `get_ups_stats.storage_size_mb`
is also masked: Python orjson vs Rust serde_json compact-encode the same
`pal_data` JSON with slightly different float/whitespace, so the reported size
differs by a few bytes — a documented serializer divergence, not a data one.

### Safe capture procedure

**Same discipline as db-presets: the backend's `psp.db` lives at the repo
root and may hold the developer's real, hand-curated pals/collections/tags.
This scenario needs EMPTY UPS tables, i.e. a FRESH `psp.db`. Never delete the
real one to get there.**

1. From the repo root, back up and move the existing `psp.db` ASIDE (don't
   delete it):
   `mv psp.db psp.db.ups-parity-backup` (PowerShell:
   `Move-Item psp.db psp.db.ups-parity-backup`).
2. Start the Python backend on a port your real client isn't using (the
   default 5174 may be held by a running desktop build / dev server):
   `uv run python psp.py --port 5199`
   This creates a brand-new, empty `psp.db` whose UPS tables start with zero
   rows. (The row ids the scenario references — `pal_id: 1`, `collection_id:
   1`, `tag_id: 1` — assume that fresh table state.)
3. From the repo root, capture against that same port:
   `uv run --with websockets scripts/capture_parity.py --scenario db-ups --url ws://127.0.0.1:5199/ws/999999999`
   Fixtures land in `rust/parity/fixtures/db-ups/`.
4. Stop the Python backend (Ctrl+C).
5. Delete the FRESH `psp.db` this run created, then restore the developer's
   original:
   `rm psp.db` then `mv psp.db.ups-parity-backup psp.db`
   (PowerShell: `Remove-Item psp.db` then
   `Move-Item psp.db.ups-parity-backup psp.db`).
6. From `rust/`: `cargo test -p psp-server --test parity` — replays `db-ups`
   (and every other captured corpus) against a fresh Rust temp DB.

Fixtures are gitignored (`rust/.gitignore`'s `/parity/fixtures/`); this
corpus, like every other, is local-only — regenerate, never commit. With no
db-ups fixtures present (the CI/default state), the replay loud-SKIPs them —
that is correct, not a failure.

## gps scenario (Phase 3, Task 3D-3)

Exercises the GPS (Global Pal Storage) surface: `load_zip_file` (a ZIP
upload, not `select_save`) → `request_gps` ×2 (first lazy-loads
`GlobalPalStorage.sav` and emits a `progress_message`; the second returns the
cached map with none) → `add_gps_pal` → `delete_gps_pals` (never responds —
`gps_handler.py:106-114` / `handlers/gps.rs::handle_delete_gps_pals`) →
`request_gps` → `clone_gps_pal_to_player` (a pal id NOT present in GPS, to
capture the per-pal `errors` list path). `scripts/capture_parity.py`'s
`capture_gps` builds this sequence dynamically (like `capture_phase2`, not a
`FIXED_SCENARIOS` list) for two reasons: the ZIP upload has to be built from
`--save-dir` via `build_save_zip_bytes` (now also embedding
`GlobalPalStorage.sav` when the save dir has one), and
`clone_gps_pal_to_player`'s `destination_player_uid` needs a REAL player uid
from this corpus — harvested from `load_zip_file`'s own
`get_player_summaries` response burst, the same `CORPUS_PLAYER_UID`
substitution pattern `phase2` uses for its own ids.

**Why `load_zip_file` directly, not `select_save` (unlike `load_path`/
`phase2`).** GPS is read from a temp file staged only on the ZIP-upload path:
`resolve_zip_layout`/`zip_gps_temp_path` in
`rust/psp-server/src/handlers/save_file.rs` (mirroring
`save_file_handler.py:205`) extract an uploaded ZIP's `GlobalPalStorage.sav`
entry to `<tempdir>/<save_id>_GlobalPalStorage.sav` and point
`save.gps.file_path` there; `select_save` never touches this path at all. The
scenario needs to exercise that exact temp-file lazy-load, not the ordinary
on-disk one.

**NO GPS-containing corpus exists in this checkout** — no save directory
combining `Level.sav`/`Players/` with a `GlobalPalStorage.sav` (the one
`GlobalPalStorage.sav` under `tests/fixtures/saves/` sits alone, with no
matching `Level.sav` beside it, and is used only by Python's
`tests/game/conftest.py::GPS_FILE` and Rust's env-var-gated
`rust/psp-core/tests/gps.rs` direct-parse tests — neither is a WS-replayable
corpus). `capture_gps` and its
masking in `parity.rs` are scaffolding, proven correct by synthetic unit
tests (below) rather than a live capture — a developer with such a corpus
runs the procedure below later.

**Why `get_gps_response` needs its own walker, not a `PARITY_IGNORED_PATHS`
entry.** `get_gps_response`'s `data` is `save.gps.pals`
(`BTreeMap<i32, PalDto>`) JSON-encoded as a slot-keyed OBJECT — `{"0":
{pal...}, "3": {pal...}}` — not an array and not a uuid-keyed dict like
`get_presets`. A static JSON pointer can reach exactly one fixed path, never
every value of a variable-keyed map, so `parity.rs`'s `mask_gps_response_frame`
(wired into `mask_ignored_paths`, same pattern as `mask_ups_list_frames`)
walks every pal value in the map and masks its `instance_id`: after
`add_gps_pal`, the new pal's `instance_id` is a fresh `uuid4` minted
INDEPENDENTLY by Python (capture) and Rust (replay) and can never match.
Every OTHER GPS pal in the map was read from the same on-disk
`GlobalPalStorage.sav` by both backends, so its `instance_id` is already
identical between the two — masking it too is a harmless no-op, and doing so
unconditionally (rather than trying to single out just the new pal) keeps the
walker simple. GPS pals carry no DB timestamps (unlike db-ups), so
`instance_id` is the only field masked; every other field (`character_id`,
`nickname`, every stat, the slot key itself) stays strictly compared. The
no-save-loaded/no-gps-file `error`/`available: false` response shapes (see
`handle_request_gps` in `rust/psp-server/src/handlers/gps.rs`) have no pal
map under `data` at all, so the walker is a no-op for them.

Proven by three synthetic unit tests in `parity.rs`
(`mask_gps_response_frame_masks_only_instance_id_per_slot`,
`mask_ignored_paths_masks_gps_response_map_by_slot`,
`gps_response_masking_is_neither_too_weak_nor_too_strong`), mirroring the
db-ups list-frame tests, since the gps fixtures themselves don't exist yet
and the live replay path loud-SKIPs this corpus entirely.

### Safe capture procedure

**Same discipline as db-presets/db-ups: the backend's `psp.db` lives at the
repo root and may hold the developer's real, hand-curated data. The gps
scenario itself touches NO `psp.db` table (no `get_settings`/
`sync_app_state`/DB-CRUD request is in its sequence — it starts straight from
`load_zip_file`), but back it up anyway as defensive practice, the same as
`phase2`'s "psp.db safety" note above: never take the risk of a stray write
touching the developer's real database.**

1. From the repo root, back up and move the existing `psp.db` ASIDE (don't
   delete it):
   `mv psp.db psp.db.gps-parity-backup` (PowerShell:
   `Move-Item psp.db psp.db.gps-parity-backup`).
2. Start the Python backend on a port your real client isn't using — the
   default 5174 may already be held by a running desktop build (`PSP.exe`)/
   dev server:
   `uv run python psp.py --port 5199`
3. From the repo root, capture against that same port, pointing `--save-dir`
   at a corpus save directory that has a `GlobalPalStorage.sav` alongside its
   `Level.sav`/`Players/` (pass a **native, absolute** path — see the
   `phase2` section above for why mixed separators cause a spurious `level`
   mismatch):
   `uv run --with websockets scripts/capture_parity.py --scenario gps --save-dir "D:\...\some-corpus-with-gps" --url ws://127.0.0.1:5199/ws/999999999`
   Fixtures land in `rust/parity/fixtures/gps/`.
4. Stop the Python backend (Ctrl+C).
5. Delete the FRESH `psp.db` this run created, then restore the developer's
   original:
   `rm psp.db` then `mv psp.db.gps-parity-backup psp.db`
   (PowerShell: `Remove-Item psp.db` then
   `Move-Item psp.db.gps-parity-backup psp.db`).
6. From `rust/`: `cargo test -p psp-server --test parity` — replays `gps`
   (and every other captured corpus) against a fresh Rust temp DB.

Fixtures are gitignored (`rust/.gitignore`'s `/parity/fixtures/`); this
corpus, like every other, is local-only — regenerate, never commit. With no
gps fixtures present (the CI/default state, and every state in THIS
checkout), the replay simply never iterates a `gps` directory — that is
correct, not a failure.

## transfer scenario (Phase 3, Task 3E-3)

Exercises the player-transfer WS surface (`load_source_save` /
`get_source_players` / `transfer_player` / `unload_source_save`, a port of
`ws/handlers/transfer_handler.py`): `select_save` (loads the corpus save as
the ordinary MAIN save, so `transfer_player` has a target) → `load_source_save`
(unsupported `gamepass` type, error path) → `get_source_players` (both empty)
→ `load_source_save` (the SAME corpus save again, `role: "source"`, so it is
now loaded a second time as the transfer source) → `get_source_players`
(source populated, target still empty — no standalone target was loaded) →
`transfer_player` (spawn mode: `target_player_uid: null`, spawning the source
player into the main save under its own uid) → `unload_source_save`.
`scripts/capture_parity.py`'s `capture_transfer` builds this sequence
dynamically (like `capture_phase2`/`capture_gps`, not a `FIXED_SCENARIOS`
list), harvesting `CORPUS_PLAYER_UID` from `select_save`'s own
`get_player_summaries` response burst.

**This scenario deliberately does NOT exercise the standalone-target
auto-save-to-disk branch** (`handlers/tools.rs::handle_transfer_player`'s
`has_standalone_target` path — `load_source_save` with `role: "target"`,
which backs up and overwrites the target save directory on a successful
transfer). That path performs a REAL filesystem write, is covered by its own
plumbing being reused byte-for-byte from Phase 2's `write_steam_modded_save`
write conventions (see `handlers::save_file::write_transfer_target_save`),
and needs a corpus the developer is comfortable having backed-up-and-rewritten
by the capture run; a future task can add a dedicated `role: "target"` corpus
if live parity coverage of that branch is ever wanted. The error-path and
`{success: true}`-without-a-standalone-target flows this scenario DOES cover
are exercised by `psp-server/tests/transfer_ws.rs` at the unit/integration
level instead.

**Masking assessment: this scenario needs NO `PARITY_IGNORED_PATHS` entries.**
Every response it captures is a fixed-shape object —
`{"success": true, "role", "player_count", "world_name"}` from
`load_source_save`; `{"source": {...}, "target": {...}}` from
`get_source_players`, keyed by the REAL, already-on-disk player uuids both
backends read from the same corpus save (not freshly minted); and a bare
`{"success": true}` from `transfer_player` in spawn mode (no id is echoed on
that response shape at all — unlike `add_pal`/`add_gps_pal`, whose fresh
`instance_id` is masked, spawn-mode `transfer_player` never puts an id on the
wire in the first place). The one place a fresh `uuid4` genuinely IS minted on
this code path — `create_guild_for_player`'s new guild id, when the spawned
player has no guild to join — stays inside the target's in-memory SAVE TREE;
it is never serialized into a WS response this scenario reads, so it cannot
diverge the comparison. If a future extension of this scenario ever adds
`download_save_file`/`save_modded_save` (which DO serialize save bytes onto
the wire), revisit this assessment — this note applies only to the sequence
above.

### Safe capture procedure

**Same discipline as db-presets/db-ups/gps: the backend's `psp.db` lives at
the repo root and may hold the developer's real, hand-curated data. The
transfer scenario itself touches NO `psp.db` table (no `get_settings`/
`sync_app_state`/DB-CRUD request is in its sequence), but back it up anyway as
defensive practice.**

**This scenario WRITES to nothing on disk in this checkout's default
configuration** (no `role: "target"` load, so `handle_transfer_player`'s
disk-write branch never fires) — but treat the corpus save directory as
read-only anyway and use a disposable copy, not a save you care about, in
case a future edit to this scenario adds a `role: "target"` step.

1. From the repo root, back up and move the existing `psp.db` ASIDE (don't
   delete it):
   `mv psp.db psp.db.transfer-parity-backup` (PowerShell:
   `Move-Item psp.db psp.db.transfer-parity-backup`).
2. Start the Python backend on a port your real client isn't using — the
   default 5174 may already be held by a running desktop build (`PSP.exe`)/
   dev server:
   `uv run python psp.py --port 5199`
3. From the repo root, capture against that same port, pointing `--save-dir`
   at a corpus save directory with **at most 2 players** (same nondeterminism
   caveat as `load_path` — `select_save`'s player-summary extraction races on
   a `ThreadPoolExecutor` above that threshold), passing a **native, absolute**
   path (see the `phase2` section above for why mixed separators cause a
   spurious `level` mismatch):
   `uv run --with websockets scripts/capture_parity.py --scenario transfer --save-dir "D:\...\tests\fixtures\saves\world2" --url ws://127.0.0.1:5199/ws/999999999`
   Fixtures land in `rust/parity/fixtures/transfer/`.
4. Stop the Python backend (Ctrl+C).
5. Delete the FRESH `psp.db` this run created, then restore the developer's
   original:
   `rm psp.db` then `mv psp.db.transfer-parity-backup psp.db`
   (PowerShell: `Remove-Item psp.db` then
   `Move-Item psp.db.transfer-parity-backup psp.db`).
6. From `rust/`: `cargo test -p psp-server --test parity` — replays
   `transfer` (and every other captured corpus) against a fresh Rust temp DB.

Fixtures are gitignored (`rust/.gitignore`'s `/parity/fixtures/`); this
corpus, like every other, is local-only — regenerate, never commit. With no
transfer fixtures present (the CI/default state, and every state in THIS
checkout), the replay simply never iterates a `transfer` directory — that is
correct, not a failure.

## tools scenario (Phase 3, Task 3E-5 — final Phase-3 corpus)

A FIXED_SCENARIOS list (`TOOLS_SCENARIO` in `scripts/capture_parity.py`),
independent of any on-disk save — no `--save-dir` needed. Exercises the
remaining tools-surface requests no earlier corpus covers:
`convert_steam_id` (a vanity-URL input and a garbage input — the plain-uid
and steam-id shapes are already golden-vector-validated at the unit level in
Task 3E-1, and `static-data`/`phase2` don't touch this handler at all), a
same-uid `swap_player_uids` call against a save-less backend (its `{"error":
"No save file loaded."}` soft-rejection path), and a `get_raw_data` call with
every id `null`/`level: false` (its `{}` empty-object path). No save is
loaded for any of it.

**Why `get_raw_data` needs a structural comparator, not
`PARITY_IGNORED_PATHS`.** `get_raw_data`'s `data` is two DIFFERENT,
non-comparable JSON dialects for the same underlying save subtree (Contract
deviation 6): Python's `debug_handler.py` returns
`guild.save_data`/`player.save_data`/`pal.character_save`/etc. —
palworld-save-tools' GVAS-dict form — while Rust's
`psp_core::domain::raw::SaveSession::raw_json_for` serializes uesave's own
typed tree straight through serde. Neither a `PARITY_IGNORED_PATHS` field
mask nor a value-exact `compare_responses` pass can reconcile two
independently-shaped dialects of the SAME data (unlike, say, `add_pal`'s
freshly-minted `instance_id`, which differs in VALUE but agrees in SHAPE).
`parity.rs`'s `PARITY_STRUCTURAL_TYPES` list (currently `["get_raw_data"]`)
routes this type to `compare_raw_data_structural` instead: it asserts both
sides' `data` are JSON objects, and that the actual (Rust) side is non-empty
whenever the expected (Python) side was — i.e. "did Rust resolve a target
when Python did" — never comparing the two objects' CONTENT. With no save
loaded (this scenario's only `get_raw_data` fixture), both sides answer `{}`
and the check is a trivial pass; a fixture that resolves a real, non-empty
target belongs in a future save-backed corpus (see this task's report for
why one wasn't added here — Contract deviation 6 already made the payload
content non-comparable, so a live-save capture would add coverage of "does
Rust panic/error resolving a real target" without adding any NEW comparable
assertion `psp-server`'s own always-run `tools_ws.rs::
get_raw_data_level_resolves_against_a_loaded_world1_save` test doesn't
already cover locally).

**`get_guild_raw_data` is NOT in this scenario.** It is a permanently dead
wire type (registered in `MessageType`, never routed in Python's
`bootstrap.py` nor in `dispatcher.rs`'s `route`) — sending it produces no
response on either backend, which `capture_parity.py`'s
burst-until-`IDLE_SECONDS`-silence capture loop would record as a
zero-response fixture carrying no assertion at all. The dispatcher-level
silence itself is already covered without a capture, by
`psp-server/tests/dispatcher.rs::valid_but_unimplemented_type_sends_nothing`
and `psp-server/tests/ws_integration.rs::registered_but_unimplemented_type_
is_silent`.

### Safe capture procedure

**Same discipline as db-presets/db-ups/gps/transfer: the backend's `psp.db`
lives at the repo root and may hold the developer's real, hand-curated data.
This scenario touches NO `psp.db` table (no `get_settings`/`sync_app_state`/
DB-CRUD request is in its sequence, and no save is loaded), but back it up
anyway as defensive practice.**

1. From the repo root, back up and move the existing `psp.db` ASIDE (don't
   delete it):
   `mv psp.db psp.db.tools-parity-backup` (PowerShell:
   `Move-Item psp.db psp.db.tools-parity-backup`).
2. Start the Python backend on a port your real client isn't using — the
   default 5174 may already be held by a running desktop build (`PSP.exe`)/
   dev server:
   `uv run python psp.py --port 5199`
3. From the repo root, capture against that same port:
   `uv run --with websockets scripts/capture_parity.py --scenario tools --url ws://127.0.0.1:5199/ws/999999999`
   Fixtures land in `rust/parity/fixtures/tools/`.
4. Stop the Python backend (Ctrl+C).
5. Delete the FRESH `psp.db` this run created, then restore the developer's
   original:
   `rm psp.db` then `mv psp.db.tools-parity-backup psp.db`
   (PowerShell: `Remove-Item psp.db` then
   `Move-Item psp.db.tools-parity-backup psp.db`).
6. From `rust/`: `cargo test -p psp-server --test parity` — replays `tools`
   (and every other captured corpus) against a fresh Rust temp DB.

Fixtures are gitignored (`rust/.gitignore`'s `/parity/fixtures/`); this
corpus, like every other, is local-only — regenerate, never commit. With no
tools fixtures present (the CI/default state, and every state in THIS
checkout), the replay simply never iterates a `tools` directory — that is
correct, not a failure. `compare_raw_data_structural`'s own correctness is
proven independently of any live fixture by four synthetic unit tests in
`parity.rs` (`compare_raw_data_structural_errs_when_actual_is_empty_but_
expected_was_not`, `compare_raw_data_structural_oks_both_sides_empty`,
`compare_raw_data_structural_oks_non_empty_sides_with_differing_content`,
`compare_raw_data_structural_errs_when_data_is_not_an_object`), mirroring the
`get_presets`/db-ups/gps comparators' own synthetic-test pattern above.

## gamepass scenario (Phase 4, Task P4-14 — final Phase-4 corpus)

Exercises the gamepass load/convert/save-back/unlock-map surface:
`select_save` (gamepass branch) → `select_gamepass_save` →
`convert_save_format` (standalone gamepass→steam) → `save_modded_save`
(gamepass branch) → `unlock_map`.

**Captured / not captured:**

- Captured: `select_save` (gamepass), `select_gamepass_save`,
  `convert_save_format` (standalone gamepass→steam), `save_modded_save`
  (gamepass), `unlock_map`.
- NOT captured, with reasons:
  - `scan_gamepass_saves`, `delete_gamepass_save`, `delete_gamepass_player`,
    `rename_gamepass_world` — the Python handlers hardcode
    `find_container_path()` against the REAL `%LOCALAPPDATA%` install;
    capturing would read/mutate the machine's actual saves. Covered by Rust
    WS integration tests on synthetic containers instead
    (`psp-server/tests/phase4_ws.rs`).
  - `convert_save_format` loaded→gamepass — writes into the real install
    (same reason).
  - `convert_sav_file` — JSON schema intentionally diverges (uesave schema,
    spec §4); already exercised by `phase4_ws.rs::
    convert_sav_file_round_trips_over_ws` without needing byte-parity against
    Python's palworld-save-tools GVAS-dict dialect.

### Why a synthetic container, not a real gamepass install

Every other corpus in this harness points requests at an existing corpus
save directory. GamePass saves live in a machine-specific
`%LOCALAPPDATA%\Packages\...\SystemAppData\wgs\<container-id>\` tree that
doesn't exist in this checkout (or on CI) at all, so this corpus instead
PACKAGES the existing `tests/fixtures/saves/world2` steam save (chosen for
the same 1-player-avoids-`ThreadPoolExecutor`-nondeterminism reason
`load_path`'s README section documents above — `world2` is exactly 1 player)
into a synthetic wgs container tree, using Python's OWN
`palworld_save_pal.utils.gamepass.container_types`/`container_utils` code
(`ContainerIndex`, `FILETIME`, `create_new_container`) — so the on-disk
fixture the two backends are compared against is Python-authoritative, not
reverse-engineered. `scripts/capture_parity.py`'s `build_gamepass_corpus`
builds it under `rust/parity/tmp/gamepass/wgs/0009000000000000_0000000000
0000000000000000000000/`, snapshots it once (`wgs-pristine/`, restored
before every replay run since `save_modded_save` mutates the live tree), and
both backends operate on that SAME physical directory.

### The `unlock_map` LocalData.sav problem, and its resolution

`unlock_map` needs a `LocalData.sav` — the file whose `SaveData.
WorldMapMaskTextureV4` byte array it zeroes — but **neither `world1` nor
`world2` (this repo's only two steam corpora) has one** (map-unlock data is
optional/rare; most corpus saves never carry it). Rather than skip
`unlock_map` or only capture its error path, `build_gamepass_corpus` grafts a
synthetic `WorldMapMaskTextureV4` byte array (`[1, 2, 3, 0, 4]` — a mix of
zero and non-zero bytes so a byte COUNT would be discriminating, though the
WS response never echoes one) into a COPY of the corpus `LevelMeta.sav`'s
`SaveData` struct, done in Python (`_graft_world_map_mask`, using
`palworld_save_tools`' own `GvasFile`/`archive.py` read-modify-write) so the
resulting `LocalData.sav`-shaped fixture stays capture-authoritative. This is
the EXACT technique `rust/psp-core/src/localdata.rs`'s own
`unlock_world_map_zeroes_synthetic_mask_grafted_into_real_savedata` hermetic
test already uses on the Rust side (grafting into a real committed
`LevelMeta.sav` because the corpus lacks a `LocalData.sav`) — `unlock_map`
only ever reads `SaveData.WorldMapMaskTextureV4` and never validates the rest
of the struct's shape or file provenance beyond the literal filename
`LocalData.sav` (`map_unlock_handler.py:39-41`), so a LevelMeta-shaped
carrier is a legitimate stand-in for both backends, not a shortcut that
changes what's being tested.

`unlock_map` mutates its input file **in place** (`map_unlock_handler.py:
82-83` / `save_file.rs`'s `unlock_map_on_disk` both overwrite `path` with the
zeroed bytes after backing it up to `<path>.backup`), so `build_gamepass_
corpus` writes BOTH `rust/parity/tmp/gamepass/LocalData.sav` (the working
copy the `004_unlock_map.json` request points at) and an untouched `LocalData
.sav.pristine` copy alongside it. `parity.rs`'s per-replay reset
(`reset_gamepass_corpus_filesystem_state`) restores `LocalData.sav` from
`.pristine` before every run, mirroring how it restores `wgs/` from
`wgs-pristine/`.

### Determinism findings

**No new `PARITY_IGNORED_PATHS` entries were needed.** The only
irreducibly-nondeterministic value this corpus produces is
`save_modded_save`'s response index 1 — a `progress_message` frame carrying
`f"Created backup at: {backup_path}"`, where `backup_path` embeds a
wall-clock `strftime("%Y%m%d%H%M%S")` timestamp
(`local_file_handler.py:93-94` / `save_file.rs`'s
`write_modded_gamepass_containers`) computed independently by Python at
capture time and Rust at replay time. This can NOT be a
`PARITY_IGNORED_PATHS` entry: that mechanism masks by response `type` alone,
and EVERY frame in this corpus's `save_modded_save` sequence (four
deterministic frames plus this one nondeterministic one) shares the generic
`progress_message` wire type — also shared by countless deterministic
progress lines in every OTHER corpus. A type-wide mask would blank all of
those too. Instead, `parity.rs` adds a narrow, dedicated
`mask_gamepass_backup_progress_line(request_type, response_index, value)`,
called beside (not folded into) `mask_ignored_paths` at both of its call
sites, which masks ONLY response index 1 of a `save_modded_save` request, and
only after confirming the text still starts with `"Created backup at: "` — a
future re-ordering of the progress sequence fails LOUDLY (the aggregate
`compare_responses` catches the raw text mismatch) instead of silently
masking the wrong field. Proven narrow and correct by a dedicated synthetic
unit test, `mask_gamepass_backup_progress_line_masks_only_index_one_of_
save_modded_save`.

Two fields that LOOKED like determinism risks going in, but turned out fine
against the live fixtures (recorded here so a future re-investigation doesn't
repeat the analysis from scratch):

- **`select_save`/`select_gamepass_save`'s `last_modified`/`size` fields**
  (`GamepassSaveData`/`GamepassContainerInfo`, `file_manager.py:49-63`) come
  from the `Container.mtime`/`.size` fields embedded IN the container's own
  binary metadata (written once by `create_new_container` at corpus-BUILD
  time), not from the filesystem's OS-level mtime — so they're stable across
  a `wgs-pristine/` → `wgs/` restore, unlike a real on-disk mtime would be.
  `last_modified` is a FILETIME-derived float
  (`(ticks - epoch_ticks) / 10_000_000`); Python computes the numerator as an
  exact arbitrary-precision int before converting to `float`, while Rust's
  `Filetime::to_unix_seconds` (`gamepass/format.rs`) converts `self.0` (a
  `u64`) to `f64` FIRST, then subtracts — a different order of operations
  that, for large FILETIME magnitudes (this checkout's are ~1.4×10¹⁷ ticks,
  well past `f64`'s 2⁵³ exact-integer boundary), could in principle round to
  a different bit pattern. The live replay proved this ISN'T a live bug for
  the actual values this corpus produces (byte-identical JSON on both
  sides) — left unmasked on purpose, since masking a field that already
  matches would hide a real future regression instead of proving one didn't
  happen.
- **`loaded_save_files.level`** (fixture `001_select_gamepass_save.json`)
  embeds `settings.save_dir` verbatim. Rather than have `parity.rs`
  independently re-resolve `rust/parity/tmp/gamepass/wgs/<container-id>` on
  the Rust side (risking a spurious drive-letter-case or path-separator
  mismatch vs. Python's own resolved string — the exact failure mode the
  `phase2` section above warns about for mixed-separator `--save-dir`
  input), `gamepass_save_dir_from_first_fixture` reads the directory
  straight out of fixture `000_select_save.json`'s own captured `data.path`
  and writes THAT literal string into the replay server's
  `settings.save_dir` before connecting — guaranteeing both backends embed
  the identical string, by construction rather than by coincidence.

**`settings.save_dir` is global, shared-server state, not scoped to one
corpus.** All corpora in one `cargo test -p psp-server --test parity` run
replay against a SINGLE `start_server` instance (see `replay_all_fixtures`),
so a naive `update_save_dir` call for the gamepass corpus would leak into
every OTHER corpus's `get_settings`/`sync_app_state` fixtures replayed
afterward in the same run (caught live: `static-data/001_get_settings.json`
failed with the gamepass container path where the corpus's real default
`save_dir` was expected, the first time this was implemented without a
restore step). `parity.rs` now reads `settings.save_dir`'s value BEFORE
overwriting it for the gamepass corpus and restores it right after that
corpus's fixtures finish, so corpus ordering (`corpus_dirs.sort()` puts
`gamepass` alphabetically before `phase2`/`static-data`/etc.) can't leak
state between unrelated corpora.

### Safe capture procedure

**Same discipline as db-presets/db-ups/gps/transfer/tools: the backend's
`psp.db` lives at the repo root and may hold the developer's real,
hand-curated data. This scenario WRITES `settings.save_dir`, so back it up
and use a throwaway fresh one — never risk the developer's real save_dir
setting.**

Unlike every other corpus, the gamepass scenario needs a step run
**BEFORE** the Python backend process starts: `Settings.__init__` caches
`save_dir` from the DB at import time (`state.py`'s module-level
`app_state = AppState()`, same import-order fact documented in "Known Python
quirks affecting capture" above), so the container tree has to exist, and
its path has to already be in the DB, before `psp.py` boots.

1. From the repo root, back up and move the existing `psp.db` ASIDE (don't
   delete it):
   `mv psp.db psp.db.gamepass-parity-backup` (PowerShell:
   `Move-Item psp.db psp.db.gamepass-parity-backup`).
2. Build the container tree and persist its path into a FRESH `psp.db`
   (`create_db_and_tables()` inside `prepare_gamepass_corpus` creates it if
   missing, so this step needs no separate two-start warm-up dance):
   `uv run python scripts/capture_parity.py --scenario gamepass --prepare-gamepass --save-dir "<absolute path to tests/fixtures/saves/world2>"`
   This prints the container dir — copy it for step 4.
3. Start the Python backend on a port your real client isn't using:
   `uv run python psp.py --port 5199`
4. From the repo root, capture against that same port, passing the container
   dir step 2 printed:
   `uv run --with websockets scripts/capture_parity.py --scenario gamepass --save-dir "<container dir from step 2>" --url ws://127.0.0.1:5199/ws/999999999`
   Fixtures land in `rust/parity/fixtures/gamepass/`; shared mutable state
   (the wgs tree, its pristine snapshot, the synthetic LocalData.sav + its
   pristine copy, and the standalone-conversion output) lands in
   `rust/parity/tmp/gamepass/`.
5. Stop the Python backend (Ctrl+C).
6. Delete the FRESH `psp.db` this run created, then restore the developer's
   original:
   `rm psp.db` then `mv psp.db.gamepass-parity-backup psp.db`
   (PowerShell: `Remove-Item psp.db` then
   `Move-Item psp.db.gamepass-parity-backup psp.db`).
7. From `rust/`: `cargo test -p psp-server --test parity` — replays
   `gamepass` (and every other captured corpus) against a fresh Rust temp DB.

Both `rust/parity/fixtures/` and `rust/parity/tmp/` are gitignored
(`rust/.gitignore`); this corpus, like every other, is local-only —
regenerate, never commit.
