
use crate::error::DbError;

pub struct SignalDevice {
    pub device_id: String,
    pub secret_hex: String,
    pub name: String,
    pub created_at_ms: i64,
    pub last_seen_ms: Option<i64>,
}

impl std::fmt::Debug for SignalDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignalDevice")
            .field("device_id", &self.device_id)
            .field("secret_hex", &"<redacted>")
            .field("name", &self.name)
            .field("created_at_ms", &self.created_at_ms)
            .field("last_seen_ms", &self.last_seen_ms)
            .finish()
    }
}

const SELECT_COLUMNS: &str = "device_id, secret_hex, name, created_at_ms, last_seen_ms";

fn map_device(r: &crate::DbRow) -> Result<SignalDevice, DbError> {
    Ok(SignalDevice {
        device_id: r.get_string("device_id")?,
        secret_hex: r.get_string("secret_hex")?,
        name: r.get_string("name")?,
        created_at_ms: r.get_i64("created_at_ms")?,
        last_seen_ms: r.get_opt_i64("last_seen_ms")?,
    })
}

pub async fn list_devices(db: &dyn crate::DbDriver) -> Result<Vec<SignalDevice>, DbError> {
    let rows = db
        .query(
            &format!("SELECT {SELECT_COLUMNS} FROM signal_devices ORDER BY created_at_ms"),
            &[],
        )
        .await?;
    rows.iter().map(map_device).collect()
}

pub async fn insert_device(db: &dyn crate::DbDriver, device: &SignalDevice) -> Result<(), DbError> {
    db.execute(
        "INSERT INTO signal_devices (device_id, secret_hex, name, created_at_ms, last_seen_ms) \
         VALUES (?, ?, ?, ?, ?)",
        &[
            device.device_id.as_str().into(),
            device.secret_hex.as_str().into(),
            device.name.as_str().into(),
            device.created_at_ms.into(),
            device.last_seen_ms.into(),
        ],
    )
    .await?;
    Ok(())
}

pub async fn rename_device(
    db: &dyn crate::DbDriver,
    device_id: &str,
    name: &str,
) -> Result<bool, DbError> {
    let n = db
        .execute(
            "UPDATE signal_devices SET name = ? WHERE device_id = ?",
            &[name.into(), device_id.into()],
        )
        .await?;
    Ok(n > 0)
}

pub async fn delete_device(db: &dyn crate::DbDriver, device_id: &str) -> Result<bool, DbError> {
    let n = db
        .execute(
            "DELETE FROM signal_devices WHERE device_id = ?",
            &[device_id.into()],
        )
        .await?;
    Ok(n > 0)
}

pub async fn delete_all_devices(db: &dyn crate::DbDriver) -> Result<(), DbError> {
    db.execute("DELETE FROM signal_devices", &[]).await?;
    Ok(())
}

pub async fn touch_device(
    db: &dyn crate::DbDriver,
    device_id: &str,
    last_seen_ms: i64,
) -> Result<(), DbError> {
    db.execute(
        "UPDATE signal_devices SET last_seen_ms = ? WHERE device_id = ?",
        &[last_seen_ms.into(), device_id.into()],
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_driver() -> crate::SqlxSqliteDriver {
        let dir = tempfile::tempdir().unwrap();
        let pool = crate::open(&dir.path().join("ps-rs.db")).await.unwrap();
        std::mem::forget(dir);
        crate::SqlxSqliteDriver::new(pool)
    }

    fn sample(device_id: &str) -> SignalDevice {
        SignalDevice {
            device_id: device_id.to_string(),
            secret_hex: "deadbeef".to_string(),
            name: "Phone".to_string(),
            created_at_ms: 1_700_000_000_000,
            last_seen_ms: None,
        }
    }

    #[tokio::test]
    async fn insert_then_list_round_trips_the_row() {
        let db = test_driver().await;
        insert_device(&db, &sample("dev-1")).await.unwrap();

        let devices = list_devices(&db).await.unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].device_id, "dev-1");
        assert_eq!(devices[0].secret_hex, "deadbeef");
        assert_eq!(devices[0].name, "Phone");
        assert_eq!(devices[0].created_at_ms, 1_700_000_000_000);
        assert_eq!(devices[0].last_seen_ms, None);
    }

    #[tokio::test]
    async fn rename_device_returns_false_for_an_unknown_id() {
        let db = test_driver().await;
        insert_device(&db, &sample("dev-1")).await.unwrap();

        assert!(rename_device(&db, "dev-1", "Tablet").await.unwrap());
        assert_eq!(list_devices(&db).await.unwrap()[0].name, "Tablet");

        assert!(!rename_device(&db, "no-such-device", "X").await.unwrap());
    }

    #[tokio::test]
    async fn delete_device_returns_false_for_an_unknown_id() {
        let db = test_driver().await;
        insert_device(&db, &sample("dev-1")).await.unwrap();

        assert!(!delete_device(&db, "no-such-device").await.unwrap());
        assert!(delete_device(&db, "dev-1").await.unwrap());
        assert!(list_devices(&db).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn delete_all_devices_clears_the_table() {
        let db = test_driver().await;
        insert_device(&db, &sample("dev-1")).await.unwrap();
        insert_device(&db, &sample("dev-2")).await.unwrap();

        delete_all_devices(&db).await.unwrap();
        assert!(list_devices(&db).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn touch_device_updates_last_seen_ms() {
        let db = test_driver().await;
        insert_device(&db, &sample("dev-1")).await.unwrap();

        touch_device(&db, "dev-1", 1_700_000_500_000).await.unwrap();

        let devices = list_devices(&db).await.unwrap();
        assert_eq!(devices[0].last_seen_ms, Some(1_700_000_500_000));
    }

    #[test]
    fn debug_redacts_the_secret() {
        let device = sample("dev-1");
        let formatted = format!("{device:?}");
        assert!(!formatted.contains("deadbeef"), "secret leaked into Debug output");
        assert!(formatted.contains("<redacted>"));
        assert!(formatted.contains("dev-1"));
    }
}
