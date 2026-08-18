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

pub type lua_CFunction = extern "C" fn(*mut lua_State) -> c_int;
pub type lua_Hook = extern "C" fn(*mut lua_State, *mut c_void);
pub type lua_Alloc =
    extern "C" fn(ud: *mut c_void, ptr: *mut c_void, osize: usize, nsize: usize) -> *mut c_void;

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
