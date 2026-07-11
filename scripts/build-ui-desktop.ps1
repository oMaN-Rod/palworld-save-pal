# Builds the SvelteKit UI for the desktop app (Rust/Tauri or Python) into ui_build/.
$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot

Set-Content -Path (Join-Path $repoRoot "ui/.env") -Value "PUBLIC_WS_URL=127.0.0.1:5174/ws`nPUBLIC_DESKTOP_MODE=true"

Push-Location (Join-Path $repoRoot "ui")
try {
    bun install
    if ($LASTEXITCODE -ne 0) { throw "bun install failed" }
    bun run build
    if ($LASTEXITCODE -ne 0) { throw "bun run build failed" }
}
finally {
    Pop-Location
}

Write-Host "Desktop UI built to $(Join-Path $repoRoot 'ui_build')"
