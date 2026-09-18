use ps_db::mod_deployments::NewDeploymentFile;

/// A driver whose database already holds the two targets and three mod versions
/// every test below references. `deployment_files.target_id` and
/// `.mod_version_id` are enforced foreign keys on this driver, so the parents
/// have to exist before a row can be recorded.
async fn seeded() -> (ps_db::SqlxSqliteDriver, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let pool = ps_db::open(&dir.path().join("test.db")).await.unwrap();
    let db = ps_db::SqlxSqliteDriver::new(pool);

    for id in ["t", "other"] {
        ps_db::mod_targets::upsert(
            &db,
            &ps_db::mod_targets::NewModTarget {
                id: id.to_string(),
                kind: "client".to_string(),
                name: id.to_string(),
                root_path: "C:/g".to_string(),
                platform: "win64".to_string(),
                ue4ss_mode: "standard".to_string(),
                layout_overrides: "{}".to_string(),
                detected: "{}".to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    }

    for (mod_id, versions) in [
        ("m", ["1.0", "2.0"].as_slice()),
        ("other", ["1.0"].as_slice()),
    ] {
        ps_db::mod_library::upsert_mod(
            &db,
            &ps_db::mod_library::NewMod {
                id: mod_id.to_string(),
                name: mod_id.to_string(),
                mod_type: "ue4ss".to_string(),
                source_kind: "local".to_string(),
                source_ref: "{}".to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        for version in versions {
            ps_db::mod_library::insert_version(
                &db,
                &ps_db::mod_library::NewModVersion {
                    id: format!("{mod_id}@{version}"),
                    mod_id: mod_id.to_string(),
                    version: version.to_string(),
                    library_dir: format!("/lib/{mod_id}/{version}"),
                    manifest: "{}".to_string(),
                    source_ref: "{}".to_string(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        }
    }
    (db, dir)
}

fn a_file(path: &str, version: Option<&str>, role: &str) -> NewDeploymentFile {
    NewDeploymentFile {
        target_id: "t".to_string(),
        path: path.to_string(),
        mod_version_id: version.map(str::to_string),
        hash: format!("hash-of-{path}"),
        role: role.to_string(),
        rel_path: Some("Mods/Cool/main.lua".to_string()),
    }
}

#[tokio::test]
async fn recorded_files_are_readable_by_target_path_and_version() {
    let (db, _dir) = seeded().await;
    ps_db::mod_deployments::record(
        &db,
        &[
            a_file("C:/g/a.lua", Some("m@1.0"), "file"),
            a_file("C:/g/b.lua", Some("m@1.0"), "file"),
            a_file("C:/g/c.lua", Some("other@1.0"), "file"),
        ],
    )
    .await
    .unwrap();

    let all = ps_db::mod_deployments::files_of(&db, "t").await.unwrap();
    assert_eq!(all.len(), 3);
    assert!(
        all.windows(2).all(|w| w[0].path <= w[1].path),
        "sorted by path"
    );
    assert!(!all[0].deployed_at.is_empty());

    let one = ps_db::mod_deployments::file_at(&db, "t", "C:/g/b.lua")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(one.hash, "hash-of-C:/g/b.lua");
    assert_eq!(one.rel_path.as_deref(), Some("Mods/Cool/main.lua"));

    let mine = ps_db::mod_deployments::files_for_version(&db, "t", "m@1.0")
        .await
        .unwrap();
    assert_eq!(mine.len(), 2);

    assert!(ps_db::mod_deployments::file_at(&db, "t", "C:/g/nope.lua")
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn recording_the_same_path_reattributes_it() {
    let (db, _dir) = seeded().await;
    ps_db::mod_deployments::record(&db, &[a_file("C:/g/a.lua", Some("m@1.0"), "file")])
        .await
        .unwrap();
    ps_db::mod_deployments::record(&db, &[a_file("C:/g/a.lua", Some("m@2.0"), "file")])
        .await
        .unwrap();

    let all = ps_db::mod_deployments::files_of(&db, "t").await.unwrap();
    assert_eq!(
        all.len(),
        1,
        "one destination is owned by exactly one version"
    );
    assert_eq!(all[0].mod_version_id.as_deref(), Some("m@2.0"));
    assert!(ps_db::mod_deployments::files_for_version(&db, "t", "m@1.0")
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn a_shared_marker_has_no_owning_version() {
    let (db, _dir) = seeded().await;
    ps_db::mod_deployments::record(
        &db,
        &[
            a_file("C:/g/mods.txt", None, "shared_marker"),
            a_file("C:/g/a.lua", Some("m@1.0"), "file"),
        ],
    )
    .await
    .unwrap();

    let markers = ps_db::mod_deployments::shared_markers(&db, "t")
        .await
        .unwrap();
    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].path, "C:/g/mods.txt");
    assert_eq!(markers[0].mod_version_id, None);
}

#[tokio::test]
async fn the_role_and_ownership_check_rejects_both_mismatches() {
    let (db, _dir) = seeded().await;
    assert!(
        ps_db::mod_deployments::record(&db, &[a_file("C:/g/a.lua", None, "file")])
            .await
            .is_err(),
        "an ordinary file must name its version"
    );
    assert!(
        ps_db::mod_deployments::record(
            &db,
            &[a_file("C:/g/mods.txt", Some("m@1.0"), "shared_marker")]
        )
        .await
        .is_err(),
        "a shared marker must not name a version"
    );
    assert!(ps_db::mod_deployments::files_of(&db, "t")
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn one_bad_row_records_none_of_the_batch() {
    let (db, _dir) = seeded().await;
    let result = ps_db::mod_deployments::record(
        &db,
        &[
            a_file("C:/g/a.lua", Some("m@1.0"), "file"),
            a_file("C:/g/b.lua", None, "file"),
            a_file("C:/g/c.lua", Some("m@1.0"), "file"),
        ],
    )
    .await;
    assert!(result.is_err());
    assert!(
        ps_db::mod_deployments::files_of(&db, "t")
            .await
            .unwrap()
            .is_empty(),
        "the good rows before the bad one must not survive"
    );
}

#[tokio::test]
async fn deployed_targets_of_lists_distinct_targets_for_the_mods_versions() {
    let (db, _dir) = seeded().await;
    let mut on_other = a_file("C:/g/x.lua", Some("m@1.0"), "file");
    on_other.target_id = "other".to_string();
    ps_db::mod_deployments::record(
        &db,
        &[
            a_file("C:/g/a.lua", Some("m@1.0"), "file"),
            a_file("C:/g/b.lua", Some("m@2.0"), "file"),
            a_file("C:/g/c.lua", Some("other@1.0"), "file"),
            on_other,
        ],
    )
    .await
    .unwrap();

    assert_eq!(
        ps_db::mod_deployments::deployed_targets_of(&db, "m")
            .await
            .unwrap(),
        vec!["other".to_string(), "t".to_string()],
        "both versions of m count, and the list is distinct and sorted"
    );
    assert_eq!(
        ps_db::mod_deployments::deployed_targets_of(&db, "other")
            .await
            .unwrap(),
        vec!["t".to_string()]
    );
    assert!(ps_db::mod_deployments::deployed_targets_of(&db, "nobody")
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn forget_removes_only_the_named_paths() {
    let (db, _dir) = seeded().await;
    ps_db::mod_deployments::record(
        &db,
        &[
            a_file("C:/g/a.lua", Some("m@1.0"), "file"),
            a_file("C:/g/b.lua", Some("m@1.0"), "file"),
        ],
    )
    .await
    .unwrap();

    ps_db::mod_deployments::forget(&db, "t", &["C:/g/a.lua", "C:/g/absent.lua"])
        .await
        .unwrap();
    let remaining: Vec<String> = ps_db::mod_deployments::files_of(&db, "t")
        .await
        .unwrap()
        .into_iter()
        .map(|f| f.path)
        .collect();
    assert_eq!(remaining, vec!["C:/g/b.lua".to_string()]);

    ps_db::mod_deployments::forget(&db, "t", &[]).await.unwrap();
    assert_eq!(
        ps_db::mod_deployments::files_of(&db, "t")
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn a_target_has_at_most_one_open_journal() {
    let (db, _dir) = seeded().await;
    assert!(ps_db::mod_deployments::journal_of(&db, "t")
        .await
        .unwrap()
        .is_none());

    let journal = ps_db::mod_deployments::open_journal(&db, "t", "op-1", r#"{"entries":[]}"#)
        .await
        .unwrap();
    assert_eq!(journal.op_id, "op-1");
    assert_eq!(journal.plan, r#"{"entries":[]}"#);
    assert!(!journal.started_at.is_empty());

    assert!(
        ps_db::mod_deployments::open_journal(&db, "t", "op-2", "{}")
            .await
            .is_err(),
        "a second open would overwrite the plan recovery needs"
    );
    assert_eq!(
        ps_db::mod_deployments::journal_of(&db, "t")
            .await
            .unwrap()
            .unwrap()
            .op_id,
        "op-1"
    );

    ps_db::mod_deployments::open_journal(&db, "other", "op-3", "{}")
        .await
        .unwrap();
    let open: Vec<String> = ps_db::mod_deployments::open_journals(&db)
        .await
        .unwrap()
        .into_iter()
        .map(|j| j.target_id)
        .collect();
    assert_eq!(open, vec!["other".to_string(), "t".to_string()]);

    assert!(ps_db::mod_deployments::close_journal(&db, "t")
        .await
        .unwrap());
    assert!(!ps_db::mod_deployments::close_journal(&db, "t")
        .await
        .unwrap());
    assert!(ps_db::mod_deployments::journal_of(&db, "t")
        .await
        .unwrap()
        .is_none());
    assert!(ps_db::mod_deployments::open_journal(&db, "t", "op-4", "{}")
        .await
        .is_ok());
}
