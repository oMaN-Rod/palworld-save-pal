mod support;

use psp_plugin::manifest::Capability;
use psp_plugin::status::RunStatus;

fn write_harness() -> support::Harness {
    support::harness(&[Capability::SaveRead, Capability::SaveWrite, Capability::GameData])
}

/// `base.delete()` deletes the base's worker pals too (`delete_guild_pals` internally, not just the `BaseCampSaveData` entry), so the owning guild's `pal_count`, not just its `base_count`, goes stale without pruning.
#[test]
fn base_delete_keeps_the_owning_guilds_summary_consistent_with_a_full_rebuild() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "local base_id, guild_id
         for g in save.guilds() do
           for b in save.bases() do
             if b.guild_id == g.id then base_id = b.id guild_id = g.id break end
           end
           if base_id then break end
         end
         local deleted = false
         for b in save.bases() do if b.id == base_id then deleted = b.delete() break end end
         return tostring(guild_id) .. '|' .. tostring(deleted)",
    );
    assert_eq!(status, RunStatus::Ok);
    let value = value.expect("a string");
    let mut parts = value.split('|');
    let guild_id: uuid::Uuid = parts.next().expect("a guild id").parse().expect("a uuid");
    let deleted: bool = parts.next().expect("a bool").parse().expect("a bool");
    assert!(deleted, "the fixture must have at least one base to delete");

    let incremental = h.session().guild_summaries.get(&guild_id).cloned().expect("guild still exists");
    h.session_mut().rebuild_player_caches().expect("rebuilds");
    let rebuilt = h.session().guild_summaries.get(&guild_id).cloned().expect("guild still exists");

    assert_eq!(incremental.base_count, rebuilt.base_count, "base_count must match a full rebuild");
    assert_eq!(
        incremental.pal_count, rebuilt.pal_count,
        "pal_count must match a full rebuild -- a base's pals go with it"
    );
}

/// `g.player_count == 0` can never match any guild here: `delete_player` refuses to delete a guild's admin (always first in its own member list) while the guild is loaded, so a guild can reach exactly one member but never zero through this API. This test instead picks a guild with at least two REAL players by counting `p.guild_id` directly -- `g.player_count` alone can include departed members with no live player entity -- deletes one non-admin member, then runs `delete_where` keyed on the guild's now-reduced live count.
#[test]
fn delete_where_over_guilds_removes_only_matching_guilds() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "local target
         for p in save.players() do
           if p.guild_id then
             local n = 0
             for q in save.players() do
               if q.guild_id == p.guild_id then n = n + 1 end
             end
             if n >= 2 then target = p.guild_id break end
           end
         end

         local admin
         for g in save.guilds() do if g.id == target then admin = g.admin_uid end end
         local other
         for g in save.guilds() do if g.id ~= target then other = g.id break end end

         local victim
         for p in save.players() do
           if p.guild_id == target and p.uid ~= admin then victim = p.uid break end
         end
         local deleted
         for p in save.players() do
           if p.uid == victim then deleted = p.delete() break end
         end

         local reduced_count
         for g in save.guilds() do if g.id == target then reduced_count = g.player_count end end

         local removed = save.guilds():delete_where(function(g)
           return g.id == target and g.player_count == reduced_count
         end)

         local target_gone = true
         local other_survives = false
         for g in save.guilds() do
           if g.id == target then target_gone = false end
           if g.id == other then other_survives = true end
         end

         return tostring(deleted) .. ',' .. tostring(removed) .. ',' .. tostring(target_gone) .. ',' .. tostring(other_survives)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(
        value.as_deref(),
        Some("true,1,true,true"),
        "the non-admin member was deleted, exactly the target guild was removed by delete_where, and the other guild survived"
    );
}

