use std::collections::BTreeMap;
use std::ffi::{c_int, c_void, CString};

use psp_lua_sys::ffi::*;

use crate::host::{host_fn, HostError};
use crate::sandbox::read_string;

/// Its address is the key; the byte's value is never read.
static MODULES_KEY: u8 = 0;

const SOURCES: c_int = 1;
const LOADED: c_int = 2;
const LOADING: c_int = 3;
const PLUGIN_ID: c_int = 4;

pub fn module_path(name: &str) -> String {
    format!("{}.lua", name.replace('.', "/"))
}

/// Re-reads argument 1 fresh rather than caching it: argument 1 stays on the
/// stack for the whole call, so nothing is lost by not holding a `String`
/// across the raising calls below. `read_string` only calls `lua_type` and
/// `lua_tolstring` on an already-string value, neither of which allocates or
/// can `longjmp`, so this itself is always safe to call.
unsafe fn module_name(state: *mut lua_State) -> Result<String, HostError> {
    read_string(state, 1).ok_or_else(|| HostError::new("require expects a module name string"))
}

unsafe fn module_key(state: *mut lua_State) -> Result<CString, HostError> {
    CString::new(module_name(state)?)
        .map_err(|_| HostError::new("a module name may not contain a NUL byte"))
}

/// A C function's return count communicates the top `n` stack values back to
/// the caller; everything below them is simply discarded when it returns
/// (`luaD_poscall`/`moveresults` in `ldo.c`). `require_body` relies on this
/// throughout instead of using `lua_replace` (which `psp-lua-sys` does not
/// expose): it never needs to squeeze the stack back down to a single slot
/// before returning, only to leave the right value on top.
///
/// `lua_getfield`/`lua_setfield` can raise `LUA_ERRMEM` and `longjmp` straight
/// past this frame (unlike `luaL_loadbufferx` and `lua_pcall`, which catch
/// their own internal errors and only ever return a status code). Every such
/// call below sits in its own `{ }` block that builds the one `CString` it
/// needs and nothing else, so nothing droppable besides that `CString` is
/// live when the call runs.
fn require_body(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        if lua_type(state, 1) != LUA_TSTRING {
            return Err(HostError::new("require expects a module name string"));
        }

        lua_rawgetp(state, LUA_REGISTRYINDEX, &MODULES_KEY as *const u8 as *const c_void);
        if lua_type(state, -1) != LUA_TTABLE {
            return Err(HostError::new("the module loader is not installed"));
        }
        let registry = lua_gettop(state);

        {
            let key = module_key(state)?;
            lua_rawgeti(state, registry, LOADED as i64);
            lua_getfield(state, -1, key.as_ptr());
        }
        if lua_type(state, -1) != LUA_TNIL {
            return Ok(1);
        }
        lua_pop(state, 2);

        let in_progress = {
            let key = module_key(state)?;
            lua_rawgeti(state, registry, LOADING as i64);
            lua_getfield(state, -1, key.as_ptr());
            lua_type(state, -1) != LUA_TNIL
        };
        lua_pop(state, 2);
        if in_progress {
            let name = module_name(state)?;
            return Err(HostError::new(format!("circular require of module {name:?}")));
        }

        {
            let path_key = {
                let name = module_name(state)?;
                let path = module_path(&name);
                CString::new(path)
                    .map_err(|_| HostError::new("a module name may not contain a NUL byte"))?
            };
            lua_rawgeti(state, registry, SOURCES as i64);
            lua_getfield(state, -1, path_key.as_ptr());
        }
        let Some(source) = read_string(state, -1) else {
            let name = module_name(state)?;
            let path = module_path(&name);
            return Err(HostError::new(format!(
                "module {name:?} not found (no source at {path:?})"
            )));
        };
        lua_pop(state, 2);

        let chunk_name = {
            lua_rawgeti(state, registry, PLUGIN_ID as i64);
            let plugin_id = read_string(state, -1).unwrap_or_default();
            lua_pop(state, 1);
            let name = module_name(state)?;
            let path = module_path(&name);
            match CString::new(format!("={plugin_id}/{path}")) {
                Ok(chunk_name) => chunk_name,
                Err(_) => return Err(HostError::new("the plugin id contains a NUL byte")),
            }
        };
        let loaded = luaL_loadbufferx(
            state,
            source.as_ptr().cast(),
            source.len(),
            chunk_name.as_ptr(),
            c"t".as_ptr(),
        );
        drop(chunk_name);
        drop(source);
        if loaded != LUA_OK {
            let name = module_name(state)?;
            let message =
                read_string(state, -1).unwrap_or_else(|| format!("module {name:?} failed to load"));
            return Err(HostError::new(message));
        }

        {
            let key = module_key(state)?;
            lua_rawgeti(state, registry, LOADING as i64);
            lua_pushboolean(state, 1);
            lua_setfield(state, -2, key.as_ptr());
        }
        lua_pop(state, 1);

        let called = lua_pcall(state, 0, 1, 0);

        {
            let key = module_key(state)?;
            lua_rawgeti(state, registry, LOADING as i64);
            lua_pushnil(state);
            lua_setfield(state, -2, key.as_ptr());
        }
        lua_pop(state, 1);

        if called != LUA_OK {
            let name = module_name(state)?;
            let message = read_string(state, -1)
                .unwrap_or_else(|| format!("module {name:?} raised an error"));
            return Err(HostError::new(message));
        }

        if lua_type(state, -1) == LUA_TNIL {
            lua_pop(state, 1);
            lua_pushboolean(state, 1);
        }

        {
            let key = module_key(state)?;
            lua_rawgeti(state, registry, LOADED as i64);
            lua_pushvalue(state, -2);
            lua_setfield(state, -2, key.as_ptr());
        }
        lua_pop(state, 1);

        Ok(1)
    }
}

