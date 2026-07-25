use async_trait::async_trait;
use psp_db::error::DbError;
use psp_db::{DbDriver, DbRow, DbValue};

/// A no-op DB driver: reads are always empty, writes are dropped. DB-backed
/// features (presets, UPS, blueprints) show empty until a persistent driver
/// replaces this one.
pub struct StubDriver;

#[async_trait]
impl DbDriver for StubDriver {
    async fn execute(&self, _sql: &str, _params: &[DbValue]) -> Result<u64, DbError> {
        Ok(0)
    }
    async fn query(&self, _sql: &str, _params: &[DbValue]) -> Result<Vec<DbRow>, DbError> {
        Ok(Vec::new())
    }
}
