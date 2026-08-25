use std::ffi::{c_int, CStr};

use psp_lua_sys::ffi::*;

use super::marshal::check_args;
use super::{with_context, HostError, PushHostFn};
use crate::context::DeleteWhereKind;
use crate::host_fn;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HandleKind {
    Player,
    Pal,
    Guild,
    Base,
    Container,
    Slot,
}

impl std::fmt::Display for HandleKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            HandleKind::Player => "player",
            HandleKind::Pal => "pal",
            HandleKind::Guild => "guild",
            HandleKind::Base => "base",
            HandleKind::Container => "container",
            HandleKind::Slot => "slot",
        };
        f.write_str(text)
    }
}

fn metatable_name(kind: HandleKind) -> &'static CStr {
    match kind {
        HandleKind::Player => c"psp.player",
        HandleKind::Pal => c"psp.pal",
        HandleKind::Guild => c"psp.guild",
        HandleKind::Base => c"psp.base",
        HandleKind::Container => c"psp.container",
        HandleKind::Slot => c"psp.slot",
    }
}

/// Must stay allocation-free and `Copy`: Lua frees userdata without running Rust
/// drop glue, so a `String` or `Vec` here would leak on every handle created.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Handle {
    pub kind: HandleKind,
    pub id: uuid::Uuid,
    pub slot: i32,
    pub epoch: u64,
}

pub unsafe fn push_handle(state: *mut lua_State, handle: Handle) {
    let ptr = lua_newuserdatauv(state, std::mem::size_of::<Handle>(), 0) as *mut Handle;
    std::ptr::write(ptr, handle);
    luaL_setmetatable(state, metatable_name(handle.kind).as_ptr());
}

/// `luaL_testudata`, not `luaL_checkudata`, which raises. A handle whose epoch
/// no longer matches is refused: re-resolving it could read the wrong entity.
pub unsafe fn read_handle(
    state: *mut lua_State,
    index: c_int,
    expected: HandleKind,
) -> Result<Handle, HostError> {
    let ptr = luaL_testudata(state, index, metatable_name(expected).as_ptr()) as *mut Handle;
    if ptr.is_null() {
        return Err(HostError::new(format!("expected a {expected} handle")));
    }
    let handle = std::ptr::read(ptr);
    with_context(state, |ctx| {
        if handle.epoch != ctx.mutation_epoch() {
            Err(invalidated_handle_error())
        } else {
            Ok(())
        }
    })?;
    Ok(handle)
}

pub fn invalidated_handle_error() -> HostError {
    HostError::new(
        "this handle was invalidated by a change made during iteration; use a bulk form such as delete_where",
    )
}

pub fn handle_kind_for(kind: DeleteWhereKind) -> HandleKind {
    match kind {
        DeleteWhereKind::Player => HandleKind::Player,
        DeleteWhereKind::Guild => HandleKind::Guild,
        DeleteWhereKind::Pal => HandleKind::Pal,
    }
}

fn handle_tostring(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 1, "tostring")?;
        let ptr = lua_touserdata(state, 1) as *const Handle;
        if ptr.is_null() {
            super::marshal::push_str(state, "psp.handle");
            return Ok(1);
        }
        let handle = std::ptr::read(ptr);
        super::marshal::push_str(state, &format!("{}:{}", handle.kind, handle.id));
        Ok(1)
    }
}

host_fn!(push_handle_tostring, handle_tostring);

/// `__metatable` is set to a string so a script cannot replace the dispatch.
unsafe fn install_one(
    state: *mut lua_State,
    kind: HandleKind,
    index_fn: PushHostFn,
    newindex_fn: Option<PushHostFn>,
) {
    let name = metatable_name(kind);
    luaL_newmetatable(state, name.as_ptr());
    index_fn(state);
    lua_setfield(state, -2, c"__index".as_ptr());
    if let Some(newindex_fn) = newindex_fn {
        newindex_fn(state);
        lua_setfield(state, -2, c"__newindex".as_ptr());
    }
    push_handle_tostring(state);
    lua_setfield(state, -2, c"__tostring".as_ptr());
    lua_pushstring(state, name.as_ptr());
    lua_setfield(state, -2, c"__metatable".as_ptr());
    lua_pop(state, 1);
}

pub unsafe fn install_metatables(state: *mut lua_State) {
    install_one(state, HandleKind::Player, super::save_read::push_player_index as PushHostFn, None);
    install_one(
        state,
        HandleKind::Pal,
        super::save_read::push_pal_index as PushHostFn,
        Some(super::fields::pal::push_pal_newindex as PushHostFn),
    );
    install_one(state, HandleKind::Guild, super::save_read::push_guild_index as PushHostFn, None);
    install_one(state, HandleKind::Base, super::save_read::push_base_index as PushHostFn, None);
    install_one(state, HandleKind::Container, super::save_read::push_container_index as PushHostFn, None);
    install_one(state, HandleKind::Slot, super::save_read::push_slot_index as PushHostFn, None);
}
