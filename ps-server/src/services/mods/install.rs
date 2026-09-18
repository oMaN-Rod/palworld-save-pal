use std::path::Path;

use ps_core::mods::{
    build_manifest, mod_id, AnalyzeInput, ArchiveEntry, InstallManifest, ModIdInput, ModType,
    Platform, TargetKind,
};

use super::extract::{self, ArchiveFormat};
use super::library;
use super::paths::LibraryPaths;

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error(transparent)]
    Extract(#[from] extract::ExtractError),
    #[error(transparent)]
    Library(#[from] library::LibraryError),
    #[error("nothing in this archive could be placed")]
    NothingRouted,
    #[error("{0} is a Steam-subscribed Workshop package already in the library")]
    AlreadyManaged(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// The manifest carries open decisions and `accept_decisions` was not set:
    /// nothing was stored. The caller re-sends with `accept_decisions: true`
    /// to install with the manifest's defaulted routes.
    #[error("installing this archive needs decisions accepted first")]
    NeedsDecisions(Box<InstallManifest>),
}

/// Where the archive came from. Supplied by the caller, never inferred from the
/// file name: the analyzer's `SourceHint` reads a Nexus id out of a name like
/// `MyMod 3.zip`, which is a hint for update checks and must not become an
/// identity.
pub enum Provenance<'a> {
    Local,
    Nexus {
        nexus_mod_id: u32,
        file_id: Option<&'a str>,
        variant: Option<&'a str>,
        version: Option<&'a str>,
    },
    Workshop {
        package: &'a str,
    },
    Bundled {
        framework_key: &'a str,
    },
}

impl Provenance<'_> {
    fn source_kind(&self) -> &'static str {
        match self {
            Provenance::Local => "local",
            Provenance::Nexus { .. } => "nexus",
            Provenance::Workshop { .. } => "workshop",
            Provenance::Bundled { .. } => "bundled",
        }
    }
}

pub struct InstallRequest<'a> {
    pub archive: &'a Path,
    pub target_platform: Platform,
    pub target_kind: TargetKind,
    pub provenance: Provenance<'a>,
    pub custom_name: Option<&'a str>,
    pub keep_archive: bool,
    /// When the manifest carries decisions, `install_archive` stores nothing
    /// and returns `InstallError::NeedsDecisions` unless this is `true`.
    /// `analyze_archive` never reads this field: analysis never stores
    /// regardless.
    pub accept_decisions: bool,
}

pub struct Installed {
    pub mod_id: String,
    pub manifest: InstallManifest,
    pub stored: library::Stored,
}

pub fn identity(manifest: &InstallManifest, provenance: &Provenance<'_>) -> String {
    match provenance {
        Provenance::Local => mod_id(&ModIdInput::Local {
            folder: &manifest.folder_name,
            mod_type: manifest.mod_type,
        }),
        Provenance::Nexus {
            nexus_mod_id,
            variant,
            ..
        } => mod_id(&ModIdInput::Nexus {
            nexus_mod_id: *nexus_mod_id,
            variant: *variant,
        }),
        Provenance::Workshop { package } => mod_id(&ModIdInput::Local {
            folder: package,
            mod_type: ModType::Workshop,
        }),
        Provenance::Bundled { framework_key } => {
            mod_id(&ModIdInput::Framework { key: framework_key })
        }
    }
}

/// What installing would do, without touching the library. The archive is
/// staged and dropped, so the cost is one extraction.
pub async fn analyze_archive(request: &InstallRequest<'_>) -> Result<InstallManifest, InstallError> {
    let staged = stage(request.archive)?;
    manifest_for(&staged, request)
}

