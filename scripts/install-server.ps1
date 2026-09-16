# PalStudio server — one-line bootstrap installer (Windows PowerShell).
#
#   irm https://raw.githubusercontent.com/oMaN-Rod/palworld-save-pal/main/scripts/install-server.ps1 | iex
#
# The desktop-app installer served at https://palstudio.app/install lives in
# the repo root install.ps1; THIS script is the advanced, server-oriented
# channel.
#
# Installs the latest release's prebuilt server bundle (binary + desktop
# launcher + web UI + game data) under %LOCALAPPDATA%\PalStudio, adds the
# install dir to the user PATH, then asks how you want to run it:
#   - standalone: no service; run `palstudio` and pick desktop or webapp
#   - background service: a robust logon Scheduled Task (auto-restart,
#     single-instance) running the headless webapp server
#
# Environment overrides (set before invoking, e.g. `$env:VERSION='v1.4.2'`):
#   MODE        'standalone' | 'service' | 'ask' (default: ask when interactive)
#   VERSION     pin a release tag (default: latest)
#   REPO        GitHub owner/name (default oMaN-Rod/palworld-save-pal)
#   HOST/PORT   bind address for the service (default 127.0.0.1 / 5174)
#   LISTEN      network policy seed: localhost|lan|wan|tailscale (first boot only)
#   PIN         network policy seed: require this PIN from non-loopback peers
#   NO_SERVICE  '1' as an alias for MODE=standalone

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$Repo = if ($env:REPO) { $env:REPO } else { 'oMaN-Rod/palworld-save-pal' }
$Host_ = if ($env:HOST) { $env:HOST } else { '127.0.0.1' }
$Port = if ($env:PORT) { $env:PORT } else { '5174' }
$Mode = if ($env:MODE) { $env:MODE } elseif ($env:NO_SERVICE -eq '1') { 'standalone' } else { 'ask' }

function Write-Info($msg) { Write-Host "==> $msg" -ForegroundColor Cyan }
function Write-Warn($msg) { Write-Host "warning: $msg" -ForegroundColor Yellow }
function Die($msg) { Write-Host "error: $msg" -ForegroundColor Red; exit 1 }

# --------------------------------------------------------------- host info
$arch = switch ($env:PROCESSOR_ARCHITECTURE) {
    'AMD64' { 'x64' }
    'ARM64' { Die 'Windows ARM64 has no prebuilt bundle yet; build from source: https://github.com/oMaN-Rod/palworld-save-pal' }
    default { Die "unsupported architecture '$($env:PROCESSOR_ARCHITECTURE)'" }
}
$platform = "windows-$arch"

