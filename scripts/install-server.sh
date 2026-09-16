#!/bin/sh
# PalStudio server — one-line bootstrap installer (Linux / macOS).
#
#   curl -fsSL https://raw.githubusercontent.com/oMaN-Rod/palworld-save-pal/main/scripts/install-server.sh | sh
#
# The desktop-app installer served at https://palstudio.app/install lives in
# the repo root install.sh; THIS script is the advanced, server-oriented
# channel.
#
# Installs the latest release's prebuilt server bundle (binary + desktop
# launcher + web UI + game data), links the binary onto PATH, then asks how
# you want to run it:
#   - standalone: no service; run `palstudio` and pick desktop or webapp
#     (the choice is remembered)
#   - background service: systemd (Linux) or launchd (macOS) running the
#     headless webapp server, started immediately and at boot/login
#
# Environment overrides (for curl|sh, CI, or unattended boxes):
#   MODE        standalone | service | ask        (default: ask on a TTY, service otherwise)
#   VERSION     pin a release tag                 (e.g. VERSION=v1.4.2)
#   REPO        GitHub owner/name                 (default oMaN-Rod/palworld-save-pal)
#   HOST/PORT   bind address for the service      (default 127.0.0.1 / 5174)
#   LISTEN      network policy seed: localhost|lan|wan|tailscale (first boot only)
#   PIN         network policy seed: require this PIN from non-loopback peers
#   PREFIX      install location                  (default /opt/palstudio for root,
#                                                 ~/.local/share/palstudio otherwise)
#   NO_SERVICE  set to 1 as an alias for MODE=standalone
set -eu

REPO="${REPO:-oMaN-Rod/palworld-save-pal}"
VERSION="${VERSION:-}"
HOST="${HOST:-127.0.0.1}"
PORT="${PORT:-5174}"
LISTEN="${LISTEN:-}"
PIN="${PIN:-}"
PREFIX="${PREFIX:-}"
MODE="${MODE:-}"
NO_SERVICE="${NO_SERVICE:-0}"
[ "$NO_SERVICE" = 1 ] && MODE="${MODE:-standalone}"

log()  { printf '==> %s\n' "$*"; }
warn() { printf 'warning: %s\n' "$*" >&2; }
die()  { printf 'error: %s\n' "$*" >&2; exit 1; }

need() { command -v "$1" >/dev/null 2>&1 || die "$1 is required but not found"; }
need curl
need tar

# ---------------------------------------------------------------- host info
kernel=$(uname -s)
machine=$(uname -m)
case "$kernel" in
  Linux)  os=linux ;;
  Darwin) os=macos ;;
  *) die "unsupported OS '$kernel' — this installer covers Linux and macOS" ;;
esac
case "$machine" in
  x86_64|amd64) arch=x86_64 ;;
  aarch64|arm64) arch=aarch64 ;;
  *) die "unsupported architecture '$machine'" ;;
esac
platform="${os}-${arch}"

# ---------------------------------------------------------------- version
if [ -z "$VERSION" ]; then
  log "looking up the latest release of $REPO"
  VERSION=$(curl -fsSL -H 'Accept: application/vnd.github+json' \
    "https://api.github.com/repos/$REPO/releases/latest" \
    | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1)
  [ -n "$VERSION" ] || die "could not resolve the latest release tag from GitHub"
fi
asset="palstudio-${VERSION}-server-${platform}.tar.gz"
checksums_asset="palstudio-${VERSION}-server-checksums.txt"
log "installing PalStudio server $VERSION ($platform)"

# ---------------------------------------------------------------- locations
am_root=0
[ "$(id -u)" = 0 ] && am_root=1
if [ -z "$PREFIX" ]; then
  if [ "$am_root" = 1 ]; then PREFIX=/opt/palstudio; else PREFIX="$HOME/.local/share/palstudio"; fi
fi
if [ "$am_root" = 1 ]; then
  bindir=/usr/local/bin
else
  bindir="$HOME/.local/bin"
fi
mkdir -p "$PREFIX" "$bindir"

# ---------------------------------------------------------------- download
tmp=$(mktemp -d) || die "mktemp failed"
trap 'rm -rf "$tmp"' EXIT
bundle="$tmp/$asset"
base_url="https://github.com/$REPO/releases/download/$VERSION"
log "downloading $base_url/$asset"
curl -fSL --retry 3 -o "$bundle" "$base_url/$asset" \
  || die "download failed — does $VERSION ship a $platform bundle?"

