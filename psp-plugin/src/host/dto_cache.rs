use std::collections::BTreeMap;

use psp_core::domain::{pal, player, world};
use psp_core::dto::ordered_map::OrderedMap;
use psp_core::dto::pal::PalDto;
use psp_core::dto::player::PlayerDto;
use psp_core::error::CoreError;
use psp_core::progress::{null_progress, ProgressSink};
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
/// Both halves are attempted for the same reason a failing pal does not abort
/// the rest: whichever ran first has already drained its map, so bailing out
/// would discard the other's pending writes rather than merely deferring them.
pub(crate) fn flush(ctx: &mut RunContext<'_>) -> Result<usize, HostError> {
    if ctx.dry_run {
        return Ok(0);
    }
    let pals = flush_pals(ctx);
    let players = flush_players(ctx);
    match (pals, players) {
        (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        (Ok(pals), Ok(players)) => Ok(pals.saturating_add(players)),
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
}
