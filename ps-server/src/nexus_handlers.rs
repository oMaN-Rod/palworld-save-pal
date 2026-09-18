//! Wire handlers for Nexus Mods: the stored API key and account, browsing,
//! downloads, nxm:// link delivery, link handler registration and update
//! checks. Every one runs only for the desktop app's own loopback connection.
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use ps_core::nexus::{Account, SearchParams};

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;
use crate::mods_handlers::{
    default_true, emit_progress, emit_refusal, install_archive_and_reply, target_or_refusal,
    ArchiveInstall, ArchiveInstallOutcome,
};
use crate::services::mods::uploads::UploadStore;
use crate::services::mods::{deploy, install, layout, library, LibraryPaths};
use crate::services::nexus::api::{NexusError, NxmAuth, MAX_DOWNLOAD};
use crate::services::nexus::keystore::{self, ApiKey, KeyStoreError};
use crate::services::nexus::now_secs;
use crate::services::nexus::protocol::{HandlerStatus, RegistryError};
use crate::services::ServerServices;

pub(crate) fn nexus_allowed(
    ctx: &mut HandlerCtx<'_>,
    message_type: MessageType,
    context: &Value,
) -> bool {
    if ctx.app.config.desktop_mode && ctx.is_loopback {
        return true;
    }
    emit_refusal(
        ctx,
        message_type,
        context.clone(),
        "desktop_only",
        "Nexus Mods features run only in the desktop app".to_string(),
        json!({}),
    );
    false
}

pub(crate) fn refuse_nexus(
    ctx: &mut HandlerCtx<'_>,
    message_type: MessageType,
    context: Value,
    error: &NexusError,
) {
    emit_refusal(
        ctx,
        message_type,
        context,
        error.code(),
        error.to_string(),
        error.detail(),
    );
}

pub(crate) fn refuse_key_store(
    ctx: &mut HandlerCtx<'_>,
    message_type: MessageType,
    context: Value,
    error: &KeyStoreError,
) {
    emit_refusal(
        ctx,
        message_type,
        context,
        error.code(),
        error.to_string(),
        json!({}),
    );
}

async fn stored_key(
    services: &ServerServices,
    ctx: &mut HandlerCtx<'_>,
    message_type: MessageType,
    context: &Value,
) -> Option<Option<ApiKey>> {
    match keystore::read_key(&services.nexus_keys).await {
        Ok(key) => Some(key),
        Err(error) => {
            refuse_key_store(ctx, message_type, context.clone(), &error);
            None
        }
    }
}

pub(crate) async fn required_key(
    services: &ServerServices,
    ctx: &mut HandlerCtx<'_>,
    message_type: MessageType,
    context: &Value,
) -> Option<ApiKey> {
    match stored_key(services, ctx, message_type, context).await? {
        Some(key) => Some(key),
        None => {
            emit_refusal(
                ctx,
                message_type,
                context.clone(),
                "key_required",
                "add a Nexus Mods API key first".to_string(),
                json!({}),
            );
            None
        }
    }
}

fn account_reply(services: &ServerServices, account: Option<&Account>) -> Value {
    json!({
        "has_key": account.is_some(),
        "account": account,
        "rate_limit": services.nexus.rate_limit(),
    })
}

pub async fn handle_nexus_account_get(
    services: &ServerServices,
    _data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let message_type = MessageType::NexusAccountGet;
    let context = json!({});
    if !nexus_allowed(ctx, message_type, &context) {
        return Ok(());
    }
    let Some(key) = stored_key(services, ctx, message_type, &context).await else {
        return Ok(());
    };
    let Some(key) = key else {
        ctx.emitter
            .emit(message_type, &account_reply(services, None));
        return Ok(());
    };
    match services.nexus.validate(&key).await {
        Ok(account) => ctx
            .emitter
            .emit(message_type, &account_reply(services, Some(&account))),
        Err(error) => refuse_nexus(ctx, message_type, json!({ "has_key": true }), &error),
    }
    Ok(())
}

