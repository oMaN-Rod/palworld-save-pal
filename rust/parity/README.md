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
under "Determinism policy" above. Any FUTURE entry must be a narrow,
enumerated `(message_type, json_pointer)` mask, never a whole-payload
wildcard, and must carry a one-line comment in `parity.rs` explaining why the
Python and Rust values are expected to legitimately differ (timestamps,
generated uuids, Python-serializer quirks). A mask that swallows more than
the specific field it names turns a passing test into a lie.
