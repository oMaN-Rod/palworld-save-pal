use crate::error::DbError;
use crate::{DbDriver, DbValue};

pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "settings",
        sql: include_str!("../migrations/0001_settings.sql"),
    },
    Migration {
        version: 2,
        name: "presets",
        sql: include_str!("../migrations/0002_presets.sql"),
    },
    Migration {
        version: 3,
        name: "ups",
        sql: include_str!("../migrations/0003_ups.sql"),
    },
    Migration {
        version: 4,
        name: "servers",
        sql: include_str!("../migrations/0004_servers.sql"),
    },
    Migration {
        version: 5,
        name: "meta",
        sql: include_str!("../migrations/0005_meta.sql"),
    },
    Migration {
        version: 6,
        name: "blueprints",
        sql: include_str!("../migrations/0006_blueprints.sql"),
    },
    Migration {
        version: 7,
        name: "ups_awakened_imported",
        sql: include_str!("../migrations/0007_ups_awakened_imported.sql"),
    },
    Migration {
        version: 8,
        name: "plugins",
        sql: include_str!("../migrations/0008_plugins.sql"),
    },
    Migration {
        version: 9,
        name: "signal_devices",
        sql: include_str!("../migrations/0009_signal_devices.sql"),
    },
    Migration {
        version: 10,
        name: "amity_instances",
        sql: include_str!("../migrations/0010_amity_instances.sql"),
    },
];

const CREATE_TRACKER: &str =
    "CREATE TABLE IF NOT EXISTS _ps_migrations (version INTEGER PRIMARY KEY)";
const SELECT_APPLIED: &str = "SELECT version FROM _ps_migrations";
const SELECT_TRACKERS: &str = "SELECT \
    (SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '_psp_migrations') AS legacy, \
    (SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '_ps_migrations') AS current";
const RENAME_LEGACY_TRACKER: &str = "ALTER TABLE _psp_migrations RENAME TO _ps_migrations";

/// A database created under the old tracker name keeps its applied versions;
/// without this every migration would re-run against existing tables.
async fn adopt_legacy_tracker(db: &dyn DbDriver) -> Result<(), DbError> {
    let rows = db.query(SELECT_TRACKERS, &[]).await?;
    let Some(row) = rows.first() else {
        return Ok(());
    };
    if row.get_i64("legacy")? == 1 && row.get_i64("current")? == 0 {
        db.execute(RENAME_LEGACY_TRACKER, &[]).await?;
    }
    Ok(())
}

/// Each migration's SQL runs as a single `execute` call — the driver must run
/// multi-statement scripts when given no params.
pub async fn run_migrations(db: &dyn DbDriver) -> Result<(), DbError> {
    adopt_legacy_tracker(db).await?;
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
            "INSERT INTO _ps_migrations (version) VALUES (?)",
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

    struct MockDriver {
        applied: Mutex<Vec<i64>>,
        executes: Mutex<Vec<String>>,
        legacy_tracker: bool,
    }

    impl MockDriver {
        fn new(legacy_tracker: bool) -> Self {
            Self {
                applied: Mutex::new(vec![]),
                executes: Mutex::new(vec![]),
                legacy_tracker,
            }
        }
    }

    #[async_trait::async_trait]
    impl DbDriver for MockDriver {
        async fn execute(&self, sql: &str, params: &[DbValue]) -> Result<u64, DbError> {
            self.executes.lock().unwrap().push(sql.to_string());
            if sql.starts_with("INSERT INTO _ps_migrations") {
                if let Some(DbValue::Integer(v)) = params.first() {
                    self.applied.lock().unwrap().push(*v);
                }
            }
            Ok(0)
        }
        async fn query(&self, sql: &str, _params: &[DbValue]) -> Result<Vec<DbRow>, DbError> {
            if sql == SELECT_TRACKERS {
                let cols = Arc::new(vec!["legacy".to_string(), "current".to_string()]);
                let renamed = self
                    .executes
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|s| s == RENAME_LEGACY_TRACKER);
                let legacy = i64::from(self.legacy_tracker && !renamed);
                return Ok(vec![DbRow::from_parts(
                    cols,
                    vec![DbValue::Integer(legacy), DbValue::Integer(1 - legacy)],
                )]);
            }
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
    async fn renames_the_legacy_tracker_once() {
        let driver = MockDriver::new(true);
        run_migrations(&driver).await.unwrap();
        run_migrations(&driver).await.unwrap();
        let renames = driver
            .executes
            .lock()
            .unwrap()
            .iter()
            .filter(|s| *s == RENAME_LEGACY_TRACKER)
            .count();
        assert_eq!(renames, 1);
    }

    #[tokio::test]
    async fn applies_all_then_is_idempotent() {
        let driver = MockDriver::new(false);
        run_migrations(&driver).await.unwrap();
        assert_eq!(
            driver.applied.lock().unwrap().clone(),
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
        );
        let migration_execs = driver
            .executes
            .lock()
            .unwrap()
            .iter()
            .filter(|s| !s.contains("_ps_migrations"))
            .count();
        assert_eq!(migration_execs, MIGRATIONS.len());

        driver.executes.lock().unwrap().clear();
        run_migrations(&driver).await.unwrap();
        let reruns = driver
            .executes
            .lock()
            .unwrap()
            .iter()
            .filter(|s| !s.contains("_ps_migrations"))
            .count();
        assert_eq!(reruns, 0, "already-applied migrations must not re-run");
    }
}
