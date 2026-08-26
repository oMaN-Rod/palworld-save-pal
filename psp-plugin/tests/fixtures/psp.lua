---@meta

---Requires capability: save.read.
---
---@class player
---@field uid string The player's UUID, as a string. Read-only.
---@field name string The player's nickname. Neither an empty string nor the save's own placeholder for a nameless player can be assigned: both would read back as something other than what was written.
---@field level integer|nil The player's level, 1-255, or nil for a player the save records no level for at all. Assigning nil raises rather than clearing it: the save has no way to record a player with no level, so only an integer can be written.
---@field guild_id string|nil The UUID, as a string, of the guild this player belongs to, or nil if the player is in no guild. Read-only.
---@field pal_count integer How many pals this player owns. Read-only: it is derived by counting, not stored.
---@field last_online string|nil An ISO-8601 timestamp of when the player was last online, or nil if the save records none. Read-only.
---@field last_online_ts integer|nil The Unix timestamp, in seconds, of when the player was last online, or nil if the save records none. Read-only.
---@field instance_id string|nil The id of this player's own character entry, or nil if the save records none. Read-only. Reading it requires the players capability.
---@field exp integer The player's experience points. Reading it requires the players capability.
---@field hp integer The player's current HP. Unlike a pal's, this is written through as given rather than recalculated when the save is written. Reading it requires the players capability.
---@field stomach number The player's current fullness. Stored as a 32-bit float, so a value outside that range is refused rather than written as an infinity. Reading it requires the players capability.
---@field sanity number The player's current sanity. Stored as a 32-bit float, so a value outside that range is refused rather than written as an infinity. Reading it requires the players capability.
---@field technology_points integer Unspent technology points. Reading it requires the players capability.
---@field boss_technology_points integer Unspent ancient technology points. Reading it requires the players capability.
---@field technologies string[] The technologies this player has unlocked, as recipe names. Assigning replaces the whole list, and any string is accepted: nothing checks a name against the game's own technology list, here or on the way into the save. The read returns a fresh table each time, so changing that table changes nothing. Reading it requires the players capability.
---@field completed_missions string[] The quests this player has completed, as quest names. Assigning replaces the whole list, and any string is accepted: nothing checks a name against the game's own quest list, here or on the way into the save. Reading it requires the players capability.
---@field current_missions string[] The quests this player has in progress, as quest names. Assigning replaces the whole list; each name becomes a fresh quest entry with no progress recorded against it. Reading it requires the players capability.
---@field unlocked_fast_travel_points string[] The fast-travel points this player has unlocked, as flag keys. Assigning replaces the whole set. Reading it requires the players capability.
---@field collected_effigies string[] The Lifmunk effigies this player has collected, as flag keys. Assigning replaces the whole set, and moves effigy_possess_num by the number of keys newly collected minus the number un-collected, never below zero -- so it counts unspent effigies, not collected ones, and un-collecting more than are unspent leaves it at zero rather than going negative. That move is not immediate; effigy_possess_num says when it becomes readable. Reading it requires the players capability.
---@field effigy_possess_num integer How many unspent Lifmunk effigies this player holds -- not how many they have collected, since spending one does not un-collect it. Read-only in itself: it moves when collected_effigies is assigned, and only then. It also lags that assignment, because the count is recomputed where the player is written back rather than where the list is assigned: a read in between still answers the old number, and a dry run, which never writes the player back, answers the old number for the whole run. Reading it requires the players capability.
---@field defeated_bosses string[] The bosses this player has defeated, as flag keys, with the tower bosses merged in. Read-only. Reading it requires the players capability.
---@field status_point_list table<string, integer> Points spent on each base stat, keyed by max_hp, max_sp, attack, weight, capture_rate, work_speed, hunger_reduction, swim_speed, food_decay_reduction, jump_power, glider_speed, climb_speed, status_ailment_resist, exp_bonus, rainbow_passive_rate, move_speed, sphere_homing and stamina_reduction. Assigning replaces the whole map: a key you leave out is set to zero, which is the only way the save can express "no points spent" -- there is no way to remove a stat once the save carries one. A key the save has never carried and that you leave out (or assign zero) stays absent and reads back nil. A key this map does not know is refused rather than silently dropped. Reading it requires the players capability.
---@field ext_status_point_list table<string, integer> Points spent on each extended stat, keyed by max_hp, max_sp, attack, weight and work_speed -- the base-stat keys minus capture_rate, which the extended list has no entry for. Assigning replaces the whole map on the same terms as status_point_list, and a key it does not know is refused. Reading it requires the players capability.
---@field pal_box_id string|nil The id of this player's pal box container, or nil if the save records none. Read-only. Reading it requires the players capability.
---@field otomo_container_id string|nil The id of this player's party container, or nil if the save records none. Read-only. Reading it requires the players capability.
---@field common_container_id string|nil The id of the player's main inventory container, or nil if it could not be read. Read-only. Reading it requires the players capability.
---@field essential_container_id string|nil The id of the player's key-items container, whose `AdditionalInventory_` entries decide how large the main inventory should be. Read-only. Reading it requires the players capability.
---@field pals fun(): (fun(): pal|nil) An iterator over every pal this player owns, for use in a `for` loop. Requires capability: save.read.
---@field delete fun(): boolean Deletes this player, along with the item and character containers the player owns. Refuses (returns false, changes nothing) if the player is their guild's admin. A true result is a structural write and invalidates every live handle and iterator across all scopes, including this one. Requires capability: save.write.

