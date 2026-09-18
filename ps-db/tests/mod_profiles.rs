use ps_db::mod_profiles::{NewProfile, ProfileModRow};

async fn test_driver() -> (ps_db::SqlxSqliteDriver, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let pool = ps_db::open(&dir.path().join("test.db")).await.unwrap();
    (ps_db::SqlxSqliteDriver::new(pool), dir)
}

async fn a_target(db: &ps_db::SqlxSqliteDriver, id: &str) {
    ps_db::mod_targets::upsert(
        db,
        &ps_db::mod_targets::NewModTarget {
            id: id.to_string(),
            kind: "client".to_string(),
            name: "Steam".to_string(),
            root_path: "C:/Games/Palworld".to_string(),
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

async fn a_mod_with_two_versions(db: &ps_db::SqlxSqliteDriver, mod_id: &str) {
    ps_db::mod_library::upsert_mod(
        db,
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
    for version in ["1.0", "2.0"] {
        ps_db::mod_library::insert_version(
            db,
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

fn new_profile(id: &str, target_id: &str, is_default: bool) -> NewProfile {
    NewProfile {
        id: id.to_string(),
        target_id: target_id.to_string(),
        name: id.to_string(),
        is_default,
    }
}

#[tokio::test]
async fn the_first_profile_is_active_and_a_later_one_is_not() {
    let (db, _dir) = test_driver().await;
    a_target(&db, "t").await;

    let default = ps_db::mod_profiles::create(&db, &new_profile("p-default", "t", true))
        .await
        .unwrap();
    assert!(default.is_default);
    assert!(default.is_active);
    assert_eq!(default.ue4ss_control_mode, "enabled_txt");
    assert!(!default.force_order_ue4ss);

    let second = ps_db::mod_profiles::create(&db, &new_profile("p-2", "t", false))
        .await
        .unwrap();
    assert!(!second.is_active);
    assert_eq!(
        ps_db::mod_profiles::for_target(&db, "t")
            .await
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        ps_db::mod_profiles::active_for_target(&db, "t")
            .await
            .unwrap()
            .unwrap()
            .id,
        "p-default"
    );
}

#[tokio::test]
async fn activate_moves_the_flag_and_rejects_a_foreign_profile() {
    let (db, _dir) = test_driver().await;
    a_target(&db, "t").await;
    a_target(&db, "other").await;
    ps_db::mod_profiles::create(&db, &new_profile("p-default", "t", true))
        .await
        .unwrap();
    ps_db::mod_profiles::create(&db, &new_profile("p-2", "t", false))
        .await
        .unwrap();
    ps_db::mod_profiles::create(&db, &new_profile("p-elsewhere", "other", true))
        .await
        .unwrap();

    let now_active = ps_db::mod_profiles::activate(&db, "t", "p-2")
        .await
        .unwrap();
    assert_eq!(now_active.id, "p-2");
    let actives: Vec<String> = ps_db::mod_profiles::for_target(&db, "t")
        .await
        .unwrap()
        .into_iter()
        .filter(|p| p.is_active)
        .map(|p| p.id)
        .collect();
    assert_eq!(actives, vec!["p-2".to_string()]);

    assert!(
        ps_db::mod_profiles::activate(&db, "t", "p-elsewhere")
            .await
            .is_err(),
        "a profile belonging to another target is not activatable here"
    );
    assert!(ps_db::mod_profiles::activate(&db, "t", "nope")
        .await
        .is_err());
    // The failed attempts left the other target alone.
    assert!(
        ps_db::mod_profiles::active_for_target(&db, "other")
            .await
            .unwrap()
            .unwrap()
            .is_active
    );
}

#[tokio::test]
async fn deleting_the_active_profile_falls_back_to_the_default() {
    let (db, _dir) = test_driver().await;
    a_target(&db, "t").await;
    ps_db::mod_profiles::create(&db, &new_profile("p-default", "t", true))
        .await
        .unwrap();
    ps_db::mod_profiles::create(&db, &new_profile("p-2", "t", false))
        .await
        .unwrap();
    ps_db::mod_profiles::activate(&db, "t", "p-2")
        .await
        .unwrap();

    let now_active = ps_db::mod_profiles::delete(&db, "p-2").await.unwrap();
    assert_eq!(now_active.id, "p-default");
    assert!(now_active.is_active);
    assert!(ps_db::mod_profiles::get(&db, "p-2")
        .await
        .unwrap()
        .is_none());

    assert!(
        ps_db::mod_profiles::delete(&db, "p-default").await.is_err(),
        "the default profile is undeletable"
    );
}

#[tokio::test]
async fn deleting_an_inactive_profile_leaves_the_active_one_alone() {
    let (db, _dir) = test_driver().await;
    a_target(&db, "t").await;
    ps_db::mod_profiles::create(&db, &new_profile("p-default", "t", true))
        .await
        .unwrap();
    ps_db::mod_profiles::create(&db, &new_profile("p-2", "t", false))
        .await
        .unwrap();

    let still_active = ps_db::mod_profiles::delete(&db, "p-2").await.unwrap();
    assert_eq!(still_active.id, "p-default");
    assert_eq!(
        ps_db::mod_profiles::for_target(&db, "t")
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn rename_and_options_round_trip() {
    let (db, _dir) = test_driver().await;
    a_target(&db, "t").await;
    ps_db::mod_profiles::create(&db, &new_profile("p", "t", true))
        .await
        .unwrap();

    ps_db::mod_profiles::rename(&db, "p", "Heavy Modding")
        .await
        .unwrap();
    ps_db::mod_profiles::set_options(&db, "p", "mods_txt", true, false)
        .await
        .unwrap();

    let row = ps_db::mod_profiles::get(&db, "p").await.unwrap().unwrap();
    assert_eq!(row.name, "Heavy Modding");
    assert_eq!(row.ue4ss_control_mode, "mods_txt");
    assert!(row.force_order_ue4ss);
    assert!(!row.force_order_palschema);
}

#[tokio::test]
async fn a_profile_entry_follows_current_until_it_is_pinned() {
    let (db, _dir) = test_driver().await;
    a_target(&db, "t").await;
    a_mod_with_two_versions(&db, "coolmod").await;
    ps_db::mod_profiles::create(&db, &new_profile("p", "t", true))
        .await
        .unwrap();

    ps_db::mod_profiles::set_mod(
        &db,
        &ProfileModRow {
            profile_id: "p".to_string(),
            mod_id: "coolmod".to_string(),
            mod_version_id: None,
            enabled: true,
            load_order: 0,
        },
    )
    .await
    .unwrap();
    let entries = ps_db::mod_profiles::mods_of(&db, "p").await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].mod_version_id, None);
    assert!(entries[0].enabled);

    ps_db::mod_profiles::set_mod(
        &db,
        &ProfileModRow {
            profile_id: "p".to_string(),
            mod_id: "coolmod".to_string(),
            mod_version_id: Some("coolmod@2.0".to_string()),
            enabled: false,
            load_order: 3,
        },
    )
    .await
    .unwrap();
    let entries = ps_db::mod_profiles::mods_of(&db, "p").await.unwrap();
    assert_eq!(entries.len(), 1, "the same mod is one row, not two");
    assert_eq!(entries[0].mod_version_id.as_deref(), Some("coolmod@2.0"));
    assert!(!entries[0].enabled);
    assert_eq!(entries[0].load_order, 3);

    assert!(ps_db::mod_profiles::unset_mod(&db, "p", "coolmod")
        .await
        .unwrap());
    assert!(ps_db::mod_profiles::mods_of(&db, "p")
        .await
        .unwrap()
        .is_empty());
    assert!(!ps_db::mod_profiles::unset_mod(&db, "p", "coolmod")
        .await
        .unwrap());
}

#[tokio::test]
async fn unset_if_disabled_leaves_a_re_enabled_entry_alone() {
    let (db, _dir) = test_driver().await;
    a_target(&db, "t").await;
    a_mod_with_two_versions(&db, "coolmod").await;
    ps_db::mod_profiles::create(&db, &new_profile("p", "t", true))
        .await
        .unwrap();
    ps_db::mod_profiles::set_mod(
        &db,
        &ProfileModRow {
            profile_id: "p".to_string(),
            mod_id: "coolmod".to_string(),
            mod_version_id: None,
            enabled: false,
            load_order: 0,
        },
    )
    .await
    .unwrap();

    assert!(
        !ps_db::mod_profiles::unset_if_disabled(&db, "p", "ghost")
            .await
            .unwrap(),
        "no such entry"
    );

    ps_db::mod_profiles::set_mod(
        &db,
        &ProfileModRow {
            profile_id: "p".to_string(),
            mod_id: "coolmod".to_string(),
            mod_version_id: None,
            enabled: true,
            load_order: 0,
        },
    )
    .await
    .unwrap();
    assert!(
        !ps_db::mod_profiles::unset_if_disabled(&db, "p", "coolmod")
            .await
            .unwrap(),
        "re-enabled between the read and the delete: left alone"
    );
    assert!(ps_db::mod_profiles::mods_of(&db, "p")
        .await
        .unwrap()
        .iter()
        .any(|entry| entry.mod_id == "coolmod"));

    ps_db::mod_profiles::set_mod(
        &db,
        &ProfileModRow {
            profile_id: "p".to_string(),
            mod_id: "coolmod".to_string(),
            mod_version_id: None,
            enabled: false,
            load_order: 0,
        },
    )
    .await
    .unwrap();
    assert!(ps_db::mod_profiles::unset_if_disabled(&db, "p", "coolmod")
        .await
        .unwrap());
    assert!(ps_db::mod_profiles::mods_of(&db, "p")
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn set_mod_rejects_an_unknown_mod_and_a_foreign_pin() {
    let (db, _dir) = test_driver().await;
    a_target(&db, "t").await;
    a_mod_with_two_versions(&db, "coolmod").await;
    a_mod_with_two_versions(&db, "othermod").await;
    ps_db::mod_profiles::create(&db, &new_profile("p", "t", true))
        .await
        .unwrap();

    let entry = |mod_id: &str, pin: Option<&str>| ProfileModRow {
        profile_id: "p".to_string(),
        mod_id: mod_id.to_string(),
        mod_version_id: pin.map(str::to_string),
        enabled: true,
        load_order: 0,
    };

    assert!(ps_db::mod_profiles::set_mod(&db, &entry("ghost", None))
        .await
        .is_err());
    assert!(
        ps_db::mod_profiles::set_mod(&db, &entry("coolmod", Some("coolmod@9.9")))
            .await
            .is_err(),
        "a version that does not exist"
    );
    assert!(
        ps_db::mod_profiles::set_mod(&db, &entry("coolmod", Some("othermod@1.0")))
            .await
            .is_err(),
        "a version belonging to a different mod"
    );
    assert!(ps_db::mod_profiles::mods_of(&db, "p")
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn set_load_order_renumbers_only_the_listed_mods() {
    let (db, _dir) = test_driver().await;
    a_target(&db, "t").await;
    for mod_id in ["a", "b", "c"] {
        a_mod_with_two_versions(&db, mod_id).await;
    }
    ps_db::mod_profiles::create(&db, &new_profile("p", "t", true))
        .await
        .unwrap();
    for mod_id in ["a", "b", "c"] {
        ps_db::mod_profiles::set_mod(
            &db,
            &ProfileModRow {
                profile_id: "p".to_string(),
                mod_id: mod_id.to_string(),
                mod_version_id: None,
                enabled: true,
                load_order: 99,
            },
        )
        .await
        .unwrap();
    }

    ps_db::mod_profiles::set_load_order(&db, "p", &["c", "a"])
        .await
        .unwrap();
    let orders: Vec<(String, i64)> = ps_db::mod_profiles::mods_of(&db, "p")
        .await
        .unwrap()
        .into_iter()
        .map(|e| (e.mod_id, e.load_order))
        .collect();
    assert_eq!(
        orders,
        vec![
            ("c".to_string(), 0),
            ("a".to_string(), 1),
            ("b".to_string(), 99),
        ],
        "mods_of sorts by load_order, and the unlisted mod keeps its own"
    );
}

async fn a_framework_mod(db: &ps_db::SqlxSqliteDriver, mod_id: &str, version: &str) -> String {
    ps_db::mod_library::upsert_mod(
        db,
        &ps_db::mod_library::NewMod {
            id: mod_id.to_string(),
            name: mod_id.to_string(),
            mod_type: "framework".to_string(),
            source_kind: "bundled".to_string(),
            source_ref: r#"{"framework":"ue4ss"}"#.to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let id = format!("{mod_id}@{version}");
    ps_db::mod_library::insert_version(
        db,
        &ps_db::mod_library::NewModVersion {
            id: id.clone(),
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
    id
}

#[tokio::test]
async fn a_framework_slot_holds_one_version_per_target() {
    let (db, _dir) = test_driver().await;
    a_target(&db, "t").await;
    let v1 = a_framework_mod(&db, "ue4ss", "3.0.1").await;

    ps_db::mod_profiles::set_framework(&db, "t", "ue4ss", &v1)
        .await
        .unwrap();
    let slot = ps_db::mod_profiles::framework_of(&db, "t", "ue4ss")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(slot.mod_version_id, v1);

    let v2 = "ue4ss@3.1.0".to_string();
    ps_db::mod_library::insert_version(
        &db,
        &ps_db::mod_library::NewModVersion {
            id: v2.clone(),
            mod_id: "ue4ss".to_string(),
            version: "3.1.0".to_string(),
            library_dir: "/lib/ue4ss/3.1.0".to_string(),
            manifest: "{}".to_string(),
            source_ref: "{}".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    ps_db::mod_profiles::set_framework(&db, "t", "ue4ss", &v2)
        .await
        .unwrap();

    let slots = ps_db::mod_profiles::frameworks_of(&db, "t").await.unwrap();
    assert_eq!(
        slots.len(),
        1,
        "upgrading replaces the slot, it does not add one"
    );
    assert_eq!(slots[0].mod_version_id, v2);

    assert!(ps_db::mod_profiles::remove_framework(&db, "t", "ue4ss")
        .await
        .unwrap());
    assert!(ps_db::mod_profiles::frameworks_of(&db, "t")
        .await
        .unwrap()
        .is_empty());
    assert!(!ps_db::mod_profiles::remove_framework(&db, "t", "ue4ss")
        .await
        .unwrap());
}

#[tokio::test]
async fn set_framework_rejects_a_missing_version_and_an_ordinary_mod() {
    let (db, _dir) = test_driver().await;
    a_target(&db, "t").await;
    a_mod_with_two_versions(&db, "coolmod").await;

    assert!(
        ps_db::mod_profiles::set_framework(&db, "t", "ue4ss", "ue4ss@3.0.1")
            .await
            .is_err(),
        "the version does not exist"
    );
    assert!(
        ps_db::mod_profiles::set_framework(&db, "t", "ue4ss", "coolmod@1.0")
            .await
            .is_err(),
        "coolmod is not a framework"
    );
    assert!(ps_db::mod_profiles::frameworks_of(&db, "t")
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn a_framework_version_counts_as_in_use() {
    let (db, _dir) = test_driver().await;
    a_target(&db, "t").await;
    let v1 = a_framework_mod(&db, "ue4ss", "3.0.1").await;
    ps_db::mod_profiles::set_framework(&db, "t", "ue4ss", &v1)
        .await
        .unwrap();

    let usage = ps_db::mod_library::version_usage(&db, &v1).await.unwrap();
    assert_eq!(usage.frameworks, vec!["t".to_string()]);
    assert!(usage.is_in_use());
}

#[tokio::test]
async fn framework_targets_of_lists_targets_with_any_version_in_a_slot() {
    let (db, _dir) = test_driver().await;
    a_target(&db, "t").await;
    a_target(&db, "u").await;
    a_target(&db, "v").await;
    let v1 = a_framework_mod(&db, "ue4ss", "3.0.1").await;
    let v2 = "ue4ss@3.1.0".to_string();
    ps_db::mod_library::insert_version(
        &db,
        &ps_db::mod_library::NewModVersion {
            id: v2.clone(),
            mod_id: "ue4ss".to_string(),
            version: "3.1.0".to_string(),
            library_dir: "/lib/ue4ss/3.1.0".to_string(),
            manifest: "{}".to_string(),
            source_ref: "{}".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    ps_db::mod_profiles::set_framework(&db, "u", "ue4ss", &v1)
        .await
        .unwrap();
    ps_db::mod_profiles::set_framework(&db, "t", "ue4ss", &v2)
        .await
        .unwrap();

    assert_eq!(
        ps_db::mod_profiles::framework_targets_of(&db, "ue4ss")
            .await
            .unwrap(),
        vec!["t".to_string(), "u".to_string()],
        "every version of the mod counts, and the list is sorted"
    );
    assert!(ps_db::mod_profiles::framework_targets_of(&db, "nobody")
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn a_world_link_is_one_profile_per_world() {
    let (db, _dir) = test_driver().await;
    a_target(&db, "t").await;
    ps_db::mod_profiles::create(&db, &new_profile("p-default", "t", true))
        .await
        .unwrap();
    ps_db::mod_profiles::create(&db, &new_profile("p-2", "t", false))
        .await
        .unwrap();

    let link = ps_db::mod_profiles::link_world(&db, "C:/saves/ABCD1234", "Home", "p-default")
        .await
        .unwrap();
    assert_eq!(link.profile_id, "p-default");
    assert_eq!(link.world_name, "Home");

    let relinked = ps_db::mod_profiles::link_world(&db, "C:/saves/ABCD1234", "Home Renamed", "p-2")
        .await
        .unwrap();
    assert_eq!(relinked.profile_id, "p-2");
    assert_eq!(relinked.world_name, "Home Renamed");
    assert_eq!(
        ps_db::mod_profiles::list_world_links(&db)
            .await
            .unwrap()
            .len(),
        1
    );

    assert_eq!(
        ps_db::mod_profiles::worlds_for_profile(&db, "p-2")
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(ps_db::mod_profiles::worlds_for_profile(&db, "p-default")
        .await
        .unwrap()
        .is_empty());

    assert!(ps_db::mod_profiles::unlink_world(&db, "C:/saves/ABCD1234")
        .await
        .unwrap());
    assert!(ps_db::mod_profiles::world_link(&db, "C:/saves/ABCD1234")
        .await
        .unwrap()
        .is_none());
    assert!(!ps_db::mod_profiles::unlink_world(&db, "C:/saves/ABCD1234")
        .await
        .unwrap());
}

#[tokio::test]
async fn link_world_rejects_an_unknown_profile_and_deleting_a_profile_unlinks_its_worlds() {
    let (db, _dir) = test_driver().await;
    a_target(&db, "t").await;
    ps_db::mod_profiles::create(&db, &new_profile("p-default", "t", true))
        .await
        .unwrap();
    ps_db::mod_profiles::create(&db, &new_profile("p-2", "t", false))
        .await
        .unwrap();

    assert!(
        ps_db::mod_profiles::link_world(&db, "C:/saves/X", "X", "ghost")
            .await
            .is_err()
    );

    ps_db::mod_profiles::link_world(&db, "C:/saves/X", "X", "p-2")
        .await
        .unwrap();
    ps_db::mod_profiles::delete(&db, "p-2").await.unwrap();
    assert!(
        ps_db::mod_profiles::world_link(&db, "C:/saves/X")
            .await
            .unwrap()
            .is_none(),
        "Task 5's delete removes the world_profiles rows itself"
    );
}

#[tokio::test]
async fn deleting_the_active_profile_leaves_exactly_one_active() {
    let (db, _dir) = test_driver().await;
    a_target(&db, "t").await;
    ps_db::mod_profiles::create(&db, &new_profile("p-default", "t", true))
        .await
        .unwrap();
    ps_db::mod_profiles::create(&db, &new_profile("p-2", "t", false))
        .await
        .unwrap();
    ps_db::mod_profiles::create(&db, &new_profile("p-3", "t", false))
        .await
        .unwrap();
    ps_db::mod_profiles::activate(&db, "t", "p-2")
        .await
        .unwrap();

    ps_db::mod_profiles::delete(&db, "p-2").await.unwrap();
    let actives: Vec<String> = ps_db::mod_profiles::for_target(&db, "t")
        .await
        .unwrap()
        .into_iter()
        .filter(|p| p.is_active)
        .map(|p| p.id)
        .collect();
    assert_eq!(
        actives,
        vec!["p-default".to_string()],
        "the delete and the fallback activation are one transaction"
    );
}
