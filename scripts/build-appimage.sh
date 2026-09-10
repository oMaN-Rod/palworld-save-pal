#!/usr/bin/env bash
# Usage: ./scripts/build-appimage.sh
#
# The release CI's AppImage steps, runnable locally: a Tauri appimage bundle,
# then appimage-strip-graphics.sh, so a local AppImage behaves like a shipped one.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if [ "$(uname -s)" != "Linux" ]; then
  echo "AppImages can only be built on Linux (got $(uname -s))." >&2
  exit 1
fi

if ! cargo tauri --version >/dev/null 2>&1; then
  echo "cargo-tauri not found. Install it: cargo install tauri-cli --version '^2' --locked" >&2
  exit 1
fi

version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
echo "Building PalStudio AppImage v$version"

(cd ps-ui && bun install)

# linuxdeploy's bundled strip predates SHT_RELR and aborts on the libraries of
# current distros ("unknown type [0x13]"). Those libraries ship stripped.
export NO_STRIP=1
# Tauri's beforeBuildCommand builds the desktop UI, as in CI.
(cd ps-desktop && cargo tauri build --bundles appimage)

shopt -s nullglob
appimages=(target/release/bundle/appimage/*_"$version"_*.AppImage)
if [ "${#appimages[@]}" -ne 1 ]; then
  echo "Expected one AppImage for v$version in target/release/bundle/appimage, found ${#appimages[@]}." >&2
  exit 1
fi
appimage="${appimages[0]}"

bash scripts/appimage-strip-graphics.sh "$appimage"

mkdir -p dist
cp "$appimage" "dist/PalStudio-$version-linux.AppImage"
echo "Done: dist/PalStudio-$version-linux.AppImage"
