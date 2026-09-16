# Installing PalStudio

| Channel | Best for | Command |
| --- | --- | --- |
| One-line install | End users | `curl -fsSL https://palstudio.app/install \| bash` / `irm https://palstudio.app/install \| iex` |
| Desktop artifacts | Manual downloads | [releases](https://github.com/oMaN-Rod/palworld-save-pal/releases) |
| Server bootstrap | Servers / headless boxes | `scripts/install-server.sh` / `.ps1` (see below) |
| From source | Rust toolchains | `cargo install --git https://github.com/oMaN-Rod/palworld-save-pal palstudio` |
| Docker | Self-hosters | `docker compose up -d` |

## One-line install

`https://palstudio.app/install` is a single entry point that serves the right
script for the caller (the Cloudflare Worker behind the domain sniffs the
User-Agent; browsers land on the releases page). The scripts pull the actual
artifacts from the latest GitHub release and verify their SHA-256 against the
release checksums.

**Linux / macOS**

```sh
curl -fsSL https://palstudio.app/install | bash
```

- **Linux** installs the AppImage into `~/.local/bin/PalStudio.AppImage`
  (`/usr/local/bin` as root). AppImages need libfuse2; without it, run
  `PalStudio.AppImage --appimage-extract-and-run`.
- **macOS** mounts the .dmg (a universal binary — arm64 and x86_64), copies
  `PalStudio.app` into `/Applications` and clears the quarantine attribute so
  unsigned builds open normally.

**Windows (PowerShell)**

```powershell
irm https://palstudio.app/install | iex
```

Installs the standalone zip under `%LOCALAPPDATA%\PalStudio` — the launcher
CLI in `bin\` (`palstudio`, `palstudio webapp`, `palstudio serve`), the
desktop app in `bin\palstudio-desktop.exe`, plus the web UI and game data —
then adds `bin\` to the user PATH and creates Start Menu / Desktop shortcuts
to the desktop app. The database stays at `ps-rs.db` inside the install dir;
re-running the installer updates in place and preserves it.

Prefer the MSI?

```powershell
$env:PALSTUDIO_MSI = '1'; irm https://palstudio.app/install | iex
```

(or `irm https://palstudio.app/install.ps1 -OutFile install.ps1` then
`.\install.ps1 -Msi`). The MSI is also a plain download on the releases page.

Environment overrides (both scripts): `PALSTUDIO_VERSION` (pin a tag, e.g.
`v1.4.2`), `PALSTUDIO_REPO` (owner/name), `PALSTUDIO_API_BASE` /
`PALSTUDIO_DOWNLOAD_BASE` (mirrors/testing), and per-platform:
`PALSTUDIO_BIN_DIR` (AppImage destination), `PALSTUDIO_APP_DIR` (.app
destination), `PALSTUDIO_INSTALL_DIR` (zip location),
`PALSTUDIO_SKIP_SHORTCUTS=1`, `PALSTUDIO_MSI=1`.

## Desktop (direct download)

