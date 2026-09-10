param(
    [string]$Root = "",
    [string]$RepoModDir = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path,
    [string]$Ue4ssRepo = "https://github.com/Okaetsu/RE-UE4SS.git",
    [string]$Ue4ssRef = "palworld",
    # The fork's source commit the mod is built against; the UE4SS runtime it produces reports
    # its own SHA (v3.0.1 Beta, c838a8ac), which is what UE4SS.log shows.
    [string]$Ue4ssCommit = "2281fa311e417b1dfddedbcd49972d764fddb244"
)
$ErrorActionPreference = "Stop"
if (-not $Root) {
    $repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
    $Root = Join-Path (Split-Path -Parent $repoRoot) "amity-build"
}
New-Item -ItemType Directory -Force $Root | Out-Null

$ue4ss = Join-Path $Root "RE-UE4SS"
if (-not (Test-Path $ue4ss)) {
    git clone --branch $Ue4ssRef --single-branch $Ue4ssRepo $ue4ss
    if ($LASTEXITCODE -ne 0) { throw "clone failed" }
}
git -C $ue4ss checkout $Ue4ssCommit
if ($LASTEXITCODE -ne 0) { throw "checkout $Ue4ssCommit failed" }
git -C $ue4ss submodule update --init --recursive
if ($LASTEXITCODE -ne 0) { throw "submodule init failed (UEPseudo access?)" }

$junction = Join-Path $Root "PSAmity"
if (-not (Test-Path $junction)) {
    New-Item -ItemType Junction -Path $junction -Target $RepoModDir | Out-Null
}

Set-Content -Path (Join-Path $Root "CMakeLists.txt") -Encoding UTF8 -Value @'
cmake_minimum_required(VERSION 3.22)
project(AmityWorkspace)
add_subdirectory(RE-UE4SS)
add_subdirectory(PSAmity)
'@
Write-Host "workspace ready: $Root"
