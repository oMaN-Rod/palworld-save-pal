use std::sync::Arc;

use serde_json::Value;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use psp_app::blueprint_registry::BlueprintRegistry;
use psp_app::dispatcher::{dispatch, HandlerCtx, SessionAttachment};
use psp_app::emitter::Emitter;
use psp_app::envelope::Envelope;
use psp_app::messages::MessageType;
use psp_app::{AppState, SharedSession};
use psp_core::session::Session;

use super::framing::{chunk_frame, AssemblerOutcome, ChunkAssembler};
use crate::signal_handlers::refuse;

pub const REMOTE_DENYLIST: &[MessageType] = &[
    MessageType::UnlockMap,
    MessageType::SaveEditedSav,
    MessageType::ExportBlueprintFile,
    MessageType::ExportPreset,
    MessageType::ExportPresets,
    MessageType::ImportPreset,
    MessageType::ExportPlugin,
    MessageType::ExportOverviewStats,
    MessageType::LoadSourceSave,
    MessageType::ConvertSaveFormat,
    MessageType::SignalSetSource,
    MessageType::SignalStatus,
    MessageType::SignalStartPairing,
    MessageType::SignalStopPairing,
    MessageType::SignalSetArmed,
    MessageType::SignalListDevices,
    MessageType::SignalRenameDevice,
    MessageType::SignalRevokeDevice,
    MessageType::SignalResetRemoteAccess,
    MessageType::SubscribeLive,
    MessageType::OpenFolder,
    MessageType::OpenInBrowser,
    MessageType::OpenUrl,
    MessageType::DownloadSaveFile,
    MessageType::LoadZipFile,
    MessageType::ConvertSavFile,
    MessageType::InstallServerMod,
    MessageType::InstallPlugin,
];

pub async fn run_ctl_bridge(
    mut ctl_in_rx: mpsc::Receiver<Value>,
    ctl_out_tx: mpsc::Sender<String>,
    app: Arc<AppState>,
    cancel: CancellationToken,
) {
    let (emit_tx, emit_rx) = mpsc::unbounded_channel::<String>();
    let emitter = Emitter::new(emit_tx);
    let drain = tokio::spawn(drain_emitter(emit_rx, ctl_out_tx));

    let mut session_arc: SharedSession = Arc::new(tokio::sync::Mutex::new(Session::new()));
    let mut current_id: Option<Uuid> = None;
    let mut blueprints = BlueprintRegistry::default();
    let mut assembler = ChunkAssembler::default();

    loop {
        let envelope = tokio::select! {
            _ = cancel.cancelled() => break,
            envelope = ctl_in_rx.recv() => match envelope {
                Some(envelope) => envelope,
                None => break,
            },
        };
        let value = match assembler.accept(envelope) {
            AssemblerOutcome::NotChunk(value) => value,
            AssemblerOutcome::Complete(text) => match serde_json::from_str(&text) {
                Ok(value) => value,
                Err(_) => continue,
            },
            AssemblerOutcome::Pending | AssemblerOutcome::Rejected => continue,
        };
        handle_envelope(
            value,
            &app,
            &emitter,
            &mut session_arc,
            &mut current_id,
            &mut blueprints,
        )
        .await;
    }
    drop(emitter);
    let _ = drain.await;
}

pub async fn drain_emitter(
    mut emit_rx: mpsc::UnboundedReceiver<String>,
    ctl_out_tx: mpsc::Sender<String>,
) {
    let mut next_id: u64 = 0;
    while let Some(frame) = emit_rx.recv().await {
        for piece in chunk_frame(frame, &mut next_id) {
            if ctl_out_tx.send(piece).await.is_err() {
                return;
            }
        }
    }
}

async fn handle_envelope(
    value: Value,
    app: &Arc<AppState>,
    emitter: &Emitter,
    session_arc: &mut SharedSession,
    current_id: &mut Option<Uuid>,
    blueprints: &mut BlueprintRegistry,
) {
    let Some(wire_type) = value.get("type").and_then(Value::as_str) else {
        tracing::warn!("ctl bridge: envelope has no type");
        return;
    };
    let Some(message_type) = MessageType::from_wire(wire_type) else {
        tracing::warn!(message_type = wire_type, "ctl bridge: invalid message type");
        return;
    };
    if REMOTE_DENYLIST.contains(&message_type) {
        refuse(emitter, message_type, "Not available over a remote session");
        return;
    }

    let envelope = Envelope {
        message_type: wire_type.to_string(),
        data: value.get("data").cloned().unwrap_or(Value::Null),
    };

    let holds_own_session_lock = !matches!(
        message_type,
        MessageType::ReattachSession | MessageType::EjectSession
    );
    let session_clone = session_arc.clone();
    let mut scratch = Session::new();
    let mut session_guard = if holds_own_session_lock {
        Some(session_clone.lock().await)
    } else {
        None
    };
    let session: &mut Session = match session_guard.as_mut() {
        Some(guard) => guard,
        None => &mut scratch,
    };
    let ctx = HandlerCtx {
        session,
        app,
        emitter,
        blueprints,
        is_loopback: false,
        attachment: Some(SessionAttachment {
            current_id,
            arc: session_arc,
        }),
    };
    dispatch(envelope, ctx).await;
}
