#[tokio::test]
async fn migrations_create_full_phase3_schema() {
    let temp_dir = tempfile::tempdir().unwrap();
    let pool = psp_db::open(&temp_dir.path().join("psp-rs.db"))
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
    let pool = psp_db::open(&temp_dir.path().join("psp-rs.db"))
        .await
        .unwrap();
    assert_eq!(
        psp_db::meta::get(&pool, "legacy_import").await.unwrap(),
        None
    );
    psp_db::meta::set(&pool, "legacy_import", "{\"done\":true}")
        .await
        .unwrap();
    assert_eq!(
        psp_db::meta::get(&pool, "legacy_import")
            .await
            .unwrap()
            .as_deref(),
        Some("{\"done\":true}")
    );
    // set is an upsert
    psp_db::meta::set(&pool, "legacy_import", "v2")
        .await
        .unwrap();
    assert_eq!(
        psp_db::meta::get(&pool, "legacy_import")
            .await
            .unwrap()
            .as_deref(),
        Some("v2")
    );
}

#[test]
fn iso_naive_formats_without_timezone_suffix() {
    let with_micros = chrono::NaiveDate::from_ymd_opt(2026, 1, 2)
        .unwrap()
        .and_hms_micro_opt(3, 4, 5, 123_456)
        .unwrap();
    assert_eq!(
        psp_db::time::iso_naive(with_micros),
        "2026-01-02T03:04:05.123456"
    );
    let without_micros = chrono::NaiveDate::from_ymd_opt(2026, 1, 2)
        .unwrap()
        .and_hms_opt(3, 4, 5)
        .unwrap();
    // The fraction is dropped entirely when microsecond == 0.
    assert_eq!(
        psp_db::time::iso_naive(without_micros),
        "2026-01-02T03:04:05"
    );
}

/// Migration files must be LF-only on every platform.
///
/// `sqlx::migrate!` embeds each file with `include_str!` and checksums the bytes
/// exactly as they sit on disk. Every database in the wild was migrated from LF
/// files, so a CRLF checkout changes the checksum and makes sqlx reject an
/// already-applied migration -- "migration N was previously applied but has been
/// modified" -- even though the SQL is byte-for-byte identical apart from the line
/// endings. On Windows `core.autocrlf=true` does exactly that on checkout, which
/// is why `.gitattributes` pins `*.sql` to `eol=lf`. This test is the guard.
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
