use crate::error::DbError;

#[derive(Debug, Clone, PartialEq)]
pub struct PluginRow {
    pub id: String,
    pub manifest: String,
    pub sources: String,
    pub enabled: bool,
    pub granted_capabilities: String,
    pub bundled: bool,
    pub installed_at: String,
    pub updated_at: String,
}

pub struct NewPlugin<'a> {
    pub id: &'a str,
    pub manifest: &'a str,
    pub sources: &'a str,
    pub granted_capabilities: &'a str,
    pub bundled: bool,
}

const SELECT_ALL: &str = "SELECT id, manifest, sources, enabled, granted_capabilities, bundled, \
                          installed_at, updated_at FROM plugins ORDER BY id ASC";
const SELECT_ONE: &str = "SELECT id, manifest, sources, enabled, granted_capabilities, bundled, \
                          installed_at, updated_at FROM plugins WHERE id = ?1";

fn map_plugin(r: &crate::DbRow) -> Result<PluginRow, DbError> {
    Ok(PluginRow {
        id: r.get_string("id")?,
        manifest: r.get_string("manifest")?,
        sources: r.get_string("sources")?,
        enabled: r.get_bool("enabled")?,
        granted_capabilities: r.get_string("granted_capabilities")?,
        bundled: r.get_bool("bundled")?,
        installed_at: r.get_string("installed_at")?,
        updated_at: r.get_string("updated_at")?,
    })
}

pub async fn get_all(db: &dyn crate::DbDriver) -> Result<Vec<PluginRow>, DbError> {
    let rows = db.query(SELECT_ALL, &[]).await?;
    rows.iter().map(map_plugin).collect()
}

pub async fn get(db: &dyn crate::DbDriver, id: &str) -> Result<Option<PluginRow>, DbError> {
    let rows = db.query(SELECT_ONE, &[id.into()]).await?;
    rows.first().map(map_plugin).transpose()
}

/// Replaces every mutable field of an existing row, or creates it. Used for a user
/// install: `granted_capabilities` is overwritten, unlike `seed_bundled`.
pub async fn upsert(db: &dyn crate::DbDriver, plugin: &NewPlugin<'_>) -> Result<PluginRow, DbError> {
    let installed_at = crate::time::now_iso_naive_utc();
    let updated_at = crate::time::now_iso_utc_offset();
    db.execute(
        "INSERT INTO plugins (id, manifest, sources, granted_capabilities, bundled, installed_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
         ON CONFLICT(id) DO UPDATE SET manifest = ?2, sources = ?3, granted_capabilities = ?4, \
         bundled = ?5, updated_at = ?7",
        &[
            plugin.id.into(),
            plugin.manifest.into(),
            plugin.sources.into(),
            plugin.granted_capabilities.into(),
            plugin.bundled.into(),
            installed_at.into(),
            updated_at.into(),
        ],
    ).await?;
    fetch_or_err(db, plugin.id).await
}

/// Refreshes `manifest`, `sources`, `bundled` and `updated_at` from the binary on
/// startup, leaving `enabled` and `granted_capabilities` untouched so a user's
/// choices survive an app update that ships new bundled scripts.
pub async fn seed_bundled(db: &dyn crate::DbDriver, plugin: &NewPlugin<'_>) -> Result<PluginRow, DbError> {
    let installed_at = crate::time::now_iso_naive_utc();
    let updated_at = crate::time::now_iso_utc_offset();
    db.execute(
        "INSERT INTO plugins (id, manifest, sources, granted_capabilities, bundled, installed_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
         ON CONFLICT(id) DO UPDATE SET manifest = ?2, sources = ?3, bundled = ?5, updated_at = ?7",
        &[
            plugin.id.into(),
            plugin.manifest.into(),
            plugin.sources.into(),
            plugin.granted_capabilities.into(),
            plugin.bundled.into(),
            installed_at.into(),
            updated_at.into(),
        ],
    ).await?;
    fetch_or_err(db, plugin.id).await
}

async fn fetch_or_err(db: &dyn crate::DbDriver, id: &str) -> Result<PluginRow, DbError> {
    get(db, id)
        .await?
        .ok_or_else(|| DbError::Backend(format!("no plugin row `{id}` after write")))
}

pub async fn set_enabled(db: &dyn crate::DbDriver, id: &str, enabled: bool) -> Result<bool, DbError> {
    let updated_at = crate::time::now_iso_utc_offset();
    let affected = db.execute(
        "UPDATE plugins SET enabled = ?1, updated_at = ?2 WHERE id = ?3",
        &[enabled.into(), updated_at.into(), id.into()],
    ).await?;
    Ok(affected > 0)
}

pub async fn set_granted(db: &dyn crate::DbDriver, id: &str, granted: &str) -> Result<bool, DbError> {
    let updated_at = crate::time::now_iso_utc_offset();
    let affected = db.execute(
        "UPDATE plugins SET granted_capabilities = ?1, updated_at = ?2 WHERE id = ?3",
        &[granted.into(), updated_at.into(), id.into()],
    ).await?;
    Ok(affected > 0)
}

pub async fn set_sources(db: &dyn crate::DbDriver, id: &str, sources: &str) -> Result<bool, DbError> {
    let updated_at = crate::time::now_iso_utc_offset();
    let affected = db.execute(
        "UPDATE plugins SET sources = ?1, updated_at = ?2 WHERE id = ?3",
        &[sources.into(), updated_at.into(), id.into()],
    ).await?;
    Ok(affected > 0)
}

pub async fn set_manifest(db: &dyn crate::DbDriver, id: &str, manifest: &str) -> Result<bool, DbError> {
    let updated_at = crate::time::now_iso_utc_offset();
    let affected = db.execute(
        "UPDATE plugins SET manifest = ?1, updated_at = ?2 WHERE id = ?3",
        &[manifest.into(), updated_at.into(), id.into()],
    ).await?;
    Ok(affected > 0)
}

/// Deletes a plugin's storage rows explicitly before the plugin row itself: the
/// schema's `ON DELETE CASCADE` needs `PRAGMA foreign_keys = ON`, which neither
/// driver guarantees.
pub async fn remove(db: &dyn crate::DbDriver, id: &str) -> Result<bool, DbError> {
    db.execute("DELETE FROM plugin_storage WHERE plugin_id = ?1", &[id.into()]).await?;
    let affected = db.execute("DELETE FROM plugins WHERE id = ?1", &[id.into()]).await?;
    Ok(affected > 0)
}

pub async fn storage_get_all(
    db: &dyn crate::DbDriver,
    plugin_id: &str,
) -> Result<std::collections::BTreeMap<String, String>, DbError> {
    let rows = db.query(
        "SELECT key, value FROM plugin_storage WHERE plugin_id = ?1",
        &[plugin_id.into()],
    ).await?;
    rows.iter()
        .map(|r| Ok((r.get_string("key")?, r.get_string("value")?)))
        .collect()
}

pub async fn storage_put_many(
    db: &dyn crate::DbDriver,
    plugin_id: &str,
    entries: &[(String, String)],
) -> Result<(), DbError> {
    for (key, value) in entries {
        db.execute(
            "INSERT INTO plugin_storage (plugin_id, key, value) VALUES (?1, ?2, ?3) \
             ON CONFLICT(plugin_id, key) DO UPDATE SET value = ?3",
            &[plugin_id.into(), key.into(), value.into()],
        ).await?;
    }
    Ok(())
}
