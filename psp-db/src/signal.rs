//! SQLite persistence for Signal settings (see migration 0009).
//!
//! The stored shape never includes a password; the REST AdminPassword is
//! kept only in the running poller's memory and re-entered after restarts.
//! The row struct mirrors `psp_signal::store::SignalStored` field-for-field
//! without depending on that crate — psp-server adapts between the two.
use serde::{Deserialize, Serialize};

use crate::{DbDriver, DbError};

/// Signal settings as persisted. Mirrors `SignalStored` in psp-signal.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SignalConfigRow {
    pub enabled: bool,
    pub bind: String,
    pub port: u16,
    pub interval_ms: u64,
    pub allowed_origins: Vec<String>,
    pub source_type: Option<String>,
    pub source_url: Option<String>,
    pub gamedata_path: Option<String>,
    pub token: String,
}

impl SignalConfigRow {
    pub fn defaults() -> Self {
        Self {
            enabled: false,
            bind: "127.0.0.1".into(),
            port: 8788,
            interval_ms: 1000,
            allowed_origins: Vec::new(),
            source_type: None,
            source_url: None,
            gamedata_path: None,
            token: String::new(),
        }
    }
}

const SELECT: &str = "SELECT enabled, bind, port, interval_ms, allowed_origins, \
                      source_type, source_url, gamedata_path, token \
                      FROM signal_config WHERE id = 1";

fn map_row(row: &crate::DbRow) -> Result<SignalConfigRow, DbError> {
    let allowed_raw = row.get_string("allowed_origins")?;
    let allowed_origins: Vec<String> = serde_json::from_str(&allowed_raw).unwrap_or_default();
    Ok(SignalConfigRow {
        enabled: row.get_bool("enabled")?,
        bind: row.get_string("bind")?,
        port: row.get_i64("port")?.clamp(0, u16::MAX as i64) as u16,
        interval_ms: row.get_i64("interval_ms")?.max(0) as u64,
        allowed_origins,
        source_type: row.get_opt_str("source_type")?,
        source_url: row.get_opt_str("source_url")?,
        gamedata_path: row.get_opt_str("gamedata_path")?,
        token: row.get_string("token")?,
    })
}

/// Reads the stored settings; an absent row is seeded and yields defaults,
/// so a fresh install behaves like "never configured".
pub async fn get_signal_config(db: &dyn DbDriver) -> Result<SignalConfigRow, DbError> {
    let defaults = SignalConfigRow::defaults();
    let rows = db.query(SELECT, &[]).await?;
    match rows.first() {
        Some(row) => map_row(row),
        None => {
            save_signal_config(db, &defaults).await?;
            Ok(defaults)
        }
    }
}

pub async fn save_signal_config(db: &dyn DbDriver, stored: &SignalConfigRow) -> Result<(), DbError> {
    let allowed = serde_json::to_string(&stored.allowed_origins)
        .map_err(|error| DbError::Backend(error.to_string()))?;
    db.execute(
        "INSERT INTO signal_config (id, enabled, bind, port, interval_ms, allowed_origins, \
         source_type, source_url, gamedata_path, token) \
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
         ON CONFLICT(id) DO UPDATE SET enabled = ?1, bind = ?2, port = ?3, interval_ms = ?4, \
         allowed_origins = ?5, source_type = ?6, source_url = ?7, gamedata_path = ?8, token = ?9",
        &[
            stored.enabled.into(),
            stored.bind.clone().into(),
            (stored.port as i64).into(),
            (stored.interval_ms as i64).into(),
            allowed.into(),
            stored.source_type.clone().into(),
            stored.source_url.clone().into(),
            stored.gamedata_path.clone().into(),
            stored.token.clone().into(),
        ],
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DbRow, DbValue};
    use std::sync::Arc;

    /// Minimal in-memory driver honoring just this module's statements.
    struct MemDriver {
        row: std::sync::Mutex<Option<SignalConfigRow>>,
    }

    #[async_trait::async_trait]
    impl DbDriver for MemDriver {
        async fn execute(&self, sql: &str, params: &[DbValue]) -> Result<u64, DbError> {
            if sql.starts_with("INSERT INTO signal_config") {
                *self.row.lock().unwrap() = Some(SignalConfigRow {
                    enabled: matches!(params[0], DbValue::Integer(1)),
                    bind: match params[1].clone() { DbValue::Text(t) => t, _ => String::new() },
                    port: match params[2] { DbValue::Integer(p) => p as u16, _ => 0 },
                    interval_ms: match params[3] { DbValue::Integer(v) => v as u64, _ => 0 },
                    allowed_origins: match params[4].clone() {
                        DbValue::Text(t) => serde_json::from_str(&t).unwrap_or_default(),
                        _ => Vec::new(),
                    },
                    source_type: match params[5].clone() { DbValue::Text(t) => Some(t), _ => None },
                    source_url: match params[6].clone() { DbValue::Text(t) => Some(t), _ => None },
                    gamedata_path: match params[7].clone() { DbValue::Text(t) => Some(t), _ => None },
                    token: match params[8].clone() { DbValue::Text(t) => t, _ => String::new() },
                });
            }
            Ok(1)
        }

        async fn query(&self, sql: &str, _params: &[DbValue]) -> Result<Vec<DbRow>, DbError> {
            if !sql.starts_with("SELECT enabled") {
                return Ok(Vec::new());
            }
            let guard = self.row.lock().unwrap();
            Ok(guard
                .as_ref()
                .map(|stored| {
                    vec![DbRow::from_parts(
                        Arc::new(
                            [
                                "enabled", "bind", "port", "interval_ms", "allowed_origins",
                                "source_type", "source_url", "gamedata_path", "token",
                            ]
                            .iter()
                            .map(|c| c.to_string())
                            .collect(),
                        ),
                        vec![
                            DbValue::Integer(i64::from(stored.enabled)),
                            DbValue::Text(stored.bind.clone()),
                            DbValue::Integer(stored.port as i64),
                            DbValue::Integer(stored.interval_ms as i64),
                            DbValue::Text(serde_json::to_string(&stored.allowed_origins).unwrap()),
                            opt_text(stored.source_type.clone()),
                            opt_text(stored.source_url.clone()),
                            opt_text(stored.gamedata_path.clone()),
                            DbValue::Text(stored.token.clone()),
                        ],
                    )]
                })
                .unwrap_or_default())
        }
    }

    /// None ↔ NULL, Some(s) ↔ Text(s): the real driver's binding semantics.
    fn opt_text(value: Option<String>) -> DbValue {
        value.map(DbValue::Text).unwrap_or(DbValue::Null)
    }

    #[tokio::test]
    async fn absent_row_seeds_defaults_and_round_trips() {
        let db = MemDriver { row: std::sync::Mutex::new(None) };
        let fresh = get_signal_config(&db).await.unwrap();
        assert_eq!(fresh, SignalConfigRow::defaults());

        let mut stored = fresh;
        stored.enabled = true;
        stored.source_type = Some("rest".into());
        stored.source_url = Some("http://pal.example:8212".into());
        stored.allowed_origins = vec!["https://map.example".into()];
        save_signal_config(&db, &stored).await.unwrap();
        let back = get_signal_config(&db).await.unwrap();
        assert_eq!(back, stored);
    }
}
