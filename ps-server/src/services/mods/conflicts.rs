//! Assembling one profile's conflict report on demand: which enabled mods are
//! missing a framework or a Workshop dependency, which paks overwrite the
//! same asset, which PalSchema mods edit the same row, and which legacy paks
//! cannot run on a Game Pass target. The paks come from the enabled mods'
//! library routes and from the game's own pak folders on a local target.
//! Every directory listing and every byte read from a mod file happens inside
//! `spawn_blocking`; nothing parsed out of one becomes a filesystem path.
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use ps_core::mods::conflicts::{
    group_overlaps, iostore_sibling, is_pak_route, palschema_row_keys, required_frameworks,
    workshop_dependency, Dependency, PakRef, PakSource,
};
use ps_core::mods::pak_index::{self, PakIndexError};
use ps_core::mods::{
    parse_workshop_info, resolve_layout, FrameworkKey, InstallManifest, ModType, RouteKind,
    TargetKind,
};
use ps_db::mod_targets::ModTarget;
use serde::Serialize;

use super::frameworks::status::{self, StatusError};
use super::layout::{self, LayoutResolveError};
use super::library;
use super::paths::LibraryPaths;
use super::scan;

#[derive(Debug, thiserror::Error)]
pub enum ConflictError {
    #[error(transparent)]
    Db(#[from] ps_db::DbError),
    #[error(transparent)]
    Layout(#[from] LayoutResolveError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencySource {
    ModType,
    WorkshopInfo,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Conflict {
    MissingDependency {
        mod_id: String,
        dependency: String,
        source: DependencySource,
    },
    GamepassPakIncompatible {
        mod_id: String,
        files: Vec<String>,
    },
    PakOverlap {
        paks: Vec<PakRef>,
        winner: PakRef,
        assets: Vec<String>,
        asset_count: usize,
    },
    PalschemaRow {
        key: String,
        mods: Vec<PakRef>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct UnreadableEntry {
    pub mod_id: Option<String>,
    pub file: String,
    pub reason: &'static str,
    pub source: PakSource,
    pub path: Option<String>,
}

impl UnreadableEntry {
    fn library(mod_id: &str, file: impl Into<String>, reason: &'static str) -> Self {
        Self::of(PakRef::library(mod_id, file), reason)
    }

    fn of(pak: PakRef, reason: &'static str) -> Self {
        Self {
            mod_id: pak.mod_id,
            file: pak.file,
            reason,
            source: pak.source,
            path: pak.path,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ConflictReport {
    pub target_id: String,
    pub profile_id: String,
    pub platform: String,
    pub conflicts: Vec<Conflict>,
    pub unreadable: Vec<UnreadableEntry>,
}

struct EnabledMod {
    mod_id: String,
    manifest: InstallManifest,
    version_dir: PathBuf,
}

struct PakTask {
    pak: PakRef,
    path: PathBuf,
}

struct PalschemaTask {
    mod_id: String,
    file: String,
    path: PathBuf,
}

struct WorkshopTask {
    mod_id: String,
    path: PathBuf,
}

const BASE_GAME_PAK_PREFIXES: &[&str] = &["pakchunk", "pal-windows", "pal-wingdk"];

struct PakDirs {
    mods: PathBuf,
    logicmods: PathBuf,
    workshop: PathBuf,
}

enum WorkshopOwner {
    Enabled(String),
    NotEnabled,
}

/// What the blocking half needs to list and attribute paks found on disk.
struct DiskScan {
    dirs: PakDirs,
    deployed: HashSet<String>,
    workshop_owners: HashMap<String, WorkshopOwner>,
}

struct DiskPak {
    rel: String,
    file: String,
    path: PathBuf,
    workshop_folder: Option<String>,
}

#[derive(Default)]
struct DiskListing {
    paks: Vec<DiskPak>,
    /// Relative paths of directories that exist but could not be listed.
    unlisted: Vec<String>,
}

fn is_mod_pak(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".pak")
        && !BASE_GAME_PAK_PREFIXES
            .iter()
            .any(|prefix| lower.starts_with(prefix))
}

type DirEntries = Vec<(String, PathBuf, bool)>;

/// A directory's entries sorted by name, with `true` for a directory. A
/// symlinked directory is reported as not a directory, so the walk never
/// follows a link into another tree. A missing directory lists as empty; one
/// that exists but cannot be listed is recorded in `listing.unlisted`.
fn entries_of(dir: &Path, rel: &str, listing: &mut DiskListing) -> DirEntries {
    let read = match std::fs::read_dir(dir) {
        Ok(read) => read,
        Err(error) => {
            if error.kind() != std::io::ErrorKind::NotFound {
                listing.unlisted.push(rel.to_string());
            }
            return Vec::new();
        }
    };
    let mut out: DirEntries = read
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let is_dir = entry.file_type().ok()?.is_dir();
            Some((
                entry.file_name().to_string_lossy().into_owned(),
                entry.path(),
                is_dir,
            ))
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn push_paks(
    entries: &DirEntries,
    rel: &str,
    workshop_folder: Option<&str>,
    listing: &mut DiskListing,
) {
    for (name, path, is_dir) in entries {
        if !is_dir && is_mod_pak(name) && path.is_file() {
            listing.paks.push(DiskPak {
                rel: format!("{rel}/{name}"),
                file: name.clone(),
                path: path.clone(),
                workshop_folder: workshop_folder.map(str::to_string),
            });
        }
    }
}

fn list_disk_paks(dirs: &PakDirs) -> DiskListing {
    let mut listing = DiskListing::default();
    let mods = entries_of(&dirs.mods, "~mods", &mut listing);
    push_paks(&mods, "~mods", None, &mut listing);
    for (name, path, is_dir) in &mods {
        if *is_dir {
            let rel = format!("~mods/{name}");
            let entries = entries_of(path, &rel, &mut listing);
            push_paks(&entries, &rel, None, &mut listing);
        }
    }
    let logicmods = entries_of(&dirs.logicmods, "LogicMods", &mut listing);
    push_paks(&logicmods, "LogicMods", None, &mut listing);
    for (name, path, is_dir) in entries_of(&dirs.workshop, "~WorkshopMods", &mut listing) {
        if is_dir {
            let rel = format!("~WorkshopMods/{name}");
            let entries = entries_of(&path, &rel, &mut listing);
            push_paks(&entries, &rel, Some(&name), &mut listing);
        }
    }
    listing
}

fn has_iostore_siblings(pak: &Path) -> bool {
    pak.with_extension("utoc").is_file() && pak.with_extension("ucas").is_file()
}

fn disk_pak_tasks(scan: &DiskScan) -> (Vec<PakTask>, Vec<UnreadableEntry>) {
    let listing = list_disk_paks(&scan.dirs);
    let mut tasks = Vec::new();
    let mut unreadable: Vec<UnreadableEntry> = listing
        .unlisted
        .into_iter()
        .map(|rel| {
            let file = basename(&rel).to_string();
            UnreadableEntry::of(PakRef::disk(None, file, rel), "io")
        })
        .collect();
    for pak in listing.paks {
        if scan.deployed.contains(&scan::path_key(&pak.path)) {
            continue;
        }
        let mod_id = match &pak.workshop_folder {
            Some(folder) => match scan.workshop_owners.get(&folder.to_ascii_lowercase()) {
                Some(WorkshopOwner::Enabled(mod_id)) => Some(mod_id.clone()),
                Some(WorkshopOwner::NotEnabled) => continue,
                None => None,
            },
            None => None,
        };
        let pak_ref = PakRef::disk(mod_id, pak.file, pak.rel);
        if has_iostore_siblings(&pak.path) {
            unreadable.push(UnreadableEntry::of(pak_ref, "iostore"));
        } else {
            tasks.push(PakTask {
                pak: pak_ref,
                path: pak.path,
            });
        }
    }
    (tasks, unreadable)
}

/// `None` for a Docker target. `~WorkshopMods` has no layout field, so it is
/// taken beside the game's own pak directories under the root.
async fn disk_scan(
    db: &dyn ps_db::DbDriver,
    target: &ModTarget,
    profile_id: &str,
) -> Result<Option<DiskScan>, ConflictError> {
    let spec = layout::spec_for(target)?;
    if spec.kind == TargetKind::DockerServer {
        return Ok(None);
    }
    let resolved = resolve_layout(&spec).map_err(LayoutResolveError::from)?;
    let dirs = PakDirs {
        workshop: resolved
            .root
            .join("Pal")
            .join("Content")
            .join("Paks")
            .join("~WorkshopMods"),
        mods: resolved.paks_mods_dir,
        logicmods: resolved.logicmods_dir,
    };

    let deployed = ps_db::mod_deployments::files_of(db, &target.id)
        .await?
        .iter()
        .map(|row| scan::path_key(Path::new(&row.path)))
        .collect();

    let enabled: HashSet<String> = ps_db::mod_profiles::mods_of(db, profile_id)
        .await?
        .into_iter()
        .filter(|entry| entry.enabled)
        .map(|entry| entry.mod_id)
        .collect();
    let mut workshop_owners: HashMap<String, WorkshopOwner> = HashMap::new();
    for row in ps_db::mod_library::list_mods(db).await? {
        let is_enabled = enabled.contains(&row.id);
        for name in library::package_names(db, &row.id).await? {
            let key = name.to_ascii_lowercase();
            match workshop_owners.get(&key) {
                Some(WorkshopOwner::Enabled(_)) => {}
                Some(WorkshopOwner::NotEnabled) if !is_enabled => {}
                _ => {
                    let owner = if is_enabled {
                        WorkshopOwner::Enabled(row.id.clone())
                    } else {
                        WorkshopOwner::NotEnabled
                    };
                    workshop_owners.insert(key, owner);
                }
            }
        }
    }

    Ok(Some(DiskScan {
        dirs,
        deployed,
        workshop_owners,
    }))
}

/// The file name a route's own path ends in, ignoring any directory prefix.
fn basename(rel_path: &str) -> &str {
    rel_path.rsplit('/').next().unwrap_or(rel_path)
}

fn ends_with_ci(value: &str, suffix: &str) -> bool {
    value.len() >= suffix.len() && value[value.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
}

fn pak_unreadable_reason(error: &PakIndexError) -> &'static str {
    match error {
        PakIndexError::NotAPak => "not_a_pak",
        PakIndexError::UnsupportedVersion(_) => "unsupported_version",
        PakIndexError::Encrypted => "encrypted",
        PakIndexError::NoDirectoryIndex | PakIndexError::Malformed(_) => "malformed",
        PakIndexError::Io(_) => "io",
    }
}

fn read_pak_at(path: &Path) -> Result<pak_index::PakListing, PakIndexError> {
    let mut file =
        std::fs::File::open(path).map_err(|error| PakIndexError::Io(error.to_string()))?;
    pak_index::read_pak_listing(&mut file)
}

/// The enabled entries of `profile_id` resolved to a version and its parsed
/// manifest. An entry with no resolvable version, or whose manifest will not
/// parse, is skipped rather than failing the report: the apply step refuses
/// it elsewhere with a precise code.
async fn enabled_mods(
    db: &dyn ps_db::DbDriver,
    profile_id: &str,
) -> Result<Vec<EnabledMod>, ps_db::DbError> {
    let mut out = Vec::new();
    for entry in ps_db::mod_profiles::mods_of(db, profile_id).await? {
        if !entry.enabled {
            continue;
        }
        let version = match &entry.mod_version_id {
            Some(version_id) => ps_db::mod_library::get_version(db, version_id).await?,
            None => ps_db::mod_library::current_version(db, &entry.mod_id).await?,
        };
        let Some(version) = version else { continue };
        let Ok(manifest) = serde_json::from_str::<InstallManifest>(&version.manifest) else {
            continue;
        };
        out.push(EnabledMod {
            mod_id: entry.mod_id,
            manifest,
            version_dir: PathBuf::from(&version.library_dir),
        });
    }
    Ok(out)
}

/// Whether `key` is reported present by framework detection; `None` (a Docker
/// target) skips the check entirely rather than reporting anything missing.
fn is_present(detection: &Option<status::Detection>, key: FrameworkKey) -> Option<bool> {
    detection.as_ref().map(|detection| {
        // A key with no entry reads as "not installed"; that's only correct
        // because `detect` always pushes every `FrameworkKey::ALL`.
        detection
            .installed
            .iter()
            .find(|(installed_key, _)| *installed_key == key)
            .is_some_and(|(_, installed)| installed.present)
    })
}

pub async fn report(
    db: &dyn ps_db::DbDriver,
    target: &ModTarget,
    profile_id: &str,
) -> Result<ConflictReport, ConflictError> {
    let enabled = enabled_mods(db, profile_id).await?;
    let disk = disk_scan(db, target, profile_id).await?;

    let detection = match status::detect(db, target).await {
        Ok(detection) => Some(detection),
        Err(StatusError::Docker) => None,
        Err(StatusError::Db(error)) => return Err(ConflictError::Db(error)),
        Err(StatusError::Layout(error)) => return Err(ConflictError::Layout(error)),
    };

    let is_gamepass = target.platform == "wingdk";
    let mut enabled_workshop_packages = std::collections::BTreeSet::new();
    for enabled_mod in &enabled {
        if enabled_mod.manifest.mod_type == ModType::Workshop {
            enabled_workshop_packages.insert(enabled_mod.manifest.folder_name.clone());
        }
    }

    let mut missing: Vec<(String, String, DependencySource)> = Vec::new();
    let mut gamepass: Vec<(String, Vec<String>)> = Vec::new();
    let mut pak_tasks: Vec<PakTask> = Vec::new();
    let mut palschema_tasks: Vec<PalschemaTask> = Vec::new();
    let mut workshop_tasks: Vec<WorkshopTask> = Vec::new();
    let mut unreadable: Vec<UnreadableEntry> = Vec::new();
    let mut workshop_dir: Option<Option<PathBuf>> = None;

    for enabled_mod in &enabled {
        let manifest = &enabled_mod.manifest;

        for key in required_frameworks(manifest.mod_type) {
            if is_present(&detection, *key) == Some(false) {
                missing.push((
                    enabled_mod.mod_id.clone(),
                    key.display_name().to_string(),
                    DependencySource::ModType,
                ));
            }
        }

        let mut legacy_files = Vec::new();
        for route in &manifest.routes {
            if !is_pak_route(route) {
                continue;
            }
            let file = basename(&route.rel_path).to_string();
            let path =
                LibraryPaths::route_path_in(&enabled_mod.version_dir, route.kind, &route.rel_path);
            if iostore_sibling(manifest, route).is_some() {
                unreadable.push(UnreadableEntry::library(
                    &enabled_mod.mod_id,
                    file.clone(),
                    "iostore",
                ));
            } else {
                pak_tasks.push(PakTask {
                    pak: PakRef::library(enabled_mod.mod_id.clone(), file.clone()),
                    path,
                });
                if is_gamepass {
                    legacy_files.push(file);
                }
            }
        }
        if !legacy_files.is_empty() {
            legacy_files.sort();
            gamepass.push((enabled_mod.mod_id.clone(), legacy_files));
        }

        for route in &manifest.routes {
            if route.kind != RouteKind::PalSchema {
                continue;
            }
            if !(ends_with_ci(&route.rel_path, ".json") || ends_with_ci(&route.rel_path, ".jsonc"))
            {
                continue;
            }
            let path =
                LibraryPaths::route_path_in(&enabled_mod.version_dir, route.kind, &route.rel_path);
            palschema_tasks.push(PalschemaTask {
                mod_id: enabled_mod.mod_id.clone(),
                file: route.rel_path.clone(),
                path,
            });
        }

        if manifest.mod_type == ModType::Workshop {
            let info_route = manifest.routes.iter().find(|route| {
                route.kind == RouteKind::Workshop && basename(&route.rel_path) == "Info.json"
            });
            match info_route {
                Some(route) => {
                    let path = LibraryPaths::route_path_in(
                        &enabled_mod.version_dir,
                        route.kind,
                        &route.rel_path,
                    );
                    workshop_tasks.push(WorkshopTask {
                        mod_id: enabled_mod.mod_id.clone(),
                        path,
                    });
                }
                None => {
                    let item_id = enabled_mod
                        .mod_id
                        .strip_prefix("workshop-")
                        .filter(|id| library::is_workshop_item_id(id));
                    if let Some(item_id) = item_id {
                        if workshop_dir.is_none() {
                            workshop_dir =
                                Some(scan::steam_workshop_dir(db, target).await.ok().flatten());
                        }
                        if let Some(Some(dir)) = &workshop_dir {
                            workshop_tasks.push(WorkshopTask {
                                mod_id: enabled_mod.mod_id.clone(),
                                path: dir.join(item_id).join("Info.json"),
                            });
                        }
                    }
                }
            }
        }
    }

    let (
        pak_entries,
        mut pak_unreadable,
        palschema_entries,
        mut palschema_unreadable,
        workshop_infos,
        mut workshop_unreadable,
    ) = tokio::task::spawn_blocking(move || {
        read_all(pak_tasks, palschema_tasks, workshop_tasks, disk)
    })
    .await
    .map_err(|error| std::io::Error::other(error.to_string()))?;
    unreadable.append(&mut pak_unreadable);
    unreadable.append(&mut palschema_unreadable);
    unreadable.append(&mut workshop_unreadable);

    for (mod_id, info) in &workshop_infos {
        for dependency in &info.dependencies {
            match workshop_dependency(dependency) {
                Dependency::Framework(key) => {
                    if is_present(&detection, key) == Some(false) {
                        missing.push((
                            mod_id.clone(),
                            key.display_name().to_string(),
                            DependencySource::WorkshopInfo,
                        ));
                    }
                }
                Dependency::Package(name) => {
                    if !name.is_empty() && !enabled_workshop_packages.contains(&name) {
                        missing.push((mod_id.clone(), name, DependencySource::WorkshopInfo));
                    }
                }
            }
        }
    }

    missing.sort();
    let mut conflicts: Vec<Conflict> = missing
        .into_iter()
        .map(|(mod_id, dependency, source)| Conflict::MissingDependency {
            mod_id,
            dependency,
            source,
        })
        .collect();

    gamepass.sort_by(|a, b| a.0.cmp(&b.0));
    conflicts.extend(
        gamepass
            .into_iter()
            .map(|(mod_id, files)| Conflict::GamepassPakIncompatible { mod_id, files }),
    );

    conflicts.extend(group_overlaps(&pak_entries).into_iter().map(|overlap| {
        Conflict::PakOverlap {
            paks: overlap.paks,
            winner: overlap.winner,
            assets: overlap.assets,
            asset_count: overlap.asset_count,
        }
    }));

    let mut rows: BTreeMap<String, Vec<PakRef>> = BTreeMap::new();
    for (key, pak_ref) in palschema_entries {
        let entry = rows.entry(key).or_default();
        if !entry.contains(&pak_ref) {
            entry.push(pak_ref);
        }
    }
    for (key, mut mods) in rows {
        if mods.len() < 2 {
            continue;
        }
        mods.sort_by(|a, b| {
            (a.mod_id.as_deref(), a.file.as_str()).cmp(&(b.mod_id.as_deref(), b.file.as_str()))
        });
        conflicts.push(Conflict::PalschemaRow { key, mods });
    }

    unreadable.sort_by(|a, b| {
        (a.mod_id.as_deref(), a.file.as_str(), a.path.as_deref()).cmp(&(
            b.mod_id.as_deref(),
            b.file.as_str(),
            b.path.as_deref(),
        ))
    });

    Ok(ConflictReport {
        target_id: target.id.clone(),
        profile_id: profile_id.to_string(),
        platform: target.platform.clone(),
        conflicts,
        unreadable,
    })
}

type ReadAllResult = (
    Vec<(PakRef, Vec<String>)>,
    Vec<UnreadableEntry>,
    Vec<(String, PakRef)>,
    Vec<UnreadableEntry>,
    Vec<(String, ps_core::mods::WorkshopInfo)>,
    Vec<UnreadableEntry>,
);

/// The synchronous half of gathering: the pak folders on disk are listed, and
/// every pak, PalSchema file and Workshop `Info.json` this report needs is
/// read here, inside one blocking task.
fn read_all(
    pak_tasks: Vec<PakTask>,
    palschema_tasks: Vec<PalschemaTask>,
    workshop_tasks: Vec<WorkshopTask>,
    disk: Option<DiskScan>,
) -> ReadAllResult {
    let mut pak_tasks = pak_tasks;
    let mut pak_unreadable = Vec::new();
    if let Some(scan) = &disk {
        let (tasks, unreadable) = disk_pak_tasks(scan);
        pak_tasks.extend(tasks);
        pak_unreadable = unreadable;
    }
    let mut pak_entries = Vec::new();
    for task in pak_tasks {
        match read_pak_at(&task.path) {
            Ok(listing) => {
                let assets: Vec<String> = listing
                    .files
                    .iter()
                    .map(|file| pak_index::asset_key(&listing.mount_point, file))
                    .collect();
                pak_entries.push((task.pak, assets));
            }
            Err(error) => {
                pak_unreadable.push(UnreadableEntry::of(task.pak, pak_unreadable_reason(&error)))
            }
        }
    }

    let mut palschema_entries = Vec::new();
    let mut palschema_unreadable = Vec::new();
    for task in palschema_tasks {
        let text = match std::fs::read_to_string(&task.path) {
            Ok(text) => text,
            Err(_) => {
                palschema_unreadable.push(UnreadableEntry::library(&task.mod_id, task.file, "io"));
                continue;
            }
        };
        match palschema_row_keys(&task.file, &text) {
            Ok(keys) => {
                for key in keys {
                    palschema_entries
                        .push((key, PakRef::library(task.mod_id.clone(), task.file.clone())));
                }
            }
            Err(_) => palschema_unreadable.push(UnreadableEntry::library(
                &task.mod_id,
                task.file,
                "invalid_json",
            )),
        }
    }

    let mut workshop_infos = Vec::new();
    let mut workshop_unreadable = Vec::new();
    for task in workshop_tasks {
        let text = match std::fs::read_to_string(&task.path) {
            Ok(text) => text,
            Err(_) => {
                workshop_unreadable.push(UnreadableEntry::library(&task.mod_id, "Info.json", "io"));
                continue;
            }
        };
        match parse_workshop_info(&text) {
            Ok(info) => workshop_infos.push((task.mod_id, info)),
            Err(_) => workshop_unreadable.push(UnreadableEntry::library(
                &task.mod_id,
                "Info.json",
                "invalid_json",
            )),
        }
    }

    (
        pak_entries,
        pak_unreadable,
        palschema_entries,
        palschema_unreadable,
        workshop_infos,
        workshop_unreadable,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basename_drops_a_directory_prefix() {
        assert_eq!(basename("CoolPkg/Info.json"), "Info.json");
        assert_eq!(basename("Info.json"), "Info.json");
    }

    #[test]
    fn ends_with_ci_matches_regardless_of_case() {
        assert!(ends_with_ci("Mod_P.PAK", ".pak"));
        assert!(ends_with_ci("mod.jsonc", ".jsonc"));
        assert!(!ends_with_ci("mod.json", ".jsonc"));
        assert!(!ends_with_ci("pak", ".pak"));
    }

    #[test]
    fn pak_reasons_cover_every_pak_index_error() {
        assert_eq!(pak_unreadable_reason(&PakIndexError::NotAPak), "not_a_pak");
        assert_eq!(
            pak_unreadable_reason(&PakIndexError::UnsupportedVersion(9)),
            "unsupported_version"
        );
        assert_eq!(
            pak_unreadable_reason(&PakIndexError::Encrypted),
            "encrypted"
        );
        assert_eq!(
            pak_unreadable_reason(&PakIndexError::NoDirectoryIndex),
            "malformed"
        );
        assert_eq!(
            pak_unreadable_reason(&PakIndexError::Malformed("x")),
            "malformed"
        );
        assert_eq!(
            pak_unreadable_reason(&PakIndexError::Io("x".to_string())),
            "io"
        );
    }

    #[test]
    fn a_base_game_or_non_pak_name_is_not_a_mod_pak() {
        assert!(is_mod_pak("Cool_P.pak"));
        assert!(is_mod_pak("Cool_P.PAK"));
        assert!(!is_mod_pak("pakchunk0-Windows.pak"));
        assert!(!is_mod_pak("Pal-Windows.pak"));
        assert!(!is_mod_pak("Pal-WinGDK.pak"));
        assert!(!is_mod_pak("Cool_P.utoc"));
    }

    #[test]
    fn listing_labels_each_location_and_sorts_by_name() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = PakDirs {
            mods: dir.path().join("~mods"),
            logicmods: dir.path().join("LogicMods"),
            workshop: dir.path().join("~WorkshopMods"),
        };
        for rel in [
            "~mods/b_P.pak",
            "~mods/a_P.pak",
            "~WorkshopMods/F/w_P.pak",
            "LogicMods/l_P.pak",
        ] {
            let path = dir.path().join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"x").unwrap();
        }
        let rels: Vec<(String, Option<String>)> = list_disk_paks(&dirs)
            .paks
            .into_iter()
            .map(|pak| (pak.rel, pak.workshop_folder))
            .collect();
        assert_eq!(
            rels,
            vec![
                ("~mods/a_P.pak".to_string(), None),
                ("~mods/b_P.pak".to_string(), None),
                ("LogicMods/l_P.pak".to_string(), None),
                ("~WorkshopMods/F/w_P.pak".to_string(), Some("F".to_string())),
            ]
        );
    }

    #[test]
    fn a_pak_folder_that_cannot_be_listed_is_unreadable_but_a_missing_one_is_not() {
        let dir = tempfile::tempdir().unwrap();
        let scan = DiskScan {
            dirs: PakDirs {
                mods: dir.path().join("~mods"),
                logicmods: dir.path().join("LogicMods"),
                workshop: dir.path().join("~WorkshopMods"),
            },
            deployed: HashSet::new(),
            workshop_owners: HashMap::new(),
        };
        std::fs::create_dir_all(&scan.dirs.mods).unwrap();
        std::fs::write(scan.dirs.mods.join("a_P.pak"), b"x").unwrap();
        std::fs::write(&scan.dirs.logicmods, b"a file, not a directory").unwrap();

        let (tasks, unreadable) = disk_pak_tasks(&scan);

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].pak.path.as_deref(), Some("~mods/a_P.pak"));
        assert_eq!(unreadable.len(), 1);
        let entry = &unreadable[0];
        assert_eq!(entry.reason, "io");
        assert_eq!(entry.mod_id, None);
        assert_eq!(entry.source, PakSource::Disk);
        assert_eq!(entry.path.as_deref(), Some("LogicMods"));
        assert_eq!(entry.file, "LogicMods");
    }

    #[tokio::test]
    async fn a_blocking_read_panic_maps_to_a_conflict_error_instead_of_unwinding() {
        let result: Result<(), std::io::Error> = tokio::task::spawn_blocking(|| {
            panic!("simulated blocking read panic");
        })
        .await
        .map_err(|error| std::io::Error::other(error.to_string()));
        let error: ConflictError = result.unwrap_err().into();

        assert!(matches!(error, ConflictError::Io(_)));
    }
}