pub async fn handle_nexus_key_set(
    services: &ServerServices,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let message_type = MessageType::NexusKeySet;
    let context = json!({});
    if !nexus_allowed(ctx, message_type, &context) {
        return Ok(());
    }
    let Some(key) = data
        .get("key")
        .and_then(Value::as_str)
        .and_then(ApiKey::parse)
    else {
        emit_refusal(
            ctx,
            message_type,
            context,
            "invalid_key",
            "that is not a Nexus Mods API key".to_string(),
            json!({}),
        );
        return Ok(());
    };
    let account = match services.nexus.validate(&key).await {
        Ok(account) => account,
        Err(error) => {
            refuse_nexus(ctx, message_type, context, &error);
            return Ok(());
        }
    };
    if let Err(error) = keystore::write_key(&services.nexus_keys, key).await {
        refuse_key_store(ctx, message_type, context, &error);
        return Ok(());
    }
    ctx.emitter
        .emit(message_type, &account_reply(services, Some(&account)));
    Ok(())
}

pub async fn handle_nexus_key_clear(
    services: &ServerServices,
    _data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let message_type = MessageType::NexusKeyClear;
    let context = json!({});
    if !nexus_allowed(ctx, message_type, &context) {
        return Ok(());
    }
    match keystore::clear_key(&services.nexus_keys).await {
        Ok(()) => ctx
            .emitter
            .emit(message_type, &account_reply(services, None)),
        Err(error) => refuse_key_store(ctx, message_type, context, &error),
    }
    Ok(())
}

pub async fn handle_nexus_categories(
    services: &ServerServices,
    _data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let message_type = MessageType::NexusCategories;
    let context = json!({});
    if !nexus_allowed(ctx, message_type, &context) {
        return Ok(());
    }
    let Some(key) = required_key(services, ctx, message_type, &context).await else {
        return Ok(());
    };
    match services.nexus.categories(&key).await {
        Ok(categories) => ctx
            .emitter
            .emit(message_type, &json!({ "categories": categories })),
        Err(error) => refuse_nexus(ctx, message_type, context, &error),
    }
    Ok(())
}

pub async fn handle_nexus_search(
    services: &ServerServices,
    data: SearchParams,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let message_type = MessageType::NexusSearch;
    let offset = data.offset.min(ps_core::nexus::MAX_OFFSET);
    let context = json!({ "offset": offset });
    if !nexus_allowed(ctx, message_type, &context) {
        return Ok(());
    }
    match services.nexus.search(&data).await {
        Ok(page) => ctx.emitter.emit(
            message_type,
            &json!({
                "offset": offset,
                "count": page.nodes.len(),
                "total_count": page.total_count,
                "mods": page.nodes,
            }),
        ),
        Err(error) => refuse_nexus(ctx, message_type, context, &error),
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct NexusModFilesData {
    pub mod_id: u32,
}

pub async fn handle_nexus_mod_files(
    services: &ServerServices,
    data: NexusModFilesData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let message_type = MessageType::NexusModFiles;
    let context = json!({ "mod_id": data.mod_id });
    if !nexus_allowed(ctx, message_type, &context) {
        return Ok(());
    }
    match services.nexus.mod_files(&[data.mod_id]).await {
        Ok(mut files) => {
            let files = files.remove(&data.mod_id).unwrap_or_default();
            let latest_file_id = ps_core::nexus::latest_file(&files).map(|file| file.file_id);
            ctx.emitter.emit(
                message_type,
                &json!({ "mod_id": data.mod_id, "files": files, "latest_file_id": latest_file_id }),
            );
        }
        Err(error) => refuse_nexus(ctx, message_type, context, &error),
    }
    Ok(())
}

#[derive(serde::Deserialize)]
pub struct NexusDownloadData {
    pub target_id: String,
    pub mod_id: u32,
    pub file_id: u32,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub expires: Option<u64>,
    #[serde(default)]
    pub accept_defaults: bool,
    #[serde(default = "default_true")]
    pub enable: bool,
}

impl std::fmt::Debug for NexusDownloadData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NexusDownloadData")
            .field("target_id", &self.target_id)
            .field("mod_id", &self.mod_id)
            .field("file_id", &self.file_id)
            .field("key", &self.key.as_ref().map(|_| "<redacted>"))
            .field("expires", &self.expires)
            .field("accept_defaults", &self.accept_defaults)
            .field("enable", &self.enable)
            .finish()
    }
}

pub fn download_dir(downloads: &Path, mod_id: u32, file_id: u32) -> PathBuf {
    downloads.join("nexus").join(format!("{mod_id}-{file_id}"))
}

