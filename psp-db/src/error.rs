#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[cfg(feature = "sqlx-driver")]
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[cfg(feature = "sqlx-driver")]
    #[error(transparent)]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Backend(String),
    #[error("{0}")]
    Decode(String),
    #[error("{0}")]
    Other(String),
}
