# Single source of truth: the [workspace.package] version in Cargo.toml. Every
# crate inherits it (version.workspace = true), the get_version wire reply is
# CARGO_PKG_VERSION, and the Tauri bundle version derives from Cargo (no version
# in tauri.conf.json), so this one line drives the whole project.
param(
    [string]$Version,
    [switch]$Help
)

if ($Help -or -not $Version) {
    Write-Host "Usage: .\scripts\bump.ps1 <version>   e.g. .\scripts\bump.ps1 0.18.0"
    Write-Host "Bumps the [workspace.package] version in Cargo.toml."
    if ($Help) { exit 0 } else { exit 1 }
}

if ($Version -notmatch '^\d+\.\d+\.\d+(-[a-zA-Z0-9\-\.]+)?(\+[a-zA-Z0-9\-\.]+)?$') {
    Write-Error "Invalid version '$Version'. Use semantic versioning, e.g. 0.18.0"
    exit 1
}

$cargoToml = Join-Path $PSScriptRoot "..\Cargo.toml"
$content = Get-Content -Path $cargoToml -Raw
# Only the [workspace.package] line starts with `version = ` at column 0;
# dependency versions live inside `{ ... }` tables and are never anchored here.
if ($content -notmatch '(?m)^version = "[^"]*"') {
    Write-Error "Could not find the [workspace.package] version line in $cargoToml"
    exit 1
}
$updated = $content -replace '(?m)^version = "[^"]*"', "version = `"$Version`""
Set-Content -Path $cargoToml -Value $updated -NoNewline
Write-Host "Bumped project version to $Version (Cargo.toml [workspace.package])"

# The MSI bundler rejects a pre-release identifier that isn't numeric, so the
# installer carries just the numeric core while the app keeps the full string.
# Kept in step here because a version hardcoded in two places always drifts.
$tauriConf = Join-Path $PSScriptRoot "..\ps-desktop\tauri.conf.json"
$numericVersion = [regex]::Match($Version, '^\d+\.\d+\.\d+').Value
$conf = Get-Content -Path $tauriConf -Raw
$wixVersion = '("wix"\s*:\s*\{\s*"version"\s*:\s*")[^"]*(")'
if ($conf -notmatch $wixVersion) {
    Write-Error "Could not find bundle.windows.wix.version in $tauriConf"
    exit 1
}
$conf = [regex]::Replace($conf, $wixVersion, '${1}' + $numericVersion + '${2}')
Set-Content -Path $tauriConf -Value $conf -NoNewline
Write-Host "Set the MSI installer version to $numericVersion (tauri.conf.json bundle.windows.wix.version)"
