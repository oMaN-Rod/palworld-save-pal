//! Wire handlers that report, install and remove a target's frameworks.
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;
use crate::mods_handlers::{
    apply_after_selection, emit_progress, emit_refusal, run_apply, selection_of, target_or_refusal,
};
use crate::services::mods::frameworks::install::{self, display_of};
use crate::services::mods::frameworks::source::Channel;
use crate::services::mods::frameworks::status;
use crate::services::mods::{deploy, detect, layout, running, LibraryPaths};
use crate::services::ServerServices;

use ps_core::mods::{
    native_separators, normalize_physical_path, FrameworkKey, PreflightError, TargetKind, Ue4ssMode,
};

#[derive(Debug, serde::Deserialize)]
pub struct FrameworkStatusData {
    pub target_id: String,
    #[serde(default)]
    pub check_latest: bool,
    #[serde(default)]
    pub channel: Option<Channel>,
    #[serde(default)]
    pub dev: bool,
}

/// Every stored version of `key`'s library mod, oldest-installed order (see
/// `ps_db::mod_library::versions_of`), each with the display string its
/// `source_ref` carries or, absent that, its raw version.
pub async fn library_versions(
    db: &dyn ps_db::DbDriver,
    key: FrameworkKey,
) -> Result<Vec<Value>, ps_db::DbError> {
    let rows = ps_db::mod_library::versions_of(db, &key.mod_id()).await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let display = display_of(&row.source_ref, &row.version);
            json!({
                "mod_version_id": row.id,
                "version": row.version,
                "display": display,
                "installed_at": row.installed_at,
                "is_current": row.is_current,
            })
        })
        .collect())
}

fn status_error_refusal(error: &status::StatusError) -> (&'static str, String) {
    match error {
        status::StatusError::Docker => ("not_supported_on_target", error.to_string()),
        status::StatusError::Db(_) => ("db", error.to_string()),
        status::StatusError::Layout(_) => ("layout_error", error.to_string()),
    }
}

