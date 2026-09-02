#!/usr/bin/env bash
# Usage: ./scripts/build-desktop-browser.sh [--skip-ui]   (--skip-ui if ui_build is current)
#
# Builds the Linux browser-mode AppImage: same embedded psp-server, no Tauri
# webview — a terminal launcher that opens the system browser instead (see
# psp-desktop/src/browser_mode.rs). Sibling of build-desktop.sh, which builds
# the normal webview bundle.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if [ "$(uname -s)" != "Linux" ]; then
  echo "browser-mode is Linux-only; build the webview app with scripts/build-desktop.sh." >&2
  exit 1
fi

if ! cargo tauri --version >/dev/null 2>&1; then
  echo "cargo-tauri not found. Install it: cargo install tauri-cli --version '^2' --locked" >&2
  exit 1
fi

version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"

echo "Building Palworld Save Pal browser-mode v$version (linux AppImage)"

if [ "${1:-}" != "--skip-ui" ]; then
  bash "$repo_root/scripts/build-ui-desktop.sh"
fi

# tauri.browser-mode.conf.json overrides productName so the browser-mode
# AppImage can sit beside the webview one in dist/; --features selects the
# terminal launcher instead of the webview window.
#
# NO_STRIP: linuxdeploy's bundled strip predates DT_RELR (.relr.dyn) sections,
# which current Arch-family system libraries use — every strip call fails and
# the bundle aborts. Skipping strip also matches the workspace's keep-symbols
# policy (backtraces for save-corruption crash reports).
( cd psp-desktop && NO_STRIP=true cargo tauri build --bundles appimage \
    --features browser-mode \
    --config tauri.browser-mode.conf.json )

mkdir -p dist
artifact="$(ls target/release/bundle/appimage/*.AppImage | head -n 1)"
cp "$artifact" "dist/PalworldSavePal-Browser-$version-linux.AppImage"
echo "Done: dist/PalworldSavePal-Browser-$version-linux.AppImage"
