use std::ffi::c_int;

use psp_lua_sys::ffi::*;
use psp_plugin::host::marshal::{arg_integer, arg_string, push_str, table_to_json};
use psp_plugin::host::{host_fn, register_table, HostError};
use psp_plugin::sandbox::{Cancel, Limits, Sandbox};
use psp_plugin::status::RunStatus;

fn double_impl(state: *mut lua_State) -> Result<c_int, HostError> {
    let n = unsafe { arg_integer(state, 1, "n") }?;
    let doubled = n.checked_mul(2).ok_or_else(|| HostError::new("n is too large to double"))?;
    unsafe { lua_pushinteger(state, doubled) };
    Ok(1)
}

fn shout_impl(state: *mut lua_State) -> Result<c_int, HostError> {
    let text = unsafe { arg_string(state, 1, "text") }?;
    unsafe { push_str(state, &text.to_uppercase()) };
    Ok(1)
}

fn describe_impl(state: *mut lua_State) -> Result<c_int, HostError> {
    let value = unsafe { table_to_json(state, 1) }?;
    unsafe { push_str(state, &value.to_string()) };
    Ok(1)
}

host_fn!(double, double_impl);
host_fn!(shout, shout_impl);
host_fn!(describe, describe_impl);

fn probe(source: &str) -> (RunStatus, Option<String>) {
    let mut sb = Sandbox::new(Limits::default(), Cancel::new()).expect("a sandbox must open");
    unsafe {
        register_table(
            sb.as_ptr(),
            "probe",
            &[("double", double), ("shout", shout), ("describe", describe)],
        );
    }
    let status = sb.eval("=probe", source);
    let returned = sb.take_return_string();
    (status, returned)
}

#[test]
fn a_host_function_returns_its_value_to_lua() {
    let (status, value) = probe("return tostring(probe.double(21))");
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("42"));
}

#[test]
fn a_host_error_becomes_a_catchable_lua_error() {
    let (status, value) = probe(
        "local ok, err = pcall(probe.double, 'not a number') return tostring(ok) .. '|' .. tostring(err)",
    );
    assert_eq!(status, RunStatus::Ok);
    let value = value.expect("the chunk returns a string");
    assert!(value.starts_with("false|"), "got {value}");
    assert!(value.contains("n"), "the message should name the argument: {value}");
}

#[test]
fn an_overflowing_argument_errors_instead_of_wrapping() {
    let (status, value) = probe(
        "local ok, err = pcall(probe.double, math.maxinteger) return tostring(ok)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("false"));
}

#[test]
fn a_missing_argument_errors_rather_than_reading_garbage() {
    let (status, value) = probe("local ok = pcall(probe.double) return tostring(ok)");
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("false"));
}

#[test]
fn strings_round_trip_through_the_boundary() {
    let (status, value) = probe("return probe.shout('héllo wörld')");
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("HÉLLO WÖRLD"));
}

#[test]
fn an_embedded_nul_byte_does_not_truncate_a_string() {
    let (status, value) = probe("return tostring(#probe.shout('a\\0b'))");
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("3"));
}

#[test]
fn a_nested_table_converts_to_json() {
    let (status, value) = probe("return probe.describe({ a = 1, b = { true, 'x' } })");
    assert_eq!(status, RunStatus::Ok);
    let json: serde_json::Value =
        serde_json::from_str(&value.expect("a string comes back")).expect("valid JSON");
    assert_eq!(json["a"], serde_json::json!(1));
    assert_eq!(json["b"], serde_json::json!([true, "x"]));
}

#[test]
fn a_self_referential_table_is_refused_rather_than_looping_forever() {
    let (status, value) = probe(
        "local t = {} t.self = t local ok, err = pcall(probe.describe, t) return tostring(ok)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("false"));
}

#[test]
fn a_pathologically_deep_table_is_refused() {
    let build = "local t = {} local c = t for _ = 1, 500 do c.next = {} c = c.next end \
                 local ok = pcall(probe.describe, t) return tostring(ok)";
    let (status, value) = probe(build);
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("false"));
}

#[test]
fn a_table_nested_past_the_lua_stack_guarantee_still_converts_cleanly() {
    // 25 stays under MAX_TABLE_DEPTH (32); it instead exceeds LUA_MINSTACK (20), the free stack slots Lua guarantees without lua_checkstack.
    let build = "local t = {} local c = t for _ = 1, 25 do c.next = {} c = c.next end \
                 local ok, err = pcall(probe.describe, t) return tostring(ok) .. '|' .. tostring(err)";
    let (status, value) = probe(build);
    assert_eq!(status, RunStatus::Ok);
    let value = value.expect("a string comes back");
    assert!(value.starts_with("true|"), "got {value}");
}

#[test]
fn a_pathologically_wide_table_is_refused() {
    let build = "local t = {} for i = 1, 200000 do t[i] = i end \
                 local ok = pcall(probe.describe, t) return tostring(ok)";
    let (status, value) = probe(build);
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("false"));
}

#[test]
fn every_host_function_survives_every_lua_type_as_every_argument() {
    let hostile = "local values = { nil, true, 0, -1, 1/0, -1/0, 0/0, math.maxinteger, \
                   math.mininteger, '', 'x', {}, print, coroutine.create(function() end) } \
                   for _, fn in pairs({ probe.double, probe.shout, probe.describe }) do \
                     for i = 1, 14 do pcall(fn, values[i]) end \
                     for i = 1, 14 do for j = 1, 14 do pcall(fn, values[i], values[j]) end end \
                   end \
                   return 'survived'";
    let (status, value) = probe(hostile);
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("survived"));
}

#[test]
fn the_stack_is_left_balanced_after_thousands_of_calls() {
    let (status, value) = probe(
        "for i = 1, 20000 do probe.double(i) probe.shout('x') end return 'balanced'",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("balanced"));
}
