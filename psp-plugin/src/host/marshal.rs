use std::ffi::{c_int, CStr};

use psp_lua_sys::ffi::*;
use serde_json::Value;

use super::{HostError, MAX_TABLE_DEPTH, MAX_TABLE_NODES};

pub(crate) unsafe fn type_name(state: *mut lua_State, index: c_int) -> String {
    let tag = lua_type(state, index);
    type_name_of(state, tag)
}

unsafe fn type_name_of(state: *mut lua_State, tag: c_int) -> String {
    let ptr = lua_typename(state, tag);
    if ptr.is_null() {
        return "unknown".to_string();
    }
    CStr::from_ptr(ptr).to_string_lossy().into_owned()
}

/// The value at `index` must already be a string: `lua_tolstring` would convert
/// a non-string in place, which allocates and can break an enclosing `lua_next`.
pub(crate) unsafe fn read_string_at(state: *mut lua_State, index: c_int) -> Option<String> {
    let mut len: usize = 0;
    let ptr = lua_tolstring(state, index, &mut len);
    if ptr.is_null() {
        return None;
    }
    let bytes = std::slice::from_raw_parts(ptr.cast::<u8>(), len);
    Some(String::from_utf8_lossy(bytes).into_owned())
}

pub unsafe fn arg_integer(state: *mut lua_State, index: c_int, name: &str) -> Result<i64, HostError> {
    if lua_isinteger(state, index) == 0 {
        let actual = type_name(state, index);
        return Err(HostError::new(format!("expected an integer for {name}, got {actual}")));
    }
    Ok(lua_tointeger(state, index))
}

/// Accepts anything Lua itself coerces to a number, numeric strings included —
/// unlike `arg_integer`, which never coerces.
pub unsafe fn arg_number(state: *mut lua_State, index: c_int, name: &str) -> Result<f64, HostError> {
    let mut is_num: c_int = 0;
    let value = lua_tonumberx(state, index, &mut is_num);
    if is_num == 0 {
        let actual = type_name(state, index);
        return Err(HostError::new(format!("expected a number for {name}, got {actual}")));
    }
    Ok(value)
}

pub unsafe fn arg_string(state: *mut lua_State, index: c_int, name: &str) -> Result<String, HostError> {
    if lua_type(state, index) != LUA_TSTRING {
        let actual = type_name(state, index);
        return Err(HostError::new(format!("expected a string for {name}, got {actual}")));
    }
    read_string_at(state, index)
        .ok_or_else(|| HostError::new(format!("failed to read the string value of {name}")))
}

pub unsafe fn arg_bool(state: *mut lua_State, index: c_int, name: &str) -> Result<bool, HostError> {
    if lua_type(state, index) != LUA_TBOOLEAN {
        let actual = type_name(state, index);
        return Err(HostError::new(format!("expected a boolean for {name}, got {actual}")));
    }
    Ok(lua_toboolean(state, index) != 0)
}

pub unsafe fn arg_uuid(state: *mut lua_State, index: c_int, name: &str) -> Result<uuid::Uuid, HostError> {
    let text = arg_string(state, index, name)?;
    uuid::Uuid::parse_str(&text)
        .map_err(|_| HostError::new(format!("{name} is not a valid UUID: {text}")))
}

pub unsafe fn opt_integer(
    state: *mut lua_State,
    index: c_int,
    name: &str,
) -> Result<Option<i64>, HostError> {
    match lua_type(state, index) {
        LUA_TNONE | LUA_TNIL => Ok(None),
        _ => arg_integer(state, index, name).map(Some),
    }
}

pub unsafe fn opt_string(
    state: *mut lua_State,
    index: c_int,
    name: &str,
) -> Result<Option<String>, HostError> {
    match lua_type(state, index) {
        LUA_TNONE | LUA_TNIL => Ok(None),
        _ => arg_string(state, index, name).map(Some),
    }
}

