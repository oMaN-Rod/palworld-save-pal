param(
    [string]$GameDir = "",
    [string]$Root = "",
    [string]$Config = "Game__Shipping__Win64",
    [string]$Ue4ssZip = "",
    [switch]$SkipUe4ss,
    [switch]$RemoveWin64Ue4ss
)
$ErrorActionPreference = "Stop"

if (-not $GameDir) { throw "-GameDir is required: the folder holding Pal\Binaries\Win64\Palworld-Win64-Shipping.exe" }
$win64 = Join-Path $GameDir "Pal\Binaries\Win64"
if (-not (Test-Path (Join-Path $win64 "Palworld-Win64-Shipping.exe"))) { throw "game not found at $GameDir" }
if (Get-Process -Name "Palworld-Win64-Shipping" -ErrorAction SilentlyContinue) {
    throw "Palworld is running; the mod DLL is locked while it runs. Close the game and re-run."
}

$nativeUe4ss = Join-Path $GameDir "Mods\NativeMods\UE4SS"
$nativeExists = Test-Path (Join-Path $nativeUe4ss "UE4SS.dll")

if ($nativeExists) {
    $win64HasUe4ss = (Test-Path (Join-Path $win64 "dwmapi.dll")) -and (Test-Path (Join-Path $win64 "ue4ss\UE4SS.dll"))
    if ($win64HasUe4ss) {
        if ($RemoveWin64Ue4ss) {
            Remove-Item -Force (Join-Path $win64 "dwmapi.dll")
            Remove-Item -Recurse -Force (Join-Path $win64 "ue4ss")
            Write-Host "Removed second UE4SS instance from Win64"
        } else {
            Write-Host "WARNING: a second UE4SS instance exists at $win64 (dwmapi.dll + ue4ss\UE4SS.dll) alongside the native-mods UE4SS at $nativeUe4ss. Running both crashes the game. Re-run with -RemoveWin64Ue4ss to remove the Win64 instance, or remove it yourself, before installing." -ForegroundColor Red
            exit 1
        }
    }
    $modsRoot = Join-Path $nativeUe4ss "Mods"
    $targetLabel = "native-mods instance"
} else {
    if (-not $SkipUe4ss -and -not (Test-Path (Join-Path $win64 "dwmapi.dll"))) {
        if (-not $Ue4ssZip -or -not (Test-Path $Ue4ssZip)) {
            throw "no UE4SS instance found in $GameDir. Subscribe to the Steam Workshop UE4SS, or pass -Ue4ssZip <UE4SS release zip> to install one under Pal\Binaries\Win64."
        }
        $tmp = Join-Path $env:TEMP "amity-ue4ss-extract"
        if (Test-Path $tmp) { Remove-Item -Recurse -Force $tmp }
        Expand-Archive -Path $Ue4ssZip -DestinationPath $tmp
        Copy-Item (Join-Path $tmp "dwmapi.dll") $win64
        Copy-Item -Recurse (Join-Path $tmp "ue4ss") (Join-Path $win64 "ue4ss")
        Remove-Item -Force (Join-Path $win64 "ue4ss\UE4SS.pdb") -ErrorAction SilentlyContinue
        Remove-Item -Recurse -Force $tmp
        Write-Host "UE4SS installed"
    } elseif (-not $SkipUe4ss) {
        Write-Host "UE4SS already present"
    }
    $modsRoot = Join-Path $win64 "ue4ss\Mods"
    $targetLabel = "Win64 instance"
}

$buildArgs = @{ Config = $Config }
if ($Root) { $buildArgs.Root = $Root }
& (Join-Path $PSScriptRoot "build.ps1") @buildArgs | Tee-Object -Variable buildOut
$dll = ($buildOut | Select-String "^MOD_DLL=(.+)$").Matches[0].Groups[1].Value
if (-not $dll) { throw "build.ps1 did not report MOD_DLL" }

$modDir = Join-Path $modsRoot "PSAmity"
# A pre-rename install would load alongside this one and contend for the port.
$legacyModDir = Join-Path $modsRoot "PSPAmity"
if (Test-Path $legacyModDir) {
    $legacyIni = Join-Path $legacyModDir "PSPAmity.ini"
    if ((Test-Path $legacyIni) -and -not (Test-Path (Join-Path $modDir "PSAmity.ini"))) {
        New-Item -ItemType Directory -Force $modDir | Out-Null
        Copy-Item $legacyIni (Join-Path $modDir "PSAmity.ini")
    }
    Remove-Item -Recurse -Force $legacyModDir
    Write-Host "removed pre-rename install at $legacyModDir"
}
New-Item -ItemType Directory -Force (Join-Path $modDir "dlls") | Out-Null
Copy-Item $dll (Join-Path $modDir "dlls\main.dll") -Force
Set-Content -Path (Join-Path $modDir "enabled.txt") -Value "" -NoNewline -Encoding ascii
Write-Host "mod installed to $modDir ($targetLabel)"
