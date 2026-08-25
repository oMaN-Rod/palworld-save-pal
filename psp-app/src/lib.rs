pub mod blueprint_registry;
pub mod desktop_dialogs;
pub mod dispatcher;
pub mod emitter;
pub mod envelope;
pub mod handler_error;
pub mod handlers;
pub mod lsp;
pub mod messages;
pub mod plugin_registry;

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use psp_core::gamedata::GameData;
use psp_core::session::Session;
use uuid::Uuid;

/// The per-session `tokio::Mutex` may be held across a handler's `.await`s;
/// the store's outer `std::Mutex` is only ever held briefly.
pub type SharedSession = Arc<tokio::sync::Mutex<Session>>;

/// Id-keyed store of parsed sessions, so a session survives a WS reconnect.
#[derive(Default)]
pub struct SessionStore {
    by_id: HashMap<Uuid, SharedSession>,
    order: VecDeque<Uuid>,
}

const MAX_STORED_SESSIONS: usize = 8;

impl SessionStore {
    pub fn register(&mut self, session: SharedSession) -> Uuid {
        let id = Uuid::new_v4();
        self.by_id.insert(id, session);
        self.order.push_back(id);
        while self.order.len() > MAX_STORED_SESSIONS {
            if let Some(evicted) = self.order.pop_front() {
                self.by_id.remove(&evicted);
            }
        }
        id
    }

    pub fn get(&self, id: &Uuid) -> Option<SharedSession> {
        self.by_id.get(id).cloned()
    }

    pub fn remove(&mut self, id: &Uuid) {
        self.by_id.remove(id);
        self.order.retain(|existing| existing != id);
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub desktop_mode: bool,
}

pub struct AppState {
    pub config: AppConfig,
    pub game_data: Arc<GameData>,
    pub driver: Arc<dyn psp_db::DbDriver>,
    pub dialogs: Arc<dyn crate::desktop_dialogs::FileDialogProvider>,
    /// The transport maintains this with a `Drop` guard around each connection
    /// so it also decrements on panic or early return.
    pub live_connections: tokio::sync::watch::Sender<usize>,
    /// Transport-owned router for native-only message types (server
    /// management, shell-open). `NullExtRouter` on targets without them.
    pub ext: Arc<dyn crate::dispatcher::ExtRouter>,
    pub lsp: Arc<dyn crate::lsp::LspService>,
    /// A connection registers its session here on load; reattach/eject read it.
    pub sessions: std::sync::Mutex<SessionStore>,
    /// Built on first handler call so wasm — where `game_data` is populated
    /// post-init — doesn't race construction.
    pub breeding_db: std::sync::OnceLock<Arc<psp_core::breeding::BreedingDB>>,
    /// Bundled plugin sources and in-flight run cancellation handles.
    pub plugins: plugin_registry::PluginRegistry,
}

impl AppState {
    pub fn breeding_db(
        &self,
    ) -> Result<&Arc<psp_core::breeding::BreedingDB>, psp_core::breeding::BreedingError> {
        if let Some(cached) = self.breeding_db.get() {
            return Ok(cached);
        }
        let db = Arc::new(psp_core::breeding::BreedingDB::from_game_data(&self.game_data)?);
        // `set` succeeds on the first writer; on a race the cell already holds a
        // valid Arc either way, so re-fetch rather than trust this call's result.
        let _ = self.breeding_db.set(db);
        Ok(self
            .breeding_db
            .get()
            .expect("breeding_db was just initialized"))
    }
}

#[cfg(test)]
mod session_store_tests {
    use super::{SessionStore, SharedSession, MAX_STORED_SESSIONS};
    use psp_core::session::Session;
    use std::sync::Arc;

    fn empty_session() -> SharedSession {
        Arc::new(tokio::sync::Mutex::new(Session::new()))
    }

    #[test]
    fn register_get_remove_round_trips() {
        let mut store = SessionStore::default();
        let session = empty_session();
        let id = store.register(Arc::clone(&session));

        let found = store.get(&id).expect("registered session is findable");
        assert!(Arc::ptr_eq(&found, &session));
        assert_eq!(store.len(), 1);

        store.remove(&id);
        assert!(store.get(&id).is_none());
        assert!(store.is_empty());
    }

    #[test]
    fn evicts_oldest_past_the_cap() {
        let mut store = SessionStore::default();
        let first_id = store.register(empty_session());
        for _ in 0..MAX_STORED_SESSIONS {
            store.register(empty_session());
        }
        assert_eq!(store.len(), MAX_STORED_SESSIONS);
        assert!(store.get(&first_id).is_none());
    }
}

#[cfg(any(test, feature = "test-support"))]
pub mod test_support {
    use std::sync::Arc;

    use tokio::sync::mpsc::UnboundedReceiver;

    use psp_core::gamedata::GameData;
    use psp_core::session::Session;

    use crate::emitter::Emitter;
    use crate::{AppConfig, AppState};

    pub struct TestContext {
        pub app: Arc<AppState>,
        pub session: Session,
        pub emitter: Emitter,
        pub blueprints: crate::blueprint_registry::BlueprintRegistry,
        pub frames: UnboundedReceiver<String>,
        /// Held for RAII only: deletes the temp tree on drop.
        pub _temp_dir: tempfile::TempDir,
    }

    impl TestContext {
        pub async fn new(populate_data_dir: impl FnOnce(&std::path::Path)) -> Self {
            let temp_dir = tempfile::tempdir().unwrap();
            let json_dir = temp_dir.path().join("data/json");
            std::fs::create_dir_all(&json_dir).unwrap();
            populate_data_dir(&json_dir);

            let db_path = temp_dir.path().join("test.db");
            let pool = psp_db::open(&db_path).await.unwrap();
            let game_data = Arc::new(GameData::load(&json_dir).unwrap());
            let (live_connections, _live_connections_rx) = tokio::sync::watch::channel(0usize);
            let app = Arc::new(AppState {
                config: AppConfig {
                    desktop_mode: false,
                },
                game_data,
                driver: Arc::new(psp_db::SqlxSqliteDriver::new(pool)),
                dialogs: Arc::new(crate::desktop_dialogs::NullDialogProvider),
                live_connections,
                ext: Arc::new(crate::dispatcher::NullExtRouter),
                lsp: Arc::new(crate::lsp::NullLspService),
                sessions: std::sync::Mutex::new(crate::SessionStore::default()),
                breeding_db: Default::default(),
                plugins: Default::default(),
            });
            let (sender, frames) = tokio::sync::mpsc::unbounded_channel();
            Self {
                app,
                session: Session::new(),
                emitter: Emitter::new(sender),
                blueprints: Default::default(),
                frames,
                _temp_dir: temp_dir,
            }
        }

        pub async fn with_ext(
            populate_data_dir: impl FnOnce(&std::path::Path),
            ext: Arc<dyn crate::dispatcher::ExtRouter>,
        ) -> Self {
            let mut test = Self::new(populate_data_dir).await;
            let app = Arc::get_mut(&mut test.app).expect("fresh TestContext app is unshared");
            app.ext = ext;
            test
        }

        pub fn next_frame_json(&mut self) -> serde_json::Value {
            next_frame_json_from(&mut self.frames)
        }

        pub fn assert_no_more_frames(&mut self) {
            assert!(self.frames.try_recv().is_err(), "unexpected extra frame");
        }
    }

    pub fn next_frame_json_from(receiver: &mut UnboundedReceiver<String>) -> serde_json::Value {
        let text = receiver.try_recv().expect("expected an emitted frame");
        serde_json::from_str(&text).unwrap()
    }
}
