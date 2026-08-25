mod support;

use psp_plugin::manifest::Capability;
use psp_plugin::status::RunStatus;

const CAPS: &[Capability] = &[Capability::SaveRead, Capability::SaveWrite];
const CAPS_RAW: &[Capability] = &[Capability::SaveRead, Capability::SaveWrite, Capability::SaveRaw];
/// Reading a player row the summary cannot answer needs `players` as well.
const CAPS_PLAYERS: &[Capability] =
    &[Capability::SaveRead, Capability::SaveWrite, Capability::Players];
/// `raw` reaches a player's own `.sav` only through the `player:<uid>` target,
/// which is gated on `players` on top of `save.raw`.
const CAPS_RAW_PLAYERS: &[Capability] =
    &[Capability::SaveRead, Capability::SaveWrite, Capability::SaveRaw, Capability::Players];

const RUNTIME_MANIFEST: &str = r#"{
  "id": "test.dto_cache", "api_version": 1, "name": "Test", "version": "1.0.0",
  "entry": "main.lua",
  "capabilities": ["save.read", "save.write"],
  "commands": [ { "id": "bump_level", "title": "Bump Level" } ]
}"#;

const RUNTIME_SOURCE: &str = r#"
function bump_level()
  for p in save.pals() do p.level = 19 break end
  return 'done'
end
"#;

/// Exercised through the real production path (`runtime::run_command`) rather
/// than the harness's own end-of-run flush, which mirrors it independently and
/// would not catch a regression here.
#[test]
fn run_command_flushes_the_dto_cache_at_the_end_of_a_real_run() {
    let outcome = support::run(RUNTIME_MANIFEST, RUNTIME_SOURCE, "bump_level", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok);
    assert_eq!(outcome.dto_flush_count, 1, "runtime::run_command must flush the cache at the end of a run");
}

/// The snapshot rebuild inside `ensure_pals_snapshot`, which a read of the
/// field just written never reaches: `pal_index` short-circuits that one
/// straight to the DTO cache. Only a read of an *unwritten* field gets there,
/// so `target.rank` is what forces the rebuild the write's
/// `note_pal_field_write` made necessary.
///
/// The rebuild has to flush **before** it re-reads the save, and the ordering
/// is observable because the flush also drains the cache: the `target.level`
/// read afterwards no longer has a pending write to short-circuit to, so it is
/// answered by the fresh snapshot. Flush first and that snapshot carries the
/// write; rebuild first and it carries the value the write replaced.
#[test]
fn a_read_of_an_unwritten_field_rebuilds_the_snapshot_from_a_flushed_save() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(
        "local target
         for p in save.pals() do target = p break end
         target.level = 12
         local rank = target.rank
         return tostring(rank ~= nil) .. '|' .. tostring(target.level)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(
        summary.as_deref(),
        Some("true|12"),
        "the read of an unwritten field must rebuild the pal snapshot from a save the \
         pending write has already been flushed into"
    );
}

#[test]
fn many_writes_to_one_entity_cost_one_flush() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(
        "local target
         for p in save.pals() do target = p break end
         target.level = 12
         target.talent_hp = 100
         target.talent_shot = 50
         return 'done'",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(summary.as_deref(), Some("done"));
    assert_eq!(
        harness.dto_flush_count(),
        1,
        "three field writes on one pal must flush that pal exactly once"
    );
}

#[test]
fn the_entry_index_is_built_once_per_run() {
    let mut harness = support::harness(CAPS);
    let (status, _) = harness.run(
        "local n = 0
         for p in save.pals() do
           p.level = 3
           n = n + 1
           if n == 5 then break end
         end
         return 'done'",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(
        harness.dto_index_build_count(),
        1,
        "five writes must share one index build"
    );
    assert!(
        harness.counts().get("dto.index.build").is_none(),
        "the index-build counter is host-internal observability, not plugin-run output"
    );
}

