-- Ports of PalworldSaveTools' Fix family onto the plugin API.

-- Level and rank only. A talent outside 0-100 is a third kind of illegal
-- value, but the host's own pal writer refuses one, so nothing here could
-- write a talent back into range and no test could reach that branch.
local function offences(pal, max_level, max_rank)
  local found = {}
  if pal.level ~= nil and pal.level > max_level then
    found[#found + 1] = string.format("level %d", pal.level)
  end
  if pal.rank ~= nil and pal.rank > max_rank then
    found[#found + 1] = string.format("rank %d", pal.rank)
  end
  return found
end

function scan_illegal_pals()
  local max_level = ctx.args.max_level
  local max_rank = ctx.args.max_rank
  local owner = ctx.args.owner
  local rows, examined = {}, 0

  for pal in save.pals() do
    if owner == "" or pal.owner_uid == owner then
      examined = examined + 1
      local found = offences(pal, max_level, max_rank)
      if #found > 0 then
        rows[#rows + 1] = {
          instance_id = pal.instance_id,
          name = pal.nickname or pal.character_id,
          level = pal.level,
          rank = pal.rank,
          problems = table.concat(found, ", "),
        }
      end
    end
  end

  return {
    summary = string.format("Found %d illegal pal(s) out of %d examined", #rows, examined),
    counts = { illegal = #rows, examined = examined },
    pals = rows,
  }
end

function fix_illegal_pals()
  local max_level = ctx.args.max_level
  local max_rank = ctx.args.max_rank

  local wanted, requested = {}, 0
  for _, id in ipairs(ctx.args.ids) do
    if not wanted[id] then
      wanted[id] = true
      requested = requested + 1
    end
  end

  local pals, clamps, found = 0, 0, 0
  for pal in save.pals() do
    if wanted[pal.instance_id] then
      found = found + 1
      local changed = 0
      if pal.level ~= nil and pal.level > max_level then
        pal.level = max_level
        changed = changed + 1
      end
      if pal.rank ~= nil and pal.rank > max_rank then
        pal.rank = max_rank
        changed = changed + 1
      end
      if changed > 0 then
        pals = pals + 1
        clamps = clamps + changed
      end
    end
  end

  local verb = ctx.dry_run and "Would clamp" or "Clamped"
  return {
    summary = string.format(
      "%s %d value(s) across %d of the %d selected pal(s)", verb, clamps, pals, requested
    ),
    counts = { pals = pals, clamps = clamps, requested = requested, missing = requested - found },
  }
end

function repair_structures()
  local repaired, examined, skipped = 0, 0, 0

  for object in save.map_objects() do
    examined = examined + 1
    local hp, max_hp = object.hp, object.max_hp
    if hp == nil or max_hp == nil then
      skipped = skipped + 1
    elseif hp < max_hp then
      object.hp = max_hp
      repaired = repaired + 1
    end
  end

  local verb = ctx.dry_run and "Would repair" or "Repaired"
  return {
    summary = string.format(
      "%s %d of %d structure(s); %d could not be read", verb, repaired, examined, skipped
    ),
    counts = { repaired = repaired, examined = examined, skipped = skipped },
  }
end

local function over_cap(points, max_points)
  local problems, worst = {}, 0
  if points ~= nil then
    for stat, value in pairs(points) do
      if value ~= nil and value > max_points then
        problems[#problems + 1] = string.format("%s %d", stat, value)
        if value > worst then worst = value end
      end
    end
  end
  table.sort(problems)
  return problems, worst
end

local function clamp_points(points, max_points)
  local clamps = 0
  if points ~= nil then
    for stat, value in pairs(points) do
      if value ~= nil and value > max_points then
        points[stat] = max_points
        clamps = clamps + 1
      end
    end
  end
  return points, clamps
end

function scan_illegal_players()
  local max_points = ctx.args.max_points
  local rows, examined = {}, 0

  for player in save.players() do
    examined = examined + 1
    local base_problems, base_worst = over_cap(player.status_point_list, max_points)
    local ext_problems, ext_worst = over_cap(player.ext_status_point_list, max_points)

    local problems = {}
    for _, p in ipairs(base_problems) do problems[#problems + 1] = p end
    for _, p in ipairs(ext_problems) do problems[#problems + 1] = "ex " .. p end

    if #problems > 0 then
      rows[#rows + 1] = {
        uid = player.uid,
        name = player.name,
        problems = table.concat(problems, ", "),
        worst = math.max(base_worst, ext_worst),
      }
    end
  end

  return {
    summary = string.format(
      "Found %d player(s) over the %d-point cap out of %d examined", #rows, max_points, examined
    ),
    counts = { illegal = #rows, examined = examined },
    players = rows,
  }
end

local SKILL_PREFIX = "EPalWazaID::"

local function catalogued_skills()
  local known = {}
  local keys = gamedata.keys("active_skills")
  if keys ~= nil then
    for _, id in ipairs(keys) do known[id] = true end
  end
  return known
end

-- pals.json's own keys keep the game's canonical casing (e.g. "SheepBall"),
-- while pal.character_key is always lowercased by the host, so a direct
-- gamedata.get("pals", character_key) never matches. This mirrors the
-- lowercase-to-canonical lookup psp-core builds for the same reason.
local function pal_key_lookup()
  local lookup = {}
  local keys = gamedata.keys("pals")
  if keys ~= nil then
    for _, key in ipairs(keys) do
      lookup[key:lower()] = key
    end
  end
  return lookup
end

local function learnable_for(character_key, pal_keys)
  if character_key == nil then return nil end
  local canonical = pal_keys[character_key:lower()]
  if canonical == nil then return nil end
  local entry = gamedata.get("pals", canonical)
  if entry == nil or entry.skill_set == nil then return nil end
  local learnable = {}
  for name, _ in pairs(entry.skill_set) do
    learnable[SKILL_PREFIX .. name] = true
  end
  return learnable
end

local function all_writable(list, known)
  if list == nil then return true end
  for _, id in ipairs(list) do
    if not known[id] then return false end
  end
  return true
end

local function without_unlearnable(list, learnable)
  local kept, dropped = {}, 0
  if list ~= nil then
    for _, id in ipairs(list) do
      if learnable[id] then
        kept[#kept + 1] = id
      else
        dropped = dropped + 1
      end
    end
  end
  return kept, dropped
end

function fix_invalid_pal_active_skills()
  local known = catalogued_skills()
  local pal_keys = pal_key_lookup()
  local pals, removed, examined = 0, 0, 0
  local skipped_unknown_species, skipped_uncatalogued = 0, 0

  for pal in save.pals() do
    examined = examined + 1
    local learnable = learnable_for(pal.character_key, pal_keys)
    local active, learned = pal.active_skills, pal.learned_skills

    if learnable == nil then
      skipped_unknown_species = skipped_unknown_species + 1
    elseif not (all_writable(active, known) and all_writable(learned, known)) then
      -- Writing either list back would be refused entry by entry, failing the
      -- whole run. A boss pal's own signature skill is not in the catalog.
      skipped_uncatalogued = skipped_uncatalogued + 1
    else
      local kept_active, dropped_active = without_unlearnable(active, learnable)
      local kept_learned, dropped_learned = without_unlearnable(learned, learnable)
      if dropped_active > 0 then pal.active_skills = kept_active end
      if dropped_learned > 0 then pal.learned_skills = kept_learned end
      if dropped_active + dropped_learned > 0 then
        pals = pals + 1
        removed = removed + dropped_active + dropped_learned
      end
    end
  end

  local verb = ctx.dry_run and "Would remove" or "Removed"
  return {
    summary = string.format(
      "%s %d unlearnable skill(s) from %d of %d pal(s); skipped %d with an unknown species and %d holding an uncatalogued skill",
      verb, removed, pals, examined, skipped_unknown_species, skipped_uncatalogued
    ),
    counts = {
      pals = pals,
      removed = removed,
      examined = examined,
      skipped_unknown_species = skipped_unknown_species,
      skipped_uncatalogued = skipped_uncatalogued,
    },
  }
end

function fix_illegal_players()
  local max_points = ctx.args.max_points

  local wanted, requested = {}, 0
  for _, id in ipairs(ctx.args.ids) do
    if not wanted[id] then
      wanted[id] = true
      requested = requested + 1
    end
  end

  local players, clamps, found = 0, 0, 0
  for player in save.players() do
    if wanted[player.uid] then
      found = found + 1
      local base, base_clamps = clamp_points(player.status_point_list, max_points)
      local ext, ext_clamps = clamp_points(player.ext_status_point_list, max_points)
      if base_clamps > 0 then player.status_point_list = base end
      if ext_clamps > 0 then player.ext_status_point_list = ext end
      if base_clamps + ext_clamps > 0 then
        players = players + 1
        clamps = clamps + base_clamps + ext_clamps
      end
    end
  end

  local verb = ctx.dry_run and "Would clamp" or "Clamped"
  return {
    summary = string.format(
      "%s %d stat(s) across %d of the %d selected player(s)", verb, clamps, players, requested
    ),
    counts = {
      players = players, clamps = clamps, requested = requested, missing = requested - found,
    },
  }
end
