# PalStudio desktop — fail-closed Windows PowerShell installer.
#
# The release asset, checksum manifest, and Ed25519 signature are fetched over
# HTTPS. The manifest is mandatory. ZIP members are validated and extracted
# into a private staging directory before the existing installation is moved
# aside and replaced in one recoverable transaction.

[CmdletBinding()]
param([switch]$Msi)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

try {
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
} catch {
    throw 'TLS 1.2 is required to download release metadata and artifacts'
}

$onWindows = [Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
    [Runtime.InteropServices.OSPlatform]::Windows)

$Repo = if ($env:PALSTUDIO_REPO) { $env:PALSTUDIO_REPO } else { 'oMaN-Rod/palworld-save-pal' }
$Version = if ($env:PALSTUDIO_VERSION) { $env:PALSTUDIO_VERSION } else { '' }
$ApiBase = if ($env:PALSTUDIO_API_BASE) { $env:PALSTUDIO_API_BASE } else { 'https://api.github.com' }
$DlBase = if ($env:PALSTUDIO_DOWNLOAD_BASE) { $env:PALSTUDIO_DOWNLOAD_BASE } else { 'https://github.com' }

# Real installs run on Windows only; the loopback-mock E2E test
# (scripts/test-install-flow.sh) drives this script under pwsh on
# Linux/macOS, so a loopback download base also relaxes the platform gate.
$dlUri = $null
$onTestMock = [Uri]::TryCreate($DlBase, [UriKind]::Absolute, [ref]$dlUri) -and $dlUri.Scheme -eq 'http' -and
    ($dlUri.Host -eq '127.0.0.1' -or $dlUri.Host -eq '::1' -or $dlUri.Host -eq '[::1]')
if (-not $onWindows -and -not $onTestMock) { throw 'This installer is for Windows only' }
$UseMsi = $Msi.IsPresent -or $env:PALSTUDIO_MSI -eq '1'
$InstallInput = if ($env:PALSTUDIO_INSTALL_DIR) {
    $env:PALSTUDIO_INSTALL_DIR
} else {
    Join-Path $env:LOCALAPPDATA 'PalStudio'
}
$SkipShortcuts = $env:PALSTUDIO_SKIP_SHORTCUTS -eq '1'

$SigningPublicKey = @'
-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEAe6TtXDrzhlHFk605YUwwC9oKz42CkwFcrta4jVGWdUM=
-----END PUBLIC KEY-----
'@

function Write-Info([string]$Message) { Write-Host "==> $Message" -ForegroundColor Cyan }
function Fail([string]$Message) { throw $Message }

# openssl ships as openssl.exe on Windows; pwsh on Linux/macOS uses the plain name.
$OpenSsl = if ($IsLinux -or $IsMacOS) { 'openssl' } else { 'openssl.exe' }

# Test hook for scripts/test-install-flow.sh: a loopback mock may substitute
# its own signing key so the signed-manifest path can be exercised end to end.
if ($env:PALSTUDIO_SIGNING_PUBLIC_KEY_FILE) {
    $dlParsed = $null
    if ([Uri]::TryCreate($DlBase, [UriKind]::Absolute, [ref]$dlParsed) -and
        $dlParsed.Scheme -eq 'http' -and
        ($dlParsed.Host -eq '127.0.0.1' -or $dlParsed.Host -eq '::1' -or $dlParsed.Host -eq '[::1]')) {
        $SigningPublicKey = [IO.File]::ReadAllText($env:PALSTUDIO_SIGNING_PUBLIC_KEY_FILE)
    } else {
        Fail 'PALSTUDIO_SIGNING_PUBLIC_KEY_FILE requires a loopback download base'
    }
}

function Require-Command([string]$Name) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        Fail "$Name is required but was not found"
    }
}

function Assert-Repository([string]$Value) {
    if ($Value -notmatch '^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$') {
        Fail 'PALSTUDIO_REPO must be a GitHub owner/name pair'
    }
}

function Assert-Version([string]$Value) {
    if ($Value -notmatch '^v?[0-9]+\.[0-9]+\.[0-9]+([.-][A-Za-z0-9.-]+)?$') {
        Fail 'release tag must look like v1.5.0'
    }
}