fn manifest_for(
    staged: &Staged,
    request: &InstallRequest<'_>,
) -> Result<InstallManifest, InstallError> {
    let archive_name = request
        .archive
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    let (modinfo, workshop_info) = if staged.allows_metadata_scan() {
        (
            read_json(staged.root(), "modinfo.json")
                .and_then(|text| ps_core::mods::parse_modinfo(&text).ok()),
            read_json(staged.root(), "Info.json")
                .and_then(|text| ps_core::mods::parse_workshop_info(&text).ok()),
        )
    } else {
        (None, None)
    };

    let mut manifest = build_manifest(&AnalyzeInput {
        entries: staged.entries(),
        archive_name: &archive_name,
        target_platform: request.target_platform,
        target_kind: request.target_kind,
        modinfo: modinfo.as_ref(),
        workshop_info: workshop_info.as_ref(),
        custom_name: None,
    });
    if manifest.routes.is_empty() {
        return Err(InstallError::NothingRouted);
    }
    reconcile_source_hint(&mut manifest, &request.provenance);
    Ok(manifest)
}

pub async fn install_archive(
    db: &dyn ps_db::DbDriver,
    paths: &LibraryPaths,
    request: &InstallRequest<'_>,
) -> Result<Installed, InstallError> {
    let staged = stage(request.archive)?;
    let manifest = manifest_for(&staged, request)?;
    if !manifest.decisions.is_empty() && !request.accept_decisions {
        return Err(InstallError::NeedsDecisions(Box::new(manifest)));
    }
    if manifest.mod_type == ModType::Workshop {
        let named = library::workshop_mods_named(db, &manifest.folder_name)
            .await
            .map_err(library::LibraryError::Db)?;
        if let Some(subscribed) = named.into_iter().find(library::is_subscribed) {
            return Err(InstallError::AlreadyManaged(subscribed.id));
        }
    }

    let id = identity(&manifest, &request.provenance);
    let source_ref = source_ref_json(&request.provenance);
    let stored = library::store(
        db,
        paths,
        &library::StoreRequest {
            mod_id: &id,
            manifest: &manifest,
            extracted_root: staged.root(),
            archive: request.keep_archive.then_some(request.archive),
            source_kind: request.provenance.source_kind(),
            source_ref: &source_ref,
            custom_name: request.custom_name,
        },
    )
    .await?;

    Ok(Installed {
        mod_id: id,
        manifest,
        stored,
    })
}

/// `build_manifest` fills `source.nexus_mod_id`/`nexus_file_id` with a guess read
/// from the archive's file name. That guess must never reach the `mods` table's
/// `nexus_mod_id` column, which `library::store` populates straight from this
/// field: a local archive guessed at by name must not be linked to a Nexus mod,
/// and a real Nexus download must be linked even when its file name carries no
/// such guess. The caller-supplied `Provenance` is what is authoritative here.
fn reconcile_source_hint(manifest: &mut InstallManifest, provenance: &Provenance<'_>) {
    match provenance {
        Provenance::Nexus {
            nexus_mod_id,
            file_id,
            version,
            ..
        } => {
            manifest.source.nexus_mod_id = Some(*nexus_mod_id);
            manifest.source.nexus_file_id = file_id.map(str::to_string);
            if let Some(version) = valid_nexus_version(*version) {
                manifest.version = version.to_string();
                manifest.source.version = Some(version.to_string());
            }
        }
        Provenance::Local | Provenance::Workshop { .. } | Provenance::Bundled { .. } => {
            manifest.source.nexus_mod_id = None;
            manifest.source.nexus_file_id = None;
        }
    }
}

/// A Nexus file version is trusted only when its trimmed length is 1..=64
/// bytes; a blank or over-long value must not override the manifest's own
/// version or appear in `source_ref`.
pub(crate) fn valid_nexus_version(version: Option<&str>) -> Option<&str> {
    version
        .map(str::trim)
        .filter(|version| !version.is_empty() && version.len() <= 64)
}

