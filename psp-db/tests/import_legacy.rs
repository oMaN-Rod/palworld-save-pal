use std::path::PathBuf;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/legacy_psp.db")
}

/// Lenient stand-in for the real pal validator: any object with a string character_id passes.
fn test_validator(value: &serde_json::Value) -> Result<serde_json::Value, String> {
    match value.get("character_id") {
        Some(serde_json::Value::String(_)) => Ok(value.clone()),
        _ => Err("character_id is not a string".to_string()),
    }
}

#[tokio::test]
async fn imports_legacy_database_once() {
    let temp_dir = tempfile::tempdir().unwrap();
    let legacy_path = temp_dir.path().join("psp.db");
    std::fs::copy(fixture_path(), &legacy_path).unwrap();
    let pool = psp_db::open(&temp_dir.path().join("psp-rs.db"))
        .await
        .unwrap();

    let report =
        psp_db::import_legacy::import_legacy_if_needed(&pool, &legacy_path, &test_validator)
            .await
            .unwrap()
            .expect("first run must import");

    assert!(report.settings_imported);
    assert_eq!(report.presets_imported, 2);
    assert_eq!(report.ups_collections_imported, 2);
    assert_eq!(report.ups_tags_imported, 2);
    assert_eq!(report.ups_pals_imported, 2);
    assert_eq!(report.ups_pals_skipped, 1);
    assert_eq!(report.ups_transfer_log_imported, 1);
    assert!(report.ups_stats_imported);
    assert_eq!(report.servers_imported, 1);
    assert!(report.backup_path.exists());
    // The legacy file must come through byte-identical to its backup: import never writes to it.
    assert_eq!(
        std::fs::read(&legacy_path).unwrap(),
        std::fs::read(&report.backup_path).unwrap()
    );

    let language: String = sqlx::query_scalar("SELECT language FROM settings WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(language, "fr");

    // The legacy `palpreset` row folds into the pal_preset JSON column, gender NAME -> value.
    let pal_preset_json: Option<String> = sqlx::query_scalar(
        "SELECT pal_preset FROM presets WHERE id = 'bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let pal_preset: serde_json::Value = serde_json::from_str(&pal_preset_json.unwrap()).unwrap();
    assert_eq!(pal_preset["id"], "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
    assert_eq!(pal_preset["gender"], "Female");
    assert_eq!(pal_preset["nickname"], "MaxFox");

    // Legacy datetimes ("2026-01-02 03:04:05.123456") convert to ISO T-form.
    let created_at: String = sqlx::query_scalar("SELECT created_at FROM ups_pals WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(created_at, "2026-01-02T03:04:05.123456");

    // Legacy UUID columns are dashless; instance_id and source_player_uid must come out
    // canonical dashed-lowercase.
    let instance_id: String = sqlx::query_scalar("SELECT instance_id FROM ups_pals WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(instance_id, "44444444-4444-4444-4444-444444444444");

    let source_player_uid: String =
        sqlx::query_scalar("SELECT source_player_uid FROM ups_pals WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(source_player_uid, "55555555-5555-5555-5555-555555555555");

    let second =
        psp_db::import_legacy::import_legacy_if_needed(&pool, &legacy_path, &test_validator)
            .await
            .unwrap();
    assert!(second.is_none());
    let pal_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ups_pals")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(pal_count, 2);
}

#[tokio::test]
async fn missing_legacy_file_is_a_noop() {
    let temp_dir = tempfile::tempdir().unwrap();
    let pool = psp_db::open(&temp_dir.path().join("psp-rs.db"))
        .await
        .unwrap();
    let report = psp_db::import_legacy::import_legacy_if_needed(
        &pool,
        &temp_dir.path().join("psp.db"),
        &test_validator,
    )
    .await
    .unwrap();
    assert!(report.is_none());
}
