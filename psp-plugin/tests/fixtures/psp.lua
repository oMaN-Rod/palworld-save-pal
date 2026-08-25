---@meta

---Requires capability: save.read.
---
---@class player
---@field uid string The player's UUID, as a string.
---@field name string The player's nickname.
---@field level integer|nil The player's level, or nil if the save has no level recorded for this player.
---@field guild_id string|nil The UUID, as a string, of the guild this player belongs to, or nil if the player is in no guild.
---@field pal_count integer How many pals this player owns.
---@field last_online string|nil An ISO-8601 timestamp of when the player was last online, or nil if the save records none.
---@field last_online_ts integer|nil The Unix timestamp, in seconds, of when the player was last online, or nil if the save records none.
---@field pals fun(): (fun(): pal|nil) An iterator over every pal this player owns, for use in a `for` loop. Requires capability: save.read.
---@field delete fun(): boolean Deletes this player, along with the item and character containers the player owns. Refuses (returns false, changes nothing) if the player is their guild's admin. A true result is a structural write and invalidates every live handle and iterator across all scopes, including this one. Requires capability: save.write.
---@field set_level fun(level: integer): nil Sets the player's level; must be between 1 and 255 inclusive, or this raises. A non-structural write: this handle and every other live handle and iterator stay valid, but any container previously read through this run is forgotten and will be re-read on next access. Requires capability: save.write.

---Requires capability: save.read.
---
---@class pal
---@field instance_id string This pal's unique id, as a string. Read-only.
---@field character_id string The pal's species id, including a BOSS_ prefix when it has one. Read-only in itself, but writing is_lucky changes it: setting is_lucky = false removes a BOSS_ prefix that only marked the pal lucky, and setting it true puts one back when the change is saved. A dry run saves nothing, so after is_lucky = true this still reads the unprefixed id.
---@field character_key string The pal's species key, as used to look up game data. Read-only, and refreshed only when the pal is next read from the save, so during a dry run it can still name the old species after a write that changed character_id.
---@field nickname string|nil The pal's nickname, or nil if it has none.
---@field owner_uid string|nil The id of the player who owns this pal, or nil if a guild base owns it instead. Read-only.
---@field guild_id string|nil The id of the guild whose base this pal works at, or nil if it works at none. Read-only.
---@field base_id string|nil The id of the base this pal works at, or nil if it works at none. Read-only.
---@field gender string "None", "Male" or "Female".
---@field level integer The pal's level, 1-255.
---@field hp integer The pal's current HP. Read-only: assigning it raises. It is recalculated whenever the pal is saved, so a value set here could not have been kept anyway; a dry run saves nothing, so this keeps reading what it read before any of the run's writes.
---@field rank integer The pal's condensing rank, 0-255.
---@field exp integer The pal's experience points.
---@field talent_hp integer HP talent value, 0-100.
---@field talent_shot integer Ranged-attack talent value, 0-100.
---@field talent_defense integer Defense talent value, 0-100.
---@field rank_hp integer HP soul rank, 0-255.
---@field rank_attack integer Attack soul rank, 0-255.
---@field rank_defense integer Defense soul rank, 0-255.
---@field rank_craftspeed integer Craft-speed soul rank, 0-255.
---@field is_boss boolean True for a boss/alpha pal. Read-only, and never true at the same time as is_lucky: a lucky pal carries the same BOSS_ prefix but is not a boss. That exclusion is applied when the pal is saved, so during a dry run this can still read true right after setting is_lucky = true on a boss pal.
---@field is_lucky boolean True for a lucky pal. Setting it false also removes the BOSS_ prefix from character_id, so the pal ends up plain rather than a boss; the write is refused, naming the species, when that prefix is part of the species' own name.
---@field is_awakened boolean True if this pal has been awakened.
---@field is_imported boolean True if this pal was imported from another save.
---@field is_predator boolean True for a predator-species pal. Read-only.
---@field is_tower boolean True for a tower-boss pal. Read-only.
---@field group_id string|nil The id of the group this pal belongs to, or nil if it belongs to none. Read-only.
---@field stomach number The pal's current fullness. Read-only: assigning it raises. The pal is fed back to full whenever it is saved, so a value set here could not have been kept anyway; a dry run saves nothing, so this keeps reading what it read before any of the run's writes.
---@field sanity number The pal's current sanity. Read-only: assigning it raises. It is restored to 100 whenever the pal is saved, so a value set here could not have been kept anyway; a dry run saves nothing, so this keeps reading what it read before any of the run's writes.
---@field max_hp integer The pal's maximum HP. Read-only: it is recalculated whenever the pal is saved, so during a dry run it does not move when level, rank or a talent changes.
---@field storage_slot integer This pal's slot number inside the container holding it. Read-only: assigning it raises rather than moving the pal. Nothing would check whether the slot you named was already taken, so allowing the write would risk putting two pals in the same place.
---@field storage_id string The id of the container holding this pal. Read-only.
---@field is_sick boolean True if this pal is sick. Read-only: sickness is cleared whenever the pal is saved, so during a dry run a sick pal keeps reading true.
---@field friendship_point integer The pal's friendship points.
---@field delete fun(): boolean Deletes this pal from its owning player or guild base. A structural write and invalidates every live handle and iterator across all scopes, including this one. Requires capability: save.write.
---@field set_level fun(level: integer): nil Sets the pal's level; must be between 1 and 255 inclusive, or this raises. A non-structural write: every live handle and iterator stays valid, but the cached pal snapshot this run holds is dropped and rebuilt on next field access, so the new level is visible immediately. Requires capability: save.write.
---@field set_talent fun(which: string, value: integer): nil Sets one soul (talent) value; `which` must be "hp", "shot" or "defense", and `value` must be between 0 and 100 inclusive, or this raises. Non-structural, with the same cached-snapshot refresh as set_level. Requires capability: save.write.