checksums="$tmp/checksums.txt"
if curl -fSL --retry 2 -o "$checksums" "$base_url/$checksums_asset"; then
  expected=$(sed -n "s/^\([0-9a-fA-F]*\)[[:space:]].*$asset\$/\1/p" "$checksums" | head -n 1)
  if [ -n "$expected" ]; then
    if command -v sha256sum >/dev/null 2>&1; then
      actual=$(sha256sum "$bundle" | cut -d' ' -f1)
    else
      need shasum
      actual=$(shasum -a 256 "$bundle" | cut -d' ' -f1)
    fi
    [ "$actual" = "$expected" ] || die "checksum mismatch for $asset (expected $expected, got $actual)"
    log "checksum verified"
  else
    warn "no '$asset' entry in $checksums_asset; skipping verification"
  fi
else
  warn "no $checksums_asset on the release; skipping verification"
fi

# ------------------------------------------------------- stop before replace
stop_service() {
  if [ "$os" = linux ] && command -v systemctl >/dev/null 2>&1; then
    if [ "$am_root" = 1 ]; then
      systemctl stop palstudio.service 2>/dev/null || true
    else
      systemctl --user stop palstudio.service 2>/dev/null || true
    fi
  elif [ "$os" = macos ] && command -v launchctl >/dev/null 2>&1; then
    launchctl unload "$HOME/Library/LaunchAgents/app.palstudio.server.plist" 2>/dev/null || true
  fi
}

# ---------------------------------------------------------------- install
stage="$tmp/stage"
mkdir -p "$stage"
tar -xzf "$bundle" -C "$stage"
[ -d "$stage/palstudio/bin" ] || die "bundle layout error: no palstudio/bin inside $asset"

stop_service
for dir in bin ui data; do
  [ -d "$stage/palstudio/$dir" ] || continue
  rm -rf "$PREFIX/$dir.new"
  mv "$stage/palstudio/$dir" "$PREFIX/$dir.new"
  [ -d "$PREFIX/$dir" ] && mv "$PREFIX/$dir" "$PREFIX/$dir.old" || true
  mv "$PREFIX/$dir.new" "$PREFIX/$dir"
  rm -rf "$PREFIX/$dir.old"
done
binary="$PREFIX/bin/palstudio"
chmod +x "$binary"
ln -sf "$binary" "$bindir/palstudio"
log "installed under $PREFIX (command: $bindir/palstudio)"

case ":$PATH:" in
  *":$bindir:"*) ;;
  *) warn "$bindir is not on your PATH — add it to run 'palstudio' from a shell" ;;
esac

# ---------------------------------------------------------------- mode
# Interactive boxes get the question; curl|sh pipelines and cron jobs never
# see a TTY and default to the background service.
if [ -z "$MODE" ]; then
  if [ -t 0 ] && [ -t 1 ]; then
    MODE=ask
  else
    MODE=service
  fi
fi
if [ "$MODE" = ask ]; then
  printf '\nHow do you want to run PalStudio?\n'
  printf '  1) standalone      — run `palstudio` when you need it (desktop app by default; \\`palstudio webapp\\` for the browser)\n'
  printf '  2) background service — always-on webapp server (systemd/launchd)\n'
  printf 'Choice [1]: '
  read -r answer 2>/dev/null || answer=1
  case "$answer" in
    2|s|service) MODE=service ;;
    *) MODE=standalone ;;
  esac
fi

# --------------------------------------------------------------- service
# The service always runs the headless webapp (`serve`); standalone users
# get the mode picker on first `palstudio` launch.
server_args="serve --host $HOST --port $PORT --ui-dir $PREFIX/ui --data-dir $PREFIX/data --db $PREFIX/ps-rs.db"
network_env=""
[ -n "$LISTEN" ] && network_env="Environment=PS_LISTEN=$LISTEN"$'\n'
[ -n "$PIN" ] && network_env="${network_env}Environment=PS_PIN=$PIN"$'\n'

if [ "$MODE" = service ]; then
if [ "$os" = linux ] && command -v systemctl >/dev/null 2>&1; then
  unit=$([ "$am_root" = 1 ] && echo /etc/systemd/system/palstudio.service \
                                  || echo "$HOME/.config/systemd/user/palstudio.service")
  mkdir -p "$(dirname "$unit")"
  if [ "$am_root" = 1 ]; then
    cat >"$unit" <<EOF
