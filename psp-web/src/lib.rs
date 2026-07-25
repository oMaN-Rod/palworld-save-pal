mod stub_driver;

use std::cell::RefCell;
use std::sync::Arc;

use wasm_bindgen::prelude::*;

use psp_app::blueprint_registry::BlueprintRegistry;
use psp_app::dispatcher::{dispatch, HandlerCtx, NullExtRouter, SessionAttachment};
use psp_app::emitter::Emitter;
use psp_app::envelope::Envelope;
use psp_app::{AppConfig, AppState, SessionStore};
use psp_core::gamedata::GameData;
use psp_core::session::Session;
use uuid::Uuid;

thread_local! {
    static STATE: RefCell<Option<WebState>> = const { RefCell::new(None) };
    static EMIT: RefCell<Option<js_sys::Function>> = const { RefCell::new(None) };
}

// `current` is the session behind an Arc<Mutex> (the store's SharedSession shape),
// so `register_current_session` — which the GVAS load handler calls — has a real
// arc to register. One worker = one session; reattach across reloads is not wired.
struct WebState {
    app: Arc<AppState>,
    current: Arc<tokio::sync::Mutex<Session>>,
    current_id: Option<Uuid>,
    blueprints: BlueprintRegistry,
}

#[wasm_bindgen]
pub fn init() {
    console_error_panic_hook::set_once();
    let (live_connections, _rx) = tokio::sync::watch::channel(0usize);
    // GameData is empty until `init_game_data`; handlers that need it before
    // then simply return empty lists.
    let app = Arc::new(AppState {
        config: AppConfig { desktop_mode: false },
        game_data: Arc::new(GameData::from_entries(Vec::new()).expect("empty game data")),
        driver: Arc::new(stub_driver::StubDriver),
        dialogs: Arc::new(psp_app::desktop_dialogs::NullDialogProvider),
        live_connections,
        ext: Arc::new(NullExtRouter),
        sessions: std::sync::Mutex::new(SessionStore::default()),
    });
    STATE.with(|s| {
        *s.borrow_mut() = Some(WebState {
            app,
            current: Arc::new(tokio::sync::Mutex::new(Session::new())),
            current_id: None,
            blueprints: BlueprintRegistry::default(),
        })
    });
}

#[wasm_bindgen]
pub fn set_emit_callback(cb: js_sys::Function) {
    EMIT.with(|e| *e.borrow_mut() = Some(cb));
}

/// `entries` is a JS array of `[filename, jsonText]` pairs.
#[wasm_bindgen]
pub fn init_game_data(entries: JsValue) -> Result<(), JsValue> {
    let pairs: Vec<(String, String)> = serde_wasm_bindgen::from_value(entries)?;
    let game_data = GameData::from_entries(pairs).map_err(|e| JsValue::from_str(&e.to_string()))?;
    STATE.with(|s| {
        if let Some(state) = s.borrow_mut().as_mut() {
            Arc::get_mut(&mut state.app)
                .expect("app is unshared at init")
                .game_data = Arc::new(game_data);
        }
    });
    Ok(())
}

#[wasm_bindgen]
pub async fn dispatch_frame(frame_json: String) -> Result<(), JsValue> {
    let envelope: Envelope =
        serde_json::from_str(&frame_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let (emitter, mut frames) = Emitter::test_channel();

    // Take the state out to satisfy the borrow checker across the await, then put back.
    let mut state = STATE.with(|s| s.borrow_mut().take()).expect("init() first");
    // Lock a CLONE of the current arc so the arc slot itself stays free for the
    // attachment's `&mut` — exactly the native ws frame loop's pattern.
    let session_arc = Arc::clone(&state.current);
    let mut session_guard = session_arc.lock().await;
    {
        let ctx = HandlerCtx {
            session: &mut session_guard,
            app: &state.app,
            emitter: &emitter,
            blueprints: &mut state.blueprints,
            attachment: Some(SessionAttachment {
                current_id: &mut state.current_id,
                arc: &mut state.current,
            }),
        };
        dispatch(envelope, ctx).await;
    }
    drop(session_guard);
    drop(emitter);
    STATE.with(|s| *s.borrow_mut() = Some(state));

    EMIT.with(|e| {
        if let Some(cb) = e.borrow().as_ref() {
            while let Ok(frame) = frames.try_recv() {
                let _ = cb.call1(&JsValue::NULL, &JsValue::from_str(&frame));
            }
        }
    });
    Ok(())
}
