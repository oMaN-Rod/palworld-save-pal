use std::collections::BTreeMap;

use psp_core::domain::{containers, guild, guild_tail, pal, player, world};
use psp_core::dto::guild::{BaseDto, GuildDto};
use psp_core::dto::ordered_map::OrderedMap;
use psp_core::dto::pal::PalDto;
use psp_core::dto::player::PlayerDto;
use psp_core::error::CoreError;
use psp_core::progress::{null_progress, ProgressSink};
use psp_core::props;
use psp_core::ue::{MapEntry, PalStruct, Property, StructValue};
use uuid::Uuid;

use super::HostError;
use crate::context::RunContext;

fn core_error(error: CoreError) -> HostError {
    HostError::new(error.to_string())
}

pub(crate) struct CachedPal {
    dto: PalDto,
    dirty: bool,
    /// Names of the fields a `pal_write` call has actually touched this run,
    /// not yet flushed. `pal_index` checks this before falling back to the
    /// pal summary: the summary only ever reflects a real flush's write (and
    /// never a dry run's, which never flushes at all), so a field recorded
    /// here must be served from this cached DTO directly instead.
    written: Vec<&'static str>,
}

/// `pub`, not `pub(crate)`: `RunContext` is `pub` and its fields (this one
/// included) are set by name in the test harness, a separate crate. The type
/// itself never needs to be nameable there -- `Default::default()` resolves
/// by inference -- but leaving it `pub(crate)` trips the warn-by-default
/// `private_interfaces` lint on `RunContext::dto_cache`, which is `pub`.
#[derive(Default)]
pub struct DtoCache {
    pal: BTreeMap<Uuid, CachedPal>,
    player: BTreeMap<Uuid, CachedPlayer>,
    guild: BTreeMap<Uuid, CachedGuild>,
    base: BTreeMap<Uuid, CachedBase>,
}

pub(crate) struct CachedPlayer {
    dto: PlayerDto,
    dirty: bool,
    /// Names of the fields a `player_write` call has actually touched this
    /// run, not yet flushed, read by `player_index` for the same reason
    /// `CachedPal::written` is -- with one extra: the player summary is not a
    /// run-scoped cache that a write can simply drop and have rebuilt. It is
    /// session state built once when the save was opened, and nothing
    /// recomputes it, so a field served from it would answer with the
    /// pre-write value for the rest of the run.
    written: Vec<&'static str>,
    /// The stat keys the save already carried a row for when this DTO was
    /// loaded, for `GotStatusPointList` and `GotExStatusPointList` in turn.
    ///
    /// `apply_status_points` can overwrite a row and can append one for a
    /// positive value, but it can never remove one -- so a zero lands in the
    /// save only where a row already exists, and is dropped on the floor
    /// everywhere else. Deciding which of the two happened needs the key set as
    /// the *save* has it, which the DTO stops being able to answer the moment a
    /// write edits it: an assignment that adds a rowless key at a positive
    /// value, followed by one that zeroes it, would otherwise look exactly like
    /// zeroing a key that has a row. Captured here at load, and gone when the
    /// flush drains the entry, so the next load recaptures from the save.
    saved_status_rows: Vec<String>,
    saved_ext_status_rows: Vec<String>,
}

/// `CharacterSaveParameterMap` entry id -> position, built once per run and
/// invalidated (`RunContext::note_mutation`) whenever a structural write can
/// have reordered or removed entries.
pub(crate) fn pal_entry_index<'ctx>(
    ctx: &'ctx mut RunContext<'_>,
) -> Result<&'ctx BTreeMap<Uuid, usize>, HostError> {
    if ctx.pal_entry_index.is_none() {
        let entries = world::character_map(&ctx.session.level).map_err(core_error)?;
        let mut index = BTreeMap::new();
        for (position, entry) in entries.iter().enumerate() {
            if world::entry_is_player(entry) {
                continue;
            }
            if let Some(id) = world::entry_instance_id(entry) {
                index.insert(id, position);
            }
        }
        ctx.pal_entry_index = Some(index);
        ctx.dto_index_build_count = ctx.dto_index_build_count.saturating_add(1);
    }
    ctx.pal_entry_index
        .as_ref()
        .ok_or_else(|| HostError::new("pal entry index missing after build"))
}

fn load_pal_dto(ctx: &mut RunContext<'_>, id: Uuid) -> Result<PalDto, HostError> {
    let position = *pal_entry_index(ctx)?
        .get(&id)
        .ok_or_else(|| HostError::new(format!("pal {id} not found")))?;
    let entries = world::character_map(&ctx.session.level).map_err(core_error)?;
    let entry = entries
        .get(position)
        .ok_or_else(|| HostError::new(format!("pal {id} not found")))?;
    pal::pal_dto_from_entry(entry, ctx.game_data).ok_or_else(|| HostError::new(format!("pal {id} not found")))
}

