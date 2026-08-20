-- Ports of PalworldSaveTools' Reset functions onto the plugin API.
--
-- Every command here removes save data the game rebuilds on next load. That is
-- what makes the family safe to express as a delete: the absent key is a valid
-- state the game already handles, not a hole that has to be filled in.

function reset_supply_drops()
  local removed = raw.delete("level", "worldSaveData.SupplySaveData")
  local verb = ctx.dry_run and "would be cleared" or "cleared"
  return {
    summary = removed and ("Supply drop data " .. verb) or "No supply drop data was present",
    counts = { supply_save_data = removed and 1 or 0 },
  }
end

function reset_anti_air_turrets()
  local removed = raw.delete("level", "worldSaveData.FixedWeaponDestroySaveData")
  local verb = ctx.dry_run and "would be cleared" or "cleared"
  return {
    summary = removed and ("Anti-air turret state " .. verb) or "No anti-air turret state was present",
    counts = { fixed_weapon_destroy_save_data = removed and 1 or 0 },
  }
end

function reset_oilrig()
  local removed = raw.delete("level", "worldSaveData.OilrigSaveData")
  local verb = ctx.dry_run and "would be cleared" or "cleared"
  return {
    summary = removed and ("Oil rig state " .. verb) or "No oil rig state was present",
    counts = { oilrig_save_data = removed and 1 or 0 },
  }
end

function reset_invader()
  local removed = raw.delete("level", "worldSaveData.InvaderSaveData")
  local verb = ctx.dry_run and "would be cleared" or "cleared"
  return {
    summary = removed and ("Invader state " .. verb) or "No invader state was present",
    counts = { invader_save_data = removed and 1 or 0 },
  }
end

-- Removes both dungeon keys together: they are meaningless apart, since point
-- markers describe dungeons that the other key holds the state for, so
-- removing one alone would leave the world describing dungeons it has no
-- state for. The count is how many of the two were actually present, not how
-- many were attempted.
function reset_dungeons()
  local removed = 0
  for _, key in ipairs({ "DungeonPointMarkerSaveData", "DungeonSaveData" }) do
    if raw.delete("level", "worldSaveData." .. key) then
      removed = removed + 1
    end
  end
  local verb = ctx.dry_run and "would be cleared" or "cleared"
  return {
    summary = removed > 0
      and string.format("Dungeon state %s (%d key(s))", verb, removed)
      or "No dungeon state was present",
    counts = { dungeon_save_data = removed },
  }
end

function reset_lock_gimmick()
  local removed = raw.delete("level", "worldSaveData.LockGimmickSaveData")
  local verb = ctx.dry_run and "would be cleared" or "cleared"
  return {
    summary = removed and ("Lock gimmick state " .. verb) or "No lock gimmick state was present",
    counts = { lock_gimmick_save_data = removed and 1 or 0 },
  }
end

-- The only Reset command that is not a single key: completed-quest state lives
-- in each player's own file, so this is O(players) and a public-server world
-- has thousands. `raw.delete` returning false for an absent array is the
-- meaningful "this player had nothing to clear" answer, not an error case.
--
-- The uids are collected into a plain table before any deletion runs: each
-- `raw.delete` bumps the mutation epoch, and a `save.players()` handle read
-- against a stale epoch errors as an invalidated handle. Finishing the
-- iteration first, then deleting from the plain uid list, avoids holding a
-- live handle across a mutation.
function fix_missions()
  local uids = {}
  for player in save.players() do
    table.insert(uids, player.uid)
  end

  local cleared = 0
  for _, uid in ipairs(uids) do
    if raw.delete("player:" .. uid, "SaveData.CompletedQuestArray_FullRelease") then
      cleared = cleared + 1
    end
  end
  local verb = ctx.dry_run and "Would clear" or "Cleared"
  return {
    summary = string.format("%s completed quests for %d player(s)", verb, cleared),
    counts = { players = cleared },
  }
end
