# Wire-contract fixtures

Committed golden snapshots of the WebSocket wire protocol the Svelte frontend
consumes. `psp-server/tests/wire_contract.rs` replays each fixture against an
in-process Rust server and asserts every response frame still matches, in order.
The suite is a regression net against **accidental protocol drift** — it fails
loudly if the committed corpus is missing, so it can never pass with zero
coverage.

## Layout

    fixtures/<corpus>/<nnn>_<message_type>.json

`<nnn>` is a zero-padded 3-digit scenario index. Replay order is filename order,
over one WebSocket connection per corpus. Committed corpora:

- `phase2` — pal/player/guild reads and edits (move, heal, add, delete, rename, …)
- `steam-1p`, `steam-world1` — select-save + load-zip flows on Steam saves
- `static-data` — settings and app-state responses

## Fixture format

    {
      "request":   { "type": "select_save", "data": { ... } },
      "responses": [
        { "type": "progress_message", "data": "Loading Level.sav..." },
        { "type": "loaded_save_files", "data": { ... } }
      ]
    }

- `responses` is ordered; the replay reads exactly `responses.len()` frames and
  asserts JSON equality frame-by-frame (floats included).
- `progress_message` frames are asserted like everything else: same sequence,
  same strings.
- Irreducibly nondeterministic values (a freshly generated `instance_id`, a
  timestamped download name, a zip's container framing) are masked on both sides
  before comparison. Each such tolerance is listed in `IGNORED_PATHS` (or handled
  by a dedicated comparator: `compare_download_equivalent`, `mask_gps_response_frame`,
  `compare_get_presets_equivalent`) in `wire_contract.rs`, with a justifying
  comment. Anything else that differs is a REAL bug to fix in domain code.

## Updating a fixture

When a wire response legitimately changes (a new field, a reordered frame),
update the affected `responses` in the fixture `.json` to match the new expected
output, and confirm `cargo test -p psp-server --test wire_contract` passes. Keep
the change scoped to the intended protocol change — an unexplained diff in an
unrelated frame means a regression, not a fixture that needs updating.

## Provenance

These snapshots were first captured from a now-retired reference implementation
of this server and can no longer be re-captured here; they are maintained as
committed golden inputs. Fixtures that only echoed static `data/json` content
(and so broke on every game-data patch without exercising real request/response
behaviour) are not part of this corpus.
