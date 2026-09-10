use std::sync::Arc;

use crate::error::DbError;

/// `bool` maps to `Integer(0|1)`; JSON columns are carried as `Text`.
#[derive(Debug, Clone, PartialEq)]
pub enum DbValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl From<i64> for DbValue { fn from(v: i64) -> Self { DbValue::Integer(v) } }
impl From<f64> for DbValue { fn from(v: f64) -> Self { DbValue::Real(v) } }
impl From<bool> for DbValue { fn from(v: bool) -> Self { DbValue::Integer(v as i64) } }
impl From<String> for DbValue { fn from(v: String) -> Self { DbValue::Text(v) } }
impl From<&str> for DbValue { fn from(v: &str) -> Self { DbValue::Text(v.to_string()) } }
impl From<Vec<u8>> for DbValue { fn from(v: Vec<u8>) -> Self { DbValue::Blob(v) } }
impl<T: Into<DbValue>> From<Option<T>> for DbValue {
    fn from(v: Option<T>) -> Self { v.map(Into::into).unwrap_or(DbValue::Null) }
}
// `&String`, `&i64` etc. show up in the domain code (`.bind(&record.notes)`); accept them.
impl From<&String> for DbValue { fn from(v: &String) -> Self { DbValue::Text(v.clone()) } }
impl From<&i64> for DbValue { fn from(v: &i64) -> Self { DbValue::Integer(*v) } }

#[derive(Debug, Clone)]
pub struct DbRow {
    cols: Arc<Vec<String>>,
    vals: Vec<DbValue>,
}

impl DbRow {
    pub fn from_parts(cols: Arc<Vec<String>>, vals: Vec<DbValue>) -> Self { Self { cols, vals } }

    fn index_of(&self, col: &str) -> Result<usize, DbError> {
        self.cols.iter().position(|c| c == col)
            .ok_or_else(|| DbError::Decode(format!("no column `{col}` in result")))
    }
    fn at(&self, col: &str) -> Result<&DbValue, DbError> { Ok(&self.vals[self.index_of(col)?]) }
    fn at_pos(&self, i: usize) -> Result<&DbValue, DbError> {
        self.vals.get(i).ok_or_else(|| DbError::Decode(format!("no column at index {i}")))
    }

    pub fn get_i64(&self, col: &str) -> Result<i64, DbError> { int(self.at(col)?, col) }
    pub fn get_f64(&self, col: &str) -> Result<f64, DbError> { real(self.at(col)?, col) }
    pub fn get_bool(&self, col: &str) -> Result<bool, DbError> { Ok(int(self.at(col)?, col)? != 0) }
    pub fn get_str(&self, col: &str) -> Result<&str, DbError> { text(self.at(col)?, col) }
    pub fn get_string(&self, col: &str) -> Result<String, DbError> { Ok(self.get_str(col)?.to_string()) }
    pub fn get_blob(&self, col: &str) -> Result<Vec<u8>, DbError> { blob(self.at(col)?, col) }
    pub fn get_opt_str(&self, col: &str) -> Result<Option<String>, DbError> { opt_text(self.at(col)?, col) }
    pub fn get_opt_i64(&self, col: &str) -> Result<Option<i64>, DbError> { opt_int(self.at(col)?, col) }
    pub fn get_json(&self, col: &str) -> Result<serde_json::Value, DbError> {
        match self.at(col)? {
            DbValue::Null => Ok(serde_json::Value::Null),
            DbValue::Text(t) => serde_json::from_str(t)
                .map_err(|e| DbError::Decode(format!("column `{col}` is not valid JSON: {e}"))),
            _ => Err(DbError::Decode(format!("column `{col}` is not text/JSON"))),
        }
    }

    pub fn get_i64_at(&self, i: usize) -> Result<i64, DbError> { int(self.at_pos(i)?, "?") }
    pub fn get_opt_i64_at(&self, i: usize) -> Result<Option<i64>, DbError> { opt_int(self.at_pos(i)?, "?") }
    pub fn get_str_at(&self, i: usize) -> Result<&str, DbError> { text(self.at_pos(i)?, "?") }
    pub fn get_opt_str_at(&self, i: usize) -> Result<Option<String>, DbError> { opt_text(self.at_pos(i)?, "?") }
}

