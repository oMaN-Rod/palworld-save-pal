//! Server-management handlers. Mirrors ws/handlers/server_handler.py.
//!
//! Error convention (Python parity): business failures emit message type
//! `error` with data {"message": "<text>"} — no trace key — and the handler
//! returns Ok(()). Only payload-parse failures use the HandlerError path.
use std::path::Path;

use serde_json::Value;

use psp_core::session::{SaveKind, SaveSession};
use psp_db::servers::{NewServer, ServerRecord};

use crate::dispatcher::HandlerCtx;
use crate::emitter::Emitter;
use crate::handler_error::HandlerError;
use crate::handlers::save_file;
use crate::messages::MessageType;
use crate::services::{
    docker, docker_mods, native_config, native_mods, native_process, ServerProcessStatus,
};
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

fn default_image_name() -> String {
    "omanrod/psp-palworld-server".to_string()
}
fn default_server_type() -> String {
    "docker".to_string()
}
fn default_game_port() -> i64 {
    8211
}
fn default_query_port() -> i64 {
    27015
}
fn default_rest_api_port() -> i64 {
    8212
}
fn default_server_name() -> String {
    "PSP Palworld Server".to_string()
}
fn default_admin_password() -> String {
    "admin".to_string()
}
fn default_max_players() -> i64 {
    16
}