#[test]
fn delete_where_reports_removed_plus_skipped_equal_to_matched_for_orphaned_pals() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "local matched = 0
         for p in save.pals() do
           if not gamedata.is_valid_pal(p.character_id) then matched = matched + 1 end
         end
         local removed, skipped = save.pals():delete_where(function(p)
           return not gamedata.is_valid_pal(p.character_id)
         end)
         return tostring(matched) .. ',' .. tostring(removed) .. ',' .. tostring(skipped)",
    );
    assert_eq!(status, RunStatus::Ok);
    let value = value.expect("the chunk returns a string");
    let parts: Vec<i64> = value.split(',').map(|p| p.parse().expect("integer field")).collect();
    assert_eq!(parts.len(), 3, "got {value}");
    let (matched, removed, skipped) = (parts[0], parts[1], parts[2]);
    assert!(matched > 0, "the fixture must have at least one pal that fails is_valid_pal");
    assert!(skipped > 0, "the fixture must have at least one orphaned pal delete_where cannot resolve");
    assert_eq!(removed + skipped, matched, "every matched id must be accounted for by removed or skipped");
}

#[test]
fn delete_where_returns_zero_when_nothing_matches() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "return tostring(save.guilds():delete_where(function() return false end))",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("0"));
}

/// A host-mechanism test, proving the iterator and bulk removal work -- not
/// the predicate the later delete_non_base_map_objects command will use. The
/// 3308 objects this predicate matches are the fixture's unattached map
/// objects, not "everything a repair or cleanup command should ever touch".
#[test]
fn delete_where_removes_the_matched_objects_in_one_pass() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "local before = 0
         for _ in save.map_objects() do before = before + 1 end
         local removed, unresolved = save.map_objects():delete_where(function(obj)
           return obj.base_id == nil
         end)
         local after = 0
         for _ in save.map_objects() do after = after + 1 end
         return table.concat({ before, removed, unresolved, after }, ',')",
    );
    assert_eq!(status, RunStatus::Ok);
    let value = value.expect("a string");
    let parts: Vec<i64> = value.split(',').map(|p| p.parse().expect("an integer")).collect();
    assert_eq!(parts.len(), 4, "got {value}");
    let (before, removed, unresolved, after) = (parts[0], parts[1], parts[2], parts[3]);
    assert_eq!(before, 5452, "the fixture's map object count");
    assert_eq!(removed, 3308, "the fixture's unattached map objects");
    assert_eq!(unresolved, 0);
    assert_eq!(after, 2144);
}

#[test]
fn a_predicate_that_errors_aborts_the_bulk_delete_without_partial_damage() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "local before = save.info().guild_count
         local ok = pcall(function()
             save.guilds():delete_where(function() error('nope') end)
         end)
         return tostring(ok) .. ',' .. tostring(save.info().guild_count == before)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("false,true"));
}

#[test]
fn deleting_a_guild_admin_is_refused_rather_than_corrupting_the_guild() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "local refused = 0
         for g in save.guilds() do
           for p in save.players() do
             if p.uid == g.admin_uid then
               if p.delete() == false then refused = refused + 1 end
             end
           end
         end
         return tostring(refused > 0)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("true"));
}

/// Uses `pal.delete()`, not a guild `delete_where`: pal deletion always succeeds and unconditionally bumps `mutation_epoch`, unlike a guild-emptying predicate, which can never actually fire against this write API (see `delete_where_over_guilds_removes_only_matching_guilds`).
#[test]
fn a_handle_used_after_a_mutation_raises_a_clean_error() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "local first for g in save.guilds() do first = g break end
         local pal_id for p in save.pals() do pal_id = p.instance_id break end
         local deleted
         for p in save.pals() do if p.instance_id == pal_id then deleted = p.delete() break end end
         local ok, err = pcall(function() return first.name end)
         return tostring(deleted) .. ',' .. tostring(ok) .. ',' .. tostring(type(err) == 'string')",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("true,false,true"));
}

#[test]
fn iterating_while_mutating_raises_rather_than_reading_a_moved_row() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "local ok = pcall(function()
             for g in save.guilds() do g.delete() end
         end)
         return tostring(ok)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("false"));
}

#[test]
fn setting_a_pal_level_is_visible_on_the_next_read() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "local id for p in save.pals() do id = p.instance_id break end
         for p in save.pals() do if p.instance_id == id then p.level = 37 break end end
         for p in save.pals() do if p.instance_id == id then return tostring(p.level) end end
         return 'not found'",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("37"));
}

