//! reattach_session / eject_session: a connection can re-attach to a stored
//! session by id after a reconnect, or eject it.
//!
//! Neither handler ever holds more than one per-session `tokio::Mutex` guard
//! at a time. `ws.rs::process_text_frame` deliberately skips pre-locking the
//! connection's current session for these two message types; holding two
//! per-session guards on one task would let two connections that reattach to
//! each other's ids form a lock cycle and deadlock permanently.

use uuid::Uuid;

use psp_core::session::Session;

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::handlers::save_file::emit_reattach_overview;
use crate::messages::MessageType;

#[derive(Debug, serde::Deserialize)]
pub struct ReattachSessionData {
    pub session_id: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct EjectSessionData {
    pub session_id: String,
}

/// Locks exactly one per-session mutex: the target arc. Even when the requested
/// id is already the one attached, this is still a single safe lock — the
/// connection's current guard is never held while taking the target's, so no
/// cross-connection reattach cycle can deadlock.
pub async fn handle_reattach_session(
    data: ReattachSessionData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Ok(requested_id) = Uuid::parse_str(&data.session_id) else {
        ctx.emitter
            .emit(MessageType::SessionNotFound, &data.session_id);
        return Ok(());
    };

    // Brief map lookup; the std lock is dropped before any `.await`.
    let target_arc = {
        let store = ctx.app.sessions.lock().expect("session store poisoned");
        store.get(&requested_id)
    };
    let Some(target_arc) = target_arc else {
        ctx.emitter
            .emit(MessageType::SessionNotFound, &data.session_id);
        return Ok(());
    };

    // Guard is scoped so it's dropped before the swap below.
    {
        let target_guard = target_arc.lock().await;
        if let Some(save) = target_guard.save.as_ref() {
            emit_reattach_overview(save, requested_id, ctx.emitter);
        }
    }

    let attachment = ctx
        .attachment
        .as_mut()
        .expect("reattach_session requires a connection attachment");
    *attachment.arc = target_arc;
    *attachment.current_id = Some(requested_id);
    Ok(())
}

/// Removes the session from the store, then confirms with `eject_session`
/// (data: the id). Resets this connection only when it was attached to the
/// ejected id — ejecting some other id must not wipe the client's own session.
/// The reset swaps in a fresh empty session, so it holds no per-session guard.
pub async fn handle_eject_session(
    data: EjectSessionData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let requested_id = Uuid::parse_str(&data.session_id).ok();
    if let Some(requested_id) = requested_id {
        ctx.app
            .sessions
            .lock()
            .expect("session store poisoned")
            .remove(&requested_id);
    }

    if let Some(attachment) = ctx.attachment.as_mut() {
        if requested_id.is_some() && requested_id == *attachment.current_id {
            *attachment.arc = std::sync::Arc::new(tokio::sync::Mutex::new(Session::new()));
            *attachment.current_id = None;
        }
    }

    ctx.emitter
        .emit(MessageType::EjectSession, &data.session_id);
    Ok(())
}
