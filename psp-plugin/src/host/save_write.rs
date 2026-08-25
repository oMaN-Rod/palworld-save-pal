use std::ffi::{c_int, c_void};

use psp_core::domain::raw_path::{RawPath, RawScope};
use psp_core::domain::{containers, guild, map_object, pal, player};
use psp_core::dto::container::{ItemContainerDto, ItemContainerSlotDto};
use psp_core::error::CoreError;
use psp_core::progress::{null_progress, ProgressSink};
use psp_lua_sys::ffi::*;
use uuid::Uuid;

use super::api_def::{ApiFunction, ApiParam, ApiType};
use super::handle::{handle_kind_for, invalidated_handle_error};
use super::marshal::{arg_integer, arg_string, arg_uuid, check_args, push_str, table_to_json};
use super::services::append_log_line;
use super::{free_message, trampoline, with_context, HostError, HostFn, PushHostFn};
use crate::context::{ClearSlotsState, DeleteWhereKind, DeleteWhereState, LogLevel, RunContext};
use crate::host::handle::{push_handle, Handle, HandleKind};
use crate::host_fn;
use crate::manifest::Capability;

fn core_error(error: CoreError) -> HostError {
    HostError::new(error.to_string())
}

unsafe fn read_string_if(state: *mut lua_State, index: c_int) -> Option<String> {
    if lua_type(state, index) != LUA_TSTRING {
        return None;
    }
    super::marshal::read_string_at(state, index)
}

/// The condition `delete_player` refuses on, read without mutating it.
fn player_is_guild_admin(ctx: &RunContext<'_>, player_id: Uuid) -> bool {
    let Some(guild_id) = ctx.session.player_summaries.get(&player_id).and_then(|s| s.guild_id) else {
        return false;
    };
    if !ctx.session.loaded_guilds.contains(&guild_id) {
        return false;
    }
    ctx.session.guild_summaries.get(&guild_id).and_then(|g| g.admin_player_uid) == Some(player_id)
}

fn prune_player_summary(ctx: &mut RunContext<'_>, player_id: Uuid) {
    ctx.session.player_summaries.remove(&player_id);
    ctx.session.player_summary_order.retain(|id| *id != player_id);
}

fn prune_guild_summary(ctx: &mut RunContext<'_>, guild_id: Uuid) {
    ctx.session.guild_summaries.remove(&guild_id);
    ctx.session.guild_summary_order.retain(|id| *id != guild_id);
}

/// A guild's `player_count` is denormalized: deleting one member does not touch it.
fn decrement_guild_member_count(ctx: &mut RunContext<'_>, guild_id: Uuid) {
    if let Some(summary) = ctx.session.guild_summaries.get_mut(&guild_id) {
        summary.player_count = summary.player_count.saturating_sub(1);
    }
}

fn decrement_owner_pal_count(ctx: &mut RunContext<'_>, owner_uid: Option<Uuid>, guild_id: Option<Uuid>) {
    if let Some(owner_uid) = owner_uid {
        if let Some(summary) = ctx.session.player_summaries.get_mut(&owner_uid) {
            summary.pal_count = summary.pal_count.saturating_sub(1);
        }
    }
    if let Some(guild_id) = guild_id {
        if let Some(summary) = ctx.session.guild_summaries.get_mut(&guild_id) {
            summary.pal_count = summary.pal_count.saturating_sub(1);
        }
    }
}

fn log_skip(ctx: &mut RunContext<'_>, kind: DeleteWhereKind, id: Uuid, reason: &str) {
    append_log_line(ctx, LogLevel::Warn, format!("{} skipped {id}: {reason}", count_key(kind)));
}

fn base_guild_id(ctx: &RunContext<'_>, base_id: Uuid) -> Option<Uuid> {
    ctx.session
        .base_camp_map()
        .unwrap_or(&[])
        .iter()
        .find(|entry| psp_core::props::as_uuid(&entry.key) == Some(base_id))
        .and_then(guild::base_guild_and_container)
        .map(|(guild_id, _)| guild_id)
}

/// The bound id is never epoch-checked at call time, which is sound only because
/// no host method creates a container, player, guild, pal or base: a captured id
/// resolves to its own entity or to nothing, never to a different one.
unsafe fn push_bound(state: *mut lua_State, body: HostFn, payload: &str) {
    let dispatch = (trampoline as *const () as usize) as *mut c_void;
    let bodyptr = ((body as HostFn) as usize) as *mut c_void;
    let free = (free_message as *const () as usize) as *mut c_void;
    lua_pushlightuserdata(state, dispatch);
    lua_pushlightuserdata(state, bodyptr);
    lua_pushlightuserdata(state, free);
    push_str(state, payload);
    lua_pushcclosure(state, psp_host_trampoline, 4);
}

unsafe fn bound_uuid(state: *mut lua_State, name: &str) -> Result<Uuid, HostError> {
    let text = arg_string(state, lua_upvalueindex(4), name)?;
    Uuid::parse_str(&text).map_err(|_| HostError::new(format!("corrupt {name} binding")))
}

