//! Native smoke tests. The wasm equivalents live in `tests/wasm.rs`.
#![cfg(not(target_arch = "wasm32"))]

use psp_lua_sys::ffi::*;
use std::ffi::{c_char, CString};

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

/// Loads and runs `src`, returning `(status, top_of_stack_as_string)`.
/// On failure the string is Lua's error message.
///
/// Opens the base library first. `pcall`, `error`, `tostring`, `type` and
/// `collectgarbage` all live in `lbaselib.c`, so without this every test in
/// this task fails with "attempt to call a nil value".
fn eval_string(src: &str) -> (i32, String) {
    unsafe {
        let state = luaL_newstate();
        assert!(!state.is_null(), "luaL_newstate returned NULL");
        luaL_requiref(state, LUA_GNAME.as_ptr() as *const c_char, luaopen_base, 1);
        lua_pop(state, 1);
        let chunk = CString::new(src).expect("test source contains a NUL byte");
        let mut status = luaL_loadstring(state, chunk.as_ptr());
        if status == LUA_OK {
            status = lua_pcall(state, 0, 1, 0);
        }
        let mut len: usize = 0;
        let ptr = lua_tolstring(state, -1, &mut len);
        let text = if ptr.is_null() {
            String::new()
        } else {
            String::from_utf8_lossy(std::slice::from_raw_parts(ptr as *const u8, len)).into_owned()
        };
        lua_close(state);
        (status, text)
    }
}

#[test]
fn reports_syntax_errors_without_crashing() {
    let (status, message) = eval_string("this is not lua");
    assert_eq!(status, LUA_ERRSYNTAX);
    assert!(message.contains("syntax error"), "unexpected message: {message}");
}

#[test]
fn propagates_runtime_errors_as_status_and_message() {
    let (status, message) = eval_string(r#"error("boom")"#);
    assert_eq!(status, LUA_ERRRUN);
    assert!(message.contains("boom"), "unexpected message: {message}");
}

/// This is the test that would fail if the build used a broken error mechanism:
/// `pcall` requires a working `longjmp` (or an equivalent) to unwind.
#[test]
fn pcall_recovers_from_a_script_error() {
    let (status, text) = eval_string(
        r#"local ok, err = pcall(function() error("inner") end)
           return tostring(ok) .. ":" .. tostring(err)"#,
    );
    assert_eq!(status, LUA_OK);
    assert!(text.starts_with("false:"), "unexpected result: {text}");
    assert!(text.contains("inner"), "unexpected result: {text}");
}

#[test]
fn pcall_recovers_repeatedly_within_one_state() {
    let (status, text) = eval_string(
        r#"pcall(function() error("first") end)
           local ok, err = pcall(function() error("second") end)
           return tostring(ok) .. ":" .. tostring(err)"#,
    );
    assert_eq!(status, LUA_OK);
    assert!(text.contains("second"), "unexpected result: {text}");
}

/// Deep recursion must produce a catchable Lua error, not a native stack
/// overflow. `f(n) = 1 + f(n+1)` is deliberately NOT a tail call.
///
/// The substring check happens here in Rust, not via Lua's `string.find`:
/// only the base library is open in this task, and `:find` needs the string
/// library (it installs the string metatable), which is out of scope here.
#[test]
fn stack_overflow_is_catchable_and_the_state_survives() {
    let (status, text) = eval_string(
        r#"local function f(n) return 1 + f(n + 1) end
           local ok, err = pcall(f, 1)
           if ok then return "unexpectedly succeeded" end
           return tostring(err)"#,
    );
    assert_eq!(status, LUA_OK);
    assert!(text.contains("stack overflow"), "unexpected result: {text}");
}

#[test]
fn errors_can_carry_non_string_values() {
    let (status, text) = eval_string(
        r#"local ok, err = pcall(function() error({ code = 7 }) end)
           return tostring(ok) .. ":" .. type(err) .. ":" .. tostring(err.code)"#,
    );
    assert_eq!(status, LUA_OK);
    assert_eq!(text, "false:table:7");
}
