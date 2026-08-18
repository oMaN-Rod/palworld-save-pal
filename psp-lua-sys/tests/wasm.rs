//! wasm32 tests. Run with:
//!   wasm-pack test --node psp-lua-sys
//!
//! These run under Node, not a browser, because this crate uses no browser
//! API. Browser validation of this build belongs where the JS glue actually
//! matters: `psp-web`'s browser test suite, once it links this crate.
#![cfg(target_arch = "wasm32")]

use psp_lua_sys::ffi::*;
use std::ffi::{c_char, CString};
use wasm_bindgen_test::*;

unsafe fn open_sandboxed_libraries(state: *mut lua_State) {
    let libs: [(&[u8], lua_CFunction); 6] = [
        (LUA_GNAME, luaopen_base),
        (LUA_COLIBNAME, luaopen_coroutine),
        (LUA_MATHLIBNAME, luaopen_math),
        (LUA_STRLIBNAME, luaopen_string),
        (LUA_TABLIBNAME, luaopen_table),
        (LUA_UTF8LIBNAME, luaopen_utf8),
    ];
    for (name, opener) in libs {
        luaL_requiref(state, name.as_ptr() as *const c_char, opener, 1);
        lua_pop(state, 1);
    }
    // `luaopen_base` brings these three with it. `dofile`/`loadfile` open files
    // through lauxlib, and `load` accepts binary chunks; a plugin needs none of
    // them, so they are removed rather than left reachable.
    for global in [b"dofile\0".as_slice(), b"loadfile\0".as_slice(), b"load\0".as_slice()] {
        lua_pushnil(state);
        lua_setglobal(state, global.as_ptr() as *const c_char);
    }
}

fn eval_sandboxed(src: &str) -> (i32, String) {
    unsafe {
        let state = luaL_newstate();
        assert!(!state.is_null(), "luaL_newstate returned NULL");
        open_sandboxed_libraries(state);
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

#[wasm_bindgen_test]
fn evaluates_arithmetic() {
    let (status, text) = eval_sandboxed("return tostring(1 + 1)");
    assert_eq!(status, LUA_OK);
    assert_eq!(text, "2");
}

/// The single most important wasm assertion: `longjmp`-based error recovery
/// works under wasi-sdk's SjLj emulation.
#[wasm_bindgen_test]
fn pcall_recovers() {
    let (status, text) = eval_sandboxed(
        r#"local ok, err = pcall(function() error("inner") end)
           return tostring(ok) .. ":" .. tostring(err)"#,
    );
    assert_eq!(status, LUA_OK);
    assert!(text.starts_with("false:"), "unexpected result: {text}");
    assert!(text.contains("inner"), "unexpected result: {text}");
}

#[wasm_bindgen_test]
fn stack_overflow_is_catchable() {
    let (status, text) = eval_sandboxed(
        r#"local function f(n) return 1 + f(n + 1) end
           local ok, err = pcall(f, 1)
           if ok then return "unexpectedly succeeded" end
           if not tostring(err):find("stack overflow") then return "wrong error" end
           return "recovered""#,
    );
    assert_eq!(status, LUA_OK);
    assert_eq!(text, "recovered");
}

#[wasm_bindgen_test]
fn excluded_libraries_are_absent() {
    let (status, text) = eval_sandboxed(
        r#"return table.concat({
             tostring(io), tostring(os), tostring(package), tostring(debug),
             tostring(require), tostring(dofile), tostring(loadfile), tostring(load),
           }, ",")"#,
    );
    assert_eq!(status, LUA_OK);
    assert_eq!(text, "nil,nil,nil,nil,nil,nil,nil,nil");
}

#[wasm_bindgen_test]
fn reports_syntax_errors_without_crashing() {
    let (status, text) = eval_sandboxed("this is not lua");
    assert_eq!(status, LUA_ERRSYNTAX);
    assert!(text.contains("syntax error"), "unexpected message: {text}");
}

#[wasm_bindgen_test]
fn errors_can_carry_non_string_values() {
    let (status, text) = eval_sandboxed(
        r#"local ok, err = pcall(function() error({ code = 7 }) end)
           return tostring(ok) .. ":" .. type(err) .. ":" .. tostring(err.code)"#,
    );
    assert_eq!(status, LUA_OK);
    assert_eq!(text, "false:table:7");
}

#[wasm_bindgen_test]
fn coroutines_run_and_surface_their_errors() {
    let (status, text) = eval_sandboxed(
        r#"local co = coroutine.create(function() error("in-co") end)
           local ok, err = coroutine.resume(co)
           return tostring(ok) .. ":" .. tostring(err)"#,
    );
    assert_eq!(status, LUA_OK);
    assert!(text.starts_with("false:"), "unexpected result: {text}");
    assert!(text.contains("in-co"), "unexpected result: {text}");
}

/// The linked wasm module carries two allocators: Rust's `dlmalloc` (via
/// `memory.grow`) and wasi-libc's `malloc`/`realloc`/`free` (via `sbrk` over
/// `memory.grow`), and Lua's `l_alloc` uses the latter. The other tests in
/// this file barely allocate, so memory growth on this target is otherwise
/// untested; 200k table allocations plus a full `collectgarbage()` exercises
/// it cheaply.
#[wasm_bindgen_test]
fn garbage_collection_runs_under_allocation_churn() {
    let (status, text) = eval_sandboxed(
        r#"for i = 1, 200000 do local _ = { i, tostring(i) } end
           collectgarbage()
           return "gc-ok""#,
    );
    assert_eq!(status, LUA_OK);
    assert_eq!(text, "gc-ok");
}
