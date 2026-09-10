use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use base64::Engine;
use ps_app::live::{LiveFrame, SignalSourceStatus};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;
use webrtc::data_channel::{DataChannel, DataChannelEvent};
use webrtc::peer_connection::{
    PeerConnection, PeerConnectionBuilder, PeerConnectionEventHandler, RTCConfigurationBuilder,
    RTCIceCandidateInit, RTCIceServer, RTCPeerConnectionIceEvent, RTCPeerConnectionState,
    RTCSessionDescription,
};

use super::crypto::{open, seal, PairingKeys};

const BASE64: base64::engine::general_purpose::GeneralPurpose =
    base64::engine::general_purpose::STANDARD;

const GUEST_AAD: &[u8] = b"guest";
const HOST_AAD: &[u8] = b"host";

const STUN_URLS: [&str; 2] = [
    "stun:stun.cloudflare.com:3478",
    "stun:stun.l.google.com:19302",
];

const LOCAL_ADDRS: [&str; 3] = ["0.0.0.0:0", "[::]:0", "127.0.0.1:0"];

const DEFAULT_MAX_MESSAGE_SIZE: usize = 65_536;

/// What this process can actually put on the wire. The local stack's setting
/// engine defaults to a bounded 65536 and the association takes the smaller of
/// that and the peer's advertisement, so a browser offering 262144 does not
/// raise our ceiling. A larger message fails as `ErrOutboundPacketTooLarge` on
/// the driver's write path, after `send_text` has already returned `Ok` and
/// where nothing here can observe it, so the frame would simply vanish.
const LOCAL_MAX_MESSAGE_SIZE: usize = 65_536;

const LIVE_HIGH_WATER: usize = 256 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum PeerError {
    #[error("webrtc error: {0}")]
    WebRtc(#[from] webrtc::error::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerEvent {
    Connected,
    Disconnected,
    Failed,
}

#[derive(Clone)]
pub struct IceServerConfig {
    pub urls: Vec<String>,
    pub username: Option<String>,
    pub credential: Option<String>,
}

impl std::fmt::Debug for IceServerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IceServerConfig")
            .field("urls", &self.urls)
            .field("username", &self.username.as_ref().map(|_| "<redacted>"))
            .field(
                "credential",
                &self.credential.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "t")]
enum Envelope {
    #[serde(rename = "offer")]
    Offer { sdp: String },
    #[serde(rename = "answer")]
    Answer { sdp: String },
    #[serde(rename = "ice")]
    Ice { c: String },
}

pub struct HostPeer {
    connection: Arc<dyn PeerConnection>,
    send_budget: Arc<AtomicUsize>,
}

impl HostPeer {
    #[allow(clippy::too_many_arguments)]
    pub async fn run(
        keys: PairingKeys,
        ice_servers: Vec<IceServerConfig>,
        mut signaling_rx: mpsc::Receiver<String>,
        signaling_tx: mpsc::Sender<String>,
        live_rx: watch::Receiver<Option<LiveFrame>>,
        status_rx: watch::Receiver<SignalSourceStatus>,
        ctl_out_rx: mpsc::Receiver<String>,
        ctl_in_tx: mpsc::Sender<Value>,
        events: mpsc::Sender<PeerEvent>,
        cancel: CancellationToken,
    ) -> Result<(), PeerError> {
        let seal_key = keys.seal_key;
        let send_budget = Arc::new(AtomicUsize::new(DEFAULT_MAX_MESSAGE_SIZE));
        let (state_tx, mut state_rx) = mpsc::channel(16);
        let channels = cancel.child_token();

        let connection: Arc<dyn PeerConnection> = Arc::new(
            PeerConnectionBuilder::new()
                .with_configuration(
                    RTCConfigurationBuilder::new()
                        .with_ice_servers(ice_server_list(&ice_servers))
                        .build(),
                )
                .with_handler(Arc::new(HostHandler {
                    seal_key,
                    signaling_tx: signaling_tx.clone(),
                    states: state_tx,
                    live_rx,
                    status_rx,
                    send_budget: send_budget.clone(),
                    ctl_out_rx: std::sync::Mutex::new(Some(ctl_out_rx)),
                    ctl_in_tx,
                    cancel: channels.clone(),
                }))
                .with_udp_addrs(LOCAL_ADDRS.iter().map(|a| (*a).to_string()).collect())
                .build()
                .await?,
        );

        let peer = Self {
            connection,
            send_budget,
        };
        let outcome = peer
            .drive(
                &seal_key,
                &mut signaling_rx,
                &signaling_tx,
                &mut state_rx,
                &events,
                &cancel,
            )
            .await;
        channels.cancel();
        peer.connection.close().await.ok();
        outcome
    }

