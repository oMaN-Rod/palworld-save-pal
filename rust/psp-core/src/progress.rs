use std::sync::Arc;

/// Emits human-readable progress strings; the server layer wires this to
/// the `progress_message` WS type.
pub type ProgressSink = Arc<dyn Fn(&str) + Send + Sync>;

/// A sink that discards all progress messages.
pub fn null_progress() -> ProgressSink {
    Arc::new(|_| {})
}

#[cfg(test)]
mod tests {
    use super::null_progress;

    #[test]
    fn null_progress_accepts_messages() {
        let sink = null_progress();
        sink("Loading Level.sav...");
    }
}