fn int(v: &DbValue, col: &str) -> Result<i64, DbError> {
    match v {
        DbValue::Integer(i) => Ok(*i),
        // A non-sqlx driver (wa-sqlite via JS numbers) can hand back a whole
        // REAL where SQLite stored an INTEGER; `real()` already tolerates the
        // mirror case.
        DbValue::Real(r) if r.fract() == 0.0 => Ok(*r as i64),
        DbValue::Null => Err(DbError::Decode(format!("column `{col}` is NULL"))),
        _ => Err(DbError::Decode(format!("column `{col}` is not an integer"))),
    }
}
fn opt_int(v: &DbValue, col: &str) -> Result<Option<i64>, DbError> {
    match v { DbValue::Null => Ok(None), _ => int(v, col).map(Some) }
}
fn real(v: &DbValue, col: &str) -> Result<f64, DbError> {
    match v {
        DbValue::Real(r) => Ok(*r),
        DbValue::Integer(i) => Ok(*i as f64),
        _ => Err(DbError::Decode(format!("column `{col}` is not a real"))),
    }
}
fn text<'a>(v: &'a DbValue, col: &str) -> Result<&'a str, DbError> {
    match v {
        DbValue::Text(t) => Ok(t),
        DbValue::Null => Err(DbError::Decode(format!("column `{col}` is NULL"))),
        _ => Err(DbError::Decode(format!("column `{col}` is not text"))),
    }
}
fn opt_text(v: &DbValue, col: &str) -> Result<Option<String>, DbError> {
    match v { DbValue::Null => Ok(None), _ => text(v, col).map(|s| Some(s.to_string())) }
}
fn blob(v: &DbValue, col: &str) -> Result<Vec<u8>, DbError> {
    match v {
        DbValue::Blob(b) => Ok(b.clone()),
        DbValue::Null => Err(DbError::Decode(format!("column `{col}` is NULL"))),
        _ => Err(DbError::Decode(format!("column `{col}` is not a blob"))),
    }
}

pub struct SqlBuilder {
    sql: String,
    params: Vec<DbValue>,
}

impl SqlBuilder {
    pub fn new(prefix: &str) -> Self { Self { sql: prefix.to_string(), params: Vec::new() } }
    pub fn push(&mut self, sql: &str) -> &mut Self { self.sql.push_str(sql); self }
    pub fn push_bind(&mut self, value: impl Into<DbValue>) -> &mut Self {
        self.sql.push('?');
        self.params.push(value.into());
        self
    }
    pub fn separated<'a>(&'a mut self, sep: &'a str) -> Separated<'a> {
        Separated { builder: self, sep, first: true }
    }
    pub fn into_parts(self) -> (String, Vec<DbValue>) { (self.sql, self.params) }
}

pub struct Separated<'a> {
    builder: &'a mut SqlBuilder,
    sep: &'a str,
    first: bool,
}
impl Separated<'_> {
    pub fn push_bind(&mut self, value: impl Into<DbValue>) -> &mut Self {
        if !self.first { self.builder.sql.push_str(self.sep); }
        self.first = false;
        self.builder.push_bind(value);
        self
    }
}

/// A second (non-sqlx) implementation must: run `query()` for writes too (some
/// call sites pass `INSERT … RETURNING id`); bind params by 1-based index since
/// a placeholder may repeat; have `execute()` return the statement's own
/// changed-row count, not a cumulative total or last-insert rowid; and round-trip
/// `DbValue::Integer`/`Real` since a whole number may arrive as either.
#[async_trait::async_trait]
pub trait DbDriver: Send + Sync {
    async fn execute(&self, sql: &str, params: &[DbValue]) -> Result<u64, DbError>;
    async fn query(&self, sql: &str, params: &[DbValue]) -> Result<Vec<DbRow>, DbError>;

    /// Runs several statements as one atomic unit where the driver supports it.
    /// The default loops `execute()`, which is today's behavior — lightweight
    /// drivers (the wasm OPFS driver, test doubles) need no change. Drivers
    /// with a real transaction facility (see `SqlxSqliteDriver`) override this
    /// so multi-statement mutations pay one commit instead of one transaction
    /// per statement and cannot interleave across pool connections. Statements
    /// must be writes; rows are not returned.
    async fn execute_batch(&self, statements: &[(&str, Vec<DbValue>)]) -> Result<(), DbError> {
        for (sql, params) in statements {
            self.execute(sql, params).await?;
        }
        Ok(())
    }
}