fn pal_routing(ctx: &mut RunContext<'_>, pal_id: Uuid) -> Result<Option<(Option<Uuid>, Option<(Uuid, Uuid)>)>, HostError> {
    super::save_read::ensure_pals_snapshot(ctx)?;
    let Some((snapshot, index)) = ctx.pals.as_ref() else {
        return Ok(None);
    };
    let Some(entry) = index.get(&pal_id) else {
        return Ok(None);
    };
    let Some(summary) = snapshot.get(entry.position) else {
        return Ok(None);
    };
    Ok(Some((summary.owner_uid, summary.guild_id.zip(summary.base_id))))
}

fn player_delete_body(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 0, "player.delete")?;
        let player_id = bound_uuid(state, "player id")?;
        let removed = with_context(state, |ctx| {
            super::dto_cache::flush(ctx)?;
            let fallback = null_progress();
            let progress: &ProgressSink = ctx.progress.unwrap_or(&fallback);
            player::get_player_details(ctx.session, ctx.game_data, player_id, progress)
                .map_err(core_error)?
                .ok_or_else(|| HostError::new(format!("player {player_id} not found")))?;

            if ctx.dry_run {
                let would_delete = !player_is_guild_admin(ctx, player_id);
                if would_delete {
                    ctx.bump("player.delete", 1);
                }
                return Ok(would_delete);
            }

            let deleted =
                player::delete_player(ctx.session, ctx.game_data, player_id, progress).map_err(core_error)?;
            if deleted {
                let guild_id = ctx.session.player_summaries.get(&player_id).and_then(|s| s.guild_id);
                prune_player_summary(ctx, player_id);
                if let Some(guild_id) = guild_id {
                    decrement_guild_member_count(ctx, guild_id);
                }
                ctx.note_mutation();
            }
            Ok(deleted)
        })?;
        lua_pushboolean(state, c_int::from(removed));
        Ok(1)
    }
}

pub(crate) unsafe fn push_player_delete(state: *mut lua_State, player_id: Uuid) {
    push_bound(state, player_delete_body, &player_id.to_string());
}

fn pal_delete_body(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 0, "pal.delete")?;
        let pal_id = bound_uuid(state, "pal id")?;
        with_context(state, |ctx| {
            super::dto_cache::flush(ctx)?;
            let Some((owner_uid, guild_base)) = pal_routing(ctx, pal_id)? else {
                return Err(HostError::new(format!("pal {pal_id} not found")));
            };
            let fallback = null_progress();
            let progress: &ProgressSink = ctx.progress.unwrap_or(&fallback);

            if ctx.dry_run {
                match (owner_uid, guild_base) {
                    (Some(owner), _) => {
                        player::get_player_details(ctx.session, ctx.game_data, owner, progress)
                            .map_err(core_error)?
                            .ok_or_else(|| HostError::new(format!("pal {pal_id}'s owner {owner} not found")))?;
                    }
                    (None, Some((guild_id, _))) => {
                        guild::get_guild_details(ctx.session, ctx.game_data, guild_id)
                            .map_err(core_error)?
                            .ok_or_else(|| HostError::new(format!("guild {guild_id} not found")))?;
                    }
                    (None, None) => {
                        return Err(HostError::new(format!("pal {pal_id} has neither an owner nor a guild base")))
                    }
                }
                ctx.bump("pal.delete", 1);
                return Ok(());
            }

            match (owner_uid, guild_base) {
                (Some(owner), _) => {
                    player::get_player_details(ctx.session, ctx.game_data, owner, progress)
                        .map_err(core_error)?
                        .ok_or_else(|| HostError::new(format!("pal {pal_id}'s owner {owner} not found")))?;
                    pal::delete_player_pals(ctx.session, owner, &[pal_id]).map_err(core_error)?;
                }
                (None, Some((guild_id, base_id))) => {
                    guild::get_guild_details(ctx.session, ctx.game_data, guild_id)
                        .map_err(core_error)?
                        .ok_or_else(|| HostError::new(format!("guild {guild_id} not found")))?;
                    pal::delete_guild_pals(ctx.session, guild_id, base_id, &[pal_id]).map_err(core_error)?;
                }
                (None, None) => {
                    return Err(HostError::new(format!("pal {pal_id} has neither an owner nor a guild base")))
                }
            }
            decrement_owner_pal_count(ctx, owner_uid, guild_base.map(|(guild_id, _)| guild_id));
            ctx.note_mutation();
            Ok(())
        })?;
        lua_pushboolean(state, 1);
        Ok(1)
    }
}

pub(crate) unsafe fn push_pal_delete(state: *mut lua_State, pal_id: Uuid) {
    push_bound(state, pal_delete_body, &pal_id.to_string());
}

