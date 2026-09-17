# dev.ps1 — one-shot launcher / preflight for PalStudio.
# Windows entry point (the bash sibling is dev.sh for macOS/Linux).
#
# Does NOT auto-install anything (except the opt-in -InstallWasm): on a missing
# or wrong tool it prints the exact command to fix it and exits non-zero.
# Preflight does not verify the WebView2/MSVC build tools needed by
# -Desktop/-BuildDesktop. Defaults to -Webapp; run `.\dev.ps1 -Help` for the
# full flag list.
#
# Keep the UTF-8 BOM: without it 5.1 reads the file as ANSI and fails to parse.
# PowerShell execution policy: if blocked, use:
#   powershell -ExecutionPolicy Bypass -File .\dev.ps1 [args]
# Or: Set-ExecutionPolicy -Scope CurrentUser RemoteSigned

# param() MUST be the first executable statement in a .ps1. Everything else
# (the comment header above, then blank lines/comments) is allowed before it.
param(
    [switch]$Web, [switch]$Webapp, [switch]$Webhost, [switch]$Websuite,
    [switch]$Desktop, [switch]$Landing,
    [switch]$Docker, [switch]$Serve, [switch]$Signal,
    [switch]$BuildDesktop, [switch]$BuildAppImage, [switch]$BuildWeb, [switch]$Build, [switch]$Amity,
    [switch]$Check, [switch]$InstallWasm, [switch]$Json,
    [string]$GameDir, [string]$AmityWorkspace, [string]$Ue4ssZip,
    [switch]$SkipUe4ss, [switch]$RemoveWin64Ue4ss,
    [string]$HostAddr, [int]$VitePort, [int]$ServerPort,
    [int]$BrokerPort, [int]$WebPort, [switch]$LocalOnly,
    [switch]$NoServer, [switch]$SkipCheck, [switch]$NoInstall, [switch]$NoMux,
    [switch]$RebuildWasm, [string]$ForceCheckMode, [switch]$LiveTurn, [switch]$Help
)

if ($PSVersionTable.PSVersion.Major -lt 7) {
    $pwsh = Get-Command pwsh -ErrorAction SilentlyContinue
    if (-not $pwsh) {
        Write-Host "dev.ps1 needs PowerShell 7+. Install:  winget install Microsoft.PowerShell" -ForegroundColor Red
        exit 1
    }
    $forward = @(foreach ($kv in $PSBoundParameters.GetEnumerator()) {
        if ($kv.Value -is [switch]) { if ($kv.Value) { "-$($kv.Key)" } }
        else { "-$($kv.Key)"; "$($kv.Value)" }
    })
    & $pwsh.Source -NoProfile -ExecutionPolicy Bypass -File $PSCommandPath @forward
    exit $LASTEXITCODE
}

$ErrorActionPreference = "Stop"

# Every port/path below is load-bearing in the real config.
$RepoRoot       = Split-Path -Parent $MyInvocation.MyCommand.Path
$UiDir          = Join-Path $RepoRoot "ps-ui"
$PsDesktopDir  = Join-Path $RepoRoot "ps-desktop"
$PsWebDir      = Join-Path $RepoRoot "ps-web"
$BrokerDir      = Join-Path $RepoRoot "signal-broker"
$AmityDir       = Join-Path $RepoRoot "ps-amity"
$EnvFile        = Join-Path $UiDir ".env"
$NodeModules    = Join-Path $UiDir "node_modules"
$BrokerModules  = Join-Path $BrokerDir "node_modules"
$WasmOut        = Join-Path $UiDir "src/lib/wasm/ps"

$VitePortDefault   = 5173   # vite.config.ts server.port, strictPort:true
$ServerPortDefault = 5174   # ps-server default + Docker EXPOSE + WS_URL host
$BrokerPortDefault = 8787
$WebPortDefault    = 5175

$DesktopWsUrl = "127.0.0.1:$ServerPortDefault/ws"

$MuxSession = "ps"

$AmityWorkspaceDefault = Join-Path (Split-Path -Parent $RepoRoot) "amity-build"

$script:ChildJobs            = New-Object System.Collections.Generic.List[object]
$script:PreviousEnvExists    = $false
$script:PreviousEnvContent   = ""
$script:RestoreEnv           = $false

function Log-Info($m) { Write-Host "› $m" -ForegroundColor Cyan }
function Log-Ok($m)   { Write-Host "✓ $m" -ForegroundColor Green }
function Log-Warn($m) { Write-Host "⚠ $m" -ForegroundColor Yellow }
function Log-Fail($m) { Write-Host "✗ $m" -ForegroundColor Red }
function Die($m, [int]$code = 1) { Log-Fail $m; exit $code }
function Banner($t) {
    $line = "─" * 52
    Write-Host ""
    Write-Host $line -ForegroundColor DarkGray
    Write-Host "  $t" -ForegroundColor White
    Write-Host $line -ForegroundColor DarkGray
}

function Resolve-Tool($name) {
    $cmd = Get-Command $name -ErrorAction SilentlyContinue
    if ($cmd -and $cmd.Source) { return $cmd.Source }
    $ext = if ($IsWindows -or $env:OS -eq "Windows_NT") { ".exe" } else { "" }
    foreach ($d in @(
        (Join-Path $HOME ".cargo/bin"),
        (Join-Path $HOME ".local/bin"),
        (Join-Path $HOME ".bun/bin")
    )) {
        $candidate = Join-Path $d "$name$ext"
        if (Test-Path $candidate) { return $candidate }
    }
    return $null
}

function probe_version($name, [string[]]$argList) {
    $p = Resolve-Tool $name
    if (-not $p) { return "" }
    try {
        $out = & $p @argList 2>&1 | Select-Object -First 1
        return ($out -as [string]).Trim()
    } catch { return "" }
}

function Check-Bun() {
    if (-not (Resolve-Tool "bun")) {
        return @{ Name="bun"; Status="crit"; Detail="not found on PATH";
                  Hint="Install:  powershell -c `"irm https://bun.sh/install.ps1 | iex`"" }
    }
    $v = probe_version "bun" @("--version")
    return @{ Name="bun"; Status="ok"; Detail=$(if ($v) { $v } else { "present" }); Hint="" }
}

function Check-Node() {
    if (-not (Resolve-Tool "node")) {
        return @{ Name="Node.js"; Status="warn"; Detail="not found (optional; some npm scripts use it)"; Hint="" }
    }
    $v = probe_version "node" @("--version")
    return @{ Name="Node.js"; Status="ok"; Detail=$(if ($v) { $v } else { "present" }); Hint="" }
}

function Check-Git() {
    if (-not (Resolve-Tool "git")) {
        return @{ Name="git"; Status="warn"; Detail="not found (optional for dev)"; Hint="" }
    }
    $v = probe_version "git" @("--version")
    return @{ Name="git"; Status="ok"; Detail=$(if ($v) { $v } else { "present" }); Hint="" }
}

function Check-Cargo([bool]$strict) {
    $status = if ($strict) { "crit" } else { "warn" }
    if (-not (Resolve-Tool "cargo")) {
        return @{ Name="Rust (cargo)"; Status=$status; Detail="not found on PATH";
                  Hint="winget install Rustlang.Rustup   (or https://rustup.rs)" }
    }
    $v = probe_version "cargo" @("--version")
    return @{ Name="Rust (cargo)"; Status="ok"; Detail=$(if ($v) { $v } else { "present" }); Hint="" }
}

function Check-TauriCli([bool]$strict) {
    $status = if ($strict) { "crit" } else { "warn" }
    if (-not (Resolve-Tool "cargo")) {
        return @{ Name="Tauri CLI"; Status=$status; Detail="cargo missing";
                  Hint="winget install Rustlang.Rustup   (or https://rustup.rs)" }
    }
    $v = ""
    try {
        # cargo tauri writes the version line; capture it. Don't rely on
        # $LASTEXITCODE here — the `2>&1 | Select` pipeline can reset it.
        $v = (& cargo tauri --version 2>&1 | Select-Object -First 1) -as [string]
    } catch { }
    # A non-empty version string is the reliable signal the subcommand exists.
    if ($v -and $v.Trim()) {
        return @{ Name="Tauri CLI"; Status="ok"; Detail=$v.Trim(); Hint="" }
    }
    return @{ Name="Tauri CLI"; Status=$status; Detail="cargo tauri subcommand not available";
              Hint='cargo install tauri-cli --version "^2" --locked' }
}

function Check-WasmPack([bool]$strict) {
    $status = if ($strict) { "crit" } else { "warn" }
    if (-not (Resolve-Tool "wasm-pack")) {
        return @{ Name="wasm-pack"; Status=$status; Detail="not found on PATH";
                  Hint="cargo install wasm-pack && rustup target add wasm32-unknown-unknown" }
    }
    $v = probe_version "wasm-pack" @("--version")
    return @{ Name="wasm-pack"; Status="ok"; Detail=$(if ($v) { $v } else { "present" }); Hint="" }
}

function Check-WasmTarget([bool]$strict) {
    $status = if ($strict) { "crit" } else { "warn" }
    if (-not (Resolve-Tool "rustup")) {
        return @{ Name="wasm32-unknown-unknown"; Status=$status; Detail="rustup not found";
                  Hint="winget install Rustlang.Rustup   (or https://rustup.rs)" }
    }
    try {
        $installed = & rustup target list --installed 2>&1
        if ($installed -match "wasm32-unknown-unknown") {
            return @{ Name="wasm32-unknown-unknown"; Status="ok"; Detail="installed"; Hint="" }
        }
    } catch { }
    return @{ Name="wasm32-unknown-unknown"; Status=$status; Detail="target not installed";
              Hint="rustup target add wasm32-unknown-unknown" }
}

function Check-Docker([bool]$strict) {
    $status = if ($strict) { "crit" } else { "warn" }
    if (-not (Resolve-Tool "docker")) {
        return @{ Name="Docker"; Status=$status; Detail="not found on PATH";
                  Hint="Install Docker Desktop — https://docs.docker.com/get-docker/" }
    }
    try {
        & docker info *> $null
        if ($LASTEXITCODE -eq 0) {
            return @{ Name="Docker"; Status="ok"; Detail="daemon reachable"; Hint="" }
        }
    } catch { }
    return @{ Name="Docker"; Status=$status; Detail="CLI present but daemon not reachable";
              Hint="Start Docker Desktop." }
}

function Find-VsCMake() {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path $vswhere)) { return $null }
    $install = & $vswhere -latest -products * -property installationPath 2>$null | Select-Object -First 1
    if (-not $install) { return $null }
    $bundled = Join-Path $install "Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe"
    if (Test-Path $bundled) { return $bundled } else { return $null }
}

