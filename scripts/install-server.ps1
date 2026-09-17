# PalStudio server — fail-closed Windows PowerShell installer.
#
# Downloads a release bundle only after a signed checksum manifest verifies,
# rejects unsafe archive members, and atomically replaces the prior install.
# A failed task registration or health check restores the previous files and
# scheduled-task definition.

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

try {
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
} catch {
    throw 'TLS 1.2 is required to download release metadata and artifacts'
}

$Repo = if ($env:REPO) { $env:REPO } else { 'oMaN-Rod/palworld-save-pal' }
$Version = if ($env:VERSION) { $env:VERSION } else { '' }
$BindHost = if ($env:HOST) { $env:HOST } else { '127.0.0.1' }
$Port = if ($env:PS_PORT) { $env:PS_PORT } elseif ($env:PORT) { $env:PORT } else { '5174' }
$Listen = if ($env:PS_LISTEN) { $env:PS_LISTEN } elseif ($env:LISTEN) { $env:LISTEN } else { '' }
$Pin = if ($null -ne $env:PS_PIN) { $env:PS_PIN } elseif ($null -ne $env:PIN) { $env:PIN } else { '' }
$PrefixInput = if ($env:PREFIX) { $env:PREFIX } elseif ($env:PALSTUDIO_INSTALL_DIR) { $env:PALSTUDIO_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'PalStudio' }
$Mode = if ($env:MODE) { $env:MODE } elseif ($env:NO_SERVICE -eq '1') { 'standalone' } else { '' }

$SigningPublicKey = @'
-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEAe6TtXDrzhlHFk605YUwwC9oKz42CkwFcrta4jVGWdUM=
-----END PUBLIC KEY-----
'@

function Write-Info([string]$Message) { Write-Host "==> $Message" -ForegroundColor Cyan }
function Fail([string]$Message) { throw $Message }

function Require-Command([string]$Name) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) { Fail "$Name is required but was not found" }
}

if ($Repo -notmatch '^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$') { Fail 'REPO must be a GitHub owner/name pair' }
if ($BindHost -match '[\r\n\x00\s;|&<>`"]') { Fail 'HOST contains invalid characters' }
if ($Port -notmatch '^[0-9]+$' -or [int64]$Port -lt 1 -or [int64]$Port -gt 65535) { Fail 'PORT must be between 1 and 65535' }
if ($Listen -and $Listen -notin @('localhost', 'lan', 'tailscale', 'wan')) { Fail 'PS_LISTEN/LISTEN must be localhost, lan, tailscale, or wan' }
if ($Pin -and ($Pin.Length -lt 4 -or $Pin.Length -gt 128 -or $Pin -match '[\r\n\x00-\x1f\x7f]')) { Fail 'PS_PIN/PIN must contain 4 to 128 non-control characters' }
if ($Mode -and $Mode -notin @('ask', 'standalone', 'service')) { Fail 'MODE must be ask, standalone, or service' }

$Prefix = [IO.Path]::GetFullPath($PrefixInput)
if ([string]::IsNullOrWhiteSpace($Prefix) -or $Prefix -eq [IO.Path]::GetPathRoot($Prefix)) { Fail 'PREFIX must name a dedicated installation directory' }

