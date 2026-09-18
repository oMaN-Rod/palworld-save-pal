use std::path::Path;

use ps_core::mods::{Platform, Ue4ssMode};
use ps_server::services::mods::detect::{self, HAZARD_UE4SS_DUAL_INSTANCE};

fn touch(path: &Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, b"x").unwrap();
}

/// A minimal but plausible Palworld tree.
fn install(root: &Path, platform: &str) {
    let binaries = root.join("Pal/Binaries").join(platform);
    touch(&binaries.join("Palworld-Win64-Shipping.exe"));
    std::fs::create_dir_all(root.join("Pal/Content/Paks")).unwrap();
}

#[test]
fn library_folders_vdf_yields_every_library_path() {
    let vdf = r#"
"libraryfolders"
{
    "0"
    {
        "path"        "C:\\Program Files (x86)\\Steam"
        "label"        ""
        "apps"
        {
            "1623730"        "12345"
        }
    }
    "1"
    {
        "path"        "D:\\SteamLibrary"
        "apps"
        {
        }
    }
}
"#;
    let paths = detect::steam_library_paths(vdf);
    assert_eq!(
        paths,
        vec![
            r"C:\Program Files (x86)\Steam".to_string(),
            r"D:\SteamLibrary".to_string(),
        ],
        "both libraries, with the escaped backslashes unescaped"
    );
}

#[test]
fn a_vdf_with_no_libraries_yields_nothing_rather_than_failing() {
    assert!(detect::steam_library_paths("").is_empty());
    assert!(detect::steam_library_paths("not a vdf at all {{{").is_empty());
    assert!(detect::steam_library_paths(r#""libraryfolders" { }"#).is_empty());
}

#[test]
fn a_reg_query_listing_yields_the_named_string_value() {
    let user = "\r\nHKEY_CURRENT_USER\\Software\\Valve\\Steam\r\n    SteamPath    REG_SZ    d:/programs/steam\r\n\r\n";
    let machine = "\r\nHKEY_LOCAL_MACHINE\\SOFTWARE\\WOW6432Node\\Valve\\Steam\r\n    InstallPath    REG_SZ    C:\\Program Files (x86)\\Steam\r\n";

    assert_eq!(
        detect::registry_string_value(user, "SteamPath").as_deref(),
        Some("d:/programs/steam")
    );
    assert_eq!(
        detect::registry_string_value(machine, "InstallPath").as_deref(),
        Some(r"C:\Program Files (x86)\Steam"),
        "a value containing spaces is kept whole"
    );
    assert_eq!(detect::registry_string_value(user, "InstallPath"), None);
    assert_eq!(detect::registry_string_value("", "SteamPath"), None);
}

#[test]
fn an_install_is_found_under_a_steam_root_outside_program_files() {
    let dir = tempfile::tempdir().unwrap();
    let steam = dir.path().join("Programs").join("Steam");
    let vdf = format!(
        "\"libraryfolders\"\n{{\n    \"0\"\n    {{\n        \"path\"        \"{}\"\n    }}\n}}\n",
        steam.to_string_lossy().replace('\\', "\\\\")
    );
    touch(&steam.join("steamapps/libraryfolders.vdf"));
    std::fs::write(steam.join("steamapps/libraryfolders.vdf"), vdf).unwrap();
    let game = steam.join("steamapps/common/Palworld");
    install(&game, "Win64");

    let found = detect::find_installs_in(&[steam.clone(), steam]);

    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(Path::new(&found[0].root), game);
}

#[test]
fn an_install_is_recognised_by_its_paks_dir_and_an_executable() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Palworld");
    assert!(!detect::looks_like_install(&root), "nothing there yet");
    std::fs::create_dir_all(root.join("Pal/Content/Paks")).unwrap();
    assert!(
        !detect::looks_like_install(&root),
        "a paks dir alone is not an install"
    );
    touch(&root.join("Pal/Binaries/Win64/Palworld-Win64-Shipping.exe"));
    assert!(detect::looks_like_install(&root));
}

#[test]
fn a_folder_picked_inside_the_install_is_walked_back_up_to_the_root() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Palworld");
    install(&root, "Win64");
    for picked in [
        root.clone(),
        root.join("Pal"),
        root.join("Pal/Content"),
        root.join("Pal/Content/Paks"),
        root.join("Pal/Binaries/Win64"),
    ] {
        assert_eq!(
            detect::normalise_root(&picked).as_deref(),
            Some(root.as_path()),
            "picking {picked:?} should resolve to the install root"
        );
    }
}

