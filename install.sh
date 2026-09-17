#!/bin/sh
# PalStudio desktop — one-line installer (Linux / macOS).
#
#   curl -fsSL https://palstudio.app/install | bash
#
# palstudio.app/install sniffs the caller and serves this script to curl;
# the actual artifacts always come from the GitHub releases. This script
# detects OS and architecture, then:
#   Linux  → downloads the AppImage into ~/.local/bin (/usr/local/bin as root)
#   macOS  → downloads the .dmg and copies PalStudio.app into /Applications
#            (the quarantine attribute is cleared so unsigned builds open)
#
# The .deb and every other artifact stay available on the releases page:
#   https://github.com/oMaN-Rod/palworld-save-pal/releases
# Server/headless installs (launcher CLI + systemd/launchd service) use
# scripts/install-server.sh — see docs/install.md.
#
# Environment overrides (for curl|bash, CI, or unattended boxes):
#   PALSTUDIO_VERSION        pin a release tag            (e.g. v1.4.2)
#   PALSTUDIO_REPO           GitHub owner/name            (default oMaN-Rod/palworld-save-pal)
#   PALSTUDIO_API_BASE       GitHub API base              (default https://api.github.com)
#   PALSTUDIO_DOWNLOAD_BASE  release download base        (default https://github.com)
#   PALSTUDIO_BIN_DIR        AppImage destination         (Linux; default ~/.local/bin,
#                                                          /usr/local/bin as root)
#   PALSTUDIO_APP_DIR        .app destination             (macOS; default /Applications,
#                                                          ~/Applications when unwritable)
set -eu

REPO="${PALSTUDIO_REPO:-oMaN-Rod/palworld-save-pal}"
VERSION="${PALSTUDIO_VERSION:-}"
API_BASE="${PALSTUDIO_API_BASE:-https://api.github.com}"
DL_BASE="${PALSTUDIO_DOWNLOAD_BASE:-https://github.com}"

log()  { printf '==> %s\n' "$*"; }
warn() { printf 'warning: %s\n' "$*" >&2; }
die()  { printf 'error: %s\n' "$*" >&2; exit 1; }

need() { command -v "$1" >/dev/null 2>&1 || die "$1 is required but not found"; }
need curl

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
if [ "$os" = linux ]; then
  [ "$arch" = x86_64 ] || die "no aarch64 Linux AppImage yet — use the server installer (scripts/install-server.sh, aarch64 bundles) or the releases page"
  asset_kind=linux.AppImage
else
  # The dmg is a universal binary: both arm64 and x86_64 Macs.
  asset_kind=macos.dmg
fi

# ---------------------------------------------------------------- version
if [ -z "$VERSION" ]; then
  log "looking up the latest release of $REPO"
  VERSION=$(curl -fsSL -H 'Accept: application/vnd.github+json' \
    "$API_BASE/repos/$REPO/releases/latest" \
    | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1)
  [ -n "$VERSION" ] || die "could not resolve the latest release tag from GitHub"
fi
asset="PalStudio-${VERSION}-${asset_kind}"
checksums_asset="PalStudio-${VERSION}-checksums.txt"
log "installing PalStudio desktop $VERSION ($os $arch)"

# ---------------------------------------------------------------- download
tmp=$(mktemp -d) || die "mktemp failed"
trap 'rm -rf "$tmp"' EXIT
download="$tmp/$asset"
base_url="$DL_BASE/$REPO/releases/download/$VERSION"
log "downloading $base_url/$asset"
curl -fSL --retry 3 -o "$download" "$base_url/$asset" \
  || die "download failed — does $VERSION ship a $asset_kind asset?"

# sha256 verification against the release's desktop checksums manifest.
checksums="$tmp/checksums.txt"
if curl -fSL --retry 2 -o "$checksums" "$base_url/$checksums_asset"; then
  # sha256sum lines are "<hash>  <name>"; compare the name as a plain string
  # so dots in the asset name cannot act as regex wildcards.
  expected=$(awk -v f="$asset" '$2 == f { print $1; exit }' "$checksums")
  if [ -n "$expected" ]; then
    if command -v sha256sum >/dev/null 2>&1; then
      actual=$(sha256sum "$download" | cut -d' ' -f1)
    else
      need shasum
      actual=$(shasum -a 256 "$download" | cut -d' ' -f1)
    fi
    [ "$actual" = "$expected" ] || die "checksum mismatch for $asset (expected $expected, got $actual)"
    log "checksum verified"
  else
    warn "no '$asset' entry in $checksums_asset; skipping verification"
  fi
else
  warn "no $checksums_asset on the release; skipping verification"
fi

# ---------------------------------------------------------------- install
if [ "$os" = linux ]; then
  if [ -z "${PALSTUDIO_BIN_DIR:-}" ]; then
    if [ "$(id -u)" = 0 ]; then bin_dir=/usr/local/bin; else bin_dir="$HOME/.local/bin"; fi
  else
    bin_dir="$PALSTUDIO_BIN_DIR"
  fi
  mkdir -p "$bin_dir"
  dest="$bin_dir/PalStudio.AppImage"
  # Stage beside the destination and rename over: replacing a running
  # AppImage in place would fail with ETXTBSY, a rename never does.
  cp "$download" "$dest.new"
  chmod 0755 "$dest.new"
  mv -f "$dest.new" "$dest"
  log "installed $dest"
  case ":$PATH:" in
    *":$bin_dir:"*) ;;
    *) warn "$bin_dir is not on your PATH — add it to launch PalStudio by name" ;;
  esac
  printf '\n'
  log "PalStudio $VERSION installed."
  printf '  Launch it:  %s\n' "$dest"
  printf '  AppImages need libfuse2; without it run:  %s --appimage-extract-and-run\n' "$dest"
  printf '  Menu integration (optional): AppImageLauncher or appimaged.\n'
else
  need hdiutil
  need xattr
  app_dir="${PALSTUDIO_APP_DIR:-/Applications}"
  if [ ! -w "$app_dir" ] 2>/dev/null; then
    app_dir="$HOME/Applications"
    mkdir -p "$app_dir"
    warn "/Applications is not writable; using $app_dir"
  fi
  mnt="$tmp/mnt"
  mkdir -p "$mnt"
  log "mounting the dmg"
  hdiutil attach -readonly -nobrowse -mountpoint "$mnt" "$download" >/dev/null \
    || die "could not mount $asset"
  app=$(find "$mnt" -maxdepth 1 -type d -name '*.app' | head -n 1)
  if [ -z "$app" ]; then
    hdiutil detach "$mnt" >/dev/null 2>&1 || true
    die "no .app found inside $asset"
  fi
  dest="$app_dir/$(basename "$app")"
  log "installing into $app_dir"
  rm -rf "$dest"
  cp -R "$app" "$dest"
  hdiutil detach "$mnt" >/dev/null 2>&1 || true
  # Unsigned/un-notarized builds would be blocked by Gatekeeper because the
  # dmg was downloaded; clearing the attribute lets them open normally.
  xattr -dr com.apple.quarantine "$dest" 2>/dev/null || true
  printf '\n'
  log "PalStudio $VERSION installed."
  printf '  Launch it:  open -a PalStudio\n'
fi