fn guild_delete_body(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 0, "guild.delete")?;
        let guild_id = bound_uuid(state, "guild id")?;
        with_context(state, |ctx| {
            super::dto_cache::flush(ctx)?;
            let details = guild::get_guild_details(ctx.session, ctx.game_data, guild_id)
                .map_err(core_error)?
                .ok_or_else(|| HostError::new(format!("guild {guild_id} not found")))?;

            if ctx.dry_run {
                ctx.bump("guild.delete", 1);
                return Ok(());
            }

            // Captured before the delete call: `delete_guild_and_players` also
            // deletes each loaded member, and afterward they are gone from here.
            let deleted_players: Vec<Uuid> =
                details.players.iter().copied().filter(|id| ctx.session.loaded_players.contains_key(id)).collect();

            let fallback = null_progress();
            let progress: &ProgressSink = ctx.progress.unwrap_or(&fallback);
            guild::delete_guild_and_players(ctx.session, ctx.game_data, guild_id, progress)
                .map_err(core_error)?;
            prune_guild_summary(ctx, guild_id);
            for player_id in deleted_players {
                prune_player_summary(ctx, player_id);
            }
            ctx.note_mutation();
            Ok(())
        })?;
        lua_pushboolean(state, 1);
        Ok(1)
    }
}

pub(crate) unsafe fn push_guild_delete(state: *mut lua_State, guild_id: Uuid) {
    push_bound(state, guild_delete_body, &guild_id.to_string());
}

/// `delete_base` deletes the base's worker pals too, so the guild's `pal_count`
/// must be read from the base before the delete call removes it.
fn base_delete_body(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 0, "base.delete")?;
        let base_id = bound_uuid(state, "base id")?;
        with_context(state, |ctx| {
            super::dto_cache::flush(ctx)?;
            let Some(guild_id) = base_guild_id(ctx, base_id) else {
                return Err(HostError::new(format!("base {base_id} not found")));
            };
            if ctx.dry_run {
                ctx.bump("base.delete", 1);
                return Ok(());
            }
            let details = guild::get_guild_details(ctx.session, ctx.game_data, guild_id)
                .map_err(core_error)?
                .ok_or_else(|| HostError::new(format!("guild {guild_id} not found")))?;
            let base_pal_count = details
                .bases
                .as_ref()
                .and_then(|bases| bases.get(&base_id))
                .map(|base| base.pals.len())
                .unwrap_or(0) as i64;

            guild::delete_base(ctx.session, ctx.game_data, base_id).map_err(core_error)?;
            if let Some(summary) = ctx.session.guild_summaries.get_mut(&guild_id) {
                summary.base_count = summary.base_count.saturating_sub(1);
                summary.pal_count = summary.pal_count.saturating_sub(base_pal_count);
            }
            ctx.note_mutation();
            Ok(())
        })?;
        lua_pushboolean(state, 1);
        Ok(1)
    }
}

pub(crate) unsafe fn push_base_delete(state: *mut lua_State, base_id: Uuid) {
    push_bound(state, base_delete_body, &base_id.to_string());
}

/// Structural, not an in-place empty: `apply_item_container_dto` removes the raw
/// slot entry, shifting every later slot, so this calls `note_mutation`.
fn slot_clear_body(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 0, "slot.clear")?;
        let payload = arg_string(state, lua_upvalueindex(4), "slot binding")?;
        let (container_text, slot_text) = payload
            .rsplit_once(':')
            .ok_or_else(|| HostError::new("corrupt slot.clear binding"))?;
        let container_id = Uuid::parse_str(container_text)
            .map_err(|_| HostError::new("corrupt slot.clear binding"))?;
        let slot_index: i32 = slot_text
            .parse()
            .map_err(|_| HostError::new("corrupt slot.clear binding"))?;

        with_context(state, |ctx| {
            if ctx.dry_run {
                ctx.bump("slot.clear", 1);
                super::dto_cache::forget_pending_slot(ctx, container_id, slot_index);
                return Ok(());
            }
            super::dto_cache::flush(ctx)?;
            let clear = ItemContainerDto {
                id: container_id,
                r#type: String::new(),
                slots: vec![ItemContainerSlotDto {
                    dynamic_item: None,
                    slot_index,
                    count: 0,
                    static_id: Some("None".to_string()),
                    local_id: None,
                }],
                key: None,
                slot_num: 0,
            };
            containers::apply_item_container_dto(ctx.session, container_id, &clear, None).map_err(core_error)?;
            ctx.note_mutation();
            Ok(())
        })?;
        Ok(0)
    }
}

pub(crate) unsafe fn push_slot_clear(state: *mut lua_State, container_id: Uuid, slot_index: i32) {
    push_bound(state, slot_clear_body, &format!("{container_id}:{slot_index}"));
}

/// `set_container_slot_count` writes on its resize branch, so it cannot predict.
fn would_destroy_occupied_slot(
    ctx: &mut RunContext<'_>,
    container_id: Uuid,
    slot_count: i32,
) -> Result<bool, HostError> {
    let dto = super::save_read::read_container(ctx, container_id)
        .ok_or_else(|| HostError::new(format!("container {container_id} not found")))?;
    Ok(dto.slots.iter().any(|slot| {
        slot.slot_index >= slot_count && !matches!(slot.static_id.as_deref(), Some("") | Some("None") | None)
    }))
}

