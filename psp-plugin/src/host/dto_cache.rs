use std::collections::BTreeMap;

use psp_core::domain::{pal, world};
use psp_core::dto::pal::PalDto;
use psp_core::error::CoreError;
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
pub(crate) fn flush(ctx: &mut RunContext<'_>) -> Result<usize, HostError> {
    if ctx.dry_run {
        return Ok(0);
    }
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