#[test]
fn a_folder_that_is_not_an_install_normalises_to_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = dir.path().join("Documents/NotPalworld");
    std::fs::create_dir_all(&elsewhere).unwrap();
    assert_eq!(detect::normalise_root(&elsewhere), None);
}

#[test]
fn the_platform_comes_from_which_binaries_directory_exists() {
    let dir = tempfile::tempdir().unwrap();
    let steam = dir.path().join("steam");
    install(&steam, "Win64");
    assert_eq!(detect::platform_of_install(&steam), Some(Platform::Win64));

    let gamepass = dir.path().join("gamepass");
    install(&gamepass, "WinGDK");
    assert_eq!(
        detect::platform_of_install(&gamepass),
        Some(Platform::WinGdk)
    );

    let neither = dir.path().join("neither");
    std::fs::create_dir_all(&neither).unwrap();
    assert_eq!(detect::platform_of_install(&neither), None);
}

#[test]
fn standard_ue4ss_is_detected_from_either_of_its_two_markers() {
    for marker in ["dwmapi.dll", "ue4ss/UE4SS.dll"] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Palworld");
        install(&root, "Win64");
        touch(&root.join("Pal/Binaries/Win64").join(marker));
        let (mode, hazards) = detect::detect_ue4ss(&root, Platform::Win64);
        assert_eq!(mode, Ue4ssMode::Standard, "{marker}");
        assert!(hazards.is_empty(), "{marker}: {hazards:?}");
    }
}

#[test]
fn workshop_ue4ss_is_detected_from_the_nativemods_directory() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Palworld");
    install(&root, "Win64");
    touch(&root.join("Mods/NativeMods/UE4SS/UE4SS.dll"));
    let (mode, hazards) = detect::detect_ue4ss(&root, Platform::Win64);
    assert_eq!(mode, Ue4ssMode::Workshop);
    assert!(hazards.is_empty(), "{hazards:?}");
}

#[test]
fn no_ue4ss_at_all_is_mode_none() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Palworld");
    install(&root, "Win64");
    let (mode, hazards) = detect::detect_ue4ss(&root, Platform::Win64);
    assert_eq!(mode, Ue4ssMode::None);
    assert!(hazards.is_empty());
}

#[test]
fn both_installs_present_is_the_dual_instance_hazard() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Palworld");
    install(&root, "Win64");
    touch(&root.join("Pal/Binaries/Win64/dwmapi.dll"));
    touch(&root.join("Mods/NativeMods/UE4SS/UE4SS.dll"));

    let (mode, hazards) = detect::detect_ue4ss(&root, Platform::Win64);
    assert_eq!(
        mode,
        Ue4ssMode::Standard,
        "standard wins when PalModSettings does not say otherwise"
    );
    assert!(
        hazards.contains(&HAZARD_UE4SS_DUAL_INSTANCE.to_string()),
        "running both crashes the game, so this must be reported: {hazards:?}"
    );
}

#[test]
fn palmodsettings_naming_ue4ss_flips_the_winner_but_keeps_the_hazard() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Palworld");
    install(&root, "Win64");
    touch(&root.join("Pal/Binaries/Win64/dwmapi.dll"));
    touch(&root.join("Mods/NativeMods/UE4SS/UE4SS.dll"));
    std::fs::write(
        root.join("Mods/PalModSettings.ini"),
        "[PalModSettings]\nbGlobalEnableMod=True\nActiveModList=UE4SS\n",
    )
    .unwrap();

    let (mode, hazards) = detect::detect_ue4ss(&root, Platform::Win64);
    assert_eq!(mode, Ue4ssMode::Workshop);
    assert!(
        hazards.contains(&HAZARD_UE4SS_DUAL_INSTANCE.to_string()),
        "{hazards:?}"
    );
}

