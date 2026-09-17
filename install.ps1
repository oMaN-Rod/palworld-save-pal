# PalStudio desktop — one-line installer (Windows PowerShell).
#
#   irm https://palstudio.app/install | iex
#
# palstudio.app/install sniffs the caller and serves this script to
# PowerShell; the actual artifacts always come from the GitHub releases.
# Installs the standalone zip (launcher CLI + desktop app + web UI + game
# data) under %LOCALAPPDATA%\PalStudio, adds bin\ to the user PATH, and
# creates Start Menu / Desktop shortcuts to the desktop app.
#
# Prefer the MSI installer instead?
#   $env:PALSTUDIO_MSI = '1'; irm https://palstudio.app/install | iex
# or, with the script downloaded:
#   irm https://palstudio.app/install.ps1 -OutFile install.ps1
#   .\install.ps1 -Msi
#
# Direct downloads (MSI, zip), the deb, etc. all stay available on the
# releases page: https://github.com/oMaN-Rod/palworld-save-pal/releases
# Server/headless installs use scripts/install-server.ps1 — see docs/install.md.
#
# Environment overrides (set before invoking, e.g. `$env:PALSTUDIO_VERSION='v1.4.2'`):
#   PALSTUDIO_MSI=1             install the MSI instead of the zip
#   PALSTUDIO_VERSION           pin a release tag (default: latest)
#   PALSTUDIO_REPO              GitHub owner/name (default oMaN-Rod/palworld-save-pal)
#   PALSTUDIO_API_BASE          GitHub API base (default https://api.github.com)
#   PALSTUDIO_DOWNLOAD_BASE     release download base (default https://github.com)
#   PALSTUDIO_INSTALL_DIR       zip install location (default %LOCALAPPDATA%\PalStudio)
#   PALSTUDIO_SKIP_SHORTCUTS=1  do not create Start Menu / Desktop shortcuts
param([switch]$Msi)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# Windows PowerShell 5.1 on older Windows 10 builds may default to TLS 1.0.
try {
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
} catch {}

# `$IsWindows` only exists on PowerShell 6+; RuntimeInformation works everywhere.
$onWindows = [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
    [System.Runtime.InteropServices.OSPlatform]::Windows)

function Write-Info($msg) { Write-Host "==> $msg" -ForegroundColor Cyan }
function Write-Warn($msg) { Write-Host "warning: $msg" -ForegroundColor Yellow }
function Die($msg) { Write-Host "error: $msg" -ForegroundColor Red; exit 1 }

$Repo = if ($env:PALSTUDIO_REPO) { $env:PALSTUDIO_REPO } else { 'oMaN-Rod/palworld-save-pal' }
$Version = $env:PALSTUDIO_VERSION
$ApiBase = if ($env:PALSTUDIO_API_BASE) { $env:PALSTUDIO_API_BASE } else { 'https://api.github.com' }
$DlBase = if ($env:PALSTUDIO_DOWNLOAD_BASE) { $env:PALSTUDIO_DOWNLOAD_BASE } else { 'https://github.com' }
$UseMsi = $Msi.IsPresent -or $env:PALSTUDIO_MSI -eq '1'

# --------------------------------------------------------------- host info
switch ($env:PROCESSOR_ARCHITECTURE) {
    'AMD64' { }
    'ARM64' { Die 'Windows ARM64 has no prebuilt build yet; build from source: https://github.com/oMaN-Rod/palworld-save-pal' }
    default { Die "unsupported architecture '$($env:PROCESSOR_ARCHITECTURE)'" }
}