/// An empty result must surface as an error, not a panic: on wasm `panic = abort`
/// means a panic aborts the module with no error frame.
pub fn scalar_i64(rows: &[DbRow]) -> Result<i64, DbError> {
    rows.first()
        .ok_or_else(|| DbError::Backend("expected one row, got none".into()))?
        .get_i64_at(0)
}

pub fn opt_scalar_i64(rows: &[DbRow]) -> Result<Option<i64>, DbError> {
    match rows.first() {
        Some(row) => row.get_opt_i64_at(0),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn row(cols: &[&str], vals: Vec<DbValue>) -> DbRow {
        DbRow::from_parts(Arc::new(cols.iter().map(|c| c.to_string()).collect()), vals)
    }

    #[test]
    fn dbvalue_from_impls() {
        assert!(matches!(DbValue::from(true), DbValue::Integer(1)));
        assert!(matches!(DbValue::from(false), DbValue::Integer(0)));
        assert!(matches!(DbValue::from(7i64), DbValue::Integer(7)));
        assert!(matches!(DbValue::from("x"), DbValue::Text(_)));
        assert!(matches!(DbValue::from(Option::<String>::None), DbValue::Null));
        assert!(matches!(DbValue::from(Some(3i64)), DbValue::Integer(3)));
    }

    #[test]
    fn dbrow_named_and_positional_getters() {
        let r = row(
            &["id", "name", "flag", "note"],
            vec![DbValue::Integer(5), DbValue::Text("hi".into()), DbValue::Integer(1), DbValue::Null],
        );
        assert_eq!(r.get_i64("id").unwrap(), 5);
        assert_eq!(r.get_str("name").unwrap(), "hi");
        assert!(r.get_bool("flag").unwrap());
        assert_eq!(r.get_opt_str("note").unwrap(), None);
        assert_eq!(r.get_str_at(1).unwrap(), "hi");
        assert!(r.get_i64("name").is_err(), "type mismatch is a Decode error");
        assert!(r.get_i64("missing").is_err(), "unknown column is a Decode error");
        assert!(r.get_i64("note").is_err(), "NULL via non-opt getter is a Decode error");
    }

    #[test]
    fn int_coerces_whole_real_but_rejects_fractional() {
        let r = row(&["a", "b"], vec![DbValue::Real(5.0), DbValue::Real(5.5)]);
        assert_eq!(r.get_i64("a").unwrap(), 5);
        assert!(r.get_i64("b").is_err(), "fractional Real is not a valid integer");
    }

    #[test]
    fn sqlbuilder_push_and_separated() {
        let mut b = SqlBuilder::new("SELECT * FROM t");
        b.push(" WHERE id IN (");
        let mut sep = b.separated(", ");
        sep.push_bind(1i64);
        sep.push_bind(2i64);
        b.push(")");
        b.push(" LIMIT ");
        b.push_bind(10i64);
        let (sql, params) = b.into_parts();
        assert_eq!(sql, "SELECT * FROM t WHERE id IN (?, ?) LIMIT ?");
        assert_eq!(params.len(), 3);
    }

    #[test]
    fn scalar_helpers_error_instead_of_panicking_on_empty() {
        assert!(scalar_i64(&[]).is_err(), "empty result is a DbError, not a panic");
        assert_eq!(opt_scalar_i64(&[]).unwrap(), None, "empty result reads as None");
    }

    #[test]
    fn scalar_helpers_read_the_first_column_of_the_first_row() {
        let rows = [row(&["n"], vec![DbValue::Integer(7)])];
        assert_eq!(scalar_i64(&rows).unwrap(), 7);
        assert_eq!(opt_scalar_i64(&rows).unwrap(), Some(7));

        let nulls = [row(&["n"], vec![DbValue::Null])];
        assert_eq!(opt_scalar_i64(&nulls).unwrap(), None, "a NULL cell reads as None");
        assert!(scalar_i64(&nulls).is_err(), "a NULL cell is an error for the non-opt helper");
    }
}
