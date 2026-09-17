use crate::error::DbError;

pub async fn get(db: &dyn crate::DbDriver, key: &str) -> Result<Option<String>, DbError> {
    let rows = db
        .query("SELECT value FROM meta WHERE key = ?", &[key.into()])
        .await?;
    rows.first().map(|r| r.get_string("value")).transpose()
}

pub async fn set(db: &dyn crate::DbDriver, key: &str, value: &str) -> Result<(), DbError> {
    db.execute(
        "INSERT INTO meta (key, value, updated_at) VALUES (?, ?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        &[
            key.into(),
            value.into(),
            crate::time::now_iso_naive_utc().into(),
        ],
    )
    .await?;
    Ok(())
}
