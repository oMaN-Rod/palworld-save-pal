-- Ports of PalworldSaveTools' Delete/Fix/Reset functions onto the plugin API.

-- `unresolved` is `delete_where`'s own count of a selected id it could not
-- resolve at apply time -- kept separate from anything the predicate itself
-- chose to spare.
function delete_empty_guilds()
  local removed, unresolved = save.guilds():delete_where(function(guild)
    return guild.player_count == 0
  end)
  local summary = string.format("Deleted %d empty guild(s)", removed)
  if unresolved > 0 then
    summary = summary .. string.format(" (%d could not be resolved and were left in place)", unresolved)
  end
  return {
    summary = summary,
    counts = { guilds = removed, unresolved = unresolved },
  }
end

function delete_inactive_players()
  local cutoff = ctx.now - (ctx.args.days * 86400)

  local admins = {}
  for guild in save.guilds() do
    if guild.admin_uid then admins[guild.admin_uid] = true end
  end

  -- `admins_skipped` is the predicate's own choice; `unresolved` (delete_where's
  -- second return) is the host's own count for a matched player it could not
  -- resolve at apply time.
  local admins_skipped = 0
  local removed, unresolved = save.players():delete_where(function(player)
    if admins[player.uid] then
      admins_skipped = admins_skipped + 1
      return false
    end
    if player.last_online_ts == nil then
      return false
    end
    return player.last_online_ts < cutoff
  end)

  local summary = string.format("Deleted %d inactive player(s)", removed)
  if admins_skipped > 0 then
    summary = summary .. string.format(" (%d guild admin(s) left in place)", admins_skipped)
  end
  if unresolved > 0 then
    summary = summary .. string.format(" (%d could not be resolved and were left in place)", unresolved)
  end
  return {
    summary = summary,
    counts = { players = removed, admins_skipped = admins_skipped, unresolved = unresolved },
  }
end

-- The `GroupSaveDataMap` guild-tail player-info block also carries a copy of
-- this timestamp for guild membership display, but it decodes to a typed
-- Rust enum rather than a generic property bag, so `raw.*` cannot reach it --
-- every `GroupSaveDataMap` entry's `RawData` reports `kind == "opaque"`. This
-- command only clamps the `CharacterSaveParameterMap` copy, which is also
-- what `save.players().last_online_ts` reads from.
function fix_all_negative_timestamps()
  local now_tick = raw.get('level', 'worldSaveData.GameTimeSaveData.RealDateTimeTicks')
  local clamped = 0

  raw.visit('level', 'worldSaveData.CharacterSaveParameterMap', function(node)
    if node.key == 'LastOnlineRealTime' and type(node.value) == 'number' and node.value > now_tick then
      raw.set('level', node.path, now_tick)
      clamped = clamped + 1
    end
  end)

  return {
    summary = string.format("Clamped %d future timestamp(s)", clamped),
    counts = { timestamps = clamped },
  }
end

-- `slot.clear()` is structural -- it removes the slot rather than emptying it
-- in place, invalidating any live iterator -- so this walks via
-- `save.clear_slots_where`, which decides every clear in one predicate pass
-- before applying them, rather than clearing while iterating.
--
-- Unknown ids are aggregated by distinct id rather than logged per stack,
-- since a world in this state usually holds many stacks of a handful of bad
-- ids.
function remove_invalid_items_from_save()
  local checked = 0
  local unknown, distinct = {}, 0

  local cleared = save.clear_slots_where(function(slot)
    local item = slot.item_id
    if item == nil or item == "" then
      return false
    end
    checked = checked + 1
    if gamedata.is_valid_item(item) then
      return false
    end
    if unknown[item] == nil then
      unknown[item] = 0
      distinct = distinct + 1
    end
    unknown[item] = unknown[item] + 1
    return true
  end)

  for item, count in pairs(unknown) do
    log.warn(string.format("unknown item %q in %d stack(s)", item, count))
  end

  local verb = ctx.dry_run and "Would clear" or "Cleared"
  return {
    summary = string.format(
      "%s %d invalid stack(s) across %d distinct item id(s), out of %d checked",
      verb, cleared, distinct, checked
    ),
    counts = { slots = cleared, checked = checked, distinct_items = distinct },
  }
end
