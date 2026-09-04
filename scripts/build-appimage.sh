#!/usr/bin/env bash
# Usage: ./scripts/build-appimage.sh [--skip-ui]   (--skip-ui if ui_build is current)
#
# Mirrors the release CI's Linux job: Tauri appimage bundle followed by the
# host-graphics strip (appimage-strip-graphics.sh), so a locally built AppImage
# behaves like the shipped one instead of reshipping Ubuntu-era wayland/glvnd
# libs that break Mesa driver loading on other distros. Linux-only — tauri's
# appimage bundler targets the host OS.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if [ "$(uname -s)" != "Linux" ]; then
  echo "AppImage bundles can only be built on Linux (got $(uname -s))." >&2
  exit 1
fi

if ! cargo tauri --version >/dev/null 2>&1; then
  echo "cargo-tauri not found. Install it: cargo install tauri-cli --version '^2' --locked" >&2
  exit 1
fi

version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"

echo "Building Palworld Save Pal AppImage v$version"

if [ "${1:-}" != "--skip-ui" ]; then
  bash "$repo_root/scripts/build-ui-desktop.sh"
fi

# linuxdeploy's bundled strip (2018-era binutils) dies on the `.relr.dyn`
# section (SHT_RELR) that modern distro libraries are built with — every
# bundled .so fails with "unknown type [0x13]" and the bundle aborts with the
# opaque "failed to run linuxdeploy". NO_STRIP skips that pass; it costs
# nothing here because distro libs are already package-stripped, and the psp
# binary itself carries no RELR entries.
export NO_STRIP=1

( cd psp-desktop && cargo tauri build --bundles appimage )

appimage="$(ls "target/release/bundle/appimage/"*.AppImage | head -n 1)"
bash "$repo_root/scripts/appimage-strip-graphics.sh" "$appimage"

mkdir -p dist
cp "$appimage" "dist/PalworldSavePal-$version-linux.AppImage"
echo "Done: dist/PalworldSavePal-$version-linux.AppImage"
