#!/usr/bin/env bash
# Regenerates the committed wasm Lua archive. Only needed when bumping Lua or
# wasi-sdk; a normal build links the committed artifacts.
#
# Usage: WASI_SDK=/path/to/wasi-sdk-33.0 ./scripts/build-wasm-lua.sh
set -euo pipefail

: "${WASI_SDK:?set WASI_SDK to a wasi-sdk 33+ installation}"

here="$(cd "$(dirname "$0")/.." && pwd)"
src="$here/vendor/lua-5.4.8/src"
out="$here/prebuilt/wasm32-unknown-unknown"
sysroot="$WASI_SDK/share/wasi-sysroot"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# Must match LUA_SOURCES in build.rs exactly.
units="lapi lauxlib lbaselib lcode lcorolib lctype ldebug ldo ldump lfunc lgc \
llex lmathlib lmem lobject lopcodes lparser lstate lstring lstrlib ltable \
ltablib ltm lundump lutf8lib lvm lzio"

mkdir -p "$out"
for unit in $units; do
  "$WASI_SDK/bin/clang" \
    --target=wasm32-wasip1 \
    --sysroot="$sysroot" \
    -mllvm -wasm-enable-sjlj \
    -O2 -D_WASI_EMULATED_SIGNAL \
    -c "$src/$unit.c" -o "$tmp/$unit.o"
done

"$WASI_SDK/bin/ar" rcs "$out/liblua.a" "$tmp"/*.o

# The static wasi-libc pieces the archive needs at link time, vendored so a
# normal checkout links without the SDK.
for lib in libsetjmp.a libc.a libwasi-emulated-signal.a; do
  cp "$sysroot/lib/wasm32-wasip1/$lib" "$out/$lib"
done

echo "wrote $out"
ls -la "$out"
