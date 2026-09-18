use std::sync::Arc;

use serde_json::Value;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use ps_app::blueprint_registry::BlueprintRegistry;
use ps_app::dispatcher::{dispatch, HandlerCtx, SessionAttachment};
use ps_app::emitter::Emitter;
use ps_app::envelope::Envelope;
use ps_app::messages::MessageType;
use ps_app::{AppState, SharedSession};
use ps_core::session::Session;

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
    MessageType::GameInstances,
    MessageType::GameAddInstance,
    MessageType::GameUpdateInstance,
    MessageType::GameInstanceSetTarget,
    MessageType::GameDeleteInstance,
    MessageType::GameSelectInstance,
    MessageType::GameTestInstance,
    MessageType::SubscribeLive,
    MessageType::OpenFolder,
    MessageType::OpenInBrowser,
    MessageType::OpenUrl,
    MessageType::DownloadSaveFile,
    MessageType::LoadZipFile,
    MessageType::ConvertSavFile,
    MessageType::InstallPlugin,
    MessageType::ModAnalyze,
    MessageType::ModInstall,
    MessageType::ModTargetAdd,
    MessageType::ModBackupList,
    MessageType::ModBackupRestore,
    MessageType::ModBackupDelete,
    MessageType::ProfileExport,
    MessageType::ProfileImport,
    MessageType::GameLaunch,
    MessageType::ModIostoreConvert,
    MessageType::NexusAccountGet,
    MessageType::NexusKeySet,
    MessageType::NexusKeyClear,
    MessageType::NexusCategories,
    MessageType::NexusSearch,
    MessageType::NexusModFiles,
    MessageType::NexusDownload,
    MessageType::NexusLinkSubscribe,
    MessageType::NexusLink,
    MessageType::NexusHandlerStatus,
    MessageType::NexusHandlerRegister,
    MessageType::ModUpdateCheck,
    MessageType::ModUpdateIgnore,
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
    let mut mod_verification_subscribed = false;
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
            &mut mod_verification_subscribed,
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

const REMOTE_DENIED_MESSAGE: &str = "Not available over a remote session";

fn is_mods_message(wire_type: &str) -> bool {
    wire_type.starts_with("mod_")
        || wire_type.starts_with("profile_")
        || wire_type.starts_with("world_profile_")
        || wire_type.starts_with("nexus_")
        || wire_type == "game_launch"
}

/// The mods UI reads a refusal as `error: { code, message }` beside the fields
/// that identify the request, the shape `mods_handlers::emit_refusal` sends.
fn refuse_mods_request(emitter: &Emitter, request: MessageType, data: Option<&Value>) {
    let mut reply = serde_json::Map::new();
    for key in [
        "target_id",
        "path",
        "set",
        "root_path",
        "profile_id",
        "world_key",
        "mod_id",
        "file_id",
    ] {
        if let Some(field) = data.and_then(|data| data.get(key)) {
            reply.insert(key.to_string(), field.clone());
        }
    }
    reply.insert(
        "error".to_string(),
        serde_json::json!({ "code": "remote_denied", "message": REMOTE_DENIED_MESSAGE }),
    );
    emitter.emit(request, &Value::Object(reply));
}

#[allow(clippy::too_many_arguments)]
async fn handle_envelope(
    value: Value,
    app: &Arc<AppState>,
    emitter: &Emitter,
    session_arc: &mut SharedSession,
    current_id: &mut Option<Uuid>,
    blueprints: &mut BlueprintRegistry,
    mod_verification_subscribed: &mut bool,
) {
    let Some(wire_type) = value.get("type").and_then(Value::as_str) else {
        tracing::warn!("ctl bridge: envelope has no type");
        return;
    };
    let Some(message_type) = MessageType::from_wire(wire_type) else {
        tracing::warn!(message_type = wire_type, "ctl bridge: invalid message type");
        return;
    };
    if REMOTE_DENYLIST.contains(&message_type)
        && !app
            .ext
            .remote_allows(message_type, value.get("data").unwrap_or(&Value::Null))
    {
        if is_mods_message(wire_type) {
            refuse_mods_request(emitter, message_type, value.get("data"));
        } else {
            refuse(emitter, message_type, REMOTE_DENIED_MESSAGE);
        }
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
        mod_verification_subscribed: Some(mod_verification_subscribed),
        attachment: Some(SessionAttachment {
            current_id,
            arc: session_arc,
        }),
    };
    dispatch(envelope, ctx).await;
}
