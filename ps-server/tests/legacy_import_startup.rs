use std::path::PathBuf;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../ps-db/tests/fixtures/legacy_psp.db")
}

fn repo_data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data")
}

#[tokio::test]
async fn startup_imports_legacy_db_next_to_new_db() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::copy(fixture_path(), temp_dir.path().join("psp.db")).unwrap();
    let ui_dir = temp_dir.path().join("ui");
    std::fs::create_dir_all(&ui_dir).unwrap();

    let config = ps_server::ServerConfig {
        host: "127.0.0.1".parse().unwrap(),
        port: Some(0),
        ui_dir,
        data_dir: repo_data_dir(),
        db_path: temp_dir.path().join("ps-rs.db"),
        desktop_mode: false,
        hosted: false,
        websuite: false,
        allow_network_edits: true,
    };
    let handle = ps_server::start_server(config).await.unwrap();

    let pool = ps_db::open(&temp_dir.path().join("ps-rs.db"))
        .await
        .unwrap();
    let language: String = sqlx::query_scalar("SELECT language FROM settings WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(language, "fr");
    // Valid rows imported, broken pal_data row rejected by the real PalDto validator.
    let pal_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ups_pals")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(pal_count, 2);
    assert!(temp_dir.path().join("psp.db.pre-rust-import.bak").exists());

    handle.shutdown().await;
}
