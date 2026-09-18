//! Wire handlers that export a profile to a `.psmods` file and import one.
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{json, Value};

use ps_db::mod_library::{ModRow, ModVersionRow};
use ps_db::mod_profiles::{NewProfile, ProfileModRow, ProfileRow};
use ps_db::mod_targets::ModTarget;
use ps_db::{DbDriver, DbError};

use crate::desktop_dialogs::FileSaveRequest;
use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;
use crate::mods_handlers::{
    emit_refusal, enable_refusal, install_error_code, profile_json, resolve_archive_path,
    target_or_refusal,
};
use crate::mods_profile_handlers::{
    checked_name, free_profile_id, profile_or_refusal, MAX_NAME_CHARS,
};
use crate::services::mods::install::{self, InstallError, InstallRequest, Provenance};
use crate::services::mods::library::LibraryError;
use crate::services::mods::share::{self, ShareError, SharedEntry, SharedFramework, SharedProfile};
use crate::services::mods::uploads::UploadStore;
use crate::services::mods::{layout, LibraryPaths};

#[derive(Debug, serde::Deserialize)]
pub struct ProfileExportData {
    pub target_id: String,
    pub profile_id: String,
    pub include_archives: bool,
    pub path: String,
}

type ResolvedEntry = (ProfileModRow, ModRow, ModVersionRow);

struct Export {
    profile: SharedProfile,
    archives: Vec<(String, PathBuf)>,
    missing_archives: Vec<String>,
    unresolved: Vec<String>,
}

pub async fn handle_profile_export(
    data: ProfileExportData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let message_type = MessageType::ProfileExport;
    let context = json!({
        "target_id": data.target_id,
        "profile_id": data.profile_id,
        "path": data.path,
    });
    let Some(target) =
        target_or_refusal(ctx, message_type, &data.target_id, context.clone()).await?
    else {
        return Ok(());
    };
    let Some(profile) = profile_or_refusal(
        ctx,
        message_type,
        context.clone(),
        &target.id,
        Some(&data.profile_id),
    )
    .await
    else {
        return Ok(());
    };
    let Some(dest) = export_path(ctx, &data.path, &profile.name, &context).await else {
        return Ok(());
    };

    let built = build_export(&*ctx.app.driver, &target, &profile, data.include_archives).await;
    let Export {
        profile: shared,
        archives,
        missing_archives,
        unresolved,
    } = match built {
        Ok(export) => export,
        Err(error) => {
            emit_refusal(
                ctx,
                message_type,
                context,
                "db",
                error.to_string(),
                json!({}),
            );
            return Ok(());
        }
    };
    let entries = shared.entries.len();
    let archives_included = archives.len();

    let target_file = dest.clone();
    let written =
        tokio::task::spawn_blocking(move || share::write_psmods(&target_file, &shared, &archives))
            .await;
    let failure = match written {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(error.to_string()),
        Err(error) => Some(error.to_string()),
    };
    if let Some(reason) = failure {
        emit_refusal(
            ctx,
            message_type,
            context,
            "export_failed",
            format!("could not write {}: {reason}", dest.display()),
            json!({ "reason": reason }),
        );
        return Ok(());
    }

    ctx.emitter.emit(
        message_type,
        &json!({
            "target_id": target.id,
            "profile_id": profile.id,
            "path": ps_core::mods::native_separators(&dest.to_string_lossy(), cfg!(windows)),
            "entries": entries,
            "archives_included": archives_included,
            "missing_archives": missing_archives,
            "unresolved": unresolved,
        }),
    );
    Ok(())
}

