use std::sync::Arc;

use sqlx::{Column, Row, SqlitePool, TypeInfo, ValueRef};

use crate::driver::{DbDriver, DbRow, DbValue};
use crate::error::DbError;

pub struct SqlxSqliteDriver {
    pool: SqlitePool,
}

impl SqlxSqliteDriver {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn bind_all<'q>(
    mut q: sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>>,
    params: &'q [DbValue],
) -> sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>> {
    for p in params {
        // sqlx-sqlite impls Encode for i64/f64/&str/&[u8], NOT for &i64/&String/&Vec<u8>.
        q = match p {
            DbValue::Null => q.bind(Option::<i64>::None),
            DbValue::Integer(i) => q.bind(*i),
            DbValue::Real(r) => q.bind(*r),
            DbValue::Text(t) => q.bind(t.as_str()),
            DbValue::Blob(b) => q.bind(b.as_slice()),
        };
    }
    q
}

fn decode_row(row: &sqlx::sqlite::SqliteRow, cols: Arc<Vec<String>>) -> Result<DbRow, DbError> {
    let mut vals = Vec::with_capacity(cols.len());
    for i in 0..cols.len() {
        let raw = row
            .try_get_raw(i)
            .map_err(|e| DbError::Backend(e.to_string()))?;
        let value = if raw.is_null() {
            DbValue::Null
        } else {
            match raw.type_info().name() {
                "INTEGER" | "BOOLEAN" => DbValue::Integer(
                    row.try_get::<i64, _>(i)
                        .map_err(|e| DbError::Backend(e.to_string()))?,
                ),
                "REAL" => DbValue::Real(
                    row.try_get::<f64, _>(i)
                        .map_err(|e| DbError::Backend(e.to_string()))?,
                ),
                "BLOB" => DbValue::Blob(
                    row.try_get::<Vec<u8>, _>(i)
                        .map_err(|e| DbError::Backend(e.to_string()))?,
                ),
                _ => DbValue::Text(
                    row.try_get::<String, _>(i)
                        .map_err(|e| DbError::Backend(e.to_string()))?,
                ),
            }
        };
        vals.push(value);
    }
    Ok(DbRow::from_parts(cols, vals))
}

#[async_trait::async_trait]
impl DbDriver for SqlxSqliteDriver {
    async fn execute(&self, sql: &str, params: &[DbValue]) -> Result<u64, DbError> {
        let q = bind_all(sqlx::query(sql), params);
        let result = q
            .execute(&self.pool)
            .await
            .map_err(|e| DbError::Backend(e.to_string()))?;
        Ok(result.rows_affected())
    }

    async fn query(&self, sql: &str, params: &[DbValue]) -> Result<Vec<DbRow>, DbError> {
        let q = bind_all(sqlx::query(sql), params);
        let rows = q
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DbError::Backend(e.to_string()))?;
        let cols: Arc<Vec<String>> = match rows.first() {
            Some(r) => Arc::new(r.columns().iter().map(|c| c.name().to_string()).collect()),
            None => Arc::new(Vec::new()),
        };
        rows.iter().map(|r| decode_row(r, cols.clone())).collect()
    }

    /// One pool transaction: each statement commits together, and a failure
    /// rolls the whole batch back (the `Transaction` is dropped uncommitted).
    async fn execute_batch(&self, statements: &[(&str, Vec<DbValue>)]) -> Result<(), DbError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| DbError::Backend(e.to_string()))?;
        for (sql, params) in statements {
            bind_all(sqlx::query(sql), params)
                .execute(&mut *tx)
                .await
                .map_err(|e| DbError::Backend(e.to_string()))?;
        }
        tx.commit()
            .await
            .map_err(|e| DbError::Backend(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DbValue;

    async fn driver() -> SqlxSqliteDriver {
        let dir = tempfile::tempdir().unwrap();
        let pool = crate::open(&dir.path().join("t.db")).await.unwrap();
        std::mem::forget(dir);
        SqlxSqliteDriver::new(pool)
    }

    #[tokio::test]
    async fn execute_and_query_round_trip_by_name() {
        let d = driver().await;
        d.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT, ratio REAL, blob BLOB, flag INTEGER)", &[]).await.unwrap();
        let affected = d.execute(
            "INSERT INTO t (name, ratio, blob, flag) VALUES (?, ?, ?, ?)",
            &[DbValue::from("hi"), DbValue::from(1.5f64), DbValue::from(vec![1u8, 2, 3]), DbValue::from(true)],
        ).await.unwrap();
        assert_eq!(affected, 1);

        let rows = d.query("SELECT id, name, ratio, blob, flag FROM t", &[]).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get_i64("id").unwrap(), 1);
        assert_eq!(rows[0].get_str("name").unwrap(), "hi");
        assert_eq!(rows[0].get_f64("ratio").unwrap(), 1.5);
        assert_eq!(rows[0].get_blob("blob").unwrap(), vec![1, 2, 3]);
        assert!(rows[0].get_bool("flag").unwrap());
    }

    #[tokio::test]
    async fn returning_and_null_and_empty() {
        let d = driver().await;
        d.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, note TEXT)", &[]).await.unwrap();
        let id = d.query("INSERT INTO t (note) VALUES (?) RETURNING id", &[DbValue::Null]).await.unwrap()[0]
            .get_i64_at(0).unwrap();
        assert_eq!(id, 1);
        let rows = d.query("SELECT note FROM t WHERE id = ?", &[DbValue::from(id)]).await.unwrap();
        assert_eq!(rows[0].get_opt_str("note").unwrap(), None);
        assert!(d.query("SELECT id FROM t WHERE id = 999", &[]).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn execute_batch_commits_and_rolls_back_atomically() {
        let d = driver().await;
        d.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, n INTEGER)", &[])
            .await
            .unwrap();

        // All statements in a batch commit together.
        d.execute_batch(&[
            ("INSERT INTO t (n) VALUES (?)", vec![DbValue::from(1i64)]),
            ("INSERT INTO t (n) VALUES (?)", vec![DbValue::from(2i64)]),
        ])
        .await
        .unwrap();
        let rows = d.query("SELECT n FROM t ORDER BY n", &[]).await.unwrap();
        assert_eq!(rows.len(), 2);

        // A failing statement rolls the whole batch back, not just itself.
        let result = d
            .execute_batch(&[
                ("INSERT INTO t (n) VALUES (?)", vec![DbValue::from(3i64)]),
                ("INSERT INTO no_such_table (n) VALUES (4)", vec![]),
            ])
            .await;
        assert!(result.is_err());
        let rows = d.query("SELECT n FROM t ORDER BY n", &[]).await.unwrap();
        assert_eq!(
            rows.iter()
                .map(|r| r.get_i64_at(0).unwrap())
                .collect::<Vec<_>>(),
            vec![1, 2],
            "the first statement of the failed batch must not persist"
        );
    }
}