    async fn drive(
        &self,
        seal_key: &[u8; 32],
        signaling_rx: &mut mpsc::Receiver<String>,
        signaling_tx: &mpsc::Sender<String>,
        state_rx: &mut mpsc::Receiver<RTCPeerConnectionState>,
        events: &mpsc::Sender<PeerEvent>,
        cancel: &CancellationToken,
    ) -> Result<(), PeerError> {
        let mut early_candidates: Vec<RTCIceCandidateInit> = Vec::new();
        let mut answered = false;
        let mut signaling_open = true;
        let mut connected = false;

        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                state = state_rx.recv() => {
                    let Some(state) = state else { break };
                    let Some(event) = peer_event(state) else { continue };
                    if events.send(event).await.is_err() {
                        break;
                    }
                    connected = event == PeerEvent::Connected;
                    if matches!(state, RTCPeerConnectionState::Failed | RTCPeerConnectionState::Closed) {
                        break;
                    }
                }
                blob = signaling_rx.recv(), if signaling_open => {
                    let Some(blob) = blob else {
                        signaling_open = false;
                        continue;
                    };
                    let Some(envelope) = open_envelope(seal_key, &blob) else { continue };
                    match signaling_step(envelope, answered) {
                        SignalingStep::Answer(sdp) => {
                            self.answer(seal_key, sdp, signaling_tx, cancel).await?;
                            answered = true;
                            for candidate in early_candidates.drain(..) {
                                self.connection.add_ice_candidate(candidate).await.ok();
                            }
                        }
                        SignalingStep::AddCandidate(c) => {
                            let Ok(candidate) = serde_json::from_str::<RTCIceCandidateInit>(&c) else {
                                continue;
                            };
                            self.connection.add_ice_candidate(candidate).await.ok();
                        }
                        SignalingStep::HoldCandidate(c) => {
                            let Ok(candidate) = serde_json::from_str::<RTCIceCandidateInit>(&c) else {
                                continue;
                            };
                            early_candidates.push(candidate);
                        }
                        SignalingStep::Ignore => {}
                    }
                }
            }
        }
        if connected {
            events.send(PeerEvent::Disconnected).await.ok();
        }
        Ok(())
    }

    async fn answer(
        &self,
        seal_key: &[u8; 32],
        sdp: String,
        signaling_tx: &mpsc::Sender<String>,
        cancel: &CancellationToken,
    ) -> Result<(), PeerError> {
        let offer = RTCSessionDescription::offer(sdp)?;
        self.send_budget
            .store(negotiated_send_budget(&offer.sdp), Ordering::Relaxed);
        self.connection.set_remote_description(offer).await?;

        let answer = self.connection.create_answer(None).await?;
        let blob = seal_envelope(
            seal_key,
            &Envelope::Answer {
                sdp: answer.sdp.clone(),
            },
        );
        tokio::select! {
            _ = cancel.cancelled() => return Ok(()),
            sent = signaling_tx.send(blob) => sent.ok(),
        };
        self.connection.set_local_description(answer).await?;
        Ok(())
    }
}

struct HostHandler {
    seal_key: [u8; 32],
    signaling_tx: mpsc::Sender<String>,
    states: mpsc::Sender<RTCPeerConnectionState>,
    live_rx: watch::Receiver<Option<LiveFrame>>,
    status_rx: watch::Receiver<SignalSourceStatus>,
    send_budget: Arc<AtomicUsize>,
    ctl_out_rx: std::sync::Mutex<Option<mpsc::Receiver<String>>>,
    ctl_in_tx: mpsc::Sender<Value>,
    cancel: CancellationToken,
}

#[async_trait::async_trait]
impl PeerConnectionEventHandler for HostHandler {
    async fn on_ice_candidate(&self, event: RTCPeerConnectionIceEvent) {
        let Ok(init) = event.candidate.to_json() else {
            return;
        };
        let Ok(candidate) = serde_json::to_string(&init) else {
            return;
        };
        let blob = seal_envelope(&self.seal_key, &Envelope::Ice { c: candidate });
        self.signaling_tx.try_send(blob).ok();
    }

    async fn on_connection_state_change(&self, state: RTCPeerConnectionState) {
        self.states.send(state).await.ok();
    }