/// `None` means a refusal or a cancellation was already emitted.
async fn export_path(
    ctx: &mut HandlerCtx<'_>,
    path: &str,
    profile_name: &str,
    context: &Value,
) -> Option<PathBuf> {
    let message_type = MessageType::ProfileExport;
    if path == "__select__" {
        if !ctx.app.config.desktop_mode {
            emit_refusal(
                ctx,
                message_type,
                context.clone(),
                "desktop_only",
                "desktop mode is required to choose where to save".to_string(),
                json!({}),
            );
            return None;
        }
        let picked = ctx
            .app
            .dialogs
            .save_file(FileSaveRequest {
                filter_name: "PalStudio mod profile",
                filter_extensions: &["psmods"],
                suggested_file_name: format!("{}.psmods", ps_core::mods::slugify(profile_name)),
                initial_directory: None,
            })
            .await;
        if picked.is_none() {
            let mut reply = context.clone();
            reply["canceled"] = json!(true);
            ctx.emitter.emit(message_type, &reply);
        }
        return picked;
    }
    let candidate = Path::new(path);
    let is_psmods = candidate
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("psmods"));
    let parent_exists = candidate.parent().is_some_and(Path::is_dir);
    if !candidate.is_absolute() || !is_psmods || !parent_exists {
        emit_refusal(
            ctx,
            message_type,
            context.clone(),
            "invalid_path",
            format!("{path} is not an absolute .psmods path in an existing directory"),
            json!({}),
        );
        return None;
    }
    Some(candidate.to_path_buf())
}

async fn build_export(
    db: &dyn DbDriver,
    target: &ModTarget,
    profile: &ProfileRow,
    include_archives: bool,
) -> Result<Export, DbError> {
    let (resolved, unresolved) = resolve_entries(db, &profile.id).await?;
    let mut entries = Vec::with_capacity(resolved.len());
    let mut archives = Vec::new();
    let mut missing_archives = Vec::new();
    for (row, module, version) in resolved {
        let mut archive = None;
        if include_archives {
            let kept = version
                .archive_path
                .as_deref()
                .map(PathBuf::from)
                .filter(|source| source.is_file());
            match kept {
                Some(source) => {
                    let file_name = source.file_name().unwrap_or_default().to_string_lossy();
                    let name = share::archive_entry_name(&version.id, &file_name);
                    archive = Some(name.clone());
                    archives.push((name, source));
                }
                None => missing_archives.push(version.id.clone()),
            }
        }
        entries.push(SharedEntry {
            mod_id: row.mod_id,
            name: module.name,
            mod_type: module.mod_type,
            source_kind: module.source_kind,
            version: version.version,
            mod_version_id: version.id,
            enabled: row.enabled,
            load_order: row.load_order,
            archive,
        });
    }

    let mut frameworks = Vec::new();
    for framework in ps_db::mod_profiles::frameworks_of(db, &target.id).await? {
        if let Some(version) =
            ps_db::mod_library::get_version(db, &framework.mod_version_id).await?
        {
            frameworks.push(SharedFramework {
                framework: framework.framework,
                mod_id: version.mod_id,
                version: version.version,
                mod_version_id: version.id,
            });
        }
    }

    Ok(Export {
        profile: SharedProfile {
            format: share::FORMAT.to_string(),
            format_version: share::FORMAT_VERSION,
            name: profile.name.clone(),
            exported_at: chrono::Utc::now().to_rfc3339(),
            target_kind: target.kind.clone(),
            target_platform: target.platform.clone(),
            entries,
            frameworks,
        },
        archives,
        missing_archives,
        unresolved,
    })
}

/// Pairs each entry with its mod and the version it would deploy: its pin, or
/// the mod's current version. Entries with neither are returned by mod id.
async fn resolve_entries(
    db: &dyn DbDriver,
    profile_id: &str,
) -> Result<(Vec<ResolvedEntry>, Vec<String>), DbError> {
    let mut resolved = Vec::new();
    let mut unresolved = Vec::new();
    for row in ps_db::mod_profiles::mods_of(db, profile_id).await? {
        let module = ps_db::mod_library::get_mod(db, &row.mod_id).await?;
        let version = match row.mod_version_id.as_deref() {
            Some(id) => ps_db::mod_library::get_version(db, id).await?,
            None => ps_db::mod_library::current_version(db, &row.mod_id).await?,
        };
        match (module, version) {
            (Some(module), Some(version)) => resolved.push((row, module, version)),
            _ => unresolved.push(row.mod_id),
        }
    }
    Ok((resolved, unresolved))
}

#[derive(Debug, serde::Deserialize)]
pub struct ProfileImportData {
    pub target_id: String,
    pub path: String,
    #[serde(default)]
    pub name: Option<String>,
}

