use ps_db::DbDriver;

async fn test_driver() -> (ps_db::SqlxSqliteDriver, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let pool = ps_db::open(&dir.path().join("test.db")).await.unwrap();
    (ps_db::SqlxSqliteDriver::new(pool), dir)
}

fn client_target(id: &str) -> ps_db::mod_targets::NewModTarget {
    ps_db::mod_targets::NewModTarget {
        id: id.to_string(),
        kind: "client".to_string(),
        server_id: None,
        name: "Steam".to_string(),
        root_path: "C:/Games/Palworld".to_string(),
        platform: "win64".to_string(),
        ue4ss_mode: "standard".to_string(),
        layout_overrides: "{}".to_string(),
        detected: "{}".to_string(),
    }
}

#[tokio::test]
async fn upsert_inserts_then_updates_the_same_id() {
    let (db, _dir) = test_driver().await;
    let first = ps_db::mod_targets::upsert(&db, &client_target("client-steam"))
        .await
        .unwrap();
    assert_eq!(first.id, "client-steam");
    assert_eq!(first.ue4ss_mode, "standard");
    assert_eq!(first.last_scanned_at, None);

    // Backdating the row makes the created_at assertion below real, instead of
    // two writes landing in the same second and matching by accident.
    db.execute(
        "UPDATE mod_targets SET created_at = '2020-01-01T00:00:00' WHERE id = ?1",
        &["client-steam".into()],
    )
    .await
    .unwrap();

    let mut changed = client_target("client-steam");
    changed.ue4ss_mode = "workshop".to_string();
    changed.name = "Steam (renamed)".to_string();
    let second = ps_db::mod_targets::upsert(&db, &changed).await.unwrap();
    assert_eq!(second.ue4ss_mode, "workshop");
    assert_eq!(second.name, "Steam (renamed)");
    assert_eq!(second.created_at, "2020-01-01T00:00:00");
    assert_ne!(second.updated_at, "2020-01-01T00:00:00");

    assert_eq!(ps_db::mod_targets::list(&db).await.unwrap().len(), 1);
}

