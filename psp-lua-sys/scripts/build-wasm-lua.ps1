# Regenerates the committed wasm Lua archive. See build-wasm-lua.sh for notes.
# Usage: $env:WASI_SDK="C:\wasi-sdk-33.0"; .\scripts\build-wasm-lua.ps1
$ErrorActionPreference = "Stop"

if (-not $env:WASI_SDK) { throw "set WASI_SDK to a wasi-sdk 33+ installation" }

$here    = Split-Path -Parent $PSScriptRoot
$src     = Join-Path $here "vendor\lua-5.4.8\src"
$out     = Join-Path $here "prebuilt\wasm32-unknown-unknown"
$sysroot = Join-Path $env:WASI_SDK "share\wasi-sysroot"
$tmp     = Join-Path ([System.IO.Path]::GetTempPath()) ([System.Guid]::NewGuid().ToString())

# Must match LUA_SOURCES in build.rs exactly.
$units = @(
  "lapi","lauxlib","lbaselib","lcode","lcorolib","lctype","ldebug","ldo",
  "ldump","lfunc","lgc","llex","lmathlib","lmem","lobject","lopcodes",
  "lparser","lstate","lstring","lstrlib","ltable","ltablib","ltm",
  "lundump","lutf8lib","lvm","lzio"
)

New-Item -ItemType Directory -Force -Path $tmp, $out | Out-Null
try {
    foreach ($unit in $units) {
        & "$env:WASI_SDK\bin\clang.exe" `
            --target=wasm32-wasip1 `
            "--sysroot=$sysroot" `
            -mllvm -wasm-enable-sjlj `
            -O2 -D_WASI_EMULATED_SIGNAL `
            -c "$src\$unit.c" -o "$tmp\$unit.o"
        if ($LASTEXITCODE -ne 0) { throw "clang failed on $unit.c" }
    }

    $objects = Get-ChildItem "$tmp\*.o" | ForEach-Object { $_.FullName }
    # ar rcs inserts/replaces members, it never truncates; without removing the
    # archive first, a unit dropped from the unit list would leave its stale
    # .o linked in from a previous run.
    Remove-Item "$out\liblua.a" -Force -ErrorAction SilentlyContinue
    & "$env:WASI_SDK\bin\ar.exe" rcs "$out\liblua.a" @objects
    if ($LASTEXITCODE -ne 0) { throw "ar failed" }

    foreach ($lib in @("libsetjmp.a","libc.a","libwasi-emulated-signal.a")) {
        Copy-Item "$sysroot\lib\wasm32-wasip1\$lib" "$out\$lib" -Force
    }
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

Write-Output "wrote $out"
Get-ChildItem $out
