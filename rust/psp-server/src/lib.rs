pub mod dispatcher;
pub mod emitter;
pub mod envelope;
pub mod handler_error;
pub mod handlers;
pub mod messages;

use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;

use psp_core::gamedata::GameData;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Web default 0.0.0.0; desktop 127.0.0.1.
    pub host: IpAddr,
    /// Default 5174. 0 = pick a free port (tests).
    pub port: u16,
    /// Directory holding the built SvelteKit UI ("./ui").
    pub ui_dir: PathBuf,
    /// Directory holding "json/" with the game data ("./data").
    pub data_dir: PathBuf,
    /// The NEW database file ("./psp-rs.db"); the legacy ./psp.db is imported in Phase 3.
    pub db_path: PathBuf,
    /// Swaps select_save/open_folder behavior in Phase 5.
    pub desktop_mode: bool,
}

pub struct AppState {
    pub config: ServerConfig,
    pub game_data: Arc<GameData>,
    pub db: sqlx::SqlitePool,
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
    /// synthetic (initially empty) game-data dir, plus an Emitter whose frames
    /// land in `frames`.
    pub struct TestContext {
        pub app: Arc<AppState>,
        pub session: Session,
        pub emitter: Emitter,
        pub frames: UnboundedReceiver<Message>,
        /// Held for RAII only (deletes the temp tree on drop) — underscore
        /// prefix keeps clippy's dead_code lint quiet.
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
            let app = Arc::new(AppState {
                config,
                game_data,
                db,
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
            match self.frames.try_recv().expect("expected an emitted frame") {
                Message::Text(text) => serde_json::from_str(text.as_str()).unwrap(),
                other => panic!("expected text frame, got {other:?}"),
            }
        }

        pub fn assert_no_more_frames(&mut self) {
            assert!(self.frames.try_recv().is_err(), "unexpected extra frame");
        }
    }
}
