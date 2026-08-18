//! Symbols wasi-libc expects that we deliberately do not provide for real.
//!
//! Two groups:
//!  1. libc functions wasi-libc leaves undefined on this target. Of these,
//!     only `clock` is actually referenced by the compiled sources today —
//!     `ltablib.c` calls it to randomise a sort pivot, and it returns 0 here
//!     so scripts cannot read the host clock. `system`, `tmpnam` and
//!     `tmpfile` are not called by anything in this build (they belong to
//!     `loslib.c`/`liolib.c`, which are excluded); they are kept as
//!     insurance because wasi-sdk's `libc.a` does not define them either, so
//!     anything that started referencing them would otherwise fail to link.
//!  2. wasi-libc's own import declarations, defined locally so the linked module
//!     imports nothing at all. Most fail with ENOSYS (52); three are special:
//!     - the environ pair must succeed — wasi-libc calls `_Exit` if environment
//!       initialisation fails, which would abort the module on startup.
//!     - `fd_prestat_get` must return EBADF (8), not ENOSYS. wasi-libc's
//!       preopen scan calls it on ascending file descriptors and stops the
//!       scan only on EBADF; any other nonzero errno is treated as a real
//!       failure and also calls `_Exit`. EBADF is what a real WASI runtime
//!       returns once the descriptor table is exhausted, so it reads to
//!       wasi-libc as "no more preopens," which is true here.

use std::ffi::{c_char, c_int, c_void};

/// Referenced today, by `ltablib.c`'s sort-pivot randomisation.
#[no_mangle]
pub extern "C" fn clock() -> i64 {
    0
}

/// Insurance: not called by the compiled sources, kept because wasi-sdk's
/// `libc.a` does not define it.
#[no_mangle]
pub extern "C" fn system(_command: *const c_char) -> c_int {
    -1
}

/// Insurance: not called by the compiled sources, kept because wasi-sdk's
/// `libc.a` does not define it.
#[no_mangle]
pub extern "C" fn tmpnam(_buf: *mut c_char) -> *mut c_char {
    std::ptr::null_mut()
}

/// Insurance: not called by the compiled sources, kept because wasi-sdk's
/// `libc.a` does not define it.
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
    __imported_wasi_snapshot_preview1_fd_prestat_dir_name(a: i32, b: i32, c: i32);
    __imported_wasi_snapshot_preview1_fd_read(a: i32, b: i32, c: i32, d: i32);
    __imported_wasi_snapshot_preview1_fd_renumber(a: i32, b: i32);
    __imported_wasi_snapshot_preview1_fd_seek(a: i32, b: i64, c: i32, d: i32);
    __imported_wasi_snapshot_preview1_fd_write(a: i32, b: i32, c: i32, d: i32);
    __imported_wasi_snapshot_preview1_path_open(
        a: i32, b: i32, c: i32, d: i32, e: i32, f: i64, g: i64, h: i32, i: i32,
    );
}

/// EBADF (8), not ENOSYS. wasi-libc's preopen scan probes file descriptors
/// starting at 3 and stops the loop only on EBADF; any other nonzero errno
/// is treated as a real failure and triggers `_Exit`. Returning EBADF tells
/// it there are no preopened directories, which is accurate here.
#[no_mangle]
pub extern "C" fn __imported_wasi_snapshot_preview1_fd_prestat_get(_a: i32, _b: i32) -> i32 {
    8
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
