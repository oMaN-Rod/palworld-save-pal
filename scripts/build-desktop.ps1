# Usage: .\scripts\build-desktop.ps1 [-SkipUi]   (-SkipUi if ui_build is current)
#
# The portable zip mirrors the release workflow's standalone: launcher CLI in
# bin\ (all verbs), desktop app in bin\palstudio-desktop.exe, plus ui_build\
# and data\ which the unpackaged desktop app resolves from its working
# directory. Requires the Microsoft Edge WebView2 runtime (present on
# up-to-date Windows 10/11).
param([switch]$SkipUi)
$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $repoRoot

if (-not (Get-Command "cargo-tauri" -ErrorAction SilentlyContinue) -and
    -not (cargo tauri --version 2>$null)) {
    throw "cargo-tauri not found. Install it: cargo install tauri-cli --version '^2' --locked"
}

$version = (Select-String -Path "Cargo.toml" -Pattern '^version = "([^"]*)"').Matches[0].Groups[1].Value
Write-Host "Building PalStudio desktop v$version (windows)"

Push-Location "ps-desktop"
try {
    cargo tauri build --bundles msi
    if ($LASTEXITCODE -ne 0) { throw "cargo tauri build failed" }
}
finally { Pop-Location }

# tauri-cli renames the cargo output (palstudio-desktop.exe) to mainBinaryName
# (palstudio.exe) while packaging; grab whichever spelling is on disk before
# the launcher build claims the palstudio.exe path.
$desktopExe = if (Test-Path "target/release/palstudio-desktop.exe") {
    "target/release/palstudio-desktop.exe"
} else {
    "target/release/palstudio.exe"
}

cargo build --release --package palstudio
if ($LASTEXITCODE -ne 0) { throw "cargo build -p palstudio failed" }

$dist = Join-Path $repoRoot "dist"
New-Item -ItemType Directory -Force -Path $dist | Out-Null

$msi = Get-ChildItem "target/release/bundle/msi/*.msi" | Select-Object -First 1
Copy-Item $msi.FullName (Join-Path $dist "PalStudio-$version-windows.msi")

$staging = Join-Path $dist "PalStudio"
if (Test-Path $staging) { Remove-Item -Recurse -Force $staging }
New-Item -ItemType Directory -Force -Path (Join-Path $staging "bin") | Out-Null
Copy-Item "target/release/palstudio.exe" (Join-Path $staging "bin/palstudio.exe")
Copy-Item $desktopExe (Join-Path $staging "bin/palstudio-desktop.exe")
Copy-Item -Recurse "ui_build" (Join-Path $staging "ui_build")
Copy-Item -Recurse "data" (Join-Path $staging "data")

$zip = Join-Path $dist "PalStudio-$version-windows-standalone.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path $staging -DestinationPath $zip
Remove-Item -Recurse -Force $staging

Write-Host "Done. Artifacts in dist/:"
Get-ChildItem $dist -Filter "PalStudio-$version-windows*" | ForEach-Object { Write-Host "  $($_.Name)" }
