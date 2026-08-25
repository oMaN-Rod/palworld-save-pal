use std::ffi::c_int;

use psp_lua_sys::ffi::*;

use super::api_def::{ApiField, ApiFunction, ApiParam, ApiType};
use super::fields::Access;
use super::marshal::{arg_number, arg_string, check_args, push_str};
use super::{register_table, with_context, HostError, PushHostFn};
use crate::context::{LogLevel, LogLine, RunContext};
use crate::host_fn;
use crate::manifest::ParamValue;

const MAX_LOG_LINES: usize = 1000;

pub(crate) fn append_log_line(ctx: &mut RunContext<'_>, level: LogLevel, message: String) {
    if ctx.log.len() < MAX_LOG_LINES - 1 {
        ctx.log.push(LogLine { level, message });
    } else if ctx.log.len() == MAX_LOG_LINES - 1 {
        ctx.log.push(LogLine {
            level: LogLevel::Warn,
            message: format!("log output truncated after {MAX_LOG_LINES} lines"),
        });
    }
}

fn write_log(state: *mut lua_State, level: LogLevel, name: &str) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 1, name)?;
        let message = arg_string(state, 1, "message")?;
        with_context(state, |ctx| {
            append_log_line(ctx, level, message.clone());
            Ok(())
        })?;
    }
    Ok(0)
}

fn log_info(state: *mut lua_State) -> Result<c_int, HostError> {
    write_log(state, LogLevel::Info, "log.info")
}

fn log_warn(state: *mut lua_State) -> Result<c_int, HostError> {
    write_log(state, LogLevel::Warn, "log.warn")
}

fn log_error(state: *mut lua_State) -> Result<c_int, HostError> {
    write_log(state, LogLevel::Error, "log.error")
}

host_fn!(push_log_info, log_info);
host_fn!(push_log_warn, log_warn);
host_fn!(push_log_error, log_error);

pub const LOG_FUNCTIONS: &[ApiFunction] = &[
    ApiFunction {
        name: "info",
        params: &[ApiParam { name: "message", ty: ApiType::String, optional: false }],
        returns: ApiType::Nil,
        doc: "Appends an info-level line to this run's log, capped at 1000 lines total across \
              log.info/warn/error combined; further calls after the cap are silent no-ops.",
        capability: None,
    },
    ApiFunction {
        name: "warn",
        params: &[ApiParam { name: "message", ty: ApiType::String, optional: false }],
        returns: ApiType::Nil,
        doc: "Appends a warning-level line to this run's log, subject to the same 1000-line cap \
              as log.info.",
        capability: None,
    },
    ApiFunction {
        name: "error",
        params: &[ApiParam { name: "message", ty: ApiType::String, optional: false }],
        returns: ApiType::Nil,
        doc: "Appends an error-level line to this run's log, subject to the same 1000-line cap \
              as log.info. Does not itself abort the run -- raise a Lua error for that.",
        capability: None,
    },
];

const LOG_PUSH_FNS: [PushHostFn; LOG_FUNCTIONS.len()] = [push_log_info, push_log_warn, push_log_error];

fn log_bindings() -> [(&'static str, PushHostFn); LOG_FUNCTIONS.len()] {
    std::array::from_fn(|i| (LOG_FUNCTIONS[i].name, LOG_PUSH_FNS[i]))
}

pub unsafe fn install_log(state: *mut lua_State) {
    register_table(state, "log", &log_bindings());
}

/// The sink is shared with `psp-core`'s own domain calls, so a consumer sees
/// more frames than the script itself reports.
fn progress_report(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 2, "progress.report")?;
        let message = arg_string(state, 1, "message")?;
        let fraction = match lua_type(state, 2) {
            LUA_TNONE | LUA_TNIL => None,
            _ => Some(arg_number(state, 2, "fraction")?),
        };
        if let Some(value) = fraction {
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(HostError::new(
                    "fraction must be a finite number between 0.0 and 1.0",
                ));
            }
        }
        with_context(state, |ctx| {
            if let Some(sink) = ctx.progress {
                let text = match fraction {
                    Some(value) => format!("{message} ({:.0}%)", value * 100.0),
                    None => message.clone(),
                };
                sink(&text);
            }
            Ok(())
        })?;
    }
    Ok(0)
}

