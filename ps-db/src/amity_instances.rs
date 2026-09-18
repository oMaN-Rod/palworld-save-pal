use crate::error::DbError;

#[derive(Clone)]
pub struct AmityInstance {
    pub id: i64,
    pub name: String,
    pub host: String,
    pub port: i64,
    pub token: String,
    pub target_id: Option<String>,
}

impl std::fmt::Debug for AmityInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AmityInstance")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("token", &"<redacted>")
            .field("target_id", &self.target_id)
            .finish()
    }
}

#[derive(Clone, Default)]
pub struct NewAmityInstance {
    pub name: String,
    pub host: String,
    pub port: i64,
    pub token: String,
}

impl std::fmt::Debug for NewAmityInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NewAmityInstance")
            .field("name", &self.name)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("token", &"<redacted>")
            .finish()
    }
}

const SELECT_COLUMNS: &str = "id, name, host, port, token, target_id";

fn map_instance(r: &crate::DbRow) -> Result<AmityInstance, DbError> {
    Ok(AmityInstance {
        id: r.get_i64("id")?,
        name: r.get_string("name")?,
        host: r.get_string("host")?,
        port: r.get_i64("port")?,
        token: r.get_string("token")?,
        target_id: r.get_opt_str("target_id")?,
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
    crate::scalar_i64(
        &db.query(
            "INSERT INTO amity_instances (name, host, port, token, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
            &[
                new.name.as_str().into(),
                new.host.as_str().into(),
                new.port.into(),
                new.token.as_str().into(),
                now.as_str().into(),
                now.as_str().into(),
            ],
        )
        .await?,
    )
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

pub async fn set_target(db: &dyn crate::DbDriver, id: i64, target_id: Option<&str>) -> Result<(), DbError> {
    db.execute(
        "UPDATE amity_instances SET target_id = ?, updated_at = ? WHERE id = ?",
        &[
            target_id.map(str::to_string).into(),
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
        let pool = crate::open(&dir.path().join("ps-rs.db")).await.unwrap();
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
    async fn insert_twice_returns_distinct_correct_ids() {
        let db = test_driver().await;
        let first = insert_instance(&db, &NewAmityInstance {
            name: "First".into(), host: "10.0.0.1".into(), port: 1111, token: "a".into(),
        }).await.unwrap();
        let second = insert_instance(&db, &NewAmityInstance {
            name: "Second".into(), host: "10.0.0.2".into(), port: 2222, token: "b".into(),
        }).await.unwrap();

        assert_ne!(first, second);
        assert_eq!(get_instance(&db, first).await.unwrap().unwrap().name, "First");
        assert_eq!(get_instance(&db, second).await.unwrap().unwrap().name, "Second");
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
            target_id: Some("client-steam".into()),
        };
        let rendered = format!("{instance:?}");
        assert!(!rendered.contains("super-secret"));
        assert!(rendered.contains("redacted"));
        assert!(rendered.contains("client-steam"));
    }

    #[tokio::test]
    async fn target_id_starts_unset_and_survives_an_edit() {
        let db = test_driver().await;
        crate::mod_targets::upsert(&db, &crate::mod_targets::NewModTarget {
            id: "client-steam".into(), kind: "client".into(), server_id: None,
            name: "Steam".into(), root_path: "/".into(), platform: "win64".into(),
            ue4ss_mode: "none".into(), layout_overrides: "{}".into(), detected: "{}".into(),
        }).await.unwrap();
        let id = insert_instance(&db, &NewAmityInstance {
            name: "Box".into(), host: "10.0.0.1".into(), port: 8788, token: "t".into(),
        }).await.unwrap();
        assert_eq!(get_instance(&db, id).await.unwrap().unwrap().target_id, None);

        set_target(&db, id, Some("client-steam")).await.unwrap();
        assert_eq!(
            get_instance(&db, id).await.unwrap().unwrap().target_id.as_deref(),
            Some("client-steam"),
        );

        update_instance(&db, id, &NewAmityInstance {
            name: "Renamed".into(), host: "10.0.0.2".into(), port: 9999, token: "t2".into(),
        }).await.unwrap();
        let found = get_instance(&db, id).await.unwrap().unwrap();
        assert_eq!(found.name, "Renamed");
        assert_eq!(found.target_id.as_deref(), Some("client-steam"), "an edit must not clear the pairing");

        set_target(&db, id, None).await.unwrap();
        assert_eq!(get_instance(&db, id).await.unwrap().unwrap().target_id, None);
    }

    #[test]
    fn new_amity_instance_debug_redacts_the_token() {
        let new = NewAmityInstance {
            name: "n".into(), host: "h".into(), port: 1, token: "super-secret".into(),
        };
        let rendered = format!("{new:?}");
        assert!(!rendered.contains("super-secret"));
        assert!(rendered.contains("redacted"));
    }
}
