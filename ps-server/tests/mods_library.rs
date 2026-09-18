use std::path::Path;

use ps_core::mods::{FileRoute, InstallManifest, ModType, RouteKind, SourceHint};
use ps_server::services::mods::{library, LibraryPaths};

async fn db_and_root() -> (ps_db::SqlxSqliteDriver, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let pool = ps_db::open(&dir.path().join("test.db")).await.unwrap();
    (ps_db::SqlxSqliteDriver::new(pool), dir)
}

fn manifest(version: &str, routes: Vec<FileRoute>) -> InstallManifest {
    InstallManifest {
        folder_name: "CoolMod".to_string(),
        display_name: "Cool Mod".to_string(),
        mod_type: ModType::Ue4ss,
        version: version.to_string(),
        routes,
        decisions: Vec::new(),
        platform_filtered: None,
        source: SourceHint::default(),
    }
}

fn route(archive_path: &str, rel_path: &str, kind: RouteKind) -> FileRoute {
    FileRoute {
        archive_path: archive_path.to_string(),
        rel_path: rel_path.to_string(),
        kind,
    }
}

/// An extracted tree holding the files the routes name.
fn extracted(root: &Path, files: &[(&str, &[u8])]) {
    for (path, body) in files {
        let full = root.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, body).unwrap();
    }
}

fn request<'a>(
    manifest: &'a InstallManifest,
    extracted_root: &'a Path,
    archive: Option<&'a Path>,
) -> library::StoreRequest<'a> {
    library::StoreRequest {
        mod_id: "coolmod-ue4ss",
        manifest,
        extracted_root,
        archive,
        source_kind: "local",
        source_ref: "{}",
        custom_name: None,
    }
}

#[tokio::test]
async fn storing_a_version_writes_the_files_the_archive_and_the_rows() {
    let (db, dir) = db_and_root().await;
    let paths = LibraryPaths::new(dir.path());
    let source = dir.path().join("extracted");
    extracted(
        &source,
        &[
            ("CoolMod/Scripts/main.lua", b"print('hi')"),
            ("CoolMod/CoolMod_P.pak", b"pakbytes"),
        ],
    );
    let archive = dir.path().join("CoolMod-1.0.zip");
    std::fs::write(&archive, b"zipbytes").unwrap();

    let m = manifest(
        "1.0",
        vec![
            route(
                "CoolMod/Scripts/main.lua",
                "CoolMod/Scripts/main.lua",
                RouteKind::Ue4ss,
            ),
            route("CoolMod/CoolMod_P.pak", "CoolMod_P.pak", RouteKind::Pak),
        ],
    );
    let stored = library::store(&db, &paths, &request(&m, &source, Some(&archive)))
        .await
        .unwrap();

    assert_eq!(stored.version.id, "coolmod-ue4ss@1.0");
    assert_eq!(stored.mod_row.name, "Cool Mod");
    assert_eq!(stored.mod_row.mod_type, "ue4ss");
    assert_eq!(stored.files.len(), 2);

    let lua = paths.route_path(
        "coolmod-ue4ss",
        "1.0",
        RouteKind::Ue4ss,
        "CoolMod/Scripts/main.lua",
    );
    assert_eq!(std::fs::read_to_string(&lua).unwrap(), "print('hi')");
    let pak = paths.route_path("coolmod-ue4ss", "1.0", RouteKind::Pak, "CoolMod_P.pak");
    assert_eq!(std::fs::read(&pak).unwrap(), b"pakbytes");

    let kept = paths.archives_dir("coolmod-ue4ss").join("CoolMod-1.0.zip");
    assert_eq!(std::fs::read(&kept).unwrap(), b"zipbytes");
    assert_eq!(
        stored.version.archive_path.as_deref(),
        Some(kept.to_string_lossy().as_ref())
    );

    assert_eq!(
        stored.version.library_dir,
        paths.version_dir("coolmod-ue4ss", "1.0").to_string_lossy()
    );
    let round_tripped: InstallManifest = serde_json::from_str(&stored.version.manifest).unwrap();
    assert_eq!(round_tripped, m, "the manifest is stored verbatim");
    assert!(stored.version.is_current, "the first version is current");
}

#[tokio::test]
async fn the_same_version_twice_is_refused() {
    let (db, dir) = db_and_root().await;
    let paths = LibraryPaths::new(dir.path());
    let source = dir.path().join("extracted");
    extracted(&source, &[("CoolMod/main.lua", b"a")]);
    let m = manifest(
        "1.0",
        vec![route(
            "CoolMod/main.lua",
            "CoolMod/main.lua",
            RouteKind::Ue4ss,
        )],
    );

    library::store(&db, &paths, &request(&m, &source, None))
        .await
        .unwrap();
    let again = library::store(&db, &paths, &request(&m, &source, None)).await;
    assert!(matches!(
        again,
        Err(library::LibraryError::AlreadyInstalled(_))
    ));
}

