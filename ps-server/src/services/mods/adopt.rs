use std::path::Path;

use ps_core::mods::{
    mod_id, FileRoute, InstallManifest, ModIdInput, ModType, RouteKind, SourceHint,
};
use ps_db::mod_targets::ModTarget;

use super::layout::{self, LayoutResolveError};
use super::library::{self, LibraryError};
use super::paths::LibraryPaths;
use super::scan::{AdoptionCandidate, CandidateSource};

/// The one version a subscribed package has: Steam updates its files in place,
/// so there is never a second to switch to.
pub const SUBSCRIBED_VERSION: &str = "steam-subscribed";

#[derive(Debug, thiserror::Error)]
pub enum AdoptError {
    #[error("the library already manages a Workshop package named {0}")]
    AlreadyManaged(String),
    #[error("{0} is not a Steam Workshop item folder")]
    NotAWorkshopItem(String),
    #[error("candidate {0} has no files")]
    Empty(String),
    #[error("target {0} has no active profile")]
    NoActiveProfile(String),
    #[error("{0} is not under the target's {1} directory")]
    OutsideLayout(String, &'static str),
    #[error(transparent)]
    Layout(#[from] LayoutResolveError),
    #[error(transparent)]
    Library(#[from] LibraryError),
    #[error(transparent)]
    Db(#[from] ps_db::DbError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug)]
pub struct Adopted {
    pub mod_id: String,
    pub version_id: String,
    pub files: usize,
    pub enabled: bool,
}

/// Scoped to the target, because a mod id carries no target and two targets can
/// each hold a hand-installed mod of the same name. Without the scope the second
/// target's adoption collides with the first's version on the same day, and on a
/// later day silently resolves to the first target's bytes.
pub fn adopted_version(today: &str, target_id: &str) -> String {
    format!("adopted-{today}-{}", ps_core::mods::slugify(target_id))
}

fn mod_type_for(kind: RouteKind) -> ModType {
    match kind {
        RouteKind::Ue4ss => ModType::Ue4ss,
        RouteKind::PalSchema => ModType::PalSchema,
        RouteKind::Pak => ModType::Pak,
        RouteKind::LogicMods => ModType::LogicMods,
        RouteKind::NativeDll => ModType::NativeDll,
        RouteKind::Workshop => ModType::Workshop,
        RouteKind::Framework | RouteKind::Binaries | RouteKind::Ue4ssCore => ModType::Framework,
        RouteKind::Companion | RouteKind::Passthrough => ModType::Pak,
    }
}

fn base_label(kind: RouteKind) -> &'static str {
    match kind {
        RouteKind::Ue4ss => "UE4SS mods",
        RouteKind::PalSchema => "PalSchema mods",
        RouteKind::Pak => "~mods",
        RouteKind::LogicMods => "LogicMods",
        RouteKind::NativeDll => "NativeMods",
        RouteKind::Workshop => "local Workshop",
        _ => "target",
    }
}

/// Copies the candidate's files into the library, records them at the paths they
/// already occupy, and joins the active profile with the enabled state the scan
/// read from disk. Nothing on the target is written, moved or removed: an adopted
/// mod must look to the game exactly as it did before.
pub async fn adopt(
    db: &dyn ps_db::DbDriver,
    paths: &LibraryPaths,
    target: &ModTarget,
    candidate: &AdoptionCandidate,
) -> Result<Adopted, AdoptError> {
    if candidate.source == CandidateSource::SteamSubscribed {
        return adopt_subscribed(db, paths, target, candidate).await;
    }
    if candidate.files.is_empty() {
        return Err(AdoptError::Empty(candidate.name.clone()));
    }

    let resolved = layout::layout_for(target)?;
    let base = resolved
        .base_for(candidate.kind)
        .ok_or_else(|| {
            AdoptError::OutsideLayout(candidate.name.clone(), base_label(candidate.kind))
        })?
        .to_path_buf();

    // Every file must sit under the location its kind names, because `rel_path` is
    // computed against that base and the deployer later joins it back on.
    let mut routes = Vec::with_capacity(candidate.files.len());
    for file in &candidate.files {
        let path = Path::new(file);
        let relative = path
            .strip_prefix(&base)
            .map_err(|_| AdoptError::OutsideLayout(file.clone(), base_label(candidate.kind)))?;
        routes.push(FileRoute {
            archive_path: slash(relative),
            rel_path: slash(relative),
            kind: candidate.kind,
        });
    }

    let profile = ps_db::mod_profiles::active_for_target(db, &target.id)
        .await?
        .ok_or_else(|| AdoptError::NoActiveProfile(target.id.clone()))?;

    let id = mod_id(&ModIdInput::Local {
        folder: &candidate.name,
        mod_type: mod_type_for(candidate.kind),
    });
    let manifest = InstallManifest {
        folder_name: candidate.name.clone(),
        display_name: candidate.name.clone(),
        mod_type: mod_type_for(candidate.kind),
        version: adopted_version(&today_utc(), &target.id),
        routes,
        decisions: Vec::new(),
        platform_filtered: None,
        source: SourceHint::default(),
    };

    let stored = library::store(
        db,
        paths,
        &library::StoreRequest {
            mod_id: &id,
            manifest: &manifest,
            // The target's own directory is the source tree: a route's
            // `archive_path` is its path relative to this base.
            extracted_root: &base,
            archive: None,
            source_kind: "adopted",
            source_ref: r#"{"kind":"adopted"}"#,
            custom_name: None,
        },
    )
    .await?;

    // Everything after the library write is rolled back on failure. `store`
    // refuses a second attempt at the same version id, so without this a failure
    // here would leave the library holding a version the target does not reference
    // and every retry refused until the date rolled over.
    let version_id = stored.version.id.clone();
    match finish(
        db, target, candidate, &profile, &id, &manifest, &stored, &base,
    )
    .await
    {
        Ok(()) => {}
        Err(error) => {
            let _ = ps_db::mod_deployments::forget(
                db,
                &target.id,
                &recorded_paths(&manifest, &base)
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
            )
            .await;
            let _ = library::delete_version(db, &version_id).await;
            return Err(error);
        }
    }

    Ok(Adopted {
        mod_id: id,
        version_id: stored.version.id,
        files: manifest.routes.len(),
        enabled: candidate.enabled,
    })
}

/// Registers a Steam-subscribed package: a library row, one version with no
/// routes and an empty library directory, and an entry in the active profile
/// carrying its `ActiveModList` state. Steam's directory is never written and no
/// deployment row is recorded, so the only thing an apply can change for it is
/// that line. A package another target already registered shares its row.
async fn adopt_subscribed(
    db: &dyn ps_db::DbDriver,
    paths: &LibraryPaths,
    target: &ModTarget,
    candidate: &AdoptionCandidate,
) -> Result<Adopted, AdoptError> {
    let workshop_id = Path::new(&candidate.root)
        .file_name()
        .map(|leaf| leaf.to_string_lossy().into_owned())
        .filter(|leaf| library::is_workshop_item_id(leaf))
        .ok_or_else(|| AdoptError::NotAWorkshopItem(candidate.root.clone()))?;
    let profile = ps_db::mod_profiles::active_for_target(db, &target.id)
        .await?
        .ok_or_else(|| AdoptError::NoActiveProfile(target.id.clone()))?;
    let id = library::subscribed_mod_id(&workshop_id);
    let version_id = library::version_id(&id, SUBSCRIBED_VERSION);

    let created = match ps_db::mod_library::get_mod(db, &id).await? {
        Some(row)
            if library::is_subscribed(&row)
                && library::workshop_id_of(&row).as_deref() == Some(workshop_id.as_str())
                && library::workshop_package_of(&row).as_deref()
                    == Some(candidate.name.as_str()) =>
        {
            if ps_db::mod_library::get_version(db, &version_id)
                .await?
                .is_none()
            {
                return Err(AdoptError::Db(ps_db::DbError::Other(format!(
                    "subscribed package {id} has no version {version_id}"
                ))));
            }
            false
        }
        Some(_) => return Err(AdoptError::AlreadyManaged(candidate.name.clone())),
        None => {
            if !library::workshop_mods_named(db, &candidate.name)
                .await?
                .is_empty()
            {
                return Err(AdoptError::AlreadyManaged(candidate.name.clone()));
            }
            let info = std::fs::read_to_string(Path::new(&candidate.root).join("Info.json"))
                .ok()
                .and_then(|text| ps_core::mods::parse_workshop_info(&text).ok());
            let display_name = info
                .as_ref()
                .map(|info| info.mod_name.trim().to_string())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| candidate.name.clone());
            let author = info
                .as_ref()
                .map(|info| info.author.trim().to_string())
                .filter(|author| !author.is_empty());
            let manifest = InstallManifest {
                folder_name: candidate.name.clone(),
                display_name,
                mod_type: ModType::Workshop,
                version: SUBSCRIBED_VERSION.to_string(),
                routes: Vec::new(),
                decisions: Vec::new(),
                platform_filtered: None,
                source: SourceHint {
                    workshop_package: Some(candidate.name.clone()),
                    author,
                    ..SourceHint::default()
                },
            };
            let source_ref = serde_json::json!({
                "kind": "workshop",
                "package": candidate.name,
                "workshop_id": workshop_id,
            })
            .to_string();
            library::store(
                db,
                paths,
                &library::StoreRequest {
                    mod_id: &id,
                    manifest: &manifest,
                    extracted_root: Path::new(""),
                    archive: None,
                    source_kind: library::WORKSHOP_SOURCE_KIND,
                    source_ref: &source_ref,
                    custom_name: None,
                },
            )
            .await?;
            true
        }
    };

    let entries = ps_db::mod_profiles::mods_of(db, &profile.id).await?;
    let load_order = entries
        .iter()
        .find(|entry| entry.mod_id == id)
        .map(|entry| entry.load_order)
        .unwrap_or(entries.len() as i64);
    let joined = ps_db::mod_profiles::set_mod(
        db,
        &ps_db::mod_profiles::ProfileModRow {
            profile_id: profile.id.clone(),
            mod_id: id.clone(),
            mod_version_id: None,
            enabled: candidate.enabled,
            load_order,
        },
    )
    .await;
    if let Err(error) = joined {
        if created {
            let _ = library::delete_mod(db, paths, &id).await;
        }
        return Err(error.into());
    }

    Ok(Adopted {
        mod_id: id,
        version_id,
        files: 0,
        enabled: candidate.enabled,
    })
}

#[allow(clippy::too_many_arguments)]
async fn finish(
    db: &dyn ps_db::DbDriver,
    target: &ModTarget,
    candidate: &AdoptionCandidate,
    profile: &ps_db::mod_profiles::ProfileRow,
    id: &str,
    manifest: &InstallManifest,
    stored: &library::Stored,
    base: &Path,
) -> Result<(), AdoptError> {
    // Rows describing the files as they are on disk, so the next scan reports
    // managed-intact and the next apply plans nothing. The hashes come from
    // `store`, which computed them from the copies it had just made of these very
    // files: re-reading the live game file here would be a second full pass over
    // every pak and would open a window in which the recorded hash describes bytes
    // the library does not hold.
    let mut rows = Vec::with_capacity(manifest.routes.len());
    for (route, copied) in manifest.routes.iter().zip(stored.files.iter()) {
        debug_assert_eq!(route.rel_path, copied.rel_path);
        rows.push(ps_db::mod_deployments::NewDeploymentFile {
            target_id: target.id.clone(),
            path: on_disk_path(base, &route.rel_path)
                .to_string_lossy()
                .into_owned(),
            mod_version_id: Some(stored.version.id.clone()),
            hash: copied.hash.clone(),
            role: "file".to_string(),
            rel_path: Some(route.rel_path.clone()),
        });
    }
    ps_db::mod_deployments::record(db, &rows).await?;

    let load_order = ps_db::mod_profiles::mods_of(db, &profile.id).await?.len() as i64;
    ps_db::mod_profiles::set_mod(
        db,
        &ps_db::mod_profiles::ProfileModRow {
            profile_id: profile.id.clone(),
            mod_id: id.to_string(),
            // Pinned rather than NULL: a mod id carries no target, so two targets
            // adopting a mod of the same name share one `mods` row, and "follow the
            // current version" would resolve to whichever target adopted first.
            mod_version_id: Some(stored.version.id.clone()),
            enabled: candidate.enabled,
            load_order,
        },
    )
    .await?;
    Ok(())
}

fn on_disk_path(base: &Path, rel_path: &str) -> std::path::PathBuf {
    let mut path = base.to_path_buf();
    for segment in rel_path.split('/').filter(|s| !s.is_empty()) {
        path.push(segment);
    }
    path
}

fn recorded_paths(manifest: &InstallManifest, base: &Path) -> Vec<String> {
    manifest
        .routes
        .iter()
        .map(|route| {
            on_disk_path(base, &route.rel_path)
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

fn slash(relative: &Path) -> String {
    relative
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn today_utc() -> String {
    chrono::Utc::now().format("%Y%m%d").to_string()
}
