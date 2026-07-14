async fn test_pool() -> sqlx::SqlitePool {
    let dir = tempfile::tempdir().unwrap();
    let pool = psp_db::open(&dir.path().join("psp-rs.db")).await.unwrap();
    std::mem::forget(dir);
    pool
}

fn game_data() -> serde_json::Value {
    serde_json::json!({"SheepBall": {"element_types": ["Neutral"], "is_pal": true}})
}

async fn seed_pal(pool: &sqlx::SqlitePool) -> psp_db::ups::UpsPalRecord {
    psp_db::ups::add_pal(
        pool,
        psp_db::ups::NewUpsPal {
            character_id: "SheepBall".into(),
            nickname: Some("Fluffy".into()),
            level: 12,
            pal_data: serde_json::json!({"character_id": "SheepBall", "nickname": "Fluffy",
                "level": 12, "is_boss": false, "is_lucky": false}),
            source_save_file: None,
            source_player_uid: None,
            source_player_name: None,
            source_storage_type: None,
            source_storage_slot: None,
            collection_id: None,
            tags: vec!["shiny".into()],
            notes: None,
        },
        &game_data(),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn update_pal_syncs_columns_both_directions() {
    let pool = test_pool().await;
    let pal = seed_pal(&pool).await;

    // Updating the denormalized columns writes back into the pal_data JSON.
    let mut updates = serde_json::Map::new();
    updates.insert("nickname".into(), serde_json::json!("Rex"));
    updates.insert("level".into(), serde_json::json!(30));
    let updated = psp_db::ups::update_pal(&pool, pal.id, &updates)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.nickname.as_deref(), Some("Rex"));
    assert_eq!(updated.pal_data["nickname"], "Rex");
    assert_eq!(updated.pal_data["level"], 30);
    assert!(updated.updated_at.ends_with("+00:00"));

    // The sync runs the other way too: updating pal_data makes the columns follow the JSON.
    let mut updates = serde_json::Map::new();
    updates.insert(
        "pal_data".into(),
        serde_json::json!({"character_id": "Kitsunebi", "nickname": "Foxy", "level": 44}),
    );
    let updated = psp_db::ups::update_pal(&pool, pal.id, &updates)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.character_id, "Kitsunebi");
    assert_eq!(updated.nickname.as_deref(), Some("Foxy"));
    assert_eq!(updated.level, 44);

    assert!(
        psp_db::ups::update_pal(&pool, 9999, &serde_json::Map::new())
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn clone_delete_and_nuke() {
    let pool = test_pool().await;
    let pal = seed_pal(&pool).await;

    let clone = psp_db::ups::clone_pal(&pool, pal.id, &game_data())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(clone.nickname.as_deref(), Some("Fluffy (Clone)"));
    assert_eq!(clone.notes.as_deref(), Some("Clone of Fluffy"));
    assert_eq!(clone.source_storage_type.as_deref(), Some("ups_clone"));
    let original = psp_db::ups::get_pal_by_id(&pool, pal.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(original.clone_count, 1);

    let deleted = psp_db::ups::delete_pals(&pool, &[pal.id, 9999], &game_data())
        .await
        .unwrap();
    assert_eq!(deleted, 1);

    let nuked = psp_db::ups::nuke_all_pals(&pool, &game_data())
        .await
        .unwrap();
    assert_eq!(nuked, 1); // only the clone remained
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ups_pals")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(remaining, 0);
    let nuke_logs: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ups_transfer_log WHERE operation_type = 'nuke_delete'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(nuke_logs, 1);
}

#[tokio::test]
async fn tag_rename_and_delete_propagate_to_pals() {
    let pool = test_pool().await;
    let pal = seed_pal(&pool).await;
    let tag = psp_db::ups::create_or_update_tag(&pool, "shiny", None, Some("#0f0"))
        .await
        .unwrap();

    let mut updates = serde_json::Map::new();
    updates.insert("name".into(), serde_json::json!("sparkly"));
    psp_db::ups::update_tag(&pool, tag.id, &updates)
        .await
        .unwrap()
        .unwrap();
    let after_rename = psp_db::ups::get_pal_by_id(&pool, pal.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after_rename.tags, serde_json::json!(["sparkly"]));

    assert!(psp_db::ups::delete_tag(&pool, tag.id).await.unwrap());
    let after_delete = psp_db::ups::get_pal_by_id(&pool, pal.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after_delete.tags, serde_json::json!([]));
}

#[tokio::test]
async fn collection_update_and_delete_detach_pals() {
    let pool = test_pool().await;
    let collection = psp_db::ups::create_collection(&pool, "Favs", None, None)
        .await
        .unwrap();
    let pal = seed_pal(&pool).await;
    let mut updates = serde_json::Map::new();
    updates.insert("collection_id".into(), serde_json::json!(collection.id));
    psp_db::ups::update_pal(&pool, pal.id, &updates)
        .await
        .unwrap();

    let mut collection_updates = serde_json::Map::new();
    collection_updates.insert("is_favorite".into(), serde_json::json!(true));
    let updated = psp_db::ups::update_collection(&pool, collection.id, &collection_updates)
        .await
        .unwrap()
        .unwrap();
    assert!(updated.is_favorite);

    assert!(psp_db::ups::delete_collection(&pool, collection.id)
        .await
        .unwrap());
    let detached = psp_db::ups::get_pal_by_id(&pool, pal.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(detached.collection_id, None);
}
