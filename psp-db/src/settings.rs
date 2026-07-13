use sqlx::SqlitePool;

use crate::error::DbError;

#[derive(Debug, Clone, sqlx::FromRow)]
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

/// Returns the settings row, inserting the default row on first access.
pub async fn get_settings(pool: &SqlitePool) -> Result<SettingsRow, DbError> {
    if let Some(row) = sqlx::query_as::<_, SettingsRow>(SELECT_SETTINGS)
        .fetch_optional(pool)
        .await?
    {
        return Ok(row);
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
    sqlx::query(
        "INSERT INTO settings (id, language, save_dir, clone_prefix, new_pal_prefix, debug_mode, cheat_mode) \
         VALUES (1, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO NOTHING",
    )
    .bind(&defaults.language)
    .bind(&defaults.save_dir)
    .bind(&defaults.clone_prefix)
    .bind(&defaults.new_pal_prefix)
    .bind(defaults.debug_mode)
    .bind(defaults.cheat_mode)
    .execute(pool)
    .await?;

    // Re-select rather than return `defaults`: the committed row may be a racer's, and a
    // still-missing row is a real error worth surfacing as RowNotFound.
    let row = sqlx::query_as::<_, SettingsRow>(SELECT_SETTINGS)
        .fetch_one(pool)
        .await?;
    Ok(row)
}

/// Upserts every column except save_dir: the DO UPDATE branch omits it, so the bound
/// default only lands when this call is the one creating the row.
pub async fn update_settings(
    pool: &SqlitePool,
    update: &SettingsUpdate,
) -> Result<SettingsRow, DbError> {
    sqlx::query(
        "INSERT INTO settings (id, language, save_dir, clone_prefix, new_pal_prefix, debug_mode, cheat_mode) \
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6) \
         ON CONFLICT(id) DO UPDATE SET language = ?1, clone_prefix = ?3, new_pal_prefix = ?4, \
         debug_mode = ?5, cheat_mode = ?6",
    )
    .bind(&update.language)
    .bind(default_steam_save_dir())
    .bind(&update.clone_prefix)
    .bind(&update.new_pal_prefix)
    .bind(update.debug_mode)
    .bind(update.cheat_mode)
    .execute(pool)
    .await?;
    get_settings(pool).await
}

/// Sets the singleton settings row's `save_dir`, leaning on `get_settings` to create
/// the row (with defaults) first so the UPDATE always has something to hit.
pub async fn update_save_dir(pool: &SqlitePool, save_dir: &str) -> Result<(), DbError> {
    get_settings(pool).await?;
    sqlx::query("UPDATE settings SET save_dir = ?1 WHERE id = 1")
        .bind(save_dir)
        .execute(pool)
        .await?;
    Ok(())
}

/// Reads the singleton settings row's save_dir. None means the row does not exist yet
/// (fresh DB, before `get_settings` seeds it).
pub async fn saved_save_dir(pool: &SqlitePool) -> Result<Option<String>, DbError> {
    let row: Option<(String,)> = sqlx::query_as("SELECT save_dir FROM settings WHERE id = 1")
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|(save_dir,)| save_dir))
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
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        "~".to_string()
    }
}
