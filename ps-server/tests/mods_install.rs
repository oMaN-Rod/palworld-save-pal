use std::io::Write;
use std::path::Path;

use ps_core::mods::{ModType, Platform, RouteKind, TargetKind};
use ps_server::services::mods::{install, LibraryPaths};

async fn db_and_root() -> (ps_db::SqlxSqliteDriver, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let pool = ps_db::open(&dir.path().join("test.db")).await.unwrap();
    (ps_db::SqlxSqliteDriver::new(pool), dir)
}

fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let file = std::fs::File::create(path).unwrap();
    let mut writer = zip::ZipWriter::new(file);
    let options: zip::write::FileOptions<'_, ()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, body) in entries {
        writer.start_file(*name, options).unwrap();
        writer.write_all(body).unwrap();
    }
    writer.finish().unwrap();
}

fn request<'a>(
    archive: &'a Path,
    provenance: install::Provenance<'a>,
) -> install::InstallRequest<'a> {
    install::InstallRequest {
        archive,
        target_platform: Platform::Win64,
        target_kind: TargetKind::Client,
        provenance,
        custom_name: None,
        keep_archive: true,
        accept_decisions: true,
    }
}

#[tokio::test]
async fn a_ue4ss_zip_becomes_a_library_entry() {
    let (db, dir) = db_and_root().await;
    let paths = LibraryPaths::new(dir.path());
    let archive = dir.path().join("CoolMod-1.2.zip");
    write_zip(
        &archive,
        &[
            ("CoolMod/Scripts/main.lua", b"print('hi')"),
            ("CoolMod/enabled.txt", b""),
        ],
    );

    let installed =
        install::install_archive(&db, &paths, &request(&archive, install::Provenance::Local))
            .await
            .unwrap();

    assert_eq!(installed.manifest.mod_type, ModType::Ue4ss);
    assert_eq!(installed.mod_id, "coolmod-ue4ss");
    assert_eq!(installed.stored.mod_row.source_kind, "local");
    assert!(
        !installed.manifest.routes.is_empty(),
        "{:?}",
        installed.manifest
    );
    let lua = paths.route_path(
        "coolmod-ue4ss",
        &installed.manifest.version,
        RouteKind::Ue4ss,
        "CoolMod/Scripts/main.lua",
    );
    assert_eq!(std::fs::read_to_string(&lua).unwrap(), "print('hi')");
    assert!(
        installed.stored.version.archive_path.is_some(),
        "keep_archive was true"
    );
}

#[tokio::test]
async fn a_custom_name_is_stored_beside_the_archive_name_without_renaming_the_mod() {
    let (db, dir) = db_and_root().await;
    let paths = LibraryPaths::new(dir.path());
    let archive = dir.path().join("Cool Mod.zip");
    write_zip(&archive, &[("CoolMod/Scripts/main.lua", b"print('hi')")]);

    let mut renamed = request(&archive, install::Provenance::Local);
    renamed.custom_name = Some("My Renamed Mod");

    let installed = install::install_archive(&db, &paths, &renamed)
        .await
        .unwrap();

    assert_eq!(installed.mod_id, "coolmod-ue4ss");
    assert_eq!(installed.manifest.display_name, "Cool Mod");
    assert_eq!(
        installed.stored.mod_row.name, "Cool Mod",
        "the row keeps the archive's own name"
    );
    assert_eq!(
        installed.stored.mod_row.custom_name.as_deref(),
        Some("My Renamed Mod")
    );
}

#[tokio::test]
async fn a_local_archive_named_like_a_nexus_download_does_not_become_a_nexus_mod() {
    let (db, dir) = db_and_root().await;
    let paths = LibraryPaths::new(dir.path());
    // The analyzer reads "3" out of this name as a Nexus id hint.
    let archive = dir.path().join("CoolMod 3.zip");
    write_zip(&archive, &[("CoolMod/Scripts/main.lua", b"print('hi')")]);

    let installed =
        install::install_archive(&db, &paths, &request(&archive, install::Provenance::Local))
            .await
            .unwrap();

    assert_eq!(
        installed.mod_id, "coolmod-ue4ss",
        "provenance is Local, so the filename's Nexus hint must not become the identity"
    );
    assert_eq!(installed.stored.mod_row.source_kind, "local");
    assert_eq!(
        installed.stored.mod_row.nexus_mod_id, None,
        "and the column stays empty"
    );
}

