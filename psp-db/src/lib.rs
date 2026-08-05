pub mod blueprints;
pub mod driver;
pub mod error;
pub mod meta;
pub mod migrate;
pub mod presets;
pub mod servers;
pub mod settings;
pub mod time;
pub mod ups;

pub use driver::{opt_scalar_i64, scalar_i64, DbDriver, DbRow, DbValue, SqlBuilder};
pub use error::DbError;
pub use migrate::{run_migrations, Migration, MIGRATIONS};

#[cfg(feature = "sqlx-driver")]
pub mod sqlx_driver;
#[cfg(feature = "sqlx-driver")]
pub use sqlx_driver::SqlxSqliteDriver;

#[cfg(feature = "sqlx-driver")]
pub mod import_legacy;

#[cfg(feature = "sqlx-driver")]
use std::path::Path;

#[cfg(feature = "sqlx-driver")]
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};

/// Opens (creating if missing) the SQLite database at `db_path` and runs the
/// embedded migrations.
#[cfg(feature = "sqlx-driver")]
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
