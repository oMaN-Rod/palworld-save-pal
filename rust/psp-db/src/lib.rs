// SQLite persistence. Settings table only in Phase 0; full schema lands in Phase 3.
pub mod error;
pub mod import_legacy;
pub mod meta;
pub mod presets;
pub mod settings;
pub mod time;

use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};

use crate::error::DbError;

/// Opens (creating if missing) the SQLite database at `db_path` and runs
/// the embedded migrations. The legacy Python `psp.db` importer is Phase 3;
/// this file is the NEW database (default name `psp-rs.db`).
pub async fn open(db_path: &Path) -> Result<SqlitePool, DbError> {
    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}