#[tokio::test]
async fn a_nexus_download_uses_the_id_the_caller_supplies() {
    let (db, dir) = db_and_root().await;
    let paths = LibraryPaths::new(dir.path());
    let archive = dir.path().join("CoolMod-1.0.zip");
    write_zip(&archive, &[("CoolMod/Scripts/main.lua", b"print('hi')")]);

    let installed = install::install_archive(
        &db,
        &paths,
        &request(
            &archive,
            install::Provenance::Nexus {
                nexus_mod_id: 4821,
                file_id: Some("99001"),
                variant: None,
                version: None,
            },
        ),
    )
    .await
    .unwrap();

    assert_eq!(installed.mod_id, "nexus-4821");
    assert_eq!(installed.stored.mod_row.source_kind, "nexus");
    assert_eq!(installed.stored.mod_row.nexus_mod_id, Some(4821));
}

#[tokio::test]
async fn a_nexus_variant_gets_its_own_identity() {
    let (db, dir) = db_and_root().await;
    let paths = LibraryPaths::new(dir.path());
    let archive = dir.path().join("CoolMod-lite.zip");
    write_zip(&archive, &[("CoolMod/Scripts/main.lua", b"print('hi')")]);

    let installed = install::install_archive(
        &db,
        &paths,
        &request(
            &archive,
            install::Provenance::Nexus {
                nexus_mod_id: 4821,
                file_id: None,
                variant: Some("Lite Edition"),
                version: None,
            },
        ),
    )
    .await
    .unwrap();
    assert_eq!(installed.mod_id, "nexus-4821-lite_edition");
}

#[tokio::test]
async fn a_nexus_file_version_becomes_the_library_version() {
    let (db, dir) = db_and_root().await;
    let paths = LibraryPaths::new(dir.path());
    let archive = dir.path().join("Cool Mod-4821-1-2-0-99001.zip");
    write_zip(&archive, &[("CoolMod/Scripts/main.lua", b"print('hi')")]);

    let installed = install::install_archive(
        &db,
        &paths,
        &request(
            &archive,
            install::Provenance::Nexus {
                nexus_mod_id: 4821,
                file_id: Some("99001"),
                variant: None,
                version: Some(" 1.2.0 "),
            },
        ),
    )
    .await
    .unwrap();

    assert_eq!(installed.mod_id, "nexus-4821");
    assert_eq!(installed.manifest.version, "1.2.0");
    assert_eq!(installed.manifest.source.version.as_deref(), Some("1.2.0"));
    assert_eq!(installed.stored.version.id, "nexus-4821@1.2.0");
    let source_ref: serde_json::Value =
        serde_json::from_str(&installed.stored.version.source_ref).unwrap();
    assert_eq!(source_ref["version"], "1.2.0");
    assert_eq!(source_ref["file_id"], "99001");
}

#[tokio::test]
async fn a_blank_nexus_version_falls_back_to_the_manifest_version() {
    let (db, dir) = db_and_root().await;
    let paths = LibraryPaths::new(dir.path());
    let archive = dir.path().join("Cool Mod-4821-1-2-0-99001.zip");
    write_zip(&archive, &[("CoolMod/Scripts/main.lua", b"print('hi')")]);

    let installed = install::install_archive(
        &db,
        &paths,
        &request(
            &archive,
            install::Provenance::Nexus {
                nexus_mod_id: 4821,
                file_id: Some("99001"),
                variant: None,
                version: Some("   "),
            },
        ),
    )
    .await
    .unwrap();

    assert_eq!(installed.manifest.version, "unversioned");
    assert_eq!(installed.manifest.source.version, None);
    let source_ref: serde_json::Value =
        serde_json::from_str(&installed.stored.version.source_ref).unwrap();
    assert_eq!(source_ref["version"], serde_json::Value::Null);
}

