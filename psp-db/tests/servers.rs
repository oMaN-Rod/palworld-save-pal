use psp_db::servers::{
    allocated_ports, create_server, delete_server, get_server, list_servers,
    server_with_install_path, update_server, NewServer,
};

async fn test_pool() -> (sqlx::SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let pool = psp_db::open(&dir.path().join("test.db")).await.unwrap();
    (pool, dir)
}

fn sample_server(container_name: &str) -> NewServer {
    NewServer {
        name: "My Server".to_string(),
        container_name: container_name.to_string(),
        image_name: "omanrod/psp-palworld-server".to_string(),
        server_type: "docker".to_string(),
        game_port: 8211,
        query_port: 27015,
        rest_api_port: 8212,
        data_volume_name: format!("psp-{container_name}-data"),
        saves_path: "/srv/saves".to_string(),
        mods_path: "/srv/mods".to_string(),
        logicmods_path: "/srv/logicmods".to_string(),
        nativemods_path: "/srv/nativemods".to_string(),
        install_path: String::new(),
        steamcmd_path: String::new(),
        launch_args: String::new(),
        workshop_dir: String::new(),
        server_name: "PSP Palworld Server".to_string(),
        server_description: String::new(),
        server_password: String::new(),
        admin_password: "admin".to_string(),
        max_players: 16,
        env_vars: serde_json::Map::new(),
    }
}

#[tokio::test]
async fn create_then_get_round_trips_all_fields() {
    let (pool, _dir) = test_pool().await;
    let created = create_server(&pool, sample_server("alpha")).await.unwrap();
    assert!(created.id >= 1);
    assert!(created.pid.is_none());
    let fetched = get_server(&pool, created.id).await.unwrap().unwrap();
    assert_eq!(fetched.container_name, "alpha");
    assert_eq!(fetched.image_name, "omanrod/psp-palworld-server");
    assert_eq!(fetched.server_type, "docker");
    assert_eq!(fetched.game_port, 8211);
    assert_eq!(fetched.query_port, 27015);
    assert_eq!(fetched.rest_api_port, 8212);
    assert_eq!(fetched.data_volume_name, "psp-alpha-data");
    assert_eq!(fetched.max_players, 16);
    assert_eq!(fetched.admin_password, "admin");
    assert!(fetched.env_vars.0.is_empty());
    assert_eq!(fetched.created_at, created.created_at);
    assert_eq!(fetched.updated_at, created.updated_at);
    // Server timestamps are naive UTC: T-separated, no space, and no offset suffix.
    assert!(
        created.created_at.contains('T') && !created.created_at.contains(' '),
        "created_at must be T-separated ISO, got {:?}",
        created.created_at
    );
    assert!(
        !created.created_at.contains("+00:00"),
        "created_at must not carry a UTC offset suffix, got {:?}",
        created.created_at
    );
    assert!(
        created.updated_at.contains('T') && !created.updated_at.contains(' '),
        "updated_at must be T-separated ISO, got {:?}",
        created.updated_at
    );
}

#[tokio::test]
async fn get_server_returns_none_for_unknown_id() {
    let (pool, _dir) = test_pool().await;
    assert!(get_server(&pool, 999).await.unwrap().is_none());
}

#[tokio::test]
async fn list_servers_orders_by_created_at() {
    let (pool, _dir) = test_pool().await;
    create_server(&pool, sample_server("first")).await.unwrap();
    create_server(&pool, sample_server("second")).await.unwrap();
    let all = list_servers(&pool).await.unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].container_name, "first");
    assert_eq!(all[1].container_name, "second");
}

