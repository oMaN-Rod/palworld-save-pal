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

// Remaining Lua value type tags, from lua.h.
pub const LUA_TNONE: c_int = -1;
pub const LUA_TLIGHTUSERDATA: c_int = 2;
pub const LUA_TUSERDATA: c_int = 7;
pub const LUA_TTHREAD: c_int = 8;

// Remaining hook masks, from lua.h. Only LUA_MASKCOUNT is used today; the rest
// are declared so a mask is never assembled from a magic number.
pub const LUA_MASKCALL: c_int = 1 << 0;
pub const LUA_MASKRET: c_int = 1 << 1;
pub const LUA_MASKLINE: c_int = 1 << 2;

// Registry sentinels, from lauxlib.h and lua.h.
pub const LUA_NOREF: c_int = -2;
pub const LUA_REFNIL: c_int = -1;
pub const LUA_RIDX_GLOBALS: i64 = 2;

extern "C" {
    pub fn lua_newstate(f: lua_Alloc, ud: *mut c_void) -> *mut lua_State;
    pub fn lua_atpanic(state: *mut lua_State, panicf: lua_CFunction) -> lua_CFunction;
    pub fn lua_sethook(state: *mut lua_State, func: Option<lua_Hook>, mask: c_int, count: c_int);
    /// Raises the value on top of the stack as an error. Never returns.
    pub fn lua_error(state: *mut lua_State) -> c_int;

    pub fn lua_absindex(state: *mut lua_State, index: c_int) -> c_int;
    pub fn lua_checkstack(state: *mut lua_State, n: c_int) -> c_int;
    pub fn lua_pushvalue(state: *mut lua_State, index: c_int);

    pub fn lua_pushinteger(state: *mut lua_State, n: i64);
    pub fn lua_pushnumber(state: *mut lua_State, n: f64);
    pub fn lua_pushlstring(state: *mut lua_State, s: *const c_char, len: usize) -> *const c_char;
    pub fn lua_pushstring(state: *mut lua_State, s: *const c_char) -> *const c_char;
    pub fn lua_pushboolean(state: *mut lua_State, b: c_int);
    pub fn lua_pushcclosure(state: *mut lua_State, f: lua_CFunction, n: c_int);
    pub fn lua_pushlightuserdata(state: *mut lua_State, p: *mut c_void);

    pub fn lua_tonumberx(state: *mut lua_State, index: c_int, isnum: *mut c_int) -> f64;
    pub fn lua_rawlen(state: *mut lua_State, index: c_int) -> u64;
    pub fn lua_isinteger(state: *mut lua_State, index: c_int) -> c_int;
    pub fn lua_isnumber(state: *mut lua_State, index: c_int) -> c_int;
    pub fn lua_isstring(state: *mut lua_State, index: c_int) -> c_int;
    pub fn lua_typename(state: *mut lua_State, tp: c_int) -> *const c_char;

    pub fn lua_createtable(state: *mut lua_State, narr: c_int, nrec: c_int);
    pub fn lua_settable(state: *mut lua_State, index: c_int);
    pub fn lua_gettable(state: *mut lua_State, index: c_int) -> c_int;
    pub fn lua_setfield(state: *mut lua_State, index: c_int, k: *const c_char);
    pub fn lua_getfield(state: *mut lua_State, index: c_int, k: *const c_char) -> c_int;
    pub fn lua_getglobal(state: *mut lua_State, name: *const c_char) -> c_int;
    pub fn lua_rawset(state: *mut lua_State, index: c_int);
    pub fn lua_rawget(state: *mut lua_State, index: c_int) -> c_int;
    pub fn lua_rawseti(state: *mut lua_State, index: c_int, n: i64);
    pub fn lua_rawgeti(state: *mut lua_State, index: c_int, n: i64) -> c_int;
    pub fn lua_next(state: *mut lua_State, index: c_int) -> c_int;

    pub fn lua_rawgetp(state: *mut lua_State, index: c_int, p: *const c_void) -> c_int;
    pub fn lua_rawsetp(state: *mut lua_State, index: c_int, p: *const c_void);

    pub fn lua_newuserdatauv(state: *mut lua_State, sz: usize, nuvalue: c_int) -> *mut c_void;
    pub fn lua_touserdata(state: *mut lua_State, index: c_int) -> *mut c_void;
    pub fn lua_setmetatable(state: *mut lua_State, objindex: c_int) -> c_int;
    pub fn lua_getmetatable(state: *mut lua_State, objindex: c_int) -> c_int;

    pub fn luaL_newmetatable(state: *mut lua_State, tname: *const c_char) -> c_int;
    pub fn luaL_setmetatable(state: *mut lua_State, tname: *const c_char);
    pub fn luaL_testudata(state: *mut lua_State, ud: c_int, tname: *const c_char) -> *mut c_void;
    pub fn luaL_ref(state: *mut lua_State, t: c_int) -> c_int;
    pub fn luaL_unref(state: *mut lua_State, t: c_int, r: c_int);
    pub fn luaL_tolstring(state: *mut lua_State, index: c_int, len: *mut usize) -> *const c_char;

    /// `mode` selects which chunk forms are accepted: `b` binary, `t` text,
    /// `bt` both. The sandbox always passes `t`, so precompiled bytecode --
    /// which is not verified and can escape the sandbox -- is refused.
    pub fn luaL_loadbufferx(
        state: *mut lua_State,
        buff: *const c_char,
        sz: usize,
        name: *const c_char,
        mode: *const c_char,
    ) -> c_int;
}