function Test-Reparse([string]$Path) {
    $item = Get-Item -LiteralPath $Path -Force -ErrorAction Stop
    return (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)
}

function Assert-NoReparseTree([string]$Root) {
    if (Test-Reparse $Root) { Fail "reparse points are not allowed in $Root" }
    Get-ChildItem -LiteralPath $Root -Force -Recurse -ErrorAction Stop | ForEach-Object {
        if (($_.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
            Fail "reparse point is not allowed: $($_.FullName)"
        }
    }
}

function Assert-NoReparsePath([string]$Path) {
    $current = [IO.Path]::GetFullPath($Path)
    while ($current) {
        if (Test-Path -LiteralPath $current) {
            if (Test-Reparse $current) { Fail "reparse point is not allowed in path: $current" }
        }
        $parent = Split-Path -Parent $current
        if (-not $parent -or $parent -eq $current) { break }
        $current = $parent
    }
}

function Assert-DedicatedInstallPath([string]$Path) {
    $full = [IO.Path]::GetFullPath($Path)
    $root = [IO.Path]::GetPathRoot($full)
    if ([string]::IsNullOrWhiteSpace($full) -or $full.TrimEnd('\') -eq $root.TrimEnd('\')) {
        Fail 'PALSTUDIO_INSTALL_DIR must name a dedicated installation directory'
    }
    $parent = Split-Path -Parent $full
    if (-not $parent) { Fail 'PALSTUDIO_INSTALL_DIR has no usable parent directory' }
    New-Item -ItemType Directory -Force -Path $parent | Out-Null
    Assert-NoReparsePath $parent
    if (Test-Path -LiteralPath $full) {
        if (Test-Reparse $full) { Fail 'PALSTUDIO_INSTALL_DIR may not be a reparse point' }
        if (-not (Get-Item -LiteralPath $full).PSIsContainer) { Fail 'PALSTUDIO_INSTALL_DIR must be a directory' }
    }
    return $full
}

function Set-PrivateAcl([string]$Path) {
    if (Test-Reparse $Path) { Fail "refusing to secure a reparse point: $Path" }
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent().Name
    $grant = '{0}:(OI)(CI)F' -f $identity
    & icacls.exe $Path '/inheritance:r' '/grant:r' $grant '/T' | Out-Null
    if ($LASTEXITCODE -ne 0) { Fail "could not apply private ACLs to $Path" }
}

function Test-LoopbackUri([Uri]$Parsed) {
    return $Parsed.Scheme -eq 'http' -and
        ($Parsed.Host -eq '127.0.0.1' -or $Parsed.Host -eq '::1' -or $Parsed.Host -eq '[::1]')
}

function Invoke-ReleaseDownload([string]$Uri, [string]$Destination) {
    $parsed = $null
    if (-not [Uri]::TryCreate($Uri, [UriKind]::Absolute, [ref]$parsed) -or
        ($parsed.Scheme -ne 'https' -and -not (Test-LoopbackUri $parsed)) -or
        [string]::IsNullOrWhiteSpace($parsed.Host) -or
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
                ($currentParsed.Scheme -ne 'https' -and -not (Test-LoopbackUri $currentParsed)) -or
                [string]::IsNullOrWhiteSpace($currentParsed.Host)) {
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

function ConvertTo-PsLiteral([string]$Value) {
    return "'{0}'" -f $Value.Replace("'", "''")
}

function Assert-SafeZip([string]$Archive) {
    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [IO.Compression.ZipFile]::OpenRead($Archive)
    $names = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::OrdinalIgnoreCase)
    $total = 0L
    try {
        foreach ($entry in $zip.Entries) {
            $name = ([string]$entry.FullName).Replace('\', '/')
            $trimmed = $name.TrimEnd('/')
            if ([string]::IsNullOrWhiteSpace($trimmed) -or $name -match '[\r\n\x00]' -or
                $name.StartsWith('/') -or $name -match '^[A-Za-z]:/' -or
                $name -match '(^|/)\.\.?(/|$)' -or
                (-not $trimmed.Equals('PalStudio', [StringComparison]::OrdinalIgnoreCase) -and
                 -not $trimmed.StartsWith('PalStudio/', [StringComparison]::OrdinalIgnoreCase))) {
                Fail "unsafe or unexpected ZIP member: $name"
            }
            if (-not $names.Add($trimmed)) { Fail "duplicate ZIP member: $name" }

            $external = [uint32]$entry.ExternalAttributes
            $unixMode = ($external -shr 16) -band 0xFFFF
            $fileType = $unixMode -band 0xF000
            if ($fileType -ne 0 -and $fileType -ne 0x4000 -and $fileType -ne 0x8000) {
                Fail "ZIP links or special files are not permitted: $name"
            }
            if (($unixMode -band 0x0E00) -ne 0) { Fail "privileged ZIP mode is not permitted: $name" }
            if ($entry.Length -gt 536870912) { Fail "ZIP member is too large: $name" }
            $total += [int64]$entry.Length
            if ($total -gt 2147483648) { Fail 'ZIP expands beyond the permitted size limit' }
        }
    } finally {
        $zip.Dispose()
    }
}

function Expand-SafeZip([string]$Archive, [string]$Destination) {
    Assert-SafeZip $Archive
    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $sep = [IO.Path]::DirectorySeparatorChar
    $root = ([IO.Path]::GetFullPath($Destination)).TrimEnd('\', '/') + $sep
    $zip = [IO.Compression.ZipFile]::OpenRead($Archive)
    try {
        foreach ($entry in $zip.Entries) {
            $name = ([string]$entry.FullName).Replace('\', '/')
            $destinationPath = [IO.Path]::GetFullPath((Join-Path $Destination ($name -replace '/', [string]$sep)))
            if (-not $destinationPath.StartsWith($root, [StringComparison]::OrdinalIgnoreCase)) {
                Fail "ZIP member escapes the extraction directory: $name"
            }
            $isDirectory = $name.EndsWith('/')
            if ($isDirectory) {
                New-Item -ItemType Directory -Force -Path $destinationPath | Out-Null
                continue
            }
            $parent = Split-Path -Parent $destinationPath
            New-Item -ItemType Directory -Force -Path $parent | Out-Null
            Assert-NoReparsePath $parent
            if (Test-Path -LiteralPath $destinationPath) { Fail "ZIP member collision: $name" }
            $input = $entry.Open()
            $output = [IO.File]::Open($destinationPath, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
            try { $input.CopyTo($output) } finally { $output.Dispose(); $input.Dispose() }
        }
    } finally {
        $zip.Dispose()
    }
    Assert-NoReparseTree $Destination
}

function Preserve-UserState([string]$Existing, [string]$Staged) {
    foreach ($name in @('ps-rs.db', 'launcher.json')) {
        $source = Join-Path $Existing $name
        if (-not (Test-Path -LiteralPath $source)) { continue }
        if (Test-Reparse $source) { Fail "existing user state may not be a reparse point: $source" }
        Copy-Item -LiteralPath $source -Destination (Join-Path $Staged $name) -Force
    }
}

function New-Shortcut([string]$Path, [string]$Target, [string]$WorkingDirectory) {
    $parent = Split-Path -Parent $Path
    New-Item -ItemType Directory -Force -Path $parent | Out-Null
    if (Test-Path -LiteralPath $Path) {
        if (Test-Reparse $Path) { Fail "shortcut path is a reparse point: $Path" }
        Remove-Item -LiteralPath $Path -Force
    }
    $shell = New-Object -ComObject WScript.Shell
    $shortcut = $shell.CreateShortcut($Path)
    $shortcut.TargetPath = $Target
    $shortcut.WorkingDirectory = $WorkingDirectory
    $shortcut.Description = 'PalStudio desktop app'
    $shortcut.Save()
}

$Prefix = $null
$Temporary = $null
$Backup = $null
$RollbackNeeded = $false
$ShortcutChanges = @()

try {
    Assert-Repository $Repo
    Require-Command $OpenSsl
    if ($onWindows) { Require-Command 'icacls.exe' }
    $Prefix = Assert-DedicatedInstallPath $InstallInput

    if (-not $Version) {
        Write-Info "looking up the latest release of $Repo"
        $latest = Invoke-RestMethod -UseBasicParsing -Uri "$ApiBase/repos/$Repo/releases/latest" -Headers @{
            'User-Agent' = 'palstudio-installer'
            Accept = 'application/vnd.github+json'
        } -TimeoutSec 30
        $Version = [string]$latest.tag_name
    }
    Assert-Version $Version

    $kind = if ($UseMsi) { 'windows.msi' } else { 'windows-standalone.zip' }
    $asset = "PalStudio-$Version-$kind"
    $checksumsAsset = "PalStudio-$Version-checksums.txt"
    $base = "$DlBase/$Repo/releases/download/$Version"
    Write-Info "installing PalStudio desktop $Version ($kind)"

    $Temporary = Join-Path ([IO.Path]::GetTempPath()) ("palstudio-install-{0}" -f ([Guid]::NewGuid().ToString('N')))
    New-Item -ItemType Directory -Path $Temporary | Out-Null
    $download = Join-Path $Temporary $asset
    $checksums = Join-Path $Temporary $checksumsAsset
    $signature = "$checksums.sig"
    $publicKey = Join-Path $Temporary 'release-public.pem'
    Invoke-ReleaseDownload "$base/$asset" $download
    Invoke-ReleaseDownload "$base/$checksumsAsset" $checksums
    try {
        Invoke-ReleaseDownload "$base/$checksumsAsset.sig" $signature
    } catch {
        Fail "release $Version has no signed checksum manifest ($checksumsAsset.sig); releases published before signed manifests cannot be installed"
    }
    [IO.File]::WriteAllText($publicKey, $SigningPublicKey, [Text.UTF8Encoding]::new($false))
    & $OpenSsl pkeyutl -verify -pubin -inkey $publicKey -rawin -in $checksums -sigfile $signature 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) { Fail 'signed release manifest verification failed' }

    $expected = $null
    foreach ($line in Get-Content -LiteralPath $checksums) {
        $parts = $line -split '\s+'
        if ($parts.Count -ge 2 -and $parts[1] -eq $asset) { $expected = $parts[0]; break }
    }
    if ($expected -notmatch '^[0-9a-fA-F]{64}$') { Fail "signed manifest has no valid checksum for $asset" }
    $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $download).Hash
    if ($actual -ine $expected) { Fail "checksum mismatch for $asset" }
    Write-Info 'signed manifest and checksum verified'

    if ($UseMsi) {
        Write-Info 'running the verified MSI installer'
        $process = Start-Process -FilePath 'msiexec.exe' -ArgumentList @('/i', $download, '/passive', '/norestart') -Wait -PassThru
        if ($process.ExitCode -ne 0 -and $process.ExitCode -ne 3010) {
            Fail "msiexec failed with exit code $($process.ExitCode)"
        }
        Write-Host "PalStudio $Version installed by MSI." -ForegroundColor Green
    } else {
        $extract = Join-Path $Temporary 'extract'
        New-Item -ItemType Directory -Path $extract | Out-Null
        Expand-SafeZip $download $extract
        $staged = Join-Path $extract 'PalStudio'
        $launcher = Join-Path $staged 'bin\palstudio.exe'
        if (-not (Test-Path -LiteralPath $launcher -PathType Leaf)) { Fail 'bundle layout error: expected PalStudio\bin\palstudio.exe' }
        if (-not (Test-Path -LiteralPath (Join-Path $staged 'ui_build\index.html') -PathType Leaf)) { Fail 'bundle is missing ui_build\index.html' }
        if (-not (Test-Path -LiteralPath (Join-Path $staged 'data\json') -PathType Container)) { Fail 'bundle is missing data\json' }
        if (Test-Path -LiteralPath $Prefix) {
            Assert-NoReparseTree $Prefix
            Preserve-UserState $Prefix $staged
        }
        Assert-NoReparseTree $extract

        $Backup = "$Prefix.previous.$PID"
        if (Test-Path -LiteralPath $Backup) { Fail "rollback path already exists: $Backup" }
        if (Test-Path -LiteralPath $Prefix) { Move-Item -LiteralPath $Prefix -Destination $Backup }
        $RollbackNeeded = $true
        Move-Item -LiteralPath $staged -Destination $Prefix
        if ($onWindows) { Set-PrivateAcl $Prefix }
        if (-not (Test-Path -LiteralPath (Join-Path $Prefix 'bin\palstudio.exe') -PathType Leaf)) { Fail 'installed launcher validation failed' }
        if (-not (Test-Path -LiteralPath (Join-Path $Prefix 'ui_build\index.html') -PathType Leaf)) { Fail 'installed UI validation failed' }

        if ($onWindows -and -not $SkipShortcuts) {
            $desktopExe = Join-Path $Prefix 'bin\palstudio-desktop.exe'
            if (-not (Test-Path -LiteralPath $desktopExe -PathType Leaf)) { Fail 'bundle has no desktop executable' }
            $shortcutPaths = @(
                (Join-Path ([Environment]::GetFolderPath('Programs')) 'PalStudio\PalStudio.lnk'),
                (Join-Path ([Environment]::GetFolderPath('Desktop')) 'PalStudio.lnk')
            )
            foreach ($shortcutPath in $shortcutPaths) {
                $oldPath = Join-Path $Temporary ("old-shortcut-{0}.lnk" -f ([Guid]::NewGuid().ToString('N')))
                $hadOld = Test-Path -LiteralPath $shortcutPath
                if ($hadOld) {
                    if (Test-Reparse $shortcutPath) { Fail "shortcut path is a reparse point: $shortcutPath" }
                    Copy-Item -LiteralPath $shortcutPath -Destination $oldPath -Force
                }
                $ShortcutChanges += [pscustomobject]@{ Path = $shortcutPath; Old = $oldPath; HadOld = $hadOld }
                New-Shortcut $shortcutPath $desktopExe $Prefix
            }
        }

        if ($env:PALSTUDIO_SKIP_PATH -ne '1') {
            $binDir = Join-Path $Prefix 'bin'
            $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
            $pathEntries = @()
            if ($userPath) { $pathEntries = @($userPath -split ';' | Where-Object { $_ }) }
            if ($pathEntries -notcontains $binDir) {
                [Environment]::SetEnvironmentVariable('Path', (($pathEntries + $binDir) -join ';'), 'User')
            }
        }

        if (Test-Path -LiteralPath $Backup) { Remove-Item -LiteralPath $Backup -Recurse -Force }
        $RollbackNeeded = $false
        Write-Host "PalStudio $Version installed under $Prefix." -ForegroundColor Green
    }
} catch {
    $message = $_.Exception.Message
    if ($RollbackNeeded) {
        try {
            if (Test-Path -LiteralPath $Prefix) { Move-Item -LiteralPath $Prefix -Destination "$Prefix.failed.$PID" -Force }
            if ($Backup -and (Test-Path -LiteralPath $Backup)) { Move-Item -LiteralPath $Backup -Destination $Prefix -Force }
        } catch {
            Write-Error "installation rollback failed: $($_.Exception.Message)"
        }
    }
    foreach ($change in $ShortcutChanges) {
        try {
            if (Test-Path -LiteralPath $change.Path) { Remove-Item -LiteralPath $change.Path -Force }
            if ($change.HadOld -and (Test-Path -LiteralPath $change.Old)) {
                New-Item -ItemType Directory -Force -Path (Split-Path -Parent $change.Path) | Out-Null
                Move-Item -LiteralPath $change.Old -Destination $change.Path -Force
            }
        } catch {
            Write-Error "shortcut rollback failed for $($change.Path): $($_.Exception.Message)"
        }
    }
    throw "PalStudio installation failed: $message"
} finally {
    if ($Temporary -and (Test-Path -LiteralPath $Temporary)) {
        Remove-Item -LiteralPath $Temporary -Recurse -Force -ErrorAction SilentlyContinue
    }
}
