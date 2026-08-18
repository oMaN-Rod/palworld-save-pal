//! Raw FFI bindings to a vendored Lua 5.4.8.
//!
//! This crate deliberately exposes no safe abstractions: `psp-plugin` owns the
//! safe layer, the capability model and the sandbox. Everything here is `unsafe`
//! to call and mirrors the C API one-for-one.
//!
//! The build never links a C++ runtime. On `wasm32-unknown-unknown`, Lua's
//! `setjmp`/`longjmp` error mechanism is provided by wasi-sdk's `libsetjmp`
//! rather than by C++ exceptions, which avoids linking a C++ runtime.

pub mod ffi;

#[cfg(target_arch = "wasm32")]
mod wasm_stubs;