/// Asserting only that the assignment was refused would pass just as happily
/// if assignment stopped working altogether, so the refusal has to be pinned
/// by its reason: it must name the field and the range it enforces, and it
/// must not be the error a pal that cannot be assigned to at all reports.
#[test]
fn an_out_of_range_level_is_refused() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "for p in save.pals() do
           local ok, err = pcall(function() p.level = -5 end)
           return tostring(ok) .. '|' .. tostring(err)
         end
         return 'no pals'",
    );
    assert_eq!(status, RunStatus::Ok);
    let value = value.expect("a string");
    let (ok, err) = value.split_once('|').expect("an ok flag and an error");
    assert_eq!(ok, "false", "an out-of-range level must be refused: {value}");
    assert!(
        !err.contains("attempt to index a psp.pal value"),
        "the refusal must come from validating the value, not from the pal having no \
         assignment path at all: {err}"
    );
    assert!(
        err.contains("level must be between 1 and 255"),
        "the refusal must name the field and the range it enforces: {err}"
    );
}

#[test]
fn an_in_range_level_succeeds() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "for p in save.pals() do
           local ok = pcall(function() p.level = 42 end)
           return tostring(ok) .. ',' .. tostring(p.level)
         end
         return 'no pals'",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("true,42"));
}

#[test]
fn a_container_slot_can_be_cleared() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "local cleared = 0
         for c in save.containers() do
           for s in c.slots() do
             if s.item_id ~= nil and s.item_id ~= '' then s.clear() cleared = cleared + 1 break end
           end
           if cleared > 0 then break end
         end
         return tostring(cleared)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("1"));
}

/// `slot.clear()` removes the raw slot entry rather than emptying it in place, and `container.slots()` indexes positionally, so a clear invalidates every live iterator over the same container -- hence the re-fetch-and-recheck loop below rather than a single pass.
#[test]
fn clearing_every_occupied_slot_in_a_container_empties_it() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "local container_id
         for c in save.containers() do
           for s in c.slots() do
             if s.item_id ~= nil and s.item_id ~= '' then container_id = c.id break end
           end
           if container_id then break end
         end

         local cleared = 0
         while true do
           local found = false
           for c in save.containers() do
             if c.id == container_id then
               for s in c.slots() do
                 if s.item_id ~= nil and s.item_id ~= '' then
                   s.clear()
                   cleared = cleared + 1
                   found = true
                   break
                 end
               end
             end
             if found then break end
           end
           if not found then break end
         end

         local remaining = 0
         for c in save.containers() do
           if c.id == container_id then
             for s in c.slots() do
               if s.item_id ~= nil and s.item_id ~= '' then remaining = remaining + 1 end
             end
           end
         end

         return tostring(cleared > 0) .. ',' .. tostring(remaining)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(
        value.as_deref(),
        Some("true,0"),
        "at least one slot was cleared, and every occupied slot in the container is now empty"
    );
}

#[test]
fn clear_slots_where_empties_every_slot_its_predicate_selects() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "local before = 0
         for c in save.containers() do
           for s in c.slots() do
             if s.item_id ~= nil then before = before + 1 end
           end
         end

         local cleared = save.clear_slots_where(function(s) return s.item_id ~= nil end)

         local after = 0
         for c in save.containers() do
           for s in c.slots() do
             if s.item_id ~= nil then after = after + 1 end
           end
         end
         return string.format('%d,%d,%d', before, cleared, after)",
    );
    assert_eq!(status, RunStatus::Ok);
    let value = value.expect("a string");
    let parts: Vec<i64> = value.split(',').map(|p| p.parse().expect("an integer")).collect();
    let (before, cleared, after) = (parts[0], parts[1], parts[2]);
    assert!(before > 0, "the fixture must have occupied slots");
    assert_eq!(cleared, before, "every occupied slot was selected, so every one must be cleared");
    assert_eq!(after, 0, "no occupied slot may survive a clear that selected all of them");
}

#[test]
fn clear_slots_where_selecting_nothing_changes_nothing() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "local first
         for c in save.containers() do first = c break end
         local cleared = save.clear_slots_where(function() return false end)
         return string.format('%d,%s', cleared, tostring(first.slot_count ~= nil))",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("0,true"));
}

#[test]
fn a_nested_clear_slots_where_is_refused() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "local ok = pcall(function()
           save.clear_slots_where(function()
             save.clear_slots_where(function() return false end)
             return false
           end)
         end)
         return tostring(ok)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("false"));
}