fn container_set_slot_count_body(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 1, "container.set_slot_count")?;
        let container_id = bound_uuid(state, "container id")?;
        let requested = arg_integer(state, 1, "slot count")?;
        let slot_count: i32 = requested
            .try_into()
            .map_err(|_| HostError::new(format!("slot count out of range: {requested}")))?;

        let resized = with_context(state, |ctx| {
            if ctx.dry_run {
                let would_destroy = would_destroy_occupied_slot(ctx, container_id, slot_count)?;
                ctx.bump("container.set_slot_count", 1);
                return Ok(!would_destroy);
            }
            super::dto_cache::flush(ctx)?;
            let outcome = containers::set_container_slot_count(ctx.session, container_id, slot_count)
                .map_err(core_error)?;
            let resized = matches!(outcome, containers::SlotCountOutcome::Resized { .. });
            if resized {
                ctx.note_mutation();
            }
            Ok(resized)
        })?;
        lua_pushboolean(state, c_int::from(resized));
        Ok(1)
    }
}

pub(crate) unsafe fn push_container_set_slot_count(state: *mut lua_State, container_id: Uuid) {
    push_bound(state, container_set_slot_count_body, &container_id.to_string());
}

/// Three phases: snapshot, select by calling the predicate with no `&mut
/// RunContext` held across the `lua_pcall`, then apply. Clears `ctx.clear_slots`
/// before any error leaves, so a later call never sees a stale "already running".
fn clear_slots_where_body(state: *mut lua_State) -> Result<c_int, HostError> {
    let result = clear_slots_where_run(state);
    if result.is_err() {
        unsafe {
            let _ = with_context(state, |ctx| {
                ctx.clear_slots = None;
                Ok(())
            });
        }
    }
    result
}

fn clear_slots_where_run(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        if lua_gettop(state) < 1 || !lua_isfunction(state, 1) {
            let actual = super::marshal::type_name(state, 1);
            return Err(HostError::new(format!(
                "clear_slots_where expects a function for its argument, got {actual}"
            )));
        }
        let predicate_index: c_int = 1;

        let epoch = with_context(state, |ctx| {
            if ctx.clear_slots.is_some() {
                return Err(HostError::new(
                    "a clear_slots_where is already running; nested bulk clears are refused",
                ));
            }
            let containers: Vec<Uuid> = ctx
                .session
                .item_container_map()
                .map_err(core_error)?
                .iter()
                .filter_map(|entry| {
                    psp_core::props::get(psp_core::props::struct_props(&entry.key)?, &["ID"])
                        .and_then(psp_core::props::as_uuid)
                })
                .collect();
            ctx.clear_slots = Some(ClearSlotsState { containers, kill: Vec::new() });
            Ok(ctx.mutation_epoch())
        })?;

        // Plain integers: a `longjmp` past this frame loses them without
        // skipping a destructor, unlike the `Vec`s parked in the context.
        let mut container_cursor = 0usize;
        let mut slot_cursor = 0usize;
        let mut examined = 0i64;
        let mut callback_error: Option<String> = None;

        loop {
            let next = with_context(state, |ctx| {
                // The cursor walks by position in a container re-read each step,
                // so a structural write from the predicate must stop the walk.
                if ctx.mutation_epoch() != epoch {
                    return Err(invalidated_handle_error());
                }
                loop {
                    let Some(container_id) = ctx
                        .clear_slots
                        .as_ref()
                        .ok_or_else(|| HostError::new("clear_slots_where state is missing"))?
                        .containers
                        .get(container_cursor)
                        .copied()
                    else {
                        return Ok(None);
                    };
                    match super::save_read::read_container(ctx, container_id)
                        .and_then(|dto| dto.slots.get(slot_cursor).map(|slot| slot.slot_index))
                    {
                        Some(slot_index) => {
                            slot_cursor += 1;
                            return Ok(Some((container_id, slot_index)));
                        }
                        None => {
                            container_cursor += 1;
                            slot_cursor = 0;
                        }
                    }
                }
            })?;
            let Some((container_id, slot_index)) = next else { break };

            lua_pushvalue(state, predicate_index);
            push_handle(
                state,
                Handle { kind: HandleKind::Slot, id: container_id, slot: slot_index, epoch },
            );
            let call_status = lua_pcall(state, 1, 1, 0);
            if call_status != LUA_OK {
                let message = read_string_if(state, -1).unwrap_or_else(|| {
                    "the clear_slots_where predicate raised a non-string error".to_string()
                });
                lua_pop(state, 1);
                callback_error = Some(message);
                break;
            }
            let truthy = lua_toboolean(state, -1) != 0;
            lua_pop(state, 1);
            examined += 1;

            if truthy {
                with_context(state, |ctx| {
                    ctx.clear_slots
                        .as_mut()
                        .ok_or_else(|| HostError::new("clear_slots_where state is missing"))?
                        .kill
                        .push((container_id, slot_index));
                    Ok(())
                })?;
            }
        }

        if let Some(message) = callback_error {
            with_context(state, |ctx| {
                ctx.clear_slots = None;
                Ok(())
            })?;
            return Err(HostError::new(message));
        }

        let cleared = with_context(state, |ctx| {
            super::dto_cache::flush(ctx)?;
            let stateful = ctx
                .clear_slots
                .take()
                .ok_or_else(|| HostError::new("clear_slots_where state is missing"))?;
            let cleared = stateful.kill.len() as i64;

            if ctx.dry_run {
                ctx.bump("slot.clear", cleared);
                for (container_id, slot_index) in &stateful.kill {
                    super::dto_cache::forget_pending_slot(ctx, *container_id, *slot_index);
                }
                return Ok(cleared);
            }
            if cleared == 0 {
                return Ok(0);
            }

            let mut batch: Vec<(Uuid, ItemContainerDto)> = Vec::new();
            let mut grouped = 0usize;
            while grouped < stateful.kill.len() {
                let container_id = stateful.kill[grouped].0;
                let slots: Vec<ItemContainerSlotDto> = stateful.kill[grouped..]
                    .iter()
                    .take_while(|(id, _)| *id == container_id)
                    .map(|(_, slot_index)| ItemContainerSlotDto {
                        dynamic_item: None,
                        slot_index: *slot_index,
                        count: 0,
                        static_id: Some("None".to_string()),
                        local_id: None,
                    })
                    .collect();
                grouped += slots.len();
                batch.push((
                    container_id,
                    ItemContainerDto {
                        id: container_id,
                        r#type: String::new(),
                        slots,
                        key: None,
                        slot_num: 0,
                    },
                ));
            }

            let fallback = null_progress();
            let progress: &ProgressSink = ctx.progress.unwrap_or(&fallback);
            progress(&format!(
                "Clearing {cleared} item slot(s) across {} container(s)",
                batch.len()
            ));
            containers::apply_item_container_dtos(ctx.session, &batch).map_err(core_error)?;
            ctx.note_mutation();
            Ok(cleared)
        })?;

        lua_pushinteger(state, cleared);
        lua_pushinteger(state, examined);
        Ok(2)
    }
}

