param(
    [string]$Root = "",
    [string]$Config = "Game__Shipping__Win64",
    [string[]]$Targets = @("PSAmity")
)
$ErrorActionPreference = "Stop"

if (-not $Root) {
    $repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
    $Root = Join-Path (Split-Path -Parent $repoRoot) "amity-build"
}
if (-not (Test-Path (Join-Path $Root "RE-UE4SS\CMakeLists.txt"))) {
    throw "UE4SS workspace not found at $Root. Run scripts\setup-workspace.ps1 -Root `"$Root`" once first."
}
$Targets = @($Targets | ForEach-Object { $_ -split "," } | Where-Object { $_ })

function Find-CMake {
    $onPath = Get-Command cmake -ErrorAction SilentlyContinue
    if ($onPath) { return $onPath.Source }
    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path $vswhere) {
        $install = & $vswhere -latest -products * -property installationPath 2>$null | Select-Object -First 1
        if ($install) {
            $bundled = Join-Path $install "Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe"
            if (Test-Path $bundled) { return $bundled }
        }
    }
    throw "cmake not found on PATH or in a Visual Studio install (add the 'C++ CMake tools for Windows' component, or winget install Kitware.CMake)."
}
$cmake = Find-CMake

# CMake names its Visual Studio generators "Visual Studio <major> <year>", and
# vswhere reports both halves, so the installed toolchain picks its own generator.
# Pinning one breaks whenever a runner image rolls forward: `windows-latest` moved
# from VS 2022 to VS 2026 and a hardcoded "Visual Studio 17 2022" stopped resolving.
function Find-Generator {
    if ($env:AMITY_CMAKE_GENERATOR) { return $env:AMITY_CMAKE_GENERATOR }
    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path $vswhere)) { return $null }
    $version = & $vswhere -latest -products * -property installationVersion 2>$null | Select-Object -First 1
    $year = & $vswhere -latest -products * -property catalog_productLineVersion 2>$null | Select-Object -First 1
    if (-not $version -or -not $year) { return $null }
    $generator = "Visual Studio $($version.Split('.')[0]) $year"
    # A cmake too old to know this Visual Studio would fail the same way a stale
    # pin does, so only pass a generator cmake actually advertises.
    $known = & $cmake --help 2>$null | Select-String -SimpleMatch $generator
    if (-not $known) { return $null }
    return $generator
}

Push-Location $Root
try {
    if (-not (Test-Path (Join-Path $Root "build\CMakeCache.txt"))) {
        $generator = Find-Generator
        if ($generator) {
            Write-Host "configuring with generator: $generator"
            & $cmake -B build -G $generator
        } else {
            # No vswhere: let CMake fall back to its own newest-VS default.
            Write-Host "configuring with CMake's default generator"
            & $cmake -B build
        }
        if ($LASTEXITCODE -ne 0) { throw "configure failed" }
    }
    & $cmake --build build --config $Config --target @Targets
    if ($LASTEXITCODE -ne 0) { throw "build failed" }
    $dll = Get-ChildItem -Recurse -Filter "PSAmity.dll" (Join-Path $Root "build") |
        Where-Object { $_.FullName -like "*$Config*" } | Select-Object -First 1
    if (-not $dll) { throw "PSAmity.dll not found under $Root\build for config $Config" }
    Write-Output "MOD_DLL=$($dll.FullName)"
} finally { Pop-Location }