fn write_pal_dto(ctx: &mut RunContext<'_>, id: Uuid, dto: &PalDto) -> Result<(), HostError> {
    let position = *pal_entry_index(ctx)?
        .get(&id)
        .ok_or_else(|| HostError::new(format!("pal {id} not found")))?;
    let entries_mut = world::character_map_mut(&mut ctx.session.level).map_err(core_error)?;
    let entry_mut = entries_mut
        .get_mut(position)
        .ok_or_else(|| HostError::new(format!("pal {id} not found")))?;
    let save_parameter = world::entry_save_parameter_mut(entry_mut)
        .ok_or_else(|| HostError::new("pal save parameter missing"))?;
    pal::apply_pal_dto(save_parameter, dto, false, ctx.game_data);
    Ok(())
}

/// Parses and caches on first access; a later call for the same id returns the
/// cached (possibly dirty) copy instead of re-parsing.
pub(crate) fn pal_read<'ctx>(ctx: &'ctx mut RunContext<'_>, id: Uuid) -> Result<&'ctx PalDto, HostError> {
    if !ctx.dto_cache.pal.contains_key(&id) {
        let dto = load_pal_dto(ctx, id)?;
        ctx.dto_cache.pal.insert(id, CachedPal { dto, dirty: false, written: Vec::new() });
    }
    ctx.dto_cache
        .pal
        .get(&id)
        .map(|cached| &cached.dto)
        .ok_or_else(|| HostError::new(format!("pal {id} not found")))
}

/// Mutates the cached DTO and marks it dirty; the write only lands on the save
/// at the next `flush`. Also drops `ctx.pals` (and `ctx.container`) itself,
/// the same way the two existing setters used to do after calling this --
/// folded in here so a later caller that forgets it cannot read stale data.
///
/// `fields` names every field `f` actually touches, so `pal_index` can serve
/// exactly those fields from this cached DTO on a later read this run,
/// without waiting for a flush that a dry run will never perform. Usually one
/// name; `is_lucky`'s writer passes two, since demoting a lucky pal can
/// rewrite `character_id` too.
pub(crate) fn pal_write(
    ctx: &mut RunContext<'_>,
    id: Uuid,
    fields: &[&'static str],
    f: impl FnOnce(&mut PalDto),
) -> Result<(), HostError> {
    pal_read(ctx, id)?;
    let Some(cached) = ctx.dto_cache.pal.get_mut(&id) else {
        return Err(HostError::new(format!("pal {id} not found")));
    };
    f(&mut cached.dto);
    cached.dirty = true;
    for field in fields {
        if !cached.written.contains(field) {
            cached.written.push(field);
        }
    }
    ctx.note_pal_field_write();
    Ok(())
}

/// Drops `field`'s pending-write claim without touching the DTO. The claim is
/// only ever right while the cached value already equals what a flush would
/// write; a later write that breaks that for a field it does not itself
/// rewrite has to release it, since `written` is cumulative for the run and
/// simply not re-adding the name leaves the earlier claim standing.
pub(crate) fn pal_release_field(ctx: &mut RunContext<'_>, id: Uuid, field: &str) {
    if let Some(cached) = ctx.dto_cache.pal.get_mut(&id) {
        cached.written.retain(|written| *written != field);
    }
}

/// Loads a player's full DTO, which is also what puts the player into
/// `session.loaded_players` -- the precondition `apply_player_dto` errors on,
/// and the reason this cache is a load-then-apply pair rather than the plain
/// read-modify-write the pal cache gets away with. The load is real disk I/O
/// (the player's own `.sav`, and their `.dps` if one exists), which is why the
/// summary answers every row it can without coming through here.
///
/// The five item-container DTOs are dropped the moment the DTO is cached,
/// before anything can write through them. `apply_player_dto` routes a
/// populated one into `apply_item_container_dto`, which removes raw slot
/// entries and -- for the essential container -- resizes the paired common
/// container to a size computed from scratch. Both rewrite containers
/// underneath handles and iterators that have no way to know, so the write has
/// to be structural or the containers have to be out of it. Nulling here makes
/// it the second, and makes it impossible for a caller to opt back in by
/// forgetting to.
fn load_player_dto(ctx: &mut RunContext<'_>, uid: Uuid) -> Result<PlayerDto, HostError> {
    let fallback = null_progress();
    let progress: &ProgressSink = ctx.progress.unwrap_or(&fallback);
    let mut dto = player::get_player_details(ctx.session, ctx.game_data, uid, progress)
        .map_err(core_error)?
        .ok_or_else(|| HostError::new(format!("player {uid} not found")))?;
    drop_item_container_dtos(&mut dto);
    Ok(dto)
}

/// The five `Option<ItemContainerDto>` fields, and only those: they are exactly
/// the fields `apply_player_dto` routes into `apply_item_container_dto`.
/// `pal_box` and `party` are character containers it never touches, and the
/// remaining container-ish fields (`pal_box_id`, `otomo_container_id`) are bare
/// ids it never writes at all.
///
/// Destructured exhaustively, with no `..`, so a field added to `PlayerDto`
/// later stops compiling here rather than silently arriving in the cache. A
/// sixth item container would otherwise make every player write structural
/// again with nothing to notice: the test that enumerates these five by name
/// can only ever enumerate the five that existed when it was written.
fn drop_item_container_dtos(dto: &mut PlayerDto) {
    let PlayerDto {
        common_container,
        essential_container,
        weapon_load_out_container,
        player_equipment_armor_container,
        food_equip_container,
        pals: _,
        pal_box: _,
        party: _,
        guild_id: _,
        uid: _,
        instance_id: _,
        nickname: _,
        level: _,
        technologies: _,
        technology_points: _,
        boss_technology_points: _,
        exp: _,
        hp: _,
        stomach: _,
        sanity: _,
        status_point_list: _,
        ext_status_point_list: _,
        pal_box_id: _,
        otomo_container_id: _,
        completed_missions: _,
        current_missions: _,
        unlocked_fast_travel_points: _,
        collected_effigies: _,
        collected_relics: _,
        defeated_bosses: _,
        effigy_possess_num: _,
        location: _,
        last_online_time: _,
        dps: _,
    } = dto;
    *common_container = None;
    *essential_container = None;
    *weapon_load_out_container = None;
    *player_equipment_armor_container = None;
    *food_equip_container = None;
}

/// Parses and caches on first access; a later call for the same player returns
/// the cached (possibly dirty) copy instead of re-reading their `.sav`.
pub(crate) fn player_read<'ctx>(
    ctx: &'ctx mut RunContext<'_>,
    uid: Uuid,
) -> Result<&'ctx PlayerDto, HostError> {
    if !ctx.dto_cache.player.contains_key(&uid) {
        let dto = load_player_dto(ctx, uid)?;
        let saved_status_rows = dto.status_point_list.iter().map(|(key, _)| key.clone()).collect();
        let saved_ext_status_rows = dto.ext_status_point_list.iter().map(|(key, _)| key.clone()).collect();
        ctx.dto_cache.player.insert(
            uid,
            CachedPlayer { dto, dirty: false, written: Vec::new(), saved_status_rows, saved_ext_status_rows },
        );
    }
    ctx.dto_cache
        .player
        .get(&uid)
        .map(|cached| &cached.dto)
        .ok_or_else(|| HostError::new(format!("player {uid} not found")))
}