host_fn!(push_clear_slots_where, clear_slots_where_body);

fn unlock_private_chests_body(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 0, "save.unlock_private_chests")?;
        let cleared = with_context(state, |ctx| {
            if ctx.dry_run {
                let count = map_object::count_private_chest_locks(ctx.session).map_err(core_error)?;
                ctx.bump("save.unlock_private_chests", count as i64);
                return Ok(count);
            }
            super::dto_cache::flush(ctx)?;
            let cleared = map_object::unlock_private_chests(ctx.session).map_err(core_error)?;
            if cleared > 0 {
                ctx.note_mutation();
            }
            Ok(cleared)
        })?;
        lua_pushinteger(state, cleared as i64);
        Ok(1)
    }
}

host_fn!(push_unlock_private_chests, unlock_private_chests_body);

fn delete_dps_pals_body(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 2, "save.delete_dps_pals")?;
        let player_uid = arg_uuid(state, 1, "player_uid")?;
        let indexes_json = table_to_json(state, 2)?;
        let indexes: Vec<i32> = indexes_json
            .as_array()
            .ok_or_else(|| HostError::new("save.delete_dps_pals expects an array table for indexes"))?
            .iter()
            .map(|value| {
                value
                    .as_i64()
                    .and_then(|n| i32::try_from(n).ok())
                    .ok_or_else(|| HostError::new("save.delete_dps_pals indexes must all be integers"))
            })
            .collect::<Result<Vec<i32>, HostError>>()?;

        let emptied = with_context(state, |ctx| {
            if !ctx.grants(Capability::Players) {
                return Err(HostError::new("save.delete_dps_pals requires the players capability"));
            }
            super::dto_cache::flush(ctx)?;
            // A player who exists but has no `_dps.sav` at all is a real, common state
            // (dimensional storage is unlocked separately) -- `raw_len` errors for it the
            // same way `raw.*` does, but this is not the caller's fault to handle, so it is
            // treated the same as a DPS array with nothing in it, matching how
            // `delete_player_dps_pals` itself is a no-op for that player. A missing player
            // still raises, from `ensure_player_loaded` above.
            ctx.session.ensure_player_loaded(player_uid).map_err(core_error)?;
            let path = RawPath::parse("SaveParameterArray").map_err(core_error)?;
            let slot_count =
                ctx.session.raw_len(RawScope::PlayerDps(player_uid), &path).ok().flatten().unwrap_or(0);
            let valid = indexes.iter().filter(|&&index| index >= 0 && (index as usize) < slot_count).count();

            if ctx.dry_run {
                ctx.bump("save.delete_dps_pals", valid as i64);
                return Ok(valid);
            }
            if valid > 0 {
                pal::delete_player_dps_pals(ctx.session, ctx.game_data, player_uid, &indexes)
                    .map_err(core_error)?;
                ctx.note_mutation();
            }
            Ok(valid)
        })?;

        lua_pushinteger(state, emptied as i64);
        Ok(1)
    }
}

host_fn!(push_delete_dps_pals, delete_dps_pals_body);

fn kind_to_int(kind: DeleteWhereKind) -> i64 {
    match kind {
        DeleteWhereKind::Player => 0,
        DeleteWhereKind::Guild => 1,
        DeleteWhereKind::Pal => 2,
        DeleteWhereKind::MapObject => 3,
    }
}

fn kind_from_int(value: i64) -> Option<DeleteWhereKind> {
    match value {
        0 => Some(DeleteWhereKind::Player),
        1 => Some(DeleteWhereKind::Guild),
        2 => Some(DeleteWhereKind::Pal),
        3 => Some(DeleteWhereKind::MapObject),
        _ => None,
    }
}

