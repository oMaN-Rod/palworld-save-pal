use std::collections::HashMap;
use std::ffi::{c_int, c_void, CStr};
use std::sync::OnceLock;

use psp_core::domain::{containers, pal, world};
use psp_core::dto::summary::IsoDateTime;
use psp_core::error::CoreError;
use psp_core::props;
use psp_core::session::SaveSession;
use psp_lua_sys::ffi::*;
use uuid::Uuid;

use super::api_def::{ApiFunction, ApiHandle, ApiParam, ApiType};
use super::fields::{
    base as base_fields, container as container_fields, guild as guild_fields, pal as pal_fields,
    player as player_fields, slot as slot_fields,
    push_field_value, FieldValue,
};
use super::handle::{handle_kind_for, invalidated_handle_error, push_handle, read_handle, Handle, HandleKind};
use super::marshal::{arg_string, check_args, push_str};
use super::{free_message, register_table, trampoline, with_context, HostError, HostFn, PushHostFn};
use crate::context::{DeleteWhereKind, PalIndexEntry, RunContext};
use crate::host_fn;
use crate::manifest::Capability;

fn core_error(error: CoreError) -> HostError {
    HostError::new(error.to_string())
}

/// Deliberately avoids `session.character_index`: nothing here refreshes it, and
/// after a raw removal a stale position resolves to a different pal entirely.
fn boss_and_lucky_by_pal(session: &SaveSession) -> HashMap<Uuid, (bool, bool)> {
    let mut flags = HashMap::new();
    let Ok(entries) = session.character_map() else {
        return flags;
    };
    for entry in entries {
        if world::entry_is_player(entry) {
            continue;
        }
        let Some(id) = world::entry_instance_id(entry) else {
            continue;
        };
        let Some(save_parameter) = world::entry_save_parameter(entry) else {
            continue;
        };
        let character_id =
            props::get(save_parameter, &["CharacterID"]).and_then(props::as_str).unwrap_or("");
        flags.insert(id, pal::boss_and_lucky(save_parameter, character_id));
    }
    flags
}

pub(crate) fn ensure_pals_snapshot(ctx: &mut RunContext<'_>) -> Result<(), HostError> {
    if ctx.pals.is_none() {
        super::dto_cache::flush(ctx)?;
        let snapshot = pal::pal_summaries(ctx.session, ctx.game_data).map_err(core_error)?;
        let flags = boss_and_lucky_by_pal(ctx.session);
        let mut index: HashMap<Uuid, PalIndexEntry> = HashMap::with_capacity(snapshot.len());
        for (position, summary) in snapshot.iter().enumerate() {
            let (is_boss, is_lucky) = flags.get(&summary.instance_id).copied().unwrap_or((false, false));
            index.insert(summary.instance_id, PalIndexEntry { position, is_boss, is_lucky });
        }
        ctx.pals = Some((snapshot, index));
    }
    Ok(())
}

pub(crate) fn iso_string(value: Option<IsoDateTime>) -> Option<String> {
    let value = value?;
    serde_json::to_value(value).ok()?.as_str().map(str::to_string)
}

/// The two player rows the summary can answer but the field table cannot read
/// from it: `name` and `level` are assignable, so their rows have to read the
/// cached `PlayerDto` for a value written this run -- which means the cheap,
/// no-load path for an *unwritten* one has to live outside the table. Every
/// other summary-backed row is a `Reader::Summary` row and is answered by
/// `player_get` directly, with no duplicate of its reader here.
///
/// `None` means "not answered here", which is a different thing from answering
/// `nil`, and the distinction is load-bearing: `level` is legitimately `nil`
/// for a player the save records no `Level` byte for. Reporting that as "not
/// answered" would fall through to the row's `PlayerDto` reader, which pays a
/// lazy `.sav` load and then hands back `build_player_dto`'s own default --
/// turning a shipped `nil` into a `1`.
fn player_field(ctx: &RunContext<'_>, uid: Uuid, field: &str) -> Option<FieldValue> {
    if !player_fields::SUMMARY_SHORTCUT_FIELDS.contains(&field) {
        return None;
    }
    let summary = ctx.session.player_summaries.get(&uid)?;
    Some(match field {
        "name" => FieldValue::Str(summary.nickname.clone()),
        "level" => summary.level.map(FieldValue::Int).unwrap_or(FieldValue::Nil),
        _ => return None,
    })
}