/// Mutates the cached DTO and marks it dirty; the write only lands on the save
/// at the next `flush`. Non-structural by construction (see
/// `load_player_dto`), so handles and iterators stay valid and the mutation
/// epoch does not move.
pub(crate) fn player_write(
    ctx: &mut RunContext<'_>,
    uid: Uuid,
    fields: &[&'static str],
    f: impl FnOnce(&mut PlayerDto),
) -> Result<(), HostError> {
    player_read(ctx, uid)?;
    let Some(cached) = ctx.dto_cache.player.get_mut(&uid) else {
        return Err(HostError::new(format!("player {uid} not found")));
    };
    f(&mut cached.dto);
    cached.dirty = true;
    for field in fields {
        if !cached.written.contains(field) {
            cached.written.push(field);
        }
    }
    ctx.note_write();
    Ok(())
}

/// Whether `field` has a pending, not-yet-flushed write for this player this
/// run.
pub(crate) fn player_field_was_written(ctx: &RunContext<'_>, uid: Uuid, field: &str) -> bool {
    ctx.dto_cache.player.get(&uid).is_some_and(|cached| cached.written.iter().any(|written| *written == field))
}

/// The stat keys `field`'s list had a row for in the save this run started
/// from (see `CachedPlayer::saved_status_rows`). Empty for a player not yet
/// cached, which is also the right answer: nothing can have been written for
/// one, so no caller needs the distinction yet.
pub(crate) fn player_saved_status_rows<'ctx>(
    ctx: &'ctx RunContext<'_>,
    uid: Uuid,
    field: &str,
) -> &'ctx [String] {
    let Some(cached) = ctx.dto_cache.player.get(&uid) else {
        return &[];
    };
    match field {
        "status_point_list" => &cached.saved_status_rows,
        "ext_status_point_list" => &cached.saved_ext_status_rows,
        _ => &[],
    }
}

fn write_player_dto(ctx: &mut RunContext<'_>, uid: Uuid, dto: PlayerDto) -> Result<(), HostError> {
    let fallback = null_progress();
    let progress: &ProgressSink = ctx.progress.unwrap_or(&fallback);
    let mut modified = OrderedMap::new();
    modified.insert(uid, dto);
    player::update_players(ctx.session, ctx.game_data, &modified, progress).map_err(core_error)
}