function Check-CMake() {
    $cmake = Resolve-Tool "cmake"
    if (-not $cmake) { $cmake = Find-VsCMake }
    if (-not $cmake) {
        return @{ Name="cmake"; Status="crit"; Detail="not found on PATH or in a Visual Studio install";
                  Hint="winget install Kitware.CMake, or add the 'C++ CMake tools for Windows' VS component" }
    }
    $v = try { ((& $cmake --version 2>&1 | Select-Object -First 1) -as [string]).Trim() } catch { "" }
    return @{ Name="cmake"; Status="ok"; Detail=$(if ($v) { $v } else { $cmake }); Hint="" }
}

function Find-PalworldDir() {
    $steam = try { (Get-ItemProperty "HKCU:\Software\Valve\Steam" -ErrorAction Stop).SteamPath } catch { $null }
    if (-not $steam) { return $null }
    $libraries = @($steam)
    $vdf = Join-Path $steam "steamapps\libraryfolders.vdf"
    if (Test-Path $vdf) {
        $libraries += @(Select-String -Path $vdf -Pattern '"path"\s+"([^"]+)"' |
            ForEach-Object { $_.Matches[0].Groups[1].Value -replace '\\\\', '\' })
    }
    foreach ($library in $libraries) {
        $candidate = Join-Path $library "steamapps\common\Palworld"
        if (Test-Path (Join-Path $candidate "Pal\Binaries\Win64\Palworld-Win64-Shipping.exe")) { return $candidate }
    }
    return $null
}

function Resolve-GameDir() { if ($GameDir) { $GameDir } else { Find-PalworldDir } }
function Resolve-AmityWorkspace() { if ($AmityWorkspace) { $AmityWorkspace } else { $AmityWorkspaceDefault } }

function Check-Palworld() {
    $dir = Resolve-GameDir
    if (-not $dir -or -not (Test-Path (Join-Path $dir "Pal\Binaries\Win64\Palworld-Win64-Shipping.exe"))) {
        return @{ Name="Palworld"; Status="crit";
                  Detail=$(if ($dir) { "no game executable under $dir" } else { "no Steam install found" });
                  Hint="Pass -GameDir <...\steamapps\common\Palworld>" }
    }
    if (Get-Process -Name "Palworld-Win64-Shipping" -ErrorAction SilentlyContinue) {
        return @{ Name="Palworld"; Status="crit"; Detail="running — the mod DLL is locked while the game is open";
                  Hint="Close Palworld, then re-run." }
    }
    $native = Test-Path (Join-Path $dir "Mods\NativeMods\UE4SS\UE4SS.dll")
    $win64 = Test-Path (Join-Path $dir "Pal\Binaries\Win64\ue4ss\UE4SS.dll")
    if ($native -and $win64) {
        return @{ Name="Palworld"; Status="crit"; Detail="$dir has BOTH the Workshop UE4SS and a Win64 UE4SS; running both crashes the game";
                  Hint="Add -RemoveWin64Ue4ss to drop the Win64 copy, or remove it yourself." }
    }
    $instance = if ($native) { "Workshop UE4SS" } elseif ($win64) { "Win64 UE4SS" } else { "no UE4SS" }
    if (-not $native -and -not $win64 -and -not $Ue4ssZip -and -not $SkipUe4ss) {
        return @{ Name="Palworld"; Status="crit"; Detail="$dir ($instance)";
                  Hint="Subscribe to the Steam Workshop UE4SS, or pass -Ue4ssZip <UE4SS release zip>." }
    }
    return @{ Name="Palworld"; Status="ok"; Detail="$dir ($instance)"; Hint="" }
}

function Check-AmityWorkspace() {
    $ws = Resolve-AmityWorkspace
    if (-not (Test-Path (Join-Path $ws "RE-UE4SS\CMakeLists.txt"))) {
        return @{ Name="UE4SS workspace"; Status="crit"; Detail="not set up at $ws";
                  Hint="One-time: .\ps-amity\scripts\setup-workspace.ps1 -Root `"$ws`"   (clones the pinned RE-UE4SS fork; needs git)" }
    }
    return @{ Name="UE4SS workspace"; Status="ok"; Detail=$ws; Hint="" }
}

function Check-Repo() {
    if ((Test-Path (Join-Path $RepoRoot "ps-server/Cargo.toml")) -and (Test-Path $UiDir)) {
        return $null
    }
    return @{ Name="PalStudio repo"; Status="crit";
              Detail="ps-server/Cargo.toml or ps-ui/ not found at $RepoRoot";
              Hint="Run dev.ps1 from the PalStudio repository root." }
}

function Check-DiskSpace($mode) {
    $min = switch ($mode) {
        "web"           { 800 }
        "webapp"        { 800 }
        "webhost"       { 800 }
        "desktop"       { 2500 }
        "build"         { 3500 }
        "websuite"      { 1500 }
        "landing"       { 300 }
        "docker"        { 2500 }
        "signal"        { 3000 }
        "build-desktop" { 3500 }
        "build-web"     { 3500 }
        "amity"         { 4000 }
        default         { 800 }
    }
    $free = 100000
    try {
        $drive = Get-PSDrive -Name $RepoRoot.Substring(0,1) -ErrorAction SilentlyContinue
        if ($drive) { $free = [math]::Floor($drive.Free / 1MB) }
    } catch { }
    if ($free -ge $min) {
        return @{ Name="Disk space"; Status="ok"; Detail="$free MB free (need $min MB)"; Hint="" }
    }
    return @{ Name="Disk space"; Status="crit";
              Detail="$free MB free — $mode mode needs ≥$min MB";
              Hint="Free space on $RepoRoot" }
}

function Get-PortOwner([int]$port) {
    try {
        return (Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction Stop |
            Select-Object -First 1).OwningProcess
    } catch { return $null }
}

function Check-Port($port) {
    $owner = Get-PortOwner $port
    if ($owner) {
        $name = try { (Get-Process -Id $owner -ErrorAction Stop).ProcessName } catch { "pid $owner" }
        return @{ Name="Port $port"; Status="warn"; Detail="in use by $name (pid $owner)";
                  Hint="Stop it, or pick another port. -Signal skips components whose port is taken." }
    }
    return @{ Name="Port $port"; Status="ok"; Detail="free"; Hint="" }
}

function Run-Preflight($mode) {
    $results = New-Object System.Collections.Generic.List[object]
    if ($mode -eq "amity") {
        $results.Add((Check-CMake)) | Out-Null
        $results.Add((Check-Git)) | Out-Null
        $repo = Check-Repo
        if ($repo) { $results.Add($repo) | Out-Null }
        $results.Add((Check-AmityWorkspace)) | Out-Null
        $results.Add((Check-Palworld)) | Out-Null
        $results.Add((Check-DiskSpace $mode)) | Out-Null
        return $results
    }
    $results.Add((Check-Bun)) | Out-Null

    $needsRust = $mode -in @("web","webapp","webhost","desktop","serve","build","build-desktop","build-web","docker","signal")
    $needsStrictRust = $mode -in @("desktop","serve","web","webapp","webhost","build","build-desktop","signal")
    if ($needsRust) { $results.Add((Check-Cargo $needsStrictRust)) | Out-Null }

    if ($mode -in @("desktop","build-desktop","signal")) {
        $results.Add((Check-TauriCli $true)) | Out-Null
    }
    if ($mode -in @("websuite","build-web","signal")) {
        $results.Add((Check-WasmPack $true)) | Out-Null
        $results.Add((Check-WasmTarget $true)) | Out-Null
    }
    if ($mode -eq "docker") { $results.Add((Check-Docker $true)) | Out-Null }

    $results.Add((Check-Node)) | Out-Null
    $results.Add((Check-Git)) | Out-Null
    $repo = Check-Repo
    if ($repo) { $results.Add($repo) | Out-Null }
    $results.Add((Check-DiskSpace $mode)) | Out-Null

    if ($mode -in @("web","webapp","webhost","desktop")) {
        $results.Add((Check-Port $VitePortDefault)) | Out-Null
        $results.Add((Check-Port $ServerPortDefault)) | Out-Null
    } elseif ($mode -in @("serve","docker")) {
        $results.Add((Check-Port $ServerPortDefault)) | Out-Null
    } elseif ($mode -in @("websuite","landing")) {
        $results.Add((Check-Port $VitePortDefault)) | Out-Null
    } elseif ($mode -eq "signal") {
        $results.Add((Check-Port $(if ($BrokerPort) { $BrokerPort } else { $BrokerPortDefault }))) | Out-Null
        $results.Add((Check-Port $VitePortDefault)) | Out-Null
        $results.Add((Check-Port $ServerPortDefault)) | Out-Null
        $results.Add((Check-Port $(if ($WebPort) { $WebPort } else { $WebPortDefault }))) | Out-Null
    }
    return $results
}

function Report-Preflight($mode, [bool]$asJson) {
    $results = Run-Preflight $mode
    if ($asJson) {
        # Build the JSON payload and write it directly to stdout (NOT via the
        # function return pipeline — PowerShell mixes output and return values).
        $payload = @($results | ForEach-Object {
            [ordered]@{ name=$_.Name; status=$_.Status; detail=$_.Detail; hint=$_.Hint }
        })
        # Serialize each element; wrap in [] for a proper array even with 1 elem.
        $json = $payload | ConvertTo-Json -Compress -Depth 5
        if ($payload.Count -eq 1) { $json = "[$json]" }
        [Console]::Out.WriteLine($json)
        $hasCrit = ($results | Where-Object { $_.Status -eq "crit" }).Count -gt 0
        if ($hasCrit) { return 1 } else { return 0 }
    }
    $nOk = 0; $nWarn = 0; $nCrit = 0
    Write-Host ""
    foreach ($r in $results) {
        $color = switch ($r.Status) { "ok" { "Green" } "warn" { "Yellow" } "crit" { "Red" } }
        $sym   = switch ($r.Status) { "ok" { "✓" } "warn" { "⚠" } "crit" { "✗" } }
        if ($r.Status -eq "ok")   { $nOk++ }
        if ($r.Status -eq "warn") { $nWarn++ }
        if ($r.Status -eq "crit") { $nCrit++ }
        Write-Host "  " -NoNewline
        Write-Host $sym -NoNewline -ForegroundColor $color
        Write-Host " " -NoNewline
        Write-Host $r.Name.PadRight(22) -NoNewline -ForegroundColor White
        Write-Host " " -NoNewline
        Write-Host $r.Detail
        if ($r.Hint) { Write-Host "      → $($r.Hint)" -ForegroundColor DarkGray }
    }
    Write-Host ""
    Write-Host "  $nOk ok  " -NoNewline -ForegroundColor White
    Write-Host "$nWarn warn  " -NoNewline -ForegroundColor Yellow
    Write-Host "$nCrit critical" -NoNewline -ForegroundColor Red
    Write-Host ""
    if ($nCrit -gt 0) {
        Write-Host "  Fix the $nCrit critical issue(s) above, then re-run dev.ps1." -ForegroundColor Red
        Write-Host "  (Tip: dev.ps1 -Check for a standalone report.)" -ForegroundColor DarkGray
    }
    if ($nCrit -gt 0) { return 1 } else { return 0 }
}

# Mirrors ps-ui/scripts/ensure-{desktop,web}-env.mjs — keep both in sync.
function Snapshot-Env() {
    if (Test-Path $EnvFile) {
        $script:PreviousEnvExists = $true
        $script:PreviousEnvContent = Get-Content -Raw $EnvFile
    } else {
        $script:PreviousEnvExists = $false
        $script:PreviousEnvContent = ""
    }
}

function Restore-EnvOnExit() {
    if (-not $script:RestoreEnv) { return }
    if ($script:PreviousEnvExists) {
        try { Set-Content -NoNewline -Path $EnvFile -Value $script:PreviousEnvContent } catch { }
    } else {
        try { Remove-Item -Force $EnvFile -ErrorAction SilentlyContinue } catch { }
    }
}

function Write-WebEnv([string]$wsUrl) {
    New-Item -ItemType Directory -Force -Path $UiDir | Out-Null
    $val = if ($wsUrl) { $wsUrl } else { "" }
    Set-Content -NoNewline -Path $EnvFile -Value "PUBLIC_WS_URL=$val`nPUBLIC_DESKTOP_MODE=false`n"
    Log-Info "Wrote ps-ui/.env (web mode, WS_URL=$(if ($wsUrl) { $wsUrl } else { '<empty>' }))"
}

function Write-DesktopEnv() {
    New-Item -ItemType Directory -Force -Path $UiDir | Out-Null
    Set-Content -NoNewline -Path $EnvFile -Value "PUBLIC_WS_URL=$DesktopWsUrl`nPUBLIC_DESKTOP_MODE=true`n"
    Log-Info "Wrote ps-ui/.env (desktop mode)"
}

function Ensure-BrokerInstall() {
    $bun = Resolve-Tool "bun"
    if (-not $bun) { Die "bun not found — run .\dev.ps1 -Check first." }
    if (Test-Path $BrokerModules) {
        Log-Info "signal-broker/node_modules present — skipping bun install."
        return
    }
    Log-Info "Running 'bun install' in signal-broker/…"
    Push-Location $BrokerDir
    try {
        & $bun install
        if ($LASTEXITCODE -ne 0) { Pop-Location; Die "bun install (signal-broker) failed." }
    } finally { Pop-Location }
    Log-Ok "signal-broker install complete."
}

# We let child processes inherit the console (no output redirection), so their
# output streams naturally alongside ours. Async event handlers fire on a thread
# with no PowerShell runspace and crash — avoiding redirection sidesteps that.
# Tree-kill on cleanup handles grandchildren (vite→esbuild, cargo→rustc).
function Spawn-FgTagged($tag, [string[]]$cmd, [string]$cwd, $envVars) {
    Log-Info "Starting $tag : $($cmd -join ' ')"
    $exe = $cmd[0]
    $argList = if ($cmd.Count -gt 1) { $cmd[1..($cmd.Count-1)] } else { @() }
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $exe
    foreach ($a in $argList) { [void]$psi.ArgumentList.Add($a) }
    if ($cwd) { $psi.WorkingDirectory = $cwd }
    $psi.UseShellExecute = $false
    # Do NOT redirect — child output goes straight to the inherited console.
    if ($envVars) {
        foreach ($k in $envVars.Keys) { $psi.EnvironmentVariables[$k] = $envVars[$k] }
    }
    $p = New-Object System.Diagnostics.Process
    $p.StartInfo = $psi
    $p.Start() | Out-Null
    $p.WaitForExit()
    return $p.ExitCode
}

function Spawn-BgTagged($tag, [string[]]$cmd, [string]$cwd, $envVars) {
    Log-Info "Starting $tag : $($cmd -join ' ')"
    $exe = $cmd[0]
    $argList = if ($cmd.Count -gt 1) { $cmd[1..($cmd.Count-1)] } else { @() }
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $exe
    foreach ($a in $argList) { [void]$psi.ArgumentList.Add($a) }
    if ($cwd) { $psi.WorkingDirectory = $cwd }
    $psi.UseShellExecute = $false
    if ($envVars) {
        foreach ($k in $envVars.Keys) { $psi.EnvironmentVariables[$k] = $envVars[$k] }
    }
    $p = New-Object System.Diagnostics.Process
    $p.StartInfo = $psi
    $p.Start() | Out-Null
    $script:ChildJobs.Add($p) | Out-Null
    return $p
}

function Get-Mux() {
    if ($NoMux) { return $null }
    $mux = Resolve-Tool "psmux"
    if (-not $mux) { return $null }
    try { if ([Console]::IsOutputRedirected) { return $null } } catch { }
    return $mux
}

function Quote-Ps([string]$s) { "'" + ($s -replace "'", "''") + "'" }

function Mux-PaneBody($tag, [string[]]$cmd, [string]$cwd, $envVars) {
    $parts = @()
    if ($cwd) { $parts += "Set-Location $(Quote-Ps $cwd)" }
    if ($envVars) {
        foreach ($k in $envVars.Keys) { $parts += "`$env:$k = $(Quote-Ps $envVars[$k])" }
    }
    $parts += "& " + (($cmd | ForEach-Object { Quote-Ps $_ }) -join " ")
    $parts += "Write-Host $(Quote-Ps "[dev] $tag exited") -ForegroundColor Yellow"
    return $parts -join "; "
}

function Mux-Launch($mux, $panes) {
    & $mux has-session -t $MuxSession *> $null
    if ($LASTEXITCODE -eq 0) {
        Die "psmux session '$MuxSession' already exists.
    Attach:  psmux attach -t $MuxSession
    Stop:    psmux kill-session -t $MuxSession
    Or re-run with -NoMux to run inline."
    }
    $pwsh = (Get-Process -Id $PID).Path
    $first = $true
    foreach ($pane in $panes) {
        Log-Info "Starting $($pane.Tag) (psmux pane): $($pane.Cmd -join ' ')"
        # psmux silently drops some -Command bodies (paths, env assignments)
        # while reporting success; base64 gives it nothing to parse.
        $body = Mux-PaneBody $pane.Tag $pane.Cmd $pane.Cwd $pane.Env
        $enc = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($body))
        if ($first) {
            & $mux new-session -d -s $MuxSession -n $mode -- $pwsh -NoProfile -NoExit -EncodedCommand $enc
            $first = $false
        } else {
            & $mux split-window -t $MuxSession -- $pwsh -NoProfile -NoExit -EncodedCommand $enc
        }
        if ($LASTEXITCODE -ne 0) { Die "psmux failed to start pane '$($pane.Tag)'." }
    }
    & $mux select-layout -t $MuxSession tiled *> $null
}

function Mux-Attach($mux) {
    Write-Host "  psmux session '$MuxSession': Ctrl-B d detaches (keeps running); Ctrl-B x kills a pane." -ForegroundColor DarkGray
    Write-Host "  Stop everything: psmux kill-session -t $MuxSession" -ForegroundColor DarkGray
    Write-Host ""
    if ($env:TMUX) { & $mux switch-client -t $MuxSession } else { & $mux attach -t $MuxSession }
    & $mux has-session -t $MuxSession *> $null
    if ($LASTEXITCODE -eq 0) {
        Log-Info "Session '$MuxSession' still running. Reattach: psmux attach -t $MuxSession"
        $script:RestoreEnv = $false
    } else {
        Log-Info "Session '$MuxSession' ended."
    }
}

function Cleanup-Children() {
    # Idempotent: safe to call from both Wait-OnProcs's finally and the
    # script-scope Invoke-WithCleanup finally. The list is cleared each call.
    foreach ($p in $script:ChildJobs) {
        if ($p -and -not $p.HasExited) {
            # Kill the whole process tree. On Windows use taskkill /T (reliable
            # tree-walk — reaches vite→esbuild, cargo→rustc). On .NET 5+
            # $p.Kill($true) (entireProcessTree) works cross-platform; fall back
            # to plain Kill() where unavailable.
            try {
                if ($IsWindows -or $env:OS -eq "Windows_NT") {
                    & taskkill /T /F /PID $p.Id 2>&1 | Out-Null
                } else {
                    try { $p.Kill($true) } catch { $p.Kill() }
                }
            } catch {
                try { $p.Kill() } catch { }
            }
        }
    }
    $script:ChildJobs.Clear()
}

function Wait-ForHttp($url, $label, [int]$timeout = 60) {
    Log-Info "Waiting for $label at $url …"
    $deadline = (Get-Date).AddSeconds($timeout)
    while ((Get-Date) -lt $deadline) {
        try {
            $resp = Invoke-WebRequest -Uri $url -UseBasicParsing -TimeoutSec 2
            if ($resp) { Log-Ok "$label is up: $url"; return $true }
        } catch { Start-Sleep -Seconds 1 }
    }
    Log-Warn "$label did not become reachable at $url within ${timeout}s"
    return $false
}

function Test-TcpPort([string]$targetHost, [int]$port, [int]$timeoutMs = 3000) {
    $client = New-Object System.Net.Sockets.TcpClient
    try {
        $iar = $client.BeginConnect($targetHost, $port, $null, $null)
        if ($iar.AsyncWaitHandle.WaitOne($timeoutMs) -and $client.Connected) {
            $client.EndConnect($iar); return $true
        }
        return $false
    } catch { return $false } finally { $client.Close() }
}

function Wait-TcpPort([string]$targetHost, [int]$port, [string]$label, [int]$timeout = 60) {
    Log-Info "Waiting for $label at ${targetHost}:$port …"
    $deadline = (Get-Date).AddSeconds($timeout)
    while ((Get-Date) -lt $deadline) {
        if (Test-TcpPort $targetHost $port 2000) { Log-Ok "$label is up: ${targetHost}:$port"; return $true }
        Start-Sleep -Seconds 1
    }
    Log-Warn "$label not reachable at ${targetHost}:$port within ${timeout}s"
    return $false
}

function Get-TurnMintStatus([int]$brokerPort) {
    $url = "http://127.0.0.1:$brokerPort/signal/turn"
    try {
        $resp = Invoke-WebRequest -Uri $url -UseBasicParsing -TimeoutSec 15 -SkipHttpErrorCheck
        $code = [int]$resp.StatusCode
        if ($code -eq 200 -and $resp.Content -match '"credential"') {
            return @{ Ok = $true; Definitive = $true; Detail = "minting (/signal/turn 200)" }
        }
        if ($code -eq 503 -and $resp.Content -match 'unconfigured') {
            return @{ Ok = $false; Definitive = $true; Detail = "503 unconfigured (no TURN_SECRET loaded)" }
        }
        return @{ Ok = $false; Definitive = $false; Detail = "status $code (broker still starting?)" }
    } catch {
        return @{ Ok = $false; Definitive = $false; Detail = "no response yet ($($_.Exception.Message))" }
    }
}

function Assert-LiveTurnSecret() {
    $devVars = Join-Path $RepoRoot ".dev.vars"
    if (-not (Test-Path $devVars)) {
        Die "-LiveTurn needs the real relay secret. Create $devVars with:
    TURN_SECRET=<value of /etc/turnserver.secret on the relay host>
  (it is gitignored)."
    }
    $line = Get-Content $devVars | Where-Object { $_ -match '^\s*TURN_SECRET\s*=' } | Select-Object -First 1
    if (-not $line) { Die "-LiveTurn: $devVars has no TURN_SECRET= line." }
    $val = ($line -replace '^\s*TURN_SECRET\s*=\s*', '').Trim()
    if (-not $val) { Die "-LiveTurn: TURN_SECRET in $devVars is empty." }
    if ($val -eq 'local-dev-secret') {
        Die "-LiveTurn: $devVars still holds the throwaway 'local-dev-secret'; the live relay rejects credentials minted with it. Use the real /etc/turnserver.secret value."
    }
    Log-Ok "-LiveTurn: real TURN_SECRET present in .dev.vars (value not shown)."
}

function Check-LiveRelayReachable() {
    $relayHost = "turn.palstudio.app"
    foreach ($p in @(443, 3478)) {
        if (Test-TcpPort $relayHost $p 3000) {
            Log-Ok "live relay reachable: ${relayHost}:$p/tcp"
        } else {
            Log-Warn "live relay NOT reachable: ${relayHost}:$p/tcp — a restrictive local network or the relay host's provider firewall may block it. turns:443 is the fallback; relay tests can still pass if 443 is open."
        }
    }
}

function Report-TurnReadiness([int]$brokerPort) {
    Wait-TcpPort "127.0.0.1" $brokerPort "broker" 60 | Out-Null
    Log-Info "Checking /signal/turn (wrangler compiles the worker on its first request; this can take a bit)…"
    $deadline = (Get-Date).AddSeconds(90)
    $st = Get-TurnMintStatus $brokerPort
    while (-not $st.Definitive -and (Get-Date) -lt $deadline) {
        Start-Sleep -Seconds 2
        $st = Get-TurnMintStatus $brokerPort
    }
    if ($st.Ok) {
        Log-Ok "TURN: broker is $($st.Detail)."
        return $true
    }
    if ($LiveTurn) {
        Log-Fail "TURN: broker $($st.Detail). -LiveTurn requires a minting broker."
        return $false
    }
    Log-Warn "TURN: broker $($st.Detail) — sessions will be STUN-only. Put a real TURN_SECRET in .dev.vars (repo root) and add -LiveTurn to test the relay."
    return $true
}

function Ensure-BunInstall([bool]$force) {
    $bun = Resolve-Tool "bun"
    if (-not $bun) { Die "bun not found — run .\dev.ps1 -Check first." }
    if ((Test-Path $NodeModules) -and -not $force) {
        Log-Info "ps-ui/node_modules present — skipping bun install."
        return
    }
    Log-Info "Running 'bun install' in ps-ui/ (first run can take a while)…"
    Push-Location $UiDir
    try {
        & $bun install
        if ($LASTEXITCODE -ne 0) { Pop-Location; Die "bun install failed." }
    } finally { Pop-Location }
    Log-Ok "bun install complete."
}

function Ensure-Wasm([bool]$rebuild) {
    # ps_bg.wasm existing is not a safe skip condition: ps.js is tracked and
    # ps_bg.wasm is gitignored, so a checkout/pull restores the throwing stub
    # over the real entry while the stale .wasm survives. Mirrors ensure_wasm
    # in dev.sh.
    $wasmFile = Join-Path $WasmOut "ps_bg.wasm"
    $entryJs  = Join-Path $WasmOut "ps.js"
    $pkgJson  = Join-Path $WasmOut "package.json"
    $stubMarker = "ps wasm not built" # text baked into the committed ps.js placeholder

    $reason = $null
    if ($rebuild) {
        $reason = "-RebuildWasm"
    } elseif (-not (Test-Path $wasmFile)) {
        $reason = "ps_bg.wasm missing"
    } elseif (-not (Test-Path $pkgJson) -or -not (Test-Path $entryJs)) {
        $reason = "incomplete wasm package (interrupted build?)"
    } elseif (Select-String -Path $entryJs -Pattern $stubMarker -Quiet) {
        $reason = "ps.js is the committed placeholder (git restored it over the build output)"
    } else {
        $wasmMtime = (Get-Item $wasmFile).LastWriteTime
        $crateDirs = @("ps-web", "ps-app", "ps-core", "ps-db") |
            ForEach-Object { Join-Path $RepoRoot $_ } |
            Where-Object { Test-Path $_ }
        $newer = Get-ChildItem -Path $crateDirs -Recurse -File -Include *.rs, Cargo.toml -ErrorAction SilentlyContinue |
            Where-Object { $_.LastWriteTime -gt $wasmMtime } |
            Select-Object -First 1
        $rootToml = Join-Path $RepoRoot "Cargo.toml"
        if (-not $newer -and (Test-Path $rootToml) -and ((Get-Item $rootToml).LastWriteTime -gt $wasmMtime)) {
            $newer = Get-Item $rootToml
        }
        if ($newer) { $reason = "newer Rust sources (e.g. $($newer.FullName))" }
    }

    if (-not $reason) {
        Log-Info "WASM up to date (ps-ui/src/lib/wasm/ps/ps_bg.wasm) (-RebuildWasm to redo)."
        return
    }

    $cargo = Resolve-Tool "cargo"
    $wasmPack = Resolve-Tool "wasm-pack"
    if (-not $cargo)    { Die "cargo not found — run .\dev.ps1 -Check first." }
    if (-not $wasmPack) { Die "wasm-pack not found — run .\dev.ps1 -InstallWasm first." }
    Log-Info "Building ps-web (wasm-pack): $reason"
    # Clear generated output only — an interrupted build must still leave a
    # resolvable $lib/wasm/ps behind, so the tracked placeholders have to
    # survive for wasm-pack to overwrite.
    $cleaned = $false
    if (Get-Command git -ErrorAction SilentlyContinue) {
        Push-Location $RepoRoot
        try {
            $rel = $WasmOut.Substring($RepoRoot.Length).TrimStart('\', '/')
            & git clean -fdx -- $rel *> $null
            $cleaned = ($LASTEXITCODE -eq 0)
        } catch { } finally { Pop-Location }
    }
    if (-not $cleaned) {
        if (Test-Path $WasmOut) { Remove-Item -Recurse -Force $WasmOut }
    }
    Push-Location $PsWebDir
    try {
        & $wasmPack build --target web --out-name ps --out-dir $WasmOut
        if ($LASTEXITCODE -ne 0) { Pop-Location; Die "wasm-pack build failed." }
    } finally { Pop-Location }
    if (-not (Test-Path $wasmFile)) { Die "wasm-pack reported success but ps_bg.wasm is missing." }
    Log-Ok "ps-web WASM built."
}

function Gen-JsonManifest() {
    $bun = Resolve-Tool "bun"
    if (-not $bun) { Die "bun not found." }
    $scriptPath = Join-Path $RepoRoot "scripts/gen-json-manifest.mjs"
    if (-not (Test-Path $scriptPath)) { Log-Warn "$scriptPath missing — skipping."; return }
    Log-Info "Generating JSON manifest (scripts/gen-json-manifest.mjs)…"
    Push-Location $RepoRoot
    try {
        & $bun $scriptPath
        if ($LASTEXITCODE -ne 0) { Pop-Location; Die "JSON manifest generation failed." }
    } finally { Pop-Location }
}

# The one exception to "never auto-install".
function Run-InstallWasm() {
    Banner "Install: WASM toolchain  (wasm32 target + wasm-pack)"
    $rustup = Resolve-Tool "rustup"
    if (-not $rustup) {
        Die "rustup is required to manage the wasm32 target, but it's not on PATH.
    Install it first:
      winget install Rustlang.Rustup   (or https://rustup.rs)
    Then open a NEW terminal and re-run:  .\dev.ps1 -InstallWasm"
    }
    $installed = & $rustup target list --installed 2>&1
    if ($installed -match "wasm32-unknown-unknown") {
        Log-Ok "wasm32-unknown-unknown target already installed."
    } else {
        Log-Info "Installing wasm32-unknown-unknown target (rustup target add)…"
        & $rustup target add wasm32-unknown-unknown
        if ($LASTEXITCODE -ne 0) { Die "rustup target add failed." }
        $installed = & $rustup target list --installed 2>&1
        if ($installed -notmatch "wasm32-unknown-unknown") {
            Die "rustup reported success but the target isn't listed."
        }
        Log-Ok "wasm32-unknown-unknown target installed."
    }

    $cargo = Resolve-Tool "cargo"
    if (-not $cargo) { Die "cargo not found — rustup toolchain incomplete." }
    if (Resolve-Tool "wasm-pack") {
        Log-Info "wasm-pack already present ($(probe_version 'wasm-pack' @('--version')))."
        Write-Host "  Reinstall/upgrade with: cargo install wasm-pack --force" -ForegroundColor DarkGray
    } else {
        Log-Info "Installing wasm-pack (cargo install — first run compiles, ~2-3 min)…"
        & $cargo install wasm-pack
        if ($LASTEXITCODE -ne 0) {
            Die "cargo install wasm-pack failed.
    Common cause: missing MSVC C/C++ build tools (Visual Studio Build Tools) or libssl dev headers."
        }
        if (-not (Resolve-Tool "wasm-pack")) {
            Write-Host ""
            Write-Host "wasm-pack installed but not on PATH." -ForegroundColor Yellow
            Write-Host "  It's at ~/.cargo/bin/wasm-pack." -ForegroundColor DarkGray
            Write-Host "  Open a NEW terminal (so PATH refreshes), then verify:" -ForegroundColor DarkGray
            Write-Host "    wasm-pack --version" -ForegroundColor DarkGray
            Write-Host "  Then re-run: .\dev.ps1 -Check -Websuite" -ForegroundColor White
            return
        }
        Log-Ok "wasm-pack installed ($(probe_version 'wasm-pack' @('--version')))."
    }

    Write-Host ""
    Banner "Verifying  (-Check -Websuite)"
    Report-Preflight "websuite" $false | Out-Null
}

function Run-Webapp {
    # The tool-only SPA against a hand-launched LOCAL webapp server: the
    # network policy is clamped to localhost (the Network page offers just
    # the port). Use -Webhost for the full hosted policy.
    $h = if ($HostAddr) { $HostAddr } else { "127.0.0.1" }
    $vitePort = if ($VitePort) { $VitePort } else { $VitePortDefault }
    $serverPort = if ($ServerPort) { $ServerPort } else { $ServerPortDefault }
    $wsUrl = "${h}:$serverPort/ws"
    $bun = Resolve-Tool "bun"
    $cargo = Resolve-Tool "cargo"
    if (-not $bun)   { Die "bun not found." }
    if (-not $cargo) { Die "cargo not found." }

    Ensure-BunInstall $false
    Write-WebEnv $wsUrl
    Banner "Dev: webapp  (${h}:$vitePort  +  ps-server :$serverPort, localhost-tier)"

    # PS_SERVER_PORT feeds the proxy targets in vite.config.ts, so /api and
    # /network-unlock follow -ServerPort instead of the hardcoded 5174.
    $components = @(@{ Tag="vite"; Cwd=$UiDir; Env=@{ "PS_SERVER_PORT" = "$serverPort" }
        Cmd=@($bun, "run", "dev:vite", "--", "--host", $h, "--port", "$vitePort") })
    if (-not $NoServer) {
        $components += @{ Tag="ps-server"; Cwd=$RepoRoot; Env=$null
            Cmd=@($cargo, "run", "-p", "ps-server", "--",
                "--host", $h, "--port", "$serverPort",
                "--ui-dir", $UiDir, "--data-dir", (Join-Path $RepoRoot "data"),
                "--db", (Join-Path $RepoRoot "ps-rs.db"), "--dev") }
    }

    $mux = Get-Mux
    if ($mux -and $components.Count -ge 2) {
        Mux-Launch $mux $components
        Write-Host ""
        Write-Host "  ▸ PalStudio webapp dev:  http://${h}:$vitePort" -ForegroundColor Cyan
        Mux-Attach $mux
        return
    }

    $vite = Spawn-BgTagged $components[0].Tag $components[0].Cmd $components[0].Cwd $null
    $server = $null
    if ($components.Count -gt 1) {
        $server = Spawn-BgTagged $components[1].Tag $components[1].Cmd $components[1].Cwd $null
    }
    Wait-ForHttp "http://${h}:$vitePort" "Vite" 60 | Out-Null
    Write-Host ""
    Write-Host "  ▸ PalStudio webapp dev running:  http://${h}:$vitePort" -ForegroundColor Cyan
    Write-Host "  Local tool tier — the Network page offers the port only." -ForegroundColor DarkGray
    Write-Host "  For full network settings: .\dev.ps1 -Webhost" -ForegroundColor DarkGray
    Write-Host "  Ctrl-C to stop. dev.ps1 restores ps-ui/.env on exit." -ForegroundColor DarkGray
    Write-Host ""
    if ($server) { Wait-OnProcs @($vite) @($server) } else { Wait-OnProcs @($vite) @() }
}

function Run-Webhost {
    # The SERVER edition from source: same children as -Webapp, but the
    # server runs hosted (--hosted) so the full network policy applies and
    # the Network page is unrestricted (listen modes, allowlists, PIN, ...).
    # The stored policy still defaults to localhost-only; open the Network
    # page to expose it to the LAN/tailnet. The server binds broadly
    # (0.0.0.0) so a listen-mode switch needs no rebind; -HostAddr pins it.
    $viteHost   = if ($HostAddr) { $HostAddr } else { "127.0.0.1" }
    $serverHost = if ($HostAddr) { $HostAddr } else { "0.0.0.0" }
    $vitePort = if ($VitePort) { $VitePort } else { $VitePortDefault }
    $serverPort = if ($ServerPort) { $ServerPort } else { $ServerPortDefault }
    $wsUrl = "$(if ($HostAddr) { $HostAddr } else { '127.0.0.1' }):$serverPort/ws"
    $bun = Resolve-Tool "bun"
    $cargo = Resolve-Tool "cargo"
    if (-not $bun)   { Die "bun not found." }
    if (-not $cargo) { Die "cargo not found." }

    Ensure-BunInstall $false
    Write-WebEnv $wsUrl
    Banner "Dev: webhost  (${viteHost}:$vitePort  +  ps-server :$serverPort hosted)"

    $components = @(@{ Tag="vite"; Cwd=$UiDir; Env=@{ "PS_SERVER_PORT" = "$serverPort" }
        Cmd=@($bun, "run", "dev:vite", "--", "--host", $viteHost, "--port", "$vitePort") })
    if (-not $NoServer) {
        $components += @{ Tag="ps-server"; Cwd=$RepoRoot; Env=$null
            Cmd=@($cargo, "run", "-p", "ps-server", "--",
                "--host", $serverHost, "--port", "$serverPort", "--hosted",
                "--ui-dir", $UiDir, "--data-dir", (Join-Path $RepoRoot "data"),
                "--db", (Join-Path $RepoRoot "ps-rs.db"), "--dev") }
    }

    $mux = Get-Mux
    if ($mux -and $components.Count -ge 2) {
        Mux-Launch $mux $components
        Write-Host ""
        Write-Host "  ▸ PalStudio webhost dev:  http://${viteHost}:$vitePort" -ForegroundColor Cyan
        Mux-Attach $mux
        return
    }

    $vite = Spawn-BgTagged $components[0].Tag $components[0].Cmd $components[0].Cwd $null
    $server = $null
    if ($components.Count -gt 1) {
        $server = Spawn-BgTagged $components[1].Tag $components[1].Cmd $components[1].Cwd $null
    }
    Wait-ForHttp "http://${viteHost}:$vitePort" "Vite" 60 | Out-Null
    Write-Host ""
    Write-Host "  ▸ PalStudio webhost dev running:  http://${viteHost}:$vitePort" -ForegroundColor Cyan
    Write-Host "  Hosted tier — full Network settings (listen modes, allowlists, PIN)." -ForegroundColor DarkGray
    Write-Host "  Default policy is still localhost-only; open the Network page to expose." -ForegroundColor DarkGray
    Write-Host "  Ctrl-C to stop. dev.ps1 restores ps-ui/.env on exit." -ForegroundColor DarkGray
    Write-Host ""
    if ($server) { Wait-OnProcs @($vite) @($server) } else { Wait-OnProcs @($vite) @() }
}

function Run-Desktop {
    $cargo = Resolve-Tool "cargo"
    if (-not $cargo) { Die "cargo not found." }
    try { & cargo tauri --version *> $null } catch { }
    if ($LASTEXITCODE -ne 0) {
        Die "Tauri CLI not available. Install it:`n    cargo install tauri-cli --version `"^2`" --locked"
    }
    Ensure-BunInstall $false
    Write-DesktopEnv
    if (-not (Test-Path (Join-Path $RepoRoot "ui_build"))) {
        New-Item -ItemType Directory -Force -Path (Join-Path $RepoRoot "ui_build") | Out-Null
        Log-Info "Created empty ui_build/ (Tauri dev resource check)."
    }
    Banner "Dev: desktop  (Tauri + embedded ps-server)"
    $tauri = Spawn-BgTagged "tauri" @($cargo, "tauri", "dev") $PsDesktopDir $null
    Write-Host "  Ctrl-C to stop. dev.ps1 restores ps-ui/.env on exit." -ForegroundColor DarkGray
    Write-Host ""
    Wait-OnProcs @($tauri) @()
}

function Run-Websuite {
    # The full public website from source: landing page + tool running
    # entirely in the browser (VITE_TRANSPORT=worker, wasm build).
    $bun = Resolve-Tool "bun"
    if (-not $bun) { Die "bun not found." }
    $h = if ($HostAddr) { $HostAddr } else { "127.0.0.1" }
    $port = if ($VitePort) { $VitePort } else { $VitePortDefault }
    Ensure-BunInstall $false
    Ensure-Wasm $RebuildWasm
    Gen-JsonManifest
    Write-WebEnv ""
    Banner "Dev: websuite  (landing page + tool, browser-only)"
    $vite = Spawn-BgTagged "vite" @($bun, "run", "dev:vite", "--", "--host", $h, "--port", "$port") $UiDir @{ "VITE_TRANSPORT" = "worker" }
    Wait-ForHttp "http://${h}:$port" "Vite (websuite)" 60 | Out-Null
    Write-Host ""
    Write-Host "  ▸ PalStudio websuite dev running:  http://${h}:$port" -ForegroundColor Cyan
    Write-Host "  Landing-page mode (VITE_TRANSPORT=worker). Ctrl-C to stop." -ForegroundColor DarkGray
    Write-Host ""
    Wait-OnProcs @($vite) @()
}

function Run-Landing {
    $bun = Resolve-Tool "bun"
    if (-not $bun) { Die "bun not found." }
    $h = if ($HostAddr) { $HostAddr } else { "127.0.0.1" }
    $port = if ($VitePort) { $VitePort } else { $VitePortDefault }
    Ensure-BunInstall $false
    Write-WebEnv ""
    Banner "Dev: landing-only  (${h}:$port, no wasm / no server)"
    $vite = Spawn-BgTagged "vite" @($bun, "run", "dev:vite", "--", "--host", $h, "--port", "$port") $UiDir @{ "VITE_TRANSPORT" = "worker"; "VITE_LANDING_ONLY" = "true" }
    Wait-ForHttp "http://${h}:$port" "Vite (landing)" 60 | Out-Null
    Write-Host ""
    Write-Host "  ▸ PalStudio landing preview:  http://${h}:$port" -ForegroundColor Cyan
    Write-Host "  Landing page only — WASM/server skipped (VITE_LANDING_ONLY)." -ForegroundColor DarkGray
    Write-Host "  Buttons that load a save won't work. Ctrl-C to stop." -ForegroundColor DarkGray
    Write-Host ""
    Wait-OnProcs @($vite) @()
}

function Run-Signal {
    $bun = Resolve-Tool "bun"
    $cargo = Resolve-Tool "cargo"
    if (-not $bun)   { Die "bun not found." }
    if (-not $cargo) { Die "cargo not found." }
    try { & cargo tauri --version *> $null } catch { }
    if ($LASTEXITCODE -ne 0) {
        Die "Tauri CLI not available. Install it:`n    cargo install tauri-cli --version `"^2`" --locked"
    }
    if (-not (Test-Path (Join-Path $RepoRoot "wrangler.jsonc"))) { Die "wrangler.jsonc not found at repo root." }

    $brokerPort = if ($BrokerPort) { $BrokerPort } else { $BrokerPortDefault }
    $webPort    = if ($WebPort)    { $WebPort }    else { $WebPortDefault }
    if ($webPort -in @($VitePortDefault, $ServerPortDefault, $brokerPort)) {
        Die "-WebPort $webPort collides with the desktop vite ($VitePortDefault), ps-server ($ServerPortDefault) or broker ($brokerPort)."
    }

    $lanIp = $null
    if (-not $LocalOnly) {
        $lanIp = if ($HostAddr) { $HostAddr } else { Detect-LanIp }
        if (-not $lanIp) { Log-Warn "No LAN IP detected; binding to localhost only (phones won't reach it)." }
    }
    $advertiseHost = if ($lanIp) { $lanIp } else { "localhost" }
    $bind          = if ($lanIp) { "0.0.0.0" } else { "127.0.0.1" }

    if ($LiveTurn) {
        Assert-LiveTurnSecret
        Check-LiveRelayReachable
    }

    Ensure-BunInstall $false
    Ensure-BrokerInstall
    Ensure-Wasm $RebuildWasm
    Gen-JsonManifest
    Write-DesktopEnv
    if (-not (Test-Path (Join-Path $RepoRoot "ui_build"))) {
        New-Item -ItemType Directory -Force -Path (Join-Path $RepoRoot "ui_build") | Out-Null
        Log-Info "Created empty ui_build/ (Tauri dev resource check)."
    }

    $brokerUrl  = "ws://${advertiseHost}:$brokerPort"
    $pairingUrl = "http://${advertiseHost}:$webPort"
    Banner "Dev: signal  (broker :$brokerPort  +  desktop  +  web $pairingUrl)"

    $components = @()
    $owner = Get-PortOwner $brokerPort
    if ($owner) {
        Log-Warn "broker: port $brokerPort already in use (pid $owner) — skipped."
    } else {
        $components += @{ Tag="broker"; Cwd=$BrokerDir; Env=$null
            Cmd=@($bun, "run", "dev", "--", "--ip", $bind, "--port", "$brokerPort") }
    }

    $owner = Get-PortOwner $VitePortDefault
    if ($owner) {
        Log-Warn "desktop: port $VitePortDefault already in use (pid $owner) — skipped."
    } else {
        $components += @{ Tag="tauri"; Cwd=$PsDesktopDir; Cmd=@($cargo, "tauri", "dev")
            Env=@{
                "PS_SIGNAL_BROKER_URL"       = "ws://localhost:$brokerPort"
                "PS_SIGNAL_PAIRING_URL_BASE" = $pairingUrl
                "PUBLIC_DESKTOP_MODE"         = "true"
                "PUBLIC_WS_URL"               = $DesktopWsUrl
            } }
    }

    $webStarted = $false
    $owner = Get-PortOwner $webPort
    if ($owner) {
        Log-Warn "web: port $webPort already in use (pid $owner) — skipped."
    } else {
        $webStarted = $true
        $components += @{ Tag="web"; Cwd=$UiDir
            Cmd=@($bun, "run", "dev:vite", "--", "--host", $bind, "--port", "$webPort")
            Env=@{
                "VITE_TRANSPORT"         = "worker"
                "VITE_SIGNAL_BROKER_URL" = $brokerUrl
                "PUBLIC_DESKTOP_MODE"    = "false"
            } }
    }

    if ($components.Count -eq 0) { Die "Every component's port is already in use — nothing to start." }

    $summary = {
        Write-Host ""
        Write-Host "  ▸ broker:   $brokerUrl" -ForegroundColor Cyan
        Write-Host "  ▸ desktop:  cargo tauri dev (pairing links point at $pairingUrl)" -ForegroundColor Cyan
        Write-Host "  ▸ web:      $pairingUrl" -ForegroundColor Cyan
        if ($LiveTurn) {
            Write-Host "  ▸ relay:    turn.palstudio.app (live) — force it with $pairingUrl/signal?relay=1" -ForegroundColor Magenta
            Write-Host "              a direct LAN connection never touches TURN; ?relay=1 makes it, then confirm 'Connected via relay' on the card." -ForegroundColor DarkGray
        }
    }

    $mux = Get-Mux
    if ($mux -and $components.Count -ge 2) {
        Mux-Launch $mux $components
        if (-not (Report-TurnReadiness $brokerPort)) {
            & $mux kill-session -t $MuxSession *> $null
            Die "-LiveTurn: broker is not minting TURN credentials (see above)."
        }
        & $summary
        Mux-Attach $mux
        return
    }

    $started = @()
    foreach ($c in $components) { $started += Spawn-BgTagged $c.Tag $c.Cmd $c.Cwd $c.Env }
    if (-not (Report-TurnReadiness $brokerPort)) { Die "-LiveTurn: broker is not minting TURN credentials (see above)." }
    if ($webStarted) { Wait-ForHttp "http://127.0.0.1:$webPort" "Vite (web)" 60 | Out-Null }
    & $summary
    Write-Host "  Ctrl-C stops everything. dev.ps1 restores ps-ui/.env on exit." -ForegroundColor DarkGray
    Write-Host ""
    Wait-OnProcs $started @()
}

function Run-Serve {
    # Hosted like the real server edition (Docker CMD, `palstudio serve`,
    # background services): full network policy, Network page unrestricted.
    $cargo = Resolve-Tool "cargo"
    if (-not $cargo) { Die "cargo not found." }
    $h = if ($HostAddr) { $HostAddr } else { "0.0.0.0" }
    $port = if ($ServerPort) { $ServerPort } else { $ServerPortDefault }
    Banner "Serve: ps-server hosted  (${h}:$port)"
    $server = Spawn-BgTagged "ps-server" @($cargo, "run", "-p", "ps-server", "--",
        "--host", $h, "--port", "$port", "--hosted",
        "--ui-dir", $UiDir, "--data-dir", (Join-Path $RepoRoot "data"),
        "--db", (Join-Path $RepoRoot "ps-rs.db"), "--dev") $RepoRoot $null
    Wait-OnProcs @($server) @()
}

function Run-Docker {
    $docker = Resolve-Tool "docker"
    if (-not $docker) { Die "docker not found." }
    if (-not (Test-Path (Join-Path $RepoRoot "docker-compose.yml"))) {
        Die "docker-compose.yml not found at repo root."
    }
    if (-not (Test-Path (Join-Path $RepoRoot "docker-compose.build.yml"))) {
        Die "docker-compose.build.yml not found at repo root."
    }
    $h = if ($HostAddr) { $HostAddr } else { (Detect-LanIp) }
    if (-not $h) { $h = "127.0.0.1" }
    $wsUrl = "${h}:$ServerPortDefault/ws"
    Banner "Docker: build + up  (PUBLIC_WS_URL=$wsUrl, port $ServerPortDefault)"
    Log-Info "Building image (first build is slow; bakes WS_URL into the SPA)…"
    # The base compose file pulls the prebuilt GHCR image; the build override
    # rebuilds it locally with this machine's WS_URL baked in — the same
    # command scripts/build-docker.ps1 runs.
    $env:PUBLIC_WS_URL = $wsUrl
    $rc = Spawn-FgTagged "docker-build" @($docker, "compose",
        "-f", "docker-compose.yml", "-f", "docker-compose.build.yml", "build") $RepoRoot $null
    if ($rc -ne 0) { Die "docker compose build failed." }
    $rc = Spawn-FgTagged "docker-up" @($docker, "compose",
        "-f", "docker-compose.yml", "-f", "docker-compose.build.yml", "up", "-d") $RepoRoot $null
    if ($rc -ne 0) { Die "docker compose up failed." }
    Log-Ok "Docker backend up — connect at http://${h}:$ServerPortDefault"
    Write-Host "  Logs: docker compose logs -f   ·   Stop: docker compose down" -ForegroundColor DarkGray
}

function Run-BuildDesktop {
    $cargo = Resolve-Tool "cargo"
    if (-not $cargo) { Die "cargo not found." }
    Ensure-BunInstall $true
    Write-DesktopEnv
    Banner "Build: desktop (cargo tauri build)"
    $scriptPath = Join-Path $RepoRoot "scripts/build-desktop.ps1"
    if (Test-Path $scriptPath) {
        Log-Info "Using platform script scripts/build-desktop.ps1"
        $rc = Spawn-FgTagged "build-desktop" @("powershell", "-ExecutionPolicy", "Bypass", "-File", $scriptPath) $RepoRoot $null
    } else {
        $rc = Spawn-FgTagged "build-desktop" @($cargo, "tauri", "build") $RepoRoot $null
    }
    if ($rc -ne 0) { Die "desktop build failed." }
    Log-Ok "Desktop build complete."
}

function Run-BuildWeb {
    $bun = Resolve-Tool "bun"
    if (-not $bun) { Die "bun not found." }
    Ensure-BunInstall $true
    Ensure-Wasm $RebuildWasm
    Gen-JsonManifest
    Write-WebEnv ""
    Banner "Build: web (landing-page bundle → ui_build/)"
    $rc = Spawn-FgTagged "build" @($bun, "run", "build") $UiDir @{ "VITE_TRANSPORT" = "worker" }
    if ($rc -ne 0) { Die "web build failed." }
    Log-Ok "Web build complete → ui_build/"
}

function Run-BuildPlain {
    $bun = Resolve-Tool "bun"
    if (-not $bun) { Die "bun not found." }
    Ensure-BunInstall $true
    Write-WebEnv "127.0.0.1:$ServerPortDefault/ws"
    Banner "Build: plain SPA (server-served → ui_build/)"
    $rc = Spawn-FgTagged "build" @($bun, "run", "build") $UiDir $null
    if ($rc -ne 0) { Die "build failed." }
    Log-Ok "Plain SPA build complete → ui_build/"
}

function Run-Amity {
    $dir = Resolve-GameDir
    if (-not $dir) { Die "Palworld install not found. Pass -GameDir <...\steamapps\common\Palworld>." }
    $ws = Resolve-AmityWorkspace
    Banner "Amity: build the UE4SS mod and install it into $dir"
    $script = Join-Path $AmityDir "scripts/install-local.ps1"
    $argList = @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $script, "-GameDir", $dir, "-Root", $ws)
    if ($Ue4ssZip)         { $argList += @("-Ue4ssZip", $Ue4ssZip) }
    if ($SkipUe4ss)        { $argList += "-SkipUe4ss" }
    if ($RemoveWin64Ue4ss) { $argList += "-RemoveWin64Ue4ss" }
    $pwsh = (Get-Command pwsh).Source
    $rc = Spawn-FgTagged "amity" (@($pwsh) + $argList) $AmityDir $null
    if ($rc -ne 0) { Die "Amity mod build/install failed." }
    Log-Ok "PSAmity installed. Launch Palworld, load a world, and look for '[PSAmity] bridge listening' in UE4SS.log."
    Write-Host "  Then: -Desktop and open the Game (live mod) source, or bun ps-amity/tools/probe.ts caps" -ForegroundColor DarkGray
}

function Detect-LanIp() {
    try {
        $defaultRoute = Get-NetRoute -DestinationPrefix '0.0.0.0/0' -ErrorAction Stop |
            Sort-Object RouteMetric | Select-Object -First 1
        if ($defaultRoute) {
            $ip = (Get-NetIPAddress -InterfaceIndex $defaultRoute.ifIndex -AddressFamily IPv4 -ErrorAction Stop |
                Select-Object -First 1).IPAddress
            if ($ip) { return $ip }
        }
    } catch { }
    try {
        $ips = Get-NetIPAddress -AddressFamily IPv4 |
            Where-Object { $_.IPAddress -ne '127.0.0.1' -and $_.IPAddress -notlike '169.254.*' -and $_.IPAddress -notlike '172.*' } |
            Sort-Object -Property { $_.PrefixOrigin -ne 'Manual' }, PrefixLength
        if ($ips) { return $ips[0].IPAddress }
    } catch { }
    return $null
}

# Wait-OnProcs <primary[]> <secondary[]> — block until a primary exits or Ctrl-C.
function Wait-OnProcs($primary, $secondary) {
    try {
        while ($true) {
            foreach ($p in $primary) {
                if ($p.HasExited) {
                    Log-Warn "process exited (code $($p.ExitCode))."
                    return
                }
            }
            Start-Sleep -Seconds 1
        }
    } finally {
        Cleanup-Children
        Restore-EnvOnExit
    }
}

function Show-Usage() {
    @'
dev.ps1 — PalStudio dev/launch/build helper (Windows).
Runs from source; does NOT auto-install tools (run -Check for a report card).

run — launch from source (pick one; defaults to -Webapp):
  -Desktop          Dev: Tauri native window + embedded server.
  -Webapp           Dev: Vite + ps-server — the tool-only SPA against a local
                    server (localhost-clamped; Network page = port only).
                    Alias: -Web (the old name).
  -Webhost          Dev: Vite + ps-server --hosted — the server edition:
                    full Network page (listen modes, allowlists, PIN).
  -Websuite         Dev: landing page + tool (VITE_TRANSPORT=worker).
  -Landing          Dev: landing page ONLY — no WASM, no server (VITE_LANDING_ONLY).
  -Docker           Build & run the self-build Docker image (compose).
  -Serve            Run only the Rust ps-server, hosted like the real server
                    edition (Docker CMD / `palstudio serve`).
  -Signal           Dev: PalStudio Signal loop — broker (wrangler) + desktop (Tauri)
                    + web site on :5175, advertised on the LAN IP so phones
                    can pair. Components whose port is taken are skipped.

build — production artifacts:
  -BuildDesktop     Production desktop build → dist/.
  -BuildAppImage    Linux only; from Windows run ./dev.sh --build-appimage in WSL.
  -BuildWeb         Production web build (landing page) → ui_build/.
  -Build            Plain SPA build (server-served) → ui_build/.
  -Amity            Build the PalStudio Amity UE4SS mod and install it into the local
                    Palworld install (auto-detected from Steam). The game must
                    be closed. One-time setup first:
                    .\ps-amity\scripts\setup-workspace.ps1

options:
  -Check            Run only the preflight for the selected mode, then exit.
                    Combine with a mode flag (e.g. -Check -Desktop).
  -InstallWasm      Install the WASM toolchain (wasm32 target + wasm-pack).
  -HostAddr <ip>    Host/IP bind or WS_URL host (-Webapp/-Webhost/-Serve/-Docker);
                    LAN IP to advertise (-Signal, auto-detected by default).
  -VitePort <p>     Vite port (default 5173).
  -ServerPort <p>   ps-server port (default 5174).
  -BrokerPort <p>   (-Signal) wrangler dev port (default 8787).
  -WebPort <p>      (-Signal) web site port (default 5175).
  -LocalOnly        (-Signal) bind everything to localhost; no LAN advertising.
  -LiveTurn         (-Signal) test against the LIVE coturn relay: require a real
                    TURN_SECRET in repo-root .dev.vars, probe the relay's ports,
                    fail if the broker isn't minting, and print the
                    /signal?relay=1 forced-relay test URL. Plain -Signal already
                    reports TURN status but never fails on it.
  -NoServer         (-Webapp/-Webhost) skip ps-server (Vite only).
  -SkipCheck        Skip the preflight (advanced).
  -NoInstall        Skip bun install if node_modules exists.
  -NoMux            Run components inline instead of in psmux panes. Modes
                    that start several components (-Webapp, -Webhost, -Signal)
                    use psmux when it is installed: one pane each, session "ps".
  -RebuildWasm      (-Websuite/-BuildWeb) force wasm-pack rebuild.
  -GameDir <path>   (-Amity) Palworld install dir (…\steamapps\common\Palworld)
                    when Steam auto-detection does not find it.
  -AmityWorkspace <path>  (-Amity) the UE4SS CMake workspace (default: an
                    amity-build folder beside this repo).
  -Ue4ssZip <path>  (-Amity) UE4SS release zip to install under Win64 when the
                    game has no UE4SS instance and the Workshop one is absent.
  -SkipUe4ss        (-Amity) install only the mod, never UE4SS itself.
  -RemoveWin64Ue4ss (-Amity) drop a Win64 UE4SS that would run beside the
                    Workshop one (running both crashes the game).
  -Json             Machine-readable preflight JSON (implies -Check).
  -ForceCheckMode <m>   Override the preflight mode (advanced).
  -Help             Show this help.

macOS/Linux users: run dev.sh instead.

NOTE: -HostAddr (not -Host) is used because -Host is a reserved PowerShell
common parameter name.
'@ | Write-Host
}

if ($Help) { Show-Usage; exit 0 }

if ($BuildAppImage) {
    Die "AppImages can only be built on Linux. From WSL or a Linux machine:  ./dev.sh --build-appimage"
}

$mode = if ($ForceCheckMode) { $ForceCheckMode }
        elseif ($Webhost)     { "webhost" }
        elseif ($Websuite)    { "websuite" }
        elseif ($Webapp -or $Web) { "webapp" }
        elseif ($Desktop)     { "desktop" }
        elseif ($Landing)     { "landing" }
        elseif ($Docker)      { "docker" }
        elseif ($Serve)       { "serve" }
        elseif ($Signal)      { "signal" }
        elseif ($BuildDesktop){ "build-desktop" }
        elseif ($BuildWeb)    { "build-web" }
        elseif ($Build)       { "build" }
        elseif ($Amity)       { "amity" }
        else                  { "webapp" }

if ($InstallWasm) { Run-InstallWasm; return }

Snapshot-Env
$script:RestoreEnv = $true

if ($Check -or $Json) {
    if (-not $Json) { Banner "Environment check  (mode: $mode)" }
    $rc = Report-Preflight $mode $Json
    exit $rc
}

if (-not $SkipCheck) {
    Banner "Preflight  (mode: $mode)"
    $rc = Report-Preflight $mode $false
    if ($rc -ne 0) {
        Write-Host ""
        Write-Host "Preflight reported critical issues — aborting." -ForegroundColor Red
        Write-Host "Re-run with -SkipCheck to bypass (not recommended)." -ForegroundColor DarkGray
        Restore-EnvOnExit
        exit $rc
    }
    Write-Host ""
}

if ($NoInstall) {
    function Ensure-BunInstall([bool]$force) { Log-Info "-NoInstall: skipping bun install." }
}

# Wrap the entire dispatch in try/finally so spawned children are ALWAYS torn
# down on exit — including Ctrl-C, mid-build failures, or exits during
# Wait-ForHttp (before Wait-OnProcs's own finally runs). This mirrors the bash
# script's global EXIT trap. Wait-OnProcs still has its own finally for the
# normal child-exit path; Cleanup-Children is idempotent (clears the list).
$script:CleanupDone = $false
function Invoke-WithCleanup([scriptblock]$body) {
    try {
        & $body
    } catch {
        Write-Host "Error: $_" -ForegroundColor Red
        throw
    } finally {
        if (-not $script:CleanupDone) {
            $script:CleanupDone = $true
            if ($script:ChildJobs.Count -gt 0) {
                Write-Host "Cleaning up spawned processes…" -ForegroundColor Yellow
            }
            Cleanup-Children
            Restore-EnvOnExit
        }
    }
}

Invoke-WithCleanup {
    switch ($mode) {
        "webapp"        { Run-Webapp }
        "webhost"       { Run-Webhost }
        "websuite"      { Run-Websuite }
        "desktop"       { Run-Desktop }
        "landing"       { Run-Landing }
        "docker"        { Run-Docker }
        "serve"         { Run-Serve }
        "signal"        { Run-Signal }
        "build-desktop" { Run-BuildDesktop }
        "build-web"     { Run-BuildWeb }
        "build"         { Run-BuildPlain }
        "amity"         { Run-Amity }
    }
}