#[test]
fn mutating_from_inside_the_clear_predicate_raises_rather_than_skipping_slots() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "local ok = pcall(function()
           save.clear_slots_where(function(s)
             save.guilds():delete_where(function() return true end)
             return false
           end)
         end)
         return tostring(ok)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("false"));
}

#[test]
fn clear_slots_where_without_the_write_capability_is_absent() {
    let mut h = support::harness(&[Capability::SaveRead, Capability::GameData]);
    let (status, value) = h.run("return type(save.clear_slots_where)");
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("nil"));
}

#[test]
fn unlock_private_chests_reports_what_it_cleared_and_is_idempotent() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "local first = save.unlock_private_chests()
         local second = save.unlock_private_chests()
         return string.format('%d,%d', first, second)",
    );
    assert_eq!(status, RunStatus::Ok);
    let value = value.expect("a string");
    let parts: Vec<i64> = value.split(',').map(|p| p.parse().expect("an integer")).collect();
    assert!(parts[0] > 0, "the fixture must have a locked chest to clear: {value}");
    assert_eq!(parts[1], 0, "a second call has nothing left to clear: {value}");
}

#[test]
fn unlock_private_chests_without_the_write_capability_is_absent() {
    let mut h = support::harness(&[Capability::SaveRead, Capability::GameData]);
    let (status, value) = h.run("return type(save.unlock_private_chests)");
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("nil"));
}

/// `note_mutation`'s bump is run-wide, so the captured handle need not be a chest itself to prove the clear invalidated it.
#[test]
fn a_successful_unlock_invalidates_a_live_handle() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "local first for g in save.guilds() do first = g break end
         local cleared = save.unlock_private_chests()
         local ok = pcall(function() return first.name end)
         return tostring(cleared) .. ',' .. tostring(ok)",
    );
    assert_eq!(status, RunStatus::Ok);
    let value = value.expect("a string");
    let parts: Vec<&str> = value.split(',').collect();
    assert!(
        parts[0].parse::<i64>().expect("an integer") > 0,
        "the fixture must have a locked chest to clear: {value}"
    );
    assert_eq!(
        parts[1], "false",
        "clearing a lock must bump the mutation epoch and invalidate the live handle: {value}"
    );
}

/// Pins the host binding's `cleared > 0` guard: a version that called `ctx.note_mutation()` unconditionally would pass every other test in this file but fail this one.
#[test]
fn a_no_op_unlock_does_not_invalidate_a_live_handle() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "local first_pass = save.unlock_private_chests()
         local first for g in save.guilds() do first = g break end
         local second_pass = save.unlock_private_chests()
         local ok = pcall(function() return first.name end)
         return tostring(first_pass) .. ',' .. tostring(second_pass) .. ',' .. tostring(ok)",
    );
    assert_eq!(status, RunStatus::Ok);
    let value = value.expect("a string");
    let parts: Vec<&str> = value.split(',').collect();
    assert!(
        parts[0].parse::<i64>().expect("an integer") > 0,
        "the fixture must have a locked chest to clear on the first pass: {value}"
    );
    assert_eq!(parts[1], "0", "nothing should be left to clear on the second call: {value}");
    assert_eq!(parts[2], "true", "a no-op unlock must not bump the mutation epoch: {value}");
}

/// `ctx.dry_run` never calls the writing `unlock_private_chests`, only the read-only `count_private_chest_locks`.
#[test]
fn a_dry_run_unlock_does_not_invalidate_a_live_handle() {
    let mut h = support::harness_dry(&[Capability::SaveRead, Capability::SaveWrite, Capability::GameData]);
    let (status, value) = h.run(
        "local first for g in save.guilds() do first = g break end
         local predicted = save.unlock_private_chests()
         local ok = pcall(function() return first.name end)
         return tostring(predicted) .. ',' .. tostring(ok)",
    );
    assert_eq!(status, RunStatus::Ok);
    let value = value.expect("a string");
    let parts: Vec<&str> = value.split(',').collect();
    assert!(
        parts[0].parse::<i64>().expect("an integer") > 0,
        "the fixture must have a locked chest to predict: {value}"
    );
    assert_eq!(parts[1], "true", "a dry run must not bump the mutation epoch: {value}");
}

