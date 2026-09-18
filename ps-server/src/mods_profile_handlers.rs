//! Wire handlers that create, rename, delete, activate, reorder and configure a
//! target's profiles, link worlds to them, and launch the game with one.
use serde_json::{json, Value};

use crate::dispatcher::HandlerCtx;
use crate::emitter::Emitter;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;
use crate::mods_handlers::{
    apply_after_selection, emit_refusal, profile_json, run_apply, target_or_refusal, SelectionApply,
};
use crate::services::mods::launch::{self, LaunchError};
use crate::services::mods::{layout, running, LibraryPaths};
use crate::services::ServerServices;

pub(crate) const MAX_NAME_CHARS: usize = 64;

#[derive(Debug, serde::Deserialize)]
pub struct ProfileCreateData {
    pub target_id: String,
    pub name: String,
    #[serde(default)]
    pub copy_from: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ProfileRenameData {
    pub target_id: String,
    pub profile_id: String,
    pub name: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct ProfileRefData {
    pub target_id: String,
    pub profile_id: String,
}

/// Resolves `profile_id`, or the target's active profile when it is `None`. A
/// profile of another target is refused as not found.
pub(crate) async fn profile_or_refusal(
    ctx: &mut HandlerCtx<'_>,
    message_type: MessageType,
    context: Value,
    target_id: &str,
    profile_id: Option<&str>,
) -> Option<ps_db::mod_profiles::ProfileRow> {
    let db = &*ctx.app.driver;
    let found = match profile_id {
        Some(id) => ps_db::mod_profiles::get(db, id).await,
        None => ps_db::mod_profiles::active_for_target(db, target_id).await,
    };
    match found {
        Ok(Some(profile)) if profile.target_id == target_id => Some(profile),
        Ok(_) if profile_id.is_none() => {
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
        Ok(_) => {
            let id = profile_id.unwrap_or_default();
            emit_refusal(
                ctx,
                message_type,
                context,
                "profile_not_found",
                format!("no profile {id} on target {target_id}"),
                json!({ "profile_id": id }),
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

/// `Err` carries the refusal code and the conflicting profile id, if any.
pub(crate) async fn checked_name(
    db: &dyn ps_db::DbDriver,
    target_id: &str,
    name: &str,
    renaming: Option<&str>,
) -> Result<Result<String, (&'static str, Option<String>)>, ps_db::DbError> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_NAME_CHARS {
        return Ok(Err(("invalid_name", None)));
    }
    let holder = ps_db::mod_profiles::for_target(db, target_id)
        .await?
        .into_iter()
        .find(|p| {
            Some(p.id.as_str()) != renaming && p.name.to_lowercase() == trimmed.to_lowercase()
        });
    Ok(match holder {
        Some(holder) => Err(("name_taken", Some(holder.id))),
        None => Ok(trimmed.to_string()),
    })
}

pub(crate) async fn free_profile_id(
    db: &dyn ps_db::DbDriver,
    target_id: &str,
    name: &str,
) -> Result<String, ps_db::DbError> {
    let hyphenated = ps_core::mods::slugify(name).replace('_', "-");
    let slug = match hyphenated.trim_matches('-') {
        "" => "profile".to_string(),
        slug => slug.to_string(),
    };
    let mut candidate = format!("{target_id}/{slug}");
    let mut n = 2;
    while ps_db::mod_profiles::get(db, &candidate).await?.is_some() {
        candidate = format!("{target_id}/{slug}-{n}");
        n += 1;
    }
    Ok(candidate)
}

/// Runs `checked_name`, emitting its refusal and returning `None` when the name
/// cannot be used.
async fn name_or_refusal(
    ctx: &mut HandlerCtx<'_>,
    message_type: MessageType,
    context: Value,
    target_id: &str,
    name: &str,
    renaming: Option<&str>,
) -> Option<String> {
    let (code, message, detail) =
        match checked_name(&*ctx.app.driver, target_id, name, renaming).await {
            Ok(Ok(name)) => return Some(name),
            Ok(Err(("name_taken", holder))) => (
                "name_taken",
                format!("another profile of {target_id} is named {}", name.trim()),
                json!({ "profile_id": holder }),
            ),
            Ok(Err((code, _))) => (
                code,
                format!("a profile name is 1 to {MAX_NAME_CHARS} characters"),
                json!({}),
            ),
            Err(error) => ("db", error.to_string(), json!({})),
        };
    emit_refusal(ctx, message_type, context, code, message, detail);
    None
}

pub async fn handle_profile_create(
    data: ProfileCreateData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let message_type = MessageType::ProfileCreate;
    let context = json!({ "target_id": data.target_id, "name": data.name });
    let Some(target) =
        target_or_refusal(ctx, message_type, &data.target_id, context.clone()).await?
    else {
        return Ok(());
    };
    let Some(name) = name_or_refusal(
        ctx,
        message_type,
        context.clone(),
        &target.id,
        &data.name,
        None,
    )
    .await
    else {
        return Ok(());
    };
    let source = match data.copy_from.as_deref() {
        Some(id) => {
            match profile_or_refusal(ctx, message_type, context.clone(), &target.id, Some(id)).await
            {
                Some(source) => Some(source),
                None => return Ok(()),
            }
        }
        None => None,
    };

    let db = &*ctx.app.driver;
    let created = async {
        let id = free_profile_id(db, &target.id, &name).await?;
        let profile = ps_db::mod_profiles::create(
            db,
            &ps_db::mod_profiles::NewProfile {
                id,
                target_id: target.id.clone(),
                name,
                is_default: false,
            },
        )
        .await?;
        if let Some(source) = &source {
            for entry in ps_db::mod_profiles::mods_of(db, &source.id).await? {
                ps_db::mod_profiles::set_mod(
                    db,
                    &ps_db::mod_profiles::ProfileModRow {
                        profile_id: profile.id.clone(),
                        ..entry
                    },
                )
                .await?;
            }
        }
        profile_json(db, &profile).await
    }
    .await;
    match created {
        Ok(profile) => ctx.emitter.emit(
            message_type,
            &json!({ "target_id": target.id, "profile": profile }),
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

pub async fn handle_profile_rename(
    data: ProfileRenameData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let message_type = MessageType::ProfileRename;
    let context = json!({
        "target_id": data.target_id,
        "profile_id": data.profile_id,
        "name": data.name,
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
    let Some(name) = name_or_refusal(
        ctx,
        message_type,
        context.clone(),
        &target.id,
        &data.name,
        Some(&profile.id),
    )
    .await
    else {
        return Ok(());
    };

    let db = &*ctx.app.driver;
    let renamed = async {
        ps_db::mod_profiles::rename(db, &profile.id, &name).await?;
        let profile = ps_db::mod_profiles::get(db, &profile.id)
            .await?
            .ok_or_else(|| {
                ps_db::DbError::Other(format!("profile {} vanished after rename", profile.id))
            })?;
        profile_json(db, &profile).await
    }
    .await;
    match renamed {
        Ok(profile) => ctx.emitter.emit(
            message_type,
            &json!({ "target_id": target.id, "profile": profile }),
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

pub async fn handle_profile_activate(
    services: &ServerServices,
    library: &LibraryPaths,
    data: ProfileRefData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let message_type = MessageType::ProfileActivate;
    let context = json!({ "target_id": data.target_id, "profile_id": data.profile_id });
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
    let db = &*ctx.app.driver;
    if let Err(error) = ps_db::mod_profiles::activate(db, &target.id, &profile.id).await {
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
    let selection = apply_after_selection(services, library, db, ctx.emitter, &target).await;
    ctx.emitter.emit(
        message_type,
        &json!({
            "target_id": target.id,
            "profile_id": profile.id,
            "request_id": selection.request_id,
            "pending": selection.pending,
            "apply": selection.apply,
        }),
    );
    Ok(())
}

pub async fn handle_profile_delete(
    services: &ServerServices,
    library: &LibraryPaths,
    data: ProfileRefData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let message_type = MessageType::ProfileDelete;
    let context = json!({ "target_id": data.target_id, "profile_id": data.profile_id });
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
    if profile.is_default {
        emit_refusal(
            ctx,
            message_type,
            context,
            "default_profile",
            format!(
                "{} is the default profile and cannot be deleted",
                profile.id
            ),
            json!({}),
        );
        return Ok(());
    }
    let db = &*ctx.app.driver;
    let active = match ps_db::mod_profiles::delete(db, &profile.id).await {
        Ok(active) => active,
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
    let selection = if profile.is_active {
        apply_after_selection(services, library, db, ctx.emitter, &target).await
    } else {
        SelectionApply::none()
    };
    ctx.emitter.emit(
        message_type,
        &json!({
            "target_id": target.id,
            "profile_id": profile.id,
            "deleted": true,
            "active_profile_id": active.id,
            "request_id": selection.request_id,
            "pending": selection.pending,
            "apply": selection.apply,
        }),
    );
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct ProfileReorderData {
    pub target_id: String,
    #[serde(default)]
    pub profile_id: Option<String>,
    pub kind: String,
    pub ordered_mod_ids: Vec<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ProfileSetOptionsData {
    pub target_id: String,
    #[serde(default)]
    pub profile_id: Option<String>,
    #[serde(default)]
    pub ue4ss_control_mode: Option<String>,
    #[serde(default)]
    pub force_order_ue4ss: Option<bool>,
    #[serde(default)]
    pub force_order_palschema: Option<bool>,
}

/// `None` when `kind` has no user-set order.
fn kind_matches(kind: &str, mod_type: &str) -> Option<bool> {
    match kind {
        "ue4ss" => Some(matches!(mod_type, "ue4ss" | "hybrid")),
        "palschema" => Some(mod_type == "palschema"),
        _ => None,
    }
}

async fn entries_of_kind(
    db: &dyn ps_db::DbDriver,
    profile_id: &str,
    kind: &str,
) -> Result<Vec<ps_db::mod_profiles::ProfileModRow>, ps_db::DbError> {
    let mut entries = Vec::new();
    for entry in ps_db::mod_profiles::mods_of(db, profile_id).await? {
        let mod_type = ps_db::mod_library::get_mod(db, &entry.mod_id)
            .await?
            .map(|row| row.mod_type)
            .unwrap_or_default();
        if kind_matches(kind, &mod_type) == Some(true) {
            entries.push(entry);
        }
    }
    Ok(entries)
}

/// The `load_order` values a reorder hands out, smallest first. Reusing the
/// entries' own values leaves every other kind's position untouched; tied values
/// (adopted mods can share one) are spread upward from the smallest first.
fn order_slots(entries: &[ps_db::mod_profiles::ProfileModRow]) -> Vec<i64> {
    let mut slots: Vec<i64> = entries.iter().map(|entry| entry.load_order).collect();
    slots.sort_unstable();
    if slots.windows(2).any(|pair| pair[0] == pair[1]) {
        let min = slots.first().copied().unwrap_or(0);
        slots = (0..slots.len() as i64).map(|offset| min + offset).collect();
    }
    slots
}

pub async fn handle_profile_reorder(
    services: &ServerServices,
    library: &LibraryPaths,
    data: ProfileReorderData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let message_type = MessageType::ProfileReorder;
    let mut context = json!({
        "target_id": data.target_id,
        "profile_id": data.profile_id,
        "kind": data.kind,
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
        data.profile_id.as_deref(),
    )
    .await
    else {
        return Ok(());
    };
    context["profile_id"] = json!(profile.id);
    if kind_matches(&data.kind, "").is_none() {
        emit_refusal(
            ctx,
            message_type,
            context,
            "invalid_kind",
            format!("{} mods have no order to set", data.kind),
            json!({ "kind": data.kind }),
        );
        return Ok(());
    }
    let db = &*ctx.app.driver;
    let entries = match entries_of_kind(db, &profile.id, &data.kind).await {
        Ok(entries) => entries,
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
    let mut requested: Vec<&str> = data.ordered_mod_ids.iter().map(String::as_str).collect();
    requested.sort_unstable();
    let mut expected: Vec<&str> = entries.iter().map(|entry| entry.mod_id.as_str()).collect();
    expected.sort_unstable();
    if requested != expected {
        emit_refusal(
            ctx,
            message_type,
            context,
            "invalid_order",
            format!(
                "the order must list each of {}'s {} mods exactly once",
                profile.id, data.kind
            ),
            json!({
                "expected": entries.iter().map(|entry| &entry.mod_id).collect::<Vec<_>>(),
            }),
        );
        return Ok(());
    }

    let reordered = async {
        for (mod_id, load_order) in data.ordered_mod_ids.iter().zip(order_slots(&entries)) {
            if let Some(entry) = entries.iter().find(|entry| &entry.mod_id == mod_id) {
                ps_db::mod_profiles::set_mod(
                    db,
                    &ps_db::mod_profiles::ProfileModRow {
                        load_order,
                        ..entry.clone()
                    },
                )
                .await?;
            }
        }
        Ok::<(), ps_db::DbError>(())
    }
    .await;
    if let Err(error) = reordered {
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

    let selection = if profile.is_active {
        apply_after_selection(services, library, db, ctx.emitter, &target).await
    } else {
        SelectionApply::none()
    };
    ctx.emitter.emit(
        message_type,
        &json!({
            "target_id": target.id,
            "profile_id": profile.id,
            "kind": data.kind,
            "ordered_mod_ids": data.ordered_mod_ids,
            "request_id": selection.request_id,
            "pending": selection.pending,
            "apply": selection.apply,
        }),
    );
    Ok(())
}

pub async fn handle_profile_set_options(
    services: &ServerServices,
    library: &LibraryPaths,
    data: ProfileSetOptionsData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let message_type = MessageType::ProfileSetOptions;
    let mut context = json!({ "target_id": data.target_id, "profile_id": data.profile_id });
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
        data.profile_id.as_deref(),
    )
    .await
    else {
        return Ok(());
    };
    context["profile_id"] = json!(profile.id);
    if let Some(mode) = data.ue4ss_control_mode.as_deref() {
        if !matches!(mode, "enabled_txt" | "mods_txt") {
            emit_refusal(
                ctx,
                message_type,
                context,
                "invalid_option",
                format!("{mode} is not a UE4SS control mode"),
                json!({ "ue4ss_control_mode": mode }),
            );
            return Ok(());
        }
    }

    let db = &*ctx.app.driver;
    let updated = async {
        ps_db::mod_profiles::set_options(
            db,
            &profile.id,
            data.ue4ss_control_mode
                .as_deref()
                .unwrap_or(&profile.ue4ss_control_mode),
            data.force_order_ue4ss.unwrap_or(profile.force_order_ue4ss),
            data.force_order_palschema
                .unwrap_or(profile.force_order_palschema),
        )
        .await?;
        let updated = ps_db::mod_profiles::get(db, &profile.id)
            .await?
            .ok_or_else(|| {
                ps_db::DbError::Other(format!(
                    "profile {} vanished after setting options",
                    profile.id
                ))
            })?;
        profile_json(db, &updated).await
    }
    .await;
    let updated = match updated {
        Ok(updated) => updated,
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

    let selection = if profile.is_active {
        apply_after_selection(services, library, db, ctx.emitter, &target).await
    } else {
        SelectionApply::none()
    };
    ctx.emitter.emit(
        message_type,
        &json!({
            "target_id": target.id,
            "profile": updated,
            "request_id": selection.request_id,
            "pending": selection.pending,
            "apply": selection.apply,
        }),
    );
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct WorldProfileSetData {
    pub world_key: String,
    pub world_name: String,
    pub profile_id: Option<String>,
}

pub async fn handle_world_profile_set(
    data: WorldProfileSetData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let message_type = MessageType::WorldProfileSet;
    let context = json!({
        "world_key": data.world_key,
        "world_name": data.world_name,
        "profile_id": data.profile_id,
    });
    let world_key = data.world_key.trim();
    let world_name = data.world_name.trim();
    if world_key.is_empty() || world_name.is_empty() {
        emit_refusal(
            ctx,
            message_type,
            context,
            "invalid_world",
            "world_key and world_name must not be empty".to_string(),
            json!({}),
        );
        return Ok(());
    }

    let db = &*ctx.app.driver;
    if let Some(profile_id) = data.profile_id.as_deref() {
        match ps_db::mod_profiles::get(db, profile_id).await {
            Ok(Some(_)) => {}
            Ok(None) => {
                emit_refusal(
                    ctx,
                    message_type,
                    context,
                    "profile_not_found",
                    format!("no profile {profile_id}"),
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
        }
        let linked = ps_db::mod_profiles::link_world(db, world_key, world_name, profile_id).await;
        if let Err(error) = linked {
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
    } else if let Err(error) = ps_db::mod_profiles::unlink_world(db, world_key).await {
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

    ctx.emitter.emit(
        message_type,
        &json!({
            "world_key": world_key,
            "world_name": world_name,
            "profile_id": data.profile_id,
            "linked": data.profile_id.is_some(),
        }),
    );
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct GameLaunchData {
    pub target_id: String,
    #[serde(default)]
    pub world_key: Option<String>,
}

type Refusal = (&'static str, String, Value);

fn db_refusal(error: ps_db::DbError) -> Refusal {
    ("db", error.to_string(), json!({}))
}

async fn linked_profile(
    db: &dyn ps_db::DbDriver,
    world_key: Option<&str>,
) -> Result<Option<ps_db::mod_profiles::ProfileRow>, ps_db::DbError> {
    let Some(world_key) = world_key else {
        return Ok(None);
    };
    match ps_db::mod_profiles::world_link(db, world_key).await? {
        Some(link) => ps_db::mod_profiles::get(db, &link.profile_id).await,
        None => Ok(None),
    }
}

pub async fn handle_game_launch(
    services: &ServerServices,
    library: &LibraryPaths,
    data: GameLaunchData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let message_type = MessageType::GameLaunch;
    let world_key = data.world_key.as_deref().map(str::trim);
    let context = json!({ "target_id": data.target_id, "world_key": world_key });
    let Some(target) =
        target_or_refusal(ctx, message_type, &data.target_id, context.clone()).await?
    else {
        return Ok(());
    };
    let launched = launch_game(
        services,
        library,
        &*ctx.app.driver,
        ctx.app.config.desktop_mode,
        ctx.emitter,
        &target,
        world_key,
    )
    .await;
    match launched {
        Ok(reply) => ctx.emitter.emit(message_type, &reply),
        Err((code, message, detail)) => {
            emit_refusal(ctx, message_type, context, code, message, detail)
        }
    }
    Ok(())
}

async fn launch_game(
    services: &ServerServices,
    library: &LibraryPaths,
    db: &dyn ps_db::DbDriver,
    desktop_mode: bool,
    emitter: &Emitter,
    target: &ps_db::mod_targets::ModTarget,
    world_key: Option<&str>,
) -> Result<Value, Refusal> {
    let not_a_client = || {
        (
            "not_supported_on_target",
            format!("{} is not a game install", target.id),
            json!({ "kind": target.kind }),
        )
    };
    if target.kind != "client" {
        return Err(not_a_client());
    }
    if !desktop_mode {
        return Err((
            "desktop_only",
            "the game can only be launched from the desktop app".to_string(),
            json!({}),
        ));
    }
    let linked = linked_profile(db, world_key).await.map_err(db_refusal)?;
    if let Some(profile) = linked.as_ref().filter(|p| p.target_id != target.id) {
        return Err((
            "world_profile_other_target",
            format!(
                "the world is linked to {}, a profile of {}",
                profile.id, profile.target_id
            ),
            json!({ "profile_id": profile.id, "profile_target_id": profile.target_id }),
        ));
    }
    if running::target_is_running(services, db, target)
        .await
        .map_err(db_refusal)?
    {
        return Err((
            "target_locked",
            "the game is already running".to_string(),
            json!({}),
        ));
    }

    let (profile_id, activated) = match linked {
        Some(profile) if !profile.is_active => {
            ps_db::mod_profiles::activate(db, &target.id, &profile.id)
                .await
                .map_err(db_refusal)?;
            (Some(profile.id), true)
        }
        Some(profile) => (Some(profile.id), false),
        None => (
            ps_db::mod_profiles::active_for_target(db, &target.id)
                .await
                .map_err(db_refusal)?
                .map(|profile| profile.id),
            false,
        ),
    };

    let apply = run_apply(services, library, db, emitter, target, &[], true).await;
    if let Some(error) = apply.get("error") {
        let message = error["message"]
            .as_str()
            .unwrap_or("the profile could not be applied")
            .to_string();
        return Err(("apply_failed", message, json!({ "apply": apply })));
    }

    let layout = layout::layout_for(target)
        .map_err(|error| ("layout_error", error.to_string(), json!({})))?;
    let command = launch::launch_command(target, &layout).map_err(|error| match &error {
        LaunchError::NotAClient => not_a_client(),
        LaunchError::UnsupportedPlatform(platform) => (
            "unsupported_platform",
            error.to_string(),
            json!({ "platform": platform }),
        ),
        LaunchError::Unavailable => ("launch_unavailable", error.to_string(), json!({})),
    })?;
    services.launcher.launch(&command).map_err(|error| {
        (
            "launch_failed",
            format!("the game could not be started: {error}"),
            json!({ "reason": error.to_string() }),
        )
    })?;

    Ok(json!({
        "target_id": target.id,
        "world_key": world_key,
        "profile_id": profile_id,
        "activated": activated,
        "apply": apply,
        "launched": true,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(load_orders: &[i64]) -> Vec<ps_db::mod_profiles::ProfileModRow> {
        load_orders
            .iter()
            .enumerate()
            .map(|(index, load_order)| ps_db::mod_profiles::ProfileModRow {
                profile_id: "client-steam/default".to_string(),
                mod_id: format!("mod-{index}"),
                mod_version_id: None,
                enabled: true,
                load_order: *load_order,
            })
            .collect()
    }

    #[test]
    fn distinct_load_orders_are_reused_smallest_first() {
        assert_eq!(order_slots(&entries(&[7, 2, 5])), [2, 5, 7]);
    }

    #[test]
    fn tied_load_orders_spread_up_from_the_smallest() {
        assert_eq!(order_slots(&entries(&[4, 4, 9])), [4, 5, 6]);
    }

    mod launch {
        use std::sync::Arc;

        use serde_json::Value;

        use super::super::*;
        use crate::servers_handlers::test_env::TestEnv;
        use crate::services::mods::launch::{LaunchCommand, RecordingLauncher, STEAM_RUN_URL};

        struct Fixture {
            env: TestEnv,
            services: Arc<ServerServices>,
            recorder: Arc<RecordingLauncher>,
            library: LibraryPaths,
        }

        impl Fixture {
            fn new(env: TestEnv, running: bool) -> Self {
                let recorder = Arc::new(RecordingLauncher::default());
                let mut services =
                    ServerServices::with_docker(env.docker.clone(), env._scratch.path());
                services.launcher = recorder.clone();
                services.running_override = Some(running);
                let library = LibraryPaths::new(env._scratch.path());
                Self {
                    env,
                    services: Arc::new(services),
                    recorder,
                    library,
                }
            }

            async fn client(&self, name: &str) -> String {
                let root = self.root(name);
                std::fs::create_dir_all(root.join("Pal/Binaries/Win64")).unwrap();
                std::fs::write(
                    root.join("Pal/Binaries/Win64/Palworld-Win64-Shipping.exe"),
                    b"stub",
                )
                .unwrap();
                std::fs::create_dir_all(root.join("Pal/Content/Paks")).unwrap();
                let detected = crate::services::mods::detect::describe(&root, "manual").unwrap();
                crate::services::mods::detect::register(&*self.env.app.driver, &detected, name)
                    .await
                    .unwrap()
                    .id
            }

            fn root(&self, name: &str) -> std::path::PathBuf {
                self.env._scratch.path().join(name).join("Palworld")
            }

            async fn profile(&self, target_id: &str, name: &str) -> String {
                ps_db::mod_profiles::create(
                    &*self.env.app.driver,
                    &ps_db::mod_profiles::NewProfile {
                        id: format!("{target_id}/{}", name.to_lowercase()),
                        target_id: target_id.to_string(),
                        name: name.to_string(),
                        is_default: false,
                    },
                )
                .await
                .unwrap()
                .id
            }

            async fn active(&self, target_id: &str) -> String {
                ps_db::mod_profiles::active_for_target(&*self.env.app.driver, target_id)
                    .await
                    .unwrap()
                    .unwrap()
                    .id
            }

            async fn launch(&mut self, target_id: &str, world_key: Option<&str>) -> Value {
                let data = GameLaunchData {
                    target_id: target_id.to_string(),
                    world_key: world_key.map(str::to_string),
                };
                handle_game_launch(&self.services, &self.library, data, &mut self.env.ctx())
                    .await
                    .unwrap();
                self.env
                    .drain()
                    .into_iter()
                    .rev()
                    .find(|frame| frame["type"] == "game_launch")
                    .expect("a game_launch reply")["data"]
                    .clone()
            }

            fn launched(&self) -> Vec<LaunchCommand> {
                self.recorder.launched.lock().unwrap().clone()
            }
        }

        #[tokio::test]
        async fn launching_applies_the_active_profile_then_opens_steam() {
            let mut fixture = Fixture::new(TestEnv::new_desktop().await, false);
            let target_id = fixture.client("Steam").await;

            let reply = fixture.launch(&target_id, None).await;

            assert!(reply["error"].is_null(), "{reply:?}");
            assert_eq!(reply["target_id"], target_id);
            assert!(reply["world_key"].is_null(), "{reply:?}");
            assert_eq!(reply["profile_id"], format!("{target_id}/default"));
            assert_eq!(reply["activated"], false);
            assert_eq!(reply["launched"], true);
            assert_eq!(reply["apply"]["target_id"], target_id);
            assert!(reply["apply"]["error"].is_null(), "{reply:?}");
            assert_eq!(
                fixture.launched(),
                [LaunchCommand::Open(STEAM_RUN_URL.to_string())]
            );
        }

        #[tokio::test]
        async fn a_linked_world_activates_its_profile_first() {
            let mut fixture = Fixture::new(TestEnv::new_desktop().await, false);
            let target_id = fixture.client("Steam").await;
            let hard = fixture.profile(&target_id, "Hard").await;
            let key = "C:/saves/w";
            ps_db::mod_profiles::link_world(&*fixture.env.app.driver, key, "W", &hard)
                .await
                .unwrap();

            let reply = fixture.launch(&target_id, Some(key)).await;

            assert!(reply["error"].is_null(), "{reply:?}");
            assert_eq!(reply["world_key"], key);
            assert_eq!(reply["activated"], true);
            assert_eq!(reply["profile_id"], hard);
            assert_eq!(fixture.active(&target_id).await, hard);
            assert_eq!(fixture.launched().len(), 1);
        }

        #[tokio::test]
        async fn a_world_key_with_surrounding_whitespace_finds_its_link() {
            let mut fixture = Fixture::new(TestEnv::new_desktop().await, false);
            let target_id = fixture.client("Steam").await;
            let hard = fixture.profile(&target_id, "Hard").await;
            let key = "C:/saves/w";
            ps_db::mod_profiles::link_world(&*fixture.env.app.driver, key, "W", &hard)
                .await
                .unwrap();

            let reply = fixture.launch(&target_id, Some(" C:/saves/w \t")).await;

            assert!(reply["error"].is_null(), "{reply:?}");
            assert_eq!(reply["world_key"], key);
            assert_eq!(reply["activated"], true);
            assert_eq!(reply["profile_id"], hard);
            assert_eq!(fixture.active(&target_id).await, hard);
        }

        #[tokio::test]
        async fn a_world_linked_to_another_target_is_refused() {
            let mut fixture = Fixture::new(TestEnv::new_desktop().await, false);
            let target_id = fixture.client("Steam").await;
            let other_id = fixture.client("Other").await;
            let other_profile = format!("{other_id}/default");
            ps_db::mod_profiles::link_world(&*fixture.env.app.driver, "W", "W", &other_profile)
                .await
                .unwrap();

            let reply = fixture.launch(&target_id, Some("W")).await;

            assert_eq!(
                reply["error"]["code"], "world_profile_other_target",
                "{reply:?}"
            );
            assert_eq!(reply["error"]["profile_id"], other_profile);
            assert_eq!(reply["error"]["profile_target_id"], other_id);
            assert_eq!(reply["target_id"], target_id);
            assert_eq!(reply["world_key"], "W");
            assert!(fixture.launched().is_empty());
            assert_eq!(
                fixture.active(&target_id).await,
                format!("{target_id}/default")
            );
        }

        #[tokio::test]
        async fn a_running_game_refuses_before_activating() {
            let mut fixture = Fixture::new(TestEnv::new_desktop().await, true);
            let target_id = fixture.client("Steam").await;
            let hard = fixture.profile(&target_id, "Hard").await;
            ps_db::mod_profiles::link_world(&*fixture.env.app.driver, "W", "W", &hard)
                .await
                .unwrap();

            let reply = fixture.launch(&target_id, Some("W")).await;

            assert_eq!(reply["error"]["code"], "target_locked", "{reply:?}");
            assert_eq!(
                fixture.active(&target_id).await,
                format!("{target_id}/default")
            );
            assert!(fixture.launched().is_empty());
        }

        #[tokio::test]
        async fn without_desktop_mode_launch_is_refused() {
            let mut fixture = Fixture::new(TestEnv::new().await, false);
            let target_id = fixture.client("Steam").await;

            let reply = fixture.launch(&target_id, None).await;

            assert_eq!(reply["error"]["code"], "desktop_only", "{reply:?}");
            assert_eq!(reply["target_id"], target_id);
            assert!(fixture.launched().is_empty());
        }

        #[tokio::test]
        async fn a_launcher_failure_is_reported() {
            let mut fixture = Fixture::new(TestEnv::new_desktop().await, false);
            let target_id = fixture.client("Steam").await;
            fixture
                .recorder
                .fail
                .store(true, std::sync::atomic::Ordering::SeqCst);

            let reply = fixture.launch(&target_id, None).await;

            assert_eq!(reply["error"]["code"], "launch_failed", "{reply:?}");
            assert_eq!(reply["error"]["reason"], "refused by test");
        }
    }
}