Grab the matching artifact from the
[releases](https://github.com/oMaN-Rod/palworld-save-pal/releases) page —
`windows.msi` or `windows-standalone.zip`, `macos.dmg` (universal),
`linux.AppImage` / `linux.deb` — and install it as usual. All of them stay
available alongside the one-line installer; use whichever you prefer. The
desktop app bundles the UI and game data and keeps its database in your
per-user app data dir (the standalone zip keeps it beside the exe).

## Server / headless (bootstrap script)

The server-oriented installer — prebuilt bundle with launcher + desktop app +
web UI + game data — asks for standalone vs background service on interactive
terminals and seeds the [network policy](#network-policy-all-editions-incl-docker):

```sh
curl -fsSL https://raw.githubusercontent.com/oMaN-Rod/palworld-save-pal/main/scripts/install-server.sh | sh
```

```powershell
irm https://raw.githubusercontent.com/oMaN-Rod/palworld-save-pal/main/scripts/install-server.ps1 | iex
```

What it does:

1. Detects OS and architecture, resolves the latest release (or `$VERSION`,
   e.g. `VERSION=v1.4.2 sh install-server.sh`).
2. Downloads the prebuilt server bundle and verifies its SHA-256 against the
   release checksums.
3. Installs under `~/.local/share/palstudio` (or `/opt/palstudio` as root,
   `%LOCALAPPDATA%\PalStudio` on Windows) and links `palstudio` onto PATH.
4. Asks how you want to run it (interactive terminals only):
   - **standalone** — no service; run `palstudio` and pick **desktop** or
     **webapp** (the choice is remembered). `palstudio desktop` opens the
     bundled desktop app; `palstudio webapp` serves and opens your browser;
     `palstudio serve` is the headless server.
   - **background service** — systemd (Linux, with lingering enabled), a
     launchd agent (macOS) or a robust logon Scheduled Task (Windows,
     auto-restart, single-instance) running `palstudio serve`, i.e. the
     webapp edition, started immediately and at boot/login.
   Non-interactive runs (curl | sh, CI) default to the background service;
   `MODE=standalone|service|ask` picks explicitly.
5. Waits for the server to answer and prints the URL.

Useful environment overrides: `REPO`, `VERSION`, `MODE`, `HOST` (default
`127.0.0.1`; `0.0.0.0` to expose the LAN), `PORT`, `PREFIX`, `LISTEN`,
`PIN` (the last two seed the network policy on first boot).

## Network policy (all editions, incl. Docker)

PalStudio ships a Sunshine-style network policy for **itself** (distinct
from the Palworld game servers it manages), configured on the in-app
**Network** page (server/webapp editions) or seeded at install time:

- **Listen modes** — `localhost` (default; this machine only), `lan`,
  `tailscale` (only the tailnet's 100.64.0.0/10 range), `wan`. Modes are
  enforced per connection, so switching never restarts the listener.
- **Port** — editable in the Network page (the server rebinds in place) or
  pinned via `PORT`/`--port`/`PS_PORT`. Loopback is always trusted and can
  always write.
- **Allowlists** — `connect` decides who may talk to the instance at all,
  `write` who may edit (everyone else gets a read-only UI; mutations over
  HTTP and WS are refused with 403 / a read-only error frame). Entries are
  IPs or CIDR ranges; empty means "everyone the listen mode admits".
- **PIN** — optional password for network peers (`networkonly`) or even
  localhost (`always`). Wrong attempts are PBKDF2-slowed; sessions are
  in-memory cookies that die with the process. Locked-out browsers land on
  a minimal `/network-unlock` page.
- **Fail-closed** — auth demanded without a PIN configured refuses network
  peers outright instead of silently letting them in.
- **Tailscale** — the Network page shows this node's tailnet addresses and
  can toggle `tailscale funnel` for the port when the CLI is present
  (funnel traffic arrives via the local proxy and cannot be IP-filtered —
  the page insists on a PIN first).
- **UPnP** — an explicit, clearly-discouraged toggle (feature-gated at
  build time); tailscale is the recommended remote path.

Docker seeds the same policy through the environment on every boot
(`PS_NETWORK_ENV=always`): `PS_LISTEN` (default `lan`) and `PS_PIN` in the
compose file, e.g. `PALSTUDIO_LISTEN=tailscale PALSTUDIO_PIN=1234 docker
compose up -d`. Native installs apply `PS_LISTEN`/`PS_PIN` on first boot
only, after which the Network page owns the policy.

### Switching runtime mode later

The install-time standalone-vs-service choice is not permanent: the
Network page has a **Runtime mode** card that (un)registers the same
systemd unit / launchd agent / Scheduled Task the installer creates —
straight from the tool:

- **Switch to background service** registers the definition from the
  currently-running command line (a `webapp` automatically graduates to the
  `serve` verb), starts it, and exits the standalone process; the service's
  restart backoff takes the port the moment it frees. The page polls and
  reconnects once the service is up.
- **Switch to standalone** removes the definition and exits; the page then
  tells you to start PalStudio yourself (`palstudio` — desktop by default —
  or `palstudio serve`).

The desktop app itself is standalone by design (localhost-only); the card
only appears in the server/webapp editions.

### Network settings tiers

How much of the Network page you get depends on how the instance runs:

| Context | Network page |
| --- | --- |
| Desktop app (Tauri) | none — localhost by construction |
| `palstudio webapp` (hand-launched, incl. the AppImage webapp) | **port only**, localhost-enforced |
| `palstudio serve` / `palstudio host` | full |
| Background service (systemd / launchd / Task Scheduler) | full |
| Docker | full (`--hosted` in the image CMD) |

The local-webapp tier is enforced server-side, not just hidden: the listen
mode is clamped to localhost, the PIN/allowlists/UPnP/funnel are off, and
PUTs that try to change anything but the port are refused with a pointer to
`palstudio host` or the service. The stored policy is left untouched, so
graduating to a hosted context restores whatever was configured before.

## From source

```sh
cargo install --git https://github.com/oMaN-Rod/palworld-save-pal palstudio
palstudio   # → http://127.0.0.1:5174
```

Compiles from source (a C/C++ compiler is required — the Oodle decompressor
and Lua are built in-tree). On first run the binary downloads the UI and
game data for its own version into the platform data home
(`~/.local/share/palstudio`, `~/Library/Application Support/palstudio`,
`%APPDATA%\palstudio`), verifies the checksum, and starts — so the matching
GitHub release must already exist for released tags. Later runs reuse the
provisioned assets. See `palstudio --help` for explicit `--ui-dir` /
`--data-dir` / `--db` overrides and `PALSTUDIO_REPO` for asset mirrors.

## Docker

```bash
mkdir palstudio && cd palstudio
curl -fsSLO https://raw.githubusercontent.com/oMaN-Rod/palworld-save-pal/main/docker-compose.yml
docker compose up -d
```

Pulls `ghcr.io/oman-rod/palworld-save-pal:latest` (multi-arch: amd64/arm64),
persists the database in the `palstudio-db` volume, restarts unless stopped,
and healthchecks the HTTP listener.

The UI's WebSocket endpoint is baked at image build time as
`127.0.0.1:5174/ws` — correct when you browse from the Docker host. To serve
browsers on other machines, rebuild with your host IP instead:

```bash
PUBLIC_WS_URL=192.168.1.20:5174/ws \
  docker compose -f docker-compose.yml -f docker-compose.build.yml up -d --build
```

(or `scripts/build-docker.sh` / `.ps1`, which detect the IP). Do not
bind-mount over `/app/data` — the game data ships inside the image and a
bind mount would shadow it with an empty dir.

**Changing the port in Docker**: change it in the compose file
(`PALSTUDIO_PORT=8080 docker compose up -d`), not the in-app Network page.
The published mapping forwards to the container's internal port, so an
in-app port change rebinds the listener away from the mapping and strands
the UI; the server also warns about this when it happens. The internal port
can be moved with a `PS_PORT` entry in the service environment, which then
wins on every boot (`PS_NETWORK_ENV=always`).

## Testing the install flow (maintainers)

The one-liners can be exercised end-to-end without touching palstudio.app or
GitHub. `scripts/install-mock-server.py` serves the `/install` endpoint (same
User-Agent routing as the Worker) *and* a fake GitHub release built from a
local directory:

```sh
./scripts/test-install-flow.sh
```

That harness fabricates a release (AppImage, dmg, msi, standalone zip with
the real layout, checksums), runs the actual `curl | bash` and `irm | iex`
one-liners against the mock, and asserts installs, checksum verification,
update-over-existing behavior and tamper rejection.

To test against **real** GitHub artifacts before going live, push the release
workflow changes to a fork, cut a prerelease tag, and point the scripts at
it — both installers accept overrides:

```sh
PALSTUDIO_REPO=<owner>/fork PALSTUDIO_VERSION=v1.5.0-test \
  curl -fsSL https://palstudio.app/install | bash
```

(`releases/latest` ignores drafts and prereleases, so pin `PALSTUDIO_VERSION`
when testing those.) The Worker route itself can be checked locally with
`bun run dev` in `signal-broker/` (wrangler dev) or `python3
scripts/install-mock-server.py --release-dir … --tag …`; `signal-broker`
unit-tests the User-Agent routing (`bun run test`).

## Release checklist (maintainers)

The channels are produced by CI, in this order:

1. `scripts/bump.ps1 <version>` → commit → tag `v<version>`.
2. Run **Build and Release** (`release.yml`) from the tag — desktop
   installers (MSI, standalone zip with the `bin/` layout, universal dmg,
   AppImage, deb) plus `PalStudio-<tag>-checksums.txt` and the five
   `palstudio-<tag>-server-*.tar.gz` bundles with their checksums.
3. Publishing the draft release fires **Publish Docker image** (GHCR) and
   the Nexus/Discord uploads. Deploy Web (`deploy-web.yml`) ships the site
   **including the current `install.sh` / `install.ps1`** to
   palstudio.app/install — re-run it manually after installer-only changes.
