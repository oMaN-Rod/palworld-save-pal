#![cfg(target_arch = "wasm32")]

use psp_plugin::manifest::{Manifest, Origin};
use psp_plugin::sandbox::{Cancel, Limits, Sandbox};
use psp_plugin::status::RunStatus;
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn a_sandbox_opens_and_runs_a_chunk_on_wasm() {
    let mut sb = Sandbox::new(Limits::default(), Cancel::new()).expect("a sandbox must open");
    assert_eq!(sb.eval("=test", "return tostring(6 * 7)"), RunStatus::Ok);
    assert_eq!(sb.take_return_string().as_deref(), Some("42"));
}

#[wasm_bindgen_test]
fn the_excluded_libraries_are_absent_on_wasm() {
    let mut sb = Sandbox::new(Limits::default(), Cancel::new()).expect("a sandbox must open");
    let probe = "return table.concat({type(io), type(os), type(package), type(debug)}, ',')";
    assert_eq!(sb.eval("=test", probe), RunStatus::Ok);
    assert_eq!(sb.take_return_string().as_deref(), Some("nil,nil,nil,nil"));
}

#[wasm_bindgen_test]
fn the_memory_ceiling_holds_on_wasm() {
    let mut sb = Sandbox::new(
        Limits { memory_bytes: 8 * 1024 * 1024, ..Limits::default() },
        Cancel::new(),
    )
    .expect("a sandbox must open");
    let bomb = "local t = {} while true do t[#t + 1] = string.rep('x', 4096) end";
    assert_eq!(sb.eval("=bomb", bomb), RunStatus::MemoryExceeded);
}

#[wasm_bindgen_test]
fn the_wall_clock_limit_holds_on_wasm() {
    // A hang here (not a failure) means chrono's wasmbind path is inactive and timeouts are inert.
    let mut sb = Sandbox::new(
        Limits { wall_clock_ms: 250, ..Limits::default() },
        Cancel::new(),
    )
    .expect("a sandbox must open");
    assert_eq!(sb.eval("=spin", "while true do end"), RunStatus::Timeout);
}

#[wasm_bindgen_test]
fn a_manifest_parses_on_wasm() {
    let manifest = Manifest::parse(
        r#"{"id":"a.b","api_version":1,"name":"A","version":"1.0.0","entry":"main.lua"}"#,
        Origin::User,
    )
    .expect("parses");
    assert_eq!(manifest.id, "a.b");
}
