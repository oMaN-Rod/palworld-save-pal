#![cfg(target_arch = "wasm32")]
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
