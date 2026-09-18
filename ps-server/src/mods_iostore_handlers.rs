//! Wire handler that converts a Game Pass target's legacy pak mod into
//! IoStore containers, storing the result as a new library version pinned
//! only on that target's active profile.
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::json;

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;
use crate::mods_handlers::{apply_after_selection, emit_progress, emit_refusal, target_or_refusal};
use crate::services::mods::frameworks::install::scratch_root;
use crate::services::mods::frameworks::source::Progress;
use crate::services::mods::iostore::ConvertedPak;
use crate::services::mods::paths::{kind_segment, LibraryPaths};
use crate::services::mods::{deploy, library};
use crate::services::ServerServices;

use ps_core::mods::conflicts::{iostore_sibling, is_pak_route};
use ps_core::mods::{FileRoute, InstallManifest, RouteKind};

#[derive(Debug, serde::Deserialize)]
pub struct ModIostoreConvertData {
    pub target_id: String,
    pub mod_id: String,
}

/// Removes the scratch tree on drop, so a refusal partway through a
/// conversion leaves nothing behind.
struct ScratchGuard(PathBuf);

impl Drop for ScratchGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `rel_path` with its extension swapped, its directory left as it was.
fn sibling_rel_path(rel_path: &str, extension: &str) -> String {
    let split_at = rel_path.rfind('/').map(|index| index + 1).unwrap_or(0);
    let (dir, name) = rel_path.split_at(split_at);
    let base = match name.rfind('.') {
        Some(index) => &name[..index],
        None => name,
    };
    format!("{dir}{base}.{extension}")
}

/// Joins a `/`-separated archive path onto `root` one normal component at a
/// time, mirroring `library::store`'s own resolution of `archive_path`.
fn join_archive_path(root: &Path, archive_path: &str) -> PathBuf {
    let mut path = root.to_path_buf();
    for segment in archive_path.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            continue;
        }
        path.push(segment);
    }
    path
}