---Requires capability: save.read.
---
---@class guild
---@field id string The guild's UUID, as a string.
---@field name string The guild's name.
---@field admin_uid string|nil The UUID, as a string, of the guild's admin player, or nil if the guild has none.
---@field player_count integer How many players belong to this guild.
---@field base_count integer How many bases this guild has.
---@field level integer|nil The guild's level, or nil if the save has no level recorded for it.
---@field pal_count integer How many pals belong to this guild's bases.
---@field chest_container_id string|nil The UUID, as a string, of this guild's shared chest container, or nil if the guild has no chest.
---@field delete fun(): boolean Deletes this guild, its bases, and every loaded member player. An unloaded member is skipped, not deleted. A structural write and invalidates every live handle and iterator across all scopes, including this one. Requires capability: save.write.

---Requires capability: save.read.
---
---@class base
---@field id string The base's UUID, as a string.
---@field guild_id string|nil The UUID, as a string, of the guild this base belongs to, or nil if it could not be resolved.
---@field x number|nil The base's world X coordinate, or nil if its location could not be resolved.
---@field y number|nil The base's world Y coordinate, or nil if its location could not be resolved.
---@field z number|nil The base's world Z coordinate, or nil if its location could not be resolved.
---@field delete fun(): boolean Deletes this base and every pal working it, and updates its guild's base_count and pal_count. A structural write and invalidates every live handle and iterator across all scopes, including this one. Requires capability: save.write.

---Requires capability: save.read.
---
---@class container
---@field id string The container's UUID, as a string.
---@field slot_count integer|nil How many slots this container has, or nil if the container could not be read.
---@field slots fun(): (fun(): slot|nil) An iterator over every occupied slot in this container, for use in a `for` loop. Requires capability: save.read.
---@field set_slot_count fun(count: integer): boolean Resizes the container to hold `count` slots, returning true if it resized. Refuses (returns false, changes nothing) rather than destroying an occupied slot that shrinking would drop. A true result is a structural write and invalidates every live handle and iterator across all scopes, including this one. Requires capability: save.write.

