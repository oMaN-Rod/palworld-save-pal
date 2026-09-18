//! Wire handlers for the mod-target and mod-library messages: listing,
//! detecting, adding, removing and scanning targets, adopting a hand-installed
//! mod, analyzing and installing archives, listing/removing library mods and
//! versions, listing/restoring/deleting backup sets, and listing, selecting,
//! planning and applying a target's profile.
use serde_json::{json, Value};

use crate::desktop_dialogs::FileDialogRequest;
use crate::dispatcher::HandlerCtx;
use crate::emitter::Emitter;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;
use crate::mods_profile_handlers::profile_or_refusal;
use crate::services::mods::{
    adopt, deploy, detect, digest, install, layout, library, paths::LibraryPaths, running, scan,
};
use crate::services::ServerServices;

use ps_db::mod_targets::ModTarget;

/// Every extension `extract::sniff_format` recognises by magic bytes, plus the
/// loose-file kinds `install.rs` stages as a single-file "archive" when the
/// format sniff comes back `Unknown`.
const ARCHIVE_EXTENSIONS: &[&str] = &["zip", "7z", "rar", "tar", "gz", "tgz", "pak", "lua", "dll"];

/// A refusal is a reply under the request's own message type, carrying an
/// `error` object with `code`/`message` plus whatever `detail` adds (pass
/// `json!({})` for none). An `error`-typed frame would send the UI to its
/// fatal error page, which is wrong for something as ordinary as a folder that
/// is not a Palworld install.
pub(crate) fn emit_refusal(
    ctx: &mut HandlerCtx<'_>,
    message_type: MessageType,
    context: Value,
    code: &str,
    message: String,
    detail: Value,
) {
    let mut reply = match context {
        Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    let mut error = serde_json::Map::new();
    error.insert("code".to_string(), Value::String(code.to_string()));
    error.insert("message".to_string(), Value::String(message));
    if let Value::Object(detail_map) = detail {
        for (key, value) in detail_map {
            error.insert(key, value);
        }
    }
    reply.insert("error".to_string(), Value::Object(error));
    ctx.emitter.emit(message_type, &Value::Object(reply));
}

/// `context` is echoed on the refusal. A request whose replies carry more than
/// its `target_id`, such as the `path` of an analyze or install, passes that too,
/// so a client with several pending can tell which one was refused.
pub(crate) async fn target_or_refusal(
    ctx: &mut HandlerCtx<'_>,
    message_type: MessageType,
    target_id: &str,
    context: Value,
) -> Result<Option<ModTarget>, HandlerError> {
    match ps_db::mod_targets::get(&*ctx.app.driver, target_id).await {
        Ok(Some(target)) => Ok(Some(target)),
        Ok(None) => {
            emit_refusal(
                ctx,
                message_type,
                context,
                "target_not_found",
                format!("no mod target {target_id}"),
                json!({}),
            );
            Ok(None)
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
            Ok(None)
        }
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct TargetIdData {
    pub target_id: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct TargetScanData {
    pub target_id: String,
    #[serde(default)]
    pub candidates_only: bool,
}

#[derive(Debug, serde::Deserialize)]
pub struct TargetAddData {
    pub root_path: String,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct AdoptData {
    pub target_id: String,
    pub candidate_name: String,
    #[serde(default)]
    pub source: Option<scan::CandidateSource>,
    #[serde(default)]
    pub root: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct AnalyzeData {
    pub path: String,
    pub target_id: String,
}

pub(crate) fn default_true() -> bool {
    true
}

#[derive(Debug, serde::Deserialize)]
pub struct InstallData {
    pub path: String,
    pub target_id: String,
    #[serde(default)]
    pub custom_name: Option<String>,
    #[serde(default)]
    pub accept_defaults: bool,
    #[serde(default = "default_true")]
    pub enable: bool,
}

#[derive(Debug, serde::Deserialize)]
pub struct ModIdData {
    pub mod_id: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct VersionIdData {
    pub version_id: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct SetCurrentData {
    pub mod_id: String,
    pub version_id: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct BackupTargetData {
    pub target_id: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct BackupRestoreData {
    pub target_id: String,
    pub set: String,
    #[serde(default)]
    pub paths: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize)]
pub struct BackupSetData {
    pub target_id: String,
    pub set: String,
}

/// Resolves an archive path field, following the `"__select__"` convention.
/// `None` means a reply was already emitted (a refusal or a cancellation) and
/// the caller should return without doing anything else. `target_id` is
/// echoed on every reply this emits, so the caller must have already
/// confirmed it names a real target. `filter` is the picker's file type name
/// and extensions.
pub(crate) async fn resolve_archive_path(
    ctx: &mut HandlerCtx<'_>,
    message_type: MessageType,
    target_id: &str,
    path: &str,
    filter: (&'static str, &'static [&'static str]),
) -> Option<String> {
    if path == "__select__" {
        if !ctx.app.config.desktop_mode {
            emit_refusal(
                ctx,
                message_type,
                json!({ "target_id": target_id, "path": path }),
                "desktop_only",
                "desktop mode is required to browse for a file".to_string(),
                json!({}),
            );
            return None;
        }
        let picked = ctx
            .app
            .dialogs
            .pick_file(FileDialogRequest {
                filter_name: filter.0,
                filter_extensions: filter.1,
                initial_directory: None,
            })
            .await;
        return match picked {
            Some(picked) => Some(picked.to_string_lossy().into_owned()),
            None => {
                ctx.emitter.emit(
                    message_type,
                    &json!({ "target_id": target_id, "path": path, "canceled": true }),
                );
                None
            }
        };
    }
    let candidate = std::path::Path::new(path);
    if !candidate.is_absolute() || !candidate.is_file() {
        emit_refusal(
            ctx,
            message_type,
            json!({ "target_id": target_id, "path": path }),
            "invalid_path",
            format!("{path} is not an absolute path to an existing file"),
            json!({}),
        );
        return None;
    }
    Some(path.to_string())
}

/// A bare identifier: ASCII letters, digits, `_`, `-` and `.` only (`.` stays
/// allowed because `slugify` keeps it, and real target ids can contain one --
/// excluding it would refuse real, already-registered targets). Never empty,
/// never containing `..`, never ending in `.` or space: Win32 silently strips
/// a trailing dot/space from a final segment, so `"..."` or `"x."` would
/// still parse as one `Normal` component while resolving to a different
/// directory than their spelling suggests.
fn is_safe_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
        && !value.contains("..")
        && !value.ends_with('.')
}

/// The real shape a `BackupSet` ever writes is `{ordinal:04}/{file_name}`
/// (`ps_core::mods::backup_key`) -- always exactly two `Normal` components,
/// never absolute. An `index.json` is data the deployer wrote, but restore
/// treats it as untrusted before joining it onto a real directory: a key
/// outside that shape is refused rather than joined.
fn is_valid_backup_key(key: &str) -> bool {
    let path = std::path::Path::new(key);
    if path.is_absolute() {
        return false;
    }
    let components: Vec<_> = path.components().collect();
    components.len() == 2
        && components
            .iter()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
}

/// Every directory a restored file may legitimately land in: the target's
/// root, plus every base directory its layout resolves (when it resolves at
/// all -- a target whose layout is broken still has a root). Nothing else
/// checks that the *directory* an `index.json` entry names is one of ours;
/// this is what does. An empty base is dropped: `Path::starts_with("")` is
/// true for every path, so an empty layout override would otherwise disable
/// containment entirely.
fn restore_bases(target: &ModTarget) -> Vec<std::path::PathBuf> {
    let mut bases = vec![std::path::PathBuf::from(&target.root_path)];
    if let Ok(resolved) = layout::layout_for(target) {
        bases.push(resolved.root);
        bases.push(resolved.paks_mods_dir);
        bases.push(resolved.logicmods_dir);
        for base in [
            resolved.binaries_dir,
            resolved.ue4ss_dir,
            resolved.ue4ss_mods_dir,
            resolved.palschema_mods_dir,
            resolved.nativemods_dir,
            resolved.workshop_local_dir,
        ]
        .into_iter()
        .flatten()
        {
            bases.push(base);
        }
    }
    bases.retain(|base| !base.as_os_str().is_empty());
    bases
}

fn is_under_any(path: &std::path::Path, bases: &[std::path::PathBuf]) -> bool {
    bases.iter().any(|base| path.starts_with(base))
}

/// `Path::starts_with` compares components lexically and never resolves `.`
/// or `..`, so `<root>/../outside/x` lexically starts with `<root>` while
/// naming a sibling of it. This is checked before `is_under_any` so that
/// bypass never reaches it: absolute, and no component but a prefix, a root
/// or a normal name.
fn is_lexically_contained(path: &std::path::Path) -> bool {
    path.is_absolute()
        && path.components().all(|c| {
            matches!(
                c,
                std::path::Component::Prefix(_)
                    | std::path::Component::RootDir
                    | std::path::Component::Normal(_)
            )
        })
}

/// The shared entry point for all three backup handlers: a `target_id` must be
/// shaped like a real id *and* name a target that actually exists, checked in
/// that order so a garbage id never reaches a database round trip, let alone
/// `LibraryPaths::backups_dir`. Every backup message takes this before
/// touching a `set` name or the filesystem.
async fn backup_target_or_refusal(
    ctx: &mut HandlerCtx<'_>,
    message_type: MessageType,
    target_id: &str,
) -> Result<Option<ModTarget>, HandlerError> {
    if !is_safe_id(target_id) {
        emit_refusal(
            ctx,
            message_type,
            json!({ "target_id": target_id }),
            "invalid_target",
            format!("{target_id} is not a valid target id"),
            json!({}),
        );
        return Ok(None);
    }
    target_or_refusal(
        ctx,
        message_type,
        target_id,
        json!({ "target_id": target_id }),
    )
    .await
}

/// The target's apply lock, or `None` once `apply_in_progress` has been replied.
fn apply_lock_or_refusal(
    ctx: &mut HandlerCtx<'_>,
    library: &LibraryPaths,
    target: &ModTarget,
    message_type: MessageType,
    context: Value,
) -> Option<tokio::sync::OwnedMutexGuard<()>> {
    let guard = deploy::try_lock_target(library, target);
    if guard.is_none() {
        emit_refusal(
            ctx,
            message_type,
            context,
            "apply_in_progress",
            format!("an apply is running on {}", target.id),
            json!({ "target_id": target.id }),
        );
    }
    guard
}

pub(crate) fn install_error_code(error: &install::InstallError) -> &'static str {
    match error {
        install::InstallError::Extract(_) => "extract_failed",
        install::InstallError::Library(_) => "library_error",
        install::InstallError::NothingRouted => "nothing_routed",
        install::InstallError::AlreadyManaged(_) => "already_managed",
        install::InstallError::Io(_) => "io_error",
        install::InstallError::NeedsDecisions(_) => "needs_decisions",
    }
}

fn install_request<'a>(
    archive: &'a std::path::Path,
    spec: &ps_core::mods::TargetSpec,
    custom_name: Option<&'a str>,
    accept_decisions: bool,
) -> install::InstallRequest<'a> {
    install::InstallRequest {
        archive,
        target_platform: spec.platform,
        target_kind: spec.kind,
        provenance: install::Provenance::Local,
        custom_name,
        keep_archive: true,
        accept_decisions,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArchiveInstallOutcome {
    Installed,
    NeedsDecisions,
    Refused,
}

pub(crate) struct ArchiveInstall<'a> {
    pub message_type: MessageType,
    /// Echoed on the reply and on every refusal.
    pub context: Value,
    pub target: &'a ModTarget,
    pub spec: &'a ps_core::mods::TargetSpec,
    pub archive: &'a std::path::Path,
    pub provenance: install::Provenance<'a>,
    pub custom_name: Option<&'a str>,
    pub accept_defaults: bool,
    pub enable: bool,
}

fn reply_with(context: &Value, fields: Vec<(&'static str, Value)>) -> Value {
    let mut reply = match context {
        Value::Object(map) => map.clone(),
        _ => serde_json::Map::new(),
    };
    for (key, value) in fields {
        reply.insert(key.to_string(), value);
    }
    Value::Object(reply)
}

pub(crate) async fn install_archive_and_reply(
    ctx: &mut HandlerCtx<'_>,
    library: &LibraryPaths,
    request: ArchiveInstall<'_>,
) -> ArchiveInstallOutcome {
    let ArchiveInstall {
        message_type,
        context,
        target,
        spec,
        archive,
        provenance,
        custom_name,
        accept_defaults,
        enable,
    } = request;
    let app = std::sync::Arc::clone(ctx.app);
    let db = &*app.driver;
    let install_request = install::InstallRequest {
        archive,
        target_platform: spec.platform,
        target_kind: spec.kind,
        provenance,
        custom_name,
        keep_archive: true,
        accept_decisions: accept_defaults,
    };
    let installed = match install::install_archive(db, library, &install_request).await {
        Ok(installed) => installed,
        Err(install::InstallError::NeedsDecisions(manifest)) => {
            ctx.emitter.emit(
                message_type,
                &reply_with(
                    &context,
                    vec![("needs_decisions", json!(manifest.decisions))],
                ),
            );
            return ArchiveInstallOutcome::NeedsDecisions;
        }
        Err(install::InstallError::Library(library::LibraryError::AlreadyInstalled(
            version_id,
        ))) => {
            let mod_id = version_id
                .split_once('@')
                .map(|(mod_id, _)| mod_id.to_string())
                .unwrap_or_else(|| version_id.clone());
            emit_refusal(
                ctx,
                message_type,
                context,
                "already_installed",
                format!("{version_id} is already in the library"),
                json!({ "mod_id": mod_id, "version_id": version_id }),
            );
            return ArchiveInstallOutcome::Refused;
        }
        Err(error) => {
            let detail = match &error {
                install::InstallError::AlreadyManaged(mod_id) => json!({ "mod_id": mod_id }),
                _ => json!({}),
            };
            emit_refusal(
                ctx,
                message_type,
                context,
                install_error_code(&error),
                error.to_string(),
                detail,
            );
            return ArchiveInstallOutcome::Refused;
        }
    };

    // The version is already stored at this point: a failure past here must
    // never be reported as "install failed", or a retry would fail again with
    // `already_installed` while the UI still thinks nothing happened.
    let enable_error = if enable {
        match install_enable_refusal(db, target, &installed.mod_id).await {
            Err(error) => Some(error_object("db", error.to_string(), json!({}))),
            Ok(Some((code, message, detail))) => Some(error_object(code, message, detail)),
            Ok(None) => enable_in_active_profile(db, &target.id, &installed.mod_id, true)
                .await
                .err()
                .map(|error| error_object("db", error.to_string(), json!({}))),
        }
    } else {
        None
    };

    let mut fields = vec![
        ("mod_id", json!(installed.mod_id)),
        ("version_id", json!(installed.stored.version.id)),
        (
            "manifest",
            serde_json::to_value(&installed.manifest).unwrap_or(Value::Null),
        ),
    ];
    if let Some(enable_error) = enable_error {
        fields.push(("enable_error", enable_error));
    }
    ctx.emitter
        .emit(message_type, &reply_with(&context, fields));
    ArchiveInstallOutcome::Installed
}

fn version_json(version: &ps_db::mod_library::ModVersionRow) -> Value {
    let manifest: Value = serde_json::from_str(&version.manifest).unwrap_or(Value::Null);
    let size_bytes = library::version_size(version).ok();
    json!({
        "id": version.id,
        "mod_id": version.mod_id,
        "version": version.version,
        "archive_path": version.archive_path,
        "library_dir": version.library_dir,
        "manifest": manifest,
        "source_ref": version.source_ref,
        "installed_at": version.installed_at,
        "is_current": version.is_current,
        "size_bytes": size_bytes,
    })
}

async fn mod_json(
    db: &dyn ps_db::DbDriver,
    row: &ps_db::mod_library::ModRow,
) -> Result<Value, ps_db::DbError> {
    let versions = ps_db::mod_library::versions_of(db, &row.id).await?;
    let current = ps_db::mod_library::current_version(db, &row.id).await?;
    Ok(json!({
        "id": row.id,
        "name": row.name,
        "custom_name": row.custom_name,
        "mod_type": row.mod_type,
        "author": row.author,
        "summary": row.summary,
        "source_kind": row.source_kind,
        "source_ref": row.source_ref,
        "nexus_mod_id": row.nexus_mod_id,
        "ignored_version": row.ignored_version,
        "notes": row.notes,
        "created_at": row.created_at,
        "updated_at": row.updated_at,
        "versions": versions.iter().map(version_json).collect::<Vec<_>>(),
        "current_version_id": current.map(|v| v.id),
    }))
}

/// Enables (or disables) a mod in the target's active profile. An entry that
/// already exists keeps its `load_order` and its pinned `mod_version_id`
/// (`None` still means "follow the mod's current version") -- only `enabled`
/// changes, so a reinstall or a plain toggle never reorders or unpins a user's
/// choice. A new entry is appended after the current highest `load_order`,
/// unpinned.
pub(crate) async fn enable_in_active_profile(
    db: &dyn ps_db::DbDriver,
    target_id: &str,
    mod_id: &str,
    enabled: bool,
) -> Result<ps_db::mod_profiles::ProfileModRow, ps_db::DbError> {
    let profile = ps_db::mod_profiles::active_for_target(db, target_id)
        .await?
        .ok_or_else(|| {
            ps_db::DbError::Other(format!("target {target_id} has no active profile"))
        })?;
    let existing = ps_db::mod_profiles::mods_of(db, &profile.id).await?;
    let entry = match existing.iter().find(|entry| entry.mod_id == mod_id) {
        Some(current) => ps_db::mod_profiles::ProfileModRow {
            profile_id: profile.id,
            mod_id: mod_id.to_string(),
            mod_version_id: current.mod_version_id.clone(),
            enabled,
            load_order: current.load_order,
        },
        None => {
            let load_order = existing
                .iter()
                .map(|entry| entry.load_order)
                .max()
                .map(|max| max + 1)
                .unwrap_or(0);
            ps_db::mod_profiles::ProfileModRow {
                profile_id: profile.id,
                mod_id: mod_id.to_string(),
                mod_version_id: None,
                enabled,
                load_order,
            }
        }
    };
    ps_db::mod_profiles::set_mod(db, &entry).await?;
    Ok(entry)
}

/// Stored rows can predate native spellings, so a host path read from one is
/// respelled before it goes on the wire.
fn native_path(path: &str) -> String {
    ps_core::mods::native_separators(path, cfg!(windows))
}

fn native_paths(paths: &[String]) -> Vec<String> {
    paths.iter().map(|path| native_path(path)).collect()
}

fn native_report(mut report: scan::ScanReport) -> scan::ScanReport {
    for file in &mut report.files {
        file.path = native_path(&file.path);
    }
    for candidate in &mut report.candidates {
        candidate.root = native_path(&candidate.root);
        candidate.files = native_paths(&candidate.files);
    }
    report.drifted = native_paths(&report.drifted);
    report.missing = native_paths(&report.missing);
    report.unreadable = native_paths(&report.unreadable);
    report
}

fn target_json(target: &ModTarget) -> Value {
    let layout = layout::layout_for(target).ok();
    serde_json::json!({
        "id": target.id,
        "kind": target.kind,
        "server_id": target.server_id,
        "name": target.name,
        "root_path": native_path(&target.root_path),
        "platform": target.platform,
        "ue4ss_mode": target.ue4ss_mode,
        "layout_overrides": serde_json::from_str::<Value>(&target.layout_overrides)
            .unwrap_or(Value::Null),
        "detected": serde_json::from_str::<Value>(&target.detected).unwrap_or(Value::Null),
        "last_scanned_at": target.last_scanned_at,
        // A target whose layout will not resolve is still listed. The UI has to be
        // able to show it in order to let the user fix it.
        "layout": layout.map(|l| serde_json::json!({
            "ue4ss_mods_dir": l.ue4ss_mods_dir,
            "palschema_mods_dir": l.palschema_mods_dir,
            "paks_mods_dir": l.paks_mods_dir,
            "logicmods_dir": l.logicmods_dir,
            "nativemods_dir": l.nativemods_dir,
            "workshop_local_dir": l.workshop_local_dir,
            "mods_txt": l.mods_txt,
            "palmodsettings_ini": l.palmodsettings_ini,
        })),
    })
}

pub async fn handle_mod_target_list(
    _data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    match ps_db::mod_targets::list(&*ctx.app.driver).await {
        Ok(targets) => {
            let json: Vec<Value> = targets.iter().map(target_json).collect();
            ctx.emitter.emit(
                MessageType::ModTargetList,
                &serde_json::json!({ "targets": json }),
            );
        }
        Err(error) => emit_refusal(
            ctx,
            MessageType::ModTargetList,
            serde_json::json!({}),
            "db",
            error.to_string(),
            json!({}),
        ),
    }
    Ok(())
}

pub async fn handle_mod_target_detect(
    _data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let detected: Vec<Value> = detect::find_installs()
        .iter()
        .map(|d| {
            serde_json::json!({
                "root": d.root,
                "platform": d.platform,
                "ue4ss_mode": d.ue4ss_mode,
                "hazards": d.hazards,
                "source": d.source,
            })
        })
        .collect();
    ctx.emitter.emit(
        MessageType::ModTargetDetect,
        &serde_json::json!({ "detected": detected }),
    );
    Ok(())
}

pub async fn handle_mod_target_add(
    data: TargetAddData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let root_path = if data.root_path == "__select__" {
        if !ctx.app.config.desktop_mode {
            emit_refusal(
                ctx,
                MessageType::ModTargetAdd,
                json!({ "root_path": data.root_path }),
                "desktop_only",
                "desktop mode is required to browse for a folder".to_string(),
                json!({}),
            );
            return Ok(());
        }
        match ctx.app.dialogs.pick_folder(None).await {
            Some(picked) => picked.to_string_lossy().into_owned(),
            None => {
                ctx.emitter.emit(
                    MessageType::ModTargetAdd,
                    &json!({ "root_path": data.root_path, "canceled": true }),
                );
                return Ok(());
            }
        }
    } else {
        data.root_path.clone()
    };
    let context = serde_json::json!({ "root_path": root_path });
    let picked = std::path::Path::new(&root_path);
    let described = detect::normalise_root(picked)
        .and_then(|root| detect::describe(&root, "manual").map(|d| (root, d)));
    let Some((root, described)) = described else {
        emit_refusal(
            ctx,
            MessageType::ModTargetAdd,
            context,
            "not_an_install",
            format!("{} is not a Palworld installation", picked.display()),
            json!({}),
        );
        return Ok(());
    };
    let name = data.name.unwrap_or_else(|| {
        root.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Palworld".to_string())
    });
    match detect::register(&*ctx.app.driver, &described, &name).await {
        Ok(target) => ctx.emitter.emit(
            MessageType::ModTargetAdd,
            &serde_json::json!({ "target": target_json(&target) }),
        ),
        Err(error) => emit_refusal(
            ctx,
            MessageType::ModTargetAdd,
            context,
            "register_failed",
            error.to_string(),
            json!({}),
        ),
    }
    Ok(())
}

pub async fn handle_mod_target_remove(
    services: &ServerServices,
    library: &LibraryPaths,
    data: TargetIdData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let context = serde_json::json!({ "target_id": data.target_id });
    let Some(target) = target_or_refusal(
        ctx,
        MessageType::ModTargetRemove,
        &data.target_id,
        context.clone(),
    )
    .await?
    else {
        return Ok(());
    };
    let Some(_apply_guard) = apply_lock_or_refusal(
        ctx,
        library,
        &target,
        MessageType::ModTargetRemove,
        context.clone(),
    ) else {
        return Ok(());
    };
    // Server targets are recreated on every startup by
    // `mod_target_service::ensure_all`; removing one here would reappear and
    // orphan its deployment rows in the meantime.
    if target.kind == "server" {
        emit_refusal(
            ctx,
            MessageType::ModTargetRemove,
            context,
            "server_target",
            format!(
                "{} belongs to a server and cannot be removed here; delete the server instead",
                target.id
            ),
            json!({}),
        );
        return Ok(());
    }
    match ps_db::mod_targets::remove(&*ctx.app.driver, &data.target_id).await {
        Ok(removed) => {
            if let Err(error) = library::drop_released(&*ctx.app.driver, &data.target_id).await {
                tracing::warn!(%error, target_id = %data.target_id, "failed to drop released workshop packages");
            }
            services.verification.remove(&data.target_id);
            ctx.emitter.emit(
                MessageType::ModTargetRemove,
                &serde_json::json!({ "target_id": data.target_id, "removed": removed }),
            );
        }
        Err(error) => emit_refusal(
            ctx,
            MessageType::ModTargetRemove,
            context,
            "db",
            error.to_string(),
            json!({}),
        ),
    }
    Ok(())
}

pub async fn handle_mod_target_scan(
    data: TargetScanData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(target) = target_or_refusal(
        ctx,
        MessageType::ModTargetScan,
        &data.target_id,
        json!({ "target_id": data.target_id }),
    )
    .await?
    else {
        return Ok(());
    };
    match scan::scan_with(&*ctx.app.driver, &target, data.candidates_only).await {
        Ok(report) => ctx.emitter.emit(
            MessageType::ModTargetScan,
            &serde_json::to_value(native_report(report)).unwrap_or(Value::Null),
        ),
        Err(error) => emit_refusal(
            ctx,
            MessageType::ModTargetScan,
            serde_json::json!({ "target_id": data.target_id }),
            "scan_failed",
            error.to_string(),
            json!({}),
        ),
    }
    Ok(())
}

pub async fn handle_mod_adopt(
    library: &LibraryPaths,
    data: AdoptData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let context = serde_json::json!({
        "target_id": data.target_id,
        "candidate_name": data.candidate_name,
        "source": data.source,
        "root": data.root,
    });
    let Some(target) =
        target_or_refusal(ctx, MessageType::ModAdopt, &data.target_id, context.clone()).await?
    else {
        return Ok(());
    };
    let report = match scan::scan_with(&*ctx.app.driver, &target, true).await {
        Ok(report) => report,
        Err(error) => {
            emit_refusal(
                ctx,
                MessageType::ModAdopt,
                context,
                "scan_failed",
                error.to_string(),
                json!({}),
            );
            return Ok(());
        }
    };
    let case_insensitive = cfg!(any(windows, target_os = "macos"));
    let wanted_root = data.root.as_deref().map(|root| {
        let lexical = std::path::Path::new(root).components().all(|component| {
            !matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        });
        lexical.then(|| ps_core::mods::normalize_physical_path(root, case_insensitive))
    });
    let Some(candidate) = report.candidates.iter().find(|c| {
        c.name == data.candidate_name
            && data
                .source
                .as_ref()
                .is_none_or(|source| &c.source == source)
            && wanted_root.as_ref().is_none_or(|root| {
                root.as_deref()
                    == Some(
                        ps_core::mods::normalize_physical_path(&c.root, case_insensitive).as_str(),
                    )
            })
    }) else {
        emit_refusal(
            ctx,
            MessageType::ModAdopt,
            context,
            "not_a_candidate",
            format!(
                "{} is not an adoption candidate on {}",
                data.candidate_name, data.target_id
            ),
            json!({}),
        );
        return Ok(());
    };
    match adopt::adopt(&*ctx.app.driver, library, &target, candidate).await {
        Ok(adopted) => ctx.emitter.emit(
            MessageType::ModAdopt,
            &serde_json::json!({
                "target_id": data.target_id,
                "candidate_name": data.candidate_name,
                "source": data.source,
                "root": data.root,
                "mod_id": adopted.mod_id,
                "version_id": adopted.version_id,
                "files": adopted.files,
                "enabled": adopted.enabled,
            }),
        ),
        Err(error) => {
            let code = match &error {
                adopt::AdoptError::AlreadyManaged(_) => "already_managed",
                adopt::AdoptError::NoActiveProfile(_) => "no_active_profile",
                _ => "adopt_failed",
            };
            emit_refusal(
                ctx,
                MessageType::ModAdopt,
                context,
                code,
                error.to_string(),
                json!({}),
            );
        }
    }
    Ok(())
}

pub async fn handle_mod_analyze(
    data: AnalyzeData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(target) = target_or_refusal(
        ctx,
        MessageType::ModAnalyze,
        &data.target_id,
        json!({ "target_id": data.target_id, "path": data.path }),
    )
    .await?
    else {
        return Ok(());
    };
    let Some(resolved_path) = resolve_archive_path(
        ctx,
        MessageType::ModAnalyze,
        &data.target_id,
        &data.path,
        ("Mod archives", ARCHIVE_EXTENSIONS),
    )
    .await
    else {
        return Ok(());
    };
    let spec = match layout::spec_for(&target) {
        Ok(spec) => spec,
        Err(error) => {
            emit_refusal(
                ctx,
                MessageType::ModAnalyze,
                json!({ "path": resolved_path, "target_id": data.target_id }),
                "layout_error",
                error.to_string(),
                json!({}),
            );
            return Ok(());
        }
    };
    let archive = std::path::PathBuf::from(&resolved_path);
    let request = install_request(&archive, &spec, None, true);
    match install::analyze_archive(&request).await {
        Ok(manifest) => ctx.emitter.emit(
            MessageType::ModAnalyze,
            &json!({ "path": resolved_path, "target_id": data.target_id, "manifest": manifest }),
        ),
        Err(error) => emit_refusal(
            ctx,
            MessageType::ModAnalyze,
            json!({ "path": resolved_path, "target_id": data.target_id }),
            install_error_code(&error),
            error.to_string(),
            json!({}),
        ),
    }
    Ok(())
}

pub async fn handle_mod_install(
    library: &LibraryPaths,
    data: InstallData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(target) = target_or_refusal(
        ctx,
        MessageType::ModInstall,
        &data.target_id,
        json!({ "target_id": data.target_id, "path": data.path }),
    )
    .await?
    else {
        return Ok(());
    };
    let Some(resolved_path) = resolve_archive_path(
        ctx,
        MessageType::ModInstall,
        &data.target_id,
        &data.path,
        ("Mod archives", ARCHIVE_EXTENSIONS),
    )
    .await
    else {
        return Ok(());
    };
    let spec = match layout::spec_for(&target) {
        Ok(spec) => spec,
        Err(error) => {
            emit_refusal(
                ctx,
                MessageType::ModInstall,
                json!({ "path": resolved_path, "target_id": data.target_id }),
                "layout_error",
                error.to_string(),
                json!({}),
            );
            return Ok(());
        }
    };
    let archive = std::path::PathBuf::from(&resolved_path);
    install_archive_and_reply(
        ctx,
        library,
        ArchiveInstall {
            message_type: MessageType::ModInstall,
            context: json!({ "path": resolved_path, "target_id": data.target_id }),
            target: &target,
            spec: &spec,
            archive: &archive,
            provenance: install::Provenance::Local,
            custom_name: data.custom_name.as_deref(),
            accept_defaults: data.accept_defaults,
            enable: data.enable,
        },
    )
    .await;
    Ok(())
}

pub async fn handle_mod_list(_data: Value, ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let db = &*ctx.app.driver;
    let rows = match ps_db::mod_library::list_mods(db).await {
        Ok(rows) => rows,
        Err(error) => {
            emit_refusal(
                ctx,
                MessageType::ModList,
                json!({}),
                "db",
                error.to_string(),
                json!({}),
            );
            return Ok(());
        }
    };
    let mut mods = Vec::with_capacity(rows.len());
    for row in &rows {
        match mod_json(db, row).await {
            Ok(value) => mods.push(value),
            Err(error) => {
                emit_refusal(
                    ctx,
                    MessageType::ModList,
                    json!({}),
                    "db",
                    error.to_string(),
                    json!({}),
                );
                return Ok(());
            }
        }
    }
    ctx.emitter
        .emit(MessageType::ModList, &json!({ "mods": mods }));
    Ok(())
}

/// The mod is refused while any profile of any target still holds it -- not
/// only the active one -- even though nothing has necessarily been deployed
/// yet: `mod_usage` alone only sees `deployment_files`/`target_frameworks`,
/// and a profile entry from `mod_install`'s `enable` step writes only
/// `profile_mods`. A database error partway through the scan refuses rather
/// than being treated as "not in use": a destructive operation must never
/// fail open.
pub async fn handle_mod_remove(
    library: &LibraryPaths,
    data: ModIdData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let db = &*ctx.app.driver;
    let context = json!({ "mod_id": data.mod_id });

    let subscribed = match library::is_subscribed_mod(db, &data.mod_id).await {
        Ok(subscribed) => subscribed,
        Err(error) => {
            emit_refusal(
                ctx,
                MessageType::ModRemove,
                context,
                "db",
                error.to_string(),
                json!({}),
            );
            return Ok(());
        }
    };
    let holders = match holders_of(db, &data.mod_id).await {
        Ok(holders) => holders,
        Err(error) => {
            emit_refusal(
                ctx,
                MessageType::ModRemove,
                context,
                "db",
                error.to_string(),
                json!({}),
            );
            return Ok(());
        }
    };
    remove_held(library, &data.mod_id, subscribed, holders, ctx).await;
    Ok(())
}

pub(crate) struct Holder {
    target_id: String,
    target_name: String,
    profile_id: String,
    profile_name: String,
    enabled: bool,
}

pub(crate) struct Holders {
    deployed_anywhere: bool,
    deployed: Vec<String>,
    frameworks: Vec<String>,
    targets: std::collections::BTreeSet<String>,
    profiles: std::collections::BTreeSet<String>,
    holders: Vec<Holder>,
    all_targets: Vec<ModTarget>,
}

impl Holders {
    fn in_use(&self, subscribed: bool) -> bool {
        !(subscribed && !self.deployed_anywhere)
            && (!self.targets.is_empty() || !self.profiles.is_empty())
    }

    fn target_name(&self, target_id: &str) -> String {
        self.all_targets
            .iter()
            .find(|target| target.id == target_id)
            .map_or_else(|| target_id.to_string(), |target| target.name.clone())
    }

    fn usage(&self) -> Value {
        json!({
            "targets": self.targets.iter().collect::<Vec<_>>(),
            "profiles": self.profiles.iter().collect::<Vec<_>>(),
            "holders": self.holders.iter().map(|holder| json!({
                "target_id": holder.target_id,
                "target_name": holder.target_name,
                "profile_id": holder.profile_id,
                "profile_name": holder.profile_name,
                "enabled": holder.enabled,
            })).collect::<Vec<_>>(),
            "deployed": self.deployed.iter().map(|target_id| json!({
                "target_id": target_id,
                "target_name": self.target_name(target_id),
            })).collect::<Vec<_>>(),
            "frameworks": self.frameworks.iter().map(|target_id| json!({
                "target_id": target_id,
                "target_name": self.target_name(target_id),
            })).collect::<Vec<_>>(),
        })
    }
}

pub(crate) struct HeldEntry {
    pub target: ModTarget,
    pub profile: ps_db::mod_profiles::ProfileRow,
    pub entry: ps_db::mod_profiles::ProfileModRow,
}

/// Every profile entry, on every target, that holds `mod_id` -- enabled or
/// not. `holders_of` and `release_disabled` both need this walk, so it exists
/// exactly once.
pub(crate) async fn holding_entries(
    db: &dyn ps_db::DbDriver,
    mod_id: &str,
) -> Result<Vec<HeldEntry>, ps_db::DbError> {
    let mut held = Vec::new();
    for target in ps_db::mod_targets::list(db).await? {
        for profile in ps_db::mod_profiles::for_target(db, &target.id).await? {
            let entries = ps_db::mod_profiles::mods_of(db, &profile.id).await?;
            if let Some(entry) = entries.into_iter().find(|entry| entry.mod_id == mod_id) {
                held.push(HeldEntry {
                    target: target.clone(),
                    profile,
                    entry,
                });
            }
        }
    }
    Ok(held)
}

/// Every target the mod is deployed on or any of whose profiles holds it,
/// enabled or not, and every profile entry holding it. `deployed` only counts
/// `deployment_files`: a framework slot in `target_frameworks` makes
/// `deployed_anywhere` true (through `mod_usage`) but is not itself a deployed
/// file of this mod; `frameworks` lists those slots instead.
pub(crate) async fn holders_of(
    db: &dyn ps_db::DbDriver,
    mod_id: &str,
) -> Result<Holders, ps_db::DbError> {
    let usage_targets = ps_db::mod_library::mod_usage(db, mod_id).await?;
    let deployed_anywhere = !usage_targets.is_empty();
    let deployed = ps_db::mod_deployments::deployed_targets_of(db, mod_id).await?;
    let frameworks = ps_db::mod_profiles::framework_targets_of(db, mod_id).await?;
    let all_targets = ps_db::mod_targets::list(db).await?;
    let mut targets: std::collections::BTreeSet<String> = usage_targets.into_iter().collect();
    let mut profiles = std::collections::BTreeSet::new();
    let mut holders = Vec::new();
    for held in holding_entries(db, mod_id).await? {
        targets.insert(held.target.id.clone());
        profiles.insert(held.profile.id.clone());
        holders.push(Holder {
            target_id: held.target.id,
            target_name: held.target.name,
            profile_id: held.profile.id,
            profile_name: held.profile.name,
            enabled: held.entry.enabled,
        });
    }
    holders.sort_by(|a, b| {
        (a.target_id.as_str(), a.profile_id.as_str())
            .cmp(&(b.target_id.as_str(), b.profile_id.as_str()))
    });
    Ok(Holders {
        deployed_anywhere,
        deployed,
        frameworks,
        targets,
        profiles,
        holders,
        all_targets,
    })
}

/// Locks every holder `scanned` found, then scans again: a target that took the
/// mod up in between was never locked, so the removal is refused rather than
/// leaving it a line nothing would remove.
pub(crate) async fn remove_held(
    library: &LibraryPaths,
    mod_id: &str,
    subscribed: bool,
    scanned: Holders,
    ctx: &mut HandlerCtx<'_>,
) {
    let db = &*ctx.app.driver;
    let context = json!({ "mod_id": mod_id });
    if scanned.in_use(subscribed) {
        emit_refusal(
            ctx,
            MessageType::ModRemove,
            context,
            "version_in_use",
            format!("mod {mod_id} is in use"),
            scanned.usage(),
        );
        return;
    }

    // Held through the release and the delete, so an apply cannot forget a release
    // it never planned with.
    let mut guards = Vec::new();
    for target in scanned
        .all_targets
        .iter()
        .filter(|t| scanned.targets.contains(&t.id))
    {
        match deploy::try_lock_target(library, target) {
            Some(guard) => guards.push(guard),
            None => {
                emit_refusal(
                    ctx,
                    MessageType::ModRemove,
                    context,
                    "apply_in_progress",
                    format!(
                        "{} is busy with an apply or another removal; try again when it finishes",
                        target.id
                    ),
                    json!({ "target_id": target.id }),
                );
                return;
            }
        }
    }
    let rescanned = match holders_of(db, mod_id).await {
        Ok(holders) => holders,
        Err(error) => {
            emit_refusal(
                ctx,
                MessageType::ModRemove,
                context,
                "db",
                error.to_string(),
                json!({}),
            );
            return;
        }
    };
    if rescanned.in_use(subscribed) || !rescanned.targets.is_subset(&scanned.targets) {
        emit_refusal(
            ctx,
            MessageType::ModRemove,
            context,
            "version_in_use",
            format!("mod {mod_id} is in use"),
            rescanned.usage(),
        );
        return;
    }
    let holders: Vec<String> = scanned.targets.iter().cloned().collect();
    if let Err(error) = library::release_workshop_packages(db, mod_id, &holders).await {
        emit_refusal(
            ctx,
            MessageType::ModRemove,
            context,
            "db",
            error.to_string(),
            json!({}),
        );
        return;
    }
    let deleted = library::delete_mod(db, library, mod_id).await;
    drop(guards);
    match deleted {
        Ok(()) => ctx.emitter.emit(
            MessageType::ModRemove,
            &json!({ "mod_id": mod_id, "removed": true }),
        ),
        Err(error) => emit_refusal(
            ctx,
            MessageType::ModRemove,
            context,
            "remove_failed",
            error.to_string(),
            json!({}),
        ),
    }
}

/// Refuses a version operation on a Steam-subscribed package, which has one
/// synthetic version and nothing to switch to or delete. Reports whether it did,
/// and fails closed on a database error.
async fn subscribed_refusal(
    ctx: &mut HandlerCtx<'_>,
    message_type: MessageType,
    context: Value,
    mod_id: Option<&str>,
    version_id: &str,
) -> bool {
    let db = &*ctx.app.driver;
    let owner = match ps_db::mod_library::get_version(db, version_id).await {
        Ok(version) => version.map(|version| version.mod_id),
        Err(error) => {
            emit_refusal(
                ctx,
                message_type,
                context,
                "db",
                error.to_string(),
                json!({}),
            );
            return true;
        }
    };
    for candidate in mod_id.into_iter().map(str::to_string).chain(owner) {
        match library::is_subscribed_mod(db, &candidate).await {
            Ok(false) => {}
            Ok(true) => {
                emit_refusal(
                    ctx,
                    message_type,
                    context,
                    "subscribed_package",
                    format!(
                        "{candidate} is a Steam-subscribed Workshop package; Steam owns its versions"
                    ),
                    json!({ "mod_id": candidate }),
                );
                return true;
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
                return true;
            }
        }
    }
    false
}

pub async fn handle_mod_version_set_current(
    data: SetCurrentData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let context = json!({ "mod_id": data.mod_id, "version_id": data.version_id });
    if subscribed_refusal(
        ctx,
        MessageType::ModVersionSetCurrent,
        context.clone(),
        Some(&data.mod_id),
        &data.version_id,
    )
    .await
    {
        return Ok(());
    }
    let db = &*ctx.app.driver;
    match ps_db::mod_library::set_current_version(db, &data.mod_id, &data.version_id).await {
        Ok(()) => ctx.emitter.emit(
            MessageType::ModVersionSetCurrent,
            &json!({ "mod_id": data.mod_id, "version_id": data.version_id }),
        ),
        Err(error) => emit_refusal(
            ctx,
            MessageType::ModVersionSetCurrent,
            context,
            "db",
            error.to_string(),
            json!({}),
        ),
    }
    Ok(())
}

pub async fn handle_mod_version_delete(
    data: VersionIdData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let context = json!({ "version_id": data.version_id });
    if subscribed_refusal(
        ctx,
        MessageType::ModVersionDelete,
        context.clone(),
        None,
        &data.version_id,
    )
    .await
    {
        return Ok(());
    }
    let db = &*ctx.app.driver;
    let usage = match ps_db::mod_library::version_usage(db, &data.version_id).await {
        Ok(usage) => usage,
        Err(error) => {
            emit_refusal(
                ctx,
                MessageType::ModVersionDelete,
                context,
                "db",
                error.to_string(),
                json!({}),
            );
            return Ok(());
        }
    };
    if usage.is_in_use() {
        emit_refusal(
            ctx,
            MessageType::ModVersionDelete,
            context,
            "version_in_use",
            format!("mod version {} is in use", data.version_id),
            json!({
                "is_current": usage.is_current,
                "targets": usage.deployed_targets,
                "profiles": usage.pinned_profiles,
                "frameworks": usage.frameworks,
                "open_applies": usage.open_applies,
            }),
        );
        return Ok(());
    }
    match library::delete_version(db, &data.version_id).await {
        Ok(()) => ctx.emitter.emit(
            MessageType::ModVersionDelete,
            &json!({ "version_id": data.version_id, "removed": true }),
        ),
        Err(error) => emit_refusal(
            ctx,
            MessageType::ModVersionDelete,
            context,
            "remove_failed",
            error.to_string(),
            json!({}),
        ),
    }
    Ok(())
}

pub async fn handle_mod_backup_list(
    library: &LibraryPaths,
    data: BackupTargetData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(_target) =
        backup_target_or_refusal(ctx, MessageType::ModBackupList, &data.target_id).await?
    else {
        return Ok(());
    };
    let dir = library.backups_dir(&data.target_id);
    let mut sets: Vec<(String, u64, Vec<deploy::backup::BackupEntry>)> = Vec::new();
    if dir.exists() {
        let read = match std::fs::read_dir(&dir) {
            Ok(read) => read,
            Err(error) => {
                emit_refusal(
                    ctx,
                    MessageType::ModBackupList,
                    json!({ "target_id": data.target_id }),
                    "io_error",
                    error.to_string(),
                    json!({}),
                );
                return Ok(());
            }
        };
        for entry in read.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let size_bytes = digest::dir_size(&path).unwrap_or(0);
            let entries: Vec<deploy::backup::BackupEntry> =
                std::fs::read_to_string(path.join("index.json"))
                    .ok()
                    .and_then(|text| serde_json::from_str(&text).ok())
                    .unwrap_or_default();
            sets.push((name, size_bytes, entries));
        }
    }
    // Set names are `{stamp}-{op_id}`; the timestamp prefix sorts lexically, so
    // reversing the name order is newest first.
    sets.sort_by(|a, b| b.0.cmp(&a.0));
    let sets_json: Vec<Value> = sets
        .into_iter()
        .map(|(name, size_bytes, mut entries)| {
            for entry in &mut entries {
                entry.original_path = native_path(&entry.original_path);
            }
            json!({ "name": name, "size_bytes": size_bytes, "entries": entries })
        })
        .collect();
    ctx.emitter.emit(
        MessageType::ModBackupList,
        &json!({ "target_id": data.target_id, "sets": sets_json }),
    );
    Ok(())
}

pub async fn handle_mod_backup_restore(
    services: &ServerServices,
    library: &LibraryPaths,
    data: BackupRestoreData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(target) =
        backup_target_or_refusal(ctx, MessageType::ModBackupRestore, &data.target_id).await?
    else {
        return Ok(());
    };
    let context = json!({ "target_id": data.target_id, "set": data.set });
    let Some(_apply_guard) = apply_lock_or_refusal(
        ctx,
        library,
        &target,
        MessageType::ModBackupRestore,
        context.clone(),
    ) else {
        return Ok(());
    };
    match running::target_is_running(services, &*ctx.app.driver, &target).await {
        Ok(false) => {}
        Ok(true) => {
            emit_refusal(
                ctx,
                MessageType::ModBackupRestore,
                context,
                "target_locked",
                format!("the game or server for {} is running", target.id),
                json!({}),
            );
            return Ok(());
        }
        Err(error) => {
            emit_refusal(
                ctx,
                MessageType::ModBackupRestore,
                context,
                "db",
                error.to_string(),
                json!({}),
            );
            return Ok(());
        }
    }
    if !is_safe_id(&data.set) {
        emit_refusal(
            ctx,
            MessageType::ModBackupRestore,
            context,
            "invalid_set",
            format!("{} is not a valid backup set name", data.set),
            json!({}),
        );
        return Ok(());
    }
    let set_dir = library.backups_dir(&data.target_id).join(&data.set);
    let entries: Vec<deploy::backup::BackupEntry> =
        match std::fs::read_to_string(set_dir.join("index.json")) {
            Ok(text) => match serde_json::from_str(&text) {
                Ok(entries) => entries,
                Err(error) => {
                    emit_refusal(
                        ctx,
                        MessageType::ModBackupRestore,
                        context,
                        "index_unreadable",
                        error.to_string(),
                        json!({}),
                    );
                    return Ok(());
                }
            },
            Err(error) => {
                emit_refusal(
                    ctx,
                    MessageType::ModBackupRestore,
                    context,
                    "index_unreadable",
                    format!("no readable index for set {}: {error}", data.set),
                    json!({}),
                );
                return Ok(());
            }
        };

    let bases = restore_bases(&target);
    let mut restored = Vec::new();
    let mut skipped = Vec::new();
    for entry in &entries {
        if let Some(restrict) = &data.paths {
            let wanted = restrict.iter().any(|path| {
                std::path::Path::new(path) == std::path::Path::new(&entry.original_path)
            });
            if !wanted {
                continue;
            }
        }
        if !is_valid_backup_key(&entry.backup_key) {
            skipped.push(json!({ "path": native_path(&entry.original_path), "reason": "invalid_backup_key" }));
            continue;
        }
        let original = std::path::Path::new(&entry.original_path);
        if !is_lexically_contained(original) || !is_under_any(original, &bases) {
            skipped.push(
                json!({ "path": native_path(&entry.original_path), "reason": "outside_target" }),
            );
            continue;
        }
        match deploy::hash_if_present(original) {
            Ok(Some(_)) | Err(_) => {
                skipped.push(
                    json!({ "path": native_path(&entry.original_path), "reason": "occupied" }),
                );
                continue;
            }
            Ok(None) => {}
        }
        let blob = set_dir.join(&entry.backup_key);
        let matches_hash = digest::hash_file(&blob)
            .map(|actual| actual == entry.hash)
            .unwrap_or(false);
        if !matches_hash {
            skipped.push(json!({ "path": native_path(&entry.original_path), "reason": "changed" }));
            continue;
        }
        if let Err(error) = deploy::write::copy_staged(&blob, original) {
            skipped.push(
                json!({ "path": native_path(&entry.original_path), "reason": error.to_string() }),
            );
            continue;
        }
        restored.push(entry.original_path.clone());
    }

    ctx.emitter.emit(
        MessageType::ModBackupRestore,
        &json!({
            "target_id": data.target_id,
            "set": data.set,
            "restored": native_paths(&restored),
            "skipped": skipped,
        }),
    );
    Ok(())
}

pub async fn handle_mod_backup_delete(
    library: &LibraryPaths,
    data: BackupSetData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(target) =
        backup_target_or_refusal(ctx, MessageType::ModBackupDelete, &data.target_id).await?
    else {
        return Ok(());
    };
    let context = json!({ "target_id": data.target_id, "set": data.set });
    let Some(_apply_guard) = apply_lock_or_refusal(
        ctx,
        library,
        &target,
        MessageType::ModBackupDelete,
        context.clone(),
    ) else {
        return Ok(());
    };
    if !is_safe_id(&data.set) {
        emit_refusal(
            ctx,
            MessageType::ModBackupDelete,
            context,
            "invalid_set",
            format!("{} is not a valid backup set name", data.set),
            json!({}),
        );
        return Ok(());
    }
    let db = &*ctx.app.driver;
    match ps_db::mod_deployments::journal_of(db, &data.target_id).await {
        Ok(Some(_)) => {
            emit_refusal(
                ctx,
                MessageType::ModBackupDelete,
                context,
                "journal_open",
                format!(
                    "target {} has an open apply; its backups cannot be deleted yet",
                    data.target_id
                ),
                json!({}),
            );
            return Ok(());
        }
        Ok(None) => {}
        Err(error) => {
            emit_refusal(
                ctx,
                MessageType::ModBackupDelete,
                context,
                "db",
                error.to_string(),
                json!({}),
            );
            return Ok(());
        }
    }
    let set_dir = library.backups_dir(&data.target_id).join(&data.set);
    if !set_dir.exists() {
        emit_refusal(
            ctx,
            MessageType::ModBackupDelete,
            context,
            "set_not_found",
            format!("no backup set {} for target {}", data.set, data.target_id),
            json!({}),
        );
        return Ok(());
    }
    match std::fs::remove_dir_all(&set_dir) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            emit_refusal(
                ctx,
                MessageType::ModBackupDelete,
                context,
                "io_error",
                error.to_string(),
                json!({}),
            );
            return Ok(());
        }
    }
    ctx.emitter.emit(
        MessageType::ModBackupDelete,
        &json!({ "target_id": data.target_id, "set": data.set, "removed": true }),
    );
    Ok(())
}

/// Distinguishes a field that is absent (`None`) from one that is explicitly
/// `null` (`Some(None)`); paired with `#[serde(default)]`.
fn explicit_option<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    <Option<String> as serde::Deserialize>::deserialize(deserializer).map(Some)
}

#[derive(Debug, serde::Deserialize)]
pub struct ProfileSetModData {
    pub target_id: String,
    pub mod_id: String,
    pub enabled: bool,
    /// Omitted keeps an existing entry's pin; `null` follows the mod's current
    /// version; a string pins that version.
    #[serde(default, deserialize_with = "explicit_option")]
    pub mod_version_id: Option<Option<String>>,
    #[serde(default)]
    pub profile_id: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ProfileApplyData {
    pub target_id: String,
    #[serde(default)]
    pub replace_occupants: Vec<String>,
}

const PLAN_OPS: &[&str] = &[
    "keep",
    "reattribute",
    "replace",
    "preserve",
    "add",
    "move",
    "remove",
    "remove_preserve",
];

fn plan_counts(plan: &ps_core::mods::DeployPlan) -> Value {
    let mut counts: serde_json::Map<String, Value> = PLAN_OPS
        .iter()
        .map(|op| (op.to_string(), json!(0)))
        .collect();
    for entry in &plan.entries {
        let op = serde_json::to_value(entry)
            .ok()
            .and_then(|v| v.get("op").and_then(Value::as_str).map(str::to_string));
        if let Some(op) = op {
            let current = counts.get(&op).and_then(Value::as_u64).unwrap_or(0);
            counts.insert(op, json!(current + 1));
        }
    }
    Value::Object(counts)
}

fn apply_error_parts(error: &deploy::apply::ApplyError) -> (&'static str, Value) {
    use deploy::apply::ApplyError;
    use ps_core::mods::PreflightError;
    match error {
        ApplyError::TargetLocked(_) => ("target_locked", json!({})),
        ApplyError::ApplyInProgress(_) => ("apply_in_progress", json!({})),
        ApplyError::NoActiveProfile(_) => ("no_active_profile", json!({})),
        ApplyError::Preflight(PreflightError::DestinationConflict {
            path,
            mod_version_ids,
        }) => (
            "destination_conflict",
            json!({ "path": native_path(path), "mod_version_ids": mod_version_ids }),
        ),
        ApplyError::Preflight(PreflightError::UnmanagedOccupant { paths }) => (
            "unmanaged_occupant",
            json!({ "paths": native_paths(paths) }),
        ),
        ApplyError::ReplacePartial { moved, source, .. } => {
            let (cause, _) = apply_error_parts(source);
            (
                "replace_partial",
                json!({ "moved": native_paths(moved), "cause": cause }),
            )
        }
        ApplyError::Db(_) => ("db", json!({})),
        ApplyError::Desired(deploy::desired::DesiredError::NoBaseForKind(kind)) => {
            ("not_supported_on_target", json!({ "kind": kind }))
        }
        ApplyError::Desired(_) => ("desired", json!({})),
        ApplyError::Layout(_) => ("layout", json!({})),
        ApplyError::Io(_) => ("io", json!({})),
    }
}

/// Puts a failed apply's `error` on the reply, and names the backup set when
/// replaced occupants were already moved before the failure.
fn insert_apply_error(
    reply: &mut serde_json::Map<String, Value>,
    error: &deploy::apply::ApplyError,
) {
    if let deploy::apply::ApplyError::ReplacePartial { backup_dir, .. } = error {
        reply.insert("backup_dir".to_string(), json!(native_path(backup_dir)));
    }
    let (code, detail) = apply_error_parts(error);
    reply.insert(
        "error".to_string(),
        error_object(code, error.to_string(), detail),
    );
}

fn error_object(code: &str, message: String, detail: Value) -> Value {
    let mut error = serde_json::Map::new();
    error.insert("code".to_string(), json!(code));
    error.insert("message".to_string(), json!(message));
    if let Value::Object(detail) = detail {
        error.extend(detail);
    }
    Value::Object(error)
}

/// The two residual states no retry can clear: a path that cannot be read, and
/// a path whose contents disagree with what the app recorded.
fn attention_reason(path: &str) -> &'static str {
    match std::fs::metadata(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => "unreadable",
        _ => "drift",
    }
}

pub(crate) fn emit_progress(
    emitter: &Emitter,
    request_id: &str,
    target_id: &str,
    stage: &str,
    pct: u8,
    message: &str,
) {
    emitter.emit(
        MessageType::ModProgress,
        &json!({
            "request_id": request_id,
            "target_id": target_id,
            "stage": stage,
            "pct": pct,
            "message": message,
        }),
    );
}

/// The body of `profile_apply`, shared with `profile_set_mod` so both produce
/// the identical reply object. Every outcome, refusals included, is a reply
/// object rather than a separate frame.
pub(crate) async fn run_apply(
    services: &ServerServices,
    library: &LibraryPaths,
    db: &dyn ps_db::DbDriver,
    emitter: &Emitter,
    target: &ModTarget,
    replace_occupants: &[std::path::PathBuf],
    report_progress: bool,
) -> Value {
    run_apply_under(
        services,
        library,
        db,
        emitter,
        target,
        replace_occupants,
        report_progress,
        ApplyLock::Take,
    )
    .await
}

/// `run_apply` under an apply lock on `target` that the caller took and keeps
/// past the reply. `None`, a lock the caller could not take, gets the reply
/// `run_apply` gives when another apply holds it.
pub(crate) async fn run_apply_locked(
    services: &ServerServices,
    library: &LibraryPaths,
    db: &dyn ps_db::DbDriver,
    emitter: &Emitter,
    target: &ModTarget,
    report_progress: bool,
    lock: Option<&tokio::sync::OwnedMutexGuard<()>>,
) -> Value {
    let lock = match lock {
        Some(guard) => ApplyLock::Held(guard),
        None => ApplyLock::Busy,
    };
    run_apply_under(
        services,
        library,
        db,
        emitter,
        target,
        &[],
        report_progress,
        lock,
    )
    .await
}

enum ApplyLock<'a> {
    Take,
    Held(&'a tokio::sync::OwnedMutexGuard<()>),
    Busy,
}

#[allow(clippy::too_many_arguments)]
async fn run_apply_under(
    services: &ServerServices,
    library: &LibraryPaths,
    db: &dyn ps_db::DbDriver,
    emitter: &Emitter,
    target: &ModTarget,
    replace_occupants: &[std::path::PathBuf],
    report_progress: bool,
    lock: ApplyLock<'_>,
) -> Value {
    let request_id = deploy::backup::new_op_id();
    let progress = |stage: &str, pct: u8, message: &str| {
        if report_progress {
            emit_progress(emitter, &request_id, &target.id, stage, pct, message);
        }
    };
    let mut reply = serde_json::Map::new();
    reply.insert("target_id".to_string(), json!(target.id));
    reply.insert("request_id".to_string(), json!(request_id));
    reply.insert("mid_apply".to_string(), json!(false));
    reply.insert("failed".to_string(), json!([]));
    reply.insert("preserved".to_string(), json!([]));
    reply.insert("new_copies".to_string(), json!([]));
    reply.insert("skipped_new_copies".to_string(), json!([]));
    reply.insert("backup_dir".to_string(), Value::Null);
    reply.insert("needs_attention".to_string(), json!([]));
    reply.insert(
        "counts".to_string(),
        plan_counts(&ps_core::mods::DeployPlan::default()),
    );

    progress("checking", 0, "checking whether the target is running");
    let running = match running::target_is_running(services, db, target).await {
        Ok(running) => running,
        Err(error) => {
            reply.insert(
                "error".to_string(),
                error_object("db", error.to_string(), json!({})),
            );
            progress("done", 100, "stopped");
            return Value::Object(reply);
        }
    };

    progress("applying", 10, "applying the active profile");
    let snapshot = running::Snapshot(running);
    let options = deploy::ApplyOptions {
        running: &snapshot,
        move_strategy: deploy::MoveStrategy::Auto,
        stamp: None,
        replace_occupants,
    };
    let applied = match lock {
        ApplyLock::Take => deploy::apply::apply(db, library, target, &options).await,
        ApplyLock::Held(guard) => {
            deploy::apply::apply_locked(db, library, target, guard, &options).await
        }
        ApplyLock::Busy => Err(deploy::apply::ApplyError::ApplyInProgress(
            target.id.clone(),
        )),
    };
    match applied {
        Ok(outcome) => {
            let needs_attention: Vec<Value> = outcome
                .failed
                .iter()
                .map(|path| json!({ "path": native_path(path), "reason": attention_reason(path) }))
                .collect();
            reply.insert("mid_apply".to_string(), json!(outcome.mid_apply));
            reply.insert("failed".to_string(), json!(native_paths(&outcome.failed)));
            reply.insert(
                "preserved".to_string(),
                json!(native_paths(&outcome.preserved)),
            );
            reply.insert(
                "new_copies".to_string(),
                json!(native_paths(&outcome.new_copies)),
            );
            reply.insert(
                "skipped_new_copies".to_string(),
                json!(native_paths(&outcome.skipped_new_copies)),
            );
            reply.insert(
                "backup_dir".to_string(),
                json!(outcome.backup_dir.as_deref().map(native_path)),
            );
            reply.insert("needs_attention".to_string(), json!(needs_attention));
            reply.insert("counts".to_string(), plan_counts(&outcome.plan));
            if let Some(error) = &outcome.error {
                insert_apply_error(&mut reply, error);
            }
        }
        Err(error) => insert_apply_error(&mut reply, &error),
    }
    progress("done", 100, "finished");
    Value::Object(reply)
}

/// A route kind the mod needs that this target's layout can never hold, whatever
/// its UE4SS mode: a Workshop package on a Docker server, a native DLL on a
/// client. A Workshop package is checked even with no files of its own, since it
/// is activated through a settings file only a Workshop-capable layout has.
async fn unsupported_kind(
    db: &dyn ps_db::DbDriver,
    target: &ModTarget,
    mod_id: &str,
    pinned: Option<&str>,
) -> Result<Option<ps_core::mods::RouteKind>, ps_db::DbError> {
    use ps_core::mods::{ModType, RouteKind};
    let version = match pinned {
        Some(id) => ps_db::mod_library::get_version(db, id).await?,
        None => ps_db::mod_library::current_version(db, mod_id).await?,
    };
    let Some(version) = version else {
        return Ok(None);
    };
    let Ok(manifest) = serde_json::from_str::<ps_core::mods::InstallManifest>(&version.manifest)
    else {
        return Ok(None);
    };
    let Ok(mut spec) = layout::spec_for(target) else {
        return Ok(None);
    };
    spec.ue4ss_mode = ps_core::mods::Ue4ssMode::Standard;
    let Ok(capable) = ps_core::mods::resolve_layout(&spec) else {
        return Ok(None);
    };
    let package = (manifest.mod_type == ModType::Workshop).then_some(RouteKind::Workshop);
    Ok(manifest
        .routes
        .iter()
        .map(|route| route.kind)
        .chain(package)
        .filter(|kind| {
            !matches!(
                kind,
                RouteKind::Companion | RouteKind::Framework | RouteKind::Passthrough
            )
        })
        .find(|kind| capable.base_for(*kind).is_none()))
}

/// The Steam item a subscribed mod names when this target's Workshop directory
/// does not hold it, and whether that directory could be found at all.
async fn missing_subscription(
    db: &dyn ps_db::DbDriver,
    target: &ModTarget,
    mod_id: &str,
) -> Result<Option<(String, bool)>, ps_db::DbError> {
    let Some(row) = ps_db::mod_library::get_mod(db, mod_id).await? else {
        return Ok(None);
    };
    let Some(workshop_id) = library::workshop_id_of(&row).filter(|_| library::is_subscribed(&row))
    else {
        return Ok(None);
    };
    let dir = match scan::steam_workshop_dir(db, target).await {
        Ok(dir) => dir,
        Err(scan::ScanError::Db(error)) => return Err(error),
        Err(_) => None,
    };
    Ok(match dir {
        None => Some((workshop_id, false)),
        Some(dir) if dir.join(&workshop_id).is_dir() => None,
        Some(_) => Some((workshop_id, true)),
    })
}

pub(crate) type Refusal = (&'static str, String, Value);

/// Why enabling `mod_id` on `target` is refused before anything is written. A
/// missing version or manifest stays permissive, because the apply refuses it
/// with a precise code; a database error is returned rather than let through.
pub(crate) async fn enable_refusal(
    db: &dyn ps_db::DbDriver,
    target: &ModTarget,
    mod_id: &str,
    pinned: Option<&str>,
) -> Result<Option<Refusal>, ps_db::DbError> {
    if let Some(row) = ps_db::mod_library::get_mod(db, mod_id).await? {
        if row.mod_type == "framework" {
            return Ok(Some((
                "framework_mod",
                format!("{mod_id} is a framework; install it on the target instead"),
                json!({}),
            )));
        }
    }
    if let Some(kind) = unsupported_kind(db, target, mod_id, pinned).await? {
        let kind_name = json!(kind);
        return Ok(Some((
            "not_supported_on_target",
            format!(
                "{} has no place for {} files",
                target.id,
                kind_name.as_str().unwrap_or_default()
            ),
            json!({ "kind": kind }),
        )));
    }
    if let Some((workshop_id, dir_found)) = missing_subscription(db, target, mod_id).await? {
        let message = if dir_found {
            format!(
                "Steam Workshop item {workshop_id} is not in {}'s Workshop directory",
                target.id
            )
        } else {
            format!(
                "{}'s Steam Workshop directory is unknown, so item {workshop_id} cannot be found",
                target.id
            )
        };
        return Ok(Some((
            "not_subscribed_on_target",
            message,
            json!({ "workshop_id": workshop_id }),
        )));
    }
    Ok(None)
}

/// `enable_refusal` for the version `enable_in_active_profile` would keep pinned.
async fn install_enable_refusal(
    db: &dyn ps_db::DbDriver,
    target: &ModTarget,
    mod_id: &str,
) -> Result<Option<Refusal>, ps_db::DbError> {
    let pinned = match ps_db::mod_profiles::active_for_target(db, &target.id).await? {
        Some(profile) => ps_db::mod_profiles::mods_of(db, &profile.id)
            .await?
            .into_iter()
            .find(|entry| entry.mod_id == mod_id)
            .and_then(|entry| entry.mod_version_id),
        None => None,
    };
    enable_refusal(db, target, mod_id, pinned.as_deref()).await
}

async fn active_profile_or_refusal(
    ctx: &mut HandlerCtx<'_>,
    message_type: MessageType,
    context: Value,
    target_id: &str,
) -> Option<ps_db::mod_profiles::ProfileRow> {
    match ps_db::mod_profiles::active_for_target(&*ctx.app.driver, target_id).await {
        Ok(Some(profile)) => Some(profile),
        Ok(None) => {
            emit_refusal(
                ctx,
                message_type,
                context,
                "no_active_profile",
                format!("target {target_id} has no active profile"),
                json!({}),
            );
            None
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
            None
        }
    }
}

fn profile_mod_json(entry: &ps_db::mod_profiles::ProfileModRow) -> Value {
    json!({
        "profile_id": entry.profile_id,
        "mod_id": entry.mod_id,
        "mod_version_id": entry.mod_version_id,
        "enabled": entry.enabled,
        "load_order": entry.load_order,
    })
}

pub(crate) async fn profile_json(
    db: &dyn ps_db::DbDriver,
    profile: &ps_db::mod_profiles::ProfileRow,
) -> Result<Value, ps_db::DbError> {
    let entries = ps_db::mod_profiles::mods_of(db, &profile.id).await?;
    let worlds = ps_db::mod_profiles::worlds_for_profile(db, &profile.id).await?;
    Ok(json!({
        "id": profile.id,
        "target_id": profile.target_id,
        "name": profile.name,
        "is_active": profile.is_active,
        "is_default": profile.is_default,
        "ue4ss_control_mode": profile.ue4ss_control_mode,
        "force_order_ue4ss": profile.force_order_ue4ss,
        "force_order_palschema": profile.force_order_palschema,
        "created_at": profile.created_at,
        "updated_at": profile.updated_at,
        "mods": entries.iter().map(profile_mod_json).collect::<Vec<_>>(),
        "worlds": worlds
            .iter()
            .map(|w| json!({ "world_key": w.world_key, "world_name": w.world_name }))
            .collect::<Vec<_>>(),
    }))
}

/// The reply fields a committed selection change carries: `pending` means the
/// apply was refused because the target is running or another apply holds it.
pub(crate) struct SelectionApply {
    pub request_id: Value,
    pub pending: bool,
    pub apply: Value,
}

impl SelectionApply {
    pub(crate) fn none() -> Self {
        Self {
            request_id: Value::Null,
            pending: false,
            apply: Value::Null,
        }
    }
}

pub(crate) fn selection_of(apply: Value) -> SelectionApply {
    let request_id = apply.get("request_id").cloned().unwrap_or(Value::Null);
    let pending = matches!(
        apply
            .get("error")
            .and_then(|error| error.get("code"))
            .and_then(Value::as_str),
        Some("target_locked" | "apply_in_progress")
    );
    SelectionApply {
        request_id,
        pending,
        apply: if pending { Value::Null } else { apply },
    }
}

pub(crate) async fn apply_after_selection(
    services: &ServerServices,
    library: &LibraryPaths,
    db: &dyn ps_db::DbDriver,
    emitter: &Emitter,
    target: &ModTarget,
) -> SelectionApply {
    selection_of(run_apply(services, library, db, emitter, target, &[], true).await)
}

pub async fn handle_profile_list(
    data: TargetIdData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(_target) = target_or_refusal(
        ctx,
        MessageType::ProfileList,
        &data.target_id,
        json!({ "target_id": data.target_id }),
    )
    .await?
    else {
        return Ok(());
    };
    let db = &*ctx.app.driver;
    let context = json!({ "target_id": data.target_id });
    let rows = match ps_db::mod_profiles::for_target(db, &data.target_id).await {
        Ok(rows) => rows,
        Err(error) => {
            emit_refusal(
                ctx,
                MessageType::ProfileList,
                context,
                "db",
                error.to_string(),
                json!({}),
            );
            return Ok(());
        }
    };
    let mut profiles = Vec::with_capacity(rows.len());
    for profile in &rows {
        match profile_json(db, profile).await {
            Ok(json) => profiles.push(json),
            Err(error) => {
                emit_refusal(
                    ctx,
                    MessageType::ProfileList,
                    context,
                    "db",
                    error.to_string(),
                    json!({}),
                );
                return Ok(());
            }
        }
    }
    ctx.emitter.emit(
        MessageType::ProfileList,
        &json!({ "target_id": data.target_id, "profiles": profiles }),
    );
    Ok(())
}

pub async fn handle_profile_plan(
    data: TargetIdData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(target) = target_or_refusal(
        ctx,
        MessageType::ProfilePlan,
        &data.target_id,
        json!({ "target_id": data.target_id }),
    )
    .await?
    else {
        return Ok(());
    };
    let context = json!({ "target_id": data.target_id });
    let Some(profile) =
        active_profile_or_refusal(ctx, MessageType::ProfilePlan, context.clone(), &target.id).await
    else {
        return Ok(());
    };
    match deploy::apply::plan_for(&*ctx.app.driver, &target, &profile.id).await {
        Ok((plan, _, _)) => ctx.emitter.emit(
            MessageType::ProfilePlan,
            &json!({
                "target_id": data.target_id,
                "profile_id": profile.id,
                "entries": plan.entries,
                "counts": plan_counts(&plan),
            }),
        ),
        Err(error) => {
            let (code, detail) = apply_error_parts(&error);
            let mut context = context;
            context["profile_id"] = json!(profile.id);
            emit_refusal(
                ctx,
                MessageType::ProfilePlan,
                context,
                code,
                error.to_string(),
                detail,
            );
        }
    }
    Ok(())
}

pub async fn handle_profile_apply(
    services: &ServerServices,
    library: &LibraryPaths,
    data: ProfileApplyData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(target) = target_or_refusal(
        ctx,
        MessageType::ProfileApply,
        &data.target_id,
        json!({ "target_id": data.target_id }),
    )
    .await?
    else {
        return Ok(());
    };
    let replace: Vec<std::path::PathBuf> = data
        .replace_occupants
        .iter()
        .map(std::path::PathBuf::from)
        .collect();
    let reply = run_apply(
        services,
        library,
        &*ctx.app.driver,
        ctx.emitter,
        &target,
        &replace,
        true,
    )
    .await;
    ctx.emitter.emit(MessageType::ProfileApply, &reply);
    Ok(())
}

/// A selection mutation: it commits even while the target is running, because
/// desired state may differ from applied state. What a pure preflight can
/// detect against the proposed selection -- two versions routing one
/// destination -- is refused with the previous entry restored, so nothing
/// remains written.
pub async fn handle_profile_set_mod(
    services: &ServerServices,
    library: &LibraryPaths,
    data: ProfileSetModData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let Some(target) = target_or_refusal(
        ctx,
        MessageType::ProfileSetMod,
        &data.target_id,
        json!({ "target_id": data.target_id }),
    )
    .await?
    else {
        return Ok(());
    };
    let mut context = json!({
        "target_id": data.target_id,
        "profile_id": data.profile_id,
        "mod_id": data.mod_id,
        "enabled": data.enabled,
    });
    let Some(profile) = profile_or_refusal(
        ctx,
        MessageType::ProfileSetMod,
        context.clone(),
        &target.id,
        data.profile_id.as_deref(),
    )
    .await
    else {
        return Ok(());
    };
    context["profile_id"] = json!(profile.id);
    let db = &*ctx.app.driver;
    let existing = match ps_db::mod_profiles::mods_of(db, &profile.id).await {
        Ok(entries) => entries,
        Err(error) => {
            emit_refusal(
                ctx,
                MessageType::ProfileSetMod,
                context,
                "db",
                error.to_string(),
                json!({}),
            );
            return Ok(());
        }
    };
    let previous = existing
        .iter()
        .find(|entry| entry.mod_id == data.mod_id)
        .cloned();

    if let Some(Some(version_id)) = &data.mod_version_id {
        match ps_db::mod_library::get_version(db, version_id).await {
            Ok(Some(version)) if version.mod_id == data.mod_id => {}
            Ok(_) => {
                emit_refusal(
                    ctx,
                    MessageType::ProfileSetMod,
                    context,
                    "version_not_of_mod",
                    format!("{version_id} is not a version of {}", data.mod_id),
                    json!({ "mod_version_id": version_id }),
                );
                return Ok(());
            }
            Err(error) => {
                emit_refusal(
                    ctx,
                    MessageType::ProfileSetMod,
                    context,
                    "db",
                    error.to_string(),
                    json!({}),
                );
                return Ok(());
            }
        }
    }

    if data.enabled {
        let pinned = match &data.mod_version_id {
            Some(pin) => pin.clone(),
            None => previous.as_ref().and_then(|p| p.mod_version_id.clone()),
        };
        let refusal = match enable_refusal(db, &target, &data.mod_id, pinned.as_deref()).await {
            Ok(refusal) => refusal,
            Err(error) => Some(("db", error.to_string(), json!({}))),
        };
        if let Some((code, message, detail)) = refusal {
            emit_refusal(
                ctx,
                MessageType::ProfileSetMod,
                context,
                code,
                message,
                detail,
            );
            return Ok(());
        }
    }

    let load_order = previous.as_ref().map(|p| p.load_order).unwrap_or_else(|| {
        existing
            .iter()
            .map(|entry| entry.load_order)
            .max()
            .map(|max| max + 1)
            .unwrap_or(0)
    });
    let entry = ps_db::mod_profiles::ProfileModRow {
        profile_id: profile.id.clone(),
        mod_id: data.mod_id.clone(),
        mod_version_id: match &data.mod_version_id {
            Some(pin) => pin.clone(),
            None => previous.as_ref().and_then(|p| p.mod_version_id.clone()),
        },
        enabled: data.enabled,
        load_order,
    };
    if let Err(error) = ps_db::mod_profiles::set_mod(db, &entry).await {
        emit_refusal(
            ctx,
            MessageType::ProfileSetMod,
            context,
            "db",
            error.to_string(),
            json!({}),
        );
        return Ok(());
    }

    if let Err(deploy::apply::ApplyError::Preflight(
        ps_core::mods::PreflightError::DestinationConflict {
            path,
            mod_version_ids,
        },
    )) = deploy::apply::plan_for(db, &target, &profile.id).await
    {
        let restored = match &previous {
            Some(previous) => ps_db::mod_profiles::set_mod(db, previous).await,
            None => ps_db::mod_profiles::unset_mod(db, &profile.id, &data.mod_id)
                .await
                .map(|_| ()),
        };
        let mut detail = json!({ "path": path, "mod_version_ids": mod_version_ids });
        if let Err(error) = restored {
            detail["restore_error"] = json!(error.to_string());
        }
        emit_refusal(
            ctx,
            MessageType::ProfileSetMod,
            context,
            "destination_conflict",
            format!("two mod versions resolve to {path}"),
            detail,
        );
        return Ok(());
    }

    let selection = if profile.is_active {
        apply_after_selection(services, library, db, ctx.emitter, &target).await
    } else {
        SelectionApply::none()
    };
    ctx.emitter.emit(
        MessageType::ProfileSetMod,
        &json!({
            "target_id": data.target_id,
            "profile_id": profile.id,
            "mod_id": entry.mod_id,
            "enabled": entry.enabled,
            "mod_version_id": entry.mod_version_id,
            "request_id": selection.request_id,
            "pending": selection.pending,
            "apply": selection.apply,
        }),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failure_after_replacing_names_the_set_and_what_moved() {
        let error = deploy::apply::ApplyError::ReplacePartial {
            moved: vec!["C:/game/Mods/CoolMod/main.lua".to_string()],
            backup_dir: "C:/app/mods/_backups/t/20260101-000000-abc".to_string(),
            source: Box::new(deploy::apply::ApplyError::Io(std::io::Error::other("disk"))),
        };
        let mut reply = serde_json::Map::new();
        reply.insert("mid_apply".to_string(), json!(false));
        reply.insert("backup_dir".to_string(), Value::Null);
        insert_apply_error(&mut reply, &error);

        assert_eq!(
            reply["backup_dir"],
            native_path("C:/app/mods/_backups/t/20260101-000000-abc")
        );
        assert_eq!(reply["error"]["code"], "replace_partial");
        assert_eq!(reply["error"]["cause"], "io");
        assert_eq!(
            reply["error"]["moved"],
            json!([native_path("C:/game/Mods/CoolMod/main.lua")])
        );
        #[cfg(windows)]
        assert_eq!(
            reply["backup_dir"],
            r"C:\app\mods\_backups\t\20260101-000000-abc"
        );
    }

    #[test]
    fn a_listed_target_carries_its_paths_in_the_hosts_separators() {
        let target = ModTarget {
            id: "client-palworld".to_string(),
            kind: "client".to_string(),
            server_id: None,
            name: "Steam".to_string(),
            root_path: r"D:\Games\Steam\steamapps/common/Palworld".to_string(),
            platform: "win64".to_string(),
            ue4ss_mode: "standard".to_string(),
            layout_overrides: "{}".to_string(),
            detected: "{}".to_string(),
            last_scanned_at: None,
            created_at: String::new(),
            updated_at: String::new(),
        };

        let json = target_json(&target);

        let layout = json["layout"].as_object().expect("the layout resolves");
        if cfg!(windows) {
            assert_eq!(
                json["root_path"],
                r"D:\Games\Steam\steamapps\common\Palworld"
            );
            for (key, value) in layout {
                let path = value.as_str().unwrap_or_default();
                assert!(!path.contains('/'), "{key}: {path}");
            }
            assert_eq!(
                layout["paks_mods_dir"],
                r"D:\Games\Steam\steamapps\common\Palworld\Pal\Content\Paks\~mods"
            );
        } else {
            assert_eq!(json["root_path"], target.root_path);
        }
    }

    #[test]
    fn apply_errors_map_to_their_own_codes() {
        let io = deploy::apply::ApplyError::Io(std::io::Error::other("disk"));
        let db = deploy::apply::ApplyError::Db(ps_db::DbError::Other("db".to_string()));
        assert_eq!(apply_error_parts(&io).0, "io");
        assert_eq!(apply_error_parts(&db).0, "db");
    }

    #[tokio::test]
    async fn a_holder_added_between_the_scan_and_the_lock_refuses_the_removal() {
        use crate::servers_handlers::test_env::TestEnv;
        use ps_core::mods::{InstallManifest, ModType, SourceHint};
        let mut env = TestEnv::new().await;
        let db = env.app.driver.clone();
        let library = LibraryPaths::new(env._scratch.path());
        for id in ["client-a", "client-b"] {
            ps_db::mod_targets::upsert(
                &*db,
                &ps_db::mod_targets::NewModTarget {
                    id: id.to_string(),
                    kind: "client".to_string(),
                    name: id.to_string(),
                    root_path: env._scratch.path().join(id).to_string_lossy().into_owned(),
                    platform: "win64".to_string(),
                    ue4ss_mode: "none".to_string(),
                    layout_overrides: "{}".to_string(),
                    detected: "{}".to_string(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
            ps_db::mod_profiles::create(
                &*db,
                &ps_db::mod_profiles::NewProfile {
                    id: format!("{id}/default"),
                    target_id: id.to_string(),
                    name: "Default".to_string(),
                    is_default: true,
                },
            )
            .await
            .unwrap();
        }
        let manifest = InstallManifest {
            folder_name: "SteamPack".to_string(),
            display_name: "SteamPack".to_string(),
            mod_type: ModType::Workshop,
            version: "steam-subscribed".to_string(),
            routes: Vec::new(),
            decisions: Vec::new(),
            platform_filtered: None,
            source: SourceHint::default(),
        };
        let mod_id = "workshop-3300000001";
        library::store(
            &*db,
            &library,
            &library::StoreRequest {
                mod_id,
                manifest: &manifest,
                extracted_root: std::path::Path::new(""),
                archive: None,
                source_kind: "workshop",
                source_ref: r#"{"kind":"workshop","package":"SteamPack","workshop_id":"3300000001"}"#,
                custom_name: None,
            },
        )
        .await
        .unwrap();
        let hold = |target_id: &'static str| ps_db::mod_profiles::ProfileModRow {
            profile_id: format!("{target_id}/default"),
            mod_id: mod_id.to_string(),
            mod_version_id: None,
            enabled: true,
            load_order: 0,
        };
        ps_db::mod_profiles::set_mod(&*db, &hold("client-a"))
            .await
            .unwrap();

        let scanned = holders_of(&*db, mod_id).await.unwrap();
        ps_db::mod_profiles::set_mod(&*db, &hold("client-b"))
            .await
            .unwrap();
        let mut ctx = env.ctx();
        remove_held(&library, mod_id, true, scanned, &mut ctx).await;
        let messages = env.drain();

        assert_eq!(messages.len(), 1, "{messages:?}");
        assert_eq!(messages[0]["type"], "mod_remove");
        let error = &messages[0]["data"]["error"];
        assert_eq!(error["code"], "version_in_use", "{error}");
        assert!(
            error["targets"]
                .as_array()
                .unwrap()
                .iter()
                .any(|target| target == "client-b"),
            "{error}"
        );
        assert!(ps_db::mod_library::get_mod(&*db, mod_id)
            .await
            .unwrap()
            .is_some());
        assert!(library::released_packages(&*db, "client-a")
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn a_framework_slot_is_not_counted_as_deployed() {
        use ps_core::mods::{InstallManifest, ModType, SourceHint};
        let scratch = tempfile::tempdir().unwrap();
        let db = ps_db::SqlxSqliteDriver::new(
            ps_db::open(&scratch.path().join("test.db")).await.unwrap(),
        );
        let library = LibraryPaths::new(scratch.path());
        ps_db::mod_targets::upsert(
            &db,
            &ps_db::mod_targets::NewModTarget {
                id: "client-a".to_string(),
                kind: "client".to_string(),
                name: "Palworld".to_string(),
                root_path: scratch
                    .path()
                    .join("client-a")
                    .to_string_lossy()
                    .into_owned(),
                platform: "win64".to_string(),
                ue4ss_mode: "none".to_string(),
                layout_overrides: "{}".to_string(),
                detected: "{}".to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let manifest = InstallManifest {
            folder_name: "UE4SS".to_string(),
            display_name: "UE4SS".to_string(),
            mod_type: ModType::Framework,
            version: "1.0".to_string(),
            routes: Vec::new(),
            decisions: Vec::new(),
            platform_filtered: None,
            source: SourceHint::default(),
        };
        let mod_id = "ue4ss-framework";
        library::store(
            &db,
            &library,
            &library::StoreRequest {
                mod_id,
                manifest: &manifest,
                extracted_root: std::path::Path::new(""),
                archive: None,
                source_kind: "local",
                source_ref: "{}",
                custom_name: None,
            },
        )
        .await
        .unwrap();
        let version_id = ps_db::mod_library::current_version(&db, mod_id)
            .await
            .unwrap()
            .unwrap()
            .id;
        ps_db::mod_profiles::set_framework(&db, "client-a", "ue4ss", &version_id)
            .await
            .unwrap();

        let holders = holders_of(&db, mod_id).await.unwrap();
        assert!(
            holders.in_use(false),
            "mod_usage still counts the framework slot"
        );
        let usage = holders.usage();
        assert_eq!(usage["deployed"], json!([]), "{usage}");
        assert_eq!(
            usage["frameworks"],
            json!([{ "target_id": "client-a", "target_name": "Palworld" }]),
            "{usage}"
        );
    }
}
