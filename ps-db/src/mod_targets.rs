use crate::error::DbError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModTarget {
    pub id: String,
    pub kind: String,
    pub server_id: Option<i64>,
    pub name: String,
    pub root_path: String,
    pub platform: String,
    pub ue4ss_mode: String,
    pub layout_overrides: String,
    pub detected: String,
    pub last_scanned_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default)]
pub struct NewModTarget {
    pub id: String,
    pub kind: String,
    pub server_id: Option<i64>,
    pub name: String,
    pub root_path: String,
    pub platform: String,
    pub ue4ss_mode: String,
    pub layout_overrides: String,
    pub detected: String,
}

const SELECT_COLUMNS: &str = "id, kind, server_id, name, root_path, platform, ue4ss_mode, \
                              layout_overrides, detected, last_scanned_at, created_at, updated_at";

fn map_target(r: &crate::DbRow) -> Result<ModTarget, DbError> {
    Ok(ModTarget {
        id: r.get_string("id")?,
        kind: r.get_string("kind")?,
        server_id: r.get_opt_i64("server_id")?,
        name: r.get_string("name")?,
        root_path: r.get_string("root_path")?,
        platform: r.get_string("platform")?,
        ue4ss_mode: r.get_string("ue4ss_mode")?,
        layout_overrides: r.get_string("layout_overrides")?,
        detected: r.get_string("detected")?,
        last_scanned_at: r.get_opt_str("last_scanned_at")?,
        created_at: r.get_string("created_at")?,
        updated_at: r.get_string("updated_at")?,
    })
}

/// Inserts, or replaces every field except `created_at` and the scan stamp.
pub async fn upsert(db: &dyn crate::DbDriver, target: &NewModTarget) -> Result<ModTarget, DbError> {
    let now = crate::time::now_iso_naive_utc();
    db.execute(
        "INSERT INTO mod_targets (id, kind, server_id, name, root_path, platform, ue4ss_mode, \
         layout_overrides, detected, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10) \
         ON CONFLICT(id) DO UPDATE SET kind = ?2, server_id = ?3, name = ?4, root_path = ?5, \
         platform = ?6, ue4ss_mode = ?7, layout_overrides = ?8, detected = ?9, updated_at = ?10",
        &[
            target.id.as_str().into(),
            target.kind.as_str().into(),
            target.server_id.into(),
            target.name.as_str().into(),
            target.root_path.as_str().into(),
            target.platform.as_str().into(),
            target.ue4ss_mode.as_str().into(),
            target.layout_overrides.as_str().into(),
            target.detected.as_str().into(),
            now.as_str().into(),
        ],
    )
    .await?;
    get(db, &target.id)
        .await?
        .ok_or_else(|| DbError::Other(format!("mod target {} vanished after write", target.id)))
}

pub async fn get(db: &dyn crate::DbDriver, id: &str) -> Result<Option<ModTarget>, DbError> {
    let rows = db
        .query(
            &format!("SELECT {SELECT_COLUMNS} FROM mod_targets WHERE id = ?1"),
            &[id.into()],
        )
        .await?;
    rows.first().map(map_target).transpose()
}

pub async fn list(db: &dyn crate::DbDriver) -> Result<Vec<ModTarget>, DbError> {
    let rows = db
        .query(
            &format!("SELECT {SELECT_COLUMNS} FROM mod_targets ORDER BY id ASC"),
            &[],
        )
        .await?;
    rows.iter().map(map_target).collect()
}

pub async fn for_server(
    db: &dyn crate::DbDriver,
    server_id: i64,
) -> Result<Option<ModTarget>, DbError> {
    let rows = db
        .query(
            &format!("SELECT {SELECT_COLUMNS} FROM mod_targets WHERE server_id = ?1"),
            &[server_id.into()],
        )
        .await?;
    rows.first().map(map_target).transpose()
}

async fn set_json_column(
    db: &dyn crate::DbDriver,
    sql: &str,
    id: &str,
    value: &str,
) -> Result<(), DbError> {
    db.execute(
        sql,
        &[
            value.into(),
            crate::time::now_iso_naive_utc().as_str().into(),
            id.into(),
        ],
    )
    .await?;
    Ok(())
}

pub async fn set_layout_overrides(
    db: &dyn crate::DbDriver,
    id: &str,
    json: &str,
) -> Result<(), DbError> {
    set_json_column(
        db,
        "UPDATE mod_targets SET layout_overrides = ?1, updated_at = ?2 WHERE id = ?3",
        id,
        json,
    )
    .await
}

pub async fn set_detected(db: &dyn crate::DbDriver, id: &str, json: &str) -> Result<(), DbError> {
    set_json_column(
        db,
        "UPDATE mod_targets SET detected = ?1, updated_at = ?2 WHERE id = ?3",
        id,
        json,
    )
    .await
}

pub async fn mark_scanned(db: &dyn crate::DbDriver, id: &str) -> Result<(), DbError> {
    let now = crate::time::now_iso_naive_utc();
    db.execute(
        "UPDATE mod_targets SET last_scanned_at = ?1, updated_at = ?1 WHERE id = ?2",
        &[now.as_str().into(), id.into()],
    )
    .await?;
    Ok(())
}

pub async fn set_ue4ss_mode(db: &dyn crate::DbDriver, id: &str, mode: &str) -> Result<(), DbError> {
    set_json_column(
        db,
        "UPDATE mod_targets SET ue4ss_mode = ?1, updated_at = ?2 WHERE id = ?3",
        id,
        mode,
    )
    .await
}