#[test]
fn palmodsettings_with_modding_off_does_not_flip_the_winner() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Palworld");
    install(&root, "Win64");
    touch(&root.join("Pal/Binaries/Win64/dwmapi.dll"));
    touch(&root.join("Mods/NativeMods/UE4SS/UE4SS.dll"));
    std::fs::write(
        root.join("Mods/PalModSettings.ini"),
        "[PalModSettings]\nbGlobalEnableMod=False\nActiveModList=UE4SS\n",
    )
    .unwrap();

    let (mode, _) = detect::detect_ue4ss(&root, Platform::Win64);
    assert_eq!(
        mode,
        Ue4ssMode::Standard,
        "the flag and the list are both required to flip it"
    );
}

#[test]
fn describe_reports_a_whole_target_or_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Palworld");
    install(&root, "Win64");
    touch(&root.join("Pal/Binaries/Win64/dwmapi.dll"));

    let described = detect::describe(&root, "steam").unwrap();
    assert_eq!(described.root, root.to_string_lossy());
    assert_eq!(described.platform, "win64");
    assert_eq!(described.ue4ss_mode, "standard");
    assert_eq!(described.source, "steam");
    assert!(described.hazards.is_empty());

    let empty = dir.path().join("nothing");
    std::fs::create_dir_all(&empty).unwrap();
    assert!(detect::describe(&empty, "steam").is_none());
}

async fn db_in(dir: &Path) -> ps_db::SqlxSqliteDriver {
    let pool = ps_db::open(&dir.join("test.db")).await.unwrap();
    ps_db::SqlxSqliteDriver::new(pool)
}

#[tokio::test]
async fn registering_an_install_creates_a_target_and_a_default_profile() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_in(dir.path()).await;
    let root = dir.path().join("Palworld");
    install(&root, "Win64");
    touch(&root.join("Pal/Binaries/Win64/dwmapi.dll"));
    let described = detect::describe(&root, "steam").unwrap();

    let target = detect::register(&db, &described, "Steam").await.unwrap();
    assert_eq!(target.id, "client-palworld");
    assert_eq!(target.kind, "client");
    assert_eq!(target.platform, "win64");
    assert_eq!(target.ue4ss_mode, "standard");
    assert_eq!(target.root_path, root.to_string_lossy());
    assert_eq!(target.name, "Steam");

    let profiles = ps_db::mod_profiles::for_target(&db, &target.id)
        .await
        .unwrap();
    assert_eq!(profiles.len(), 1);
    assert!(profiles[0].is_default);
    assert!(profiles[0].is_active);
}

#[tokio::test]
async fn registering_the_same_install_twice_does_not_create_a_second_target() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_in(dir.path()).await;
    let root = dir.path().join("Palworld");
    install(&root, "Win64");
    let described = detect::describe(&root, "steam").unwrap();

    detect::register(&db, &described, "Steam").await.unwrap();
    detect::register(&db, &described, "Steam (again)")
        .await
        .unwrap();

    assert_eq!(ps_db::mod_targets::list(&db).await.unwrap().len(), 1);
    let profiles = ps_db::mod_profiles::for_target(&db, "client-palworld")
        .await
        .unwrap();
    assert_eq!(profiles.len(), 1, "and no second default profile");
}