fn pal_field(ctx: &RunContext<'_>, id: Uuid, field: &str) -> FieldValue {
    let Some((snapshot, index)) = ctx.pals.as_ref() else {
        return FieldValue::Nil;
    };
    let Some(entry) = index.get(&id) else {
        return FieldValue::Nil;
    };
    let Some(summary) = snapshot.get(entry.position) else {
        return FieldValue::Nil;
    };
    match field {
        "instance_id" => FieldValue::Str(summary.instance_id.to_string()),
        "character_id" => FieldValue::Str(summary.character_id.clone()),
        "nickname" => summary.nickname.clone().map(FieldValue::Str).unwrap_or(FieldValue::Nil),
        "owner_uid" => summary
            .owner_uid
            .map(|u| FieldValue::Str(u.to_string()))
            .unwrap_or(FieldValue::Nil),
        "guild_id" => summary
            .guild_id
            .map(|u| FieldValue::Str(u.to_string()))
            .unwrap_or(FieldValue::Nil),
        "base_id" => summary
            .base_id
            .map(|u| FieldValue::Str(u.to_string()))
            .unwrap_or(FieldValue::Nil),
        "gender" => summary.gender.clone().map(FieldValue::Str).unwrap_or(FieldValue::Nil),
        "level" => FieldValue::Int(summary.level),
        "hp" => FieldValue::Int(summary.hp),
        "rank" => FieldValue::Int(summary.rank),
        "exp" => FieldValue::Int(summary.exp),
        "talent_hp" => FieldValue::Int(summary.talent_hp),
        "talent_shot" => FieldValue::Int(summary.talent_shot),
        "talent_defense" => FieldValue::Int(summary.talent_defense),
        "rank_hp" => FieldValue::Int(summary.rank_hp),
        "rank_attack" => FieldValue::Int(summary.rank_attack),
        "rank_defense" => FieldValue::Int(summary.rank_defense),
        "rank_craftspeed" => FieldValue::Int(summary.rank_craftspeed),
        "is_boss" => FieldValue::Bool(entry.is_boss),
        "is_lucky" => FieldValue::Bool(entry.is_lucky),
        _ => FieldValue::Nil,
    }
}

unsafe fn write_granted(state: *mut lua_State) -> Result<bool, HostError> {
    with_context(state, |ctx| Ok(ctx.grants(Capability::SaveWrite)))
}

fn player_index(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 2, "player field")?;
        let handle = read_handle(state, 1, HandleKind::Player)?;
        let field = arg_string(state, 2, "field")?;

        if field == "pals" {
            drop(field);
            push_player_pals_factory(state, handle.id);
            return Ok(1);
        }
        if field == "delete" && write_granted(state)? {
            drop(field);
            super::save_write::push_player_delete(state, handle.id);
            return Ok(1);
        }

        let value = with_context(state, |ctx| {
            // A field this run has already written but not yet flushed must be
            // served from the DTO cache directly. The pal side does this
            // because its summary only reflects a flush; the player side has
            // the stronger reason that its summary is session state nothing
            // recomputes at all, so a stale row would stay stale.
            if super::dto_cache::player_field_was_written(ctx, handle.id, &field) {
                return player_fields::player_get(ctx, handle.id, &field);
            }
            // The summary answers the two rows it has a shortcut for without
            // touching the disk, including when its answer is nil. Every other
            // name falls through to the field table, which knows the rest --
            // and answers nil, loading nothing, for a name that is not a field
            // at all.
            match player_field(ctx, handle.id, &field) {
                Some(value) => Ok(value),
                None => player_fields::player_get(ctx, handle.id, &field),
            }
        })?;
        drop(field);
        push_field_value(state, value)?;
        Ok(1)
    }
}

