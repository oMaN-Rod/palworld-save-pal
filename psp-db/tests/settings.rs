use psp_db::settings::{
    get_settings, saved_save_dir, update_save_dir, update_settings, SettingsUpdate,
};

#[tokio::test]
async fn first_get_inserts_python_default_row() {
    let temp_dir = tempfile::tempdir().unwrap();
    let pool = psp_db::open(&temp_dir.path().join("test.db"))
        .await
        .unwrap();

    let settings = get_settings(&pool).await.unwrap();

    assert_eq!(settings.language, "en");
    assert_eq!(settings.clone_prefix, "©️");
    assert_eq!(settings.new_pal_prefix, "🆕");
    assert!(!settings.debug_mode);
    assert!(!settings.cheat_mode);
    assert_eq!(
        settings.save_dir,
        psp_db::settings::default_steam_save_dir()
    );
}

#[tokio::test]
async fn concurrent_first_calls_all_return_ok_with_same_default_row() {
    let temp_dir = tempfile::tempdir().unwrap();
    let pool = psp_db::open(&temp_dir.path().join("test.db"))
        .await
        .unwrap();

    let mut handles = Vec::new();
    for _ in 0..8 {
        let pool = pool.clone();
        handles.push(tokio::spawn(async move { get_settings(&pool).await }));
    }

    for handle in handles {
        let settings = handle
            .await
            .expect("task panicked")
            .expect("get_settings must not error on a concurrent first call");

        assert_eq!(settings.language, "en");
        assert_eq!(settings.clone_prefix, "©️");
        assert_eq!(settings.new_pal_prefix, "🆕");
        assert!(!settings.debug_mode);
        assert!(!settings.cheat_mode);
        assert_eq!(
            settings.save_dir,
            psp_db::settings::default_steam_save_dir()
        );
    }
}

#[tokio::test]
async fn update_persists_everything_except_save_dir() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let pool = psp_db::open(&db_path).await.unwrap();
    get_settings(&pool).await.unwrap();

    // update_settings always binds default_steam_save_dir() to its save_dir placeholder,
    // so only a custom value can prove the ON CONFLICT branch left the column alone —
    // asserting against the default would pass even if it were overwritten.
    let custom_save_dir = "C:/custom/save/dir";
    sqlx::query("UPDATE settings SET save_dir = ?1 WHERE id = 1")
        .bind(custom_save_dir)
        .execute(&pool)
        .await
        .unwrap();

    let updated = update_settings(
        &pool,
        &SettingsUpdate {
            language: "fr".into(),
            clone_prefix: "copy-".into(),
            new_pal_prefix: "new-".into(),
            debug_mode: true,
            cheat_mode: true,
        },
    )
    .await
    .unwrap();

    assert_eq!(updated.language, "fr");
    assert!(updated.debug_mode);
    assert_eq!(
        updated.save_dir, custom_save_dir,
        "save_dir must not change"
    );

    // Survives reopen (persistence, migrations idempotent).
    drop(pool);
    let reopened = psp_db::open(&db_path).await.unwrap();
    let reloaded = get_settings(&reopened).await.unwrap();
    assert_eq!(reloaded.language, "fr");
    assert_eq!(reloaded.save_dir, custom_save_dir);
}

#[tokio::test]
async fn update_on_empty_db_creates_row_with_default_save_dir() {
    let temp_dir = tempfile::tempdir().unwrap();
    let pool = psp_db::open(&temp_dir.path().join("test.db"))
        .await
        .unwrap();

    let created = update_settings(
        &pool,
        &SettingsUpdate {
            language: "de".into(),
            clone_prefix: "©️".into(),
            new_pal_prefix: "🆕".into(),
            debug_mode: false,
            cheat_mode: false,
        },
    )
    .await
    .unwrap();

    assert_eq!(created.language, "de");
    assert_eq!(created.save_dir, psp_db::settings::default_steam_save_dir());
}

#[tokio::test]
async fn saved_save_dir_round_trips_through_update_save_dir() {
    let temp_dir = tempfile::tempdir().unwrap();
    let pool = psp_db::open(&temp_dir.path().join("test.db"))
        .await
        .unwrap();

    assert_eq!(
        saved_save_dir(&pool).await.unwrap(),
        None,
        "fresh DB has no settings row yet"
    );

    update_save_dir(&pool, "/saves/world-1").await.unwrap();

    assert_eq!(
        saved_save_dir(&pool).await.unwrap(),
        Some("/saves/world-1".to_string())
    );
}