#[tokio::test]
async fn a_route_with_no_source_file_fails_and_leaves_nothing_behind() {
    let (db, dir) = db_and_root().await;
    let paths = LibraryPaths::new(dir.path());
    let source = dir.path().join("extracted");
    extracted(&source, &[("CoolMod/main.lua", b"a")]);
    let m = manifest(
        "1.0",
        vec![
            route("CoolMod/main.lua", "CoolMod/main.lua", RouteKind::Ue4ss),
            route("CoolMod/absent.lua", "CoolMod/absent.lua", RouteKind::Ue4ss),
        ],
    );

    let failed = library::store(&db, &paths, &request(&m, &source, None)).await;
    assert!(matches!(
        failed,
        Err(library::LibraryError::MissingRouteFile(_))
    ));
    assert!(
        !paths.version_dir("coolmod-ue4ss", "1.0").exists(),
        "a failed store must leave no library bytes"
    );
    assert!(
        ps_db::mod_library::get_version(&db, "coolmod-ue4ss@1.0")
            .await
            .unwrap()
            .is_none(),
        "and no row"
    );
}

#[tokio::test]
async fn a_second_version_lands_beside_the_first_and_is_not_current() {
    let (db, dir) = db_and_root().await;
    let paths = LibraryPaths::new(dir.path());
    let source = dir.path().join("extracted");
    extracted(&source, &[("CoolMod/main.lua", b"v1")]);
    let one = manifest(
        "1.0",
        vec![route(
            "CoolMod/main.lua",
            "CoolMod/main.lua",
            RouteKind::Ue4ss,
        )],
    );
    library::store(&db, &paths, &request(&one, &source, None))
        .await
        .unwrap();

    let source2 = dir.path().join("extracted2");
    extracted(&source2, &[("CoolMod/main.lua", b"v2")]);
    let two = manifest(
        "2.0",
        vec![route(
            "CoolMod/main.lua",
            "CoolMod/main.lua",
            RouteKind::Ue4ss,
        )],
    );
    let second = library::store(&db, &paths, &request(&two, &source2, None))
        .await
        .unwrap();

    assert!(!second.version.is_current, "promotion is explicit");
    assert_eq!(
        std::fs::read_to_string(paths.route_path(
            "coolmod-ue4ss",
            "1.0",
            RouteKind::Ue4ss,
            "CoolMod/main.lua"
        ))
        .unwrap(),
        "v1",
        "the first version's bytes are untouched"
    );
    assert_eq!(
        std::fs::read_to_string(paths.route_path(
            "coolmod-ue4ss",
            "2.0",
            RouteKind::Ue4ss,
            "CoolMod/main.lua"
        ))
        .unwrap(),
        "v2"
    );
    assert_eq!(
        ps_db::mod_library::versions_of(&db, "coolmod-ue4ss")
            .await
            .unwrap()
            .len(),
        2
    );
}

#[tokio::test]
async fn the_hashes_returned_are_the_hashes_of_the_stored_bytes() {
    let (db, dir) = db_and_root().await;
    let paths = LibraryPaths::new(dir.path());
    let source = dir.path().join("extracted");
    extracted(&source, &[("CoolMod/main.lua", b"print('hi')")]);
    let m = manifest(
        "1.0",
        vec![route(
            "CoolMod/main.lua",
            "CoolMod/main.lua",
            RouteKind::Ue4ss,
        )],
    );
    let stored = library::store(&db, &paths, &request(&m, &source, None))
        .await
        .unwrap();

    let file = &stored.files[0];
    assert_eq!(file.size, 11);
    assert_eq!(file.rel_path, "CoolMod/main.lua");
    assert_eq!(file.kind, RouteKind::Ue4ss);
    let on_disk = paths.route_path("coolmod-ue4ss", "1.0", RouteKind::Ue4ss, "CoolMod/main.lua");
    assert_eq!(
        file.hash,
        ps_server::services::mods::digest::hash_file(&on_disk).unwrap()
    );
}