host_fn!(push_require, require_body);

/// Args for [`install_body`], handed across the FFI boundary as lightuserdata
/// rather than captured in a closure: `install_body` must stay a plain
/// `extern "C" fn` so it can be the callee of the `lua_pcall` in [`install`].
struct InstallArgs<'a> {
    plugin_id: &'a str,
    entries: &'a [(CString, &'a str)],
}

/// Builds the modules registry table. Runs under `install`'s `lua_pcall`: like
/// `host::install_globals_body`, an unprotected allocation failure here would
/// reach `lua_atpanic` and abort the process rather than return an error.
unsafe extern "C" fn install_body(state: *mut lua_State) -> c_int {
    let args = lua_touserdata(state, 1) as *const InstallArgs;
    if args.is_null() {
        return 0;
    }
    let args = &*args;

    lua_createtable(state, 4, 0);

    lua_createtable(state, 0, args.entries.len() as c_int);
    for (key, source) in args.entries {
        lua_pushlstring(state, source.as_ptr().cast(), source.len());
        lua_setfield(state, -2, key.as_ptr());
    }
    lua_rawseti(state, -2, SOURCES as i64);

    lua_createtable(state, 0, 0);
    lua_rawseti(state, -2, LOADED as i64);

    lua_createtable(state, 0, 0);
    lua_rawseti(state, -2, LOADING as i64);

    lua_pushlstring(state, args.plugin_id.as_ptr().cast(), args.plugin_id.len());
    lua_rawseti(state, -2, PLUGIN_ID as i64);

    lua_rawsetp(state, LUA_REGISTRYINDEX, &MODULES_KEY as *const u8 as *const c_void);

    push_require(state);
    lua_setglobal(state, c"require".as_ptr());

    0
}

pub unsafe fn install(
    state: *mut lua_State,
    plugin_id: &str,
    sources: &BTreeMap<String, String>,
) -> Result<(), HostError> {
    let mut entries = Vec::with_capacity(sources.len());
    for (path, source) in sources {
        let Ok(key) = CString::new(path.as_str()) else {
            return Err(HostError::new("a source path may not contain a NUL byte"));
        };
        entries.push((key, source.as_str()));
    }

    let args = InstallArgs { plugin_id, entries: &entries };

    lua_pushcfunction(state, install_body);
    lua_pushlightuserdata(state, &args as *const InstallArgs as *mut c_void);
    if lua_pcall(state, 1, 0, 0) != LUA_OK {
        let message = read_string(state, -1);
        lua_pop(state, 1);
        return Err(HostError::new(
            message.unwrap_or_else(|| "failed to install the module loader".to_string()),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::module_path;

    #[test]
    fn a_dotted_name_becomes_a_nested_lua_path() {
        assert_eq!(module_path("lib.util"), "lib/util.lua");
    }

    #[test]
    fn a_bare_name_becomes_a_top_level_lua_file() {
        assert_eq!(module_path("util"), "util.lua");
    }
}
