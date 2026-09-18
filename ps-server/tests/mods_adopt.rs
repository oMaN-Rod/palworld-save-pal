use std::path::Path;

mod common;

use ps_core::mods::RouteKind;
use ps_db::mod_targets::{ModTarget, NewModTarget};
use ps_server::services::mods::{adopt, scan, LibraryPaths};

async fn db_and_dir() -> (ps_db::SqlxSqliteDriver, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let pool = ps_db::open(&dir.path().join("test.db")).await.unwrap();
    (ps_db::SqlxSqliteDriver::new(pool), dir)
}

fn write(path: &Path, body: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

fn install(root: &Path) {
    write(
        &root.join("Pal/Binaries/Win64/Palworld-Win64-Shipping.exe"),
        b"x",
    );
    std::fs::create_dir_all(root.join("Pal/Binaries/Win64/ue4ss/Mods")).unwrap();
    std::fs::create_dir_all(root.join("Pal/Content/Paks/~mods")).unwrap();
    std::fs::create_dir_all(root.join("Pal/Content/Paks/LogicMods")).unwrap();
}

fn ue4ss_mods(root: &Path) -> std::path::PathBuf {
    root.join("Pal/Binaries/Win64/ue4ss/Mods")
}

/// A target with an active default profile, which adoption requires.
async fn target_with_profile(db: &ps_db::SqlxSqliteDriver, root: &Path) -> ModTarget {
    let target = ps_db::mod_targets::upsert(
        db,
        &NewModTarget {
            id: "client-steam".to_string(),
            kind: "client".to_string(),
            name: "Steam".to_string(),
            root_path: root.to_string_lossy().into_owned(),
            platform: "win64".to_string(),
            ue4ss_mode: "standard".to_string(),
            layout_overrides: "{}".to_string(),
            detected: "{}".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    ps_db::mod_profiles::create(
        db,
        &ps_db::mod_profiles::NewProfile {
            id: "client-steam/default".to_string(),
            target_id: "client-steam".to_string(),
            name: "Default".to_string(),
            is_default: true,
        },
    )
    .await
    .unwrap();
    target
}

#[tokio::test]
async fn the_version_name_is_the_date() {
    assert_eq!(
        adopt::adopted_version("20260913", "client-steam"),
        "adopted-20260913-client-steam"
    );
}

#[tokio::test]
async fn adopting_a_ue4ss_mod_leaves_the_next_scan_with_nothing_to_do() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    install(&root);
    let target = target_with_profile(&db, &root).await;

    let main = ue4ss_mods(&root).join("HandMade/Scripts/main.lua");
    let marker = ue4ss_mods(&root).join("HandMade/enabled.txt");
    write(&main, b"print('mine')");
    write(&marker, b"");
    let before = std::fs::read(&main).unwrap();

    let report = scan::scan(&db, &target).await.unwrap();
    assert_eq!(report.candidates.len(), 1, "{:?}", report.candidates);
    let adopted = adopt::adopt(&db, &paths, &target, &report.candidates[0])
        .await
        .unwrap();

    assert_eq!(adopted.mod_id, "handmade-ue4ss");
    assert!(adopted.version_id.starts_with("handmade-ue4ss@adopted-"));
    assert_eq!(adopted.files, 2);
    assert!(adopted.enabled, "enabled.txt was present");

    // Nothing on disk changed.
    assert_eq!(std::fs::read(&main).unwrap(), before);
    assert!(marker.is_file(), "the marker is left exactly as it was");

    // And the next scan sees a fully managed, unchanged target.
    let after = scan::scan(&db, &target).await.unwrap();
    assert!(
        after.candidates.is_empty(),
        "nothing is left to adopt: {:?}",
        after.candidates
    );
    assert!(after.drifted.is_empty(), "{:?}", after.drifted);
    assert!(after.missing.is_empty(), "{:?}", after.missing);
    assert!(
        after
            .files
            .iter()
            .all(|f| f.state == scan::FileState::ManagedIntact),
        "{:?}",
        after.files
    );
}

#[tokio::test]
async fn the_adopted_files_are_copied_into_the_library_under_their_route_kind() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    install(&root);
    let target = target_with_profile(&db, &root).await;
    write(
        &ue4ss_mods(&root).join("HandMade/Scripts/main.lua"),
        b"print('mine')",
    );

    let report = scan::scan(&db, &target).await.unwrap();
    let adopted = adopt::adopt(&db, &paths, &target, &report.candidates[0])
        .await
        .unwrap();

    let version = ps_db::mod_library::get_version(&db, &adopted.version_id)
        .await
        .unwrap()
        .unwrap();
    let copied = Path::new(&version.library_dir).join("ue4ss/HandMade/Scripts/main.lua");
    assert_eq!(
        std::fs::read_to_string(&copied).unwrap(),
        "print('mine')",
        "the library holds its own copy at {copied:?}"
    );
    let row = ps_db::mod_library::get_mod(&db, "handmade-ue4ss")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.source_kind, "adopted");
}

