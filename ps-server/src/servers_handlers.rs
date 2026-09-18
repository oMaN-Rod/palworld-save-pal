//! Server-management handlers.
//!
//! Error convention: a refusal a client can act on replies under the request's
//! own message type with an `error` object carrying `code` and `message`. A
//! failure with no such reply emits message type `error` with data
//! `{"message": "<text>"}`, and the handler returns `Ok(())`. Only
//! payload-parse failures take the `HandlerError` path.
use std::collections::HashSet;
use std::path::Path;

use serde_json::Value;

use ps_core::session::{SaveKind, SaveSession};
use ps_db::servers::{NewServer, ServerRecord};

use crate::dispatcher::HandlerCtx;
use crate::emitter::Emitter;
use crate::handler_error::HandlerError;
use crate::handlers::save_file;
use crate::messages::MessageType;
use crate::mods_handlers::emit_refusal;
use crate::services::mods::{deploy, settings, LibraryPaths};
use crate::services::{
    docker, native_config, native_process, ServerProcessStatus,
    ServerServices,
};

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
        "paks_path": record.paks_path,
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
        "env_vars": Value::Object(record.env_vars.clone()),
        "created_at": record.created_at,
        "updated_at": record.updated_at,
    })
}

pub fn server_public_wire_json(record: &ServerRecord) -> Value {
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
        "server_name": record.server_name,
        "server_description": record.server_description,
        "max_players": record.max_players,
        "created_at": record.created_at,
        "updated_at": record.updated_at,
    })
}

/// The FIRST world dir under saves/SaveGames/0 that has a Players dir wins;
/// later world dirs are not counted.
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

pub(crate) async fn server_status(
    services: &ServerServices,
    record: &ServerRecord,
) -> Option<ServerProcessStatus> {
    if record.server_type == "native" {
        Some(native_process::process_status(record.pid))
    } else {
        docker::container_status(services.docker.as_ref(), &record.container_name).await
    }
}

