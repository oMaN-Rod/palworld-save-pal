//! /ws/{client_id} — port of psp.py:50-60 + ws/manager.py.

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::response::Response;
use futures::{SinkExt, StreamExt};

use uuid::Uuid;

use psp_core::session::Session;

use crate::dispatcher::{dispatch, HandlerCtx, SessionAttachment};
use crate::emitter::Emitter;
use crate::envelope::Envelope;
use crate::messages::MessageType;
use crate::AppState;

/// 1 GB — matches uvicorn ws_max_size=2**30 (psp.py:95); load_zip_file
/// sends a whole zip as a JSON int array.
pub const MAX_WS_MESSAGE_BYTES: usize = 1 << 30;

pub async fn ws_upgrade(
    upgrade: WebSocketUpgrade,
    Path(client_id): Path<String>,
    State(app): State<Arc<AppState>>,
) -> Response {
    upgrade
        .max_message_size(MAX_WS_MESSAGE_BYTES)
        .max_frame_size(MAX_WS_MESSAGE_BYTES)
        .on_upgrade(move |socket| connection_loop(socket, client_id, app))
}

/// Increments `AppState::live_connections` on construction and decrements it
/// when dropped — fires on the normal loop exit below, but also on an early
/// `return` or a panic unwinding through `connection_loop`, which a plain
/// "decrement after the loop" statement would miss. Folding the increment
/// into `new` (rather than leaving it as a separate statement next to
/// construction) makes the 1:1 pairing structural instead of merely
/// positional: a future edit can no longer insert a fallible call between
/// "increment" and "build the guard" and silently break the gauge.
struct LiveConnectionGuard(tokio::sync::watch::Sender<usize>);

impl LiveConnectionGuard {
    fn new(sender: tokio::sync::watch::Sender<usize>) -> Self {
        sender.send_modify(|count| *count += 1);
        Self(sender)
    }
}

impl Drop for LiveConnectionGuard {
    fn drop(&mut self) {
        self.0.send_modify(|count| *count = count.saturating_sub(1));
    }
}

