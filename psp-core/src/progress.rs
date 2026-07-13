use std::sync::Arc;

/// Emits human-readable progress strings; the server layer wires this to
/// the `progress_message` WS type.
pub type ProgressSink = Arc<dyn Fn(&str) + Send + Sync>;

pub fn null_progress() -> ProgressSink {
    Arc::new(|_| {})
}

#[cfg(test)]
mod tests {
    use super::{null_progress, ProgressSink};
    use std::sync::Arc;

    #[test]
    fn null_progress_discards_messages_and_is_shareable() {
        let sink: ProgressSink = null_progress();
        let clone = Arc::clone(&sink);
        std::thread::spawn(move || clone("from another thread"))
            .join()
            .expect("ProgressSink must be Send + Sync");
        assert_eq!(Arc::strong_count(&sink), 1);
    }
}
