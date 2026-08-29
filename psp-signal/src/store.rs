//! Persistence for Signal settings.
//!
//! The stored shape is deliberately narrower than runtime state: the
//! AdminPassword is NEVER persisted (it lives only in the poller's memory),
//! and the access token is stored so a restart keeps clients valid.

use async_trait::async_trait;

/// What survives a restart. Persisted by the host app; `psp-signal` itself
/// only defines the shape and an in-memory default.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SignalStored {
    /// Autostart the feed server when PalStudio starts.
    pub enabled: bool,
    /// Bind address for the Signal API ("127.0.0.1" restricts to this PC).
    pub bind: String,
    /// 0 means "pick a free port" (tests); UIs offer a concrete default.
    pub port: u16,
    /// Poll interval in milliseconds.
    pub interval_ms: u64,
    /// Extra browser origins allowed to read the feed.
    pub allowed_origins: Vec<String>,
    /// Source type: "rest", "gamedata", or "fake". None = idle.
    pub source_type: Option<String>,
    /// REST base URL (normalized). Never carries credentials.
    pub source_url: Option<String>,
    /// Explicit bridge-file path override (gamedata source).
    pub gamedata_path: Option<String>,
    /// The access token (regenerable).
    pub token: String,
}

impl SignalStored {
    pub fn defaults() -> Self {
        Self {
            enabled: false,
            bind: "127.0.0.1".into(),
            port: 8788,
            interval_ms: 1000,
            allowed_origins: Vec::new(),
            source_type: None,
            source_url: None,
            gamedata_path: None,
            token: String::new(),
        }
    }
}

/// Implemented by the host (psp-server persists through the SQLite driver).
#[async_trait]
pub trait SignalStore: Send + Sync {
    /// Returns the stored settings, or defaults when never saved.
    async fn load(&self) -> SignalStored;
    /// Upserts the stored settings.
    async fn save(&self, stored: &SignalStored);
}

/// In-memory store for tests and headless embedding.
#[derive(Default)]
pub struct MemorySignalStore {
    stored: std::sync::Mutex<SignalStored>,
}

impl MemorySignalStore {
    pub fn new(stored: SignalStored) -> Self {
        Self {
            stored: std::sync::Mutex::new(stored),
        }
    }
}

#[async_trait]
impl SignalStore for MemorySignalStore {
    async fn load(&self) -> SignalStored {
        self.stored.lock().expect("signal store lock").clone()
    }

    async fn save(&self, stored: &SignalStored) {
        *self.stored.lock().expect("signal store lock") = stored.clone();
    }
}
