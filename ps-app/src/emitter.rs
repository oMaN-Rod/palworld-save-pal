use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, WeakUnboundedSender};

use ps_core::progress::ProgressSink;

use crate::messages::MessageType;

/// Cheaply cloneable handle that queues outgoing frames onto the
/// per-connection writer task.
#[derive(Clone)]
pub struct Emitter {
    sender: UnboundedSender<String>,
}

impl Emitter {
    pub fn new(sender: UnboundedSender<String>) -> Self {
        Self { sender }
    }

    pub fn emit<T: serde::Serialize>(&self, message_type: MessageType, data: &T) {
        // A failing `Serialize` impl must not panic the connection, so log and
        // drop the frame instead.
        let payload = match serde_json::to_string(data) {
            Ok(payload) => payload,
            Err(serialize_error) => {
                tracing::error!(%serialize_error, message_type = message_type.as_wire(),
                    "failed to serialize outgoing message");
                return;
            }
        };
        // Assemble the envelope around the pre-serialized payload instead of
        // round-tripping it through a `Value` tree, which deep-copies large
        // frames (pal lists, download blobs) twice.
        let mut text = String::with_capacity(payload.len() + message_type.as_wire().len() + 24);
        text.push_str("{\"type\":\"");
        // Wire strings are snake_case identifiers — no JSON escaping needed.
        text.push_str(message_type.as_wire());
        text.push_str("\",\"data\":");
        text.push_str(&payload);
        text.push('}');
        // Send failure just means the client disconnected — drop silently.
        let _ = self.sender.send(text);
    }

    pub fn emit_error(&self, message: &str, trace: &str) {
        self.emit(
            MessageType::Error,
            &serde_json::json!({ "message": message, "trace": trace }),
        );
    }

    pub async fn closed(&self) {
        self.sender.closed().await;
    }

    pub fn progress_sink(&self) -> ProgressSink {
        let emitter = self.clone();
        std::sync::Arc::new(move |progress_text: &str| {
            emitter.emit(MessageType::ProgressMessage, &progress_text);
        })
    }

    pub fn test_channel() -> (Self, UnboundedReceiver<String>) {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        (Self::new(sender), receiver)
    }

    pub fn downgrade(&self) -> WeakEmitter {
        WeakEmitter {
            sender: self.sender.downgrade(),
        }
    }
}

/// Holds no sender alive, so a registry that outlives a handler cannot keep a
/// closed connection's writer task running.
#[derive(Clone)]
pub struct WeakEmitter {
    sender: WeakUnboundedSender<String>,
}

impl WeakEmitter {
    pub fn is_connected(&self) -> bool {
        self.sender
            .upgrade()
            .is_some_and(|sender| !sender.is_closed())
    }

    pub fn upgrade(&self) -> Option<Emitter> {
        self.sender
            .upgrade()
            .filter(|sender| !sender.is_closed())
            .map(|sender| Emitter { sender })
    }

    pub fn is_same_connection(&self, emitter: &Emitter) -> bool {
        self.sender
            .upgrade()
            .is_some_and(|sender| sender.same_channel(&emitter.sender))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn text_frame_as_json(frame: String) -> Value {
        serde_json::from_str(&frame).unwrap()
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

    #[test]
    fn a_weak_emitter_knows_its_connection_and_upgrades_only_while_open() {
        let (first, first_rx) = Emitter::test_channel();
        let (second, _second_rx) = Emitter::test_channel();
        let weak = first.downgrade();
        assert!(weak.is_same_connection(&first));
        assert!(weak.is_same_connection(&first.clone()));
        assert!(!weak.is_same_connection(&second));
        assert!(weak.upgrade().is_some());

        drop(first_rx);
        assert!(
            weak.upgrade().is_none(),
            "a closed connection does not upgrade"
        );

        let (third, _third_rx) = Emitter::test_channel();
        let weak_third = third.downgrade();
        drop(third);
        assert!(weak_third.upgrade().is_none(), "no strong sender is left");
        assert!(!weak_third.is_same_connection(&second));
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
        let envelope: serde_json::Value = serde_json::from_str(&frame).unwrap();
        assert_eq!(envelope["type"], "detect_workshop_dir");
        assert_eq!(envelope["data"]["workshop_dir"], "");
    }

    #[test]
    fn a_weak_emitter_reports_the_connection_gone_once_every_strong_handle_drops() {
        let (emitter, receiver) = Emitter::test_channel();
        let weak = emitter.downgrade();
        assert!(weak.is_connected());
        let clone = emitter.clone();
        drop(emitter);
        assert!(weak.is_connected());
        drop(clone);
        assert!(!weak.is_connected());
        drop(receiver);
    }

    #[test]
    fn a_weak_emitter_reports_a_dropped_receiver_as_gone() {
        let (emitter, receiver) = Emitter::test_channel();
        let weak = emitter.downgrade();
        drop(receiver);
        assert!(!weak.is_connected());
    }
}
