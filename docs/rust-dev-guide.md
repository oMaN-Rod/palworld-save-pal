# Rust workspace developer guide

The backend is a Cargo workspace (edition 2021) living at the repo root,
with four crates:

| Crate | Role |
|---|---|
| `psp-core` | Domain logic: save sessions over uesave-rs typed structs, DTOs, game-data loading, presets/transfer/steam-id logic. No web deps. |
| `psp-db` | sqlx + SQLite (`psp-rs.db`), embedded migrations, one-time legacy `psp.db` importer. |
| `psp-server` | Axum: SPA static serving, `GET /ws/{client_id}` WebSocket (the 123-message API), `POST /api/convert/*`. Lib + bin. |
| `psp-desktop` | Tauri v2 shell: spawns the embedded server on `127.0.0.1:5174`, native dialogs. |

Save parsing is provided by [uesave-rs](https://github.com/oMaN-Rod/uesave-rs)
(branch `palworld-v1`), consumed today as a **path dependency** pointing at a
sibling checkout: `../uesave-rs/uesave` relative to this repo. Clone
uesave-rs next to this repo and check out `palworld-v1` before building. A
switch to a git dependency (pinned via `Cargo.lock`) is planned but not yet
in effect.

## Everyday commands

All from the repo root:

```bash
cargo run -p psp-server -- --dev        # backend on 127.0.0.1:5174
cargo run -p psp-desktop                # desktop app (embedded server)
cargo fmt --all                         # required before every commit
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace                  # unit + integration + parity
```

Frontend type check: `bun run check` (from `ui/`).

## Server CLI

`psp-server --host 0.0.0.0 --port 5174 --ui-dir ./ui --data-dir ./data --db ./psp-rs.db [--dev]`

On first start, a legacy Python `psp.db` found next to the `--db` file is
backed up and imported (settings, presets, UPS, servers) — once.

## Parity harness

The wire contract was verified during the port by capture/replay:

1. `scripts/capture_parity.py` drove a running **Python** backend over a
   corpus save and recorded every request → ordered response list into
   `parity/fixtures/<corpus>/<nn>_<message_type>.json`. Re-capturing now
   requires checking out the retired Python backend from git history.
2. `psp-server/tests/parity.rs` starts an in-process Rust server, replays
   each committed fixture in filename order, and asserts response-sequence
   equality (`PARITY_IGNORED_PATHS` allowlists justified divergences).

Run it alone:

```bash
cargo test -p psp-server --test parity -- --nocapture
```

Fixtures are committed under `parity/fixtures/`; the corpus `.sav` files are
not (size) — fixtures whose save is absent are skipped, so CI runs a subset
and the full-corpus run is a local release gate.

End-to-end request timing (e.g. `select_save`) shows up in the `--dev`
server logs — no separate perf script is needed.

## Docker

`Dockerfile` is 3-stage: bun UI build → cargo-chef cached Rust build →
debian-slim runtime. `docker compose up --build -d` builds and runs;
`PUBLIC_WS_URL` is a build arg (baked into the SPA). No prebuilt image is
published — Docker is self-build only. The DB persists in the `./db` bind
mount.

## CI

`.github/workflows/desktop-rust.yml` builds the Tauri desktop bundles for
Windows, Linux, and Mac.