pub(crate) async fn online_player_count(
    services: &ServerServices,
    record: &ServerRecord,
    status: &Option<ServerProcessStatus>,
) -> u64 {
    if status
        .as_ref()
        .map(|current| current.running)
        .unwrap_or(false)
    {
        services
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

/// One inspect yields both the status and whether `start_server` would rebuild
/// the container, since the listing is polled.
async fn server_status_and_recreate_flag(
    services: &ServerServices,
    record: &ServerRecord,
) -> (Option<ServerProcessStatus>, bool) {
    if record.server_type == "native" {
        return (Some(native_process::process_status(record.pid)), false);
    }
    let inspected = services
        .docker
        .inspect_container(&record.container_name)
        .await;
    let needs_recreate =
        matches!(&inspected, Ok(Some(inspect)) if container_needs_recreate(record, inspect));
    (
        docker::status_from_inspect_result(&inspected),
        needs_recreate,
    )
}

async fn server_entry_with_runtime_fields(
    services: &ServerServices,
    record: &ServerRecord,
) -> Value {
    let (status, needs_recreate) = server_status_and_recreate_flag(services, record).await;
    let mut entry = server_to_wire_json(record);
    entry["status"] = serde_json::to_value(&status).expect("status serializes");
    entry["container_needs_recreate"] = Value::from(needs_recreate);
    entry["relocation_pending"] = Value::from(record.pending_relocation.is_some());
    entry["total_players"] = Value::from(count_total_players(&record.saves_path));
    entry["player_count"] = Value::from(online_player_count(services, record, &status).await);
    let version = server_version(services, record, &status).await;
    let latest_version = match version {
        Some(_) => services.latest_version.get().await,
        None => None,
    };
    entry["version"] = Value::from(version);
    entry["latest_version"] = Value::from(latest_version);
    entry
}

async fn server_version(
    services: &ServerServices,
    record: &ServerRecord,
    status: &Option<ServerProcessStatus>,
) -> Option<String> {
    if !status.as_ref().is_some_and(|current| current.running) {
        return None;
    }
    services
        .palworld_api
        .get_server_version(
            "127.0.0.1",
            record.rest_api_port as u16,
            &record.admin_password,
        )
        .await
}

/// Whether an apply already holds this server's target, in which case a polled
/// listing leaves the relocation to whoever is running it.
async fn relocation_apply_in_progress(
    db: &dyn ps_db::DbDriver,
    library: &LibraryPaths,
    record: &ServerRecord,
) -> bool {
    matches!(
        ps_db::mod_targets::get(db, &format!("server-{}", record.id)).await,
        Ok(Some(target)) if deploy::try_lock_target(library, &target).is_none()
    )
}

/// A server with a pending relocation gets an attempt to finish it before it is
/// listed, at most once per `RelocationRetries::INTERVAL` and without progress
/// frames, because the listing is polled. An attempt that cannot finish still
/// lists the server.
pub async fn handle_list_servers(
    services: &ServerServices,
    library: &LibraryPaths,
    _data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let db = &*ctx.app.driver;
    match ps_db::servers::list_servers(db).await {
        Ok(mut records) => {
            for record in records.iter_mut() {
                if record.pending_relocation.is_none()
                    || relocation_apply_in_progress(db, library, record).await
                {
                    continue;
                }
                let server_lock = services.server_locks.of(record.id);
                let Ok(_server_guard) = server_lock.try_lock() else {
                    continue;
                };
                if !services.relocation_retries.begin_attempt(record.id) {
                    continue;
                }
                let outcome = finish_relocation(
                    services,
                    library,
                    db,
                    ctx.emitter,
                    record.clone(),
                    true,
                    false,
                )
                .await;
                if let Some((code, message)) = &outcome.error {
                    tracing::warn!(server_id = record.id, code, %message, "a pending relocation did not finish");
                }
                *record = outcome.record;
            }
            let mut server_list = Vec::with_capacity(records.len());
            for record in &records {
                server_list.push(server_entry_with_runtime_fields(services, record).await);
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
    services: &ServerServices,
    data: ServerIdData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    match ps_db::servers::get_server(&*ctx.app.driver, data.server_id).await {
        Ok(Some(record)) => {
            let entry = server_entry_with_runtime_fields(services, &record).await;
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
    let workshop_dir = settings::find_steam_workshop_dir().unwrap_or_default();
    ctx.emitter.emit(
        MessageType::DetectWorkshopDir,
        &serde_json::json!({ "workshop_dir": workshop_dir }),
    );
    Ok(())
}

pub async fn handle_get_server_stats(
    services: &ServerServices,
    data: ServerIdData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    match ps_db::servers::get_server(&*ctx.app.driver, data.server_id).await {
        Ok(Some(record)) => {
            let stats = if record.server_type == "native" {
                native_process::process_stats(record.pid)
            } else {
                docker::container_stats(services.docker.as_ref(), &record.container_name).await
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
    "PalStudio Palworld Server".to_string()
}
fn default_admin_password() -> String {
    "admin".to_string()
}
fn default_max_players() -> i64 {
    16
}

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
    db: &dyn ps_db::DbDriver,
    server_id: i64,
    steamcmd_path: &str,
) -> Result<(), String> {
    let mut updates = serde_json::Map::new();
    updates.insert(
        "steamcmd_path".to_string(),
        Value::String(steamcmd_path.to_string()),
    );
    ps_db::servers::update_server(db, server_id, &updates)
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// Returns `Err(String)` only for failures the caller renders as
/// "Failed to create server: {e}"; every business rejection emits its own
/// `error` frame and returns `Ok(())`.
async fn create_server_impl(
    services: &ServerServices,
    data: CreateServerData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), String> {
    let emitter = ctx.emitter;
    let db = &*ctx.app.driver;
    let is_native = data.server_type == "native";

    let allocated = ps_db::servers::allocated_ports(db)
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
            workshop_dir = settings::find_steam_workshop_dir().unwrap_or_default();
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
            paks_path: native_config::paks_path(&data.install_path),
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
        let mut record = ps_db::servers::create_server(db, new_server)
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
            record = ps_db::servers::get_server(db, record.id)
                .await
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "server row vanished during creation".to_string())?;
            if record.steamcmd_path.is_empty() {
                persist_steamcmd_path(db, record.id, &steamcmd_path).await?;
                record = ps_db::servers::get_server(db, record.id)
                    .await
                    .map_err(|error| error.to_string())?
                    .ok_or_else(|| "server row vanished during creation".to_string())?;
            }
        }

        let created = native_process::create_native_server(&record, source_path.as_deref()).await;
        if !created {
            ps_db::servers::delete_server(db, record.id)
                .await
                .map_err(|error| error.to_string())?;
            emit_creation_progress(emitter, "");
            emit_business_error(
                emitter,
                "Failed to create native server installation".to_string(),
            );
            return Ok(());
        }

        if let Err(error) = crate::mod_target_service::ensure_for(db, &record, &services.app_root).await {
            tracing::error!(%error, server_id = record.id, "mod target creation failed");
        }

        emit_creation_progress(emitter, "Writing server configuration files...");
        let mut warnings: Vec<String> = Vec::new();
        if let Err(error) = settings::ensure_mod_settings(&record) {
            tracing::warn!(%error, server_id = record.id, "mod settings were not prepared");
            warnings.push(error.to_string());
        }
        emit_creation_progress(emitter, "");

        let mut result = server_to_wire_json(&record);
        result["status"] =
            serde_json::to_value(native_process::process_status(record.pid)).expect("serializes");
        result["player_count"] = Value::from(0);
        if !warnings.is_empty() {
            result["warnings"] = Value::from(warnings);
        }
        emitter.emit(MessageType::CreateServer, &result);
    } else {
        emit_creation_progress(emitter, "Validating server configuration...");
        let base_path = docker_server_dir(&services.app_root, &data.container_name);
        let new_server = NewServer {
            name: data.name.clone(),
            container_name: data.container_name.clone(),
            image_name: data.image_name.clone(),
            server_type: "docker".to_string(),
            game_port: data.game_port,
            query_port: data.query_port,
            rest_api_port: data.rest_api_port,
            data_volume_name: format!("ps-{}-data", data.container_name),
            saves_path: base_path.join("saves").to_string_lossy().to_string(),
            mods_path: base_path.join("mods").to_string_lossy().to_string(),
            logicmods_path: base_path.join("logicmods").to_string_lossy().to_string(),
            nativemods_path: base_path.join("nativemods").to_string_lossy().to_string(),
            paks_path: base_path.join("paks").to_string_lossy().to_string(),
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
        let record = ps_db::servers::create_server(db, new_server)
            .await
            .map_err(|error| error.to_string())?;

        emit_creation_progress(
            emitter,
            &format!("Pulling Docker image {}...", data.image_name),
        );
        docker::create_server_container(services.docker.as_ref(), &record)
            .await
            .map_err(|error| error.to_string())?;
        emit_creation_progress(emitter, "Container started successfully");
        emit_creation_progress(emitter, "");

        if let Err(error) = crate::mod_target_service::ensure_for(db, &record, &services.app_root).await {
            tracing::error!(%error, server_id = record.id, "mod target creation failed");
        }

        let status =
            docker::container_status(services.docker.as_ref(), &record.container_name).await;
        let mut result = server_to_wire_json(&record);
        result["status"] = serde_json::to_value(&status).expect("serializes");
        result["player_count"] = Value::from(0);
        emitter.emit(MessageType::CreateServer, &result);
    }
    Ok(())
}

pub async fn handle_create_server(
    services: &ServerServices,
    data: CreateServerData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if let Err(message) = create_server_impl(services, data, ctx).await {
        emit_business_error(ctx.emitter, format!("Failed to create server: {message}"));
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct ImportServerData {
    pub install_path: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub query_port: Option<i64>,
    #[serde(default)]
    pub launch_args: Option<String>,
    #[serde(default)]
    pub workshop_dir: Option<String>,
}

fn assign_port(label: &str, wanted: i64, taken: &mut HashSet<u16>, notes: &mut Vec<String>) -> i64 {
    let mut got = wanted;
    while taken.contains(&(got as u16)) {
        got += 1;
    }
    taken.insert(got as u16);
    if got != wanted {
        notes.push(format!(
            "{label} port {wanted} was already in use; assigned {got} instead."
        ));
    }
    got
}

fn reassign_import_ports(
    game: i64,
    query: i64,
    rest: i64,
    allocated: &HashSet<u16>,
) -> ((i64, i64, i64), Vec<String>) {
    let mut taken = allocated.clone();
    let mut notes = Vec::new();
    let game_new = assign_port("Game", game, &mut taken, &mut notes);
    let query_new = assign_port("Query", query, &mut taken, &mut notes);
    let rest_new = assign_port("REST API", rest, &mut taken, &mut notes);
    ((game_new, query_new, rest_new), notes)
}

fn import_slug(name: &str) -> String {
    let mut slug = String::new();
    let mut prev_dash = false;
    for ch in name.to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            prev_dash = false;
        } else if !slug.is_empty() && !prev_dash {
            slug.push('-');
            prev_dash = true;
        }
    }
    let trimmed = slug.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "imported-server".to_string()
    } else {
        trimmed
    }
}

async fn import_server_impl(
    services: &ServerServices,
    data: ImportServerData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), String> {
    let emitter = ctx.emitter;
    let db = &*ctx.app.driver;

    let install_path = if data.install_path == "__select__" {
        if !ctx.app.config.desktop_mode {
            emit_business_error(emitter, "Desktop mode is required to browse for a folder".to_string());
            return Ok(());
        }
        match ctx.app.dialogs.pick_folder(None).await {
            Some(path) => path.to_string_lossy().into_owned(),
            None => return Ok(()), // user canceled — no error frame
        }
    } else {
        data.install_path.clone()
    };

    if !Path::new(&install_path).join("PalServer.exe").exists() {
        emit_business_error(emitter, "PalServer.exe not found in the selected folder".to_string());
        return Ok(());
    }

    if ps_db::servers::server_with_install_path(db, &install_path)
        .await
        .map_err(|error| error.to_string())?
        .is_some()
    {
        emit_business_error(emitter, "This server is already registered".to_string());
        return Ok(());
    }

    let config = native_config::parse_server_config_from_ini(&install_path);

    // Ports are reassigned in the DB only, not written back to the ini.
    let allocated = ps_db::servers::allocated_ports(db)
        .await
        .map_err(|error| error.to_string())?;
    let query_port = data.query_port.unwrap_or(27015);
    let ((game_port, query_port, rest_api_port), notifications) =
        reassign_import_ports(config.game_port, query_port, config.rest_api_port, &allocated);

    let steamcmd_path = native_process::find_steamcmd().unwrap_or_default();
    let mut workshop_dir = data.workshop_dir.clone().unwrap_or_default();
    if workshop_dir.is_empty() {
        workshop_dir = settings::find_steam_workshop_dir().unwrap_or_default();
    }

    // Display name: user value, else parsed ServerName, else folder basename.
    let name = if data.name.trim().is_empty() {
        if !config.server_name.is_empty() {
            config.server_name.clone()
        } else {
            Path::new(&install_path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Imported Server".to_string())
        }
    } else {
        data.name.trim().to_string()
    };

    let new_server = NewServer {
        name: name.clone(),
        container_name: import_slug(&name),
        image_name: String::new(),
        server_type: "native".to_string(),
        game_port,
        query_port,
        rest_api_port,
        data_volume_name: String::new(),
        saves_path: native_config::saves_path(&install_path),
        mods_path: native_config::mods_path_for_import(&install_path),
        logicmods_path: native_config::logicmods_path(&install_path),
        nativemods_path: native_config::nativemods_path(&install_path),
        paks_path: native_config::paks_path(&install_path),
        install_path: install_path.clone(),
        steamcmd_path,
        launch_args: data.launch_args.clone().unwrap_or_default(),
        workshop_dir,
        server_name: config.server_name,
        server_description: config.server_description,
        server_password: config.server_password,
        admin_password: config.admin_password,
        max_players: config.max_players,
        env_vars: config.env_vars,
    };

    let record = ps_db::servers::create_server(db, new_server)
        .await
        .map_err(|error| error.to_string())?;

    if let Err(error) = crate::mod_target_service::ensure_for(db, &record, &services.app_root).await {
        tracing::error!(%error, server_id = record.id, "mod target creation failed");
    }

    let mut result = server_to_wire_json(&record);
    result["status"] =
        serde_json::to_value(native_process::process_status(record.pid)).expect("serializes");
    result["player_count"] = Value::from(0);
    result["total_players"] = Value::from(count_total_players(&record.saves_path));
    result["notifications"] = Value::from(notifications);
    emitter.emit(MessageType::ImportServer, &result);
    Ok(())
}

pub async fn handle_import_server(
    services: &ServerServices,
    data: ImportServerData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if let Err(message) = import_server_impl(services, data, ctx).await {
        emit_business_error(ctx.emitter, format!("Failed to import server: {message}"));
    }
    Ok(())
}

/// A running server is restarted under its target's apply lock, so no apply
/// lands between the stop and the start. While an apply holds the lock the
/// server is left running and the refusal is returned with the record.
async fn apply_native_runtime_change(
    services: &ServerServices,
    library: &LibraryPaths,
    db: &dyn ps_db::DbDriver,
    mut record: ServerRecord,
) -> Result<(ServerRecord, Option<RefusalParts>), String> {
    native_config::write_palworld_settings(&record).map_err(|error| error.to_string())?;
    if !native_process::process_status(record.pid).running {
        return Ok((record, None));
    }
    let _apply_guard = match lock_server_target(library, db, record.id).await {
        Ok(guard) => guard,
        Err(refusal) => return Ok((record, Some(refusal))),
    };
    native_process::stop_server_process(&record, &services.palworld_api).await;
    if let Ok(new_pid) = native_process::start_server_process(&record) {
        let mut pid_update = serde_json::Map::new();
        pid_update.insert("pid".to_string(), Value::from(new_pid));
        if let Some(refreshed) = ps_db::servers::update_server(db, record.id, &pid_update)
            .await
            .map_err(|error| error.to_string())?
        {
            record = refreshed;
        }
    }
    Ok((record, None))
}

const MOD_PATH_COLUMNS: [&str; 4] = ["mods_path", "logicmods_path", "nativemods_path", "paks_path"];

fn relocating_columns(record: &ServerRecord) -> Vec<&'static str> {
    let mut columns = MOD_PATH_COLUMNS.to_vec();
    if record.server_type == "native" {
        columns.push("install_path");
    }
    columns
}

fn path_column<'a>(record: &'a ServerRecord, column: &str) -> &'a str {
    match column {
        "mods_path" => &record.mods_path,
        "logicmods_path" => &record.logicmods_path,
        "nativemods_path" => &record.nativemods_path,
        "paks_path" => &record.paks_path,
        "install_path" => &record.install_path,
        _ => "",
    }
}

fn same_host_path(left: &str, right: &str) -> bool {
    let case_insensitive = cfg!(any(windows, target_os = "macos"));
    ps_core::mods::normalize_physical_path(left, case_insensitive)
        == ps_core::mods::normalize_physical_path(right, case_insensitive)
}

/// Whether `updates` moves any directory the server's managed mods live under.
/// A path merely spelled differently is not a move.
fn relocation_changes(record: &ServerRecord, updates: &serde_json::Map<String, Value>) -> bool {
    relocating_columns(record).into_iter().any(|column| {
        updates
            .get(column)
            .and_then(Value::as_str)
            .is_some_and(|new_path| !same_host_path(path_column(record, column), new_path))
    })
}

/// `path` re-rooted from `old_base` onto `new_base`, or `None` when it does not
/// sit under `old_base`.
fn rebased(path: &str, old_base: &str, new_base: &str) -> Option<String> {
    use std::path::Component;
    let lexical = |p: &Path| {
        p.components()
            .all(|c| !matches!(c, Component::ParentDir | Component::CurDir))
    };
    let (path, old_base) = (Path::new(path), Path::new(old_base));
    if old_base.as_os_str().is_empty() || !lexical(path) || !lexical(old_base) {
        return None;
    }
    let key = |component: Component| {
        ps_core::mods::normalize_physical_path(
            &component.as_os_str().to_string_lossy(),
            cfg!(any(windows, target_os = "macos")),
        )
    };
    let mut rest = path.components();
    for base in old_base.components() {
        if key(rest.next()?) != key(base) {
            return None;
        }
    }
    Some(
        Path::new(new_base)
            .join(rest.as_path())
            .to_string_lossy()
            .into_owned(),
    )
}

/// A native server's mod columns under its install move with it, unless the
/// update sets them itself.
fn with_mod_paths_following_install(
    record: &ServerRecord,
    updates: &serde_json::Map<String, Value>,
) -> serde_json::Map<String, Value> {
    let mut updates = updates.clone();
    let Some(new_install) = updates
        .get("install_path")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        return updates;
    };
    if record.server_type != "native" || same_host_path(&record.install_path, &new_install) {
        return updates;
    }
    for column in MOD_PATH_COLUMNS {
        if updates.contains_key(column) {
            continue;
        }
        if let Some(moved) = rebased(path_column(record, column), &record.install_path, &new_install)
        {
            updates.insert(column.to_string(), Value::from(moved));
        }
    }
    updates
}

/// The single map that commits the new paths and the relocation flag together,
/// so no state exists in which the paths changed without the flag.
fn relocation_updates(
    record: &ServerRecord,
    updates: &serde_json::Map<String, Value>,
    was_running: bool,
) -> serde_json::Map<String, Value> {
    let updates = &with_mod_paths_following_install(record, updates);
    let mut from = serde_json::Map::new();
    let mut to = serde_json::Map::new();
    for column in relocating_columns(record) {
        let old_path = path_column(record, column);
        from.insert(column.to_string(), Value::from(old_path));
        let new_path = updates
            .get(column)
            .and_then(Value::as_str)
            .unwrap_or(old_path);
        to.insert(column.to_string(), Value::from(new_path));
    }
    let mut committed = updates.clone();
    committed.insert(
        "pending_relocation".to_string(),
        Value::from(
            serde_json::json!({
                "op": "relocate",
                "was_running": was_running,
                "from": from,
                "to": to,
            })
            .to_string(),
        ),
    );
    committed
}

/// A flag this code did not write reads as a relocation to the record's current
/// columns that restarts nothing. The apply never consults `from` or `to`, so
/// finishing it moves nothing a plain apply would not.
fn pending_relocation_was_running(server_id: i64, flag: &str) -> bool {
    match serde_json::from_str::<Value>(flag) {
        Err(error) => {
            tracing::warn!(server_id, raw = flag, %error, "pending_relocation does not parse; finishing it without a restart");
            false
        }
        Ok(parsed) => match (
            parsed.get("op").and_then(Value::as_str),
            parsed.get("was_running").and_then(Value::as_bool),
        ) {
            (Some("relocate"), Some(was_running)) => was_running,
            _ => {
                tracing::warn!(server_id, raw = flag, "pending_relocation is not a relocate record; finishing it without a restart");
                false
            }
        },
    }
}

/// `flag` with `was_running` replaced, keeping whatever else it records.
fn pending_relocation_with(flag: &str, was_running: bool) -> String {
    let mut parsed = serde_json::from_str::<Value>(flag)
        .ok()
        .filter(Value::is_object)
        .unwrap_or_else(|| serde_json::json!({}));
    parsed["op"] = Value::from("relocate");
    parsed["was_running"] = Value::from(was_running);
    parsed.to_string()
}

fn apply_closed_cleanly(apply: &Value) -> bool {
    apply.is_null() || (apply.get("mid_apply") == Some(&Value::Bool(false)) && apply.get("error").is_none())
}

struct RelocationOutcome {
    record: ServerRecord,
    /// The `profile_apply` reply object, or null when the server has no target.
    apply: Value,
    /// Whether the flag is now clear.
    closed: bool,
    error: Option<(&'static str, String)>,
    /// The apply closed, but creating or starting the server afterwards failed.
    server_setup_failed: bool,
}

/// Re-enterable from any point after the flag was committed: the overrides are
/// rewritten from the record again, because a crash may have come before they
/// were. The flag is cleared only after a clean apply and before the container
/// is created; if creating or starting then fails, the flag is written back so
/// `was_running` survives and the next attempt, whose apply is a no-op, retries.
/// The target's apply lock is held from before the apply until the restart
/// returns, so no other apply can write to the target in between.
async fn finish_relocation(
    services: &ServerServices,
    library: &LibraryPaths,
    db: &dyn ps_db::DbDriver,
    emitter: &Emitter,
    record: ServerRecord,
    start_if_was_running: bool,
    report_progress: bool,
) -> RelocationOutcome {
    let Some(flag) = record.pending_relocation.clone() else {
        return RelocationOutcome {
            record,
            apply: Value::Null,
            closed: true,
            error: None,
            server_setup_failed: false,
        };
    };
    let was_running = pending_relocation_was_running(record.id, &flag);
    let not_closed = |record, apply, code, message: String| RelocationOutcome {
        record,
        apply,
        closed: false,
        error: Some((code, message)),
        server_setup_failed: false,
    };
    if let Err(error) = ps_db::mod_targets::rewrite_server_overrides(db, &record).await {
        return not_closed(record, Value::Null, "db", error.to_string());
    }
    let target = match ps_db::mod_targets::get(db, &format!("server-{}", record.id)).await {
        Ok(target) => target,
        Err(error) => return not_closed(record, Value::Null, "db", error.to_string()),
    };
    let apply_guard = target
        .as_ref()
        .and_then(|target| deploy::try_lock_target(library, target));
    let apply = match &target {
        Some(target) => {
            crate::mods_handlers::run_apply_locked(
                services,
                library,
                db,
                emitter,
                target,
                report_progress,
                apply_guard.as_ref(),
            )
            .await
        }
        None => Value::Null,
    };
    if !apply_closed_cleanly(&apply) {
        return RelocationOutcome {
            record,
            apply,
            closed: false,
            error: None,
            server_setup_failed: false,
        };
    }

    let mut clear = serde_json::Map::new();
    clear.insert("pending_relocation".to_string(), Value::Null);
    let record = match ps_db::servers::update_server(db, record.id, &clear).await {
        Ok(Some(cleared)) => cleared,
        Ok(None) => {
            return not_closed(record, apply, "not_found", "Server not found".to_string())
        }
        Err(error) => return not_closed(record, apply, "db", error.to_string()),
    };
    let mut outcome = RelocationOutcome {
        record,
        apply,
        closed: true,
        error: None,
        server_setup_failed: false,
    };
    let start = start_if_was_running && was_running;
    let failure = if outcome.record.server_type == "native" {
        match start.then(|| native_process::start_server_process(&outcome.record)) {
            None => None,
            Some(Ok(pid)) => {
                let mut pid_update = serde_json::Map::new();
                pid_update.insert("pid".to_string(), Value::from(pid));
                match ps_db::servers::update_server(db, outcome.record.id, &pid_update).await {
                    Ok(Some(started)) => outcome.record = started,
                    Ok(None) => {}
                    Err(error) => outcome.error = Some(("db", error.to_string())),
                }
                None
            }
            Some(Err(failure)) => Some((failure.code, failure.message)),
        }
    } else {
        let docker_api = services.docker.as_ref();
        let container_name = outcome.record.container_name.clone();
        // A container step 1 failed to remove still carries the old binds.
        if matches!(
            docker_api.inspect_container(&container_name).await,
            Ok(Some(_))
        ) {
            docker::stop_server_container(docker_api, &container_name).await;
            docker::remove_server_container(docker_api, &container_name, None).await;
        }
        match docker::create_server_container_stopped(docker_api, &outcome.record).await {
            Err(error) => Some(("container_create_failed", error.to_string())),
            Ok(_) if start && !docker::start_server_container(docker_api, &container_name).await => {
                Some((
                    "container_start_failed",
                    format!("container {container_name} did not start"),
                ))
            }
            Ok(_) => None,
        }
    };
    let Some((code, message)) = failure else {
        return outcome;
    };
    let mut restore = serde_json::Map::new();
    restore.insert(
        "pending_relocation".to_string(),
        Value::from(pending_relocation_with(&flag, was_running)),
    );
    // Not closed even when the write-back fails, so a start reports this failure
    // rather than carrying on without a container.
    outcome.closed = false;
    match ps_db::servers::update_server(db, outcome.record.id, &restore).await {
        Ok(Some(restored)) => outcome.record = restored,
        Ok(None) => {}
        Err(error) => {
            tracing::error!(server_id = outcome.record.id, %error, code, "could not keep the relocation pending after a failure")
        }
    }
    outcome.error = Some((code, message));
    outcome.server_setup_failed = true;
    outcome
}

/// Stop, commit the paths with the flag, apply, and only on a clean close
/// recreate the container.
async fn relocate_server(
    services: &ServerServices,
    library: &LibraryPaths,
    old_record: ServerRecord,
    updates: &serde_json::Map<String, Value>,
    write_native_settings: bool,
    ctx: &mut HandlerCtx<'_>,
) {
    let context = serde_json::json!({ "server_id": old_record.id });
    let Some(status) = server_status(services, &old_record).await else {
        emit_refusal(
            ctx,
            MessageType::UpdateServer,
            context,
            "server_state_unknown",
            "the server's running state could not be read, so none of the requested changes \
             were applied"
                .to_string(),
            serde_json::json!({}),
        );
        return;
    };
    let running = status.running;
    let was_running = running
        || old_record
            .pending_relocation
            .as_deref()
            .is_some_and(|flag| pending_relocation_was_running(old_record.id, flag));
    let mut committed = relocation_updates(&old_record, updates, was_running);
    if old_record.server_type == "native" {
        if running && native_process::stop_server_process(&old_record, &services.palworld_api).await
        {
            committed.insert("pid".to_string(), Value::Null);
        }
    } else {
        let docker_api = services.docker.as_ref();
        docker::stop_server_container(docker_api, &old_record.container_name).await;
        docker::remove_server_container(docker_api, &old_record.container_name, None).await;
    }

    let record =
        match ps_db::servers::update_server(&*ctx.app.driver, old_record.id, &committed).await {
            Ok(Some(record)) => record,
            Ok(None) => {
                emit_refusal(
                    ctx,
                    MessageType::UpdateServer,
                    context,
                    "not_found",
                    "Server not found".to_string(),
                    serde_json::json!({}),
                );
                return;
            }
            Err(error) => {
                emit_refusal(
                    ctx,
                    MessageType::UpdateServer,
                    context,
                    "db",
                    error.to_string(),
                    serde_json::json!({}),
                );
                return;
            }
        };
    let settings_error = if record.server_type == "native" && write_native_settings {
        native_config::write_palworld_settings(&record)
            .err()
            .map(|error| ("settings_write_failed", error.to_string()))
    } else {
        None
    };

    let outcome = finish_relocation(
        services,
        library,
        &*ctx.app.driver,
        ctx.emitter,
        record,
        true,
        true,
    )
    .await;
    let status = server_status(services, &outcome.record).await;
    let mut reply = server_to_wire_json(&outcome.record);
    reply["status"] = serde_json::to_value(&status).expect("serializes");
    reply["relocation_pending"] = Value::from(!outcome.closed);
    reply["apply"] = outcome.apply;
    match outcome.error.or(settings_error) {
        Some((code, message)) => emit_refusal(
            ctx,
            MessageType::UpdateServer,
            reply,
            code,
            message,
            serde_json::json!({}),
        ),
        None => ctx.emitter.emit(MessageType::UpdateServer, &reply),
    }
}

async fn update_server_impl(
    services: &ServerServices,
    library: &LibraryPaths,
    data: UpdateServerData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), String> {
    let server_lock = services.server_locks.of(data.server_id);
    let _server_guard = server_lock.lock().await;
    let emitter = ctx.emitter;
    let db = &*ctx.app.driver;
    let Some(old_record) = ps_db::servers::get_server(db, data.server_id)
        .await
        .map_err(|error| error.to_string())?
    else {
        emit_business_error(emitter, "Server not found".to_string());
        return Ok(());
    };
    let mut updates = data.updates;
    updates.remove("pending_relocation");

    let env_changed = updates
        .get("env_vars")
        .map(|value| !value.is_null())
        .unwrap_or(false);
    let ports_changed = ["game_port", "query_port", "rest_api_port"]
        .iter()
        .any(|key| updates.contains_key(*key));
    let identity_changed = [
        "server_name",
        "server_description",
        "server_password",
        "admin_password",
        "max_players",
    ]
    .iter()
    .any(|key| updates.contains_key(*key));
    let needs_apply = env_changed || ports_changed || identity_changed;
    // Only consumed when spawning PalServer.exe, so they matter to native only.
    let native_runtime_changed = ["launch_args", "workshop_dir"]
        .iter()
        .any(|key| updates.contains_key(*key));

    if relocation_changes(&old_record, &updates) {
        let write_native_settings = needs_apply || native_runtime_changed;
        relocate_server(
            services,
            library,
            old_record,
            &updates,
            write_native_settings,
            ctx,
        )
        .await;
        return Ok(());
    }

    let Some(mut record) = ps_db::servers::update_server(db, data.server_id, &updates)
        .await
        .map_err(|error| error.to_string())?
    else {
        emit_business_error(emitter, "Failed to update server".to_string());
        return Ok(());
    };

    let mut refusal = None;
    let relocation_pending = record.pending_relocation.is_some();
    if relocation_pending {
        // The relocation's close creates the container from every current setting.
        if record.server_type == "native" && (needs_apply || native_runtime_changed) {
            native_config::write_palworld_settings(&record).map_err(|error| error.to_string())?;
        }
    } else if record.server_type == "native" {
        if needs_apply || native_runtime_changed {
            (record, refusal) =
                apply_native_runtime_change(services, library, db, record).await?;
        }
    } else if needs_apply {
        match lock_server_target(library, db, record.id).await {
            Err(parts) => refusal = Some(parts),
            Ok(_apply_guard) => {
                let docker_api = services.docker.as_ref();
                docker::stop_server_container(docker_api, &old_record.container_name).await;
                docker::remove_server_container(docker_api, &old_record.container_name, None)
                    .await;
                docker::create_server_container(docker_api, &record)
                    .await
                    .map_err(|error| error.to_string())?;
            }
        }
    }

    let status = server_status(services, &record).await;
    let mut result = server_to_wire_json(&record);
    result["status"] = serde_json::to_value(&status).expect("serializes");
    if relocation_pending {
        result["relocation_pending"] = Value::from(true);
    }
    match refusal {
        Some((code, message, detail)) => {
            emit_refusal(ctx, MessageType::UpdateServer, result, code, message, detail)
        }
        None => emitter.emit(MessageType::UpdateServer, &result),
    }
    Ok(())
}

pub async fn handle_update_server(
    services: &ServerServices,
    library: &LibraryPaths,
    data: UpdateServerData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if let Err(message) = update_server_impl(services, library, data, ctx).await {
        emit_business_error(ctx.emitter, format!("Failed to update server: {message}"));
    }
    Ok(())
}

const GAMEDATA_LAUNCH_ARG: &str = "-enable-gamedata-api";

fn has_launch_arg(launch_args: &str, arg: &str) -> bool {
    launch_args
        .split_whitespace()
        .any(|existing| existing.eq_ignore_ascii_case(arg))
}

fn with_launch_arg_appended(launch_args: &str, arg: &str) -> String {
    if launch_args.trim().is_empty() {
        arg.to_string()
    } else {
        format!("{launch_args} {arg}")
    }
}

fn refuse_gamedata_launch_arg(emitter: &Emitter, message: impl Into<String>) {
    emitter.emit(
        MessageType::EnsureGamedataLaunchArg,
        &serde_json::json!({ "error": message.into() }),
    );
}

async fn ensure_gamedata_launch_arg_impl(
    services: &ServerServices,
    library: &LibraryPaths,
    data: ServerIdData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), String> {
    let emitter = ctx.emitter;
    let db = &*ctx.app.driver;
    let Some(record) = ps_db::servers::get_server(db, data.server_id)
        .await
        .map_err(|error| error.to_string())?
    else {
        refuse_gamedata_launch_arg(emitter, "Server not found");
        return Ok(());
    };
    if record.server_type != "native" {
        refuse_gamedata_launch_arg(
            emitter,
            "Launch arguments are not supported for container servers yet",
        );
        return Ok(());
    }

    let (record, refusal) = if has_launch_arg(&record.launch_args, GAMEDATA_LAUNCH_ARG) {
        (record, None)
    } else {
        let mut updates = serde_json::Map::new();
        updates.insert(
            "launch_args".to_string(),
            Value::from(with_launch_arg_appended(
                &record.launch_args,
                GAMEDATA_LAUNCH_ARG,
            )),
        );
        let Some(updated) = ps_db::servers::update_server(db, record.id, &updates)
            .await
            .map_err(|error| error.to_string())?
        else {
            refuse_gamedata_launch_arg(emitter, "Failed to update server");
            return Ok(());
        };
        apply_native_runtime_change(services, library, db, updated).await?
    };

    let status = server_status(services, &record).await;
    let mut result = server_to_wire_json(&record);
    result["status"] = serde_json::to_value(&status).expect("serializes");
    match refusal {
        Some((code, message, detail)) => emit_refusal(
            ctx,
            MessageType::EnsureGamedataLaunchArg,
            result,
            code,
            message,
            detail,
        ),
        None => emitter.emit(MessageType::EnsureGamedataLaunchArg, &result),
    }
    Ok(())
}

pub async fn handle_ensure_gamedata_launch_arg(
    services: &ServerServices,
    library: &LibraryPaths,
    data: ServerIdData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if let Err(message) = ensure_gamedata_launch_arg_impl(services, library, data, ctx).await {
        emit_business_error(
            ctx.emitter,
            format!("Failed to enable the world data endpoint: {message}"),
        );
    }
    Ok(())
}

pub async fn handle_delete_server(
    services: &ServerServices,
    library: &LibraryPaths,
    data: ServerIdData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let server_lock = services.server_locks.of(data.server_id);
    let _server_guard = server_lock.lock().await;
    let _apply_guard = match lock_server_target(library, &*ctx.app.driver, data.server_id).await {
        Ok(guard) => guard,
        Err((code, message, detail)) => {
            emit_refusal(
                ctx,
                MessageType::DeleteServer,
                serde_json::json!({ "server_id": data.server_id }),
                code,
                message,
                detail,
            );
            return Ok(());
        }
    };
    let emitter = ctx.emitter;
    let db = &*ctx.app.driver;
    let result: Result<(), String> = async {
        let Some(record) = ps_db::servers::get_server(db, data.server_id)
            .await
            .map_err(|error| error.to_string())?
        else {
            emit_business_error(emitter, "Server not found".to_string());
            return Ok(());
        };
        if record.server_type == "native" {
            if record.pid.is_some() {
                native_process::stop_server_process(&record, &services.palworld_api).await;
            }
            // Native installs keep their files on disk; only the DB row goes.
        } else {
            let docker_api = services.docker.as_ref();
            docker::stop_server_container(docker_api, &record.container_name).await;
            // Removal result is deliberately ignored: a Docker-side failure must
            // not block deleting the DB row or change the response.
            docker::remove_server_container(
                docker_api,
                &record.container_name,
                Some(&record.data_volume_name),
            )
            .await;
        }
        if let Err(error) = crate::mod_target_service::forget(db, record.id).await {
            tracing::error!(%error, server_id = record.id, "mod target removal failed");
        }
        ps_db::servers::delete_server(db, record.id)
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

fn docker_server_dir(app_root: &Path, container_name: &str) -> std::path::PathBuf {
    app_root.join("servers").join(container_name)
}

/// A Docker server created before `paks_path` existed has an empty one, which
/// `start_server` backfills with this.
fn effective_docker_paks_path(app_root: &Path, record: &ServerRecord) -> String {
    if record.paks_path.is_empty() {
        docker_server_dir(app_root, &record.container_name)
            .join("paks")
            .to_string_lossy()
            .into_owned()
    } else {
        record.paks_path.clone()
    }
}

/// Whether `start_server` will rebuild this existing, inspected container
/// because it predates the `~mods` bind; a server without a `paks_path` is
/// backfilled first, so it counts too. A pending relocation owns the
/// container's lifecycle, so it never counts.
fn container_needs_recreate(record: &ServerRecord, inspect: &Value) -> bool {
    record.server_type != "native"
        && record.pending_relocation.is_none()
        && !docker::mounts_paks_mods(inspect)
}

fn overrides_carry_paks_mods_dir(layout_overrides: &str) -> bool {
    serde_json::from_str::<Value>(layout_overrides)
        .ok()
        .and_then(|overrides| {
            overrides
                .get("paks_mods_dir")
                .and_then(Value::as_str)
                .map(|dir| !dir.is_empty())
        })
        .unwrap_or(false)
}

/// Gives a server an effective `paks_path` and its target a matching
/// `paks_mods_dir`. The overrides are written first, and a target still lacking
/// `paks_mods_dir` re-triggers this, so a failure between the two writes heals
/// on the next start.
async fn backfill_docker_paks_path(
    app_root: &Path,
    db: &dyn ps_db::DbDriver,
    record: &mut ServerRecord,
) -> Result<(), String> {
    let target_id = format!("server-{}", record.id);
    let target = ps_db::mod_targets::get(db, &target_id)
        .await
        .map_err(|error| error.to_string())?;
    let target_lacks_paks = target
        .as_ref()
        .is_some_and(|target| !overrides_carry_paks_mods_dir(&target.layout_overrides));
    if !record.paks_path.is_empty() && !target_lacks_paks {
        return Ok(());
    }
    let mut backfilled = record.clone();
    backfilled.paks_path = effective_docker_paks_path(app_root, record);
    if target.is_some() {
        ps_db::mod_targets::set_layout_overrides(
            db,
            &target_id,
            &ps_db::mod_targets::server_layout_overrides(&backfilled),
        )
        .await
        .map_err(|error| error.to_string())?;
    }
    if record.paks_path.is_empty() {
        let mut updates = serde_json::Map::new();
        updates.insert(
            "paks_path".to_string(),
            Value::from(backfilled.paks_path.clone()),
        );
        if let Some(updated) = ps_db::servers::update_server(db, record.id, &updates)
            .await
            .map_err(|error| error.to_string())?
        {
            *record = updated;
        }
    }
    Ok(())
}

/// Brings the container in line with the record before a start. A container
/// predating the `~mods` bind is rebuilt; no host path changes, so no mod files
/// move. The image pull and directory creation run before the old container is
/// removed, so a failure there leaves it in place. A missing container is
/// created from the record unless a relocation is pending, and one Docker
/// cannot inspect is left alone.
async fn prepare_container_for_start(
    docker_api: &dyn docker::DockerApi,
    record: &ServerRecord,
) -> Result<(), String> {
    match docker_api.inspect_container(&record.container_name).await {
        Ok(Some(inspect)) if container_needs_recreate(record, &inspect) => {
            docker::prepare_server_container(docker_api, record)
                .await
                .map_err(|error| error.to_string())?;
            docker::stop_server_container(docker_api, &record.container_name).await;
            docker::remove_server_container(docker_api, &record.container_name, None).await;
            docker_api
                .create_and_start_container(docker::container_spec(record))
                .await
                .map_err(|error| error.to_string())?;
        }
        Ok(None) if record.pending_relocation.is_none() => {
            docker::create_server_container(docker_api, record)
                .await
                .map_err(|error| error.to_string())?;
        }
        _ => docker::create_bind_host_dirs(record).map_err(|error| error.to_string())?,
    }
    Ok(())
}

/// Re-enters a pending relocation; while it cannot finish, refuses the start
/// under `server_status_update` and returns false. A lookup failure returns true
/// so the start reports it as before.
async fn relocation_allows_start(
    services: &ServerServices,
    library: &LibraryPaths,
    server_id: i64,
    ctx: &mut HandlerCtx<'_>,
) -> bool {
    let record = match ps_db::servers::get_server(&*ctx.app.driver, server_id).await {
        Ok(Some(record)) if record.pending_relocation.is_some() => record,
        _ => return true,
    };
    let outcome = finish_relocation(
        services,
        library,
        &*ctx.app.driver,
        ctx.emitter,
        record,
        false,
        true,
    )
    .await;
    if outcome.closed {
        return true;
    }
    let status = server_status(services, &outcome.record).await;
    let mut detail = serde_json::json!({ "apply": outcome.apply });
    let (code, message) = match outcome.error {
        Some(failure) if outcome.server_setup_failed => failure,
        cause => {
            if let Some((code, message)) = cause {
                detail["cause"] = serde_json::json!({ "code": code, "message": message });
            }
            (
                "relocation_pending",
                "the server's mod folders have not finished moving".to_string(),
            )
        }
    };
    emit_refusal(
        ctx,
        MessageType::ServerStatusUpdate,
        serde_json::json!({ "server_id": server_id, "status": status, "success": false }),
        code,
        message,
        detail,
    );
    false
}

type RefusalParts = (&'static str, String, Value);

/// The apply lock of the server's mod target, or `None` when the server has no
/// target.
async fn lock_server_target(
    library: &LibraryPaths,
    db: &dyn ps_db::DbDriver,
    server_id: i64,
) -> Result<Option<tokio::sync::OwnedMutexGuard<()>>, RefusalParts> {
    let target_id = format!("server-{server_id}");
    match ps_db::mod_targets::get(db, &target_id).await {
        Err(error) => Err(("db", error.to_string(), serde_json::json!({}))),
        Ok(None) => Ok(None),
        Ok(Some(target)) => deploy::try_lock_target(library, &target)
            .map(Some)
            .ok_or_else(|| {
                (
                    "apply_in_progress",
                    format!("an apply is running on {target_id}"),
                    serde_json::json!({ "target_id": target_id }),
                )
            }),
    }
}

/// Held until the server has started, so no apply runs while it starts. `None`
/// means the start was refused and the reply sent.
async fn start_apply_guard(
    services: &ServerServices,
    library: &LibraryPaths,
    server_id: i64,
    ctx: &mut HandlerCtx<'_>,
) -> Option<Option<tokio::sync::OwnedMutexGuard<()>>> {
    let (code, message, detail) =
        match lock_server_target(library, &*ctx.app.driver, server_id).await {
            Ok(guard) => return Some(guard),
            Err(parts) => parts,
        };
    let status = match ps_db::servers::get_server(&*ctx.app.driver, server_id).await {
        Ok(Some(record)) => server_status(services, &record).await,
        _ => None,
    };
    emit_refusal(
        ctx,
        MessageType::ServerStatusUpdate,
        serde_json::json!({ "server_id": server_id, "status": status, "success": false }),
        code,
        message,
        detail,
    );
    None
}

pub async fn handle_start_server(
    services: &ServerServices,
    library: &LibraryPaths,
    data: ServerIdData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let server_lock = services.server_locks.of(data.server_id);
    let _server_guard = server_lock.lock().await;
    if !relocation_allows_start(services, library, data.server_id, ctx).await {
        return Ok(());
    }
    let Some(_apply_guard) = start_apply_guard(services, library, data.server_id, ctx).await else {
        return Ok(());
    };
    let emitter = ctx.emitter;
    let db = &*ctx.app.driver;
    let result: Result<(), StartRefusal> = async {
        let Some(mut record) = ps_db::servers::get_server(db, data.server_id)
            .await
            .map_err(|error| ("db", error.to_string(), None))?
        else {
            return Err(("not_found", "Server not found".to_string(), None));
        };
        let (failure, status) = if record.server_type == "native" {
            if native_process::update_on_boot(&record) {
                update_native_install(emitter, &record).await;
            }
            match native_process::start_server_process(&record) {
                Ok(pid) => {
                    let mut pid_update = serde_json::Map::new();
                    pid_update.insert("pid".to_string(), Value::from(pid));
                    ps_db::servers::update_server(db, record.id, &pid_update)
                        .await
                        .map_err(|error| {
                            let running = native_process::process_status(Some(i64::from(pid)));
                            ("db", error.to_string(), Some(running))
                        })?;
                    (
                        None,
                        Some(native_process::process_status(Some(i64::from(pid)))),
                    )
                }
                Err(failure) => (Some(failure), Some(native_process::process_status(None))),
            }
        } else {
            let docker_api = services.docker.as_ref();
            let prepared = match backfill_docker_paks_path(&services.app_root, db, &mut record).await
            {
                Err(message) => Err(native_process::StartFailure { code: "db", message }),
                Ok(()) => prepare_container_for_start(docker_api, &record)
                    .await
                    .map_err(|message| native_process::StartFailure {
                        code: "container_create_failed",
                        message,
                    }),
            };
            let failure = match prepared {
                Err(failure) => Some(failure),
                Ok(()) => (!docker::start_server_container(docker_api, &record.container_name)
                    .await)
                    .then(|| native_process::StartFailure {
                        code: "server_start_failed",
                        message: format!("the container {} did not start", record.container_name),
                    }),
            };
            let status = docker::container_status(docker_api, &record.container_name).await;
            (failure, status)
        };
        let mut reply = serde_json::json!({
            "server_id": record.id,
            "status": status,
            "success": failure.is_none(),
        });
        if let Some(failure) = failure {
            reply["error"] =
                serde_json::json!({ "code": failure.code, "message": failure.message });
        }
        emitter.emit(MessageType::ServerStatusUpdate, &reply);
        Ok(())
    }
    .await;
    if let Err((code, message, status)) = result {
        emit_refusal(
            ctx,
            MessageType::ServerStatusUpdate,
            serde_json::json!({ "server_id": data.server_id, "status": status, "success": false }),
            code,
            message,
            serde_json::json!({}),
        );
    }
    Ok(())
}

/// A start that failed before a status reply, with the status known at the time.
type StartRefusal = (&'static str, String, Option<ServerProcessStatus>);

/// A failed update still lets the server start on the build it already has.
async fn update_native_install(emitter: &Emitter, record: &ServerRecord) {
    if record.steamcmd_path.is_empty() {
        return;
    }
    emit_creation_progress(emitter, "Checking for server updates via SteamCMD...");
    native_process::update_server(&record.steamcmd_path, &record.install_path).await;
    emit_creation_progress(emitter, "");
}

pub async fn handle_stop_server(
    services: &ServerServices,
    data: ServerIdData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let emitter = ctx.emitter;
    let db = &*ctx.app.driver;
    let result: Result<(), String> = async {
        let Some(record) = ps_db::servers::get_server(db, data.server_id)
            .await
            .map_err(|error| error.to_string())?
        else {
            emit_business_error(emitter, "Server not found".to_string());
            return Ok(());
        };
        // An explicit stop outranks the restart a pending relocation would do.
        if let Some(flag) = &record.pending_relocation {
            let mut stopped = serde_json::Map::new();
            stopped.insert(
                "pending_relocation".to_string(),
                Value::from(pending_relocation_with(flag, false)),
            );
            ps_db::servers::update_server(db, record.id, &stopped)
                .await
                .map_err(|error| error.to_string())?;
        }
        emit_creation_progress(emitter, &format!("Stopping server \"{}\"...", record.name));
        let (success, status) = if record.server_type == "native" {
            emit_creation_progress(emitter, "Sending shutdown command to server...");
            let success =
                native_process::stop_server_process(&record, &services.palworld_api).await;
            if success {
                let mut pid_update = serde_json::Map::new();
                pid_update.insert("pid".to_string(), Value::Null);
                ps_db::servers::update_server(db, record.id, &pid_update)
                    .await
                    .map_err(|error| error.to_string())?;
            }
            (success, Some(native_process::process_status(None)))
        } else {
            emit_creation_progress(emitter, "Stopping Docker container...");
            let docker_api = services.docker.as_ref();
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

#[derive(Debug, serde::Deserialize)]
pub struct ServerApiCallData {
    pub server_id: i64,
    pub endpoint: String,
    #[serde(default = "default_api_method")]
    pub method: String,
    #[serde(default)]
    pub payload: Option<Value>,
}

/// Proxies to the Palworld dedicated-server REST API at
/// 127.0.0.1:{rest_api_port} using the server's admin_password.
pub async fn handle_server_api_call(
    services: &ServerServices,
    data: ServerApiCallData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let emitter = ctx.emitter;
    let record = match ps_db::servers::get_server(&*ctx.app.driver, data.server_id).await {
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
    match services
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

/// Locates the server's world under `<saves_path>/SaveGames/0/<world>/Level.sav`
/// and runs it through the SAME load pipeline `handle_select_save` uses, so the
/// two paths cannot drift in behavior or error strings.
///
/// Returns `Err(String)` only for failures the caller renders as
/// "Failed to load server save: {e}"; every business rejection (server not
/// found, still running, no save data, no Level.sav, invalid steam directory)
/// emits its own `error` frame and returns `Ok(())`.
async fn load_server_save_impl(
    services: &ServerServices,
    data: ServerIdData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), String> {
    let emitter = ctx.emitter;
    let db = &*ctx.app.driver;
    let Some(record) = ps_db::servers::get_server(db, data.server_id)
        .await
        .map_err(|error| error.to_string())?
    else {
        emit_business_error(emitter, "Server not found".to_string());
        return Ok(());
    };

    // A running server holds the save file open and will overwrite whatever we
    // write back, so refuse to load from it.
    let status = server_status(services, &record).await;
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

    let save_games_path = Path::new(&record.saves_path).join("SaveGames").join("0");
    if !save_games_path.is_dir() {
        emit_business_error(
            emitter,
            format!("No save data found at {}", save_games_path.display()),
        );
        return Ok(());
    }

    // The first (usually only) world directory wins; a multi-world server is
    // not addressable here.
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

    let layout = match save_file::validate_steam_save_directory(&level_sav_path.to_string_lossy()) {
        Ok(layout) => layout,
        Err(error) => {
            emit_business_error(emitter, error.to_string());
            return Ok(());
        }
    };
    let progress = emitter.progress_sink();

    // Blocking read + parse of a potentially huge save — keep it off the async
    // workers (same rationale as api_convert). Progress frames still flow
    // through the same emitter channel, in the same order, because the tail
    // frames below only go out after the `.await`.
    let level_sav = layout.level_sav.clone();
    let level_meta = layout.level_meta.clone();
    let players_dir = layout.players_dir.clone();
    let gps_path = layout.global_pal_storage_sav.clone();
    let save_id = level_sav_path.to_string_lossy().into_owned();
    let (session, player_discovery_order) =
        tokio::task::spawn_blocking(move || -> Result<(SaveSession, Vec<uuid::Uuid>), String> {
            let level_sav_bytes = std::fs::read(&level_sav).map_err(|error| error.to_string())?;
            let level_meta_bytes = match &level_meta {
                Some(meta_path) => {
                    Some(std::fs::read(meta_path).map_err(|error| error.to_string())?)
                }
                None => None,
            };
            let (player_file_refs, player_discovery_order) =
                save_file::discover_player_file_refs(&players_dir)
                    .map_err(|error| error.to_string())?;
            let session = SaveSession::load(
                SaveKind::Steam {
                    level_path: level_sav.clone(),
                },
                save_id,
                "steam",
                &level_sav_bytes,
                level_meta_bytes.as_deref(),
                None,
                player_file_refs,
                gps_path,
                // Emit the leading generic "Loading Level.sav..." progress frame.
                true,
                &progress,
            )
            .map_err(|error| error.to_string())?;
            Ok((session, player_discovery_order))
        })
        .await
        .map_err(|join_error| join_error.to_string())??;

    // Point save_dir at the loaded world so a later write-back lands there.
    ps_db::settings::update_save_dir(&*ctx.app.driver, &world_dir.to_string_lossy())
        .await
        .map_err(|error| error.to_string())?;

    let has_gps = layout.global_pal_storage_sav.is_some();
    let session_id = ctx.register_current_session();
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
            "world_option_present": session.world_option.is_some(),
            "server_id": record.id,
            "server_name": record.name,
            "session_id": session_id.to_string(),
        }),
    );
    save_file::emit_summary_messages(&session, emitter);

    ctx.session.save = Some(session);
    Ok(())
}

pub async fn handle_load_server_save(
    services: &ServerServices,
    data: ServerIdData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if let Err(message) = load_server_save_impl(services, data, ctx).await {
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
    use crate::AppState;
    use ps_app::AppConfig;

    pub(crate) struct TestEnv {
        pub app: Arc<AppState>,
        pub services: Arc<ServerServices>,
        pub docker: Arc<MockDocker>,
        pub session: ps_core::session::Session,
        pub emitter: crate::emitter::Emitter,
        pub blueprints: crate::blueprint_registry::BlueprintRegistry,
        pub receiver: tokio::sync::mpsc::UnboundedReceiver<String>,
        pub _scratch: tempfile::TempDir,
        clock_seconds: Arc<std::sync::atomic::AtomicU64>,
    }

    impl TestEnv {
        pub(crate) async fn new() -> Self {
            Self::build(false).await
        }

        pub(crate) async fn new_desktop() -> Self {
            Self::build(true).await
        }

        async fn build(desktop_mode: bool) -> Self {
            let scratch = tempfile::tempdir().unwrap();
            let db = ps_db::open(&scratch.path().join("test.db")).await.unwrap();
            let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data");
            let game_data =
                Arc::new(ps_core::gamedata::GameData::load(&data_dir).expect("repo data dir"));
            let docker = Arc::new(MockDocker::default());
            let (live_connections, _live_connections_rx) = tokio::sync::watch::channel(0usize);
            let (live_bus, _live_bus_rx) = tokio::sync::watch::channel(None);
            let app = Arc::new(AppState {
                config: AppConfig { desktop_mode },
                game_data,
                driver: Arc::new(ps_db::SqlxSqliteDriver::new(db)),
                dialogs: Arc::new(crate::desktop_dialogs::NullDialogProvider),
                live_connections,
                live_bus,
                ext: Arc::new(crate::dispatcher::NullExtRouter),
                lsp: Arc::new(ps_app::lsp::NullLspService),
                sessions: std::sync::Mutex::new(crate::SessionStore::default()),
                breeding_db: Default::default(),
                plugins: Default::default(),
            });
            let (emitter, receiver) = crate::emitter::Emitter::test_channel();
            let clock_seconds = Arc::new(std::sync::atomic::AtomicU64::new(0));
            let mut services = ServerServices::with_docker(docker.clone(), scratch.path());
            let started = std::time::Instant::now();
            let elapsed = clock_seconds.clone();
            services.relocation_retries =
                crate::services::RelocationRetries::with_clock(Box::new(move || {
                    started
                        + std::time::Duration::from_secs(
                            elapsed.load(std::sync::atomic::Ordering::SeqCst),
                        )
                }));
            Self {
                app,
                services: Arc::new(services),
                docker,
                session: ps_core::session::Session::new(),
                emitter,
                blueprints: Default::default(),
                receiver,
                _scratch: scratch,
                clock_seconds,
            }
        }

        pub(crate) fn advance_clock(&self, by: std::time::Duration) {
            self.clock_seconds
                .fetch_add(by.as_secs(), std::sync::atomic::Ordering::SeqCst);
        }

        /// Like `new()`, but with `desktop_mode: true` and a `QueuedDialogProvider`
        /// that returns `folders` in order for folder-picker calls.
        pub(crate) async fn new_desktop_with_folders(
            folders: Vec<Option<std::path::PathBuf>>,
        ) -> Self {
            let mut env = Self::new().await;
            let mut config = env.app.config.clone();
            config.desktop_mode = true;
            let (live_connections, _live_connections_rx) = tokio::sync::watch::channel(0usize);
            let (live_bus, _live_bus_rx) = tokio::sync::watch::channel(None);
            let app = std::sync::Arc::new(AppState {
                config,
                game_data: env.app.game_data.clone(),
                driver: env.app.driver.clone(),
                dialogs: Arc::new(crate::desktop_dialogs::QueuedDialogProvider::new_with_folders(
                    folders,
                )),
                live_connections,
                live_bus,
                ext: Arc::new(crate::dispatcher::NullExtRouter),
                lsp: Arc::new(ps_app::lsp::NullLspService),
                sessions: std::sync::Mutex::new(crate::SessionStore::default()),
                breeding_db: Default::default(),
                plugins: Default::default(),
            });
            env.app = app;
            env
        }

        pub(crate) fn ctx(&mut self) -> crate::dispatcher::HandlerCtx<'_> {
            crate::dispatcher::HandlerCtx {
                session: &mut self.session,
                app: &self.app,
                emitter: &self.emitter,
                blueprints: &mut self.blueprints,
                is_loopback: false,
                mod_verification_subscribed: None,
                attachment: None,
            }
        }

        pub(crate) fn drain(&mut self) -> Vec<serde_json::Value> {
            let mut envelopes = Vec::new();
            while let Ok(frame) = self.receiver.try_recv() {
                envelopes.push(serde_json::from_str(&frame).unwrap());
            }
            envelopes
        }
    }

    pub(crate) fn docker_new_server(container_name: &str) -> ps_db::servers::NewServer {
        ps_db::servers::NewServer {
            name: format!("Server {container_name}"),
            container_name: container_name.to_string(),
            image_name: "omanrod/psp-palworld-server".to_string(),
            server_type: "docker".to_string(),
            game_port: 8211,
            query_port: 27015,
            rest_api_port: 8212,
            data_volume_name: format!("ps-{container_name}-data"),
            saves_path: String::new(),
            mods_path: String::new(),
            logicmods_path: String::new(),
            nativemods_path: String::new(),
            paks_path: format!("/srv/{container_name}/paks"),
            install_path: String::new(),
            steamcmd_path: String::new(),
            launch_args: String::new(),
            workshop_dir: String::new(),
            server_name: "PalStudio Palworld Server".to_string(),
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
        let services = env.services.clone();
        let library = LibraryPaths::new(env._scratch.path());
        let mut ctx = env.ctx();
        handle_list_servers(&services, &library, serde_json::Value::Null, &mut ctx)
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
        let record = ps_db::servers::create_server(&*env.app.driver, docker_new_server("alpha"))
            .await
            .unwrap();
        env.docker.statuses.lock().unwrap().insert(
            "alpha".to_string(),
            serde_json::json!({"State": {"Status": "exited", "Running": false, "StartedAt": null}}),
        );
        let services = env.services.clone();
        let library = LibraryPaths::new(env._scratch.path());
        let mut ctx = env.ctx();
        handle_list_servers(&services, &library, serde_json::Value::Null, &mut ctx)
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
        assert_eq!(entry["version"], Value::Null);
        assert_eq!(entry["latest_version"], Value::Null);
        assert_eq!(entry["env_vars"], serde_json::json!({}));
        assert!(entry["created_at"].as_str().unwrap().contains('T'));
    }

    #[tokio::test]
    async fn get_server_unknown_id_emits_business_error() {
        let mut env = TestEnv::new().await;
        let services = env.services.clone();
        let mut ctx = env.ctx();
        handle_get_server(&services, ServerIdData { server_id: 999 }, &mut ctx)
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
    async fn get_server_reports_whether_a_relocation_is_pending() {
        let mut env = TestEnv::new().await;
        let record = ps_db::servers::create_server(&*env.app.driver, docker_new_server("alpha"))
            .await
            .unwrap();
        env.docker.statuses.lock().unwrap().insert(
            "alpha".to_string(),
            serde_json::json!({"State": {"Status": "exited", "Running": false, "StartedAt": null}}),
        );
        let services = env.services.clone();
        let mut ctx = env.ctx();
        handle_get_server(&services, ServerIdData { server_id: record.id }, &mut ctx)
            .await
            .unwrap();
        assert_eq!(env.drain()[0]["data"]["relocation_pending"], false);

        set_pending_relocation(&env, record.id).await;
        let mut ctx = env.ctx();
        handle_get_server(&services, ServerIdData { server_id: record.id }, &mut ctx)
            .await
            .unwrap();
        assert_eq!(env.drain()[0]["data"]["relocation_pending"], true);
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
        let record = ps_db::servers::create_server(&*env.app.driver, new_server)
            .await
            .unwrap();
        let services = env.services.clone();
        let mut ctx = env.ctx();
        handle_get_server(
            &services,
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
        let record = ps_db::servers::create_server(&*env.app.driver, docker_new_server("gamma"))
            .await
            .unwrap();
        let services = env.services.clone();
        let mut ctx = env.ctx();
        handle_get_server_stats(
            &services,
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
        assert_eq!(data.image_name, "omanrod/psp-palworld-server");
        assert_eq!(data.server_type, "docker");
        assert_eq!(data.game_port, 8211);
        assert_eq!(data.query_port, 27015);
        assert_eq!(data.rest_api_port, 8212);
        assert_eq!(data.server_name, "PalStudio Palworld Server");
        assert_eq!(data.admin_password, "admin");
        assert_eq!(data.max_players, 16);

        let services = env.services.clone();
        let mut ctx = env.ctx();
        handle_create_server(&services, data, &mut ctx)
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
        assert_eq!(created["data_volume_name"], "ps-alpha-data");
        assert_eq!(created["status"]["running"], true); // mock create starts it
        assert_eq!(created["player_count"], 0);
        assert!(created.get("total_players").is_none()); // create has no total_players
        let listed = ps_db::servers::list_servers(&*env.app.driver).await.unwrap();
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
        ps_db::servers::create_server(&*env.app.driver, docker_new_server("first"))
            .await
            .unwrap();
        let data: CreateServerData = serde_json::from_value(serde_json::json!({
            "name": "Second",
            "container_name": "second"
        }))
        .unwrap();
        let services = env.services.clone();
        let mut ctx = env.ctx();
        handle_create_server(&services, data, &mut ctx)
            .await
            .unwrap();
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
        let services = env.services.clone();
        let mut ctx = env.ctx();
        handle_create_server(&services, data, &mut ctx)
            .await
            .unwrap();
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
        let new_server = scratch_docker_server(&env, "alpha");
        let record = ps_db::servers::create_server(&*env.app.driver, new_server)
            .await
            .unwrap();
        let mut updates = serde_json::Map::new();
        updates.insert("server_name".to_string(), serde_json::json!("Renamed"));
        let services = env.services.clone();
        let library = LibraryPaths::new(env._scratch.path());
        let mut ctx = env.ctx();
        handle_update_server(
            &services,
            &library,
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
        assert!(calls.contains(&"stop:alpha".to_string()));
        assert!(calls.contains(&"remove_container:alpha".to_string()));
        assert!(!calls.contains(&"remove_volume:ps-alpha-data".to_string()));
        assert!(calls.contains(&"create_and_start:alpha".to_string()));
    }

    #[tokio::test]
    async fn update_server_without_relevant_keys_skips_recreation() {
        let mut env = TestEnv::new().await;
        let record = ps_db::servers::create_server(&*env.app.driver, docker_new_server("alpha"))
            .await
            .unwrap();
        let mut updates = serde_json::Map::new();
        updates.insert("name".to_string(), serde_json::json!("Display Only"));
        let services = env.services.clone();
        let library = LibraryPaths::new(env._scratch.path());
        let mut ctx = env.ctx();
        handle_update_server(
            &services,
            &library,
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
        let services = env.services.clone();
        let library = LibraryPaths::new(env._scratch.path());
        let mut ctx = env.ctx();
        handle_update_server(
            &services,
            &library,
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
    async fn a_native_start_over_an_unreadable_mod_settings_file_says_why() {
        let mut env = TestEnv::new().await;
        let install = env._scratch.path().join("gamma");
        std::fs::create_dir_all(install.join("Mods")).unwrap();
        std::fs::write(install.join("PalServer.exe"), b"not a real executable").unwrap();
        let ini = install.join("Mods").join("PalModSettings.ini");
        std::fs::write(&ini, [0xC3, 0x28, b'A']).unwrap();
        let mut new_server = docker_new_server("gamma");
        new_server.server_type = "native".to_string();
        new_server.install_path = install.to_string_lossy().into_owned();
        let record = ps_db::servers::create_server(&*env.app.driver, new_server)
            .await
            .unwrap();
        let services = env.services.clone();
        let library = LibraryPaths::new(env._scratch.path());
        let mut ctx = env.ctx();
        handle_start_server(
            &services,
            &library,
            ServerIdData {
                server_id: record.id,
            },
            &mut ctx,
        )
        .await
        .unwrap();
        let messages = env.drain();
        let reply = messages
            .iter()
            .find(|message| message["type"] == "server_status_update")
            .unwrap_or_else(|| panic!("{messages:?}"));
        assert_eq!(reply["data"]["success"], false);
        assert_eq!(reply["data"]["error"]["code"], "settings_unreadable");
        assert!(
            reply["data"]["error"]["message"]
                .as_str()
                .unwrap()
                .contains("PalModSettings.ini"),
            "{reply}"
        );
        assert!(messages.iter().all(|message| message["type"] != "error"));
        assert_eq!(std::fs::read(&ini).unwrap(), [0xC3, 0x28, b'A']);
    }

    #[tokio::test]
    async fn starting_an_unknown_server_is_refused_under_server_status_update() {
        let mut env = TestEnv::new().await;
        let services = env.services.clone();
        let library = LibraryPaths::new(env._scratch.path());
        let mut ctx = env.ctx();
        handle_start_server(&services, &library, ServerIdData { server_id: 999 }, &mut ctx)
            .await
            .unwrap();
        let messages = env.drain();
        assert_eq!(messages.len(), 1, "{messages:?}");
        assert_eq!(messages[0]["type"], "server_status_update");
        assert_eq!(
            messages[0]["data"],
            serde_json::json!({
                "server_id": 999,
                "status": null,
                "success": false,
                "error": { "code": "not_found", "message": "Server not found" }
            })
        );
    }

    #[tokio::test]
    async fn a_start_that_cannot_read_the_server_is_refused_under_server_status_update() {
        let mut env = TestEnv::new().await;
        let record = ps_db::servers::create_server(&*env.app.driver, docker_new_server("alpha"))
            .await
            .unwrap();
        ps_db::DbDriver::execute(
            &*env.app.driver,
            "ALTER TABLE servers RENAME TO servers_unreadable",
            &[],
        )
        .await
        .unwrap();
        let services = env.services.clone();
        let library = LibraryPaths::new(env._scratch.path());
        let mut ctx = env.ctx();
        handle_start_server(
            &services,
            &library,
            ServerIdData {
                server_id: record.id,
            },
            &mut ctx,
        )
        .await
        .unwrap();
        let messages = env.drain();
        assert!(
            messages.iter().all(|message| message["type"] != "error"),
            "{messages:?}"
        );
        let reply = messages
            .iter()
            .find(|message| message["type"] == "server_status_update")
            .unwrap_or_else(|| panic!("{messages:?}"));
        assert_eq!(reply["data"]["server_id"], record.id);
        assert_eq!(reply["data"]["success"], false);
        assert_eq!(reply["data"]["error"]["code"], "db", "{reply}");
        assert!(reply["data"]["error"]["message"].is_string(), "{reply}");
    }

    #[tokio::test]
    async fn ensure_gamedata_launch_arg_appends_to_native_server() {
        let mut env = TestEnv::new().await;
        let mut new_server = docker_new_server("alpha");
        new_server.server_type = "native".to_string();
        new_server.install_path = env
            ._scratch
            .path()
            .join("alpha")
            .to_string_lossy()
            .into_owned();
        let record = ps_db::servers::create_server(&*env.app.driver, new_server)
            .await
            .unwrap();
        let services = env.services.clone();
        let library = LibraryPaths::new(env._scratch.path());
        let mut ctx = env.ctx();
        handle_ensure_gamedata_launch_arg(
            &services,
            &library,
            ServerIdData {
                server_id: record.id,
            },
            &mut ctx,
        )
        .await
        .unwrap();
        let messages = env.drain();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["type"], "ensure_gamedata_launch_arg");
        assert_eq!(messages[0]["data"]["launch_args"], "-enable-gamedata-api");
    }

    #[tokio::test]
    async fn ensure_gamedata_launch_arg_is_idempotent() {
        let mut env = TestEnv::new().await;
        let mut new_server = docker_new_server("beta");
        new_server.server_type = "native".to_string();
        new_server.launch_args = "-someflag".to_string();
        new_server.install_path = env
            ._scratch
            .path()
            .join("beta")
            .to_string_lossy()
            .into_owned();
        let record = ps_db::servers::create_server(&*env.app.driver, new_server)
            .await
            .unwrap();
        let services = env.services.clone();
        let library = LibraryPaths::new(env._scratch.path());

        let mut ctx = env.ctx();
        handle_ensure_gamedata_launch_arg(
            &services,
            &library,
            ServerIdData {
                server_id: record.id,
            },
            &mut ctx,
        )
        .await
        .unwrap();
        env.drain();

        let mut ctx = env.ctx();
        handle_ensure_gamedata_launch_arg(
            &services,
            &library,
            ServerIdData {
                server_id: record.id,
            },
            &mut ctx,
        )
        .await
        .unwrap();
        let messages = env.drain();
        assert_eq!(messages.len(), 1);
        let launch_args = messages[0]["data"]["launch_args"].as_str().unwrap();
        assert_eq!(launch_args.matches("-enable-gamedata-api").count(), 1);
        assert_eq!(launch_args, "-someflag -enable-gamedata-api");
    }

    #[tokio::test]
    async fn ensure_gamedata_launch_arg_refuses_docker_server() {
        let mut env = TestEnv::new().await;
        let record = ps_db::servers::create_server(&*env.app.driver, docker_new_server("gamma"))
            .await
            .unwrap();
        let services = env.services.clone();
        let library = LibraryPaths::new(env._scratch.path());
        let mut ctx = env.ctx();
        handle_ensure_gamedata_launch_arg(
            &services,
            &library,
            ServerIdData {
                server_id: record.id,
            },
            &mut ctx,
        )
        .await
        .unwrap();
        let messages = env.drain();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["type"], "ensure_gamedata_launch_arg");
        assert_eq!(
            messages[0]["data"]["error"],
            "Launch arguments are not supported for container servers yet"
        );
    }

    #[tokio::test]
    async fn ensure_gamedata_launch_arg_unknown_server_replies_under_its_own_type() {
        let mut env = TestEnv::new().await;
        let services = env.services.clone();
        let library = LibraryPaths::new(env._scratch.path());
        let mut ctx = env.ctx();
        handle_ensure_gamedata_launch_arg(
            &services,
            &library,
            ServerIdData { server_id: 42 },
            &mut ctx,
        )
        .await
        .unwrap();
        let messages = env.drain();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["type"], "ensure_gamedata_launch_arg");
        assert_eq!(messages[0]["data"]["error"], "Server not found");
    }

    #[tokio::test]
    async fn delete_docker_server_stops_removes_with_volumes_and_deletes_row() {
        let mut env = TestEnv::new().await;
        let record = ps_db::servers::create_server(&*env.app.driver, docker_new_server("alpha"))
            .await
            .unwrap();
        let services = env.services.clone();
        let library = LibraryPaths::new(env._scratch.path());
        let mut ctx = env.ctx();
        handle_delete_server(
            &services,
            &library,
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
        assert!(calls.contains(&"remove_volume:ps-alpha-data".to_string()));
        assert!(ps_db::servers::get_server(&*env.app.driver, record.id)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn deleting_a_server_removes_its_mod_target() {
        let mut env = TestEnv::new().await;
        let record = ps_db::servers::create_server(&*env.app.driver, docker_new_server("alpha"))
            .await
            .unwrap();
        let target_id = crate::mod_target_service::ensure_for(&*env.app.driver, &record, &env.services.app_root)
            .await
            .unwrap()
            .expect("a new server has no target yet");

        let services = env.services.clone();
        let library = LibraryPaths::new(env._scratch.path());
        let mut ctx = env.ctx();
        handle_delete_server(
            &services,
            &library,
            ServerIdData {
                server_id: record.id,
            },
            &mut ctx,
        )
        .await
        .unwrap();

        assert!(ps_db::mod_targets::for_server(&*env.app.driver, record.id)
            .await
            .unwrap()
            .is_none());
        assert!(
            ps_db::mod_profiles::get(&*env.app.driver, &format!("{target_id}/default"))
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn start_docker_server_creates_a_missing_container_and_emits_status_update() {
        let mut env = TestEnv::new().await;
        let new_server = scratch_docker_server(&env, "alpha");
        let record = ps_db::servers::create_server(&*env.app.driver, new_server)
            .await
            .unwrap();

        let messages = start(&mut env, record.id).await;

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["type"], "server_status_update");
        assert_eq!(messages[0]["data"]["server_id"], record.id);
        assert_eq!(messages[0]["data"]["success"], true);
        assert_eq!(messages[0]["data"]["status"]["running"], true);
        let calls = env.docker.calls.lock().unwrap().clone();
        assert_eq!(
            calls,
            vec![
                "ensure_image:omanrod/psp-palworld-server",
                "create_and_start:alpha",
                "start:alpha",
            ]
        );
        assert!(Path::new(&record.paks_path).is_dir());
    }

    #[test]
    fn server_wire_json_carries_paks_path() {
        let record = crate::services::docker::test_support::docker_record();
        assert_eq!(server_to_wire_json(&record)["paks_path"], "/srv/alpha/paks");
    }

    fn pre_feature_binds() -> serde_json::Value {
        serde_json::json!([
            "ps-alpha-data:/palworld/:rw",
            "/srv/alpha/saves:/palworld/Pal/Saved/:rw",
            "/srv/alpha/mods:/palworld/Pal/Binaries/Win64/Mods/:rw",
            "/srv/alpha/logicmods:/palworld/Pal/Content/Paks/LogicMods/:rw",
            "/srv/alpha/nativemods:/palworld/nativemods/:rw",
        ])
    }

    fn scratch_docker_server(env: &TestEnv, container_name: &str) -> NewServer {
        let mut new_server = docker_new_server(container_name);
        new_server.paks_path = env
            ._scratch
            .path()
            .join(container_name)
            .join("paks")
            .to_string_lossy()
            .into_owned();
        new_server
    }

    async fn set_pending_relocation(env: &TestEnv, server_id: i64) {
        let mut updates = serde_json::Map::new();
        updates.insert(
            "pending_relocation".to_string(),
            serde_json::json!(r#"{"op":"relocate","was_running":false}"#),
        );
        ps_db::servers::update_server(&*env.app.driver, server_id, &updates)
            .await
            .unwrap()
            .unwrap();
    }

    async fn start(env: &mut TestEnv, server_id: i64) -> Vec<serde_json::Value> {
        let services = env.services.clone();
        let library = LibraryPaths::new(env._scratch.path());
        let mut ctx = env.ctx();
        handle_start_server(&services, &library, ServerIdData { server_id }, &mut ctx)
            .await
            .unwrap();
        env.drain()
    }

    #[tokio::test]
    async fn a_failed_container_creation_on_start_replies_under_server_status_update() {
        let mut env = TestEnv::new().await;
        let new_server = scratch_docker_server(&env, "alpha");
        let record = ps_db::servers::create_server(&*env.app.driver, new_server)
            .await
            .unwrap();
        env.docker
            .fail_create
            .lock()
            .unwrap()
            .insert("alpha".to_string());

        let messages = start(&mut env, record.id).await;

        assert_eq!(messages.len(), 1, "{messages:?}");
        assert_eq!(messages[0]["type"], "server_status_update");
        let reply = &messages[0]["data"];
        assert_eq!(reply["server_id"], record.id);
        assert_eq!(reply["success"], false);
        assert_eq!(reply["error"]["code"], "container_create_failed", "{reply}");
        assert!(reply["error"]["message"].is_string(), "{reply}");
        assert!(!calls(&env).iter().any(|call| call.starts_with("start")));
    }

    #[tokio::test]
    async fn a_failed_recreate_on_start_replies_under_server_status_update() {
        let mut env = TestEnv::new().await;
        let new_server = scratch_docker_server(&env, "alpha");
        let record = ps_db::servers::create_server(&*env.app.driver, new_server)
            .await
            .unwrap();
        env.docker.statuses.lock().unwrap().insert(
            "alpha".to_string(),
            serde_json::json!({
                "State": {"Status": "exited", "Running": false, "StartedAt": null},
                "HostConfig": {"Binds": pre_feature_binds()}
            }),
        );
        env.docker
            .fail_create
            .lock()
            .unwrap()
            .insert("alpha".to_string());

        let messages = start(&mut env, record.id).await;

        assert_eq!(messages.len(), 1, "{messages:?}");
        assert_eq!(messages[0]["type"], "server_status_update");
        let reply = &messages[0]["data"];
        assert_eq!(reply["success"], false);
        assert_eq!(reply["error"]["code"], "container_create_failed", "{reply}");
        assert_eq!(
            calls(&env),
            vec![
                "ensure_image:omanrod/psp-palworld-server",
                "stop:alpha",
                "remove_container:alpha",
            ]
        );
    }

    #[tokio::test]
    async fn start_is_refused_while_an_apply_holds_the_servers_target() {
        let mut env = TestEnv::new().await;
        let new_server = scratch_docker_server(&env, "alpha");
        let record = ps_db::servers::create_server(&*env.app.driver, new_server)
            .await
            .unwrap();
        let target_id =
            crate::mod_target_service::ensure_for(&*env.app.driver, &record, &env.services.app_root)
                .await
                .unwrap()
                .unwrap();
        let target = ps_db::mod_targets::get(&*env.app.driver, &target_id)
            .await
            .unwrap()
            .unwrap();
        let guard = deploy::try_lock_target(&LibraryPaths::new(env._scratch.path()), &target)
            .expect("no apply is running");

        let messages = start(&mut env, record.id).await;

        assert_eq!(messages.len(), 1, "{messages:?}");
        assert_eq!(messages[0]["type"], "server_status_update");
        let reply = &messages[0]["data"];
        assert_eq!(reply["server_id"], record.id);
        assert_eq!(reply["success"], false);
        assert_eq!(reply["error"]["code"], "apply_in_progress", "{reply}");
        assert_eq!(reply["error"]["target_id"], target_id.as_str());
        assert!(calls(&env).is_empty(), "{:?}", calls(&env));
        assert!(!Path::new(&record.paks_path).exists());

        drop(guard);
        let messages = start(&mut env, record.id).await;
        assert_eq!(data_of(&messages, "server_status_update")["success"], true);
    }

    #[tokio::test]
    async fn deleting_a_server_is_refused_while_an_apply_holds_its_target() {
        let mut env = TestEnv::new().await;
        let record = ps_db::servers::create_server(&*env.app.driver, docker_new_server("alpha"))
            .await
            .unwrap();
        let target_id =
            crate::mod_target_service::ensure_for(&*env.app.driver, &record, &env.services.app_root)
                .await
                .unwrap()
                .unwrap();
        let target = ps_db::mod_targets::get(&*env.app.driver, &target_id)
            .await
            .unwrap()
            .unwrap();
        let library = LibraryPaths::new(env._scratch.path());
        let _guard = deploy::try_lock_target(&library, &target).expect("no apply is running");

        let services = env.services.clone();
        let mut ctx = env.ctx();
        handle_delete_server(&services, &library, ServerIdData { server_id: record.id }, &mut ctx)
            .await
            .unwrap();
        let messages = env.drain();

        assert_eq!(messages.len(), 1, "{messages:?}");
        assert_eq!(messages[0]["type"], "delete_server");
        assert_eq!(messages[0]["data"]["server_id"], record.id);
        assert_eq!(messages[0]["data"]["error"]["code"], "apply_in_progress");
        assert_eq!(messages[0]["data"]["error"]["target_id"], target_id.as_str());
        assert!(calls(&env).is_empty(), "{:?}", calls(&env));
        assert!(stored(&env, record.id).await.id == record.id);
        assert!(ps_db::mod_targets::get(&*env.app.driver, &target_id)
            .await
            .unwrap()
            .is_some());
    }

    #[test]
    fn a_native_install_move_carries_only_the_mod_columns_under_it() {
        let old_install = Path::new("/srv/old-install");
        let new_install = Path::new("/srv/new-install");
        let under = |base: &Path, rel: &str| base.join(rel).to_string_lossy().into_owned();
        let mut record = docker::test_support::docker_record();
        record.server_type = "native".to_string();
        record.install_path = old_install.to_string_lossy().into_owned();
        record.mods_path = under(old_install, "Pal/Binaries/Win64/Mods");
        record.logicmods_path = under(old_install, "Pal/Content/Paks/LogicMods");
        record.paks_path = under(old_install, "Pal/Content/Paks/~mods");
        record.nativemods_path = "/elsewhere/nativemods".to_string();
        let mut updates = serde_json::Map::new();
        updates.insert(
            "install_path".to_string(),
            serde_json::json!(new_install.to_string_lossy()),
        );

        let committed = relocation_updates(&record, &updates, false);

        for (column, rel) in [
            ("mods_path", "Pal/Binaries/Win64/Mods"),
            ("logicmods_path", "Pal/Content/Paks/LogicMods"),
            ("paks_path", "Pal/Content/Paks/~mods"),
        ] {
            let moved = committed[column].as_str().unwrap_or_default();
            assert!(same_host_path(moved, &under(new_install, rel)), "{column}: {moved}");
        }
        assert!(committed.get("nativemods_path").is_none(), "{committed:?}");
        let flag: serde_json::Value =
            serde_json::from_str(committed["pending_relocation"].as_str().unwrap()).unwrap();
        assert!(same_host_path(
            flag["to"]["nativemods_path"].as_str().unwrap(),
            "/elsewhere/nativemods"
        ));
        assert!(same_host_path(
            flag["to"]["mods_path"].as_str().unwrap(),
            &under(new_install, "Pal/Binaries/Win64/Mods")
        ));
    }

    #[test]
    fn a_mod_column_only_sharing_a_prefix_with_the_install_is_not_carried() {
        let mut record = docker::test_support::docker_record();
        record.server_type = "native".to_string();
        record.install_path = "/srv/old".to_string();
        record.mods_path = "/srv/old-mods/Mods".to_string();
        record.logicmods_path = "/srv/old/../escape/LogicMods".to_string();
        let mut updates = serde_json::Map::new();
        updates.insert("install_path".to_string(), serde_json::json!("/srv/new"));

        let committed = relocation_updates(&record, &updates, false);

        assert!(committed.get("mods_path").is_none(), "{committed:?}");
        assert!(committed.get("logicmods_path").is_none(), "{committed:?}");
    }

    #[tokio::test]
    async fn start_recreates_a_container_whose_binds_lack_paks_mods() {
        let mut env = TestEnv::new().await;
        let new_server = scratch_docker_server(&env, "alpha");
        let record = ps_db::servers::create_server(&*env.app.driver, new_server)
            .await
            .unwrap();
        env.docker.statuses.lock().unwrap().insert(
            "alpha".to_string(),
            serde_json::json!({
                "State": {"Status": "exited", "Running": false, "StartedAt": null},
                "HostConfig": {"Binds": pre_feature_binds()}
            }),
        );

        let messages = start(&mut env, record.id).await;

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["type"], "server_status_update");
        assert_eq!(messages[0]["data"]["success"], true);
        assert_eq!(messages[0]["data"]["status"]["running"], true);
        let calls = env.docker.calls.lock().unwrap().clone();
        assert_eq!(
            calls,
            vec![
                "ensure_image:omanrod/psp-palworld-server",
                "stop:alpha",
                "remove_container:alpha",
                "create_and_start:alpha",
                "start:alpha",
            ]
        );
        let status = env.docker.statuses.lock().unwrap()["alpha"].clone();
        assert!(crate::services::docker::mounts_paks_mods(&status));
        assert!(Path::new(&record.paks_path).is_dir());
    }

    #[tokio::test]
    async fn start_does_not_recreate_a_container_that_mounts_paks_mods() {
        let mut env = TestEnv::new().await;
        let new_server = scratch_docker_server(&env, "alpha");
        let record = ps_db::servers::create_server(&*env.app.driver, new_server)
            .await
            .unwrap();
        let mut binds = pre_feature_binds();
        binds
            .as_array_mut()
            .unwrap()
            .push(r"\srv\alpha\paks:/palworld/Pal/Content/Paks/~mods/:rw".into());
        env.docker.statuses.lock().unwrap().insert(
            "alpha".to_string(),
            serde_json::json!({
                "State": {"Status": "exited", "Running": false, "StartedAt": null},
                "HostConfig": {"Binds": binds}
            }),
        );

        let messages = start(&mut env, record.id).await;

        assert_eq!(messages[0]["data"]["success"], true);
        let calls = env.docker.calls.lock().unwrap().clone();
        assert_eq!(calls, vec!["start:alpha"]);
        assert!(Path::new(&record.paks_path).is_dir());
    }

    #[tokio::test]
    async fn start_does_not_create_a_missing_container_while_a_relocation_is_pending() {
        let mut env = TestEnv::new().await;
        let new_server = scratch_docker_server(&env, "alpha");
        let record = ps_db::servers::create_server(&*env.app.driver, new_server)
            .await
            .unwrap();
        crate::mod_target_service::ensure_for(&*env.app.driver, &record, &env.services.app_root)
            .await
            .unwrap();
        set_pending_relocation(&env, record.id).await;
        env.docker
            .fail_inspect
            .lock()
            .unwrap()
            .insert("alpha".to_string());

        let messages = start(&mut env, record.id).await;

        let reply = data_of(&messages, "server_status_update");
        assert_eq!(reply["success"], false);
        assert_eq!(reply["error"]["code"], "relocation_pending");
        assert_eq!(reply["error"]["apply"]["error"]["code"], "target_locked");
        let calls = env.docker.calls.lock().unwrap().clone();
        assert!(!calls
            .iter()
            .any(|call| call.starts_with("create_and_start:")));
    }

    fn relocatable_docker_server(env: &TestEnv, container_name: &str) -> NewServer {
        let base = env._scratch.path().join(container_name);
        let column = |name: &str| base.join(name).to_string_lossy().into_owned();
        let mut new_server = docker_new_server(container_name);
        new_server.saves_path = column("saves");
        new_server.mods_path = column("mods");
        new_server.logicmods_path = column("logicmods");
        new_server.nativemods_path = column("nativemods");
        new_server.paks_path = column("paks");
        new_server
    }

    async fn store_cool_mod(db: &dyn ps_db::DbDriver, library: &LibraryPaths, scratch: &Path) {
        use crate::services::mods::library;
        use ps_core::mods::{FileRoute, InstallManifest, ModType, RouteKind, SourceHint};
        let extracted = scratch.join("cool-mod-extracted");
        std::fs::create_dir_all(extracted.join("CoolMod")).unwrap();
        std::fs::write(extracted.join("CoolMod").join("config.lua"), b"defaults").unwrap();
        let manifest = InstallManifest {
            folder_name: "CoolMod".to_string(),
            display_name: "CoolMod".to_string(),
            mod_type: ModType::Ue4ss,
            version: "1.0".to_string(),
            routes: vec![FileRoute {
                archive_path: "CoolMod/config.lua".to_string(),
                rel_path: "CoolMod/config.lua".to_string(),
                kind: RouteKind::Ue4ss,
            }],
            decisions: Vec::new(),
            platform_filtered: None,
            source: SourceHint::default(),
        };
        library::store(
            db,
            library,
            &library::StoreRequest {
                mod_id: "coolmod",
                manifest: &manifest,
                extracted_root: &extracted,
                archive: None,
                source_kind: "local",
                source_ref: "{}",
                custom_name: None,
            },
        )
        .await
        .unwrap();
    }

    /// A stopped docker server `alpha` whose applied mod file the user has edited.
    async fn server_with_an_edited_mod(env: &mut TestEnv) -> (ServerRecord, std::path::PathBuf) {
        let db = env.app.driver.clone();
        let record = ps_db::servers::create_server(&*db, relocatable_docker_server(env, "alpha"))
            .await
            .unwrap();
        let target_id = crate::mod_target_service::ensure_for(&*db, &record, &env.services.app_root)
            .await
            .unwrap()
            .expect("a new server has no target yet");
        let library = LibraryPaths::new(env._scratch.path());
        store_cool_mod(&*db, &library, env._scratch.path()).await;
        ps_db::mod_profiles::set_mod(
            &*db,
            &ps_db::mod_profiles::ProfileModRow {
                profile_id: format!("{target_id}/default"),
                mod_id: "coolmod".to_string(),
                mod_version_id: None,
                enabled: true,
                load_order: 0,
            },
        )
        .await
        .unwrap();
        let target = ps_db::mod_targets::get(&*db, &target_id)
            .await
            .unwrap()
            .unwrap();
        let applied =
            crate::mods_handlers::run_apply(&env.services, &library, &*db, &env.emitter, &target, &[], true)
                .await;
        assert!(!applied.is_null() && apply_closed_cleanly(&applied), "{applied}");
        let deployed = Path::new(&record.mods_path).join("CoolMod").join("config.lua");
        assert!(deployed.is_file(), "{deployed:?}");
        std::fs::write(&deployed, b"the user's settings").unwrap();
        env.docker.statuses.lock().unwrap().insert(
            "alpha".to_string(),
            serde_json::json!({
                "State": {"Status": "exited", "Running": false, "StartedAt": null},
                "HostConfig": {"Binds": docker::build_binds(&record)}
            }),
        );
        env.drain();
        env.docker.calls.lock().unwrap().clear();
        (record, deployed)
    }

    fn moved_mods_dir(env: &TestEnv) -> std::path::PathBuf {
        env._scratch.path().join("alpha").join("mods-moved")
    }

    async fn update(
        env: &mut TestEnv,
        server_id: i64,
        updates: serde_json::Map<String, serde_json::Value>,
    ) -> Vec<serde_json::Value> {
        let services = env.services.clone();
        let library = LibraryPaths::new(env._scratch.path());
        let mut ctx = env.ctx();
        handle_update_server(
            &services,
            &library,
            UpdateServerData { server_id, updates },
            &mut ctx,
        )
        .await
        .unwrap();
        env.drain()
    }

    async fn update_mods_path(
        env: &mut TestEnv,
        server_id: i64,
        mods_path: &Path,
    ) -> Vec<serde_json::Value> {
        let mut updates = serde_json::Map::new();
        updates.insert(
            "mods_path".to_string(),
            serde_json::json!(mods_path.to_string_lossy()),
        );
        update(env, server_id, updates).await
    }

    async fn list(env: &mut TestEnv) -> Vec<serde_json::Value> {
        let services = env.services.clone();
        let library = LibraryPaths::new(env._scratch.path());
        let mut ctx = env.ctx();
        handle_list_servers(&services, &library, serde_json::Value::Null, &mut ctx)
            .await
            .unwrap();
        env.drain()
    }

    fn data_of<'a>(messages: &'a [serde_json::Value], message_type: &str) -> &'a serde_json::Value {
        &messages
            .iter()
            .find(|message| message["type"] == message_type)
            .unwrap_or_else(|| panic!("no {message_type} frame in {messages:?}"))["data"]
    }

    fn calls(env: &TestEnv) -> Vec<String> {
        env.docker.calls.lock().unwrap().clone()
    }

    async fn stored(env: &TestEnv, server_id: i64) -> ServerRecord {
        ps_db::servers::get_server(&*env.app.driver, server_id)
            .await
            .unwrap()
            .unwrap()
    }

    /// Relocates `alpha`'s mods onto a directory already holding an unmanaged
    /// file at the mod's destination. Returns the edited file, the blocker, and
    /// the update's frames.
    async fn a_blocked_relocation(
        env: &mut TestEnv,
    ) -> (
        ServerRecord,
        std::path::PathBuf,
        std::path::PathBuf,
        Vec<serde_json::Value>,
    ) {
        let (record, deployed) = server_with_an_edited_mod(env).await;
        let moved = moved_mods_dir(env);
        let blocker = moved.join("CoolMod").join("config.lua");
        std::fs::create_dir_all(blocker.parent().unwrap()).unwrap();
        std::fs::write(&blocker, b"someone else's").unwrap();
        let messages = update_mods_path(env, record.id, &moved).await;
        (record, deployed, blocker, messages)
    }

    #[tokio::test]
    async fn changing_a_docker_mods_path_moves_managed_files_and_recreates_without_starting_a_stopped_server(
    ) {
        let mut env = TestEnv::new().await;
        let (record, deployed) = server_with_an_edited_mod(&mut env).await;
        let moved = moved_mods_dir(&env);

        let messages = update_mods_path(&mut env, record.id, &moved).await;

        let reply = data_of(&messages, "update_server");
        assert!(reply.get("error").is_none(), "{reply}");
        assert_eq!(reply["relocation_pending"], false);
        assert_eq!(reply["apply"]["mid_apply"], false);
        assert!(reply["apply"].get("error").is_none(), "{reply}");
        assert_eq!(
            calls(&env),
            vec![
                "stop:alpha",
                "remove_container:alpha",
                "ensure_image:omanrod/psp-palworld-server",
                "create:alpha",
            ]
        );
        assert_eq!(
            std::fs::read(moved.join("CoolMod").join("config.lua")).unwrap(),
            b"the user's settings"
        );
        assert!(!deployed.exists(), "{deployed:?}");
        let stored = stored(&env, record.id).await;
        assert_eq!(stored.pending_relocation, None);
        assert_eq!(Path::new(&stored.mods_path), moved.as_path());
        let inspect = env.docker.statuses.lock().unwrap()["alpha"].clone();
        assert_eq!(inspect["State"]["Running"], false);
        assert_eq!(
            inspect["HostConfig"]["Binds"],
            serde_json::json!(docker::build_binds(&stored))
        );
    }

    #[tokio::test]
    async fn a_running_docker_server_is_started_again_after_relocation() {
        let mut env = TestEnv::new().await;
        let (record, _deployed) = server_with_an_edited_mod(&mut env).await;
        env.docker.statuses.lock().unwrap().get_mut("alpha").unwrap()["State"] =
            serde_json::json!({"Status": "running", "Running": true, "StartedAt": "x"});
        let moved = moved_mods_dir(&env);

        let messages = update_mods_path(&mut env, record.id, &moved).await;

        let reply = data_of(&messages, "update_server");
        assert!(reply.get("error").is_none(), "{reply}");
        assert_eq!(reply["relocation_pending"], false);
        assert_eq!(reply["status"]["running"], true);
        assert_eq!(
            calls(&env),
            vec![
                "stop:alpha",
                "remove_container:alpha",
                "ensure_image:omanrod/psp-palworld-server",
                "create:alpha",
                "start:alpha",
            ]
        );
        assert_eq!(
            std::fs::read(moved.join("CoolMod").join("config.lua")).unwrap(),
            b"the user's settings"
        );
        assert_eq!(stored(&env, record.id).await.pending_relocation, None);
    }

    #[tokio::test]
    async fn a_failed_relocation_keeps_the_flag_and_creates_no_container() {
        let mut env = TestEnv::new().await;
        let (record, deployed, blocker, messages) = a_blocked_relocation(&mut env).await;

        let reply = data_of(&messages, "update_server");
        assert!(reply.get("error").is_none(), "{reply}");
        assert_eq!(reply["relocation_pending"], true);
        assert_eq!(reply["apply"]["error"]["code"], "unmanaged_occupant", "{reply}");
        assert!(
            !calls(&env).iter().any(|call| call.starts_with("create")),
            "{:?}",
            calls(&env)
        );
        let stored = stored(&env, record.id).await;
        assert_eq!(
            Path::new(&stored.mods_path),
            moved_mods_dir(&env).as_path(),
            "the path change committed with the flag"
        );
        let flag: serde_json::Value =
            serde_json::from_str(stored.pending_relocation.as_deref().expect("flag kept")).unwrap();
        assert_eq!(flag["was_running"], false);
        assert_eq!(
            Path::new(flag["from"]["mods_path"].as_str().unwrap()),
            Path::new(&record.mods_path)
        );
        assert_eq!(std::fs::read(&deployed).unwrap(), b"the user's settings");
        assert_eq!(std::fs::read(&blocker).unwrap(), b"someone else's");
    }

    #[tokio::test]
    async fn starting_a_server_with_a_pending_relocation_finishes_it_first() {
        let mut env = TestEnv::new().await;
        let (record, _deployed, blocker, _) = a_blocked_relocation(&mut env).await;
        std::fs::remove_file(&blocker).unwrap();
        env.docker.calls.lock().unwrap().clear();

        let messages = start(&mut env, record.id).await;

        let reply = data_of(&messages, "server_status_update");
        assert!(reply.get("error").is_none(), "{reply}");
        assert_eq!(reply["success"], true);
        assert_eq!(stored(&env, record.id).await.pending_relocation, None);
        assert_eq!(
            calls(&env),
            vec![
                "ensure_image:omanrod/psp-palworld-server",
                "create:alpha",
                "start:alpha",
            ]
        );
        assert_eq!(
            std::fs::read(moved_mods_dir(&env).join("CoolMod").join("config.lua")).unwrap(),
            b"the user's settings"
        );
    }

    #[tokio::test]
    async fn starting_while_the_relocation_still_cannot_finish_is_refused_with_a_code() {
        let mut env = TestEnv::new().await;
        let (record, _deployed, _blocker, _) = a_blocked_relocation(&mut env).await;
        env.docker.calls.lock().unwrap().clear();

        let messages = start(&mut env, record.id).await;

        assert!(messages.iter().all(|message| message["type"] != "error"));
        let reply = data_of(&messages, "server_status_update");
        assert_eq!(reply["server_id"], record.id);
        assert_eq!(reply["success"], false);
        assert_eq!(reply["error"]["code"], "relocation_pending");
        assert_eq!(
            reply["error"]["apply"]["error"]["code"],
            "unmanaged_occupant"
        );
        assert!(
            !calls(&env)
                .iter()
                .any(|call| call.starts_with("create") || call.starts_with("start")),
            "{:?}",
            calls(&env)
        );
        assert!(stored(&env, record.id).await.pending_relocation.is_some());
    }

    #[tokio::test]
    async fn listing_retries_a_pending_relocation_and_lists_the_server_either_way() {
        let mut env = TestEnv::new().await;
        let (record, _deployed, blocker, _) = a_blocked_relocation(&mut env).await;
        env.docker.calls.lock().unwrap().clear();

        let messages = list(&mut env).await;
        assert_eq!(messages.len(), 1, "a polled attempt reports no progress: {messages:?}");
        let servers = data_of(&messages, "list_servers")["servers"].as_array().unwrap();
        assert_eq!(servers.len(), 1);
        assert!(stored(&env, record.id).await.pending_relocation.is_some());

        std::fs::remove_file(&blocker).unwrap();
        list(&mut env).await;
        assert!(
            stored(&env, record.id).await.pending_relocation.is_some(),
            "a poll inside the retry interval does not attempt again"
        );
        assert!(calls(&env).is_empty(), "{:?}", calls(&env));

        env.advance_clock(crate::services::RelocationRetries::INTERVAL);
        let messages = list(&mut env).await;
        let servers = data_of(&messages, "list_servers")["servers"].as_array().unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(stored(&env, record.id).await.pending_relocation, None);
        assert_eq!(
            calls(&env),
            vec!["ensure_image:omanrod/psp-palworld-server", "create:alpha"],
            "a stopped server is recreated but not started"
        );
    }

    #[tokio::test]
    async fn changing_only_the_server_name_still_takes_the_existing_recreate_path() {
        let mut env = TestEnv::new().await;
        let new_server = relocatable_docker_server(&env, "alpha");
        let record = ps_db::servers::create_server(&*env.app.driver, new_server)
            .await
            .unwrap();
        let mut updates = serde_json::Map::new();
        updates.insert("server_name".to_string(), serde_json::json!("Renamed"));

        let messages = update(&mut env, record.id, updates).await;

        let reply = data_of(&messages, "update_server");
        assert!(reply.get("relocation_pending").is_none(), "{reply}");
        assert_eq!(
            calls(&env),
            vec![
                "stop:alpha",
                "remove_container:alpha",
                "ensure_image:omanrod/psp-palworld-server",
                "create_and_start:alpha",
            ]
        );
        assert_eq!(stored(&env, record.id).await.pending_relocation, None);
    }

    #[tokio::test]
    async fn a_mods_path_spelled_differently_is_not_a_relocation() {
        let mut env = TestEnv::new().await;
        let new_server = relocatable_docker_server(&env, "alpha");
        let record = ps_db::servers::create_server(&*env.app.driver, new_server)
            .await
            .unwrap();
        let respelled = if cfg!(windows) {
            record.mods_path.replace('\\', "/")
        } else {
            format!("{}/", record.mods_path)
        };
        assert_ne!(respelled, record.mods_path);
        let mut updates = serde_json::Map::new();
        updates.insert("mods_path".to_string(), serde_json::json!(respelled));

        let messages = update(&mut env, record.id, updates).await;

        let reply = data_of(&messages, "update_server");
        assert!(reply.get("relocation_pending").is_none(), "{reply}");
        assert!(calls(&env).is_empty(), "{:?}", calls(&env));
        assert_eq!(stored(&env, record.id).await.pending_relocation, None);
    }

    #[test]
    fn the_paths_and_the_flag_are_committed_in_one_update() {
        let record = docker::test_support::docker_record();
        let mut updates = serde_json::Map::new();
        updates.insert("mods_path".to_string(), serde_json::json!("/srv/moved/mods"));
        updates.insert("server_name".to_string(), serde_json::json!("Renamed"));
        assert!(relocation_changes(&record, &updates));

        let committed = relocation_updates(&record, &updates, true);

        assert_eq!(committed["mods_path"], "/srv/moved/mods");
        assert_eq!(committed["server_name"], "Renamed");
        let flag_text = committed["pending_relocation"].as_str().unwrap();
        assert!(pending_relocation_was_running(record.id, flag_text));
        let flag: serde_json::Value = serde_json::from_str(flag_text).unwrap();
        assert_eq!(flag["op"], "relocate");
        assert_eq!(flag["from"]["mods_path"], "/srv/alpha/mods");
        assert_eq!(flag["to"]["mods_path"], "/srv/moved/mods");
        assert_eq!(flag["to"]["logicmods_path"], "/srv/alpha/logicmods");
        assert!(flag["to"].get("install_path").is_none());
    }

    #[tokio::test]
    async fn a_client_cannot_write_the_relocation_flag() {
        let mut env = TestEnv::new().await;
        let record = ps_db::servers::create_server(&*env.app.driver, docker_new_server("alpha"))
            .await
            .unwrap();
        let mut updates = serde_json::Map::new();
        updates.insert(
            "pending_relocation".to_string(),
            serde_json::json!(r#"{"op":"relocate","was_running":true}"#),
        );

        update(&mut env, record.id, updates).await;

        assert_eq!(stored(&env, record.id).await.pending_relocation, None);
    }

    fn set_running(env: &TestEnv, container_name: &str) {
        env.docker
            .statuses
            .lock()
            .unwrap()
            .get_mut(container_name)
            .unwrap()["State"] =
            serde_json::json!({"Status": "running", "Running": true, "StartedAt": "x"});
    }

    fn write_blocker(dir: &Path) -> std::path::PathBuf {
        let blocker = dir.join("CoolMod").join("config.lua");
        std::fs::create_dir_all(blocker.parent().unwrap()).unwrap();
        std::fs::write(&blocker, b"someone else's").unwrap();
        blocker
    }

    #[tokio::test]
    async fn a_settings_save_during_a_pending_relocation_creates_no_container() {
        let mut env = TestEnv::new().await;
        let (record, _deployed, _blocker, _) = a_blocked_relocation(&mut env).await;
        env.docker.calls.lock().unwrap().clear();
        let mut updates = serde_json::Map::new();
        updates.insert("env_vars".to_string(), serde_json::json!({"EXP_RATE": "3.0"}));
        updates.insert("server_name".to_string(), serde_json::json!("Renamed"));

        let messages = update(&mut env, record.id, updates).await;

        let reply = data_of(&messages, "update_server");
        assert!(reply.get("error").is_none(), "{reply}");
        assert_eq!(reply["relocation_pending"], true);
        let calls = calls(&env);
        assert!(
            !calls
                .iter()
                .any(|call| call.starts_with("create") || call.starts_with("start")),
            "{calls:?}"
        );
        let stored = stored(&env, record.id).await;
        assert_eq!(stored.server_name, "Renamed");
        assert_eq!(stored.env_vars["EXP_RATE"], "3.0");
        assert!(stored.pending_relocation.is_some());
    }

    #[tokio::test]
    async fn a_relocation_is_refused_whole_when_the_running_state_cannot_be_read() {
        let mut env = TestEnv::new().await;
        let (record, deployed) = server_with_an_edited_mod(&mut env).await;
        env.docker
            .fail_inspect
            .lock()
            .unwrap()
            .insert("alpha".to_string());
        let mut updates = serde_json::Map::new();
        updates.insert(
            "mods_path".to_string(),
            serde_json::json!(moved_mods_dir(&env).to_string_lossy()),
        );
        updates.insert("server_name".to_string(), serde_json::json!("Renamed"));

        let messages = update(&mut env, record.id, updates).await;

        let reply = data_of(&messages, "update_server");
        assert_eq!(reply["server_id"], record.id);
        assert_eq!(reply["error"]["code"], "server_state_unknown");
        assert!(calls(&env).is_empty(), "{:?}", calls(&env));
        let stored = stored(&env, record.id).await;
        assert_eq!(stored.pending_relocation, None);
        assert_eq!(Path::new(&stored.mods_path), Path::new(&record.mods_path));
        assert_eq!(stored.server_name, record.server_name);
        assert_eq!(std::fs::read(&deployed).unwrap(), b"the user's settings");
    }

    #[tokio::test]
    async fn a_failed_container_creation_after_a_clean_close_keeps_the_flag_and_its_restart() {
        let mut env = TestEnv::new().await;
        let (record, _deployed) = server_with_an_edited_mod(&mut env).await;
        set_running(&env, "alpha");
        env.docker
            .fail_create
            .lock()
            .unwrap()
            .insert("alpha".to_string());
        let moved = moved_mods_dir(&env);

        let messages = update_mods_path(&mut env, record.id, &moved).await;

        let reply = data_of(&messages, "update_server");
        assert_eq!(reply["error"]["code"], "container_create_failed", "{reply}");
        assert_eq!(reply["relocation_pending"], true);
        assert!(reply["apply"].get("error").is_none(), "{reply}");
        assert_eq!(
            std::fs::read(moved.join("CoolMod").join("config.lua")).unwrap(),
            b"the user's settings"
        );
        let flag = stored(&env, record.id)
            .await
            .pending_relocation
            .expect("the flag is written back");
        assert!(pending_relocation_was_running(record.id, &flag));

        list(&mut env).await;
        assert!(
            stored(&env, record.id).await.pending_relocation.is_some(),
            "a listing that meets the same failure keeps the flag"
        );

        env.docker.calls.lock().unwrap().clear();
        let messages = start(&mut env, record.id).await;
        let reply = data_of(&messages, "server_status_update");
        assert_eq!(reply["success"], false);
        assert_eq!(reply["error"]["code"], "container_create_failed", "{reply}");
        assert!(
            !calls(&env).iter().any(|call| call.starts_with("start")),
            "{:?}",
            calls(&env)
        );

        env.docker.fail_create.lock().unwrap().clear();
        env.docker.calls.lock().unwrap().clear();
        let messages = start(&mut env, record.id).await;
        let reply = data_of(&messages, "server_status_update");
        assert_eq!(reply["success"], true, "{reply}");
        assert_eq!(stored(&env, record.id).await.pending_relocation, None);
        assert_eq!(
            calls(&env),
            vec![
                "ensure_image:omanrod/psp-palworld-server",
                "create:alpha",
                "start:alpha",
            ]
        );
    }

    #[tokio::test]
    async fn stopping_a_server_with_a_pending_relocation_keeps_recovery_from_starting_it() {
        let mut env = TestEnv::new().await;
        let (record, _deployed) = server_with_an_edited_mod(&mut env).await;
        set_running(&env, "alpha");
        let moved = moved_mods_dir(&env);
        let blocker = write_blocker(&moved);
        update_mods_path(&mut env, record.id, &moved).await;
        let flag = stored(&env, record.id).await.pending_relocation.unwrap();
        assert!(pending_relocation_was_running(record.id, &flag));

        let services = env.services.clone();
        let mut ctx = env.ctx();
        handle_stop_server(
            &services,
            ServerIdData {
                server_id: record.id,
            },
            &mut ctx,
        )
        .await
        .unwrap();
        env.drain();

        let flag = stored(&env, record.id)
            .await
            .pending_relocation
            .expect("stopping leaves the relocation pending");
        assert!(!pending_relocation_was_running(record.id, &flag));
        let flag: serde_json::Value = serde_json::from_str(&flag).unwrap();
        assert!(flag["to"]["mods_path"].is_string(), "{flag}");

        std::fs::remove_file(&blocker).unwrap();
        env.docker.calls.lock().unwrap().clear();
        list(&mut env).await;
        assert_eq!(stored(&env, record.id).await.pending_relocation, None);
        let calls = calls(&env);
        assert!(calls.contains(&"create:alpha".to_string()), "{calls:?}");
        assert!(
            !calls.iter().any(|call| call.starts_with("start")),
            "{calls:?}"
        );
    }

    #[tokio::test]
    async fn a_client_cannot_clear_a_pending_relocation_flag() {
        let mut env = TestEnv::new().await;
        let (record, _deployed, _blocker, _) = a_blocked_relocation(&mut env).await;
        let mut updates = serde_json::Map::new();
        updates.insert("pending_relocation".to_string(), serde_json::Value::Null);

        update(&mut env, record.id, updates).await;

        assert!(stored(&env, record.id).await.pending_relocation.is_some());
    }

    #[tokio::test]
    async fn a_crash_between_the_path_commit_and_the_overrides_rewrite_still_moves_the_files() {
        let mut env = TestEnv::new().await;
        let (record, deployed) = server_with_an_edited_mod(&mut env).await;
        let moved = moved_mods_dir(&env);
        env.docker.statuses.lock().unwrap().remove("alpha");
        let mut updates = serde_json::Map::new();
        updates.insert(
            "mods_path".to_string(),
            serde_json::json!(moved.to_string_lossy()),
        );
        ps_db::servers::update_server(
            &*env.app.driver,
            record.id,
            &relocation_updates(&record, &updates, false),
        )
        .await
        .unwrap()
        .unwrap();
        let target = ps_db::mod_targets::for_server(&*env.app.driver, record.id)
            .await
            .unwrap()
            .unwrap();
        let overrides: serde_json::Value = serde_json::from_str(&target.layout_overrides).unwrap();
        assert_eq!(
            Path::new(overrides["ue4ss_mods_dir"].as_str().unwrap()),
            Path::new(&record.mods_path),
            "the overrides were never rewritten"
        );

        let messages = start(&mut env, record.id).await;

        let reply = data_of(&messages, "server_status_update");
        assert_eq!(reply["success"], true, "{reply}");
        assert_eq!(
            std::fs::read(moved.join("CoolMod").join("config.lua")).unwrap(),
            b"the user's settings"
        );
        assert!(!deployed.exists(), "{deployed:?}");
        assert_eq!(stored(&env, record.id).await.pending_relocation, None);
    }

    #[tokio::test]
    async fn a_relocation_requested_over_a_pending_one_keeps_the_first_ones_restart() {
        let mut env = TestEnv::new().await;
        let (record, deployed) = server_with_an_edited_mod(&mut env).await;
        set_running(&env, "alpha");
        let first = moved_mods_dir(&env);
        let blocker = write_blocker(&first);
        update_mods_path(&mut env, record.id, &first).await;
        assert!(stored(&env, record.id).await.pending_relocation.is_some());
        env.docker.calls.lock().unwrap().clear();
        let second = env._scratch.path().join("alpha").join("mods-second");

        let messages = update_mods_path(&mut env, record.id, &second).await;

        let reply = data_of(&messages, "update_server");
        assert!(reply.get("error").is_none(), "{reply}");
        assert_eq!(reply["relocation_pending"], false);
        assert_eq!(
            calls(&env),
            vec![
                "stop:alpha",
                "remove_container:alpha",
                "ensure_image:omanrod/psp-palworld-server",
                "create:alpha",
                "start:alpha",
            ]
        );
        assert_eq!(
            std::fs::read(second.join("CoolMod").join("config.lua")).unwrap(),
            b"the user's settings"
        );
        assert!(!deployed.exists(), "{deployed:?}");
        assert_eq!(std::fs::read(&blocker).unwrap(), b"someone else's");
        assert_eq!(stored(&env, record.id).await.pending_relocation, None);
    }

    #[tokio::test]
    async fn an_unreadable_flag_is_finished_as_a_relocation_to_the_current_paths() {
        let mut env = TestEnv::new().await;
        let (record, deployed) = server_with_an_edited_mod(&mut env).await;
        env.docker.statuses.lock().unwrap().remove("alpha");
        let mut updates = serde_json::Map::new();
        updates.insert("pending_relocation".to_string(), serde_json::json!("not json"));
        ps_db::servers::update_server(&*env.app.driver, record.id, &updates)
            .await
            .unwrap()
            .unwrap();

        let messages = start(&mut env, record.id).await;

        let reply = data_of(&messages, "server_status_update");
        assert_eq!(reply["success"], true, "{reply}");
        assert_eq!(stored(&env, record.id).await.pending_relocation, None);
        assert_eq!(std::fs::read(&deployed).unwrap(), b"the user's settings");
        assert_eq!(
            calls(&env),
            vec![
                "ensure_image:omanrod/psp-palworld-server",
                "create:alpha",
                "start:alpha",
            ]
        );
    }

    #[tokio::test]
    async fn a_relocation_meeting_an_apply_in_progress_stays_pending() {
        let mut env = TestEnv::new().await;
        let (record, _deployed, blocker, _) = a_blocked_relocation(&mut env).await;
        std::fs::remove_file(&blocker).unwrap();
        env.docker.calls.lock().unwrap().clear();
        let target = ps_db::mod_targets::for_server(&*env.app.driver, record.id)
            .await
            .unwrap()
            .unwrap();
        let held = deploy::try_lock_target(&LibraryPaths::new(env._scratch.path()), &target)
            .expect("nothing else holds the target");

        let messages = list(&mut env).await;
        assert_eq!(messages.len(), 1, "the listing skips silently: {messages:?}");
        assert!(stored(&env, record.id).await.pending_relocation.is_some());

        let messages = start(&mut env, record.id).await;
        let reply = data_of(&messages, "server_status_update");
        assert_eq!(reply["success"], false);
        assert_eq!(reply["error"]["code"], "relocation_pending");
        assert_eq!(
            reply["error"]["apply"]["error"]["code"],
            "apply_in_progress",
            "{reply}"
        );
        assert!(stored(&env, record.id).await.pending_relocation.is_some());
        assert!(
            !calls(&env)
                .iter()
                .any(|call| call.starts_with("create") || call.starts_with("start")),
            "{:?}",
            calls(&env)
        );

        drop(held);
        let messages = start(&mut env, record.id).await;
        assert_eq!(data_of(&messages, "server_status_update")["success"], true);
        assert_eq!(stored(&env, record.id).await.pending_relocation, None);
    }

    #[tokio::test]
    async fn a_listing_that_meets_a_held_apply_lock_retries_on_the_next_poll() {
        let mut env = TestEnv::new().await;
        let (record, _deployed, blocker, _) = a_blocked_relocation(&mut env).await;
        std::fs::remove_file(&blocker).unwrap();
        env.docker.calls.lock().unwrap().clear();
        let target = ps_db::mod_targets::for_server(&*env.app.driver, record.id)
            .await
            .unwrap()
            .unwrap();
        let held = deploy::try_lock_target(&LibraryPaths::new(env._scratch.path()), &target)
            .expect("nothing else holds the target");

        list(&mut env).await;
        assert!(stored(&env, record.id).await.pending_relocation.is_some());
        assert!(calls(&env).is_empty(), "{:?}", calls(&env));

        drop(held);
        list(&mut env).await;
        assert_eq!(
            stored(&env, record.id).await.pending_relocation,
            None,
            "the skipped poll stamped no retry time, so the next one finishes it"
        );
        assert!(
            calls(&env).contains(&"create:alpha".to_string()),
            "{:?}",
            calls(&env)
        );
    }

    #[tokio::test]
    async fn a_relocations_restart_runs_under_the_servers_apply_lock() {
        let mut env = TestEnv::new().await;
        let (record, _deployed) = server_with_an_edited_mod(&mut env).await;
        env.docker.statuses.lock().unwrap().get_mut("alpha").unwrap()["State"] =
            serde_json::json!({"Status": "running", "Running": true, "StartedAt": "x"});
        let target = ps_db::mod_targets::for_server(&*env.app.driver, record.id)
            .await
            .unwrap()
            .unwrap();
        let library = LibraryPaths::new(env._scratch.path());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let (seen, probe_library, probe_target) =
            (observed.clone(), library.clone(), target.clone());
        *env.docker.on_create_or_start.lock().unwrap() = Some(Box::new(move |call| {
            let free = deploy::try_lock_target(&probe_library, &probe_target).is_some();
            seen.lock().unwrap().push((call.to_string(), free));
        }));
        let moved = moved_mods_dir(&env);

        let messages = update_mods_path(&mut env, record.id, &moved).await;

        let reply = data_of(&messages, "update_server");
        assert!(reply.get("error").is_none(), "{reply}");
        assert_eq!(reply["relocation_pending"], false);
        assert_eq!(
            *observed.lock().unwrap(),
            vec![
                ("create:alpha".to_string(), false),
                ("start:alpha".to_string(), false)
            ],
            "the target's apply lock was free while the relocation restarted the server"
        );
        assert!(deploy::try_lock_target(&library, &target).is_some());
    }

    #[tokio::test]
    async fn a_docker_settings_update_meeting_a_held_apply_lock_leaves_the_container_alone() {
        let mut env = TestEnv::new().await;
        let (record, _deployed) = server_with_an_edited_mod(&mut env).await;
        let target = ps_db::mod_targets::for_server(&*env.app.driver, record.id)
            .await
            .unwrap()
            .unwrap();
        let held = deploy::try_lock_target(&LibraryPaths::new(env._scratch.path()), &target)
            .expect("nothing else holds the target");
        let before = calls(&env).len();
        let mut updates = serde_json::Map::new();
        updates.insert("game_port".to_string(), serde_json::json!(8212));

        let messages = update(&mut env, record.id, updates).await;

        let reply = data_of(&messages, "update_server");
        assert_eq!(reply["error"]["code"], "apply_in_progress", "{reply}");
        assert!(calls(&env)[before..].is_empty(), "{:?}", calls(&env));
        drop(held);
    }

    #[tokio::test]
    async fn a_relocation_update_meeting_a_held_apply_lock_stays_pending() {
        let mut env = TestEnv::new().await;
        let (record, deployed) = server_with_an_edited_mod(&mut env).await;
        let target = ps_db::mod_targets::for_server(&*env.app.driver, record.id)
            .await
            .unwrap()
            .unwrap();
        let held = deploy::try_lock_target(&LibraryPaths::new(env._scratch.path()), &target)
            .expect("nothing else holds the target");
        let moved = moved_mods_dir(&env);

        let messages = update_mods_path(&mut env, record.id, &moved).await;

        let reply = data_of(&messages, "update_server");
        assert!(reply.get("error").is_none(), "{reply}");
        assert_eq!(reply["relocation_pending"], true);
        assert_eq!(reply["apply"]["error"]["code"], "apply_in_progress", "{reply}");
        assert!(
            !calls(&env)
                .iter()
                .any(|call| call.starts_with("create") || call.starts_with("start")),
            "{:?}",
            calls(&env)
        );
        assert!(stored(&env, record.id).await.pending_relocation.is_some());
        assert_eq!(std::fs::read(&deployed).unwrap(), b"the user's settings");
        drop(held);
    }

    #[tokio::test]
    async fn moving_a_native_install_moves_its_managed_mods_with_it() {
        let mut env = TestEnv::new().await;
        let db = env.app.driver.clone();
        let old_install = env._scratch.path().join("gamma-a");
        let new_install = env._scratch.path().join("gamma-b");
        let columns = [
            "Pal/Binaries/Win64/Mods",
            "Pal/Content/Paks/LogicMods",
            "Pal/Binaries/Win64/NativeMods",
            "Pal/Content/Paks/~mods",
        ];
        let under = |rel: &str| old_install.join(rel).to_string_lossy().into_owned();
        let mut new_server = docker_new_server("gamma");
        new_server.server_type = "native".to_string();
        new_server.install_path = old_install.to_string_lossy().into_owned();
        new_server.mods_path = under(columns[0]);
        new_server.logicmods_path = under(columns[1]);
        new_server.nativemods_path = under(columns[2]);
        new_server.paks_path = under(columns[3]);
        let record = ps_db::servers::create_server(&*db, new_server).await.unwrap();
        let target_id = crate::mod_target_service::ensure_for(&*db, &record, &env.services.app_root)
            .await
            .unwrap()
            .expect("a new server has no target yet");
        let library = LibraryPaths::new(env._scratch.path());
        store_cool_mod(&*db, &library, env._scratch.path()).await;
        ps_db::mod_profiles::set_mod(
            &*db,
            &ps_db::mod_profiles::ProfileModRow {
                profile_id: format!("{target_id}/default"),
                mod_id: "coolmod".to_string(),
                mod_version_id: None,
                enabled: true,
                load_order: 0,
            },
        )
        .await
        .unwrap();
        let target = ps_db::mod_targets::get(&*db, &target_id)
            .await
            .unwrap()
            .unwrap();
        let applied = crate::mods_handlers::run_apply(
            &env.services,
            &library,
            &*db,
            &env.emitter,
            &target,
            &[],
            true,
        )
        .await;
        assert!(!applied.is_null() && apply_closed_cleanly(&applied), "{applied}");
        let old_file = Path::new(&record.mods_path).join("CoolMod").join("config.lua");
        assert!(old_file.is_file(), "{old_file:?}");
        std::fs::write(&old_file, b"the user's settings").unwrap();
        env.drain();

        let mut updates = serde_json::Map::new();
        updates.insert(
            "install_path".to_string(),
            serde_json::json!(new_install.to_string_lossy()),
        );
        let messages = update(&mut env, record.id, updates).await;

        let reply = data_of(&messages, "update_server");
        assert!(reply.get("error").is_none(), "{reply}");
        assert_eq!(reply["relocation_pending"], false);
        assert_eq!(reply["apply"]["mid_apply"], false);
        assert!(reply["apply"].get("error").is_none(), "{reply}");
        let stored = stored(&env, record.id).await;
        assert_eq!(stored.pending_relocation, None);
        assert_eq!(Path::new(&stored.install_path), new_install.as_path());
        let moved_columns = [
            &stored.mods_path,
            &stored.logicmods_path,
            &stored.nativemods_path,
            &stored.paks_path,
        ];
        for (column, rel) in moved_columns.into_iter().zip(columns) {
            assert_eq!(Path::new(column), new_install.join(rel).as_path());
        }
        let new_file = new_install.join(columns[0]).join("CoolMod").join("config.lua");
        assert_eq!(std::fs::read(&new_file).unwrap(), b"the user's settings");
        assert!(!old_file.exists(), "{old_file:?}");
        let rows = ps_db::mod_deployments::files_of(&*db, &target_id).await.unwrap();
        assert!(
            rows.iter().any(|row| Path::new(&row.path) == new_file.as_path()),
            "{rows:?}"
        );
        for row in &rows {
            assert!(Path::new(&row.path).starts_with(&new_install), "{row:?}");
        }
        let moved_target = ps_db::mod_targets::get(&*db, &target_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(Path::new(&moved_target.root_path), new_install.as_path());
    }

    #[tokio::test]
    async fn two_concurrent_starts_recreate_a_pre_feature_container_once() {
        let env = TestEnv::new().await;
        let new_server = scratch_docker_server(&env, "alpha");
        let record = ps_db::servers::create_server(&*env.app.driver, new_server)
            .await
            .unwrap();
        env.docker.statuses.lock().unwrap().insert(
            "alpha".to_string(),
            serde_json::json!({
                "State": {"Status": "exited", "Running": false, "StartedAt": null},
                "HostConfig": {"Binds": pre_feature_binds()}
            }),
        );
        *env.docker.ensure_image_rendezvous.lock().unwrap() =
            Some(std::sync::Arc::new(tokio::sync::Barrier::new(2)));
        let services = env.services.clone();
        let library = LibraryPaths::new(env._scratch.path());
        let mut first_session = ps_core::session::Session::new();
        let mut second_session = ps_core::session::Session::new();
        let mut first_blueprints = crate::blueprint_registry::BlueprintRegistry::default();
        let mut second_blueprints = crate::blueprint_registry::BlueprintRegistry::default();
        let mut first = crate::dispatcher::HandlerCtx {
            session: &mut first_session,
            app: &env.app,
            emitter: &env.emitter,
            blueprints: &mut first_blueprints,
            is_loopback: false,
            mod_verification_subscribed: None,
            attachment: None,
        };
        let mut second = crate::dispatcher::HandlerCtx {
            session: &mut second_session,
            app: &env.app,
            emitter: &env.emitter,
            blueprints: &mut second_blueprints,
            is_loopback: false,
            mod_verification_subscribed: None,
            attachment: None,
        };

        let (first_result, second_result) = tokio::join!(
            handle_start_server(
                &services,
                &library,
                ServerIdData {
                    server_id: record.id
                },
                &mut first
            ),
            handle_start_server(
                &services,
                &library,
                ServerIdData {
                    server_id: record.id
                },
                &mut second
            ),
        );
        first_result.unwrap();
        second_result.unwrap();

        let calls = calls(&env);
        for prefix in ["stop:", "remove_container:", "create_and_start:"] {
            assert_eq!(
                calls.iter().filter(|call| call.starts_with(prefix)).count(),
                1,
                "{prefix} in {calls:?}"
            );
        }
    }

    async fn paks_mods_override(env: &TestEnv, target_id: &str) -> Option<String> {
        let target = ps_db::mod_targets::get(&*env.app.driver, target_id)
            .await
            .unwrap()
            .unwrap();
        let overrides: serde_json::Value = serde_json::from_str(&target.layout_overrides).unwrap();
        overrides["paks_mods_dir"].as_str().map(str::to_string)
    }

    #[tokio::test]
    async fn backfill_fills_an_empty_paks_path_and_the_target_override() {
        let env = TestEnv::new().await;
        let mut new_server = docker_new_server("alpha");
        new_server.paks_path = String::new();
        let mut record = ps_db::servers::create_server(&*env.app.driver, new_server)
            .await
            .unwrap();
        let target_id = crate::mod_target_service::ensure_for(&*env.app.driver, &record, &env.services.app_root)
            .await
            .unwrap()
            .expect("a new server has no target yet");
        assert_eq!(paks_mods_override(&env, &target_id).await, None);

        backfill_docker_paks_path(&env.services.app_root, &*env.app.driver, &mut record)
            .await
            .unwrap();

        let expected = docker_server_dir(&env.services.app_root, "alpha").join("paks");
        assert_eq!(Path::new(&record.paks_path), expected);
        let stored = ps_db::servers::get_server(&*env.app.driver, record.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(Path::new(&stored.paks_path), expected);
        let paks_mods_dir = paks_mods_override(&env, &target_id).await.unwrap();
        assert_eq!(Path::new(&paks_mods_dir), expected);
    }

    #[tokio::test]
    async fn start_rewrites_target_overrides_that_lack_paks_mods_dir() {
        let mut env = TestEnv::new().await;
        let new_server = scratch_docker_server(&env, "alpha");
        let record = ps_db::servers::create_server(&*env.app.driver, new_server)
            .await
            .unwrap();
        let target_id = crate::mod_target_service::ensure_for(&*env.app.driver, &record, &env.services.app_root)
            .await
            .unwrap()
            .expect("a new server has no target yet");
        ps_db::mod_targets::set_layout_overrides(
            &*env.app.driver,
            &target_id,
            r#"{"ue4ss_mods_dir":""}"#,
        )
        .await
        .unwrap();
        env.docker.statuses.lock().unwrap().insert(
            "alpha".to_string(),
            serde_json::json!({
                "State": {"Status": "exited", "Running": false, "StartedAt": null},
                "HostConfig": {"Binds": docker::build_binds(&record)}
            }),
        );

        start(&mut env, record.id).await;

        let paks_mods_dir = paks_mods_override(&env, &target_id).await.unwrap();
        assert_eq!(Path::new(&paks_mods_dir), Path::new(&record.paks_path));
        let calls = env.docker.calls.lock().unwrap().clone();
        assert_eq!(calls, vec!["start:alpha"]);
    }

    #[tokio::test]
    async fn listing_flags_only_containers_that_start_would_recreate() {
        let mut env = TestEnv::new().await;
        let db = env.app.driver.clone();
        let exited = serde_json::json!({"Status": "exited", "Running": false, "StartedAt": null});
        let mut bound_binds = pre_feature_binds();
        bound_binds
            .as_array_mut()
            .unwrap()
            .push("/srv/bound/paks:/palworld/Pal/Content/Paks/~mods/:rw".into());
        for (name, binds) in [
            ("stale", Some(pre_feature_binds())),
            ("bound", Some(bound_binds)),
            ("missing", None),
            ("broken", None),
            ("moving", Some(pre_feature_binds())),
            ("legacy", Some(pre_feature_binds())),
            ("native", None),
        ] {
            let mut new_server = docker_new_server(name);
            if name == "legacy" {
                new_server.paks_path = String::new();
            }
            if name == "native" {
                new_server.server_type = "native".to_string();
            }
            let record = ps_db::servers::create_server(&*db, new_server)
                .await
                .unwrap();
            if name == "moving" {
                crate::mod_target_service::ensure_for(&*db, &record, &env.services.app_root)
                    .await
                    .unwrap();
                set_pending_relocation(&env, record.id).await;
            }
            if let Some(binds) = binds {
                env.docker.statuses.lock().unwrap().insert(
                    name.to_string(),
                    serde_json::json!({"State": exited.clone(), "HostConfig": {"Binds": binds}}),
                );
            }
        }
        env.docker
            .fail_inspect
            .lock()
            .unwrap()
            .insert("broken".to_string());
        env.docker
            .fail_inspect
            .lock()
            .unwrap()
            .insert("moving".to_string());

        let services = env.services.clone();
        let library = LibraryPaths::new(env._scratch.path());
        let mut ctx = env.ctx();
        handle_list_servers(&services, &library, serde_json::Value::Null, &mut ctx)
            .await
            .unwrap();

        let messages = env.drain();
        let flags: std::collections::BTreeMap<String, serde_json::Value> = messages[0]["data"]
            ["servers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| {
                (
                    entry["container_name"].as_str().unwrap().to_string(),
                    entry["container_needs_recreate"].clone(),
                )
            })
            .collect();
        let expected: std::collections::BTreeMap<String, serde_json::Value> = [
            ("stale", true),
            ("bound", false),
            ("missing", false),
            ("broken", false),
            ("moving", false),
            ("legacy", true),
            ("native", false),
        ]
        .into_iter()
        .map(|(name, flag)| (name.to_string(), serde_json::json!(flag)))
        .collect();
        assert_eq!(flags, expected);
        let legacy = ps_db::servers::list_servers(&*db)
            .await
            .unwrap()
            .into_iter()
            .find(|record| record.container_name == "legacy")
            .unwrap();
        assert!(legacy.paks_path.is_empty());
        assert!(env.docker.calls.lock().unwrap().is_empty());

        let stale_id = ps_db::servers::list_servers(&*db)
            .await
            .unwrap()
            .into_iter()
            .find(|record| record.container_name == "stale")
            .unwrap()
            .id;
        let mut ctx = env.ctx();
        handle_get_server(
            &services,
            ServerIdData {
                server_id: stale_id,
            },
            &mut ctx,
        )
        .await
        .unwrap();
        let messages = env.drain();
        assert_eq!(messages[0]["type"], "get_server");
        assert_eq!(messages[0]["data"]["container_needs_recreate"], true);
    }

    #[tokio::test]
    async fn stop_docker_server_emits_progress_then_status_update() {
        let mut env = TestEnv::new().await;
        let record = ps_db::servers::create_server(&*env.app.driver, docker_new_server("alpha"))
            .await
            .unwrap();
        env.docker.statuses.lock().unwrap().insert(
            "alpha".to_string(),
            serde_json::json!({"State": {"Status": "running", "Running": true, "StartedAt": "x"}}),
        );
        let services = env.services.clone();
        let mut ctx = env.ctx();
        handle_stop_server(
            &services,
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
        let record = ps_db::servers::create_server(&*env.app.driver, new_server)
            .await
            .unwrap();
        let services = env.services.clone();
        let mut ctx = env.ctx();
        handle_server_api_call(
            &services,
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
        let record = ps_db::servers::create_server(&*env.app.driver, new_server)
            .await
            .unwrap();
        let services = env.services.clone();
        let mut ctx = env.ctx();
        handle_server_api_call(
            &services,
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
    async fn load_server_save_requires_stopped_server() {
        let mut env = TestEnv::new().await;
        let record = ps_db::servers::create_server(&*env.app.driver, docker_new_server("running"))
            .await
            .unwrap();
        env.docker.statuses.lock().unwrap().insert(
            "running".to_string(),
            serde_json::json!({"State": {"Status": "running", "Running": true, "StartedAt": "x"}}),
        );
        let services = env.services.clone();
        let mut ctx = env.ctx();
        handle_load_server_save(
            &services,
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
        let record = ps_db::servers::create_server(&*env.app.driver, new_server)
            .await
            .unwrap();
        let services = env.services.clone();
        let mut ctx = env.ctx();
        handle_load_server_save(
            &services,
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

    /// Full load path against the committed `v1_relics` world save fixture
    /// (Level.sav + Players/). Never skips.
    #[tokio::test]
    async fn load_server_save_loads_world_and_emits_summaries() {
        let source_save_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/saves/v1_relics");
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
        let record = ps_db::servers::create_server(&*env.app.driver, new_server)
            .await
            .unwrap();
        // The full load path registers the loaded session in the store, which
        // needs a connection attachment; the real ws loop supplies one. Provide
        // a throwaway arc/id so the load can complete under a synthetic ctx.
        let mut session_arc: crate::SharedSession =
            std::sync::Arc::new(tokio::sync::Mutex::new(ps_core::session::Session::new()));
        let mut current_id: Option<uuid::Uuid> = None;
        let services = env.services.clone();
        let mut ctx = crate::dispatcher::HandlerCtx {
            session: &mut env.session,
            app: &env.app,
            emitter: &env.emitter,
            blueprints: &mut env.blueprints,
            is_loopback: false,
            mod_verification_subscribed: None,
            attachment: Some(crate::dispatcher::SessionAttachment {
                current_id: &mut current_id,
                arc: &mut session_arc,
            }),
        };
        handle_load_server_save(
            &services,
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

    fn write_importable_install(root: &std::path::Path, option_settings: &str) -> String {
        std::fs::create_dir_all(root).unwrap();
        std::fs::write(root.join("PalServer.exe"), b"x").unwrap();
        let cfg = root.join("Pal").join("Saved").join("Config").join("WindowsServer");
        std::fs::create_dir_all(&cfg).unwrap();
        std::fs::write(
            cfg.join("PalWorldSettings.ini"),
            format!("[/Script/Pal.PalGameWorldSettings]\nOptionSettings=({option_settings})\n"),
        )
        .unwrap();
        root.to_string_lossy().into_owned()
    }

    #[tokio::test]
    async fn import_server_inserts_native_row_non_destructively() {
        let mut env = TestEnv::new().await;
        let install_dir = env._scratch.path().join("MyServer");
        let install = write_importable_install(
            &install_dir,
            "ServerName=\"Imported\",PublicPort=9911,RESTAPIPort=9912,ServerPlayerMaxNum=20,ExpRate=2.000000,MyCustomKey=42",
        );

        let data = ImportServerData {
            install_path: install.clone(),
            name: String::new(),
            query_port: None,
            launch_args: None,
            workshop_dir: Some(String::new()),
        };
        let services = env.services.clone();
        let mut ctx = env.ctx();
        handle_import_server(&services, data, &mut ctx).await.unwrap();

        let messages = env.drain();
        let imported = messages
            .iter()
            .find(|m| m["type"] == "import_server")
            .expect("import_server frame");
        let d = &imported["data"];
        assert_eq!(d["server_type"], "native");
        assert_eq!(d["name"], "Imported");
        assert_eq!(d["server_name"], "Imported");
        assert_eq!(d["game_port"], 9911);
        assert_eq!(d["rest_api_port"], 9912);
        assert_eq!(d["max_players"], 20);
        assert!(d["pid"].is_null());
        assert_eq!(d["env_vars"]["EXP_RATE"], "2.000000");
        assert_eq!(d["notifications"], serde_json::json!([]));

        // Non-destructive: we did not rewrite the ini (custom key + original port intact).
        let ini = std::fs::read_to_string(
            install_dir
                .join("Pal").join("Saved").join("Config").join("WindowsServer")
                .join("PalWorldSettings.ini"),
        )
        .unwrap();
        assert!(ini.contains("MyCustomKey=42"));
        assert!(ini.contains("PublicPort=9911"));

        let listed = ps_db::servers::list_servers(&*env.app.driver).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].server_type, "native");
        assert!(listed[0].pid.is_none());
    }

    #[tokio::test]
    async fn import_server_missing_exe_errors() {
        let mut env = TestEnv::new().await;
        let empty = env._scratch.path().join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        let data = ImportServerData {
            install_path: empty.to_string_lossy().into_owned(),
            name: String::new(),
            query_port: None,
            launch_args: None,
            workshop_dir: Some(String::new()),
        };
        let services = env.services.clone();
        let mut ctx = env.ctx();
        handle_import_server(&services, data, &mut ctx).await.unwrap();
        let messages = env.drain();
        assert_eq!(messages[0]["type"], "error");
        assert_eq!(
            messages[0]["data"]["message"],
            "PalServer.exe not found in the selected folder"
        );
    }

    #[tokio::test]
    async fn import_server_duplicate_install_path_errors() {
        let mut env = TestEnv::new().await;
        let install_dir = env._scratch.path().join("Dup");
        let install = write_importable_install(&install_dir, "ServerName=\"Dup\",PublicPort=9921,RESTAPIPort=9922");
        let mut existing = docker_new_server("dup");
        existing.server_type = "native".to_string();
        existing.install_path = install.clone();
        ps_db::servers::create_server(&*env.app.driver, existing).await.unwrap();

        let data = ImportServerData {
            install_path: install.clone(),
            name: String::new(),
            query_port: None,
            launch_args: None,
            workshop_dir: Some(String::new()),
        };
        let services = env.services.clone();
        let mut ctx = env.ctx();
        handle_import_server(&services, data, &mut ctx).await.unwrap();
        let messages = env.drain();
        assert_eq!(messages[0]["type"], "error");
        assert_eq!(messages[0]["data"]["message"], "This server is already registered");
    }

    #[tokio::test]
    async fn import_reassigns_conflicting_ports_and_leaves_ini_untouched() {
        let mut env = TestEnv::new().await;
        let mut occupant = docker_new_server("occupant");
        occupant.game_port = 9911;
        occupant.query_port = 27015;
        occupant.rest_api_port = 9912;
        ps_db::servers::create_server(&*env.app.driver, occupant).await.unwrap();

        let install_dir = env._scratch.path().join("Conflict");
        let install = write_importable_install(
            &install_dir,
            "ServerName=\"Conflict\",PublicPort=9911,RESTAPIPort=9912,MyCustomKey=7",
        );
        let data = ImportServerData {
            install_path: install.clone(),
            name: String::new(),
            query_port: Some(27015), // also conflicts with occupant's query port
            launch_args: None,
            workshop_dir: Some(String::new()),
        };
        let services = env.services.clone();
        let mut ctx = env.ctx();
        handle_import_server(&services, data, &mut ctx).await.unwrap();

        let messages = env.drain();
        let d = &messages.iter().find(|m| m["type"] == "import_server").unwrap()["data"];
        assert_ne!(d["game_port"], 9911);
        assert_ne!(d["rest_api_port"], 9912);
        assert_ne!(d["query_port"], 27015);
        let notes = d["notifications"].as_array().unwrap();
        assert_eq!(notes.len(), 3);

        // The ini on disk still holds the ORIGINAL ports (import wrote nothing).
        let ini = std::fs::read_to_string(
            install_dir.join("Pal").join("Saved").join("Config").join("WindowsServer").join("PalWorldSettings.ini"),
        )
        .unwrap();
        assert!(ini.contains("PublicPort=9911"));
        assert!(ini.contains("MyCustomKey=7"));
    }

    #[tokio::test]
    async fn import_resolves_select_sentinel_via_folder_dialog() {
        let scratch = tempfile::tempdir().unwrap();
        let install_dir = scratch.path().join("Picked");
        let install = write_importable_install(
            &install_dir,
            "ServerName=\"Picked\",PublicPort=9931,RESTAPIPort=9932",
        );

        let mut env = TestEnv::new_desktop_with_folders(vec![Some(install_dir.clone())]).await;
        let data = ImportServerData {
            install_path: "__select__".to_string(),
            name: "Chosen Name".to_string(),
            query_port: None,
            launch_args: None,
            workshop_dir: Some(String::new()),
        };
        let services = env.services.clone();
        let mut ctx = env.ctx();
        handle_import_server(&services, data, &mut ctx).await.unwrap();

        let messages = env.drain();
        let d = &messages.iter().find(|m| m["type"] == "import_server").unwrap()["data"];
        assert_eq!(d["install_path"], install);
        assert_eq!(d["name"], "Chosen Name");
        assert_eq!(d["game_port"], 9931);
    }

    #[test]
    fn reassign_import_ports_leaves_free_ports_unchanged() {
        let allocated: std::collections::HashSet<u16> = [8211u16].into_iter().collect();
        let ((game, query, rest), notes) = reassign_import_ports(9000, 9001, 9002, &allocated);
        assert_eq!((game, query, rest), (9000, 9001, 9002));
        assert!(notes.is_empty());
    }

    #[test]
    fn public_projection_carries_no_secrets_or_paths() {
        let mut env_vars = serde_json::Map::new();
        env_vars.insert("SECRET_KEY".to_string(), serde_json::Value::String("secret_value".to_string()));

        let record = ServerRecord {
            id: 42,
            name: "test-server".to_string(),
            container_name: "test-container".to_string(),
            image_name: "test-image".to_string(),
            server_type: "docker".to_string(),
            game_port: 8211,
            query_port: 27015,
            rest_api_port: 8212,
            data_volume_name: "test-volume".to_string(),
            saves_path: "C:/Users/serveradmin/Saved/SaveGames".to_string(),
            mods_path: "C:/srv/mods".to_string(),
            logicmods_path: "C:/srv/logicmods".to_string(),
            nativemods_path: "C:/srv/nativemods".to_string(),
            paks_path: "C:/srv/paks".to_string(),
            install_path: "C:/srv".to_string(),
            steamcmd_path: "/usr/steamcmd".to_string(),
            pid: Some(1234),
            launch_args: "-arg1 -arg2".to_string(),
            workshop_dir: "/workshop".to_string(),
            server_name: "My Server".to_string(),
            server_description: "Test server".to_string(),
            server_password: "hunter2".to_string(),
            admin_password: "hunter2".to_string(),
            max_players: 32,
            env_vars,
            pending_relocation: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-02T00:00:00Z".to_string(),
        };

        let v = server_public_wire_json(&record);
        let s = v.to_string();

        for banned in &["admin_password", "server_password", "hunter2", "install_path",
                       "C:/srv", "C:/Users", "serveradmin", "env_vars", "launch_args", "pid",
                       "steamcmd", "workshop", "saves_path", "mods_path", "logicmods_path",
                       "nativemods_path", "SECRET_KEY", "secret_value"] {
            assert!(!s.contains(banned), "leaked: {}", banned);
        }

        assert!(v.get("name").is_some());
        assert!(v.get("server_type").is_some());
        assert!(v.get("server_name").is_some());
        assert_eq!(v["id"], 42);
        assert_eq!(v["max_players"], 32);
    }
}