#[tokio::test]
async fn a_custom_name_overrides_the_display_name() {
    let (db, dir) = db_and_root().await;
    let paths = LibraryPaths::new(dir.path());
    let source = dir.path().join("extracted");
    extracted(&source, &[("CoolMod/main.lua", b"a")]);
    let m = manifest(
        "1.0",
        vec![route(
            "CoolMod/main.lua",
            "CoolMod/main.lua",
            RouteKind::Ue4ss,
        )],
    );
    let mut req = request(&m, &source, None);
    req.custom_name = Some("My Renamed Mod");
    let stored = library::store(&db, &paths, &req).await.unwrap();
    assert_eq!(
        stored.mod_row.name, "Cool Mod",
        "the archive's name is kept"
    );
    assert_eq!(
        stored.mod_row.custom_name.as_deref(),
        Some("My Renamed Mod"),
        "and the user's name sits beside it"
    );
}

#[tokio::test]
async fn version_size_sums_the_stored_files() {
    let (db, dir) = db_and_root().await;
    let paths = LibraryPaths::new(dir.path());
    let source = dir.path().join("extracted");
    extracted(
        &source,
        &[("CoolMod/a.lua", b"12345"), ("CoolMod/b.lua", b"123")],
    );
    let m = manifest(
        "1.0",
        vec![
            route("CoolMod/a.lua", "CoolMod/a.lua", RouteKind::Ue4ss),
            route("CoolMod/b.lua", "CoolMod/b.lua", RouteKind::Ue4ss),
        ],
    );
    let stored = library::store(&db, &paths, &request(&m, &source, None))
        .await
        .unwrap();
    assert_eq!(library::version_size(&stored.version).unwrap(), 8);
}

