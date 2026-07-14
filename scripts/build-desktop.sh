#!/usr/bin/env bash
# Builds the desktop artifact into dist/ for the current OS:
#   macOS:  PalworldSavePal-<version>-macos.dmg
#   Linux:  PalworldSavePal-<version>-linux.deb
#
# Usage: ./scripts/build-desktop.sh [--skip-ui]   (--skip-ui if ui_build is current)
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if ! cargo tauri --version >/dev/null 2>&1; then
  echo "cargo-tauri not found. Install it: cargo install tauri-cli --version '^2' --locked" >&2
  exit 1
fi

version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"

case "$(uname -s)" in
  Darwin) bundle="dmg"; os="macos"; ext="dmg" ;;
  Linux)  bundle="deb"; os="linux"; ext="deb" ;;
  *) echo "Unsupported OS $(uname -s); use scripts/build-desktop.ps1 on Windows." >&2; exit 1 ;;
esac

echo "Building Palworld Save Pal desktop v$version ($os)"

if [ "${1:-}" != "--skip-ui" ]; then
  bash "$repo_root/scripts/build-ui-desktop.sh"
fi

( cd psp-desktop && cargo tauri build --bundles "$bundle" )

mkdir -p dist
artifact="$(ls "target/release/bundle/$bundle/"*."$ext" | head -n 1)"
cp "$artifact" "dist/PalworldSavePal-$version-$os.$ext"
echo "Done: dist/PalworldSavePal-$version-$os.$ext"