fn count_key(kind: DeleteWhereKind) -> &'static str {
    match kind {
        DeleteWhereKind::Player => "players.delete_where",
        DeleteWhereKind::Guild => "guilds.delete_where",
        DeleteWhereKind::Pal => "pals.delete_where",
        DeleteWhereKind::MapObject => "map_objects.delete_where",
    }
}

/// Not `kill.len()`: the real apply phase skips guild admins and pals whose
/// owner cannot be loaded, and this must skip exactly the same ids.
fn count_dry_run(
    ctx: &mut RunContext<'_>,
    kind: DeleteWhereKind,
    kill: &[Uuid],
) -> Result<(i64, i64), HostError> {
    match kind {
        DeleteWhereKind::Player => {
            // Resolving first is what populates `loaded_guilds`; without it
            // `player_is_guild_admin` answers `false` for every admin.
            let fallback = null_progress();
            let mut count = 0i64;
            let mut skipped = 0i64;
            for id in kill {
                let progress: &ProgressSink = ctx.progress.unwrap_or(&fallback);
                let resolved = player::get_player_details(ctx.session, ctx.game_data, *id, progress)
                    .map_err(core_error)?
                    .is_some();
                if !resolved {
                    skipped += 1;
                    log_skip(ctx, kind, *id, "player details could not be resolved");
                } else if player_is_guild_admin(ctx, *id) {
                    skipped += 1;
                    log_skip(ctx, kind, *id, "player is a guild admin");
                } else {
                    count += 1;
                }
            }
            Ok((count, skipped))
        }
        DeleteWhereKind::Guild => Ok((kill.len() as i64, 0)),
        DeleteWhereKind::MapObject => {
            super::save_read::ensure_map_objects_snapshot(ctx)?;
            let resolvable = match ctx.map_objects.as_ref() {
                Some((_, index)) => kill.iter().filter(|id| index.contains_key(id)).count(),
                None => 0,
            };
            Ok((resolvable as i64, kill.len() as i64 - resolvable as i64))
        }
        DeleteWhereKind::Pal => {
            let fallback = null_progress();
            let mut count = 0i64;
            let mut skipped = 0i64;
            for id in kill {
                let Some((owner_uid, guild_base)) = pal_routing(ctx, *id)? else {
                    skipped += 1;
                    log_skip(ctx, kind, *id, "pal could not be routed to an owner or guild base");
                    continue;
                };
                let progress: &ProgressSink = ctx.progress.unwrap_or(&fallback);
                let (resolves, reason) = match (owner_uid, guild_base) {
                    (Some(owner), _) => (
                        player::get_player_details(ctx.session, ctx.game_data, owner, progress)
                            .map_err(core_error)?
                            .is_some(),
                        "owning player's details could not be resolved",
                    ),
                    (None, Some((guild_id, _))) => (
                        guild::get_guild_details(ctx.session, ctx.game_data, guild_id)
                            .map_err(core_error)?
                            .is_some(),
                        "owning guild's details could not be resolved",
                    ),
                    (None, None) => (false, "pal has neither an owner player nor a guild base"),
                };
                if resolves {
                    count += 1;
                } else {
                    skipped += 1;
                    log_skip(ctx, kind, *id, reason);
                }
            }
            Ok((count, skipped))
        }
    }
}

pub(crate) unsafe fn push_delete_where(state: *mut lua_State, kind: DeleteWhereKind) {
    let dispatch = (trampoline as *const () as usize) as *mut c_void;
    let bodyptr = ((delete_where_body as HostFn) as usize) as *mut c_void;
    let free = (free_message as *const () as usize) as *mut c_void;
    lua_pushlightuserdata(state, dispatch);
    lua_pushlightuserdata(state, bodyptr);
    lua_pushlightuserdata(state, free);
    lua_pushinteger(state, kind_to_int(kind));
    lua_pushcclosure(state, psp_host_trampoline, 4);
}

