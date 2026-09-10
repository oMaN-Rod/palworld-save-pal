use std::time::Duration;

use futures::{SinkExt, StreamExt};
use rand::Rng;
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::Message;
use tokio_util::sync::CancellationToken;

use super::peer::IceServerConfig;

const PRODUCTION_BROKER_URL: &str = "wss://palstudio.app";
const TURN_FETCH_TIMEOUT: Duration = Duration::from_secs(5);
const MIN_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(60);
const ROOM_EXPIRED_CLOSE: u16 = 4001;

pub struct BrokerConfig {
    pub base_ws_url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomKind {
    Pair,
    Meet,
}

impl RoomKind {
    fn query(self) -> &'static str {
        match self {
            RoomKind::Pair => "",
            RoomKind::Meet => "&kind=meet",
        }
    }
}

fn link_url(base_ws_url: &str, room_id: &str, kind: RoomKind) -> String {
    format!(
        "{base_ws_url}/signal/ws?room={room_id}&role=host{}",
        kind.query()
    )
}

pub fn default_broker_url() -> String {
    std::env::var("PS_SIGNAL_BROKER_URL").unwrap_or_else(|_| PRODUCTION_BROKER_URL.to_string())
}

pub fn turn_credential_url(base_ws_url: &str) -> String {
    let http_base = if let Some(rest) = base_ws_url.strip_prefix("wss://") {
        format!("https://{rest}")
    } else if let Some(rest) = base_ws_url.strip_prefix("ws://") {
        format!("http://{rest}")
    } else {
        base_ws_url.to_string()
    };
    format!("{}/signal/turn", http_base.trim_end_matches('/'))
}

#[derive(Deserialize)]
struct TurnResponse {
    urls: Vec<String>,
    username: String,
    credential: String,
}

fn parse_turn_response(body: &str) -> Option<Vec<IceServerConfig>> {
    let parsed: TurnResponse = serde_json::from_str(body).ok()?;
    if parsed.urls.is_empty() {
        return None;
    }
    Some(vec![IceServerConfig {
        urls: parsed.urls,
        username: Some(parsed.username),
        credential: Some(parsed.credential),
    }])
}

pub async fn fetch_turn_ice_servers() -> Vec<IceServerConfig> {
    let url = turn_credential_url(&default_broker_url());
    let client = match reqwest::Client::builder()
        .timeout(TURN_FETCH_TIMEOUT)
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            tracing::warn!(%error, "building the TURN credential client failed, dialing STUN-only");
            return Vec::new();
        }
    };

    let response = match client.get(&url).send().await {
        Ok(response) => response,
        Err(error) => {
            tracing::warn!(%error, "fetching TURN credentials failed, dialing STUN-only");
            return Vec::new();
        }
    };
    if !response.status().is_success() {
        tracing::warn!(
            status = %response.status(),
            "TURN credential endpoint returned an error, dialing STUN-only"
        );
        return Vec::new();
    }
    let body = match response.text().await {
        Ok(body) => body,
        Err(error) => {
            tracing::warn!(%error, "reading the TURN credential response failed, dialing STUN-only");
            return Vec::new();
        }
    };
    match parse_turn_response(&body) {
        Some(servers) => servers,
        None => {
            tracing::warn!("TURN credential response was malformed, dialing STUN-only");
            Vec::new()
        }
    }
}

struct Backoff {
    current: Duration,
}

impl Backoff {
    fn new() -> Self {
        Self {
            current: MIN_BACKOFF,
        }
    }

    fn on_success(&mut self) {
        self.current = MIN_BACKOFF;
    }

    fn next_delay(&mut self) -> Duration {
        let half = self.current / 2;
        let delay = half + rand::rng().random_range(Duration::ZERO..=half);
        self.current = (self.current * 2).min(MAX_BACKOFF);
        delay
    }
}

enum ConnectionEnd {
    Cancelled,
    LinkClosed,
    RoomExpired,
    Disconnected,
}

pub async fn run_host_broker_link(
    cfg: BrokerConfig,
    room_id: String,
    kind: RoomKind,
    mut to_guest_rx: mpsc::Receiver<String>,
    from_guest_tx: mpsc::Sender<String>,
    cancel: CancellationToken,
) {
    let url = link_url(&cfg.base_ws_url, &room_id, kind);
    let mut backoff = Backoff::new();

    while !cancel.is_cancelled() {
        let end = run_connection(
            &url,
            &mut to_guest_rx,
            &from_guest_tx,
            &cancel,
            &mut backoff,
        )
        .await;
        match end {
            ConnectionEnd::Cancelled => {
                tracing::debug!("broker link: cancelled");
                break;
            }
            ConnectionEnd::LinkClosed => {
                tracing::debug!("broker link: ending because a channel half closed");
                break;
            }
            ConnectionEnd::RoomExpired => {
                tracing::debug!("broker link: the room expired, not reconnecting");
                break;
            }
            ConnectionEnd::Disconnected => {
                tracing::debug!("broker link: disconnected, will reconnect");
            }
        }
        if cancel.is_cancelled() {
            break;
        }

        let wait = backoff.next_delay();
        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = tokio::time::sleep(wait) => {}
        }
    }
}

fn close_end(frame: Option<CloseFrame<'_>>) -> ConnectionEnd {
    match frame {
        Some(frame) if u16::from(frame.code) == ROOM_EXPIRED_CLOSE => ConnectionEnd::RoomExpired,
        _ => ConnectionEnd::Disconnected,
    }
}

