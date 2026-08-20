use psp_db::plugins::{
    get, get_all, remove, seed_bundled, set_enabled, set_granted, set_manifest, set_sources,
    storage_get_all, storage_put_many, upsert, NewPlugin,
};

async fn open_db() -> (tempfile::TempDir, psp_db::SqlxSqliteDriver) {
    let temp_dir = tempfile::tempdir().unwrap();
    let pool = psp_db::open(&temp_dir.path().join("test.db")).await.unwrap();
    let db = psp_db::SqlxSqliteDriver::new(pool);
    (temp_dir, db)
}

#[tokio::test]
async fn get_all_is_empty_on_a_fresh_database() {
    let (_dir, db) = open_db().await;
    let all = get_all(&db).await.unwrap();
    assert!(all.is_empty());
}

#[tokio::test]
async fn an_upserted_plugin_round_trips_every_field() {
    let (_dir, db) = open_db().await;
    let new_plugin = NewPlugin {
        id: "pst.dedupe",
        manifest: r#"{"name":"Dedupe"}"#,
        sources: r#"{"main.lua":"return {}"}"#,
        granted_capabilities: r#"["storage"]"#,
        bundled: true,
    };

    let row = upsert(&db, &new_plugin).await.unwrap();

    assert_eq!(row.id, "pst.dedupe");
    assert_eq!(row.manifest, r#"{"name":"Dedupe"}"#);
    assert_eq!(row.sources, r#"{"main.lua":"return {}"}"#);
    assert_eq!(row.granted_capabilities, r#"["storage"]"#);
    assert!(row.bundled);
    assert!(row.enabled, "a freshly inserted plugin defaults to enabled");
    assert!(!row.installed_at.is_empty());
    assert!(!row.updated_at.is_empty());

    let fetched = get(&db, "pst.dedupe").await.unwrap().unwrap();
    assert_eq!(fetched, row);

    let all = get_all(&db).await.unwrap();
    assert_eq!(all, vec![row]);
}

#[tokio::test]
async fn upsert_replaces_an_existing_row_including_its_granted_capabilities() {
    let (_dir, db) = open_db().await;
    let first = NewPlugin {
        id: "pst.dedupe",
        manifest: r#"{"version":1}"#,
        sources: r#"{"main.lua":"old"}"#,
        granted_capabilities: r#"["storage"]"#,
        bundled: false,
    };
    upsert(&db, &first).await.unwrap();

    let second = NewPlugin {
        id: "pst.dedupe",
        manifest: r#"{"version":2}"#,
        sources: r#"{"main.lua":"new"}"#,
        granted_capabilities: r#"["storage","network"]"#,
        bundled: false,
    };
    let row = upsert(&db, &second).await.unwrap();

    assert_eq!(row.manifest, r#"{"version":2}"#);
    assert_eq!(row.sources, r#"{"main.lua":"new"}"#);
    assert_eq!(row.granted_capabilities, r#"["storage","network"]"#);

    let all = get_all(&db).await.unwrap();
    assert_eq!(all.len(), 1, "upsert on an existing id must not create a second row");
}

#[tokio::test]
async fn seed_bundled_refreshes_the_manifest_and_sources() {
    let (_dir, db) = open_db().await;
    let v1 = NewPlugin {
        id: "pst.egg-timer",
        manifest: r#"{"version":1}"#,
        sources: r#"{"main.lua":"return 1"}"#,
        granted_capabilities: r#"[]"#,
        bundled: true,
    };
    seed_bundled(&db, &v1).await.unwrap();

    let v2 = NewPlugin {
        id: "pst.egg-timer",
        manifest: r#"{"version":2}"#,
        sources: r#"{"main.lua":"return 2"}"#,
        granted_capabilities: r#"[]"#,
        bundled: true,
    };
    let row = seed_bundled(&db, &v2).await.unwrap();

    assert_eq!(row.manifest, r#"{"version":2}"#);
    assert_eq!(row.sources, r#"{"main.lua":"return 2"}"#);
}

#[tokio::test]
async fn seed_bundled_preserves_a_disabled_flag_the_user_set() {
    let (_dir, db) = open_db().await;
    let v1 = NewPlugin {
        id: "pst.egg-timer",
        manifest: r#"{"version":1}"#,
        sources: r#"{"main.lua":"return 1"}"#,
        granted_capabilities: r#"[]"#,
        bundled: true,
    };
    seed_bundled(&db, &v1).await.unwrap();

    let toggled = set_enabled(&db, "pst.egg-timer", false).await.unwrap();
    assert!(toggled);

    let v2 = NewPlugin {
        id: "pst.egg-timer",
        manifest: r#"{"version":2}"#,
        sources: r#"{"main.lua":"return 2"}"#,
        granted_capabilities: r#"[]"#,
        bundled: true,
    };
    let row = seed_bundled(&db, &v2).await.unwrap();

    assert!(!row.enabled, "re-seeding must not silently re-enable a disabled plugin");
    assert_eq!(row.manifest, r#"{"version":2}"#, "re-seeding must still refresh the manifest");
}

#[tokio::test]
async fn seed_bundled_preserves_granted_capabilities_the_user_chose() {
    let (_dir, db) = open_db().await;
    let v1 = NewPlugin {
        id: "pst.egg-timer",
        manifest: r#"{"version":1}"#,
        sources: r#"{"main.lua":"return 1"}"#,
        granted_capabilities: r#"[]"#,
        bundled: true,
    };
    seed_bundled(&db, &v1).await.unwrap();

    let granted_changed = set_granted(&db, "pst.egg-timer", r#"["storage","clock"]"#)
        .await
        .unwrap();
    assert!(granted_changed);

    let v2 = NewPlugin {
        id: "pst.egg-timer",
        manifest: r#"{"version":2}"#,
        sources: r#"{"main.lua":"return 2"}"#,
        granted_capabilities: r#"[]"#,
        bundled: true,
    };
    let row = seed_bundled(&db, &v2).await.unwrap();

    assert_eq!(
        row.granted_capabilities, r#"["storage","clock"]"#,
        "re-seeding must not clobber capabilities the user granted"
    );
}

#[tokio::test]
async fn set_enabled_toggles_and_reports_whether_a_row_matched() {
    let (_dir, db) = open_db().await;
    let new_plugin = NewPlugin {
        id: "pst.dedupe",
        manifest: "{}",
        sources: "{}",
        granted_capabilities: "[]",
        bundled: false,
    };
    upsert(&db, &new_plugin).await.unwrap();

    let matched = set_enabled(&db, "pst.dedupe", false).await.unwrap();
    assert!(matched);
    let row = get(&db, "pst.dedupe").await.unwrap().unwrap();
    assert!(!row.enabled);

    let matched_again = set_enabled(&db, "pst.dedupe", true).await.unwrap();
    assert!(matched_again);
    let row = get(&db, "pst.dedupe").await.unwrap().unwrap();
    assert!(row.enabled);
}

#[tokio::test]
async fn set_enabled_on_an_unknown_id_reports_false() {
    let (_dir, db) = open_db().await;
    let matched = set_enabled(&db, "does.not.exist", true).await.unwrap();
    assert!(!matched);
}

#[tokio::test]
async fn remove_deletes_the_row_and_reports_true() {
    let (_dir, db) = open_db().await;
    let new_plugin = NewPlugin {
        id: "pst.dedupe",
        manifest: "{}",
        sources: "{}",
        granted_capabilities: "[]",
        bundled: false,
    };
    upsert(&db, &new_plugin).await.unwrap();

    let removed = remove(&db, "pst.dedupe").await.unwrap();
    assert!(removed);
    assert_eq!(get(&db, "pst.dedupe").await.unwrap(), None);
}

#[tokio::test]
async fn remove_on_an_unknown_id_reports_false() {
    let (_dir, db) = open_db().await;
    let removed = remove(&db, "does.not.exist").await.unwrap();
    assert!(!removed);
}

#[tokio::test]
async fn remove_also_deletes_that_plugins_storage() {
    let (_dir, db) = open_db().await;
    let new_plugin = NewPlugin {
        id: "pst.dedupe",
        manifest: "{}",
        sources: "{}",
        granted_capabilities: "[]",
        bundled: false,
    };
    upsert(&db, &new_plugin).await.unwrap();
    storage_put_many(
        &db,
        "pst.dedupe",
        &[("count".to_string(), "3".to_string())],
    )
    .await
    .unwrap();

    remove(&db, "pst.dedupe").await.unwrap();

    let storage = storage_get_all(&db, "pst.dedupe").await.unwrap();
    assert!(storage.is_empty(), "removing the plugin must also clear its storage");
}

#[tokio::test]
async fn storage_put_many_inserts_and_then_updates_by_key() {
    let (_dir, db) = open_db().await;
    let new_plugin = NewPlugin {
        id: "pst.dedupe",
        manifest: "{}",
        sources: "{}",
        granted_capabilities: "[]",
        bundled: false,
    };
    upsert(&db, &new_plugin).await.unwrap();

    storage_put_many(
        &db,
        "pst.dedupe",
        &[
            ("a".to_string(), "1".to_string()),
            ("b".to_string(), "2".to_string()),
        ],
    )
    .await
    .unwrap();

    storage_put_many(&db, "pst.dedupe", &[("a".to_string(), "99".to_string())])
        .await
        .unwrap();

    let all = storage_get_all(&db, "pst.dedupe").await.unwrap();
    assert_eq!(all.len(), 2, "the update must not create a duplicate row for key `a`");
    assert_eq!(all.get("a"), Some(&"99".to_string()));
    assert_eq!(all.get("b"), Some(&"2".to_string()));
}

#[tokio::test]
async fn storage_get_all_is_scoped_to_one_plugin() {
    let (_dir, db) = open_db().await;
    for id in ["pst.a", "pst.b"] {
        upsert(
            &db,
            &NewPlugin {
                id,
                manifest: "{}",
                sources: "{}",
                granted_capabilities: "[]",
                bundled: false,
            },
        )
        .await
        .unwrap();
    }

    storage_put_many(&db, "pst.a", &[("x".to_string(), "from-a".to_string())])
        .await
        .unwrap();
    storage_put_many(&db, "pst.b", &[("x".to_string(), "from-b".to_string())])
        .await
        .unwrap();

    let a = storage_get_all(&db, "pst.a").await.unwrap();
    let b = storage_get_all(&db, "pst.b").await.unwrap();

    assert_eq!(a.get("x"), Some(&"from-a".to_string()));
    assert_eq!(b.get("x"), Some(&"from-b".to_string()));
}

#[tokio::test]
async fn updated_at_moves_on_upsert_but_installed_at_does_not() {
    let (_dir, db) = open_db().await;
    let v1 = NewPlugin {
        id: "pst.dedupe",
        manifest: r#"{"version":1}"#,
        sources: "{}",
        granted_capabilities: "[]",
        bundled: false,
    };
    let first = upsert(&db, &v1).await.unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(5)).await;

    let v2 = NewPlugin {
        id: "pst.dedupe",
        manifest: r#"{"version":2}"#,
        sources: "{}",
        granted_capabilities: "[]",
        bundled: false,
    };
    let second = upsert(&db, &v2).await.unwrap();

    assert_eq!(
        second.installed_at, first.installed_at,
        "installed_at must survive an upsert"
    );
    assert_ne!(
        second.updated_at, first.updated_at,
        "updated_at must move on an upsert"
    );
}

#[tokio::test]
async fn upsert_leaves_a_disabled_plugin_disabled() {
    let (_dir, db) = open_db().await;
    let v1 = NewPlugin {
        id: "pst.dedupe",
        manifest: r#"{"version":1}"#,
        sources: r#"{"main.lua":"old"}"#,
        granted_capabilities: "[]",
        bundled: false,
    };
    upsert(&db, &v1).await.unwrap();

    let disabled = set_enabled(&db, "pst.dedupe", false).await.unwrap();
    assert!(disabled);

    let v2 = NewPlugin {
        id: "pst.dedupe",
        manifest: r#"{"version":2}"#,
        sources: r#"{"main.lua":"new"}"#,
        granted_capabilities: "[]",
        bundled: false,
    };
    let row = upsert(&db, &v2).await.unwrap();

    assert!(!row.enabled, "upsert must not switch a disabled plugin back on");
    assert_eq!(row.manifest, r#"{"version":2}"#, "upsert must still refresh the manifest");
    assert_eq!(row.sources, r#"{"main.lua":"new"}"#, "upsert must still refresh sources");
}

#[tokio::test]
async fn set_sources_replaces_only_the_sources_column() {
    let (_dir, db) = open_db().await;
    upsert(
        &db,
        &NewPlugin {
            id: "pst.dedupe",
            manifest: r#"{"name":"Dedupe"}"#,
            sources: r#"{"main.lua":"return {}"}"#,
            granted_capabilities: r#"["storage"]"#,
            bundled: false,
        },
    )
    .await
    .unwrap();

    let matched = set_sources(&db, "pst.dedupe", r#"{"main.lua":"local x = 1"}"#)
        .await
        .unwrap();
    assert!(matched);

    let row = get(&db, "pst.dedupe").await.unwrap().unwrap();
    assert_eq!(row.sources, r#"{"main.lua":"local x = 1"}"#);
    assert_eq!(row.manifest, r#"{"name":"Dedupe"}"#);
    assert_eq!(row.granted_capabilities, r#"["storage"]"#);
    assert!(row.enabled);
}

#[tokio::test]
async fn set_manifest_replaces_only_the_manifest_column() {
    let (_dir, db) = open_db().await;
    upsert(
        &db,
        &NewPlugin {
            id: "pst.dedupe",
            manifest: r#"{"name":"Dedupe"}"#,
            sources: r#"{"main.lua":"return {}"}"#,
            granted_capabilities: r#"["storage"]"#,
            bundled: false,
        },
    )
    .await
    .unwrap();

    let matched = set_manifest(&db, "pst.dedupe", r#"{"name":"Deduplicate"}"#).await.unwrap();
    assert!(matched);

    let row = get(&db, "pst.dedupe").await.unwrap().unwrap();
    assert_eq!(row.manifest, r#"{"name":"Deduplicate"}"#);
    assert_eq!(row.sources, r#"{"main.lua":"return {}"}"#);
    assert_eq!(row.granted_capabilities, r#"["storage"]"#);
}

#[tokio::test]
async fn setting_sources_or_manifest_on_an_unknown_id_reports_false() {
    let (_dir, db) = open_db().await;
    assert!(!set_sources(&db, "does.not.exist", "{}").await.unwrap());
    assert!(!set_manifest(&db, "does.not.exist", "{}").await.unwrap());
}