#[tokio::test]
async fn registering_the_same_install_respelled_does_not_create_a_second_target() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_in(dir.path()).await;
    let root = dir.path().join("Palworld");
    install(&root, "Win64");
    let described = detect::describe(&root, "steam").unwrap();
    let mut respelled = described.clone();
    respelled.root = described.root.replace('\\', "/");
    if cfg!(any(windows, target_os = "macos")) {
        respelled.root = respelled.root.to_uppercase();
    }

    detect::register(&db, &described, "Steam").await.unwrap();
    let again = detect::register(&db, &respelled, "Picked").await.unwrap();

    assert_eq!(again.id, "client-palworld");
    assert_eq!(ps_db::mod_targets::list(&db).await.unwrap().len(), 1);
}

#[tokio::test]
async fn refresh_updates_the_mode_and_the_hazards_but_not_the_users_paths() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_in(dir.path()).await;
    let root = dir.path().join("Palworld");
    install(&root, "Win64");
    let described = detect::describe(&root, "steam").unwrap();
    let target = detect::register(&db, &described, "Steam").await.unwrap();
    assert_eq!(target.ue4ss_mode, "none");

    // The user edits an override, then installs UE4SS both ways.
    ps_db::mod_targets::set_layout_overrides(&db, &target.id, r#"{"ue4ss_mods_dir":"D:/Mine"}"#)
        .await
        .unwrap();
    touch(&root.join("Pal/Binaries/Win64/dwmapi.dll"));
    touch(&root.join("Mods/NativeMods/UE4SS/UE4SS.dll"));

    let refreshed = detect::refresh(&db, &target).await.unwrap();
    assert_eq!(refreshed.ue4ss_mode, "standard");
    assert!(
        refreshed.detected.contains(HAZARD_UE4SS_DUAL_INSTANCE),
        "{}",
        refreshed.detected
    );
    assert_eq!(
        refreshed.layout_overrides, r#"{"ue4ss_mods_dir":"D:/Mine"}"#,
        "refresh must not touch the user's overrides"
    );
    assert_eq!(refreshed.root_path, root.to_string_lossy());
}

#[tokio::test]
async fn refreshing_a_target_whose_install_is_gone_reports_rather_than_wiping_the_row() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_in(dir.path()).await;
    let root = dir.path().join("Palworld");
    install(&root, "Win64");
    let described = detect::describe(&root, "steam").unwrap();
    let target = detect::register(&db, &described, "Steam").await.unwrap();

    std::fs::remove_dir_all(&root).unwrap();
    let result = detect::refresh(&db, &target).await;
    assert!(
        result.is_err(),
        "a vanished install is an error, not a silent reset"
    );
    let still_there = ps_db::mod_targets::get(&db, &target.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(still_there.root_path, root.to_string_lossy());
}

#[tokio::test]
async fn two_installs_with_the_same_folder_name_get_separate_targets() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_in(dir.path()).await;
    let install_under = |drive: &str| {
        dir.path()
            .join(drive)
            .join("steamapps")
            .join("common")
            .join("Palworld")
    };
    let first = install_under("C");
    let second = install_under("D");
    install(&first, "Win64");
    install(&second, "Win64");

    let a = detect::register(&db, &detect::describe(&first, "steam").unwrap(), "C drive")
        .await
        .unwrap();
    let b = detect::register(&db, &detect::describe(&second, "steam").unwrap(), "D drive")
        .await
        .unwrap();

    assert_ne!(
        a.id, b.id,
        "two installs sharing a folder name must not share one target"
    );
    assert_eq!(a.root_path, first.to_string_lossy());
    let a_again = ps_db::mod_targets::get(&db, &a.id).await.unwrap().unwrap();
    assert_eq!(
        a_again.root_path,
        first.to_string_lossy(),
        "and registering the second must not repoint the first"
    );
    assert_eq!(ps_db::mod_targets::list(&db).await.unwrap().len(), 2);
}

