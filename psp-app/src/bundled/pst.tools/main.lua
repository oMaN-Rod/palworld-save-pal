-- Ports of PalworldSaveTools' Misc functions onto the plugin API.

local TICKS_PER_DAY = 864000000000

function edit_game_days()
  local days = ctx.args.days
  raw.set("level", "worldSaveData.GameTimeSaveData.GameDateTimeTicks", days * TICKS_PER_DAY)
  local verb = ctx.dry_run and "Would set" or "Set"
  return {
    summary = string.format("%s world clock to day %d", verb, days),
    counts = { days = days },
  }
end

-- Read-only: a plugin cannot write files, so the lines are returned in the
-- result for the UI to offer as copyable text. A base whose location cannot
-- be read is skipped and counted under its own `unresolved` key rather than
-- folded into `bases` or dropped silently.
function paldefender_commands()
  local lines = {}
  local unresolved = 0
  for base in save.bases() do
    if base.x and base.y and base.z then
      lines[#lines + 1] = string.format("/killnearestbase %.0f %.0f %.0f", base.x, base.y, base.z)
    else
      unresolved = unresolved + 1
    end
  end
  local summary = string.format("Generated %d command(s)", #lines)
  if unresolved > 0 then
    summary = summary .. string.format(" (%d base(s) could not be resolved and were skipped)", unresolved)
  end
  return {
    summary = summary,
    counts = { bases = #lines, unresolved = unresolved },
    lines = lines,
  }
end

function modify_one_container_slots()
  local wanted = string.lower(ctx.args.container_id)
  for container in save.containers() do
    if string.lower(container.id) == wanted then
      local resized = container.set_slot_count(ctx.args.slots)
      local verb = ctx.dry_run and "Would resize" or "Resized"
      local summary = resized
        and string.format("%s container to %d slot(s)", verb, ctx.args.slots)
        or "Refused: the container holds items beyond the requested slot count"
      return {
        summary = summary,
        counts = { containers = resized and 1 or 0, refused = resized and 0 or 1 },
      }
    end
  end
  return {
    summary = string.format("No container with id %s", tostring(wanted)),
    counts = { containers = 0, refused = 0 },
  }
end

-- A player's main inventory container id is reachable, just not through
-- player.* (save_read.rs's player_field carries no container id): it lives
-- in the player's own save file at SaveData.InventoryInfo.CommonContainerId
-- .ID, reached through the "player:<uid>" raw scope. Older saves spell the
-- key "inventoryInfo".
--
-- A successful set_slot_count is structural and invalidates every live
-- handle/iterator, so the container walk restarts after each one; a refusal
-- writes nothing and leaves the walk (and the rest of `wanted`) valid.
function modify_all_player_slots()
  local wanted = {}
  local unresolved = 0
  for player in save.players() do
    local scope = "player:" .. player.uid
    local ok, id = pcall(raw.get, scope, "SaveData.InventoryInfo.CommonContainerId.ID")
    if not ok then
      ok, id = pcall(raw.get, scope, "SaveData.inventoryInfo.CommonContainerId.ID")
    end
    if ok and id then
      wanted[id] = true
    else
      unresolved = unresolved + 1
    end
  end

  local resized = 0
  local refused = 0
  local more = true
  while more do
    more = false
    for container in save.containers() do
      if wanted[container.id] then
        wanted[container.id] = nil
        if container.set_slot_count(ctx.args.slots) then
          resized = resized + 1
          more = true
          break
        else
          refused = refused + 1
        end
      end
    end
  end

  local verb = ctx.dry_run and "Would resize" or "Resized"
  local summary = string.format("%s %d player inventory container(s) to %d slot(s)", verb, resized, ctx.args.slots)
  if refused > 0 then
    summary = summary .. string.format(" (%d refused: still holds items beyond the requested slot count)", refused)
  end
  if unresolved > 0 then
    summary = summary .. string.format(" (%d player(s) could not be resolved and were skipped)", unresolved)
  end
  return {
    summary = summary,
    counts = { containers = resized, refused = refused, unresolved = unresolved },
  }
end

-- Same collect-then-write shape as modify_all_player_slots, for the same
-- reason: a successful set_slot_count invalidates every live handle and
-- iterator, so chest ids are gathered into a plain table before any write,
-- and the container walk restarts after each success.
function modify_all_guild_chest_slots()
  local wanted = {}
  for guild in save.guilds() do
    local id = guild.chest_container_id
    if id then
      wanted[id] = true
    end
  end

  local resized = 0
  local refused = 0
  local more = true
  while more do
    more = false
    for container in save.containers() do
      if wanted[container.id] then
        wanted[container.id] = nil
        if container.set_slot_count(ctx.args.slots) then
          resized = resized + 1
          more = true
          break
        else
          refused = refused + 1
        end
      end
    end
  end

  local unresolved = 0
  for _ in pairs(wanted) do
    unresolved = unresolved + 1
  end

  local verb = ctx.dry_run and "Would resize" or "Resized"
  local summary = string.format("%s %d guild chest container(s) to %d slot(s)", verb, resized, ctx.args.slots)
  if refused > 0 then
    summary = summary .. string.format(" (%d refused: still holds items beyond the requested slot count)", refused)
  end
  if unresolved > 0 then
    summary = summary .. string.format(" (%d chest(s) could not be resolved and were skipped)", unresolved)
  end
  return {
    summary = summary,
    counts = { containers = resized, refused = refused, unresolved = unresolved },
  }
end

function unlock_all_private_chests()
  local cleared = save.unlock_private_chests()
  local verb = ctx.dry_run and "Would clear" or "Cleared"
  return {
    summary = string.format("%s %d chest lock(s)", verb, cleared),
    counts = { locks = cleared },
  }
end

local WORK_SUITABILITY_MAX = 10

local function dps_element_index(field_path)
  local index = field_path:match('^SaveParameterArray%[(%d+)%]')
  return index and tonumber(index)
end

-- Not every player has unlocked dimensional storage, so a player may have no
-- `_dps.sav`; that specific failure is treated as empty dimensional storage
-- rather than failing the whole command over one player.
local function pcall_dps_visit(uid, callback)
  local ok, err = pcall(raw.visit, 'player_dps:' .. uid, 'SaveParameterArray', callback)
  if not ok then
    if type(err) == 'string' and err:find('has no DPS save', 1, true) then
      return
    end
    error(err, 0)
  end
end

local function collect_occupied_dps_slots(uid)
  local occupied = {}
  pcall_dps_visit(uid, function(node)
    if node.key == 'CharacterID' then
      local species = node.value
      if species ~= nil and species ~= '' and species ~= 'None' then
        local index = dps_element_index(node.path)
        if index then
          occupied[index] = true
        end
      end
    end
  end)
  return occupied
end

-- `raw.set` bypasses schema validation, so only a key already present on the
-- slot -- one this same walk actually visited -- is ever set; nothing new is
-- added to a slot that never had it.
local function max_dps_slots(uid, occupied, values)
  local target = 'player_dps:' .. uid
  pcall_dps_visit(uid, function(node)
    local value = values[node.key]
    if value ~= nil then
      local index = dps_element_index(node.path)
      if index and occupied[index] then
        raw.set(target, node.path, value)
      end
    end
  end)
end

local function count_keys(t)
  local n = 0
  for _ in pairs(t) do
    n = n + 1
  end
  return n
end

function max_all_pals()
  local cheat = ctx.args.cheat_mode
  local level = cheat and 255 or 80
  local talent = cheat and 255 or 100
  local talent_capped = talent > 100 and 100 or talent
  local soul = cheat and 255 or 20
  local condense = cheat and 255 or 5

  local pals, examined, skipped = 0, 0, 0

  -- Every read happens in this first pass, before any write: a field write
  -- drops the `save.pals()` snapshot a read of a not-yet-written field falls
  -- back to, so a write on one pal would force a full snapshot rebuild for
  -- the very next pal's `work_suitability` read -- quadratic over a save
  -- with thousands of pals. Reading `work_suitability` here, ahead of any
  -- write, keeps every read on the one snapshot this pass builds once.
  local writable = {}
  for pal in save.pals() do
    examined = examined + 1
    if pal.level == nil then
      skipped = skipped + 1
    else
      writable[#writable + 1] = { pal = pal, work_suitability = pal.work_suitability }
    end
  end

  for _, entry in ipairs(writable) do
    local pal = entry.pal
    pal.level = level
    pal.rank = condense
    pal.talent_hp = talent_capped
    pal.talent_shot = talent_capped
    pal.talent_defense = talent_capped
    pal.rank_hp = soul
    pal.rank_attack = soul
    pal.rank_defense = soul
    pal.rank_craftspeed = soul
    pal.friendship_point = 200000
    pal.is_awakened = true

    local suitability = entry.work_suitability
    if suitability ~= nil then
      local changed = false
      for work, rank in pairs(suitability) do
        if rank ~= nil and rank < WORK_SUITABILITY_MAX then
          suitability[work] = WORK_SUITABILITY_MAX
          changed = true
        end
      end
      if changed then pal.work_suitability = suitability end
    end

    pals = pals + 1
  end

  local dps_values = {
    Level = level,
    Rank = condense,
    Talent_HP = talent_capped,
    Talent_Shot = talent_capped,
    Talent_Defense = talent_capped,
    Rank_HP = soul,
    Rank_Attack = soul,
    Rank_Defence = soul,
    Rank_CraftSpeed = soul,
    FriendshipPoint = 200000,
    bIsAwakening = true,
  }

  local dps_pals = 0
  for player in save.players() do
    local occupied = collect_occupied_dps_slots(player.uid)
    local occupied_count = count_keys(occupied)
    if occupied_count > 0 then
      max_dps_slots(player.uid, occupied, dps_values)
      dps_pals = dps_pals + occupied_count
    end
  end

  local verb = ctx.dry_run and "Would max" or "Maxed"
  local summary = string.format(
    "%s %d of %d world pal(s) (%d could not be read) and %d dimensional storage pal(s)",
    verb, pals, examined, skipped, dps_pals
  )
  if dps_pals > 0 then
    summary = summary .. " (work suitability was not raised in dimensional storage)"
  end
  return {
    summary = summary,
    counts = { pals = pals, dps_pals = dps_pals, examined = examined, skipped = skipped },
  }
end

local VIEWING_CAGE_TECHNOLOGY = "DisplayCharacter"

function unlock_viewing_cage_for_player()
  local target = ctx.args.player_uid
  local unlocked, already, found = 0, 0, 0

  for player in save.players() do
    if player.uid == target then
      found = found + 1
      local technologies = player.technologies
      if technologies == nil then
        technologies = {}
      end

      local has = false
      for _, id in ipairs(technologies) do
        if id == VIEWING_CAGE_TECHNOLOGY then
          has = true
          break
        end
      end

      if has then
        already = already + 1
      else
        technologies[#technologies + 1] = VIEWING_CAGE_TECHNOLOGY
        player.technologies = technologies
        unlocked = unlocked + 1
      end
    end
  end

  local verb = ctx.dry_run and "Would unlock" or "Unlocked"
  return {
    summary = (found == 0)
      and "No player matched the given id"
      or string.format(
        "%s the viewing cage for %d player(s); %d already had it", verb, unlocked, already
      ),
    counts = { unlocked = unlocked, already = already, missing = (found == 0) and 1 or 0 },
  }
end
