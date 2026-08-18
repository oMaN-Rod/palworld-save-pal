//! Native smoke tests. The wasm equivalents live in `tests/wasm.rs`.
#![cfg(not(target_arch = "wasm32"))]

use psp_lua_sys::ffi::*;
use std::ffi::{c_char, CString};

/// Creates a state, opens libraries via `open_libs`, then loads and pcalls
/// `src`. Returns the live state (still open, for the caller to read a result
/// from and then close) and the pcall status.
///
/// # Safety
/// `open_libs` must leave the interpreter stack balanced.
unsafe fn run(src: &str, open_libs: impl FnOnce(*mut lua_State)) -> (*mut lua_State, i32) {
    let state = luaL_newstate();
    assert!(!state.is_null(), "luaL_newstate returned NULL");
    open_libs(state);
    let chunk = CString::new(src).expect("test source contains a NUL byte");
    let mut status = luaL_loadstring(state, chunk.as_ptr());
    if status == LUA_OK {
        status = lua_pcall(state, 0, 1, 0);
    }
    (state, status)
}

/// Reads the value on top of the stack as a string, or an empty string if it
/// isn't one.
///
/// # Safety
/// `state` must be live and hold at least one value.
unsafe fn read_string_result(state: *mut lua_State) -> String {
    let mut len: usize = 0;
    let ptr = lua_tolstring(state, -1, &mut len);
    if ptr.is_null() {
        String::new()
    } else {
        String::from_utf8_lossy(std::slice::from_raw_parts(ptr as *const u8, len)).into_owned()
    }
}

/// Loads and runs `src`, returning its single integer result, or the Lua
/// status code on failure. Test-only helper: panicking here is fine.
fn eval_int(src: &str) -> Result<i64, i32> {
    unsafe {
        let (state, status) = run(src, |_state| {});
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
/// `collectgarbage` are registered by `luaopen_base`, not built into the VM,
/// so calling them requires opening it.
fn eval_string(src: &str) -> (i32, String) {
    unsafe {
        let (state, status) = run(src, |state| {
            luaL_requiref(state, LUA_GNAME.as_ptr() as *const c_char, luaopen_base, 1);
            lua_pop(state, 1);
        });
        let text = read_string_result(state);
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
/// this helper opens only the base library, and `:find` needs the string
/// library, which installs the string metatable.
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

/// Opens exactly the libraries a plugin is allowed to see. This mirrors what
/// `psp-plugin`'s sandbox will do; it lives here so the crate can prove the
/// excluded libraries are absent from the build itself.
///
/// # Safety
/// `state` must be a freshly created state.
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
        let (state, status) = run(src, |state| open_sandboxed_libraries(state));
        let text = read_string_result(state);
        lua_close(state);
        (status, text)
    }
}

#[test]
fn opens_the_permitted_libraries() {
    let (status, text) = eval_sandboxed(
        r#"return table.concat({
             type(string.format), type(table.concat), type(math.floor),
             type(coroutine.create), type(utf8.char), type(tostring),
           }, ",")"#,
    );
    assert_eq!(status, LUA_OK);
    assert_eq!(text, "function,function,function,function,function,function");
}

#[test]
fn excluded_libraries_are_absent() {
    // io/os/package/debug are absent because their translation units are not
    // compiled at all; require comes from package. dofile/loadfile/load ARE
    // registered by luaopen_base and are explicitly removed by
    // open_sandboxed_libraries — this asserts that removal, not their absence.
    let (status, text) = eval_sandboxed(
        r#"return table.concat({
             tostring(io), tostring(os), tostring(package), tostring(debug),
             tostring(require), tostring(dofile), tostring(loadfile), tostring(load),
           }, ",")"#,
    );
    assert_eq!(status, LUA_OK);
    assert_eq!(text, "nil,nil,nil,nil,nil,nil,nil,nil");
}

#[test]
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

#[test]
fn garbage_collection_runs_under_allocation_churn() {
    let (status, text) = eval_sandboxed(
        r#"for i = 1, 200000 do local _ = { i, tostring(i) } end
           collectgarbage()
           return "gc-ok""#,
    );
    assert_eq!(status, LUA_OK);
    assert_eq!(text, "gc-ok");
}