    async fn on_data_channel(&self, channel: Arc<dyn DataChannel>) {
        match channel.label().await.unwrap_or_default().as_str() {
            "ctl" => {
                let ctl_out_rx = self
                    .ctl_out_rx
                    .lock()
                    .expect("ctl_out_rx mutex is never poisoned")
                    .take()
                    .unwrap_or_else(|| mpsc::channel(1).1);
                tokio::spawn(serve_ctl(
                    channel,
                    self.status_rx.clone(),
                    ctl_out_rx,
                    self.ctl_in_tx.clone(),
                    self.cancel.clone(),
                ));
            }
            "live" => {
                tokio::spawn(forward_live(
                    channel,
                    self.live_rx.clone(),
                    self.send_budget.clone(),
                    self.cancel.clone(),
                ));
            }
            _ => {}
        }
    }
}

async fn serve_ctl(
    channel: Arc<dyn DataChannel>,
    status_rx: watch::Receiver<SignalSourceStatus>,
    mut ctl_out_rx: mpsc::Receiver<String>,
    ctl_in_tx: mpsc::Sender<Value>,
    cancel: CancellationToken,
) {
    let mut open = false;
    let mut pending: Vec<String> = Vec::new();
    let mut ctl_out_open = true;
    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            event = channel.poll() => match event {
                Some(DataChannelEvent::OnOpen) => {
                    open = true;
                    for envelope in pending.drain(..) {
                        if channel.send_text(&envelope).await.is_err() {
                            break;
                        }
                    }
                }
                Some(DataChannelEvent::OnMessage(message)) => {
                    match ctl_reply(&message.data, &status_rx) {
                        Some(reply) => {
                            if channel.send_text(&reply).await.is_err() {
                                break;
                            }
                        }
                        None => {
                            if let Some(envelope) = parse_ctl_envelope(&message.data) {
                                tokio::select! {
                                    _ = cancel.cancelled() => break,
                                    result = ctl_in_tx.send(envelope) => {
                                        if result.is_err() { break; }
                                    }
                                }
                            }
                        }
                    }
                }
                None | Some(DataChannelEvent::OnClose) => break,
                Some(_) => {}
            },
            envelope = ctl_out_rx.recv(), if ctl_out_open => {
                let Some(envelope) = envelope else {
                    ctl_out_open = false;
                    continue;
                };
                if open {
                    if channel.send_text(&envelope).await.is_err() {
                        break;
                    }
                } else {
                    pending.push(envelope);
                }
            }
        }
    }
}

fn ctl_reply(request: &[u8], status_rx: &watch::Receiver<SignalSourceStatus>) -> Option<String> {
    let request: Value = serde_json::from_slice(request).ok()?;
    let reply = match request.get("type")?.as_str()? {
        "ping" => json!({ "type": "pong" }),
        "status" => {
            let status = status_rx.borrow().clone();
            json!({ "type": "status", "data": status })
        }
        _ => return None,
    };
    Some(reply.to_string())
}

fn parse_ctl_envelope(request: &[u8]) -> Option<Value> {
    let value: Value = serde_json::from_slice(request).ok()?;
    value.get("type")?.as_str()?;
    Some(value)
}

async fn forward_live(
    channel: Arc<dyn DataChannel>,
    mut live_rx: watch::Receiver<Option<LiveFrame>>,
    send_budget: Arc<AtomicUsize>,
    cancel: CancellationToken,
) {
    let mut open = false;
    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            event = channel.poll() => match event {
                Some(DataChannelEvent::OnOpen) => {
                    open = true;
                    let frame = live_rx.borrow_and_update().clone();
                    if let Some(frame) = frame {
                        send_frame(&channel, &frame, &send_budget).await;
                    }
                }
                None | Some(DataChannelEvent::OnClose) => break,
                Some(_) => {}
            },
            changed = live_rx.changed(), if open => {
                if changed.is_err() {
                    break;
                }
                let frame = live_rx.borrow_and_update().clone();
                if let Some(frame) = frame {
                    send_frame(&channel, &frame, &send_budget).await;
                }
            }
        }
    }
}

async fn send_frame(channel: &Arc<dyn DataChannel>, frame: &LiveFrame, send_budget: &AtomicUsize) {
    if channel.outstanding_bytes().await.unwrap_or(0) > LIVE_HIGH_WATER {
        return;
    }
    for part in split_frame(frame, send_budget.load(Ordering::Relaxed)) {
        if channel.send_text(&part).await.is_err() {
            return;
        }
    }
}

