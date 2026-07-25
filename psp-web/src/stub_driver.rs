use async_trait::async_trait;
use psp_db::error::DbError;
use psp_db::{DbDriver, DbRow, DbValue};

/// The M1 web driver: reads are always empty, writes are dropped. DB-backed
/// features (presets, UPS, blueprints) show empty; `get_settings` degrades to
/// defaults (see psp-db). Real persistence is M2 (wa-sqlite/OPFS).
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
