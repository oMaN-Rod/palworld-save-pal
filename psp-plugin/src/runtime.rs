use std::collections::BTreeMap;
use std::ffi::{c_int, CString};

use psp_core::gamedata::GameData;
use psp_core::progress::ProgressSink;
use psp_core::session::SaveSession;
use psp_lua_sys::ffi::*;
use serde_json::Value;

use crate::context::{LogLine, RunContext};
use crate::host;
use crate::host::HostError;
use crate::manifest::{Capability, Manifest};
use crate::sandbox::{Cancel, Limits, Sandbox};
use crate::status::RunStatus;

pub struct RunRequest<'a> {
    pub manifest: &'a Manifest,
    pub sources: &'a BTreeMap<String, String>,
    pub command_id: &'a str,
    pub args: &'a Value,
    pub dry_run: bool,
    pub granted: &'a [Capability],
}

pub struct RunOutcome {
    pub status: RunStatus,
    pub summary: Option<String>,
    /// Populated only under a dry run; a real run always leaves this empty.
    pub counts: BTreeMap<String, i64>,
    pub result: Option<Value>,
    pub log: Vec<LogLine>,
    pub storage_writes: Vec<(String, String)>,
    /// How many pals the DTO cache actually wrote back this run. Host-internal
    /// observability, not plugin output -- kept out of `counts` on purpose.
    pub dto_flush_count: u64,
    /// How many times the run rebuilt the `pals` snapshot. Host-internal
    /// observability alongside `dto_flush_count`, and the only way a test can
    /// see a command reading and writing pals in the same pass.
    pub pal_snapshot_build_count: u64,
}

pub struct RunServices<'a> {
    pub session: &'a mut SaveSession,
    pub game_data: &'a GameData,
    pub progress: Option<&'a ProgressSink>,
    pub storage: &'a BTreeMap<String, String>,
    pub confirm: Option<&'a dyn Fn(&str) -> bool>,
    pub limits: Limits,
    pub cancel: Cancel,
}

fn error_before_context(message: String) -> RunOutcome {
    RunOutcome {
        status: RunStatus::Error(message),
        summary: None,
        counts: BTreeMap::new(),
        result: None,
        log: Vec::new(),
        storage_writes: Vec::new(),
        dto_flush_count: 0,
        pal_snapshot_build_count: 0,
    }
}

fn finish(
    mut ctx: RunContext<'_>,
    status: RunStatus,
    summary: Option<String>,
    result: Option<Value>,
    lifted_counts: BTreeMap<String, i64>,
) -> RunOutcome {
    ctx.counts.extend(lifted_counts);
    RunOutcome {
        status,
        summary,
        counts: ctx.counts,
        result,
        log: ctx.log,
        storage_writes: ctx.storage_writes,
        dto_flush_count: ctx.dto_flush_count,
        pal_snapshot_build_count: ctx.pal_snapshot_build_count,
    }
}

