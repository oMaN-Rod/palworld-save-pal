//! The /ws/{client_id} endpoint: one connection loop per client.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, Path, State};
use axum::http::HeaderMap;
use axum::response::Response;
use futures::{SinkExt, StreamExt};

use uuid::Uuid;

use ps_core::session::Session;

use crate::dispatcher::{dispatch, HandlerCtx, SessionAttachment};
use crate::emitter::Emitter;
use crate::envelope::Envelope;
use crate::messages::MessageType;
use crate::AppState;

/// Network messages are deliberately bounded. A save import must fit inside
/// this limit, but a peer must never be able to reserve a gigabyte per frame.
pub const MAX_WS_MESSAGE_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_WS_FRAME_BYTES: usize = MAX_WS_MESSAGE_BYTES;
/// Per-connection outgoing byte budget: frames may pile up while the client
/// is slow, but a client that stops draining entirely is disconnected rather
/// than allowed to grow the queue without bound.
const WS_MAX_QUEUED_BYTES: usize = 64 * 1024 * 1024;
const WS_WRITE_TIMEOUT: Duration = Duration::from_secs(30);
const WS_IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const POLICY_POLL_INTERVAL: Duration = Duration::from_secs(1);

pub async fn ws_upgrade(
    upgrade: WebSocketUpgrade,
    Path(client_id): Path<String>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    State(app): State<Arc<AppState>>,
    axum::Extension(runtime): axum::Extension<Arc<crate::network::NetworkRuntime>>,
) -> Response {
    // The network gate has already refused disallowed/unauthenticated peers;
    // here the verdict is stamped onto the connection so the dispatcher can
    // enforce the write allowlist per message. Funnel-forwarded clients are
    // judged by their forwarded address, never as the proxy's loopback.
    let inbound = runtime.inbound(peer.ip(), &headers);
    let direct = matches!(inbound, crate::network::InboundPeer::Direct(_));
    let peer_ip = inbound.effective();
    let acl = if direct {
        crate::network_policy::acl_for(&app.network_policy, peer_ip)
    } else {
        crate::network_policy::acl_for_forwarded(&app.network_policy, peer_ip)
    };
    let is_loopback = is_loopback_peer(peer) && direct;
    let policy_generation = crate::network_policy::policy_generation(&app.network_policy);
    let session_token = crate::network::session_token_from(&headers);
    upgrade
        .max_message_size(MAX_WS_MESSAGE_BYTES)
        .max_frame_size(MAX_WS_FRAME_BYTES)
        .on_upgrade(move |socket| {
            connection_loop(
                socket,
                client_id,
                peer_ip,
                !direct,
                is_loopback,
                acl.can_write,
                policy_generation,
                session_token,
                app,
            )
        })
}

