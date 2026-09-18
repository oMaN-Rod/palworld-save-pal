use ps_db::DbDriver;

async fn test_driver() -> (ps_db::SqlxSqliteDriver, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let pool = ps_db::open(&dir.path().join("test.db")).await.unwrap();
    (ps_db::SqlxSqliteDriver::new(pool), dir)
}

fn a_mod(id: &str) -> ps_db::mod_library::NewMod {
    ps_db::mod_library::NewMod {
        id: id.to_string(),
        name: "Cool Mod".to_string(),
        custom_name: None,
        mod_type: "ue4ss".to_string(),
        author: None,
        summary: None,
        source_kind: "local".to_string(),
        source_ref: "{}".to_string(),
        nexus_mod_id: None,
        ignored_version: None,
        notes: None,
    }
}

fn a_version(mod_id: &str, version: &str) -> ps_db::mod_library::NewModVersion {
    ps_db::mod_library::NewModVersion {
        id: format!("{mod_id}@{version}"),
        mod_id: mod_id.to_string(),
        version: version.to_string(),
        archive_path: None,
        library_dir: format!("/lib/{mod_id}/{version}"),
        manifest: r#"{"routes":[]}"#.to_string(),
        source_ref: "{}".to_string(),
    }
}

#[tokio::test]
async fn a_mod_and_its_first_version_round_trip() {
    let (db, _dir) = test_driver().await;
    let row = ps_db::mod_library::upsert_mod(&db, &a_mod("coolmod-ue4ss"))
        .await
        .unwrap();
    assert_eq!(row.name, "Cool Mod");
    assert_eq!(row.custom_name, None);
    assert_eq!(row.nexus_mod_id, None);

    let v1 = ps_db::mod_library::insert_version(&db, &a_version("coolmod-ue4ss", "1.0"))
        .await
        .unwrap();
    assert!(v1.is_current, "the first version of a mod is current");
    assert_eq!(v1.manifest, r#"{"routes":[]}"#);
    assert_eq!(v1.archive_path, None);

    assert_eq!(
        ps_db::mod_library::versions_of(&db, "coolmod-ue4ss")
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn a_second_version_does_not_become_current_on_its_own() {
    let (db, _dir) = test_driver().await;
    ps_db::mod_library::upsert_mod(&db, &a_mod("m"))
        .await
        .unwrap();
    ps_db::mod_library::insert_version(&db, &a_version("m", "1.0"))
        .await
        .unwrap();
    let v2 = ps_db::mod_library::insert_version(&db, &a_version("m", "2.0"))
        .await
        .unwrap();
    assert!(!v2.is_current);
    assert_eq!(
        ps_db::mod_library::current_version(&db, "m")
            .await
            .unwrap()
            .unwrap()
            .id,
        "m@1.0"
    );

    ps_db::mod_library::set_current_version(&db, "m", "m@2.0")
        .await
        .unwrap();
    assert_eq!(
        ps_db::mod_library::current_version(&db, "m")
            .await
            .unwrap()
            .unwrap()
            .id,
        "m@2.0"
    );
    let currents = ps_db::mod_library::versions_of(&db, "m")
        .await
        .unwrap()
        .into_iter()
        .filter(|v| v.is_current)
        .count();
    assert_eq!(currents, 1, "exactly one version is current");
}

#[tokio::test]
async fn the_current_version_cannot_be_removed() {
    let (db, _dir) = test_driver().await;
    ps_db::mod_library::upsert_mod(&db, &a_mod("m"))
        .await
        .unwrap();
    ps_db::mod_library::insert_version(&db, &a_version("m", "1.0"))
        .await
        .unwrap();
    ps_db::mod_library::insert_version(&db, &a_version("m", "2.0"))
        .await
        .unwrap();

    let usage = ps_db::mod_library::version_usage(&db, "m@1.0")
        .await
        .unwrap();
    assert!(usage.is_current);
    assert!(usage.is_in_use());
    assert!(ps_db::mod_library::remove_version(&db, "m@1.0")
        .await
        .is_err());

    let usage = ps_db::mod_library::version_usage(&db, "m@2.0")
        .await
        .unwrap();
    assert!(!usage.is_in_use(), "{usage:?}");
    ps_db::mod_library::remove_version(&db, "m@2.0")
        .await
        .unwrap();
    assert_eq!(
        ps_db::mod_library::versions_of(&db, "m")
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn a_deployed_version_cannot_be_removed_and_blocks_its_mod() {
    let (db, _dir) = test_driver().await;
    ps_db::mod_library::upsert_mod(&db, &a_mod("m"))
        .await
        .unwrap();
    ps_db::mod_library::insert_version(&db, &a_version("m", "1.0"))
        .await
        .unwrap();
    ps_db::mod_library::insert_version(&db, &a_version("m", "2.0"))
        .await
        .unwrap();
    ps_db::mod_targets::upsert(
        &db,
        &ps_db::mod_targets::NewModTarget {
            id: "client-steam".to_string(),
            kind: "client".to_string(),
            name: "Steam".to_string(),
            root_path: "C:/g".to_string(),
            platform: "win64".to_string(),
            ue4ss_mode: "standard".to_string(),
            layout_overrides: "{}".to_string(),
            detected: "{}".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    db.execute(
        "INSERT INTO deployment_files (target_id, path, mod_version_id, hash, role, deployed_at) \
         VALUES ('client-steam', 'C:/g/a.lua', 'm@2.0', 'h', 'file', '2026-01-01T00:00:00')",
        &[],
    )
    .await
    .unwrap();

    let usage = ps_db::mod_library::version_usage(&db, "m@2.0")
        .await
        .unwrap();
    assert!(!usage.is_current);
    assert_eq!(usage.deployed_targets, vec!["client-steam".to_string()]);
    assert!(ps_db::mod_library::remove_version(&db, "m@2.0")
        .await
        .is_err());

    assert_eq!(
        ps_db::mod_library::mod_usage(&db, "m").await.unwrap(),
        vec!["client-steam".to_string()]
    );
    assert!(ps_db::mod_library::remove_mod(&db, "m").await.is_err());
}

#[tokio::test]
async fn removing_an_undeployed_mod_removes_its_versions() {
    let (db, _dir) = test_driver().await;
    ps_db::mod_library::upsert_mod(&db, &a_mod("m"))
        .await
        .unwrap();
    ps_db::mod_library::insert_version(&db, &a_version("m", "1.0"))
        .await
        .unwrap();
    assert!(ps_db::mod_library::remove_mod(&db, "m").await.unwrap());
    assert!(ps_db::mod_library::versions_of(&db, "m")
        .await
        .unwrap()
        .is_empty());
    assert!(ps_db::mod_library::get_mod(&db, "m")
        .await
        .unwrap()
        .is_none());
    assert!(!ps_db::mod_library::remove_mod(&db, "m").await.unwrap());
}

#[tokio::test]
async fn set_current_version_refuses_an_unknown_or_foreign_version() {
    let (db, _dir) = test_driver().await;
    ps_db::mod_library::upsert_mod(&db, &a_mod("m"))
        .await
        .unwrap();
    ps_db::mod_library::insert_version(&db, &a_version("m", "1.0"))
        .await
        .unwrap();
    ps_db::mod_library::upsert_mod(&db, &a_mod("other"))
        .await
        .unwrap();
    ps_db::mod_library::insert_version(&db, &a_version("other", "1.0"))
        .await
        .unwrap();

    assert!(
        ps_db::mod_library::set_current_version(&db, "m", "m@9.9")
            .await
            .is_err(),
        "a version that does not exist"
    );
    assert!(
        ps_db::mod_library::set_current_version(&db, "m", "other@1.0")
            .await
            .is_err(),
        "a version belonging to a different mod"
    );
    assert_eq!(
        ps_db::mod_library::current_version(&db, "m")
            .await
            .unwrap()
            .unwrap()
            .id,
        "m@1.0",
        "a refused call must not have cleared the current flag"
    );
}

#[tokio::test]
async fn an_open_apply_blocks_library_deletion() {
    let (db, _dir) = test_driver().await;
    ps_db::mod_targets::upsert(
        &db,
        &ps_db::mod_targets::NewModTarget {
            id: "client-steam".to_string(),
            kind: "client".to_string(),
            name: "Steam".to_string(),
            root_path: "C:/g".to_string(),
            platform: "win64".to_string(),
            ue4ss_mode: "standard".to_string(),
            layout_overrides: "{}".to_string(),
            detected: "{}".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    ps_db::mod_library::upsert_mod(&db, &a_mod("m"))
        .await
        .unwrap();
    ps_db::mod_library::insert_version(&db, &a_version("m", "1.0"))
        .await
        .unwrap();
    ps_db::mod_library::insert_version(&db, &a_version("m", "2.0"))
        .await
        .unwrap();

    // m@2.0 is not current and nothing references it, so it is removable.
    let usage = ps_db::mod_library::version_usage(&db, "m@2.0")
        .await
        .unwrap();
    assert!(!usage.is_in_use(), "{usage:?}");

    ps_db::mod_deployments::open_journal(&db, "client-steam", "op-1", r#"{"entries":[]}"#)
        .await
        .unwrap();

    let usage = ps_db::mod_library::version_usage(&db, "m@2.0")
        .await
        .unwrap();
    assert_eq!(usage.open_applies, vec!["client-steam".to_string()]);
    assert!(usage.is_in_use());
    assert!(ps_db::mod_library::remove_version(&db, "m@2.0")
        .await
        .is_err());
    assert!(ps_db::mod_library::remove_mod(&db, "m").await.is_err());

    ps_db::mod_deployments::close_journal(&db, "client-steam")
        .await
        .unwrap();
    ps_db::mod_library::remove_version(&db, "m@2.0")
        .await
        .unwrap();
}

#[tokio::test]
async fn ignored_version_is_set_and_cleared() {
    let (db, _dir) = test_driver().await;
    ps_db::mod_library::upsert_mod(&db, &a_mod("m"))
        .await
        .unwrap();

    assert!(
        ps_db::mod_library::set_ignored_version(&db, "m", Some("1.3.0"))
            .await
            .unwrap()
    );
    let row = ps_db::mod_library::get_mod(&db, "m")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.ignored_version.as_deref(), Some("1.3.0"));

    assert!(ps_db::mod_library::set_ignored_version(&db, "m", None)
        .await
        .unwrap());
    let row = ps_db::mod_library::get_mod(&db, "m")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.ignored_version, None);

    assert!(
        !ps_db::mod_library::set_ignored_version(&db, "missing", Some("1"))
            .await
            .unwrap()
    );
}