enum Resolution {
    Pinned(String),
    FollowCurrent,
    Missing(&'static str),
}

struct ImportEntry {
    mod_id: String,
    enabled: bool,
    pin: Option<String>,
}

#[derive(Default)]
struct Resolved {
    entries: Vec<ImportEntry>,
    pinned: Vec<String>,
    following_current: Vec<String>,
    installed: Vec<Value>,
    missing: Vec<Value>,
    frameworks: Vec<Value>,
}

/// Deletes an import's scratch directory however the import ends.
struct ScratchDir(PathBuf);

impl Drop for ScratchDir {
    fn drop(&mut self) {
        remove_scratch(&self.0);
    }
}

fn remove_scratch(dir: &Path) {
    if let Err(error) = std::fs::remove_dir_all(dir) {
        if error.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!(%error, path = %dir.display(), "could not delete import scratch");
        }
    }
}

/// What installing one included archive produced, shared by every entry naming it.
enum ArchiveOutcome {
    Installed { mod_id: String, version_id: String },
    AlreadyInstalled(String),
    Failed(&'static str),
}

pub async fn handle_profile_import(
    library: &LibraryPaths,
    uploads: &UploadStore,
    data: ProfileImportData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let message_type = MessageType::ProfileImport;
    let context = json!({ "target_id": data.target_id, "path": data.path });
    let Some(target) =
        target_or_refusal(ctx, message_type, &data.target_id, context.clone()).await?
    else {
        return Ok(());
    };
    let Some(resolved_path) = resolve_archive_path(
        ctx,
        message_type,
        &target.id,
        &data.path,
        ("PalStudio mod profile", &["psmods"]),
    )
    .await
    else {
        return Ok(());
    };
    let file = PathBuf::from(&resolved_path);

    let read_from = file.clone();
    let shared = match tokio::task::spawn_blocking(move || share::read_profile(&read_from)).await {
        Ok(Ok(shared)) => Arc::new(shared),
        Ok(Err(error)) => {
            let detail = match &error {
                ShareError::UnsupportedFormat {
                    format,
                    format_version,
                } => json!({ "format": format, "format_version": format_version }),
                _ => json!({}),
            };
            emit_refusal(
                ctx,
                message_type,
                context,
                error.code(),
                error.to_string(),
                detail,
            );
            return Ok(());
        }
        Err(error) => {
            let reason = error.to_string();
            emit_refusal(
                ctx,
                message_type,
                context,
                "import_failed",
                format!("could not read {resolved_path}: {reason}"),
                json!({ "reason": reason }),
            );
            return Ok(());
        }
    };

    let db = &*ctx.app.driver;
    let requested = match &data.name {
        Some(name) => name.clone(),
        None => shared.name.trim().chars().take(MAX_NAME_CHARS).collect(),
    };
    let name = match available_name(db, &target.id, &requested).await {
        Ok(Some(name)) => name,
        Ok(None) => {
            emit_refusal(
                ctx,
                message_type,
                context,
                "invalid_name",
                format!("a profile name is 1 to {MAX_NAME_CHARS} characters"),
                json!({}),
            );
            return Ok(());
        }
        Err(error) => {
            emit_refusal(
                ctx,
                message_type,
                context,
                "db",
                error.to_string(),
                json!({}),
            );
            return Ok(());
        }
    };
    let spec = match layout::spec_for(&target) {
        Ok(spec) => spec,
        Err(error) => {
            emit_refusal(
                ctx,
                message_type,
                context,
                "layout_error",
                error.to_string(),
                json!({}),
            );
            return Ok(());
        }
    };

    let scratch = ScratchDir(uploads.imports_dir().join(uuid::Uuid::new_v4().to_string()));
    let created = async {
        let resolved = resolve_import(db, library, &spec, &file, &shared, &scratch.0).await?;
        let profile = ps_db::mod_profiles::create(
            db,
            &NewProfile {
                id: free_profile_id(db, &target.id, &name).await?,
                target_id: target.id.clone(),
                name,
                is_default: false,
            },
        )
        .await?;
        Ok::<_, DbError>((resolved, profile))
    }
    .await;
    let (resolved, profile) = match created {
        Ok(created) => created,
        Err(error) => {
            emit_refusal(
                ctx,
                message_type,
                context,
                "db",
                error.to_string(),
                json!({}),
            );
            return Ok(());
        }
    };

    let written = async {
        let disabled = write_entries(db, &target, &profile.id, &resolved.entries).await?;
        Ok::<_, DbError>((disabled, profile_json(db, &profile).await?))
    }
    .await;
    match written {
        Ok((disabled, profile_json)) => ctx.emitter.emit(
            message_type,
            &json!({
                "target_id": target.id,
                "path": ps_core::mods::native_separators(&resolved_path, cfg!(windows)),
                "profile": profile_json,
                "pinned": resolved.pinned,
                "following_current": resolved.following_current,
                "installed": resolved.installed,
                "missing": resolved.missing,
                "disabled": disabled,
                "frameworks": resolved.frameworks,
            }),
        ),
        Err(error) => {
            let reason = error.to_string();
            emit_refusal(
                ctx,
                message_type,
                context,
                "import_failed",
                format!(
                    "profile {} was created but not filled: {reason}",
                    profile.id
                ),
                json!({ "reason": reason, "profile_id": profile.id }),
            );
        }
    }
    Ok(())
}

/// `name`, or the first of `name (2)`, `name (3)`, … that no profile of the
/// target holds, trimming `name` so the suffix fits. `None` when `name` is empty
/// or too long.
async fn available_name(
    db: &dyn DbDriver,
    target_id: &str,
    name: &str,
) -> Result<Option<String>, DbError> {
    match checked_name(db, target_id, name, None).await? {
        Ok(free) => return Ok(Some(free)),
        Err(("name_taken", _)) => {}
        Err(_) => return Ok(None),
    }
    let base = name.trim();
    for n in 2.. {
        let suffix = format!(" ({n})");
        let kept: String = base
            .chars()
            .take(MAX_NAME_CHARS - suffix.chars().count())
            .collect();
        let candidate = format!("{}{suffix}", kept.trim_end());
        if let Ok(free) = checked_name(db, target_id, &candidate, None).await? {
            return Ok(Some(free));
        }
    }
    unreachable!("an unbounded range always yields a free name")
}

async fn resolve_import(
    db: &dyn DbDriver,
    library: &LibraryPaths,
    spec: &ps_core::mods::TargetSpec,
    file: &Path,
    shared: &Arc<SharedProfile>,
    scratch: &Path,
) -> Result<Resolved, DbError> {
    let mut entries: Vec<&SharedEntry> = shared.entries.iter().collect();
    entries.sort_by_key(|entry| entry.load_order);
    let mut seen = HashSet::new();
    let mut archives = HashMap::new();
    let mut resolved = Resolved::default();
    for (index, entry) in entries.into_iter().enumerate() {
        if !seen.insert(entry.mod_id.as_str()) {
            continue;
        }
        let out_dir = scratch.join(index.to_string());
        let (resolution, installed) = resolve_entry(
            db,
            library,
            spec,
            file,
            shared,
            entry,
            out_dir,
            &mut archives,
        )
        .await?;
        if let Some(version_id) = installed {
            resolved
                .installed
                .push(json!({ "mod_id": entry.mod_id, "version_id": version_id }));
        }
        let pin = match resolution {
            Resolution::Pinned(version_id) => {
                resolved.pinned.push(entry.mod_id.clone());
                Some(version_id)
            }
            Resolution::FollowCurrent => {
                resolved.following_current.push(entry.mod_id.clone());
                None
            }
            Resolution::Missing(reason) => {
                resolved.missing.push(json!({
                    "mod_id": entry.mod_id,
                    "name": entry.name,
                    "version": entry.version,
                    "reason": reason,
                }));
                continue;
            }
        };
        resolved.entries.push(ImportEntry {
            mod_id: entry.mod_id.clone(),
            enabled: entry.enabled,
            pin,
        });
    }
    for framework in &shared.frameworks {
        let in_library = ps_db::mod_library::get_version(db, &framework.mod_version_id)
            .await?
            .is_some();
        resolved.frameworks.push(json!({
            "framework": framework.framework,
            "mod_id": framework.mod_id,
            "version": framework.version,
            "in_library": in_library,
        }));
    }
    Ok(resolved)
}

/// The entry's resolution, and the version id when its archive was installed.
/// `archives` holds the outcome of every archive already processed, so an
/// archive several entries name is extracted and installed only once.
#[allow(clippy::too_many_arguments)]
async fn resolve_entry(
    db: &dyn DbDriver,
    library: &LibraryPaths,
    spec: &ps_core::mods::TargetSpec,
    file: &Path,
    shared: &Arc<SharedProfile>,
    entry: &SharedEntry,
    out_dir: PathBuf,
    archives: &mut HashMap<String, ArchiveOutcome>,
) -> Result<(Resolution, Option<String>), DbError> {
    if let Some(version) = ps_db::mod_library::get_version(db, &entry.mod_version_id).await? {
        if version.mod_id == entry.mod_id {
            return Ok((Resolution::Pinned(version.id), None));
        }
    }
    let Some(archive) = entry.archive.as_deref() else {
        return Ok((
            follow_current_or(db, &entry.mod_id, "not_in_library").await?,
            None,
        ));
    };
    if !archives.contains_key(archive) {
        let outcome = install_included(db, library, spec, file, shared, archive, out_dir).await;
        archives.insert(archive.to_string(), outcome);
    }
    Ok(match &archives[archive] {
        ArchiveOutcome::Failed(reason) => (Resolution::Missing(reason), None),
        ArchiveOutcome::Installed { mod_id, .. } if *mod_id != entry.mod_id => {
            (Resolution::Missing("id_mismatch"), None)
        }
        ArchiveOutcome::Installed { version_id, .. } => {
            let resolution = if *version_id == entry.mod_version_id {
                Resolution::Pinned(version_id.clone())
            } else {
                Resolution::FollowCurrent
            };
            (resolution, Some(version_id.clone()))
        }
        ArchiveOutcome::AlreadyInstalled(version_id) => {
            let resolution = match ps_db::mod_library::get_version(db, version_id).await? {
                Some(version) if version.mod_id != entry.mod_id => {
                    Resolution::Missing("id_mismatch")
                }
                Some(version) if version.id == entry.mod_version_id => {
                    Resolution::Pinned(version.id)
                }
                Some(_) => follow_current_or(db, &entry.mod_id, "library_error").await?,
                None => Resolution::Missing("library_error"),
            };
            (resolution, None)
        }
    })
}

/// Extracts and installs one included archive into `out_dir`, deleting the
/// extracted copy as soon as the attempt ends; the library keeps its own.
async fn install_included(
    db: &dyn DbDriver,
    library: &LibraryPaths,
    spec: &ps_core::mods::TargetSpec,
    file: &Path,
    shared: &Arc<SharedProfile>,
    archive: &str,
    out_dir: PathBuf,
) -> ArchiveOutcome {
    let outcome = match extract(file, shared, archive.to_string(), out_dir.clone()).await {
        None => ArchiveOutcome::Failed("extract_failed"),
        Some(extracted) => {
            let request = InstallRequest {
                archive: &extracted,
                target_platform: spec.platform,
                target_kind: spec.kind,
                provenance: Provenance::Local,
                custom_name: None,
                keep_archive: true,
                accept_decisions: true,
            };
            match install::install_archive(db, library, &request).await {
                Ok(installed) => ArchiveOutcome::Installed {
                    mod_id: installed.mod_id,
                    version_id: installed.stored.version.id,
                },
                Err(InstallError::Library(LibraryError::AlreadyInstalled(version_id))) => {
                    ArchiveOutcome::AlreadyInstalled(version_id)
                }
                Err(error) => ArchiveOutcome::Failed(install_error_code(&error)),
            }
        }
    };
    remove_scratch(&out_dir);
    outcome
}

async fn follow_current_or(
    db: &dyn DbDriver,
    mod_id: &str,
    reason: &'static str,
) -> Result<Resolution, DbError> {
    Ok(
        match ps_db::mod_library::current_version(db, mod_id).await? {
            Some(_) => Resolution::FollowCurrent,
            None => Resolution::Missing(reason),
        },
    )
}

/// The extracted archive, or `None` when extraction failed or produced no file,
/// as a reserved device name such as `NUL.zip` does on Windows.
async fn extract(
    file: &Path,
    shared: &Arc<SharedProfile>,
    archive: String,
    out_dir: PathBuf,
) -> Option<PathBuf> {
    let file = file.to_path_buf();
    let shared = Arc::clone(shared);
    let extracted = tokio::task::spawn_blocking(move || {
        std::fs::create_dir_all(&out_dir)?;
        share::extract_archive(&file, &shared, &archive, &out_dir)
    })
    .await;
    match extracted {
        Ok(Ok(path)) if path.is_file() => Some(path),
        Ok(Ok(path)) => {
            tracing::warn!(path = %path.display(), "extracting an imported archive produced no file");
            None
        }
        Ok(Err(error)) => {
            tracing::warn!(%error, "could not extract an imported archive");
            None
        }
        Err(error) => {
            tracing::warn!(%error, "archive extraction did not finish");
            None
        }
    }
}

/// Writes each entry at its position, disabling the ones the target refuses to
/// enable; returns those as `{ mod_id, code }`.
async fn write_entries(
    db: &dyn DbDriver,
    target: &ModTarget,
    profile_id: &str,
    entries: &[ImportEntry],
) -> Result<Vec<Value>, DbError> {
    let mut disabled = Vec::new();
    for (load_order, entry) in entries.iter().enumerate() {
        let mut enabled = entry.enabled;
        if enabled {
            if let Some((code, ..)) =
                enable_refusal(db, target, &entry.mod_id, entry.pin.as_deref()).await?
            {
                enabled = false;
                disabled.push(json!({ "mod_id": entry.mod_id, "code": code }));
            }
        }
        ps_db::mod_profiles::set_mod(
            db,
            &ProfileModRow {
                profile_id: profile_id.to_string(),
                mod_id: entry.mod_id.clone(),
                mod_version_id: entry.pin.clone(),
                enabled,
                load_order: load_order as i64,
            },
        )
        .await?;
    }
    Ok(disabled)
}

#[cfg(test)]
mod tests {
    use ps_db::mod_library::{NewMod, NewModVersion};