pub async fn handle_framework_status(
    services: &ServerServices,
    data: FrameworkStatusData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let context = json!({ "target_id": data.target_id });
    let Some(target) = target_or_refusal(
        ctx,
        MessageType::FrameworkStatus,
        &data.target_id,
        context.clone(),
    )
    .await?
    else {
        return Ok(());
    };

    let db = &*ctx.app.driver;
    let detection = match status::detect(db, &target).await {
        Ok(detection) => detection,
        Err(error) => {
            let (code, message) = status_error_refusal(&error);
            emit_refusal(
                ctx,
                MessageType::FrameworkStatus,
                context,
                code,
                message,
                json!({}),
            );
            return Ok(());
        }
    };

    let channel = data.channel.unwrap_or(Channel::Latest);
    let mut frameworks = Vec::with_capacity(detection.installed.len());
    for (key, installed) in &detection.installed {
        let library = match library_versions(db, *key).await {
            Ok(library) => library,
            Err(error) => {
                emit_refusal(
                    ctx,
                    MessageType::FrameworkStatus,
                    context,
                    "db",
                    error.to_string(),
                    json!({}),
                );
                return Ok(());
            }
        };

        let mut latest = None;
        let mut latest_error = None;
        let mut latest_version = None;
        if data.check_latest {
            match services.frameworks.latest(*key, channel, data.dev).await {
                Ok(release) => {
                    let in_library = library.iter().any(|entry| {
                        entry.get("version").and_then(Value::as_str)
                            == Some(release.version.as_str())
                    });
                    latest_version = Some(release.version.clone());
                    latest = Some(json!({
                        "version": release.version,
                        "display": release.display,
                        "in_library": in_library,
                    }));
                }
                Err(error) => {
                    latest_error =
                        Some(json!({ "code": error.code(), "message": error.to_string() }));
                }
            }
        }

        let update_available = installed.present
            && latest_version
                .as_deref()
                .is_some_and(|latest| installed.version.as_deref() != Some(latest));

        frameworks.push(json!({
            "key": key.as_str(),
            "name": key.display_name(),
            "installed": installed,
            "library": library,
            "latest": latest,
            "latest_error": latest_error,
            "update_available": update_available,
        }));
    }

    ctx.emitter.emit(
        MessageType::FrameworkStatus,
        &json!({
            "target_id": data.target_id,
            "ue4ss_mode": detection.ue4ss_mode,
            "hazards": detection.hazards,
            "frameworks": frameworks,
        }),
    );
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct FrameworkInstallData {
    pub target_id: String,
    pub key: String,
    #[serde(default)]
    pub mod_version_id: Option<String>,
    #[serde(default)]
    pub channel: Option<Channel>,
    #[serde(default)]
    pub dev: bool,
    #[serde(default)]
    pub replace: bool,
}

/// Puts back a framework slot's previous version (or clears it, when it had
/// none) and the target's previous UE4SS mode. Used to undo the commit half of
/// an install when the preflight or the version write that follows it fails.
async fn restore_framework_slot(
    db: &dyn ps_db::DbDriver,
    target_id: &str,
    key: FrameworkKey,
    previous_slot: Option<&ps_db::mod_profiles::TargetFramework>,
    previous_mode: &str,
) -> Result<(), ps_db::DbError> {
    let slot_result = match previous_slot {
        Some(slot) => {
            ps_db::mod_profiles::set_framework(db, target_id, key.as_str(), &slot.mod_version_id)
                .await
        }
        None => ps_db::mod_profiles::remove_framework(db, target_id, key.as_str())
            .await
            .map(|_| ()),
    };
    let mode_result = ps_db::mod_targets::set_ue4ss_mode(db, target_id, previous_mode).await;
    slot_result.and(mode_result)
}

pub async fn handle_framework_install(
    services: &ServerServices,
    library: &LibraryPaths,
    data: FrameworkInstallData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let context = json!({ "target_id": data.target_id, "key": data.key });
    let Some(target) = target_or_refusal(
        ctx,
        MessageType::FrameworkInstall,
        &data.target_id,
        context.clone(),
    )
    .await?
    else {
        return Ok(());
    };
    let Some(key) = FrameworkKey::parse(&data.key) else {
        emit_refusal(
            ctx,
            MessageType::FrameworkInstall,
            context,
            "invalid_framework",
            format!("{} is not a known framework", data.key),
            json!({}),
        );
        return Ok(());
    };

    let spec = match layout::spec_for(&target) {
        Ok(spec) => spec,
        Err(error) => {
            emit_refusal(
                ctx,
                MessageType::FrameworkInstall,
                context,
                "layout_error",
                error.to_string(),
                json!({}),
            );
            return Ok(());
        }
    };
    if spec.kind == TargetKind::DockerServer {
        emit_refusal(
            ctx,
            MessageType::FrameworkInstall,
            context,
            "not_supported_on_target",
            "framework installation does not apply to a Docker server".to_string(),
            json!({}),
        );
        return Ok(());
    }

    let db = &*ctx.app.driver;
    let (mode, _dual) = detect::detect_ue4ss(Path::new(&target.root_path), spec.platform);
    let ue4ss_slot = match ps_db::mod_profiles::framework_of(db, &target.id, "ue4ss").await {
        Ok(slot) => slot,
        Err(error) => {
            emit_refusal(
                ctx,
                MessageType::FrameworkInstall,
                context,
                "db",
                error.to_string(),
                json!({}),
            );
            return Ok(());
        }
    };

    if key == FrameworkKey::Ue4ss && mode == Ue4ssMode::Workshop {
        emit_refusal(
            ctx,
            MessageType::FrameworkInstall,
            context,
            "framework_conflict",
            format!("{} is running UE4SS in workshop mode", target.id),
            json!({ "detail": "workshop" }),
        );
        return Ok(());
    }
    if matches!(key, FrameworkKey::PalSchema | FrameworkKey::Amity)
        && ue4ss_slot.is_none()
        && mode != Ue4ssMode::Standard
    {
        emit_refusal(
            ctx,
            MessageType::FrameworkInstall,
            context,
            "framework_required",
            format!("{} needs UE4SS installed first", key.display_name()),
            json!({ "framework": "ue4ss" }),
        );
        return Ok(());
    }
    if key == FrameworkKey::Ue4ss && spec.kind == TargetKind::NativeServer {
        match status::probe_layout(&target, Ue4ssMode::Standard) {
            Ok(probed) => {
                if let (Some(ue4ss_dir), Some(mods_dir)) = (
                    probed.ue4ss_dir.as_deref(),
                    probed.ue4ss_mods_dir.as_deref(),
                ) {
                    let expected = ue4ss_dir.join("Mods");
                    let case_insensitive = cfg!(any(windows, target_os = "macos"));
                    let expected_key =
                        normalize_physical_path(&expected.to_string_lossy(), case_insensitive);
                    let actual_key =
                        normalize_physical_path(&mods_dir.to_string_lossy(), case_insensitive);
                    if expected_key != actual_key {
                        emit_refusal(
                            ctx,
                            MessageType::FrameworkInstall,
                            context,
                            "mods_dir_mismatch",
                            format!(
                                "{}'s UE4SS mods directory does not match the expected path",
                                target.id
                            ),
                            json!({
                                "expected": native_separators(&expected.to_string_lossy(), cfg!(windows)),
                                "actual": native_separators(&mods_dir.to_string_lossy(), cfg!(windows)),
                            }),
                        );
                        return Ok(());
                    }
                }
            }
            Err(error) => {
                emit_refusal(
                    ctx,
                    MessageType::FrameworkInstall,
                    context,
                    "layout_error",
                    error.to_string(),
                    json!({}),
                );
                return Ok(());
            }
        }
    }

    let stored = if let Some(mod_version_id) = &data.mod_version_id {
        match ps_db::mod_library::get_version(db, mod_version_id).await {
            Ok(Some(row)) if row.mod_id == key.mod_id() => install::StoredFramework {
                installed_new: false,
                display: display_of(&row.source_ref, &row.version),
                version: row.version,
                mod_version_id: row.id,
            },
            Ok(_) => {
                emit_refusal(
                    ctx,
                    MessageType::FrameworkInstall,
                    context,
                    "version_not_of_mod",
                    format!("{mod_version_id} is not a version of {}", key.mod_id()),
                    json!({ "mod_version_id": mod_version_id }),
                );
                return Ok(());
            }
            Err(error) => {
                emit_refusal(
                    ctx,
                    MessageType::FrameworkInstall,
                    context,
                    "db",
                    error.to_string(),
                    json!({}),
                );
                return Ok(());
            }
        }
    } else {
        let request_id = deploy::backup::new_op_id();
        emit_progress(
            ctx.emitter,
            &request_id,
            &target.id,
            "resolving",
            0,
            "resolving the latest release",
        );
        let channel = data.channel.unwrap_or(Channel::Latest);
        let release = match services.frameworks.latest(key, channel, data.dev).await {
            Ok(release) => release,
            Err(error) => {
                emit_refusal(
                    ctx,
                    MessageType::FrameworkInstall,
                    context,
                    error.code(),
                    error.to_string(),
                    json!({}),
                );
                return Ok(());
            }
        };
        emit_progress(
            ctx.emitter,
            &request_id,
            &target.id,
            "downloading",
            0,
            "downloading the release",
        );
        let emitter = ctx.emitter;
        let progress_request_id = request_id.clone();
        let progress_target_id = target.id.clone();
        let last_pct = std::sync::Mutex::new(None::<u8>);
        let progress = move |got: u64, total: Option<u64>| {
            let pct = total
                .map(|total| (((got * 100) / total.max(1)).min(99)) as u8)
                .unwrap_or(0);
            let mut last_pct = last_pct.lock().expect("progress mutex poisoned");
            if *last_pct == Some(pct) {
                return;
            }
            *last_pct = Some(pct);
            emit_progress(
                emitter,
                &progress_request_id,
                &progress_target_id,
                "downloading",
                pct,
                "downloading the release",
            );
        };
        let stored =
            match install::store_release(db, library, &*services.frameworks, &release, &progress)
                .await
            {
                Ok(stored) => stored,
                Err(error) => {
                    emit_refusal(
                        ctx,
                        MessageType::FrameworkInstall,
                        context,
                        error.code(),
                        error.to_string(),
                        json!({}),
                    );
                    return Ok(());
                }
            };
        emit_progress(
            ctx.emitter,
            &request_id,
            &target.id,
            "storing",
            100,
            "storing the release",
        );
        stored
    };

    let target_id = target.id.clone();
    let previous_slot = match ps_db::mod_profiles::framework_of(db, &target_id, key.as_str()).await
    {
        Ok(slot) => slot,
        Err(error) => {
            emit_refusal(
                ctx,
                MessageType::FrameworkInstall,
                context,
                "db",
                error.to_string(),
                json!({}),
            );
            return Ok(());
        }
    };
    let previous_mode = target.ue4ss_mode.clone();

    if target.ue4ss_mode == "none" && (key == FrameworkKey::Ue4ss || mode == Ue4ssMode::Standard) {
        if let Err(error) = ps_db::mod_targets::set_ue4ss_mode(db, &target_id, "standard").await {
            emit_refusal(
                ctx,
                MessageType::FrameworkInstall,
                context,
                "db",
                error.to_string(),
                json!({}),
            );
            return Ok(());
        }
    }
    if let Err(error) =
        ps_db::mod_profiles::set_framework(db, &target_id, key.as_str(), &stored.mod_version_id)
            .await
    {
        let _ = restore_framework_slot(db, &target_id, key, previous_slot.as_ref(), &previous_mode)
            .await;
        emit_refusal(
            ctx,
            MessageType::FrameworkInstall,
            context,
            "db",
            error.to_string(),
            json!({}),
        );
        return Ok(());
    }
    let target = match ps_db::mod_targets::get(db, &target_id).await {
        Ok(Some(target)) => target,
        Ok(None) => {
            let _ =
                restore_framework_slot(db, &target_id, key, previous_slot.as_ref(), &previous_mode)
                    .await;
            emit_refusal(
                ctx,
                MessageType::FrameworkInstall,
                context,
                "db",
                format!("target {target_id} vanished during install"),
                json!({}),
            );
            return Ok(());
        }
        Err(error) => {
            let _ =
                restore_framework_slot(db, &target_id, key, previous_slot.as_ref(), &previous_mode)
                    .await;
            emit_refusal(
                ctx,
                MessageType::FrameworkInstall,
                context,
                "db",
                error.to_string(),
                json!({}),
            );
            return Ok(());
        }
    };

    let profile = match ps_db::mod_profiles::active_for_target(db, &target_id).await {
        Ok(Some(profile)) => profile,
        Ok(None) => {
            let _ =
                restore_framework_slot(db, &target_id, key, previous_slot.as_ref(), &previous_mode)
                    .await;
            emit_refusal(
                ctx,
                MessageType::FrameworkInstall,
                context,
                "no_active_profile",
                format!("target {target_id} has no active profile"),
                json!({}),
            );
            return Ok(());
        }
        Err(error) => {
            let _ =
                restore_framework_slot(db, &target_id, key, previous_slot.as_ref(), &previous_mode)
                    .await;
            emit_refusal(
                ctx,
                MessageType::FrameworkInstall,
                context,
                "db",
                error.to_string(),
                json!({}),
            );
            return Ok(());
        }
    };

    let mut occupants: Vec<String> = Vec::new();
    match deploy::apply::plan_for(db, &target, &profile.id).await {
        Ok(_) => {}
        Err(deploy::apply::ApplyError::Preflight(PreflightError::DestinationConflict {
            path,
            mod_version_ids,
        })) => {
            let _ =
                restore_framework_slot(db, &target_id, key, previous_slot.as_ref(), &previous_mode)
                    .await;
            emit_refusal(
                ctx,
                MessageType::FrameworkInstall,
                context,
                "destination_conflict",
                format!("two mod versions resolve to {path}"),
                json!({
                    "path": native_separators(&path, cfg!(windows)),
                    "mod_version_ids": mod_version_ids,
                }),
            );
            return Ok(());
        }
        Err(deploy::apply::ApplyError::Preflight(PreflightError::UnmanagedOccupant { paths })) => {
            occupants = paths;
        }
        Err(_) => {}
    }

    let selection = if data.replace {
        let replace_paths: Vec<PathBuf> = occupants.iter().map(PathBuf::from).collect();
        selection_of(
            run_apply(
                services,
                library,
                db,
                ctx.emitter,
                &target,
                &replace_paths,
                true,
            )
            .await,
        )
    } else {
        apply_after_selection(services, library, db, ctx.emitter, &target).await
    };

    ctx.emitter.emit(
        MessageType::FrameworkInstall,
        &json!({
            "target_id": target_id,
            "key": key.as_str(),
            "mod_version_id": stored.mod_version_id,
            "version": stored.version,
            "display": stored.display,
            "installed_new": stored.installed_new,
            "request_id": selection.request_id,
            "pending": selection.pending,
            "apply": selection.apply,
        }),
    );
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct FrameworkRemoveData {
    pub target_id: String,
    pub key: String,
    #[serde(default)]
    pub force: bool,
}

/// Enabled active-profile mods whose library `mod_type` is one of `mod_types`.
/// BPModLoaderMod loads `logicmods` mods, so removing UE4SS breaks them too.
async fn enabled_profile_mods_of_types(
    db: &dyn ps_db::DbDriver,
    target_id: &str,
    mod_types: &[&str],
) -> Result<Vec<String>, ps_db::DbError> {
    let mut found = Vec::new();
    if mod_types.is_empty() {
        return Ok(found);
    }
    let Some(profile) = ps_db::mod_profiles::active_for_target(db, target_id).await? else {
        return Ok(found);
    };
    for entry in ps_db::mod_profiles::mods_of(db, &profile.id).await? {
        if !entry.enabled {
            continue;
        }
        if let Some(row) = ps_db::mod_library::get_mod(db, &entry.mod_id).await? {
            if mod_types.contains(&row.mod_type.as_str()) {
                found.push(entry.mod_id.clone());
            }
        }
    }
    Ok(found)
}

const UE4SS_DEPENDENT_MOD_TYPES: &[&str] = &["ue4ss", "palschema", "hybrid", "logicmods"];

/// Other installed frameworks and enabled profile mods that need `key` to keep
/// working, so a plain `framework_remove` cannot leave them broken underneath.
async fn framework_dependents(
    db: &dyn ps_db::DbDriver,
    target_id: &str,
    key: FrameworkKey,
) -> Result<Vec<String>, ps_db::DbError> {
    let mut dependents = Vec::new();
    let profile_mod_types: &[&str] = match key {
        FrameworkKey::Ue4ss => {
            for other in [FrameworkKey::PalSchema, FrameworkKey::Amity] {
                if ps_db::mod_profiles::framework_of(db, target_id, other.as_str())
                    .await?
                    .is_some()
                {
                    dependents.push(other.as_str().to_string());
                }
            }
            UE4SS_DEPENDENT_MOD_TYPES
        }
        FrameworkKey::PalSchema => &["palschema"],
        FrameworkKey::Amity => &[],
    };
    dependents.extend(enabled_profile_mods_of_types(db, target_id, profile_mod_types).await?);
    Ok(dependents)
}

pub async fn handle_framework_remove(
    services: &ServerServices,
    library: &LibraryPaths,
    data: FrameworkRemoveData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let context = json!({ "target_id": data.target_id, "key": data.key });
    let Some(target) = target_or_refusal(
        ctx,
        MessageType::FrameworkRemove,
        &data.target_id,
        context.clone(),
    )
    .await?
    else {
        return Ok(());
    };
    let Some(key) = FrameworkKey::parse(&data.key) else {
        emit_refusal(
            ctx,
            MessageType::FrameworkRemove,
            context,
            "invalid_framework",
            format!("{} is not a known framework", data.key),
            json!({}),
        );
        return Ok(());
    };

    let db = &*ctx.app.driver;
    let slot = match ps_db::mod_profiles::framework_of(db, &target.id, key.as_str()).await {
        Ok(slot) => slot,
        Err(error) => {
            emit_refusal(
                ctx,
                MessageType::FrameworkRemove,
                context,
                "db",
                error.to_string(),
                json!({}),
            );
            return Ok(());
        }
    };
    if slot.is_none() {
        emit_refusal(
            ctx,
            MessageType::FrameworkRemove,
            context,
            "framework_not_installed",
            format!("{} has no {} installed", target.id, key.display_name()),
            json!({}),
        );
        return Ok(());
    }

    if !data.force {
        let dependents = match framework_dependents(db, &target.id, key).await {
            Ok(dependents) => dependents,
            Err(error) => {
                emit_refusal(
                    ctx,
                    MessageType::FrameworkRemove,
                    context,
                    "db",
                    error.to_string(),
                    json!({}),
                );
                return Ok(());
            }
        };
        if !dependents.is_empty() {
            emit_refusal(
                ctx,
                MessageType::FrameworkRemove,
                context,
                "framework_required",
                format!(
                    "{} is required by {}",
                    key.display_name(),
                    dependents.join(", ")
                ),
                json!({ "framework": key.as_str(), "dependents": dependents }),
            );
            return Ok(());
        }
    }

    // UE4SS is the runtime that hosts PalSchema and Amity: forcing it out with
    // them still installed would leave a target whose every future apply fails.
    let mut also_removed: Vec<String> = Vec::new();
    if key == FrameworkKey::Ue4ss && data.force {
        for other in [FrameworkKey::PalSchema, FrameworkKey::Amity] {
            let other_slot =
                match ps_db::mod_profiles::framework_of(db, &target.id, other.as_str()).await {
                    Ok(slot) => slot,
                    Err(error) => {
                        emit_refusal(
                            ctx,
                            MessageType::FrameworkRemove,
                            context,
                            "db",
                            error.to_string(),
                            json!({}),
                        );
                        return Ok(());
                    }
                };
            if other_slot.is_none() {
                continue;
            }
            if let Err(error) =
                ps_db::mod_profiles::remove_framework(db, &target.id, other.as_str()).await
            {
                emit_refusal(
                    ctx,
                    MessageType::FrameworkRemove,
                    context,
                    "db",
                    error.to_string(),
                    json!({}),
                );
                return Ok(());
            }
            also_removed.push(other.as_str().to_string());
        }
    }

    if let Err(error) = ps_db::mod_profiles::remove_framework(db, &target.id, key.as_str()).await {
        emit_refusal(
            ctx,
            MessageType::FrameworkRemove,
            context,
            "db",
            error.to_string(),
            json!({}),
        );
        return Ok(());
    }

    let selection = apply_after_selection(services, library, db, ctx.emitter, &target).await;

    if key == FrameworkKey::Ue4ss && !selection.pending && selection.apply.get("error").is_none() {
        let clear =
            match enabled_profile_mods_of_types(db, &target.id, UE4SS_DEPENDENT_MOD_TYPES).await {
                Ok(found) => found.is_empty(),
                Err(error) => {
                    tracing::warn!(
                        %error,
                        target_id = %target.id,
                        "checking active profile mods before a framework-remove refresh failed"
                    );
                    false
                }
            };
        if clear {
            if let Err(error) = detect::refresh(db, &target).await {
                tracing::warn!(
                    %error,
                    target_id = %target.id,
                    "refreshing the target after a framework remove failed"
                );
            }
        }
    }

    ctx.emitter.emit(
        MessageType::FrameworkRemove,
        &json!({
            "target_id": target.id,
            "key": key.as_str(),
            "removed": true,
            "also_removed": also_removed,
            "request_id": selection.request_id,
            "pending": selection.pending,
            "apply": selection.apply,
        }),
    );
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct FrameworkHazardRemoveData {
    pub target_id: String,
    pub hazard: String,
}

/// The regular files a hazard's removal must move: the proxy hazard's paths
/// directly, or every regular file under the legacy folder's one path.
fn hazard_files(hazard_code: &str, hazard: &status::Hazard) -> Vec<PathBuf> {
    if hazard_code == status::HAZARD_WORKSHOP_PROXY_DLL {
        return hazard.paths.iter().map(PathBuf::from).collect();
    }
    let Some(root) = hazard.paths.first().map(PathBuf::from) else {
        return Vec::new();
    };
    walkdir::WalkDir::new(&root)
        .follow_links(false)
        .follow_root_links(false)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .collect()
}

/// Removes every directory under `root` left empty by the moves, deepest
/// first, then `root` itself if it too ended up empty. `remove_dir` only ever
/// succeeds on an empty directory, so a symlink or a directory still holding
/// something is untouched.
fn remove_empty_dirs(root: &Path) {
    let mut dirs: Vec<PathBuf> = walkdir::WalkDir::new(root)
        .follow_links(false)
        .follow_root_links(false)
        .min_depth(1)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_dir())
        .map(|entry| entry.into_path())
        .collect();
    dirs.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for dir in dirs {
        let _ = std::fs::remove_dir(&dir);
    }
    if std::fs::symlink_metadata(root).is_ok_and(|meta| meta.is_dir()) {
        let _ = std::fs::remove_dir(root);
    }
}

pub async fn handle_framework_hazard_remove(
    services: &ServerServices,
    library: &LibraryPaths,
    data: FrameworkHazardRemoveData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let context = json!({ "target_id": data.target_id, "hazard": data.hazard });
    let Some(target) = target_or_refusal(
        ctx,
        MessageType::FrameworkHazardRemove,
        &data.target_id,
        context.clone(),
    )
    .await?
    else {
        return Ok(());
    };
    if data.hazard != status::HAZARD_WORKSHOP_PROXY_DLL
        && data.hazard != status::HAZARD_AMITY_LEGACY_FOLDER
    {
        emit_refusal(
            ctx,
            MessageType::FrameworkHazardRemove,
            context,
            "invalid_hazard",
            format!("{} is not a known hazard", data.hazard),
            json!({}),
        );
        return Ok(());
    }

    let db = &*ctx.app.driver;
    let detection = match status::detect(db, &target).await {
        Ok(detection) => detection,
        Err(error) => {
            let (code, message) = status_error_refusal(&error);
            emit_refusal(
                ctx,
                MessageType::FrameworkHazardRemove,
                context,
                code,
                message,
                json!({}),
            );
            return Ok(());
        }
    };
    let Some(hazard) = detection
        .hazards
        .iter()
        .find(|hazard| hazard.code == data.hazard)
        .cloned()
    else {
        emit_refusal(
            ctx,
            MessageType::FrameworkHazardRemove,
            context,
            "hazard_not_found",
            format!("{} has no {} hazard", target.id, data.hazard),
            json!({}),
        );
        return Ok(());
    };

    let mut files = hazard_files(&data.hazard, &hazard);
    if data.hazard == status::HAZARD_WORKSHOP_PROXY_DLL {
        let deployed = match ps_db::mod_deployments::files_of(db, &target.id).await {
            Ok(rows) => rows,
            Err(error) => {
                emit_refusal(
                    ctx,
                    MessageType::FrameworkHazardRemove,
                    context,
                    "db",
                    error.to_string(),
                    json!({}),
                );
                return Ok(());
            }
        };
        let case_insensitive = cfg!(any(windows, target_os = "macos"));
        let deployed_keys: std::collections::HashSet<String> = deployed
            .iter()
            .map(|row| normalize_physical_path(&row.path, case_insensitive))
            .collect();
        files.retain(|path| {
            !deployed_keys.contains(&normalize_physical_path(
                &path.to_string_lossy(),
                case_insensitive,
            ))
        });
    }
    if files.is_empty() {
        emit_refusal(
            ctx,
            MessageType::FrameworkHazardRemove,
            context,
            "hazard_not_found",
            format!("{} has no {} hazard", target.id, data.hazard),
            json!({}),
        );
        return Ok(());
    }

    let Some(_guard) = deploy::try_lock_target(library, &target) else {
        emit_refusal(
            ctx,
            MessageType::FrameworkHazardRemove,
            context,
            "apply_in_progress",
            format!("an apply is running on {}", target.id),
            json!({ "target_id": target.id }),
        );
        return Ok(());
    };
    match running::target_is_running(services, db, &target).await {
        Ok(false) => {}
        Ok(true) => {
            emit_refusal(
                ctx,
                MessageType::FrameworkHazardRemove,
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
                MessageType::FrameworkHazardRemove,
                context,
                "db",
                error.to_string(),
                json!({}),
            );
            return Ok(());
        }
    }

    let mut set = match deploy::backup::BackupSet::open(
        library,
        &target.id,
        &deploy::backup::utc_stamp(),
        &deploy::backup::new_op_id(),
    ) {
        Ok(set) => set,
        Err(error) => {
            emit_refusal(
                ctx,
                MessageType::FrameworkHazardRemove,
                context,
                "io",
                error.to_string(),
                json!({}),
            );
            return Ok(());
        }
    };

    let backup_dir = native_separators(&set.dir().to_string_lossy(), cfg!(windows));
    let mut moved: Vec<String> = Vec::new();
    for path in &files {
        match set.take(path, deploy::MoveStrategy::Auto) {
            Ok(_) => moved.push(native_separators(&path.to_string_lossy(), cfg!(windows))),
            Err(error) => {
                emit_refusal(
                    ctx,
                    MessageType::FrameworkHazardRemove,
                    context,
                    "io",
                    error.to_string(),
                    json!({ "moved": moved, "backup_dir": backup_dir }),
                );
                return Ok(());
            }
        }
    }

    if data.hazard == status::HAZARD_AMITY_LEGACY_FOLDER {
        if let Some(root) = hazard.paths.first().map(PathBuf::from) {
            remove_empty_dirs(&root);
        }
    }

    ctx.emitter.emit(
        MessageType::FrameworkHazardRemove,
        &json!({
            "target_id": target.id,
            "hazard": data.hazard,
            "moved": moved,
            "backup_dir": backup_dir,
        }),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use crate::servers_handlers::test_env::TestEnv;
    use crate::services::mods::frameworks::source::{
        FrameworkSource, Progress, Release, ReleaseAsset, SourceError,
    };

    #[derive(Default)]
    struct FakeSource {
        calls: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl FrameworkSource for FakeSource {
        async fn latest(
            &self,
            key: FrameworkKey,
            _channel: Channel,
            _dev: bool,
        ) -> Result<Release, SourceError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            match key {
                FrameworkKey::Ue4ss => Ok(Release {
                    key,
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
                }),
                FrameworkKey::PalSchema => Err(SourceError::RateLimited),
                FrameworkKey::Amity => Err(SourceError::Unavailable),
            }
        }

        async fn fetch(
            &self,
            _release: &Release,
            _dest: &Path,
            _progress: Progress<'_>,
        ) -> Result<(), SourceError> {
            unreachable!()
        }
    }

    struct Fixture {
        env: TestEnv,
        services: ServerServices,
        fake: Arc<FakeSource>,
    }

    impl Fixture {
        async fn new() -> Self {
            let env = TestEnv::new().await;
            let fake = Arc::new(FakeSource::default());
            let mut services = ServerServices::with_docker(env.docker.clone(), env._scratch.path());
            services.frameworks = fake.clone();
            Self {
                env,
                services,
                fake,
            }
        }

        async fn client(&mut self) -> String {
            let root = self.env._scratch.path().join("Palworld");
            std::fs::create_dir_all(root.join("Pal/Binaries/Win64/ue4ss")).unwrap();
            std::fs::write(
                root.join("Pal/Binaries/Win64/Palworld-Win64-Shipping.exe"),
                b"stub",
            )
            .unwrap();
            std::fs::write(root.join("Pal/Binaries/Win64/dwmapi.dll"), b"stub").unwrap();
            std::fs::write(root.join("Pal/Binaries/Win64/ue4ss/ue4ss.version"), b"old").unwrap();
            std::fs::create_dir_all(root.join("Pal/Content/Paks")).unwrap();
            let detected = crate::services::mods::detect::describe(&root, "manual").unwrap();
            crate::services::mods::detect::register(&*self.env.app.driver, &detected, "Steam")
                .await
                .unwrap()
                .id
        }

        async fn status(&mut self, data: FrameworkStatusData) -> Value {
            handle_framework_status(&self.services, data, &mut self.env.ctx())
                .await
                .unwrap();
            self.env
                .drain()
                .into_iter()
                .rev()
                .find(|frame| frame["type"] == "framework_status")
                .expect("a framework_status reply")["data"]
                .clone()
        }
    }

    fn data(target_id: &str, check_latest: bool) -> FrameworkStatusData {
        FrameworkStatusData {
            target_id: target_id.to_string(),
            check_latest,
            channel: None,
            dev: false,
        }
    }

    fn framework<'a>(reply: &'a Value, key: &str) -> &'a Value {
        reply["frameworks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["key"] == key)
            .unwrap_or_else(|| panic!("no {key} entry in {reply:?}"))
    }

    #[tokio::test]
    async fn without_check_latest_every_latest_is_null_and_the_source_is_never_called() {
        let mut fixture = Fixture::new().await;
        let target_id = fixture.client().await;

        let reply = fixture.status(data(&target_id, false)).await;

        assert!(reply["error"].is_null(), "{reply:?}");
        for key in ["ue4ss", "palschema", "amity"] {
            let entry = framework(&reply, key);
            assert!(entry["latest"].is_null(), "{key}: {entry:?}");
            assert!(entry["latest_error"].is_null(), "{key}: {entry:?}");
        }
        assert_eq!(fixture.fake.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn check_latest_reports_the_source_release_and_its_errors() {
        let mut fixture = Fixture::new().await;
        let target_id = fixture.client().await;

        let reply = fixture.status(data(&target_id, true)).await;

        assert!(reply["error"].is_null(), "{reply:?}");
        let ue4ss = framework(&reply, "ue4ss");
        assert_eq!(ue4ss["latest"]["version"], "2281fa31");
        assert_eq!(ue4ss["update_available"], true, "{ue4ss:?}");

        let palschema = framework(&reply, "palschema");
        assert_eq!(palschema["latest_error"]["code"], "rate_limited");
        assert!(palschema["latest"].is_null());
    }

    #[tokio::test]
    async fn an_unknown_target_is_refused() {
        let mut fixture = Fixture::new().await;

        let reply = fixture.status(data("nope", false)).await;

        assert_eq!(reply["error"]["code"], "target_not_found", "{reply:?}");
    }

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

    #[derive(Default)]
    struct FakeInstallSource {
        repeat_progress: bool,
    }

    #[async_trait::async_trait]
    impl FrameworkSource for FakeInstallSource {
        async fn latest(
            &self,
            key: FrameworkKey,
            _channel: Channel,
            _dev: bool,
        ) -> Result<Release, SourceError> {
            match key {
                FrameworkKey::Ue4ss => Ok(Release {
                    key,
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
                }),
                FrameworkKey::PalSchema => Ok(Release {
                    key,
                    tag: "0.6.71".to_string(),
                    version: "0.6.71".to_string(),
                    display: "0.6.71".to_string(),
                    asset: ReleaseAsset {
                        name: "PalSchema.zip".to_string(),
                        url: "https://example.invalid/PalSchema.zip".to_string(),
                        size: Some(10),
                    },
                    origin: "github",
                    repo: Some("PalSchema/PalSchema"),
                }),
                FrameworkKey::Amity => Err(SourceError::Unavailable),
            }
        }

        async fn fetch(
            &self,
            release: &Release,
            dest: &Path,
            progress: Progress<'_>,
        ) -> Result<(), SourceError> {
            if self.repeat_progress && release.key == FrameworkKey::Ue4ss {
                progress(1, Some(10));
                progress(1, Some(10));
                progress(5, Some(10));
                progress(5, Some(10));
                progress(9, Some(10));
            }
            let bytes = match release.key {
                FrameworkKey::Ue4ss => zip_bytes(&[
                    ("dwmapi.dll", b"proxy".as_slice()),
                    ("ue4ss/UE4SS.dll", b"runtime".as_slice()),
                    ("ue4ss/Mods/mods.txt", b"ignored".as_slice()),
                    (
                        "ue4ss/Mods/Keybinds/Scripts/main.lua",
                        b"keybinds".as_slice(),
                    ),
                ]),
                FrameworkKey::PalSchema => zip_bytes(&[
                    ("PalSchema/dlls/main.dll", b"main".as_slice()),
                    ("PalSchema/enabled.txt", b"".as_slice()),
                ]),
                FrameworkKey::Amity => unreachable!("Amity has no bundled fixture"),
            };
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(dest, bytes)?;
            Ok(())
        }
    }

    struct InstallFixture {
        env: TestEnv,
        services: ServerServices,
        library: LibraryPaths,
    }

    impl InstallFixture {
        async fn new() -> Self {
            Self::with_source(Arc::new(FakeInstallSource::default())).await
        }

        async fn with_source(source: Arc<dyn FrameworkSource>) -> Self {
            let env = TestEnv::new().await;
            let mut services = ServerServices::with_docker(env.docker.clone(), env._scratch.path());
            services.running_override = Some(false);
            services.frameworks = source;
            let library = LibraryPaths::new(env._scratch.path());
            Self {
                env,
                services,
                library,
            }
        }

        fn root(&self) -> PathBuf {
            self.env._scratch.path().join("Palworld")
        }

        async fn client(&mut self, ue4ss_mode: &str) -> String {
            let root = self.root();
            std::fs::create_dir_all(root.join("Pal/Binaries/Win64")).unwrap();
            std::fs::write(
                root.join("Pal/Binaries/Win64/Palworld-Win64-Shipping.exe"),
                b"stub",
            )
            .unwrap();
            std::fs::create_dir_all(root.join("Pal/Content/Paks/~mods")).unwrap();
            let db = &*self.env.app.driver;
            let target = ps_db::mod_targets::upsert(
                db,
                &ps_db::mod_targets::NewModTarget {
                    id: "client-steam".to_string(),
                    kind: "client".to_string(),
                    name: "Steam".to_string(),
                    root_path: root.to_string_lossy().into_owned(),
                    platform: "win64".to_string(),
                    ue4ss_mode: ue4ss_mode.to_string(),
                    layout_overrides: "{}".to_string(),
                    detected: "{}".to_string(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
            ps_db::mod_profiles::create(
                db,
                &ps_db::mod_profiles::NewProfile {
                    id: format!("{}/default", target.id),
                    target_id: target.id.clone(),
                    name: "Default".to_string(),
                    is_default: true,
                },
            )
            .await
            .unwrap();
            target.id
        }

        async fn install(&mut self, data: FrameworkInstallData) -> (Value, Vec<Value>) {
            handle_framework_install(&self.services, &self.library, data, &mut self.env.ctx())
                .await
                .unwrap();
            let frames = self.env.drain();
            let reply = frames
                .iter()
                .rev()
                .find(|frame| frame["type"] == "framework_install")
                .expect("a framework_install reply")["data"]
                .clone();
            (reply, frames)
        }

        async fn remove(&mut self, data: FrameworkRemoveData) -> Value {
            handle_framework_remove(&self.services, &self.library, data, &mut self.env.ctx())
                .await
                .unwrap();
            self.env
                .drain()
                .into_iter()
                .rev()
                .find(|frame| frame["type"] == "framework_remove")
                .expect("a framework_remove reply")["data"]
                .clone()
        }

        async fn hazard_remove(&mut self, data: FrameworkHazardRemoveData) -> Value {
            handle_framework_hazard_remove(
                &self.services,
                &self.library,
                data,
                &mut self.env.ctx(),
            )
            .await
            .unwrap();
            self.env
                .drain()
                .into_iter()
                .rev()
                .find(|frame| frame["type"] == "framework_hazard_remove")
                .expect("a framework_hazard_remove reply")["data"]
                .clone()
        }

        /// Stores `mod_id` as a `palschema`-typed library mod and enables it in
        /// the target's default profile, the way a user's own PalSchema mod
        /// (not the framework itself) would sit alongside it.
        async fn add_palschema_mod(&mut self, target_id: &str, mod_id: &str) {
            let scratch = tempfile::tempdir().unwrap();
            std::fs::write(scratch.path().join("main.lua"), b"x").unwrap();
            let manifest = ps_core::mods::InstallManifest {
                folder_name: mod_id.to_string(),
                display_name: mod_id.to_string(),
                mod_type: ps_core::mods::ModType::PalSchema,
                version: "1.0".to_string(),
                routes: vec![ps_core::mods::FileRoute {
                    archive_path: "main.lua".to_string(),
                    rel_path: "main.lua".to_string(),
                    kind: ps_core::mods::RouteKind::PalSchema,
                }],
                decisions: Vec::new(),
                platform_filtered: None,
                source: ps_core::mods::SourceHint::default(),
            };
            let db = &*self.env.app.driver;
            crate::services::mods::library::store(
                db,
                &self.library,
                &crate::services::mods::library::StoreRequest {
                    mod_id,
                    manifest: &manifest,
                    extracted_root: scratch.path(),
                    archive: None,
                    source_kind: "local",
                    source_ref: "{}",
                    custom_name: None,
                },
            )
            .await
            .unwrap();
            ps_db::mod_profiles::set_mod(
                db,
                &ps_db::mod_profiles::ProfileModRow {
                    profile_id: format!("{target_id}/default"),
                    mod_id: mod_id.to_string(),
                    mod_version_id: None,
                    enabled: true,
                    load_order: 5,
                },
            )
            .await
            .unwrap();
        }
    }

    fn remove_data(target_id: &str, key: &str, force: bool) -> FrameworkRemoveData {
        FrameworkRemoveData {
            target_id: target_id.to_string(),
            key: key.to_string(),
            force,
        }
    }

    fn hazard_remove_data(target_id: &str, hazard: &str) -> FrameworkHazardRemoveData {
        FrameworkHazardRemoveData {
            target_id: target_id.to_string(),
            hazard: hazard.to_string(),
        }
    }

    fn install_data(target_id: &str, key: &str) -> FrameworkInstallData {
        FrameworkInstallData {
            target_id: target_id.to_string(),
            key: key.to_string(),
            mod_version_id: None,
            channel: None,
            dev: false,
            replace: false,
        }
    }

    #[tokio::test]
    async fn a_fresh_ue4ss_install_deploys_and_switches_the_target_to_standard() {
        let mut fixture = InstallFixture::new().await;
        let target_id = fixture.client("none").await;

        let (reply, frames) = fixture.install(install_data(&target_id, "ue4ss")).await;

        assert_eq!(
            reply["mod_version_id"], "framework-ue4ss@2281fa31",
            "{reply:?}"
        );
        assert_eq!(reply["installed_new"], true, "{reply:?}");
        assert_eq!(reply["pending"], false, "{reply:?}");
        assert!(reply["apply"]["error"].is_null(), "{reply:?}");

        let root = fixture.root();
        assert!(root.join("Pal/Binaries/Win64/dwmapi.dll").is_file());
        assert!(root.join("Pal/Binaries/Win64/ue4ss/UE4SS.dll").is_file());
        let version =
            std::fs::read_to_string(root.join("Pal/Binaries/Win64/ue4ss/ue4ss.version")).unwrap();
        assert_eq!(version, "2281fa31");
        assert!(root
            .join("Pal/Binaries/Win64/ue4ss/Mods/Keybinds/Scripts/main.lua")
            .is_file());
        let mods_txt =
            std::fs::read_to_string(root.join("Pal/Binaries/Win64/ue4ss/Mods/mods.txt")).unwrap();
        assert!(
            mods_txt.starts_with("CheatManagerEnablerMod : 0"),
            "{mods_txt:?}"
        );

        let target = ps_db::mod_targets::get(&*fixture.env.app.driver, &target_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(target.ue4ss_mode, "standard");

        assert!(
            frames
                .iter()
                .any(|frame| frame["type"] == "mod_progress"
                    && frame["data"]["stage"] == "downloading"),
            "{frames:?}"
        );
    }

    #[tokio::test]
    async fn a_hand_installed_ue4ss_lets_palschema_install_and_promotes_the_stored_mode() {
        let mut fixture = InstallFixture::new().await;
        let target_id = fixture.client("none").await;
        let root = fixture.root();
        std::fs::write(
            root.join("Pal/Binaries/Win64/dwmapi.dll"),
            b"hand-installed",
        )
        .unwrap();

        let (reply, _) = fixture.install(install_data(&target_id, "palschema")).await;

        assert!(reply["apply"]["error"].is_null(), "{reply:?}");
        assert!(root
            .join("Pal/Binaries/Win64/ue4ss/Mods/PalSchema/dlls/main.dll")
            .is_file());
        let target = ps_db::mod_targets::get(&*fixture.env.app.driver, &target_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(target.ue4ss_mode, "standard");
    }

    #[tokio::test]
    async fn downloading_progress_frames_only_fire_on_a_percentage_change() {
        let mut fixture = InstallFixture::with_source(Arc::new(FakeInstallSource {
            repeat_progress: true,
        }))
        .await;
        let target_id = fixture.client("none").await;

        let (reply, frames) = fixture.install(install_data(&target_id, "ue4ss")).await;

        assert!(reply["apply"]["error"].is_null(), "{reply:?}");
        let downloading_pcts: Vec<u64> = frames
            .iter()
            .filter(|frame| {
                frame["type"] == "mod_progress" && frame["data"]["stage"] == "downloading"
            })
            .map(|frame| frame["data"]["pct"].as_u64().unwrap())
            .collect();
        assert_eq!(
            downloading_pcts,
            vec![0, 10, 50, 90],
            "{downloading_pcts:?}"
        );
    }

    #[tokio::test]
    async fn palschema_without_ue4ss_is_refused_and_writes_no_slot() {
        let mut fixture = InstallFixture::new().await;
        let target_id = fixture.client("none").await;

        let (reply, _) = fixture.install(install_data(&target_id, "palschema")).await;

        assert_eq!(reply["error"]["code"], "framework_required", "{reply:?}");
        let slot =
            ps_db::mod_profiles::framework_of(&*fixture.env.app.driver, &target_id, "palschema")
                .await
                .unwrap();
        assert!(slot.is_none(), "{slot:?}");
    }

    #[tokio::test]
    async fn ue4ss_over_workshop_mode_is_a_framework_conflict() {
        let mut fixture = InstallFixture::new().await;
        let target_id = fixture.client("none").await;
        let root = fixture.root();
        std::fs::create_dir_all(root.join("Mods/NativeMods/UE4SS")).unwrap();
        std::fs::write(root.join("Mods/NativeMods/UE4SS/x.txt"), b"x").unwrap();

        let (reply, _) = fixture.install(install_data(&target_id, "ue4ss")).await;

        assert_eq!(reply["error"]["code"], "framework_conflict", "{reply:?}");
        assert_eq!(reply["error"]["detail"], "workshop", "{reply:?}");
    }

    #[tokio::test]
    async fn an_occupied_destination_is_refused_then_replaced() {
        let mut fixture = InstallFixture::new().await;
        let target_id = fixture.client("none").await;
        let root = fixture.root();
        std::fs::write(root.join("Pal/Binaries/Win64/dwmapi.dll"), b"old").unwrap();

        let (first, _) = fixture.install(install_data(&target_id, "ue4ss")).await;
        assert_eq!(
            first["apply"]["error"]["code"], "unmanaged_occupant",
            "{first:?}"
        );
        let mod_version_id = first["mod_version_id"].as_str().unwrap().to_string();
        let slot = ps_db::mod_profiles::framework_of(&*fixture.env.app.driver, &target_id, "ue4ss")
            .await
            .unwrap();
        assert!(slot.is_some(), "the slot must survive a pending apply");

        let (second, _) = fixture
            .install(FrameworkInstallData {
                target_id: target_id.clone(),
                key: "ue4ss".to_string(),
                mod_version_id: Some(mod_version_id),
                channel: None,
                dev: false,
                replace: true,
            })
            .await;

        assert!(second["apply"]["error"].is_null(), "{second:?}");
        assert!(!second["apply"]["backup_dir"].is_null(), "{second:?}");
        let bytes = std::fs::read(root.join("Pal/Binaries/Win64/dwmapi.dll")).unwrap();
        assert_eq!(bytes, b"proxy");
    }

    #[tokio::test]
    async fn a_version_of_another_mod_is_refused() {
        let mut fixture = InstallFixture::new().await;
        let target_id = fixture.client("none").await;
        let scratch = tempfile::tempdir().unwrap();
        std::fs::write(scratch.path().join("mod_P.pak"), b"pak").unwrap();
        let manifest = ps_core::mods::InstallManifest {
            folder_name: "CoolMod".to_string(),
            display_name: "CoolMod".to_string(),
            mod_type: ps_core::mods::ModType::Pak,
            version: "1.0".to_string(),
            routes: vec![ps_core::mods::FileRoute {
                archive_path: "mod_P.pak".to_string(),
                rel_path: "mod_P.pak".to_string(),
                kind: ps_core::mods::RouteKind::Pak,
            }],
            decisions: Vec::new(),
            platform_filtered: None,
            source: ps_core::mods::SourceHint::default(),
        };
        let stored = crate::services::mods::library::store(
            &*fixture.env.app.driver,
            &fixture.library,
            &crate::services::mods::library::StoreRequest {
                mod_id: "cool-mod",
                manifest: &manifest,
                extracted_root: scratch.path(),
                archive: None,
                source_kind: "local",
                source_ref: "{}",
                custom_name: None,
            },
        )
        .await
        .unwrap();

        let (reply, _) = fixture
            .install(FrameworkInstallData {
                target_id: target_id.clone(),
                key: "ue4ss".to_string(),
                mod_version_id: Some(stored.version.id.clone()),
                channel: None,
                dev: false,
                replace: false,
            })
            .await;

        assert_eq!(reply["error"]["code"], "version_not_of_mod", "{reply:?}");
    }

    #[tokio::test]
    async fn force_order_keeps_palschemas_enabled_marker() {
        let mut fixture = InstallFixture::new().await;
        let target_id = fixture.client("none").await;

        let (ue4ss_reply, _) = fixture.install(install_data(&target_id, "ue4ss")).await;
        assert!(ue4ss_reply["apply"]["error"].is_null(), "{ue4ss_reply:?}");

        let (palschema_reply, _) = fixture.install(install_data(&target_id, "palschema")).await;
        assert!(
            palschema_reply["apply"]["error"].is_null(),
            "{palschema_reply:?}"
        );

        let db = &*fixture.env.app.driver;
        ps_db::mod_profiles::set_options(
            db,
            &format!("{target_id}/default"),
            "mods_txt",
            true,
            false,
        )
        .await
        .unwrap();

        let target = ps_db::mod_targets::get(db, &target_id)
            .await
            .unwrap()
            .unwrap();
        let _ = crate::mods_handlers::run_apply(
            &fixture.services,
            &fixture.library,
            db,
            &fixture.env.emitter,
            &target,
            &[],
            false,
        )
        .await;

        let root = fixture.root();
        assert!(root
            .join("Pal/Binaries/Win64/ue4ss/Mods/PalSchema/enabled.txt")
            .is_file());
    }

    #[tokio::test]
    async fn ue4ss_dependents_block_removal_then_force_removes_it() {
        let mut fixture = InstallFixture::new().await;
        let target_id = fixture.client("none").await;
        let (ue4ss_reply, _) = fixture.install(install_data(&target_id, "ue4ss")).await;
        assert!(ue4ss_reply["apply"]["error"].is_null(), "{ue4ss_reply:?}");
        let (palschema_reply, _) = fixture.install(install_data(&target_id, "palschema")).await;
        assert!(
            palschema_reply["apply"]["error"].is_null(),
            "{palschema_reply:?}"
        );

        let blocked = fixture
            .remove(remove_data(&target_id, "ue4ss", false))
            .await;
        assert_eq!(
            blocked["error"]["code"], "framework_required",
            "{blocked:?}"
        );
        assert_eq!(
            blocked["error"]["dependents"],
            json!(["palschema"]),
            "{blocked:?}"
        );
        let slot = ps_db::mod_profiles::framework_of(&*fixture.env.app.driver, &target_id, "ue4ss")
            .await
            .unwrap();
        assert!(slot.is_some(), "the slot must survive a blocked removal");

        let removed = fixture.remove(remove_data(&target_id, "ue4ss", true)).await;
        assert_eq!(removed["removed"], true, "{removed:?}");
        assert_eq!(removed["also_removed"], json!(["palschema"]), "{removed:?}");
        assert!(removed["apply"]["error"].is_null(), "{removed:?}");

        let root = fixture.root();
        assert!(!root.join("Pal/Binaries/Win64/dwmapi.dll").exists());
        assert!(!root.join("Pal/Binaries/Win64/ue4ss/UE4SS.dll").exists());
        let palschema_slot =
            ps_db::mod_profiles::framework_of(&*fixture.env.app.driver, &target_id, "palschema")
                .await
                .unwrap();
        assert!(palschema_slot.is_none(), "{palschema_slot:?}");
        let target = ps_db::mod_targets::get(&*fixture.env.app.driver, &target_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(target.ue4ss_mode, "none");

        let db = &*fixture.env.app.driver;
        let reapplied = crate::mods_handlers::run_apply(
            &fixture.services,
            &fixture.library,
            db,
            &fixture.env.emitter,
            &target,
            &[],
            false,
        )
        .await;
        assert!(reapplied["error"].is_null(), "{reapplied:?}");
    }

    #[tokio::test]
    async fn ue4ss_removal_dependents_include_enabled_logicmods() {
        let mut fixture = InstallFixture::new().await;
        let target_id = fixture.client("none").await;
        let (ue4ss_reply, _) = fixture.install(install_data(&target_id, "ue4ss")).await;
        assert!(ue4ss_reply["apply"]["error"].is_null(), "{ue4ss_reply:?}");

        let scratch = tempfile::tempdir().unwrap();
        std::fs::write(scratch.path().join("mod_P.pak"), b"x").unwrap();
        let manifest = ps_core::mods::InstallManifest {
            folder_name: "LogicPack".to_string(),
            display_name: "LogicPack".to_string(),
            mod_type: ps_core::mods::ModType::LogicMods,
            version: "1.0".to_string(),
            routes: vec![ps_core::mods::FileRoute {
                archive_path: "mod_P.pak".to_string(),
                rel_path: "mod_P.pak".to_string(),
                kind: ps_core::mods::RouteKind::LogicMods,
            }],
            decisions: Vec::new(),
            platform_filtered: None,
            source: ps_core::mods::SourceHint::default(),
        };
        let db = &*fixture.env.app.driver;
        crate::services::mods::library::store(
            db,
            &fixture.library,
            &crate::services::mods::library::StoreRequest {
                mod_id: "cool-logicmod",
                manifest: &manifest,
                extracted_root: scratch.path(),
                archive: None,
                source_kind: "local",
                source_ref: "{}",
                custom_name: None,
            },
        )
        .await
        .unwrap();
        ps_db::mod_profiles::set_mod(
            db,
            &ps_db::mod_profiles::ProfileModRow {
                profile_id: format!("{target_id}/default"),
                mod_id: "cool-logicmod".to_string(),
                mod_version_id: None,
                enabled: true,
                load_order: 5,
            },
        )
        .await
        .unwrap();

        let blocked = fixture
            .remove(remove_data(&target_id, "ue4ss", false))
            .await;

        assert_eq!(
            blocked["error"]["code"], "framework_required",
            "{blocked:?}"
        );
        let dependents = blocked["error"]["dependents"].as_array().unwrap();
        assert!(
            dependents
                .iter()
                .any(|dependent| dependent == "cool-logicmod"),
            "{blocked:?}"
        );
    }

    #[tokio::test]
    async fn palschema_dependents_block_removal() {
        let mut fixture = InstallFixture::new().await;
        let target_id = fixture.client("none").await;
        let (ue4ss_reply, _) = fixture.install(install_data(&target_id, "ue4ss")).await;
        assert!(ue4ss_reply["apply"]["error"].is_null(), "{ue4ss_reply:?}");
        let (palschema_reply, _) = fixture.install(install_data(&target_id, "palschema")).await;
        assert!(
            palschema_reply["apply"]["error"].is_null(),
            "{palschema_reply:?}"
        );
        fixture
            .add_palschema_mod(&target_id, "cool-palschema-mod")
            .await;

        let blocked = fixture
            .remove(remove_data(&target_id, "palschema", false))
            .await;

        assert_eq!(
            blocked["error"]["code"], "framework_required",
            "{blocked:?}"
        );
        let dependents = blocked["error"]["dependents"].as_array().unwrap();
        assert!(
            dependents
                .iter()
                .any(|dependent| dependent == "cool-palschema-mod"),
            "{blocked:?}"
        );
    }

    #[tokio::test]
    async fn removing_an_uninstalled_framework_is_refused() {
        let mut fixture = InstallFixture::new().await;
        let target_id = fixture.client("none").await;

        let reply = fixture
            .remove(remove_data(&target_id, "amity", false))
            .await;

        assert_eq!(
            reply["error"]["code"], "framework_not_installed",
            "{reply:?}"
        );
    }

    #[tokio::test]
    async fn the_workshop_proxy_dll_hazard_is_moved_to_a_backup_set() {
        let mut fixture = InstallFixture::new().await;
        let target_id = fixture.client("none").await;
        let root = fixture.root();
        std::fs::create_dir_all(root.join("Mods/NativeMods/UE4SS")).unwrap();
        std::fs::write(root.join("Mods/NativeMods/UE4SS/x.txt"), b"x").unwrap();
        std::fs::write(
            root.join("Mods/PalModSettings.ini"),
            b"[PalModSettings]\nbGlobalEnableMod=True\nActiveModList=UE4SS\n",
        )
        .unwrap();
        std::fs::write(root.join("Pal/Binaries/Win64/dwmapi.dll"), b"x").unwrap();

        let reply = fixture
            .hazard_remove(hazard_remove_data(&target_id, "workshop_proxy_dll"))
            .await;

        assert!(reply["error"].is_null(), "{reply:?}");
        let moved = reply["moved"].as_array().unwrap();
        assert_eq!(moved.len(), 1, "{reply:?}");
        assert!(
            moved[0].as_str().unwrap().ends_with("dwmapi.dll"),
            "{reply:?}"
        );
        assert!(!root.join("Pal/Binaries/Win64/dwmapi.dll").exists());
        let backup_dir = reply["backup_dir"].as_str().unwrap();
        assert!(
            std::path::Path::new(backup_dir)
                .join("index.json")
                .is_file(),
            "{reply:?}"
        );

        let repeat = fixture
            .hazard_remove(hazard_remove_data(&target_id, "workshop_proxy_dll"))
            .await;
        assert_eq!(repeat["error"]["code"], "hazard_not_found", "{repeat:?}");
    }

    #[tokio::test]
    async fn a_managed_ue4ss_dwmapi_is_excluded_from_the_workshop_proxy_hazard() {
        let mut fixture = InstallFixture::new().await;
        let target_id = fixture.client("none").await;
        let (ue4ss_reply, _) = fixture.install(install_data(&target_id, "ue4ss")).await;
        assert!(ue4ss_reply["apply"]["error"].is_null(), "{ue4ss_reply:?}");

        let root = fixture.root();
        std::fs::create_dir_all(root.join("Mods/NativeMods/UE4SS")).unwrap();
        std::fs::write(root.join("Mods/NativeMods/UE4SS/x.txt"), b"x").unwrap();
        std::fs::write(
            root.join("Mods/PalModSettings.ini"),
            b"[PalModSettings]\nbGlobalEnableMod=True\nActiveModList=UE4SS\n",
        )
        .unwrap();

        let reply = fixture
            .hazard_remove(hazard_remove_data(&target_id, "workshop_proxy_dll"))
            .await;

        assert_eq!(reply["error"]["code"], "hazard_not_found", "{reply:?}");
        assert!(
            root.join("Pal/Binaries/Win64/dwmapi.dll").is_file(),
            "the managed dll must stay in place"
        );
    }

    #[tokio::test]
    async fn the_legacy_amity_folder_hazard_moves_every_file_and_removes_itself() {
        let mut fixture = InstallFixture::new().await;
        let target_id = fixture.client("none").await;
        let root = fixture.root();
        std::fs::write(root.join("Pal/Binaries/Win64/dwmapi.dll"), b"x").unwrap();
        let legacy = root.join("Pal/Binaries/Win64/ue4ss/Mods/PSPAmity");
        std::fs::create_dir_all(legacy.join("dlls")).unwrap();
        std::fs::write(legacy.join("dlls/main.dll"), b"x").unwrap();
        std::fs::write(legacy.join("PSPAmity.ini"), b"x").unwrap();

        let reply = fixture
            .hazard_remove(hazard_remove_data(&target_id, "amity_legacy_folder"))
            .await;

        assert!(reply["error"].is_null(), "{reply:?}");
        let moved = reply["moved"].as_array().unwrap();
        assert_eq!(moved.len(), 2, "{reply:?}");
        assert!(!legacy.exists(), "the legacy folder must be gone");
    }

    #[tokio::test]
    async fn a_symlinked_legacy_amity_folder_is_not_removed_as_a_hazard() {
        let mut fixture = InstallFixture::new().await;
        let target_id = fixture.client("none").await;
        let root = fixture.root();
        std::fs::write(root.join("Pal/Binaries/Win64/dwmapi.dll"), b"x").unwrap();
        let mods_dir = root.join("Pal/Binaries/Win64/ue4ss/Mods");
        std::fs::create_dir_all(&mods_dir).unwrap();
        let outside = fixture.env._scratch.path().join("outside-amity");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("main.dll"), b"x").unwrap();
        let link = mods_dir.join("PSPAmity");
        #[cfg(windows)]
        let created = std::os::windows::fs::symlink_dir(&outside, &link).is_ok();
        #[cfg(unix)]
        let created = std::os::unix::fs::symlink(&outside, &link).is_ok();
        if !created {
            eprintln!("skipping: could not create a symlink in this environment");
            return;
        }

        let reply = fixture
            .hazard_remove(hazard_remove_data(&target_id, "amity_legacy_folder"))
            .await;

        assert_eq!(reply["error"]["code"], "hazard_not_found", "{reply:?}");
        assert!(outside.join("main.dll").exists(), "{reply:?}");
        assert!(link.exists(), "the symlink itself must be untouched");
    }

    #[tokio::test]
    async fn legacy_folder_cleanup_removes_only_empty_directories() {
        let mut fixture = InstallFixture::new().await;
        let target_id = fixture.client("none").await;
        let root = fixture.root();
        std::fs::write(root.join("Pal/Binaries/Win64/dwmapi.dll"), b"x").unwrap();
        let legacy = root.join("Pal/Binaries/Win64/ue4ss/Mods/PSPAmity");
        std::fs::create_dir_all(legacy.join("dlls")).unwrap();
        std::fs::write(legacy.join("dlls/main.dll"), b"x").unwrap();
        std::fs::create_dir_all(legacy.join("leftover")).unwrap();
        let survivor_target = fixture.env._scratch.path().join("outside-file.txt");
        std::fs::write(&survivor_target, b"kept").unwrap();
        let survivor_link = legacy.join("leftover/kept.txt");
        #[cfg(windows)]
        let linked = std::os::windows::fs::symlink_file(&survivor_target, &survivor_link).is_ok();
        #[cfg(unix)]
        let linked = std::os::unix::fs::symlink(&survivor_target, &survivor_link).is_ok();
        if !linked {
            eprintln!("skipping: could not create a symlink in this environment");
            return;
        }

        let reply = fixture
            .hazard_remove(hazard_remove_data(&target_id, "amity_legacy_folder"))
            .await;

        assert!(reply["error"].is_null(), "{reply:?}");
        assert!(
            legacy.exists(),
            "the legacy root must survive a non-empty child"
        );
        assert!(
            legacy.join("leftover").exists(),
            "a directory holding a symlink must survive"
        );
        assert!(
            survivor_link.exists(),
            "the symlink itself must never be moved or deleted"
        );
        assert!(
            !legacy.join("dlls").exists(),
            "an emptied directory must be removed"
        );
    }

    #[tokio::test]
    async fn a_locked_target_refuses_hazard_removal() {
        let mut fixture = InstallFixture::new().await;
        let target_id = fixture.client("none").await;
        let root = fixture.root();
        std::fs::write(root.join("Pal/Binaries/Win64/dwmapi.dll"), b"x").unwrap();
        let legacy = root.join("Pal/Binaries/Win64/ue4ss/Mods/PSPAmity");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join("main.dll"), b"x").unwrap();
        let target = ps_db::mod_targets::get(&*fixture.env.app.driver, &target_id)
            .await
            .unwrap()
            .unwrap();
        let _guard = deploy::try_lock_target(&fixture.library, &target).unwrap();

        let reply = fixture
            .hazard_remove(hazard_remove_data(&target_id, "amity_legacy_folder"))
            .await;

        assert_eq!(reply["error"]["code"], "apply_in_progress", "{reply:?}");
    }

    #[tokio::test]
    async fn an_unknown_hazard_is_invalid() {
        let mut fixture = InstallFixture::new().await;
        let target_id = fixture.client("none").await;

        let reply = fixture
            .hazard_remove(hazard_remove_data(&target_id, "ue4ss_dual_instance"))
            .await;

        assert_eq!(reply["error"]["code"], "invalid_hazard", "{reply:?}");
    }
}
