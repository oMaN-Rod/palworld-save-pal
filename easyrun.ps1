# easyrun.ps1 — one-shot launcher / preflight for Palworld Save Pal (PSP).
# Windows entry point (the bash sibling is easyrun.sh for macOS/Linux).
#
# Does NOT auto-install anything (except the opt-in -InstallWasm): on a missing
# or wrong tool it prints the exact command to fix it and exits non-zero.
# Preflight does not verify the WebView2/MSVC build tools needed by
# -Desktop/-BuildDesktop. Defaults to -Web; run `.\easyrun.ps1 -Help` for the
# full flag list.
#
# PowerShell execution policy: if blocked, use:
#   powershell -ExecutionPolicy Bypass -File .\easyrun.ps1 [args]
# Or: Set-ExecutionPolicy -Scope CurrentUser RemoteOnce

# param() MUST be the first executable statement in a .ps1. Everything else
# (the comment header above, then blank lines/comments) is allowed before it.
param(
    [switch]$Web, [switch]$Desktop, [switch]$Webapp, [switch]$Landing,
    [switch]$Docker, [switch]$Serve,
    [switch]$Browser, [switch]$BuildBrowser,
    [switch]$BuildDesktop, [switch]$BuildWeb, [switch]$Build,
    [switch]$Check, [switch]$InstallWasm, [switch]$Json,
    [string]$HostAddr, [int]$VitePort, [int]$ServerPort,
    [switch]$NoServer, [switch]$SkipCheck, [switch]$NoInstall,
    [switch]$RebuildWasm, [string]$ForceCheckMode, [switch]$Help
)

$ErrorActionPreference = "Stop"

# Every port/path below is load-bearing in the real config.
$RepoRoot       = Split-Path -Parent $MyInvocation.MyCommand.Path
$UiDir          = Join-Path $RepoRoot "psp-ui"
$PspDesktopDir  = Join-Path $RepoRoot "psp-desktop"
$PspWebDir      = Join-Path $RepoRoot "psp-web"
$EnvFile        = Join-Path $UiDir ".env"
$NodeModules    = Join-Path $UiDir "node_modules"
$WasmOut        = Join-Path $UiDir "src/lib/wasm/psp"

$VitePortDefault   = 5173   # vite.config.ts server.port, strictPort:true
$ServerPortDefault = 5174   # psp-server default + Docker EXPOSE + WS_URL host

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

function Check-Repo() {
    if ((Test-Path (Join-Path $RepoRoot "psp-server/Cargo.toml")) -and (Test-Path $UiDir)) {
        return $null
    }
    return @{ Name="PSP repo"; Status="crit";
              Detail="psp-server/Cargo.toml or psp-ui/ not found at $RepoRoot";
              Hint="Run easyrun.ps1 from the Palworld Save Pal repository root." }
}