#[tokio::test]
async fn the_enabled_state_read_from_disk_reaches_the_active_profile() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    install(&root);
    let target = target_with_profile(&db, &root).await;

    // mods.txt says one is off and one is on; neither has a marker file.
    write(&ue4ss_mods(&root).join("OnMod/Scripts/main.lua"), b"a");
    write(&ue4ss_mods(&root).join("OffMod/Scripts/main.lua"), b"b");
    write(
        &ue4ss_mods(&root).join("mods.txt"),
        b"OnMod : 1\nOffMod : 0\n",
    );

    let report = scan::scan(&db, &target).await.unwrap();
    for candidate in &report.candidates {
        adopt::adopt(&db, &paths, &target, candidate).await.unwrap();
    }

    let entries = ps_db::mod_profiles::mods_of(&db, "client-steam/default")
        .await
        .unwrap();
    let on = entries.iter().find(|e| e.mod_id == "onmod-ue4ss").unwrap();
    let off = entries.iter().find(|e| e.mod_id == "offmod-ue4ss").unwrap();
    assert!(on.enabled, "mods.txt had it on");
    assert!(
        !off.enabled,
        "mods.txt had it off, and adoption must not turn it on"
    );
    assert!(
        on.mod_version_id
            .as_deref()
            .is_some_and(|v| v.starts_with("onmod-ue4ss@adopted-")),
        "pinned, not following the current version: two targets can adopt a mod of          the same name and must not resolve to each other's bytes"
    );

    // The marker file is untouched: adoption writes no markers.
    assert_eq!(
        std::fs::read_to_string(ue4ss_mods(&root).join("mods.txt")).unwrap(),
        "OnMod : 1\nOffMod : 0\n"
    );
}

#[tokio::test]
async fn adopting_a_loose_pak_records_it_at_its_real_path() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    install(&root);
    let target = target_with_profile(&db, &root).await;
    let pak = root.join("Pal/Content/Paks/~mods/Cool_P.pak");
    write(&pak, b"pakbytes");

    let report = scan::scan(&db, &target).await.unwrap();
    let candidate = report
        .candidates
        .iter()
        .find(|c| c.kind == RouteKind::Pak)
        .unwrap();
    let adopted = adopt::adopt(&db, &paths, &target, candidate).await.unwrap();
    assert_eq!(adopted.mod_id, "cool_p.pak-pak");

    let rows = ps_db::mod_deployments::files_of(&db, "client-steam")
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        Path::new(&rows[0].path),
        pak.as_path(),
        "the row names the file where it actually is"
    );
    assert_eq!(
        rows[0].hash,
        ps_server::services::mods::digest::hash_file(&pak).unwrap()
    );
    assert_eq!(rows[0].rel_path.as_deref(), Some("Cool_P.pak"));
    assert_eq!(
        rows[0].mod_version_id.as_deref(),
        Some(adopted.version_id.as_str())
    );
    assert_eq!(rows[0].role, "file");
    assert_eq!(
        std::fs::read(&pak).unwrap(),
        b"pakbytes",
        "still there, unchanged"
    );
}

