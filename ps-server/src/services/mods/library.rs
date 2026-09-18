use std::collections::HashSet;
use std::path::Path;

use ps_core::mods::{InstallManifest, RouteKind};

use super::digest;
use super::paths::LibraryPaths;

pub const WORKSHOP_SOURCE_KIND: &str = "workshop";

/// Steam owns a subscribed package's files and versions: the library holds a row
/// and one synthetic version with no routes, and PalStudio only edits its
/// `ActiveModList` line. An installed Workshop archive shares the source kind but
/// never records a Steam item id.
pub fn is_subscribed(row: &ps_db::mod_library::ModRow) -> bool {
    row.source_kind == WORKSHOP_SOURCE_KIND && workshop_id_of(row).is_some()
}

pub fn is_workshop_item_id(id: &str) -> bool {
    !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit())
}

fn source_ref_field(row: &ps_db::mod_library::ModRow, field: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(&row.source_ref).ok()?;
    value.get(field)?.as_str().map(str::to_string)
}

pub fn workshop_id_of(row: &ps_db::mod_library::ModRow) -> Option<String> {
    source_ref_field(row, "workshop_id").filter(|id| is_workshop_item_id(id))
}

pub fn workshop_package_of(row: &ps_db::mod_library::ModRow) -> Option<String> {
    source_ref_field(row, "package")
}

/// Keyed on the Steam item rather than the package name, whose slug two
/// different packages can share.
pub fn subscribed_mod_id(workshop_id: &str) -> String {
    format!("workshop-{workshop_id}")
}

pub async fn is_subscribed_mod(db: &dyn ps_db::DbDriver, mod_id: &str) -> Result<bool, ps_db::DbError> {
    Ok(ps_db::mod_library::get_mod(db, mod_id)
        .await?
        .as_ref()
        .is_some_and(is_subscribed))
}

pub(crate) async fn package_names(db: &dyn ps_db::DbDriver, mod_id: &str) -> Result<Vec<String>, ps_db::DbError> {
    let mut names: Vec<String> = Vec::new();
    for version in ps_db::mod_library::versions_of(db, mod_id).await? {
        if let Ok(manifest) = serde_json::from_str::<InstallManifest>(&version.manifest) {
            if manifest.mod_type == ps_core::mods::ModType::Workshop
                && !names.contains(&manifest.folder_name)
            {
                names.push(manifest.folder_name);
            }
        }
    }
    Ok(names)
}

/// Workshop mods whose versions name `package` exactly as `ActiveModList` holds it.
pub async fn workshop_mods_named(
    db: &dyn ps_db::DbDriver,
    package: &str,
) -> Result<Vec<ps_db::mod_library::ModRow>, ps_db::DbError> {
    let mut named = Vec::new();
    for row in ps_db::mod_library::list_mods(db).await? {
        if row.mod_type == "workshop" && package_names(db, &row.id).await?.iter().any(|name| name == package) {
            named.push(row);
        }
    }
    Ok(named)
}

/// Mods a target holds in any profile or has deployed files for.
pub async fn mods_on_target(
    db: &dyn ps_db::DbDriver,
    target_id: &str,
) -> Result<HashSet<String>, ps_db::DbError> {
    let mut mods = held_by_target(db, target_id).await?;
    let versions: HashSet<String> = ps_db::mod_deployments::files_of(db, target_id)
        .await?
        .into_iter()
        .filter_map(|row| row.mod_version_id)
        .collect();
    for version_id in versions {
        if let Some(version) = ps_db::mod_library::get_version(db, &version_id).await? {
            mods.insert(version.mod_id);
        }
    }
    Ok(mods)
}

pub async fn held_by_target(
    db: &dyn ps_db::DbDriver,
    target_id: &str,
) -> Result<HashSet<String>, ps_db::DbError> {
    let mut held = HashSet::new();
    for profile in ps_db::mod_profiles::for_target(db, target_id).await? {
        for entry in ps_db::mod_profiles::mods_of(db, &profile.id).await? {
            held.insert(entry.mod_id);
        }
    }
    Ok(held)
}

fn released_key(target_id: &str) -> String {
    format!("mods.released_workshop_packages.{target_id}")
}

/// Packages whose `ActiveModList` line the next apply on the target must still
/// remove although the library no longer names them: removing a subscribed
/// package deletes the only row that did.
pub async fn released_packages(
    db: &dyn ps_db::DbDriver,
    target_id: &str,
) -> Result<Vec<String>, ps_db::DbError> {
    Ok(ps_db::meta::get(db, &released_key(target_id))
        .await?
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default())
}