function Check-DiskSpace($mode) {
    $min = switch ($mode) {
        "web"           { 800 }
        "desktop"       { 2500 }
        "build"         { 3500 }
        "webapp"        { 1500 }
        "landing"       { 300 }
        "docker"        { 2500 }
        "build-desktop" { 3500 }
        "build-web"     { 3500 }
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

function Check-Port($port) {
    # On Windows we skip the bind probe (unreliable); the dev server reports
    # clearly if it can't bind.
    return @{ Name="Port $port"; Status="ok"; Detail="checked at launch (Windows)"; Hint="" }
}

function Run-Preflight($mode) {
    $results = New-Object System.Collections.Generic.List[object]
    $results.Add((Check-Bun)) | Out-Null

    $needsRust = $mode -in @("web","desktop","serve","webapp","build","build-desktop","build-web","docker")
    $needsStrictRust = $mode -in @("desktop","serve","web","build","build-desktop")
    if ($needsRust) { $results.Add((Check-Cargo $needsStrictRust)) | Out-Null }

    if ($mode -in @("desktop","build-desktop")) {
        $results.Add((Check-TauriCli $true)) | Out-Null
    }
    if ($mode -in @("webapp","build-web")) {
        $results.Add((Check-WasmPack $true)) | Out-Null
        $results.Add((Check-WasmTarget $true)) | Out-Null
    }
    if ($mode -eq "docker") { $results.Add((Check-Docker $true)) | Out-Null }

    $results.Add((Check-Node)) | Out-Null
    $results.Add((Check-Git)) | Out-Null
    $repo = Check-Repo
    if ($repo) { $results.Add($repo) | Out-Null }
    $results.Add((Check-DiskSpace $mode)) | Out-Null

    if ($mode -eq "web") {
        $results.Add((Check-Port $VitePortDefault)) | Out-Null
        $results.Add((Check-Port $ServerPortDefault)) | Out-Null
    } elseif ($mode -in @("serve","docker")) {
        $results.Add((Check-Port $ServerPortDefault)) | Out-Null
    } elseif ($mode -in @("webapp","landing")) {
        $results.Add((Check-Port $VitePortDefault)) | Out-Null
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
        Write-Host "  Fix the $nCrit critical issue(s) above, then re-run easyrun.ps1." -ForegroundColor Red
        Write-Host "  (Tip: easyrun.ps1 -Check for a standalone report.)" -ForegroundColor DarkGray
    }
    if ($nCrit -gt 0) { return 1 } else { return 0 }
}

# Mirrors psp-ui/scripts/ensure-{desktop,web}-env.mjs — keep both in sync.
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
    Log-Info "Wrote psp-ui/.env (web mode, WS_URL=$(if ($wsUrl) { $wsUrl } else { '<empty>' }))"
}

function Write-DesktopEnv() {
    New-Item -ItemType Directory -Force -Path $UiDir | Out-Null
    Set-Content -NoNewline -Path $EnvFile -Value "PUBLIC_WS_URL=127.0.0.1:5174/ws`nPUBLIC_DESKTOP_MODE=true`n"
    Log-Info "Wrote psp-ui/.env (desktop mode)"
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

function Ensure-BunInstall([bool]$force) {
    $bun = Resolve-Tool "bun"
    if (-not $bun) { Die "bun not found — run .\easyrun.ps1 -Check first." }
    if ((Test-Path $NodeModules) -and -not $force) {
        Log-Info "psp-ui/node_modules present — skipping bun install."
        return
    }
    Log-Info "Running 'bun install' in psp-ui/ (first run can take a while)…"
    Push-Location $UiDir
    try {
        & $bun install
        if ($LASTEXITCODE -ne 0) { Pop-Location; Die "bun install failed." }
    } finally { Pop-Location }
    Log-Ok "bun install complete."
}

function Ensure-Wasm([bool]$rebuild) {
    # psp_bg.wasm existing is not a safe skip condition: psp.js is tracked and
    # psp_bg.wasm is gitignored, so a checkout/pull restores the throwing stub
    # over the real entry while the stale .wasm survives. Mirrors ensure_wasm
    # in easyrun.sh.
    $wasmFile = Join-Path $WasmOut "psp_bg.wasm"
    $entryJs  = Join-Path $WasmOut "psp.js"
    $pkgJson  = Join-Path $WasmOut "package.json"
    $stubMarker = "psp wasm not built" # text baked into the committed psp.js placeholder

    $reason = $null
    if ($rebuild) {
        $reason = "-RebuildWasm"
    } elseif (-not (Test-Path $wasmFile)) {
        $reason = "psp_bg.wasm missing"
    } elseif (-not (Test-Path $pkgJson) -or -not (Test-Path $entryJs)) {
        $reason = "incomplete wasm package (interrupted build?)"
    } elseif (Select-String -Path $entryJs -Pattern $stubMarker -Quiet) {
        $reason = "psp.js is the committed placeholder (git restored it over the build output)"
    } else {
        $wasmMtime = (Get-Item $wasmFile).LastWriteTime
        $crateDirs = @("psp-web", "psp-app", "psp-core", "psp-db") |
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
        Log-Info "WASM up to date (psp-ui/src/lib/wasm/psp/psp_bg.wasm) (-RebuildWasm to redo)."
        return
    }

    $cargo = Resolve-Tool "cargo"
    $wasmPack = Resolve-Tool "wasm-pack"
    if (-not $cargo)    { Die "cargo not found — run .\easyrun.ps1 -Check first." }
    if (-not $wasmPack) { Die "wasm-pack not found — run .\easyrun.ps1 -InstallWasm first." }
    Log-Info "Building psp-web (wasm-pack): $reason"
    # Clear generated output only — an interrupted build must still leave a
    # resolvable $lib/wasm/psp behind, so the tracked placeholders have to
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
    Push-Location $PspWebDir
    try {
        & $wasmPack build --target web --out-name psp --out-dir $WasmOut
        if ($LASTEXITCODE -ne 0) { Pop-Location; Die "wasm-pack build failed." }
    } finally { Pop-Location }
    if (-not (Test-Path $wasmFile)) { Die "wasm-pack reported success but psp_bg.wasm is missing." }
    Log-Ok "psp-web WASM built."
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
    Then open a NEW terminal and re-run:  .\easyrun.ps1 -InstallWasm"
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
            Write-Host "  Then re-run: .\easyrun.ps1 -Check -Webapp" -ForegroundColor White
            return
        }
        Log-Ok "wasm-pack installed ($(probe_version 'wasm-pack' @('--version')))."
    }

    Write-Host ""
    Banner "Verifying  (-Check -Webapp)"
    Report-Preflight "webapp" $false | Out-Null
}

