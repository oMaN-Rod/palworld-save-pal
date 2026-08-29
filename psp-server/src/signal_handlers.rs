//! Signal message handlers: the bridge between the WS message flow and the
//! `psp-signal` manager. Every action answers with a full `signal_status_update`
//! payload, so the tab's poll-driven UI never needs a second round trip.
use std::sync::Arc;

use serde_json::{json, Value};

use psp_app::emitter::Emitter;
use psp_app::messages::MessageType;
use psp_signal::manager::SignalManager;
use psp_signal::poller::SourceConfig;
use psp_signal::store::SignalStored;

/// The status payload every Signal response carries. It is one object the
/// tab renders wholesale: API address, access token, source, feed state,
/// config, and a trimmed live-frame preview.
pub async fn status_payload(manager: &SignalManager) -> Value {
    let snapshot = manager.snapshot().await;
    let stored = manager.stored().await;
    let frame = match &snapshot.frame {
        Some(frame) => json!({
            "ok": frame.ok,
            "source": frame.source,
            "age": frame.age,
            "stale": frame.stale,
            "unit": frame.unit,
            "actors": frame.actors,
        }),
        None => json!({"ok": false, "actors": []}),
    };
    json!({
        "running": snapshot.running,
        "api": {
            "url": format!("http://{}", snapshot.bind_addr),
            "bind": snapshot.bind_addr.ip().to_string(),
            "port": snapshot.bind_addr.port(),
            "lanIp": snapshot.lan_ip,
            "loopbackOnly": snapshot.bind_addr.ip().is_loopback(),
        },
        "access": {
            "token": snapshot.token,
        },
        "source": {
            "kind": snapshot.source.map(|kind| kind.as_str()),
            "url": snapshot.source_url,
            "locked": snapshot.source_locked,
            "passwordSet": snapshot.password_set,
        },
        "feed": {
            "state": snapshot.status.state.as_str(),
            "error": snapshot.status.error,
            "age": snapshot.status.last_ok_age,
            "actors": snapshot.status.actor_count,
            "stale": snapshot.status.state == psp_signal::model::FeedState::Stale,
        },
        "frame": frame,
        "config": {
            "enabled": stored.enabled,
            "bind": stored.bind,
            "port": stored.port,
            "intervalMs": stored.interval_ms,
            "allowedOrigins": stored.allowed_origins,
        },
    })
}

async fn emit_status(manager: &Arc<SignalManager>, emitter: &Emitter) {
    emitter.emit(MessageType::SignalStatusUpdate, &status_payload(manager).await);
}

pub async fn handle_get_status(
    manager: &Arc<SignalManager>,
    emitter: &Emitter,
) -> Result<(), psp_app::handler_error::HandlerError> {
    emit_status(manager, emitter).await;
    Ok(())
}

#[derive(serde::Deserialize)]
pub struct UpdateConfigData {
    pub enabled: Option<bool>,
    pub bind: Option<String>,
    pub port: Option<u16>,
    #[serde(default, alias = "intervalMs")]
    pub interval_ms: Option<u64>,
    #[serde(default, alias = "allowedOrigins")]
    pub allowed_origins: Option<Vec<String>>,
    /// When true the API is (re)started with the new settings immediately.
    #[serde(default, alias = "applyNow")]
    pub apply_now: bool,
}

pub async fn handle_update_config(
    manager: &Arc<SignalManager>,
    data: UpdateConfigData,
    emitter: &Emitter,
) -> Result<(), psp_app::handler_error::HandlerError> {
    manager
        .update_settings(
            data.enabled,
            data.bind,
            data.port,
            data.interval_ms,
            data.allowed_origins,
        )
        .await;
    if data.apply_now && manager.is_running().await {
        manager.stop().await;
        if let Err(error) = manager.start().await {
            emitter.emit(MessageType::Error, &json!({"message": error.to_string()}));
        }
    }
    emit_status(manager, emitter).await;
    Ok(())
}

