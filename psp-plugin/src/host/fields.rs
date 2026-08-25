use std::ffi::c_int;

use psp_core::dto::ordered_map::OrderedMap;
use psp_core::dto::summary::PalSummary;
use psp_lua_sys::ffi::*;
use serde::Serialize;

use super::api_def::ApiType;
use super::{marshal, HostError};

pub mod pal;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    ReadWrite,
    ReadOnly,
}

/// A marshaled value in either direction: read off a summary/DTO to push to
/// Lua, or read off the Lua stack for a `__newindex` write. `List` and `Map`
/// are the two collection shapes a row can hold; everything else is a scalar.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FieldValue {
    Nil,
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    List(Vec<String>),
    Map(OrderedMap<String, i64>),
}

pub(crate) fn field_value_type_name(value: &FieldValue) -> &'static str {
    match value {
        FieldValue::Nil => "nil",
        FieldValue::Str(_) => "string",
        FieldValue::Int(_) => "integer",
        FieldValue::Float(_) => "number",
        FieldValue::Bool(_) => "boolean",
        FieldValue::List(_) => "list",
        FieldValue::Map(_) => "map",
    }
}

/// A fresh table every time for the two collection shapes: the table Lua ends
/// up holding is a copy, so mutating it writes through to nothing.
pub(crate) unsafe fn push_field_value(state: *mut lua_State, value: FieldValue) -> Result<(), HostError> {
    match value {
        FieldValue::Nil => lua_pushnil(state),
        FieldValue::Str(s) => marshal::push_str(state, &s),
        FieldValue::Int(i) => lua_pushinteger(state, i),
        FieldValue::Float(f) => lua_pushnumber(state, f),
        FieldValue::Bool(b) => lua_pushboolean(state, c_int::from(b)),
        FieldValue::List(items) => {
            if lua_checkstack(state, 3) == 0 {
                return Err(HostError::new("the Lua stack cannot grow to build this list"));
            }
            lua_createtable(state, c_int::try_from(items.len()).unwrap_or(c_int::MAX), 0);
            for (position, item) in items.iter().enumerate() {
                marshal::push_str(state, item);
                lua_rawseti(state, -2, i64::try_from(position).unwrap_or(i64::MAX).saturating_add(1));
            }
        }
        FieldValue::Map(entries) => {
            if lua_checkstack(state, 4) == 0 {
                return Err(HostError::new("the Lua stack cannot grow to build this map"));
            }
            lua_createtable(state, 0, c_int::try_from(entries.len()).unwrap_or(c_int::MAX));
            for (key, number) in entries.iter() {
                marshal::push_str(state, key);
                lua_pushinteger(state, *number);
                lua_rawset(state, -3);
            }
        }
    }
    Ok(())
}

/// Reads whatever is at `index` into a [`FieldValue`], for a `__newindex`
/// write whose declared field type is not yet known to the caller. A table is
/// read as whichever collection shape it actually has, so a row that wanted
/// the other one -- or a scalar row that wanted no table at all -- refuses it
/// by type in its own validation. A function or other non-scalar is rejected
/// outright here.
pub(crate) unsafe fn read_field_value(
    state: *mut lua_State,
    index: c_int,
    name: &str,
) -> Result<FieldValue, HostError> {
    match lua_type(state, index) {
        LUA_TTABLE => match marshal::read_collection(state, index, name)? {
            marshal::TableCollection::List(items) => Ok(FieldValue::List(items)),
            marshal::TableCollection::Map(pairs) => Ok(FieldValue::Map(pairs.into_iter().collect())),
        },
        LUA_TNIL | LUA_TNONE => Ok(FieldValue::Nil),
        LUA_TBOOLEAN => Ok(FieldValue::Bool(lua_toboolean(state, index) != 0)),
        LUA_TSTRING => marshal::read_string_at(state, index)
            .map(FieldValue::Str)
            .ok_or_else(|| HostError::new("failed to read the assigned string value")),
        LUA_TNUMBER => {
            if lua_isinteger(state, index) != 0 {
                Ok(FieldValue::Int(lua_tointeger(state, index)))
            } else {
                let mut is_num: c_int = 0;
                let value = lua_tonumberx(state, index, &mut is_num);
                Ok(FieldValue::Float(value))
            }
        }
        _ => {
            let actual = marshal::type_name(state, index);
            Err(HostError::new(format!("cannot assign a {actual} value to {name}")))
        }
    }
}

/// Where a row's value actually comes from. Most fields live on the full
/// DTO, but a couple (`pal.guild_id`, `pal.base_id`) are computed
/// positionally by `pal_summaries` and have no corresponding DTO field at
/// all -- `Summary` lets a row source from the cached summary instead of
/// forcing every row through the same shape.
pub(crate) enum Reader<T> {
    Dto(fn(&T) -> FieldValue),
    Summary(fn(&PalSummary) -> FieldValue),
}

/// One row of a handle's field table: name, declared type and access for the
/// editor/agreement surface, plus the reader/validator/mutator that make it
/// actually work. Every handle currently has its own concrete DTO, so this is
/// not generic over one -- `pal.rs` is the only implementation today.
pub struct FieldSpec<T> {
    pub name: &'static str,
    pub ty: ApiType,
    pub access: Access,
    pub doc: &'static str,
    pub(crate) read: Reader<T>,
    /// `None` for every `Access::ReadOnly` row. `validate` inspects the
    /// current DTO (some rows, like `pal.is_lucky`, need to see what else is
    /// already set before deciding whether the write is safe) and the
    /// incoming value; `apply` is the pure mutation, only ever called after
    /// `validate` returns `Ok`.
    pub(crate) write: Option<FieldWrite<T>>,
}

pub(crate) struct FieldWrite<T> {
    pub(crate) validate: fn(&T, &FieldValue) -> Result<(), HostError>,
    pub(crate) apply: fn(&mut T, FieldValue),
}