function Invoke-ReleaseDownload([string]$Uri, [string]$Destination) {
    $parsed = $null
    if (-not [Uri]::TryCreate($Uri, [UriKind]::Absolute, [ref]$parsed) -or
        $parsed.Scheme -ne 'https' -or [string]::IsNullOrWhiteSpace($parsed.Host) -or
        $Uri -match '[\r\n\x00]') {
        Fail "refusing non-HTTPS release URL: $Uri"
    }
    Add-Type -AssemblyName System.Net.Http
    $handler = [Net.Http.HttpClientHandler]::new()
    $handler.AllowAutoRedirect = $false
    $client = [Net.Http.HttpClient]::new($handler)
    $current = $Uri
    try {
        for ($redirect = 0; $redirect -le 5; $redirect++) {
            $currentParsed = $null
            if (-not [Uri]::TryCreate($current, [UriKind]::Absolute, [ref]$currentParsed) -or
                $currentParsed.Scheme -ne 'https' -or [string]::IsNullOrWhiteSpace($currentParsed.Host)) {
                Fail "release download redirected to a non-HTTPS URL: $current"
            }
            $response = $client.GetAsync($current, [Net.Http.HttpCompletionOption]::ResponseHeadersRead).GetAwaiter().GetResult()
            try {
                $status = [int]$response.StatusCode
                if ($status -ge 300 -and $status -lt 400) {
                    $location = $response.Headers.Location
                    if ($null -eq $location) { Fail 'release download returned a redirect without a location' }
                    $current = ([Uri]::new($currentParsed, $location)).AbsoluteUri
                    continue
                }
                if (-not $response.IsSuccessStatusCode) { Fail "release download failed with HTTP $status" }
                $stream = [IO.File]::Open($Destination, [IO.FileMode]::Create, [IO.FileAccess]::Write, [IO.FileShare]::None)
                try {
                    $response.Content.CopyToAsync($stream).GetAwaiter().GetResult()
                } finally {
                    $stream.Dispose()
                }
                return
            } finally {
                $response.Dispose()
            }
        }
        Fail 'release download followed too many redirects'
    } finally {
        $client.Dispose()
        $handler.Dispose()
    }
}

if (-not $Version) {
    Write-Info "looking up the latest release of $Repo"
    $latest = Invoke-RestMethod -UseBasicParsing -Uri "https://api.github.com/repos/$Repo/releases/latest" -Headers @{ 'User-Agent' = 'palstudio-installer'; Accept = 'application/vnd.github+json' } -TimeoutSec 30
    $Version = [string]$latest.tag_name
}
if ($Version -notmatch '^v?[0-9]+\.[0-9]+\.[0-9]+([.-][A-Za-z0-9.-]+)?$') { Fail 'VERSION must be a release tag such as v1.5.0' }

$arch = switch ($env:PROCESSOR_ARCHITECTURE) {
    'AMD64' { 'x64'; break }
    'ARM64' { Fail 'Windows ARM64 has no published server bundle' }
    default { Fail "unsupported architecture '$($env:PROCESSOR_ARCHITECTURE)'" }
}
$platform = "windows-$arch"
$Asset = "palstudio-$Version-server-$platform.tar.gz"
$ChecksumsAsset = "palstudio-$Version-server-checksums.txt"
$Base = "https://github.com/$Repo/releases/download/$Version"
Write-Info "installing PalStudio server $Version ($platform)"

# openssl ships as openssl.exe on Windows; pwsh on Linux/macOS uses the plain name.
$OpenSsl = if ($IsLinux -or $IsMacOS) { 'openssl' } else { 'openssl.exe' }
Require-Command 'tar.exe'
Require-Command $OpenSsl
$tmp = Join-Path ([IO.Path]::GetTempPath()) ("palstudio-install-{0}" -f ([Guid]::NewGuid().ToString('N')))
New-Item -ItemType Directory -Path $tmp | Out-Null
$rollbackNeeded = $false
$backup = "$Prefix.previous.$PID"
$oldTaskXml = $null
$oldTaskWasRunning = $false
$taskChanged = $false
$taskWasPresent = $false

function Test-Reparse([string]$Path) {
    $item = Get-Item -LiteralPath $Path -Force -ErrorAction Stop
    return (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)
}