fn is_loopback_peer(peer: SocketAddr) -> bool {
    peer.ip().to_canonical().is_loopback()
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
async fn connection_loop(
    socket: WebSocket,
    client_id: String,
    peer: IpAddr,
    forwarded: bool,
    is_loopback: bool,
    write_allowed: bool,
    policy_generation: u64,
    session_token: Option<String>,
    app: Arc<AppState>,
) {
    tracing::info!(%client_id, %peer, forwarded, is_loopback, write_allowed, "client connected");
    let _live_connection_guard = LiveConnectionGuard::new(app.live_connections.clone());

    let (mut outgoing_sink, mut incoming_stream) = socket.split();
    let (frame_sender, mut frame_receiver) = tokio::sync::mpsc::unbounded_channel::<String>();
    let queue_guard = ps_app::emitter::OutgoingQueueGuard::new(WS_MAX_QUEUED_BYTES);

    // Drains the mpsc channel onto the socket so handlers never block on I/O.
    // Exits when the channel closes (all Emitters dropped) or the send fails
    // (client gone) — either way `frame_receiver.recv()` eventually returns
    // `None` or the loop `break`s, so this task always terminates. A client
    // that stops draining trips the queue budget and is disconnected here:
    // frames are never dropped one by one from a live connection.
    let writer_guard = Arc::clone(&queue_guard);
    let writer_task = tokio::spawn(async move {
        while let Some(frame) = frame_receiver.recv().await {
            if writer_guard.is_terminated() {
                tracing::warn!("websocket writer terminating an over-budget connection");
                break;
            }
            match tokio::time::timeout(
                WS_WRITE_TIMEOUT,
                outgoing_sink.send(Message::Text(frame.clone().into())),
            )
            .await
            {
                Ok(Ok(())) => writer_guard.release(frame.len()),
                Ok(Err(error)) => {
                    tracing::debug!(%error, "websocket writer closed");
                    break;
                }
                Err(_) => {
                    tracing::warn!("websocket writer timed out; closing connection");
                    break;
                }
            }
        }
    });

    let emitter = Emitter::new_network(frame_sender, MAX_WS_MESSAGE_BYTES, queue_guard);

    // The connection owns ONE session `Arc` slot, reused for every message so
    // per-connection state (a loaded save, gamepass scan results, a transfer
    // source) persists across messages. A load registers this `Arc` in
    // `AppState::sessions` under a fresh id; `reattach_session` can REPLACE the
    // slot with the store's arc for another id, so it is `mut`.
    let mut current_session: Arc<tokio::sync::Mutex<Session>> =
        Arc::new(tokio::sync::Mutex::new(Session::new()));
    let mut current_session_id: Option<Uuid> = None;
    // Owned per-connection, dropped when the socket closes; lives across every
    // message the connection dispatches.
    let mut blueprints = crate::blueprint_registry::BlueprintRegistry::default();

    // `incoming_stream.next()` returns `None` on a clean disconnect and
    // `Some(Err(_))` on a protocol error (e.g. the client vanishing mid-frame
    // without a Close handshake); handlers run serially, each awaited before the
    // next frame is read. So the loop always terminates via one of the arms below.
    let idle_deadline = tokio::time::Instant::now() + WS_IDLE_TIMEOUT;
    let mut idle_deadline = idle_deadline;
    loop {
        // Polling the generation makes policy changes revoke already-open WS
        // sessions within one second, even when the browser is idle. The
        // connection is also re-evaluated immediately before every dispatch.
        if crate::network_policy::policy_generation(&app.network_policy) != policy_generation {
            tracing::info!(%client_id, "websocket invalidated by network policy change");
            break;
        }
        let remaining = idle_deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            tracing::info!(%client_id, "websocket idle timeout");
            break;
        }
        tokio::select! {
            _ = tokio::time::sleep(POLICY_POLL_INTERVAL) => continue,
            incoming = tokio::time::timeout(remaining, incoming_stream.next()) => match incoming {
            Ok(Some(Ok(Message::Text(text)))) => {
                idle_deadline = tokio::time::Instant::now() + WS_IDLE_TIMEOUT;
                if crate::network_policy::policy_generation(&app.network_policy) != policy_generation {
                    tracing::info!(%client_id, "websocket invalidated before dispatch");
                    break;
                }
                let acl = if forwarded {
                    crate::network_policy::acl_for_forwarded(&app.network_policy, peer)
                } else {
                    crate::network_policy::acl_for(&app.network_policy, peer)
                };
                if !acl.can_connect {
                    tracing::info!(%client_id, %peer, "websocket peer is no longer admitted");
                    break;
                }
                if acl.auth_required
                    && !session_token.as_deref().is_some_and(|token| {
                        app.network_policy
                            .as_ref()
                            .is_some_and(|policy| policy.has_valid_session(token))
                    })
                {
                    tracing::info!(%client_id, %peer, "websocket session expired or was revoked");
                    break;
                }
                process_text_frame(
                    text.as_str(),
                    &mut current_session,
                    &mut current_session_id,
                    &app,
                    &emitter,
                    &mut blueprints,
                    is_loopback,
                    acl.can_write,
                )
                .await;
            }
            Ok(Some(Ok(Message::Close(_)))) => break,
            // Ping/pong handled by axum; binary frames are not part of the protocol.
            Ok(Some(Ok(_))) => {
                idle_deadline = tokio::time::Instant::now() + WS_IDLE_TIMEOUT;
            }
            Ok(Some(Err(protocol_error))) => {
                tracing::warn!(%client_id, %protocol_error, "websocket protocol error; closing connection");
                break;
            }
            Ok(None) => break,
            Err(_) => {
                tracing::info!(%client_id, "websocket idle timeout");
                break;
            }
            }
        }
    }

    drop(emitter); // closes the channel → writer task exits
    let _ = writer_task.await;
    tracing::warn!(%client_id, "client disconnected");
}

#[allow(clippy::too_many_arguments)]
async fn process_text_frame(
    text: &str,
    current_session: &mut Arc<tokio::sync::Mutex<Session>>,
    current_session_id: &mut Option<Uuid>,
    app: &Arc<AppState>,
    emitter: &Emitter,
    blueprints: &mut crate::blueprint_registry::BlueprintRegistry,
    is_loopback: bool,
    write_allowed: bool,
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
                blueprints,
                is_loopback,
                write_allowed,
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
                blueprints,
                is_loopback,
                write_allowed,
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
    fn max_ws_message_and_frame_bytes_are_bounded() {
        assert_eq!(MAX_WS_MESSAGE_BYTES, 64 * 1024 * 1024);
        assert_eq!(MAX_WS_FRAME_BYTES, MAX_WS_MESSAGE_BYTES);
    }

    #[test]
    fn an_ipv4_mapped_loopback_peer_is_recognized_as_loopback() {
        let peer: SocketAddr = "[::ffff:127.0.0.1]:12345".parse().unwrap();
        assert!(is_loopback_peer(peer));
    }

    #[test]
    fn a_plain_loopback_peer_is_recognized_as_loopback() {
        let peer: SocketAddr = "127.0.0.1:12345".parse().unwrap();
        assert!(is_loopback_peer(peer));
        let peer: SocketAddr = "[::1]:12345".parse().unwrap();
        assert!(is_loopback_peer(peer));
    }

    #[test]
    fn a_remote_peer_is_not_loopback() {
        let peer: SocketAddr = "203.0.113.5:12345".parse().unwrap();
        assert!(!is_loopback_peer(peer));
    }
}
