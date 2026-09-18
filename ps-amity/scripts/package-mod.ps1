param(
    [string]$Root = "",
    [string]$Config = "Game__Shipping__Win64",
    [switch]$Bundle
)
$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

$versionLine = Select-String -Path (Join-Path $repoRoot "mod\amity_mod.hpp") -Pattern '#define AMITY_VERSION "([^"]+)"'
if (-not $versionLine) { throw "AMITY_VERSION not found in mod\amity_mod.hpp" }
$Version = $versionLine.Matches[0].Groups[1].Value

$buildArgs = @{ Config = $Config }
if ($Root) { $buildArgs.Root = $Root }
& (Join-Path $PSScriptRoot "build.ps1") @buildArgs | Tee-Object -Variable buildOut
$dll = ($buildOut | Select-String "^MOD_DLL=(.+)$").Matches[0].Groups[1].Value
if (-not $dll) { throw "build.ps1 did not report MOD_DLL" }

$distDir = Join-Path $repoRoot "dist"
$stageDir = Join-Path $distDir "stage"
if (Test-Path $stageDir) { Remove-Item -Recurse -Force $stageDir }
$modDir = Join-Path $stageDir "PSAmity"
New-Item -ItemType Directory -Force (Join-Path $modDir "dlls") | Out-Null

Copy-Item $dll (Join-Path $modDir "dlls\main.dll") -Force
Set-Content -Path (Join-Path $modDir "enabled.txt") -Value "" -NoNewline -Encoding ascii
Copy-Item (Join-Path $repoRoot "THIRD_PARTY_NOTICES.md") (Join-Path $modDir "THIRD_PARTY_NOTICES.md") -Force
Copy-Item (Join-Path $repoRoot "dist\PSAmity.ini") (Join-Path $modDir "PSAmity.ini") -Force

$zipPath = Join-Path $distDir "PSAmity-UE4SS-$Version.zip"
if (Test-Path $zipPath) { Remove-Item -Force $zipPath }
Compress-Archive -Path (Join-Path $stageDir "PSAmity") -DestinationPath $zipPath

if ($Bundle) {
    $bundleDir = Join-Path (Split-Path -Parent $repoRoot) "ps-desktop\resources\amity"
    New-Item -ItemType Directory -Force $bundleDir | Out-Null
    Get-ChildItem $bundleDir -Filter "PSAmity-UE4SS-*.zip" | Remove-Item -Force
    Copy-Item $zipPath $bundleDir -Force
    Write-Host "BUNDLED=$(Join-Path $bundleDir (Split-Path -Leaf $zipPath))"
}

Write-Host "ZIP=$zipPath"
