use std::alloc::{alloc, dealloc, realloc, Layout};
use std::ffi::{c_int, c_void, CString};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use psp_lua_sys::ffi::*;

use crate::status::RunStatus;

pub const DEFAULT_MEMORY_BYTES: usize = 256 * 1024 * 1024;
pub const DEFAULT_WALL_CLOCK_MS: i64 = 120_000;
pub const DEFAULT_HOOK_INTERVAL: c_int = 10_000;

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub memory_bytes: usize,
    pub wall_clock_ms: i64,
    pub hook_interval: c_int,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            memory_bytes: DEFAULT_MEMORY_BYTES,
            wall_clock_ms: DEFAULT_WALL_CLOCK_MS,
            hook_interval: DEFAULT_HOOK_INTERVAL,
        }
    }
}

#[derive(Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// Covers the alignment of every scalar Lua stores, on every supported target.
const ALLOC_ALIGN: usize = 16;

struct AllocState {
    used: usize,
    limit: usize,
    refused: bool,
}

/// `lua_Alloc`: when `ptr` is null, `osize` is a type tag, not a size. Returning
/// null makes Lua raise `LUA_ERRMEM`, which is how the ceiling is enforced.
unsafe extern "C" fn limited_alloc(
    ud: *mut c_void,
    ptr: *mut c_void,
    osize: usize,
    nsize: usize,
) -> *mut c_void {
    let state = &mut *(ud as *mut AllocState);
    let old = if ptr.is_null() { 0 } else { osize };

    if nsize == 0 {
        if !ptr.is_null() {
            if let Ok(layout) = Layout::from_size_align(old, ALLOC_ALIGN) {
                dealloc(ptr as *mut u8, layout);
            }
            state.used = state.used.saturating_sub(old);
        }
        return std::ptr::null_mut();
    }

    let projected = state.used.saturating_sub(old).saturating_add(nsize);
    if projected > state.limit {
        state.refused = true;
        return std::ptr::null_mut();
    }

    let Ok(new_layout) = Layout::from_size_align(nsize, ALLOC_ALIGN) else {
        return std::ptr::null_mut();
    };

    let fresh = if ptr.is_null() {
        alloc(new_layout)
    } else {
        let Ok(old_layout) = Layout::from_size_align(old, ALLOC_ALIGN) else {
            return std::ptr::null_mut();
        };
        realloc(ptr as *mut u8, old_layout, nsize)
    };

    if fresh.is_null() {
        return std::ptr::null_mut();
    }
    state.used = projected;
    fresh as *mut c_void
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Trip {
    Timeout,
    Cancelled,
}

struct Interrupt {
    deadline: DateTime<Utc>,
    cancel: Cancel,
    trip: Option<Trip>,
}

/// `lua_error` below `longjmp`s past this frame: it must own no droppable value
/// and must never allocate. A dynamic message would corrupt the heap on
/// `x86_64-pc-windows-msvc`, where that `longjmp` runs a real SEH unwind.
unsafe extern "C" fn interrupt_hook(state: *mut lua_State, _debug: *mut c_void) {
    let slot = lua_getextraspace(state) as *mut *mut Interrupt;
    if slot.is_null() || (*slot).is_null() {
        return;
    }
    let interrupt = &mut **slot;

    // A recorded trip means a pcall inside the script swallowed our error.
    let reason = match interrupt.trip {
        Some(Trip::Cancelled) => Some(Trip::Cancelled),
        Some(Trip::Timeout) => Some(Trip::Timeout),
        None if interrupt.cancel.is_cancelled() => Some(Trip::Cancelled),
        None if Utc::now() >= interrupt.deadline => Some(Trip::Timeout),
        None => None,
    };

    let Some(reason) = reason else { return };
    interrupt.trip = Some(reason);
    // `lua_sethook` is per-thread while the trip is shared, so tighten on every
    // fire: each thread must ratchet itself down the first time it notices.
    lua_sethook(state, Some(interrupt_hook), LUA_MASKCOUNT, 1);
    let message = match reason {
        Trip::Cancelled => c"plugin run cancelled".as_ptr(),
        Trip::Timeout => c"plugin run exceeded its time limit".as_ptr(),
    };
    lua_pushstring(state, message);
    lua_error(state);
}

/// Only keeps the default panic handler from running; `abort()` follows either way.
unsafe extern "C" fn quiet_panic(_state: *mut lua_State) -> c_int {
    0
}

/// Lua honours `__gc` only when the key is present at `setmetatable` time, so
/// stripping it here stops a finaliser — which runs with hooks disabled, out of
/// the deadline's reach — from ever being registered.
unsafe extern "C" fn guarded_setmetatable(state: *mut lua_State) -> c_int {
    // A short call must stay short: a padded nil reads as "remove the
    // metatable", so `setmetatable(t)` would silently strip instead of erroring.
    let top = lua_gettop(state);
    let n = top.min(2);
    if top > 2 {
        lua_settop(state, 2);
    }
    if n >= 2 && lua_type(state, 2) == LUA_TTABLE {
        // Raw: a non-raw set could fire `__newindex` on the metatable's own
        // metatable and reinstall `__gc` before the call-through below.
        lua_pushstring(state, c"__gc".as_ptr());
        lua_pushnil(state);
        lua_rawset(state, 2);
    }
    lua_pushvalue(state, lua_upvalueindex(1));
    for i in 1..=n {
        lua_pushvalue(state, i);
    }
    lua_call(state, n, 1);
    1
}

pub unsafe fn open_libraries(state: *mut lua_State) {
    let libraries: [(&[u8], lua_CFunction); 6] = [
        (LUA_GNAME, luaopen_base),
        (LUA_COLIBNAME, luaopen_coroutine),
        (LUA_MATHLIBNAME, luaopen_math),
        (LUA_STRLIBNAME, luaopen_string),
        (LUA_TABLIBNAME, luaopen_table),
        (LUA_UTF8LIBNAME, luaopen_utf8),
    ];
    for (name, opener) in libraries {
        luaL_requiref(state, name.as_ptr().cast(), opener, 1);
        lua_pop(state, 1);
    }

    // `xpcall` goes with the loaders: its message handler runs with hooks
    // disabled, so a handler that loops outruns the deadline. `pcall` is safe.
    for loader in [
        c"load".as_ptr(),
        c"loadfile".as_ptr(),
        c"dofile".as_ptr(),
        c"xpcall".as_ptr(),
    ] {
        lua_pushnil(state);
        lua_setglobal(state, loader);
    }

    lua_getglobal(state, c"setmetatable".as_ptr());
    lua_pushcclosure(state, guarded_setmetatable, 1);
    lua_setglobal(state, c"setmetatable".as_ptr());
}

/// Runs under `Sandbox::new`'s `lua_pcall`: opening the libraries allocates, and
/// an unprotected `LUA_ERRMEM` here reaches `ldo.c`'s unconditional `abort()`.
unsafe extern "C" fn setup_state(state: *mut lua_State) -> c_int {
    open_libraries(state);
    0
}

/// `alloc` and `interrupt` are raw pointers, not `Box`es: C holds a copy of each
/// for the state's whole life, and a `Box` field would retag the allocation on
/// every access through `self`, invalidating the pointer C still holds.
pub struct Sandbox {
    raw: *mut lua_State,
    alloc: *mut AllocState,
    interrupt: *mut Interrupt,
    hook_interval: c_int,
    limits: Limits,
    returned: Option<String>,
}

impl Sandbox {
    pub fn new(limits: Limits, cancel: Cancel) -> Option<Self> {
        let alloc = Box::into_raw(Box::new(AllocState {
            used: 0,
            limit: limits.memory_bytes,
            refused: false,
        }));
        let raw = unsafe { lua_newstate(limited_alloc, alloc.cast()) };
        if raw.is_null() {
            unsafe { drop(Box::from_raw(alloc)) };
            return None;
        }

        let interrupt = Box::into_raw(Box::new(Interrupt {
            deadline: Utc::now(),
            cancel,
            trip: None,
        }));
        let interval = limits.hook_interval.max(1);

        unsafe {
            lua_atpanic(raw, quiet_panic);
            let slot = lua_getextraspace(raw) as *mut *mut Interrupt;
            *slot = interrupt;

            lua_pushcfunction(raw, setup_state);
            if lua_pcall(raw, 0, 0, 0) != LUA_OK {
                *slot = std::ptr::null_mut();
                lua_close(raw);
                drop(Box::from_raw(alloc));
                drop(Box::from_raw(interrupt));
                return None;
            }
            lua_sethook(raw, Some(interrupt_hook), LUA_MASKCOUNT, interval);
        }

        Some(Self {
            raw,
            alloc,
            interrupt,
            hook_interval: interval,
            limits,
            returned: None,
        })
    }

    pub fn as_ptr(&self) -> *mut lua_State {
        self.raw
    }

    pub fn limits(&self) -> Limits {
        self.limits
    }

    pub fn eval(&mut self, chunk_name: &str, source: &str) -> RunStatus {
        self.returned = None;
        self.arm();

        let Ok(name) = CString::new(chunk_name) else {
            return RunStatus::Error("chunk name contains a NUL byte".to_string());
        };

        unsafe {
            let loaded = luaL_loadbufferx(
                self.raw,
                source.as_ptr().cast(),
                source.len(),
                name.as_ptr(),
                c"t".as_ptr(),
            );
            if loaded != LUA_OK {
                return self.classify_error(loaded);
            }

            let called = lua_pcall(self.raw, 0, 1, 0);
            if called != LUA_OK {
                return self.classify_error(called);
            }

            // A trip can cascade out through nested pcalls and still return
            // normally, so a recorded trip outranks a successful return.
            if let Some(status) = self.trip_status() {
                lua_settop(self.raw, 0);
                return status;
            }

            if lua_type(self.raw, -1) == LUA_TSTRING {
                self.returned = read_string(self.raw, -1);
            }
            lua_settop(self.raw, 0);
            RunStatus::Ok
        }
    }

    pub fn take_return_string(&mut self) -> Option<String> {
        self.returned.take()
    }

    /// Call before every `lua_pcall` a caller drives itself; `eval` does it. The
    /// interval reset matters: a tripped run tightens the hook and never widens.
    pub fn arm(&mut self) {
        unsafe {
            (*self.alloc).refused = false;
            (*self.interrupt).trip = None;
            (*self.interrupt).deadline =
                Utc::now() + chrono::Duration::milliseconds(self.limits.wall_clock_ms);
            lua_sethook(self.raw, Some(interrupt_hook), LUA_MASKCOUNT, self.hook_interval);
        }
    }

    pub fn trip_status(&self) -> Option<RunStatus> {
        match unsafe { (*self.interrupt).trip } {
            Some(Trip::Cancelled) => Some(RunStatus::Cancelled),
            Some(Trip::Timeout) => Some(RunStatus::Timeout),
            None => None,
        }
    }

    /// A recorded trip outranks the raw status: a timeout and a cancellation
    /// both surface to Lua as an ordinary runtime error.
    pub unsafe fn classify_error(&mut self, status: c_int) -> RunStatus {
        let message = read_string(self.raw, -1).unwrap_or_else(|| "unknown error".to_string());
        lua_settop(self.raw, 0);

        if let Some(status) = self.trip_status() {
            return status;
        }
        if status == LUA_ERRMEM && (*self.alloc).refused {
            return RunStatus::MemoryExceeded;
        }
        RunStatus::Error(message)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        unsafe {
            lua_sethook(self.raw, None, 0, 0);
            let slot = lua_getextraspace(self.raw) as *mut *mut Interrupt;
            *slot = std::ptr::null_mut();
            // lua_close frees Lua-owned memory through `alloc`, so it goes first.
            lua_close(self.raw);
            drop(Box::from_raw(self.alloc));
            drop(Box::from_raw(self.interrupt));
        }
    }
}

/// Refuses a non-string: `lua_tolstring` would convert it in place, which
/// allocates and can raise `LUA_ERRMEM`, and callers here run unprotected.
pub(crate) unsafe fn read_string(state: *mut lua_State, index: c_int) -> Option<String> {
    if lua_type(state, index) != LUA_TSTRING {
        return None;
    }
    let mut len: usize = 0;
    let ptr = lua_tolstring(state, index, &mut len);
    if ptr.is_null() {
        return None;
    }
    let bytes = std::slice::from_raw_parts(ptr as *const u8, len);
    Some(String::from_utf8_lossy(bytes).into_owned())
}
