#!/usr/bin/env bash
# Builds the SvelteKit UI for the desktop app (Rust/Tauri or Python) into ui_build/.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

printf "PUBLIC_WS_URL=127.0.0.1:5174/ws\nPUBLIC_DESKTOP_MODE=true" > "$repo_root/ui/.env"

cd "$repo_root/ui"
bun install
bun run build

echo "Desktop UI built to $repo_root/ui_build"
