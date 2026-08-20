use std::ffi::c_int;

use psp_lua_sys::ffi::*;

use super::api_def::{ApiFunction, ApiParam, ApiType};
use super::marshal::{arg_string, check_args, push_str};
use super::{register_table, with_context, HostError, PushHostFn};
use crate::host_fn;

fn is_valid_item(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 1, "gamedata.is_valid_item")?;
        let id = arg_string(state, 1, "id")?;
        let valid = with_context(state, |ctx| Ok(ctx.game_data.is_known_item_key(&id)))?;
        lua_pushboolean(state, c_int::from(valid));
        Ok(1)
    }
}

fn is_valid_pal(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 1, "gamedata.is_valid_pal")?;
        let id = arg_string(state, 1, "id")?;
        let valid = with_context(state, |ctx| Ok(ctx.game_data.is_known_pal_key(&id)))?;
        lua_pushboolean(state, c_int::from(valid));
        Ok(1)
    }
}

fn version(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 0, "gamedata.version")?;
        let version = with_context(state, |ctx| Ok(ctx.game_data.version().to_string()))?;
        push_str(state, &version);
        Ok(1)
    }
}

host_fn!(push_is_valid_item, is_valid_item);
host_fn!(push_is_valid_pal, is_valid_pal);
host_fn!(push_version, version);

pub const GAMEDATA_FUNCTIONS: &[ApiFunction] = &[
    ApiFunction {
        name: "is_valid_item",
        params: &[ApiParam { name: "id", ty: ApiType::String, optional: false }],
        returns: ApiType::Boolean,
        doc: "Whether the id names an item the loaded game data knows. Case-insensitive.",
        capability: None,
    },
    ApiFunction {
        name: "is_valid_pal",
        params: &[ApiParam { name: "id", ty: ApiType::String, optional: false }],
        returns: ApiType::Boolean,
        doc: "Whether the id names a pal the loaded game data knows. Case-insensitive.",
        capability: None,
    },
    ApiFunction {
        name: "version",
        params: &[],
        returns: ApiType::String,
        doc: "The version string of the loaded game data.",
        capability: None,
    },
];

/// Length tied to [`GAMEDATA_FUNCTIONS`] so a missing binding is a compile error.
const GAMEDATA_PUSH_FNS: [PushHostFn; GAMEDATA_FUNCTIONS.len()] =
    [push_is_valid_item, push_is_valid_pal, push_version];

fn gamedata_bindings() -> [(&'static str, PushHostFn); GAMEDATA_FUNCTIONS.len()] {
    std::array::from_fn(|i| (GAMEDATA_FUNCTIONS[i].name, GAMEDATA_PUSH_FNS[i]))
}

pub unsafe fn install(state: *mut lua_State) {
    register_table(state, "gamedata", &gamedata_bindings());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_names_match_described_functions_in_order() {
        let bindings = gamedata_bindings();
        let bound_names: Vec<&str> = bindings.iter().map(|(name, _)| *name).collect();
        let described_names: Vec<&str> = GAMEDATA_FUNCTIONS.iter().map(|f| f.name).collect();
        assert_eq!(bound_names, described_names);
    }
}