#[tokio::test]
async fn a_65_byte_nexus_version_falls_back_to_the_manifest_version() {
    let (db, dir) = db_and_root().await;
    let paths = LibraryPaths::new(dir.path());
    let archive = dir.path().join("Cool Mod-4821-1-2-0-99001.zip");
    write_zip(&archive, &[("CoolMod/Scripts/main.lua", b"print('hi')")]);
    let overlong = "v".repeat(65);

    let installed = install::install_archive(
        &db,
        &paths,
        &request(
            &archive,
            install::Provenance::Nexus {
                nexus_mod_id: 4821,
                file_id: Some("99001"),
                variant: None,
                version: Some(&overlong),
            },
        ),
    )
    .await
    .unwrap();

    assert_eq!(installed.manifest.version, "unversioned");
    assert_eq!(installed.manifest.source.version, None);
    let source_ref: serde_json::Value =
        serde_json::from_str(&installed.stored.version.source_ref).unwrap();
    assert_eq!(source_ref["version"], serde_json::Value::Null);
}

#[tokio::test]
async fn a_bare_pak_installs_as_a_one_file_mod() {
    let (db, dir) = db_and_root().await;
    let paths = LibraryPaths::new(dir.path());
    let pak = dir.path().join("CoolMod_P.pak");
    std::fs::write(&pak, b"pakbytes").unwrap();

    let installed =
        install::install_archive(&db, &paths, &request(&pak, install::Provenance::Local))
            .await
            .unwrap();

    assert_eq!(installed.manifest.mod_type, ModType::Pak);
    assert_eq!(installed.manifest.routes.len(), 1);
    assert_eq!(installed.manifest.routes[0].kind, RouteKind::Pak);
    let stored = paths.route_path(
        &installed.mod_id,
        &installed.manifest.version,
        RouteKind::Pak,
        &installed.manifest.routes[0].rel_path,
    );
    assert_eq!(std::fs::read(&stored).unwrap(), b"pakbytes");
}

#[tokio::test]
async fn keep_archive_false_stores_no_archive() {
    let (db, dir) = db_and_root().await;
    let paths = LibraryPaths::new(dir.path());
    let archive = dir.path().join("CoolMod-1.0.zip");
    write_zip(&archive, &[("CoolMod/Scripts/main.lua", b"print('hi')")]);

    let mut req = request(&archive, install::Provenance::Local);
    req.keep_archive = false;
    let installed = install::install_archive(&db, &paths, &req).await.unwrap();
    assert_eq!(installed.stored.version.archive_path, None);
    assert!(!paths.archives_dir(&installed.mod_id).exists());
}

#[tokio::test]
async fn an_archive_with_nothing_placeable_is_refused() {
    let (db, dir) = db_and_root().await;
    let paths = LibraryPaths::new(dir.path());
    let archive = dir.path().join("docs.zip");
    write_zip(
        &archive,
        &[("README.md", b"read me"), ("screenshot.png", b"\x89PNG")],
    );

    let failed =
        install::install_archive(&db, &paths, &request(&archive, install::Provenance::Local)).await;
    assert!(
        matches!(failed, Err(install::InstallError::NothingRouted)),
        "an archive of documentation creates no library entry"
    );
    assert!(ps_db::mod_library::list_mods(&db).await.unwrap().is_empty());
}

#[tokio::test]
async fn a_workshop_package_is_identified_by_its_package_name() {
    let (db, dir) = db_and_root().await;
    let paths = LibraryPaths::new(dir.path());
    let archive = dir.path().join("pack.zip");
    write_zip(
        &archive,
        &[
            (
                "Info.json",
                br#"{"PackageName":"CoolPack","Version":"1.0","InstallRules":[]}"#,
            ),
            ("CoolPack_P.pak", b"pakbytes"),
        ],
    );

    let installed = install::install_archive(
        &db,
        &paths,
        &request(
            &archive,
            install::Provenance::Workshop {
                package: "CoolPack",
            },
        ),
    )
    .await
    .unwrap();
    assert_eq!(installed.mod_id, "coolpack-workshop");
    assert_eq!(installed.stored.mod_row.source_kind, "workshop");
}

