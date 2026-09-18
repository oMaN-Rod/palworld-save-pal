use std::path::Path;

use ps_core::mods::{FileRoute, InstallManifest, ModType, RouteKind, SourceHint};
use ps_db::mod_targets::{ModTarget, NewModTarget};
use ps_server::services::mods::{library, LibraryPaths};

pub async fn db_and_dir() -> (ps_db::SqlxSqliteDriver, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let pool = ps_db::open(&dir.path().join("test.db")).await.unwrap();
    (ps_db::SqlxSqliteDriver::new(pool), dir)
}

pub fn write(path: &Path, body: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

/// A client install tree with the directories the layout expects.
pub fn install_tree(root: &Path) {
    write(
        &root.join("Pal/Binaries/Win64/Palworld-Win64-Shipping.exe"),
        b"x",
    );
    std::fs::create_dir_all(root.join("Pal/Binaries/Win64/ue4ss/Mods")).unwrap();
    std::fs::create_dir_all(root.join("Pal/Content/Paks/~mods")).unwrap();
    std::fs::create_dir_all(root.join("Pal/Content/Paks/LogicMods")).unwrap();
}

pub async fn client_target(db: &ps_db::SqlxSqliteDriver, root: &Path) -> ModTarget {
    install_tree(root);
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

pub async fn default_profile(db: &ps_db::SqlxSqliteDriver, target_id: &str) -> String {
    let id = format!("{target_id}/default");
    ps_db::mod_profiles::create(
        db,
        &ps_db::mod_profiles::NewProfile {
            id: id.clone(),
            target_id: target_id.to_string(),
            name: "Default".to_string(),
            is_default: true,
        },
    )
    .await
    .unwrap();
    id
}

fn kind_of(segment: &str) -> RouteKind {
    match segment {
        "ue4ss" => RouteKind::Ue4ss,
        "palschema" => RouteKind::PalSchema,
        "pak" => RouteKind::Pak,
        "logicmods" => RouteKind::LogicMods,
        "nativedll" => RouteKind::NativeDll,
        "workshop" => RouteKind::Workshop,
        "passthrough" => RouteKind::Passthrough,
        other => panic!("unknown route kind {other}"),
    }
}

fn type_of(kind: RouteKind) -> ModType {
    match kind {
        RouteKind::Ue4ss => ModType::Ue4ss,
        RouteKind::PalSchema => ModType::PalSchema,
        RouteKind::Pak => ModType::Pak,
        RouteKind::LogicMods => ModType::LogicMods,
        RouteKind::NativeDll => ModType::NativeDll,
        RouteKind::Workshop => ModType::Workshop,
        _ => ModType::Pak,
    }
}

/// Puts a version in the library with the given routes, going through the real
/// `library::store` so the rows and the bytes are exactly what an install makes.
pub async fn install_fixture_mod(
    db: &ps_db::SqlxSqliteDriver,
    paths: &LibraryPaths,
    mod_id: &str,
    version: &str,
    routes: &[(&str, &str, &[u8])],
) -> library::Stored {
    install_typed(db, paths, mod_id, version, routes, "local").await
}

pub async fn install_framework(
    db: &ps_db::SqlxSqliteDriver,
    paths: &LibraryPaths,
    mod_id: &str,
    version: &str,
    routes: &[(&str, &str, &[u8])],
) -> library::Stored {
    install_typed(db, paths, mod_id, version, routes, "framework").await
}

async fn install_typed(
    db: &ps_db::SqlxSqliteDriver,
    paths: &LibraryPaths,
    mod_id: &str,
    version: &str,
    routes: &[(&str, &str, &[u8])],
    source_kind: &str,
) -> library::Stored {
    // A unique directory per call: two fixtures sharing a `mod_id`/`version` (as
    // several tests in this suite do) would otherwise race on the same fixed
    // path, and one test's cleanup could delete another's in-flight scratch tree.
    let scratch_dir = tempfile::tempdir().unwrap();
    let scratch = scratch_dir.path().to_path_buf();
    let mut manifest_routes = Vec::new();
    for (kind, rel_path, body) in routes {
        write(&scratch.join(rel_path), body);
        manifest_routes.push(FileRoute {
            archive_path: (*rel_path).to_string(),
            rel_path: (*rel_path).to_string(),
            kind: kind_of(kind),
        });
    }
    let first_kind = manifest_routes[0].kind;
    let folder = routes[0].1.split('/').next().unwrap().to_string();
    let manifest = InstallManifest {
        folder_name: folder.clone(),
        display_name: folder,
        mod_type: if source_kind == "framework" {
            ModType::Framework
        } else {
            type_of(first_kind)
        },
        version: version.to_string(),
        routes: manifest_routes,
        decisions: Vec::new(),
        platform_filtered: None,
        source: SourceHint::default(),
    };
    let stored = library::store(
        db,
        paths,
        &library::StoreRequest {
            mod_id,
            manifest: &manifest,
            extracted_root: &scratch,
            archive: None,
            source_kind,
            source_ref: "{}",
            custom_name: None,
        },
    )
    .await
    .unwrap();
    stored
}

pub async fn enable(
    db: &ps_db::SqlxSqliteDriver,
    target_id: &str,
    mod_id: &str,
    pinned: Option<&str>,
    enabled: bool,
) {
    ps_db::mod_profiles::set_mod(
        db,
        &ps_db::mod_profiles::ProfileModRow {
            profile_id: format!("{target_id}/default"),
            mod_id: mod_id.to_string(),
            mod_version_id: pinned.map(str::to_string),
            enabled,
            load_order: 0,
        },
    )
    .await
    .unwrap();
}

/// Every directory and file under `root`, with each file's bytes, for asserting
/// that a tree nothing may write to is unchanged.
pub fn tree_snapshot(root: &Path) -> Vec<(String, Option<Vec<u8>>)> {
    let mut entries: Vec<(String, Option<Vec<u8>>)> = walkdir::WalkDir::new(root)
        .into_iter()
        .map(|entry| entry.unwrap())
        .map(|entry| {
            let relative = entry
                .path()
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            let bytes = entry
                .file_type()
                .is_file()
                .then(|| std::fs::read(entry.path()).unwrap());
            (relative, bytes)
        })
        .collect();
    entries.sort();
    entries
}

/// A client target at `{library}/steamapps/common/Palworld` with an active
/// default profile, and Steam's Workshop content directory beside it holding
/// `SteamPack` (item 3300000001) and `SteamThing` (item 3300000002). The
/// settings file lists both, so both start enabled.
pub async fn steam_client(
    db: &ps_db::SqlxSqliteDriver,
    library: &Path,
) -> (ModTarget, std::path::PathBuf, std::path::PathBuf) {
    let root = library.join("steamapps/common/Palworld");
    let target = client_target(db, &root).await;
    default_profile(db, &target.id).await;
    let content = library.join("steamapps/workshop/content/1623730");
    write(
        &content.join("3300000001/Info.json"),
        br#"{"PackageName":"SteamPack","ModName":"Steam Pack","Author":"Someone"}"#,
    );
    write(&content.join("3300000001/Paks/SteamPack_P.pak"), b"steam bytes");
    write(
        &content.join("3300000002/Info.json"),
        br#"{"PackageName":"SteamThing"}"#,
    );
    write(
        &root.join("Mods/PalModSettings.ini"),
        b"[PalModSettings]\nbGlobalEnableMod=True\nActiveModList=SteamThing\nActiveModList=SteamPack\n",
    );
    (target, root, content)
}
