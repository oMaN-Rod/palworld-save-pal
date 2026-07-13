async fn test_pool() -> sqlx::SqlitePool {
    let dir = tempfile::tempdir().unwrap();
    let pool = psp_db::open(&dir.path().join("psp-rs.db")).await.unwrap();
    // keep temp dir alive for the pool's lifetime
    std::mem::forget(dir);
    pool
}

#[tokio::test]
async fn add_and_get_preset_with_pal_preset() {
    let pool = test_pool().await;
    let preset_id = psp_db::presets::add(
        &pool,
        serde_json::json!({
            "name": "Max Fox",
            "type": "pal_preset",
            "skills": null,
            "common_container": null,
            "essential_container": null,
            "weapon_load_out_container": null,
            "player_equipment_armor_container": null,
            "food_equip_container": null,
            "storage_container": null,
            "pal_preset": {
                "lock": true, "lock_element": false, "element": "Fire",
                "character_id": "Kitsunebi", "is_lucky": false, "is_boss": true,
                "gender": "Female", "rank_hp": 10, "rank_attack": 10, "rank_defense": 10,
                "rank_craftspeed": 10, "talent_hp": 100, "talent_shot": 100,
                "talent_defense": 100, "rank": 5, "level": 60, "exp": 0,
                "learned_skills": [], "active_skills": ["EPalWazaID::FireBall"],
                "passive_skills": ["Legend"], "sanity": 100.0,
                "work_suitability": {"EmitFlame": 4}, "nickname": "MaxFox",
                "filtered_nickname": "MaxFox", "stomach": 150.0, "hp": 10000,
                "friendship_point": 42
            }
        }),
    )
    .await
    .unwrap();

    let all = psp_db::presets::get_all(&pool).await.unwrap();
    let preset = all.get(&preset_id).expect("preset present, keyed by id");
    assert_eq!(preset["name"], "Max Fox");
    assert_eq!(preset["type"], "pal_preset");
    // pal_preset id generated server-side and mirrored into pal_preset_id
    assert!(preset["pal_preset"]["id"].is_string());
    assert_eq!(preset["pal_preset_id"], preset["pal_preset"]["id"]);
    assert_eq!(preset["pal_preset"]["nickname"], "MaxFox");
}

#[tokio::test]
async fn preset_without_pal_preset_omits_the_key() {
    let pool = test_pool().await;
    let preset_id = psp_db::presets::add(
        &pool,
        serde_json::json!({"name": "Kit", "type": "inventory",
            "common_container": [{"static_id": "Wood", "count": 999, "slot_index": 0}]}),
    )
    .await
    .unwrap();
    let all = psp_db::presets::get_all(&pool).await.unwrap();
    let preset = all.get(&preset_id).unwrap();
    // The key must be absent, not null.
    assert!(preset.get("pal_preset").is_none());
    assert_eq!(preset["pal_preset_id"], serde_json::Value::Null);
}

#[tokio::test]
async fn update_delete_nuke() {
    let pool = test_pool().await;
    let id = psp_db::presets::add(&pool, serde_json::json!({"name": "A", "type": "inventory"}))
        .await
        .unwrap();
    assert!(psp_db::presets::update_name(&pool, &id, "B").await.unwrap());
    assert!(!psp_db::presets::update_name(&pool, "no-such-id", "X")
        .await
        .unwrap());
    let all = psp_db::presets::get_all(&pool).await.unwrap();
    assert_eq!(all.get(&id).unwrap()["name"], "B");
    assert!(psp_db::presets::delete(&pool, &id).await.unwrap());
    assert!(!psp_db::presets::delete(&pool, &id).await.unwrap());
    psp_db::presets::add(&pool, serde_json::json!({"name": "C", "type": "inventory"}))
        .await
        .unwrap();
    psp_db::presets::nuke(&pool).await.unwrap();
    assert!(psp_db::presets::get_all(&pool).await.unwrap().is_empty());
}

#[tokio::test]
async fn populate_from_json_only_seeds_empty_table() {
    let pool = test_pool().await;
    let seed = serde_json::json!([
        {"id": "dddddddd-dddd-dddd-dddd-dddddddddddd", "name": "Seed", "type": "inventory"}
    ]);
    psp_db::presets::populate_from_json(&pool, &seed)
        .await
        .unwrap();
    assert_eq!(psp_db::presets::get_all(&pool).await.unwrap().len(), 1);
    psp_db::presets::populate_from_json(&pool, &seed)
        .await
        .unwrap();
    assert_eq!(psp_db::presets::get_all(&pool).await.unwrap().len(), 1);
}
