//! Native smoke tests. The wasm equivalents live in `tests/wasm.rs`.
#![cfg(not(target_arch = "wasm32"))]

use psp_lua_sys::ffi::*;
use std::ffi::CString;

/// Loads and runs `src`, returning its single integer result, or the Lua
/// status code on failure. Test-only helper: panicking here is fine.
fn eval_int(src: &str) -> Result<i64, i32> {
    unsafe {
        let state = luaL_newstate();
        assert!(!state.is_null(), "luaL_newstate returned NULL");
        let chunk = CString::new(src).expect("test source contains a NUL byte");
        let mut status = luaL_loadstring(state, chunk.as_ptr());
        if status == LUA_OK {
            status = lua_pcall(state, 0, 1, 0);
        }
        let result = if status == LUA_OK {
            Ok(lua_tointegerx(state, -1, std::ptr::null_mut()))
        } else {
            Err(status)
        };
        lua_close(state);
        result
    }
}

#[test]
fn evaluates_arithmetic() {
    assert_eq!(eval_int("return 1 + 1"), Ok(2));
}

#[test]
fn evaluates_integer_expressions_using_the_vm() {
    assert_eq!(eval_int("local t = 0 for i = 1, 100 do t = t + i end return t"), Ok(5050));
}