fn subscribed_candidates(report: &scan::ScanReport) -> Vec<&scan::AdoptionCandidate> {
    report
        .candidates
        .iter()
        .filter(|c| c.source == scan::CandidateSource::SteamSubscribed)
        .collect()
}

#[tokio::test]
async fn adopting_a_subscribed_package_registers_it_and_copies_nothing() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(&dir.path().join("app"));
    let library = dir.path().join("SteamLibrary");
    let root = library.join("steamapps/common/Palworld");
    install(&root);
    let target = target_with_profile(&db, &root).await;
    let content = library.join("steamapps/workshop/content/1623730");
    write(
        &content.join("3300000001/Info.json"),
        br#"{"PackageName":"SteamPack","ModName":"Steam Pack","Author":"Someone"}"#,
    );
    write(&content.join("3300000001/Paks/SteamPack_P.pak"), b"steam bytes");
    write(
        &content.join("3300000002/Info.json"),
        br#"{"PackageName":"QuietPack"}"#,
    );
    let ini = root.join("Mods/PalModSettings.ini");
    write(
        &ini,
        b"[PalModSettings]\nbGlobalEnableMod=True\nActiveModList=SteamPack\n",
    );
    let steam_before = common::mods::tree_snapshot(&content);
    let ini_before = std::fs::read(&ini).unwrap();

    let report = scan::scan(&db, &target).await.unwrap();
    let candidates = subscribed_candidates(&report);
    assert_eq!(candidates.len(), 2, "{:?}", report.candidates);
    let mut adopted = Vec::new();
    for candidate in candidates {
        adopted.push((
            candidate.name.clone(),
            adopt::adopt(&db, &paths, &target, candidate).await.unwrap(),
        ));
    }

    assert_eq!(
        common::mods::tree_snapshot(&content),
        steam_before,
        "nothing under Steam's directory is written, moved or removed"
    );
    assert_eq!(std::fs::read(&ini).unwrap(), ini_before, "adoption writes no marker");
    assert!(
        ps_db::mod_deployments::files_of(&db, "client-steam")
            .await
            .unwrap()
            .is_empty(),
        "nothing is recorded as deployed"
    );

    let (_, pack) = adopted.iter().find(|(name, _)| name == "SteamPack").unwrap();
    assert_eq!(pack.files, 0);
    assert_eq!(pack.mod_id, "workshop-3300000001", "keyed on the Steam item");
    assert!(pack.enabled);
    let row = ps_db::mod_library::get_mod(&db, &pack.mod_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.mod_type, "workshop");
    assert_eq!(row.source_kind, "workshop");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&row.source_ref).unwrap(),
        serde_json::json!({"kind": "workshop", "package": "SteamPack", "workshop_id": "3300000001"})
    );
    let versions = ps_db::mod_library::versions_of(&db, &pack.mod_id)
        .await
        .unwrap();
    assert_eq!(versions.len(), 1, "{versions:?}");
    assert_eq!(versions[0].id, pack.version_id);
    let manifest: ps_core::mods::InstallManifest =
        serde_json::from_str(&versions[0].manifest).unwrap();
    assert_eq!(
        manifest.folder_name, "SteamPack",
        "ActiveModList names it by its PackageName"
    );
    assert!(manifest.routes.is_empty(), "{:?}", manifest.routes);
    let library_dir = Path::new(&versions[0].library_dir);
    assert!(library_dir.is_dir(), "the version has a real directory");
    assert!(library_dir.starts_with(paths.root()), "{library_dir:?}");
    assert_eq!(
        std::fs::read_dir(library_dir).unwrap().count(),
        0,
        "and the library holds no copy of Steam's files"
    );

    let entries = ps_db::mod_profiles::mods_of(&db, "client-steam/default")
        .await
        .unwrap();
    let entry_of = |name: &str| {
        let (_, adopted) = adopted.iter().find(|(n, _)| n == name).unwrap();
        entries
            .iter()
            .find(|e| e.mod_id == adopted.mod_id)
            .unwrap()
            .clone()
    };
    assert!(entry_of("SteamPack").enabled, "ActiveModList had it on");
    assert!(!entry_of("QuietPack").enabled, "ActiveModList did not list it");

    let after = scan::scan(&db, &target).await.unwrap();
    assert!(
        subscribed_candidates(&after).is_empty(),
        "a package the target holds is not offered again: {:?}",
        after.candidates
    );
}

