use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use ps_app::live::{LiveActor, LiveFrame, LiveSourceKind, SignalSourceStatus, SourceHealth};
use ps_server::signal::crypto::{derive_keys, open, seal};
use ps_server::signal::peer::{HostPeer, PeerEvent};
use serde_json::{json, Value};
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;
use webrtc::data_channel::{
    DataChannel, DataChannelEvent, RTCDataChannelInit, RTCDataChannelState,
};
use webrtc::peer_connection::{
    PeerConnection, PeerConnectionBuilder, PeerConnectionEventHandler, RTCIceCandidateInit,
    RTCPeerConnectionIceEvent, RTCSessionDescription,
};

const CODE: &str = "TEST-PAIR-CODE-FOR-LOOPBACK";
const BASE64: base64::engine::general_purpose::GeneralPurpose =
    base64::engine::general_purpose::STANDARD;

fn seal_guest(key: &[u8; 32], envelope: Value) -> String {
    BASE64.encode(seal(key, envelope.to_string().as_bytes(), b"guest"))
}

fn open_host(key: &[u8; 32], blob: &str) -> Option<Value> {
    let sealed = BASE64.decode(blob).ok()?;
    let plaintext = open(key, &sealed, b"host").ok()?;
    serde_json::from_slice(&plaintext).ok()
}

fn frame(seq: u64, actors: usize) -> LiveFrame {
    LiveFrame {
        seq,
        source: LiveSourceKind::File,
        captured_at_ms: 1_756_500_000_000,
        observed_at_ms: 1_756_500_000_500,
        fps: Some(59.5),
        ingame_time: Some("07:09".into()),
        ingame_days: Some(30),
        actors: (0..actors)
            .map(|index| LiveActor {
                id: format!("actor-{index:08x}-abcdef0123456789"),
                kind: "wild".into(),
                x: index as f64,
                y: -138_238.3,
                z: 2916.1,
                yaw: Some(26.3),
                name: Some(format!("Pal number {index}")),
                level: Some(50),
                hp: Some(8575),
                max_hp: Some(8575),
                guild: Some("A guild with a fairly long name".into()),
                owner: Some(format!("owner-{index:08x}")),
                species: Some("SomeLongSpeciesIdentifier".into()),
                active: Some(true),
            })
            .collect(),
    }
}

struct GuestHandler {
    seal_key: [u8; 32],
    to_host: mpsc::Sender<String>,
}

#[async_trait::async_trait]
impl PeerConnectionEventHandler for GuestHandler {
    async fn on_ice_candidate(&self, event: RTCPeerConnectionIceEvent) {
        let Ok(init) = event.candidate.to_json() else {
            return;
        };
        let Ok(candidate) = serde_json::to_string(&init) else {
            return;
        };
        let blob = seal_guest(&self.seal_key, json!({ "t": "ice", "c": candidate }));
        let _ = self.to_host.send(blob).await;
    }
}

type Inbox = mpsc::Receiver<String>;

struct Loopback {
    guest: Arc<dyn PeerConnection>,
    ctl: Arc<dyn DataChannel>,
    ctl_inbox: Inbox,
    live_inbox: Inbox,
    events: mpsc::Receiver<PeerEvent>,
    live_bus: watch::Sender<Option<LiveFrame>>,
    status_bus: watch::Sender<SignalSourceStatus>,
    ctl_out: mpsc::Sender<String>,
    ctl_in: mpsc::Receiver<Value>,
    cancel: CancellationToken,
    host: tokio::task::JoinHandle<()>,
}