/// The player summary is session state, not a run-scoped cache: nothing
/// recomputes it after a write, so the two rows a write can move are carried
/// across by hand, exactly as the setter this replaced did for `level`. Only
/// the rows actually written are copied -- `name` in particular is only safe to
/// copy because its own validation refuses the two strings whose summary
/// spelling differs from the DTO's.
fn refresh_player_summary(
    ctx: &mut RunContext<'_>,
    uid: Uuid,
    written: &[&'static str],
    nickname: String,
    level: i64,
) {
    let Some(summary) = ctx.session.player_summaries.get_mut(&uid) else {
        return;
    };
    if written.contains(&"name") {
        summary.nickname = nickname;
    }
    if written.contains(&"level") {
        summary.level = Some(level);
    }
}

fn flush_players(ctx: &mut RunContext<'_>) -> Result<usize, HostError> {
    let cache = std::mem::take(&mut ctx.dto_cache.player);
    if cache.is_empty() {
        return Ok(0);
    }
    let mut written_count = 0usize;
    let mut first_error: Option<HostError> = None;
    for (uid, cached) in cache {
        if !cached.dirty {
            continue;
        }
        let nickname = cached.dto.nickname.clone();
        let level = cached.dto.level;
        match write_player_dto(ctx, uid, cached.dto) {
            Ok(()) => {
                refresh_player_summary(ctx, uid, &cached.written, nickname, level);
                ctx.dto_flush_count = ctx.dto_flush_count.saturating_add(1);
                written_count += 1;
            }
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }
    if let Some(error) = first_error {
        return Err(error);
    }
    Ok(written_count)
}

pub(crate) struct CachedGuild {
    dto: GuildDto,
    dirty: bool,
    /// Names of the fields a `guild_write` call has actually touched this run,
    /// not yet flushed. Unlike the pal and player caches, nothing on the read
    /// path consults this: both writable guild rows read the cached DTO
    /// whether or not they have been written. It is kept because the flush
    /// needs it -- the guild summary the app goes on reading is session state
    /// nothing recomputes, so exactly the rows a write moved have to be
    /// carried across into it by hand.
    written: Vec<&'static str>,
}

/// No `written` list, unlike the other three caches. Those record it so a
/// later read this run can be routed away from a stale summary; the base
/// handle has no summary at all, and both of its writable rows read this DTO
/// whether or not they have been written, so there is nothing for such a list
/// to decide.
pub(crate) struct CachedBase {
    dto: BaseDto,
    dirty: bool,
}

/// The guild's name and base-camp level as the *save* holds them, read
/// straight out of the guild tail rather than out of `session.guild_summaries`.
///
/// That distinction is the whole reason this exists. The summary is session
/// state built when the save was opened; sourcing the cache from it would make
/// a read after a flush answer with whatever this run believed it wrote, even
/// if the write never reached the save at all.
fn guild_tail_name_and_level(ctx: &RunContext<'_>, id: Uuid) -> Option<(String, i32)> {
    let entries = ctx.session.group_map().ok()?;
    let entry = entries.iter().find(|entry| props::as_uuid(&entry.key) == Some(id))?;
    let guild = guild_tail::as_guild(guild_tail::entry_group_data(entry)?)?;
    Some((guild.guild_name.clone(), guild.base_camp_level))
}

/// Builds the guild DTO this cache holds, and the only `GuildDto` a write ever
/// reaches `update_guilds` with.
///
/// `bases` and `guild_chest` are `None` here and are never populated
/// afterwards. `apply_guild_dto` routes a populated `bases` into
/// `apply_base_dto` for every base -- which in turn rewrites each base's
/// storage containers -- and a populated `guild_chest` into
/// `apply_item_container_dto`. Both rewrite containers underneath handles and
/// iterators that have no way to know, so a guild write would have to be
/// structural or the two have to be out of it. Building the DTO here rather
/// than fetching one from `get_guild_details` -- which always returns
/// `bases: Some(..)` -- makes it the second, and leaves no populated field for
/// a later caller to forget to clear.
///
/// `container_id` is the one output-only field that is filled in, because the
/// `chest_container_id` row reads it; `apply_guild_dto` never looks at it.
fn load_guild_dto(ctx: &mut RunContext<'_>, id: Uuid) -> Result<GuildDto, HostError> {
    // Raises where the base loader degrades to nil, and the difference is
    // reachability rather than taste. `bases_next` hands out a handle for any
    // uuid-keyed `BaseCampSaveData` entry, so an unreadable base is reachable
    // under `save.read` alone on a save no plugin has touched. A guild handle
    // can only come from `guild_summary_order`, which `build_guild_summaries`
    // fills only with guilds whose tail already decoded, and which nothing
    // afterwards adds to -- `psp-plugin/src/host/save_write.rs:49` only
    // removes from it and `psp-core/src/domain/summaries.rs:344` is the sole
    // assignment. So the only way here is a `raw` write that breaks the guild
    // tail mid-run, which needs `save.raw`; reporting that rather than
    // answering nil is the more useful answer to a plugin that just did it.
    let (name, base_camp_level) = guild_tail_name_and_level(ctx, id).ok_or_else(|| {
        HostError::new(format!("guild {id} has no readable guild record in the save"))
    })?;
    Ok(GuildDto {
        bases: None,
        guild_chest: None,
        lab_research: None,
        name: Some(name),
        base_camp_level: Some(base_camp_level),
        id: Some(id),
        admin_player_uid: None,
        players: Vec::new(),
        container_id: guild::guild_chest_id(ctx.session, id),
        lab_research_data: Vec::new(),
    })
}

/// Parses and caches on first access; a later call for the same guild returns
/// the cached (possibly dirty) copy instead of re-reading the guild tail.
pub(crate) fn guild_read<'ctx>(
    ctx: &'ctx mut RunContext<'_>,
    id: Uuid,
) -> Result<&'ctx GuildDto, HostError> {
    if !ctx.dto_cache.guild.contains_key(&id) {
        let dto = load_guild_dto(ctx, id)?;
        ctx.dto_cache.guild.insert(id, CachedGuild { dto, dirty: false, written: Vec::new() });
    }
    ctx.dto_cache
        .guild
        .get(&id)
        .map(|cached| &cached.dto)
        .ok_or_else(|| HostError::new(format!("guild {id} not found")))
}