host_fn!(push_progress_report, progress_report);

pub const PROGRESS_FUNCTIONS: &[ApiFunction] = &[ApiFunction {
    name: "report",
    params: &[
        ApiParam { name: "message", ty: ApiType::String, optional: false },
        ApiParam { name: "fraction", ty: ApiType::Number, optional: true },
    ],
    returns: ApiType::Nil,
    doc: "Sends message (with an optional 0.0-1.0 completion fraction) to whatever is driving \
          this run's progress UI, if anything is listening. The same sink also receives the \
          host's own internal progress ticks from destructive domain calls this script \
          triggers, so a listener can see more updates than the script explicitly sent.",
    capability: None,
}];

const PROGRESS_PUSH_FNS: [PushHostFn; PROGRESS_FUNCTIONS.len()] = [push_progress_report];

fn progress_bindings() -> [(&'static str, PushHostFn); PROGRESS_FUNCTIONS.len()] {
    std::array::from_fn(|i| (PROGRESS_FUNCTIONS[i].name, PROGRESS_PUSH_FNS[i]))
}

pub unsafe fn install_progress(state: *mut lua_State) {
    register_table(state, "progress", &progress_bindings());
}

fn ui_confirm(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 1, "ui.confirm")?;
        let message = arg_string(state, 1, "message")?;
        let confirmed = with_context(state, |ctx| {
            if ctx.dry_run {
                return Ok(true);
            }
            Ok(ctx.confirm.map(|callback| callback(&message)).unwrap_or(false))
        })?;
        lua_pushboolean(state, c_int::from(confirmed));
    }
    Ok(1)
}

host_fn!(push_ui_confirm, ui_confirm);

pub const UI_FUNCTIONS: &[ApiFunction] = &[ApiFunction {
    name: "confirm",
    params: &[ApiParam { name: "message", ty: ApiType::String, optional: false }],
    returns: ApiType::Boolean,
    doc: "Shows message as a confirm dialog and returns whether the user accepted it. Under a \
          dry run this always returns true without showing anything, so a dry run can predict \
          the confirmed path. If nothing is listening for confirmations, this returns false.",
    capability: None,
}];

const UI_PUSH_FNS: [PushHostFn; UI_FUNCTIONS.len()] = [push_ui_confirm];

fn ui_bindings() -> [(&'static str, PushHostFn); UI_FUNCTIONS.len()] {
    std::array::from_fn(|i| (UI_FUNCTIONS[i].name, UI_PUSH_FNS[i]))
}

pub unsafe fn install_ui(state: *mut lua_State) {
    register_table(state, "ui", &ui_bindings());
}

const MAX_STORAGE_KEY_BYTES: usize = 128;
const MAX_STORAGE_VALUE_BYTES: usize = 64 * 1024;

fn storage_get(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 1, "storage.get")?;
        let key = arg_string(state, 1, "key")?;
        let value = with_context(state, |ctx| Ok(ctx.storage.get(&key).cloned()))?;
        match value {
            Some(text) => push_str(state, &text),
            None => lua_pushnil(state),
        }
    }
    Ok(1)
}

fn storage_set(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 2, "storage.set")?;
        let key = arg_string(state, 1, "key")?;
        let value = arg_string(state, 2, "value")?;
        if key.len() > MAX_STORAGE_KEY_BYTES {
            return Err(HostError::new(format!(
                "storage key must be at most {MAX_STORAGE_KEY_BYTES} bytes"
            )));
        }
        if value.len() > MAX_STORAGE_VALUE_BYTES {
            return Err(HostError::new(format!(
                "storage value must be at most {MAX_STORAGE_VALUE_BYTES} bytes"
            )));
        }
        with_context(state, |ctx| {
            ctx.storage.insert(key.clone(), value.clone());
            ctx.storage_writes.push((key.clone(), value.clone()));
            Ok(())
        })?;
    }
    Ok(0)
}

host_fn!(push_storage_get, storage_get);
host_fn!(push_storage_set, storage_set);

