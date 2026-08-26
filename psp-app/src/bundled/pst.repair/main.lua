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
