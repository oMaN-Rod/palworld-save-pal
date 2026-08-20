use std::path::PathBuf;

fn repo_data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data")
}

/// Proves the plugins table is already populated with the bundled set right
/// after `start_server` returns, without the test calling
/// `seed_bundled_plugins` itself.
#[tokio::test]
async fn startup_seeds_the_bundled_plugin_set_without_an_explicit_call() {
    let temp_dir = tempfile::tempdir().unwrap();
    let ui_dir = temp_dir.path().join("ui");
    std::fs::create_dir_all(&ui_dir).unwrap();

    let config = psp_server::ServerConfig {
        host: "127.0.0.1".parse().unwrap(),
        port: 0,
        ui_dir,
        data_dir: repo_data_dir(),
        db_path: temp_dir.path().join("psp-rs.db"),
        desktop_mode: false,
    };
    let handle = psp_server::start_server(config).await.unwrap();

    let rows = psp_db::plugins::get_all(&*handle.app.driver).await.unwrap();
    let bundled_ids: Vec<&str> =
        handle.app.plugins.bundled().iter().map(|plugin| plugin.id).collect();
    assert_eq!(
        rows.len(),
        bundled_ids.len(),
        "the plugins table must hold exactly the bundled set right after startup"
    );
    for row in &rows {
        assert!(row.bundled, "every seeded row must be marked bundled");
        assert!(
            bundled_ids.contains(&row.id.as_str()),
            "row {} is not one of the bundled plugin ids",
            row.id
        );
    }

    handle.shutdown().await;
}