pub const STORAGE_FUNCTIONS: &[ApiFunction] = &[
    ApiFunction {
        name: "get",
        params: &[ApiParam { name: "key", ty: ApiType::String, optional: false }],
        returns: ApiType::Union(&[ApiType::String, ApiType::Nil]),
        doc: "The value previously stored under key by this plugin, or nil if nothing has been \
              stored under it. Storage is private per plugin.",
        capability: None,
    },
    ApiFunction {
        name: "set",
        params: &[
            ApiParam { name: "key", ty: ApiType::String, optional: false },
            ApiParam { name: "value", ty: ApiType::String, optional: false },
        ],
        returns: ApiType::Nil,
        doc: "Stores value under key for this plugin, visible to storage.get for the rest of \
              this run immediately and to later runs once the host persists it. Raises if key \
              exceeds 128 bytes or value exceeds 64 KiB. Unlike every save/raw write function, \
              this is NOT skipped under a dry run -- it always writes.",
        capability: None,
    },
];

const STORAGE_PUSH_FNS: [PushHostFn; STORAGE_FUNCTIONS.len()] = [push_storage_get, push_storage_set];

fn storage_bindings() -> [(&'static str, PushHostFn); STORAGE_FUNCTIONS.len()] {
    std::array::from_fn(|i| (STORAGE_FUNCTIONS[i].name, STORAGE_PUSH_FNS[i]))
}

pub unsafe fn install_storage(state: *mut lua_State) {
    register_table(state, "storage", &storage_bindings());
}

unsafe fn push_field(state: *mut lua_State, name: &'static std::ffi::CStr, push: impl FnOnce(*mut lua_State)) {
    push(state);
    lua_setfield(state, -2, name.as_ptr());
}

pub const CTX_FIELDS: &[ApiField] = &[
    ApiField {
        name: "dry_run",
        ty: ApiType::Boolean,
        access: Access::ReadOnly,
        doc: "Whether this run is a dry run: every write function predicts its effect and \
              records a preview count instead of writing.",
    },
    ApiField {
        name: "api_version",
        ty: ApiType::Integer,
        access: Access::ReadOnly,
        doc: "The api_version this plugin's manifest declares.",
    },
    ApiField {
        name: "plugin_id",
        ty: ApiType::String,
        access: Access::ReadOnly,
        doc: "This plugin's own id, from its manifest.",
    },
    ApiField {
        name: "command_id",
        ty: ApiType::String,
        access: Access::ReadOnly,
        doc: "The id of the command this run is executing.",
    },
    ApiField {
        name: "now",
        ty: ApiType::Integer,
        access: Access::ReadOnly,
        doc: "The Unix timestamp, in seconds, of when this run started.",
    },
    ApiField {
        name: "args",
        ty: ApiType::Table,
        access: Access::ReadOnly,
        doc: "This command's arguments, already coerced to the types its manifest declares, \
              keyed by parameter name.",
    },
];

/// Every `ctx` field is `Access::ReadOnly`, and unlike a handle's fields these
/// live on a table, where an assignment would otherwise just succeed and leave
/// the script reading back a value the run never had. A key that is not one of
/// the six is a different mistake and is reported as one: calling it read-only
/// would send an author hunting for a getter that does not exist either.
fn ctx_newindex(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        let Ok(field) = arg_string(state, 2, "field") else {
            return Err(HostError::new("ctx has no field under a non-string key"));
        };
        if CTX_FIELDS.iter().any(|described| described.name == field) {
            return Err(HostError::new(format!("ctx.{field} is read-only")));
        }
        Err(HostError::new(format!("ctx has no field {field:?}")))
    }
}

host_fn!(push_ctx_newindex, ctx_newindex);

/// `__newindex` only fires for a key the table does not already hold, so the
/// values live on a backing table behind an empty proxy. That hides them from
/// `pairs` too, which this hands back: upvalue 1 is the backing table, upvalue
/// 2 the `next` captured at install time, so reassigning the global cannot
/// change what a later `pairs(ctx)` walks.
unsafe extern "C" fn ctx_pairs(state: *mut lua_State) -> c_int {
    lua_pushvalue(state, lua_upvalueindex(2));
    lua_pushvalue(state, lua_upvalueindex(1));
    lua_pushnil(state);
    3
}