# ---------------------------------------------------------------- version
if (-not $Version) {
    Write-Info "looking up the latest release of $Repo"
    $latest = Invoke-RestMethod -Headers @{ 'User-Agent' = 'palstudio-installer' } `
        -Uri "$ApiBase/repos/$Repo/releases/latest"
    if (-not $latest.tag_name) { Die 'could not resolve the latest release tag from GitHub' }
    $Version = $latest.tag_name
}
$kind = if ($UseMsi) { 'windows.msi' } else { 'windows-standalone.zip' }
$Asset = "PalStudio-$Version-$kind"
$ChecksumsAsset = "PalStudio-$Version-checksums.txt"
Write-Info "installing PalStudio desktop $Version ($kind)"

# ---------------------------------------------------------------- download
$Base = "$DlBase/$Repo/releases/download/$Version"
$tmp = New-Item -ItemType Directory -Force -Path (Join-Path ([System.IO.Path]::GetTempPath()) "palstudio-install-$(Get-Random)")
$download = Join-Path $tmp $Asset
Write-Info "downloading $Base/$Asset"
try {
    Invoke-WebRequest -UseBasicParsing -Uri "$Base/$Asset" -OutFile $download
} catch {
    Die "download failed - does $Version ship a $kind asset? ($_)"
}

$checksums = Join-Path $tmp $ChecksumsAsset
try {
    Invoke-WebRequest -UseBasicParsing -Uri "$Base/$ChecksumsAsset" -OutFile $checksums
    $expected = (Get-Content $checksums | Where-Object { $_ -match "\s$([regex]::Escape($Asset))$" } |
        Select-Object -First 1) -replace '\s.*$', ''
    if ($expected) {
        $actual = (Get-FileHash -Algorithm SHA256 $download).Hash.ToLower()
        if ($actual -ne $expected.ToLower()) {
            Die "checksum mismatch for $Asset (expected $expected, got $actual)"
        }
        Write-Info 'checksum verified'
    } else {
        Write-Warn "no '$Asset' entry in $ChecksumsAsset; skipping verification"
    }
} catch {
    Write-Warn "no $ChecksumsAsset on the release; skipping verification"
}

# ---------------------------------------------------------------- install
if ($UseMsi) {
    Write-Info 'running the MSI installer (a UAC prompt may appear)'
    # msiexec elevates itself when the package needs it; /passive shows the
    # progress bar only, /norestart never reboots the machine under us.
    $proc = Start-Process msiexec -ArgumentList '/i', "`"$download`"", '/passive', '/norestart' -Wait -PassThru
    if ($proc.ExitCode -ne 0 -and $proc.ExitCode -ne 3010) {
        Die "msiexec failed with exit code $($proc.ExitCode) (0 or 3010 = success)"
    }
    Write-Host ''
    Write-Host "  PalStudio $Version installed (MSI)." -ForegroundColor Green
    Write-Host '  Launch it from the Start Menu.'
    exit 0
}

$installDir = if ($env:PALSTUDIO_INSTALL_DIR) { $env:PALSTUDIO_INSTALL_DIR }
              else { Join-Path $env:LOCALAPPDATA 'PalStudio' }

# Replacing files under a running instance fails on Windows; stop both first.
Get-Process -Name 'palstudio', 'palstudio-desktop' -ErrorAction SilentlyContinue |
    Stop-Process -Force -ErrorAction SilentlyContinue

Write-Info "installing under $installDir"
$extract = Join-Path $tmp 'extract'
Expand-Archive -Path $download -DestinationPath $extract
$staged = Join-Path $extract 'PalStudio'
if (-not (Test-Path (Join-Path $staged 'bin'))) {
    Die "unexpected zip layout: no PalStudio\bin inside $Asset (pre-rebrand zips are not supported; use the latest release)"
}
New-Item -ItemType Directory -Force -Path $installDir | Out-Null
foreach ($dir in 'bin', 'ui_build', 'data') {
    $src = Join-Path $staged $dir
    if (-not (Test-Path $src)) { continue }
    $dst = Join-Path $installDir $dir
    if (Test-Path $dst) { Remove-Item -Recurse -Force $dst }
    Move-Item $src $dst
}
# The pre-rebrand zip kept the desktop exe at the install root; drop the
# leftover so only bin\palstudio-desktop.exe remains. ps-rs.db and any other
# user files at the root are preserved.
$legacyExe = Join-Path $installDir 'palstudio.exe'
if ((Test-Path $legacyExe) -and (Test-Path (Join-Path $installDir 'bin\palstudio.exe'))) {
    Remove-Item -Force $legacyExe
}

$binDir = Join-Path $installDir 'bin'
if ($onWindows) {
    # User PATH so `palstudio` works in new shells; the current shell keeps
    # its inherited PATH.
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if ($userPath -notlike "*$binDir*") {
        [Environment]::SetEnvironmentVariable('Path', "$userPath;$binDir", 'User')
        Write-Info "added $binDir to the user PATH (new terminals only)"
    }
}

if ($onWindows -and $env:PALSTUDIO_SKIP_SHORTCUTS -ne '1') {
    $desktopExe = Join-Path $binDir 'palstudio-desktop.exe'
    # WorkingDirectory = the install root, where the unpackaged desktop app
    # finds ui_build\ and data\.
    $shell = New-Object -ComObject WScript.Shell
    foreach ($shortcutDir in @(
            Join-Path ([Environment]::GetFolderPath('Programs')) 'PalStudio',
            [Environment]::GetFolderPath('Desktop'))) {
        New-Item -ItemType Directory -Force -Path $shortcutDir | Out-Null
        $shortcut = $shell.CreateShortcut((Join-Path $shortcutDir 'PalStudio.lnk'))
        $shortcut.TargetPath = $desktopExe
        $shortcut.WorkingDirectory = $installDir
        $shortcut.Description = 'PalStudio desktop app'
        $shortcut.Save()
    }
    Write-Info 'Start Menu and Desktop shortcuts created'
}

Write-Host ''
Write-Host "  PalStudio $Version installed under $installDir." -ForegroundColor Green
Write-Host '  Start it from the Start Menu / Desktop shortcut, and in a NEW terminal:'
Write-Host '    palstudio            (desktop/webapp picker)'
Write-Host '    palstudio webapp     (server + browser)'
Write-Host '    palstudio serve      (headless server)'
Write-Host '  Your database stays at ps-rs.db inside the install dir.'
