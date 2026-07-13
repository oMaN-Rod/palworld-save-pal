pub mod api_convert;
pub mod desktop_dialogs;
pub mod dispatcher;
pub mod emitter;
pub mod envelope;
pub mod handler_error;
pub mod handlers;
pub mod messages;
pub mod router;
pub mod services;
pub mod static_files;
pub mod ws;

use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
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
pub struct ServerConfig {
    /// Web default 0.0.0.0; desktop 127.0.0.1.
    pub host: IpAddr,
    /// 0 picks a free port (tests).
    pub port: u16,
    pub ui_dir: PathBuf,
    /// Directory holding "json/" with the game data.
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    /// Enables native file dialogs and the local folder/browser handlers.
    pub desktop_mode: bool,
}

pub struct AppState {
    pub config: ServerConfig,
    pub game_data: Arc<GameData>,
    pub db: sqlx::SqlitePool,
    pub dialogs: Arc<dyn crate::desktop_dialogs::FileDialogProvider>,
    /// Count of currently-open `/ws/{client_id}` connections, maintained by a
    /// `Drop` guard in `ws::connection_loop` so it also decrements on panic or
    /// early return. Makes reader-loop/writer-task teardown observable in tests:
    /// axum runs the upgraded socket on its own spawned task, decoupled from the
    /// HTTP connection future that `ServerHandle::shutdown` waits on.
    pub live_connections: tokio::sync::watch::Sender<usize>,
    /// Docker + Palworld REST clients used by the server-management handlers.
    /// Real `BollardDocker` in production; `mock::MockDocker` in tests.
    pub server_services: Arc<crate::services::ServerServices>,
    /// Parsed sessions keyed by id, so a session survives a WS reconnect. A
    /// connection registers its session here on load; reattach/eject read it.
    pub sessions: std::sync::Mutex<SessionStore>,
}

pub struct ServerHandle {
    pub addr: SocketAddr,
    pub app: Arc<AppState>,
    /// Subscriber on `AppState::live_connections`, seeded at 0 before any
    /// connection is accepted, so tests can await connection teardown instead
    /// of sleeping.
    pub live_connections: tokio::sync::watch::Receiver<usize>,
    shutdown_sender: tokio::sync::oneshot::Sender<()>,
    serve_task: tokio::task::JoinHandle<std::io::Result<()>>,
}

impl ServerHandle {
    /// Signals the serve loop to stop and waits for it to exit.
    pub async fn shutdown(self) {
        let _ = self.shutdown_sender.send(());
        let _ = self.serve_task.await;
    }

    /// Blocks until the server exits on its own.
    pub async fn wait(self) {
        let _ = self.serve_task.await;
    }
}

/// Picks the real `RfdDialogProvider` in desktop mode and the inert
/// `NullDialogProvider` otherwise, then defers to `start_server_with`.
pub async fn start_server(config: ServerConfig) -> anyhow::Result<ServerHandle> {
    // rfd only exists under the `desktop` feature; the headless server/Docker
    // build always uses the inert NullDialogProvider.
    #[cfg(feature = "desktop")]
    let dialogs: Arc<dyn crate::desktop_dialogs::FileDialogProvider> = if config.desktop_mode {
        Arc::new(crate::desktop_dialogs::RfdDialogProvider)
    } else {
        Arc::new(crate::desktop_dialogs::NullDialogProvider)
    };
    #[cfg(not(feature = "desktop"))]
    let dialogs: Arc<dyn crate::desktop_dialogs::FileDialogProvider> =
        Arc::new(crate::desktop_dialogs::NullDialogProvider);
    start_server_with(config, dialogs).await
}

