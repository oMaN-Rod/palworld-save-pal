//! One-for-one declarations of the Lua C API surface this project uses.
//!
//! Several Lua "functions" are C macros; those are reproduced here as `#[inline]`
//! wrappers with the same name, documented individually.
#![allow(non_camel_case_types)]

use std::ffi::{c_char, c_int, c_void};

/// Opaque interpreter state. Never construct one directly.
#[repr(C)]
pub struct lua_State {
    _private: [u8; 0],
}

// Thread/call status codes, from lua.h.
pub const LUA_OK: c_int = 0;
pub const LUA_YIELD: c_int = 1;
pub const LUA_ERRRUN: c_int = 2;
pub const LUA_ERRSYNTAX: c_int = 3;
pub const LUA_ERRMEM: c_int = 4;
pub const LUA_ERRERR: c_int = 5;

pub const LUA_MULTRET: c_int = -1;

/// `LUA_REGISTRYINDEX` is `-LUAI_MAXSTACK - 1000`, with `LUAI_MAXSTACK` being
/// 1_000_000 on the 64-bit and wasm32 configurations we build.
pub const LUA_REGISTRYINDEX: c_int = -1_001_000;

/// Hook mask selecting the instruction-count hook, used by the sandbox in Plan 2.
pub const LUA_MASKCOUNT: c_int = 1 << 3;

// Lua value type tags, from lua.h.
pub const LUA_TNIL: c_int = 0;
pub const LUA_TBOOLEAN: c_int = 1;
pub const LUA_TNUMBER: c_int = 3;
pub const LUA_TSTRING: c_int = 4;
pub const LUA_TTABLE: c_int = 5;
pub const LUA_TFUNCTION: c_int = 6;

pub type lua_CFunction = unsafe extern "C" fn(*mut lua_State) -> c_int;
pub type lua_Hook = unsafe extern "C" fn(*mut lua_State, *mut c_void);
pub type lua_Alloc = unsafe extern "C" fn(
    ud: *mut c_void,
    ptr: *mut c_void,
    osize: usize,
    nsize: usize,
) -> *mut c_void;

extern "C" {
    pub fn luaL_newstate() -> *mut lua_State;
    pub fn lua_close(state: *mut lua_State);

    pub fn luaL_loadstring(state: *mut lua_State, source: *const c_char) -> c_int;
    pub fn lua_pcallk(
        state: *mut lua_State,
        nargs: c_int,
        nresults: c_int,
        errfunc: c_int,
        ctx: isize,
        k: *const c_void,
    ) -> c_int;

    pub fn lua_tolstring(state: *mut lua_State, index: c_int, len: *mut usize) -> *const c_char;
    pub fn lua_tointegerx(state: *mut lua_State, index: c_int, isnum: *mut c_int) -> i64;
    pub fn lua_toboolean(state: *mut lua_State, index: c_int) -> c_int;

    pub fn lua_settop(state: *mut lua_State, index: c_int);
    pub fn lua_gettop(state: *mut lua_State) -> c_int;
    pub fn lua_type(state: *mut lua_State, index: c_int) -> c_int;
}

/// `lua_pcall` is a macro in C; this is its expansion with no continuation.
///
/// # Safety
/// `state` must be a live state with a callable value and `nargs` arguments on
/// top of its stack.
#[inline]
pub unsafe fn lua_pcall(
    state: *mut lua_State,
    nargs: c_int,
    nresults: c_int,
    errfunc: c_int,
) -> c_int {
    lua_pcallk(state, nargs, nresults, errfunc, 0, std::ptr::null())
}

/// `lua_tostring` is a macro in C; this is its expansion.
///
/// # Safety
/// `state` must be live and `index` a valid stack index.
#[inline]
pub unsafe fn lua_tostring(state: *mut lua_State, index: c_int) -> *const c_char {
    lua_tolstring(state, index, std::ptr::null_mut())
}

/// `lua_pop` is a macro in C; this is its expansion.
///
/// # Safety
/// `state` must be live and hold at least `n` values.
#[inline]
pub unsafe fn lua_pop(state: *mut lua_State, n: c_int) {
    lua_settop(state, -n - 1);
}

/// The global table's name, as a NUL-terminated byte string so it can be passed
/// straight to the C API without allocating.
pub const LUA_GNAME: &[u8] = b"_G\0";

extern "C" {
    /// Opens a library and, when `glb` is non-zero, binds it to a global of the
    /// same name. Leaves the module on the stack, so callers `lua_pop(state, 1)`.
    pub fn luaL_requiref(
        state: *mut lua_State,
        modname: *const c_char,
        openf: lua_CFunction,
        glb: c_int,
    );

    pub fn luaopen_base(state: *mut lua_State) -> c_int;
}

// The remaining standard library names, as NUL-terminated byte strings so they
// can be passed straight to the C API without allocating. These mirror the
// `LUA_*LIBNAME` macros in lualib.h.
pub const LUA_COLIBNAME: &[u8] = b"coroutine\0";
pub const LUA_MATHLIBNAME: &[u8] = b"math\0";
pub const LUA_STRLIBNAME: &[u8] = b"string\0";
pub const LUA_TABLIBNAME: &[u8] = b"table\0";
pub const LUA_UTF8LIBNAME: &[u8] = b"utf8\0";

extern "C" {
    pub fn luaopen_coroutine(state: *mut lua_State) -> c_int;
    pub fn luaopen_math(state: *mut lua_State) -> c_int;
    pub fn luaopen_string(state: *mut lua_State) -> c_int;
    pub fn luaopen_table(state: *mut lua_State) -> c_int;
    pub fn luaopen_utf8(state: *mut lua_State) -> c_int;

    pub fn lua_pushnil(state: *mut lua_State);
    /// Pops the value on top of the stack and stores it in global `name`.
    pub fn lua_setglobal(state: *mut lua_State, name: *const c_char);
}
