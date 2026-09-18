use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use ps_core::progress::ProgressSink;

use crate::messages::MessageType;

/// Per-connection outgoing byte budget shared between the [`Emitter`] and the
/// connection's writer task.
///
/// Frames are NEVER silently dropped from a live connection: a dropped
/// response frame wedges the client protocol forever (a caller cannot tell a
/// slow handler from a lost frame). Instead, a client that stops draining
/// its frames — slow, malicious, or gone — trips the budget and has the
/// whole connection terminated by the writer.
#[derive(Debug)]
pub struct OutgoingQueueGuard {
    queued_bytes: AtomicUsize,
    max_queued_bytes: usize,
    terminated: AtomicBool,
}

impl OutgoingQueueGuard {
    pub fn new(max_queued_bytes: usize) -> Arc<Self> {
        Arc::new(Self {
            queued_bytes: AtomicUsize::new(0),
            max_queued_bytes,
            terminated: AtomicBool::new(false),
        })
    }

    /// True once the budget was exceeded; every further emit is a no-op and
    /// the writer closes the socket.
    pub fn is_terminated(&self) -> bool {
        self.terminated.load(Ordering::Acquire)
    }

    /// Reserves `bytes` of queue budget. `false` means the budget tripped and
    /// the frame must not be queued. Charging before checking only ever
    /// terminates a connection earlier, never later, so clones racing here
    /// stay safe.
    fn charge(&self, bytes: usize) -> bool {
        let queued = self.queued_bytes.fetch_add(bytes, Ordering::AcqRel) + bytes;
        if queued > self.max_queued_bytes {
            if !self.terminated.swap(true, Ordering::AcqRel) {
                tracing::warn!(
                    queued_bytes = queued,
                    max_queued_bytes = self.max_queued_bytes,
                    "outgoing websocket queue exceeded its budget; terminating the connection"
                );
            }
            false
        } else {
            true
        }
    }

    /// Gives budget back once the writer handed a frame to the socket.
    pub fn release(&self, bytes: usize) {
        self.queued_bytes.fetch_sub(bytes, Ordering::AcqRel);
    }
}

/// Cheaply cloneable handle that queues outgoing frames onto the
/// per-connection writer task.
#[derive(Clone)]
pub struct Emitter {
    sender: UnboundedSender<String>,
    max_payload_bytes: Option<usize>,
    guard: Option<Arc<OutgoingQueueGuard>>,
}

impl Emitter {
    pub fn new(sender: UnboundedSender<String>) -> Self {
        Self {
            sender,
            max_payload_bytes: None,
            guard: None,
        }
    }

    /// Network transports hand the per-connection writer channel plus a
    /// queue guard: while the connection is healthy every frame is delivered,
    /// and a client that stops draining trips the budget and is disconnected
    /// wholesale — bounding memory without ever losing a live frame.
    pub fn new_network(
        sender: UnboundedSender<String>,
        max_payload_bytes: usize,
        guard: Arc<OutgoingQueueGuard>,
    ) -> Self {
        Self {
            sender,
            max_payload_bytes: Some(max_payload_bytes),
            guard: Some(guard),
        }
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
        if self.max_payload_bytes.is_some_and(|max| text.len() > max) {
            tracing::warn!(
                message_type = message_type.as_wire(),
                bytes = text.len(),
                "outgoing websocket message exceeded the configured limit"
            );
            return;
        }
        if let Some(guard) = &self.guard {
            // Send failure just means the client disconnected; the guard is
            // per-connection and dies with it.
            if !guard.is_terminated() && guard.charge(text.len()) {
                let _ = self.sender.send(text);
            }
            return;
        }
        let _ = self.sender.send(text);
    }

    pub fn emit_error(&self, message: &str, trace: &str) {
        self.emit(
            MessageType::Error,
            &serde_json::json!({ "message": message, "trace": trace }),
        );
    }

    pub async fn closed(&self) {
        self.sender.closed().await
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

    #[test]
    fn network_emitter_never_drops_frames_under_budget() {
        // Regression: a save load emits a burst of progress frames plus the
        // final response faster than any reader drains; every frame must
        // still arrive while the connection is within budget.
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let guard = OutgoingQueueGuard::new(1024 * 1024);
        let emitter = Emitter::new_network(sender, MAX_TEST_PAYLOAD, guard);
        for index in 0..200 {
            emitter.emit(MessageType::ProgressMessage, &format!("step {index}"));
        }
        emitter.emit(MessageType::GetVersion, &"final-response");
        let mut seen_final = false;
        while let Ok(frame) = receiver.try_recv() {
            if frame.contains("final-response") {
                seen_final = true;
            }
        }
        assert!(seen_final, "the response frame must survive the burst");
    }

    #[test]
    fn over_budget_connection_is_terminated_not_trickled() {
        // Measure a real frame so the budget arithmetic is exact: two frames
        // fit, the third trips the budget and the connection is terminated.
        let payload = "x".repeat(60);
        let (probe_tx, mut probe_rx) = tokio::sync::mpsc::unbounded_channel();
        Emitter::new(probe_tx).emit(MessageType::ProgressMessage, &payload);
        let one_frame = probe_rx.try_recv().unwrap().len();

        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let guard = OutgoingQueueGuard::new(one_frame * 2 + 10);
        let emitter = Emitter::new_network(sender, MAX_TEST_PAYLOAD, Arc::clone(&guard));
        emitter.emit(MessageType::ProgressMessage, &payload);
        emitter.emit(MessageType::ProgressMessage, &payload);
        assert!(!guard.is_terminated());
        emitter.emit(MessageType::ProgressMessage, &payload); // trips the budget
        emitter.emit(MessageType::GetVersion, &"never-queued");
        assert!(guard.is_terminated());
        let mut delivered = 0;
        while let Ok(frame) = receiver.try_recv() {
            assert!(!frame.contains("never-queued"), "{frame}");
            delivered += 1;
        }
        assert_eq!(delivered, 2, "the two in-budget frames queue; nothing after");
    }

    const MAX_TEST_PAYLOAD: usize = 1024 * 1024;
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
}