fn pal_index(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 2, "pal field")?;
        let handle = read_handle(state, 1, HandleKind::Pal)?;
        let field = arg_string(state, 2, "field")?;
        if field == "delete" && write_granted(state)? {
            drop(field);
            super::save_write::push_pal_delete(state, handle.id);
            return Ok(1);
        }
        let value = with_context(state, |ctx| {
            // A field this run has already written but not yet flushed must
            // be served from the DTO cache directly: the summary only ever
            // reflects a real flush's write, and a dry run never flushes at
            // all, so rebuilding the summary here would answer with the
            // pre-write value even though this run's own write is sitting
            // right there in the cache.
            if super::dto_cache::pal_field_was_written(ctx, handle.id, &field) {
                return pal_fields::pal_get(ctx, handle.id, &field);
            }
            // `note_pal_field_write` can have dropped the snapshot without
            // invalidating this handle, so rebuild rather than read nothing.
            ensure_pals_snapshot(ctx)?;
            let value = pal_field(ctx, handle.id, &field);
            // The summary answers most pal fields directly; anything it
            // doesn't carry (nil here, since `pal_field`'s catch-all also
            // returns nil for a genuinely unknown name) falls through to the
            // field table, which knows the rest.
            if matches!(value, FieldValue::Nil) {
                pal_fields::pal_get(ctx, handle.id, &field)
            } else {
                Ok(value)
            }
        })?;
        drop(field);
        push_field_value(state, value)?;
        Ok(1)
    }
}

fn guild_index(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 2, "guild field")?;
        let handle = read_handle(state, 1, HandleKind::Guild)?;
        let field = arg_string(state, 2, "field")?;
        if field == "delete" && write_granted(state)? {
            drop(field);
            super::save_write::push_guild_delete(state, handle.id);
            return Ok(1);
        }
        let value = with_context(state, |ctx| guild_fields::guild_get(ctx, handle.id, &field))?;
        drop(field);
        push_field_value(state, value)?;
        Ok(1)
    }
}

fn base_index(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 2, "base field")?;
        let handle = read_handle(state, 1, HandleKind::Base)?;
        let field = arg_string(state, 2, "field")?;
        if field == "delete" && write_granted(state)? {
            drop(field);
            super::save_write::push_base_delete(state, handle.id);
            return Ok(1);
        }
        let value = with_context(state, |ctx| base_fields::base_get(ctx, handle.id, &field))?;
        drop(field);
        push_field_value(state, value)?;
        Ok(1)
    }
}

pub(crate) fn read_container<'ctx>(
    ctx: &'ctx mut RunContext<'_>,
    container_id: Uuid,
) -> Option<&'ctx psp_core::dto::container::ItemContainerDto> {
    if ctx.container.as_ref().map(|(cached, _)| *cached) != Some(container_id) {
        ctx.container = containers::read_item_container(
            &ctx.session.level,
            &mut ctx.session.caches,
            ctx.game_data,
            container_id,
            "",
            None,
        )
        .map(|dto| (container_id, dto));
    }
    if ctx.dry_run {
        super::dto_cache::overlay_pending_slots(ctx, container_id);
    }
    ctx.container.as_ref().map(|(_, dto)| dto)
}

fn container_field(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 2, "container field")?;
        let handle = read_handle(state, 1, HandleKind::Container)?;
        let field = arg_string(state, 2, "field")?;

        if field == "slots" {
            drop(field);
            push_container_slots_factory(state, handle.id);
            return Ok(1);
        }

        if field == "set_slot_count" {
            if write_granted(state)? {
                drop(field);
                super::save_write::push_container_set_slot_count(state, handle.id);
                return Ok(1);
            }
            drop(field);
            lua_pushnil(state);
            return Ok(1);
        }

        let value =
            with_context(state, |ctx| container_fields::container_get(ctx, handle.id, &field))?;
        drop(field);
        push_field_value(state, value)?;
        Ok(1)
    }
}

fn slot_field(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 2, "slot field")?;
        let handle = read_handle(state, 1, HandleKind::Slot)?;
        let field = arg_string(state, 2, "field")?;

        if field == "clear" {
            if write_granted(state)? {
                drop(field);
                super::save_write::push_slot_clear(state, handle.id, handle.slot);
                return Ok(1);
            }
            drop(field);
            lua_pushnil(state);
            return Ok(1);
        }

        let value = with_context(state, |ctx| {
            slot_fields::slot_get(ctx, handle.id, handle.slot, &field)
        })?;
        drop(field);
        push_field_value(state, value)?;
        Ok(1)
    }
}