function Run-Web($opts) {
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
    Banner "Dev: web  (${h}:$vitePort  +  psp-server :$serverPort)"

    $vite = Spawn-BgTagged "vite" @($bun, "run", "dev:vite", "--", "--host", $h, "--port", "$vitePort") $UiDir $null
    $server = $null
    if (-not $NoServer) {
        $server = Spawn-BgTagged "psp-server" @($cargo, "run", "-p", "psp-server", "--",
            "--host", $h, "--port", "$serverPort",
            "--ui-dir", $UiDir, "--data-dir", (Join-Path $RepoRoot "data"),
            "--db", (Join-Path $RepoRoot "psp-rs.db"), "--dev") $RepoRoot $null
    }
    Wait-ForHttp "http://${h}:$vitePort" "Vite" 60 | Out-Null
    Write-Host ""
    Write-Host "  ▸ PSP web dev running:  http://${h}:$vitePort" -ForegroundColor Cyan
    Write-Host "  Ctrl-C to stop. easyrun restores psp-ui/.env on exit." -ForegroundColor DarkGray
    Write-Host ""
    if ($server) { Wait-OnProcs @($vite) @($server) } else { Wait-OnProcs @($vite) @() }
}

function Run-Desktop($opts) {
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
    Banner "Dev: desktop  (Tauri + embedded psp-server)"
    $tauri = Spawn-BgTagged "tauri" @($cargo, "tauri", "dev") $PspDesktopDir $null
    Write-Host "  Ctrl-C to stop. easyrun restores psp-ui/.env on exit." -ForegroundColor DarkGray
    Write-Host ""
    Wait-OnProcs @($tauri) @()
}

function Run-Webapp($opts) {
    $bun = Resolve-Tool "bun"
    if (-not $bun) { Die "bun not found." }
    $h = if ($HostAddr) { $HostAddr } else { "127.0.0.1" }
    $port = if ($VitePort) { $VitePort } else { $VitePortDefault }
    Ensure-BunInstall $false
    Ensure-Wasm $RebuildWasm
    Gen-JsonManifest
    Write-WebEnv ""
    Banner "Dev: webapp  (landing page + tool, browser-only)"
    $vite = Spawn-BgTagged "vite" @($bun, "run", "dev:vite", "--", "--host", $h, "--port", "$port") $UiDir @{ "VITE_TRANSPORT" = "worker" }
    Wait-ForHttp "http://${h}:$port" "Vite (webapp)" 60 | Out-Null
    Write-Host ""
    Write-Host "  ▸ PSP webapp dev running:  http://${h}:$port" -ForegroundColor Cyan
    Write-Host "  Landing-page mode (VITE_TRANSPORT=worker). Ctrl-C to stop." -ForegroundColor DarkGray
    Write-Host ""
    Wait-OnProcs @($vite) @()
}

function Run-Landing($opts) {
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
    Write-Host "  ▸ PSP landing preview:  http://${h}:$port" -ForegroundColor Cyan
    Write-Host "  Landing page only — WASM/server skipped (VITE_LANDING_ONLY)." -ForegroundColor DarkGray
    Write-Host "  Buttons that load a save won't work. Ctrl-C to stop." -ForegroundColor DarkGray
    Write-Host ""
    Wait-OnProcs @($vite) @()
}

function Run-Serve($opts) {
    $cargo = Resolve-Tool "cargo"
    if (-not $cargo) { Die "cargo not found." }
    $h = if ($HostAddr) { $HostAddr } else { "0.0.0.0" }
    $port = if ($ServerPort) { $ServerPort } else { $ServerPortDefault }
    Banner "Serve: psp-server  (${h}:$port)"
    $server = Spawn-BgTagged "psp-server" @($cargo, "run", "-p", "psp-server", "--",
        "--host", $h, "--port", "$port",
        "--ui-dir", $UiDir, "--data-dir", (Join-Path $RepoRoot "data"),
        "--db", (Join-Path $RepoRoot "psp-rs.db"), "--dev") $RepoRoot $null
    Wait-OnProcs @($server) @()
}

