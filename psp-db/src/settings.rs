use crate::error::DbError;

#[derive(Debug, Clone)]
pub struct SettingsRow {
    pub language: String,
    pub save_dir: String,
    pub clone_prefix: String,
    pub new_pal_prefix: String,
    pub debug_mode: bool,
    pub cheat_mode: bool,
}

/// Fields updatable through the `update_settings` message. `save_dir` is deliberately
/// absent: it is only ever set by `update_save_dir`.
#[derive(Debug, Clone)]
pub struct SettingsUpdate {
    pub language: String,
    pub clone_prefix: String,
    pub new_pal_prefix: String,
    pub debug_mode: bool,
    pub cheat_mode: bool,
}

const SELECT_SETTINGS: &str = "SELECT language, save_dir, clone_prefix, new_pal_prefix, \
                               debug_mode, cheat_mode FROM settings WHERE id = 1";

fn map_settings(r: &crate::DbRow) -> Result<SettingsRow, DbError> {
    Ok(SettingsRow {
        language: r.get_string("language")?,
        save_dir: r.get_string("save_dir")?,
        clone_prefix: r.get_string("clone_prefix")?,
        new_pal_prefix: r.get_string("new_pal_prefix")?,
        debug_mode: r.get_bool("debug_mode")?,
        cheat_mode: r.get_bool("cheat_mode")?,
    })
}

/// Returns the settings row, inserting the default row on first access.
pub async fn get_settings(db: &dyn crate::DbDriver) -> Result<SettingsRow, DbError> {
    let rows = db.query(SELECT_SETTINGS, &[]).await?;
    if let Some(row) = rows.first() {
        return map_settings(row);
    }
    let defaults = SettingsRow {
        language: "en".into(),
        save_dir: default_steam_save_dir(),
        clone_prefix: "©️".into(),
        new_pal_prefix: "🆕".into(),
        debug_mode: false,
        cheat_mode: false,
    };
    // ON CONFLICT DO NOTHING makes concurrent first calls race-safe: a loser of the
    // insert falls through to the re-select instead of failing the id = 1 primary key.
    db.execute(
        "INSERT INTO settings (id, language, save_dir, clone_prefix, new_pal_prefix, debug_mode, cheat_mode) \
         VALUES (1, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO NOTHING",
        &[
            defaults.language.clone().into(),
            defaults.save_dir.clone().into(),
            defaults.clone_prefix.clone().into(),
            defaults.new_pal_prefix.clone().into(),
            defaults.debug_mode.into(),
            defaults.cheat_mode.into(),
        ],
    ).await?;
    // Re-select rather than return `defaults`: the committed row may be a racer's. A
    // driver that cannot persist (the wasm stub: empty reads, dropped writes) has no
    // row to re-select, so fall back to the in-memory defaults rather than erroring —
    // native drivers always persist, so the fallback is unreachable there.
    let rows = db.query(SELECT_SETTINGS, &[]).await?;
    match rows.first() {
        Some(row) => map_settings(row),
        None => Ok(defaults),
    }
}

/// Upserts every column except save_dir: the DO UPDATE branch omits it, so the bound
/// default only lands when this call is the one creating the row.
pub async fn update_settings(db: &dyn crate::DbDriver, update: &SettingsUpdate) -> Result<SettingsRow, DbError> {
    db.execute(
        "INSERT INTO settings (id, language, save_dir, clone_prefix, new_pal_prefix, debug_mode, cheat_mode) \
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6) \
         ON CONFLICT(id) DO UPDATE SET language = ?1, clone_prefix = ?3, new_pal_prefix = ?4, \
         debug_mode = ?5, cheat_mode = ?6",
        &[
            update.language.clone().into(),
            default_steam_save_dir().into(),
            update.clone_prefix.clone().into(),
            update.new_pal_prefix.clone().into(),
            update.debug_mode.into(),
            update.cheat_mode.into(),
        ],
    ).await?;
    get_settings(db).await
}

/// Sets the singleton settings row's `save_dir`, leaning on `get_settings` to create
/// the row (with defaults) first so the UPDATE always has something to hit.
pub async fn update_save_dir(db: &dyn crate::DbDriver, save_dir: &str) -> Result<(), DbError> {
    get_settings(db).await?;
    db.execute("UPDATE settings SET save_dir = ?1 WHERE id = 1", &[save_dir.into()]).await?;
    Ok(())
}

/// Reads the singleton settings row's save_dir. None means the row does not exist yet
/// (fresh DB, before `get_settings` seeds it).
pub async fn saved_save_dir(db: &dyn crate::DbDriver) -> Result<Option<String>, DbError> {
    let rows = db.query("SELECT save_dir FROM settings WHERE id = 1", &[]).await?;
    rows.first().map(|r| r.get_string("save_dir")).transpose()
}

/// Platform-specific location where the Steam release of the game keeps its saves.
pub fn default_steam_save_dir() -> String {
    #[cfg(target_os = "windows")]
    {
        let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_default();
        std::path::Path::new(&local_app_data)
            .join("Pal")
            .join("Saved")
            .join("SaveGames")
            .to_string_lossy()
            .into_owned()
    }
    #[cfg(target_os = "macos")]
    {
        let user = std::env::var("USER").unwrap_or_default();
        format!(
            "/System/Volumes/Data/Users/{user}/Library/Containers/com.pocketpair.palworld.mac/Data/Library/Application Support/Epic/Pal/Saved/SaveGames"
        )
    }
    #[cfg(target_arch = "wasm32")]
    {
        // No filesystem in the browser; the web build never auto-discovers a
        // save directory.
        String::new()
    }
    #[cfg(all(
        not(target_arch = "wasm32"),
        not(target_os = "windows"),
        not(target_os = "macos")
    ))]
    {
        "~".to_string()
    }
}

#[cfg(test)]
mod fallback_tests {
    use super::*;
    use crate::{DbDriver, DbError, DbRow};

    /// A driver whose writes are dropped and whose reads are always empty — the
    /// shape of the wasm M1 stub. `get_settings` must degrade to defaults, not error.
    struct EmptyDriver;

    #[async_trait::async_trait]
    impl DbDriver for EmptyDriver {
        async fn execute(&self, _sql: &str, _params: &[crate::DbValue]) -> Result<u64, DbError> {
            Ok(0)
        }
        async fn query(&self, _sql: &str, _params: &[crate::DbValue]) -> Result<Vec<DbRow>, DbError> {
            Ok(Vec::new())
        }
    }

    #[tokio::test]
    async fn get_settings_falls_back_to_defaults_when_row_never_persists() {
        let row = get_settings(&EmptyDriver).await.expect("must not error");
        assert_eq!(row.language, "en");
        assert_eq!(row.clone_prefix, "©️");
        assert!(!row.debug_mode);
    }
}
