# AGENTS.md

Palworld Save Pal (PSP): a Palworld save-file manager. Rust backend (axum server + Tauri desktop shell), SvelteKit frontend, bundled game data. Read `docs/rust-dev-guide.md` first; `README.md` for user-facing setup.

## Layout

| Path | What it is |
|---|---|
| `psp-core` | Domain logic (save parsing, overview/leaderboard, DTOs). No web deps allowed. |
| `psp-app` | Message dispatch + handlers shared by server and desktop; wire types (`messages.rs`). |
| `psp-db` | sqlx + SQLite (`psp-rs.db`), migrations, legacy `psp.db` importer. |
| `psp-server` | Axum lib+bin: SPA static serving, WS API at `/ws/{client_id}`, `/api/convert/*`. |
| `psp-desktop` | Tauri v2 shell. `webview_app.rs` (Windows/macOS) vs `linux_launcher.rs` (Linux desktop/browser-mode launcher), selected by `cfg(target_os)` in `main.rs`. |
| `psp-ui` | SvelteKit app, adapter-static → `../ui_build`. Svelte 5 runes + Skeleton + Tailwind. |
| `psp-web`, `psp-plugin`, `psp-lua-sys` | WASM build, Lua plugin host, Lua FFI. |
| `data/json` | Game data (`json/`) + UI strings per locale (`ui/*.json`). |
| `dev.sh` / `dev.ps1` | All-in-one dev/build wrappers (see `--help`). |
| `docs/`, `contract/` | Dev guide, plugin API docs, wire-contract fixtures. |

## Commands

From repo root:

```bash
cargo fmt --all                                   # required before every commit
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p psp-server -- --dev                  # backend only, 127.0.0.1:7257
cargo test -p psp-core <name>                     # focused tests (full suite is slow)
./dev.sh --web | --desktop | --serve | --build-desktop | --build-appimage   # wrappers (Linux AppImage pipeline = tauri bundle + scripts/appimage-strip-graphics.sh, same as release CI)
```

From `psp-ui/`: `bun run check` (svelte-check), `bun run lint`, `bun run test:unit` (vitest), `bun run test:integration` (playwright), `bun run dev:web`.

Desktop: `cd psp-desktop && cargo tauri dev`. Before any non-dev `cargo run -p psp-desktop`, build the UI (`bun run build:desktop` → `ui_build/`) or startup errors.

## Ports (keep in sync everywhere)

- PSP server: **7257** (Dockerfile, docker-compose.yml, scripts/build-docker.*, dev.sh defaults, `SERVER_PORT` in psp-desktop, `PUBLIC_WS_URL=127.0.0.1:7257/ws` baked into desktop builds).
- Vite dev: **7258**, `strictPort: true` (tauri.conf.json devUrl points at it).
- Changing one without the others ships a UI with a dead WebSocket. Grep for the old number repo-wide.

## Wire protocol rules (adding a WS message)

The WS vocabulary is duplicated in several places that must all stay in sync:

1. `psp-app/src/messages.rs` `define_message_types!`
2. Same file's test tables — TWO of them: `EXPECTED_WIRE_NAMES`/`FEATURE_ADDITION_WIRE_NAMES` (exact order; `wire_names_match_python_enum_exactly` enforces it) **and the pinned count** in `message_type_count_is_expected` (`EXPECTED_WIRE_NAMES.len() == 133` today). Adding types to the table without bumping that constant breaks the suite — this has happened twice.
3. `psp-ui/src/lib/types/ws.ts` `MessageType` enum
4. Route arm: `psp-app/src/dispatcher.rs` (has a catch-all that forwards to the ext router) or `psp-server/src/server_ext.rs` (+ its `OWNED_WIRE_TYPES` list — only request types the router actually claims; emit-only reply types do NOT go there)
5. Handler + reply emit; UI resolves replies via `sendAndWait(type)` purely on reply-type match.

Shell/desktop events follow the listener pattern in `psp-server/src/lib.rs` (`set_mode_listener`, `set_ready_listener`, `set_display_mode`) — psp-server stays Tauri-agnostic; `psp-desktop/src/linux_launcher.rs` owns the window/tray/mode behavior.

## Desktop gotchas

