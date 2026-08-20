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
