//! The /ws/{client_id} endpoint: one connection loop per client.

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

/// 1 GiB, applied to both the message and frame limit. It has to be this large
/// because payloads carry whole parsed saves, and `load_zip_file` sends an
/// entire zip as a JSON int array.
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

/// Increments `AppState::live_connections` on construction and decrements it on
/// drop, so the gauge also unwinds on an early `return` or a panic inside
/// `connection_loop`. The increment lives in `new` so the pairing is structural:
/// no edit can slip a fallible call between "increment" and "build the guard".
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

/// Receives text frames until the client disconnects. Each connection owns its
/// own `Session`, so two browser tabs never clobber each other.
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
    // slot with the store's arc for another id, so it is `mut`.
    let mut current_session: Arc<tokio::sync::Mutex<Session>> =
        Arc::new(tokio::sync::Mutex::new(Session::new()));
    let mut current_session_id: Option<Uuid> = None;

    // `incoming_stream.next()` returns `None` on a clean disconnect and
    // `Some(Err(_))` on a protocol error (e.g. the client vanishing mid-frame
    // without a Close handshake); handlers run serially, each awaited before the
    // next frame is read. So the loop always terminates via one of the arms below.
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

async fn process_text_frame(
    text: &str,
    current_session: &mut Arc<tokio::sync::Mutex<Session>>,
    current_session_id: &mut Option<Uuid>,
    app: &Arc<AppState>,
    emitter: &Emitter,
) {
    // A JSON decode failure sends an `error` message whose `data` is a plain
    // STRING, not the usual {message, trace} object.
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

    // A malformed envelope (valid JSON, missing/odd "type") instead sends the
    // OBJECT shape: {"message": ..., "trace": ...}.
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
    // deadlock. They get a scratch session and lock at most the single arc they
    // need via `attachment.arc`.
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
        // Sending a >1GiB frame in a test is not affordable, so this pins the
        // value `ws_upgrade` feeds to max_message_size/max_frame_size instead of
        // exercising the limit end-to-end.
        assert_eq!(MAX_WS_MESSAGE_BYTES, 1 << 30);
    }
}
