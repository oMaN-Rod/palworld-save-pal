use std::path::Path;

use ps_core::mods::{Role, RouteKind};
use ps_server::services::mods::{deploy::desired, LibraryPaths};

mod common;
use common::mods::*;

#[tokio::test]
async fn an_enabled_mod_contributes_one_desired_file_per_route() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let target = client_target(&db, &dir.path().join("Palworld")).await;
    default_profile(&db, &target.id).await;
    let stored = install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[("ue4ss", "CoolMod/Scripts/main.lua", b"print('hi')")],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;

    let set = desired::build(&db, &target, &format!("{}/default", target.id))
        .await
        .unwrap();

    assert_eq!(set.files.len(), 1, "{:?}", set.files);
    let file = &set.files[0];
    assert_eq!(file.mod_version_id, stored.version.id);
    assert_eq!(file.rel_path, "CoolMod/Scripts/main.lua");
    assert_eq!(file.kind, RouteKind::Ue4ss);
    assert_eq!(file.role, Role::File);
    assert_eq!(
        file.expected_hash,
        ps_server::services::mods::digest::hash_bytes(b"print('hi')")
    );
    assert!(
        Path::new(&file.path).ends_with("ue4ss/Mods/CoolMod/Scripts/main.lua"),
        "{}",
        file.path
    );
    assert!(
        Path::new(&file.source).starts_with(&stored.version.library_dir),
        "the source is the library copy: {}",
        file.source
    );
    assert_eq!(set.ue4ss_entries, vec![("CoolMod".to_string(), true)]);
    assert!(set.ue4ss_purge.is_empty());
}

#[tokio::test]
async fn a_disabled_mod_contributes_no_files_but_still_a_marker_line() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let target = client_target(&db, &dir.path().join("Palworld")).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[("ue4ss", "CoolMod/Scripts/main.lua", b"a")],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, false).await;

    let set = desired::build(&db, &target, &format!("{}/default", target.id))
        .await
        .unwrap();
    assert!(set.files.is_empty(), "{:?}", set.files);
    assert_eq!(set.ue4ss_entries, vec![("CoolMod".to_string(), false)]);
}

#[tokio::test]
async fn a_pinned_version_wins_over_the_current_one() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let target = client_target(&db, &dir.path().join("Palworld")).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[("ue4ss", "CoolMod/Scripts/main.lua", b"v1")],
    )
    .await;
    let second = install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "2.0",
        &[("ue4ss", "CoolMod/Scripts/main.lua", b"v2")],
    )
    .await;
    enable(
        &db,
        &target.id,
        "coolmod-ue4ss",
        Some(&second.version.id),
        true,
    )
    .await;

    let set = desired::build(&db, &target, &format!("{}/default", target.id))
        .await
        .unwrap();
    assert_eq!(set.files[0].mod_version_id, second.version.id);
    assert_eq!(
        set.files[0].expected_hash,
        ps_server::services::mods::digest::hash_bytes(b"v2"),
        "and the hash is version 2's, not the current version's"
    );
}

#[tokio::test]
async fn a_framework_is_desired_regardless_of_the_profile() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let target = client_target(&db, &dir.path().join("Palworld")).await;
    default_profile(&db, &target.id).await;
    let framework = install_framework(
        &db,
        &paths,
        "framework-palschema",
        "3.0",
        &[("ue4ss", "PalSchema/dlls/main.dll", b"dll")],
    )
    .await;
    ps_db::mod_profiles::set_framework(&db, &target.id, "palschema", &framework.version.id)
        .await
        .unwrap();

    let set = desired::build(&db, &target, &format!("{}/default", target.id))
        .await
        .unwrap();
    assert_eq!(set.files.len(), 1, "{:?}", set.files);
    assert_eq!(set.files[0].mod_version_id, framework.version.id);
    assert!(
        set.ue4ss_entries.is_empty(),
        "a framework is not a mods.txt entry: {:?}",
        set.ue4ss_entries
    );
}