pub async fn handle_nexus_download(
    services: &ServerServices,
    library: &LibraryPaths,
    uploads: &UploadStore,
    data: NexusDownloadData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let message_type = MessageType::NexusDownload;
    let context = json!({
        "target_id": data.target_id,
        "nexus_mod_id": data.mod_id,
        "file_id": data.file_id,
    });
    if !nexus_allowed(ctx, message_type, &context) {
        return Ok(());
    }
    let key = data
        .key
        .as_deref()
        .map(str::trim)
        .filter(|key| !key.is_empty());
    let nxm = match (key, data.expires) {
        (None, None) => None,
        (Some(key), Some(expires)) => {
            if expires <= now_secs() {
                refuse_nexus(ctx, message_type, context, &NexusError::LinkExpired);
                return Ok(());
            }
            Some(NxmAuth {
                key: key.to_string(),
                expires,
            })
        }
        _ => {
            refuse_nexus(ctx, message_type, context, &NexusError::InvalidLink);
            return Ok(());
        }
    };
    let Some(target) =
        target_or_refusal(ctx, message_type, &data.target_id, context.clone()).await?
    else {
        return Ok(());
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

    let request_id = deploy::backup::new_op_id();
    emit_progress(
        ctx.emitter,
        &request_id,
        &target.id,
        "resolving",
        0,
        "looking up the file on Nexus Mods",
    );
    let files = match services.nexus.mod_files(&[data.mod_id]).await {
        Ok(mut files) => files.remove(&data.mod_id).unwrap_or_default(),
        Err(error) => {
            refuse_nexus(ctx, message_type, context, &error);
            return Ok(());
        }
    };
    let Some(file) = files.into_iter().find(|file| file.file_id == data.file_id) else {
        refuse_nexus(ctx, message_type, context, &NexusError::NotFound);
        return Ok(());
    };
    if file.size_in_bytes.is_some_and(|size| size > MAX_DOWNLOAD) {
        refuse_nexus(
            ctx,
            message_type,
            context,
            &NexusError::TooLarge(MAX_DOWNLOAD),
        );
        return Ok(());
    }

    // Spares the user's Nexus request quota and a full archive download when
    // the library already holds the version this file would install. Only a
    // version the install path would itself accept can be looked up here; a
    // blank or over-long version installs under an id derived from the
    // manifest instead, which this pre-check cannot predict.
    let mod_id = ps_core::mods::mod_id(&ps_core::mods::ModIdInput::Nexus {
        nexus_mod_id: data.mod_id,
        variant: None,
    });
    if let Some(version) = install::valid_nexus_version(Some(&file.version)) {
        let version_id = library::version_id(&mod_id, version);
        match ps_db::mod_library::get_version(&*ctx.app.driver, &version_id).await {
            Ok(Some(_)) => {
                emit_refusal(
                    ctx,
                    message_type,
                    context,
                    "already_installed",
                    format!("{version_id} is already in the library"),
                    json!({ "mod_id": mod_id, "version_id": version_id }),
                );
                return Ok(());
            }
            Ok(None) => {}
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
        }
    }

    let file_name = ps_core::nexus::archive_file_name(&file.uri, file.file_id);
    let dir = download_dir(uploads.downloads_dir(), data.mod_id, data.file_id);
    let archive = dir.join(&file_name);
    let cached = file.size_in_bytes.is_some_and(|size| {
        std::fs::metadata(&archive)
            .is_ok_and(|metadata| metadata.is_file() && metadata.len() == size)
    });

    if !cached {
        let Some(key) = required_key(services, ctx, message_type, &context).await else {
            return Ok(());
        };
        let url = match services
            .nexus
            .download_link(&key, data.mod_id, data.file_id, nxm.as_ref())
            .await
        {
            Ok(url) => url,
            Err(error) => {
                refuse_nexus(ctx, message_type, context, &error);
                return Ok(());
            }
        };
        if let Err(error) = tokio::fs::create_dir_all(&dir).await {
            refuse_nexus(
                ctx,
                message_type,
                context,
                &NexusError::Io(error.to_string()),
            );
            return Ok(());
        }
        emit_progress(
            ctx.emitter,
            &request_id,
            &target.id,
            "downloading",
            0,
            "downloading from Nexus Mods",
        );
        let emitter = ctx.emitter;
        let progress_request_id = request_id.clone();
        let progress_target_id = target.id.clone();
        let last_pct = std::sync::Mutex::new(None::<u8>);
        let progress = move |got: u64, total: Option<u64>| {
            let pct = total
                .map(|total| (((got * 100) / total.max(1)).min(99)) as u8)
                .unwrap_or(0);
            let mut last_pct = last_pct
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
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
                "downloading from Nexus Mods",
            );
        };
        if let Err(error) = services.nexus.download(&url, &archive, &progress).await {
            let _ = std::fs::remove_dir_all(&dir);
            refuse_nexus(ctx, message_type, context, &error);
            return Ok(());
        }
    }

    emit_progress(
        ctx.emitter,
        &request_id,
        &target.id,
        "installing",
        100,
        "installing the download",
    );
    let file_id = data.file_id.to_string();
    let outcome = install_archive_and_reply(
        ctx,
        library,
        ArchiveInstall {
            message_type,
            context: json!({
                "target_id": target.id,
                "nexus_mod_id": data.mod_id,
                "file_id": data.file_id,
                "version": file.version,
                "file_name": file_name,
            }),
            target: &target,
            spec: &spec,
            archive: &archive,
            provenance: install::Provenance::Nexus {
                nexus_mod_id: data.mod_id,
                file_id: Some(&file_id),
                variant: None,
                version: Some(&file.version),
            },
            custom_name: None,
            accept_defaults: data.accept_defaults,
            enable: data.enable,
        },
    )
    .await;
    if outcome == ArchiveInstallOutcome::Installed {
        let _ = std::fs::remove_dir_all(&dir);
    }
    Ok(())
}

