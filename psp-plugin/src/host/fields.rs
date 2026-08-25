use std::ffi::c_int;

use psp_core::dto::ordered_map::OrderedMap;
use psp_core::gamedata::GameData;
use psp_lua_sys::ffi::*;
use serde::Serialize;

use super::api_def::ApiType;
use super::{marshal, HostError};

pub mod base;
pub mod container;
pub mod guild;
pub mod pal;
pub mod player;
pub mod slot;

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

pub(crate) fn expect_int(name: &str, value: &FieldValue) -> Result<i64, HostError> {
    match value {
        FieldValue::Int(v) => Ok(*v),
        other => {
            Err(HostError::new(format!("expected an integer for {name}, got {}", field_value_type_name(other))))
        }
    }
}

pub(crate) fn expect_bool(name: &str, value: &FieldValue) -> Result<bool, HostError> {
    match value {
        FieldValue::Bool(v) => Ok(*v),
        other => {
            Err(HostError::new(format!("expected a boolean for {name}, got {}", field_value_type_name(other))))
        }
    }
}

pub(crate) fn expect_str<'v>(name: &str, value: &'v FieldValue) -> Result<&'v str, HostError> {
    match value {
        FieldValue::Str(v) => Ok(v.as_str()),
        other => {
            Err(HostError::new(format!("expected a string for {name}, got {}", field_value_type_name(other))))
        }
    }
}

pub(crate) fn expect_list<'v>(name: &str, value: &'v FieldValue) -> Result<&'v [String], HostError> {
    match value {
        FieldValue::List(items) => Ok(items.as_slice()),
        other => Err(HostError::new(format!(
            "expected a list of strings for {name}, got {}",
            field_value_type_name(other)
        ))),
    }
}

/// For a row the save persists as a 32-bit float. A value outside that window
/// becomes an infinity on the way in rather than the number that was assigned,
/// and a NaN stays a NaN, so both are refused here instead.
pub(crate) fn expect_finite_f32(name: &str, value: &FieldValue) -> Result<f64, HostError> {
    let number = match value {
        FieldValue::Float(v) => *v,
        FieldValue::Int(v) => *v as f64,
        other => {
            return Err(HostError::new(format!(
                "expected a number for {name}, got {}",
                field_value_type_name(other)
            )))
        }
    };
    if !number.is_finite() || number < f64::from(f32::MIN) || number > f64::from(f32::MAX) {
        return Err(HostError::new(format!(
            "{name} must be a finite number the save can hold as a 32-bit float, got {number}"
        )));
    }
    Ok(number)
}