/// Deletes the dependants explicitly, deepest first. The schema's
/// `ON DELETE CASCADE` fires only where `PRAGMA foreign_keys` is on, which the
/// sqlx driver sets and the browser's OPFS driver does not, so relying on it
/// would orphan every row below in the browser.
pub async fn remove(db: &dyn crate::DbDriver, id: &str) -> Result<bool, DbError> {
    db.execute_batch(&[
        (
            "DELETE FROM world_profiles WHERE profile_id IN \
             (SELECT id FROM profiles WHERE target_id = ?1)",
            vec![id.into()],
        ),
        (
            "DELETE FROM profile_mods WHERE profile_id IN \
             (SELECT id FROM profiles WHERE target_id = ?1)",
            vec![id.into()],
        ),
        ("DELETE FROM profiles WHERE target_id = ?1", vec![id.into()]),
        (
            "DELETE FROM target_frameworks WHERE target_id = ?1",
            vec![id.into()],
        ),
        (
            "DELETE FROM deployment_files WHERE target_id = ?1",
            vec![id.into()],
        ),
        (
            "DELETE FROM apply_journal WHERE target_id = ?1",
            vec![id.into()],
        ),
        (
            "UPDATE amity_instances SET target_id = NULL WHERE target_id = ?1",
            vec![id.into()],
        ),
    ])
    .await?;
    let affected = db
        .execute("DELETE FROM mod_targets WHERE id = ?1", &[id.into()])
        .await?;
    Ok(affected > 0)
}

/// The server's stored mod directories, which are what its Docker binds mount, so
/// the resolver and the mounts cannot diverge. `paks_mods_dir` is omitted when
/// `paks_path` is empty: a server predating that column has no `~mods` bind yet,
/// and an empty override would resolve worse than no override.
pub fn server_layout_overrides(server: &crate::servers::ServerRecord) -> String {
    let mut overrides = serde_json::Map::new();
    overrides.insert(
        "ue4ss_mods_dir".to_string(),
        server.mods_path.clone().into(),
    );
    overrides.insert(
        "logicmods_dir".to_string(),
        server.logicmods_path.clone().into(),
    );
    overrides.insert(
        "nativemods_dir".to_string(),
        server.nativemods_path.clone().into(),
    );
    if !server.paks_path.is_empty() {
        overrides.insert("paks_mods_dir".to_string(), server.paks_path.clone().into());
    }
    serde_json::Value::Object(overrides).to_string()
}

/// Re-derives an existing server target's overrides from the server record, and
/// a native target's root from its install path. Returns whether the server has
/// a target at all.
pub async fn rewrite_server_overrides(
    db: &dyn crate::DbDriver,
    server: &crate::servers::ServerRecord,
) -> Result<bool, DbError> {
    let id = format!("server-{}", server.id);
    let now = crate::time::now_iso_naive_utc();
    let overrides = server_layout_overrides(server);
    let affected = if server.server_type == "native" {
        db.execute(
            "UPDATE mod_targets SET layout_overrides = ?1, root_path = ?2, updated_at = ?3 \
             WHERE id = ?4",
            &[
                overrides.as_str().into(),
                server.install_path.as_str().into(),
                now.as_str().into(),
                id.as_str().into(),
            ],
        )
        .await?
    } else {
        db.execute(
            "UPDATE mod_targets SET layout_overrides = ?1, updated_at = ?2 WHERE id = ?3",
            &[
                overrides.as_str().into(),
                now.as_str().into(),
                id.as_str().into(),
            ],
        )
        .await?
    };
    Ok(affected > 0)
}

/// Gives every `servers` row a target and a default, active profile. This function
/// never rewrites an existing target; a server target's overrides derive from
/// the server record and are rewritten by whoever changes the record's paths. A
/// target whose default profile is missing gets only the profile, so
/// a pair half-built by an earlier crash completes on the next call. Safe to call
/// on each startup. `docker_servers_root` is the app's servers directory as a
/// string, because this crate compiles for wasm and has no path type.
pub async fn ensure_server_target(
    db: &dyn crate::DbDriver,
    server: &crate::servers::ServerRecord,
    docker_servers_root: &str,
) -> Result<Option<String>, DbError> {
    let id = format!("server-{}", server.id);
    let had_target = get(db, &id).await?.is_some();
    let had_profile = crate::mod_profiles::default_for_target(db, &id)
        .await?
        .is_some();
    if had_target && had_profile {
        return Ok(None);
    }
    if !had_target {
        let native = server.server_type == "native";
        let root_path = if native {
            server.install_path.clone()
        } else {
            std::path::Path::new(docker_servers_root)
                .join(&server.container_name)
                .to_string_lossy()
                .into_owned()
        };
        upsert(
            db,
            &NewModTarget {
                id: id.clone(),
                kind: "server".to_string(),
                server_id: Some(server.id),
                name: server.name.clone(),
                root_path,
                platform: if native { "win64" } else { "linux" }.to_string(),
                ue4ss_mode: "none".to_string(),
                layout_overrides: server_layout_overrides(server),
                detected: "{}".to_string(),
            },
        )
        .await?;
    }
    if !had_profile {
        crate::mod_profiles::create(
            db,
            &crate::mod_profiles::NewProfile {
                id: format!("{id}/default"),
                target_id: id.clone(),
                name: "Default".to_string(),
                is_default: true,
            },
        )
        .await?;
    }
    Ok(Some(id))
}

pub async fn ensure_server_targets(
    db: &dyn crate::DbDriver,
    docker_servers_root: &str,
) -> Result<Vec<String>, DbError> {
    let mut created = Vec::new();
    for server in crate::servers::list_servers(db).await? {
        if let Some(id) = ensure_server_target(db, &server, docker_servers_root).await? {
            created.push(id);
        }
    }
    Ok(created)
}
