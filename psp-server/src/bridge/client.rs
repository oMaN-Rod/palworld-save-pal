use std::collections::HashMap;

use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tokio_util::sync::CancellationToken;

use super::protocol::{BridgeEndpointFile, BridgeEnvelope, BridgeErrorData, BRIDGE_PROTOCOL_VERSION};
use super::service::BridgeError;

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
pub type WsWriteHalf = SplitSink<WsStream, Message>;
pub type WsReadHalf = SplitStream<WsStream>;

const HELLO_ID: &str = "h1";
const AUTH_ID: &str = "h2";

pub enum Command {
    Request {
        id: String,
        kind: String,
        data: Value,
        reply: oneshot::Sender<Result<Value, BridgeError>>,
    },
    CancelRequest { id: String },
}

impl Command {
    pub fn fail_offline(self) {
        if let Command::Request { reply, .. } = self {
            let _ = reply.send(Err(BridgeError::Offline));
        }
    }
}

pub enum ConnectError {
    Cancelled,
    Transport,
    Auth { code: String },
}

pub enum ConnectionEnd {
    Cancelled,
    Disconnected,
}

pub struct Connected {
    pub write: WsWriteHalf,
    pub read: WsReadHalf,
    pub mod_version: String,
}

async fn send_envelope(
    write: &mut WsWriteHalf,
    id: &str,
    kind: &str,
    data: Value,
) -> Result<(), ()> {
    let envelope = BridgeEnvelope {
        id: id.to_string(),
        kind: kind.to_string(),
        data,
    };
    let text = serde_json::to_string(&envelope).map_err(|_| ())?;
    write.send(Message::Text(text.into())).await.map_err(|_| ())
}

enum RecvOutcome {
    Cancelled,
    Closed,
}

async fn recv_envelope(
    read: &mut WsReadHalf,
    cancel: &CancellationToken,
) -> Result<BridgeEnvelope, RecvOutcome> {
    loop {
        tokio::select! {
            _ = cancel.cancelled() => return Err(RecvOutcome::Cancelled),
            message = read.next() => match message {
                Some(Ok(Message::Text(text))) => {
                    return serde_json::from_str::<BridgeEnvelope>(&text).map_err(|_| RecvOutcome::Closed);
                }
                Some(Ok(Message::Close(_))) | None => return Err(RecvOutcome::Closed),
                Some(Err(_)) => return Err(RecvOutcome::Closed),
                _ => continue,
            },
        }
    }
}

fn recv_error_to_connect_error(error: RecvOutcome) -> ConnectError {
    match error {
        RecvOutcome::Cancelled => ConnectError::Cancelled,
        RecvOutcome::Closed => ConnectError::Transport,
    }
}

pub async fn connect_and_handshake(
    endpoint: &BridgeEndpointFile,
    cancel: &CancellationToken,
) -> Result<Connected, ConnectError> {
    let url = format!("ws://127.0.0.1:{}", endpoint.port);
    let socket = tokio::select! {
        _ = cancel.cancelled() => return Err(ConnectError::Cancelled),
        connected = tokio_tungstenite::connect_async(&url) => match connected {
            Ok((socket, _)) => socket,
            Err(_) => return Err(ConnectError::Transport),
        },
    };
    let (mut write, mut read) = socket.split();

    send_envelope(
        &mut write,
        HELLO_ID,
        "hello",
        serde_json::json!({ "protocolVersion": BRIDGE_PROTOCOL_VERSION }),
    )
    .await
    .map_err(|_| ConnectError::Transport)?;
    let hello_reply = recv_envelope(&mut read, cancel)
        .await
        .map_err(recv_error_to_connect_error)?;
    if hello_reply.kind == "error" {
        let code = serde_json::from_value::<BridgeErrorData>(hello_reply.data)
            .map(|data| data.code)
            .unwrap_or_else(|_| "unknown".to_string());
        return Err(ConnectError::Auth { code });
    }
    if hello_reply.id != HELLO_ID || hello_reply.kind != "hello_ok" {
        return Err(ConnectError::Transport);
    }
    let mod_version = hello_reply
        .data
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    send_envelope(
        &mut write,
        AUTH_ID,
        "auth",
        serde_json::json!({ "token": endpoint.token }),
    )
    .await
    .map_err(|_| ConnectError::Transport)?;
    let auth_reply = recv_envelope(&mut read, cancel)
        .await
        .map_err(recv_error_to_connect_error)?;

    if auth_reply.id == AUTH_ID && auth_reply.kind == "auth_ok" {
        Ok(Connected {
            write,
            read,
            mod_version,
        })
    } else if auth_reply.kind == "error" {
        let code = serde_json::from_value::<BridgeErrorData>(auth_reply.data)
            .map(|data| data.code)
            .unwrap_or_else(|_| "unknown".to_string());
        Err(ConnectError::Auth { code })
    } else {
        Err(ConnectError::Transport)
    }
}