async fn write_released(
    db: &dyn ps_db::DbDriver,
    target_id: &str,
    packages: &[String],
) -> Result<(), ps_db::DbError> {
    let text = serde_json::to_string(packages).map_err(|e| ps_db::DbError::Other(e.to_string()))?;
    ps_db::meta::set(db, &released_key(target_id), &text).await
}

pub async fn release_workshop_packages(
    db: &dyn ps_db::DbDriver,
    mod_id: &str,
    target_ids: &[String],
) -> Result<(), ps_db::DbError> {
    let names = package_names(db, mod_id).await?;
    if names.is_empty() {
        return Ok(());
    }
    for target_id in target_ids {
        let mut released = released_packages(db, target_id).await?;
        let before = released.len();
        for name in &names {
            if !released.contains(name) {
                released.push(name.clone());
            }
        }
        if released.len() != before {
            write_released(db, target_id, &released).await?;
        }
    }
    Ok(())
}

pub async fn drop_released(db: &dyn ps_db::DbDriver, target_id: &str) -> Result<(), ps_db::DbError> {
    ps_db::meta::delete(db, &released_key(target_id)).await
}

/// Only the names an apply planned with are forgotten, so a release recorded
/// while it ran survives to the next one.
pub async fn forget_released(
    db: &dyn ps_db::DbDriver,
    target_id: &str,
    applied: &[String],
) -> Result<(), ps_db::DbError> {
    if applied.is_empty() {
        return Ok(());
    }
    let mut released = released_packages(db, target_id).await?;
    let before = released.len();
    released.retain(|name| !applied.contains(name));
    if released.len() != before {
        write_released(db, target_id, &released).await?;
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error("mod version {0} is already in the library")]
    AlreadyInstalled(String),
    #[error("mod {mod_id} is already in the library as a {existing} mod, not {incoming}")]
    ProvenanceConflict {
        mod_id: String,
        existing: String,
        incoming: String,
    },
    #[error("no free library directory for {0}")]
    NoFreeDirectory(String),
    #[error("route {0} is not in the extracted archive")]
    MissingRouteFile(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Db(#[from] ps_db::DbError),
}

pub struct StoreRequest<'a> {
    pub mod_id: &'a str,
    pub manifest: &'a InstallManifest,
    /// Root of the extracted tree; every route's `archive_path` resolves under it.
    pub extracted_root: &'a Path,
    /// The original archive, copied into the library when given.
    pub archive: Option<&'a Path>,
    pub source_kind: &'a str,
    /// Opaque JSON, stored verbatim.
    pub source_ref: &'a str,
    pub custom_name: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFile {
    pub kind: RouteKind,
    pub rel_path: String,
    pub hash: String,
    pub size: u64,
}

pub struct Stored {
    pub mod_row: ps_db::mod_library::ModRow,
    pub version: ps_db::mod_library::ModVersionRow,
    pub files: Vec<StoredFile>,
}

pub fn version_id(mod_id: &str, version: &str) -> String {
    format!("{mod_id}@{version}")
}

/// Measured at the path the row recorded, not at one recomputed from the current
/// app root: if the root moved, the bytes are still where they were written.
pub fn version_size(version: &ps_db::mod_library::ModVersionRow) -> std::io::Result<u64> {
    digest::dir_size(Path::new(&version.library_dir))
}

/// Copies the routed files into the library, keeps the archive, then writes the
/// rows. The row is written last on purpose: it is what makes a version real, and
/// a row pointing at a directory that does not exist would fail at deploy time
/// instead of here.
pub async fn store(
    db: &dyn ps_db::DbDriver,
    paths: &LibraryPaths,
    request: &StoreRequest<'_>,
) -> Result<Stored, LibraryError> {
    let manifest = request.manifest;
    let id = version_id(request.mod_id, &manifest.version);
    if ps_db::mod_library::get_version(db, &id).await?.is_some() {
        return Err(LibraryError::AlreadyInstalled(id));
    }

    let existing_mod = ps_db::mod_library::get_mod(db, request.mod_id).await?;
    if let Some(existing) = existing_mod.as_ref() {
        if existing.source_kind != request.source_kind {
            return Err(LibraryError::ProvenanceConflict {
                mod_id: request.mod_id.to_string(),
                existing: existing.source_kind.clone(),
                incoming: request.source_kind.to_string(),
            });
        }
    }

    let version_dir = choose_version_dir(db, paths, request.mod_id, &manifest.version).await?;
    let had_mod_row = existing_mod.is_some();
    match store_bytes_and_rows(db, paths, request, &id, &version_dir, existing_mod).await {
        Ok(stored) => Ok(stored),
        Err(error) => {
            // Everything this call created is removed, so a failure leaves neither
            // library bytes nor a row. The archive copy and both database writes sit
            // inside this arm, not only the file copy.
            let _ = std::fs::remove_dir_all(&version_dir);
            if !had_mod_row
                && ps_db::mod_library::versions_of(db, request.mod_id)
                    .await?
                    .is_empty()
            {
                let _ = ps_db::mod_library::remove_mod(db, request.mod_id).await;
            }
            Err(error)
        }
    }
}

/// A free directory for this version. `version_id` is keyed on the raw version
/// string while a directory name must be a sanitized slug, and the slug is lossy:
/// every version with no usable ASCII character maps to `unversioned`, which is
/// also the analyzer's fallback for an archive carrying no metadata. Two distinct
/// versions would otherwise share one directory, and deleting either would take
/// the other's bytes.
async fn choose_version_dir(
    db: &dyn ps_db::DbDriver,
    paths: &LibraryPaths,
    mod_id: &str,
    version: &str,
) -> Result<std::path::PathBuf, LibraryError> {
    let taken: std::collections::HashSet<String> = ps_db::mod_library::versions_of(db, mod_id)
        .await?
        .into_iter()
        .map(|row| row.library_dir)
        .collect();
    let base = paths.version_dir(mod_id, version);
    for attempt in 0..100u32 {
        let candidate = if attempt == 0 {
            base.clone()
        } else {
            std::path::PathBuf::from(format!("{}-{}", base.to_string_lossy(), attempt + 1))
        };
        if !taken.contains(candidate.to_string_lossy().as_ref()) && !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(LibraryError::NoFreeDirectory(format!("{mod_id}@{version}")))
}

async fn store_bytes_and_rows(
    db: &dyn ps_db::DbDriver,
    paths: &LibraryPaths,
    request: &StoreRequest<'_>,
    id: &str,
    version_dir: &Path,
    existing_mod: Option<ps_db::mod_library::ModRow>,
) -> Result<Stored, LibraryError> {
    let manifest = request.manifest;
    let files = copy_into_library(request, version_dir).await?;
    let archive_path = match request.archive {
        Some(source) => Some(keep_archive(paths, request.mod_id, source)?),
        None => None,
    };
    let manifest_json =
        serde_json::to_string(manifest).map_err(|e| LibraryError::Io(std::io::Error::other(e)))?;

    // `upsert_mod` replaces every column it is given, so the fields the user owns
    // are carried across from the existing row. Without this, installing an update
    // discards a rename, a note, and the flag that says to stop offering a version
    // — and surviving exactly this operation is that flag's entire purpose.
    let mod_row = ps_db::mod_library::upsert_mod(
        db,
        &ps_db::mod_library::NewMod {
            id: request.mod_id.to_string(),
            name: manifest.display_name.clone(),
            custom_name: request
                .custom_name
                .map(str::to_string)
                .or_else(|| existing_mod.as_ref().and_then(|m| m.custom_name.clone())),
            mod_type: mod_type_slug(manifest.mod_type).to_string(),
            author: manifest
                .source
                .author
                .clone()
                .or_else(|| existing_mod.as_ref().and_then(|m| m.author.clone())),
            summary: existing_mod.as_ref().and_then(|m| m.summary.clone()),
            source_kind: request.source_kind.to_string(),
            source_ref: request.source_ref.to_string(),
            nexus_mod_id: manifest.source.nexus_mod_id.map(i64::from),
            ignored_version: existing_mod
                .as_ref()
                .and_then(|m| m.ignored_version.clone()),
            notes: existing_mod.as_ref().and_then(|m| m.notes.clone()),
        },
    )
    .await?;

    let version = ps_db::mod_library::insert_version(
        db,
        &ps_db::mod_library::NewModVersion {
            id: id.to_string(),
            mod_id: request.mod_id.to_string(),
            version: manifest.version.clone(),
            archive_path: archive_path.clone(),
            library_dir: version_dir.to_string_lossy().into_owned(),
            manifest: manifest_json,
            source_ref: request.source_ref.to_string(),
        },
    )
    .await?;

    Ok(Stored {
        mod_row,
        version,
        files,
    })
}

async fn copy_into_library(
    request: &StoreRequest<'_>,
    version_dir: &Path,
) -> Result<Vec<StoredFile>, LibraryError> {
    std::fs::create_dir_all(version_dir)?;
    let mut files = Vec::with_capacity(request.manifest.routes.len());
    for route in &request.manifest.routes {
        let source = resolve_under(request.extracted_root, &route.archive_path);
        if !source.is_file() {
            return Err(LibraryError::MissingRouteFile(route.archive_path.clone()));
        }
        // Resolved against the directory that was chosen and will be recorded, not
        // against one recomputed from the version string: when a slug collision
        // forced a different name, recomputing writes into the other version's
        // directory and overwrites its files.
        let destination = LibraryPaths::route_path_in(version_dir, route.kind, &route.rel_path);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&source, &destination)?;
        files.push(StoredFile {
            kind: route.kind,
            rel_path: route.rel_path.clone(),
            hash: digest::hash_file(&destination)?,
            size: std::fs::metadata(&destination)?.len(),
        });
    }
    Ok(files)
}

fn keep_archive(paths: &LibraryPaths, mod_id: &str, source: &Path) -> Result<String, LibraryError> {
    let directory = paths.archives_dir(mod_id);
    std::fs::create_dir_all(&directory)?;
    let name = source
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("archive"));
    let destination = directory.join(name);
    std::fs::copy(source, &destination)?;
    Ok(destination.to_string_lossy().into_owned())
}

/// Joins a `/`-separated archive path onto a root one normal component at a time,
/// so a path the extractor should have rejected cannot reach outside the root
/// here either.
fn resolve_under(root: &Path, relative: &str) -> std::path::PathBuf {
    let mut path = root.to_path_buf();
    for segment in relative.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            continue;
        }
        path.push(segment);
    }
    path
}