/// Selects with no `&mut RunContext` held across the predicate's `lua_pcall`.
fn delete_where_body(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        if lua_gettop(state) < 2 || !lua_isfunction(state, 2) {
            let actual = super::marshal::type_name(state, 2);
            return Err(HostError::new(format!(
                "delete_where expects a function for its argument, got {actual}"
            )));
        }
        let predicate_index: c_int = 2;
        let kind_value = arg_integer(state, lua_upvalueindex(4), "delete_where kind")?;
        let kind = kind_from_int(kind_value)
            .ok_or_else(|| HostError::new("corrupt delete_where iterator kind"))?;

        let (epoch,) = with_context(state, |ctx| {
            if ctx.delete_where.is_some() {
                return Err(HostError::new(
                    "a delete_where is already running; nested bulk deletes are refused",
                ));
            }
            let ids: Vec<Uuid> = match kind {
                DeleteWhereKind::Player => ctx.session.player_summary_order.clone(),
                DeleteWhereKind::Guild => ctx.session.guild_summary_order.clone(),
                DeleteWhereKind::Pal => {
                    super::save_read::ensure_pals_snapshot(ctx)?;
                    ctx.pals
                        .as_ref()
                        .map(|(snapshot, _)| snapshot.iter().map(|p| p.instance_id).collect())
                        .unwrap_or_default()
                }
                DeleteWhereKind::MapObject => {
                    super::save_read::ensure_map_objects_snapshot(ctx)?;
                    ctx.map_objects
                        .as_ref()
                        .map(|(views, _)| views.iter().map(|view| view.instance_id).collect())
                        .unwrap_or_default()
                }
            };
            ctx.delete_where = Some(DeleteWhereState { kind, ids, kill: Vec::new() });
            Ok((ctx.mutation_epoch(),))
        })?;

        let mut index = 0usize;
        let mut callback_error: Option<String> = None;
        loop {
            let next_id = with_context(state, |ctx| {
                let dw = ctx
                    .delete_where
                    .as_ref()
                    .ok_or_else(|| HostError::new("delete_where state is missing"))?;
                Ok(dw.ids.get(index).copied())
            })?;
            let Some(id) = next_id else { break };

            lua_pushvalue(state, predicate_index);
            push_handle(state, Handle { kind: handle_kind_for(kind), id, slot: -1, epoch });
            let call_status = lua_pcall(state, 1, 1, 0);
            if call_status != LUA_OK {
                let message = read_string_if(state, -1)
                    .unwrap_or_else(|| "the delete_where predicate raised a non-string error".to_string());
                lua_pop(state, 1);
                callback_error = Some(message);
                break;
            }
            let truthy = lua_toboolean(state, -1) != 0;
            lua_pop(state, 1);

            if truthy {
                with_context(state, |ctx| {
                    let dw = ctx
                        .delete_where
                        .as_mut()
                        .ok_or_else(|| HostError::new("delete_where state is missing"))?;
                    dw.kill.push(id);
                    Ok(())
                })?;
            }
            index += 1;
        }

        if let Some(message) = callback_error {
            with_context(state, |ctx| {
                ctx.delete_where = None;
                Ok(())
            })?;
            return Err(HostError::new(message));
        }

        let (removed, skipped) = with_context(state, |ctx| {
            super::dto_cache::flush(ctx)?;
            let dw = ctx
                .delete_where
                .take()
                .ok_or_else(|| HostError::new("delete_where state is missing"))?;

            if ctx.dry_run {
                let (count, skipped) = count_dry_run(ctx, dw.kind, &dw.kill)?;
                ctx.bump(count_key(dw.kind), count);
                return Ok((count, skipped));
            }

            let fallback = null_progress();
            let progress: &ProgressSink = ctx.progress.unwrap_or(&fallback);
            let mut removed = 0i64;
            let mut skipped = 0i64;
            macro_rules! skip {
                ($id:expr, $reason:expr) => {{
                    skipped += 1;
                    log_skip(ctx, dw.kind, $id, $reason);
                    continue;
                }};
            }
            match dw.kind {
                DeleteWhereKind::Player => {
                    for id in &dw.kill {
                        if player::get_player_details(ctx.session, ctx.game_data, *id, progress)
                            .map_err(core_error)?
                            .is_none()
                        {
                            skip!(*id, "player details could not be resolved");
                        }
                        if player::delete_player(ctx.session, ctx.game_data, *id, progress).map_err(core_error)? {
                            let guild_id = ctx.session.player_summaries.get(id).and_then(|s| s.guild_id);
                            prune_player_summary(ctx, *id);
                            if let Some(guild_id) = guild_id {
                                decrement_guild_member_count(ctx, guild_id);
                            }
                            removed += 1;
                        } else {
                            skip!(*id, "player is a guild admin");
                        }
                    }
                }
                DeleteWhereKind::Guild => {
                    for id in &dw.kill {
                        let Some(details) = guild::get_guild_details(ctx.session, ctx.game_data, *id)
                            .map_err(core_error)?
                        else {
                            skip!(*id, "guild details could not be resolved");
                        };
                        let deleted_players: Vec<Uuid> = details
                            .players
                            .iter()
                            .copied()
                            .filter(|player_id| ctx.session.loaded_players.contains_key(player_id))
                            .collect();
                        guild::delete_guild_and_players(ctx.session, ctx.game_data, *id, progress)
                            .map_err(core_error)?;
                        prune_guild_summary(ctx, *id);
                        for player_id in deleted_players {
                            prune_player_summary(ctx, player_id);
                        }
                        removed += 1;
                    }
                }
                DeleteWhereKind::Pal => {
                    for id in &dw.kill {
                        let Some((owner_uid, guild_base)) = pal_routing(ctx, *id)? else {
                            skip!(*id, "pal could not be routed to an owner or guild base");
                        };
                        match (owner_uid, guild_base) {
                            (Some(owner), _) => {
                                if player::get_player_details(ctx.session, ctx.game_data, owner, progress)
                                    .map_err(core_error)?
                                    .is_none()
                                {
                                    skip!(*id, "owning player's details could not be resolved");
                                }
                                pal::delete_player_pals(ctx.session, owner, &[*id]).map_err(core_error)?;
                            }
                            (None, Some((guild_id, base_id))) => {
                                if guild::get_guild_details(ctx.session, ctx.game_data, guild_id)
                                    .map_err(core_error)?
                                    .is_none()
                                {
                                    skip!(*id, "owning guild's details could not be resolved");
                                }
                                pal::delete_guild_pals(ctx.session, guild_id, base_id, &[*id])
                                    .map_err(core_error)?;
                            }
                            (None, None) => skip!(*id, "pal has neither an owner player nor a guild base"),
                        }
                        decrement_owner_pal_count(ctx, owner_uid, guild_base.map(|(guild_id, _)| guild_id));
                        removed += 1;
                    }
                }
                DeleteWhereKind::MapObject => {
                    let removed_count =
                        map_object::remove_map_objects(ctx.session, &dw.kill).map_err(core_error)?;
                    removed = removed_count as i64;
                    skipped = dw.kill.len() as i64 - removed;
                }
            }
            if removed > 0 {
                ctx.note_mutation();
            }
            Ok((removed, skipped))
        })?;

        lua_pushinteger(state, removed);
        lua_pushinteger(state, skipped);
        Ok(2)
    }
}