---Requires capability: save.read.
---
---@class pal
---@field instance_id string This pal's unique id, as a string. Read-only.
---@field character_id string The pal's species id, including a BOSS_ prefix when it has one. Read-only in itself, but writing is_lucky changes it: setting is_lucky = false removes a BOSS_ prefix that only marked the pal lucky, and setting it true puts one back when the change is saved. A dry run saves nothing, so after is_lucky = true this still reads the unprefixed id.
---@field character_key string The pal's species key, as used to look up game data. Read-only, and refreshed only when the pal is next read from the save, so during a dry run it can still name the old species after a write that changed character_id.
---@field nickname string|nil The pal's nickname, or nil if it has none. The nil is a read-side answer only: assigning nil raises rather than clearing the nickname.
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
---@field learned_skills string[] Every active skill this pal has learned, as catalog ids like "EPalWazaID::FireBall", spelled exactly as the catalog spells them. Assigning replaces the whole list, and every entry must be an active-skill id; any id in that catalog is accepted, including a species-specific skill belonging to some other pal, which the in-app skill picker would not offer you. The read returns a fresh table each time, so changing that table changes nothing.
---@field active_skills string[] The active skills this pal has equipped, as catalog ids like "EPalWazaID::FireBall", spelled exactly as the catalog spells them. Assigning replaces the whole list; any id in that catalog is accepted, including a species-specific skill belonging to some other pal, which the in-app skill picker would not offer you. The read returns a fresh table each time, so changing that table changes nothing.
---@field passive_skills string[] The passive skills this pal carries, as catalog ids like "Rare", spelled exactly as the catalog spells them. Assigning replaces the whole list; the read returns a fresh table each time, so changing that table changes nothing.
---@field work_suitability table<string, integer> Work-suitability ranks added on top of the species' own, keyed by the wire names EmitFlame, Watering, Seeding, GenerateElectricity, Handcraft, Collection, Deforest, Mining, OilExtraction, ProductMedicine, Cool, Transport and MonsterFarm. Assigning replaces the whole map, and a key it does not know is refused. Assigning a rank of zero removes that key instead of storing it, so it reads back absent straight away, and saving never stores a zero either. A save written by some other tool can still hold one, and reading that pal gives you the zero it holds.
---@field delete fun(): boolean Deletes this pal from its owning player or guild base. A structural write and invalidates every live handle and iterator across all scopes, including this one. Requires capability: save.write.

---Requires capability: save.read.
---
---@class guild
---@field id string The guild's UUID, as a string. Read-only.
---@field name string The guild's name. An empty string cannot be assigned: the save reads one as "leave the name alone", so the assignment would change nothing rather than clearing it.
---@field admin_uid string|nil The UUID, as a string, of the guild's admin player, or nil if the guild has none. Read-only.
---@field player_count integer How many players belong to this guild. Read-only: it is derived by counting, not stored.
---@field base_count integer How many bases this guild has. Read-only: it is derived by counting, not stored.
---@field level integer The guild's base-camp level. Never nil: the guild tail stores it as a plain integer with no way to record its absence, so a guild that has a handle at all has a level. Zero cannot be assigned: the save reads one as "leave the level alone", so the assignment would change nothing. Assigning nil raises for the same reason.
---@field pal_count integer How many pals belong to this guild's bases. Read-only: it is derived by counting, not stored.
---@field chest_container_id string|nil The UUID, as a string, of this guild's shared chest container, or nil if the guild has no chest. Read-only: it is resolved from the save itself, so a plugin cannot redirect a chest edit by assigning a different id.
---@field delete fun(): boolean Deletes this guild, its bases, and every loaded member player. An unloaded member is skipped, not deleted. A structural write and invalidates every live handle and iterator across all scopes, including this one. Requires capability: save.write.

