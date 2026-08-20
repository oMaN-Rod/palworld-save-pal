pub mod api_def;
pub mod api_meta;
pub mod gamedata;
pub mod handle;
pub mod marshal;
pub mod raw;
pub mod save_read;
pub mod save_write;
pub mod services;

use std::ffi::{c_char, c_int, c_void, CString};

use psp_lua_sys::ffi::*;

use crate::context::RunContext;
use crate::manifest::Capability;

pub const MAX_TABLE_DEPTH: usize = 32;
pub const MAX_TABLE_NODES: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostError(String);

impl HostError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }

    pub fn into_message(self) -> String {
        self.0
    }
}

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// `Ok(n)` means `n` values were pushed. The implementation must never call
/// `lua_error`, `luaL_error` or any `luaL_check*` — the C trampoline raises.
pub type HostFn = fn(*mut lua_State) -> Result<c_int, HostError>;

/// Never raises: `lua_error`'s `longjmp` runs a real SEH unwind on
/// `x86_64-pc-windows-msvc`, and any Rust frame that has touched the allocator
/// carries cleanup metadata that unwind invokes, corrupting the heap. The error
/// message goes back to `psp_host_trampoline`, which raises from its own C frame.
pub unsafe extern "C" fn trampoline(
    state: *mut lua_State,
    host_fn: *mut c_void,
    message: *mut *const c_char,
    len: *mut usize,
    owner: *mut *mut c_void,
) -> c_int {
    let body: HostFn = std::mem::transmute::<*mut c_void, HostFn>(host_fn);
    match body(state) {
        // A negative count would read to the shim as "raise", with no message set.
        Ok(pushed) => pushed.max(0),
        Err(error) => {
            let boxed = Box::new(error.into_message());
            let len_value = boxed.len();
            // An empty String's pointer is non-null but dangling; the shim
            // treats NULL with len 0 as "push an empty string".
            *message = if len_value == 0 {
                std::ptr::null()
            } else {
                boxed.as_ptr().cast::<c_char>()
            };
            *len = len_value;
            *owner = Box::into_raw(boxed).cast::<c_void>();
            -1
        }
    }
}

pub unsafe extern "C" fn free_message(owner: *mut c_void) {
    drop(Box::from_raw(owner.cast::<String>()));
}

/// The only shape a host function is registered in: binding a Rust `extern "C"`
/// fn directly would put a Rust frame between `lua_error` and its `setjmp`.
#[macro_export]
macro_rules! host_fn {
    ($name:ident, $body:path) => {
        #[allow(non_upper_case_globals)]
        pub unsafe fn $name(state: *mut ::psp_lua_sys::ffi::lua_State) {
            let dispatch = ($crate::host::trampoline as *const () as usize) as *mut ::std::ffi::c_void;
            let body = (($body as $crate::host::HostFn) as usize) as *mut ::std::ffi::c_void;
            let free = ($crate::host::free_message as *const () as usize) as *mut ::std::ffi::c_void;
            ::psp_lua_sys::ffi::lua_pushlightuserdata(state, dispatch);
            ::psp_lua_sys::ffi::lua_pushlightuserdata(state, body);
            ::psp_lua_sys::ffi::lua_pushlightuserdata(state, free);
            ::psp_lua_sys::ffi::lua_pushcclosure(
                state,
                ::psp_lua_sys::ffi::psp_host_trampoline,
                3,
            );
        }
    };
}
pub use host_fn;

pub type PushHostFn = unsafe fn(*mut lua_State);

/// Its address is the key; the byte's value is never read.
static CONTEXT_KEY: u8 = 0;

pub unsafe fn set_context(state: *mut lua_State, ptr: *mut c_void) {
    lua_pushlightuserdata(state, ptr);
    lua_rawsetp(state, LUA_REGISTRYINDEX, &CONTEXT_KEY as *const u8 as *const c_void);
}

pub unsafe fn context(state: *mut lua_State) -> *mut c_void {
    lua_rawgetp(state, LUA_REGISTRYINDEX, &CONTEXT_KEY as *const u8 as *const c_void);
    let ptr = lua_touserdata(state, -1);
    lua_pop(state, 1);
    ptr
}

pub unsafe fn register_table(
    state: *mut lua_State,
    global: &str,
    functions: &[(&str, PushHostFn)],
) {
    lua_createtable(state, 0, functions.len() as c_int);
    for (name, push) in functions {
        push(state);
        let Ok(key) = CString::new(*name) else {
            lua_pop(state, 1);
            continue;
        };
        lua_setfield(state, -2, key.as_ptr());
    }
    let Ok(global) = CString::new(global) else {
        lua_pop(state, 1);
        return;
    };
    lua_setglobal(state, global.as_ptr());
}

/// `f` must not call into Lua: a script callback could re-enter another host
/// function and take a second `&mut` to the same `RunContext`, invalidating the
/// first. Acquire, record what you need, release, then call Lua.
pub unsafe fn with_context<R>(
    state: *mut lua_State,
    f: impl FnOnce(&mut RunContext<'_>) -> Result<R, HostError>,
) -> Result<R, HostError> {
    let ptr = context(state) as *mut RunContext<'_>;
    if ptr.is_null() {
        return Err(HostError::new("plugin run context is unavailable"));
    }
    f(&mut *ptr)
}

unsafe extern "C" fn install_globals_body(state: *mut lua_State) -> c_int {
    let ptr = context(state) as *const RunContext<'_>;
    if ptr.is_null() {
        return 0;
    }
    let ctx = &*ptr;
    handle::install_metatables(state);
    if ctx.grants(Capability::SaveRaw) {
        raw::install(state);
    }
    if ctx.grants(Capability::SaveRead) {
        save_read::install(state);
    }
    if ctx.grants(Capability::GameData) {
        gamedata::install(state);
    }
    if ctx.grants(Capability::SaveWrite) {
        save_write::install(state);
    }
    services::install_progress(state);
    services::install_ctx(state, ctx);
    if ctx.grants(Capability::Log) {
        services::install_log(state);
    }
    if ctx.grants(Capability::Storage) {
        services::install_storage(state);
    }
    if ctx.grants(Capability::UiDialog) {
        services::install_ui(state);
    }
    for capability in &ctx.granted {
        assert_capability_is_handled(*capability);
    }
    0
}

/// Exhaustiveness guard: a new `Capability` must not compile until something
/// installs it.
fn assert_capability_is_handled(capability: Capability) {
    match capability {
        Capability::SaveRead
        | Capability::SaveWrite
        | Capability::SaveRaw
        | Capability::Players
        | Capability::GameData
        | Capability::Log
        | Capability::Storage
        | Capability::UiDialog => {}
    }
}

/// Runs under its own `lua_pcall`: `register_table` allocates, and an
/// unprotected `LUA_ERRMEM` here reaches `lua_atpanic` and then `abort()`.
pub unsafe fn install_globals(state: *mut lua_State) -> Result<(), HostError> {
    lua_pushcfunction(state, install_globals_body);
    if lua_pcall(state, 0, 0, 0) != LUA_OK {
        let message = crate::sandbox::read_string(state, -1);
        lua_pop(state, 1);
        return Err(HostError::new(
            message.unwrap_or_else(|| "failed to install plugin globals".to_string()),
        ));
    }
    Ok(())
}

/// Call before the `RunContext` it points at is dropped.
pub unsafe fn clear_context(state: *mut lua_State) {
    lua_pushnil(state);
    lua_rawsetp(state, LUA_REGISTRYINDEX, &CONTEXT_KEY as *const u8 as *const c_void);
}
