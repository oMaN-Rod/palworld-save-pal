use std::path::Path;

use ps_core::mods::RouteKind;
use ps_db::mod_targets::{ModTarget, NewModTarget};
use ps_server::services::mods::scan::{self, CandidateSource, FileState};

async fn db_and_dir() -> (ps_db::SqlxSqliteDriver, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let pool = ps_db::open(&dir.path().join("test.db")).await.unwrap();
    (ps_db::SqlxSqliteDriver::new(pool), dir)
}

fn write(path: &Path, body: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

/// A client install with the directories the layout expects.
fn install(root: &Path) {
    write(
        &root.join("Pal/Binaries/Win64/Palworld-Win64-Shipping.exe"),
        b"x",
    );
    std::fs::create_dir_all(root.join("Pal/Binaries/Win64/ue4ss/Mods")).unwrap();
    std::fs::create_dir_all(root.join("Pal/Content/Paks/~mods")).unwrap();
    std::fs::create_dir_all(root.join("Pal/Content/Paks/LogicMods")).unwrap();
}

async fn target_row(db: &ps_db::SqlxSqliteDriver, root: &Path) -> ModTarget {
    ps_db::mod_targets::upsert(
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
    .unwrap()
}

fn ue4ss_mods(root: &Path) -> std::path::PathBuf {
    root.join("Pal/Binaries/Win64/ue4ss/Mods")
}

#[tokio::test]
async fn an_empty_target_scans_to_nothing() {
    let (db, dir) = db_and_dir().await;
    let root = dir.path().join("Palworld");
    install(&root);
    let target = target_row(&db, &root).await;
    let report = scan::scan(&db, &target).await.unwrap();
    assert_eq!(report.target_id, "client-steam");
    assert!(report.files.is_empty(), "{:?}", report.files);
    assert!(report.candidates.is_empty());
    assert!(report.drifted.is_empty());
    assert!(report.missing.is_empty());
}

#[tokio::test]
async fn a_recorded_file_whose_bytes_match_is_managed_intact() {
    let (db, dir) = db_and_dir().await;
    let root = dir.path().join("Palworld");
    install(&root);
    let target = target_row(&db, &root).await;

    let file = ue4ss_mods(&root).join("CoolMod/Scripts/main.lua");
    write(&file, b"print('hi')");
    let hash = ps_server::services::mods::digest::hash_file(&file).unwrap();
    ps_db::mod_deployments::record(
        &db,
        &[ps_db::mod_deployments::NewDeploymentFile {
            target_id: "client-steam".to_string(),
            path: file.to_string_lossy().into_owned(),
            mod_version_id: None,
            hash,
            role: "shared_marker".to_string(),
            rel_path: None,
        }],
    )
    .await
    .unwrap();

    let report = scan::scan(&db, &target).await.unwrap();
    let found = report
        .files
        .iter()
        .find(|f| f.path == file.to_string_lossy())
        .expect("the recorded file is reported");
    assert_eq!(found.state, FileState::ManagedIntact);
    assert!(report.drifted.is_empty());
    assert!(
        report.candidates.is_empty(),
        "a managed file is not an adoption candidate: {:?}",
        report.candidates
    );
}

#[tokio::test]
async fn an_edited_recorded_file_is_managed_drifted() {
    let (db, dir) = db_and_dir().await;
    let root = dir.path().join("Palworld");
    install(&root);
    let target = target_row(&db, &root).await;

    let file = ue4ss_mods(&root).join("CoolMod/Scripts/main.lua");
    write(&file, b"print('hi')");
    let hash = ps_server::services::mods::digest::hash_file(&file).unwrap();
    ps_db::mod_deployments::record(
        &db,
        &[ps_db::mod_deployments::NewDeploymentFile {
            target_id: "client-steam".to_string(),
            path: file.to_string_lossy().into_owned(),
            mod_version_id: None,
            hash,
            role: "shared_marker".to_string(),
            rel_path: None,
        }],
    )
    .await
    .unwrap();

    // The user edits the file in place.
    write(&file, b"print('edited by hand')");

    let report = scan::scan(&db, &target).await.unwrap();
    let found = report
        .files
        .iter()
        .find(|f| f.path == file.to_string_lossy())
        .unwrap();
    assert_eq!(found.state, FileState::ManagedDrifted);
    assert_eq!(report.drifted, vec![file.to_string_lossy().into_owned()]);
}

#[tokio::test]
async fn a_recorded_file_that_is_gone_is_managed_missing_not_drifted() {
    let (db, dir) = db_and_dir().await;
    let root = dir.path().join("Palworld");
    install(&root);
    let target = target_row(&db, &root).await;

    let absent = ue4ss_mods(&root).join("GoneMod/main.lua");
    ps_db::mod_deployments::record(
        &db,
        &[ps_db::mod_deployments::NewDeploymentFile {
            target_id: "client-steam".to_string(),
            path: absent.to_string_lossy().into_owned(),
            mod_version_id: None,
            hash: "whatever".to_string(),
            role: "shared_marker".to_string(),
            rel_path: None,
        }],
    )
    .await
    .unwrap();

    let report = scan::scan(&db, &target).await.unwrap();
    let found = report
        .files
        .iter()
        .find(|f| f.path == absent.to_string_lossy())
        .unwrap();
    assert_eq!(
        found.state,
        FileState::ManagedMissing,
        "a deleted file is not the same as an edited one"
    );
    assert_eq!(report.missing, vec![absent.to_string_lossy().into_owned()]);
    assert!(report.drifted.is_empty());
}

#[tokio::test]
async fn an_unmanaged_ue4ss_folder_becomes_one_candidate_with_all_its_files() {
    let (db, dir) = db_and_dir().await;
    let root = dir.path().join("Palworld");
    install(&root);
    let target = target_row(&db, &root).await;

    write(&ue4ss_mods(&root).join("HandMade/Scripts/main.lua"), b"a");
    write(&ue4ss_mods(&root).join("HandMade/Scripts/extra.lua"), b"b");
    write(&ue4ss_mods(&root).join("HandMade/enabled.txt"), b"");

    let report = scan::scan(&db, &target).await.unwrap();
    assert_eq!(report.candidates.len(), 1, "{:?}", report.candidates);
    let candidate = &report.candidates[0];
    assert_eq!(candidate.name, "HandMade");
    assert_eq!(candidate.kind, RouteKind::Ue4ss);
    assert_eq!(candidate.files.len(), 3, "{:?}", candidate.files);
    assert!(candidate.enabled, "enabled.txt is present");
    assert_eq!(candidate.source, CandidateSource::Local);
    assert!(
        report.files.iter().all(|f| f.state == FileState::Unmanaged),
        "{:?}",
        report.files
    );
}

#[tokio::test]
async fn a_ue4ss_folder_without_enabled_txt_is_reported_disabled() {
    let (db, dir) = db_and_dir().await;
    let root = dir.path().join("Palworld");
    install(&root);
    let target = target_row(&db, &root).await;
    write(&ue4ss_mods(&root).join("Quiet/Scripts/main.lua"), b"a");

    let report = scan::scan(&db, &target).await.unwrap();
    assert_eq!(report.candidates.len(), 1);
    assert!(!report.candidates[0].enabled);
}

#[tokio::test]
async fn mods_txt_overrides_the_enabled_txt_marker() {
    let (db, dir) = db_and_dir().await;
    let root = dir.path().join("Palworld");
    install(&root);
    let target = target_row(&db, &root).await;

    // On disk the folder looks enabled, but mods.txt — the file UE4SS reads for a
    // mod it lists — says otherwise.
    write(&ue4ss_mods(&root).join("Disputed/Scripts/main.lua"), b"a");
    write(&ue4ss_mods(&root).join("Disputed/enabled.txt"), b"");
    write(&ue4ss_mods(&root).join("mods.txt"), b"Disputed : 0\n");

    let report = scan::scan(&db, &target).await.unwrap();
    let candidate = report
        .candidates
        .iter()
        .find(|c| c.name == "Disputed")
        .unwrap();
    assert!(
        !candidate.enabled,
        "mods.txt is authoritative for a mod it names"
    );
}

#[tokio::test]
async fn mods_txt_enabling_a_folder_with_no_marker_reports_enabled() {
    let (db, dir) = db_and_dir().await;
    let root = dir.path().join("Palworld");
    install(&root);
    let target = target_row(&db, &root).await;
    write(&ue4ss_mods(&root).join("Listed/Scripts/main.lua"), b"a");
    write(&ue4ss_mods(&root).join("mods.txt"), b"Listed : 1\n");

    let report = scan::scan(&db, &target).await.unwrap();
    let candidate = report
        .candidates
        .iter()
        .find(|c| c.name == "Listed")
        .unwrap();
    assert!(candidate.enabled);
}

#[tokio::test]
async fn a_loose_pak_is_its_own_candidate_and_counts_as_enabled() {
    let (db, dir) = db_and_dir().await;
    let root = dir.path().join("Palworld");
    install(&root);
    let target = target_row(&db, &root).await;
    write(&root.join("Pal/Content/Paks/~mods/Cool_P.pak"), b"pak");
    write(&root.join("Pal/Content/Paks/LogicMods/Logic_P.pak"), b"pak");

    let report = scan::scan(&db, &target).await.unwrap();
    let pak = report
        .candidates
        .iter()
        .find(|c| c.kind == RouteKind::Pak)
        .expect("the ~mods pak");
    assert_eq!(pak.name, "Cool_P.pak");
    assert_eq!(pak.files.len(), 1);
    assert!(pak.enabled, "a pak is enabled by being present");

    let logic = report
        .candidates
        .iter()
        .find(|c| c.kind == RouteKind::LogicMods)
        .expect("the LogicMods pak");
    assert_eq!(logic.name, "Logic_P.pak");
}

#[tokio::test]
async fn a_workshop_package_reads_its_enabled_state_from_palmodsettings() {
    let (db, dir) = db_and_dir().await;
    let root = dir.path().join("Palworld");
    install(&root);
    let target = target_row(&db, &root).await;
    write(
        &root.join("Mods/Workshop/CoolPack/Info.json"),
        br#"{"PackageName":"CoolPack","Version":"1.0","InstallRules":[]}"#,
    );
    write(&root.join("Mods/Workshop/CoolPack/CoolPack_P.pak"), b"pak");
    write(
        &root.join("Mods/Workshop/OffPack/Info.json"),
        br#"{"PackageName":"OffPack"}"#,
    );
    write(
        &root.join("Mods/PalModSettings.ini"),
        b"[PalModSettings]\nbGlobalEnableMod=True\nActiveModList=CoolPack\n",
    );

    let report = scan::scan(&db, &target).await.unwrap();
    let on = report
        .candidates
        .iter()
        .find(|c| c.name == "CoolPack")
        .unwrap();
    assert!(on.enabled, "ActiveModList names it");
    assert_eq!(on.kind, RouteKind::Workshop);
    assert_eq!(on.source, CandidateSource::Local);
    let off = report
        .candidates
        .iter()
        .find(|c| c.name == "OffPack")
        .unwrap();
    assert!(!off.enabled);
}

#[tokio::test]
async fn the_palschema_directory_is_not_reported_as_a_ue4ss_mod() {
    let (db, dir) = db_and_dir().await;
    let root = dir.path().join("Palworld");
    install(&root);
    let target = target_row(&db, &root).await;

    // PalSchema's own directory sits inside the UE4SS mods directory, and a
    // PalSchema mod sits inside that.
    write(
        &ue4ss_mods(&root).join("PalSchema/mods/MySchema/pals/pal.json"),
        b"{}",
    );
    write(&ue4ss_mods(&root).join("Ordinary/Scripts/main.lua"), b"a");

    let report = scan::scan(&db, &target).await.unwrap();
    let names: Vec<&str> = report.candidates.iter().map(|c| c.name.as_str()).collect();
    assert!(
        !names.contains(&"PalSchema"),
        "PalSchema is the framework's directory, not a mod: {names:?}"
    );
    assert!(names.contains(&"Ordinary"), "{names:?}");
    assert!(
        names.contains(&"MySchema"),
        "the schema mod is reported under its own location: {names:?}"
    );

    let schema_file = report
        .files
        .iter()
        .filter(|f| f.path.ends_with("pal.json"))
        .count();
    assert_eq!(
        schema_file, 1,
        "a file under a nested location must be reported once, not twice"
    );
}

#[tokio::test]
async fn a_partly_edited_mod_reports_both_states_and_is_not_a_candidate() {
    let (db, dir) = db_and_dir().await;
    let root = dir.path().join("Palworld");
    install(&root);
    let target = target_row(&db, &root).await;

    let intact = ue4ss_mods(&root).join("Mixed/Scripts/main.lua");
    let edited = ue4ss_mods(&root).join("Mixed/Scripts/config.lua");
    write(&intact, b"original");
    write(&edited, b"original");
    let rows: Vec<ps_db::mod_deployments::NewDeploymentFile> = [&intact, &edited]
        .iter()
        .map(|p| ps_db::mod_deployments::NewDeploymentFile {
            target_id: "client-steam".to_string(),
            path: p.to_string_lossy().into_owned(),
            mod_version_id: None,
            hash: ps_server::services::mods::digest::hash_file(p).unwrap(),
            role: "shared_marker".to_string(),
            rel_path: None,
        })
        .collect();
    ps_db::mod_deployments::record(&db, &rows).await.unwrap();
    write(&edited, b"changed by hand");

    let report = scan::scan(&db, &target).await.unwrap();
    assert_eq!(report.drifted.len(), 1);
    assert!(
        report.drifted[0].ends_with("config.lua"),
        "{:?}",
        report.drifted
    );
    assert!(
        report.candidates.is_empty(),
        "a managed mod is never offered for adoption, drifted or not: {:?}",
        report.candidates
    );
}

#[tokio::test]
async fn a_palschema_mod_is_enabled_by_presence_not_by_a_ue4ss_marker() {
    let (db, dir) = db_and_dir().await;
    let root = dir.path().join("Palworld");
    install(&root);
    let target = target_row(&db, &root).await;
    // A hand-installed PalSchema mod: no enabled.txt, no mods.txt line. Per the
    // per-type rules its enable marker is presence, and its disable action is
    // removing files — so reporting it disabled would have the first apply delete
    // a mod that was working.
    write(
        &ue4ss_mods(&root).join("PalSchema/mods/MySchema/pals/pal.json"),
        b"{}",
    );

    let report = scan::scan(&db, &target).await.unwrap();
    let candidate = report
        .candidates
        .iter()
        .find(|c| c.name == "MySchema")
        .expect("the schema mod is a candidate");
    assert_eq!(candidate.kind, RouteKind::PalSchema);
    assert!(
        candidate.enabled,
        "a PalSchema mod is enabled by being present"
    );
}

#[tokio::test]
async fn a_recorded_file_outside_every_mod_location_is_still_classified() {
    let (db, dir) = db_and_dir().await;
    let root = dir.path().join("Palworld");
    install(&root);
    let target = target_row(&db, &root).await;

    // Three rows the location walk never reaches: a shared marker directly in the
    // UE4SS mods directory, a framework file beside the game executable, and the
    // settings ini under the root. All are present and unchanged.
    let marker = ue4ss_mods(&root).join("mods.txt");
    let framework = root.join("Pal/Binaries/Win64/dwmapi.dll");
    let ini = root.join("Mods/PalModSettings.ini");
    write(&marker, b"CoolMod : 1\n");
    write(&framework, b"dll");
    write(&ini, b"[PalModSettings]\n");

    let rows: Vec<ps_db::mod_deployments::NewDeploymentFile> = [&marker, &framework, &ini]
        .iter()
        .map(|p| ps_db::mod_deployments::NewDeploymentFile {
            target_id: "client-steam".to_string(),
            path: p.to_string_lossy().into_owned(),
            mod_version_id: None,
            hash: ps_server::services::mods::digest::hash_file(p).unwrap(),
            role: "shared_marker".to_string(),
            rel_path: None,
        })
        .collect();
    ps_db::mod_deployments::record(&db, &rows).await.unwrap();

    let report = scan::scan(&db, &target).await.unwrap();
    assert!(
        report.missing.is_empty(),
        "present, unchanged files must never be reported missing: {:?}",
        report.missing
    );
    for path in [&marker, &framework, &ini] {
        let found = report
            .files
            .iter()
            .find(|f| Path::new(&f.path) == path.as_path())
            .unwrap_or_else(|| panic!("{path:?} was not classified at all"));
        assert_eq!(found.state, FileState::ManagedIntact, "{path:?}");
    }
}

#[tokio::test]
async fn a_partly_managed_mod_is_not_offered_for_adoption() {
    let (db, dir) = db_and_dir().await;
    let root = dir.path().join("Palworld");
    install(&root);
    let target = target_row(&db, &root).await;

    // The app owns one file in this folder; the user dropped another in by hand.
    let owned = ue4ss_mods(&root).join("CoolMod/Scripts/main.lua");
    let dropped = ue4ss_mods(&root).join("CoolMod/Scripts/patch.lua");
    write(&owned, b"managed");
    write(&dropped, b"by hand");
    ps_db::mod_deployments::record(
        &db,
        &[ps_db::mod_deployments::NewDeploymentFile {
            target_id: "client-steam".to_string(),
            path: owned.to_string_lossy().into_owned(),
            mod_version_id: None,
            hash: ps_server::services::mods::digest::hash_file(&owned).unwrap(),
            role: "shared_marker".to_string(),
            rel_path: None,
        }],
    )
    .await
    .unwrap();

    let report = scan::scan(&db, &target).await.unwrap();
    assert!(
        report.candidates.is_empty(),
        "a folder the app partly owns must not be adoptable: {:?}",
        report.candidates
    );
    assert!(
        report
            .files
            .iter()
            .any(|f| Path::new(&f.path) == dropped.as_path() && f.state == FileState::Unmanaged),
        "the hand-dropped file is still reported as unmanaged"
    );
}

#[tokio::test]
async fn a_workshop_package_is_identified_by_its_package_name_not_its_folder() {
    let (db, dir) = db_and_dir().await;
    let root = dir.path().join("Palworld");
    install(&root);
    let target = target_row(&db, &root).await;
    // The folder is named for the download; ActiveModList names the package.
    write(
        &root.join("Mods/Workshop/CoolPack-v2/Info.json"),
        br#"{"PackageName":"CoolPack","Version":"2.0","InstallRules":[]}"#,
    );
    write(
        &root.join("Mods/Workshop/CoolPack-v2/CoolPack_P.pak"),
        b"pak",
    );
    write(
        &root.join("Mods/PalModSettings.ini"),
        b"[PalModSettings]\nbGlobalEnableMod=True\nActiveModList=CoolPack\n",
    );

    let report = scan::scan(&db, &target).await.unwrap();
    let candidate = report
        .candidates
        .iter()
        .find(|c| c.kind == RouteKind::Workshop)
        .expect("the workshop package");
    assert_eq!(
        candidate.name, "CoolPack",
        "the identity comes from Info.json, not from the directory name"
    );
    assert!(
        candidate.enabled,
        "and the enabled check therefore matches the ActiveModList line"
    );
}

#[tokio::test]
async fn the_mods_txt_lookup_matches_the_writers_case_insensitivity() {
    let (db, dir) = db_and_dir().await;
    let root = dir.path().join("Palworld");
    install(&root);
    let target = target_row(&db, &root).await;
    // A hand-edited mods.txt in different casing, plus a stale marker file left
    // from before the user turned the mod off.
    write(&ue4ss_mods(&root).join("CoolMod/Scripts/main.lua"), b"a");
    write(&ue4ss_mods(&root).join("CoolMod/enabled.txt"), b"");
    write(&ue4ss_mods(&root).join("mods.txt"), b"coolmod : 0\n");

    let report = scan::scan(&db, &target).await.unwrap();
    let candidate = report
        .candidates
        .iter()
        .find(|c| c.name == "CoolMod")
        .unwrap();
    assert!(
        !candidate.enabled,
        "the reader must match mods.txt the way the writer does, or an apply \
         silently turns a disabled mod back on"
    );
}

#[tokio::test]
async fn two_locations_sharing_one_directory_are_walked_once() {
    let (db, dir) = db_and_dir().await;
    let root = dir.path().join("Palworld");
    install(&root);
    let shared = dir.path().join("shared-paks");
    std::fs::create_dir_all(&shared).unwrap();
    write(&shared.join("Cool_P.pak"), b"pak");

    // A docker target can mount one host directory for both ~mods and LogicMods.
    let target = ps_db::mod_targets::upsert(
        &db,
        &NewModTarget {
            id: "client-steam".to_string(),
            kind: "client".to_string(),
            name: "Steam".to_string(),
            root_path: root.to_string_lossy().into_owned(),
            platform: "win64".to_string(),
            ue4ss_mode: "standard".to_string(),
            layout_overrides: serde_json::json!({
                "paks_mods_dir": shared.to_string_lossy(),
                "logicmods_dir": shared.to_string_lossy(),
            })
            .to_string(),
            detected: "{}".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let report = scan::scan(&db, &target).await.unwrap();
    assert_eq!(
        report
            .files
            .iter()
            .filter(|f| f.path.ends_with("Cool_P.pak"))
            .count(),
        1,
        "one file, one report: {:?}",
        report.files
    );
    assert_eq!(
        report.candidates.len(),
        1,
        "and one candidate, not one per location: {:?}",
        report.candidates
    );
}

fn steam_package(content: &Path, workshop_id: &str, package: &str) -> std::path::PathBuf {
    let folder = content.join(workshop_id);
    write(
        &folder.join("Info.json"),
        format!(r#"{{"PackageName":"{package}"}}"#).as_bytes(),
    );
    write(&folder.join(format!("{package}_P.pak")), b"pak");
    folder
}

fn subscribed(report: &scan::ScanReport) -> Vec<&scan::AdoptionCandidate> {
    report
        .candidates
        .iter()
        .filter(|c| c.source == CandidateSource::SteamSubscribed)
        .collect()
}

#[tokio::test]
async fn a_subscribed_package_beside_the_game_in_its_steam_library_is_reported() {
    let (db, dir) = db_and_dir().await;
    let library = dir.path().join("SteamLibrary");
    let root = library.join("steamapps/common/Palworld");
    install(&root);
    let target = target_row(&db, &root).await;
    let content = library.join("steamapps/workshop/content/1623730");
    let on = steam_package(&content, "3300000001", "SteamPack");
    steam_package(&content, "3300000002", "QuietPack");
    write(
        &root.join("Mods/PalModSettings.ini"),
        b"[PalModSettings]\nbGlobalEnableMod=True\nActiveModList=SteamPack\n",
    );

    let report = scan::scan(&db, &target).await.unwrap();
    let found = subscribed(&report);
    assert_eq!(found.len(), 2, "{:?}", report.candidates);
    let pack = found
        .iter()
        .find(|c| c.name == "SteamPack")
        .expect("named by its PackageName, not by its item id");
    assert_eq!(pack.kind, RouteKind::Workshop);
    assert!(pack.enabled, "ActiveModList names it");
    assert_eq!(Path::new(&pack.root), on.as_path());
    assert!(pack.files.is_empty(), "Steam owns them: {:?}", pack.files);
    let quiet = found.iter().find(|c| c.name == "QuietPack").unwrap();
    assert!(!quiet.enabled, "no ActiveModList line");
    assert!(
        report
            .files
            .iter()
            .all(|f| !Path::new(&f.path).starts_with(&content)),
        "Steam's files are not the target's files: {:?}",
        report.files
    );
}

#[tokio::test]
async fn a_client_outside_a_steam_library_falls_back_to_workshop_root_dir() {
    let (db, dir) = db_and_dir().await;
    let root = dir.path().join("Games/Palworld");
    install(&root);
    let target = target_row(&db, &root).await;
    let content = dir.path().join("elsewhere/1623730");
    steam_package(&content, "3300000001", "SteamPack");

    let report = scan::scan(&db, &target).await.unwrap();
    assert!(
        subscribed(&report).is_empty(),
        "no library to derive it from and no WorkshopRootDir: {:?}",
        report.candidates
    );

    write(
        &root.join("Mods/PalModSettings.ini"),
        format!(
            "[PalModSettings]\nbGlobalEnableMod=True\nWorkshopRootDir={}\n",
            content.display()
        )
        .as_bytes(),
    );
    let report = scan::scan(&db, &target).await.unwrap();
    let found = subscribed(&report);
    assert_eq!(found.len(), 1, "{:?}", report.candidates);
    assert_eq!(found[0].name, "SteamPack");
    assert!(!found[0].enabled);
}

#[tokio::test]
async fn a_native_server_discovers_subscribed_packages_through_its_records_workshop_dir() {
    let (db, dir) = db_and_dir().await;
    let root = dir.path().join("PalServer");
    let content = dir.path().join("steamcmd/steamapps/workshop/content/1623730");
    steam_package(&content, "3300000001", "ServerPack");
    let dir_of = |relative: &str| root.join(relative).to_string_lossy().into_owned();
    let record = ps_db::servers::create_server(
        &db,
        ps_db::servers::NewServer {
            name: "Native".to_string(),
            container_name: "native".to_string(),
            server_type: "native".to_string(),
            install_path: root.to_string_lossy().into_owned(),
            workshop_dir: content.to_string_lossy().into_owned(),
            mods_path: dir_of("Pal/Binaries/Win64/Mods"),
            logicmods_path: dir_of("Pal/Content/Paks/LogicMods"),
            nativemods_path: dir_of("Pal/Binaries/Win64/NativeMods"),
            paks_path: dir_of("Pal/Content/Paks/~mods"),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let target_id = ps_db::mod_targets::ensure_server_target(&db, &record, "unused")
        .await
        .unwrap()
        .unwrap();
    let target = ps_db::mod_targets::get(&db, &target_id)
        .await
        .unwrap()
        .unwrap();
    write(
        &root.join("Mods/PalModSettings.ini"),
        format!(
            "[PalModSettings]\nbGlobalEnableMod=True\nWorkshopRootDir={}\nActiveModList=ServerPack\n",
            dir.path().join("stale").display()
        )
        .as_bytes(),
    );

    let report = scan::scan(&db, &target).await.unwrap();
    let found = subscribed(&report);
    assert_eq!(
        found.len(),
        1,
        "the record's workshop_dir wins over a stale WorkshopRootDir: {:?}",
        report.candidates
    );
    assert_eq!(found[0].name, "ServerPack");
    assert!(found[0].enabled);
}

#[tokio::test]
async fn a_package_both_local_and_subscribed_is_reported_once_as_local() {
    let (db, dir) = db_and_dir().await;
    let library = dir.path().join("SteamLibrary");
    let root = library.join("steamapps/common/Palworld");
    install(&root);
    let target = target_row(&db, &root).await;
    write(
        &root.join("Mods/Workshop/CoolPack-download/Info.json"),
        br#"{"PackageName":"CoolPack"}"#,
    );
    write(
        &root.join("Mods/Workshop/CoolPack-download/CoolPack_P.pak"),
        b"pak",
    );
    let content = library.join("steamapps/workshop/content/1623730");
    steam_package(&content, "3300000001", "CoolPack");
    steam_package(&content, "3300000002", "OtherPack");

    let report = scan::scan(&db, &target).await.unwrap();
    let cool: Vec<&scan::AdoptionCandidate> = report
        .candidates
        .iter()
        .filter(|c| c.name.eq_ignore_ascii_case("CoolPack"))
        .collect();
    assert_eq!(cool.len(), 1, "{:?}", report.candidates);
    assert_eq!(cool[0].source, CandidateSource::Local, "the app-owned copy wins");
    assert!(
        subscribed(&report).iter().any(|c| c.name == "OtherPack"),
        "{:?}",
        report.candidates
    );
}

#[tokio::test]
async fn a_workshop_root_dir_naming_the_local_workshop_directory_is_walked_once() {
    let (db, dir) = db_and_dir().await;
    let root = dir.path().join("Games/Palworld");
    install(&root);
    let target = target_row(&db, &root).await;
    write(
        &root.join("Mods/Workshop/LocalPack/Info.json"),
        br#"{"PackageName":"LocalPack"}"#,
    );
    let respelled = format!(
        "{}/Mods/Workshop/",
        root.to_string_lossy().replace('\\', "/")
    );
    write(
        &root.join("Mods/PalModSettings.ini"),
        format!("[PalModSettings]\nbGlobalEnableMod=True\nWorkshopRootDir={respelled}\n").as_bytes(),
    );

    let report = scan::scan(&db, &target).await.unwrap();
    assert_eq!(report.candidates.len(), 1, "{:?}", report.candidates);
    assert_eq!(report.candidates[0].source, CandidateSource::Local);
}

#[tokio::test]
async fn a_docker_server_reports_no_subscribed_packages() {
    let (db, dir) = db_and_dir().await;
    let content = dir.path().join("steam/1623730");
    steam_package(&content, "3300000001", "SteamPack");
    let servers = dir.path().join("servers");
    let host = |relative: &str| {
        servers
            .join("alpha")
            .join(relative)
            .to_string_lossy()
            .into_owned()
    };
    let record = ps_db::servers::create_server(
        &db,
        ps_db::servers::NewServer {
            name: "Alpha".to_string(),
            container_name: "alpha".to_string(),
            server_type: "docker".to_string(),
            workshop_dir: content.to_string_lossy().into_owned(),
            mods_path: host("mods"),
            logicmods_path: host("logicmods"),
            nativemods_path: host("nativemods"),
            paks_path: host("paks"),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let target_id =
        ps_db::mod_targets::ensure_server_target(&db, &record, &servers.to_string_lossy())
            .await
            .unwrap()
            .unwrap();
    let target = ps_db::mod_targets::get(&db, &target_id)
        .await
        .unwrap()
        .unwrap();
    write(
        &servers.join("alpha/Mods/PalModSettings.ini"),
        format!(
            "[PalModSettings]\nWorkshopRootDir={}\nActiveModList=SteamPack\n",
            content.display()
        )
        .as_bytes(),
    );

    let report = scan::scan(&db, &target).await.unwrap();
    assert!(
        subscribed(&report).is_empty(),
        "no Steam client runs in the container: {:?}",
        report.candidates
    );
}

#[cfg(any(windows, target_os = "macos"))]
#[tokio::test]
async fn a_recorded_file_respelled_only_in_case_is_managed_intact_not_a_candidate() {
    let (db, dir) = db_and_dir().await;
    let root = dir.path().join("Palworld");
    install(&root);
    let target = target_row(&db, &root).await;
    let file = ue4ss_mods(&root).join("CoolMod/Scripts/main.lua");
    write(&file, b"print('hi')");
    let respelled = ue4ss_mods(&root).join("COOLMOD/scripts/MAIN.lua");
    ps_db::mod_deployments::record(
        &db,
        &[ps_db::mod_deployments::NewDeploymentFile {
            target_id: "client-steam".to_string(),
            path: respelled.to_string_lossy().into_owned(),
            mod_version_id: None,
            hash: ps_server::services::mods::digest::hash_file(&file).unwrap(),
            role: "shared_marker".to_string(),
            rel_path: None,
        }],
    )
    .await
    .unwrap();

    let report = scan::scan(&db, &target).await.unwrap();
    assert!(
        report.candidates.is_empty(),
        "the app's own file is not offered for adoption: {:?}",
        report.candidates
    );
    assert_eq!(report.files.len(), 1, "one file, one report: {:?}", report.files);
    assert_eq!(report.files[0].state, FileState::ManagedIntact);
}

#[tokio::test]
async fn a_recorded_file_respelled_only_in_separators_is_managed_intact_on_every_os() {
    let (db, dir) = db_and_dir().await;
    let root = dir.path().join("Palworld");
    install(&root);
    let target = target_row(&db, &root).await;
    let file = ue4ss_mods(&root).join("CoolMod/Scripts/main.lua");
    write(&file, b"print('hi')");
    let respelled = format!(
        "{}//CoolMod/Scripts//main.lua",
        ue4ss_mods(&root).to_string_lossy().replace('\\', "/")
    );
    ps_db::mod_deployments::record(
        &db,
        &[ps_db::mod_deployments::NewDeploymentFile {
            target_id: "client-steam".to_string(),
            path: respelled,
            mod_version_id: None,
            hash: ps_server::services::mods::digest::hash_file(&file).unwrap(),
            role: "shared_marker".to_string(),
            rel_path: None,
        }],
    )
    .await
    .unwrap();

    let report = scan::scan(&db, &target).await.unwrap();
    assert!(report.candidates.is_empty(), "{:?}", report.candidates);
    assert_eq!(report.files.len(), 1, "one file, one report: {:?}", report.files);
    assert_eq!(report.files[0].state, FileState::ManagedIntact);
}

#[tokio::test]
async fn a_candidates_only_scan_hashes_no_recorded_row() {
    let (db, dir) = db_and_dir().await;
    let root = dir.path().join("Palworld");
    install(&root);
    let target = target_row(&db, &root).await;
    ps_db::mod_deployments::record(
        &db,
        &[ps_db::mod_deployments::NewDeploymentFile {
            target_id: "client-steam".to_string(),
            path: ue4ss_mods(&root)
                .join("GoneMod/main.lua")
                .to_string_lossy()
                .into_owned(),
            mod_version_id: None,
            hash: "whatever".to_string(),
            role: "shared_marker".to_string(),
            rel_path: None,
        }],
    )
    .await
    .unwrap();
    write(&ue4ss_mods(&root).join("HandMade/main.lua"), b"a");

    let report = scan::scan_with(&db, &target, true).await.unwrap();
    assert!(report.files.is_empty(), "{:?}", report.files);
    assert!(report.missing.is_empty(), "{:?}", report.missing);
    assert_eq!(report.candidates.len(), 1, "{:?}", report.candidates);
    assert_eq!(report.candidates[0].name, "HandMade");
}

#[tokio::test]
async fn an_undecodable_palmodsettings_withholds_subscribed_packages_but_not_local_candidates() {
    let (db, dir) = db_and_dir().await;
    let library = dir.path().join("SteamLibrary");
    let root = library.join("steamapps/common/Palworld");
    install(&root);
    let target = target_row(&db, &root).await;
    steam_package(
        &library.join("steamapps/workshop/content/1623730"),
        "3300000001",
        "SteamPack",
    );
    write(&ue4ss_mods(&root).join("HandMade/main.lua"), b"a");
    steam_package(&root.join("Mods/Workshop"), "LocalPack", "LocalPack");
    write(&root.join("Mods/PalModSettings.ini"), &[0xC3, 0x28, b'A']);

    let local_workshop = |report: &scan::ScanReport| {
        report
            .candidates
            .iter()
            .any(|c| c.kind == ps_core::mods::RouteKind::Workshop && c.source == CandidateSource::Local)
    };
    let report = scan::scan(&db, &target).await.unwrap();
    assert!(subscribed(&report).is_empty(), "{:?}", report.candidates);
    assert!(
        !local_workshop(&report),
        "a local package's enabled state is unknowable too: {:?}",
        report.candidates
    );
    assert!(
        report
            .candidates
            .iter()
            .any(|c| c.name == "HandMade" && c.source == CandidateSource::Local),
        "{:?}",
        report.candidates
    );
    assert_eq!(report.warnings, vec![scan::PALMODSETTINGS_UNREADABLE.to_string()]);

    write(&root.join("Mods/PalModSettings.ini"), b"[PalModSettings]\n");
    let readable = scan::scan(&db, &target).await.unwrap();
    assert!(local_workshop(&readable), "{:?}", readable.candidates);
}

#[tokio::test]
async fn only_numeric_item_folders_with_an_info_json_are_offered() {
    let (db, dir) = db_and_dir().await;
    let library = dir.path().join("SteamLibrary");
    let root = library.join("steamapps/common/Palworld");
    install(&root);
    let target = target_row(&db, &root).await;
    let content = library.join("steamapps/workshop/content/1623730");
    write(&content.join("3300000001/Paks/NoInfo_P.pak"), b"pak");
    steam_package(&content, "not-an-item", "NamedFolder");
    steam_package(&content, "3300000003", "RealPack");

    let report = scan::scan(&db, &target).await.unwrap();
    let names: Vec<&str> = subscribed(&report).iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["RealPack"], "{:?}", report.candidates);
}

#[tokio::test]
async fn a_linked_item_folder_is_not_followed() {
    let (db, dir) = db_and_dir().await;
    let library = dir.path().join("SteamLibrary");
    let root = library.join("steamapps/common/Palworld");
    install(&root);
    let target = target_row(&db, &root).await;
    let content = library.join("steamapps/workshop/content/1623730");
    std::fs::create_dir_all(&content).unwrap();
    let elsewhere = steam_package(&dir.path().join("elsewhere"), "3300000009", "LinkedPack");
    let link = content.join("3300000009");

    #[cfg(unix)]
    std::os::unix::fs::symlink(&elsewhere, &link).unwrap();
    #[cfg(windows)]
    {
        let made = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link.to_string_lossy().replace('/', "\\"))
            .arg(elsewhere.to_string_lossy().replace('/', "\\"))
            .output()
            .unwrap();
        if !made.status.success() {
            eprintln!("skipped: a junction could not be created here");
            return;
        }
    }
    assert!(link.join("Info.json").is_file(), "the link resolves");

    let report = scan::scan(&db, &target).await.unwrap();
    assert!(subscribed(&report).is_empty(), "{:?}", report.candidates);
}
