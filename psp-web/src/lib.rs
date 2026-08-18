mod opfs_driver;

use std::cell::RefCell;
use std::sync::Arc;

use wasm_bindgen::prelude::*;

use psp_app::blueprint_registry::BlueprintRegistry;
use psp_app::dispatcher::{dispatch, HandlerCtx, NullExtRouter, SessionAttachment};
use psp_app::emitter::Emitter;
use psp_app::envelope::Envelope;
use psp_app::handlers::web_save::{handle_load_save_gvas_bytes, LoadSaveGvasBytes, StagedGvas};
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
        config: AppConfig {
            desktop_mode: false,
        },
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
    run_with_ctx(Op::Frame(envelope)).await
}

// The save's GVAS files, accumulated across `stage_gvas` calls and consumed by
// `load_staged_gvas`. Kept out of the frame path deliberately: these buffers
// run to hundreds of megabytes, which is past what a JS string — and therefore
// a JSON frame — can carry at all.
thread_local! {
    static STAGED: RefCell<StagedGvas> = RefCell::new(StagedGvas::default());
}

/// `slot` is one of `level`, `level_meta`, `world_option`, `player_sav`,
/// `player_dps`; `uid` is ignored except for the two player slots. `bytes`
/// arrives as a `Uint8Array` and is moved straight into the staging area, so
/// the caller can drop its own copy immediately.
#[wasm_bindgen]
pub fn stage_gvas(slot: &str, uid: &str, bytes: Vec<u8>) -> Result<(), JsValue> {
    STAGED.with(|staged| {
        staged
            .borrow_mut()
            .stage(slot, uid, bytes)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    })
}

/// Loads everything staged so far and empties the staging area.
#[wasm_bindgen]
pub async fn load_staged_gvas(save_id: String) -> Result<(), JsValue> {
    let payload = STAGED
        .with(|staged| staged.borrow_mut().take(save_id))
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    run_with_ctx(Op::LoadStaged(Box::new(payload))).await
}

/// `{ world_name, names: string[] }` — the download zip's file list. Small by
/// construction; the bytes come one at a time from `export_gvas_file`.
#[wasm_bindgen]
pub async fn export_gvas_manifest() -> Result<JsValue, JsValue> {
    with_session(|session| {
        let manifest = psp_app::handlers::web_save::export_manifest(session);
        let out = js_sys::Object::new();
        js_sys::Reflect::set(&out, &"world_name".into(), &manifest.world_name.into())?;
        let names = manifest
            .names
            .into_iter()
            .map(JsValue::from)
            .collect::<js_sys::Array>();
        js_sys::Reflect::set(&out, &"names".into(), &names)?;
        Ok(out.into())
    })
    .await
}

/// Serializes one manifest entry and hands it over as a `Uint8Array`.
#[wasm_bindgen]
pub async fn export_gvas_file(name: String) -> Result<Vec<u8>, JsValue> {
    with_session(|session| {
        psp_app::handlers::web_save::export_file(session, &name)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    })
    .await
}

/// Borrows the loaded save. `STATE` is put back on every path — an early
/// return between the take and the restore would leave the module permanently
/// uninitialized for every later call.
async fn with_session<T>(
    f: impl FnOnce(&psp_core::session::SaveSession) -> Result<T, JsValue>,
) -> Result<T, JsValue> {
    let state = STATE.with(|s| s.borrow_mut().take()).expect("init() first");
    let session_arc = Arc::clone(&state.current);
    let result = {
        let guard = session_arc.lock().await;
        match guard.save.as_ref() {
            Some(session) => f(session),
            None => Err(JsValue::from_str("No save file loaded")),
        }
    };
    STATE.with(|s| *s.borrow_mut() = Some(state));
    result
}

enum Op {
    Frame(Envelope),
    LoadStaged(Box<LoadSaveGvasBytes>),
}

async fn run_with_ctx(op: Op) -> Result<(), JsValue> {
    let (emitter, mut frames) = Emitter::test_channel();

    // Take the state out to satisfy the borrow checker across the await, then put back.
    let mut state = STATE.with(|s| s.borrow_mut().take()).expect("init() first");
    // reattach_session / eject_session must NOT run under this session's own
    // guard: they lock the TARGET arc, which for a self-reattach is this very
    // one, and a tokio mutex is not reentrant. They get a scratch session
    // instead and reach the real one through `attachment.arc` — the native ws
    // frame loop makes exactly the same split.
    let holds_own_session_lock = match &op {
        Op::Frame(envelope) => !matches!(
            psp_app::messages::MessageType::from_wire(&envelope.message_type),
            Some(
                psp_app::messages::MessageType::ReattachSession
                    | psp_app::messages::MessageType::EjectSession
            )
        ),
        Op::LoadStaged(_) => true,
    };
    // Lock a CLONE of the current arc so the arc slot itself stays free for the
    // attachment's `&mut` — exactly the native ws frame loop's pattern.
    let session_arc = Arc::clone(&state.current);
    let mut scratch = Session::new();
    let mut session_guard = if holds_own_session_lock {
        Some(session_arc.lock().await)
    } else {
        None
    };
    let mut outcome = Ok(());
    {
        let session: &mut Session = match session_guard.as_mut() {
            Some(guard) => guard,
            None => &mut scratch,
        };
        let mut ctx = HandlerCtx {
            session,
            app: &state.app,
            emitter: &emitter,
            blueprints: &mut state.blueprints,
            attachment: Some(SessionAttachment {
                current_id: &mut state.current_id,
                arc: &mut state.current,
            }),
        };
        match op {
            // `dispatch` reports handler failures as `error` frames, not as a
            // Result, so its own frames carry the outcome.
            Op::Frame(envelope) => dispatch(envelope, ctx).await,
            Op::LoadStaged(payload) => {
                outcome = handle_load_save_gvas_bytes(*payload, &mut ctx)
                    .await
                    .map_err(|e| JsValue::from_str(&e.to_string()));
            }
        }
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
    outcome
}