---Requires capability: save.read.
---
---@class slot
---@field index integer This slot's position within its container.
---@field item_id string|nil The static item id occupying this slot, or nil if the slot is empty.
---@field count integer How many of the item occupy this slot.
---@field clear fun(): nil Empties this slot, removing its underlying entry rather than overwriting it in place. A structural write and invalidates every live handle and iterator across all scopes, including this one -- looping over container.slots() and calling clear() on each raises after the first clear; collect ids first instead. Requires capability: save.write.

---Requires capability: save.raw.
---
---@class raw

---@type raw
raw = {}

---Reads the raw scalar value at path in target ("level", "player:<uid>" or "player_dps:<uid>"). Returns nil when the node exists but is not a scalar (a struct, map, array, or opaque property). Raises if path does not resolve to anything at all -- use raw.exists to probe for that without raising.
---
---Requires capability: save.raw.
---@param target string
---@param path string
---@return nil|boolean|integer|number|string
function raw.get(target, path) end

---Whether path resolves to anything at all under target, scalar or not. Never raises.
---
---Requires capability: save.raw.
---@param target string
---@param path string
---@return boolean
function raw.exists(target, path) end

---The shape of the node at path under target -- one of "scalar", "struct", "map", "array", "entry" or "opaque" -- or nil when path does not resolve. Never raises.
---
---Requires capability: save.raw.
---@param target string
---@param path string
---@return string|nil
function raw.kind(target, path) end

---Overwrites the scalar at an EXISTING path in target with value; raises if path does not resolve or value cannot be converted to that node's type. Does not bump the mutation epoch -- nothing moves, so live handles and iterators stay valid -- but does force every cached pal field to be re-read on its next access, since a raw write cannot know whether it touched pal data.
---
---Requires capability: save.raw.
---@param target string
---@param path string
---@param value integer|number|boolean|string
---@return nil
function raw.set(target, path, value) end

---Removes the node at path in target, returning whether anything was actually removed. A true result is a structural write: it invalidates every live handle and iterator across every scope, not only ones touching the same target.
---
---Requires capability: save.raw.
---@param target string
---@param path string
---@return boolean
function raw.delete(target, path) end

---The element count of the array or map at path in target, or nil when the node exists but has no length. Raises if path does not resolve to anything at all.
---
---Requires capability: save.raw.
---@param target string
---@param path string
---@return integer|nil
function raw.len(target, path) end

---Walks every node under path in target depth-first, calling callback(node) for each with a table of { key, value, path, index, depth, kind }. callback may return "remove" to delete that node's subtree, "stop" to end the walk early, or anything else to keep walking. Returns a { visited, removed, stopped, removal_errors } summary. Any removal is a structural write: it invalidates every live handle and iterator, including ones the walk itself is still using, and even a removal queued before the callback later raises or the walk is stopped is still applied.
---
---Requires capability: save.raw.
---@param target string
---@param path string
---@param callback any
---@return table
function raw.visit(target, path, callback) end

---Requires capability: save.read.
---
---@class save

---@type save
save = {}

---A { world_name, save_id, player_count, guild_count, pal_count } summary of the loaded save.
---
---Requires capability: save.read.
---@return table
function save.info() end

---An iterator over every player in the save, for use in a `for` loop.
---
---Requires capability: save.read.
---@return fun(): player|nil
function save.players() end

---An iterator over every pal in the save, for use in a `for` loop. Building it walks every character entry once, so calling this repeatedly in a loop is needlessly expensive -- call it once and reuse the iterator.
---
---Requires capability: save.read.
---@return fun(): pal|nil
function save.pals() end

---An iterator over every guild in the save, for use in a `for` loop.
---
---Requires capability: save.read.
---@return fun(): guild|nil
function save.guilds() end

---An iterator over every guild base in the save, for use in a `for` loop.
---
---Requires capability: save.read.
---@return fun(): base|nil
function save.bases() end

