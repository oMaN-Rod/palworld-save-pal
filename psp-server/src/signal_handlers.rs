use std::path::PathBuf;

use serde_json::Value;

use psp_app::live::SignalSourceStatus;

use crate::dispatcher::HandlerCtx;
use crate::emitter::Emitter;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;
use crate::services::ServerServices;
use crate::signal::manager::{
    group_code, pairing_url, ConnectedDevice, PairingState, SourceSelection,
};
use crate::signal::peer::IceServerConfig;

const PAIRING_GATE_MESSAGE: &str =
    "Pairing must be started from Palworld Save Pal on the machine it is running on.";
const REMOTE_ACCESS_GATE_MESSAGE: &str =
    "Remote access must be managed from Palworld Save Pal on the machine it is running on.";
const ALLOW_REMOTE_PAIRING_VAR: &str = "PSP_SIGNAL_ALLOW_REMOTE_PAIRING";
const UNKNOWN_DEVICE_MESSAGE: &str = "That device is not paired with this desktop.";
const EMPTY_DEVICE_NAME_MESSAGE: &str = "A device name cannot be empty.";

pub async fn handle_subscribe_live(
    _data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    ctx.emitter.emit(
        MessageType::SubscribeLive,
        &serde_json::json!({ "active": true }),
    );
    if ctx.session.live_subscribed {
        return Ok(());
    }
    ctx.session.live_subscribed = true;

    let mut rx = ctx.app.live_bus.subscribe();
    let fwd = ctx.emitter.clone();
    tokio::spawn(async move {
        loop {
            let frame = rx.borrow_and_update().clone();
            if let Some(frame) = frame {
                fwd.emit(MessageType::LiveFrame, &frame);
            }
            tokio::select! {
                _ = fwd.closed() => break,
                changed = rx.changed() => {
                    if changed.is_err() {
                        break;
                    }
                }
            }
        }
    });
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct SignalSetSourceData {
    pub kind: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub server_id: Option<i64>,
}

fn signal_status_payload(
    status: &SignalSourceStatus,
    pairing: &PairingState,
    connected_device: Option<&ConnectedDevice>,
    armed: bool,
    include_sensitive: bool,
) -> Value {
    let mut payload = serde_json::json!({
        "source": status,
        "pairing": pairing.label(),
        "armed": armed,
    });
    if let PairingState::Waiting {
        code,
        expires_at_ms,
    } = pairing
    {
        if include_sensitive {
            payload["code"] = serde_json::json!(group_code(code));
            payload["expiresAtMs"] = serde_json::json!(expires_at_ms);
        }
    }
    if include_sensitive {
        if let Some(device) = connected_device {
            payload["connectedDevice"] =
                serde_json::to_value(device).expect("ConnectedDevice always serializes");
        }
    }
    payload
}

fn manager_status_payload(
    manager: &crate::signal::manager::SignalManager,
    include_sensitive: bool,
) -> Value {
    let connected_device = manager.connected_device();
    signal_status_payload(
        &manager.status(),
        &manager.pairing(),
        connected_device.as_ref(),
        manager.armed(),
        include_sensitive,
    )
}

fn device_entry(
    device: &psp_db::signal_devices::SignalDevice,
    connected_device_id: Option<&str>,
) -> Value {
    let mut entry = serde_json::json!({
        "deviceId": device.device_id,
        "name": device.name,
        "createdAtMs": device.created_at_ms,
        "connected": connected_device_id == Some(device.device_id.as_str()),
    });
    if let Some(last_seen_ms) = device.last_seen_ms {
        entry["lastSeenMs"] = serde_json::json!(last_seen_ms);
    }
    entry
}

async fn device_list_payload(
    services: &ServerServices,
    ctx: &HandlerCtx<'_>,
) -> Result<Value, String> {
    let devices = psp_db::signal_devices::list_devices(&*ctx.app.driver)
        .await
        .map_err(|error| error.to_string())?;
    let connected = services.signal.lock().await.connected_device();
    let connected_id = connected.as_ref().map(|device| device.device_id.as_str());
    Ok(serde_json::json!({
        "devices": devices
            .iter()
            .map(|device| device_entry(device, connected_id))
            .collect::<Vec<_>>(),
    }))
}

async fn emit_device_list(
    services: &ServerServices,
    request: MessageType,
    ctx: &mut HandlerCtx<'_>,
) {
    match device_list_payload(services, ctx).await {
        Ok(payload) => ctx.emitter.emit(request, &payload),
        Err(message) => refuse(ctx.emitter, request, message),
    }
}

pub(crate) fn refuse(emitter: &Emitter, request: MessageType, message: impl Into<String>) {
    emitter.emit(request, &serde_json::json!({ "error": message.into() }));
}

fn pairing_allowed(is_loopback: bool, allow_remote: bool) -> bool {
    is_loopback || allow_remote
}

fn remote_pairing_env_allows(value: Option<&str>) -> bool {
    value == Some("1")
}

fn pairing_control_allowed(ctx: &HandlerCtx<'_>) -> bool {
    let allow_remote =
        remote_pairing_env_allows(std::env::var(ALLOW_REMOTE_PAIRING_VAR).ok().as_deref());
    pairing_allowed(ctx.is_loopback, allow_remote)
}

fn admit_pairing_control(request: MessageType, ctx: &HandlerCtx<'_>) -> bool {
    admit(request, ctx, PAIRING_GATE_MESSAGE)
}

fn admit_remote_access_control(request: MessageType, ctx: &HandlerCtx<'_>) -> bool {
    admit(request, ctx, REMOTE_ACCESS_GATE_MESSAGE)
}

fn admit(request: MessageType, ctx: &HandlerCtx<'_>, message: &str) -> bool {
    if pairing_control_allowed(ctx) {
        return true;
    }
    refuse(ctx.emitter, request, message);
    false
}

#[derive(Debug, Default, serde::Deserialize)]
struct SignalStartPairingData {
    #[serde(default)]
    turn: Option<TurnServerData>,
}

#[derive(serde::Deserialize)]
struct TurnServerData {
    urls: Vec<String>,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    credential: Option<String>,
}

impl std::fmt::Debug for TurnServerData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TurnServerData")
            .field("urls", &self.urls)
            .field("username", &self.username.as_ref().map(|_| "<redacted>"))
            .field(
                "credential",
                &self.credential.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

fn parse_ice_servers(data: Value) -> Result<Vec<IceServerConfig>, String> {
    if data.is_null() {
        return Ok(Vec::new());
    }
    let parsed: SignalStartPairingData =
        serde_json::from_value(data).map_err(|error| error.to_string())?;
    Ok(parsed
        .turn
        .map(|turn| {
            vec![IceServerConfig {
                urls: turn.urls,
                username: turn.username,
                credential: turn.credential,
            }]
        })
        .unwrap_or_default())
}

fn parse_selection(data: &SignalSetSourceData) -> Result<SourceSelection, String> {
    match data.kind.as_str() {
        "off" => Ok(SourceSelection::Off),
        "file" => Ok(SourceSelection::File {
            path: data.path.clone().map(PathBuf::from),
        }),
        "server" => match data.server_id {
            Some(server_id) => Ok(SourceSelection::Server { server_id }),
            None => Err("server_id is required for kind \"server\"".to_string()),
        },
        other => Err(format!("Unknown source kind: {other}")),
    }
}

pub async fn handle_signal_status(
    services: &ServerServices,
    _data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let include_sensitive = pairing_control_allowed(ctx);
    let manager = services.signal.lock().await;
    let payload = manager_status_payload(&manager, include_sensitive);
    drop(manager);
    ctx.emitter.emit(MessageType::SignalStatus, &payload);
    Ok(())
}

pub async fn handle_signal_set_source(
    services: &ServerServices,
    data: SignalSetSourceData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if !admit_pairing_control(MessageType::SignalSetSource, ctx) {
        return Ok(());
    }
    let selection = match parse_selection(&data) {
        Ok(selection) => selection,
        Err(message) => {
            refuse(ctx.emitter, MessageType::SignalSetSource, message);
            return Ok(());
        }
    };

    let mut manager = services.signal.lock().await;
    match manager
        .set_source(selection, ctx.app)
        .await
    {
        Ok(()) => {
            let payload = manager_status_payload(&manager, true);
            drop(manager);
            ctx.emitter.emit(MessageType::SignalSetSource, &payload);
        }
        Err(message) => refuse(ctx.emitter, MessageType::SignalSetSource, message),
    }
    Ok(())
}

pub async fn handle_signal_start_pairing(
    services: &ServerServices,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if !admit_pairing_control(MessageType::SignalStartPairing, ctx) {
        return Ok(());
    }
    let ice_servers = match parse_ice_servers(data) {
        Ok(ice_servers) => ice_servers,
        Err(message) => {
            refuse(ctx.emitter, MessageType::SignalStartPairing, message);
            return Ok(());
        }
    };
    let mut manager = services.signal.lock().await;
    let pairing = manager.start_pairing(ctx.app, ice_servers).await;
    let connected_device = manager.connected_device();
    let armed = manager.armed();
    let mut payload = signal_status_payload(
        &manager.status(),
        &pairing,
        connected_device.as_ref(),
        armed,
        true,
    );
    drop(manager);
    if let PairingState::Waiting { code, .. } = &pairing {
        payload["url"] = serde_json::json!(pairing_url(code));
    }
    ctx.emitter.emit(MessageType::SignalStartPairing, &payload);
    Ok(())
}

pub async fn handle_signal_stop_pairing(
    services: &ServerServices,
    _data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if !admit_pairing_control(MessageType::SignalStopPairing, ctx) {
        return Ok(());
    }
    let mut manager = services.signal.lock().await;
    manager.stop_pairing().await;
    let payload = manager_status_payload(&manager, true);
    drop(manager);
    ctx.emitter.emit(MessageType::SignalStopPairing, &payload);
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
struct SignalSetArmedData {
    armed: bool,
}

#[derive(Debug, serde::Deserialize)]
struct SignalDeviceData {
    device_id: String,
}

#[derive(Debug, serde::Deserialize)]
struct SignalRenameDeviceData {
    device_id: String,
    name: String,
}

fn parse_payload<T: serde::de::DeserializeOwned>(data: Value) -> Result<T, String> {
    serde_json::from_value(data).map_err(|error| error.to_string())
}

pub async fn handle_signal_set_armed(
    services: &ServerServices,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if !admit_remote_access_control(MessageType::SignalSetArmed, ctx) {
        return Ok(());
    }
    let payload: SignalSetArmedData = match parse_payload(data) {
        Ok(payload) => payload,
        Err(message) => {
            refuse(ctx.emitter, MessageType::SignalSetArmed, message);
            return Ok(());
        }
    };

    let mut manager = services.signal.lock().await;
    match manager.set_armed(payload.armed, ctx.app).await {
        Ok(()) => {
            let status = manager_status_payload(&manager, true);
            drop(manager);
            ctx.emitter.emit(MessageType::SignalSetArmed, &status);
        }
        Err(message) => {
            drop(manager);
            refuse(ctx.emitter, MessageType::SignalSetArmed, message);
        }
    }
    Ok(())
}

pub async fn handle_signal_list_devices(
    services: &ServerServices,
    _data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if !admit_remote_access_control(MessageType::SignalListDevices, ctx) {
        return Ok(());
    }
    emit_device_list(services, MessageType::SignalListDevices, ctx).await;
    Ok(())
}

pub async fn handle_signal_rename_device(
    services: &ServerServices,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if !admit_remote_access_control(MessageType::SignalRenameDevice, ctx) {
        return Ok(());
    }
    let payload: SignalRenameDeviceData = match parse_payload(data) {
        Ok(payload) => payload,
        Err(message) => {
            refuse(ctx.emitter, MessageType::SignalRenameDevice, message);
            return Ok(());
        }
    };
    let name = payload.name.trim();
    if name.is_empty() {
        refuse(
            ctx.emitter,
            MessageType::SignalRenameDevice,
            EMPTY_DEVICE_NAME_MESSAGE,
        );
        return Ok(());
    }

    match psp_db::signal_devices::rename_device(&*ctx.app.driver, &payload.device_id, name).await {
        Ok(false) => refuse(
            ctx.emitter,
            MessageType::SignalRenameDevice,
            UNKNOWN_DEVICE_MESSAGE,
        ),
        Ok(true) => emit_device_list(services, MessageType::SignalRenameDevice, ctx).await,
        Err(error) => refuse(
            ctx.emitter,
            MessageType::SignalRenameDevice,
            error.to_string(),
        ),
    }
    Ok(())
}

pub async fn handle_signal_revoke_device(
    services: &ServerServices,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if !admit_remote_access_control(MessageType::SignalRevokeDevice, ctx) {
        return Ok(());
    }
    let payload: SignalDeviceData = match parse_payload(data) {
        Ok(payload) => payload,
        Err(message) => {
            refuse(ctx.emitter, MessageType::SignalRevokeDevice, message);
            return Ok(());
        }
    };

    match psp_db::signal_devices::delete_device(&*ctx.app.driver, &payload.device_id).await {
        Ok(false) => refuse(
            ctx.emitter,
            MessageType::SignalRevokeDevice,
            UNKNOWN_DEVICE_MESSAGE,
        ),
        Ok(true) => {
            services
                .signal
                .lock()
                .await
                .revoke_device_session(&payload.device_id)
                .await;
            emit_device_list(services, MessageType::SignalRevokeDevice, ctx).await;
        }
        Err(error) => refuse(
            ctx.emitter,
            MessageType::SignalRevokeDevice,
            error.to_string(),
        ),
    }
    Ok(())
}

pub async fn handle_signal_reset_remote_access(
    services: &ServerServices,
    _data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if !admit_remote_access_control(MessageType::SignalResetRemoteAccess, ctx) {
        return Ok(());
    }
    let mut manager = services.signal.lock().await;
    match manager.reset_remote_access(ctx.app).await {
        Ok(()) => {
            let status = manager_status_payload(&manager, true);
            drop(manager);
            ctx.emitter
                .emit(MessageType::SignalResetRemoteAccess, &status);
        }
        Err(message) => {
            drop(manager);
            refuse(ctx.emitter, MessageType::SignalResetRemoteAccess, message);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::servers_handlers::test_env::TestEnv;
    use crate::signal::test_support::SignalEnvGuard;

    const BROKER_URL_VAR: &str = "PSP_SIGNAL_BROKER_URL";
    const TEST_BROKER_URL: &str = "ws://127.0.0.1:9";

    fn waiting_state() -> PairingState {
        PairingState::Waiting {
            code: "ABCDEFGHJKMNPQRSTUVWXY2345".to_string(),
            expires_at_ms: 1_756_500_000_000,
        }
    }

    fn loopback_ctx(env: &mut TestEnv) -> HandlerCtx<'_> {
        let mut ctx = env.ctx();
        ctx.is_loopback = true;
        ctx
    }

    #[test]
    fn pairing_is_allowed_from_loopback_or_by_explicit_opt_in() {
        assert!(pairing_allowed(true, false));
        assert!(!pairing_allowed(false, false));
        assert!(pairing_allowed(false, true));
        assert!(pairing_allowed(true, true));
    }

    #[test]
    fn parse_ice_servers_defaults_to_empty_when_no_payload_is_sent() {
        assert!(parse_ice_servers(Value::Null).unwrap().is_empty());
        assert!(parse_ice_servers(serde_json::json!({})).unwrap().is_empty());
    }

    #[test]
    fn parse_ice_servers_carries_a_supplied_turn_entry_through() {
        let servers = parse_ice_servers(serde_json::json!({
            "turn": {
                "urls": ["turn:relay.example:3478"],
                "username": "user",
                "credential": "secret"
            }
        }))
        .unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].urls, vec!["turn:relay.example:3478".to_string()]);
        assert_eq!(servers[0].username.as_deref(), Some("user"));
        assert_eq!(servers[0].credential.as_deref(), Some("secret"));
    }

    #[test]
    fn parse_ice_servers_rejects_a_malformed_turn_entry() {
        assert!(parse_ice_servers(serde_json::json!({ "turn": { "username": "user" } })).is_err());
    }

    #[test]
    fn debug_never_prints_a_deserialized_turn_credential() {
        let parsed: SignalStartPairingData = serde_json::from_value(serde_json::json!({
            "turn": {
                "urls": ["turn:relay.example:3478"],
                "username": "turn-account",
                "credential": "hunter2"
            }
        }))
        .unwrap();

        let rendered = format!("{parsed:?}");
        assert!(!rendered.contains("hunter2"));
        assert!(!rendered.contains("turn-account"));
        assert!(rendered.contains("turn:relay.example:3478"));
    }

    #[test]
    fn only_the_documented_opt_in_value_allows_remote_pairing() {
        assert!(remote_pairing_env_allows(Some("1")));
        assert!(!remote_pairing_env_allows(None));
        assert!(!remote_pairing_env_allows(Some("")));
        assert!(!remote_pairing_env_allows(Some("0")));
        assert!(!remote_pairing_env_allows(Some("true")));
    }

    #[tokio::test]
    async fn pairing_control_from_a_non_loopback_connection_is_refused() {
        let _env_guard = SignalEnvGuard::acquire(&[
            (ALLOW_REMOTE_PAIRING_VAR, None),
            (BROKER_URL_VAR, Some(TEST_BROKER_URL)),
        ])
        .await;
        let mut env = TestEnv::new().await;
        let services = env.services.clone();

        handle_signal_start_pairing(&services, Value::Null, &mut env.ctx())
            .await
            .unwrap();
        handle_signal_stop_pairing(&services, Value::Null, &mut env.ctx())
            .await
            .unwrap();
        handle_signal_set_source(
            &services,
            SignalSetSourceData {
                kind: "off".to_string(),
                path: None,
                server_id: None,
            },
            &mut env.ctx(),
        )
        .await
        .unwrap();

        let frames = env.drain();
        let refused: Vec<(&str, &str)> = frames
            .iter()
            .map(|frame| {
                (
                    frame["type"].as_str().unwrap(),
                    frame["data"]["error"].as_str().unwrap_or_default(),
                )
            })
            .collect();
        assert_eq!(
            refused,
            vec![
                ("signal_start_pairing", PAIRING_GATE_MESSAGE),
                ("signal_stop_pairing", PAIRING_GATE_MESSAGE),
                ("signal_set_source", PAIRING_GATE_MESSAGE),
            ]
        );
        assert_eq!(
            services.signal.lock().await.pairing(),
            PairingState::Off,
            "a refused start must not open a pairing window"
        );
    }

    fn paired_device(device_id: &str) -> psp_db::signal_devices::SignalDevice {
        psp_db::signal_devices::SignalDevice {
            device_id: device_id.to_string(),
            secret_hex: "5a".repeat(32),
            name: "Phone".to_string(),
            created_at_ms: 1_700_000_000_000,
            last_seen_ms: None,
        }
    }

    #[tokio::test]
    async fn remote_access_control_from_a_non_loopback_connection_is_refused() {
        let _env_guard = SignalEnvGuard::acquire(&[
            (ALLOW_REMOTE_PAIRING_VAR, None),
            (BROKER_URL_VAR, Some(TEST_BROKER_URL)),
        ])
        .await;
        let mut env = TestEnv::new().await;
        let services = env.services.clone();
        let driver = env.app.driver.clone();
        psp_db::signal_devices::insert_device(&*driver, &paired_device("dev-1"))
            .await
            .unwrap();

        handle_signal_set_armed(
            &services,
            serde_json::json!({ "armed": true }),
            &mut env.ctx(),
        )
        .await
        .unwrap();
        handle_signal_list_devices(&services, Value::Null, &mut env.ctx())
            .await
            .unwrap();
        handle_signal_rename_device(
            &services,
            serde_json::json!({ "device_id": "dev-1", "name": "Renamed" }),
            &mut env.ctx(),
        )
        .await
        .unwrap();
        handle_signal_revoke_device(
            &services,
            serde_json::json!({ "device_id": "dev-1" }),
            &mut env.ctx(),
        )
        .await
        .unwrap();
        handle_signal_reset_remote_access(&services, Value::Null, &mut env.ctx())
            .await
            .unwrap();

        let frames = env.drain();
        let refused: Vec<(&str, &str)> = frames
            .iter()
            .map(|frame| {
                (
                    frame["type"].as_str().unwrap(),
                    frame["data"]["error"].as_str().unwrap_or_default(),
                )
            })
            .collect();
        assert_eq!(
            refused,
            vec![
                ("signal_set_armed", REMOTE_ACCESS_GATE_MESSAGE),
                ("signal_list_devices", REMOTE_ACCESS_GATE_MESSAGE),
                ("signal_rename_device", REMOTE_ACCESS_GATE_MESSAGE),
                ("signal_revoke_device", REMOTE_ACCESS_GATE_MESSAGE),
                ("signal_reset_remote_access", REMOTE_ACCESS_GATE_MESSAGE),
            ]
        );
        assert!(
            !services.signal.lock().await.armed(),
            "a refused arm must not leave the desktop listening"
        );
        let devices = psp_db::signal_devices::list_devices(&*driver)
            .await
            .unwrap();
        assert_eq!(devices.len(), 1, "a refused revoke must not delete a row");
        assert_eq!(devices[0].name, "Phone", "a refused rename must not land");
    }

    #[test]
    fn the_status_payload_reports_armed_to_every_caller() {
        for include_sensitive in [true, false] {
            for armed in [true, false] {
                let payload = signal_status_payload(
                    &SignalSourceStatus::default(),
                    &PairingState::Off,
                    None,
                    armed,
                    include_sensitive,
                );
                assert_eq!(payload["armed"], armed, "{payload}");
            }
        }
    }

    #[test]
    fn a_device_entry_carries_what_the_ui_shows_and_never_the_secret() {
        let mut device = paired_device("dev-1");
        device.name = "My Phone".to_string();
        device.last_seen_ms = Some(1_700_000_100_000);

        let entry = device_entry(&device, Some("dev-1"));
        assert_eq!(entry["deviceId"], "dev-1");
        assert_eq!(entry["name"], "My Phone");
        assert_eq!(entry["createdAtMs"], 1_700_000_000_000i64);
        assert_eq!(entry["lastSeenMs"], 1_700_000_100_000i64);
        assert_eq!(entry["connected"], true);
        assert!(!entry.to_string().contains(&device.secret_hex), "{entry}");
    }

    #[test]
    fn a_device_that_has_never_connected_carries_no_last_seen() {
        let entry = device_entry(&paired_device("dev-1"), Some("dev-2"));
        assert!(entry.get("lastSeenMs").is_none(), "{entry}");
        assert_eq!(entry["connected"], false);
        assert_eq!(
            device_entry(&paired_device("dev-1"), None)["connected"],
            false
        );
    }

    fn a_connected_device() -> ConnectedDevice {
        ConnectedDevice {
            device_id: "dev-1".to_string(),
            name: "My Phone".to_string(),
        }
    }

    #[test]
    fn the_status_payload_carries_the_code_only_while_waiting() {
        let status = SignalSourceStatus::default();
        let waiting = signal_status_payload(&status, &waiting_state(), None, false, true);
        assert_eq!(waiting["pairing"], "waiting");
        assert_eq!(waiting["code"], "ABCD-EFGH-JKMN-PQRS-TUVW-XY23-45");
        assert_eq!(waiting["expiresAtMs"], 1_756_500_000_000i64);

        for state in [
            PairingState::Off,
            PairingState::Connected,
            PairingState::Failed,
        ] {
            let payload = signal_status_payload(&status, &state, None, false, true);
            assert_eq!(payload["pairing"], state.label());
            assert!(payload.get("code").is_none(), "{payload}");
            assert!(payload.get("expiresAtMs").is_none(), "{payload}");
        }
    }

    #[test]
    fn a_caller_who_may_not_pair_sees_the_window_but_not_the_code() {
        let payload = signal_status_payload(
            &SignalSourceStatus::default(),
            &waiting_state(),
            None,
            false,
            false,
        );
        assert_eq!(payload["pairing"], "waiting");
        assert!(payload.get("code").is_none(), "{payload}");
        assert!(payload.get("expiresAtMs").is_none(), "{payload}");
    }

    #[test]
    fn the_status_payload_carries_the_connected_device_only_for_a_privileged_caller() {
        let status = SignalSourceStatus::default();
        let device = a_connected_device();

        let privileged =
            signal_status_payload(&status, &PairingState::Off, Some(&device), false, true);
        assert_eq!(privileged["connectedDevice"]["deviceId"], "dev-1");
        assert_eq!(privileged["connectedDevice"]["name"], "My Phone");

        let unprivileged =
            signal_status_payload(&status, &PairingState::Off, Some(&device), false, false);
        assert!(
            unprivileged.get("connectedDevice").is_none(),
            "{unprivileged}"
        );
    }

    #[test]
    fn the_status_payload_omits_connected_device_when_none_is_live() {
        let payload = signal_status_payload(
            &SignalSourceStatus::default(),
            &PairingState::Off,
            None,
            false,
            true,
        );
        assert!(payload.get("connectedDevice").is_none(), "{payload}");
    }

    #[tokio::test]
    async fn signal_status_hands_the_live_code_only_to_a_caller_who_may_pair() {
        let _env_guard = SignalEnvGuard::acquire(&[
            (BROKER_URL_VAR, Some(TEST_BROKER_URL)),
            (ALLOW_REMOTE_PAIRING_VAR, None),
        ])
        .await;
        let mut env = TestEnv::new().await;
        let services = env.services.clone();
        services
            .signal
            .lock()
            .await
            .start_pairing(&env.app, Vec::new())
            .await;

        handle_signal_status(&services, Value::Null, &mut env.ctx())
            .await
            .unwrap();
        handle_signal_status(&services, Value::Null, &mut loopback_ctx(&mut env))
            .await
            .unwrap();
        services.signal.lock().await.stop_pairing().await;

        let frames = env.drain();
        let remote = &frames[0]["data"];
        assert_eq!(remote["pairing"], "waiting");
        assert!(remote.get("code").is_none(), "{remote}");
        assert!(remote.get("expiresAtMs").is_none(), "{remote}");

        let local = &frames[1]["data"];
        assert_eq!(local["pairing"], "waiting");
        assert!(local["code"].as_str().is_some(), "{local}");
        assert!(local["expiresAtMs"].as_i64().is_some(), "{local}");
    }

    #[tokio::test]
    async fn the_env_opt_in_admits_a_non_loopback_caller() {
        let _env_guard = SignalEnvGuard::acquire(&[(ALLOW_REMOTE_PAIRING_VAR, Some("1"))]).await;
        let mut env = TestEnv::new().await;

        assert!(admit_pairing_control(
            MessageType::SignalStartPairing,
            &env.ctx()
        ));
        assert!(env.drain().is_empty(), "an admitted caller gets no refusal");
    }

    #[tokio::test]
    async fn subscribe_live_is_not_gated_by_the_connection_origin() {
        let mut env = TestEnv::new().await;
        handle_subscribe_live(Value::Null, &mut env.ctx())
            .await
            .unwrap();

        let frames = env.drain();
        assert_eq!(frames[0]["type"], "subscribe_live");
        assert_eq!(frames[0]["data"]["active"], true);
    }
}