static HANDLE_TYPES: OnceLock<Vec<ApiHandle>> = OnceLock::new();

/// Not a `const`: the pal handle's fields are projected from `PAL_FIELDS` at first
/// use so the description cannot drift from the rows that implement it.
pub fn handle_types() -> &'static [ApiHandle] {
    HANDLE_TYPES
        .get_or_init(|| {
            vec![
                ApiHandle {
                    name: "player",
                    capability: Some(Capability::SaveRead),
                    fields: player_fields::api_fields(),
                    methods: &[
                        ApiFunction {
                            name: "pals",
                            params: &[],
                            returns: ApiType::Iterator("pal"),
                            doc: "An iterator over every pal this player owns, for use in a `for` loop.",
                            capability: None,
                        },
                        ApiFunction {
                            name: "delete",
                            params: &[],
                            returns: ApiType::Boolean,
                            doc: "Deletes this player, along with the item and character containers the \
                                  player owns. Refuses (returns false, changes nothing) if the player is \
                                  their guild's admin. A true result is a structural write and invalidates \
                                  every live handle and iterator across all scopes, including this one.",
                            capability: Some(Capability::SaveWrite),
                        },
                    ],
                },
                ApiHandle {
                    name: "pal",
                    capability: Some(Capability::SaveRead),
                    fields: pal_fields::api_fields(),
                    methods: &[
                        ApiFunction {
                            name: "delete",
                            params: &[],
                            returns: ApiType::Boolean,
                            doc: "Deletes this pal from its owning player or guild base. A structural write \
                                  and invalidates every live handle and iterator across all scopes, \
                                  including this one.",
                            capability: Some(Capability::SaveWrite),
                        },
                    ],
                },
                ApiHandle {
                    name: "guild",
                    capability: Some(Capability::SaveRead),
                    fields: guild_fields::api_fields(),
                    methods: &[ApiFunction {
                        name: "delete",
                        params: &[],
                        returns: ApiType::Boolean,
                        doc: "Deletes this guild, its bases, and every loaded member player. An unloaded \
                              member is skipped, not deleted. A structural write and invalidates every live \
                              handle and iterator across all scopes, including this one.",
                        capability: Some(Capability::SaveWrite),
                    }],
                },
                ApiHandle {
                    name: "base",
                    capability: Some(Capability::SaveRead),
                    fields: base_fields::api_fields(),
                    methods: &[ApiFunction {
                        name: "delete",
                        params: &[],
                        returns: ApiType::Boolean,
                        doc: "Deletes this base and every pal working it, and updates its guild's base_count \
                              and pal_count. A structural write and invalidates every live handle and \
                              iterator across all scopes, including this one.",
                        capability: Some(Capability::SaveWrite),
                    }],
                },
                ApiHandle {
                    name: "container",
                    capability: Some(Capability::SaveRead),
                    fields: container_fields::api_fields(),
                    methods: &[
                        ApiFunction {
                            name: "slots",
                            params: &[],
                            returns: ApiType::Iterator("slot"),
                            doc: "An iterator over every occupied slot in this container, for use in a \
                                  `for` loop.",
                            capability: None,
                        },
                        ApiFunction {
                            name: "set_slot_count",
                            params: &[ApiParam { name: "count", ty: ApiType::Integer, optional: false }],
                            returns: ApiType::Boolean,
                            doc: "Resizes the container to hold `count` slots, returning true if it \
                                  resized. Refuses (returns false, changes nothing) rather than destroying \
                                  an occupied slot that shrinking would drop. A true result is a structural \
                                  write and invalidates every live handle and iterator across all scopes, \
                                  including this one.",
                            capability: Some(Capability::SaveWrite),
                        },
                    ],
                },
                ApiHandle {
                    name: "slot",
                    capability: Some(Capability::SaveRead),
                    fields: slot_fields::api_fields(),
                    methods: &[ApiFunction {
                        name: "clear",
                        params: &[],
                        returns: ApiType::Nil,
                        doc: "Empties this slot, removing its underlying entry rather than overwriting it \
                              in place. A structural write and invalidates every live handle and iterator \
                              across all scopes, including this one -- looping over container.slots() and \
                              calling clear() on each raises after the first clear; collect ids first \
                              instead.",
                        capability: Some(Capability::SaveWrite),
                    }],
                },
            ]
        })
        .as_slice()
}

