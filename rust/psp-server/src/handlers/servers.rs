//! Server-management handlers. Mirrors ws/handlers/server_handler.py.
//!
//! Error convention (Python parity): business failures emit message type
//! `error` with data {"message": "<text>"} — no trace key — and the handler
//! returns Ok(()). Only payload-parse failures use the HandlerError path.
use serde_json::Value;

use psp_db::servers::ServerRecord;

use crate::dispatcher::HandlerCtx;
use crate::emitter::Emitter;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;
use crate::services::{docker, native_mods, native_process, ServerProcessStatus};
use crate::AppState;

#[derive(Debug, serde::Deserialize)]
pub struct ServerIdData {
    pub server_id: i64,
}

pub(crate) fn emit_business_error(emitter: &Emitter, message: String) {
    emitter.emit(
        MessageType::Error,
        &serde_json::json!({ "message": message }),
    );
}

/// _server_to_dict parity — key set, order, and value shapes.
pub fn server_to_wire_json(record: &ServerRecord) -> Value {
    serde_json::json!({
        "id": record.id,
        "name": record.name,
        "container_name": record.container_name,
        "image_name": record.image_name,
        "server_type": record.server_type,
        "game_port": record.game_port,
        "query_port": record.query_port,
        "rest_api_port": record.rest_api_port,
        "data_volume_name": record.data_volume_name,
        "saves_path": record.saves_path,
        "mods_path": record.mods_path,
        "logicmods_path": record.logicmods_path,
        "nativemods_path": record.nativemods_path,
        "install_path": record.install_path,
        "steamcmd_path": record.steamcmd_path,
        "pid": record.pid,
        "launch_args": record.launch_args,
        "server_name": record.server_name,
        "server_description": record.server_description,
        "server_password": record.server_password,
        "admin_password": record.admin_password,
        "max_players": record.max_players,
        "workshop_dir": record.workshop_dir,
        "env_vars": Value::Object(record.env_vars.0.clone()),
        "created_at": record.created_at,
        "updated_at": record.updated_at,
    })
}

/// _count_total_players: first world dir under saves/SaveGames/0 that has a
/// Players dir wins (Python returns inside the loop — bug-compatible).
pub fn count_total_players(saves_path: &str) -> u64 {
    let save_games = std::path::Path::new(saves_path).join("SaveGames").join("0");
    let Ok(world_dirs) = std::fs::read_dir(&save_games) else {
        return 0;
    };
    for world_dir in world_dirs.flatten() {
        let players_dir = world_dir.path().join("Players");
        if players_dir.is_dir() {
            let Ok(player_files) = std::fs::read_dir(&players_dir) else {
                return 0;
            };
            return player_files
                .flatten()
                .filter(|file| {
                    let name = file.file_name().to_string_lossy().to_string();
                    name.ends_with(".sav") && !name.contains("_dps")
                })
                .count() as u64;
        }
    }
    0
}

/// _get_server_status: native → process status by pid, docker → container status.
pub(crate) async fn server_status(
    app: &AppState,
    record: &ServerRecord,
) -> Option<ServerProcessStatus> {
    if record.server_type == "native" {
        Some(native_process::process_status(record.pid))
    } else {
        docker::container_status(app.server_services.docker.as_ref(), &record.container_name).await
    }
}

/// Online player count via the REST API when the status says running, else 0.
pub(crate) async fn online_player_count(
    app: &AppState,
    record: &ServerRecord,
    status: &Option<ServerProcessStatus>,
) -> u64 {
    if status
        .as_ref()
        .map(|current| current.running)
        .unwrap_or(false)
    {
        app.server_services
            .palworld_api
            .get_player_count(
                "127.0.0.1",
                record.rest_api_port as u16,
                &record.admin_password,
            )
            .await
    } else {
        0
    }
}

async fn server_entry_with_runtime_fields(app: &AppState, record: &ServerRecord) -> Value {
    let status = server_status(app, record).await;
    let mut entry = server_to_wire_json(record);
    entry["status"] = serde_json::to_value(&status).expect("status serializes");
    entry["total_players"] = Value::from(count_total_players(&record.saves_path));
    entry["player_count"] = Value::from(online_player_count(app, record, &status).await);
    entry
}

pub async fn handle_list_servers(
    _data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    match psp_db::servers::list_servers(&ctx.app.db).await {
        Ok(records) => {
            let mut server_list = Vec::with_capacity(records.len());
            for record in &records {
                server_list.push(server_entry_with_runtime_fields(ctx.app, record).await);
            }
            ctx.emitter.emit(
                MessageType::ListServers,
                &serde_json::json!({ "servers": server_list }),
            );
        }
        Err(error) => {
            emit_business_error(ctx.emitter, format!("Failed to list servers: {error}"));
        }
    }
    Ok(())
}