pub fn run_command(request: RunRequest<'_>, services: RunServices<'_>) -> RunOutcome {
    let Some(command) = request.manifest.command(request.command_id) else {
        return error_before_context(format!("unknown command {:?}", request.command_id));
    };

    let coerced_args = match command.coerce_args(request.args) {
        Ok(args) => args,
        Err(error) => return error_before_context(error.to_string()),
    };

    let RunServices { session, game_data, progress, storage, confirm, limits, cancel } = services;

    let Some(mut sandbox) = Sandbox::new(limits, cancel) else {
        return error_before_context("could not create the plugin runtime".to_string());
    };
    sandbox.arm();

    let granted: Vec<Capability> = request
        .granted
        .iter()
        .copied()
        .filter(|capability| request.manifest.capabilities.contains(capability))
        .collect();

    let now = chrono::Utc::now().timestamp();
    let mut ctx = RunContext::new(
        session,
        game_data,
        granted,
        request.dry_run,
        Vec::new(),
        BTreeMap::new(),
        storage.clone(),
        Vec::new(),
        progress,
        confirm,
        request.manifest.api_version,
        request.manifest.id.clone(),
        request.command_id.to_string(),
        now,
        coerced_args,
    );

    // The registry slot must be cleared before `ctx` is consumed below: that
    // ordering is what keeps `host::with_context`'s raw pointer sound.
    unsafe {
        host::set_context(sandbox.as_ptr(), (&mut ctx) as *mut RunContext<'_> as *mut _);
    }

    if let Err(error) = unsafe { host::install_globals(sandbox.as_ptr()) } {
        unsafe { host::clear_context(sandbox.as_ptr()) };
        return finish(ctx, RunStatus::Error(error.into_message()), None, None, BTreeMap::new());
    }

    if let Err(error) = unsafe {
        crate::modules::install(sandbox.as_ptr(), &request.manifest.id, request.sources)
    } {
        unsafe { host::clear_context(sandbox.as_ptr()) };
        return finish(ctx, RunStatus::Error(error.into_message()), None, None, BTreeMap::new());
    }

    let (status, summary, result, lifted_counts) = execute(&mut sandbox, &request);
    let flush = host::flush_dto_cache(&mut ctx);
    let status = host::fold_flush_error(&mut ctx, status, flush);
    unsafe { host::clear_context(sandbox.as_ptr()) };
    finish(ctx, status, summary, result, lifted_counts)
}

fn execute(
    sandbox: &mut Sandbox,
    request: &RunRequest<'_>,
) -> (RunStatus, Option<String>, Option<Value>, BTreeMap<String, i64>) {
    let raw_state = sandbox.as_ptr();

    let Some(source) = request.sources.get(&request.manifest.entry) else {
        return (
            RunStatus::Error(format!("plugin entry {:?} has no source", request.manifest.entry)),
            None,
            None,
            BTreeMap::new(),
        );
    };

    let Ok(chunk_name) = CString::new(format!("={}/{}", request.manifest.id, request.manifest.entry))
    else {
        return (
            RunStatus::Error("the plugin id or entry name contains an embedded NUL byte".to_string()),
            None,
            None,
            BTreeMap::new(),
        );
    };

    unsafe {
        let loaded = luaL_loadbufferx(
            raw_state,
            source.as_ptr().cast(),
            source.len(),
            chunk_name.as_ptr(),
            c"t".as_ptr(),
        );
        if loaded != LUA_OK {
            let status = sandbox.classify_error(loaded);
            return (status, None, None, BTreeMap::new());
        }

        let defined = lua_pcall(raw_state, 0, 0, 0);
        if defined != LUA_OK {
            let status = sandbox.classify_error(defined);
            return (status, None, None, BTreeMap::new());
        }
    }
    drop(chunk_name);

    let Ok(command_name) = CString::new(request.command_id) else {
        return (
            RunStatus::Error("the command id contains an embedded NUL byte".to_string()),
            None,
            None,
            BTreeMap::new(),
        );
    };

    unsafe {
        lua_getglobal(raw_state, command_name.as_ptr());
        drop(command_name);
        if lua_type(raw_state, -1) != LUA_TFUNCTION {
            lua_settop(raw_state, 0);
            return (
                RunStatus::Error(format!(
                    "the plugin declares command {:?} but its script defines no such function",
                    request.command_id
                )),
                None,
                None,
                BTreeMap::new(),
            );
        }

        let called = lua_pcall(raw_state, 0, 1, 0);
        if called != LUA_OK {
            let status = sandbox.classify_error(called);
            return (status, None, None, BTreeMap::new());
        }

        if let Some(status) = sandbox.trip_status() {
            lua_settop(raw_state, 0);
            return (status, None, None, BTreeMap::new());
        }

        let (summary, result, lifted_counts) = match lua_type(raw_state, -1) {
            LUA_TSTRING => (crate::sandbox::read_string(raw_state, -1), None, BTreeMap::new()),
            LUA_TTABLE => match convert_result_table(raw_state) {
                Some(value) => {
                    let (summary, counts) = lift_summary_and_counts(&value);
                    (summary, Some(value), counts)
                }
                None => (None, None, BTreeMap::new()),
            },
            _ => (None, None, BTreeMap::new()),
        };

        lua_settop(raw_state, 0);
        (RunStatus::Ok, summary, result, lifted_counts)
    }
}

/// Needs its own `lua_pcall`: `table_to_json` allocates and can raise, and by
/// here no protected frame is left — an unprotected raise reaches `abort()`.
unsafe fn convert_result_table(state: *mut lua_State) -> Option<Value> {
    // Heap + raw pointer so no live `Drop` value spans the raising calls below;
    // reclaimed as an owned value only after `lua_pcall` returns.
    let out_ptr: *mut Option<Result<Value, HostError>> = Box::into_raw(Box::new(None));

    lua_pushlightuserdata(state, out_ptr.cast());
    lua_pushcclosure(state, convert_result_trampoline, 1);
    lua_pushvalue(state, -2);
    let called = lua_pcall(state, 1, 0, 0);
    let out = *Box::from_raw(out_ptr);
    if called != LUA_OK {
        lua_pop(state, 1);
        return None;
    }

    match out {
        Some(Ok(value)) => Some(value),
        _ => None,
    }
}

unsafe extern "C" fn convert_result_trampoline(state: *mut lua_State) -> c_int {
    let out_ptr = lua_touserdata(state, lua_upvalueindex(1)) as *mut Option<Result<Value, HostError>>;
    let converted = host::marshal::table_to_json(state, 1);
    if !out_ptr.is_null() {
        *out_ptr = Some(converted);
    }
    0
}

/// Reads the converted JSON, not the Lua table, so no metamethod can interpose.
fn lift_summary_and_counts(value: &Value) -> (Option<String>, BTreeMap<String, i64>) {
    let mut counts = BTreeMap::new();
    let mut summary = None;

    if let Value::Object(map) = value {
        if let Some(Value::String(text)) = map.get("summary") {
            summary = Some(text.clone());
        }
        if let Some(Value::Object(entries)) = map.get("counts") {
            for (key, entry) in entries {
                if let Some(count) = entry.as_i64() {
                    counts.insert(key.clone(), count);
                }
            }
        }
    }

    (summary, counts)
}