host_fn!(push_player_index, player_index);
host_fn!(push_pal_index, pal_index);
host_fn!(push_guild_index, guild_index);
host_fn!(push_base_index, base_index);
host_fn!(push_container_index, container_field);
host_fn!(push_slot_index, slot_field);

/// Upvalue 4 is a two-slot table holding the next index and the creation epoch,
/// mutated in place rather than replaced; upvalue 5, when present, is the owner.
unsafe fn push_iterator(state: *mut lua_State, body: HostFn, epoch: u64, extra: Option<&str>) {
    let dispatch = (trampoline as *const () as usize) as *mut c_void;
    let bodyptr = ((body as HostFn) as usize) as *mut c_void;
    let free = (free_message as *const () as usize) as *mut c_void;
    lua_pushlightuserdata(state, dispatch);
    lua_pushlightuserdata(state, bodyptr);
    lua_pushlightuserdata(state, free);

    lua_createtable(state, 2, 0);
    lua_pushinteger(state, 0);
    lua_rawseti(state, -2, 1);
    lua_pushinteger(state, i64::try_from(epoch).unwrap_or(i64::MAX));
    lua_rawseti(state, -2, 2);

    let n: c_int = if let Some(text) = extra {
        push_str(state, text);
        5
    } else {
        4
    };
    lua_pushcclosure(state, psp_host_trampoline, n);
}

unsafe fn read_iter_box(state: *mut lua_State) -> (i64, u64) {
    lua_rawgeti(state, lua_upvalueindex(4), 1);
    let index = lua_tointeger(state, -1);
    lua_pop(state, 1);
    lua_rawgeti(state, lua_upvalueindex(4), 2);
    let epoch = lua_tointeger(state, -1) as u64;
    lua_pop(state, 1);
    (index, epoch)
}

unsafe fn advance_iter_box(state: *mut lua_State, next_index: i64) {
    lua_pushinteger(state, next_index);
    lua_rawseti(state, lua_upvalueindex(4), 1);
}

/// Userdata with a `__call` metamethod rather than a plain closure: Lua cannot
/// hang a `delete_where` method off a function value.
#[repr(C)]
#[derive(Clone, Copy)]
struct IterState {
    kind: DeleteWhereKind,
    index: i64,
    epoch: u64,
}

pub(crate) fn iter_metatable_name(kind: DeleteWhereKind) -> &'static CStr {
    match kind {
        DeleteWhereKind::Player => c"psp.iter.player",
        DeleteWhereKind::Guild => c"psp.iter.guild",
        DeleteWhereKind::Pal => c"psp.iter.pal",
    }
}

unsafe fn push_entity_iterator(state: *mut lua_State, kind: DeleteWhereKind, epoch: u64) {
    let ptr = lua_newuserdatauv(state, std::mem::size_of::<IterState>(), 0) as *mut IterState;
    std::ptr::write(ptr, IterState { kind, index: 0, epoch });
    luaL_setmetatable(state, iter_metatable_name(kind).as_ptr());
}

fn iter_call(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        let ptr = lua_touserdata(state, 1) as *mut IterState;
        if ptr.is_null() {
            return Err(HostError::new("expected an entity iterator value"));
        }
        let cur = std::ptr::read(ptr);

        let next_id = with_context(state, |ctx| {
            if ctx.mutation_epoch() != cur.epoch {
                return Err(invalidated_handle_error());
            }
            let index = usize::try_from(cur.index).unwrap_or(usize::MAX);
            let id = match cur.kind {
                DeleteWhereKind::Player => ctx.session.player_summary_order.get(index).copied(),
                DeleteWhereKind::Guild => ctx.session.guild_summary_order.get(index).copied(),
                DeleteWhereKind::Pal => {
                    ensure_pals_snapshot(ctx)?;
                    ctx.pals.as_ref().and_then(|(snapshot, _)| snapshot.get(index)).map(|p| p.instance_id)
                }
            };
            Ok(id)
        })?;

        match next_id {
            Some(id) => {
                std::ptr::write(ptr, IterState { index: cur.index.saturating_add(1), ..cur });
                push_handle(state, Handle { kind: handle_kind_for(cur.kind), id, slot: -1, epoch: cur.epoch });
                Ok(1)
            }
            None => Ok(0),
        }
    }
}

