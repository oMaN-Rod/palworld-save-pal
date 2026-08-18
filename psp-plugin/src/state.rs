use psp_lua_sys::ffi::{lua_State, lua_close, lua_gettop, luaL_newstate};

/// Owns a `lua_State` and closes it on drop.
///
/// Construction is the only fallible part: `luaL_newstate` returns null when
/// the platform allocator refuses the initial allocation.
pub struct LuaState {
    raw: *mut lua_State,
}

impl LuaState {
    pub fn new() -> Option<Self> {
        let raw = unsafe { luaL_newstate() };
        if raw.is_null() {
            None
        } else {
            Some(Self { raw })
        }
    }

    pub fn as_ptr(&self) -> *mut lua_State {
        self.raw
    }

    pub fn stack_top(&self) -> i32 {
        unsafe { lua_gettop(self.raw) }
    }
}

impl Drop for LuaState {
    fn drop(&mut self) {
        unsafe { lua_close(self.raw) };
    }
}