#[test]
fn clear_slots_where_rejects_a_non_function_argument() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "local ok, err = pcall(function() save.clear_slots_where('not a function') end)
         return tostring(ok) .. '|' .. tostring(err)",
    );
    assert_eq!(status, RunStatus::Ok);
    let value = value.expect("a string");
    assert!(value.starts_with("false|"), "{value}");
    assert!(value.contains("expects a function"), "{value}");
}

#[test]
fn a_predicate_error_propagates_and_does_not_poison_later_clears() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "local ok = pcall(function()
           save.clear_slots_where(function() error('boom') end)
         end)
         local cleared = save.clear_slots_where(function() return false end)
         return tostring(ok) .. ',' .. tostring(cleared)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("false,0"));
}

/// Distinct from the `lua_pcall`-error path above: a predicate that mutates without itself raising makes the select loop itself raise `invalidated_handle_error` via a plain `?`, which must also clear `ctx.clear_slots`.
#[test]
fn an_epoch_mismatch_during_the_select_pass_does_not_poison_later_clears() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "local ok = pcall(function()
           save.clear_slots_where(function(s)
             save.guilds():delete_where(function() return true end)
             return false
           end)
         end)
         local cleared = save.clear_slots_where(function() return false end)
         return tostring(ok) .. ',' .. tostring(cleared)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("false,0"));
}

/// Mirrors the write surface's own doc example (`for s in c.slots() do s.clear() end`).
#[test]
fn clearing_a_second_slot_in_the_same_open_iteration_raises() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "local container_id
         for c in save.containers() do
           local occupied = 0
           for s in c.slots() do
             if s.item_id ~= nil and s.item_id ~= '' then occupied = occupied + 1 end
           end
           if occupied >= 2 then container_id = c.id break end
         end
         if not container_id then return 'no multi-item container in fixture' end

         local ok = pcall(function()
           for c in save.containers() do
             if c.id == container_id then
               for s in c.slots() do
                 if s.item_id ~= nil and s.item_id ~= '' then s.clear() end
               end
             end
           end
         end)
         return tostring(ok)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("false"));
}

#[test]
fn a_delete_where_predicate_that_calls_delete_where_is_refused_rather_than_corrupting_state() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "local ok = pcall(function()
             save.guilds():delete_where(function(g)
                 save.guilds():delete_where(function() return false end)
                 return false
             end)
         end)
         return tostring(ok)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("false"));
}

