pub mod blueprint_registry;
pub mod desktop_dialogs;
pub mod dispatcher;
pub mod emitter;
pub mod envelope;
pub mod handler_error;
pub mod handlers;
pub mod messages;

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use psp_core::gamedata::GameData;
use psp_core::session::Session;
use uuid::Uuid;

/// A parsed session shared between a connection and the store. The per-session
/// `tokio::Mutex` may be held across a handler's `.await`s; the store's outer
/// `std::Mutex` is only ever held briefly.
pub type SharedSession = Arc<tokio::sync::Mutex<Session>>;

/// Id-keyed store of parsed sessions, so a session survives a WS reconnect.
/// `order` bounds growth: the oldest entry is evicted past `MAX_STORED_SESSIONS`.
#[derive(Default)]
pub struct SessionStore {
    by_id: HashMap<Uuid, SharedSession>,
    order: VecDeque<Uuid>,
}

const MAX_STORED_SESSIONS: usize = 8;

impl SessionStore {
    /// Inserts `session` under a fresh id, evicting the oldest past the cap.
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
    /// Enables native file dialogs and the local folder/browser handlers.
    pub desktop_mode: bool,
}

pub struct AppState {
    pub config: AppConfig,
    pub game_data: Arc<GameData>,
    /// The DbDriver seam: every domain call runs its SQL through this handle.
    pub driver: std::sync::Arc<dyn psp_db::DbDriver>,
    pub dialogs: Arc<dyn crate::desktop_dialogs::FileDialogProvider>,
    /// Count of currently-open `/ws/{client_id}` connections. The transport
    /// (psp-server's `ws` module) maintains it with a `Drop` guard around each
    /// connection so it also decrements on panic or early return, making
    /// reader-loop/writer-task teardown observable in tests.
    pub live_connections: tokio::sync::watch::Sender<usize>,
    /// Transport-owned router for native-only message types (server
    /// management, shell-open). NullExtRouter on targets without them.
    pub ext: Arc<dyn crate::dispatcher::ExtRouter>,
    /// Parsed sessions keyed by id, so a session survives a WS reconnect. A
    /// connection registers its session here on load; reattach/eject read it.
    pub sessions: std::sync::Mutex<SessionStore>,
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

    /// Everything a handler unit test needs: an AppState over a temp DB and a
    /// synthetic game-data dir, plus an Emitter whose frames land in `frames`.
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
        /// `populate_data_dir` writes JSON files into the future data/json dir
        /// before GameData loads it.
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
                sessions: std::sync::Mutex::new(crate::SessionStore::default()),
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
            ext: std::sync::Arc<dyn crate::dispatcher::ExtRouter>,
        ) -> Self {
            let mut test = Self::new(populate_data_dir).await;
            // AppState is behind an Arc with no other clones yet, so rebuild it.
            let app =
                std::sync::Arc::get_mut(&mut test.app).expect("fresh TestContext app is unshared");
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

    /// Also usable by tests that drive a raw `UnboundedReceiver` without a full
    /// `TestContext`.
    pub fn next_frame_json_from(receiver: &mut UnboundedReceiver<String>) -> serde_json::Value {
        let text = receiver.try_recv().expect("expected an emitted frame");
        serde_json::from_str(&text).unwrap()
    }
}