impl Loopback {
    async fn start(preamble: Vec<String>) -> Self {
        let seal_key = derive_keys(CODE).seal_key;

        let (to_host_tx, to_host_rx) = mpsc::channel::<String>(64);
        let (to_guest_tx, mut to_guest_rx) = mpsc::channel::<String>(64);
        let (event_tx, events) = mpsc::channel::<PeerEvent>(64);
        let (live_bus, live_rx) = watch::channel::<Option<LiveFrame>>(None);
        let (status_bus, status_rx) = watch::channel(SignalSourceStatus::default());
        let (ctl_out, ctl_out_rx) = mpsc::channel::<String>(16);
        let (ctl_in_tx, ctl_in) = mpsc::channel::<Value>(16);
        let cancel = CancellationToken::new();

        let host_cancel = cancel.clone();
        let host = tokio::spawn(async move {
            HostPeer::run(
                derive_keys(CODE),
                Vec::new(),
                to_host_rx,
                to_guest_tx,
                live_rx,
                status_rx,
                ctl_out_rx,
                ctl_in_tx,
                event_tx,
                host_cancel,
            )
            .await
            .expect("the host peer runs to cancellation without error");
        });

        for blob in preamble {
            to_host_tx.send(blob).await.expect("host is listening");
        }

        let guest: Arc<dyn PeerConnection> = Arc::new(
            PeerConnectionBuilder::new()
                .with_handler(Arc::new(GuestHandler {
                    seal_key,
                    to_host: to_host_tx.clone(),
                }))
                .with_udp_addrs(vec!["127.0.0.1:0".to_string()])
                .build()
                .await
                .expect("guest peer builds"),
        );

        let ctl = guest
            .create_data_channel("ctl", None)
            .await
            .expect("ctl channel");
        let live = guest
            .create_data_channel(
                "live",
                Some(RTCDataChannelInit {
                    ordered: false,
                    max_retransmits: Some(0),
                    ..Default::default()
                }),
            )
            .await
            .expect("live channel");

        let ctl_inbox = spawn_inbox(ctl.clone());
        let live_inbox = spawn_inbox(live);

        let answering = guest.clone();
        tokio::spawn(async move {
            while let Some(blob) = to_guest_rx.recv().await {
                let Some(envelope) = open_host(&seal_key, &blob) else {
                    continue;
                };
                match envelope["t"].as_str() {
                    Some("answer") => {
                        let sdp = envelope["sdp"].as_str().unwrap_or_default().to_string();
                        let answer = RTCSessionDescription::answer(sdp).expect("valid answer sdp");
                        answering
                            .set_remote_description(answer)
                            .await
                            .expect("guest set remote description");
                    }
                    Some("ice") => {
                        let candidate = envelope["c"].as_str().unwrap_or_default();
                        if let Ok(init) = serde_json::from_str::<RTCIceCandidateInit>(candidate) {
                            let _ = answering.add_ice_candidate(init).await;
                        }
                    }
                    _ => {}
                }
            }
        });

        let offer = guest.create_offer(None).await.expect("offer");
        to_host_tx
            .send(seal_guest(
                &seal_key,
                json!({ "t": "offer", "sdp": offer.sdp }),
            ))
            .await
            .expect("host is listening");
        guest
            .set_local_description(offer)
            .await
            .expect("guest set local description");

        Self {
            guest,
            ctl,
            ctl_inbox,
            live_inbox,
            events,
            live_bus,
            status_bus,
            ctl_out,
            ctl_in,
            cancel,
            host,
        }
    }