fn source_ref_json(provenance: &Provenance<'_>) -> String {
    let value = match provenance {
        Provenance::Local => serde_json::json!({ "kind": "local" }),
        Provenance::Nexus {
            nexus_mod_id,
            file_id,
            variant,
            version,
        } => serde_json::json!({
            "kind": "nexus",
            "nexus_mod_id": nexus_mod_id,
            "file_id": file_id,
            "variant": variant,
            "version": valid_nexus_version(*version),
        }),
        Provenance::Workshop { package } => {
            serde_json::json!({ "kind": "workshop", "package": package })
        }
        Provenance::Bundled { framework_key } => {
            serde_json::json!({ "kind": "bundled", "framework": framework_key })
        }
    };
    value.to_string()
}

/// An extracted archive, or a single loose file presented as one. Both shapes own
/// a temp dir so the caller has one lifetime to reason about.
enum Staged {
    Archive(extract::Extracted),
    /// A single file that is not an archive. `root` is the file's own directory,
    /// so nothing is copied: a bare 3 GiB pak would otherwise be written to the
    /// system temp volume before being written again into the library.
    LooseFile {
        root: std::path::PathBuf,
        entries: Vec<ArchiveEntry>,
    },
}

impl Staged {
    fn root(&self) -> &Path {
        match self {
            Staged::Archive(extracted) => extracted.path(),
            Staged::LooseFile { root, .. } => root,
        }
    }

    fn entries(&self) -> &[ArchiveEntry] {
        match self {
            Staged::Archive(extracted) => &extracted.entries,
            Staged::LooseFile { entries, .. } => entries,
        }
    }

    /// A loose file's root is a directory the user owns — a downloads folder, say —
    /// so it is never scanned for metadata. Only an extracted archive's payload is.
    fn allows_metadata_scan(&self) -> bool {
        matches!(self, Staged::Archive(_))
    }
}

/// Dropping a bare `.pak`, `.lua` or `.dll` on the app is a real flow and
/// `sniff_format` reports `Unknown` for it, so it is staged as an archive of one
/// and routed like anything else.
fn stage(path: &Path) -> Result<Staged, InstallError> {
    if extract::sniff_format(path)? != ArchiveFormat::Unknown {
        return Ok(Staged::Archive(extract::extract(path)?));
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| {
            InstallError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "archive has no file name",
            ))
        })?;
    let root = path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let size = std::fs::metadata(path)?.len();
    Ok(Staged::LooseFile {
        root,
        entries: vec![ArchiveEntry { path: name, size }],
    })
}

/// Finds a metadata file the way the analyzer does: by lowercased leaf name, at
/// any depth, shallowest first. `analyze_flags` sets `has_info_json` from an
/// `info.json` leaf anywhere in the tree and `detect_mod_type` returns `Workshop`
/// on that flag alone, so a lookup that searched only the root and one wrapper
/// directory would leave `build_manifest` without the `workshop_info` it needs and
/// produce a mod typed `workshop` with UE4SS routes and no version. Archives with
/// a `__MACOSX` sibling, a `Release/` wrapper, a `Mods/CoolMod/` pair, or a
/// lowercase `info.json` on a case-sensitive filesystem all take that path.
fn read_json(root: &Path, name: &str) -> Option<String> {
    let wanted = name.to_ascii_lowercase();
    let mut best: Option<(usize, std::path::PathBuf)> = None;
    for found in walkdir::WalkDir::new(root).follow_links(false) {
        let Ok(found) = found else { continue };
        if !found.file_type().is_file() {
            continue;
        }
        let matches_name = found
            .path()
            .file_name()
            .map(|leaf| leaf.to_string_lossy().to_ascii_lowercase() == wanted)
            .unwrap_or(false);
        if !matches_name {
            continue;
        }
        // A resource fork from a Mac-authored zip is not the metadata file.
        if found
            .path()
            .components()
            .any(|c| c.as_os_str().to_string_lossy().starts_with("__MACOSX"))
        {
            continue;
        }
        let depth = found.depth();
        if best.as_ref().map(|(d, _)| depth < *d).unwrap_or(true) {
            best = Some((depth, found.path().to_path_buf()));
        }
    }
    std::fs::read_to_string(best?.1).ok()
}