#[test]
fn every_write_is_absent_without_the_write_capability() {
    let mut h = support::harness(&[Capability::SaveRead]);
    let (status, value) = h.run(
        "local names = {}
         for p in save.players() do
           names[#names+1] = type(p.delete)
           names[#names+1] = tostring(pcall(function() p.level = 4 end))
           break
         end
         names[#names+1] = type(save.players().delete_where)
         for c in save.containers() do
           names[#names+1] = type(c.set_slot_count)
           break
         end
         return table.concat(names, ',')",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("nil,false,nil,nil"));
}

/// `p.delete` is a bound closure with no `self` argument; `delete_where` needs a real self at argument 1 for its own argument-count check to pass before `vals[i]` reaches its `lua_isfunction` check. A field assignment takes no argument list at all, so the hostile value goes on the right-hand side instead.
#[test]
fn every_write_function_survives_hostile_arguments() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "local vals = { nil, true, 0, -1, 1/0, 0/0, '', 'x', {}, print }
         for p in save.players() do
           for i = 1, 10 do
             pcall(function() p.level = vals[i] end)
             pcall(p.delete, vals[i])
           end
           break
         end
         for i = 1, 10 do pcall(save.guilds().delete_where, save.guilds(), vals[i]) end
         for c in save.containers() do
           for i = 1, 10 do pcall(c.set_slot_count, vals[i]) end
           break
         end
         return 'survived'",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("survived"));
}

#[test]
fn set_slot_count_grows_a_container_and_refuses_a_destructive_shrink() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "local target, occupied, capacity
         for c in save.containers() do
           local n = 0
           local highest = -1
           for s in c.slots() do
             if s.item_id ~= nil then n = n + 1 highest = s.index end
           end
           if n > 0 then target = c.id occupied = highest capacity = c.slot_count break end
         end
         if not target then return 'no occupied container in fixture' end

         local grown, shrunk, after_grow, after_refuse
         for c in save.containers() do
           if c.id == target then grown = c.set_slot_count(capacity + 5) break end
         end
         for c in save.containers() do
           if c.id == target then after_grow = c.slot_count break end
         end
         for c in save.containers() do
           if c.id == target then shrunk = c.set_slot_count(occupied) break end
         end
         for c in save.containers() do
           if c.id == target then after_refuse = c.slot_count break end
         end
         return string.format('%s,%d,%s,%d,%d', tostring(grown), after_grow, tostring(shrunk), after_refuse, capacity)",
    );
    assert_eq!(status, RunStatus::Ok);
    let value = value.expect("a string");
    let parts: Vec<&str> = value.split(',').collect();
    assert_eq!(parts[0], "true", "growing must succeed: {value}");
    let capacity: i64 = parts[4].parse().expect("a capacity");
    assert_eq!(
        parts[1].parse::<i64>().unwrap(),
        capacity + 5,
        "grown capacity must be exactly the fixture's original capacity plus 5: {value}"
    );
    assert_eq!(parts[2], "false", "a destructive shrink must be refused: {value}");
    assert_eq!(
        parts[3], parts[1],
        "a refused shrink must leave the capacity exactly as the grow left it: {value}"
    );
}

/// `set_slot_count` removes raw `Slots` entries when shrinking, so a successful shrink must `note_mutation`.
#[test]
fn a_successful_shrink_invalidates_a_live_container_handle() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "local target, occupied
         for c in save.containers() do
           local n, highest = 0, -1
           for s in c.slots() do
             if s.item_id ~= nil then n = n + 1 highest = s.index end
           end
           if n > 0 then target = c.id occupied = highest break end
         end
         if not target then return 'no occupied container in fixture' end

         for c in save.containers() do
           if c.id == target then c.set_slot_count(occupied + 10) break end
         end

         local first
         for c in save.containers() do if c.id == target then first = c break end end

         local resized
         for c in save.containers() do
           if c.id == target then resized = c.set_slot_count(occupied + 1) break end
         end

         local ok = pcall(function() return first.slot_count end)
         return tostring(resized) .. ',' .. tostring(ok)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(
        value.as_deref(),
        Some("true,false"),
        "a successful shrink must bump the mutation epoch and invalidate the live handle"
    );
}

#[test]
fn a_refused_shrink_does_not_invalidate_a_live_container_handle() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "local target, occupied
         for c in save.containers() do
           local n, highest = 0, -1
           for s in c.slots() do
             if s.item_id ~= nil then n = n + 1 highest = s.index end
           end
           if n > 0 then target = c.id occupied = highest break end
         end
         if not target then return 'no occupied container in fixture' end

         local first
         for c in save.containers() do if c.id == target then first = c break end end

         local refused
         for c in save.containers() do
           if c.id == target then refused = c.set_slot_count(occupied) break end
         end

         local ok, slot_count = pcall(function() return first.slot_count end)
         return tostring(refused) .. ',' .. tostring(ok) .. ',' .. tostring(slot_count)",
    );
    assert_eq!(status, RunStatus::Ok);
    let value = value.expect("a string");
    let parts: Vec<&str> = value.split(',').collect();
    assert_eq!(parts[0], "false", "a shrink that would destroy items must be refused: {value}");
    assert_eq!(parts[1], "true", "a refused shrink must not invalidate a live handle: {value}");
}

/// A bound closure like `set_slot_count` carries only a raw `Uuid` and, unlike a `read_handle`-gated field access, does not re-check `ctx.mutation_epoch` -- so it stays callable after a mutation, even once the container it names is gone, reaching `containers::set_container_slot_count`'s own "not found" error directly. Deleting a player cascades to that player's own item containers.
#[test]
fn set_slot_count_on_a_container_removed_by_another_write_raises() {
    let mut h = write_harness();
    let (status, value) = h.run(
        "local before = {}
         for c in save.containers() do before[c.id] = c.set_slot_count end

         for p in save.players() do if p.delete() then break end end

         local after = {}
         for c in save.containers() do after[c.id] = true end

         for id in pairs(before) do
           if not after[id] then
             local ok, err = pcall(before[id], 5)
             return tostring(ok) .. ',' .. tostring(type(err)) .. ',' .. tostring(err)
           end
         end
         return 'no container was removed by the player delete'",
    );
    assert_eq!(status, RunStatus::Ok);
    let value = value.expect("a string");
    assert_ne!(
        value, "no container was removed by the player delete",
        "the fixture's deletable player must own at least one item container for this test to reach the path it targets"
    );
    assert!(
        value.starts_with("false,string,"),
        "calling set_slot_count on a container removed by another write must raise a catchable error: {value}"
    );
}