pub async fn handle_start(
    manager: &Arc<SignalManager>,
    emitter: &Emitter,
) -> Result<(), psp_app::handler_error::HandlerError> {
    if let Err(error) = manager.start().await {
        tracing::warn!(%error, "signal start failed");
        emitter.emit(MessageType::Error, &json!({"message": error.to_string()}));
    }
    emit_status(manager, emitter).await;
    Ok(())
}

pub async fn handle_stop(
    manager: &Arc<SignalManager>,
    emitter: &Emitter,
) -> Result<(), psp_app::handler_error::HandlerError> {
    manager.stop().await;
    emit_status(manager, emitter).await;
    Ok(())
}

#[derive(serde::Deserialize)]
pub struct SetSourceData {
    /// "rest" | "gamedata" | "fake"
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub url: Option<String>,
    /// Sent but never persisted; blank keeps the current password.
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default, alias = "path")]
    pub gamedata_path: Option<String>,
}

pub async fn handle_set_source(
    manager: &Arc<SignalManager>,
    data: SetSourceData,
    emitter: &Emitter,
) -> Result<(), psp_app::handler_error::HandlerError> {
    let source = match data.kind.as_str() {
        "fake" => SourceConfig::Fake,
        "gamedata" => SourceConfig::GameData {
            path: data
                .gamedata_path
                .filter(|path| !path.is_empty())
                .map(std::path::PathBuf::from),
        },
        "rest" => {
            let url = data.url.unwrap_or_default();
            let Some(base) = psp_signal::rest::normalize_base(&url) else {
                emitter.emit(
                    MessageType::Error,
                    &json!({"message": "not an http address Signal can call"}),
                );
                return Ok(());
            };
            let password = data
                .password
                .map(|password| password.trim().to_string())
                .filter(|password| !password.is_empty());
            SourceConfig::Rest { base, password }
        }
        other => {
            emitter.emit(
                MessageType::Error,
                &json!({"message": format!("unknown signal source type: {other}")}),
            );
            return Ok(());
        }
    };
    if let Err(error) = manager.set_source(Some(source)).await {
        emitter.emit(MessageType::Error, &json!({"message": error.to_string()}));
    }
    emit_status(manager, emitter).await;
    Ok(())
}

pub async fn handle_clear_source(
    manager: &Arc<SignalManager>,
    emitter: &Emitter,
) -> Result<(), psp_app::handler_error::HandlerError> {
    if let Err(error) = manager.set_source(None).await {
        emitter.emit(MessageType::Error, &json!({"message": error.to_string()}));
    }
    emit_status(manager, emitter).await;
    Ok(())
}

pub async fn handle_regenerate_token(
    manager: &Arc<SignalManager>,
    emitter: &Emitter,
) -> Result<(), psp_app::handler_error::HandlerError> {
    manager.regenerate_token().await;
    emit_status(manager, emitter).await;
    Ok(())
}

pub async fn handle_discover_gamedata(
    manager: &Arc<SignalManager>,
    emitter: &Emitter,
) -> Result<(), psp_app::handler_error::HandlerError> {
    let candidates: Vec<Value> = manager
        .discover_game_data()
        .into_iter()
        .map(|candidate| {
            json!({
                "path": candidate.path.to_string_lossy(),
                "exists": candidate.exists,
                "origin": candidate.origin,
            })
        })
        .collect();
    emitter.emit(
        MessageType::DiscoverSignalGamedata,
        &json!({"candidates": candidates}),
    );
    Ok(())
}

/// Builds the startup source (if any) from persisted settings. The REST
/// password is never stored, so a restarted REST source starts in
/// `waiting` until the password is re-entered — by design.
pub fn source_from_stored(stored: &SignalStored) -> Option<SourceConfig> {
    match stored.source_type.as_deref() {
        Some("fake") => Some(SourceConfig::Fake),
        Some("gamedata") => Some(SourceConfig::GameData {
            path: stored
                .gamedata_path
                .as_ref()
                .filter(|path| !path.is_empty())
                .map(std::path::PathBuf::from),
        }),
        Some("rest") => stored.source_url.as_ref().map(|base| SourceConfig::Rest {
            base: base.clone(),
            password: None, // deliberately not persisted
        }),
        _ => None,
    }
}