#[test]
fn writes_to_two_entities_flush_each_once() {
    let mut harness = support::harness(CAPS);
    let (status, _) = harness.run(
        "local n = 0
         for p in save.pals() do
           p.level = 12
           n = n + 1
           if n == 2 then break end
         end
         return 'done'",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(harness.dto_flush_count(), 2);
}

#[test]
fn a_dry_run_writes_nothing_but_still_counts() {
    let mut harness = support::harness_dry(CAPS);
    let (status, _) = harness.run(
        "for p in save.pals() do p.level = 4 break end
         return 'done'",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(harness.dto_flush_count(), 0, "a dry run must not flush");
    assert!(
        harness.counts().get("pal.level").copied().unwrap_or(0) > 0,
        "a dry run must still report what it would have written"
    );
}

/// `flush` used to drain the dirty-DTO map before checking `ctx.dry_run`,
/// discarding a dry run's own pending writes instead of leaving them cached.
/// `friendship_point` has no `pal_field` arm, so this read can only be
/// answered by `fields::pal::pal_get`'s `dto_cache::pal_read` -- which would
/// see the original value, not this run's write, if `flush` had drained it.
#[test]
fn a_dry_runs_own_write_is_still_visible_to_a_later_read_in_the_same_run() {
    let mut harness = support::harness_dry(CAPS);
    let (status, summary) = harness.run(
        "local target
         for p in save.pals() do target = p break end
         target.friendship_point = 999
         return tostring(target.friendship_point)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(
        summary.as_deref(),
        Some("999"),
        "a dry run's own write must still be visible to a later read in the same run"
    );
    assert_eq!(harness.dto_flush_count(), 0, "a dry run must still never actually flush");
}

/// `friendship_point` above has no `pal_field` arm, so its read always went
/// through `pal_get`, which already consulted the DTO cache -- that test
/// never exercised the pal summary's own stale-rebuild path at all.
/// `level` does have a `pal_field` arm: before `pal_index` checked
/// `dto_cache::pal_field_was_written` first, this read went through
/// `ensure_pals_snapshot` -> `flush` (a dry-run no-op) -> a summary rebuilt
/// from the unmodified save, so it returned the pal's original level, not
/// this run's write -- the exact gap `friendship_point`'s test could not
/// catch.
#[test]
fn a_dry_runs_write_to_a_summary_answered_field_is_still_visible_to_a_later_read() {
    let mut harness = support::harness_dry(CAPS);
    let (status, summary) = harness.run(
        "local target
         for p in save.pals() do target = p break end
         local original = target.level
         local new_level = original == 77 and 78 or 77
         target.level = new_level
         return tostring(original) .. '|' .. tostring(target.level) .. '|' .. tostring(new_level)",
    );
    assert_eq!(status, RunStatus::Ok);
    let summary = summary.expect("a string");
    let mut parts = summary.split('|');
    let original: i64 = parts.next().expect("an original level").parse().expect("an integer");
    let after: i64 = parts.next().expect("a level after the write").parse().expect("an integer");
    let new_level: i64 = parts.next().expect("the intended new level").parse().expect("an integer");
    assert_ne!(new_level, original, "the chosen new level must actually differ from the original");
    assert_eq!(
        after,
        new_level,
        "a dry run's own write to a summary-answered field must still be visible to a later read"
    );
}

/// The non-dry counterpart of the test above. Proves
/// `pal_field_was_written`'s short-circuit does not break the ordinary
/// (non-dry) case: the read must still see this run's own write, now served
/// straight from the cache instead of via a flush-and-rebuild.
#[test]
fn a_real_runs_write_to_a_summary_answered_field_is_visible_to_a_later_read() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(
        "local target
         for p in save.pals() do target = p break end
         target.level = 60
         return tostring(target.level)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(summary.as_deref(), Some("60"));
}

/// End of run (`runtime::run_command`, after the command function returns,
/// before `finish`). The harness builds a `RunContext` itself rather than
/// going through `run_command`, so it flushes the same way
/// (`host::flush_dto_cache`) once the script returns -- proven here by a
/// second, independent run seeing the write that the first run never read
/// back itself.
#[test]
fn end_of_run_flush_persists_the_write_for_a_later_run() {
    let mut harness = support::harness(CAPS);
    let (status, _) = harness.run(
        "for p in save.pals() do p.level = 37 break end
         return 'done'",
    );
    assert_eq!(status, RunStatus::Ok);

    let (status, summary) = harness.run(
        "for p in save.pals() do return tostring(p.level) end
         return 'none'",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(summary.as_deref(), Some("37"), "the first run's write must have reached the save");
}

/// Any structural write that bumps the epoch (`ctx.note_mutation`'s callers)
/// -- here, `base.delete()`, which never calls `ensure_pals_snapshot` itself
/// (unlike `pal.delete()`, whose own routing already forces a flush via that
/// path). The write targets one of the base's own worker pals, so if the
/// cache were not flushed before the base (and its workers) are torn down,
/// the end-of-run flush would try to write back a pal that no longer exists
/// and the run would end in error instead of `Ok`.
#[test]
fn a_structural_base_delete_flushes_its_own_worker_pals_pending_write_first() {
    let mut harness = support::harness(CAPS);
    let (status, _) = harness.run(
        "local base_id, pal_id
         for b in save.bases() do
           for p in save.pals() do
             if p.base_id == b.id then base_id, pal_id = b.id, p.instance_id break end
           end
           if base_id then break end
         end
         assert(base_id ~= nil, 'fixture must have a base with at least one worker pal')

         for p in save.pals() do
           if p.instance_id == pal_id then p.level = 67 break end
         end

         for b in save.bases() do if b.id == base_id then b.delete() break end end
         return 'done'",
    );
    assert_eq!(
        status,
        RunStatus::Ok,
        "the worker pal's pending write must flush before the base deletes it, or \
         the end-of-run flush fails to find the (now deleted) pal and the run errors"
    );
}

/// `delete_where`. Using `save.players():delete_where(...)` rather than
/// `save.pals():delete_where(...)` matters here: `save.pals()` itself calls
/// `ensure_pals_snapshot`, which would flush any pending pal write before
/// `delete_where` ever runs and mask a missing flush in `delete_where` itself.
/// `save.players()` never touches the pal snapshot. The write targets one of
/// the victim player's own pals: `delete_player` cascades to delete every pal
/// that player owns, so if the cache were not flushed first, the end-of-run
/// flush would try to write back a pal that no longer exists and the run
/// would end in error instead of `Ok`.
#[test]
fn delete_where_over_players_flushes_an_owned_pals_pending_write_first() {
    let mut harness = support::harness(CAPS);
    let (status, removed) = harness.run(
        "local victim_uid, pal_id
         for p in save.players() do
           local is_admin = false
           if p.guild_id ~= nil then
             for g in save.guilds() do
               if g.id == p.guild_id and g.admin_uid == p.uid then is_admin = true end
             end
           end
           if not is_admin then
             for q in save.pals() do
               if q.owner_uid == p.uid then victim_uid, pal_id = p.uid, q.instance_id break end
             end
             if victim_uid then break end
           end
         end
         assert(victim_uid ~= nil, 'fixture must have a non-admin player who owns a pal')

         for p in save.pals() do
           if p.instance_id == pal_id then p.level = 79 break end
         end

         local removed = save.players():delete_where(function(p) return p.uid == victim_uid end)
         return tostring(removed)",
    );
    assert_eq!(
        status,
        RunStatus::Ok,
        "the owned pal's pending write must flush before delete_player cascades to it, or \
         the end-of-run flush fails to find the (now deleted) pal and the run errors"
    );
    assert_eq!(removed.as_deref(), Some("1"), "the fixture player must actually be deleted");
}

/// Any `raw.*` access reads (or writes) the save directly and would otherwise
/// miss a cached write. This reads through `raw.get` rather than a pal handle,
/// so it cannot be satisfied by the `ensure_pals_snapshot` rebuild alone.
#[test]
fn raw_get_flushes_a_pending_pal_write_before_reading_it() {
    let mut harness = support::harness(CAPS_RAW);
    let (status, summary) = harness.run(
        "local id
         for p in save.pals() do id = p.instance_id break end

         local count = raw.len('level', 'worldSaveData.CharacterSaveParameterMap')
         local index
         for i = 0, count - 1 do
           local this_id = raw.get('level', 'worldSaveData.CharacterSaveParameterMap[' .. i .. '].key.InstanceId')
           if this_id == id then index = i break end
         end
         assert(index ~= nil, 'pal entry not found in CharacterSaveParameterMap')

         local target
         for p in save.pals() do if p.instance_id == id then target = p break end end
         target.level = 101

         local path = 'worldSaveData.CharacterSaveParameterMap[' .. index ..
             '].value.RawData.SaveParameter.Level'
         return tostring(raw.get('level', path))",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(
        summary.as_deref(),
        Some("101"),
        "raw.get must see a pending pal write flushed ahead of its own read"
    );
}

/// A cached entry-index position is only valid until the epoch bumps: a
/// structural delete earlier in `CharacterSaveParameterMap` shifts every later
/// entry's position down by one. If the deletion didn't invalidate a position
/// cached before it, a write issued after the deletion would silently land on
/// whichever pal now sits at that stale position instead of the intended one.
#[test]
fn a_structural_delete_does_not_leave_a_stale_index_position_for_a_later_write() {
    let mut harness = support::harness(CAPS_RAW);
    let (status, summary) = harness.run(
        "local ids = {}
         for p in save.pals() do
           ids[#ids + 1] = p.instance_id
           if #ids == 3 then break end
         end
         assert(#ids == 3, 'fixture must have at least 3 pals')

         local count = raw.len('level', 'worldSaveData.CharacterSaveParameterMap')
         local positions = {}
         for i = 0, count - 1 do
           local this_id = raw.get('level', 'worldSaveData.CharacterSaveParameterMap[' .. i .. '].key.InstanceId')
           for _, id in ipairs(ids) do
             if this_id == id then positions[id] = i end
           end
         end
         for _, id in ipairs(ids) do
           assert(positions[id] ~= nil, 'every collected pal id must resolve to a map position')
         end
         table.sort(ids, function(a, b) return positions[a] < positions[b] end)
         local earlier_id, target_id = ids[1], ids[3]

         for p in save.pals() do
           if p.instance_id == target_id then p.level = 15 break end
         end

         for p in save.pals() do
           if p.instance_id == earlier_id then p.delete() break end
         end

         for p in save.pals() do
           if p.instance_id == target_id then p.level = 88 break end
         end

         for p in save.pals() do
           if p.instance_id == target_id then return tostring(p.level) end
         end
         return 'missing'",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(
        summary.as_deref(),
        Some("88"),
        "a stale cached index position would have addressed whichever pal shifted into it"
    );
}

/// A raw write between two cached pal writes on the same pal must not get
/// silently reverted. `flush` used to only clear the `dirty` flag, leaving the
/// already-parsed DTO sitting in the cache; a later `pal_write` for the same
/// id then hit that stale entry instead of re-reading the save, and the next
/// flush overwrote the raw write with it. `flush` now drains the map
/// entirely, so a cache hit after any flush point is impossible.
#[test]
fn a_raw_write_between_two_cached_writes_on_the_same_pal_is_not_reverted() {
    let mut harness = support::harness(CAPS_RAW);
    let (status, summary) = harness.run(
        "local id
         for p in save.pals() do id = p.instance_id break end

         local count = raw.len('level', 'worldSaveData.CharacterSaveParameterMap')
         local index
         for i = 0, count - 1 do
           local this_id = raw.get('level', 'worldSaveData.CharacterSaveParameterMap[' .. i .. '].key.InstanceId')
           if this_id == id then index = i break end
         end
         assert(index ~= nil, 'pal entry not found in CharacterSaveParameterMap')
         local exp_path = 'worldSaveData.CharacterSaveParameterMap[' .. index ..
             '].value.RawData.SaveParameter.Exp'

         local before_exp = raw.get('level', exp_path)
         local new_exp = before_exp + 12345

         for p in save.pals() do if p.instance_id == id then p.level = 10 break end end
         raw.set('level', exp_path, new_exp)
         for p in save.pals() do if p.instance_id == id then p.level = 11 break end end

         return tostring(raw.get('level', exp_path)) .. '|' .. tostring(new_exp) .. '|' .. tostring(before_exp)",
    );
    assert_eq!(status, RunStatus::Ok);
    let summary = summary.expect("a string");
    let mut parts = summary.split('|');
    let after: i64 = parts.next().expect("an exp value").parse().expect("an integer");
    let new_exp: i64 = parts.next().expect("a new_exp value").parse().expect("an integer");
    let before_exp: i64 = parts.next().expect("a before_exp value").parse().expect("an integer");
    assert_ne!(new_exp, before_exp, "the fixture pal's exp must actually change for this test to mean anything");
    assert_eq!(
        after, new_exp,
        "the raw write between the two level assignments must survive, not revert to \
         before_exp={before_exp}; got exp={after}"
    );
}

/// A player write reaches two different files, and a test that reads back
/// through a handle proves neither: `player_index` serves a written field from
/// the cache, and the two summary-backed rows are patched into
/// `session.player_summaries` by hand at flush time. Both would keep answering
/// correctly with `update_players` never called at all.
///
/// So this reads in a *second run*, whose `RunContext` -- and therefore whose
/// DTO cache -- is empty, forcing `get_player_details` to rebuild the DTO from
/// the save itself. `exp` comes back out of the character-map entry in
/// `Level.sav` and `technology_points` out of the player's own `.sav`, so one
/// assertion covers each half of what `apply_player_dto` writes.
#[test]
fn an_end_of_run_flush_persists_a_player_write_for_a_later_run() {
    let mut harness = support::harness(CAPS_PLAYERS);
    let (status, before) = harness.run(
        "for p in save.players() do
           return tostring(p.exp) .. '|' .. tostring(p.technology_points)
         end
         return 'none'",
    );
    assert_eq!(status, RunStatus::Ok);
    let before = before.expect("a string");
    assert_ne!(
        before, "13579|2468",
        "the fixture must not already hold the values this test writes, or it proves nothing"
    );

    let (status, _) = harness.run(
        "for p in save.players() do
           p.exp = 13579
           p.technology_points = 2468
           break
         end
         return 'done'",
    );
    assert_eq!(status, RunStatus::Ok);

    let (status, after) = harness.run(
        "for p in save.players() do
           return tostring(p.exp) .. '|' .. tostring(p.technology_points)
         end
         return 'none'",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(
        after.as_deref(),
        Some("13579|2468"),
        "the first run's write must have reached the save, not just the cache and the summary"
    );
}

/// The same-run half of the statement above, and it reads the save tree
/// directly rather than through any handle: `raw`'s `player:<uid>` target
/// resolves to `session.loaded_players[uid].sav`, which is the tree
/// `update_players` writes into and the tree that is serialized back to disk.
/// `raw.get` flushes the cache first, so a pending write must already be there.
#[test]
fn raw_get_flushes_a_pending_player_write_before_reading_it() {
    let mut harness = support::harness(CAPS_RAW_PLAYERS);
    let uid = harness.a_player_uid();
    let (status, summary) = harness.run(&format!(
        "local target
         for p in save.players() do if tostring(p.uid) == '{uid}' then target = p break end end
         assert(target ~= nil, 'the fixture player must be reachable')
         local before = raw.get('player:{uid}', 'SaveData.TechnologyPoint')
         target.technology_points = before + 4321
         return tostring(raw.get('player:{uid}', 'SaveData.TechnologyPoint')) .. '|' .. tostring(before)"
    ));
    assert_eq!(status, RunStatus::Ok);
    let summary = summary.expect("a string");
    let mut parts = summary.split('|');
    let after: i64 = parts.next().expect("an after value").parse().expect("an integer");
    let before: i64 = parts.next().expect("a before value").parse().expect("an integer");
    assert_eq!(
        after,
        before + 4321,
        "raw.get must see the flushed write in the player's own save tree, not the pre-write {before}"
    );
}

#[test]
fn many_writes_to_one_player_cost_one_flush() {
    let mut harness = support::harness(CAPS);
    let (status, summary) = harness.run(
        "local target
         for p in save.players() do target = p break end
         target.level = 12
         target.exp = 34
         target.technology_points = 56
         return 'done'",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(summary.as_deref(), Some("done"));
    assert_eq!(
        harness.dto_flush_count(),
        1,
        "three field writes on one player must flush that player exactly once"
    );
}