#[tokio::test]
async fn a_mod_with_no_version_at_all_is_an_error_naming_it() {
    let (db, dir) = db_and_dir().await;
    let target = client_target(&db, &dir.path().join("Palworld")).await;
    default_profile(&db, &target.id).await;
    // A mods row with no versions, which the library refuses to create through
    // `store` but which a failed install could leave.
    ps_db::mod_library::upsert_mod(
        &db,
        &ps_db::mod_library::NewMod {
            id: "ghost-ue4ss".to_string(),
            name: "Ghost".to_string(),
            mod_type: "ue4ss".to_string(),
            source_kind: "local".to_string(),
            source_ref: "{}".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    enable(&db, &target.id, "ghost-ue4ss", None, true).await;

    let failed = desired::build(&db, &target, &format!("{}/default", target.id)).await;
    assert!(
        matches!(failed, Err(desired::DesiredError::NoVersion(ref m)) if m == "ghost-ue4ss"),
        "{failed:?}"
    );
}

#[tokio::test]
async fn a_workshop_mod_lists_its_package_when_enabled() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let target = client_target(&db, &dir.path().join("Palworld")).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolpack-workshop",
        "1.0",
        &[("workshop", "CoolPack/CoolPack_P.pak", b"pak")],
    )
    .await;
    enable(&db, &target.id, "coolpack-workshop", None, true).await;

    let set = desired::build(&db, &target, &format!("{}/default", target.id))
        .await
        .unwrap();
    assert_eq!(set.workshop_active, vec!["CoolPack".to_string()]);
    assert!(set.ue4ss_entries.is_empty());
}

#[tokio::test]
async fn shared_markers_are_never_in_the_desired_set() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let target = client_target(&db, &dir.path().join("Palworld")).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[("ue4ss", "CoolMod/Scripts/main.lua", b"a")],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;

    let set = desired::build(&db, &target, &format!("{}/default", target.id))
        .await
        .unwrap();
    assert!(
        set.files.iter().all(|f| f.role != Role::SharedMarker),
        "a shared marker in the desired set can be classified Preserve, which the \
         design forbids: {:?}",
        set.files
    );
    assert!(
        set.files.iter().all(|f| !f.path.ends_with("mods.txt")),
        "{:?}",
        set.files
    );
}

#[tokio::test]
async fn a_mods_own_marker_and_a_paks_companions_carry_their_roles() {
    // The role is what a deployment row stores, and a row that says "file" for
    // a mod's `enabled.txt` or for a pak's `.ucas` is wrong for good: nothing
    // inside the deployer reads it back, so only this says so.
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let target = client_target(&db, &dir.path().join("Palworld")).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[
            ("ue4ss", "CoolMod/Scripts/main.lua", b"a"),
            ("ue4ss", "CoolMod/enabled.txt", b""),
        ],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-pak",
        "1.0",
        &[
            ("pak", "CoolMod_P.pak", b"pak"),
            ("pak", "CoolMod_P.ucas", b"ucas"),
            ("pak", "CoolMod_P.utoc", b"utoc"),
        ],
    )
    .await;
    enable(&db, &target.id, "coolmod-pak", None, true).await;

    let set = desired::build(&db, &target, &format!("{}/default", target.id))
        .await
        .unwrap();
    let role_of = |ends_with: &str| {
        set.files
            .iter()
            .find(|f| f.path.ends_with(ends_with))
            .unwrap_or_else(|| panic!("no desired file ending {ends_with}: {:?}", set.files))
            .role
    };
    assert_eq!(role_of("enabled.txt"), Role::Marker);
    assert_eq!(role_of("CoolMod_P.ucas"), Role::Companion);
    assert_eq!(role_of("CoolMod_P.utoc"), Role::Companion);
    assert_eq!(role_of("CoolMod_P.pak"), Role::File);
    assert_eq!(role_of("main.lua"), Role::File);
}