/// messages.py CreateServerData — field names and defaults verbatim.
#[derive(Debug, serde::Deserialize)]
pub struct CreateServerData {
    pub name: String,
    pub container_name: String,
    #[serde(default = "default_image_name")]
    pub image_name: String,
    #[serde(default = "default_server_type")]
    pub server_type: String,
    #[serde(default = "default_game_port")]
    pub game_port: i64,
    #[serde(default = "default_query_port")]
    pub query_port: i64,
    #[serde(default = "default_rest_api_port")]
    pub rest_api_port: i64,
    #[serde(default = "default_server_name")]
    pub server_name: String,
    #[serde(default)]
    pub server_description: String,
    #[serde(default)]
    pub server_password: String,
    #[serde(default = "default_admin_password")]
    pub admin_password: String,
    #[serde(default = "default_max_players")]
    pub max_players: i64,
    #[serde(default)]
    pub env_vars: serde_json::Map<String, Value>,
    #[serde(default)]
    pub steamcmd_path: String,
    #[serde(default)]
    pub install_path: String,
    #[serde(default)]
    pub launch_args: String,
    #[serde(default)]
    pub workshop_dir: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateServerData {
    pub server_id: i64,
    pub updates: serde_json::Map<String, Value>,
}

fn emit_creation_progress(emitter: &Emitter, message: &str) {
    emitter.emit(
        MessageType::ServerCreationProgress,
        &serde_json::json!({ "message": message }),
    );
}

async fn persist_steamcmd_path(
    db: &sqlx::SqlitePool,
    server_id: i64,
    steamcmd_path: &str,
) -> Result<(), String> {
    let mut updates = serde_json::Map::new();
    updates.insert(
        "steamcmd_path".to_string(),
        Value::String(steamcmd_path.to_string()),
    );
    psp_db::servers::update_server(db, server_id, &updates)
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// create_server_handler parity. Returns Err(String) only for failures Python
/// would catch in its outer try/except ("Failed to create server: {e}").
async fn create_server_impl(
    data: CreateServerData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), String> {
    let emitter = ctx.emitter;
    let db = &ctx.app.db;
    let is_native = data.server_type == "native";

    let allocated = psp_db::servers::allocated_ports(db)
        .await
        .map_err(|error| error.to_string())?;
    for port in [data.game_port, data.query_port, data.rest_api_port] {
        if allocated.contains(&(port as u16)) {
            emit_business_error(
                emitter,
                format!("Port {port} is already allocated to another server"),
            );
            return Ok(());
        }
    }

    if is_native {
        if data.install_path.is_empty() {
            emit_business_error(
                emitter,
                "Install path is required for native servers".to_string(),
            );
            return Ok(());
        }
        emit_creation_progress(emitter, "Validating server configuration...");

        let mut workshop_dir = data.workshop_dir.clone();
        if workshop_dir.is_empty() {
            emit_creation_progress(emitter, "Auto-detecting Steam Workshop directory...");
            workshop_dir = native_mods::find_steam_workshop_dir().unwrap_or_default();
            if workshop_dir.is_empty() {
                emit_creation_progress(
                    emitter,
                    "Steam Workshop directory not found (can be set later)",
                );
            } else {
                emit_creation_progress(emitter, &format!("Found Steam Workshop at {workshop_dir}"));
            }
        }

        let new_server = NewServer {
            name: data.name.clone(),
            container_name: data.container_name.clone(),
            image_name: String::new(),
            server_type: "native".to_string(),
            game_port: data.game_port,
            query_port: data.query_port,
            rest_api_port: data.rest_api_port,
            data_volume_name: String::new(),
            saves_path: native_config::saves_path(&data.install_path),
            mods_path: native_config::mods_path(&data.install_path),
            logicmods_path: native_config::logicmods_path(&data.install_path),
            nativemods_path: native_config::nativemods_path(&data.install_path),
            install_path: data.install_path.clone(),
            steamcmd_path: data.steamcmd_path.clone(),
            launch_args: data.launch_args.clone(),
            workshop_dir: workshop_dir.clone(),
            server_name: data.server_name.clone(),
            server_description: data.server_description.clone(),
            server_password: data.server_password.clone(),
            admin_password: data.admin_password.clone(),
            max_players: data.max_players,
            env_vars: data.env_vars.clone(),
        };
        let mut record = psp_db::servers::create_server(db, new_server)
            .await
            .map_err(|error| error.to_string())?;

        // SteamCMD resolution: user-provided > auto-detect > auto-download.
        let mut steamcmd_path = data.steamcmd_path.clone();
        if steamcmd_path.is_empty() {
            emit_creation_progress(emitter, "Auto-detecting SteamCMD...");
            if let Some(found) = native_process::find_steamcmd() {
                emit_creation_progress(emitter, &format!("Found SteamCMD at {found}"));
                persist_steamcmd_path(db, record.id, &found).await?;
                steamcmd_path = found;
            }
        }

        emit_creation_progress(emitter, "Searching for existing PalServer installation...");
        let source_path = native_process::find_existing_server(&steamcmd_path, &data.install_path);
        if let Some(ref source) = source_path {
            emit_creation_progress(
                emitter,
                &format!("Found existing server at {source}, copying base files..."),
            );
        } else {
            if steamcmd_path.is_empty() {
                let steamcmd_dir = native_process::default_steamcmd_dir();
                emit_creation_progress(
                    emitter,
                    &format!(
                        "SteamCMD not found. Downloading to {}...",
                        steamcmd_dir.display()
                    ),
                );
                let downloaded = native_process::ensure_steamcmd(&steamcmd_dir)
                    .await
                    .map_err(|error| error.to_string())?;
                steamcmd_path = downloaded.to_string_lossy().to_string();
                persist_steamcmd_path(db, record.id, &steamcmd_path).await?;
            } else {
                let steamcmd_dir = if steamcmd_path.ends_with(".exe") {
                    std::path::Path::new(&steamcmd_path)
                        .parent()
                        .map(|parent| parent.to_path_buf())
                        .unwrap_or_default()
                } else {
                    std::path::PathBuf::from(&steamcmd_path)
                };
                emit_creation_progress(emitter, "Setting up SteamCMD...");
                native_process::ensure_steamcmd(&steamcmd_dir)
                    .await
                    .map_err(|error| error.to_string())?;
            }
            emit_creation_progress(
                emitter,
                "Downloading Palworld Dedicated Server via SteamCMD (this may take a while)...",
            );
            record = psp_db::servers::get_server(db, record.id)
                .await
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "server row vanished during creation".to_string())?;
            if record.steamcmd_path.is_empty() {
                persist_steamcmd_path(db, record.id, &steamcmd_path).await?;
                record = psp_db::servers::get_server(db, record.id)
                    .await
                    .map_err(|error| error.to_string())?
                    .ok_or_else(|| "server row vanished during creation".to_string())?;
            }
        }

        let created = native_process::create_native_server(&record, source_path.as_deref()).await;
        if !created {
            psp_db::servers::delete_server(db, record.id)
                .await
                .map_err(|error| error.to_string())?;
            emit_creation_progress(emitter, "");
            emit_business_error(
                emitter,
                "Failed to create native server installation".to_string(),
            );
            return Ok(());
        }

        emit_creation_progress(emitter, "Writing server configuration files...");
        native_mods::write_palmodsettings(&data.install_path, true, &[], &workshop_dir)
            .map_err(|error| error.to_string())?;
        emit_creation_progress(emitter, "");

        let mut result = server_to_wire_json(&record);
        result["status"] =
            serde_json::to_value(native_process::process_status(record.pid)).expect("serializes");
        result["player_count"] = Value::from(0);
        emitter.emit(MessageType::CreateServer, &result);
    } else {
        emit_creation_progress(emitter, "Validating server configuration...");
        let base_path = std::env::current_dir()
            .map_err(|error| error.to_string())?
            .join("servers")
            .join(&data.container_name);
        let new_server = NewServer {
            name: data.name.clone(),
            container_name: data.container_name.clone(),
            image_name: data.image_name.clone(),
            server_type: "docker".to_string(),
            game_port: data.game_port,
            query_port: data.query_port,
            rest_api_port: data.rest_api_port,
            data_volume_name: format!("psp-{}-data", data.container_name),
            saves_path: base_path.join("saves").to_string_lossy().to_string(),
            mods_path: base_path.join("mods").to_string_lossy().to_string(),
            logicmods_path: base_path.join("logicmods").to_string_lossy().to_string(),
            nativemods_path: base_path.join("nativemods").to_string_lossy().to_string(),
            install_path: String::new(),
            steamcmd_path: String::new(),
            launch_args: String::new(),
            workshop_dir: String::new(),
            server_name: data.server_name.clone(),
            server_description: data.server_description.clone(),
            server_password: data.server_password.clone(),
            admin_password: data.admin_password.clone(),
            max_players: data.max_players,
            env_vars: data.env_vars.clone(),
        };
        let record = psp_db::servers::create_server(db, new_server)
            .await
            .map_err(|error| error.to_string())?;

        emit_creation_progress(
            emitter,
            &format!("Pulling Docker image {}...", data.image_name),
        );
        docker::create_server_container(ctx.app.server_services.docker.as_ref(), &record)
            .await
            .map_err(|error| error.to_string())?;
        emit_creation_progress(emitter, "Container started successfully");
        emit_creation_progress(emitter, "");

        let status = docker::container_status(
            ctx.app.server_services.docker.as_ref(),
            &record.container_name,
        )
        .await;
        let mut result = server_to_wire_json(&record);
        result["status"] = serde_json::to_value(&status).expect("serializes");
        result["player_count"] = Value::from(0);
        emitter.emit(MessageType::CreateServer, &result);
    }
    Ok(())
}

pub async fn handle_create_server(
    data: CreateServerData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if let Err(message) = create_server_impl(data, ctx).await {
        emit_business_error(ctx.emitter, format!("Failed to create server: {message}"));
    }
    Ok(())
}

async fn update_server_impl(
    data: UpdateServerData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), String> {
    let emitter = ctx.emitter;
    let db = &ctx.app.db;
    let Some(old_record) = psp_db::servers::get_server(db, data.server_id)
        .await
        .map_err(|error| error.to_string())?
    else {
        emit_business_error(emitter, "Server not found".to_string());
        return Ok(());
    };
    let Some(mut record) = psp_db::servers::update_server(db, data.server_id, &data.updates)
        .await
        .map_err(|error| error.to_string())?
    else {
        emit_business_error(emitter, "Failed to update server".to_string());
        return Ok(());
    };

    let env_changed = data
        .updates
        .get("env_vars")
        .map(|value| !value.is_null())
        .unwrap_or(false);
    let ports_changed = ["game_port", "query_port", "rest_api_port"]
        .iter()
        .any(|key| data.updates.contains_key(*key));
    let identity_changed = [
        "server_name",
        "server_description",
        "server_password",
        "admin_password",
        "max_players",
    ]
    .iter()
    .any(|key| data.updates.contains_key(*key));
    let needs_apply = env_changed || ports_changed || identity_changed;

    if record.server_type == "native" {
        if needs_apply {
            native_config::write_palworld_settings(&record).map_err(|error| error.to_string())?;
            if record.pid.is_some() {
                let status = native_process::process_status(record.pid);
                if status.running {
                    native_process::stop_server_process(
                        &record,
                        &ctx.app.server_services.palworld_api,
                    )
                    .await;
                    if let Some(new_pid) = native_process::start_server_process(&record) {
                        let mut pid_update = serde_json::Map::new();
                        pid_update.insert("pid".to_string(), Value::from(new_pid));
                        if let Some(refreshed) =
                            psp_db::servers::update_server(db, record.id, &pid_update)
                                .await
                                .map_err(|error| error.to_string())?
                        {
                            record = refreshed;
                        }
                    }
                }
            }
        }
    } else if needs_apply {
        let docker_api = ctx.app.server_services.docker.as_ref();
        docker::stop_server_container(docker_api, &old_record.container_name).await;
        docker::remove_server_container(docker_api, &old_record.container_name, false).await;
        docker::create_server_container(docker_api, &record)
            .await
            .map_err(|error| error.to_string())?;
    }

    let status = server_status(ctx.app, &record).await;
    let mut result = server_to_wire_json(&record);
    result["status"] = serde_json::to_value(&status).expect("serializes");
    emitter.emit(MessageType::UpdateServer, &result);
    Ok(())
}

pub async fn handle_update_server(
    data: UpdateServerData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if let Err(message) = update_server_impl(data, ctx).await {
        emit_business_error(ctx.emitter, format!("Failed to update server: {message}"));
    }
    Ok(())
}

pub async fn handle_delete_server(
    data: ServerIdData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let emitter = ctx.emitter;
    let db = &ctx.app.db;
    let result: Result<(), String> = async {
        let Some(record) = psp_db::servers::get_server(db, data.server_id)
            .await
            .map_err(|error| error.to_string())?
        else {
            emit_business_error(emitter, "Server not found".to_string());
            return Ok(());
        };
        if record.server_type == "native" {
            if record.pid.is_some() {
                native_process::stop_server_process(&record, &ctx.app.server_services.palworld_api)
                    .await;
            }
            // Python: remove_server(install_path, remove_data=False) — keeps files.
        } else {
            let docker_api = ctx.app.server_services.docker.as_ref();
            docker::stop_server_container(docker_api, &record.container_name).await;
            // Python's delete_server_handler discards remove_server's bool
            // return value unconditionally — the DB row and delete_server
            // response are unaffected by a Docker-side removal failure.
            docker::remove_server_container(docker_api, &record.container_name, true).await;
        }
        psp_db::servers::delete_server(db, record.id)
            .await
            .map_err(|error| error.to_string())?;
        emitter.emit(
            MessageType::DeleteServer,
            &serde_json::json!({ "server_id": record.id }),
        );
        Ok(())
    }
    .await;
    if let Err(message) = result {
        emit_business_error(emitter, format!("Failed to delete server: {message}"));
    }
    Ok(())
}

pub async fn handle_start_server(
    data: ServerIdData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let emitter = ctx.emitter;
    let db = &ctx.app.db;
    let result: Result<(), String> = async {
        let Some(record) = psp_db::servers::get_server(db, data.server_id)
            .await
            .map_err(|error| error.to_string())?
        else {
            emit_business_error(emitter, "Server not found".to_string());
            return Ok(());
        };
        let (success, status) = if record.server_type == "native" {
            let new_pid = native_process::start_server_process(&record);
            if let Some(pid) = new_pid {
                let mut pid_update = serde_json::Map::new();
                pid_update.insert("pid".to_string(), Value::from(pid));
                psp_db::servers::update_server(db, record.id, &pid_update)
                    .await
                    .map_err(|error| error.to_string())?;
            }
            let status = native_process::process_status(new_pid.map(i64::from));
            (new_pid.is_some(), Some(status))
        } else {
            let docker_api = ctx.app.server_services.docker.as_ref();
            let success = docker::start_server_container(docker_api, &record.container_name).await;
            let status = docker::container_status(docker_api, &record.container_name).await;
            (success, status)
        };
        emitter.emit(
            MessageType::ServerStatusUpdate,
            &serde_json::json!({ "server_id": record.id, "status": status, "success": success }),
        );
        Ok(())
    }
    .await;
    if let Err(message) = result {
        emit_business_error(emitter, format!("Failed to start server: {message}"));
    }
    Ok(())
}

pub async fn handle_stop_server(
    data: ServerIdData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let emitter = ctx.emitter;
    let db = &ctx.app.db;
    let result: Result<(), String> = async {
        let Some(record) = psp_db::servers::get_server(db, data.server_id)
            .await
            .map_err(|error| error.to_string())?
        else {
            emit_business_error(emitter, "Server not found".to_string());
            return Ok(());
        };
        emit_creation_progress(emitter, &format!("Stopping server \"{}\"...", record.name));
        let (success, status) = if record.server_type == "native" {
            emit_creation_progress(emitter, "Sending shutdown command to server...");
            let success =
                native_process::stop_server_process(&record, &ctx.app.server_services.palworld_api)
                    .await;
            if success {
                let mut pid_update = serde_json::Map::new();
                pid_update.insert("pid".to_string(), Value::Null);
                psp_db::servers::update_server(db, record.id, &pid_update)
                    .await
                    .map_err(|error| error.to_string())?;
            }
            (success, Some(native_process::process_status(None)))
        } else {
            emit_creation_progress(emitter, "Stopping Docker container...");
            let docker_api = ctx.app.server_services.docker.as_ref();
            let success = docker::stop_server_container(docker_api, &record.container_name).await;
            let status = docker::container_status(docker_api, &record.container_name).await;
            (success, status)
        };
        emit_creation_progress(emitter, "");
        emitter.emit(
            MessageType::ServerStatusUpdate,
            &serde_json::json!({ "server_id": record.id, "status": status, "success": success }),
        );
        Ok(())
    }
    .await;
    if let Err(message) = result {
        emit_business_error(emitter, format!("Failed to stop server: {message}"));
    }
    Ok(())
}

fn default_api_method() -> String {
    "GET".to_string()
}
fn default_mod_type() -> String {
    "ue4ss".to_string()
}

/// messages.py ServerApiCallMessage.data — field names and defaults verbatim.
#[derive(Debug, serde::Deserialize)]
pub struct ServerApiCallData {
    pub server_id: i64,
    pub endpoint: String,
    #[serde(default = "default_api_method")]
    pub method: String,
    #[serde(default)]
    pub payload: Option<Value>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ToggleServerModData {
    pub server_id: i64,
    pub mod_name: String,
    pub enabled: bool,
}

#[derive(Debug, serde::Deserialize)]
pub struct InstallServerModData {
    pub server_id: i64,
    pub mod_name: String,
    /// base64-encoded zip
    pub mod_data: String,
    #[serde(default = "default_mod_type")]
    pub mod_type: String,
}

/// server_api_call_handler parity: proxies to the Palworld dedicated-server
/// REST API at 127.0.0.1:{rest_api_port} using the server's admin_password.
pub async fn handle_server_api_call(
    data: ServerApiCallData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let emitter = ctx.emitter;
    let record = match psp_db::servers::get_server(&ctx.app.db, data.server_id).await {
        Ok(Some(record)) => record,
        Ok(None) => {
            emit_business_error(emitter, "Server not found".to_string());
            return Ok(());
        }
        Err(error) => {
            emit_business_error(emitter, format!("API call failed: {error}"));
            return Ok(());
        }
    };
    match ctx
        .app
        .server_services
        .palworld_api
        .rest_api_call(
            "127.0.0.1",
            record.rest_api_port as u16,
            &record.admin_password,
            &data.endpoint,
            &data.method,
            data.payload.as_ref(),
        )
        .await
    {
        Ok(result) => emitter.emit(
            MessageType::ServerApiResponse,
            &serde_json::json!({
                "server_id": record.id,
                "endpoint": data.endpoint,
                "result": result
            }),
        ),
        Err(error) => emit_business_error(emitter, format!("API call failed: {error}")),
    }
    Ok(())
}

/// list_server_mods_handler parity: native servers use the PalModSettings.ini
/// workshop scan; docker servers merge ue4ss (mods.txt), logic (.pak), and
/// native (.dll) mod listings.
pub async fn handle_list_server_mods(
    data: ServerIdData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let emitter = ctx.emitter;
    let record = match psp_db::servers::get_server(&ctx.app.db, data.server_id).await {
        Ok(Some(record)) => record,
        Ok(None) => {
            emit_business_error(emitter, "Server not found".to_string());
            return Ok(());
        }
        Err(error) => {
            emit_business_error(emitter, format!("Failed to list mods: {error}"));
            return Ok(());
        }
    };
    let mods = if record.server_type == "native" {
        native_mods::list_native_server_mods(&record)
    } else {
        let mut mods = docker_mods::list_ue4ss_mods(&record.mods_path);
        if let Ok(entries) = std::fs::read_dir(&record.logicmods_path) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".pak") {
                    mods.push(serde_json::json!({
                        "mod_name": name,
                        "mod_type": "logic",
                        "enabled": true
                    }));
                }
            }
        }
        mods.extend(docker_mods::list_native_dll_mods(&record.nativemods_path));
        mods
    };
    emitter.emit(
        MessageType::ListServerMods,
        &serde_json::json!({ "server_id": record.id, "mods": mods }),
    );
    Ok(())
}