[Unit]
Description=PalStudio server
After=network.target

[Service]
Environment=PALSTUDIO_SERVICE=1
${network_env}ExecStart=$binary $server_args
WorkingDirectory=$PREFIX
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF
    systemctl daemon-reload
    systemctl enable --now palstudio.service
  else
    systemctl --user status >/dev/null 2>&1 || die "systemd --user is unavailable (no dbus session); rerun with MODE=standalone or from a login session"
    cat >"$unit" <<EOF
[Unit]
Description=PalStudio server

[Service]
Environment=PALSTUDIO_SERVICE=1
${network_env}ExecStart=$binary $server_args
WorkingDirectory=$PREFIX
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
EOF
    systemctl --user daemon-reload
    loginctl enable-linger "$USER" 2>/dev/null || \
      warn "could not enable lingering; the service stops when you log out"
    systemctl --user enable --now palstudio.service
  fi
  log "systemd service enabled and started"
elif [ "$os" = macos ]; then
  plist="$HOME/Library/LaunchAgents/app.palstudio.server.plist"
  mkdir -p "$(dirname "$plist")"
  log_out="$PREFIX/palstudio.log"
  listen_env=""
  [ -n "$LISTEN" ] && listen_env="    <key>PS_LISTEN</key><string>$LISTEN</string>"
  pin_env=""
  [ -n "$PIN" ] && pin_env="    <key>PS_PIN</key><string>$PIN</string>"
  cat >"$plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>app.palstudio.server</string>
  <key>ProgramArguments</key>
  <array>
    <string>$binary</string>
    <string>serve</string>
    <string>--host</string><string>$HOST</string>
    <string>--port</string><string>$PORT</string>
    <string>--ui-dir</string><string>$PREFIX/ui</string>
    <string>--data-dir</string><string>$PREFIX/data</string>
    <string>--db</string><string>$PREFIX/ps-rs.db</string>
  </array>
  <key>WorkingDirectory</key>
  <string>$PREFIX</string>
  <key>EnvironmentVariables</key>
  <dict>
    <key>PALSTUDIO_SERVICE</key><string>1</string>
$listen_env
$pin_env
  </dict>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>StandardOutPath</key>
  <string>$log_out</string>
  <key>StandardErrorPath</key>
  <string>$log_out</string>
</dict>
</plist>
EOF
  launchctl load -w "$plist"
  log "launchd agent loaded (starts at login, kept alive; log: $log_out)"
else
  nohup "$binary" $server_args >"$PREFIX/palstudio.log" 2>&1 &
  log "no service manager found; started in background (log: $PREFIX/palstudio.log)"
fi
else
  log "standalone mode — no service installed"
  log "run 'palstudio' to choose desktop or webapp (remembered), or 'palstudio serve' for the headless server"
fi

# ---------------------------------------------------------------- wait + url
printf '\n'
if [ "$MODE" = service ]; then
  log "waiting for the server to accept connections"
  i=0
  while [ "$i" -lt 30 ]; do
    if curl -fsS -o /dev/null "http://127.0.0.1:$PORT/" 2>/dev/null; then
      break
    fi
    i=$((i + 1))
    sleep 1
  done
  printf '  PalStudio %s is running: http://127.0.0.1:%s\n' "$VERSION" "$PORT"
  [ "$HOST" = 127.0.0.1 ] || printf '  (bound to %s — reachable on your network)\n' "$HOST"
  printf '  Logs/data live under %s. To stop: ' "$PREFIX"
  if [ "$os" = linux ] && command -v systemctl >/dev/null 2>&1; then
    if [ "$am_root" = 1 ]; then printf 'sudo systemctl stop palstudio\n'; else printf 'systemctl --user stop palstudio\n'; fi
  elif [ "$os" = macos ]; then
    printf 'launchctl unload ~/Library/LaunchAgents/app.palstudio.server.plist\n'
  else
    printf "kill \$(pgrep -f '$binary serve')\n"
  fi
else
  printf '  PalStudio %s installed under %s.\n' "$VERSION" "$PREFIX"
  printf '  Start it with: palstudio          (desktop/webapp picker)\n'
  printf '                 palstudio webapp   (server + browser)\n'
  printf '                 palstudio serve    (headless server)\n'
fi