---An iterator over every item container in the save, for use in a `for` loop.
---
---Requires capability: save.read.
---@return fun(): container|nil
function save.containers() end

---Calls predicate(slot) once for every item slot in the save with nothing yet mutated, then clears every slot predicate returned truthy for. Returns the number cleared, followed by the number examined. A non-zero clear count is a structural write and invalidates every live handle and iterator, including ones the walk itself was still using -- call this instead of looping over save.containers() and clearing slots one at a time, which a structural write would break after the first clear.
---
---Requires capability: save.write.
---@param predicate any
---@return integer
function save.clear_slots_where(predicate) end

---Clears the ownership lock on every private chest and item booth, returning how many were actually cleared. A non-zero result is a structural write and invalidates every live handle and iterator.
---
---Requires capability: save.write.
---@return integer
function save.unlock_private_chests() end

---Requires capability: gamedata.
---
---@class gamedata

---@type gamedata
gamedata = {}

---Whether the id names an item the loaded game data knows. Case-insensitive.
---
---Requires capability: gamedata.
---@param id string
---@return boolean
function gamedata.is_valid_item(id) end

---Whether the id names a pal the loaded game data knows. Case-insensitive.
---
---Requires capability: gamedata.
---@param id string
---@return boolean
function gamedata.is_valid_pal(id) end

---The version string of the loaded game data.
---
---Requires capability: gamedata.
---@return string
function gamedata.version() end

---@class progress

---@type progress
progress = {}

---Sends message (with an optional 0.0-1.0 completion fraction) to whatever is driving this run's progress UI, if anything is listening. The same sink also receives the host's own internal progress ticks from destructive domain calls this script triggers, so a listener can see more updates than the script explicitly sent.
---@param message string
---@param fraction? number
---@return nil
function progress.report(message, fraction) end

---@class ctx
---@field dry_run boolean Whether this run is a dry run: every write function predicts its effect and records a preview count instead of writing.
---@field api_version integer The api_version this plugin's manifest declares.
---@field plugin_id string This plugin's own id, from its manifest.
---@field command_id string The id of the command this run is executing.
---@field now integer The Unix timestamp, in seconds, of when this run started.
---@field args table This command's arguments, already coerced to the types its manifest declares, keyed by parameter name.

---@type ctx
ctx = {}

---Requires capability: log.
---
---@class log

---@type log
log = {}

---Appends an info-level line to this run's log, capped at 1000 lines total across log.info/warn/error combined; further calls after the cap are silent no-ops.
---
---Requires capability: log.
---@param message string
---@return nil
function log.info(message) end

---Appends a warning-level line to this run's log, subject to the same 1000-line cap as log.info.
---
---Requires capability: log.
---@param message string
---@return nil
function log.warn(message) end

---Appends an error-level line to this run's log, subject to the same 1000-line cap as log.info. Does not itself abort the run -- raise a Lua error for that.
---
---Requires capability: log.
---@param message string
---@return nil
function log.error(message) end

---Requires capability: storage.
---
---@class storage

---@type storage
storage = {}

---The value previously stored under key by this plugin, or nil if nothing has been stored under it. Storage is private per plugin.
---
---Requires capability: storage.
---@param key string
---@return string|nil
function storage.get(key) end

---Stores value under key for this plugin, visible to storage.get for the rest of this run immediately and to later runs once the host persists it. Raises if key exceeds 128 bytes or value exceeds 64 KiB. Unlike every save/raw write function, this is NOT skipped under a dry run -- it always writes.
---
---Requires capability: storage.
---@param key string
---@param value string
---@return nil
function storage.set(key, value) end

---Requires capability: ui.dialog.
---
---@class ui

---@type ui
ui = {}

---Shows message as a confirm dialog and returns whether the user accepted it. Under a dry run this always returns true without showing anything, so a dry run can predict the confirmed path. If nothing is listening for confirmations, this returns false.
---
---Requires capability: ui.dialog.
---@param message string
---@return boolean
function ui.confirm(message) end
