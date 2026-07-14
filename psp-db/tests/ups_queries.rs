use psp_db::ups::{PalTypeFilter, UpsFilter};

async fn test_pool() -> sqlx::SqlitePool {
    let dir = tempfile::tempdir().unwrap();
    let pool = psp_db::open(&dir.path().join("psp-rs.db")).await.unwrap();
    std::mem::forget(dir);
    pool
}

async fn insert_pal(
    pool: &sqlx::SqlitePool,
    character_id: &str,
    nickname: Option<&str>,
    level: i64,
    is_boss: bool,
    tags: &[&str],
    created_at: &str,
) -> i64 {
    let pal_data = serde_json::json!({"character_id": character_id, "is_boss": is_boss,
        "is_lucky": false, "level": level, "nickname": nickname});
    sqlx::query_scalar(
        "INSERT INTO ups_pals (instance_id, character_id, nickname, level, pal_data, tags, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(character_id)
    .bind(nickname)
    .bind(level)
    .bind(pal_data.to_string())
    .bind(serde_json::to_string(tags).unwrap())
    .bind(created_at)
    .bind(created_at)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn search_is_case_insensitive_over_three_columns() {
    let pool = test_pool().await;
    insert_pal(
        &pool,
        "SheepBall",
        Some("Fluffy"),
        10,
        false,
        &[],
        "2026-01-01T00:00:00",
    )
    .await;
    insert_pal(
        &pool,
        "Kitsunebi",
        None,
        20,
        false,
        &[],
        "2026-01-02T00:00:00",
    )
    .await;
    let filter = UpsFilter {
        search_query: Some("fluf".into()),
        ..Default::default()
    };
    let (pals, total) = psp_db::ups::get_pals(&pool, &filter, "created_at", "desc", 0, 30)
        .await
        .unwrap();
    assert_eq!(total, 1);
    assert_eq!(pals[0].character_id, "SheepBall");
}

#[tokio::test]
async fn character_id_filter_all_means_no_filter() {
    let pool = test_pool().await;
    insert_pal(
        &pool,
        "SheepBall",
        None,
        10,
        false,
        &[],
        "2026-01-01T00:00:00",
    )
    .await;
    insert_pal(
        &pool,
        "Kitsunebi",
        None,
        20,
        false,
        &[],
        "2026-01-02T00:00:00",
    )
    .await;
    let filter = UpsFilter {
        character_id_filter: Some("All".into()),
        ..Default::default()
    };
    let (_, total) = psp_db::ups::get_pals(&pool, &filter, "created_at", "desc", 0, 30)
        .await
        .unwrap();
    assert_eq!(total, 2);
}

#[tokio::test]
async fn tag_pal_type_sort_and_pagination() {
    let pool = test_pool().await;
    insert_pal(
        &pool,
        "SheepBall",
        Some("A"),
        1,
        false,
        &["shiny"],
        "2026-01-01T00:00:00",
    )
    .await;
    insert_pal(
        &pool,
        "BOSS_Kitsunebi",
        Some("B"),
        40,
        true,
        &[],
        "2026-01-02T00:00:00",
    )
    .await;
    insert_pal(
        &pool,
        "PREDATOR_Wolf",
        Some("C"),
        30,
        false,
        &["shiny"],
        "2026-01-03T00:00:00",
    )
    .await;

    let tag_filter = UpsFilter {
        tags: Some(vec!["shiny".into()]),
        ..Default::default()
    };
    let ids = psp_db::ups::get_all_filtered_ids(&pool, &tag_filter)
        .await
        .unwrap();
    assert_eq!(ids, vec![1, 3]);

    let alpha_filter = UpsFilter {
        pal_types: Some(vec![PalTypeFilter::Alpha, PalTypeFilter::Predator]),
        ..Default::default()
    };
    let ids = psp_db::ups::get_all_filtered_ids(&pool, &alpha_filter)
        .await
        .unwrap();
    assert_eq!(ids, vec![2, 3]);

    let (page, total) = psp_db::ups::get_pals(&pool, &UpsFilter::default(), "level", "asc", 0, 2)
        .await
        .unwrap();
    assert_eq!(total, 3);
    assert_eq!(
        page.iter().map(|p| p.level).collect::<Vec<_>>(),
        vec![1, 30]
    );
    // unknown sort key falls back to created_at desc
    let (page, _) =
        psp_db::ups::get_pals(&pool, &UpsFilter::default(), "no_such_column", "desc", 0, 1)
            .await
            .unwrap();
    assert_eq!(page[0].id, 3);
}

#[tokio::test]
async fn unknown_sort_order_defaults_to_ascending() {
    // Only "desc" means DESC; every other sort_order, valid or not, means ASC.
    let pool = test_pool().await;
    insert_pal(&pool, "A", None, 1, false, &[], "2026-01-01T00:00:00").await;
    insert_pal(&pool, "B", None, 40, false, &[], "2026-01-02T00:00:00").await;
    insert_pal(&pool, "C", None, 30, false, &[], "2026-01-03T00:00:00").await;

    let (page, _) = psp_db::ups::get_pals(&pool, &UpsFilter::default(), "level", "garbage", 0, 30)
        .await
        .unwrap();
    assert_eq!(
        page.iter().map(|p| p.level).collect::<Vec<_>>(),
        vec![1, 30, 40]
    );
}