/// A base worker pal PSP itself created carries `OwnerPlayerUId` as the nil guid
/// rather than omitting it (`pal::add_guild_pal` -> `new_pal_entry(.., EMPTY_UUID, ..)`,
/// and the same shape `gps.rs` writes for an unowned clone). `pal_routing` hands that
/// back as `Some(nil)`, which the delete match reads as "owned by a player".
fn seed_base_worker(h: &mut support::Harness) -> uuid::Uuid {
    let game_data = support::load_game_data();
    let mut pairs: Vec<(uuid::Uuid, uuid::Uuid)> = {
        let summaries =
            psp_core::domain::pal::pal_summaries(h.session(), &game_data).expect("summaries build");
        summaries.iter().filter_map(|s| s.guild_id.zip(s.base_id)).collect()
    };
    pairs.sort();
    pairs.dedup();
    assert!(!pairs.is_empty(), "the corpus fixture has at least one base worker pal");

    for (guild_id, base_id) in pairs {
        psp_core::domain::guild::get_guild_details(h.session_mut(), &game_data, guild_id)
            .expect("guild details load")
            .expect("the guild exists");
        let added = psp_core::domain::pal::add_guild_pal(
            h.session_mut(),
            &game_data,
            guild_id,
            base_id,
            "Lamball",
            "NilOwnerWorker",
            None,
        )
        .expect("add_guild_pal succeeds");
        if let Some(dto) = added {
            assert_eq!(
                dto.owner_uid,
                Some(uuid::Uuid::nil()),
                "the seeded worker must carry the nil guid"
            );
            return dto.instance_id;
        }
    }
    panic!("no base in the corpus fixture has a free worker slot");
}

#[test]
fn pal_delete_removes_a_base_worker_whose_owner_uid_is_the_nil_guid() {
    let mut h = write_harness();
    let pal_id = seed_base_worker(&mut h);

    let (status, value) = h.run(&format!(
        "local target = '{pal_id}'
         local owner, base
         for p in save.pals() do
           if p.instance_id == target then owner = tostring(p.owner_uid) base = tostring(p.base_id) break end
         end
         local deleted
         for p in save.pals() do if p.instance_id == target then deleted = p.delete() break end end
         local gone = true
         for p in save.pals() do if p.instance_id == target then gone = false end end
         return owner .. ',' .. base .. ',' .. tostring(deleted) .. ',' .. tostring(gone)"
    ));
    assert_eq!(status, RunStatus::Ok, "pal.delete() must not raise for a base worker");
    let value = value.expect("the chunk returns a string");
    let parts: Vec<&str> = value.split(',').collect();
    assert_eq!(parts.get(2).copied(), Some("true"), "pal.delete() returned {value}");
    assert_eq!(parts.get(3).copied(), Some("true"), "the base worker must be gone: {value}");
}

#[test]
fn delete_where_removes_a_base_worker_whose_owner_uid_is_the_nil_guid() {
    let mut h = write_harness();
    let pal_id = seed_base_worker(&mut h);

    let (status, value) = h.run(&format!(
        "local target = '{pal_id}'
         local removed, skipped = save.pals():delete_where(function(p) return p.instance_id == target end)
         local gone = true
         for p in save.pals() do if p.instance_id == target then gone = false end end
         return tostring(removed) .. ',' .. tostring(skipped) .. ',' .. tostring(gone)"
    ));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("1,0,true"), "delete_where must remove the base worker, not skip it");
    assert!(
        h.log().is_empty(),
        "a removable base worker must not be logged as an unresolvable owner: {:?}",
        h.log()
    );
}
