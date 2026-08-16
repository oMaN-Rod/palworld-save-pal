mod opfs_driver;

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
        driver: Arc::new(opfs_driver::OpfsSqlDriver),
        dialogs: Arc::new(psp_app::desktop_dialogs::NullDialogProvider),
        live_connections,
        ext: Arc::new(NullExtRouter),
        sessions: std::sync::Mutex::new(SessionStore::default()),
        breeding_db: Default::default(),
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

#[wasm_bindgen]
pub fn set_sql_bridge(exec: js_sys::Function, query: js_sys::Function) {
    opfs_driver::SQL_EXEC.with(|c| *c.borrow_mut() = Some(exec));
    opfs_driver::SQL_QUERY.with(|c| *c.borrow_mut() = Some(query));
}

/// Lends the engine the worker's `ooz.wasm` Oodle codec, which wasm32 cannot
/// link for itself. `compress(Uint8Array) -> Uint8Array` and
/// `decompress(Uint8Array, uncompressedLength) -> Uint8Array`, both synchronous:
/// the engine calls them from inside a save encode, so the module behind them
/// must already be up.
#[wasm_bindgen]
pub fn set_oodle_bridge(compress: js_sys::Function, decompress: js_sys::Function) {
    psp_core::oodle::set_bridge(
        move |data| {
            let bytes = js_sys::Uint8Array::from(data);
            call_codec(&compress, &[bytes.into()])
        },
        move |payload, uncompressed_len| {
            let bytes = js_sys::Uint8Array::from(payload);
            call_codec(
                &decompress,
                &[bytes.into(), JsValue::from_f64(uncompressed_len as f64)],
            )
        },
    );
}

/// Applies a JS codec function and takes the `Uint8Array` it returns. A throw
/// and a wrong return type both surface as the engine's own error, since a
/// silently empty payload would be written into a save.
fn call_codec(codec: &js_sys::Function, args: &[JsValue]) -> Result<Vec<u8>, String> {
    let arguments = args.iter().collect::<js_sys::Array>();
    let returned = codec
        .apply(&JsValue::NULL, &arguments)
        .map_err(|error| js_error_message(&error))?;
    returned
        .dyn_into::<js_sys::Uint8Array>()
        .map(|bytes| bytes.to_vec())
        .map_err(|_| "the oodle bridge returned something other than a Uint8Array".to_string())
}

fn js_error_message(error: &JsValue) -> String {
    error
        .as_string()
        .or_else(|| {
            js_sys::Reflect::get(error, &JsValue::from_str("message"))
                .ok()
                .and_then(|message| message.as_string())
        })
        .unwrap_or_else(|| "the oodle bridge threw a non-Error value".to_string())
}

/// Runs the schema migrations through the driver. The worker calls this after
/// `set_sql_bridge` and before dispatching frames.
#[wasm_bindgen]
pub async fn run_migrations() -> Result<(), JsValue> {
    psp_db::run_migrations(&opfs_driver::OpfsSqlDriver)
        .await
        .map_err(|e| JsValue::from_str(&e.to_string()))
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
    // reattach_session / eject_session must NOT run under this session's own
    // guard: they lock the TARGET arc, which for a self-reattach is this very
    // one, and a tokio mutex is not reentrant. They get a scratch session
    // instead and reach the real one through `attachment.arc` — the native ws
    // frame loop makes exactly the same split.
    let holds_own_session_lock = !matches!(
        psp_app::messages::MessageType::from_wire(&envelope.message_type),
        Some(
            psp_app::messages::MessageType::ReattachSession
                | psp_app::messages::MessageType::EjectSession
        )
    );
    // Lock a CLONE of the current arc so the arc slot itself stays free for the
    // attachment's `&mut` — exactly the native ws frame loop's pattern.
    let session_arc = Arc::clone(&state.current);
    let mut scratch = Session::new();
    let mut session_guard = if holds_own_session_lock {
        Some(session_arc.lock().await)
    } else {
        None
    };
    {
        let session: &mut Session = match session_guard.as_mut() {
            Some(guard) => guard,
            None => &mut scratch,
        };
        let ctx = HandlerCtx {
            session,
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
