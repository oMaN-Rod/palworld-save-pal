#!/usr/bin/env bash
# Built via the emscripten/emsdk docker image since emcc is not installed on the host.
#
# Source list mirrors ooz-rs/build.rs (the authoritative compile list for the
# libooz decompress+compress library). OOZ_BUILD_DLL=1 excludes the CLI main()
# in kraken.cpp. simde is a git submodule of the ooz repo.
set -euo pipefail
cd "$(dirname "$0")/.."

SRC_DIR="vendor/ooz-src" # gitignored clone target
if [ ! -d "$SRC_DIR" ]; then
  git clone --depth 1 https://github.com/oMaN-Rod/ooz "$SRC_DIR"
fi
if [ ! -f "$SRC_DIR/simde/simde/x86/sse2.h" ]; then
  git -C "$SRC_DIR" submodule update --init --depth 1 simde
fi
mkdir -p psp-ui/vendor/ooz

# Docker Desktop on Windows needs a Windows-style host path, and MSYS path
# mangling must be disabled so container paths like /work survive intact.
HOST_DIR="$(pwd -W 2>/dev/null || pwd)"

# INITIAL_MEMORY stays small for a browser/worker target (mobile-friendly);
# ALLOW_MEMORY_GROWTH lets the heap grow up to MAXIMUM_MEMORY as needed.
MSYS_NO_PATHCONV=1 docker run --rm -v "$HOST_DIR:/work" -w /work emscripten/emsdk em++ \
  -O2 -std=c++17 -DOOZ_BUILD_DLL=1 -include scripts/ooz-wasm-shim.h \
  vendor/ooz-src/bitknit.cpp \
  vendor/ooz-src/kraken.cpp \
  vendor/ooz-src/lzna.cpp \
  vendor/ooz-src/compress.cpp \
  vendor/ooz-src/compr_kraken.cpp \
  vendor/ooz-src/compr_leviathan.cpp \
  vendor/ooz-src/compr_mermaid.cpp \
  vendor/ooz-src/compr_entropy.cpp \
  vendor/ooz-src/compr_match_finder.cpp \
  vendor/ooz-src/compr_multiarray.cpp \
  vendor/ooz-src/compr_tans.cpp \
  -I vendor/ooz-src -I vendor/ooz-src/simde \
  -s EXPORTED_FUNCTIONS='["_Ooz_Decompress","_Ooz_Compress","_malloc","_free"]' \
  -s EXPORTED_RUNTIME_METHODS='["HEAPU8","ccall"]' \
  -s INITIAL_MEMORY=64MB -s ALLOW_MEMORY_GROWTH=1 -s MAXIMUM_MEMORY=4GB \
  -s MODULARIZE=1 -s EXPORT_ES6=1 -s ENVIRONMENT=web,worker,node \
  -o psp-ui/vendor/ooz/ooz.mjs

ls -la psp-ui/vendor/ooz