/// psp.py:51-60 websocket_endpoint: accept, loop receiving text frames until
/// disconnect. Each connection owns its own `Session` — unlike Python's single
/// process-wide session, two browser tabs never clobber each other here.
async fn connection_loop(socket: WebSocket, client_id: String, app: Arc<AppState>) {
    tracing::info!(%client_id, "client connected");
    let _live_connection_guard = LiveConnectionGuard::new(app.live_connections.clone());

    let (mut outgoing_sink, mut incoming_stream) = socket.split();
    let (frame_sender, mut frame_receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();

    // Drains the mpsc channel onto the socket so handlers never block on I/O.
    // Exits when the channel closes (all Emitters dropped) or the send fails
    // (client gone) — either way `frame_receiver.recv()` eventually returns
    // `None` or the loop `break`s, so this task always terminates.
    let writer_task = tokio::spawn(async move {
        while let Some(frame) = frame_receiver.recv().await {
            if outgoing_sink.send(frame).await.is_err() {
                break;
            }
        }
    });

    let emitter = Emitter::new(frame_sender);

    // The connection owns ONE session `Arc` slot, reused for every message so
    // per-connection state (a loaded save, gamepass scan results, a transfer
    // source) persists across messages. A load registers this `Arc` in
    // `AppState::sessions` under a fresh id; `reattach_session` can REPLACE the
    // slot with the store's arc for another id (SP-T2), so it is `mut`.
    let mut current_session: Arc<tokio::sync::Mutex<Session>> =
        Arc::new(tokio::sync::Mutex::new(Session::new()));
    let mut current_session_id: Option<Uuid> = None;

    // ws/manager.py's `while True: await websocket.receive_text()` loop.
    // `incoming_stream.next()` returns `None` on a clean disconnect, `Some(Err(_))`
    // on a protocol-level error (e.g. the client vanishing mid-frame without a
    // Close handshake), and handlers run serially (each `process_text_frame` call
    // is awaited before the next frame is read) — so this loop always terminates,
    // on any of the three exit arms below.
    loop {
        match incoming_stream.next().await {
            Some(Ok(Message::Text(text))) => {
                process_text_frame(
                    text.as_str(),
                    &mut current_session,
                    &mut current_session_id,
                    &app,
                    &emitter,
                )
                .await;
            }
            Some(Ok(Message::Close(_))) => break,
            // Ping/pong handled by axum; binary frames are not part of the protocol.
            Some(Ok(_)) => {}
            Some(Err(protocol_error)) => {
                tracing::warn!(%client_id, %protocol_error, "websocket protocol error; closing connection");
                break;
            }
            None => break,
        }
    }

    drop(emitter); // closes the channel → writer task exits
    let _ = writer_task.await;
    tracing::warn!(%client_id, "client disconnected");
}

/// ws/manager.py:31-51 process_message, split into two failure stages.
async fn process_text_frame(
    text: &str,
    current_session: &mut Arc<tokio::sync::Mutex<Session>>,
    current_session_id: &mut Option<Uuid>,
    app: &Arc<AppState>,
    emitter: &Emitter,
) {
    // Stage 1 — JSON parse. ws/manager.py:36-42 sends an `error` message whose
    // data is a plain STRING for decode failures (orjson.JSONDecodeError).
    let raw_value: serde_json::Value = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(parse_error) => {
            tracing::error!(%parse_error, "invalid JSON received");
            emitter.emit(
                MessageType::Error,
                &format!("Invalid JSON received:\n{parse_error}"),
            );
            return;
        }
    };

    // Stage 2 — envelope shape. Python's equivalent failure (KeyError on "type"
    // when message_data["type"] is read, or dispatcher raising on an odd shape)
    // goes through the generic `except Exception` path: ws/manager.py:43-51
    // sends {"message": str(e), "trace": <traceback>} — the OBJECT shape.
    let envelope: Envelope = match serde_json::from_value(raw_value) {
        Ok(envelope) => envelope,
        Err(shape_error) => {
            tracing::error!(%shape_error, "message missing envelope fields");
            emitter.emit_error(&shape_error.to_string(), &format!("{shape_error:?}"));
            return;
        }
    };

    tracing::debug!(message_type = %envelope.message_type, "processing message");

    // reattach_session / eject_session must NOT run under the connection's own
    // per-session guard: they lock a DIFFERENT arc (the target), and holding two
    // per-session guards on one task lets two mutually-reattaching connections
    // deadlock. Hand them a scratch session and let the handler lock at most the
    // single arc it needs via `attachment.arc`.
    let holds_own_session_lock = !matches!(
        MessageType::from_wire(&envelope.message_type),
        Some(MessageType::ReattachSession | MessageType::EjectSession)
    );

    if holds_own_session_lock {
        // Lock a CLONE of the connection's current arc, not the slot itself, so
        // the slot stays mutably free for a reattach swap. The guard is held
        // across the handler's `.await`s (a `tokio::Mutex`), so the map lock
        // never is.
        let session_arc = Arc::clone(current_session);
        let mut session_guard = session_arc.lock().await;
        dispatch(
            envelope,
            HandlerCtx {
                session: &mut session_guard,
                app,
                emitter,
                attachment: Some(SessionAttachment {
                    current_id: current_session_id,
                    arc: current_session,
                }),
            },
        )
        .await;
    } else {
        // No pre-held per-session guard: reattach/eject operate on `attachment`.
        let mut scratch_session = Session::new();
        dispatch(
            envelope,
            HandlerCtx {
                session: &mut scratch_session,
                app,
                emitter,
                attachment: Some(SessionAttachment {
                    current_id: current_session_id,
                    arc: current_session,
                }),
            },
        )
        .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_ws_message_bytes_is_one_gibibyte() {
        // psp.py:95 configures uvicorn with ws_max_size=2**30; `ws_upgrade` wires
        // this same constant to both `.max_message_size(...)` and
        // `.max_frame_size(...)`. Actually sending a >1GB frame in a test is not
        // reachable at reasonable cost, so this pins the value the upgrade call
        // above uses rather than exercising the limit end-to-end.
        assert_eq!(MAX_WS_MESSAGE_BYTES, 1 << 30);
    }
}