/// Binds the listener before returning, so the port is already accepting
/// connections by the time the caller sees a `ServerHandle`. `dialogs` lets
/// callers inject a `FileDialogProvider` of their own.
pub async fn start_server_with(
    config: ServerConfig,
    dialogs: Arc<dyn crate::desktop_dialogs::FileDialogProvider>,
) -> anyhow::Result<ServerHandle> {
    let game_data = Arc::new(GameData::load(&config.data_dir.join("json"))?);
    let db = psp_db::open(&config.db_path).await?;
    let legacy_db_path = config
        .db_path
        .parent()
        .map(|dir| dir.join("psp.db"))
        .unwrap_or_else(|| std::path::PathBuf::from("psp.db"));
    let pal_data_validator = |value: &serde_json::Value| -> Result<serde_json::Value, String> {
        let dto =
            psp_core::dto::pal::PalDto::from_json_lenient(value).map_err(|e| e.to_string())?;
        serde_json::to_value(&dto).map_err(|e| e.to_string())
    };
    match psp_db::import_legacy::import_legacy_if_needed(&db, &legacy_db_path, &pal_data_validator)
        .await
    {
        Ok(Some(report)) => tracing::info!(?report, "legacy psp.db imported"),
        Ok(None) => {}
        Err(error) => {
            tracing::error!(%error, "legacy psp.db import failed; continuing with new DB")
        }
    }
    let (live_connections, live_connections_rx) = tokio::sync::watch::channel(0usize);
    let state = Arc::new(AppState {
        config: config.clone(),
        game_data,
        db,
        dialogs,
        live_connections,
        server_services: Arc::new(crate::services::ServerServices::real()),
        sessions: std::sync::Mutex::new(SessionStore::default()),
    });

    let listener = tokio::net::TcpListener::bind((config.host, config.port)).await?;
    let addr = listener.local_addr()?;
    tracing::info!(%addr, desktop_mode = config.desktop_mode, "psp-server listening");

    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel::<()>();
    let application = router::build_router(Arc::clone(&state));
    let serve_task = tokio::spawn(async move {
        axum::serve(listener, application)
            .with_graceful_shutdown(async {
                let _ = shutdown_receiver.await;
            })
            .await
    });

    Ok(ServerHandle {
        addr,
        app: state,
        live_connections: live_connections_rx,
        shutdown_sender,
        serve_task,
    })
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

#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::Arc;

    use axum::extract::ws::Message;
    use tokio::sync::mpsc::UnboundedReceiver;

    use psp_core::gamedata::GameData;
    use psp_core::session::Session;

    use crate::emitter::Emitter;
    use crate::{AppState, ServerConfig};

    /// Everything a handler unit test needs: an AppState over a temp DB and a
    /// synthetic game-data dir, plus an Emitter whose frames land in `frames`.
    pub struct TestContext {
        pub app: Arc<AppState>,
        pub session: Session,
        pub emitter: Emitter,
        pub frames: UnboundedReceiver<Message>,
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

            let config = ServerConfig {
                host: "127.0.0.1".parse().unwrap(),
                port: 0,
                ui_dir: temp_dir.path().join("ui"),
                data_dir: temp_dir.path().join("data"),
                db_path: temp_dir.path().join("test.db"),
                desktop_mode: false,
            };
            let db = psp_db::open(&config.db_path).await.unwrap();
            let game_data = Arc::new(GameData::load(&json_dir).unwrap());
            let (live_connections, _live_connections_rx) = tokio::sync::watch::channel(0usize);
            let server_services = Arc::new(crate::services::ServerServices::with_docker(Arc::new(
                crate::services::docker::mock::MockDocker::default(),
            )));
            let app = Arc::new(AppState {
                config,
                game_data,
                db,
                dialogs: Arc::new(crate::desktop_dialogs::NullDialogProvider),
                live_connections,
                server_services,
                sessions: std::sync::Mutex::new(crate::SessionStore::default()),
            });
            let (sender, frames) = tokio::sync::mpsc::unbounded_channel();
            Self {
                app,
                session: Session::new(),
                emitter: Emitter::new(sender),
                frames,
                _temp_dir: temp_dir,
            }
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
    pub fn next_frame_json_from(receiver: &mut UnboundedReceiver<Message>) -> serde_json::Value {
        match receiver.try_recv().expect("expected an emitted frame") {
            Message::Text(text) => serde_json::from_str(text.as_str()).unwrap(),
            other => panic!("expected text frame, got {other:?}"),
        }
    }
}
