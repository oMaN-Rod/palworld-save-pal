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

#[wasm_bindgen_test]
async fn world_save_round_trips_through_wasm() {
    use base64::Engine;
    psp_web::init();

    let cb = wasm_bindgen::closure::Closure::<dyn Fn(String)>::new(|frame: String| {
        FRAMES.with(|f| f.borrow_mut().push(frame));
    });
    psp_web::set_emit_callback(cb.as_ref().unchecked_ref::<js_sys::Function>().clone());
    cb.forget();
    FRAMES.with(|f| f.borrow_mut().clear());

    let gvas = include_bytes!("fixtures/world1-level.gvas");
    let level_b64 = base64::engine::general_purpose::STANDARD.encode(gvas);
    let load = serde_json::json!({
        "type": "load_save_gvas",
        "data": { "save_id": "world1", "level": level_b64, "level_meta": null,
                  "world_option": null, "players": [] }
    });
    psp_web::dispatch_frame(load.to_string()).await.unwrap();
    psp_web::dispatch_frame(r#"{"type":"download_save_gvas","data":null}"#.to_string())
        .await
        .unwrap();

    let frames = FRAMES.with(|f| f.borrow().clone());
    let bundle = frames
        .iter()
        .find_map(|f| {
            let v: serde_json::Value = serde_json::from_str(f).ok()?;
            (v["type"] == "save_gvas_bundle").then_some(v)
        })
        .expect("save_gvas_bundle emitted");
    let out = base64::engine::general_purpose::STANDARD
        .decode(bundle["data"]["level"].as_str().unwrap())
        .unwrap();
    assert_eq!(out, gvas.to_vec(), "round-trip GVAS is byte-identical");
}