#[tokio::test]
async fn a_workshop_archive_for_a_subscribed_package_is_refused() {
    use ps_server::services::mods::library;
    let (db, dir) = db_and_root().await;
    let paths = LibraryPaths::new(dir.path());
    let manifest = ps_core::mods::InstallManifest {
        folder_name: "CoolPack".to_string(),
        display_name: "CoolPack".to_string(),
        mod_type: ModType::Workshop,
        version: "steam-subscribed".to_string(),
        routes: Vec::new(),
        decisions: Vec::new(),
        platform_filtered: None,
        source: Default::default(),
    };
    library::store(
        &db,
        &paths,
        &library::StoreRequest {
            mod_id: "workshop-3300000001",
            manifest: &manifest,
            extracted_root: Path::new(""),
            archive: None,
            source_kind: "workshop",
            source_ref: r#"{"kind":"workshop","package":"CoolPack","workshop_id":"3300000001"}"#,
            custom_name: None,
        },
    )
    .await
    .unwrap();
    let archive = dir.path().join("pack.zip");
    write_zip(
        &archive,
        &[
            (
                "Info.json",
                br#"{"PackageName":"CoolPack","Version":"1.0","InstallRules":[]}"#,
            ),
            ("CoolPack_P.pak", b"pakbytes"),
        ],
    );

    for provenance in [
        install::Provenance::Local,
        install::Provenance::Workshop {
            package: "CoolPack",
        },
    ] {
        let refused = install::install_archive(&db, &paths, &request(&archive, provenance)).await;
        assert!(
            matches!(&refused, Err(install::InstallError::AlreadyManaged(id)) if id == "workshop-3300000001"),
            "an installed copy must not take over the subscribed row"
        );
    }
    let rows = ps_db::mod_library::list_mods(&db).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert!(library::is_subscribed(&rows[0]), "{rows:?}");
}

#[tokio::test]
async fn a_workshop_info_json_behind_a_wrapper_directory_is_still_found() {
    let (db, dir) = db_and_root().await;
    let paths = LibraryPaths::new(dir.path());
    let archive = dir.path().join("pack.zip");
    // Two top-level entries plus the metadata one level deeper: the shape a
    // Mac-authored or repacked zip takes, and the one a root-only lookup misses.
    write_zip(
        &archive,
        &[
            ("__MACOSX/._Info.json", b"resource fork"),
            (
                "CoolPack/Info.json",
                br#"{"PackageName":"CoolPack","Version":"2.5","InstallRules":[]}"#,
            ),
            ("CoolPack/CoolPack_P.pak", b"pakbytes"),
        ],
    );

    let installed = install::install_archive(
        &db,
        &paths,
        &request(
            &archive,
            install::Provenance::Workshop {
                package: "CoolPack",
            },
        ),
    )
    .await
    .unwrap();

    assert_eq!(
        installed.manifest.version, "2.5",
        "the version must come from Info.json, not from the unversioned fallback"
    );
}

#[tokio::test]
async fn a_bare_pak_is_not_copied_through_a_temp_directory() {
    let (db, dir) = db_and_root().await;
    let paths = LibraryPaths::new(dir.path());
    let drop_folder = dir.path().join("Downloads");
    std::fs::create_dir_all(&drop_folder).unwrap();
    let pak = drop_folder.join("CoolMod_P.pak");
    std::fs::write(&pak, b"pakbytes").unwrap();
    // A neighbour in the same folder that has nothing to do with the install.
    std::fs::write(drop_folder.join("unrelated.zip"), b"not mine").unwrap();
    std::fs::write(drop_folder.join("modinfo.json"), br#"{"version":"9.9"}"#).unwrap();

    let mut req = request(&pak, install::Provenance::Local);
    req.keep_archive = false;
    let installed = install::install_archive(&db, &paths, &req).await.unwrap();

    assert_eq!(
        installed.manifest.routes.len(),
        1,
        "only the dropped file is routed"
    );
    assert_ne!(
        installed.manifest.version, "9.9",
        "a stray modinfo.json in the user's folder must not be read"
    );
    let stored = paths.route_path(
        &installed.mod_id,
        &installed.manifest.version,
        RouteKind::Pak,
        &installed.manifest.routes[0].rel_path,
    );
    assert_eq!(std::fs::read(&stored).unwrap(), b"pakbytes");
}
