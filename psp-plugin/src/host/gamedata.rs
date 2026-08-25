use std::ffi::c_int;

use psp_lua_sys::ffi::*;
use serde_json::Value;

use super::api_def::{ApiFunction, ApiParam, ApiType};
use super::marshal::{arg_string, check_args, opt_string, push_json, push_str};
use super::{
    register_table, with_context, HostError, PushHostFn, MAX_TABLE_DEPTH, MAX_TABLE_NODES,
};
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

fn catalogs(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 0, "gamedata.catalogs")?;
        let mut names: Vec<String> = with_context(state, |ctx| {
            Ok(ctx
                .game_data
                .entry_names()
                .filter(|name| !name.contains('/'))
                .map(str::to_string)
                .collect())
        })?;
        names.sort_unstable();
        push_string_array(state, &names)?;
        Ok(1)
    }
}

fn keys(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 1, "gamedata.keys")?;
        let catalog = arg_string(state, 1, "catalog")?;
        let keys = with_context(state, |ctx| {
            Ok(ctx.game_data.entry_keys(&catalog).map(|keys| {
                keys.iter()
                    .map(|key| key.to_string())
                    .collect::<Vec<String>>()
            }))
        })?;
        match keys {
            Some(keys) => {
                push_string_array(state, &keys)?;
                Ok(1)
            }
            None => {
                lua_pushnil(state);
                Ok(1)
            }
        }
    }
}

fn get(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 2, "gamedata.get")?;
        let catalog = arg_string(state, 1, "catalog")?;
        let key = opt_string(state, 2, "key")?;
        let value = with_context(state, |ctx| {
            let catalog_value = ctx.game_data.get(&catalog);
            let selected = match &key {
                Some(key) => catalog_value
                    .and_then(Value::as_object)
                    .and_then(|entries| entries.get(key)),
                None => catalog_value,
            };
            let Some(value) = selected else { return Ok(None) };
            let nodes = count_nodes(value, 0)?;
            if nodes > MAX_TABLE_NODES {
                let (what, remedy) = match &key {
                    Some(key) => (
                        format!("the {key} entry of the {catalog} catalog"),
                        String::new(),
                    ),
                    None => (
                        format!("the {catalog} catalog"),
                        format!(
                            "; use gamedata.keys('{catalog}') and fetch entries one at a time"
                        ),
                    ),
                };
                return Err(HostError::new(format!(
                    "{what} holds {nodes} values (the limit is {MAX_TABLE_NODES}), too many for \
                     gamedata.get to return{remedy}"
                )));
            }
            Ok(Some(value.clone()))
        })?;
        match value {
            Some(value) => {
                push_json(state, &value)?;
                Ok(1)
            }
            None => {
                lua_pushnil(state);
                Ok(1)
            }
        }
    }
}

/// Mirrors how `table_to_json` counts nodes on the way out of Lua: one per
/// array element or object entry, summed over the whole tree. Guarded by the
/// same depth limit as `push_json_at`, its sibling on the way in, so a
/// pathologically deep value cannot recurse unbounded here either.
pub fn count_nodes(value: &Value, depth: usize) -> Result<usize, HostError> {
    if depth > MAX_TABLE_DEPTH {
        return Err(HostError::new("value is nested too deeply to count"));
    }
    match value {
        Value::Array(items) => {
            let mut total = items.len();
            for item in items {
                total += count_nodes(item, depth + 1)?;
            }
            Ok(total)
        }
        Value::Object(entries) => {
            let mut total = entries.len();
            for item in entries.values() {
                total += count_nodes(item, depth + 1)?;
            }
            Ok(total)
        }
        _ => Ok(0),
    }
}

unsafe fn push_string_array(state: *mut lua_State, items: &[String]) -> Result<(), HostError> {
    if lua_checkstack(state, 3) == 0 {
        return Err(HostError::new(
            "the Lua stack cannot grow to build this list",
        ));
    }
    lua_createtable(state, c_int::try_from(items.len()).unwrap_or(c_int::MAX), 0);
    for (position, item) in items.iter().enumerate() {
        push_str(state, item);
        lua_rawseti(
            state,
            -2,
            i64::try_from(position)
                .unwrap_or(i64::MAX)
                .saturating_add(1),
        );
    }
    Ok(())
}

host_fn!(push_is_valid_item, is_valid_item);
host_fn!(push_is_valid_pal, is_valid_pal);
host_fn!(push_version, version);
host_fn!(push_catalogs, catalogs);
host_fn!(push_keys, keys);
host_fn!(push_get, get);

/// A function's description and its binding, on the same line, so the two
/// cannot be transposed independently of each other.
const GAMEDATA: [(ApiFunction, PushHostFn); 6] = [
    (
        ApiFunction {
            name: "is_valid_item",
            params: &[ApiParam { name: "id", ty: ApiType::String, optional: false }],
            returns: ApiType::Boolean,
            doc: "Whether the id names an item the loaded game data knows. Case-insensitive.",
            capability: None,
        },
        push_is_valid_item,
    ),
    (
        ApiFunction {
            name: "is_valid_pal",
            params: &[ApiParam { name: "id", ty: ApiType::String, optional: false }],
            returns: ApiType::Boolean,
            doc: "Whether the id names a pal the loaded game data knows. Case-insensitive.",
            capability: None,
        },
        push_is_valid_pal,
    ),
    (
        ApiFunction {
            name: "version",
            params: &[],
            returns: ApiType::String,
            doc: "The version string of the loaded game data.",
            capability: None,
        },
        push_version,
    ),
    (
        ApiFunction {
            name: "catalogs",
            params: &[],
            returns: ApiType::List(&ApiType::String),
            doc: "The names of every top-level catalog the loaded game data ships, sorted. The nested subtrees it also loads, locale and interface strings, are not listed.",
            capability: None,
        },
        push_catalogs,
    ),
    (
        ApiFunction {
            name: "keys",
            params: &[ApiParam { name: "catalog", ty: ApiType::String, optional: false }],
            returns: ApiType::Union(&[ApiType::List(&ApiType::String), ApiType::Nil]),
            doc: "The top-level keys of the named catalog, or nil if no catalog by that name exists. Catalog names are matched case-insensitively.",
            capability: None,
        },
        push_keys,
    ),
    (
        ApiFunction {
            name: "get",
            params: &[
                ApiParam { name: "catalog", ty: ApiType::String, optional: false },
                ApiParam { name: "key", ty: ApiType::String, optional: true },
            ],
            returns: ApiType::Union(&[ApiType::Any, ApiType::Nil]),
            doc: "The named catalog, or one entry of it if key is given, or nil if the catalog or key does not exist. A stored JSON null also arrives as nil, indistinguishable from absent. Catalog names are matched case-insensitively.",
            capability: None,
        },
        push_get,
    ),
];

/// Derived from [`GAMEDATA`] for the API-definition consumer: the description
/// half of every pair, taken by index, so there is no hand-written list of
/// them that could fall out of step with the bindings beside them.
pub const GAMEDATA_FUNCTIONS: &[ApiFunction] = &{
    let mut functions = [GAMEDATA[0].0; GAMEDATA.len()];
    let mut index = 1;
    while index < GAMEDATA.len() {
        functions[index] = GAMEDATA[index].0;
        index += 1;
    }
    functions
};

fn gamedata_bindings() -> [(&'static str, PushHostFn); GAMEDATA.len()] {
    std::array::from_fn(|i| (GAMEDATA[i].0.name, GAMEDATA[i].1))
}

pub unsafe fn install(state: *mut lua_State) {
    register_table(state, "gamedata", &gamedata_bindings());
}