host_fn!(push_iter_call, iter_call);

unsafe fn install_iter_metatable(state: *mut lua_State, kind: DeleteWhereKind) {
    let name = iter_metatable_name(kind);
    luaL_newmetatable(state, name.as_ptr());
    push_iter_call(state);
    lua_setfield(state, -2, c"__call".as_ptr());
    // Left empty for `save_write::install` to add `delete_where` into.
    lua_createtable(state, 0, 1);
    lua_setfield(state, -2, c"__index".as_ptr());
    lua_pushstring(state, name.as_ptr());
    lua_setfield(state, -2, c"__metatable".as_ptr());
    lua_pop(state, 1);
}

fn bases_next(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        let (index, epoch) = read_iter_box(state);
        let id = with_context(state, |ctx| {
            if ctx.mutation_epoch() != epoch {
                return Err(invalidated_handle_error());
            }
            let index = usize::try_from(index).unwrap_or(usize::MAX);
            let entries = ctx.session.base_camp_map().unwrap_or(&[]);
            Ok(entries.get(index).and_then(|entry| props::as_uuid(&entry.key)))
        })?;

        match id {
            Some(id) => {
                advance_iter_box(state, index.saturating_add(1));
                push_handle(state, Handle { kind: HandleKind::Base, id, slot: -1, epoch });
                Ok(1)
            }
            None => Ok(0),
        }
    }
}

fn player_pals_next(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        let (index, epoch) = read_iter_box(state);
        let owner_text = arg_string(state, lua_upvalueindex(5), "owner")?;
        let owner = Uuid::parse_str(&owner_text)
            .map_err(|_| HostError::new("invalid owner uid in a player.pals() iterator"))?;
        drop(owner_text);

        let found = with_context(state, |ctx| {
            if ctx.mutation_epoch() != epoch {
                return Err(invalidated_handle_error());
            }
            let (snapshot, _) = ctx.pals.as_ref().ok_or_else(|| HostError::new("pal snapshot is missing"))?;
            let start = usize::try_from(index).unwrap_or(usize::MAX);
            let found = snapshot
                .iter()
                .enumerate()
                .skip(start)
                .find(|(_, pal)| pal.owner_uid == Some(owner))
                .map(|(i, pal)| (i, pal.instance_id));
            Ok(found)
        })?;

        match found {
            Some((i, id)) => {
                advance_iter_box(state, i64::try_from(i).unwrap_or(i64::MAX).saturating_add(1));
                push_handle(state, Handle { kind: HandleKind::Pal, id, slot: -1, epoch });
                Ok(1)
            }
            None => Ok(0),
        }
    }
}

unsafe fn push_player_pals_factory(state: *mut lua_State, owner: Uuid) {
    let dispatch = (trampoline as *const () as usize) as *mut c_void;
    let bodyptr = ((player_pals_factory as HostFn) as usize) as *mut c_void;
    let free = (free_message as *const () as usize) as *mut c_void;
    lua_pushlightuserdata(state, dispatch);
    lua_pushlightuserdata(state, bodyptr);
    lua_pushlightuserdata(state, free);
    push_str(state, &owner.to_string());
    lua_pushcclosure(state, psp_host_trampoline, 4);
}

fn player_pals_factory(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 0, "player.pals")?;
        let owner_text = arg_string(state, lua_upvalueindex(4), "owner")?;
        let epoch = with_context(state, |ctx| {
            ensure_pals_snapshot(ctx)?;
            Ok(ctx.mutation_epoch())
        })?;
        push_iterator(state, player_pals_next, epoch, Some(&owner_text));
        Ok(1)
    }
}

