use std::sync::Arc;

use crate::error::DbError;

/// A single SQLite value, matching SQLite's five storage classes. `bool` maps to
/// `Integer(0|1)`; JSON columns are carried as `Text`.
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

/// One result row: shared column names + this row's values, positionally aligned.
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
    /// TEXT column holding JSON -> Value; NULL -> Value::Null.
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
        DbValue::Integer(i) => Ok(*i as f64), // SQLite may store a whole REAL as INTEGER
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

/// Accumulates SQL text and positional params, mirroring the subset of
/// `sqlx::QueryBuilder` the domain code uses (`push`, `push_bind`, `separated`).
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
}
