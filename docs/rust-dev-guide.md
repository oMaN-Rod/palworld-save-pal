# Rust workspace developer guide

The backend is a Cargo workspace (edition 2021) living at the repo root,
with four crates:

| Crate | Role |
|---|---|
| `psp-core` | Domain logic: save sessions over uesave-rs typed structs, DTOs, game-data loading, presets/transfer/steam-id logic. No web deps. |
| `psp-db` | sqlx + SQLite (`psp-rs.db`), embedded migrations, one-time legacy `psp.db` importer. |
| `psp-server` | Axum: SPA static serving, `GET /ws/{client_id}` WebSocket (the 123-message API), `POST /api/convert/*`. Lib + bin. |
| `psp-desktop` | Tauri v2 shell: spawns the embedded server on `127.0.0.1:5174`, native dialogs. |

Save parsing is provided by [uesave-rs](https://github.com/oMaN-Rod/uesave-rs),
consumed as a **git dependency** (branch `palworld-v1`) pinned to an exact
commit via `Cargo.lock`. To pull newer uesave-rs commits:
`cargo update --package uesave`. To develop against a local checkout, add a
never-committed patch to `Cargo.toml`:

```toml
[patch."https://github.com/oMaN-Rod/uesave-rs.git"]
uesave = { path = "../uesave-rs/uesave" }
```

## Everyday commands

All from the repo root:

```bash
cargo run -p psp-server -- --dev        # backend on 127.0.0.1:5174
(cd psp-desktop && cargo tauri dev)     # desktop app, hot-reload
cargo fmt --all                         # required before every commit
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace                  # unit + integration + wire-contract
```

Frontend type check: `bun run check` (from `ui/`).

## Server CLI

`psp-server --host 0.0.0.0 --port 5174 --ui-dir ./ui --data-dir ./data --db ./psp-rs.db [--dev]`

On first start, a legacy Python `psp.db` found next to the `--db` file is
backed up and imported (settings, presets, UPS, servers) — once. This is a
one-time compatibility path for upgrades from the retired Python build; it runs
a single time, records a guard, and is inert thereafter.

## Wire-contract harness

`psp-server/tests/wire_contract.rs` guards the WebSocket wire protocol the
Svelte frontend depends on. It starts an in-process Rust server, replays each
committed fixture under `contract/fixtures/<corpus>/<nnn>_<message_type>.json`
in filename order, and asserts response-sequence equality
(`IGNORED_PATHS`, plus dedicated comparators, mask only irreducibly
nondeterministic values). It is a regression net against accidental protocol
drift — **not** a comparison against any prior implementation.

Run it alone:

```bash
cargo test -p psp-server --test wire_contract -- --nocapture
```

The fixtures are committed golden inputs, so the suite always runs and **fails
loudly if the corpus is missing** rather than skipping. See
`contract/README.md` for the fixture format and how to update one when a
response legitimately changes.

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
