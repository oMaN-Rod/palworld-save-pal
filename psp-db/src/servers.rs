use std::collections::HashSet;

use serde_json::{Map, Value};

use crate::error::DbError;

#[derive(Debug, Clone)]
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
    pub env_vars: Map<String, Value>,
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

fn map_server(r: &crate::DbRow) -> Result<ServerRecord, DbError> {
    Ok(ServerRecord {
        id: r.get_i64("id")?,
        name: r.get_string("name")?,
        container_name: r.get_string("container_name")?,
        image_name: r.get_string("image_name")?,
        server_type: r.get_string("server_type")?,
        game_port: r.get_i64("game_port")?,
        query_port: r.get_i64("query_port")?,
        rest_api_port: r.get_i64("rest_api_port")?,
        data_volume_name: r.get_string("data_volume_name")?,
        saves_path: r.get_string("saves_path")?,
        mods_path: r.get_string("mods_path")?,
        logicmods_path: r.get_string("logicmods_path")?,
        nativemods_path: r.get_string("nativemods_path")?,
        install_path: r.get_string("install_path")?,
        steamcmd_path: r.get_string("steamcmd_path")?,
        pid: r.get_opt_i64("pid")?,
        launch_args: r.get_string("launch_args")?,
        workshop_dir: r.get_string("workshop_dir")?,
        server_name: r.get_string("server_name")?,
        server_description: r.get_string("server_description")?,
        server_password: r.get_string("server_password")?,
        admin_password: r.get_string("admin_password")?,
        max_players: r.get_i64("max_players")?,
        env_vars: match r.get_json("env_vars")? {
            Value::Object(m) => m,
            _ => Map::new(),
        },
        created_at: r.get_string("created_at")?,
        updated_at: r.get_string("updated_at")?,
    })
}

pub async fn create_server(
    db: &dyn crate::DbDriver,
    new_server: NewServer,
) -> Result<ServerRecord, DbError> {
    let now = crate::time::now_iso_naive_utc();
    let env_vars_text = Value::Object(new_server.env_vars).to_string();
    let server_id = db
        .query(
            "INSERT INTO servers (name, container_name, image_name, server_type, game_port, \
             query_port, rest_api_port, data_volume_name, saves_path, mods_path, logicmods_path, \
             nativemods_path, install_path, steamcmd_path, pid, launch_args, workshop_dir, \
             server_name, server_description, server_password, admin_password, max_players, \
             env_vars, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
             RETURNING id",
            &[
                new_server.name.clone().into(),
                new_server.container_name.clone().into(),
                new_server.image_name.clone().into(),
                new_server.server_type.clone().into(),
                new_server.game_port.into(),
                new_server.query_port.into(),
                new_server.rest_api_port.into(),
                new_server.data_volume_name.clone().into(),
                new_server.saves_path.clone().into(),
                new_server.mods_path.clone().into(),
                new_server.logicmods_path.clone().into(),
                new_server.nativemods_path.clone().into(),
                new_server.install_path.clone().into(),
                new_server.steamcmd_path.clone().into(),
                new_server.launch_args.clone().into(),
                new_server.workshop_dir.clone().into(),
                new_server.server_name.clone().into(),
                new_server.server_description.clone().into(),
                new_server.server_password.clone().into(),
                new_server.admin_password.clone().into(),
                new_server.max_players.into(),
                env_vars_text.into(),
                now.clone().into(),
                now.clone().into(),
            ],
        )
        .await?[0]
        .get_i64_at(0)?;
    get_server(db, server_id)
        .await?
        .ok_or_else(|| DbError::Other(format!("server {server_id} vanished after insert")))
}

pub async fn get_server(
    db: &dyn crate::DbDriver,
    server_id: i64,
) -> Result<Option<ServerRecord>, DbError> {
    let rows = db
        .query(
            &format!("SELECT {SELECT_COLUMNS} FROM servers WHERE id = ?"),
            &[server_id.into()],
        )
        .await?;
    rows.first().map(map_server).transpose()
}

pub async fn server_with_install_path(
    db: &dyn crate::DbDriver,
    install_path: &str,
) -> Result<Option<ServerRecord>, DbError> {
    let rows = db
        .query(
            &format!("SELECT {SELECT_COLUMNS} FROM servers WHERE install_path = ?"),
            &[install_path.into()],
        )
        .await?;
    rows.first().map(map_server).transpose()
}

pub async fn list_servers(db: &dyn crate::DbDriver) -> Result<Vec<ServerRecord>, DbError> {
    let rows = db
        .query(
            &format!("SELECT {SELECT_COLUMNS} FROM servers ORDER BY created_at"),
            &[],
        )
        .await?;
    rows.iter().map(map_server).collect()
}

pub async fn update_server(
    db: &dyn crate::DbDriver,
    server_id: i64,
    updates: &Map<String, Value>,
) -> Result<Option<ServerRecord>, DbError> {
    if get_server(db, server_id).await?.is_none() {
        return Ok(None);
    }
    let mut builder = crate::SqlBuilder::new("UPDATE servers SET updated_at = ");
    builder.push_bind(crate::time::now_iso_naive_utc());
    for (key, value) in updates {
        if !UPDATABLE_COLUMNS.contains(&key.as_str()) {
            continue;
        }
        builder.push(&format!(", {key} = "));
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
    let (sql, params) = builder.into_parts();
    db.execute(&sql, &params).await?;
    get_server(db, server_id).await
}

pub async fn delete_server(db: &dyn crate::DbDriver, server_id: i64) -> Result<bool, DbError> {
    let n = db
        .execute("DELETE FROM servers WHERE id = ?", &[server_id.into()])
        .await?;
    Ok(n > 0)
}

pub async fn allocated_ports(db: &dyn crate::DbDriver) -> Result<HashSet<u16>, DbError> {
    let rows = db
        .query(
            "SELECT game_port, query_port, rest_api_port FROM servers",
            &[],
        )
        .await?;
    let mut ports = HashSet::new();
    for row in &rows {
        for column in ["game_port", "query_port", "rest_api_port"] {
            ports.insert(row.get_i64(column)? as u16);
        }
    }
    Ok(ports)
}
