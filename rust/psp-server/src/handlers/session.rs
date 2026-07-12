//! reattach_session / eject_session — session-persistence lifecycle (SP-T2).
//! Feature additions beyond the Python parity set: a connection can re-attach
//! to a stored session by id after a reconnect, or eject it.

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

/// Re-attach the connection to a stored session by id. Absent (or unparseable)
/// id → `session_not_found` (data: the id), leaving the current attachment
/// untouched. Present → emit the load overview and, for a DIFFERENT id, swap
/// the connection's arc + id to the store's.
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

    let attachment = ctx
        .attachment
        .as_mut()
        .expect("reattach_session requires a connection attachment");

    // Deadlock guard: if we're already on this id, `ctx.session` IS this arc's
    // held guard — re-locking the same mutex would hang. Read from it directly
    // and skip the swap (already attached).
    if *attachment.current_id == Some(requested_id) {
        if let Some(save) = ctx.session.save.as_ref() {
            emit_reattach_overview(save, requested_id, ctx.emitter);
        }
        return Ok(());
    }

    // Different id → a different mutex, safe to lock while holding ctx.session.
    {
        let target_guard = target_arc.lock().await;
        if let Some(save) = target_guard.save.as_ref() {
            emit_reattach_overview(save, requested_id, ctx.emitter);
        }
    }

    // Swap the connection's own arc slot + id to the store's.
    let attachment = ctx.attachment.as_mut().unwrap();
    *attachment.arc = target_arc;
    *attachment.current_id = Some(requested_id);
    Ok(())
}

/// Remove the session from the store and clear the connection back to an empty
/// session, then confirm with `eject_session` (data: the id).
pub async fn handle_eject_session(
    data: EjectSessionData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if let Ok(requested_id) = Uuid::parse_str(&data.session_id) {
        ctx.app
            .sessions
            .lock()
            .expect("session store poisoned")
            .remove(&requested_id);
    }

    // Reset every loaded field the connection holds (save/source/gamepass/...).
    *ctx.session = Session::new();
    if let Some(attachment) = ctx.attachment.as_mut() {
        *attachment.current_id = None;
    }

    ctx.emitter
        .emit(MessageType::EjectSession, &data.session_id);
    Ok(())
}