pub unsafe fn opt_bool(
    state: *mut lua_State,
    index: c_int,
    name: &str,
) -> Result<Option<bool>, HostError> {
    match lua_type(state, index) {
        LUA_TNONE | LUA_TNIL => Ok(None),
        _ => arg_bool(state, index, name).map(Some),
    }
}

pub unsafe fn check_args(state: *mut lua_State, max: c_int, name: &str) -> Result<(), HostError> {
    let top = lua_gettop(state);
    if top > max {
        return Err(HostError::new(format!(
            "{name} takes at most {max} argument(s), got {top}"
        )));
    }
    Ok(())
}

pub unsafe fn push_str(state: *mut lua_State, value: &str) {
    lua_pushlstring(state, value.as_ptr().cast(), value.len());
}

pub unsafe fn push_json(state: *mut lua_State, value: &Value) -> Result<(), HostError> {
    push_json_at(state, value, 0)
}

unsafe fn push_json_at(state: *mut lua_State, value: &Value, depth: usize) -> Result<(), HostError> {
    if depth > MAX_TABLE_DEPTH {
        return Err(HostError::new("value is nested too deeply to push into Lua"));
    }
    match value {
        Value::Null => lua_pushnil(state),
        Value::Bool(flag) => lua_pushboolean(state, c_int::from(*flag)),
        Value::Number(number) => {
            if let Some(i) = number.as_i64() {
                lua_pushinteger(state, i);
            } else {
                lua_pushnumber(state, number.as_f64().unwrap_or(0.0));
            }
        }
        Value::String(text) => push_str(state, text),
        Value::Array(items) => {
            if lua_checkstack(state, 4) == 0 {
                return Err(HostError::new("the Lua stack cannot grow to push this array"));
            }
            let len = c_int::try_from(items.len()).unwrap_or(c_int::MAX);
            lua_createtable(state, len, 0);
            for (i, item) in items.iter().enumerate() {
                push_json_at(state, item, depth + 1)?;
                let index = i64::try_from(i).unwrap_or(i64::MAX).saturating_add(1);
                lua_rawseti(state, -2, index);
            }
        }
        Value::Object(map) => {
            if lua_checkstack(state, 4) == 0 {
                return Err(HostError::new("the Lua stack cannot grow to push this object"));
            }
            let len = c_int::try_from(map.len()).unwrap_or(c_int::MAX);
            lua_createtable(state, 0, len);
            for (key, item) in map {
                push_str(state, key);
                push_json_at(state, item, depth + 1)?;
                lua_rawset(state, -3);
            }
        }
    }
    Ok(())
}

enum RawKey {
    Int(i64),
    Text(String),
}

/// Non-string keys are converted from a `lua_pushvalue` copy: converting a
/// number key in place is the mutation that breaks the enclosing `lua_next`.
unsafe fn read_key(state: *mut lua_State, index: c_int) -> Result<RawKey, HostError> {
    if lua_isinteger(state, index) != 0 {
        return Ok(RawKey::Int(lua_tointeger(state, index)));
    }
    if lua_type(state, index) == LUA_TSTRING {
        let text = read_string_at(state, index)
            .ok_or_else(|| HostError::new("failed to read a table key"))?;
        return Ok(RawKey::Text(text));
    }

    lua_pushvalue(state, index);
    let text = if lua_type(state, -1) == LUA_TNUMBER {
        let mut is_num: c_int = 0;
        let number = lua_tonumberx(state, -1, &mut is_num);
        number.to_string()
    } else {
        let tag = lua_type(state, -1);
        format!("<{}>", type_name_of(state, tag))
    };
    lua_pop(state, 1);
    Ok(RawKey::Text(text))
}

