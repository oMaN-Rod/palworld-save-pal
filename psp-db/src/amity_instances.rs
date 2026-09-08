use crate::error::DbError;

#[derive(Clone)]
pub struct AmityInstance {
    pub id: i64,
    pub name: String,
    pub host: String,
    pub port: i64,
    pub token: String,
}

impl std::fmt::Debug for AmityInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AmityInstance")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("token", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, Default)]
pub struct NewAmityInstance {
    pub name: String,
    pub host: String,
    pub port: i64,
    pub token: String,
}

const SELECT_COLUMNS: &str = "id, name, host, port, token";

fn map_instance(r: &crate::DbRow) -> Result<AmityInstance, DbError> {
    Ok(AmityInstance {
        id: r.get_i64("id")?,
        name: r.get_string("name")?,
        host: r.get_string("host")?,
        port: r.get_i64("port")?,
        token: r.get_string("token")?,
    })
}

pub async fn list_instances(db: &dyn crate::DbDriver) -> Result<Vec<AmityInstance>, DbError> {
    let rows = db
        .query(&format!("SELECT {SELECT_COLUMNS} FROM amity_instances ORDER BY id"), &[])
        .await?;
    rows.iter().map(map_instance).collect()
}

pub async fn get_instance(db: &dyn crate::DbDriver, id: i64) -> Result<Option<AmityInstance>, DbError> {
    let rows = db
        .query(&format!("SELECT {SELECT_COLUMNS} FROM amity_instances WHERE id = ?"), &[id.into()])
        .await?;
    rows.first().map(map_instance).transpose()
}

pub async fn insert_instance(db: &dyn crate::DbDriver, new: &NewAmityInstance) -> Result<i64, DbError> {
    let now = crate::time::now_iso_naive_utc();
    db.execute(
        "INSERT INTO amity_instances (name, host, port, token, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
        &[
            new.name.as_str().into(),
            new.host.as_str().into(),
            new.port.into(),
            new.token.as_str().into(),
            now.as_str().into(),
            now.as_str().into(),
        ],
    )
    .await?;

    let rows = db.query("SELECT id FROM amity_instances ORDER BY id DESC LIMIT 1", &[]).await?;
    rows.first()
        .ok_or_else(|| DbError::Other("inserted instance not found".into()))?
        .get_i64("id")
}

pub async fn update_instance(db: &dyn crate::DbDriver, id: i64, new: &NewAmityInstance) -> Result<(), DbError> {
    db.execute(
        "UPDATE amity_instances SET name = ?, host = ?, port = ?, token = ?, updated_at = ? WHERE id = ?",
        &[
            new.name.as_str().into(),
            new.host.as_str().into(),
            new.port.into(),
            new.token.as_str().into(),
            crate::time::now_iso_naive_utc().as_str().into(),
            id.into(),
        ],
    )
    .await?;
    Ok(())
}

pub async fn delete_instance(db: &dyn crate::DbDriver, id: i64) -> Result<(), DbError> {
    db.execute("DELETE FROM amity_instances WHERE id = ?", &[id.into()]).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_driver() -> crate::SqlxSqliteDriver {
        let dir = tempfile::tempdir().unwrap();
        let pool = crate::open(&dir.path().join("psp.db")).await.unwrap();
        std::mem::forget(dir);
        crate::SqlxSqliteDriver::new(pool)
    }

    #[tokio::test]
    async fn insert_then_list_round_trips() {
        let db = test_driver().await;
        let id = insert_instance(&db, &NewAmityInstance {
            name: "Remote box".into(), host: "10.0.0.14".into(), port: 8788, token: "s3cr3t".into(),
        }).await.unwrap();

        let all = list_instances(&db).await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, id);
        assert_eq!(all[0].name, "Remote box");
        assert_eq!(all[0].host, "10.0.0.14");
        assert_eq!(all[0].port, 8788);
        assert_eq!(all[0].token, "s3cr3t");
    }

    #[tokio::test]
    async fn get_returns_none_for_a_missing_id() {
        let db = test_driver().await;
        assert!(get_instance(&db, 404).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn update_replaces_every_field() {
        let db = test_driver().await;
        let id = insert_instance(&db, &NewAmityInstance {
            name: "Old".into(), host: "127.0.0.1".into(), port: 1024, token: "old".into(),
        }).await.unwrap();
        update_instance(&db, id, &NewAmityInstance {
            name: "New".into(), host: "10.0.0.2".into(), port: 9999, token: "new".into(),
        }).await.unwrap();

        let found = get_instance(&db, id).await.unwrap().unwrap();
        assert_eq!(found.name, "New");
        assert_eq!(found.host, "10.0.0.2");
        assert_eq!(found.port, 9999);
        assert_eq!(found.token, "new");
    }

    #[tokio::test]
    async fn delete_removes_the_row() {
        let db = test_driver().await;
        let id = insert_instance(&db, &NewAmityInstance {
            name: "Gone".into(), host: "127.0.0.1".into(), port: 8788, token: "t".into(),
        }).await.unwrap();
        delete_instance(&db, id).await.unwrap();
        assert!(get_instance(&db, id).await.unwrap().is_none());
        assert!(list_instances(&db).await.unwrap().is_empty());
    }

    #[test]
    fn debug_redacts_the_token() {
        let instance = AmityInstance {
            id: 1, name: "n".into(), host: "h".into(), port: 1, token: "super-secret".into(),
        };
        let rendered = format!("{instance:?}");
        assert!(!rendered.contains("super-secret"));
        assert!(rendered.contains("redacted"));
    }
}