pub unsafe fn install_ctx(state: *mut lua_State, ctx: &RunContext<'_>) {
    lua_createtable(state, 0, 6);

    push_field(state, c"dry_run", |state| unsafe {
        lua_pushboolean(state, c_int::from(ctx.dry_run));
    });
    push_field(state, c"api_version", |state| unsafe {
        lua_pushinteger(state, i64::from(ctx.api_version));
    });
    push_field(state, c"plugin_id", |state| unsafe {
        push_str(state, &ctx.plugin_id);
    });
    push_field(state, c"command_id", |state| unsafe {
        push_str(state, &ctx.command_id);
    });
    push_field(state, c"now", |state| unsafe {
        lua_pushinteger(state, ctx.now);
    });
    push_field(state, c"args", |state| unsafe {
        lua_createtable(state, 0, ctx.args.len() as c_int);
        for (key, value) in &ctx.args {
            match value {
                ParamValue::Int(int) => lua_pushinteger(state, *int),
                ParamValue::Float(float) => lua_pushnumber(state, *float),
                ParamValue::Text(text) => push_str(state, text),
                ParamValue::Bool(flag) => lua_pushboolean(state, c_int::from(*flag)),
                ParamValue::List(items) => {
                    lua_createtable(state, items.len() as c_int, 0);
                    for (index, item) in items.iter().enumerate() {
                        push_str(state, item);
                        lua_rawseti(state, -2, i64::try_from(index).unwrap_or(i64::MAX).saturating_add(1));
                    }
                }
            }
            let Ok(field) = std::ffi::CString::new(key.as_str()) else {
                lua_pop(state, 1);
                continue;
            };
            lua_setfield(state, -2, field.as_ptr());
            drop(field);
        }
    });

    // The proxy is genuinely empty, so the two raw read shapes see an empty
    // table instead of raising: `rawget(ctx, k)` is nil, and a hand-rolled
    // `next(ctx)` walk yields nothing. `pairs` is restored by `__pairs` below;
    // Lua 5.4 has no `__next`, so those two cannot be. `rawset` is the same
    // hole in the other direction -- it plants the key on the proxy itself,
    // after which plain assignment to that key stops reaching `__newindex`
    // while `pairs` still reports the backing value. Accepted rather than
    // closed: sealing it means making `ctx` a userdata, which would change
    // `type(ctx)` for every plugin author.
    lua_createtable(state, 0, 0);
    lua_createtable(state, 0, 4);

    lua_pushvalue(state, -3);
    lua_setfield(state, -2, c"__index".as_ptr());
    push_ctx_newindex(state);
    lua_setfield(state, -2, c"__newindex".as_ptr());
    lua_pushvalue(state, -3);
    lua_getglobal(state, c"next".as_ptr());
    lua_pushcclosure(state, ctx_pairs, 2);
    lua_setfield(state, -2, c"__pairs".as_ptr());
    // Without this the guard is one `setmetatable(ctx, nil)` away from gone.
    lua_pushboolean(state, 1);
    lua_setfield(state, -2, c"__metatable".as_ptr());

    lua_setmetatable(state, -2);
    lua_setglobal(state, c"ctx".as_ptr());
    lua_pop(state, 1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_names_match_described_functions_in_order() {
        let log_bound: Vec<&str> = log_bindings().iter().map(|(name, _)| *name).collect();
        let log_described: Vec<&str> = LOG_FUNCTIONS.iter().map(|f| f.name).collect();
        assert_eq!(log_bound, log_described);

        let progress_bound: Vec<&str> = progress_bindings().iter().map(|(name, _)| *name).collect();
        let progress_described: Vec<&str> = PROGRESS_FUNCTIONS.iter().map(|f| f.name).collect();
        assert_eq!(progress_bound, progress_described);

        let ui_bound: Vec<&str> = ui_bindings().iter().map(|(name, _)| *name).collect();
        let ui_described: Vec<&str> = UI_FUNCTIONS.iter().map(|f| f.name).collect();
        assert_eq!(ui_bound, ui_described);

        let storage_bound: Vec<&str> = storage_bindings().iter().map(|(name, _)| *name).collect();
        let storage_described: Vec<&str> = STORAGE_FUNCTIONS.iter().map(|f| f.name).collect();
        assert_eq!(storage_bound, storage_described);
    }
}
