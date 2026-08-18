//! Symbols wasi-libc expects that we deliberately do not provide for real.
//!
//! Two groups:
//!  1. libc functions wasi-libc leaves undefined on this target. `clock` is
//!     genuinely referenced (Lua's `ltablib` uses it to randomise a sort pivot);
//!     it returns 0 so scripts cannot read the host clock.
//!  2. wasi-libc's own import declarations, defined locally so the linked module
//!     imports nothing at all. All fail with ENOSYS (52) EXCEPT the environ pair,
//!     which must succeed — wasi-libc calls `_Exit` if environment
//!     initialisation fails, which would abort the module on startup.
#![cfg(target_arch = "wasm32")]

use std::ffi::{c_char, c_int, c_void};

#[no_mangle]
pub extern "C" fn clock() -> i64 {
    0
}

#[no_mangle]
pub extern "C" fn system(_command: *const c_char) -> c_int {
    -1
}

#[no_mangle]
pub extern "C" fn tmpnam(_buf: *mut c_char) -> *mut c_char {
    std::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn tmpfile() -> *mut c_void {
    std::ptr::null_mut()
}

/// ENOSYS. Every real capability is refused.
const ENOSYS: i32 = 52;

macro_rules! wasi_stub {
    ($($name:ident($($arg:ident: $ty:ty),* $(,)?);)+) => {$(
        #[no_mangle]
        pub extern "C" fn $name($(_: $ty),*) -> i32 { ENOSYS }
    )+};
}

wasi_stub! {
    __imported_wasi_snapshot_preview1_clock_time_get(a: i32, b: i64, c: i32);
    __imported_wasi_snapshot_preview1_fd_close(a: i32);
    __imported_wasi_snapshot_preview1_fd_fdstat_get(a: i32, b: i32);
    __imported_wasi_snapshot_preview1_fd_fdstat_set_flags(a: i32, b: i32);
    __imported_wasi_snapshot_preview1_fd_prestat_get(a: i32, b: i32);
    __imported_wasi_snapshot_preview1_fd_prestat_dir_name(a: i32, b: i32, c: i32);
    __imported_wasi_snapshot_preview1_fd_read(a: i32, b: i32, c: i32, d: i32);
    __imported_wasi_snapshot_preview1_fd_renumber(a: i32, b: i32);
    __imported_wasi_snapshot_preview1_fd_seek(a: i32, b: i64, c: i32, d: i32);
    __imported_wasi_snapshot_preview1_fd_write(a: i32, b: i32, c: i32, d: i32);
    __imported_wasi_snapshot_preview1_path_open(
        a: i32, b: i32, c: i32, d: i32, e: i32, f: i64, g: i64, h: i32, i: i32,
    );
}

/// Must report success: wasi-libc calls `_Exit` when environ setup fails.
#[no_mangle]
pub extern "C" fn __imported_wasi_snapshot_preview1_environ_get(_a: i32, _b: i32) -> i32 {
    0
}

/// Must report success AND write zero counts, for the same reason.
#[no_mangle]
pub extern "C" fn __imported_wasi_snapshot_preview1_environ_sizes_get(
    count: i32,
    size: i32,
) -> i32 {
    unsafe {
        *(count as *mut u32) = 0;
        *(size as *mut u32) = 0;
    }
    0
}

#[no_mangle]
pub extern "C" fn __imported_wasi_snapshot_preview1_proc_exit(_code: i32) {
    std::process::abort()
}