---Requires capability: save.read.
---
---@class base
---@field id string The base's UUID, as a string. Read-only.
---@field guild_id string|nil The UUID, as a string, of the guild this base belongs to, or nil if it could not be resolved. Read-only.
---@field name string|nil The base's name, or nil if the save holds no base camp record for this base -- the same case x, y and z read nil for. Newly built bases carry a generated template name rather than one the player chose. An empty string cannot be assigned: the save reads one as "leave the name alone", so the assignment would change nothing rather than clearing it. Assigning nil raises: the nil is an answer about the save's record, not a value that can be written.
---@field area_range number|nil The radius, in world units, of the base's working area, or nil in the same case name reads nil. Stored as a 32-bit float, so a value outside that range is refused rather than written as an infinity, and one that range cannot hold exactly reads back rounded to what the save will actually hold. No other bound is enforced: zero and negative radii are accepted and written as given, because nothing in the game's data or in this app establishes what a legal radius is, and refusing them here would be inventing a rule rather than reporting one. Assigning nil raises, exactly as it does for name: the nil is an answer about the save's record, not a value that can be written.
---@field x number|nil The base's world X coordinate, or nil if its location could not be resolved. Read-only: nothing in this app writes a base's position, so there is no write path to offer.
---@field y number|nil The base's world Y coordinate, or nil if its location could not be resolved. Read-only, for the same reason as x.
---@field z number|nil The base's world Z coordinate, or nil if its location could not be resolved. Read-only, for the same reason as x.
---@field delete fun(): boolean Deletes this base and every pal working it, and updates its guild's base_count and pal_count. A structural write and invalidates every live handle and iterator across all scopes, including this one. Requires capability: save.write.

---Requires capability: save.read.
---
---@class container
---@field id string The container's UUID, as a string. Read-only.
---@field slot_count integer|nil How many slots this container has, or nil if the container could not be read. Cannot be assigned: resizing a container is a structural write that invalidates every live handle and iterator, so it stays container.set_slot_count(n), which reports whether it resized and refuses rather than destroying an occupied slot.
---@field slots fun(): (fun(): slot|nil) An iterator over every occupied slot in this container, for use in a `for` loop. Requires capability: save.read.
---@field set_slot_count fun(count: integer): boolean Resizes the container to hold `count` slots, returning true if it resized. Refuses (returns false, changes nothing) rather than destroying an occupied slot that shrinking would drop. A true result is a structural write and invalidates every live handle and iterator across all scopes, including this one. Requires capability: save.write.

---Requires capability: save.read.
---
---@class slot
---@field index integer This slot's position within its container. Read-only: moving a slot means removing and re-adding its entry, which is structural.
---@field item_id string|nil The static item id occupying this slot, or nil if the slot is empty. Must name an item the loaded game data knows, matched case-insensitively and stored exactly as written; an id the catalog does not hold raises, and the check is skipped entirely when no catalog is loaded. Assigning "None" raises: it is the one value the save reads as "delete this slot", which is structural -- use slot.clear(). Assigning nil or an empty string raises too: both read back as an empty slot without emptying one, leaving an entry holding an item with no id. Assigning on a slot that carries a per-item record (durability, an egg's pal, a weapon's passives) also raises: the record names its own item and cannot be re-pointed here.
---@field count integer How many of the item occupy this slot. Must be at least 1: a slot holding none of its item is an empty slot, and emptying one is structural -- use slot.clear(). No upper bound beyond what the save can hold, because nothing in the game's data or in this app establishes a stack limit, and refusing one here would be inventing a rule rather than reporting one.
---@field clear fun(): nil Empties this slot, removing its underlying entry rather than overwriting it in place. A structural write and invalidates every live handle and iterator across all scopes, including this one -- looping over container.slots() and calling clear() on each raises after the first clear; collect ids first instead. Requires capability: save.write.