- `tauri::is_dev()` is true for ANY binary not built by `cargo tauri build` — always gate dev-server URLs on `cfg!(debug_assertions)` too (see `choose_webview_url`).
- Never `chdir()` in the packaged app: the AppImage's bundled WebKit spawns helpers via a cwd-relative path. Writable state goes through `PSP_APP_ROOT`.
- `WEBKIT_DISABLE_DMABUF_RENDERER=1` is defaulted on Linux (blank-webview workaround); respect a user-set value.
- Linux mode persistence: `mode.json` in the app data dir (`$XDG_DATA_HOME|~/.local/share` + `com.palworldsavepal.desktop`); missing/corrupt ⇒ first-run `/mode-select` overlay. First-run pivot is in-process; cross-mode switches relaunch the exe.
- `psp-desktop` is one binary: Linux runs `linux_launcher` (Desktop or Browser/tray mode), other OSes run `webview_app` unchanged.
- Launcher lifecycle invariants (don't break these): `CURRENT_MODE` is updated right after a mode is persisted (or post-pivot switches re-enter the pivot branch and build a duplicate tray); `ServiceMode` makes last-window-close a no-op in Browser mode; `UserQuitting` lets deliberate exits (tray Quit, relaunch, WS shutdown) through that guard; `ServerStarting` serializes tray-triggered server starts so two can't race into `handle_port_busy`'s `exit(0)`. Server restart work runs on plain std threads — `block_on` inside a tokio worker panics.
- The WS `shutdown` message exits the whole app from any server generation (the per-start watch receiver keeps the channel open across restarts) — it's the quit path for tray-less browser mode.
- Empirical: graceful shutdown force-closes open WebSockets in ~6 ms (verified against a live server) — Quit/relaunch cannot hang on a connected browser tab.
- Tray is libappindicator (dlopens `libayatana-appindicator3.so.1`) ⇒ StatusNotifierItem only, no XEmbed fallback; on DEs without an SNI host (vanilla GNOME, most tiling WMs) the icon never appears and creation still succeeds silently. Whether the CI AppImage bundles libayatana is unverified.
- Because tray-creation success is meaningless on Linux, `build_tray` probes the DBus session bus for a StatusNotifier watcher (`org.kde.StatusNotifierWatcher` / ayatana alias, via `gdbus`→`dbus-send` fallback) and publishes the verdict with `psp_server::set_tray_available`; the `display_mode` reply carries it as `tray_available`, and the browser editor shows a Quit banner (`TrayUnavailableBanner`, sends `shutdown`) when it is `false`. A failed probe reports `true` (don't nag working trays).

## Frontend rules

- Svelte 5 runes (`$state`, `$derived`, `$props`, `$bindable`, snippets). Avoid `state_referenced_locally` patterns (use `$derived`/captured closures).
- Path aliases: `$lib`, `$components`, `$ws`, `$types`, `$states`, `$utils`, `$i18n`, `$theme`, `$docs`.
- i18n: user-facing strings go through paraglide (`$i18n/messages`); add new keys to `data/json/ui/en.json` only. Paraglide falls back to English for keys missing from a locale, so the 15 non-English locale files intentionally lag `en.json` — do NOT backfill them by copying English in (a backfill commit was reverted for this; upstream machine-translates them with an external script that is not in the repo). Key parity with `en.json` is not an invariant. (Exception today: Linux mode-select/tray strings are hardcoded English, outside paraglide.)
- Static serving quirks are tested in `psp-server/tests/http_static.rs` (pretty-URL `{path}.html` serving, `/api` and `/ws` bypass the SPA redirect, path-traversal cases). Touch `static_files.rs` ⇒ run those tests.

## Conventions

- Rust comments explain constraints/why, not what. Heavy doc comments (`///`) on module purpose are the house style — read them before editing a file.
- uesave-rs is a git dependency pinned to branch `palworld-v1` (see `docs/rust-dev-guide.md`).
- Overview/leaderboard math computes from game data at runtime — no hardcoded game values; heuristics must be documented as such.
- Test fixtures: `tests/fixtures/saves/` (world saves with per-player files).

## Known pre-existing failures (don't chase, don't misattribute)

These fail on a Linux dev box regardless of branch state (proven by bisection to pre-date the overview/Linux-mode branch, Sept 2026). Verify your change isn't the cause by testing the parent commit, not by assuming:

- `psp-server` `session_persistence_ws::eject_of_non_attached_id_leaves_other_connection_intact` — headless `select_save` never updates `settings.save_dir` (desktop-mode-only code path) and the Linux default is the literal string `"~"`, so `save_modded_save` writes `~/LevelMeta.sav` relative to cwd → ENOENT. Needs a test-env or write-path fix upstream.
- `psp-server` `wire_contract::replay_recorded_wire_fixtures` — recorded fixtures carry the original author's Windows paths and the harness does no substitution; times out on fixture 000.
- `psp-plugin` `the_documented_catalog_census_matches_the_shipped_game_data` — `docs/plugin-api.md` says 34 catalogs, shipped data has 33.
- `bun run check` — one pre-existing type error in `src/routes/layoutPathRestore.test.ts` (vitest mock typing).

## Debugging notes

- When judging "is the suite green," never truncate `cargo test` output (`head -N` on the result lines once hid two failing suites and produced a wrong audit conclusion). Grep for `FAILED` specifically.
- Bisecting with a git worktree: do NOT share `CARGO_TARGET_DIR` with the main checkout — the mixed artifacts can produce phantom E0599 "variant not found" errors. Use an isolated target dir per worktree, or `cargo clean -p <pkg>` if already contaminated.
- To probe the embedded server without HTTP-client deps, copy the std-TCP `http_get` pattern from `linux_launcher.rs` (send a raw GET, parse the status line).
- Perf facts already established (don't re-litigate): `tauri dev` uses the host's WebKitGTK while CI AppImages bundle the ubuntu-22.04 runner's copy — dev-vs-packaged divergence is real; `scripts/appimage-strip-graphics.sh` is CI-only (release.yml/desktop-rust.yml) and predates the Linux-mode work; "WebKitGTK is slower than browsers regardless of GPU" remains an unproven hypothesis.

## Read before touching

- `docs/rust-dev-guide.md` — crate boundaries, commands, CI.
- `docs/plugin-api.md`, `docs/plugins.md` — plugin surface (`psp-plugin/tests/docs_plugin_api_md.rs` enforces doc/data consistency).
- `contract/README.md` + `contract/fixtures/` — wire-contract fixtures.
- `psp-desktop/src/linux_launcher.rs` module docs — Linux modes, tray reachability matrix, relaunch semantics.
