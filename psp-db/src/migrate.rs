use crate::error::DbError;
use crate::{DbDriver, DbValue};

pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

pub const MIGRATIONS: &[Migration] = &[
    Migration { version: 1, name: "settings", sql: include_str!("../migrations/0001_settings.sql") },
    Migration { version: 2, name: "presets", sql: include_str!("../migrations/0002_presets.sql") },
    Migration { version: 3, name: "ups", sql: include_str!("../migrations/0003_ups.sql") },
    Migration { version: 4, name: "servers", sql: include_str!("../migrations/0004_servers.sql") },
    Migration { version: 5, name: "meta", sql: include_str!("../migrations/0005_meta.sql") },
    Migration { version: 6, name: "blueprints", sql: include_str!("../migrations/0006_blueprints.sql") },
    Migration { version: 7, name: "ups_awakened_imported", sql: include_str!("../migrations/0007_ups_awakened_imported.sql") },
];

const CREATE_TRACKER: &str =
    "CREATE TABLE IF NOT EXISTS _psp_migrations (version INTEGER PRIMARY KEY)";
const SELECT_APPLIED: &str = "SELECT version FROM _psp_migrations";

/// Applies every migration whose version is not already recorded, through the
/// `DbDriver` seam. Apply-if-absent, so a persistent DB that already ran them is
/// left alone on the next open. Each migration's SQL is executed as a single
/// `execute` call (the driver runs multi-statement scripts when given no params).
pub async fn run_migrations(db: &dyn DbDriver) -> Result<(), DbError> {
    db.execute(CREATE_TRACKER, &[]).await?;
    let applied: std::collections::HashSet<i64> = db
        .query(SELECT_APPLIED, &[])
        .await?
        .iter()
        .map(|row| row.get_i64("version"))
        .collect::<Result<_, _>>()?;

    for migration in MIGRATIONS {
        if applied.contains(&migration.version) {
            continue;
        }
        db.execute(migration.sql, &[]).await?;
        db.execute(
            "INSERT INTO _psp_migrations (version) VALUES (?)",
            &[DbValue::Integer(migration.version)],
        )
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DbRow;
    use std::sync::{Arc, Mutex};

    /// Records executes; answers the version-check with a script-controlled set.
    struct MockDriver {
        applied: Mutex<Vec<i64>>,
        executes: Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl DbDriver for MockDriver {
        async fn execute(&self, sql: &str, params: &[DbValue]) -> Result<u64, DbError> {
            self.executes.lock().unwrap().push(sql.to_string());
            if sql.starts_with("INSERT INTO _psp_migrations") {
                if let Some(DbValue::Integer(v)) = params.first() {
                    self.applied.lock().unwrap().push(*v);
                }
            }
            Ok(0)
        }
        async fn query(&self, _sql: &str, _params: &[DbValue]) -> Result<Vec<DbRow>, DbError> {
            let cols = Arc::new(vec!["version".to_string()]);
            Ok(self
                .applied
                .lock()
                .unwrap()
                .iter()
                .map(|v| DbRow::from_parts(cols.clone(), vec![DbValue::Integer(*v)]))
                .collect())
        }
    }

    #[tokio::test]
    async fn applies_all_then_is_idempotent() {
        let driver = MockDriver { applied: Mutex::new(vec![]), executes: Mutex::new(vec![]) };
        run_migrations(&driver).await.unwrap();
        assert_eq!(driver.applied.lock().unwrap().clone(), vec![1, 2, 3, 4, 5, 6, 7]);
        // Each migration SQL executed exactly once, plus the tracker + one insert per migration.
        let migration_execs = driver
            .executes
            .lock()
            .unwrap()
            .iter()
            .filter(|s| !s.contains("_psp_migrations"))
            .count();
        assert_eq!(migration_execs, MIGRATIONS.len());

        // Second run: all versions present → no further migration executes.
        driver.executes.lock().unwrap().clear();
        run_migrations(&driver).await.unwrap();
        let reruns = driver
            .executes
            .lock()
            .unwrap()
            .iter()
            .filter(|s| !s.contains("_psp_migrations"))
            .count();
        assert_eq!(reruns, 0, "already-applied migrations must not re-run");
    }
}
