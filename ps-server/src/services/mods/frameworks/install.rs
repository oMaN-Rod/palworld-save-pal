//! Downloads a framework release, extracts it, and stores it in the library.
use std::path::PathBuf;

use ps_core::mods::FrameworkArchiveError;

use super::super::deploy;
use super::super::extract;
use super::super::library;
use super::super::paths::LibraryPaths;
use super::source::{FrameworkSource, Progress, Release, SourceError};

#[derive(Debug, thiserror::Error)]
pub enum FrameworkInstallError {
    #[error(transparent)]
    Source(#[from] SourceError),
    #[error(transparent)]
    Archive(#[from] FrameworkArchiveError),
    #[error(transparent)]
    Extract(#[from] extract::ExtractError),
    #[error(transparent)]
    Library(#[from] library::LibraryError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Db(#[from] ps_db::DbError),
}

impl FrameworkInstallError {
    pub fn code(&self) -> &'static str {
        match self {
            FrameworkInstallError::Source(error) => error.code(),
            FrameworkInstallError::Archive(_) => "invalid_framework_archive",
            FrameworkInstallError::Extract(_) => "extract_failed",
            FrameworkInstallError::Library(_) => "library_error",
            FrameworkInstallError::Io(_) => "io",
            FrameworkInstallError::Db(_) => "db",
        }
    }
}

#[derive(Debug)]
pub struct StoredFramework {
    pub mod_version_id: String,
    pub version: String,
    pub display: String,
    pub installed_new: bool,
}

/// Where an in-progress download and extraction sit before they are stored.
/// Removed after every install and swept again at startup.
pub fn scratch_root(library: &LibraryPaths) -> PathBuf {
    library
        .root()
        .parent()
        .unwrap_or_else(|| library.root())
        .join("downloads")
        .join(".frameworks")
}

struct ScratchGuard(PathBuf);

impl Drop for ScratchGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The version another writer already committed, read back as a successful,
/// non-fresh install rather than surfaced as an error.
async fn already_installed_result(
    db: &dyn ps_db::DbDriver,
    id: String,
) -> Result<StoredFramework, FrameworkInstallError> {
    let existing = ps_db::mod_library::get_version(db, &id)
        .await?
        .ok_or(library::LibraryError::AlreadyInstalled(id))?;
    Ok(StoredFramework {
        display: display_of(&existing.source_ref, &existing.version),
        mod_version_id: existing.id,
        version: existing.version,
        installed_new: false,
    })
}

pub(crate) fn display_of(source_ref: &str, fallback: &str) -> String {
    serde_json::from_str::<serde_json::Value>(source_ref)
        .ok()
        .and_then(|value| {
            value
                .get("display")
                .and_then(|display| display.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| fallback.to_string())
}

/// Fetches `release`'s asset, extracts it, routes it as `release.key`'s
/// package, and stores it in the library. An already-installed version is
/// returned without touching the network or the filesystem.
pub async fn store_release(
    db: &dyn ps_db::DbDriver,
    library: &LibraryPaths,
    source: &dyn FrameworkSource,
    release: &Release,
    progress: Progress<'_>,
) -> Result<StoredFramework, FrameworkInstallError> {
    let mod_id = release.key.mod_id();
    let version_id = library::version_id(&mod_id, &release.version);
    if let Some(existing) = ps_db::mod_library::get_version(db, &version_id).await? {
        return Ok(StoredFramework {
            display: display_of(&existing.source_ref, &existing.version),
            mod_version_id: existing.id,
            version: existing.version,
            installed_new: false,
        });
    }

    let scratch = scratch_root(library).join(deploy::backup::new_op_id());
    let _guard = ScratchGuard(scratch.clone());

    let archive_name = std::path::Path::new(&release.asset.name)
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("asset.zip"));
    let archive = scratch.join(archive_name);
    source.fetch(release, &archive, progress).await?;

    let extracted = tokio::task::spawn_blocking({
        let archive = archive.clone();
        move || extract::extract(&archive)
    })
    .await
    .map_err(|error| std::io::Error::other(error.to_string()))??;

    let package =
        ps_core::mods::framework_package(release.key, &release.version, &extracted.entries)?;

    for (archive_path, contents) in &package.generated {
        let mut path = extracted.path().to_path_buf();
        for segment in archive_path.split('/') {
            path.push(segment);
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, contents)?;
    }

    let source_ref = serde_json::json!({
        "framework": release.key.as_str(),
        "origin": release.origin,
        "repo": release.repo,
        "tag": release.tag,
        "asset": release.asset.name,
        "display": release.display,
    });

    let stored = match library::store(
        db,
        library,
        &library::StoreRequest {
            mod_id: &mod_id,
            manifest: &package.manifest,
            extracted_root: extracted.path(),
            archive: Some(&archive),
            source_kind: "framework",
            source_ref: &source_ref.to_string(),
            custom_name: None,
        },
    )
    .await
    {
        Ok(stored) => stored,
        // Another install of the same version won the race between our own
        // pre-check above and this write: report it, don't fail the caller.
        Err(library::LibraryError::AlreadyInstalled(id)) => {
            return already_installed_result(db, id).await
        }
        Err(error) => return Err(error.into()),
    };

    Ok(StoredFramework {
        mod_version_id: stored.version.id,
        version: release.version.clone(),
        display: release.display.clone(),
        installed_new: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::services::mods::frameworks::source::ReleaseAsset;
    use ps_core::mods::FrameworkKey;

    fn zip_bytes(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut cursor);
            for (name, body) in files {
                writer
                    .start_file(*name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                std::io::Write::write_all(&mut writer, body).unwrap();
            }
            writer.finish().unwrap();
        }
        cursor.into_inner()
    }

    struct ZipSource {
        bytes: Vec<u8>,
        fetches: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl FrameworkSource for ZipSource {
        async fn latest(
            &self,
            _key: FrameworkKey,
            _channel: super::super::source::Channel,
            _dev: bool,
        ) -> Result<Release, SourceError> {
            unreachable!()
        }

        async fn fetch(
            &self,
            _release: &Release,
            dest: &Path,
            _progress: Progress<'_>,
        ) -> Result<(), SourceError> {
            self.fetches.fetch_add(1, Ordering::SeqCst);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(dest, &self.bytes)?;
            Ok(())
        }
    }

    fn ue4ss_release() -> Release {
        Release {
            key: FrameworkKey::Ue4ss,
            tag: "2281fa31".to_string(),
            version: "2281fa31".to_string(),
            display: "2281fa31 (01.01.2026)".to_string(),
            asset: ReleaseAsset {
                name: "UE4SS.zip".to_string(),
                url: "https://example.invalid/UE4SS.zip".to_string(),
                size: Some(10),
            },
            origin: "github",
            repo: Some("UE4SS-RE/RE-UE4SS"),
        }
    }

    fn no_progress(_got: u64, _total: Option<u64>) {}

    #[tokio::test]
    async fn a_fresh_ue4ss_release_is_routed_and_stored() {
        let dir = tempfile::tempdir().unwrap();
        let db = ps_db::SqlxSqliteDriver::new(ps_db::open(&dir.path().join("t.db")).await.unwrap());
        let library = LibraryPaths::new(dir.path());
        let bytes = zip_bytes(&[
            ("dwmapi.dll", b"proxy"),
            ("ue4ss/UE4SS.dll", b"runtime"),
            ("ue4ss/Mods/mods.txt", b"ignored"),
            ("ue4ss/Mods/Keybinds/Scripts/main.lua", b"keybinds"),
        ]);
        let source = ZipSource {
            bytes,
            fetches: AtomicUsize::new(0),
        };
        let release = ue4ss_release();

        let stored = store_release(&db, &library, &source, &release, &no_progress)
            .await
            .unwrap();

        assert_eq!(stored.mod_version_id, "framework-ue4ss@2281fa31");
        assert!(stored.installed_new);

        let version = ps_db::mod_library::get_version(&db, &stored.mod_version_id)
            .await
            .unwrap()
            .unwrap();
        let manifest: ps_core::mods::InstallManifest =
            serde_json::from_str(&version.manifest).unwrap();
        let kind_of = |rel: &str| {
            manifest
                .routes
                .iter()
                .find(|route| route.rel_path == rel)
                .map(|route| route.kind)
        };
        assert_eq!(
            kind_of("dwmapi.dll"),
            Some(ps_core::mods::RouteKind::Binaries)
        );
        assert_eq!(
            kind_of("UE4SS.dll"),
            Some(ps_core::mods::RouteKind::Ue4ssCore)
        );
        assert_eq!(
            kind_of("ue4ss.version"),
            Some(ps_core::mods::RouteKind::Ue4ssCore)
        );
        assert_eq!(
            kind_of("Keybinds/Scripts/main.lua"),
            Some(ps_core::mods::RouteKind::Ue4ss)
        );

        let version_content = std::fs::read_to_string(LibraryPaths::route_path_in(
            Path::new(&version.library_dir),
            ps_core::mods::RouteKind::Ue4ssCore,
            "ue4ss.version",
        ))
        .unwrap();
        assert_eq!(version_content, "2281fa31");

        let mod_row = ps_db::mod_library::get_mod(&db, "framework-ue4ss")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(mod_row.source_kind, "framework");
        assert_eq!(mod_row.mod_type, "framework");

        assert_eq!(source.fetches.load(Ordering::SeqCst), 1);

        let second = store_release(&db, &library, &source, &release, &no_progress)
            .await
            .unwrap();
        assert!(!second.installed_new);
        assert_eq!(second.mod_version_id, stored.mod_version_id);
        assert_eq!(source.fetches.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_scratch_archive_name_with_path_separators_stays_inside_scratch() {
        let dir = tempfile::tempdir().unwrap();
        let db = ps_db::SqlxSqliteDriver::new(ps_db::open(&dir.path().join("t.db")).await.unwrap());
        let library = LibraryPaths::new(dir.path());
        let bytes = zip_bytes(&[("dwmapi.dll", b"proxy"), ("ue4ss/UE4SS.dll", b"runtime")]);
        let source = ZipSource {
            bytes,
            fetches: AtomicUsize::new(0),
        };
        let mut release = ue4ss_release();
        release.asset.name = "../../evil/UE4SS.zip".to_string();

        store_release(&db, &library, &source, &release, &no_progress)
            .await
            .unwrap();

        let scratch = scratch_root(&library);
        let remaining = std::fs::read_dir(&scratch)
            .map(|read| read.count())
            .unwrap_or(0);
        assert_eq!(remaining, 0, "scratch root must hold no entries");
        assert!(
            !scratch.parent().unwrap().join("evil").exists(),
            "the archive must never escape the op's own scratch directory"
        );
    }

    #[tokio::test]
    async fn a_version_already_committed_by_another_writer_is_read_back_not_errored() {
        let dir = tempfile::tempdir().unwrap();
        let db = ps_db::SqlxSqliteDriver::new(ps_db::open(&dir.path().join("t.db")).await.unwrap());
        let library = LibraryPaths::new(dir.path());
        let bytes = zip_bytes(&[("dwmapi.dll", b"proxy"), ("ue4ss/UE4SS.dll", b"runtime")]);
        let source = ZipSource {
            bytes,
            fetches: AtomicUsize::new(0),
        };
        let release = ue4ss_release();
        let stored = store_release(&db, &library, &source, &release, &no_progress)
            .await
            .unwrap();
        assert!(stored.installed_new);

        // Simulates `library::store` finding the row a concurrent caller
        // already committed between our own pre-check and this write.
        let result = already_installed_result(&db, stored.mod_version_id.clone())
            .await
            .unwrap();

        assert!(!result.installed_new, "{result:?}");
        assert_eq!(result.mod_version_id, stored.mod_version_id);
        assert_eq!(result.version, stored.version);
    }

    #[tokio::test]
    async fn a_zip_missing_dwmapi_is_an_invalid_archive_and_stores_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let db = ps_db::SqlxSqliteDriver::new(ps_db::open(&dir.path().join("t.db")).await.unwrap());
        let library = LibraryPaths::new(dir.path());
        let bytes = zip_bytes(&[("ue4ss/UE4SS.dll", b"runtime")]);
        let source = ZipSource {
            bytes,
            fetches: AtomicUsize::new(0),
        };
        let release = ue4ss_release();

        let error = store_release(&db, &library, &source, &release, &no_progress)
            .await
            .unwrap_err();

        assert!(matches!(error, FrameworkInstallError::Archive(_)));
        assert_eq!(error.code(), "invalid_framework_archive");
        assert!(
            ps_db::mod_library::get_version(&db, "framework-ue4ss@2281fa31")
                .await
                .unwrap()
                .is_none()
        );
        let scratch = scratch_root(&library);
        let remaining = std::fs::read_dir(&scratch)
            .map(|read| read.count())
            .unwrap_or(0);
        assert_eq!(remaining, 0, "scratch root must hold no entries");
    }
}