#[tokio::test]
async fn update_server_applies_partial_updates_and_ignores_unknown_keys() {
    let (pool, _dir) = test_pool().await;
    let created = create_server(&pool, sample_server("beta")).await.unwrap();
    let mut updates = serde_json::Map::new();
    updates.insert("pid".to_string(), serde_json::json!(4242));
    updates.insert("max_players".to_string(), serde_json::json!(32));
    updates.insert(
        "env_vars".to_string(),
        serde_json::json!({"EXP_RATE": "2.0"}),
    );
    updates.insert("not_a_column".to_string(), serde_json::json!("ignored"));
    let updated = update_server(&pool, created.id, &updates)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.pid, Some(4242));
    assert_eq!(updated.max_players, 32);
    assert_eq!(updated.container_name, "beta");
    assert_eq!(
        updated.env_vars.0.get("EXP_RATE"),
        Some(&serde_json::json!("2.0"))
    );
    assert!(updated.updated_at >= created.updated_at);
    assert!(
        updated.updated_at.contains('T') && !updated.updated_at.contains(' '),
        "updated_at must remain a T-separated ISO string after update, got {:?}",
        updated.updated_at
    );

    let mut null_pid = serde_json::Map::new();
    null_pid.insert("pid".to_string(), serde_json::Value::Null);
    let cleared = update_server(&pool, created.id, &null_pid)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(cleared.pid, None);
    assert_eq!(cleared.max_players, 32);
}

#[tokio::test]
async fn update_server_returns_none_for_unknown_id() {
    let (pool, _dir) = test_pool().await;
    let updates = serde_json::Map::new();
    assert!(update_server(&pool, 999, &updates).await.unwrap().is_none());
}

#[tokio::test]
async fn update_server_with_no_recognized_keys_is_a_no_op() {
    let (pool, _dir) = test_pool().await;
    let created = create_server(&pool, sample_server("no-op")).await.unwrap();
    let mut updates = serde_json::Map::new();
    updates.insert("not_a_column".to_string(), serde_json::json!("ignored"));
    let updated = update_server(&pool, created.id, &updates)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.container_name, "no-op");
    assert_eq!(updated.max_players, created.max_players);
    assert_eq!(updated.pid, created.pid);
}

#[tokio::test]
async fn delete_server_reports_existence() {
    let (pool, _dir) = test_pool().await;
    let created = create_server(&pool, sample_server("gamma")).await.unwrap();
    assert!(delete_server(&pool, created.id).await.unwrap());
    assert!(!delete_server(&pool, created.id).await.unwrap());
    assert!(get_server(&pool, created.id).await.unwrap().is_none());
}

#[tokio::test]
async fn allocated_ports_collects_all_three_port_columns() {
    let (pool, _dir) = test_pool().await;
    create_server(&pool, sample_server("delta")).await.unwrap();
    let ports = allocated_ports(&pool).await.unwrap();
    assert_eq!(ports.len(), 3);
    assert!(ports.contains(&8211) && ports.contains(&27015) && ports.contains(&8212));
}

#[tokio::test]
async fn allocated_ports_dedupes_shared_port_values_across_servers() {
    let (pool, _dir) = test_pool().await;
    let mut first = sample_server("eps-1");
    first.game_port = 9000;
    first.query_port = 9001;
    first.rest_api_port = 9002;
    create_server(&pool, first).await.unwrap();

    let mut second = sample_server("eps-2");
    second.game_port = 9000; // duplicate of first's game_port
    second.query_port = 9003;
    second.rest_api_port = 9004;
    create_server(&pool, second).await.unwrap();

    let ports = allocated_ports(&pool).await.unwrap();
    assert_eq!(ports.len(), 5);
    assert!(ports.contains(&9000));
    assert!(ports.contains(&9001));
    assert!(ports.contains(&9002));
    assert!(ports.contains(&9003));
    assert!(ports.contains(&9004));
}

#[tokio::test]
async fn create_server_rejects_duplicate_container_name() {
    let (pool, _dir) = test_pool().await;
    create_server(&pool, sample_server("dup")).await.unwrap();
    let result = create_server(&pool, sample_server("dup")).await;
    assert!(result.is_err(), "container_name is UNIQUE in the schema");
}

#[tokio::test]
async fn server_with_install_path_finds_matching_row() {
    let (pool, _dir) = test_pool().await;
    let mut new_server = sample_server("native-import");
    new_server.install_path = "C:\\PalServers\\World".to_string();
    let created = create_server(&pool, new_server).await.unwrap();

    let found = server_with_install_path(&pool, "C:\\PalServers\\World")
        .await
        .unwrap();
    assert_eq!(found.map(|r| r.id), Some(created.id));

    let missing = server_with_install_path(&pool, "C:\\Nope").await.unwrap();
    assert!(missing.is_none());
}
