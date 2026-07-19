use axum::extract::ws::Message;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use psp_core::progress::ProgressSink;

use crate::messages::MessageType;

/// Cheaply cloneable handle that queues outgoing frames onto the per-connection
/// writer task, each wrapped in the wire envelope {"type": ..., "data": ...}.
#[derive(Clone)]
pub struct Emitter {
    sender: UnboundedSender<Message>,
}

impl Emitter {
    pub fn new(sender: UnboundedSender<Message>) -> Self {
        Self { sender }
    }

    pub fn emit<T: serde::Serialize>(&self, message_type: MessageType, data: &T) {
        // Serialize the payload on its own first: a failing `Serialize` impl must
        // not panic the connection, so log and drop the frame instead.
        let payload = match serde_json::to_value(data) {
            Ok(value) => value,
            Err(serialize_error) => {
                tracing::error!(%serialize_error, message_type = message_type.as_wire(),
                    "failed to serialize outgoing message");
                return;
            }
        };
        let frame = serde_json::json!({ "type": message_type.as_wire(), "data": payload });
        let text =
            serde_json::to_string(&frame).expect("envelope of a Value cannot fail to serialize");
        // Send failure just means the client disconnected — drop silently.
        let _ = self.sender.send(Message::Text(text.into()));
    }

    /// Emits {"type": "error", "data": {"message": ..., "trace": ...}}.
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

    /// Test-only: an Emitter whose frames land in a receiver instead of a socket.
    pub fn test_channel() -> (Self, UnboundedReceiver<Message>) {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        (Self::new(sender), receiver)
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
    fn emit_error_has_expected_envelope_shape() {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let emitter = Emitter::new(sender);
        emitter.emit_error("boom", "trace-lines");
        let value = text_frame_as_json(receiver.try_recv().unwrap());
        assert_eq!(
            value,
            serde_json::json!({"type": "error", "data": {"message": "boom", "trace": "trace-lines"}})
        );
    }

    /// A payload whose `Serialize` impl always fails. A bare `f64::NAN` does
    /// *not* exercise this path: serde_json encodes non-finite floats as JSON
    /// `null` rather than erroring.
    struct AlwaysFailsToSerialize;

    impl serde::Serialize for AlwaysFailsToSerialize {
        fn serialize<S: serde::Serializer>(&self, _serializer: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("simulated serialization failure"))
        }
    }

    #[test]
    fn emit_drops_unserializable_payload_without_panicking_and_survives() {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let emitter = Emitter::new(sender);

        emitter.emit(MessageType::GetVersion, &AlwaysFailsToSerialize);
        assert!(matches!(
            receiver.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        ));

        // The emitter must still be usable afterwards — the connection survives.
        emitter.emit(MessageType::GetVersion, &"0.17.3");
        let value = text_frame_as_json(receiver.try_recv().unwrap());
        assert_eq!(
            value,
            serde_json::json!({"type": "get_version", "data": "0.17.3"})
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

#[cfg(test)]
mod phase6_tests {
    use super::*;

    #[tokio::test]
    async fn test_channel_captures_emitted_envelopes() {
        let (emitter, mut receiver) = Emitter::test_channel();
        emitter.emit(
            crate::messages::MessageType::DetectWorkshopDir,
            &serde_json::json!({"workshop_dir": ""}),
        );
        let frame = receiver.recv().await.unwrap();
        let axum::extract::ws::Message::Text(text) = frame else {
            panic!("expected text frame");
        };
        let envelope: serde_json::Value = serde_json::from_str(text.as_str()).unwrap();
        assert_eq!(envelope["type"], "detect_workshop_dir");
        assert_eq!(envelope["data"]["workshop_dir"], "");
    }
}
