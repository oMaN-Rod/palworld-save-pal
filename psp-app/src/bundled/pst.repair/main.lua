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