function Run-Docker($opts) {
    $docker = Resolve-Tool "docker"
    if (-not $docker) { Die "docker not found." }
    if (-not (Test-Path (Join-Path $RepoRoot "docker-compose.yml"))) {
        Die "docker-compose.yml not found at repo root."
    }
    $h = if ($HostAddr) { $HostAddr } else { (Detect-LanIp) }
    if (-not $h) { $h = "127.0.0.1" }
    $wsUrl = "${h}:$ServerPortDefault/ws"
    Banner "Docker: build + up  (PUBLIC_WS_URL=$wsUrl, port $ServerPortDefault)"
    Log-Info "Building image (first build is slow; bakes WS_URL into the SPA)…"
    $rc = Spawn-FgTagged "docker-build" @($docker, "compose", "build", "--build-arg", "PUBLIC_WS_URL=$wsUrl") $RepoRoot $null
    if ($rc -ne 0) { Die "docker compose build failed." }
    $rc = Spawn-FgTagged "docker-up" @($docker, "compose", "up", "-d") $RepoRoot $null
    if ($rc -ne 0) { Die "docker compose up failed." }
    Log-Ok "Docker backend up — connect at http://${h}:$ServerPortDefault"
    Write-Host "  Logs: docker compose logs -f   ·   Stop: docker compose down" -ForegroundColor DarkGray
}

function Run-BuildDesktop($opts) {
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

function Run-BuildWeb($opts) {
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

function Run-BuildPlain($opts) {
    $bun = Resolve-Tool "bun"
    if (-not $bun) { Die "bun not found." }
    Ensure-BunInstall $true
    Write-WebEnv "127.0.0.1:$ServerPortDefault/ws"
    Banner "Build: plain SPA (server-served → ui_build/)"
    $rc = Spawn-FgTagged "build" @($bun, "run", "build") $UiDir $null
    if ($rc -ne 0) { Die "build failed." }
    Log-Ok "Plain SPA build complete → ui_build/"
}

function Detect-LanIp() {
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
easyrun.ps1 — Palworld Save Pal dev/launch/build helper (Windows).
Runs from source; does NOT auto-install tools (run -Check for a report card).

mode (pick one; defaults to -Web):
  -Web              Dev: Vite + psp-server (tool-only SPA).
  -Desktop          Dev: Tauri native window + embedded server.
  -Webapp           Dev: landing page + tool (VITE_TRANSPORT=worker).
  -Landing          Dev: landing page ONLY — no WASM, no server (VITE_LANDING_ONLY).
  -Docker           Build & run the self-build Docker image.
  -Serve            Run only the Rust psp-server.
  -BuildDesktop     Production desktop build → dist/.
  -BuildWeb         Production web build (landing page) → ui_build/.
  -Build            Plain SPA build (server-served) → ui_build/.
  (-Browser / -BuildBrowser exist on Linux only — see easyrun.sh.)

options:
  -Check            Run only the preflight for the selected mode, then exit.
                    Combine with a mode flag (e.g. -Check -Desktop).
  -InstallWasm      Install the WASM toolchain (wasm32 target + wasm-pack).
  -HostAddr <ip>    Host/IP bind or WS_URL host (-Web/-Serve/-Docker).
  -VitePort <p>     Vite port (default 5173).
  -ServerPort <p>   psp-server port (default 5174).
  -NoServer         (-Web) skip psp-server (Vite only).
  -SkipCheck        Skip the preflight (advanced).
  -NoInstall        Skip bun install if node_modules exists.
  -RebuildWasm      (-Webapp/-BuildWeb) force wasm-pack rebuild.
  -Json             Machine-readable preflight JSON (implies -Check).
  -ForceCheckMode <m>   Override the preflight mode (advanced).
  -Help             Show this help.

macOS/Linux users: run easyrun.sh instead.

NOTE: -HostAddr (not -Host) is used because -Host is a reserved PowerShell
common parameter name.
'@ | Write-Host
}

if ($Help) { Show-Usage; exit 0 }

# browser-mode replaces the webview with a terminal launcher — a Linux-only
# feature. The cargo feature is inert on Windows, so refuse the flags instead
# of silently building the normal webview app.
if ($Browser -or $BuildBrowser) {
    Die "browser-mode is Linux-only. On Linux use: ./easyrun.sh --browser (dev) or --build-browser (AppImage)."
}

$mode = if ($ForceCheckMode) { $ForceCheckMode }
        elseif ($Desktop)     { "desktop" }
        elseif ($Webapp)      { "webapp" }
        elseif ($Landing)     { "landing" }
        elseif ($Docker)      { "docker" }
        elseif ($Serve)       { "serve" }
        elseif ($BuildDesktop){ "build-desktop" }
        elseif ($BuildWeb)    { "build-web" }
        elseif ($Build)       { "build" }
        else                  { "web" }

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
        "web"           { Run-Web $null }
        "desktop"       { Run-Desktop $null }
        "webapp"        { Run-Webapp $null }
        "landing"       { Run-Landing $null }
        "docker"        { Run-Docker $null }
        "serve"         { Run-Serve $null }
        "build-desktop" { Run-BuildDesktop $null }
        "build-web"     { Run-BuildWeb $null }
        "build"         { Run-BuildPlain $null }
    }
}