fn save_info(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 0, "save.info")?;
        let (world_name, player_count, guild_count, pal_count, save_id) = with_context(state, |ctx| {
            ensure_pals_snapshot(ctx)?;
            let pal_count = ctx.pals.as_ref().map(|(snapshot, _)| snapshot.len()).unwrap_or(0);
            Ok((
                ctx.session.world_name.clone(),
                ctx.session.player_summary_order.len(),
                ctx.session.guild_summary_order.len(),
                pal_count,
                ctx.session.save_id.clone(),
            ))
        })?;

        lua_createtable(state, 0, 5);
        push_str(state, &world_name);
        lua_setfield(state, -2, c"world_name".as_ptr());
        drop(world_name);
        push_str(state, &save_id);
        lua_setfield(state, -2, c"save_id".as_ptr());
        drop(save_id);
        lua_pushinteger(state, i64::try_from(player_count).unwrap_or(i64::MAX));
        lua_setfield(state, -2, c"player_count".as_ptr());
        lua_pushinteger(state, i64::try_from(guild_count).unwrap_or(i64::MAX));
        lua_setfield(state, -2, c"guild_count".as_ptr());
        lua_pushinteger(state, i64::try_from(pal_count).unwrap_or(i64::MAX));
        lua_setfield(state, -2, c"pal_count".as_ptr());
        Ok(1)
    }
}

fn players_iter(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 0, "save.players")?;
        let epoch = with_context(state, |ctx| Ok(ctx.mutation_epoch()))?;
        push_entity_iterator(state, DeleteWhereKind::Player, epoch);
        Ok(1)
    }
}

fn pals_iter(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 0, "save.pals")?;
        let epoch = with_context(state, |ctx| {
            ensure_pals_snapshot(ctx)?;
            Ok(ctx.mutation_epoch())
        })?;
        push_entity_iterator(state, DeleteWhereKind::Pal, epoch);
        Ok(1)
    }
}

fn guilds_iter(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 0, "save.guilds")?;
        let epoch = with_context(state, |ctx| Ok(ctx.mutation_epoch()))?;
        push_entity_iterator(state, DeleteWhereKind::Guild, epoch);
        Ok(1)
    }
}

fn bases_iter(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 0, "save.bases")?;
        let epoch = with_context(state, |ctx| Ok(ctx.mutation_epoch()))?;
        push_iterator(state, bases_next, epoch, None);
        Ok(1)
    }
}

fn containers_next(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        let (index, epoch) = read_iter_box(state);
        let id = with_context(state, |ctx| {
            if ctx.mutation_epoch() != epoch {
                return Err(invalidated_handle_error());
            }
            let index = usize::try_from(index).unwrap_or(usize::MAX);
            let entries = ctx.session.item_container_map().map_err(core_error)?;
            Ok(entries.get(index).and_then(|entry| {
                props::get(props::struct_props(&entry.key)?, &["ID"]).and_then(props::as_uuid)
            }))
        })?;

        match id {
            Some(id) => {
                advance_iter_box(state, index.saturating_add(1));
                push_handle(state, Handle { kind: HandleKind::Container, id, slot: -1, epoch });
                Ok(1)
            }
            None => Ok(0),
        }
    }
}

fn containers_iter(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 0, "save.containers")?;
        let epoch = with_context(state, |ctx| Ok(ctx.mutation_epoch()))?;
        push_iterator(state, containers_next, epoch, None);
        Ok(1)
    }
}

fn slots_next(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        let (index, epoch) = read_iter_box(state);
        let container_text = arg_string(state, lua_upvalueindex(5), "container")?;
        let container_id = Uuid::parse_str(&container_text)
            .map_err(|_| HostError::new("invalid container id in a container.slots() iterator"))?;
        drop(container_text);

        let slot_index = with_context(state, |ctx| {
            if ctx.mutation_epoch() != epoch {
                return Err(invalidated_handle_error());
            }
            let position = usize::try_from(index).unwrap_or(usize::MAX);
            Ok(read_container(ctx, container_id).and_then(|dto| dto.slots.get(position).map(|slot| slot.slot_index)))
        })?;

        match slot_index {
            Some(slot_index) => {
                advance_iter_box(state, index.saturating_add(1));
                push_handle(state, Handle { kind: HandleKind::Slot, id: container_id, slot: slot_index, epoch });
                Ok(1)
            }
            None => Ok(0),
        }
    }
}