pub async fn handle_get_server(
    data: ServerIdData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    match psp_db::servers::get_server(&ctx.app.db, data.server_id).await {
        Ok(Some(record)) => {
            let entry = server_entry_with_runtime_fields(ctx.app, &record).await;
            ctx.emitter.emit(MessageType::GetServer, &entry);
        }
        Ok(None) => emit_business_error(ctx.emitter, "Server not found".to_string()),
        Err(error) => {
            emit_business_error(ctx.emitter, format!("Failed to get server: {error}"));
        }
    }
    Ok(())
}

pub async fn handle_detect_workshop_dir(
    _data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let workshop_dir = native_mods::find_steam_workshop_dir().unwrap_or_default();
    ctx.emitter.emit(
        MessageType::DetectWorkshopDir,
        &serde_json::json!({ "workshop_dir": workshop_dir }),
    );
    Ok(())
}

pub async fn handle_get_server_stats(
    data: ServerIdData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    match psp_db::servers::get_server(&ctx.app.db, data.server_id).await {
        Ok(Some(record)) => {
            let stats = if record.server_type == "native" {
                native_process::process_stats(record.pid)
            } else {
                docker::container_stats(
                    ctx.app.server_services.docker.as_ref(),
                    &record.container_name,
                )
                .await
            };
            ctx.emitter.emit(
                MessageType::GetServerStats,
                &serde_json::json!({ "server_id": record.id, "stats": stats }),
            );
        }
        Ok(None) => emit_business_error(ctx.emitter, "Server not found".to_string()),
        Err(error) => {
            emit_business_error(ctx.emitter, format!("Failed to get server stats: {error}"));
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod test_env {
    use std::sync::Arc;

    use crate::services::docker::mock::MockDocker;
    use crate::services::ServerServices;
    use crate::{AppState, ServerConfig};

    pub(crate) struct TestEnv {
        pub app: Arc<AppState>,
        pub docker: Arc<MockDocker>,
        pub session: psp_core::session::Session,
        pub emitter: crate::emitter::Emitter,
        pub receiver: tokio::sync::mpsc::UnboundedReceiver<axum::extract::ws::Message>,
        pub _scratch: tempfile::TempDir,
    }

    impl TestEnv {
        pub(crate) async fn new() -> Self {
            let scratch = tempfile::tempdir().unwrap();
            let db = psp_db::open(&scratch.path().join("test.db")).await.unwrap();
            let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
            let game_data =
                Arc::new(psp_core::gamedata::GameData::load(&data_dir).expect("repo data dir"));
            let docker = Arc::new(MockDocker::default());
            let config = ServerConfig {
                host: "127.0.0.1".parse().unwrap(),
                port: 0,
                ui_dir: scratch.path().join("ui"),
                data_dir,
                db_path: scratch.path().join("test.db"),
                desktop_mode: false,
            };
            let (live_connections, _live_connections_rx) = tokio::sync::watch::channel(0usize);
            let app = Arc::new(AppState {
                config,
                game_data,
                db,
                dialogs: Arc::new(crate::desktop_dialogs::NullDialogProvider),
                live_connections,
                server_services: Arc::new(ServerServices::with_docker(docker.clone())),
            });
            let (emitter, receiver) = crate::emitter::Emitter::test_channel();
            Self {
                app,
                docker,
                session: psp_core::session::Session::new(),
                emitter,
                receiver,
                _scratch: scratch,
            }
        }

        pub(crate) fn ctx(&mut self) -> crate::dispatcher::HandlerCtx<'_> {
            crate::dispatcher::HandlerCtx {
                session: &mut self.session,
                app: &self.app,
                emitter: &self.emitter,
            }
        }

        /// Drain all frames emitted so far as parsed envelopes.
        pub(crate) fn drain(&mut self) -> Vec<serde_json::Value> {
            let mut envelopes = Vec::new();
            while let Ok(frame) = self.receiver.try_recv() {
                if let axum::extract::ws::Message::Text(text) = frame {
                    envelopes.push(serde_json::from_str(text.as_str()).unwrap());
                }
            }
            envelopes
        }
    }

    pub(crate) fn docker_new_server(container_name: &str) -> psp_db::servers::NewServer {
        psp_db::servers::NewServer {
            name: format!("Server {container_name}"),
            container_name: container_name.to_string(),
            image_name: "omanrod/psp-palworld-server".to_string(),
            server_type: "docker".to_string(),
            game_port: 8211,
            query_port: 27015,
            rest_api_port: 8212,
            data_volume_name: format!("psp-{container_name}-data"),
            saves_path: String::new(),
            mods_path: String::new(),
            logicmods_path: String::new(),
            nativemods_path: String::new(),
            install_path: String::new(),
            steamcmd_path: String::new(),
            launch_args: String::new(),
            workshop_dir: String::new(),
            server_name: "PSP Palworld Server".to_string(),
            server_description: String::new(),
            server_password: String::new(),
            admin_password: "admin".to_string(),
            max_players: 16,
            env_vars: serde_json::Map::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_env::{docker_new_server, TestEnv};
    use super::*;

    #[tokio::test]
    async fn list_servers_returns_empty_list() {
        let mut env = TestEnv::new().await;
        let mut ctx = env.ctx();
        handle_list_servers(serde_json::Value::Null, &mut ctx)
            .await
            .unwrap();
        let messages = env.drain();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["type"], "list_servers");
        assert_eq!(messages[0]["data"], serde_json::json!({"servers": []}));
    }

    #[tokio::test]
    async fn list_servers_includes_status_and_counts() {
        let mut env = TestEnv::new().await;
        let record = psp_db::servers::create_server(&env.app.db, docker_new_server("alpha"))
            .await
            .unwrap();
        // Mock: container exists but is exited → running=false → player_count 0
        env.docker.statuses.lock().unwrap().insert(
            "alpha".to_string(),
            serde_json::json!({"State": {"Status": "exited", "Running": false, "StartedAt": null}}),
        );
        let mut ctx = env.ctx();
        handle_list_servers(serde_json::Value::Null, &mut ctx)
            .await
            .unwrap();
        let messages = env.drain();
        let servers = messages[0]["data"]["servers"].as_array().unwrap();
        assert_eq!(servers.len(), 1);
        let entry = &servers[0];
        assert_eq!(entry["id"], record.id);
        assert_eq!(entry["container_name"], "alpha");
        assert_eq!(entry["status"]["status"], "exited");
        assert_eq!(entry["total_players"], 0);
        assert_eq!(entry["player_count"], 0);
        assert_eq!(entry["env_vars"], serde_json::json!({}));
        // created_at is an isoformat string
        assert!(entry["created_at"].as_str().unwrap().contains('T'));
    }

    #[tokio::test]
    async fn get_server_unknown_id_emits_business_error() {
        let mut env = TestEnv::new().await;
        let mut ctx = env.ctx();
        handle_get_server(ServerIdData { server_id: 999 }, &mut ctx)
            .await
            .unwrap();
        let messages = env.drain();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["type"], "error");
        assert_eq!(
            messages[0]["data"],
            serde_json::json!({"message": "Server not found"})
        );
    }

    #[tokio::test]
    async fn get_server_counts_players_from_saves_dir() {
        let mut env = TestEnv::new().await;
        // Build a saves tree: saves/SaveGames/0/WORLD/Players/{a.sav, b.sav, b_dps.sav}
        let saves_root = env._scratch.path().join("saves");
        let players_dir = saves_root
            .join("SaveGames")
            .join("0")
            .join("WORLDID")
            .join("Players");
        std::fs::create_dir_all(&players_dir).unwrap();
        std::fs::write(players_dir.join("a.sav"), b"x").unwrap();
        std::fs::write(players_dir.join("b.sav"), b"x").unwrap();
        std::fs::write(players_dir.join("b_dps.sav"), b"x").unwrap();
        let mut new_server = docker_new_server("beta");
        new_server.saves_path = saves_root.to_string_lossy().to_string();
        let record = psp_db::servers::create_server(&env.app.db, new_server)
            .await
            .unwrap();
        let mut ctx = env.ctx();
        handle_get_server(
            ServerIdData {
                server_id: record.id,
            },
            &mut ctx,
        )
        .await
        .unwrap();
        let messages = env.drain();
        assert_eq!(messages[0]["type"], "get_server");
        assert_eq!(messages[0]["data"]["total_players"], 2); // _dps excluded
                                                             // Container unknown to the mock → not_found status
        assert_eq!(messages[0]["data"]["status"]["status"], "not_found");
        assert_eq!(messages[0]["data"]["player_count"], 0);
    }

    #[tokio::test]
    async fn detect_workshop_dir_emits_workshop_dir_string() {
        let mut env = TestEnv::new().await;
        let mut ctx = env.ctx();
        handle_detect_workshop_dir(serde_json::Value::Null, &mut ctx)
            .await
            .unwrap();
        let messages = env.drain();
        assert_eq!(messages[0]["type"], "detect_workshop_dir");
        assert!(messages[0]["data"]["workshop_dir"].is_string());
    }

    #[tokio::test]
    async fn get_server_stats_returns_null_for_stopped_docker_server() {
        let mut env = TestEnv::new().await;
        let record = psp_db::servers::create_server(&env.app.db, docker_new_server("gamma"))
            .await
            .unwrap();
        let mut ctx = env.ctx();
        handle_get_server_stats(
            ServerIdData {
                server_id: record.id,
            },
            &mut ctx,
        )
        .await
        .unwrap();
        let messages = env.drain();
        assert_eq!(messages[0]["type"], "get_server_stats");
        assert_eq!(
            messages[0]["data"],
            serde_json::json!({"server_id": record.id, "stats": null})
        );
    }
}
