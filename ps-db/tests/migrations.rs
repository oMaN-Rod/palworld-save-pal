use ps_db::DbDriver;

#[tokio::test]
async fn migrations_create_full_phase3_schema() {
    let temp_dir = tempfile::tempdir().unwrap();
    let pool = ps_db::open(&temp_dir.path().join("ps-rs.db"))
        .await
        .unwrap();
    let tables: Vec<String> =
        sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .fetch_all(&pool)
            .await
            .unwrap();
    for expected in [
        "meta",
        "presets",
        "servers",
        "settings",
        "ups_collections",
        "ups_pals",
        "ups_stats",
        "ups_tags",
        "ups_transfer_log",
        "mod_targets",
        "mods",
        "mod_versions",
        "profiles",
        "profile_mods",
        "target_frameworks",
        "deployment_files",
        "apply_journal",
        "world_profiles",
    ] {
        assert!(
            tables.iter().any(|t| t == expected),
            "missing table {expected}"
        );
    }
}

#[tokio::test]
async fn meta_get_set_roundtrip() {
    let temp_dir = tempfile::tempdir().unwrap();
    let pool = ps_db::open(&temp_dir.path().join("ps-rs.db"))
        .await
        .unwrap();
    let db = ps_db::SqlxSqliteDriver::new(pool);
    assert_eq!(
        ps_db::meta::get(&db, "legacy_import").await.unwrap(),
        None
    );
    ps_db::meta::set(&db, "legacy_import", "{\"done\":true}")
        .await
        .unwrap();
    assert_eq!(
        ps_db::meta::get(&db, "legacy_import")
            .await
            .unwrap()
            .as_deref(),
        Some("{\"done\":true}")
    );
    ps_db::meta::set(&db, "legacy_import", "v2")
        .await
        .unwrap();
    assert_eq!(
        ps_db::meta::get(&db, "legacy_import")
            .await
            .unwrap()
            .as_deref(),
        Some("v2")
    );
}

#[tokio::test]
async fn run_migrations_adopts_the_legacy_tracker_table() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let db = ps_db::SqlxSqliteDriver::new(pool.clone());
    ps_db::run_migrations(&db).await.unwrap();
    sqlx::query("ALTER TABLE _ps_migrations RENAME TO _psp_migrations")
        .execute(&pool)
        .await
        .unwrap();

    ps_db::run_migrations(&db).await.unwrap();

    let trackers: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE name IN ('_psp_migrations', '_ps_migrations')",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(trackers, vec!["_ps_migrations".to_string()]);
    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _ps_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(applied, ps_db::MIGRATIONS.len() as i64);
}

#[test]
fn iso_naive_formats_without_timezone_suffix() {
    let with_micros = chrono::NaiveDate::from_ymd_opt(2026, 1, 2)
        .unwrap()
        .and_hms_micro_opt(3, 4, 5, 123_456)
        .unwrap();
    assert_eq!(
        ps_db::time::iso_naive(with_micros),
        "2026-01-02T03:04:05.123456"
    );
    let without_micros = chrono::NaiveDate::from_ymd_opt(2026, 1, 2)
        .unwrap()
        .and_hms_opt(3, 4, 5)
        .unwrap();
    assert_eq!(
        ps_db::time::iso_naive(without_micros),
        "2026-01-02T03:04:05"
    );
}

#[tokio::test]
async fn migration_eleven_adds_the_new_columns() {
    let dir = tempfile::tempdir().unwrap();
    let pool = ps_db::open(&dir.path().join("test.db")).await.unwrap();
    let db = ps_db::SqlxSqliteDriver::new(pool);
    let rows = db
        .query("SELECT name FROM pragma_table_info('servers')", &[])
        .await
        .unwrap();
    let names: Vec<String> = rows
        .iter()
        .map(|row| row.get_string("name").unwrap())
        .collect();
    assert!(names.contains(&"paks_path".to_string()), "{names:?}");
    assert!(
        names.contains(&"pending_relocation".to_string()),
        "{names:?}"
    );

    let rows = db
        .query("SELECT name FROM pragma_table_info('amity_instances')", &[])
        .await
        .unwrap();
    let names: Vec<String> = rows
        .iter()
        .map(|row| row.get_string("name").unwrap())
        .collect();
    assert!(names.contains(&"target_id".to_string()), "{names:?}");
}

/// `sqlx::migrate!` checksums migration files as they sit on disk; a CRLF checkout
/// (e.g. `core.autocrlf=true` on Windows) changes the checksum and makes sqlx
/// reject an already-applied migration. `.gitattributes` pins `*.sql` to `eol=lf`
/// for this reason — this test is the guard.
#[test]
fn migration_files_are_lf_only() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).expect("migrations dir") {
        let path = entry.expect("dir entry").path();
        if path.extension().is_none_or(|ext| ext != "sql") {
            continue;
        }
        let bytes = std::fs::read(&path).expect("read migration");
        let carriage_returns = bytes.iter().filter(|b| **b == b'\r').count();
        assert_eq!(
            carriage_returns,
            0,
            "{} has {carriage_returns} CR byte(s): a CRLF checkout changes its sqlx \
             checksum and breaks every existing database. Check .gitattributes pins \
             *.sql to eol=lf, then re-checkout the file.",
            path.display()
        );
        checked += 1;
    }
    assert!(checked > 0, "found no migration files to check");
}