#[tokio::test]
async fn overrides_detected_and_scan_stamp_are_stored_verbatim() {
    let (db, _dir) = test_driver().await;
    ps_db::mod_targets::upsert(&db, &client_target("client-steam"))
        .await
        .unwrap();

    ps_db::mod_targets::set_layout_overrides(&db, "client-steam", r#"{"ue4ss_mods_dir":"D:/M"}"#)
        .await
        .unwrap();
    ps_db::mod_targets::set_detected(&db, "client-steam", r#"{"ue4ss":"3.0.1"}"#)
        .await
        .unwrap();
    ps_db::mod_targets::mark_scanned(&db, "client-steam")
        .await
        .unwrap();

    let target = ps_db::mod_targets::get(&db, "client-steam")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(target.layout_overrides, r#"{"ue4ss_mods_dir":"D:/M"}"#);
    assert_eq!(target.detected, r#"{"ue4ss":"3.0.1"}"#);
    assert!(target.last_scanned_at.is_some());
}

#[tokio::test]
async fn a_missing_target_reads_as_none_not_an_error() {
    let (db, _dir) = test_driver().await;
    assert!(ps_db::mod_targets::get(&db, "nope")
        .await
        .unwrap()
        .is_none());
    assert!(ps_db::mod_targets::for_server(&db, 7)
        .await
        .unwrap()
        .is_none());
    assert!(!ps_db::mod_targets::remove(&db, "nope").await.unwrap());
}

#[tokio::test]
async fn for_server_finds_a_server_target() {
    let (db, _dir) = test_driver().await;
    let server = ps_db::servers::create_server(
        &db,
        ps_db::servers::NewServer {
            name: "Alpha".to_string(),
            container_name: "alpha".to_string(),
            server_type: "docker".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let mut target = client_target(&format!("server-{}", server.id));
    target.kind = "server".to_string();
    target.server_id = Some(server.id);
    ps_db::mod_targets::upsert(&db, &target).await.unwrap();

    let found = ps_db::mod_targets::for_server(&db, server.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.id, format!("server-{}", server.id));
    assert_eq!(found.server_id, Some(server.id));
}

#[tokio::test]
async fn a_target_cannot_name_a_server_that_does_not_exist() {
    let (db, _dir) = test_driver().await;
    let mut target = client_target("server-9999");
    target.kind = "server".to_string();
    target.server_id = Some(9999);
    assert!(
        ps_db::mod_targets::upsert(&db, &target).await.is_err(),
        "the sqlx driver enables PRAGMA foreign_keys, so the reference is checked"
    );
}

async fn a_server(
    db: &ps_db::SqlxSqliteDriver,
    container_name: &str,
    server_type: &str,
    install_path: &str,
) -> i64 {
    ps_db::servers::create_server(
        db,
        ps_db::servers::NewServer {
            name: format!("Server {container_name}"),
            container_name: container_name.to_string(),
            image_name: "omanrod/psp-palworld-server".to_string(),
            server_type: server_type.to_string(),
            game_port: 8211,
            query_port: 27015,
            rest_api_port: 8212,
            data_volume_name: format!("ps-{container_name}-data"),
            saves_path: "/srv/saves".to_string(),
            mods_path: "/srv/mods".to_string(),
            logicmods_path: "/srv/logicmods".to_string(),
            nativemods_path: "/srv/nativemods".to_string(),
            install_path: install_path.to_string(),
            server_name: "PalStudio Palworld Server".to_string(),
            max_players: 32,
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .id
}

#[tokio::test]
async fn the_backfill_gives_every_server_a_target_and_a_default_profile() {
    let (db, _dir) = test_driver().await;
    let native = a_server(&db, "native-one", "native", "D:/Palworld Server").await;
    let docker = a_server(&db, "docker-one", "docker", "").await;

    let mut created = ps_db::mod_targets::ensure_server_targets(&db, "C:/ps/servers")
        .await
        .unwrap();
    created.sort();
    let mut expected = vec![format!("server-{native}"), format!("server-{docker}")];
    expected.sort();
    assert_eq!(created, expected);

    let native_target = ps_db::mod_targets::for_server(&db, native)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(native_target.id, format!("server-{native}"));
    assert_eq!(native_target.kind, "server");
    assert_eq!(native_target.root_path, "D:/Palworld Server");
    assert_eq!(native_target.platform, "win64");
    assert_eq!(native_target.ue4ss_mode, "none");
    assert_eq!(native_target.name, "Server native-one");
    assert_eq!(
        native_target.layout_overrides,
        r#"{"ue4ss_mods_dir":"/srv/mods","logicmods_dir":"/srv/logicmods","nativemods_dir":"/srv/nativemods"}"#
    );

    let docker_target = ps_db::mod_targets::for_server(&db, docker)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        std::path::Path::new(&docker_target.root_path),
        std::path::Path::new("C:/ps/servers/docker-one")
    );
    assert_eq!(docker_target.platform, "linux");

    for target_id in [native_target.id.as_str(), docker_target.id.as_str()] {
        let profiles = ps_db::mod_profiles::for_target(&db, target_id)
            .await
            .unwrap();
        assert_eq!(profiles.len(), 1, "{target_id}");
        assert_eq!(profiles[0].id, format!("{target_id}/default"));
        assert!(profiles[0].is_default);
        assert!(profiles[0].is_active);
    }
}

#[tokio::test]
async fn the_backfill_is_idempotent_and_never_overwrites_an_edited_target() {
    let (db, _dir) = test_driver().await;
    let native = a_server(&db, "native-one", "native", "D:/Palworld Server").await;
    ps_db::mod_targets::ensure_server_targets(&db, "C:/ps/servers")
        .await
        .unwrap();

    let target_id = format!("server-{native}");
    ps_db::mod_targets::set_layout_overrides(&db, &target_id, r#"{"ue4ss_mods_dir":"E:/edited"}"#)
        .await
        .unwrap();
    db.execute(
        "UPDATE mod_targets SET root_path = 'E:/Moved', ue4ss_mode = 'standard' WHERE id = ?1",
        &[target_id.as_str().into()],
    )
    .await
    .unwrap();

    let created = ps_db::mod_targets::ensure_server_targets(&db, "C:/ps/servers")
        .await
        .unwrap();
    assert!(created.is_empty(), "{created:?}");

    let target = ps_db::mod_targets::get(&db, &target_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(target.root_path, "E:/Moved");
    assert_eq!(target.ue4ss_mode, "standard");
    assert_eq!(target.layout_overrides, r#"{"ue4ss_mods_dir":"E:/edited"}"#);
    assert_eq!(
        ps_db::mod_profiles::for_target(&db, &target_id)
            .await
            .unwrap()
            .len(),
        1,
        "no second default profile"
    );
}

#[tokio::test]
async fn the_backfill_does_nothing_with_no_servers() {
    let (db, _dir) = test_driver().await;
    assert!(
        ps_db::mod_targets::ensure_server_targets(&db, "C:/ps/servers")
            .await
            .unwrap()
            .is_empty()
    );
    assert!(ps_db::mod_targets::list(&db).await.unwrap().is_empty());
}

#[tokio::test]
async fn the_backfill_completes_a_pair_left_half_built() {
    let (db, _dir) = test_driver().await;
    let native = a_server(&db, "native-one", "native", "D:/Palworld Server").await;
    ps_db::mod_targets::ensure_server_targets(&db, "C:/ps/servers")
        .await
        .unwrap();

    let target_id = format!("server-{native}");
    db.execute(
        "UPDATE mod_targets SET root_path = 'E:/Moved' WHERE id = ?1",
        &[target_id.as_str().into()],
    )
    .await
    .unwrap();
    db.execute(
        "DELETE FROM profiles WHERE target_id = ?1",
        &[target_id.as_str().into()],
    )
    .await
    .unwrap();

    let created = ps_db::mod_targets::ensure_server_targets(&db, "C:/ps/servers")
        .await
        .unwrap();
    assert_eq!(
        created,
        vec![target_id.clone()],
        "the incomplete pair is revisited"
    );

    let profiles = ps_db::mod_profiles::for_target(&db, &target_id)
        .await
        .unwrap();
    assert_eq!(profiles.len(), 1);
    assert!(profiles[0].is_default);
    assert!(profiles[0].is_active);
    assert_eq!(
        ps_db::mod_targets::get(&db, &target_id)
            .await
            .unwrap()
            .unwrap()
            .root_path,
        "E:/Moved",
        "repairing the profile must not rewrite the edited target"
    );
}

#[tokio::test]
async fn the_backfill_records_the_paks_directory_when_the_server_has_one() {
    let (db, _dir) = test_driver().await;
    let server = ps_db::servers::create_server(
        &db,
        ps_db::servers::NewServer {
            name: "Docker One".to_string(),
            container_name: "docker-one".to_string(),
            server_type: "docker".to_string(),
            mods_path: "/srv/mods".to_string(),
            logicmods_path: "/srv/logicmods".to_string(),
            nativemods_path: "/srv/nativemods".to_string(),
            paks_path: "/srv/paks".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    ps_db::mod_targets::ensure_server_targets(&db, "C:/ps/servers")
        .await
        .unwrap();

    let target = ps_db::mod_targets::for_server(&db, server.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        target.layout_overrides,
        r#"{"ue4ss_mods_dir":"/srv/mods","logicmods_dir":"/srv/logicmods","nativemods_dir":"/srv/nativemods","paks_mods_dir":"/srv/paks"}"#,
        "a docker target must point ~mods at the bind-mounted host dir, not at the install"
    );
}

#[tokio::test]
async fn rewriting_after_a_mods_path_change_updates_the_stored_ue4ss_mods_dir() {
    let (db, _dir) = test_driver().await;
    let docker = a_server(&db, "docker-one", "docker", "").await;
    assert!(
        !ps_db::mod_targets::rewrite_server_overrides(
            &db,
            &ps_db::servers::get_server(&db, docker).await.unwrap().unwrap(),
        )
        .await
        .unwrap(),
        "a server without a target reports that nothing was rewritten"
    );
    ps_db::mod_targets::ensure_server_targets(&db, "C:/ps/servers")
        .await
        .unwrap();

    let mut updates = serde_json::Map::new();
    updates.insert("mods_path".to_string(), serde_json::json!("/srv/moved/mods"));
    let record = ps_db::servers::update_server(&db, docker, &updates)
        .await
        .unwrap()
        .unwrap();
    assert!(ps_db::mod_targets::rewrite_server_overrides(&db, &record)
        .await
        .unwrap());

    let target = ps_db::mod_targets::for_server(&db, docker)
        .await
        .unwrap()
        .unwrap();
    let overrides: serde_json::Value = serde_json::from_str(&target.layout_overrides).unwrap();
    assert_eq!(overrides["ue4ss_mods_dir"], "/srv/moved/mods");
    assert_eq!(overrides["logicmods_dir"], "/srv/logicmods");
    assert_eq!(
        std::path::Path::new(&target.root_path),
        std::path::Path::new("C:/ps/servers/docker-one")
    );
}

#[tokio::test]
async fn removing_a_target_clears_any_amity_pairing() {
    let (db, _dir) = test_driver().await;
    ps_db::mod_targets::upsert(&db, &client_target("client-steam"))
        .await
        .unwrap();
    let instance = ps_db::amity_instances::insert_instance(
        &db,
        &ps_db::amity_instances::NewAmityInstance {
            name: "Box".to_string(),
            host: "10.0.0.1".to_string(),
            port: 8788,
            token: "t".to_string(),
        },
    )
    .await
    .unwrap();
    ps_db::amity_instances::set_target(&db, instance, Some("client-steam"))
        .await
        .unwrap();

    assert!(ps_db::mod_targets::remove(&db, "client-steam")
        .await
        .unwrap());
    // This driver enables PRAGMA foreign_keys, so ON DELETE SET NULL clears the
    // column even without the explicit UPDATE in `remove`. The assertion states
    // the contract; only the browser's pragma-less driver can fail it.
    assert_eq!(
        ps_db::amity_instances::get_instance(&db, instance)
            .await
            .unwrap()
            .unwrap()
            .target_id,
        None
    );
}

#[tokio::test]
async fn the_ue4ss_mode_can_be_updated_on_its_own() {
    let (db, _dir) = test_driver().await;
    ps_db::mod_targets::upsert(&db, &client_target("client-steam"))
        .await
        .unwrap();
    ps_db::mod_targets::set_ue4ss_mode(&db, "client-steam", "workshop")
        .await
        .unwrap();
    let target = ps_db::mod_targets::get(&db, "client-steam")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(target.ue4ss_mode, "workshop");
    assert_eq!(target.layout_overrides, "{}", "and nothing else moved");
}

#[tokio::test]
async fn a_docker_target_root_is_the_container_name_joined_as_a_path() {
    let (db, _dir) = test_driver().await;
    let docker = a_server(&db, "docker-one", "docker", "").await;
    let servers_root = std::path::Path::new("ps").join("servers");

    ps_db::mod_targets::ensure_server_targets(&db, &servers_root.to_string_lossy())
        .await
        .unwrap();

    let target = ps_db::mod_targets::for_server(&db, docker)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        target.root_path,
        servers_root.join("docker-one").to_string_lossy()
    );
}