pub(crate) fn ranged_int(name: &str, value: &FieldValue, lo: i64, hi: i64) -> Result<i64, HostError> {
    let v = expect_int(name, value)?;
    if !(lo..=hi).contains(&v) {
        return Err(HostError::new(format!("{name} must be between {lo} and {hi}, got {v}")));
    }
    Ok(v)
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
/// forcing every row through the same shape. `player`'s summary rows have a
/// second reason to take it: building a `PlayerDto` at all costs a lazy load
/// of that player's own `.sav` from disk, which a row the summary already
/// answers must not pay.
pub(crate) enum Reader<D, S> {
    Dto(fn(&D) -> FieldValue),
    Summary(fn(&S) -> FieldValue),
}

/// One row of a handle's field table: name, declared type and access for the
/// editor/agreement surface, plus the reader/validator/mutator that make it
/// actually work. `D` is the handle's full DTO, `S` the summary its cheap
/// reads come from; the two vary together per handle.
pub struct FieldSpec<D, S> {
    pub name: &'static str,
    pub ty: ApiType,
    pub access: Access,
    pub doc: &'static str,
    pub(crate) read: Reader<D, S>,
    /// For a row that cannot be assigned but does have a working alternative:
    /// the call to name in the refusal. `container.slot_count` is the case it
    /// exists for -- resizing a container is structural, so it stays behind
    /// `set_slot_count`, and an author who reaches for the assignment has to be
    /// told the operation that works rather than only that this one does not.
    /// `None` on every ordinary read-only row, which keeps the plain message.
    pub(crate) instead_call: Option<&'static str>,
    /// `None` for every `Access::ReadOnly` row. `validate` inspects the
    /// current DTO (some rows, like `pal.is_lucky`, need to see what else is
    /// already set before deciding whether the write is safe) and the
    /// incoming value; `apply` is the pure mutation, only ever called after
    /// `validate` returns `Ok`.
    pub(crate) write: Option<FieldWrite<D>>,
    /// A check the row needs that its own `validate` cannot make, run before
    /// `validate`. `pal.is_lucky` is the case it exists for: its catalog-
    /// availability refusal has to pre-empt the row's ordinary species-name
    /// refusal, whose answer is unreliable precisely when the catalog is the
    /// thing that is missing.
    pub(crate) game_data_precheck: Option<GameDataCheck<D>>,
    /// The same, run after `validate`. `slot.item_id` is the case it exists
    /// for: the row's own refusals name the operation to use instead, and a
    /// catalog check running first would replace them with a bare "not in the
    /// catalog" for `"None"` and the empty string.
    pub(crate) game_data_postcheck: Option<GameDataCheck<D>>,
}

/// A row's extra validator, for the checks `FieldWrite::validate` cannot make
/// because it sees only the DTO and the catalogs live on `GameData`. Takes the
/// row's own name so a message can quote the field without the row's name being
/// written out a second time inside the validator.
///
/// Carried on the row, not selected by comparing the row's name to a string.
/// A string-matched validator detaches silently when the row is renamed, and a
/// detached validator does not fail loudly -- it stops validating, and the save
/// takes the value it would have refused. Every other correspondence on these
/// handles was replaced with a derived one for exactly that reason.
pub(crate) type GameDataCheck<D> =
    fn(&GameData, &'static str, &D, &FieldValue) -> Result<(), HostError>;

impl<D, S> FieldSpec<D, S> {
    /// An empty Lua table is an empty list and an empty map at once, and
    /// `marshal::read_collection` cannot tell them apart -- it reports
    /// `List(vec![])`. The row's declared type can, so the correction lives
    /// here beside `validate_write` rather than on whichever handles happen to
    /// carry a `Map` row today: a `Map` row added to a handle that has none
    /// would otherwise refuse `{}` with a type error naming neither cause.
    ///
    /// Every other value passes through untouched.
    pub(crate) fn coerce_empty_table(&self, value: FieldValue) -> FieldValue {
        match (&self.ty, value) {
            (ApiType::Map { .. }, FieldValue::List(items)) if items.is_empty() => {
                FieldValue::Map(OrderedMap::new())
            }
            (_, other) => other,
        }
    }

    /// Everything that has to pass before a row's `apply` may run: the row's
    /// game-data precheck, its own `validate`, then its postcheck.
    ///
    /// One call rather than three, so a handle cannot run the row's `validate`
    /// while silently skipping the checks the row carries -- which is the same
    /// failure the checks were moved onto the row to prevent, one level up.
    pub(crate) fn validate_write(
        &self,
        game_data: &GameData,
        current: &D,
        value: &FieldValue,
    ) -> Result<(), HostError> {
        if let Some(check) = self.game_data_precheck {
            check(game_data, self.name, current, value)?;
        }
        if let Some(write) = self.write.as_ref() {
            (write.validate)(current, value)?;
        }
        if let Some(check) = self.game_data_postcheck {
            check(game_data, self.name, current, value)?;
        }
        Ok(())
    }

    /// The refusal for an assignment to a row that has no write. Structural
    /// rows name the operation that does work; every other read-only row keeps
    /// the plain wording the four earlier handles already report.
    pub(crate) fn not_assignable(&self) -> HostError {
        match self.instead_call {
            Some(call) => HostError::new(format!(
                "{} cannot be assigned: changing it is structural and invalidates every live \
                 handle and iterator, so it stays a call -- use {call} instead",
                self.name
            )),
            None => HostError::new(format!("{} is read-only", self.name)),
        }
    }
}

pub(crate) struct FieldWrite<D> {
    pub(crate) validate: fn(&D, &FieldValue) -> Result<(), HostError>,
    pub(crate) apply: fn(&mut D, FieldValue),
}