# ---------------------------------------------------------------- version
$Version = $env:VERSION
if (-not $Version) {
    Write-Info "looking up the latest release of $Repo"
    $latest = Invoke-RestMethod -Headers @{ 'User-Agent' = 'palstudio-installer' } `
        -Uri "https://api.github.com/repos/$Repo/releases/latest"
    if (-not $latest.tag_name) { Die 'could not resolve the latest release tag from GitHub' }
    $Version = $latest.tag_name
}
$Asset = "palstudio-$Version-server-$platform.tar.gz"
$ChecksumsAsset = "palstudio-$Version-server-checksums.txt"
Write-Info "installing PalStudio server $Version ($platform)"

# ---------------------------------------------------------------- locations
$Prefix = Join-Path $env:LOCALAPPDATA 'PalStudio'
$BinDir = Join-Path $Prefix 'bin'
New-Item -ItemType Directory -Force -Path $Prefix | Out-Null

# ---------------------------------------------------------------- download
$Base = "https://github.com/$Repo/releases/download/$Version"
$tmp = New-Item -ItemType Directory -Force -Path (Join-Path $env:TEMP "palstudio-install-$(Get-Random)")
$bundle = Join-Path $tmp $Asset
Write-Info "downloading $Base/$Asset"
try {
    Invoke-WebRequest -Uri "$Base/$Asset" -OutFile $bundle
} catch {
    Die "download failed — does $Version ship a $platform bundle? ($_)"
}

$checksums = Join-Path $tmp $ChecksumsAsset
try {
    Invoke-WebRequest -Uri "$Base/$ChecksumsAsset" -OutFile $checksums
    $expected = (Get-Content $checksums | Where-Object { $_ -match "\s$([regex]::Escape($Asset))$" } |
        Select-Object -First 1) -replace '\s.*$', ''
    if ($expected) {
        $actual = (Get-FileHash -Algorithm SHA256 $bundle).Hash.ToLower()
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
# Windows 10 1803+ ships bsdtar as tar.exe, which reads .tar.gz natively.
if (-not (Get-Command tar -ErrorAction SilentlyContinue)) {
    Die 'tar.exe not found (Windows 10 1803+ required)'
}
Write-Info "installing under $Prefix"
Stop-ScheduledTask -TaskName 'PalStudio' -ErrorAction SilentlyContinue | Out-Null
tar -xzf $bundle -C $tmp
$staged = Join-Path $tmp 'palstudio'
if (-not (Test-Path (Join-Path $staged 'bin'))) { Die "bundle layout error: no palstudio/bin inside $Asset" }
foreach ($dir in 'bin', 'ui', 'data') {
    $src = Join-Path $staged $dir
    if (-not (Test-Path $src)) { continue }
    $dst = Join-Path $Prefix $dir
    if (Test-Path $dst) { Remove-Item -Recurse -Force $dst }
    Move-Item $src $dst
}

# User PATH so `palstudio` works in new shells (the current shell keeps its
# inherited PATH; we launch via the full path below).
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if ($userPath -notlike "*$BinDir*") {
    [Environment]::SetEnvironmentVariable('Path', "$userPath;$BinDir", 'User')
    Write-Info "added $BinDir to the user PATH (new terminals only)"
}

$binary = Join-Path $BinDir 'palstudio.exe'
# The background service runs the headless webapp (`serve`); standalone users
# get the desktop/webapp picker on first `palstudio` launch.
$args_ = "serve --host $Host_ --port $Port --ui-dir `"$($Prefix)\ui`" --data-dir `"$($Prefix)\data`" --db `"$($Prefix)\ps-rs.db`""

# ---------------------------------------------------------------- mode
if ($Mode -eq 'ask') {
    Write-Host ''
    Write-Host 'How do you want to run PalStudio?'
    Write-Host '  1) standalone         - run palstudio when you need it (desktop app by default; webapp via \'palstudio webapp\')'
    Write-Host '  2) background service - always-on webapp server (Scheduled Task)'
    $answer = Read-Host 'Choice [1]'
    $Mode = if ($answer -eq '2' -or $answer -eq 's' -or $answer -eq 'service') { 'service' } else { 'standalone' }
}

# Network policy seeding (first boot only; the in-app Network page owns it
# afterwards).
$serviceEnv = @{}
if ($env:LISTEN) { $serviceEnv['PS_LISTEN'] = $env:LISTEN }
if ($env:PIN) { $serviceEnv['PS_PIN'] = $env:PIN }

# ---------------------------------------------------------------- service
if ($Mode -eq 'service') {
    Write-Info 'registering the PalStudio logon task'
    $action = New-ScheduledTaskAction -Execute $binary -Argument $args_ -WorkingDirectory $Prefix
    $trigger = New-ScheduledTaskTrigger -AtLogOn -User $env:USERNAME
    # Robustness: restart on failure, survive battery transitions, never
    # double-start, and no artificial execution time cap.
    $settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries `
        -RestartCount 3 -RestartInterval (New-TimeSpan -Minutes 1) `
        -MultipleInstances IgnoreNew `
        -ExecutionTimeLimit (New-TimeSpan -Days 3650) -StartWhenAvailable
    # -User without -Password registers an interactive-token task: no stored
    # credentials, runs only when that user is logged on.
    $principal = New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType Interactive -RunLevel Limited
    Register-ScheduledTask -TaskName 'PalStudio' -Action $action -Trigger $trigger `
        -Settings $settings -Principal $principal -Force | Out-Null
    Start-ScheduledTask -TaskName 'PalStudio'
    Write-Info 'Scheduled Task "PalStudio" started (auto-starts at logon, restarts on failure)'
} else {
    Write-Info 'standalone mode - no service installed'
    # A Start Menu entry pointing at the mode picker keeps it discoverable.
    $startMenu = Join-Path ([Environment]::GetFolderPath('Programs')) 'PalStudio'
    New-Item -ItemType Directory -Force -Path $startMenu | Out-Null
    $shortcutPath = Join-Path $startMenu 'PalStudio.lnk'
    $shell = New-Object -ComObject WScript.Shell
    $shortcut = $shell.CreateShortcut($shortcutPath)
    $shortcut.TargetPath = $binary
    $shortcut.WorkingDirectory = $Prefix
    $shortcut.Description = 'PalStudio (desktop/webapp picker)'
    $shortcut.Save()
    Write-Info "Start Menu shortcut created ($shortcutPath)"
}

if ($Mode -eq 'service') {
    # Started via the task above; wait for the listener.
} else {
    Write-Info 'tip: run PalStudio from the Start Menu, or `palstudio` in a new terminal'
}

Write-Info 'waiting for the server to accept connections'
$deadline = (Get-Date).AddSeconds(30)
$up = $false
while ((Get-Date) -lt $deadline) {
    try {
        $null = Invoke-WebRequest -Uri "http://127.0.0.1:$Port/" -UseBasicParsing -TimeoutSec 2
        $up = $true
        break
    } catch { Start-Sleep -Milliseconds 500 }
}
if (-not $up) { Write-Warn 'server did not respond within 30s — check the task/log and the port' }

Write-Host ''
if ($Mode -eq 'service') {
    Write-Host "  PalStudio $Version is running: http://127.0.0.1:$Port" -ForegroundColor Green
    Write-Host "  Installed under $Prefix."
    Write-Host '  To stop/start it: Stop-ScheduledTask PalStudio / Start-ScheduledTask PalStudio'
    Write-Host '  To remove autostart: Unregister-ScheduledTask PalStudio'
} else {
    Write-Host "  PalStudio $Version installed under $Prefix." -ForegroundColor Green
    Write-Host '  Start it from the Start Menu (PalStudio), or run: palstudio          (desktop/webapp picker)'
    Write-Host '                                                 palstudio webapp   (server + browser)'
    Write-Host '                                                 palstudio serve    (headless server)'
}