/// toggle_server_mod_handler parity.
pub async fn handle_toggle_server_mod(
    data: ToggleServerModData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let emitter = ctx.emitter;
    let record = match psp_db::servers::get_server(&ctx.app.db, data.server_id).await {
        Ok(Some(record)) => record,
        Ok(None) => {
            emit_business_error(emitter, "Server not found".to_string());
            return Ok(());
        }
        Err(error) => {
            emit_business_error(emitter, format!("Failed to toggle mod: {error}"));
            return Ok(());
        }
    };
    let toggle_result = if record.server_type == "native" {
        native_mods::toggle_native_mod(&record.install_path, &data.mod_name, data.enabled)
    } else {
        docker_mods::set_mod_enabled(&record.mods_path, &data.mod_name, data.enabled)
    };
    if let Err(error) = toggle_result {
        emit_business_error(emitter, format!("Failed to toggle mod: {error}"));
        return Ok(());
    }
    emitter.emit(
        MessageType::ToggleServerMod,
        &serde_json::json!({
            "server_id": record.id,
            "mod_name": data.mod_name,
            "enabled": data.enabled
        }),
    );
    Ok(())
}

/// install_server_mod_handler parity: native servers install via the
/// Steam-workshop-style extraction; docker servers dispatch on mod_type
/// (native DLL vs ue4ss/logic zip).
pub async fn handle_install_server_mod(
    data: InstallServerModData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let emitter = ctx.emitter;
    let record = match psp_db::servers::get_server(&ctx.app.db, data.server_id).await {
        Ok(Some(record)) => record,
        Ok(None) => {
            emit_business_error(emitter, "Server not found".to_string());
            return Ok(());
        }
        Err(error) => {
            emit_business_error(emitter, format!("Failed to install mod: {error}"));
            return Ok(());
        }
    };
    let success = if record.server_type == "native" {
        native_mods::install_native_workshop_mod(
            &record.install_path,
            &data.mod_name,
            &data.mod_data,
        )
    } else if data.mod_type == "native" {
        docker_mods::install_native_dll_mod(&record.nativemods_path, &data.mod_data)
    } else {
        let target_path = if data.mod_type == "ue4ss" {
            &record.mods_path
        } else {
            &record.logicmods_path
        };
        docker_mods::install_zip_mod(target_path, &data.mod_name, &data.mod_data)
    };
    emitter.emit(
        MessageType::InstallServerMod,
        &serde_json::json!({
            "server_id": record.id,
            "mod_name": data.mod_name,
            "success": success
        }),
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// load_server_save — port of server_handler.py::load_server_save_handler.
// ---------------------------------------------------------------------------

/// Business-error and loading core of `handle_load_server_save`. Locates the
/// server's saved world under `<saves_path>/SaveGames/0/<world>/Level.sav`
/// and runs it through the SAME Phase 1 load pipeline `handle_select_save`
/// uses (`save_file::validate_steam_save_directory` /
/// `save_file::discover_player_file_refs` / `SaveSession::load` /
/// `save_file::emit_summary_messages`) rather than reimplementing it -- see
/// `SteamSaveLayout`'s doc comment in `save_file.rs` for why those helpers
/// are `pub(crate)` in the first place.
///
/// Returns `Err(String)` only for failures Python's outer `except Exception`
/// would catch (`"Failed to load server save: {e}"`, `load_server_save_handler`'s
/// final `except` clause). Every earlier business failure (server not found,
/// still running, no save data at the expected location, no world dirs, no
/// Level.sav, or an invalid steam directory) emits its own `error` frame
/// directly and returns `Ok(())`, matching each early `return` in Python.
async fn load_server_save_impl(data: ServerIdData, ctx: &mut HandlerCtx<'_>) -> Result<(), String> {
    let emitter = ctx.emitter;
    let db = &ctx.app.db;
    let Some(record) = psp_db::servers::get_server(db, data.server_id)
        .await
        .map_err(|error| error.to_string())?
    else {
        emit_business_error(emitter, "Server not found".to_string());
        return Ok(());
    };

    // Verify server is stopped.
    let status = server_status(ctx.app, &record).await;
    if status
        .as_ref()
        .map(|current| current.running)
        .unwrap_or(false)
    {
        emit_business_error(
            emitter,
            "Server must be stopped before loading saves. Please stop the server first."
                .to_string(),
        );
        return Ok(());
    }

    // Find the save directory: saves/SaveGames/0/{world_guid}/
    let save_games_path = Path::new(&record.saves_path).join("SaveGames").join("0");
    if !save_games_path.is_dir() {
        emit_business_error(
            emitter,
            format!("No save data found at {}", save_games_path.display()),
        );
        return Ok(());
    }

    // Use the first (usually only) world directory -- matches Python's
    // `os.listdir(save_games_path)[0]` (both are filesystem-enumeration-order
    // dependent; this is bug-compatible, not a design choice).
    let world_dir = std::fs::read_dir(&save_games_path)
        .map_err(|error| error.to_string())?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.is_dir());
    let Some(world_dir) = world_dir else {
        emit_business_error(
            emitter,
            "No world saves found in server save directory".to_string(),
        );
        return Ok(());
    };

    let level_sav_path = world_dir.join("Level.sav");
    if !level_sav_path.exists() {
        emit_business_error(emitter, "Level.sav not found in save directory".to_string());
        return Ok(());
    }

    // ---- Shared Phase 1 load pipeline (identical to handle_select_save's
    // steam branch) ----
    let layout = match save_file::validate_steam_save_directory(&level_sav_path.to_string_lossy()) {
        Ok(layout) => layout,
        Err(error) => {
            emit_business_error(emitter, error.to_string());
            return Ok(());
        }
    };
    let level_sav_bytes = std::fs::read(&layout.level_sav).map_err(|error| error.to_string())?;
    let level_meta_bytes = match &layout.level_meta {
        Some(meta_path) => Some(std::fs::read(meta_path).map_err(|error| error.to_string())?),
        None => None,
    };
    let (player_file_refs, player_discovery_order) =
        save_file::discover_player_file_refs(&layout.players_dir)
            .map_err(|error| error.to_string())?;

    let progress = emitter.progress_sink();
    let session = SaveSession::load(
        SaveKind::Steam {
            level_path: layout.level_sav.clone(),
        },
        level_sav_path.to_string_lossy().into_owned(),
        "steam",
        &level_sav_bytes,
        level_meta_bytes.as_deref(),
        player_file_refs,
        layout.global_pal_storage_sav.clone(),
        // load_server_save goes through the same process_save_files pipeline
        // select_save does -- keep the leading generic "Loading Level.sav..." frame.
        true,
        &progress,
    )
    .map_err(|error| error.to_string())?;

    // Python: app_state.settings.save_dir = world_dir
    psp_db::settings::update_save_dir(db, &world_dir.to_string_lossy())
        .await
        .map_err(|error| error.to_string())?;

    let has_gps = layout.global_pal_storage_sav.is_some();
    emitter.emit(
        MessageType::LoadedSaveFiles,
        &serde_json::json!({
            "level": layout.level_sav.to_string_lossy().into_owned(),
            "players": player_discovery_order
                .iter()
                .map(|uid| uid.to_string())
                .collect::<Vec<_>>(),
            "world_name": session.world_name,
            "type": "steam",
            "size": session.size,
            "has_gps": has_gps,
            "server_id": record.id,
            "server_name": record.name,
        }),
    );
    save_file::emit_summary_messages(&session, emitter);

    ctx.session.save = Some(session);
    Ok(())
}

pub async fn handle_load_server_save(
    data: ServerIdData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if let Err(message) = load_server_save_impl(data, ctx).await {
        emit_business_error(
            ctx.emitter,
            format!("Failed to load server save: {message}"),
        );
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

    #[tokio::test]
    async fn create_docker_server_emits_progress_then_create_server() {
        let mut env = TestEnv::new().await;
        let data: CreateServerData = serde_json::from_value(serde_json::json!({
            "name": "My Server",
            "container_name": "alpha"
        }))
        .unwrap();
        // Defaults from messages.py must apply
        assert_eq!(data.image_name, "omanrod/psp-palworld-server");
        assert_eq!(data.server_type, "docker");
        assert_eq!(data.game_port, 8211);
        assert_eq!(data.query_port, 27015);
        assert_eq!(data.rest_api_port, 8212);
        assert_eq!(data.server_name, "PSP Palworld Server");
        assert_eq!(data.admin_password, "admin");
        assert_eq!(data.max_players, 16);

        let mut ctx = env.ctx();
        handle_create_server(data, &mut ctx).await.unwrap();
        let messages = env.drain();
        let types: Vec<&str> = messages
            .iter()
            .map(|message| message["type"].as_str().unwrap())
            .collect();
        assert_eq!(
            types,
            vec![
                "server_creation_progress", // Validating server configuration...
                "server_creation_progress", // Pulling Docker image ...
                "server_creation_progress", // Container started successfully
                "server_creation_progress", // "" (clear)
                "create_server",
            ]
        );
        assert_eq!(
            messages[0]["data"],
            serde_json::json!({"message": "Validating server configuration..."})
        );
        assert_eq!(
            messages[1]["data"],
            serde_json::json!({"message": "Pulling Docker image omanrod/psp-palworld-server..."})
        );
        assert_eq!(messages[3]["data"], serde_json::json!({"message": ""}));
        let created = &messages[4]["data"];
        assert_eq!(created["container_name"], "alpha");
        assert_eq!(created["data_volume_name"], "psp-alpha-data");
        assert_eq!(created["status"]["running"], true); // mock create starts it
        assert_eq!(created["player_count"], 0);
        assert!(created.get("total_players").is_none()); // create has no total_players
                                                         // DB row exists
        let listed = psp_db::servers::list_servers(&env.app.db).await.unwrap();
        assert_eq!(listed.len(), 1);
        // Host mount dirs are under <cwd>/servers/alpha
        assert!(listed[0].saves_path.ends_with(&format!(
            "servers{0}alpha{0}saves",
            std::path::MAIN_SEPARATOR
        )));
    }

    #[tokio::test]
    async fn create_server_rejects_allocated_ports() {
        let mut env = TestEnv::new().await;
        psp_db::servers::create_server(&env.app.db, docker_new_server("first"))
            .await
            .unwrap();
        let data: CreateServerData = serde_json::from_value(serde_json::json!({
            "name": "Second",
            "container_name": "second"
        }))
        .unwrap();
        let mut ctx = env.ctx();
        handle_create_server(data, &mut ctx).await.unwrap();
        let messages = env.drain();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["type"], "error");
        assert_eq!(
            messages[0]["data"],
            serde_json::json!({"message": "Port 8211 is already allocated to another server"})
        );
    }

    #[tokio::test]
    async fn create_native_server_without_install_path_errors() {
        let mut env = TestEnv::new().await;
        let data: CreateServerData = serde_json::from_value(serde_json::json!({
            "name": "Native",
            "container_name": "native1",
            "server_type": "native"
        }))
        .unwrap();
        let mut ctx = env.ctx();
        handle_create_server(data, &mut ctx).await.unwrap();
        let messages = env.drain();
        assert_eq!(messages[0]["type"], "error");
        assert_eq!(
            messages[0]["data"],
            serde_json::json!({"message": "Install path is required for native servers"})
        );
    }

    #[tokio::test]
    async fn update_server_recreates_docker_container_when_identity_changes() {
        let mut env = TestEnv::new().await;
        let record = psp_db::servers::create_server(&env.app.db, docker_new_server("alpha"))
            .await
            .unwrap();
        let mut updates = serde_json::Map::new();
        updates.insert("server_name".to_string(), serde_json::json!("Renamed"));
        let mut ctx = env.ctx();
        handle_update_server(
            UpdateServerData {
                server_id: record.id,
                updates,
            },
            &mut ctx,
        )
        .await
        .unwrap();
        let messages = env.drain();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["type"], "update_server");
        assert_eq!(messages[0]["data"]["server_name"], "Renamed");
        assert!(messages[0]["data"].get("player_count").is_none()); // update has no counts
        let calls = env.docker.calls.lock().unwrap().clone();
        // stop old, remove (no volume), recreate
        assert!(calls.contains(&"stop:alpha".to_string()));
        assert!(calls.contains(&"remove_container:alpha".to_string()));
        assert!(!calls.contains(&"remove_volume:psp-alpha-data".to_string()));
        assert!(calls.contains(&"create_and_start:alpha".to_string()));
    }

    #[tokio::test]
    async fn update_server_without_relevant_keys_skips_recreation() {
        let mut env = TestEnv::new().await;
        let record = psp_db::servers::create_server(&env.app.db, docker_new_server("alpha"))
            .await
            .unwrap();
        let mut updates = serde_json::Map::new();
        updates.insert("name".to_string(), serde_json::json!("Display Only"));
        let mut ctx = env.ctx();
        handle_update_server(
            UpdateServerData {
                server_id: record.id,
                updates,
            },
            &mut ctx,
        )
        .await
        .unwrap();
        let calls = env.docker.calls.lock().unwrap().clone();
        assert!(!calls
            .iter()
            .any(|call| call.starts_with("create_and_start")));
    }

    #[tokio::test]
    async fn update_unknown_server_emits_not_found() {
        let mut env = TestEnv::new().await;
        let mut ctx = env.ctx();
        handle_update_server(
            UpdateServerData {
                server_id: 42,
                updates: serde_json::Map::new(),
            },
            &mut ctx,
        )
        .await
        .unwrap();
        let messages = env.drain();
        assert_eq!(messages[0]["type"], "error");
        assert_eq!(messages[0]["data"]["message"], "Server not found");
    }

    #[tokio::test]
    async fn delete_docker_server_stops_removes_with_volumes_and_deletes_row() {
        let mut env = TestEnv::new().await;
        let record = psp_db::servers::create_server(&env.app.db, docker_new_server("alpha"))
            .await
            .unwrap();
        let mut ctx = env.ctx();
        handle_delete_server(
            ServerIdData {
                server_id: record.id,
            },
            &mut ctx,
        )
        .await
        .unwrap();
        let messages = env.drain();
        assert_eq!(messages[0]["type"], "delete_server");
        assert_eq!(
            messages[0]["data"],
            serde_json::json!({"server_id": record.id})
        );
        let calls = env.docker.calls.lock().unwrap().clone();
        assert!(calls.contains(&"remove_volume:psp-alpha-data".to_string()));
        assert!(psp_db::servers::get_server(&env.app.db, record.id)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn start_docker_server_emits_server_status_update() {
        let mut env = TestEnv::new().await;
        let record = psp_db::servers::create_server(&env.app.db, docker_new_server("alpha"))
            .await
            .unwrap();
        let mut ctx = env.ctx();
        handle_start_server(
            ServerIdData {
                server_id: record.id,
            },
            &mut ctx,
        )
        .await
        .unwrap();
        let messages = env.drain();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["type"], "server_status_update");
        assert_eq!(messages[0]["data"]["server_id"], record.id);
        assert_eq!(messages[0]["data"]["success"], true);
        assert_eq!(messages[0]["data"]["status"]["running"], true);
    }

    #[tokio::test]
    async fn stop_docker_server_emits_progress_then_status_update() {
        let mut env = TestEnv::new().await;
        let record = psp_db::servers::create_server(&env.app.db, docker_new_server("alpha"))
            .await
            .unwrap();
        env.docker.statuses.lock().unwrap().insert(
            "alpha".to_string(),
            serde_json::json!({"State": {"Status": "running", "Running": true, "StartedAt": "x"}}),
        );
        let mut ctx = env.ctx();
        handle_stop_server(
            ServerIdData {
                server_id: record.id,
            },
            &mut ctx,
        )
        .await
        .unwrap();
        let messages = env.drain();
        let types: Vec<&str> = messages
            .iter()
            .map(|message| message["type"].as_str().unwrap())
            .collect();
        assert_eq!(
            types,
            vec![
                "server_creation_progress", // Stopping server "..."...
                "server_creation_progress", // Stopping Docker container...
                "server_creation_progress", // "" clear
                "server_status_update",
            ]
        );
        assert_eq!(
            messages[0]["data"]["message"],
            format!("Stopping server \"{}\"...", record.name)
        );
        assert_eq!(
            messages[1]["data"]["message"],
            "Stopping Docker container..."
        );
        assert_eq!(messages[3]["data"]["success"], true);
        assert_eq!(messages[3]["data"]["status"]["running"], false);
    }

    async fn spawn_players_stub() -> u16 {
        use axum::routing::get;
        let router = axum::Router::new().route(
            "/v1/api/players",
            get(|| async { axum::Json(serde_json::json!({"players": [{"name": "one"}]})) }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        port
    }

    #[tokio::test]
    async fn server_api_call_proxies_and_emits_server_api_response() {
        let mut env = TestEnv::new().await;
        let stub_port = spawn_players_stub().await;
        let mut new_server = docker_new_server("api");
        new_server.rest_api_port = stub_port as i64;
        let record = psp_db::servers::create_server(&env.app.db, new_server)
            .await
            .unwrap();
        let mut ctx = env.ctx();
        handle_server_api_call(
            ServerApiCallData {
                server_id: record.id,
                endpoint: "players".to_string(),
                method: "GET".to_string(),
                payload: None,
            },
            &mut ctx,
        )
        .await
        .unwrap();
        let messages = env.drain();
        assert_eq!(messages[0]["type"], "server_api_response");
        assert_eq!(messages[0]["data"]["server_id"], record.id);
        assert_eq!(messages[0]["data"]["endpoint"], "players");
        assert_eq!(messages[0]["data"]["result"]["status_code"], 200);
        assert_eq!(
            messages[0]["data"]["result"]["data"]["players"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn server_api_call_failure_emits_api_call_failed() {
        let mut env = TestEnv::new().await;
        let mut new_server = docker_new_server("dead-api");
        new_server.rest_api_port = 1; // nothing listens here
        let record = psp_db::servers::create_server(&env.app.db, new_server)
            .await
            .unwrap();
        let mut ctx = env.ctx();
        handle_server_api_call(
            ServerApiCallData {
                server_id: record.id,
                endpoint: "info".to_string(),
                method: "GET".to_string(),
                payload: None,
            },
            &mut ctx,
        )
        .await
        .unwrap();
        let messages = env.drain();
        assert_eq!(messages[0]["type"], "error");
        assert!(messages[0]["data"]["message"]
            .as_str()
            .unwrap()
            .starts_with("API call failed: "));
    }

    #[tokio::test]
    async fn list_server_mods_for_docker_merges_ue4ss_logic_and_native() {
        let mut env = TestEnv::new().await;
        let scratch = env._scratch.path().to_path_buf();
        let mods_dir = scratch.join("mods");
        let logic_dir = scratch.join("logicmods");
        let native_dir = scratch.join("nativemods");
        std::fs::create_dir_all(mods_dir.join("LuaMod")).unwrap();
        std::fs::write(mods_dir.join("mods.txt"), "LuaMod : 1\n").unwrap();
        std::fs::create_dir_all(&logic_dir).unwrap();
        std::fs::write(logic_dir.join("big.pak"), b"pak").unwrap();
        std::fs::create_dir_all(&native_dir).unwrap();
        std::fs::write(native_dir.join("inject.dll"), b"MZ").unwrap();
        let mut new_server = docker_new_server("modded");
        new_server.mods_path = mods_dir.to_string_lossy().to_string();
        new_server.logicmods_path = logic_dir.to_string_lossy().to_string();
        new_server.nativemods_path = native_dir.to_string_lossy().to_string();
        let record = psp_db::servers::create_server(&env.app.db, new_server)
            .await
            .unwrap();
        let mut ctx = env.ctx();
        handle_list_server_mods(
            ServerIdData {
                server_id: record.id,
            },
            &mut ctx,
        )
        .await
        .unwrap();
        let messages = env.drain();
        assert_eq!(messages[0]["type"], "list_server_mods");
        let mods = messages[0]["data"]["mods"].as_array().unwrap();
        assert_eq!(
            mods,
            &vec![
                serde_json::json!({"mod_name": "LuaMod", "mod_type": "ue4ss", "enabled": true}),
                serde_json::json!({"mod_name": "big.pak", "mod_type": "logic", "enabled": true}),
                serde_json::json!({"mod_name": "inject.dll", "mod_type": "native", "enabled": true}),
            ]
        );
    }

    #[tokio::test]
    async fn toggle_server_mod_updates_mods_txt_and_echoes() {
        let mut env = TestEnv::new().await;
        let mods_dir = env._scratch.path().join("mods");
        std::fs::create_dir_all(&mods_dir).unwrap();
        let mut new_server = docker_new_server("toggler");
        new_server.mods_path = mods_dir.to_string_lossy().to_string();
        let record = psp_db::servers::create_server(&env.app.db, new_server)
            .await
            .unwrap();
        let mut ctx = env.ctx();
        handle_toggle_server_mod(
            ToggleServerModData {
                server_id: record.id,
                mod_name: "LuaMod".to_string(),
                enabled: true,
            },
            &mut ctx,
        )
        .await
        .unwrap();
        let messages = env.drain();
        assert_eq!(messages[0]["type"], "toggle_server_mod");
        assert_eq!(
            messages[0]["data"],
            serde_json::json!({"server_id": record.id, "mod_name": "LuaMod", "enabled": true})
        );
        let mods_txt = std::fs::read_to_string(mods_dir.join("mods.txt")).unwrap();
        assert_eq!(mods_txt, "LuaMod : 1\n");
    }

    #[tokio::test]
    async fn install_server_mod_ue4ss_extracts_and_reports_success() {
        let mut env = TestEnv::new().await;
        let mods_dir = env._scratch.path().join("mods");
        std::fs::create_dir_all(&mods_dir).unwrap();
        let mut new_server = docker_new_server("installer");
        new_server.mods_path = mods_dir.to_string_lossy().to_string();
        let record = psp_db::servers::create_server(&env.app.db, new_server)
            .await
            .unwrap();
        let zip_b64 = crate::services::docker_mods::zip_fixture::base64_zip(&[(
            "scripts/main.lua",
            "print('hi')",
        )]);
        let mut ctx = env.ctx();
        handle_install_server_mod(
            InstallServerModData {
                server_id: record.id,
                mod_name: "LuaMod".to_string(),
                mod_data: zip_b64,
                mod_type: "ue4ss".to_string(),
            },
            &mut ctx,
        )
        .await
        .unwrap();
        let messages = env.drain();
        assert_eq!(messages[0]["type"], "install_server_mod");
        assert_eq!(
            messages[0]["data"],
            serde_json::json!({"server_id": record.id, "mod_name": "LuaMod", "success": true})
        );
        assert!(mods_dir
            .join("LuaMod")
            .join("scripts")
            .join("main.lua")
            .exists());
    }

    #[tokio::test]
    async fn install_server_mod_default_mod_type_is_ue4ss() {
        let data: InstallServerModData = serde_json::from_value(serde_json::json!({
            "server_id": 1,
            "mod_name": "X",
            "mod_data": "AAAA"
        }))
        .unwrap();
        assert_eq!(data.mod_type, "ue4ss");
    }

    #[tokio::test]
    async fn load_server_save_requires_stopped_server() {
        let mut env = TestEnv::new().await;
        let record = psp_db::servers::create_server(&env.app.db, docker_new_server("running"))
            .await
            .unwrap();
        env.docker.statuses.lock().unwrap().insert(
            "running".to_string(),
            serde_json::json!({"State": {"Status": "running", "Running": true, "StartedAt": "x"}}),
        );
        let mut ctx = env.ctx();
        handle_load_server_save(
            ServerIdData {
                server_id: record.id,
            },
            &mut ctx,
        )
        .await
        .unwrap();
        let messages = env.drain();
        assert_eq!(messages[0]["type"], "error");
        assert_eq!(
            messages[0]["data"]["message"],
            "Server must be stopped before loading saves. Please stop the server first."
        );
    }

    #[tokio::test]
    async fn load_server_save_reports_missing_save_dir() {
        let mut env = TestEnv::new().await;
        let mut new_server = docker_new_server("empty-saves");
        new_server.saves_path = env
            ._scratch
            .path()
            .join("nosaves")
            .to_string_lossy()
            .to_string();
        let record = psp_db::servers::create_server(&env.app.db, new_server)
            .await
            .unwrap();
        let mut ctx = env.ctx();
        handle_load_server_save(
            ServerIdData {
                server_id: record.id,
            },
            &mut ctx,
        )
        .await
        .unwrap();
        let messages = env.drain();
        assert_eq!(messages[0]["type"], "error");
        assert!(messages[0]["data"]["message"]
            .as_str()
            .unwrap()
            .starts_with("No save data found at "));
    }

    /// Full load path — needs a real world save. Set PSP_TEST_SAVE_DIR to a
    /// directory containing Level.sav (same corpus the parity harness uses).
    /// This is the ONE test in this suite that depends on host state outside
    /// the repo -- everything else (including `cargo test -p psp-server`
    /// with no env vars set) is fully self-contained.
    #[tokio::test]
    async fn load_server_save_loads_world_and_emits_summaries() {
        let Ok(source_save_dir) = std::env::var("PSP_TEST_SAVE_DIR") else {
            eprintln!("skipped: set PSP_TEST_SAVE_DIR to a directory containing Level.sav");
            return;
        };
        let mut env = TestEnv::new().await;
        // servers layout: <saves_path>/SaveGames/0/<world>/
        let saves_root = env._scratch.path().join("saves");
        let world_dir = saves_root.join("SaveGames").join("0").join("WORLD01");
        std::fs::create_dir_all(&world_dir).unwrap();
        for entry in std::fs::read_dir(&source_save_dir).unwrap().flatten() {
            let target = world_dir.join(entry.file_name());
            if entry.path().is_dir() {
                copy_dir_recursive(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
        let mut new_server = docker_new_server("world-host");
        new_server.saves_path = saves_root.to_string_lossy().to_string();
        let record = psp_db::servers::create_server(&env.app.db, new_server)
            .await
            .unwrap();
        let mut ctx = env.ctx();
        handle_load_server_save(
            ServerIdData {
                server_id: record.id,
            },
            &mut ctx,
        )
        .await
        .unwrap();
        let messages = env.drain();
        let types: Vec<&str> = messages
            .iter()
            .map(|message| message["type"].as_str().unwrap())
            .collect();
        // progress_message* then the three-response tail
        assert!(types.contains(&"loaded_save_files"));
        let tail: Vec<&str> = types.iter().rev().take(3).rev().copied().collect();
        assert_eq!(
            tail,
            vec![
                "loaded_save_files",
                "get_player_summaries",
                "get_guild_summaries"
            ]
        );
        let loaded = messages
            .iter()
            .find(|message| message["type"] == "loaded_save_files")
            .unwrap();
        assert_eq!(loaded["data"]["type"], "steam");
        assert_eq!(loaded["data"]["server_id"], record.id);
        assert_eq!(loaded["data"]["server_name"], record.name);
        assert!(loaded["data"]["has_gps"].is_boolean());
        assert!(env.session.save.is_some());
    }

    fn copy_dir_recursive(source: &std::path::Path, dest: &std::path::Path) {
        std::fs::create_dir_all(dest).unwrap();
        for entry in std::fs::read_dir(source).unwrap().flatten() {
            let target = dest.join(entry.file_name());
            if entry.path().is_dir() {
                copy_dir_recursive(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }
}