    async fn ctl_request(&mut self, request: Value) -> Value {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        while !matches!(self.ctl.ready_state().await, Ok(RTCDataChannelState::Open)) {
            assert!(tokio::time::Instant::now() < deadline, "ctl never opened");
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        self.ctl
            .send_text(&request.to_string())
            .await
            .expect("ctl accepts the request");
        serde_json::from_str(&next_message(&mut self.ctl_inbox, 10).await)
            .expect("ctl replies with JSON")
    }

    async fn shutdown(self) {
        self.cancel.cancel();
        self.guest.close().await.ok();
        tokio::time::timeout(Duration::from_secs(10), self.host)
            .await
            .expect("the host peer stops on cancellation")
            .expect("the host task does not panic");
    }
}

fn spawn_inbox(channel: Arc<dyn DataChannel>) -> Inbox {
    let (tx, rx) = mpsc::channel(256);
    tokio::spawn(async move {
        while let Some(event) = channel.poll().await {
            match event {
                DataChannelEvent::OnMessage(message) => {
                    let text = String::from_utf8_lossy(&message.data).to_string();
                    if tx.send(text).await.is_err() {
                        break;
                    }
                }
                DataChannelEvent::OnClose => break,
                _ => {}
            }
        }
    });
    rx
}

async fn next_message(inbox: &mut Inbox, seconds: u64) -> String {
    tokio::time::timeout(Duration::from_secs(seconds), inbox.recv())
        .await
        .expect("a message arrives before the deadline")
        .expect("the data channel stays open")
}

async fn next_frame(inbox: &mut Inbox, seconds: u64) -> (Value, u64) {
    let mut pending: BTreeMap<u64, BTreeMap<u64, String>> = BTreeMap::new();
    loop {
        let text = next_message(inbox, seconds).await;
        let value: Value = serde_json::from_str(&text).expect("live frames are JSON");
        let Some(parts) = value["parts"].as_u64() else {
            return (value, 1);
        };
        let seq = value["seq"].as_u64().expect("every part carries its seq");
        let part = value["part"]
            .as_u64()
            .expect("every part carries its index");
        let data = value["data"]
            .as_str()
            .expect("every part carries a data chunk")
            .to_string();
        let collected = pending.entry(seq).or_default();
        collected.insert(part, data);
        if collected.len() as u64 != parts {
            continue;
        }
        assert_eq!(
            collected.keys().copied().collect::<Vec<_>>(),
            (1..=parts).collect::<Vec<_>>(),
            "parts are numbered 1..=parts"
        );
        let joined: String = collected.values().cloned().collect();
        let assembled =
            serde_json::from_str(&joined).expect("the joined chunks are the serialized frame");
        return (assembled, parts);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn delivers_the_frame_already_on_the_bus_and_answers_ctl_requests() {
    let mut loopback = Loopback::start(Vec::new()).await;
    loopback.live_bus.send_replace(Some(frame(1, 3)));
    loopback.status_bus.send_replace(SignalSourceStatus {
        kind: Some(LiveSourceKind::File),
        health: SourceHealth::Ok,
        error: None,
        last_frame_ms: Some(1_756_500_000_500),
        actor_count: 3,
    });

    let connected = tokio::time::timeout(Duration::from_secs(15), loopback.events.recv())
        .await
        .expect("the host reports a connection state")
        .expect("the event channel stays open");
    assert_eq!(connected, PeerEvent::Connected);

    let (live, parts) = next_frame(&mut loopback.live_inbox, 10).await;
    assert_eq!(live["seq"], 1);
    assert_eq!(parts, 1, "a small frame goes out whole");
    assert_eq!(live["actors"].as_array().map(Vec::len), Some(3));

    let pong = loopback.ctl_request(json!({ "type": "ping" })).await;
    assert_eq!(pong["type"], "pong");

    let status = loopback.ctl_request(json!({ "type": "status" })).await;
    assert_eq!(status["type"], "status");
    assert_eq!(status["data"]["health"], "ok");
    assert_eq!(status["data"]["actorCount"], 3);

    loopback.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_corrupted_blob_does_not_prevent_the_handshake() {
    let seal_key = derive_keys(CODE).seal_key;
    let mut tampered = seal(&seal_key, b"{\"t\":\"offer\",\"sdp\":\"\"}", b"guest");
    let last = tampered.len() - 1;
    tampered[last] ^= 0x01;

    let preamble = vec![
        "this is not base64 at all !!!".to_string(),
        BASE64.encode([0u8; 8]),
        BASE64.encode(&tampered),
        BASE64.encode(seal(&seal_key, b"{\"t\":\"offer\",\"sdp\":\"\"}", b"host")),
    ];

    let mut loopback = Loopback::start(preamble).await;
    loopback.live_bus.send_replace(Some(frame(1, 2)));

    let pong = loopback.ctl_request(json!({ "type": "ping" })).await;
    assert_eq!(pong["type"], "pong");

    let (live, _) = next_frame(&mut loopback.live_inbox, 10).await;
    assert_eq!(live["seq"], 1);

    loopback.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cancelling_a_connected_peer_reports_that_it_ended() {
    let mut loopback = Loopback::start(Vec::new()).await;

    let connected = tokio::time::timeout(Duration::from_secs(15), loopback.events.recv())
        .await
        .expect("the host reports a connection state")
        .expect("the event channel stays open");
    assert_eq!(connected, PeerEvent::Connected);

    loopback.cancel.cancel();

    let ended = tokio::time::timeout(Duration::from_secs(10), loopback.events.recv())
        .await
        .expect("the host reports the end of the connection")
        .expect("the event channel stays open");
    assert_eq!(ended, PeerEvent::Disconnected);

    loopback.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_large_frame_arrives_complete_across_parts() {
    let mut loopback = Loopback::start(Vec::new()).await;
    let big = frame(1, 400);
    assert!(
        serde_json::to_string(&big).expect("frame serializes").len() > 65_536,
        "the fixture must exceed one SCTP message to exercise splitting"
    );
    loopback.live_bus.send_replace(Some(big));

    let (live, parts) = next_frame(&mut loopback.live_inbox, 20).await;
    assert_eq!(live["seq"], 1);
    assert!(parts > 1, "the frame must have travelled as several parts");
    let actors = live["actors"].as_array().expect("actors survive the split");
    assert_eq!(actors.len(), 400);
    assert_eq!(actors[0]["id"], "actor-00000000-abcdef0123456789");
    assert_eq!(actors[399]["id"], "actor-0000018f-abcdef0123456789");

    loopback.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_ctl_seam_carries_envelopes_in_both_directions() {
    let mut loopback = Loopback::start(Vec::new()).await;

    let pong = loopback.ctl_request(json!({ "type": "ping" })).await;
    assert_eq!(pong["type"], "pong");
    let status = loopback.ctl_request(json!({ "type": "status" })).await;
    assert_eq!(status["type"], "status");

    loopback
        .ctl_out
        .send(json!({ "type": "probe", "data": { "n": 1 } }).to_string())
        .await
        .expect("the host peer is listening for outbound envelopes");
    let received: Value = serde_json::from_str(&next_message(&mut loopback.ctl_inbox, 10).await)
        .expect("the guest receives valid JSON");
    assert_eq!(received, json!({ "type": "probe", "data": { "n": 1 } }));

    loopback
        .ctl
        .send_text(&json!({ "type": "probe_back", "data": { "n": 2 } }).to_string())
        .await
        .expect("ctl accepts the message");
    let forwarded = tokio::time::timeout(Duration::from_secs(10), loopback.ctl_in.recv())
        .await
        .expect("the envelope arrives before the deadline")
        .expect("the host peer keeps ctl_in_tx alive");
    assert_eq!(forwarded, json!({ "type": "probe_back", "data": { "n": 2 } }));

    loopback.shutdown().await;
}