---Requires capability: save.read.
---
---@class map_object
---@field id string The MapObjectId asset name, shared by every instance of this kind. Read-only.
---@field instance_id string This instance's UUID, as a string -- unique even among map objects that share an id. Read-only.
---@field base_id string|nil The UUID, as a string, of the base this object belongs to, or nil if it is unattached. Read-only.
---@field guild_id string|nil The UUID, as a string, of the guild this object belongs to, or nil if it has none. Read-only.
---@field build_player_uid string|nil The UUID, as a string, of the player who built this object, or nil if it has none. Assigning nil clears it; assigning a uuid string sets it, without checking that the uuid names a player who exists.
---@field hp integer This object's current hit points. Any 32-bit integer is accepted, including zero or a negative value -- lowering a structure's hp is a legitimate write, and it is not clamped to max_hp.
---@field max_hp integer This object's maximum hit points. Read-only.
---@field kind string The concrete model type name this object was built from. Read-only.

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

---An iterator over every built structure, chest and resource node in the save, for use in a `for` loop.
---
---Requires capability: save.read.
---@return fun(): map_object|nil
function save.map_objects() end

---Calls predicate(slot) once for every item slot in the save with nothing yet mutated, then clears every slot predicate returned truthy for. Returns the number cleared, followed by the number examined. A non-zero clear count is a structural write and invalidates every live handle and iterator, including ones the walk itself was still using -- call this instead of looping over save.containers() and clearing slots one at a time, which a structural write would break after the first clear.
---
---Requires capability: save.write.
---@param predicate any
---@return integer
---@return integer
function save.clear_slots_where(predicate) end

---Clears the ownership lock on every private chest and item booth, returning how many were actually cleared. A non-zero result is a structural write and invalidates every live handle and iterator.
---
---Requires capability: save.write.
---@return integer
function save.unlock_private_chests() end

---Heals every pal in the world, recomputing its HP from its level, talents, condensing rank and awakening, restoring its sanity and fullness and clearing any sickness, then gives an ownerless pal the owner of the container holding it. Returns how many pals were restored, followed by how many owners were assigned. Every value is written in place, so live handles and iterators stay valid. Resolving those owners loads every player's save data, under a dry run too, so the call's cost and memory both scale with how many players the save has. Does not touch dimensional storage.
---
---Requires capability: save.write.
---@return integer
---@return integer
function save.restore_pals() end

---Removes every WorkSaveData entry whose owning map object no longer exists, returning how many were removed. A non-zero result is a structural write and invalidates every live handle and iterator.
---
---Requires capability: save.write.
---@return integer
function save.remove_orphaned_works() end

---Empties the given slot indexes of one player's dimensional storage in place -- nils the slot's InstanceId and resets its SaveParameter bag to an unused slot's shape, the same way the slot got there in the first place, without changing the storage array's length. Returns how many of the given indexes were valid. Requires capability: players.
---
---Requires capability: save.write.
---@param player_uid string
---@param indexes integer[]
---@return integer
function save.delete_dps_pals(player_uid, indexes) end

---Removes every DynamicItemSaveData entry that no item-container slot, dropped item, item booth trade or damage-drop table still points at, returning how many were removed. A non-zero result is a structural write and invalidates every live handle and iterator.
---
---Requires capability: save.write.
---@return integer
function save.remove_orphaned_dynamic_items() end

---Mints a per-item record for every container slot whose record has gone missing, returning how many were minted. Without this, such a slot is invisible to container reads and is deleted the next time its container is written. The minted record carries default condition, not the item's original durability. A non-zero result is a structural write and invalidates every live handle and iterator.
---
---Requires capability: save.write.
---@return integer
function save.repair_item_links() end

---Reassigns every pal to the guild that should own it, taken from its owning player's guild or from the base whose worker container holds it. Returns how many were reassigned, followed by how many could not be resolved. A pal that resolves to neither is left exactly as it was, never orphaned.
---
---Requires capability: save.write.
---@return integer
---@return integer
function save.rebuild_guild_membership() end

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

---The names of every top-level catalog the loaded game data ships, sorted. The nested subtrees it also loads, locale and interface strings, are not listed.
---
---Requires capability: gamedata.
---@return string[]
function gamedata.catalogs() end

---The top-level keys of the named catalog, or nil if no catalog by that name exists. Catalog names are matched case-insensitively.
---
---Requires capability: gamedata.
---@param catalog string
---@return string[]|nil
function gamedata.keys(catalog) end

---The named catalog, or one entry of it if key is given, or nil if the catalog or key does not exist. A stored JSON null also arrives as nil, indistinguishable from absent. Catalog names are matched case-insensitively.
---
---Requires capability: gamedata.
---@param catalog string
---@param key? string
---@return any|nil
function gamedata.get(catalog, key) end

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