fn split_frame(frame: &LiveFrame, max_message: usize) -> Vec<String> {
    let Ok(whole) = serde_json::to_string(frame) else {
        return Vec::new();
    };
    if whole.len() <= max_message {
        return vec![whole];
    }

    let widest = whole.len();
    let budget = max_message
        .saturating_sub(frame_part(frame.seq, widest, widest, "").len())
        .max(1);

    let mut bounds = Vec::new();
    let mut start = 0;
    while start < whole.len() {
        let end = chunk_end(&whole, start, budget);
        bounds.push((start, end));
        start = end;
    }

    let parts = bounds.len();
    bounds
        .into_iter()
        .enumerate()
        .map(|(index, (from, to))| frame_part(frame.seq, index + 1, parts, &whole[from..to]))
        .collect()
}

fn frame_part(seq: u64, part: usize, parts: usize, data: &str) -> String {
    json!({ "seq": seq, "part": part, "parts": parts, "data": data }).to_string()
}

fn chunk_end(text: &str, start: usize, budget: usize) -> usize {
    let mut end = floor_char_boundary(text, (start + budget).min(text.len()));
    loop {
        if end <= start {
            return next_char_boundary(text, start);
        }
        let escaped = escaped_len(&text[start..end]);
        if escaped <= budget {
            return end;
        }
        end = floor_char_boundary(text, end.saturating_sub(escaped - budget).saturating_sub(1));
    }
}

fn escaped_len(text: &str) -> usize {
    serde_json::to_string(text).map_or(usize::MAX, |quoted| quoted.len() - 2)
}

fn floor_char_boundary(text: &str, mut index: usize) -> usize {
    if index >= text.len() {
        return text.len();
    }
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn next_char_boundary(text: &str, start: usize) -> usize {
    text[start..]
        .chars()
        .next()
        .map_or(text.len(), |c| start + c.len_utf8())
}

fn negotiated_send_budget(remote_sdp: &str) -> usize {
    sdp_max_message_size(remote_sdp)
        .unwrap_or(DEFAULT_MAX_MESSAGE_SIZE)
        .min(LOCAL_MAX_MESSAGE_SIZE)
}

fn sdp_max_message_size(sdp: &str) -> Option<usize> {
    sdp.lines()
        .find_map(|line| line.trim().strip_prefix("a=max-message-size:"))
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|size| *size > 0)
}

fn peer_event(state: RTCPeerConnectionState) -> Option<PeerEvent> {
    match state {
        RTCPeerConnectionState::Connected => Some(PeerEvent::Connected),
        RTCPeerConnectionState::Disconnected | RTCPeerConnectionState::Closed => {
            Some(PeerEvent::Disconnected)
        }
        RTCPeerConnectionState::Failed => Some(PeerEvent::Failed),
        _ => None,
    }
}

#[derive(Debug, PartialEq)]
enum SignalingStep {
    Answer(String),
    AddCandidate(String),
    HoldCandidate(String),
    Ignore,
}

fn signaling_step(envelope: Envelope, answered: bool) -> SignalingStep {
    match envelope {
        Envelope::Offer { sdp } if !answered => SignalingStep::Answer(sdp),
        Envelope::Ice { c } if answered => SignalingStep::AddCandidate(c),
        Envelope::Ice { c } => SignalingStep::HoldCandidate(c),
        Envelope::Offer { .. } | Envelope::Answer { .. } => SignalingStep::Ignore,
    }
}

fn ice_server_list(extra: &[IceServerConfig]) -> Vec<RTCIceServer> {
    let mut servers = vec![RTCIceServer {
        urls: STUN_URLS.iter().map(|url| (*url).to_string()).collect(),
        ..Default::default()
    }];
    servers.extend(extra.iter().map(|server| RTCIceServer {
        urls: server.urls.clone(),
        username: server.username.clone().unwrap_or_default(),
        credential: server.credential.clone().unwrap_or_default(),
    }));
    servers
}

fn seal_envelope(seal_key: &[u8; 32], envelope: &Envelope) -> String {
    let plaintext = serde_json::to_vec(envelope).expect("the envelope enum always serializes");
    BASE64.encode(seal(seal_key, &plaintext, HOST_AAD))
}