unsafe fn convert_value(
    state: *mut lua_State,
    index: c_int,
    depth: usize,
    nodes: &mut usize,
) -> Result<Value, HostError> {
    match lua_type(state, index) {
        LUA_TNIL | LUA_TNONE => Ok(Value::Null),
        LUA_TBOOLEAN => Ok(Value::Bool(lua_toboolean(state, index) != 0)),
        LUA_TNUMBER => {
            if lua_isinteger(state, index) != 0 {
                Ok(Value::from(lua_tointeger(state, index)))
            } else {
                let mut is_num: c_int = 0;
                let number = lua_tonumberx(state, index, &mut is_num);
                serde_json::Number::from_f64(number)
                    .map(Value::Number)
                    .ok_or_else(|| HostError::new("a number is not finite and cannot become JSON"))
            }
        }
        LUA_TSTRING => read_string_at(state, index)
            .map(Value::String)
            .ok_or_else(|| HostError::new("failed to read a table value")),
        LUA_TTABLE => convert_table(state, index, depth + 1, nodes),
        other => Ok(Value::String(format!("<{}>", type_name_of(state, other)))),
    }
}

/// Raw accessors only: `lua_next`/`lua_rawlen` never fire a script's metatable.
/// `depth` is also what makes a self-referential table terminate.
unsafe fn convert_table(
    state: *mut lua_State,
    index: c_int,
    depth: usize,
    nodes: &mut usize,
) -> Result<Value, HostError> {
    if depth > MAX_TABLE_DEPTH {
        return Err(HostError::new("table is nested too deeply to convert"));
    }
    if lua_checkstack(state, 4) == 0 {
        return Err(HostError::new("the Lua stack cannot grow to walk this table"));
    }

    let index = lua_absindex(state, index);
    let level_top = lua_gettop(state);
    let len = lua_rawlen(state, index);
    let mut entries: Vec<(RawKey, Value)> = Vec::new();

    lua_pushnil(state);
    while lua_next(state, index) != 0 {
        *nodes += 1;
        if *nodes > MAX_TABLE_NODES {
            lua_settop(state, level_top);
            return Err(HostError::new("table has too many entries to convert"));
        }

        lua_pushvalue(state, -2);
        let key = match read_key(state, -1) {
            Ok(key) => key,
            Err(error) => {
                lua_settop(state, level_top);
                return Err(error);
            }
        };
        lua_pop(state, 1);

        let value = match convert_value(state, -1, depth, nodes) {
            Ok(value) => value,
            Err(error) => {
                lua_settop(state, level_top);
                return Err(error);
            }
        };
        entries.push((key, value));

        lua_pop(state, 1);
    }

    let mut is_array = len > 0 && entries.len() as u64 == len;
    if is_array {
        for (key, _) in &entries {
            match key {
                RawKey::Int(k) if *k >= 1 && (*k as u64) <= len => {}
                _ => {
                    is_array = false;
                    break;
                }
            }
        }
    }

    if is_array {
        let len_usize = len as usize;
        let mut slots: Vec<Value> = vec![Value::Null; len_usize];
        for (key, value) in entries {
            if let RawKey::Int(k) = key {
                if let Some(slot) = slots.get_mut((k - 1) as usize) {
                    *slot = value;
                }
            }
        }
        Ok(Value::Array(slots))
    } else {
        let mut map = serde_json::Map::new();
        for (key, value) in entries {
            let text = match key {
                RawKey::Int(k) => k.to_string(),
                RawKey::Text(t) => t,
            };
            map.insert(text, value);
        }
        Ok(Value::Object(map))
    }
}

pub unsafe fn table_to_json(state: *mut lua_State, index: c_int) -> Result<Value, HostError> {
    let saved_top = lua_gettop(state);
    let abs_index = lua_absindex(state, index);

    let result = if lua_type(state, abs_index) != LUA_TTABLE {
        let actual = type_name(state, abs_index);
        Err(HostError::new(format!("expected a table, got {actual}")))
    } else {
        let mut nodes = 0usize;
        convert_table(state, abs_index, 0, &mut nodes)
    };

    lua_settop(state, saved_top);
    result
}