/// Mutates the cached DTO and marks it dirty; the write only lands on the save
/// at the next `flush`. Non-structural by construction (see `load_guild_dto`),
/// so handles and iterators stay valid and the mutation epoch does not move.
pub(crate) fn guild_write(
    ctx: &mut RunContext<'_>,
    id: Uuid,
    fields: &[&'static str],
    f: impl FnOnce(&mut GuildDto),
) -> Result<(), HostError> {
    guild_read(ctx, id)?;
    let Some(cached) = ctx.dto_cache.guild.get_mut(&id) else {
        return Err(HostError::new(format!("guild {id} not found")));
    };
    f(&mut cached.dto);
    cached.dirty = true;
    for field in fields {
        if !cached.written.contains(field) {
            cached.written.push(field);
        }
    }
    ctx.note_write();
    Ok(())
}

/// The only `GuildDto` that ever reaches `update_guilds`, rebuilt field by
/// field from the cached one rather than handed over whole.
///
/// `bases` and `guild_chest` are the reason this exists rather than being an
/// extra copy. `apply_guild_dto` routes a populated `bases` into
/// `apply_base_dto` for every base -- which in turn rewrites each base's
/// storage containers -- and a populated `guild_chest` into
/// `apply_item_container_dto`. Both rewrite containers underneath handles and
/// iterators that have no way to know, so a guild write would have to be
/// structural or the two have to be out of it. `load_guild_dto` already
/// declines to fetch them; nulling them again here is what makes it a property
/// of the write rather than of one loader, so a later change to what the cache
/// holds cannot quietly turn it back on.
///
/// Destructured exhaustively, with no `..`, so a field added to `GuildDto`
/// later stops compiling here rather than silently arriving in the write.
fn guild_write_payload(cached: &GuildDto) -> GuildDto {
    let GuildDto {
        bases: _,
        guild_chest: _,
        lab_research: _,
        name,
        base_camp_level,
        id,
        admin_player_uid: _,
        players: _,
        container_id: _,
        lab_research_data: _,
    } = cached;
    GuildDto {
        bases: None,
        guild_chest: None,
        lab_research: None,
        name: name.clone(),
        base_camp_level: *base_camp_level,
        id: *id,
        admin_player_uid: None,
        players: Vec::new(),
        container_id: None,
        lab_research_data: Vec::new(),
    }
}

fn write_guild_dto(ctx: &mut RunContext<'_>, id: Uuid, dto: &GuildDto) -> Result<(), HostError> {
    let fallback = null_progress();
    let progress: &ProgressSink = ctx.progress.unwrap_or(&fallback);
    let mut modified = OrderedMap::new();
    modified.insert(id, guild_write_payload(dto));
    guild::update_guilds(ctx.session, ctx.game_data, &modified, progress).map_err(core_error)
}

/// The guild summary is session state, not a run-scoped cache: nothing
/// recomputes it after a write, so the two rows a write can move are carried
/// across by hand. The handle's own reads do not depend on this -- they come
/// from the guild tail -- but everything outside the run that reads the summary
/// does.
fn refresh_guild_summary(
    ctx: &mut RunContext<'_>,
    id: Uuid,
    written: &[&'static str],
    name: Option<String>,
    level: Option<i32>,
) {
    let Some(summary) = ctx.session.guild_summaries.get_mut(&id) else {
        return;
    };
    if written.contains(&"name") {
        if let Some(name) = name {
            summary.name = name;
        }
    }
    if written.contains(&"level") {
        if let Some(level) = level {
            summary.level = Some(i64::from(level));
        }
    }
}

fn flush_guilds(ctx: &mut RunContext<'_>) -> Result<usize, HostError> {
    let cache = std::mem::take(&mut ctx.dto_cache.guild);
    if cache.is_empty() {
        return Ok(0);
    }
    let mut written_count = 0usize;
    let mut first_error: Option<HostError> = None;
    for (id, cached) in cache {
        if !cached.dirty {
            continue;
        }
        let name = cached.dto.name.clone();
        let level = cached.dto.base_camp_level;
        match write_guild_dto(ctx, id, &cached.dto) {
            Ok(()) => {
                refresh_guild_summary(ctx, id, &cached.written, name, level);
                ctx.dto_flush_count = ctx.dto_flush_count.saturating_add(1);
                written_count += 1;
            }
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }
    if let Some(error) = first_error {
        return Err(error);
    }
    Ok(written_count)
}

/// The base's name and working radius as the save holds them, read straight
/// off its `BaseCampSaveData` entry. `None` for an entry whose `RawData` is
/// not a base-camp record at all -- `bases_next` hands out a handle for any
/// uuid-keyed entry without checking, so that entry is reachable. It is also
/// the case `apply_base_dto` cannot write.
fn base_camp_name_and_area(entry: &MapEntry) -> Option<(String, f64)> {
    let value_props = props::struct_props(&entry.value)?;
    let Property::Struct(StructValue::Game(PalStruct::BaseCamp(base_camp))) =
        props::get(value_props, &["RawData"])?
    else {
        return None;
    };
    Some((base_camp.name.clone(), f64::from(base_camp.area_range)))
}

/// Builds the base DTO this cache holds, and the only `BaseDto` a write ever
/// reaches `apply_base_dto` with.
///
/// `storage_containers` is empty here and is never populated afterwards, for
/// the reason `load_guild_dto` gives: `apply_base_dto` routes a populated one
/// into `apply_item_container_dto`, which rewrites containers underneath
/// handles and iterators that have no way to know. `pals` and `pal_container`
/// are left empty and `location` `None` because `apply_base_dto` never reads
/// any of them -- `BaseDto::location` is output-only, which is also why this
/// handle's `x`/`y`/`z` are read-only rows.
fn load_base_dto(ctx: &RunContext<'_>, id: Uuid) -> Result<BaseDto, HostError> {
    let entries = ctx.session.base_camp_map().unwrap_or(&[]);
    let entry = entries
        .iter()
        .find(|entry| props::as_uuid(&entry.key) == Some(id))
        .ok_or_else(|| HostError::new(format!("base {id} not found")))?;
    // Degraded to a pair of `None`s rather than an error, matching what
    // `build_guild_dto` answers for the same entry and what this handle's
    // `x`/`y`/`z` already answer for it: `base_camp_location` returns `None`
    // too, so raising for two rows while answering nil for three on the same
    // object would be an inconsistency no plugin author could predict. The
    // write side refuses instead -- see `base_camp_record_exists`.
    let (name, area_range) = match base_camp_name_and_area(entry) {
        Some((name, area_range)) => (Some(name), Some(area_range)),
        None => (None, None),
    };
    Ok(BaseDto {
        pals: OrderedMap::new(),
        container_id: None,
        slot_count: None,
        storage_containers: OrderedMap::new(),
        pal_container: None,
        id,
        name,
        location: None,
        area_range,
    })
}

/// Whether the save carries a base-camp record for `id` that
/// `apply_base_dto` could actually write into.
///
/// Read from the save rather than from the cached DTO, and deliberately so: a
/// cached `name` of `None` stops meaning "no record" the moment a write puts a
/// name there, so answering from the cache would let a second assignment slip
/// through where the first was refused.
pub(crate) fn base_camp_record_exists(ctx: &RunContext<'_>, id: Uuid) -> bool {
    ctx.session
        .base_camp_map()
        .unwrap_or(&[])
        .iter()
        .find(|entry| props::as_uuid(&entry.key) == Some(id))
        .and_then(base_camp_name_and_area)
        .is_some()
}

/// Parses and caches on first access; a later call for the same base returns
/// the cached (possibly dirty) copy instead of re-reading the entry.
pub(crate) fn base_read<'ctx>(
    ctx: &'ctx mut RunContext<'_>,
    id: Uuid,
) -> Result<&'ctx BaseDto, HostError> {
    if !ctx.dto_cache.base.contains_key(&id) {
        let dto = load_base_dto(ctx, id)?;
        ctx.dto_cache.base.insert(id, CachedBase { dto, dirty: false });
    }
    ctx.dto_cache
        .base
        .get(&id)
        .map(|cached| &cached.dto)
        .ok_or_else(|| HostError::new(format!("base {id} not found")))
}

/// Mutates the cached DTO and marks it dirty; the write only lands on the save
/// at the next `flush`. Non-structural by construction (see `load_base_dto`),
/// so handles and iterators stay valid and the mutation epoch does not move.
pub(crate) fn base_write(
    ctx: &mut RunContext<'_>,
    id: Uuid,
    f: impl FnOnce(&mut BaseDto),
) -> Result<(), HostError> {
    base_read(ctx, id)?;
    let Some(cached) = ctx.dto_cache.base.get_mut(&id) else {
        return Err(HostError::new(format!("base {id} not found")));
    };
    f(&mut cached.dto);
    cached.dirty = true;
    ctx.note_write();
    Ok(())
}

/// The only `BaseDto` that ever reaches `apply_base_dto`, rebuilt field by
/// field from the cached one for the reason `guild_write_payload` gives.
/// `storage_containers` is the field that matters: `apply_base_dto` routes a
/// populated one into `apply_item_container_dto` for every container the base
/// owns. `pals`, `container_id`, `slot_count`, `pal_container` and `location`
/// are output-only -- `apply_base_dto` never reads any of them -- and
/// `location` in particular is why this handle's `x`/`y`/`z` are read-only
/// rows and not writable ones.
///
/// Destructured exhaustively, with no `..`, so a field added to `BaseDto`
/// later stops compiling here rather than silently arriving in the write.
fn base_write_payload(cached: &BaseDto) -> BaseDto {
    let BaseDto {
        pals: _,
        container_id: _,
        slot_count: _,
        storage_containers: _,
        pal_container: _,
        id,
        name,
        location: _,
        area_range,
    } = cached;
    BaseDto {
        pals: OrderedMap::new(),
        container_id: None,
        slot_count: None,
        storage_containers: OrderedMap::new(),
        pal_container: None,
        id: *id,
        name: name.clone(),
        location: None,
        area_range: *area_range,
    }
}

fn flush_bases(ctx: &mut RunContext<'_>) -> Result<usize, HostError> {
    let cache = std::mem::take(&mut ctx.dto_cache.base);
    if cache.is_empty() {
        return Ok(0);
    }
    let mut written_count = 0usize;
    let mut first_error: Option<HostError> = None;
    for (id, cached) in cache {
        if !cached.dirty {
            continue;
        }
        match containers::apply_base_dto(ctx.session, id, &base_write_payload(&cached.dto))
            .map_err(core_error)
        {
            Ok(()) => {
                ctx.dto_flush_count = ctx.dto_flush_count.saturating_add(1);
                written_count += 1;
            }
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }
    if let Some(error) = first_error {
        return Err(error);
    }
    Ok(written_count)
}

/// Whether `field` has a pending, not-yet-flushed write for this pal this
/// run. `false` once the entry has flushed (real run) or was never written
/// (both runs) -- either way the pal summary is the right source then.
pub(crate) fn pal_field_was_written(ctx: &RunContext<'_>, id: Uuid, field: &str) -> bool {
    ctx.dto_cache.pal.get(&id).is_some_and(|cached| cached.written.iter().any(|written| *written == field))
}

/// Writes every dirty pal DTO back to the save, returning how many were
/// actually written. Under a dry run nothing is written -- and the cache is
/// left untouched rather than drained, so a later read in the same dry run
/// still sees this run's own pending writes instead of the original value.
///
/// Drains the whole map rather than just clearing dirty flags. `pal_read`
/// has exactly one caller today (`pal_write`), so no clean entry currently
/// survives to reach a flush -- but the full drain is the right choice
/// regardless, both because it is no more expensive than clearing flags in
/// place, and because it stays correct the moment a later task gives
/// `pal_read` a read-only caller: a cached-but-clean DTO left behind would
/// then be a stale copy of whatever a non-cache writer (`raw.*`, or any
/// future one) put in the save directly between the read and the next flush.
///
/// Attempts every dirty entry even if one fails to write: a mid-loop `?`
/// would drop every remaining entry along with the already-drained map,
/// turning one bad write into several silently lost ones -- exactly the
/// failure mode this cache exists to prevent. The first error is reported
/// after every entry has had its chance.
///
/// Every half is attempted for the same reason a failing pal does not abort
/// the rest: whichever ran first has already drained its map, so bailing out
/// would discard the others' pending writes rather than merely deferring them.
pub(crate) fn flush(ctx: &mut RunContext<'_>) -> Result<usize, HostError> {
    if ctx.dry_run {
        return Ok(0);
    }
    let halves = [flush_pals(ctx), flush_players(ctx), flush_guilds(ctx), flush_bases(ctx)];
    let mut written = 0usize;
    let mut first_error: Option<HostError> = None;
    for half in halves {
        match half {
            Ok(count) => written = written.saturating_add(count),
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }
    match first_error {
        Some(error) => Err(error),
        None => Ok(written),
    }
}

fn flush_pals(ctx: &mut RunContext<'_>) -> Result<usize, HostError> {
    let cache = std::mem::take(&mut ctx.dto_cache.pal);
    if cache.is_empty() {
        return Ok(0);
    }
    let mut written = 0usize;
    let mut first_error: Option<HostError> = None;
    for (id, cached) in cache {
        if !cached.dirty {
            continue;
        }
        match write_pal_dto(ctx, id, &cached.dto) {
            Ok(()) => {
                ctx.dto_flush_count = ctx.dto_flush_count.saturating_add(1);
                written += 1;
            }
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }
    if let Some(error) = first_error {
        return Err(error);
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Enumerates all five by name rather than trusting one representative:
    /// leaving any single one populated is enough to turn a player write into a
    /// container rewrite, and four of the five leave no trace on a save whose
    /// containers already agree with what the write would recompute.
    #[test]
    fn every_item_container_is_dropped_from_a_cached_player_dto() {
        let container = serde_json::json!({
            "id": "11111111-2222-3333-4444-555555555555",
            "type": "CommonContainer",
            "slots": [],
            "slot_num": 48,
        });
        let mut dto: PlayerDto = serde_json::from_value(serde_json::json!({
            "uid": "99999999-2222-3333-4444-555555555555",
            "nickname": "Tester",
            "level": 1,
            "exp": 0,
            "common_container": container,
            "essential_container": container,
            "weapon_load_out_container": container,
            "player_equipment_armor_container": container,
            "food_equip_container": container,
        }))
        .expect("the fixture payload must deserialize");

        drop_item_container_dtos(&mut dto);

        for (name, present) in [
            ("common_container", dto.common_container.is_some()),
            ("essential_container", dto.essential_container.is_some()),
            ("weapon_load_out_container", dto.weapon_load_out_container.is_some()),
            ("player_equipment_armor_container", dto.player_equipment_armor_container.is_some()),
            ("food_equip_container", dto.food_equip_container.is_some()),
        ] {
            assert!(!present, "{name} still reaches update_players, so a player write is structural");
        }
    }
    /// Both by name rather than one representative: leaving either populated
    /// is enough to turn a guild write into a rewrite of containers no handle
    /// or iterator has any way to know moved.
    ///
    /// Deliberately built from a DTO that carries both, rather than from the
    /// one the cache actually loads: what has to hold is that the write drops
    /// them, not that the loader happened not to fetch them.
    #[test]
    fn a_guild_write_carries_neither_the_guilds_bases_nor_its_chest() {
        let cached: GuildDto = serde_json::from_value(serde_json::json!({
            "name": "Tester",
            "base_camp_level": 4,
            "id": "99999999-2222-3333-4444-555555555555",
            "bases": {
                "11111111-2222-3333-4444-555555555555": {
                    "id": "11111111-2222-3333-4444-555555555555",
                    "storage_containers": {},
                },
            },
            "guild_chest": {
                "id": "22222222-2222-3333-4444-555555555555",
                "type": "GuildChest",
                "slots": [],
                "slot_num": 40,
            },
        }))
        .expect("the fixture payload must deserialize");
        assert!(cached.bases.is_some() && cached.guild_chest.is_some(), "the fixture must carry both");

        let payload = guild_write_payload(&cached);

        assert!(payload.bases.is_none(), "bases still reaches apply_guild_dto, which rewrites every base");
        assert!(
            payload.guild_chest.is_none(),
            "guild_chest still reaches apply_guild_dto, which rewrites the chest container"
        );
        assert_eq!(payload.name.as_deref(), Some("Tester"), "the write must still carry the name");
        assert_eq!(payload.base_camp_level, Some(4), "the write must still carry the level");
    }

    /// The base half of the same property: `storage_containers` is the one
    /// field `apply_base_dto` walks into a container rewrite.
    #[test]
    fn a_base_write_carries_none_of_the_bases_storage_containers() {
        let cached: BaseDto = serde_json::from_value(serde_json::json!({
            "id": "11111111-2222-3333-4444-555555555555",
            "name": "Tester",
            "area_range": 3500.0,
            "storage_containers": {
                "22222222-2222-3333-4444-555555555555": {
                    "id": "22222222-2222-3333-4444-555555555555",
                    "type": "BaseContainer",
                    "slots": [],
                    "slot_num": 6,
                },
            },
        }))
        .expect("the fixture payload must deserialize");
        assert!(!cached.storage_containers.is_empty(), "the fixture must carry a container");

        let payload = base_write_payload(&cached);

        assert!(
            payload.storage_containers.is_empty(),
            "a storage container still reaches apply_base_dto, which rewrites its slots"
        );
        assert_eq!(payload.name.as_deref(), Some("Tester"), "the write must still carry the name");
        assert_eq!(payload.area_range, Some(3500.0), "the write must still carry the radius");
    }
}