#[tokio::test]
async fn a_second_target_subscribed_to_the_same_package_shares_its_row() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(&dir.path().join("app"));
    let library = dir.path().join("SteamLibrary");
    let root_a = library.join("steamapps/common/Palworld");
    install(&root_a);
    let target_a = target_with_profile(&db, &root_a).await;
    let content = library.join("steamapps/workshop/content/1623730");
    write(
        &content.join("3300000001/Info.json"),
        br#"{"PackageName":"SteamPack"}"#,
    );
    write(
        &root_a.join("Mods/PalModSettings.ini"),
        b"[PalModSettings]\nActiveModList=SteamPack\n",
    );

    let root_b = dir.path().join("Other/Palworld");
    install(&root_b);
    let target_b = ps_db::mod_targets::upsert(
        &db,
        &NewModTarget {
            id: "client-other".to_string(),
            kind: "client".to_string(),
            name: "Other".to_string(),
            root_path: root_b.to_string_lossy().into_owned(),
            platform: "win64".to_string(),
            ue4ss_mode: "standard".to_string(),
            layout_overrides: "{}".to_string(),
            detected: "{}".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    ps_db::mod_profiles::create(
        &db,
        &ps_db::mod_profiles::NewProfile {
            id: "client-other/default".to_string(),
            target_id: "client-other".to_string(),
            name: "Default".to_string(),
            is_default: true,
        },
    )
    .await
    .unwrap();
    write(
        &root_b.join("Mods/PalModSettings.ini"),
        format!("[PalModSettings]\nWorkshopRootDir={}\n", content.display()).as_bytes(),
    );

    let report_a = scan::scan(&db, &target_a).await.unwrap();
    let first = adopt::adopt(&db, &paths, &target_a, subscribed_candidates(&report_a)[0])
        .await
        .unwrap();
    let report_b = scan::scan(&db, &target_b).await.unwrap();
    let offered = subscribed_candidates(&report_b);
    assert_eq!(
        offered.len(),
        1,
        "another target's registration does not manage this one: {:?}",
        report_b.candidates
    );
    let second = adopt::adopt(&db, &paths, &target_b, offered[0]).await.unwrap();

    assert_eq!(second.mod_id, first.mod_id);
    assert_eq!(second.version_id, first.version_id);
    assert_eq!(ps_db::mod_library::list_mods(&db).await.unwrap().len(), 1);
    let entry = &ps_db::mod_profiles::mods_of(&db, "client-other/default")
        .await
        .unwrap()[0];
    assert!(!entry.enabled, "each target keeps its own ActiveModList state");
}

#[tokio::test]
async fn a_subscribed_package_the_library_installed_itself_is_refused() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(&dir.path().join("app"));
    let root = dir.path().join("Palworld");
    install(&root);
    let target = target_with_profile(&db, &root).await;
    common::mods::install_fixture_mod(
        &db,
        &paths,
        "steampack-workshop",
        "1.0",
        &[("workshop", "SteamPack/Info.json", br#"{"PackageName":"SteamPack"}"#)],
    )
    .await;

    let folder = dir.path().join("steam/3300000001");
    let candidate = scan::AdoptionCandidate {
        name: "SteamPack".to_string(),
        kind: RouteKind::Workshop,
        root: folder.to_string_lossy().into_owned(),
        files: vec![folder.join("Info.json").to_string_lossy().into_owned()],
        enabled: true,
        source: scan::CandidateSource::SteamSubscribed,
    };
    let failed = adopt::adopt(&db, &paths, &target, &candidate).await;
    assert!(
        matches!(failed, Err(adopt::AdoptError::AlreadyManaged(_))),
        "{failed:?}"
    );
    assert!(ps_db::mod_profiles::mods_of(&db, "client-steam/default")
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn two_packages_whose_names_slug_alike_keep_their_own_rows_and_states() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(&dir.path().join("app"));
    let library = dir.path().join("SteamLibrary");
    let root = library.join("steamapps/common/Palworld");
    install(&root);
    let target = target_with_profile(&db, &root).await;
    let content = library.join("steamapps/workshop/content/1623730");
    write(
        &content.join("3300000001/Info.json"),
        br#"{"PackageName":"Better UI"}"#,
    );
    write(
        &content.join("3300000002/Info.json"),
        br#"{"PackageName":"Better_UI"}"#,
    );
    write(
        &root.join("Mods/PalModSettings.ini"),
        b"[PalModSettings]\nbGlobalEnableMod=True\nActiveModList=Better UI\n",
    );

    let report = scan::scan(&db, &target).await.unwrap();
    let candidates = subscribed_candidates(&report);
    assert_eq!(candidates.len(), 2, "{:?}", report.candidates);
    for candidate in candidates {
        adopt::adopt(&db, &paths, &target, candidate).await.unwrap();
    }

    let rows = ps_db::mod_library::list_mods(&db).await.unwrap();
    assert_eq!(rows.len(), 2, "{rows:?}");
    let entries = ps_db::mod_profiles::mods_of(&db, "client-steam/default")
        .await
        .unwrap();
    let enabled_of = |id: &str| {
        entries
            .iter()
            .find(|e| e.mod_id == id)
            .unwrap_or_else(|| panic!("{id} is not held: {entries:?}"))
            .enabled
    };
    assert!(
        enabled_of("workshop-3300000001"),
        "Better UI keeps its own ActiveModList state"
    );
    assert!(!enabled_of("workshop-3300000002"));
    let after = scan::scan(&db, &target).await.unwrap();
    assert!(
        subscribed_candidates(&after).is_empty(),
        "{:?}",
        after.candidates
    );
}

#[tokio::test]
async fn a_steam_item_whose_package_name_changed_is_not_folded_into_its_old_row() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(&dir.path().join("app"));
    let library = dir.path().join("SteamLibrary");
    let root = library.join("steamapps/common/Palworld");
    install(&root);
    let target = target_with_profile(&db, &root).await;
    let info = library.join("steamapps/workshop/content/1623730/3300000001/Info.json");
    write(&info, br#"{"PackageName":"SteamPack"}"#);

    let report = scan::scan(&db, &target).await.unwrap();
    adopt::adopt(&db, &paths, &target, subscribed_candidates(&report)[0])
        .await
        .unwrap();
    write(&info, br#"{"PackageName":"Renamed"}"#);
    let report = scan::scan(&db, &target).await.unwrap();
    let renamed = subscribed_candidates(&report);
    assert_eq!(renamed.len(), 1, "{:?}", report.candidates);
    assert_eq!(renamed[0].name, "Renamed");

    let failed = adopt::adopt(&db, &paths, &target, renamed[0]).await;
    assert!(
        matches!(failed, Err(adopt::AdoptError::AlreadyManaged(_))),
        "{failed:?}"
    );
    let row = ps_db::mod_library::get_mod(&db, "workshop-3300000001")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        ps_server::services::mods::library::workshop_package_of(&row).as_deref(),
        Some("SteamPack")
    );
    assert_eq!(
        ps_db::mod_profiles::mods_of(&db, "client-steam/default")
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn adoption_without_an_active_profile_is_refused_and_writes_nothing() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    install(&root);
    // A target with no profile at all.
    let target = ps_db::mod_targets::upsert(
        &db,
        &NewModTarget {
            id: "client-steam".to_string(),
            kind: "client".to_string(),
            name: "Steam".to_string(),
            root_path: root.to_string_lossy().into_owned(),
            platform: "win64".to_string(),
            ue4ss_mode: "standard".to_string(),
            layout_overrides: "{}".to_string(),
            detected: "{}".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    write(&ue4ss_mods(&root).join("HandMade/Scripts/main.lua"), b"a");

    let report = scan::scan(&db, &target).await.unwrap();
    let failed = adopt::adopt(&db, &paths, &target, &report.candidates[0]).await;
    assert!(matches!(failed, Err(adopt::AdoptError::NoActiveProfile(_))));
    assert!(
        ps_db::mod_deployments::files_of(&db, "client-steam")
            .await
            .unwrap()
            .is_empty(),
        "a refused adoption records nothing"
    );
}

#[tokio::test]
async fn a_candidate_whose_files_are_outside_the_layout_is_refused() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    install(&root);
    let target = target_with_profile(&db, &root).await;

    let elsewhere = dir.path().join("Elsewhere/Thing.pak");
    write(&elsewhere, b"x");
    let candidate = scan::AdoptionCandidate {
        name: "Thing.pak".to_string(),
        kind: RouteKind::Pak,
        root: elsewhere.to_string_lossy().into_owned(),
        files: vec![elsewhere.to_string_lossy().into_owned()],
        enabled: true,
        source: scan::CandidateSource::Local,
    };
    let failed = adopt::adopt(&db, &paths, &target, &candidate).await;
    assert!(
        matches!(failed, Err(adopt::AdoptError::OutsideLayout(_, _))),
        "a candidate must sit under the layout location its kind names: {failed:?}"
    );
}

#[tokio::test]
async fn two_targets_can_adopt_a_mod_of_the_same_name_on_the_same_day() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());

    let root_a = dir.path().join("A");
    let root_b = dir.path().join("B");
    install(&root_a);
    install(&root_b);
    write(
        &ue4ss_mods(&root_a).join("HandMade/Scripts/main.lua"),
        b"from A",
    );
    write(
        &ue4ss_mods(&root_b).join("HandMade/Scripts/main.lua"),
        b"from B",
    );

    let mut targets = Vec::new();
    for (id, root) in [("client-a", &root_a), ("client-b", &root_b)] {
        let target = ps_db::mod_targets::upsert(
            &db,
            &NewModTarget {
                id: id.to_string(),
                kind: "client".to_string(),
                name: id.to_string(),
                root_path: root.to_string_lossy().into_owned(),
                platform: "win64".to_string(),
                ue4ss_mode: "standard".to_string(),
                layout_overrides: "{}".to_string(),
                detected: "{}".to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        ps_db::mod_profiles::create(
            &db,
            &ps_db::mod_profiles::NewProfile {
                id: format!("{id}/default"),
                target_id: id.to_string(),
                name: "Default".to_string(),
                is_default: true,
            },
        )
        .await
        .unwrap();
        targets.push(target);
    }

    let mut adopted = Vec::new();
    for target in &targets {
        let report = scan::scan(&db, target).await.unwrap();
        assert_eq!(report.candidates.len(), 1, "{:?}", report.candidates);
        adopted.push(
            adopt::adopt(&db, &paths, target, &report.candidates[0])
                .await
                .unwrap(),
        );
    }

    assert_ne!(
        adopted[0].version_id, adopted[1].version_id,
        "each target gets its own version, so the same day is not a collision"
    );

    // Each profile entry pins its own target's version, so neither resolves to the
    // other's bytes.
    for (index, target) in targets.iter().enumerate() {
        let entries = ps_db::mod_profiles::mods_of(&db, &format!("{}/default", target.id))
            .await
            .unwrap();
        assert_eq!(
            entries[0].mod_version_id.as_deref(),
            Some(adopted[index].version_id.as_str()),
            "{} must pin its own version",
            target.id
        );
    }

    // And each target still sees its own bytes as managed and intact.
    for target in &targets {
        let after = scan::scan(&db, target).await.unwrap();
        assert!(after.candidates.is_empty(), "{:?}", after.candidates);
        assert!(after.drifted.is_empty(), "{:?}", after.drifted);
    }
    assert_eq!(
        std::fs::read_to_string(ue4ss_mods(&root_a).join("HandMade/Scripts/main.lua")).unwrap(),
        "from A",
        "neither adoption rewrote the other target's file"
    );
}