/// `lua_upvalueindex` is a macro in C; this is its expansion.
#[inline]
pub const fn lua_upvalueindex(i: c_int) -> c_int {
    LUA_REGISTRYINDEX - i
}

/// `LUA_EXTRASPACE` from luaconf.h: one pointer of scratch memory Lua reserves
/// immediately before every `lua_State`, for the host to use as it likes.
pub const LUA_EXTRASPACE: usize = std::mem::size_of::<*mut c_void>();

/// `lua_getextraspace` is a macro in C; this is its expansion.
///
/// The sandbox stores one pointer here so the count hook — which receives only
/// the state — can reach its interrupt record without touching the registry.
///
/// # Safety
/// `state` must be a live state produced by `lua_newstate`/`luaL_newstate`.
#[inline]
pub unsafe fn lua_getextraspace(state: *mut lua_State) -> *mut c_void {
    (state as *mut u8).sub(LUA_EXTRASPACE) as *mut c_void
}

/// `lua_newtable` is a macro in C; this is its expansion.
///
/// # Safety
/// `state` must be live with room for one more stack slot.
#[inline]
pub unsafe fn lua_newtable(state: *mut lua_State) {
    lua_createtable(state, 0, 0);
}

/// `lua_pushcfunction` is a macro in C; this is its expansion.
///
/// # Safety
/// `state` must be live with room for one more stack slot.
#[inline]
pub unsafe fn lua_pushcfunction(state: *mut lua_State, f: lua_CFunction) {
    lua_pushcclosure(state, f, 0);
}

/// `lua_tonumber` is a macro in C; this is its expansion.
///
/// # Safety
/// `state` must be live and `index` a valid stack index.
#[inline]
pub unsafe fn lua_tonumber(state: *mut lua_State, index: c_int) -> f64 {
    lua_tonumberx(state, index, std::ptr::null_mut())
}

/// `lua_tointeger` is a macro in C; this is its expansion.
///
/// # Safety
/// `state` must be live and `index` a valid stack index.
#[inline]
pub unsafe fn lua_tointeger(state: *mut lua_State, index: c_int) -> i64 {
    lua_tointegerx(state, index, std::ptr::null_mut())
}

/// `lua_isnil` is a macro in C; this is its expansion.
///
/// # Safety
/// `state` must be live and `index` a valid stack index.
#[inline]
pub unsafe fn lua_isnil(state: *mut lua_State, index: c_int) -> bool {
    lua_type(state, index) == LUA_TNIL
}

/// `lua_istable` is a macro in C; this is its expansion.
///
/// # Safety
/// `state` must be live and `index` a valid stack index.
#[inline]
pub unsafe fn lua_istable(state: *mut lua_State, index: c_int) -> bool {
    lua_type(state, index) == LUA_TTABLE
}

/// `lua_isfunction` is a macro in C; this is its expansion.
///
/// # Safety
/// `state` must be live and `index` a valid stack index.
#[inline]
pub unsafe fn lua_isfunction(state: *mut lua_State, index: c_int) -> bool {
    lua_type(state, index) == LUA_TFUNCTION
}
