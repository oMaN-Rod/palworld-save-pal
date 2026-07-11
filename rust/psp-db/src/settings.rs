use sqlx::SqlitePool;

use crate::error::DbError;

/// The settings row as stored (and as sent to the frontend after DTO conversion).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SettingsRow {
    pub language: String,
    pub save_dir: String,
    pub clone_prefix: String,
    pub new_pal_prefix: String,
    pub debug_mode: bool,
    pub cheat_mode: bool,
}

/// Fields updatable through the `update_settings` message (save_dir is not one of them —
/// matches palworld_save_pal/dto/settings.py).
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

/// Returns the settings row, inserting Python's defaults on first access
/// (mirrors db/ctx/settings.py get_settings).
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
    // ON CONFLICT DO NOTHING makes this race-safe: if another connection wins
    // the insert first, this call simply falls through to the re-select below
    // and returns the winner's row instead of erroring on the PRIMARY KEY
    // CHECK (id = 1) constraint.
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

    // Re-select rather than returning `defaults` locally: this reflects
    // whichever row actually got committed (ours or a concurrent racer's).
    // If the row is somehow still missing here, that's a genuine error —
    // fetch_one surfaces it as sqlx::Error::RowNotFound via DbError's
    // existing #[from] conversion, rather than silently returning `defaults`.
    let row = sqlx::query_as::<_, SettingsRow>(SELECT_SETTINGS)
        .fetch_one(pool)
        .await?;
    Ok(row)
}

/// Upserts everything except save_dir (which only gets its default on fresh insert) —
/// mirrors db/ctx/settings.py update_settings.
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

/// Port of db/ctx/settings.py::update_save_dir (43-54): sets the singleton
/// settings row's `save_dir`, creating the row first when it does not yet
/// exist. Python creates a fresh `SettingsModel(id=1, save_dir=save_dir)` on a
/// missing row and otherwise assigns `settings.save_dir = save_dir`; this
/// mirrors both by ensuring the row exists via `get_settings` (which inserts
/// Python's defaults on first access) and then `UPDATE`-ing only `save_dir`, so
/// the other columns keep whatever they held (defaults on a fresh row).
///
/// Used by the desktop native-file-dialog flow (Phase 5) and by the gamepass
/// load path's tests, which set `save_dir` directly so `select_gamepass_save`
/// can read the container directory back out of settings.
pub async fn update_save_dir(pool: &SqlitePool, save_dir: &str) -> Result<(), DbError> {
    get_settings(pool).await?;
    sqlx::query("UPDATE settings SET save_dir = ?1 WHERE id = 1")
        .bind(save_dir)
        .execute(pool)
        .await?;
    Ok(())
}

/// Port of STEAM_ROOT from palworld_save_pal/utils/file_manager.py:23-35.
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
