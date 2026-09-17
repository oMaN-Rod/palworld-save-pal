pub mod amity_instances;
pub mod blueprints;
pub mod driver;
pub mod error;
pub mod meta;
pub mod migrate;
pub mod plugins;
pub mod presets;
pub mod servers;
pub mod settings;
pub mod signal_devices;
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
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
};

#[cfg(feature = "sqlx-driver")]
fn embedded_migrator() -> sqlx::migrate::Migrator {
    use sqlx::migrate::{Migration, MigrationType, Migrator};
    use std::borrow::Cow;

    // Keep the migration set explicit and embedded in the binary. This avoids
    // resolving arbitrary files from a writable checkout or installation
    // directory, and makes backup files such as `0001_settings 2.sql`
    // impossible to interpret as a second migration.
    let migrations = vec![
        Migration::new(
            1,
            Cow::Borrowed("settings"),
            MigrationType::Simple,
            Cow::Borrowed(include_str!("../migrations/0001_settings.sql")),
            false,
        ),
        Migration::new(
            2,
            Cow::Borrowed("presets"),
            MigrationType::Simple,
            Cow::Borrowed(include_str!("../migrations/0002_presets.sql")),
            false,
        ),
        Migration::new(
            3,
            Cow::Borrowed("ups"),
            MigrationType::Simple,
            Cow::Borrowed(include_str!("../migrations/0003_ups.sql")),
            false,
        ),
        Migration::new(
            4,
            Cow::Borrowed("servers"),
            MigrationType::Simple,
            Cow::Borrowed(include_str!("../migrations/0004_servers.sql")),
            false,
        ),
        Migration::new(
            5,
            Cow::Borrowed("meta"),
            MigrationType::Simple,
            Cow::Borrowed(include_str!("../migrations/0005_meta.sql")),
            false,
        ),
        Migration::new(
            6,
            Cow::Borrowed("blueprints"),
            MigrationType::Simple,
            Cow::Borrowed(include_str!("../migrations/0006_blueprints.sql")),
            false,
        ),
        Migration::new(
            7,
            Cow::Borrowed("ups_awakened_imported"),
            MigrationType::Simple,
            Cow::Borrowed(include_str!("../migrations/0007_ups_awakened_imported.sql")),
            false,
        ),
        Migration::new(
            8,
            Cow::Borrowed("plugins"),
            MigrationType::Simple,
            Cow::Borrowed(include_str!("../migrations/0008_plugins.sql")),
            false,
        ),
        Migration::new(
            9,
            Cow::Borrowed("signal_devices"),
            MigrationType::Simple,
            Cow::Borrowed(include_str!("../migrations/0009_signal_devices.sql")),
            false,
        ),
        Migration::new(
            10,
            Cow::Borrowed("amity_instances"),
            MigrationType::Simple,
            Cow::Borrowed(include_str!("../migrations/0010_amity_instances.sql")),
            false,
        ),
    ];

    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
}

#[cfg(feature = "sqlx-driver")]
pub async fn open(db_path: &Path) -> Result<SqlitePool, DbError> {
    if db_path.as_os_str().is_empty() {
        return Err(DbError::Backend("database path is empty".into()));
    }
    if let Ok(metadata) = std::fs::symlink_metadata(db_path) {
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return Err(DbError::Backend(format!(
                "database path is not a regular file: {}",
                db_path.display()
            )));
        }
    }
    if let Some(parent) = db_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        let parent_missing = !parent.exists();
        std::fs::create_dir_all(parent)?;
        #[cfg(unix)]
        if parent_missing {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
        }
    }
    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        // WAL + NORMAL: every statement otherwise pays a rollback-journal
        // fsync (synchronous defaults to FULL), which multiplies latency on
        // the multi-statement UPS/preset/server mutations.
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal);
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await?;
    embedded_migrator().run(&pool).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let private = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(db_path, private.clone())?;
        for suffix in ["-wal", "-shm"] {
            let sibling = std::path::PathBuf::from(format!("{}{}", db_path.display(), suffix));
            if sibling.exists() {
                std::fs::set_permissions(sibling, private.clone())?;
            }
        }
    }
    Ok(pool)
}
