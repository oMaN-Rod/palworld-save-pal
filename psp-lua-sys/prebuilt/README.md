# Prebuilt wasm artifacts

`wasm32-unknown-unknown/liblua.a` is Lua 5.4.8 compiled by wasi-sdk 33 for wasm,
committed so that building the web target needs only a Rust toolchain. The
accompanying `libsetjmp.a`, `libc.a` and `libwasi-emulated-signal.a` are copied
verbatim from the wasi-sdk sysroot, because the Lua archive references them.

Regenerate after bumping Lua or wasi-sdk:

    WASI_SDK=/path/to/wasi-sdk-33.0 ./scripts/build-wasm-lua.sh   # POSIX
    $env:WASI_SDK="C:\wasi-sdk-33.0"; .\scripts\build-wasm-lua.ps1  # Windows

`build.rs` prefers a live `WASI_SDK` when set and falls back to these artifacts
otherwise, so contributors with the SDK always compile from source.

See `THIRD-PARTY-NOTICES.md` for the license and provenance of each archive.