#[tokio::test]
async fn deleting_a_version_removes_its_directory_but_keeps_the_archive() {
    let (db, dir) = db_and_root().await;
    let paths = LibraryPaths::new(dir.path());
    let source = dir.path().join("extracted");
    extracted(&source, &[("CoolMod/main.lua", b"v1")]);
    let archive = dir.path().join("CoolMod-1.0.zip");
    std::fs::write(&archive, b"zipbytes").unwrap();
    let one = manifest(
        "1.0",
        vec![route(
            "CoolMod/main.lua",
            "CoolMod/main.lua",
            RouteKind::Ue4ss,
        )],
    );
    library::store(&db, &paths, &request(&one, &source, Some(&archive)))
        .await
        .unwrap();

    let source2 = dir.path().join("extracted2");
    extracted(&source2, &[("CoolMod/main.lua", b"v2")]);
    let two = manifest(
        "2.0",
        vec![route(
            "CoolMod/main.lua",
            "CoolMod/main.lua",
            RouteKind::Ue4ss,
        )],
    );
    library::store(&db, &paths, &request(&two, &source2, None))
        .await
        .unwrap();

    // 1.0 is current, so it is refused; 2.0 is not.
    assert!(library::delete_version(&db, "coolmod-ue4ss@1.0")
        .await
        .is_err());
    assert!(paths.version_dir("coolmod-ue4ss", "1.0").exists());

    library::delete_version(&db, "coolmod-ue4ss@2.0")
        .await
        .unwrap();
    assert!(!paths.version_dir("coolmod-ue4ss", "2.0").exists());
    assert!(paths.version_dir("coolmod-ue4ss", "1.0").exists());
    assert!(
        paths
            .archives_dir("coolmod-ue4ss")
            .join("CoolMod-1.0.zip")
            .exists(),
        "the archive outlives a version deletion"
    );
    assert_eq!(
        ps_db::mod_library::versions_of(&db, "coolmod-ue4ss")
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn a_refused_deletion_touches_no_bytes() {
    let (db, dir) = db_and_root().await;
    let paths = LibraryPaths::new(dir.path());
    let source = dir.path().join("extracted");
    extracted(&source, &[("CoolMod/main.lua", b"v1")]);
    let m = manifest(
        "1.0",
        vec![route(
            "CoolMod/main.lua",
            "CoolMod/main.lua",
            RouteKind::Ue4ss,
        )],
    );
    library::store(&db, &paths, &request(&m, &source, None))
        .await
        .unwrap();
    let file = paths.route_path("coolmod-ue4ss", "1.0", RouteKind::Ue4ss, "CoolMod/main.lua");

    assert!(library::delete_version(&db, "coolmod-ue4ss@1.0")
        .await
        .is_err());
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "v1",
        "the refusal must come before any byte is removed"
    );
}

#[tokio::test]
async fn deleting_a_mod_removes_every_version_and_the_archives() {
    let (db, dir) = db_and_root().await;
    let paths = LibraryPaths::new(dir.path());
    let source = dir.path().join("extracted");
    extracted(&source, &[("CoolMod/main.lua", b"v1")]);
    let archive = dir.path().join("CoolMod-1.0.zip");
    std::fs::write(&archive, b"zipbytes").unwrap();
    let m = manifest(
        "1.0",
        vec![route(
            "CoolMod/main.lua",
            "CoolMod/main.lua",
            RouteKind::Ue4ss,
        )],
    );
    library::store(&db, &paths, &request(&m, &source, Some(&archive)))
        .await
        .unwrap();

    library::delete_mod(&db, &paths, "coolmod-ue4ss")
        .await
        .unwrap();
    assert!(!paths.mod_dir("coolmod-ue4ss").exists());
    assert!(ps_db::mod_library::get_mod(&db, "coolmod-ue4ss")
        .await
        .unwrap()
        .is_none());
    assert!(ps_db::mod_library::versions_of(&db, "coolmod-ue4ss")
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn deleting_a_version_that_is_not_in_the_library_is_an_error_not_a_silent_success() {
    let (db, dir) = db_and_root().await;
    let paths = LibraryPaths::new(dir.path());
    assert!(library::delete_version(&db, "ghost@1.0").await.is_err());
    assert!(library::delete_mod(&db, &paths, "ghost").await.is_err());
}

#[tokio::test]
async fn two_versions_that_slug_alike_get_separate_directories() {
    let (db, dir) = db_and_root().await;
    let paths = LibraryPaths::new(dir.path());
    let source = dir.path().join("extracted");
    extracted(&source, &[("CoolMod/main.lua", b"first")]);

    // A version with no usable ASCII character slugs to the same name the
    // analyzer uses when an archive carries no metadata at all.
    let cjk = manifest(
        "\u{6b63}\u{5f0f}\u{7248}",
        vec![route(
            "CoolMod/main.lua",
            "CoolMod/main.lua",
            RouteKind::Ue4ss,
        )],
    );
    let first = library::store(&db, &paths, &request(&cjk, &source, None))
        .await
        .unwrap();

    let source2 = dir.path().join("extracted2");
    extracted(&source2, &[("CoolMod/main.lua", b"second")]);
    let fallback = manifest(
        "unversioned",
        vec![route(
            "CoolMod/main.lua",
            "CoolMod/main.lua",
            RouteKind::Ue4ss,
        )],
    );
    let second = library::store(&db, &paths, &request(&fallback, &source2, None))
        .await
        .unwrap();

    assert_ne!(
        first.version.library_dir, second.version.library_dir,
        "two distinct versions must not share a library directory"
    );

    library::delete_version(&db, &second.version.id)
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(
            Path::new(&first.version.library_dir).join("ue4ss/CoolMod/main.lua")
        )
        .unwrap(),
        "first",
        "deleting one version must not take the other's bytes"
    );
}

#[tokio::test]
async fn an_install_that_fails_while_keeping_the_archive_leaves_nothing() {
    let (db, dir) = db_and_root().await;
    let paths = LibraryPaths::new(dir.path());
    let source = dir.path().join("extracted");
    extracted(&source, &[("CoolMod/main.lua", b"v1")]);
    let m = manifest(
        "1.0",
        vec![route(
            "CoolMod/main.lua",
            "CoolMod/main.lua",
            RouteKind::Ue4ss,
        )],
    );

    // An archive path that is a directory: the copy into `archives/` fails after
    // the routed files are already in place.
    let bogus_archive = dir.path().join("not-a-file");
    std::fs::create_dir_all(&bogus_archive).unwrap();

    let failed = library::store(&db, &paths, &request(&m, &source, Some(&bogus_archive))).await;
    assert!(
        failed.is_err(),
        "copying a directory as an archive must fail"
    );
    assert!(
        !paths.version_dir("coolmod-ue4ss", "1.0").exists(),
        "the routed files must not survive a failure in the archive step"
    );
    assert!(
        ps_db::mod_library::get_mod(&db, "coolmod-ue4ss")
            .await
            .unwrap()
            .is_none(),
        "and no mods row may be left behind"
    );
}

#[tokio::test]
async fn installing_an_update_keeps_the_users_own_fields() {
    let (db, dir) = db_and_root().await;
    let paths = LibraryPaths::new(dir.path());
    let source = dir.path().join("extracted");
    extracted(&source, &[("CoolMod/main.lua", b"v1")]);
    let one = manifest(
        "1.0",
        vec![route(
            "CoolMod/main.lua",
            "CoolMod/main.lua",
            RouteKind::Ue4ss,
        )],
    );
    let mut first = request(&one, &source, None);
    first.custom_name = Some("My Renamed Mod");
    library::store(&db, &paths, &first).await.unwrap();

    // The user then records a note and dismisses a version.
    let existing = ps_db::mod_library::get_mod(&db, "coolmod-ue4ss")
        .await
        .unwrap()
        .unwrap();
    ps_db::mod_library::upsert_mod(
        &db,
        &ps_db::mod_library::NewMod {
            ignored_version: Some("1.1".to_string()),
            notes: Some("keep the old config".to_string()),
            ..ps_db::mod_library::NewMod {
                id: existing.id.clone(),
                name: existing.name.clone(),
                custom_name: existing.custom_name.clone(),
                mod_type: existing.mod_type.clone(),
                author: existing.author.clone(),
                summary: existing.summary.clone(),
                source_kind: existing.source_kind.clone(),
                source_ref: existing.source_ref.clone(),
                nexus_mod_id: existing.nexus_mod_id,
                ignored_version: None,
                notes: None,
            }
        },
    )
    .await
    .unwrap();

    // The ordinary update path: nothing re-supplies the rename.
    let source2 = dir.path().join("extracted2");
    extracted(&source2, &[("CoolMod/main.lua", b"v2")]);
    let two = manifest(
        "1.1",
        vec![route(
            "CoolMod/main.lua",
            "CoolMod/main.lua",
            RouteKind::Ue4ss,
        )],
    );
    let updated = library::store(&db, &paths, &request(&two, &source2, None))
        .await
        .unwrap();

    assert_eq!(
        updated.mod_row.custom_name.as_deref(),
        Some("My Renamed Mod"),
        "an update must not discard the user's rename"
    );
    assert_eq!(
        updated.mod_row.ignored_version.as_deref(),
        Some("1.1"),
        "nor the flag that says to stop offering a version"
    );
    assert_eq!(
        updated.mod_row.notes.as_deref(),
        Some("keep the old config"),
        "nor the user's notes"
    );
}

#[tokio::test]
async fn a_local_archive_cannot_take_over_a_workshop_mod_id() {
    let (db, dir) = db_and_root().await;
    let paths = LibraryPaths::new(dir.path());
    let source = dir.path().join("extracted");
    extracted(&source, &[("CoolMod/main.lua", b"v1")]);
    let m = manifest(
        "1.0",
        vec![route(
            "CoolMod/main.lua",
            "CoolMod/main.lua",
            RouteKind::Ue4ss,
        )],
    );

    let mut workshop = request(&m, &source, None);
    workshop.source_kind = "workshop";
    library::store(&db, &paths, &workshop).await.unwrap();

    let source2 = dir.path().join("extracted2");
    extracted(&source2, &[("CoolMod/main.lua", b"hijack")]);
    let two = manifest(
        "2.0",
        vec![route(
            "CoolMod/main.lua",
            "CoolMod/main.lua",
            RouteKind::Ue4ss,
        )],
    );
    let local = request(&two, &source2, None); // source_kind "local"
    let refused = library::store(&db, &paths, &local).await;
    assert!(
        matches!(
            refused,
            Err(library::LibraryError::ProvenanceConflict { .. })
        ),
        "a local archive must not silently rewrite a workshop mod's provenance"
    );
    assert_eq!(
        ps_db::mod_library::get_mod(&db, "coolmod-ue4ss")
            .await
            .unwrap()
            .unwrap()
            .source_kind,
        "workshop"
    );
}

#[tokio::test]
async fn deleting_a_mod_removes_bytes_recorded_under_a_root_that_has_since_moved() {
    let (db, dir) = db_and_root().await;
    let old_root = dir.path().join("old");
    let paths = LibraryPaths::new(&old_root);
    let source = dir.path().join("extracted");
    extracted(&source, &[("CoolMod/main.lua", b"v1")]);
    let archive = dir.path().join("CoolMod-1.0.zip");
    std::fs::write(&archive, b"zipbytes").unwrap();
    let m = manifest(
        "1.0",
        vec![route(
            "CoolMod/main.lua",
            "CoolMod/main.lua",
            RouteKind::Ue4ss,
        )],
    );
    let stored = library::store(&db, &paths, &request(&m, &source, Some(&archive)))
        .await
        .unwrap();
    let recorded_dir = stored.version.library_dir.clone();
    let recorded_archive = stored.version.archive_path.clone().unwrap();
    assert!(Path::new(&recorded_dir).exists());

    // The app root moves; a recomputed path would find nothing.
    let moved = LibraryPaths::new(&dir.path().join("new"));
    library::delete_mod(&db, &moved, "coolmod-ue4ss")
        .await
        .unwrap();

    assert!(
        !Path::new(&recorded_dir).exists(),
        "the bytes recorded by the row must be removed, not a recomputed path"
    );
    assert!(
        !Path::new(&recorded_archive).exists(),
        "and the kept archive with them"
    );
}