fn mod_type_slug(mod_type: ps_core::mods::ModType) -> &'static str {
    use ps_core::mods::ModType;
    match mod_type {
        ModType::Ue4ss => "ue4ss",
        ModType::PalSchema => "palschema",
        ModType::Pak => "pak",
        ModType::LogicMods => "logicmods",
        ModType::NativeDll => "nativedll",
        ModType::Workshop => "workshop",
        ModType::Hybrid => "hybrid",
        ModType::Framework => "framework",
    }
}

/// The row is removed first: `ps_db::mod_library::remove_version` is what refuses
/// while the version is current, deployed, pinned, installed as a framework, or
/// covered by an open apply. Removing bytes first would destroy a deployed mod's
/// source and only then discover it was not allowed.
pub async fn delete_version(
    db: &dyn ps_db::DbDriver,
    version_id: &str,
) -> Result<(), LibraryError> {
    let Some(version) = ps_db::mod_library::get_version(db, version_id).await? else {
        return Err(LibraryError::Db(ps_db::DbError::Other(format!(
            "mod version {version_id} is not in the library"
        ))));
    };
    ps_db::mod_library::remove_version(db, version_id).await?;
    remove_tree(Path::new(&version.library_dir))
}

/// Takes the whole mod directory, archives included. A version deletion keeps the
/// archives because another version may have come from the same one and because an
/// archive is what allows a reinstall.
pub async fn delete_mod(
    db: &dyn ps_db::DbDriver,
    paths: &LibraryPaths,
    mod_id: &str,
) -> Result<(), LibraryError> {
    if ps_db::mod_library::get_mod(db, mod_id).await?.is_none() {
        return Err(LibraryError::Db(ps_db::DbError::Other(format!(
            "mod {mod_id} is not in the library"
        ))));
    }
    // Collected before the rows go, and read from the rows rather than recomputed:
    // if the app root moved since the install, the bytes are still where they were
    // written, and a recomputed path would find nothing and report success.
    let versions = ps_db::mod_library::versions_of(db, mod_id).await?;
    ps_db::mod_library::remove_mod(db, mod_id).await?;
    for version in &versions {
        remove_tree(Path::new(&version.library_dir))?;
        if let Some(archive) = version.archive_path.as_deref() {
            match std::fs::remove_file(archive) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(LibraryError::Io(error)),
            }
        }
    }
    remove_tree(&paths.mod_dir(mod_id))
}

/// A directory that is already gone is success: the row is what this operation is
/// really about, and an orphaned directory from an interrupted earlier delete must
/// not block the next one.
fn remove_tree(path: &Path) -> Result<(), LibraryError> {
    match std::fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(LibraryError::Io(error)),
    }
}
