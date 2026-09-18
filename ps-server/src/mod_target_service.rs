use std::path::Path;

pub fn servers_root_from(app_root: &Path) -> String {
    app_root.join("servers").to_string_lossy().into_owned()
}

pub async fn ensure_all(
    db: &dyn ps_db::DbDriver,
    app_root: &Path,
) -> Result<Vec<String>, ps_db::DbError> {
    ps_db::mod_targets::ensure_server_targets(db, &servers_root_from(app_root)).await
}

pub async fn ensure_for(
    db: &dyn ps_db::DbDriver,
    server: &ps_db::servers::ServerRecord,
    app_root: &Path,
) -> Result<Option<String>, ps_db::DbError> {
    ps_db::mod_targets::ensure_server_target(db, server, &servers_root_from(app_root)).await
}

pub async fn forget(db: &dyn ps_db::DbDriver, server_id: i64) -> Result<bool, ps_db::DbError> {
    let target_id = format!("server-{server_id}");
    let removed = ps_db::mod_targets::remove(db, &target_id).await?;
    if let Err(error) = crate::services::mods::library::drop_released(db, &target_id).await {
        tracing::warn!(%error, %target_id, "failed to drop released workshop packages");
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn forgetting_a_server_drops_its_released_workshop_packages() {
        let (db, _dir) = test_driver().await;
        let record = a_docker_server(&db).await;
        ps_db::mod_targets::ensure_server_target(&db, &record, "/srv")
            .await
            .unwrap();
        let target_id = format!("server-{}", record.id);
        ps_db::meta::set(
            &db,
            &format!("mods.released_workshop_packages.{target_id}"),
            r#"["CoolPack"]"#,
        )
        .await
        .unwrap();
        forget(&db, record.id).await.unwrap();
        assert!(crate::services::mods::library::released_packages(&db, &target_id)
            .await
            .unwrap()
            .is_empty());
    }

    async fn test_driver() -> (ps_db::SqlxSqliteDriver, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let pool = ps_db::open(&dir.path().join("test.db")).await.unwrap();
        (ps_db::SqlxSqliteDriver::new(pool), dir)
    }

    async fn a_docker_server(db: &ps_db::SqlxSqliteDriver) -> ps_db::servers::ServerRecord {
        ps_db::servers::create_server(
            db,
            ps_db::servers::NewServer {
                name: "Alpha".to_string(),
                container_name: "alpha".to_string(),
                server_type: "docker".to_string(),
                mods_path: "/srv/alpha/mods".to_string(),
                logicmods_path: "/srv/alpha/logicmods".to_string(),
                nativemods_path: "/srv/alpha/nativemods".to_string(),
                paks_path: "/srv/alpha/paks".to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap()
    }

    #[test]
    fn the_servers_root_keeps_the_hosts_separators() {
        let root = servers_root_from(std::path::Path::new("/home/u/.psp"));
        assert_eq!(
            root,
            std::path::Path::new("/home/u/.psp")
                .join("servers")
                .to_string_lossy()
        );
        #[cfg(windows)]
        assert_eq!(
            servers_root_from(std::path::Path::new(r"C:\ps\data")),
            r"C:\ps\data\servers"
        );
    }

    #[tokio::test]
    async fn ensure_for_creates_the_pair_once_and_forget_removes_it() {
        let (db, dir) = test_driver().await;
        let server = a_docker_server(&db).await;

        let created = ensure_for(&db, &server, dir.path()).await.unwrap();
        assert_eq!(created, Some(format!("server-{}", server.id)));
        assert!(ensure_for(&db, &server, dir.path()).await.unwrap().is_none());

        let target = ps_db::mod_targets::for_server(&db, server.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(target.kind, "server");
        assert_eq!(
            Path::new(&target.root_path),
            dir.path().join("servers").join("alpha")
        );
        assert_eq!(
            ps_db::mod_profiles::for_target(&db, &target.id)
                .await
                .unwrap()
                .len(),
            1
        );

        assert!(forget(&db, server.id).await.unwrap());
        assert!(ps_db::mod_targets::for_server(&db, server.id)
            .await
            .unwrap()
            .is_none());
        assert!(
            ps_db::mod_profiles::for_target(&db, &target.id)
                .await
                .unwrap()
                .is_empty(),
            "the profile goes with the target"
        );
        assert!(!forget(&db, server.id).await.unwrap());
    }

    #[tokio::test]
    async fn ensure_all_reconciles_what_exists() {
        let (db, dir) = test_driver().await;
        let server = a_docker_server(&db).await;
        assert_eq!(
            ensure_all(&db, dir.path()).await.unwrap(),
            vec![format!("server-{}", server.id)]
        );
        assert!(ensure_all(&db, dir.path()).await.unwrap().is_empty());
    }
}