pub async fn handle_nexus_link_subscribe(
    services: &ServerServices,
    _data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let message_type = MessageType::NexusLinkSubscribe;
    let context = json!({});
    if !nexus_allowed(ctx, message_type, &context) {
        return Ok(());
    }
    ctx.emitter.emit(message_type, &json!({ "active": true }));
    services.nexus_links.subscribe(ctx.emitter);
    Ok(())
}

pub async fn handle_nexus_handler_status(
    services: &ServerServices,
    _data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let message_type = MessageType::NexusHandlerStatus;
    let context = json!({});
    if !nexus_allowed(ctx, message_type, &context) {
        return Ok(());
    }
    let registry = std::sync::Arc::clone(&services.protocol_registry);
    match tokio::task::spawn_blocking(move || registry.status()).await {
        Ok(Ok(status)) => ctx.emitter.emit(message_type, &status),
        Ok(Err(error)) => emit_refusal(
            ctx,
            message_type,
            context,
            error.code(),
            error.to_string(),
            json!({}),
        ),
        Err(error) => emit_refusal(
            ctx,
            message_type,
            context,
            "register_failed",
            error.to_string(),
            json!({}),
        ),
    }
    Ok(())
}

pub async fn handle_mod_update_check(
    services: &ServerServices,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let message_type = MessageType::ModUpdateCheck;
    let target_id = data
        .get("target_id")
        .and_then(Value::as_str)
        .map(str::to_string);
    let context = json!({ "target_id": target_id });
    if !nexus_allowed(ctx, message_type, &context) {
        return Ok(());
    }
    let app = std::sync::Arc::clone(ctx.app);
    let db = &*app.driver;

    let in_profile: Option<std::collections::BTreeSet<String>> = match &target_id {
        None => None,
        Some(target_id) => {
            let Some(target) =
                target_or_refusal(ctx, message_type, target_id, context.clone()).await?
            else {
                return Ok(());
            };
            let profile = match ps_db::mod_profiles::active_for_target(db, &target.id).await {
                Ok(Some(profile)) => profile,
                Ok(None) => {
                    emit_refusal(
                        ctx,
                        message_type,
                        context,
                        "no_active_profile",
                        format!("{} has no active profile", target.id),
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
            match ps_db::mod_profiles::mods_of(db, &profile.id).await {
                Ok(entries) => Some(entries.into_iter().map(|entry| entry.mod_id).collect()),
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
            }
        }
    };

    let rows = match ps_db::mod_library::list_mods(db).await {
        Ok(rows) => rows,
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
    let mut linked: Vec<(ps_db::mod_library::ModRow, u32)> = rows
        .into_iter()
        .filter_map(|row| {
            let nexus_id = row
                .nexus_mod_id
                .and_then(|id| u32::try_from(id).ok())
                .filter(|id| *id > 0)?;
            Some((row, nexus_id))
        })
        .filter(|(row, _)| in_profile.as_ref().is_none_or(|ids| ids.contains(&row.id)))
        .collect();
    linked.sort_by(|a, b| a.0.id.cmp(&b.0.id));
    let truncated = linked.len() > ps_core::nexus::MAX_UPDATE_BATCH;
    linked.truncate(ps_core::nexus::MAX_UPDATE_BATCH);

    let nexus_ids: Vec<u32> = linked.iter().map(|(_, nexus_id)| *nexus_id).collect();
    let files = if nexus_ids.is_empty() {
        std::collections::BTreeMap::new()
    } else {
        match services.nexus.mod_files(&nexus_ids).await {
            Ok(files) => files,
            Err(error) => {
                refuse_nexus(ctx, message_type, context, &error);
                return Ok(());
            }
        }
    };

    let mut updates = Vec::with_capacity(linked.len());
    for (row, nexus_id) in &linked {
        let installed = match ps_db::mod_library::current_version(db, &row.id).await {
            Ok(version) => version.map(|version| version.version),
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
        let latest = files
            .get(nexus_id)
            .and_then(|files| ps_core::nexus::latest_file(files))
            .cloned();
        let state = match (&installed, &latest) {
            (Some(installed), Some(latest)) => ps_core::nexus::update_state(
                installed,
                &latest.version,
                row.ignored_version.as_deref(),
            ),
            _ => ps_core::nexus::UpdateState::UpToDate,
        };
        updates.push(json!({
            "mod_id": row.id,
            "nexus_mod_id": nexus_id,
            "installed_version": installed,
            "latest": latest,
            "state": state,
            "ignored_version": row.ignored_version,
        }));
    }
    ctx.emitter.emit(
        message_type,
        &json!({
            "target_id": target_id,
            "checked": updates.len(),
            "truncated": truncated,
            "updates": updates,
        }),
    );
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct ModUpdateIgnoreData {
    pub mod_id: String,
    #[serde(default)]
    pub version: Option<String>,
}

pub async fn handle_mod_update_ignore(
    _services: &ServerServices,
    data: ModUpdateIgnoreData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let message_type = MessageType::ModUpdateIgnore;
    let context = json!({ "mod_id": data.mod_id });
    if !nexus_allowed(ctx, message_type, &context) {
        return Ok(());
    }
    let version = data
        .version
        .as_deref()
        .map(str::trim)
        .filter(|version| !version.is_empty());
    let app = std::sync::Arc::clone(ctx.app);
    match ps_db::mod_library::set_ignored_version(&*app.driver, &data.mod_id, version).await {
        Ok(true) => ctx.emitter.emit(
            message_type,
            &json!({ "mod_id": data.mod_id, "ignored_version": version }),
        ),
        Ok(false) => emit_refusal(
            ctx,
            message_type,
            context,
            "mod_not_found",
            format!("no library mod {}", data.mod_id),
            json!({}),
        ),
        Err(error) => emit_refusal(
            ctx,
            message_type,
            context,
            "db",
            error.to_string(),
            json!({}),
        ),
    }
    Ok(())
}

enum Registration {
    Done(HandlerStatus),
    Foreign(HandlerStatus),
}

pub async fn handle_nexus_handler_register(
    services: &ServerServices,
    data: Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let message_type = MessageType::NexusHandlerRegister;
    let context = json!({});
    if !nexus_allowed(ctx, message_type, &context) {
        return Ok(());
    }
    let force = data.get("force").and_then(Value::as_bool).unwrap_or(false);
    let registry = std::sync::Arc::clone(&services.protocol_registry);
    let outcome = tokio::task::spawn_blocking(move || -> Result<Registration, RegistryError> {
        let status = registry.status()?;
        if status.foreign && !force {
            return Ok(Registration::Foreign(status));
        }
        registry.register().map(Registration::Done)
    })
    .await;
    match outcome {
        Ok(Ok(Registration::Done(status))) => ctx.emitter.emit(message_type, &status),
        Ok(Ok(Registration::Foreign(status))) => emit_refusal(
            ctx,
            message_type,
            context,
            "foreign_handler",
            "another program already opens nxm:// links".to_string(),
            json!({ "current": status.current }),
        ),
        Ok(Err(error)) => emit_refusal(
            ctx,
            message_type,
            context,
            error.code(),
            error.to_string(),
            json!({}),
        ),
        Err(error) => emit_refusal(
            ctx,
            message_type,
            context,
            "register_failed",
            error.to_string(),
            json!({}),
        ),
    }
    Ok(())
}