function Assert-NoReparseTree([string]$Root) {
    if (Test-Reparse $Root) { Fail "reparse points are not allowed in $Root" }
    Get-ChildItem -LiteralPath $Root -Force -Recurse -ErrorAction Stop | ForEach-Object {
        if (($_.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { Fail "archive contains a reparse point: $($_.FullName)" }
    }
}

function Set-PrivateAcl([string]$Path) {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent().Name
    $grant = '{0}:(OI)(CI)F' -f $identity
    & icacls.exe $Path /inheritance:r /grant:r $grant /T /C | Out-Null
    if ($LASTEXITCODE -ne 0) { Fail "could not apply private ACLs to $Path" }
}

function ConvertTo-PsLiteral([string]$Value) { return "'{0}'" -f $Value.Replace("'", "''") }

function Assert-SafeArchive([string]$Archive) {
    $listing = @(& tar.exe -tzf $Archive 2>&1)
    if ($LASTEXITCODE -ne 0) { Fail 'could not list the release archive' }
    foreach ($member in $listing) {
        $name = [string]$member
        $name = $name.Replace('\', '/')
        if (-not $name -or $name.StartsWith('/') -or $name -match '^[A-Za-z]:' -or $name -match '(^|/)\.\.?(/|$)' -or $name -match '[\r\n\x00]') { Fail "unsafe archive member: $name" }
    }
    $verbose = @(& tar.exe -tvzf $Archive 2>&1)
    if ($LASTEXITCODE -ne 0) { Fail 'could not inspect release archive entries' }
    foreach ($entry in $verbose) {
        $line = [string]$entry
        if (-not ($line.StartsWith('-') -or $line.StartsWith('d'))) { Fail 'archive links or special files are not permitted' }
        if ($line.Length -ge 10 -and $line.Substring(0, 10) -match '[sStT]') { Fail 'archive setuid, setgid, or sticky modes are not permitted' }
    }
}

try {
    $bundle = Join-Path $tmp $Asset
    $checksums = Join-Path $tmp $ChecksumsAsset
    $signature = "$checksums.sig"
    $publicKey = Join-Path $tmp 'release-public.pem'
    Invoke-ReleaseDownload "$Base/$Asset" $bundle
    Invoke-ReleaseDownload "$Base/$ChecksumsAsset" $checksums
    try {
        Invoke-ReleaseDownload "$Base/$ChecksumsAsset.sig" $signature
    } catch {
        Fail "release $Version has no signed checksum manifest ($ChecksumsAsset.sig); releases published before signed manifests cannot be installed"
    }
    [IO.File]::WriteAllText($publicKey, $SigningPublicKey, [Text.UTF8Encoding]::new($false))
    & $OpenSsl pkeyutl -verify -pubin -inkey $publicKey -rawin -in $checksums -sigfile $signature 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) { Fail 'signed release manifest verification failed' }
    $expected = $null
    foreach ($line in Get-Content -LiteralPath $checksums) {
        $parts = $line -split '\s+'
        if ($parts.Count -ge 2 -and $parts[1] -eq $Asset) { $expected = $parts[0]; break }
    }
    if ($expected -notmatch '^[0-9a-fA-F]{64}$') { Fail "signed manifest has no valid checksum for $Asset" }
    $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $bundle).Hash
    if ($actual -ine $expected) { Fail "checksum mismatch for $Asset" }
    Write-Info 'signed manifest and checksum verified'

    Assert-SafeArchive $bundle
    $extract = Join-Path $tmp 'extract'
    New-Item -ItemType Directory -Path $extract | Out-Null
    & tar.exe --no-same-owner --no-same-permissions -xzf $bundle -C $extract 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) { Fail 'could not extract the release archive' }
    $staged = Join-Path $extract 'palstudio'
    if (-not (Test-Path -LiteralPath (Join-Path $staged 'bin\palstudio.exe') -PathType Leaf)) { Fail 'bundle layout error: expected palstudio/bin/palstudio.exe' }
    if (-not (Test-Path -LiteralPath (Join-Path $staged 'ui') -PathType Container) -or -not (Test-Path -LiteralPath (Join-Path $staged 'data\json') -PathType Container)) { Fail 'bundle is missing ui/ or data/json' }
    Assert-NoReparseTree $extract

    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Prefix) | Out-Null
    if (Test-Path -LiteralPath $Prefix) {
        if (Test-Reparse $Prefix) { Fail 'PREFIX may not be a reparse point' }
        $oldDb = Join-Path $Prefix 'ps-rs.db'
        if (Test-Path -LiteralPath $oldDb) {
            if (Test-Reparse $oldDb) { Fail 'existing database may not be a reparse point' }
            Copy-Item -LiteralPath $oldDb -Destination (Join-Path $staged 'ps-rs.db') -Force
        }
    }

    if (-not $Mode) {
        $Mode = if ([Environment]::UserInteractive) { 'ask' } else { 'service' }
    }
    if ($Mode -eq 'ask') {
        Write-Host "`nHow do you want to run PalStudio?"
        Write-Host '  1) standalone'
        Write-Host '  2) background service'
        $answer = Read-Host 'Choice [1]'
        $Mode = if ($answer -in @('2', 's', 'service')) { 'service' } else { 'standalone' }
    }

    $task = Get-ScheduledTask -TaskName 'PalStudio' -ErrorAction SilentlyContinue
    if ($null -ne $task) {
        $taskWasPresent = $true
        $oldTaskXml = Export-ScheduledTask -TaskName 'PalStudio'
        $action = @($task.Actions)[0]
        if ($null -eq $action -or (([string]$action.Execute) -notmatch 'palstudio|powershell') -or (([string]$action.Arguments) -notmatch 'serve|palstudio-service')) { Fail 'existing PalStudio task is not owned by this installer' }
        $info = Get-ScheduledTaskInfo -TaskName 'PalStudio'
        if ($info.State -eq 'Running') {
            $oldTaskWasRunning = $true
            Stop-ScheduledTask -TaskName 'PalStudio'
            $deadline = (Get-Date).AddSeconds(20)
            do { Start-Sleep -Milliseconds 250; $info = Get-ScheduledTaskInfo -TaskName 'PalStudio' } while ($info.State -eq 'Running' -and (Get-Date) -lt $deadline)
            if ($info.State -eq 'Running') { Fail 'existing PalStudio task did not stop' }
        }
    }

    $wrapper = Join-Path $staged 'bin\palstudio-service.ps1'
    $binary = Join-Path $Prefix 'bin\palstudio.exe'
    $ui = Join-Path $Prefix 'ui'
    $data = Join-Path $Prefix 'data'
    $db = Join-Path $Prefix 'ps-rs.db'
    $wrapperLines = @(
        '$ErrorActionPreference = ''Stop''',
        '$env:PS_NETWORK_ENV = ''firstboot''',
        ('$env:PS_PORT = {0}' -f (ConvertTo-PsLiteral ([string]$Port)))
    )
    if ($Listen) { $wrapperLines += ('$env:PS_LISTEN = {0}' -f (ConvertTo-PsLiteral $Listen)) } else { $wrapperLines += 'Remove-Item Env:PS_LISTEN -ErrorAction SilentlyContinue' }
    if ($Pin) { $wrapperLines += ('$env:PS_PIN = {0}' -f (ConvertTo-PsLiteral $Pin)) } else { $wrapperLines += 'Remove-Item Env:PS_PIN -ErrorAction SilentlyContinue' }
    $wrapperLines += ('& {0} serve --host {1} --ui-dir {2} --data-dir {3} --db {4}' -f (ConvertTo-PsLiteral $binary), (ConvertTo-PsLiteral $BindHost), (ConvertTo-PsLiteral $ui), (ConvertTo-PsLiteral $data), (ConvertTo-PsLiteral $db))
    $wrapperLines += 'if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }'
    [IO.File]::WriteAllText($wrapper, ($wrapperLines -join [Environment]::NewLine), [Text.UTF8Encoding]::new($false))
    Set-PrivateAcl $wrapper
    Assert-NoReparseTree $extract

    if (Test-Path -LiteralPath $backup) { Fail "rollback path already exists: $backup" }
    if (Test-Path -LiteralPath $Prefix) { Move-Item -LiteralPath $Prefix -Destination $backup }
    $rollbackNeeded = $true
    Move-Item -LiteralPath $staged -Destination $Prefix
    Set-PrivateAcl $Prefix

    if ($Mode -eq 'service') {
        $powershell = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
        $taskArgs = '-NoProfile -NonInteractive -ExecutionPolicy RemoteSigned -File "{0}"' -f (Join-Path $Prefix 'bin\palstudio-service.ps1')
        $newAction = New-ScheduledTaskAction -Execute $powershell -Argument $taskArgs -WorkingDirectory $Prefix
        $trigger = New-ScheduledTaskTrigger -AtLogOn -User $env:USERNAME
        $settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -RestartCount 3 -RestartInterval (New-TimeSpan -Minutes 1) -MultipleInstances IgnoreNew -ExecutionTimeLimit (New-TimeSpan -Days 3650) -StartWhenAvailable
        $principal = New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType InteractiveToken -RunLevel Limited
        Register-ScheduledTask -TaskName 'PalStudio' -Action $newAction -Trigger $trigger -Settings $settings -Principal $principal -Force | Out-Null
        $taskChanged = $true
        Start-ScheduledTask -TaskName 'PalStudio'
        $healthHost = if ($BindHost -in @('0.0.0.0', '::')) { '127.0.0.1' } else { $BindHost }
        $healthUrl = if ($healthHost -like '*:*') { "http://[$healthHost]:$Port/" } else { "http://$healthHost`:$Port/" }
        $healthy = $false
        $deadline = (Get-Date).AddSeconds(30)
        while ((Get-Date) -lt $deadline) {
            try {
                $request = [Net.WebRequest]::Create($healthUrl)
                $request.Timeout = 3000
                $response = $request.GetResponse()
                $code = [int]$response.StatusCode
                $response.Close()
                if ($code -ge 100 -and $code -le 599) { $healthy = $true; break }
            } catch [Net.WebException] {
                if ($_.Exception.Response) { $healthy = $true; break }
            }
            Start-Sleep -Milliseconds 500
        }
        if (-not $healthy) { Fail 'server did not pass the health check within 30 seconds' }
        Write-Info 'Scheduled Task PalStudio is running'
    } else {
        if ($taskWasPresent) {
            $taskChanged = $true
            Unregister-ScheduledTask -TaskName 'PalStudio' -Confirm:$false
        }
        Write-Info 'standalone mode selected; no service is installed'
    }

    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $binDir = Join-Path $Prefix 'bin'
    $pathEntries = @($userPath -split ';' | Where-Object { $_ })
    if ($pathEntries -notcontains $binDir) { [Environment]::SetEnvironmentVariable('Path', (($pathEntries + $binDir) -join ';'), 'User') }
    if (Test-Path -LiteralPath $backup) { Remove-Item -LiteralPath $backup -Recurse -Force }
    $rollbackNeeded = $false
    Write-Info "PalStudio $Version installed under $Prefix"
} catch {
    $message = $_.Exception.Message
    if ($rollbackNeeded) {
        try {
            $failed = "$Prefix.failed.$PID"
            if (Test-Path -LiteralPath $failed) { $failed = "$Prefix.failed.$PID.$([Guid]::NewGuid().ToString('N'))" }
            if (Test-Path -LiteralPath $Prefix) { Move-Item -LiteralPath $Prefix -Destination $failed }
            if (Test-Path -LiteralPath $backup) { Move-Item -LiteralPath $backup -Destination $Prefix -Force }
            if ($taskChanged -and $taskWasPresent -and $oldTaskXml) {
                Register-ScheduledTask -TaskName 'PalStudio' -Xml $oldTaskXml -Force | Out-Null
            } elseif ($taskChanged -and -not $taskWasPresent) {
                Unregister-ScheduledTask -TaskName 'PalStudio' -Confirm:$false -ErrorAction SilentlyContinue
            }
        } catch {
            Write-Warning "rollback was incomplete: $($_.Exception.Message)"
        }
    }
    if ($oldTaskWasRunning) {
        try { Start-ScheduledTask -TaskName 'PalStudio' } catch { Write-Warning "could not restart the previous PalStudio task: $($_.Exception.Message)" }
    }
    throw "PalStudio installation failed: $message"
} finally {
    if (Test-Path -LiteralPath $tmp) { Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue }
}
