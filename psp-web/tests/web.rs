#![cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
wasm_bindgen_test_configure!(run_in_browser);

use std::cell::RefCell;
thread_local! {
    static FRAMES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

#[wasm_bindgen_test]
async fn get_version_round_trips_through_the_callback() {
    psp_web::init();
    let cb = wasm_bindgen::closure::Closure::<dyn Fn(String)>::new(|frame: String| {
        FRAMES.with(|f| f.borrow_mut().push(frame));
    });
    psp_web::set_emit_callback(cb.as_ref().unchecked_ref::<js_sys::Function>().clone());
    cb.forget();

    psp_web::dispatch_frame(r#"{"type":"get_version","data":null}"#.to_string())
        .await
        .unwrap();

    let frames = FRAMES.with(|f| f.borrow().clone());
    assert!(frames.iter().any(|f| f.contains("get_version")), "got {frames:?}");
}

/// Bytes in through `stage_gvas`, bytes out through `export_gvas_file` — the
/// GVAS never becomes a string in either direction.
#[wasm_bindgen_test]
async fn world_save_round_trips_through_wasm_as_bytes() {
    psp_web::init();

    let cb = wasm_bindgen::closure::Closure::<dyn Fn(String)>::new(|frame: String| {
        FRAMES.with(|f| f.borrow_mut().push(frame));
    });
    psp_web::set_emit_callback(cb.as_ref().unchecked_ref::<js_sys::Function>().clone());
    cb.forget();
    FRAMES.with(|f| f.borrow_mut().clear());

    let gvas = include_bytes!("fixtures/world1-level.gvas");
    psp_web::stage_gvas("level", "", gvas.to_vec()).unwrap();
    psp_web::load_staged_gvas("world1".to_string()).await.unwrap();

    let frames = FRAMES.with(|f| f.borrow().clone());
    assert!(
        frames.iter().any(|f| f.contains("loaded_save_files")),
        "got {frames:?}"
    );

    let manifest = psp_web::export_gvas_manifest().await.unwrap();
    let names = js_sys::Reflect::get(&manifest, &"names".into()).unwrap();
    let names = js_sys::Array::from(&names);
    assert_eq!(names.get(0).as_string().as_deref(), Some("Level.sav"));

    let out = psp_web::export_gvas_file("Level.sav".to_string()).await.unwrap();
    assert_eq!(out, gvas.to_vec(), "round-trip GVAS is byte-identical");
}

/// wasm32 links no Oodle codec, so `.sav`/`.psp` containers can only be written
/// here with the one the worker lends through this bridge. What is pinned is the
/// marshalling: the payload reaches JS as a `Uint8Array`, the declared
/// uncompressed length rides along as the second argument of the decompress
/// call, and whatever JS returns comes back as the engine's bytes. The stand-in
/// codec is the identity function -- no real Oodle decoder would accept its
/// output, so a read that succeeds proves the bridge was used.
#[wasm_bindgen_test]
async fn oodle_bridge_marshals_bytes_and_length_between_the_engine_and_js() {
    use wasm_bindgen::JsCast;
    let compress = js_sys::Function::new_with_args(
        "data",
        "globalThis.__oodle = { compressed: data.length }; return data;",
    );
    let decompress = js_sys::Function::new_with_args(
        "data, len",
        "globalThis.__oodle.declared = len; return data;",
    );
    psp_web::set_oodle_bridge(compress.unchecked_into(), decompress.unchecked_into());

    let gvas = include_bytes!("fixtures/world1-level.gvas");
    let save = psp_core::savio::read_gvas_bytes(gvas).expect("fixture parses");
    let sav = psp_core::savio::write_sav_bytes(&save).expect("writes through the bridge");

    assert_eq!(&sav[8..11], b"PlM", "the bridge writes Palworld's PlM container");
    let observed = js_sys::Reflect::get(&js_sys::global(), &"__oodle".into()).unwrap();
    assert_eq!(
        js_sys::Reflect::get(&observed, &"compressed".into()).unwrap().as_f64(),
        Some((sav.len() - 12) as f64),
        "js saw the GVAS bytes the engine wrote"
    );

    psp_core::savio::read_sav_bytes(&sav).expect("reads back through the bridge");
    let observed = js_sys::Reflect::get(&js_sys::global(), &"__oodle".into()).unwrap();
    assert_eq!(
        js_sys::Reflect::get(&observed, &"declared".into()).unwrap().as_f64(),
        Some((sav.len() - 12) as f64),
        "js was told how many bytes to produce"
    );
}

#[wasm_bindgen_test]
async fn sql_bridge_marshals_and_migrations_call_through() {
    use wasm_bindgen::JsCast;
    // exec: record calls; return 1. query: return [] for the applied-versions
    // read so run_migrations applies all six.
    let exec = js_sys::Function::new_with_args("sql, params", "globalThis.__execs = globalThis.__execs || []; globalThis.__execs.push(sql); return 1;");
    let query = js_sys::Function::new_with_args("sql, params", "return [];");
    psp_web::set_sql_bridge(exec.clone().unchecked_into(), query.unchecked_into());
    psp_web::run_migrations().await.unwrap();
    let execs = js_sys::Reflect::get(&js_sys::global(), &"__execs".into()).unwrap();
    let arr = js_sys::Array::from(&execs);
    // tracker + 6 migrations + 6 inserts = 13 execute calls.
    assert!(arr.length() >= 13, "expected >=13 execs, got {}", arr.length());
}