    use super::*;
    use crate::servers_handlers::test_env::TestEnv;

    #[tokio::test]
    async fn a_mod_with_no_resolvable_version_is_unresolved() {
        let env = TestEnv::new().await;
        let db = &*env.app.driver;
        let root = env._scratch.path().join("Palworld");
        std::fs::create_dir_all(root.join("Pal/Binaries/Win64")).unwrap();
        std::fs::write(
            root.join("Pal/Binaries/Win64/Palworld-Win64-Shipping.exe"),
            b"stub",
        )
        .unwrap();
        std::fs::create_dir_all(root.join("Pal/Content/Paks")).unwrap();
        let detected = crate::services::mods::detect::describe(&root, "manual").unwrap();
        let target = crate::services::mods::detect::register(db, &detected, "Client")
            .await
            .unwrap();
        let profile_id = format!("{}/default", target.id);

        for (mod_id, version_id) in [
            ("ghost-ue4ss", None),
            ("real-ue4ss", Some("real-ue4ss@1.0")),
        ] {
            ps_db::mod_library::upsert_mod(
                db,
                &NewMod {
                    id: mod_id.to_string(),
                    name: mod_id.to_string(),
                    custom_name: None,
                    mod_type: "ue4ss".to_string(),
                    author: None,
                    summary: None,
                    source_kind: "local".to_string(),
                    source_ref: "{}".to_string(),
                    nexus_mod_id: None,
                    ignored_version: None,
                    notes: None,
                },
            )
            .await
            .unwrap();
            if let Some(version_id) = version_id {
                ps_db::mod_library::insert_version(
                    db,
                    &NewModVersion {
                        id: version_id.to_string(),
                        mod_id: mod_id.to_string(),
                        version: "1.0".to_string(),
                        library_dir: "library".to_string(),
                        manifest: "{}".to_string(),
                        source_ref: "{}".to_string(),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
            }
            ps_db::mod_profiles::set_mod(
                db,
                &ProfileModRow {
                    profile_id: profile_id.clone(),
                    mod_id: mod_id.to_string(),
                    mod_version_id: None,
                    enabled: true,
                    load_order: 0,
                },
            )
            .await
            .unwrap();
        }

        let (resolved, unresolved) = resolve_entries(db, &profile_id).await.unwrap();

        assert_eq!(unresolved, vec!["ghost-ue4ss".to_string()]);
        assert_eq!(resolved.len(), 1);
        let (row, module, version) = &resolved[0];
        assert_eq!(row.mod_id, "real-ue4ss");
        assert_eq!(module.id, "real-ue4ss");
        assert_eq!(version.id, "real-ue4ss@1.0");
    }
}
