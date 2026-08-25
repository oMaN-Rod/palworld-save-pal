-- Ports of PalworldSaveTools' Delete/Fix/Reset functions onto the plugin API.

-- Deleting an array element renumbers its siblings, so a shallower delete
-- can invalidate a deeper path already collected; sorting deepest-first
-- keeps every collected path valid when its turn to delete comes.
local function strip_skin_keys(target, root, keys)
  local found = {}
  raw.visit(target, root, function(node)
    if keys[node.key] then
      found[#found + 1] = { path = node.path, depth = node.depth }
    end
  end)
  table.sort(found, function(a, b) return a.depth > b.depth end)
  local removed = 0
  for _, hit in ipairs(found) do
    if raw.delete(target, hit.path) then
      removed = removed + 1
    end
  end
  return removed
end

-- `SkinInventoryInfo` is a structural container every player's `SaveData`
-- carries whether or not a skin was ever applied, unlike `SkinName` and
-- `SkinAppliedCharacterId`, which only exist where a skin actually is. Kept
-- as its own count rather than folded into `skins` for that reason.
function delete_all_skins()
  local skins = strip_skin_keys('level', 'worldSaveData', {
    SkinName = true,
    SkinAppliedCharacterId = true,
  })

  local uids = {}
  for player in save.players() do
    table.insert(uids, player.uid)
  end
  local player_inventories = 0
  for _, uid in ipairs(uids) do
    player_inventories = player_inventories
      + strip_skin_keys('player:' .. uid, 'SaveData', { SkinInventoryInfo = true })
  end

  local verb = ctx.dry_run and "Would remove" or "Removed"
  return {
    summary = string.format(
      "%s %d skin field(s) and cleared %d player skin inventory record(s)",
      verb, skins, player_inventories
    ),
    counts = { skins = skins, player_skin_inventories = player_inventories },
  }
end

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

local function csp_field_index(path, suffix)
  return path and path:match('^worldSaveData%.CharacterSaveParameterMap%[(%d+)%]' .. suffix .. '$')
end

-- Every `CharacterSaveParameterMap` entry, player or pal, carries a
-- `PlayerUId` in its key; a pal's is the nil guid, so only entries whose
-- `SaveParameter.IsPlayer` is true are grouped by it here. A missing
-- `LastOnlineRealTime` counts as older than any recorded value; a genuine
-- tie, including two missing timestamps, keeps the lower map index, so the
-- choice never depends on visit order. Removing the discarded entry's
-- `GroupSaveDataMap` guild-roster record would mean writing inside
-- `PalStruct::GroupData`, which `raw` reports as opaque and cannot descend
-- into (see `fix_all_negative_timestamps` above), so guild membership is
-- left as it is.
function delete_duplicated_players()
  local entries = {}
  local function record(index)
    local rec = entries[index]
    if not rec then
      rec = {}
      entries[index] = rec
    end
    return rec
  end

  raw.visit('level', 'worldSaveData.CharacterSaveParameterMap', function(node)
    local index = csp_field_index(node.path, '%.key%.PlayerUId')
    if index then
      record(tonumber(index)).uid = node.value
      return
    end
    index = csp_field_index(node.path, '%.value%.RawData%.SaveParameter%.IsPlayer')
    if index then
      record(tonumber(index)).is_player = node.value
      return
    end
    index = csp_field_index(node.path, '%.value%.RawData%.SaveParameter%.LastOnlineRealTime')
    if index then
      record(tonumber(index)).last_online = node.value
    end
  end)

  local by_uid = {}
  for index, rec in pairs(entries) do
    if rec.is_player and rec.uid then
      local group = by_uid[rec.uid]
      if not group then
        group = {}
        by_uid[rec.uid] = group
      end
      group[#group + 1] = { index = index, last_online = rec.last_online }
    end
  end

  local to_delete = {}
  for _, group in pairs(by_uid) do
    if #group > 1 then
      table.sort(group, function(a, b)
        local a_online = a.last_online or -1
        local b_online = b.last_online or -1
        if a_online ~= b_online then
          return a_online > b_online
        end
        return a.index < b.index
      end)
      for i = 2, #group do
        to_delete[#to_delete + 1] = group[i].index
      end
    end
  end
  table.sort(to_delete, function(a, b) return a > b end)

  local removed = 0
  for _, index in ipairs(to_delete) do
    if raw.delete('level', 'worldSaveData.CharacterSaveParameterMap[' .. index .. ']') then
      removed = removed + 1
    end
  end

  local verb = ctx.dry_run and "Would remove" or "Removed"
  return {
    summary = string.format("%s %d duplicate player record(s)", verb, removed),
    counts = { players = removed },
  }
end

-- `SaveParameterArray[i]` addresses one DPS pal; the field a predicate needs
-- to inspect lives several segments deeper, so the matched element's own
-- index is derived from that field's own path rather than the field's own
-- address.
local function dps_element_index(field_path)
  local index = field_path:match('^SaveParameterArray%[(%d+)%]')
  return index and tonumber(index)
end

-- Not every player has unlocked dimensional storage, so this player's
-- `_dps.sav` may not exist; that specific failure is treated as an empty
-- dimensional storage (no matches) rather than failing the whole command
-- over one player. Any other error still raises.
local function collect_dps_matches(uid, field, matches)
  local hits = {}
  local ok, err = pcall(raw.visit, 'player_dps:' .. uid, 'SaveParameterArray', function(node)
    if node.key == field and matches(node.value) then
      local index = dps_element_index(node.path)
      if index then
        hits[#hits + 1] = index
      end
    end
  end)
  if not ok then
    if type(err) == 'string' and err:find('has no DPS save', 1, true) then
      return {}
    end
    error(err, 0)
  end
  return hits
end

local function collect_player_uids()
  local uids = {}
  for player in save.players() do
    table.insert(uids, player.uid)
  end
  return uids
end

function delete_imported_pals()
  local removed, unresolved = save.pals():delete_where(function(pal)
    return pal.is_imported
  end)

  local dps_removed = 0
  for _, uid in ipairs(collect_player_uids()) do
    local hits = collect_dps_matches(uid, 'bImportedCharacter', function(value)
      return value == true
    end)
    if #hits > 0 then
      dps_removed = dps_removed + save.delete_dps_pals(uid, hits)
    end
  end

  local verb = ctx.dry_run and "Would remove" or "Removed"
  local summary = string.format(
    "%s %d imported pal(s) from the world and %d from player dimensional storage",
    verb, removed, dps_removed
  )
  if unresolved > 0 then
    summary = summary .. string.format(" (%d could not be resolved and were left in place)", unresolved)
  end
  return {
    summary = summary,
    counts = { pals = removed, dimensional_storage_pals = dps_removed, unresolved = unresolved },
  }
end

-- `gamedata.is_valid_pal` has no notion of `BOSS_`/`PREDATOR_` spawn prefixes,
-- so a boss or predator id is never a catalog entry verbatim; both prefixes
-- are checked directly here rather than trusting the catalog for them.
local function is_boss_or_predator_id(character_id)
  local upper = character_id:upper()
  return upper:sub(1, 5) == 'BOSS_' or upper:sub(1, 9) == 'PREDATOR_'
end

function remove_invalid_pals_from_save()
  -- `pal.is_boss` is false for a lucky pal (`boss_and_lucky` treats the two as
  -- mutually exclusive), but a lucky pal's `character_id` carries the same
  -- `BOSS_` prefix a real boss's does, for the same reason: neither is a
  -- catalog entry verbatim. `pal.is_lucky` covers exactly that gap.
  local removed, unresolved = save.pals():delete_where(function(pal)
    if pal.is_boss or pal.is_predator or pal.is_lucky then
      return false
    end
    return not gamedata.is_valid_pal(pal.character_id)
  end)

  local dps_removed = 0
  for _, uid in ipairs(collect_player_uids()) do
    local hits = collect_dps_matches(uid, 'CharacterID', function(value)
      -- An empty slot's `CharacterID` reads back the literal `"None"`,
      -- never a real species id.
      if value == '' or value == 'None' or is_boss_or_predator_id(value) then
        return false
      end
      return not gamedata.is_valid_pal(value)
    end)
    if #hits > 0 then
      dps_removed = dps_removed + save.delete_dps_pals(uid, hits)
    end
  end

  local verb = ctx.dry_run and "Would remove" or "Removed"
  local summary = string.format(
    "%s %d invalid pal(s) from the world and %d from player dimensional storage",
    verb, removed, dps_removed
  )
  if unresolved > 0 then
    summary = summary .. string.format(" (%d could not be resolved and were left in place)", unresolved)
  end
  return {
    summary = summary,
    counts = { pals = removed, dimensional_storage_pals = dps_removed, unresolved = unresolved },
  }
end

local function dps_passive_list_paths(uid)
  local paths = {}
  local ok, err = pcall(raw.visit, 'player_dps:' .. uid, 'SaveParameterArray', function(node)
    if node.key == 'PassiveSkillList' and node.path then
      paths[#paths + 1] = node.path
    end
  end)
  if not ok then
    if type(err) == 'string' and err:find('has no DPS save', 1, true) then
      return {}
    end
    error(err, 0)
  end
  return paths
end

-- Unlike a dimensional-storage pal slot, which is fixed capacity and always
-- recycled in place, `PassiveSkillList` is a genuine variable-length list --
-- removing an offending entry from it is correct, and there is no capacity to
-- preserve. Deleting descending by index keeps every remaining index valid as
-- the list shrinks; ascending would skip an entry each time one is removed.
local function strip_invalid_dps_passives(uid, valid)
  local target = 'player_dps:' .. uid
  local removed = 0
  for _, list_path in ipairs(dps_passive_list_paths(uid)) do
    local count = raw.len(target, list_path) or 0
    for i = count - 1, 0, -1 do
      local element = list_path .. '[' .. i .. ']'
      if not valid[raw.get(target, element)] then
        if raw.delete(target, element) then
          removed = removed + 1
        end
      end
    end
  end
  return removed
end

function remove_invalid_passives_from_save()
  local valid = {}
  for _, key in ipairs(gamedata.keys('passive_skills')) do
    valid[key] = true
  end

  local removed = 0
  for pal in save.pals() do
    local skills = pal.passive_skills
    local kept = {}
    for _, skill in ipairs(skills) do
      if valid[skill] then
        kept[#kept + 1] = skill
      end
    end
    if #kept ~= #skills then
      removed = removed + (#skills - #kept)
      pal.passive_skills = kept
    end
  end

  local dps_removed = 0
  for _, uid in ipairs(collect_player_uids()) do
    dps_removed = dps_removed + strip_invalid_dps_passives(uid, valid)
  end

  local verb = ctx.dry_run and "Would remove" or "Removed"
  return {
    summary = string.format(
      "%s %d invalid passive skill(s) from pals in the world and %d from player dimensional storage",
      verb, removed, dps_removed
    ),
    counts = { passives = removed, dimensional_storage_passives = dps_removed },
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