/// Inserts one key into the `__index` table `save_read::install` left; it never
/// replaces that table, and creates one only for a state built without it.
unsafe fn add_delete_where(state: *mut lua_State, kind: DeleteWhereKind) {
    let name = super::save_read::iter_metatable_name(kind);
    luaL_newmetatable(state, name.as_ptr());
    lua_getfield(state, -1, c"__index".as_ptr());
    if lua_type(state, -1) != LUA_TTABLE {
        lua_pop(state, 1);
        lua_createtable(state, 0, 1);
        lua_pushvalue(state, -1);
        lua_setfield(state, -3, c"__index".as_ptr());
    }
    push_delete_where(state, kind);
    lua_setfield(state, -2, c"delete_where".as_ptr());
    lua_pop(state, 2);
}

pub const SAVE_WRITE_FUNCTIONS: &[ApiFunction] = &[
    ApiFunction {
        name: "clear_slots_where",
        params: &[ApiParam { name: "predicate", ty: ApiType::Any, optional: false }],
        returns: ApiType::Integer,
        doc: "Calls predicate(slot) once for every item slot in the save with nothing yet \
              mutated, then clears every slot predicate returned truthy for. Returns the \
              number cleared, followed by the number examined. A non-zero clear count is a \
              structural write and invalidates every live handle and iterator, including ones \
              the walk itself was still using -- call this instead of looping over \
              save.containers() and clearing slots one at a time, which a structural write \
              would break after the first clear.",
        capability: Some(Capability::SaveWrite),
    },
    ApiFunction {
        name: "unlock_private_chests",
        params: &[],
        returns: ApiType::Integer,
        doc: "Clears the ownership lock on every private chest and item booth, returning how \
              many were actually cleared. A non-zero result is a structural write and \
              invalidates every live handle and iterator.",
        capability: Some(Capability::SaveWrite),
    },
    ApiFunction {
        name: "delete_dps_pals",
        params: &[
            ApiParam { name: "player_uid", ty: ApiType::String, optional: false },
            ApiParam { name: "indexes", ty: ApiType::List(&ApiType::Integer), optional: false },
        ],
        returns: ApiType::Integer,
        doc: "Empties the given slot indexes of one player's dimensional storage in place -- \
              nils the slot's InstanceId and resets its SaveParameter bag to an unused slot's \
              shape, the same way the slot got there in the first place, without changing the \
              storage array's length. Returns how many of the given indexes were valid. \
              Requires capability: players.",
        capability: Some(Capability::SaveWrite),
    },
];

const SAVE_WRITE_PUSH_FNS: [PushHostFn; SAVE_WRITE_FUNCTIONS.len()] =
    [push_clear_slots_where, push_unlock_private_chests, push_delete_dps_pals];

fn save_write_bindings() -> [(&'static str, PushHostFn); SAVE_WRITE_FUNCTIONS.len()] {
    std::array::from_fn(|i| (SAVE_WRITE_FUNCTIONS[i].name, SAVE_WRITE_PUSH_FNS[i]))
}

/// Must run after `save_read::install`, whose tables this extends.
pub unsafe fn install(state: *mut lua_State) {
    add_delete_where(state, DeleteWhereKind::Player);
    add_delete_where(state, DeleteWhereKind::Guild);
    add_delete_where(state, DeleteWhereKind::Pal);
    add_delete_where(state, DeleteWhereKind::MapObject);
    add_save_functions(state);
}

unsafe fn add_save_functions(state: *mut lua_State) {
    lua_getglobal(state, c"save".as_ptr());
    if lua_type(state, -1) == LUA_TTABLE {
        for (name, push) in save_write_bindings() {
            push(state);
            let Ok(key) = std::ffi::CString::new(name) else {
                lua_pop(state, 1);
                continue;
            };
            lua_setfield(state, -2, key.as_ptr());
        }
    }
    lua_pop(state, 1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_names_match_described_functions_in_order() {
        let bindings = save_write_bindings();
        let bound_names: Vec<&str> = bindings.iter().map(|(name, _)| *name).collect();
        let described_names: Vec<&str> = SAVE_WRITE_FUNCTIONS.iter().map(|f| f.name).collect();
        assert_eq!(bound_names, described_names);
    }
}