#[tokio::test]
async fn re_registering_an_install_keeps_the_users_overrides() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_in(dir.path()).await;
    let root = dir.path().join("Palworld");
    install(&root, "Win64");
    let described = detect::describe(&root, "steam").unwrap();
    let target = detect::register(&db, &described, "Steam").await.unwrap();

    ps_db::mod_targets::set_layout_overrides(&db, &target.id, r#"{"ue4ss_mods_dir":"D:/Mine"}"#)
        .await
        .unwrap();

    // The user re-runs "detect installs" and registers the same install again.
    let again = detect::register(&db, &described, "Steam").await.unwrap();
    assert_eq!(again.id, target.id);
    assert_eq!(
        again.layout_overrides, r#"{"ue4ss_mods_dir":"D:/Mine"}"#,
        "re-registering must not reset a path the user set"
    );
}

#[test]
fn a_relative_pick_is_refused_rather_than_resolved_against_the_working_directory() {
    assert_eq!(
        detect::normalise_root(Path::new("Pal/Binaries/Win64")),
        None
    );
    assert_eq!(detect::normalise_root(Path::new("Palworld")), None);
}

fn steam_with_vdf(steam: &Path, libraries: &[&Path]) {
    let entries: String = libraries
        .iter()
        .enumerate()
        .map(|(i, library)| {
            format!(
                "    \"{i}\"\n    {{\n        \"path\"        \"{}\"\n    }}\n",
                library.to_string_lossy().replace('\\', "\\\\")
            )
        })
        .collect();
    let vdf_path = steam.join("steamapps").join("libraryfolders.vdf");
    touch(&vdf_path);
    std::fs::write(vdf_path, format!("\"libraryfolders\"\n{{\n{entries}}}\n")).unwrap();
}

#[test]
fn a_found_install_root_is_joined_component_by_component() {
    let dir = tempfile::tempdir().unwrap();
    let steam = dir.path().join("Programs").join("Steam");
    steam_with_vdf(&steam, &[&steam]);
    let game = steam.join("steamapps").join("common").join("Palworld");
    install(&game, "Win64");

    let found = detect::find_installs_in(&[steam]);

    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].root, game.to_string_lossy());
    #[cfg(windows)]
    assert!(!found[0].root.contains('/'), "{}", found[0].root);
}

#[cfg(windows)]
#[test]
fn a_root_picked_with_forward_slashes_is_described_with_backslashes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Palworld");
    install(&root, "Win64");
    let picked = root.to_string_lossy().replace('\\', "/");

    let described = detect::describe(Path::new(&picked), "manual").unwrap();

    assert_eq!(described.root, root.to_string_lossy());
}

#[test]
fn steam_library_dirs_lists_each_library_once() {
    let dir = tempfile::tempdir().unwrap();
    let steam = dir.path().join("Programs").join("Steam");
    let library = dir.path().join("Games").join("SteamLibrary");
    steam_with_vdf(&steam, &[&steam, &library]);

    let libraries = detect::steam_library_dirs_in(&[steam.clone(), steam.clone()]);

    assert_eq!(libraries, vec![steam, library]);
}

#[tokio::test]
async fn registering_an_install_stored_with_mixed_separators_heals_the_row() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_in(dir.path()).await;
    let root = dir
        .path()
        .join("Steam")
        .join("steamapps")
        .join("common")
        .join("Palworld");
    install(&root, "Win64");
    let native = root.to_string_lossy().into_owned();
    let mut mixed = detect::describe(&root, "steam").unwrap();
    mixed.root = format!(
        "{}/steamapps/common/Palworld",
        dir.path().join("Steam").to_string_lossy()
    );
    let first = detect::register(&db, &mixed, "Steam").await.unwrap();
    ps_db::mod_targets::set_layout_overrides(&db, &first.id, r#"{"ue4ss_mods_dir":"D:/Mine"}"#)
        .await
        .unwrap();

    let healed = detect::register(&db, &mixed, "Steam").await.unwrap();

    assert_eq!(healed.id, first.id);
    assert_eq!(ps_db::mod_targets::list(&db).await.unwrap().len(), 1);
    assert_eq!(healed.root_path, native);
    assert_eq!(healed.layout_overrides, r#"{"ue4ss_mods_dir":"D:/Mine"}"#);
}
