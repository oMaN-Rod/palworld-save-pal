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

## `PARITY_IGNORED_PATHS`

Starts empty. Any future entry must be a narrow, enumerated
`"<message_type>:<json_pointer>"` mask (e.g. `"loaded_save_files:/data/save_dir"`),
never a whole-payload wildcard, and must carry a one-line comment in
`parity.rs` explaining why the Python and Rust values are expected to
legitimately differ (timestamps, absolute paths, generated uuids). A mask
that swallows more than the specific field it names turns a passing test
into a lie.
