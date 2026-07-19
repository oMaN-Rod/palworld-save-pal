# Builds the SvelteKit UI for the desktop app (Rust/Tauri or Python) into ui_build/.
# The desktop env (PUBLIC_DESKTOP_MODE=true) and the build are owned by the
# ui `build:desktop` script so local, CI, and Tauri's beforeBuildCommand agree.
$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot

Push-Location (Join-Path $repoRoot "ui")
try {
    bun install
    if ($LASTEXITCODE -ne 0) { throw "bun install failed" }
    bun run build:desktop
    if ($LASTEXITCODE -ne 0) { throw "bun run build:desktop failed" }
}
finally {
    Pop-Location
}

Write-Host "Desktop UI built to $(Join-Path $repoRoot 'ui_build')"