async fn run_connection(
    url: &str,
    to_guest_rx: &mut mpsc::Receiver<String>,
    from_guest_tx: &mpsc::Sender<String>,
    cancel: &CancellationToken,
    backoff: &mut Backoff,
) -> ConnectionEnd {
    let socket = tokio::select! {
        _ = cancel.cancelled() => return ConnectionEnd::Cancelled,
        connected = tokio_tungstenite::connect_async(url) => match connected {
            Ok((socket, _)) => socket,
            Err(_) => return ConnectionEnd::Disconnected,
        },
    };
    backoff.on_success();
    let (mut write, mut read) = socket.split();

    loop {
        tokio::select! {
            _ = cancel.cancelled() => return ConnectionEnd::Cancelled,
            outgoing = to_guest_rx.recv() => {
                let Some(text) = outgoing else { return ConnectionEnd::LinkClosed };
                if write.send(Message::Text(text)).await.is_err() {
                    return ConnectionEnd::Disconnected;
                }
            }
            incoming = read.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        if from_guest_tx.send(text).await.is_err() {
                            return ConnectionEnd::LinkClosed;
                        }
                    }
                    Some(Ok(Message::Close(frame))) => return close_end(frame),
                    None => return ConnectionEnd::Disconnected,
                    Some(Err(_)) => return ConnectionEnd::Disconnected,
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signal::test_support::SignalEnvGuard;

    fn closed_with(code: u16) -> Option<CloseFrame<'static>> {
        Some(CloseFrame {
            code: code.into(),
            reason: "".into(),
        })
    }

    #[test]
    fn only_the_room_expiry_close_stops_the_link() {
        assert!(matches!(
            close_end(closed_with(ROOM_EXPIRED_CLOSE)),
            ConnectionEnd::RoomExpired
        ));
        assert!(matches!(
            close_end(closed_with(4008)),
            ConnectionEnd::Disconnected
        ));
        assert!(matches!(
            close_end(closed_with(1001)),
            ConnectionEnd::Disconnected
        ));
        assert!(matches!(close_end(None), ConnectionEnd::Disconnected));
    }

    #[test]
    fn only_a_meet_link_names_its_room_kind_on_the_upgrade() {
        assert_eq!(
            link_url("wss://broker.example", "abcd", RoomKind::Pair),
            "wss://broker.example/signal/ws?room=abcd&role=host"
        );
        assert_eq!(
            link_url("wss://broker.example", "abcd", RoomKind::Meet),
            "wss://broker.example/signal/ws?room=abcd&role=host&kind=meet"
        );
    }

    #[tokio::test]
    async fn default_broker_url_uses_the_compiled_in_default_when_unset() {
        let _env_guard = SignalEnvGuard::acquire(&[("PS_SIGNAL_BROKER_URL", None)]).await;
        assert!(std::env::var("PS_SIGNAL_BROKER_URL").is_err());
        assert_eq!(default_broker_url(), "wss://palstudio.app");
    }

    #[test]
    fn turn_credential_url_swaps_schemes() {
        assert_eq!(
            turn_credential_url("wss://palstudio.app"),
            "https://palstudio.app/signal/turn"
        );
        assert_eq!(
            turn_credential_url("ws://localhost:8787"),
            "http://localhost:8787/signal/turn"
        );
        assert_eq!(
            turn_credential_url("wss://host/"),
            "https://host/signal/turn"
        );
    }

    #[test]
    fn turn_response_parses_into_ice_servers() {
        let json = r#"{"urls":["turn:t.example:3478?transport=udp","turns:t.example:443?transport=tcp"],"username":"1700000000:ps","credential":"abc=","ttl_seconds":3600}"#;
        let servers = parse_turn_response(json).unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].urls.len(), 2);
        assert_eq!(servers[0].username.as_deref(), Some("1700000000:ps"));
        assert_eq!(servers[0].credential.as_deref(), Some("abc="));
    }

    #[test]
    fn malformed_turn_response_yields_none() {
        assert!(parse_turn_response("{}").is_none());
        assert!(parse_turn_response("not json").is_none());
    }

    #[test]
    fn backoff_resets_to_the_floor_on_success() {
        let mut backoff = Backoff::new();
        backoff.next_delay();
        backoff.next_delay();
        backoff.next_delay();

        backoff.on_success();
        let wait = backoff.next_delay();

        assert!(wait >= MIN_BACKOFF / 2);
        assert!(wait <= MIN_BACKOFF);
    }

    #[test]
    fn backoff_doubles_on_repeated_failure() {
        let mut backoff = Backoff::new();

        let first = backoff.next_delay();
        assert!(first >= MIN_BACKOFF / 2 && first <= MIN_BACKOFF);

        let second = backoff.next_delay();
        assert!(second >= MIN_BACKOFF && second <= MIN_BACKOFF * 2);

        let third = backoff.next_delay();
        assert!(third >= MIN_BACKOFF * 2 && third <= MIN_BACKOFF * 4);
    }

    #[test]
    fn backoff_caps_at_the_maximum() {
        let mut backoff = Backoff::new();
        for _ in 0..20 {
            backoff.next_delay();
        }

        let wait = backoff.next_delay();
        assert!(wait >= MAX_BACKOFF / 2);
        assert!(wait <= MAX_BACKOFF);
    }

    #[test]
    fn backoff_delay_stays_within_the_upper_half_of_the_current_value() {
        for _ in 0..100 {
            let mut backoff = Backoff {
                current: Duration::from_secs(4),
            };
            let wait = backoff.next_delay();
            assert!(wait >= Duration::from_secs(2));
            assert!(wait <= Duration::from_secs(4));
        }
    }
}
