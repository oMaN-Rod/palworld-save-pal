# Parity fixtures

Capture/replay contract harness (spec §2). Fixtures record the PYTHON
backend's exact wire behavior; `psp-server/tests/parity.rs` replays them
against the Rust server and diffs every response.

## Layout

    fixtures/<corpus>/<nn>_<message_type>.json

`<nn>` is the zero-padded scenario index — replay order is filename order,
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
   and delete it) so settings are defaults, and start the Python backend:
   `uv run python psp.py --port 5174`
2. Do NOT load a save.
3. `uv run --with websockets scripts/capture_parity.py --corpus static-data`
4. `cd rust` then `cargo test -p psp-server --test parity`

Fixtures embed machine-local values (e.g. the default `save_dir`) —
they are gitignored; regenerate locally, never commit.

Later phases add corpora that first `select_save` a corpus save file.

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
- **Fixtures present and mismatched**: `pretty_assertions::assert_eq!`
  panics with a labeled, colorized diff of the expected vs. actual
  response arrays and the offending fixture path, failing the test.

## `PARITY_IGNORED_PATHS`

Starts empty. Any future entry must be a narrow, enumerated
`"<message_type>:<json_pointer>"` mask (e.g. `"loaded_save_files:/data/save_dir"`),
never a whole-payload wildcard, and must carry a one-line comment in
`parity.rs` explaining why the Python and Rust values are expected to
legitimately differ (timestamps, absolute paths, generated uuids). A mask
that swallows more than the specific field it names turns a passing test
into a lie.
