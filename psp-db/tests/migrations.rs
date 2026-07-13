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
fn iso_naive_matches_python_isoformat() {
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
