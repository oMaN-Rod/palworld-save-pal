use std::collections::HashSet;

use serde_json::{Map, Value};
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};

use crate::error::DbError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ServerRecord {
    pub id: i64,
    pub name: String,
    pub container_name: String,
    pub image_name: String,
    pub server_type: String,
    pub game_port: i64,
    pub query_port: i64,
    pub rest_api_port: i64,
    pub data_volume_name: String,
    pub saves_path: String,
    pub mods_path: String,
    pub logicmods_path: String,
    pub nativemods_path: String,
    pub install_path: String,
    pub steamcmd_path: String,
    pub pid: Option<i64>,
    pub launch_args: String,
    pub workshop_dir: String,
    pub server_name: String,
    pub server_description: String,
    pub server_password: String,
    pub admin_password: String,
    pub max_players: i64,
    pub env_vars: sqlx::types::Json<Map<String, Value>>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default)]
pub struct NewServer {
    pub name: String,
    pub container_name: String,
    pub image_name: String,
    pub server_type: String,
    pub game_port: i64,
    pub query_port: i64,
    pub rest_api_port: i64,
    pub data_volume_name: String,
    pub saves_path: String,
    pub mods_path: String,
    pub logicmods_path: String,
    pub nativemods_path: String,
    pub install_path: String,
    pub steamcmd_path: String,
    pub launch_args: String,
    pub workshop_dir: String,
    pub server_name: String,
    pub server_description: String,
    pub server_password: String,
    pub admin_password: String,
    pub max_players: i64,
    pub env_vars: Map<String, Value>,
}

const SELECT_COLUMNS: &str = "id, name, container_name, image_name, server_type, game_port, \
    query_port, rest_api_port, data_volume_name, saves_path, mods_path, logicmods_path, \
    nativemods_path, install_path, steamcmd_path, pid, launch_args, workshop_dir, server_name, \
    server_description, server_password, admin_password, max_players, env_vars, created_at, \
    updated_at";

/// `update_server` interpolates update keys straight into SQL, so they must be
/// checked against this whitelist first.
const UPDATABLE_COLUMNS: &[&str] = &[
    "name",
    "container_name",
    "image_name",
    "server_type",
    "game_port",
    "query_port",
    "rest_api_port",
    "data_volume_name",
    "saves_path",
    "mods_path",
    "logicmods_path",
    "nativemods_path",
    "install_path",
    "steamcmd_path",
    "pid",
    "launch_args",
    "workshop_dir",
    "server_name",
    "server_description",
    "server_password",
    "admin_password",
    "max_players",
    "env_vars",
];

pub async fn create_server(
    pool: &SqlitePool,
    new_server: NewServer,
) -> Result<ServerRecord, DbError> {
    let now = crate::time::now_iso_naive_utc();
    let env_vars_text = Value::Object(new_server.env_vars).to_string();
    let inserted = sqlx::query(
        "INSERT INTO servers (name, container_name, image_name, server_type, game_port, \
         query_port, rest_api_port, data_volume_name, saves_path, mods_path, logicmods_path, \
         nativemods_path, install_path, steamcmd_path, pid, launch_args, workshop_dir, \
         server_name, server_description, server_password, admin_password, max_players, \
         env_vars, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&new_server.name)
    .bind(&new_server.container_name)
    .bind(&new_server.image_name)
    .bind(&new_server.server_type)
    .bind(new_server.game_port)
    .bind(new_server.query_port)
    .bind(new_server.rest_api_port)
    .bind(&new_server.data_volume_name)
    .bind(&new_server.saves_path)
    .bind(&new_server.mods_path)
    .bind(&new_server.logicmods_path)
    .bind(&new_server.nativemods_path)
    .bind(&new_server.install_path)
    .bind(&new_server.steamcmd_path)
    .bind(&new_server.launch_args)
    .bind(&new_server.workshop_dir)
    .bind(&new_server.server_name)
    .bind(&new_server.server_description)
    .bind(&new_server.server_password)
    .bind(&new_server.admin_password)
    .bind(new_server.max_players)
    .bind(env_vars_text)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    let server_id = inserted.last_insert_rowid();
    get_server(pool, server_id)
        .await?
        .ok_or_else(|| DbError::Other(format!("server {server_id} vanished after insert")))
}

pub async fn get_server(
    pool: &SqlitePool,
    server_id: i64,
) -> Result<Option<ServerRecord>, DbError> {
    let record = sqlx::query_as::<_, ServerRecord>(&format!(
        "SELECT {SELECT_COLUMNS} FROM servers WHERE id = ?"
    ))
    .bind(server_id)
    .fetch_optional(pool)
    .await?;
    Ok(record)
}

pub async fn server_with_install_path(
    pool: &SqlitePool,
    install_path: &str,
) -> Result<Option<ServerRecord>, DbError> {
    let record = sqlx::query_as::<_, ServerRecord>(&format!(
        "SELECT {SELECT_COLUMNS} FROM servers WHERE install_path = ?"
    ))
    .bind(install_path)
    .fetch_optional(pool)
    .await?;
    Ok(record)
}

pub async fn list_servers(pool: &SqlitePool) -> Result<Vec<ServerRecord>, DbError> {
    let records = sqlx::query_as::<_, ServerRecord>(&format!(
        "SELECT {SELECT_COLUMNS} FROM servers ORDER BY created_at"
    ))
    .fetch_all(pool)
    .await?;
    Ok(records)
}

pub async fn update_server(
    pool: &SqlitePool,
    server_id: i64,
    updates: &Map<String, Value>,
) -> Result<Option<ServerRecord>, DbError> {
    if get_server(pool, server_id).await?.is_none() {
        return Ok(None);
    }
    let mut builder = QueryBuilder::<Sqlite>::new("UPDATE servers SET updated_at = ");
    builder.push_bind(crate::time::now_iso_naive_utc());
    for (key, value) in updates {
        if !UPDATABLE_COLUMNS.contains(&key.as_str()) {
            continue;
        }
        builder.push(format!(", {key} = "));
        match value {
            Value::Null => {
                builder.push_bind(Option::<String>::None);
            }
            Value::Bool(flag) => {
                builder.push_bind(*flag);
            }
            Value::Number(number) if number.is_i64() => {
                builder.push_bind(number.as_i64().unwrap());
            }
            Value::Number(number) => {
                builder.push_bind(number.as_f64().unwrap_or(0.0));
            }
            Value::String(text) => {
                builder.push_bind(text.clone());
            }
            json_value => {
                // objects/arrays (env_vars) stored as JSON text
                builder.push_bind(json_value.to_string());
            }
        }
    }
    builder.push(" WHERE id = ");
    builder.push_bind(server_id);
    builder.build().execute(pool).await?;
    get_server(pool, server_id).await
}

pub async fn delete_server(pool: &SqlitePool, server_id: i64) -> Result<bool, DbError> {
    let result = sqlx::query("DELETE FROM servers WHERE id = ?")
        .bind(server_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn allocated_ports(pool: &SqlitePool) -> Result<HashSet<u16>, DbError> {
    let rows = sqlx::query("SELECT game_port, query_port, rest_api_port FROM servers")
        .fetch_all(pool)
        .await?;
    let mut ports = HashSet::new();
    for row in rows {
        for column in ["game_port", "query_port", "rest_api_port"] {
            let port: i64 = row.get(column);
            ports.insert(port as u16);
        }
    }
    Ok(ports)
}