pub async fn handle_mod_iostore_convert(
    services: &ServerServices,
    library: &LibraryPaths,
    data: ModIostoreConvertData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let message_type = MessageType::ModIostoreConvert;
    let context = json!({ "target_id": data.target_id, "mod_id": data.mod_id });

    if !ctx.app.config.desktop_mode {
        emit_refusal(
            ctx,
            message_type,
            context,
            "desktop_only",
            "desktop mode is required to convert a mod to IoStore".to_string(),
            json!({}),
        );
        return Ok(());
    }

    let Some(target) =
        target_or_refusal(ctx, message_type, &data.target_id, context.clone()).await?
    else {
        return Ok(());
    };

    if target.platform != "wingdk" {
        emit_refusal(
            ctx,
            message_type,
            context.clone(),
            "not_supported_on_target",
            format!("{} is not a Game Pass target", target.id),
            json!({}),
        );
        return Ok(());
    }

    let db = &*ctx.app.driver;
    let profile = match ps_db::mod_profiles::active_for_target(db, &target.id).await {
        Ok(Some(profile)) => profile,
        Ok(None) => {
            emit_refusal(
                ctx,
                message_type,
                context.clone(),
                "no_active_profile",
                format!("target {} has no active profile", target.id),
                json!({}),
            );
            return Ok(());
        }
        Err(error) => {
            emit_refusal(
                ctx,
                message_type,
                context.clone(),
                "db",
                error.to_string(),
                json!({}),
            );
            return Ok(());
        }
    };

    let entries = match ps_db::mod_profiles::mods_of(db, &profile.id).await {
        Ok(entries) => entries,
        Err(error) => {
            emit_refusal(
                ctx,
                message_type,
                context.clone(),
                "db",
                error.to_string(),
                json!({}),
            );
            return Ok(());
        }
    };
    let Some(entry) = entries
        .into_iter()
        .find(|entry| entry.mod_id == data.mod_id)
    else {
        emit_refusal(
            ctx,
            message_type,
            context.clone(),
            "mod_not_in_profile",
            format!("{} is not in {}'s active profile", data.mod_id, target.id),
            json!({}),
        );
        return Ok(());
    };

    let resolved_version = match &entry.mod_version_id {
        Some(version_id) => ps_db::mod_library::get_version(db, version_id).await,
        None => ps_db::mod_library::current_version(db, &data.mod_id).await,
    };
    let resolved_version = match resolved_version {
        Ok(version) => version,
        Err(error) => {
            emit_refusal(
                ctx,
                message_type,
                context.clone(),
                "db",
                error.to_string(),
                json!({}),
            );
            return Ok(());
        }
    };
    let Some(version) = resolved_version else {
        emit_refusal(
            ctx,
            message_type,
            context.clone(),
            "version_not_found",
            format!("{} has no resolvable version to convert", data.mod_id),
            json!({}),
        );
        return Ok(());
    };

    let manifest: InstallManifest = match serde_json::from_str(&version.manifest) {
        Ok(manifest) => manifest,
        Err(_) => {
            emit_refusal(
                ctx,
                message_type,
                context.clone(),
                "not_convertible",
                format!("{} has no readable manifest", version.id),
                json!({}),
            );
            return Ok(());
        }
    };

    let legacy_routes: Vec<&FileRoute> = manifest
        .routes
        .iter()
        .filter(|route| is_pak_route(route))
        .collect();
    if legacy_routes.is_empty() {
        emit_refusal(
            ctx,
            message_type,
            context.clone(),
            "not_convertible",
            format!("{} has no legacy pak to convert", version.id),
            json!({}),
        );
        return Ok(());
    }
    let routes_to_convert: Vec<&FileRoute> = legacy_routes
        .iter()
        .copied()
        .filter(|route| iostore_sibling(&manifest, route).is_none())
        .collect();
    if routes_to_convert.is_empty() {
        emit_refusal(
            ctx,
            message_type,
            context.clone(),
            "already_iostore",
            format!("{} is already IoStore-packed", version.id),
            json!({}),
        );
        return Ok(());
    }

    let new_version_string = format!("{}+iostore", manifest.version);
    let new_version_id = library::version_id(&data.mod_id, &new_version_string);
    let converted_names: Vec<String> = routes_to_convert
        .iter()
        .map(|route| route.rel_path.clone())
        .collect();

    let already_stored = match ps_db::mod_library::get_version(db, &new_version_id).await {
        Ok(existing) => existing,
        Err(error) => {
            emit_refusal(
                ctx,
                message_type,
                context.clone(),
                "db",
                error.to_string(),
                json!({}),
            );
            return Ok(());
        }
    };
    let reused = already_stored.is_some();

    if !reused {
        let mod_row = match ps_db::mod_library::get_mod(db, &data.mod_id).await {
            Ok(Some(row)) => row,
            Ok(None) => {
                emit_refusal(
                    ctx,
                    message_type,
                    context.clone(),
                    "db",
                    format!("{} is not in the library", data.mod_id),
                    json!({}),
                );
                return Ok(());
            }
            Err(error) => {
                emit_refusal(
                    ctx,
                    message_type,
                    context.clone(),
                    "db",
                    error.to_string(),
                    json!({}),
                );
                return Ok(());
            }
        };
        let previous_current = match ps_db::mod_library::current_version(db, &data.mod_id).await {
            Ok(previous) => previous,
            Err(error) => {
                emit_refusal(
                    ctx,
                    message_type,
                    context.clone(),
                    "db",
                    error.to_string(),
                    json!({}),
                );
                return Ok(());
            }
        };

        let request_id = deploy::backup::new_op_id();
        let version_dir = PathBuf::from(&version.library_dir);
        let scratch = scratch_root(library).join(&request_id);
        let _scratch_guard = ScratchGuard(scratch.clone());
        let extracted_root = scratch.join("extracted");

        let mut converted_paks: HashMap<(RouteKind, String), ConvertedPak> = HashMap::new();
        let total = routes_to_convert.len();
        let no_progress: Progress = &|_, _| {};
        for (index, route) in routes_to_convert.iter().enumerate() {
            let pct = (((index + 1) * 99) / total).min(99) as u8;
            emit_progress(
                ctx.emitter,
                &request_id,
                &target.id,
                "converting",
                pct,
                &format!("converting {}", route.rel_path),
            );
            let input = LibraryPaths::route_path_in(&version_dir, route.kind, &route.rel_path);
            let out_dir = scratch.join("out").join(index.to_string());
            match services
                .iostore
                .convert(&input, &out_dir, no_progress)
                .await
            {
                Ok(converted) => {
                    converted_paks.insert((route.kind, route.rel_path.clone()), converted);
                }
                Err(error) => {
                    emit_refusal(
                        ctx,
                        message_type,
                        context.clone(),
                        error.code(),
                        error.to_string(),
                        json!({}),
                    );
                    return Ok(());
                }
            }
        }

        emit_progress(
            ctx.emitter,
            &request_id,
            &target.id,
            "storing",
            100,
            "storing the converted version",
        );

        // A carried-through route (e.g. an orphan `.ucas` the analyzer typed
        // as `Pak`) may already occupy the path a fresh sibling below will
        // write to; the fresh output wins, so such a route is dropped here
        // rather than stored twice under `library::store`.
        let mut generated_siblings: std::collections::HashSet<(RouteKind, String)> =
            std::collections::HashSet::new();
        for route in &routes_to_convert {
            generated_siblings.insert((route.kind, sibling_rel_path(&route.rel_path, "utoc")));
            generated_siblings.insert((route.kind, sibling_rel_path(&route.rel_path, "ucas")));
        }

        let mut new_routes: Vec<FileRoute> = Vec::with_capacity(manifest.routes.len() + total * 2);
        for route in &manifest.routes {
            let key = (route.kind, route.rel_path.clone());
            let archive_path = format!("{}/{}", kind_segment(route.kind), route.rel_path);
            let destination = join_archive_path(&extracted_root, &archive_path);
            if let Some(parent) = destination.parent() {
                if let Err(error) = std::fs::create_dir_all(parent) {
                    emit_refusal(
                        ctx,
                        message_type,
                        context.clone(),
                        "io",
                        error.to_string(),
                        json!({}),
                    );
                    return Ok(());
                }
            }
            let Some(converted) = converted_paks.get(&key) else {
                if generated_siblings.contains(&key) {
                    continue;
                }
                let source = LibraryPaths::route_path_in(&version_dir, route.kind, &route.rel_path);
                if let Err(error) = std::fs::copy(&source, &destination) {
                    emit_refusal(
                        ctx,
                        message_type,
                        context.clone(),
                        "io",
                        error.to_string(),
                        json!({}),
                    );
                    return Ok(());
                }
                new_routes.push(FileRoute {
                    archive_path,
                    rel_path: route.rel_path.clone(),
                    kind: route.kind,
                });
                continue;
            };
            if let Err(error) = std::fs::copy(&converted.pak, &destination) {
                emit_refusal(
                    ctx,
                    message_type,
                    context.clone(),
                    "io",
                    error.to_string(),
                    json!({}),
                );
                return Ok(());
            }
            new_routes.push(FileRoute {
                archive_path,
                rel_path: route.rel_path.clone(),
                kind: route.kind,
            });
            for (extension, source) in [("utoc", &converted.utoc), ("ucas", &converted.ucas)] {
                let sibling = sibling_rel_path(&route.rel_path, extension);
                let sibling_archive = format!("{}/{}", kind_segment(route.kind), sibling);
                let sibling_destination = join_archive_path(&extracted_root, &sibling_archive);
                if let Some(parent) = sibling_destination.parent() {
                    if let Err(error) = std::fs::create_dir_all(parent) {
                        emit_refusal(
                            ctx,
                            message_type,
                            context.clone(),
                            "io",
                            error.to_string(),
                            json!({}),
                        );
                        return Ok(());
                    }
                }
                if let Err(error) = std::fs::copy(source, &sibling_destination) {
                    emit_refusal(
                        ctx,
                        message_type,
                        context.clone(),
                        "io",
                        error.to_string(),
                        json!({}),
                    );
                    return Ok(());
                }
                new_routes.push(FileRoute {
                    archive_path: sibling_archive,
                    rel_path: sibling,
                    kind: route.kind,
                });
            }
        }

        {
            let mut seen = std::collections::HashSet::new();
            for route in &new_routes {
                if !seen.insert((route.kind, route.rel_path.clone())) {
                    emit_refusal(
                        ctx,
                        message_type,
                        context.clone(),
                        "not_convertible",
                        format!("{} would produce a duplicate route", version.id),
                        json!({}),
                    );
                    return Ok(());
                }
            }
        }

        let new_manifest = InstallManifest {
            folder_name: manifest.folder_name.clone(),
            display_name: manifest.display_name.clone(),
            mod_type: manifest.mod_type,
            version: new_version_string.clone(),
            routes: new_routes,
            decisions: Vec::new(),
            platform_filtered: manifest.platform_filtered,
            source: manifest.source.clone(),
        };

        let stored = library::store(
            db,
            library,
            &library::StoreRequest {
                mod_id: &data.mod_id,
                manifest: &new_manifest,
                extracted_root: &extracted_root,
                archive: None,
                source_kind: &mod_row.source_kind,
                source_ref: &version.source_ref,
                custom_name: None,
            },
        )
        .await;
        if let Err(error) = stored {
            emit_refusal(
                ctx,
                message_type,
                context.clone(),
                "library_error",
                error.to_string(),
                json!({}),
            );
            return Ok(());
        }

        // `library::store` only marks a version current when it is the mod's
        // first; this restore is what keeps it that way if that ever changes,
        // so a target that follows the mod's current version never picks up
        // Game Pass-only bytes a Steam target cannot read.
        if let Some(previous_current) = previous_current {
            if let Err(error) =
                ps_db::mod_library::set_current_version(db, &data.mod_id, &previous_current.id)
                    .await
            {
                emit_refusal(
                    ctx,
                    message_type,
                    context.clone(),
                    "db",
                    error.to_string(),
                    json!({}),
                );
                return Ok(());
            }
        }
    }

    let pinned_entry = ps_db::mod_profiles::ProfileModRow {
        profile_id: profile.id.clone(),
        mod_id: data.mod_id.clone(),
        mod_version_id: Some(new_version_id.clone()),
        enabled: entry.enabled,
        load_order: entry.load_order,
    };
    if let Err(error) = ps_db::mod_profiles::set_mod(db, &pinned_entry).await {
        emit_refusal(
            ctx,
            message_type,
            context.clone(),
            "db",
            error.to_string(),
            json!({}),
        );
        return Ok(());
    }

    let selection = apply_after_selection(services, library, db, ctx.emitter, &target).await;

    ctx.emitter.emit(
        message_type,
        &json!({
            "target_id": data.target_id,
            "mod_id": data.mod_id,
            "mod_version_id": new_version_id,
            "version": new_version_string,
            "converted": if reused { Vec::<String>::new() } else { converted_names },
            "reused": reused,
            "request_id": selection.request_id,
            "pending": selection.pending,
            "apply": selection.apply,
        }),
    );
    Ok(())
}
