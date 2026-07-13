use psp_db::ups::NewUpsPal;

async fn test_pool() -> sqlx::SqlitePool {
    let dir = tempfile::tempdir().unwrap();
    let pool = psp_db::open(&dir.path().join("psp-rs.db")).await.unwrap();
    std::mem::forget(dir);
    pool
}

fn pals_game_data() -> serde_json::Value {
    serde_json::json!({
        "SheepBall": {"element_types": ["Neutral"], "is_pal": true},
        "Kitsunebi": {"element_types": ["Fire"], "is_pal": true},
        "Believer_CrossBow": {"element_types": [], "is_pal": false}
    })
}

fn new_pal(character_id: &str, is_boss: bool) -> NewUpsPal {
    NewUpsPal {
        character_id: character_id.to_string(),
        nickname: Some("Fluffy".to_string()),
        level: 12,
        pal_data: serde_json::json!({"character_id": character_id, "is_boss": is_boss,
            "is_lucky": false, "level": 12, "nickname": "Fluffy"}),
        source_save_file: Some("MyWorld".to_string()),
        source_player_uid: Some("55555555-5555-5555-5555-555555555555".to_string()),
        source_player_name: Some("Omar".to_string()),
        source_storage_type: Some("pal_box".to_string()),
        source_storage_slot: Some(3),
        collection_id: None,
        tags: vec!["shiny".to_string()],
        notes: None,
    }
}

#[tokio::test]
async fn add_pal_inserts_logs_and_updates_stats() {
    let pool = test_pool().await;
    let game_data = pals_game_data();
    let record = psp_db::ups::add_pal(&pool, new_pal("SheepBall", false), &game_data)
        .await
        .unwrap();
    assert_eq!(record.character_id, "SheepBall");
    assert_eq!(record.transfer_count, 0);
    assert!(uuid::Uuid::parse_str(&record.instance_id).is_ok());
    assert_eq!(record.tags, serde_json::json!(["shiny"]));

    let log_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ups_transfer_log WHERE operation_type = 'import' AND pal_id = ?",
    )
    .bind(record.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(log_count, 1);

    let stats = psp_db::ups::get_stats(&pool, &game_data).await.unwrap();
    assert_eq!(stats.total_pals, 1);
    assert_eq!(
        stats.most_popular_character_id.as_deref(),
        Some("SheepBall")
    );
    assert_eq!(stats.element_distribution, "{\"Neutral\":1}");
}

#[tokio::test]
async fn stats_count_special_categories_exclusively() {
    let pool = test_pool().await;
    let game_data = pals_game_data();
    psp_db::ups::add_pal(&pool, new_pal("Kitsunebi", true), &game_data)
        .await
        .unwrap();
    psp_db::ups::add_pal(&pool, new_pal("PREDATOR_Wolf", false), &game_data)
        .await
        .unwrap();
    psp_db::ups::add_pal(&pool, new_pal("Sheep_oilrig", false), &game_data)
        .await
        .unwrap();
    psp_db::ups::add_pal(&pool, new_pal("SUMMON_Rock", false), &game_data)
        .await
        .unwrap();
    psp_db::ups::add_pal(&pool, new_pal("Believer_CrossBow", false), &game_data)
        .await
        .unwrap();
    let stats = psp_db::ups::get_stats(&pool, &game_data).await.unwrap();
    assert_eq!(stats.alpha_count, 1);
    assert_eq!(stats.human_count, 1);
    // predator/oilrig/summon are mutually exclusive: a character_id counts toward at most one.
    assert_eq!(stats.predator_count, 1);
    assert_eq!(stats.oilrig_count, 1);
    assert_eq!(stats.summon_count, 1);
}

#[tokio::test]
async fn storage_size_mb_counts_bytes_not_chars() {
    let pool = test_pool().await;
    let game_data = pals_game_data();
    // Multi-byte nickname (ふわふわ = 4 chars, 12 UTF-8 bytes) makes byte count
    // diverge from char count, so a char-based LENGTH() reads too low.
    let mut pal = new_pal("SheepBall", false);
    pal.nickname = Some("ふわふわ".to_string());
    pal.pal_data = serde_json::json!({"character_id": "SheepBall", "is_boss": false,
        "is_lucky": false, "level": 12, "nickname": "ふわふわ"});
    psp_db::ups::add_pal(&pool, pal, &game_data).await.unwrap();

    let expected_bytes: i64 =
        sqlx::query_scalar("SELECT SUM(LENGTH(CAST(pal_data AS BLOB))) FROM ups_pals")
            .fetch_one(&pool)
            .await
            .unwrap();
    let expected_mb = expected_bytes as f64 / (1024.0 * 1024.0);

    let stats = psp_db::ups::get_stats(&pool, &game_data).await.unwrap();
    assert!(
        (stats.storage_size_mb - expected_mb).abs() < 1e-12,
        "storage_size_mb {} should equal byte-based {}",
        stats.storage_size_mb,
        expected_mb
    );
}

#[tokio::test]
async fn collection_counts_follow_membership() {
    let pool = test_pool().await;
    let game_data = pals_game_data();
    let collection = psp_db::ups::create_collection(&pool, "Favs", None, None)
        .await
        .unwrap();
    let mut pal = new_pal("SheepBall", false);
    pal.collection_id = Some(collection.id);
    psp_db::ups::add_pal(&pool, pal, &game_data).await.unwrap();
    let collections = psp_db::ups::get_collections(&pool).await.unwrap();
    assert_eq!(collections[0].pal_count, 1);
}
