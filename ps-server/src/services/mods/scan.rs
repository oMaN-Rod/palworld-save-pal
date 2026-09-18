use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use ps_core::mods::{
    normalize_physical_path, InstallManifest, ModsTxt, PalModSettings, RouteKind, TargetKind,
    TargetLayout,
};
use ps_db::mod_targets::ModTarget;

use super::digest;
use super::layout::{self, LayoutResolveError};
use super::library;

pub const PALMODSETTINGS_UNREADABLE: &str = "palmodsettings_unreadable";

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error(transparent)]
    Layout(#[from] LayoutResolveError),
    #[error(transparent)]
    Db(#[from] ps_db::DbError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileState {
    ManagedIntact,
    ManagedDrifted,
    /// A recorded path that is no longer on disk. Distinct from drifted because the
    /// deployer re-adds a missing file and preserves an edited one.
    ManagedMissing,
    /// Present but unreadable — a permission error on a mounted host directory, a
    /// file another process holds open. Kept apart from missing because the
    /// deployer re-adds a missing file, and re-adding over a file that is merely
    /// unreadable would overwrite content nothing has inspected.
    ManagedUnreadable,
    Unmanaged,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ScannedFile {
    pub path: String,
    pub kind: Option<RouteKind>,
    pub state: FileState,
    pub mod_version_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateSource {
    Local,
    SteamSubscribed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AdoptionCandidate {
    /// The mod's identity: a folder name, a file name, or for a Workshop package
    /// the `PackageName` from its `Info.json`, which is what `ActiveModList` holds.
    pub name: String,
    pub kind: RouteKind,
    /// The folder or file this candidate is, absolute.
    pub root: String,
    /// Empty for a Steam-subscribed package: Steam owns those files.
    pub files: Vec<String>,
    pub enabled: bool,
    pub source: CandidateSource,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ScanReport {
    pub target_id: String,
    pub files: Vec<ScannedFile>,
    pub candidates: Vec<AdoptionCandidate>,
    pub drifted: Vec<String>,
    pub missing: Vec<String>,
    pub unreadable: Vec<String>,
    pub warnings: Vec<String>,
}

/// One layout location and the kind of thing that lives in it, plus whether an
/// entry there is a directory (a mod folder) or a single file (a pak, a DLL).
struct Location {
    dir: PathBuf,
    kind: RouteKind,
    folders: bool,
}

/// `Path` equality is case-sensitive on Windows, so two spellings of one file
/// share this key instead.
pub(crate) fn path_key(path: &Path) -> String {
    normalize_physical_path(
        &path.to_string_lossy(),
        cfg!(any(windows, target_os = "macos")),
    )
}

/// Two locations can resolve to one directory — a docker target may mount the same
/// host path for `~mods` and `LogicMods` — and without de-duplication every file
/// there would be walked twice, reported twice, and offered as two candidates with
/// two different mod ids. The first kind in this order wins.
fn locations(layout: &TargetLayout) -> Vec<Location> {
    let mut out: Vec<Location> = Vec::new();
    let mut push = |dir: PathBuf, kind: RouteKind, folders: bool| {
        if out.iter().any(|existing| existing.dir == dir) {
            return;
        }
        out.push(Location { dir, kind, folders });
    };
    if let Some(dir) = layout.ue4ss_mods_dir.clone() {
        push(dir, RouteKind::Ue4ss, true);
    }
    if let Some(dir) = layout.palschema_mods_dir.clone() {
        push(dir, RouteKind::PalSchema, true);
    }
    push(layout.paks_mods_dir.clone(), RouteKind::Pak, false);
    push(layout.logicmods_dir.clone(), RouteKind::LogicMods, false);
    if let Some(dir) = layout.nativemods_dir.clone() {
        push(dir, RouteKind::NativeDll, false);
    }
    if let Some(dir) = layout.workshop_local_dir.clone() {
        push(dir, RouteKind::Workshop, true);
    }
    out
}

/// Every file under `root`, following a symlink when deciding whether an entry is
/// a file so a linked-in shared library is not silently dropped, while not
/// following links during traversal so a cycle cannot hang the walk.
fn files_under(root: &Path) -> Vec<PathBuf> {
    walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .map(|entry| entry.path().to_path_buf())
        .filter(|path| path.is_file())
        .collect()
}

/// A Workshop package is identified by the `PackageName` in its `Info.json`, not by
/// the directory it happens to sit in: `ActiveModList` holds the package name, and
/// a hand-extracted package is routinely in a folder named for its download.
fn workshop_package_name(folder: &Path) -> Option<String> {
    let text = std::fs::read_to_string(folder.join("Info.json")).ok()?;
    let info = ps_core::mods::parse_workshop_info(&text).ok()?;
    let name = info.package_name.trim().to_string();
    (!name.is_empty()).then_some(name)
}

/// The enable markers, read from disk so adoption can reproduce them exactly.
/// UE4SS is the only kind with a marker file; PalSchema, paks, LogicMods and
/// NativeMods DLLs are enabled by presence; a Workshop package is enabled by its
/// `ActiveModList` line, matched exactly as the settings writer matches it.
pub fn enabled_state(layout: &TargetLayout, kind: RouteKind, name: &str) -> bool {
    match kind {
        RouteKind::Ue4ss => {
            if let Some(mods_txt) = layout.mods_txt.as_deref() {
                if let Ok(Some(file)) = super::ini_text::read(mods_txt) {
                    // Case-insensitively, because `ModsTxt::set_managed` drops
                    // existing lines that way when it writes. A reader stricter
                    // than the writer lets an apply flip a mod the user turned off.
                    if let Some((_, enabled)) = ModsTxt::parse(&file.text)
                        .entries()
                        .into_iter()
                        .find(|(entry, _)| entry.eq_ignore_ascii_case(name))
                    {
                        return enabled;
                    }
                }
            }
            layout
                .ue4ss_mods_dir
                .as_ref()
                .map(|dir| dir.join(name).join("enabled.txt").is_file())
                .unwrap_or(false)
        }
        RouteKind::Workshop => layout
            .palmodsettings_ini
            .as_deref()
            .and_then(|ini| super::ini_text::read(ini).ok().flatten())
            .map(|file| {
                let settings = PalModSettings::parse(&file.text);
                settings.enabled && settings.active_mods.iter().any(|entry| entry == name)
            })
            .unwrap_or(false),
        // PalSchema, Pak, LogicMods and NativeDll are enabled by presence.
        _ => true,
    }
}

/// Steam keeps a game's Workshop content in the library holding the game, so a
/// root of `{library}/steamapps/common/{game}` pairs with
/// `{library}/steamapps/workshop/content/1623730`.
fn steam_library_workshop_dir(root: &Path) -> Option<PathBuf> {
    let named = |path: &Path, name: &str| {
        path.file_name()
            .is_some_and(|leaf| leaf.to_string_lossy().eq_ignore_ascii_case(name))
    };
    let common = root.parent()?;
    let steamapps = common.parent()?;
    (root.file_name().is_some() && named(common, "common") && named(steamapps, "steamapps")).then(
        || {
            steamapps
                .join("workshop")
                .join("content")
                .join(super::settings::WORKSHOP_APP_ID)
        },
    )
}

/// Where Steam keeps this target's subscribed packages. A Docker server has none:
/// no Steam client runs in the container.
pub async fn steam_workshop_dir(
    db: &dyn ps_db::DbDriver,
    target: &ModTarget,
) -> Result<Option<PathBuf>, ScanError> {
    let resolved = layout::layout_for(target)?;
    workshop_dir_in(db, target, &resolved).await
}

async fn workshop_dir_in(
    db: &dyn ps_db::DbDriver,
    target: &ModTarget,
    layout: &TargetLayout,
) -> Result<Option<PathBuf>, ScanError> {
    let known = match layout::spec_for(target)?.kind {
        TargetKind::DockerServer => return Ok(None),
        TargetKind::Client => steam_library_workshop_dir(&layout.root),
        TargetKind::NativeServer => match target.server_id {
            Some(server_id) => ps_db::servers::get_server(db, server_id)
                .await?
                .map(|record| record.workshop_dir.trim().to_string())
                .filter(|dir| !dir.is_empty())
                .map(PathBuf::from),
            None => None,
        },
    };
    if known.is_some() {
        return Ok(known);
    }
    Ok(layout
        .palmodsettings_ini
        .as_deref()
        .and_then(|ini| super::ini_text::read(ini).ok().flatten())
        .map(|file| {
            PalModSettings::parse(&file.text)
                .workshop_root_dir
                .trim()
                .to_string()
        })
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from))
}

/// Package names a subscribed candidate on this target must not repeat: every
/// Workshop package PalStudio installed, and every subscribed package a profile
/// of this target already holds.
async fn managed_packages(
    db: &dyn ps_db::DbDriver,
    target_id: &str,
) -> Result<Vec<String>, ScanError> {
    let held = library::held_by_target(db, target_id).await?;
    let mut names = Vec::new();
    for row in ps_db::mod_library::list_mods(db).await? {
        if row.mod_type != "workshop" || (library::is_subscribed(&row) && !held.contains(&row.id)) {
            continue;
        }
        for version in ps_db::mod_library::versions_of(db, &row.id).await? {
            if let Ok(manifest) = serde_json::from_str::<InstallManifest>(&version.manifest) {
                names.push(manifest.folder_name);
            }
        }
    }
    Ok(names)
}

/// Read-only, and only as deep as each item's `Info.json`. A linked item folder
/// is skipped rather than followed.
fn subscribed_candidates(
    layout: &TargetLayout,
    steam_dir: &Path,
    local_folders: &HashSet<String>,
    mut taken: Vec<String>,
) -> Vec<AdoptionCandidate> {
    let Ok(entries) = std::fs::read_dir(steam_dir) else {
        return Vec::new();
    };
    let mut folders: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|entry| library::is_workshop_item_id(&entry.file_name().to_string_lossy()))
        .map(|entry| entry.path())
        .filter(|path| std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_dir()))
        .collect();
    folders.sort();
    let mut out = Vec::new();
    for folder in folders {
        if local_folders.contains(&path_key(&folder)) {
            continue;
        }
        let Some(name) = workshop_package_name(&folder) else {
            continue;
        };
        if taken.contains(&name) {
            continue;
        }
        taken.push(name.clone());
        out.push(AdoptionCandidate {
            enabled: enabled_state(layout, RouteKind::Workshop, &name),
            name,
            kind: RouteKind::Workshop,
            root: folder.to_string_lossy().into_owned(),
            files: Vec::new(),
            source: CandidateSource::SteamSubscribed,
        });
    }
    out
}

pub async fn scan(db: &dyn ps_db::DbDriver, target: &ModTarget) -> Result<ScanReport, ScanError> {
    scan_with(db, target, false).await
}

/// With `candidates_only`, recorded rows are not hashed and `files`, `drifted`,
/// `missing` and `unreadable` stay empty.
pub async fn scan_with(
    db: &dyn ps_db::DbDriver,
    target: &ModTarget,
    candidates_only: bool,
) -> Result<ScanReport, ScanError> {
    let resolved = layout::layout_for(target)?;

    // Keyed on the normalised path: a recorded path and the same file as
    // `read_dir` rebuilds it can differ in separators — a docker target's
    // `root_path` is stored forward-slashed — or only in case.
    let recorded: HashMap<String, ps_db::mod_deployments::DeploymentFile> =
        ps_db::mod_deployments::files_of(db, &target.id)
            .await?
            .into_iter()
            .map(|row| (path_key(Path::new(&row.path)), row))
            .collect();

    let mut files: Vec<ScannedFile> = Vec::new();

    // Every recorded row is classified from its own path, independently of the
    // directory walk. A recorded file can legitimately sit outside every mod
    // location — a framework DLL beside the executable, `PalModSettings.ini`, a
    // shared marker — and classifying those only when the walk happened to reach
    // them reported present, intact files as missing forever.
    if !candidates_only {
        for row in recorded.values() {
            let path = Path::new(&row.path);
            let state = if !path.exists() {
                FileState::ManagedMissing
            } else {
                match digest::hash_file(path) {
                    Ok(hash) if hash == row.hash => FileState::ManagedIntact,
                    Ok(_) => FileState::ManagedDrifted,
                    Err(_) => FileState::ManagedUnreadable,
                }
            };
            files.push(ScannedFile {
                path: row.path.clone(),
                kind: None,
                state,
                mod_version_id: row.mod_version_id.clone(),
            });
        }
    }

    let all_locations = locations(&resolved);
    // Locations nest: `palschema_mods_dir` lives under `ue4ss_mods_dir`. Only the
    // nested location's own subtree is skipped, not the whole entry containing it,
    // because the framework's files sit beside that subtree and are managed too.
    let nested: Vec<PathBuf> = all_locations.iter().map(|l| l.dir.clone()).collect();

    let mut candidates: Vec<AdoptionCandidate> = Vec::new();
    let mut local_workshop_folders: HashSet<String> = HashSet::new();
    let mut local_workshop_names: Vec<String> = Vec::new();
    for location in &all_locations {
        if !location.dir.is_dir() {
            continue;
        }
        let entries = match std::fs::read_dir(&location.dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            let is_dir = path.is_dir();
            if !is_dir && is_marker_file(&resolved, &path) {
                continue;
            }

            let package = (location.kind == RouteKind::Workshop && is_dir).then(|| {
                let name = workshop_package_name(&path)
                    .unwrap_or_else(|| entry.file_name().to_string_lossy().into_owned());
                local_workshop_folders.insert(path_key(&path));
                local_workshop_names.push(name.clone());
                name
            });

            let group: Vec<PathBuf> = if is_dir {
                files_under(&path)
                    .into_iter()
                    // A file belonging to a location nested *inside* this one is
                    // that location's. The test must be for a strict descendant:
                    // `palschema_mods_dir` sits under `ue4ss_mods_dir`, so a plain
                    // "under any other location" test would also exclude every
                    // PalSchema file from the PalSchema location itself.
                    .filter(|file| {
                        !nested.iter().any(|dir| {
                            dir != &location.dir
                                && dir.starts_with(&location.dir)
                                && file.starts_with(dir)
                        })
                    })
                    .collect()
            } else {
                vec![path.clone()]
            };

            let mut unmanaged = 0usize;
            for file in &group {
                if recorded.contains_key(&path_key(file)) {
                    continue;
                }
                unmanaged += 1;
                if !candidates_only {
                    files.push(ScannedFile {
                        path: file.to_string_lossy().into_owned(),
                        kind: Some(location.kind),
                        state: FileState::Unmanaged,
                        mod_version_id: None,
                    });
                }
            }

            // A candidate is an entry of the shape this location holds, none of
            // whose files the app already manages. A mod that is *partly* managed
            // is never offered: adopting it would re-point the rows the app owns.
            if group.is_empty() || unmanaged != group.len() {
                continue;
            }
            if location.folders != is_dir {
                continue;
            }
            let name =
                package.unwrap_or_else(|| entry.file_name().to_string_lossy().into_owned());
            candidates.push(AdoptionCandidate {
                enabled: enabled_state(&resolved, location.kind, &name),
                name,
                kind: location.kind,
                root: path.to_string_lossy().into_owned(),
                files: group
                    .iter()
                    .map(|f| f.to_string_lossy().into_owned())
                    .collect(),
                source: CandidateSource::Local,
            });
        }
    }

    // Subscribed enabled states come from the settings file, so a file that cannot
    // be decoded withholds them rather than reporting every package disabled.
    let mut warnings = Vec::new();
    let ini_unreadable = resolved
        .palmodsettings_ini
        .as_deref()
        .is_some_and(|ini| super::ini_text::read(ini).is_err());
    if ini_unreadable {
        warnings.push(PALMODSETTINGS_UNREADABLE.to_string());
        candidates.retain(|candidate| candidate.kind != RouteKind::Workshop);
    } else if let Some(steam_dir) = workshop_dir_in(db, target, &resolved).await? {
        let mut taken = local_workshop_names;
        taken.extend(managed_packages(db, &target.id).await?);
        candidates.extend(subscribed_candidates(
            &resolved,
            &steam_dir,
            &local_workshop_folders,
            taken,
        ));
    }

    // A file can be reached by the walk and also be recorded; the recorded pass
    // already reported it, so drop the duplicate the walk would add.
    let mut seen: HashSet<String> = HashSet::new();
    files.retain(|file| seen.insert(path_key(Path::new(&file.path))));

    files.sort_by(|a, b| a.path.cmp(&b.path));
    candidates.sort_by(|a, b| a.name.cmp(&b.name));
    let drifted = state_paths(&files, FileState::ManagedDrifted);
    let missing = state_paths(&files, FileState::ManagedMissing);
    let unreadable = state_paths(&files, FileState::ManagedUnreadable);
    Ok(ScanReport {
        target_id: target.id.clone(),
        files,
        candidates,
        drifted,
        missing,
        unreadable,
        warnings,
    })
}

fn state_paths(files: &[ScannedFile], state: FileState) -> Vec<String> {
    files
        .iter()
        .filter(|f| f.state == state)
        .map(|f| f.path.clone())
        .collect()
}

/// A marker the engine writes rather than a mod's own file. Reporting `mods.txt`
/// as an unmanaged stray would offer it for adoption.
fn is_marker_file(layout: &TargetLayout, path: &Path) -> bool {
    [
        layout.mods_txt.as_deref(),
        layout.palmodsettings_ini.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|marker| marker == path)
}