fn open_envelope(seal_key: &[u8; 32], blob: &str) -> Option<Envelope> {
    let sealed = BASE64.decode(blob.trim()).ok()?;
    let plaintext = open(seal_key, &sealed, GUEST_AAD).ok()?;
    serde_json::from_slice(&plaintext).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ps_app::live::{LiveActor, LiveSourceKind};

    fn frame(actors: usize) -> LiveFrame {
        LiveFrame {
            seq: 42,
            source: LiveSourceKind::File,
            captured_at_ms: 1,
            observed_at_ms: 2,
            fps: None,
            ingame_time: None,
            ingame_days: None,
            actors: (0..actors)
                .map(|index| LiveActor {
                    id: format!("actor-{index:08}"),
                    kind: "wild".into(),
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                    yaw: None,
                    name: Some("A pal with a name".into()),
                    level: Some(50),
                    hp: Some(100),
                    max_hp: Some(100),
                    guild: None,
                    owner: None,
                    species: Some("SomeSpecies".into()),
                    active: None,
                })
                .collect(),
        }
    }

    fn reassemble(messages: &[String]) -> Value {
        if messages.len() == 1 {
            return serde_json::from_str(&messages[0]).unwrap();
        }
        let mut chunks: Vec<(u64, String)> = messages
            .iter()
            .map(|message| {
                let value: Value = serde_json::from_str(message).unwrap();
                assert_eq!(value["seq"], 42);
                assert_eq!(value["parts"], messages.len());
                (
                    value["part"].as_u64().unwrap(),
                    value["data"].as_str().unwrap().to_string(),
                )
            })
            .collect();
        chunks.sort_by_key(|(part, _)| *part);
        assert_eq!(
            chunks.iter().map(|(part, _)| *part).collect::<Vec<_>>(),
            (1..=messages.len() as u64).collect::<Vec<_>>()
        );
        let joined: String = chunks.into_iter().map(|(_, data)| data).collect();
        serde_json::from_str(&joined).unwrap()
    }

    #[test]
    fn a_frame_that_fits_is_sent_whole_without_part_fields() {
        let messages = split_frame(&frame(2), 65_536);

        assert_eq!(messages.len(), 1);
        let value: Value = serde_json::from_str(&messages[0]).unwrap();
        assert!(value.get("part").is_none());
        assert!(value.get("parts").is_none());
        assert!(value.get("data").is_none());
        assert_eq!(value["actors"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn split_parts_fit_the_budget_and_rejoin_into_the_frame() {
        let original = frame(400);
        let messages = split_frame(&original, 4096);

        assert!(messages.len() > 1);
        for (index, message) in messages.iter().enumerate() {
            assert!(
                message.len() <= 4096,
                "part {index} is {} bytes",
                message.len()
            );
        }
        assert_eq!(
            reassemble(&messages),
            serde_json::to_value(&original).unwrap()
        );
    }

    #[test]
    fn a_budget_smaller_than_the_envelope_still_makes_progress() {
        let original = frame(2);
        let messages = split_frame(&original, 1);

        assert!(messages.len() > 1);
        assert_eq!(
            reassemble(&messages),
            serde_json::to_value(&original).unwrap()
        );
    }

    #[test]
    fn multi_byte_characters_are_never_cut_in_half() {
        let mut original = frame(40);
        for actor in &mut original.actors {
            actor.name = Some("name-\u{3042}\u{1f600}\u{7f8e}".repeat(4));
        }

        let messages = split_frame(&original, 512);

        assert!(messages.len() > 1);
        for message in &messages {
            assert!(message.len() <= 512);
        }
        assert_eq!(
            reassemble(&messages),
            serde_json::to_value(&original).unwrap()
        );
    }

    #[test]
    fn max_message_size_comes_from_the_sdp_attribute() {
        let sdp = "v=0\r\nm=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\na=max-message-size:262144\r\n";

        assert_eq!(sdp_max_message_size(sdp), Some(262_144));
        assert_eq!(sdp_max_message_size("v=0\r\n"), None);
        assert_eq!(sdp_max_message_size("a=max-message-size:0\r\n"), None);
    }

    #[test]
    fn a_browser_advertisement_is_capped_by_the_local_send_limit() {
        let sdp = "v=0\r\nm=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\na=max-message-size:262144\r\n";
        assert_eq!(sdp_max_message_size(sdp), Some(262_144));

        let budget = negotiated_send_budget(sdp);
        assert_eq!(budget, LOCAL_MAX_MESSAGE_SIZE);

        let original = frame(800);
        assert!(
            serde_json::to_string(&original).unwrap().len() > LOCAL_MAX_MESSAGE_SIZE,
            "the fixture must not fit in one message"
        );
        let unclamped = split_frame(&original, 262_144);
        assert_eq!(unclamped.len(), 1);
        assert!(unclamped[0].len() > LOCAL_MAX_MESSAGE_SIZE);

        let messages = split_frame(&original, budget);

        assert!(messages.len() > 1, "the frame must still be split");
        for (index, message) in messages.iter().enumerate() {
            assert!(
                message.len() <= LOCAL_MAX_MESSAGE_SIZE,
                "part {index} is {} bytes, which the local stack would drop",
                message.len()
            );
        }
        assert_eq!(
            reassemble(&messages),
            serde_json::to_value(&original).unwrap()
        );
    }

    #[test]
    fn an_absent_or_smaller_advertisement_is_honoured() {
        assert_eq!(negotiated_send_budget("v=0\r\n"), DEFAULT_MAX_MESSAGE_SIZE);
        assert_eq!(
            negotiated_send_budget("a=max-message-size:16384\r\n"),
            16_384
        );
    }

    #[test]
    fn the_default_stun_servers_lead_and_user_entries_follow() {
        let servers = ice_server_list(&[IceServerConfig {
            urls: vec!["turn:relay.example:3478".into()],
            username: Some("user".into()),
            credential: Some("secret".into()),
        }]);

        assert_eq!(servers.len(), 2);
        assert_eq!(servers[0].urls, STUN_URLS);
        assert_eq!(servers[1].credential, "secret");
    }

    #[test]
    fn debug_never_prints_turn_credentials() {
        let rendered = format!(
            "{:?}",
            IceServerConfig {
                urls: vec!["turn:relay.example:3478".into()],
                username: Some("turn-account".into()),
                credential: Some("hunter2".into()),
            }
        );

        assert!(!rendered.contains("hunter2"));
        assert!(!rendered.contains("turn-account"));
        assert!(rendered.contains("turn:relay.example:3478"));
    }

    #[test]
    fn ctl_answers_ping_and_status_and_ignores_the_rest() {
        let (_tx, rx) = watch::channel(SignalSourceStatus {
            actor_count: 7,
            ..Default::default()
        });

        let pong: Value =
            serde_json::from_str(&ctl_reply(br#"{"type":"ping"}"#, &rx).unwrap()).unwrap();
        assert_eq!(pong["type"], "pong");

        let status: Value =
            serde_json::from_str(&ctl_reply(br#"{"type":"status"}"#, &rx).unwrap()).unwrap();
        assert_eq!(status["type"], "status");
        assert_eq!(status["data"]["actorCount"], 7);

        assert!(ctl_reply(br#"{"type":"launch_the_missiles"}"#, &rx).is_none());
        assert!(ctl_reply(b"not json", &rx).is_none());
    }

    #[test]
    fn envelopes_round_trip_only_under_the_senders_role() {
        let key = [5u8; 32];
        let sealed = seal_envelope(
            &key,
            &Envelope::Answer {
                sdp: "v=0".to_string(),
            },
        );

        assert!(open_envelope(&key, &sealed).is_none());
        assert!(open_envelope(&key, "not base64 !!!").is_none());

        let guest = BASE64.encode(seal(&key, br#"{"t":"offer","sdp":"v=0"}"#, GUEST_AAD));
        assert!(matches!(
            open_envelope(&key, &guest),
            Some(Envelope::Offer { sdp }) if sdp == "v=0"
        ));
    }

    #[test]
    fn a_second_offer_after_answering_is_ignored() {
        let offer = || Envelope::Offer {
            sdp: "v=0".to_string(),
        };
        assert_eq!(
            signaling_step(offer(), false),
            SignalingStep::Answer("v=0".to_string())
        );
        assert_eq!(signaling_step(offer(), true), SignalingStep::Ignore);
    }

    #[test]
    fn candidates_wait_for_the_answer_and_flow_after_it() {
        let ice = || Envelope::Ice {
            c: "{}".to_string(),
        };
        assert_eq!(
            signaling_step(ice(), false),
            SignalingStep::HoldCandidate("{}".to_string())
        );
        assert_eq!(
            signaling_step(ice(), true),
            SignalingStep::AddCandidate("{}".to_string())
        );
        assert_eq!(
            signaling_step(
                Envelope::Answer {
                    sdp: "v=0".to_string()
                },
                true
            ),
            SignalingStep::Ignore
        );
    }
}
