# Builds the Windows desktop artifacts into dist/:
#   PalworldSavePal-<version>-windows.msi              MSI installer
#   PalworldSavePal-<version>-windows-standalone.zip   portable (psp.exe + ui_build + data)
#
# The portable build runs extract-and-run: launch psp.exe from the
# extracted folder — it serves the bundled ui_build/ and keeps its psp-rs.db
# alongside the exe. Requires the Microsoft Edge WebView2 runtime (present on
# up-to-date Windows 10/11).
#
# Usage: .\scripts\build-desktop.ps1 [-SkipUi]   (-SkipUi if ui_build is current)
param([switch]$SkipUi)
$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $repoRoot

if (-not (Get-Command "cargo-tauri" -ErrorAction SilentlyContinue) -and
    -not (cargo tauri --version 2>$null)) {
    throw "cargo-tauri not found. Install it: cargo install tauri-cli --version '^2' --locked"
}

$version = (Select-String -Path "Cargo.toml" -Pattern '^version = "([^"]*)"').Matches[0].Groups[1].Value
Write-Host "Building Palworld Save Pal desktop v$version (windows)"

if (-not $SkipUi) {
    & (Join-Path $PSScriptRoot "build-ui-desktop.ps1")
}

Push-Location "psp-desktop"
try {
    cargo tauri build --bundles msi
    if ($LASTEXITCODE -ne 0) { throw "cargo tauri build failed" }
}
finally { Pop-Location }

$dist = Join-Path $repoRoot "dist"
New-Item -ItemType Directory -Force -Path $dist | Out-Null

# MSI installer.
$msi = Get-ChildItem "target/release/bundle/msi/*.msi" | Select-Object -First 1
Copy-Item $msi.FullName (Join-Path $dist "PalworldSavePal-$version-windows.msi")

# Portable standalone: exe + ui_build + data in one folder, zipped.
$staging = Join-Path $dist "PalworldSavePal"
if (Test-Path $staging) { Remove-Item -Recurse -Force $staging }
New-Item -ItemType Directory -Force -Path $staging | Out-Null
Copy-Item "target/release/psp.exe" (Join-Path $staging "psp.exe")
Copy-Item -Recurse "ui_build" (Join-Path $staging "ui_build")
Copy-Item -Recurse "data" (Join-Path $staging "data")

$zip = Join-Path $dist "PalworldSavePal-$version-windows-standalone.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path $staging -DestinationPath $zip
Remove-Item -Recurse -Force $staging

Write-Host "Done. Artifacts in dist/:"
Get-ChildItem $dist -Filter "PalworldSavePal-$version-windows*" | ForEach-Object { Write-Host "  $($_.Name)" }
