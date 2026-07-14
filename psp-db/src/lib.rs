pub mod error;
pub mod import_legacy;
pub mod meta;
pub mod presets;
pub mod servers;
pub mod settings;
pub mod time;
pub mod ups;

use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};

use crate::error::DbError;

/// Opens (creating if missing) the SQLite database at `db_path` and runs the
/// embedded migrations.
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