unsafe fn push_container_slots_factory(state: *mut lua_State, container_id: Uuid) {
    let dispatch = (trampoline as *const () as usize) as *mut c_void;
    let bodyptr = ((container_slots_factory as HostFn) as usize) as *mut c_void;
    let free = (free_message as *const () as usize) as *mut c_void;
    lua_pushlightuserdata(state, dispatch);
    lua_pushlightuserdata(state, bodyptr);
    lua_pushlightuserdata(state, free);
    push_str(state, &container_id.to_string());
    lua_pushcclosure(state, psp_host_trampoline, 4);
}

fn container_slots_factory(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 0, "container.slots")?;
        let owner_text = arg_string(state, lua_upvalueindex(4), "container")?;
        let epoch = with_context(state, |ctx| Ok(ctx.mutation_epoch()))?;
        push_iterator(state, slots_next, epoch, Some(&owner_text));
        Ok(1)
    }
}

host_fn!(push_info, save_info);
host_fn!(push_players, players_iter);
host_fn!(push_pals, pals_iter);
host_fn!(push_guilds, guilds_iter);
host_fn!(push_bases, bases_iter);
host_fn!(push_containers, containers_iter);

pub const SAVE_READ_FUNCTIONS: &[ApiFunction] = &[
    ApiFunction {
        name: "info",
        params: &[],
        returns: ApiType::Table,
        doc: "A { world_name, save_id, player_count, guild_count, pal_count } summary of the \
              loaded save.",
        capability: None,
    },
    ApiFunction {
        name: "players",
        params: &[],
        returns: ApiType::Iterator("player"),
        doc: "An iterator over every player in the save, for use in a `for` loop.",
        capability: None,
    },
    ApiFunction {
        name: "pals",
        params: &[],
        returns: ApiType::Iterator("pal"),
        doc: "An iterator over every pal in the save, for use in a `for` loop. Building it \
              walks every character entry once, so calling this repeatedly in a loop is \
              needlessly expensive -- call it once and reuse the iterator.",
        capability: None,
    },
    ApiFunction {
        name: "guilds",
        params: &[],
        returns: ApiType::Iterator("guild"),
        doc: "An iterator over every guild in the save, for use in a `for` loop.",
        capability: None,
    },
    ApiFunction {
        name: "bases",
        params: &[],
        returns: ApiType::Iterator("base"),
        doc: "An iterator over every guild base in the save, for use in a `for` loop.",
        capability: None,
    },
    ApiFunction {
        name: "containers",
        params: &[],
        returns: ApiType::Iterator("container"),
        doc: "An iterator over every item container in the save, for use in a `for` loop.",
        capability: None,
    },
];

const SAVE_READ_PUSH_FNS: [PushHostFn; SAVE_READ_FUNCTIONS.len()] =
    [push_info, push_players, push_pals, push_guilds, push_bases, push_containers];

fn save_read_bindings() -> [(&'static str, PushHostFn); SAVE_READ_FUNCTIONS.len()] {
    std::array::from_fn(|i| (SAVE_READ_FUNCTIONS[i].name, SAVE_READ_PUSH_FNS[i]))
}

/// Must run before `save_write::install`, which extends what this creates.
pub unsafe fn install(state: *mut lua_State) {
    install_iter_metatable(state, DeleteWhereKind::Player);
    install_iter_metatable(state, DeleteWhereKind::Guild);
    install_iter_metatable(state, DeleteWhereKind::Pal);
    register_table(state, "save", &save_read_bindings());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_names_match_described_functions_in_order() {
        let bindings = save_read_bindings();
        let bound_names: Vec<&str> = bindings.iter().map(|(name, _)| *name).collect();
        let described_names: Vec<&str> = SAVE_READ_FUNCTIONS.iter().map(|f| f.name).collect();
        assert_eq!(bound_names, described_names);
    }
}
