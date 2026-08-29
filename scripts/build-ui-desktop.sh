#!/usr/bin/env bash
# The desktop env (PUBLIC_DESKTOP_MODE=true) and the build are owned by the
# psp-ui `build:desktop` script so local, CI, and Tauri's beforeBuildCommand agree.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$repo_root/psp-ui"
bun install
bun run build:desktop

echo "Desktop UI built to $repo_root/ui_build"
