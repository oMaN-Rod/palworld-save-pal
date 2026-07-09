use axum::extract::ws::Message;
use tokio::sync::mpsc::UnboundedSender;

use psp_core::progress::ProgressSink;

use crate::messages::MessageType;

/// Cheaply cloneable handle that queues outgoing frames onto the
/// per-connection writer task. Response construction mirrors
/// ws/utils.py:29 build_response: {"type": <wire>, "data": <payload>}.
#[derive(Clone)]
pub struct Emitter {
    sender: UnboundedSender<Message>,
}

impl Emitter {
    pub fn new(sender: UnboundedSender<Message>) -> Self {
        Self { sender }
    }

    pub fn emit<T: serde::Serialize>(&self, message_type: MessageType, data: &T) {
        let payload = serde_json::json!({ "type": message_type.as_wire(), "data": data });
        match serde_json::to_string(&payload) {
            Ok(text) => {
                // Send failure just means the client disconnected — drop silently.
                let _ = self.sender.send(Message::Text(text.into()));
            }
            Err(serialize_error) => {
                tracing::error!(%serialize_error, message_type = message_type.as_wire(),
                    "failed to serialize outgoing message");
            }
        }
    }

    /// {"type": "error", "data": {"message": ..., "trace": ...}} — ws/manager.py:46-49.
    pub fn emit_error(&self, message: &str, trace: &str) {
        self.emit(
            MessageType::Error,
            &serde_json::json!({ "message": message, "trace": trace }),
        );
    }

    /// Progress strings become progress_message frames (UI spinner text).
    pub fn progress_sink(&self) -> ProgressSink {
        let emitter = self.clone();
        std::sync::Arc::new(move |progress_text: &str| {
            emitter.emit(MessageType::ProgressMessage, &progress_text);
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn text_frame_as_json(frame: Message) -> Value {
        match frame {
            Message::Text(text) => serde_json::from_str(text.as_str()).unwrap(),
            other => panic!("expected text frame, got {other:?}"),
        }
    }

    #[test]
    fn emit_wraps_payload_in_envelope() {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let emitter = Emitter::new(sender);
        emitter.emit(MessageType::GetVersion, &"0.17.3");
        let value = text_frame_as_json(receiver.try_recv().unwrap());
        assert_eq!(
            value,
            serde_json::json!({"type": "get_version", "data": "0.17.3"})
        );
    }

    #[test]
    fn emit_error_matches_python_shape() {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let emitter = Emitter::new(sender);
        emitter.emit_error("boom", "trace-lines");
        let value = text_frame_as_json(receiver.try_recv().unwrap());
        assert_eq!(
            value,
            serde_json::json!({"type": "error", "data": {"message": "boom", "trace": "trace-lines"}})
        );
    }

    #[test]
    fn progress_sink_emits_progress_message() {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let emitter = Emitter::new(sender);
        let sink = emitter.progress_sink();
        sink("Loading Level.sav...");
        let value = text_frame_as_json(receiver.try_recv().unwrap());
        assert_eq!(
            value,
            serde_json::json!({"type": "progress_message", "data": "Loading Level.sav..."})
        );
    }
}