pub async fn pump(
    mut write: WsWriteHalf,
    mut read: WsReadHalf,
    cancel: &CancellationToken,
    command_rx: &mut mpsc::Receiver<Command>,
) -> ConnectionEnd {
    let mut pending: HashMap<String, oneshot::Sender<Result<Value, BridgeError>>> = HashMap::new();

    let end = loop {
        tokio::select! {
            _ = cancel.cancelled() => break ConnectionEnd::Cancelled,
            command = command_rx.recv() => {
                match command {
                    None => break ConnectionEnd::Disconnected,
                    Some(Command::Request { id, kind, data, reply }) => {
                        let envelope = BridgeEnvelope { id: id.clone(), kind, data };
                        let text = match serde_json::to_string(&envelope) {
                            Ok(text) => text,
                            Err(_) => {
                                let _ = reply.send(Err(BridgeError::Transport));
                                continue;
                            }
                        };
                        pending.insert(id, reply);
                        if write.send(Message::Text(text.into())).await.is_err() {
                            break ConnectionEnd::Disconnected;
                        }
                    }
                    Some(Command::CancelRequest { id }) => {
                        pending.remove(&id);
                    }
                }
            }
            incoming = read.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        let Ok(envelope) = serde_json::from_str::<BridgeEnvelope>(&text) else {
                            tracing::debug!("bridge: malformed frame from mod, dropping");
                            continue;
                        };
                        match pending.remove(&envelope.id) {
                            None => {
                                tracing::debug!(id = %envelope.id, "bridge: reply for an unknown or no-longer-pending id, dropping");
                            }
                            Some(sender) => {
                                let result = if envelope.kind == "error" {
                                    match serde_json::from_value::<BridgeErrorData>(envelope.data) {
                                        Ok(error) => Err(BridgeError::Mod { code: error.code, message: error.message }),
                                        Err(_) => Err(BridgeError::Transport),
                                    }
                                } else {
                                    Ok(envelope.data)
                                };
                                let _ = sender.send(result);
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break ConnectionEnd::Disconnected,
                    Some(Err(_)) => break ConnectionEnd::Disconnected,
                    _ => {}
                }
            }
        }
    };

    for (_, sender) in pending.drain() {
        let _ = sender.send(Err(BridgeError::Offline));
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn hello_error_reply_preserves_the_mod_error_code() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            let _ = ws.next().await;
            let error = serde_json::json!({
                "id": HELLO_ID,
                "type": "error",
                "data": { "code": "protocol_version_unsupported", "message": "unsupported protocol version" },
            });
            let _ = ws.send(Message::Text(error.to_string().into())).await;
        });

        let endpoint = BridgeEndpointFile {
            protocol_version: BRIDGE_PROTOCOL_VERSION,
            port: addr.port(),
            token: "irrelevant".to_string(),
            pid: std::process::id(),
            started_at: "2026-09-03T00:00:00Z".to_string(),
        };
        let cancel = CancellationToken::new();
        let result = connect_and_handshake(&endpoint, &cancel).await;
        server.await.unwrap();

        match result {
            Err(ConnectError::Auth { code }) => assert_eq!(code, "protocol_version_unsupported"),
            Err(ConnectError::Transport) => panic!("hello-phase error lost its code, fell back to Transport"),
            Err(ConnectError::Cancelled) => panic!("expected Err(ConnectError::Auth), got Cancelled"),
            Ok(_) => panic!("expected Err(ConnectError::Auth), got Ok(Connected)"),
        }
    }
}
