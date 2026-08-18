# Third-party notices

This directory commits prebuilt static archives for `wasm32-unknown-unknown`
so that linking Lua for the web target does not require a wasi-sdk install.
All were produced by, or copied from, **wasi-sdk 33.0**.

## `liblua.a`

- **Upstream project:** Lua (https://www.lua.org)
- **Version:** 5.4.8
- **License:** MIT
- **Provenance:** built from the vendored sources at
  `psp-lua-sys/vendor/lua-5.4.8/src` by `scripts/build-wasm-lua.ps1` /
  `build-wasm-lua.sh`, using wasi-sdk 33.0's `clang`.
- **Full license text:** `psp-lua-sys/vendor/LUA-LICENSE.txt`

## `libc.a`, `libsetjmp.a`, `libwasi-emulated-signal.a`

- **Upstream project:** wasi-libc (https://github.com/WebAssembly/wasi-libc)
- **Version:** as shipped in wasi-sdk 33.0's sysroot
  (`share/wasi-sysroot/lib/wasm32-wasip1/`)
- **License:** Apache-2.0 WITH LLVM-exception, OR MIT (dual-licensed)
- **Provenance:** copied verbatim, unmodified, from the wasi-sdk 33.0 tarball.
  The tarball ships no LICENSE file alongside these archives; the full license
  text is available from the wasi-libc repository linked above.
