# Plugin API reference

This is a reference for writing a Palworld Save Pal plugin, not a tutorial.
It documents the manifest schema, the full host API surface, the capability
model, the sandbox limits, and the mutation-during-iteration rule, then walks
through one real bundled command. Where this document and the code disagree,
the code (`psp-plugin/src/host/`, `psp-plugin/src/manifest.rs`,
`psp-plugin/src/sandbox.rs`) is correct.

A plugin is a `manifest.json` plus one or more `.lua` source files. It can be
installed from the UI either as a single bare `.lua` file (which gets a
manifest synthesised for it — see below) or as a `.zip` containing
`manifest.json` and its sources.

## Manifest schema

```json
{
  "id": "pst.cleanup",
  "api_version": 1,
  "name": "Save Cleanup",
  "version": "1.0.0",
  "author": "PST",
  "license": "GPL-3.0-only",
  "entry": "main.lua",
  "capabilities": ["save.read", "save.write", "gamedata", "log"],
  "commands": [
    {
      "id": "delete_empty_guilds",
      "title": "Delete Empty Guilds",
      "description": "Removes guilds that have no remaining members.",
      "destructive": true,
      "params": []
    }
  ]
}
```

| Field | Type | Required | Rule |
|---|---|---|---|
| `id` | string | yes | 1-64 characters, lowercase letters, digits, and single `.` `_` `-` separators. Cannot start or end with a separator, and separators cannot repeat (`a..b` is invalid). This is also the plugin's row key — an install with an id that already names a bundled plugin is refused. |
| `api_version` | integer | yes | Must equal the runtime's supported version (`1` today). A manifest declaring any other value is refused before anything is parsed further. |
| `name` | string | yes | Non-empty after trimming. |
| `version` | string | yes | Non-empty after trimming. Free-form (not semver-checked). |
| `author` | string | no | Free-form. |
| `license` | string | no | Free-form. |
| `entry` | string | yes | A plain `.lua` filename — no `/`, `\`, or `:`, not `.` or `..`, must end in `.lua` and be longer than just `.lua`. Must name a key present in the plugin's `sources`. |
| `capabilities` | array of strings | no (defaults to none) | Each must be one of the eight names in the capability table below. No duplicates. `save.raw` is refused unless the plugin's origin is bundled (see below). `save.write` is refused unless `save.read` is also declared. |
| `commands` | array of command objects | no (defaults to none) | See below. |

### Command objects

| Field | Type | Required | Rule |
|---|---|---|---|
| `id` | string | yes | Must be usable as a Lua global identifier (starts with a letter or `_`, then letters/digits/`_`) and must not be a Lua reserved word. The script must define a top-level `function <id>() ... end` with this exact name — the runtime looks it up as a global by that name when the command runs. No two commands on one manifest may share an id. |
| `title` | string | yes | Shown in the UI. |
| `description` | string | no | Shown in the UI. Should describe only what the command does — see the worked example below. |
| `destructive` | bool | no (default `false`) | Purely descriptive; the runtime does not gate anything on it. The UI uses it to decide whether to offer a dry run. |
| `params` | array of param objects | no (defaults to none) | See below. |

### Param objects

| Field | Type | Required | Rule |
|---|---|---|---|
| `id` | string | yes | A valid Lua identifier, not a reserved word, unique within the command. This becomes the key under `ctx.args`. |
| `type` | string | yes | One of `int`, `float`, `string`, `bool`, `enum`. |
| `label` | string | yes | Shown in the UI. |
| `description` | string | no | Shown in the UI. |
| `default` | JSON value | no | Used when the caller omits (or sends JSON `null` for) this argument. Its JSON type must match `type` (an integer for `int` — a float with a fractional part or out of `i64` range is rejected; a string that is one of `options` for `enum`; etc). If there is no default and the caller supplies nothing, the run is refused before the script starts. |
| `min` / `max` | number | no | Inclusive bounds, checked for `int` and `float` only. `min` must not exceed `max`. |
| `options` | array of strings | only for `enum` | Must be non-empty for an `enum` param. The supplied (or default) value must be one of these, compared as an exact string match. |

Argument coercion (`run_plugin_command`'s `args`) happens once, before the
script runs: every declared param is resolved from the supplied JSON object,
falls back to its default, is type- and range-checked, and is written into
`ctx.args`. An argument key the command does not declare is refused outright —
there is no silent pass-through of extra fields.

### Bare `.lua` install

Installing a single `.lua` file (rather than a `.zip`) synthesises a manifest
for it:

- The script must define a top-level `function main()`. A plain source-text
  scan looks for this — not a full Lua parse — so it can be fooled by a
  `function main` inside a comment or string, but not by common formatting.
- `id` is the filename's stem, slugified (lowercased, runs of non-alphanumeric
  characters collapsed to a single `-`, trimmed, capped at 64 characters).
- `capabilities` is always empty. A bare-`.lua` install has no way to request
  any capability — if the script needs `log`, `gamedata`, or save access, ship
  it as a `.zip` with an explicit manifest instead.
- `commands` is exactly one entry: `{ "id": "main", "title": "Main" }`.

## Capabilities

A capability that is not both declared in the manifest *and* granted at run
time installs no global at all — a script that references it fails with "field
X is nil" at the point of use, not with a capability error. This is
deliberate: the set of installed globals is exactly the set of things the
script is actually allowed to touch.

| Capability | Wire name | Installs |
|---|---|---|
| Save read | `save.read` | The `save` global's read half: `save.info()`, `save.players()`, `save.pals()`, `save.guilds()`, `save.bases()`, `save.containers()`, and every handle field read. |
| Save write | `save.write` | The `save` global's write half: `delete_where` on the player/guild/pal iterators, `save.clear_slots_where()`, and every handle's mutating methods (`player.delete()`, `pal.set_level()`, `slot.clear()`, ...). Requires `save.read` to also be declared — the manifest is refused otherwise. |
| Save raw | `save.raw` | The `raw` global (`raw.get`/`exists`/`kind`/`set`/`delete`/`len`/`visit`). **Bundled plugins only in v1** — see below. |
| Players | `players` | Installs nothing of its own. Gates the `player:<uid>` and `player_dps:<uid>` raw targets — `raw.get`/`raw.set`/etc. against a per-player scope refuse with an error unless this is also granted. `raw` itself still needs `save.raw`. |
| Game data | `gamedata` | The `gamedata` global (`gamedata.is_valid_item()`, `gamedata.is_valid_pal()`, `gamedata.version()`). |
| UI dialog | `ui.dialog` | The `ui` global (`ui.confirm()`). |
| Storage | `storage` | The `storage` global (`storage.get()`, `storage.set()`). |
| Log | `log` | The `log` global (`log.info()`, `log.warn()`, `log.error()`). |

`progress` and `ctx` are installed unconditionally for every run — reporting
progress and reading run metadata cannot leak anything, so neither needs a
capability.

A capability also has to appear in the manifest's own `capabilities` array to
take effect: `run_plugin_command`'s `granted` list is intersected against the
manifest's declared capabilities before installation, so a caller cannot grant
a plugin more than it declared it wants.

### Why `save.raw` is bundled-only

Manifest validation refuses `save.raw` outright for a plugin whose origin is
`User` (`ManifestError::RawIsBundledOnly`) — installing a `.zip` or `.lua` file
through the UI always produces a `User`-origin plugin, so no user-installed
plugin can ever declare it, regardless of what its manifest asks for.

The reason is that raw access is unaudited mutation of save internals with no
schema validation behind it. It is also the main road for a real PalworldSaveTools
port — most of PST's own functions read and write the level tree directly, not
through a typed model — so it cannot simply be left out of the API. Bundled
scripts carry the same review burden as any other Rust or Lua code in this
repository, so they may use it; handing that same access to arbitrary
third-party code before there is a signing and review process would make a
corrupted save report a matter of when, not if. This is revisited once a
plugin repository with review exists.

## The host API

### `ctx` — always installed

A plain table, rebuilt once per run before the script's command function is
called:

| Field | Type | Meaning |
|---|---|---|
| `ctx.dry_run` | bool | Whether this run is a dry run. Every mutating host function checks this itself — a script does not need to branch on it except to change its own wording (see the worked example). Branching the *logic* on it means maintaining two implementations of one count, which is how the two drift apart; reach for a bulk form whose predicate pass is mode-independent instead. |
| `ctx.api_version` | integer | The manifest's declared `api_version`. |
| `ctx.plugin_id` | string | The manifest's `id`. |
| `ctx.command_id` | string | The command id this run invoked. |
| `ctx.now` | integer | Unix seconds, sampled once before the script runs. Lua's `os` library is excluded from the sandbox, so this is the only clock a script has. |
| `ctx.args` | table | The command's declared parameters, coerced and defaulted, keyed by their `id`. |

### `progress` — always installed

- `progress.report(message, fraction)` — `fraction` is optional; when given it
  must be a finite number in `[0.0, 1.0]` or the call errors. Reports go to the
  same progress sink `psp-core`'s own domain calls use, so a consumer can see
  more updates than the script explicitly sent — treat it as "more updates
  than expected," not a 1:1 count with `report` calls.

### `log` — requires `log`

- `log.info(message)`, `log.warn(message)`, `log.error(message)` — each
  appends one line to the run's log, returned to the caller alongside the
  result. Capped at 1000 lines; the 1000th line is replaced with a single
  truncation notice and every call after that is a silent no-op.

### `storage` — requires `storage`

Per-plugin key/value persistence across runs.

- `storage.get(key) -> string|nil`
- `storage.set(key, value)` — `key` is capped at 128 bytes, `value` at 64 KiB;
  either limit is a hard error. Writes are buffered in memory for the run and
  only persisted by the caller once the run finishes with an `ok` status —
  a failed or timed-out run's storage writes are discarded.

### `ui` — requires `ui.dialog`

- `ui.confirm(message) -> bool` — under a dry run this always returns `true`
  without prompting anything (a dry run never actually needs confirmation,
  since nothing is written). On a real run it calls back into the host's
  confirmation channel if one was supplied, and returns `false` if not.

### `gamedata` — requires `gamedata`

- `gamedata.is_valid_item(id) -> bool` — `id` is the item's internal catalog
  id (e.g. `Wood`), matched exactly, never a display name.
- `gamedata.is_valid_pal(id) -> bool` — `id` is the pal's internal catalog id
  (e.g. `CuteFox`), matched case-insensitively (a save's `character_id` casing
  does not always match the catalog).
- `gamedata.version() -> string`

### `save` — read half requires `save.read`

- `save.info() -> { world_name, save_id, player_count, guild_count, pal_count }`
- `save.players()`, `save.pals()`, `save.guilds()`, `save.bases()`,
  `save.containers()` — each returns a stateless Lua iterator over handles, for
  use as `for x in save.players() do ... end`.

Every handle exposes fields through `__index` (a plain field read, not a
method call — `player.name`, not `player.name()`):

| Handle | Fields |
|---|---|
| `player` | `uid`, `name`, `level`, `guild_id`, `pal_count`, `last_online` (ISO string or nil), `last_online_ts` (Unix seconds or nil), `pals` (a `player.pals()` iterator factory) |
| `pal` | `instance_id`, `character_id`, `nickname`, `owner_uid`, `guild_id`, `base_id`, `gender`, `level`, `hp`, `rank`, `exp`, `talent_hp`, `talent_shot`, `talent_defense`, `rank_hp`, `rank_attack`, `rank_defense`, `rank_craftspeed`, `is_boss`, `is_lucky` |
| `guild` | `id`, `name`, `admin_uid`, `player_count`, `base_count`, `level`, `pal_count`, `chest_container_id` (a container id string, or `nil` when the guild has no chest) |
| `base` | `id`, `guild_id`, `x`, `y`, `z` |
| `container` | `id`, `slot_count`, `slots` (a `container.slots()` iterator factory) |
| `slot` | `index`, `item_id` (nil for an empty slot), `count` |

An unresolvable field name returns `nil` rather than erroring — reading a
typo'd field looks like reading an absent one.

### `save` — write half requires `save.write` (and therefore also `save.read`)

Adds mutating methods onto the same handles, plus a bulk `delete_where` on the
three entity iterators that support it:

- `player.delete() -> bool` — refuses (returns `false`, does not error) rather
  than deleting a guild admin.
- `player.set_level(n)` — `n` must be `1..=255`.
- `pal.delete() -> bool`
- `pal.set_level(n)` — `n` must be `1..=255`.
- `pal.set_talent(which, value)` — `which` is `"hp"`, `"shot"`, or `"defense"`;
  `value` must be `0..=100`.
- `guild.delete() -> bool` — also deletes every loaded member's player entity.
- `base.delete() -> bool` — also deletes the base's worker pals.
- `slot.clear()` — **structural**, not an in-place empty: it removes the raw
  slot entry rather than overwriting it, so every later slot in that container
  shifts down one position. This invalidates the `container.slots()` iterator
  that produced the handle, exactly like any other structural write — see the
  mutation rule below.
- `container.set_slot_count(n) -> bool` — resizes the container to hold `n`
  slots. Returns `true` when it resized, `false` when it **refused** because
  an occupied slot falls outside the new capacity — a refusal writes nothing.
  Raises a catchable error if the container id no longer resolves. Growing
  raises the capacity without creating slot entries: the raw `Slots` array is
  sparse, holding an entry only for an occupied slot, so an empty slot is
  represented by absence, the same representation `slot.clear()` produces.
  A *successful* call is **structural**, exactly like `slot.clear()` — it
  bumps the mutation epoch and invalidates every live handle and iterator, so
  reading a field off the same handle right afterward raises; re-fetch it
  instead. A refusal does not bump the epoch and leaves every handle valid.
- `save.players():delete_where(fn(player) -> bool)`,
  `save.guilds():delete_where(fn(guild) -> bool)`,
  `save.pals():delete_where(fn(pal) -> bool)` — collects a snapshot of every
  id first, calls `fn` once per id with a handle (no save mutation may happen
  from inside the predicate — see below), then deletes every id the predicate
  returned truthy for. Returns two integers: `removed, unresolved`.
  `unresolved` counts ids the predicate selected but that the host itself
  could not resolve at apply time (e.g. an already-dangling reference) — a
  different thing from the predicate choosing to skip an id itself, and worth
  keeping as a separate count in a command's own summary (see the worked
  example).
- `save.clear_slots_where(fn(slot) -> bool) -> cleared, examined` — the slot
  counterpart to `delete_where`, and the only practical way to clear more than
  one slot in a run. Walks every container once, calls `fn` with a slot handle
  while nothing has been mutated yet, then applies every selected clear
  afterwards, grouped so each container is written once. `examined` is the
  number of slots the predicate saw, `cleared` the number it selected. It hangs
  off `save` rather than off `save.containers()` because that iterator is a
  plain function value, which Lua gives you nowhere to attach a method to.
- `save.unlock_private_chests() -> integer` — clears the ownership lock
  (`private_lock_player_uid`) on every locked private chest and item booth in
  the world, and returns the number of locks **actually cleared**; a chest
  that was already unlocked is not counted. Deliberately does **not** touch
  `PasswordLock` module state (its `password`/`player_infos` fields) — that is
  a separate access-control mechanism, and clearing it is a larger change than
  a user asking to unlock chests has consented to. It is **structural**, and
  bumps the mutation epoch, exactly like `slot.clear()` and
  `container.set_slot_count()` — but only when the count is non-zero; a run
  that clears nothing (including any dry run, which never writes) leaves every
  handle and iterator valid.

Every write method's dry-run behaviour is: validate what it can, bump an
internal count, and return without changing anything. `slot.clear()` under a
dry run is a documented no-op, which is a trap for any loop that clears until a
container is empty: the loop's exit condition never arrives and the run times
out instead. `save.clear_slots_where` has no such split — its predicate pass is
identical in both modes and only the apply phase is skipped — so prefer it over
hand-rolling the loop, and reach for `ctx.dry_run` branching only if you must.

### `raw` — requires `save.raw` (bundled plugins only)

Direct GVAS tree access. Still gated on `save.raw`, and per-player targets
also require `players`.

```lua
raw.get(target, path)              -- read one scalar; errors if path does not resolve
raw.exists(target, path) -> bool   -- true when path resolves to anything at all
raw.kind(target, path) -> string|nil  -- "scalar"|"struct"|"map"|"array"|"entry"|"opaque", or nil
raw.set(target, path, value)       -- write one scalar at that path
raw.delete(target, path) -> bool   -- remove a key/element; false (not an error) if absent
raw.len(target, path)              -- element count of a map/array; errors if path does not resolve
raw.visit(target, path, fn)        -- host-driven depth-first walk; fn(node) returns "keep"|"remove"|"stop"
```

`target` is one of:

- `"level"` — the level file's GVAS root.
- `"player:<uid>"` — one player's own `.sav` tree. Requires `players`.
- `"player_dps:<uid>"` — that player's `_dps.sav` tree. Requires `players`.

`path` is a dotted/indexed address:
`worldSaveData.CharacterSaveParameterMap[3].value.RawData.SaveParameter.IsPlayer`
— a segment is a property key, or `[n]` for a map/array index.

Error discipline: `raw.get` and `raw.len` **error** when `path` does not
resolve to any node at all (a typo'd segment or an out-of-range index) — a
literal path string is exactly the kind of typo that needs to fail loudly at
the point of use. `raw.exists` and `raw.kind` are the deliberate probes for
"is anything here" and "what shape is it", and never error on an unresolved
path — they answer `false`/`nil`. `raw.delete` returns `false` (not an error)
when nothing was there; scripts rely on that as a meaningful outcome, not an
error case.

`raw.visit`'s callback receives one table per visited node:

| Field | Meaning |
|---|---|
| `node.key` | The node's own key/field name, or `nil` at the root. |
| `node.value` | The node's scalar value, or `nil` if it is not a scalar (a struct/map/array/opaque node). |
| `node.path` | A ready-made, re-parseable path string for this node, usable directly with `raw.get`/`raw.set`/`raw.delete` — or `nil` when the node cannot be faithfully addressed (an unfaithful path would be worse than none). |
| `node.index` | The node's index within its parent array/map, or `nil`. |
| `node.depth` | Depth from the walk's starting path. |
| `node.kind` | `"scalar"`, `"struct"`, `"map"`, `"array"`, `"entry"`, or `"opaque"`. |

The callback returns one of `"keep"` (default — anything other than `"remove"`
or `"stop"` is treated as `"keep"`), `"remove"`, or `"stop"` (stop the walk
immediately). `raw.visit` returns
`{ visited, removed, stopped, removal_errors }`. Under a dry run, a
`"remove"` action is applied to the traversal (so counts match a real run
exactly) but never actually queued for writing.

Nested `raw.visit` calls, and calling `raw.visit`/`delete_where` from inside
another one's callback, are refused with an error rather than silently
reentering.

## The API definition

Every global table, its functions, and every handle and its fields also
exist as data, not just as prose. `psp_plugin::host::api_def::api_definition()`
returns an `ApiDefinition` built from the same Rust source that registers
these functions with Lua — not maintained separately from it — describing
every global table, every function's parameters, return type, and doc string,
every handle type and its fields, and the capability each one requires. It
serialises to JSON, adjacently tagged so a consumer can discriminate on a
single `kind` field per type variant (a type's own key on the wire is named
`type`). The same definition can also be rendered as a `---@meta` file
(`psp_plugin::host::api_meta::lua_meta`) in the annotation format
`lua-language-server` reads, for anyone who wants type information for the
host API in their own Lua tooling.

The reason to trust either output is a test, not a promise: it installs every
host global into a real Lua state with every capability granted, asks Lua
itself what it can see, and compares that against the definition in both
directions — everything Lua exposes is described, and everything described
actually exists in Lua. A drift between the two fails that test, not a
review.

Handle fields get a further check: every field this document lists under a
handle is probed against a live handle from a real save fixture and confirmed
to resolve without error, with a value whose Lua type matches what's
described. That check only runs one way, though. It cannot also prove the
reverse — that nothing a handle answers goes undescribed — because a handle's
fields are dispatched through an `__index` function rather than a plain
table, with no `__pairs` to enumerate them, so Lua has no way to ask a handle
what it holds, only to ask about one name at a time. An unrecognised field
name comes back as plain `nil`, indistinguishable from a legitimately-nilable
field that just happens to be absent on this instance — so a phantom field
only gets caught here if the description also claims it is never nil. In
short: a field listed under a handle above is proven to exist with the type
stated; a name not listed there is not thereby proven absent.

## The mutation-during-iteration rule

A **handle** (returned by `save.players()`, `player.pals()`,
`save.containers()`, etc.) carries the mutation epoch that was current when it
was created. Every structural change — anything that adds or removes an
entity, including `slot.clear()` — bumps that epoch. A handle whose epoch no
longer matches the run's current epoch is refused on its next field read or
method call with:

> this handle was invalidated by a change made during iteration; use a bulk
> form such as `delete_where`

This is why `for p in save.players() do if should_delete(p) then p.delete()
end end` is unsafe in general (deleting one player invalidates every handle
still live from that same iterator, including the one just used) and why the
bulk forms exist: `delete_where` does its own predicate pass first — no
mutation happens until every id has been decided — then applies the deletions
in a single pass afterward, once no live handle from the original iterator is
still in use. `save.clear_slots_where` does the same for item slots.

Restarting the walk after each single write is the shape to avoid, even though
it is the only one the per-handle methods alone permit. It is quadratic in
matches, and for slots it is worse than that: every `slot.clear()` also drops
the world's container index, so each restart pays to rebuild that index across
the whole save. On a large world it does not finish. If you find yourself
writing one, the bulk form you need is missing — say so rather than working
around it.

**`raw` writes trip this too, and across scope boundaries.** `raw.delete`
bumps the same `mutation_epoch` on a real (non-dry) run, but only when it
actually removes something; a removing `raw.visit` bumps it the same way,
once, after the walk finishes if it removed at least one node. `raw.set` does
not bump it — overwriting a scalar in place moves nothing, so handles and
iterators stay valid. The epoch itself is a single counter for the whole run,
not scoped to whichever `target` the raw call touched, so a `raw.delete`
against `player:<uid>` invalidates every live handle and iterator for the rest
of that run — including a `save.players()` iterator still walking the level
file, even though the two touch entirely different files. Concretely, this
shape does not work:

```lua
for player in save.players() do
  raw.delete("player:" .. player.uid, "SaveData.SomeArray")
end
```

It raises on the second iteration: the first `raw.delete` removes something
and bumps the epoch, and the next call into the `save.players()` iterator sees
its captured epoch no longer matches. The fix is the same two-pass shape
`delete_where` uses internally — finish reading everything first, then issue
the raw writes from a plain table with no live handle held across them:

```lua
local uids = {}
for player in save.players() do uids[#uids + 1] = player.uid end
for _, uid in ipairs(uids) do
  raw.delete("player:" .. uid, "SaveData.SomeArray")
end
```

`fix_missions` in the bundled `pst.reset` plugin uses exactly this shape —
see below.

## Sandbox limits and terminating statuses

| Limit | Default | |
|---|---|---|
| Wall clock | 120,000 ms | Checked via a Lua instruction-count hook (every 10,000 VM instructions) against a wall-clock deadline sampled at the start of the run. Sized for the largest real worlds (a public server with thousands of players holds enough containers that a whole-world command legitimately runs for minutes), not for typical ones. |
| Memory ceiling | 256 MiB | Enforced by the sandbox's own allocator; an allocation that would exceed it fails Lua's allocation instead of succeeding. |

A run ends with one of these statuses:

| Status | Wire value | Meaning |
|---|---|---|
| Ok | `"ok"` | The command function returned (or fell off the end without returning). |
| Timeout | `"timeout"` | The wall-clock limit was reached. |
| Cancelled | `"cancelled"` | The run was cancelled. **Not reachable in v1** — the `Cancel` registry and `cancel_plugin_run` message exist and are tested, but no client ever holds a run's id while it is still in flight (it first appears in the terminal result frame), so nothing can ever actually name a run to cancel. The panel shows the control disabled with an explanation instead of pretending it works. |
| MemoryExceeded | `"memory_exceeded"` | The memory ceiling was hit. |
| Error | `"error"` | A script error (a Lua error, a manifest/argument problem, or a host function refusing). The wire frame's `message` field carries the text. |

**Every limit above is enforced for ordinary Lua execution. Pattern-matching
backtracking is a documented exception.** `string.match`, `string.gmatch`,
`string.gsub`, and `string.find` with a backtracking pattern run entirely in C:
no Lua VM instruction retires and no Lua call is made while backtracking runs,
so the wall-clock hook cannot interrupt it. A pathological pattern can
therefore consume unbounded CPU with no way for the host to stop it. This is
accepted for v1 (every plugin today is first-party or one the user
deliberately chose to install) and is a **hard gate before any plugin
repository ships** — see spec §20.3.

The sandbox opens exactly six Lua libraries: the base library, `coroutine`,
`math`, `string`, `table`, and `utf8`. `io`, `os`, `package`, and `debug` are
never opened. `load`, `loadfile`, `dofile`, and `xpcall` are removed even
though the base library installs them (`xpcall`'s message handler runs with
interrupt hooks disabled, so neither the deadline nor cancellation could ever
reach a handler that hangs). `pcall` stays and is safe to use. Precompiled Lua
bytecode is refused at load — only text source is accepted. A script's
`setmetatable` calls are routed through a wrapper that strips any `__gc` key
before applying the metatable, since Lua's own garbage collector disables
hooks around finalisers and a `__gc` handler is therefore also outside the
timeout's reach.

A command's return value becomes the run's result: returning a plain string
sets the run's `summary` with no structured `result`; returning a table
converts the whole table to JSON as `result`, and additionally lifts a
top-level string `summary` field and a top-level object `counts` field (each
value coerced to an integer) up onto the outcome directly, alongside whatever
else the table contained. Returning nothing is a successful run with neither.
A table deeper than 32 levels or wider than 100,000 total nodes fails the
conversion — the run still succeeds, but with no `result`.

Under a dry run specifically, every mutating host function (`raw.set`,
`pal.delete`, `players():delete_where()`, ...) also bumps its own count key
(for example `"guilds.delete_where"`) the moment it runs, independently of
whatever the script itself returns. The outcome's final `counts` map is the
union of those host-bumped keys and the script's own returned `counts` table
— so a dry-run caller can see keys the script never set itself (see the
worked example below, where a live run showed both `guilds` and
`guilds.delete_where` in the same `counts` map). A real (non-dry) run never
gets these host-bumped keys — mutating functions report what they did through
their own return values instead, so a real run's `counts` reflects only what
the script itself put there.

## Multi-file plugins and `require`

A plugin's sources are not limited to the single file named by `entry`. Any
other file in `sources` is pulled in only when something calls `require` for
it — nothing scans the source map and runs every file it finds.

`require` is implemented by the host itself, not by Lua's own `package`
library — that library is one of the ones never opened (see the sandbox
limits below), so there is no `loadlib`, no C searcher, and no filesystem
`package.path`/`package.cpath` for a script to point anywhere. `load`,
`loadfile`, and `dofile` stay `nil` as well. `require` is the only way to pull
in another file, and it resolves exclusively against the plugin's own
`sources` — there is no way to reach another plugin's files, the filesystem,
or a Lua library installed on the machine running the host.

A module name maps dots to path segments and always resolves to a `.lua` key:
`require('lib.util')` looks up the source stored at `lib/util.lua`;
`require('util')` looks up `util.lua`. The name does not need to be a valid
Lua identifier, but the key it maps to has to exist, case-sensitively, in the
plugin's `sources`, or the call errors.

A module runs at most once per command run: the first `require` for a name
loads and executes that file's top level and caches whatever it returns.
Every later `require` for the same name in the same run returns the cached
value without executing the file again. A module whose top level returns
nothing caches as `true` rather than `nil`, so a later `require` for it does
not look uncached. A module that ends up requiring itself, directly or
through a chain of other requires, while its own top level is still running
is refused with a "circular require" error rather than recursing forever.

## Worked example: `delete_empty_guilds`

From the bundled `pst.cleanup` plugin (`psp-app/src/bundled/pst.cleanup/main.lua`):

```lua
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
```

- `save.guilds()` returns a fresh iterator; calling `:delete_where(...)` on it
  (rather than iterating it directly) invokes the bulk form. This requires
  both `save.read` and `save.write` in the manifest.
- The predicate reads one field (`guild.player_count`) off each guild handle
  and returns a plain boolean — no mutation happens inside it, satisfying the
  mutation-during-iteration rule; `delete_where` performs the actual deletes
  itself, afterward, in its own pass.
- `removed` is how many guilds were actually deleted; `unresolved` is
  `delete_where`'s own second return — guilds the predicate selected that the
  host could not resolve at apply time (a dangling reference), kept separate
  from anything the predicate itself chose to spare so a summary can report
  "the script chose to leave this alone" and "this could not be resolved at
  all" as two different things.
- The returned table's `summary` and `counts` are both lifted onto the run
  outcome directly (and the whole table is also available as `result`), so a
  UI panel can render either the summary text or the structured counts
  without parsing the other.

## The `pst.reset` plugin

Bundled alongside `pst.cleanup` (`psp-app/src/bundled/pst.reset/`), `pst.reset`
("World Reset") ports PalworldSaveTools' Reset family: seven commands that
each clear save state the game regenerates on next load. Every command but
one is a single `raw.delete` against one key under the level's
`worldSaveData`; the manifest declares `save.read`, `save.raw`, and
`players` — `players` because `fix_missions` addresses `player:<uid>` raw
targets. `raw.delete` is gated on `save.raw` alone, so the family needs no
`save.write` (which would additionally install `delete_where`/
`clear_slots_where`, a write surface this plugin has no use for) and no `log`
(no command calls `log.*`).

| id | title | removes |
|---|---|---|
| `reset_supply_drops` | Reset Supply Drops | `level`: `worldSaveData.SupplySaveData` |
| `reset_anti_air_turrets` | Reset Anti-Air Turrets | `level`: `worldSaveData.FixedWeaponDestroySaveData` |
| `reset_oilrig` | Reset Oil Rig | `level`: `worldSaveData.OilrigSaveData` |
| `reset_invader` | Reset Invaders | `level`: `worldSaveData.InvaderSaveData` |
| `reset_dungeons` | Reset Dungeons | `level`: `worldSaveData.DungeonPointMarkerSaveData` **and** `worldSaveData.DungeonSaveData` |
| `reset_lock_gimmick` | Reset Lock Gimmicks | `level`: `worldSaveData.LockGimmickSaveData` |
| `fix_missions` | Reset Missions | every player's `SaveData.CompletedQuestArray_FullRelease` |

- **`reset_dungeons` removes two keys under one command**, because they are
  meaningless apart: the point-marker key describes dungeons that the other
  key holds the actual state for, so removing only one would leave the world
  describing dungeons it has no state for. Its `counts.dungeon_save_data` is
  how many of the two keys were actually present and removed (0, 1, or 2) —
  not how many deletes were attempted.
- **`fix_missions` is filed under Reset despite its name.** It clears every
  player's completed-quest list so the game re-offers those missions, which is
  a reset of quest-completion state, not a repair of a broken value — hence
  its home in this family rather than Fix. The command id is `fix_missions`;
  the title shown in the UI is "Reset Missions".
- **`fix_missions` reports a count rather than writing a log file.** This port
  has no file system access from inside the sandbox, so it returns a count of
  affected players in the result instead (`counts.players`).
- Unlike the other six commands, `fix_missions` touches per-player files
  rather than the level, and does it with the two-pass collect-then-delete
  shape described in the mutation-during-iteration rule above — a `raw.delete`
  against one player's file bumps the same run-wide mutation epoch a
  `save.players()` iterator over the level relies on, so every uid has to be
  collected before any delete runs.

Bundled command ids are not guaranteed stable across releases — that is only
acceptable because no plugin repository exists yet and there are no
third-party plugins depending on where a bundled command lives; it would not
be acceptable once either exists.

## The `pst.tools` plugin

Bundled at `psp-app/src/bundled/pst.tools/`, `pst.tools` ("Save Tools") ports
PalworldSaveTools' Misc family of world-clock, diagnostic, and
inventory-resize functions. The manifest declares `save.read`, `save.write`,
`save.raw`, and `players`: `save.write` for `container.set_slot_count()` and
`save.unlock_private_chests()`, `save.raw` for `edit_game_days`'s direct write
to the world clock, and `players` because `modify_all_player_slots` addresses
a `player:<uid>` raw target to find each player's inventory container id.

| id | title | does |
|---|---|---|
| `edit_game_days` | Set Game Day | Sets the world clock to the given in-game day (`0..=100000`). |
| `paldefender_commands` | PalDefender Commands | Generates a `/killnearestbase x y z` console command for every base camp. |
| `modify_one_container_slots` | Resize One Container | Sets one container's slot count by id (`1..=1000`). Refuses rather than dropping slots that still hold items. |
| `modify_all_player_slots` | Resize All Player Inventories | Sets every player's main inventory to the given slot count (`42..=999`). Refuses per player rather than dropping slots that still hold items. |
| `modify_all_guild_chest_slots` | Resize All Guild Chests | Sets every guild's chest container to the given slot count (`1..=1000`, default `50`). Refuses per container rather than dropping slots that still hold items. |
| `unlock_all_private_chests` | Unlock All Private Chests | Clears the ownership lock on every private chest and item booth in the world. No params. |

### Behavioural notes

- **`paldefender_commands` returns its lines in the result rather than writing
  a log file.** A plugin has no filesystem access, so this returns the
  generated lines in the result's `lines` field instead, for the UI to offer
  as copyable text. A base whose location cannot be read is skipped and
  counted under its own `unresolved` key rather than folded into `bases` or
  dropped silently.
- **Every resize command refuses a shrink that would drop occupied slots.**
  `container.set_slot_count` compares the requested capacity against each
  slot actually holding an item, so it refuses precisely the shrinks that
  would destroy something and no others.
- **`unlock_all_private_chests` covers item booths, but leaves the
  `is_private_lock` flag untouched.** It clears `private_lock_player_uid` on
  every lockable model, item booths included, but does not zero each booth's
  own `is_private_lock` flag.

### Data unreachable through `raw`

Some save data lives on a **typed Rust struct** inside the save rather than in
the generic property bag `raw.*` walks. `raw.kind` reports a node backed by
such a struct as `"opaque"`, and an opaque node has no children — no `raw`
path expression can reach inside one, no matter how it is written. That data
is not undecoded, though — only unreachable through `raw`; a command can still
read or write it through a host method bound onto a typed field or handle, the
same way `save.unlock_private_chests()` and `guild.chest_container_id` do for
the map-object lock and the guild chest id.

This is not unique to those two — the guild-tail data behind
`GroupSaveDataMap` (member online timestamps, base ownership, and more) is
opaque to `raw` for the identical reason, and any future command reaching for
it through `raw` will hit the same wall.

## The plugin editor

A per-plugin panel entry (`onEdit` on each `PluginCard`) opens `/plugins/editor?id=<id>`.
"New plugin" prompts for a name, derives an id from it (lowercased, runs of
non-alphanumeric characters collapsed to a single `-`, leading/trailing `-`
trimmed, and cut to the 64 characters a manifest id may hold, trimming a `-`
the cut lands on), creates the plugin with a one-command Lua scaffold and `log` as its
only capability, and opens the editor on it directly.

### Files, the manifest tab, and canonical storage

The editor shows one tab per file: `manifest.json` first, then every entry in
`sources`, alphabetically. The `manifest.json` tab is not the row's raw
stored text — it is the parsed manifest re-serialised as indented JSON for
editing, and on save it is parsed again and stored back through Rust's own
`serde_json` serialisation (compact, key order fixed by the struct
definition), not as whatever text was typed. Saving a `.lua` tab writes that
file's text into the plugin's `sources` map unchanged.

A manifest whose `id` does not match the plugin it is being saved into is
refused. The id keys the row, its storage, and its capability grant, so a
manifest declaring a different one would leave a running command's
`ctx.plugin_id` disagreeing with everything the host looks up by it.

A refused save — a manifest that will not parse, one that asks for a
capability its origin may not have, a mismatched id, or a bundled plugin —
answers under `save_plugin_source` with an `error` string rather than as a
transport-level error, and creating a plugin answers under `create_plugin`
the same way. The editor keeps the unsaved buffer and shows the message as a
toast; nothing is lost and no other tab is disturbed.

### Adding and deleting source files

The editor is not limited to editing files a plugin already has — it can add
a new `.lua` source and delete an existing one, which is how a multi-file
plugin using `require` grows past its `entry` file. A new file's path must be
relative, use forward slashes only, and end in `.lua`; the editor checks this
as the name is typed, but that check is convenience, not the boundary — the
same validation runs again on the server for every add, regardless of what
the client sent, and rejects an absolute path, a `..` or `.` segment, a
backslash, a drive-letter prefix, and similar attempts to write outside the
plugin's own source map. `manifest.json` cannot be deleted, and neither can
whichever file the manifest currently names as `entry`.

### Syntax and manifest checks

Every edit (debounced) is checked: a `.lua` tab is parsed — not run — with
`luaL_loadbufferx`, the exact same call the runtime uses to load a script
before invoking a command, so a syntax error the editor flags is a syntax
error a real run would also hit. This holds on every deployment, because
desktop, the standalone server, and the web build all dispatch
`CheckPluginSyntax` (and every other plugin message) through the same
`psp-app` handler table — desktop embeds `psp-server` directly, and `psp-web`
imports the identical `dispatch` function `psp-server` uses. The
`manifest.json` tab is checked by parsing it as a manifest under the plugin's
real origin (bundled manifests are judged more permissively than user ones),
using the same `Manifest::parse` the install and save paths use.

### Two tiers: baseline and full

The editor's checks and assists come in two tiers.

The **baseline** tier works on every deployment the editor runs on — desktop,
a self-hosted Docker deployment, and the web build alike. It is the syntax
check described above, plus the completion, hover, and signature help
described next, all generated from the host API definition and filtered by
the plugin's granted capabilities.

The **full** tier adds `lua-language-server` on top of the baseline, and only
where that binary can run: desktop and a self-hosted Docker deployment. It
adds type inference, go-to-definition, find-references, rename, and
diagnostics beyond what a syntax-only parse can report. It is not available
in the web build — a language server is a native process a browser has
nowhere to run — so the web editor never attempts it and shows a visible,
non-blocking notice explaining why, while continuing to work on the baseline
tier. See below for how the full tier is acquired and what happens when it
is not available.

### Completions, hover and signature help

Typing in a `.lua` tab offers completion, hover, and signature help for the
host API's **global tables and their members** — the globals themselves, and
what follows the dot on `save.`, `log.`, `ui.`, and the rest — generated from
the same `ApiDefinition` the Rust host builds from its own function
registrations, not maintained separately. Each entry is filtered against the
plugin's granted capabilities: a global or function gated on a capability the
plugin was not granted is left out of every list, exactly as if it did not
exist. This grant is the plugin's **stored** `granted_capabilities`, fetched
when the editor opens — editing the manifest's `capabilities` array does not
change what the editor offers until the manifest is saved and the plugin is
reopened.

Completions stop at the global tables. A value held in a variable gets none:
inside `for p in save.players() do`, typing `p.` offers nothing, because
knowing that `p` holds a player handle means inferring the type of an
expression, and the editor does no type inference. Handle types — player,
pal, guild, container — are exactly where that shows: they are fully
described in the `ApiDefinition` and rendered in the `---@meta` output, so
`lua-language-server` reading that file does offer their fields, but the
editor's own completion lists only ever start from a global name.

**`delete_where` is not offered as a completion, on any plugin, regardless of
capability.** It is not a gap in the capability filter — it is a gap in what
the API definition describes at all. `save.players()`, `save.pals()`, and
`save.guilds()` each return a plain Lua function value, and `delete_where` is
installed on that function's own `__index` metatable, not on any global table
or handle the generator walks. Nothing built from `ApiDefinition` — the
editor's completions included — knows it exists. This is a known gap, not a
broken install: code that calls `save.players():delete_where(...)` is correct
Lua and runs exactly as documented above; the editor simply has nothing to
suggest for it.

### The command/function agreement warning

The editor cross-checks the manifest's `commands` array against the entry
file's top-level function definitions and warns both ways: a command with no
matching function ("will fail"), and a function with no matching command
("nothing can run it"). This mirrors what the runtime actually does when a
command runs — it looks the command id up as a Lua *global* with
`lua_getglobal` — so the check only recognises a definition that lands in a
global of that exact name: `function name() ... end` at the start of a line,
or `name = function() ... end`, both counted even inside a long-bracket
string (the check is a line scan, not a parse, so it errs toward warning
rather than missing something). A `local function run()`, `function
M.run()`, or `function M:run()` binds no global the runtime's
`lua_getglobal` lookup — or this check — can see, so none of those satisfy a
command of the same name; the editor will warn "no global function of that
name" even though the code parses and the name reads correctly to a person.

### The language server: download on first use, and graceful degradation

On desktop and Docker, `lua-language-server` is downloaded on first use
rather than shipped inside the application — the editor opens on the
baseline tier immediately and upgrades to the full tier once the binary is
in place. The download is a pinned release (about 4.5 MB, platform-specific)
verified against a pinned SHA-256 digest before anything from it is
installed; a checksum that does not match leaves nothing on disk, rather
than installing an unverified binary. Once installed for a given version, it
is reused on every later launch — nothing is re-downloaded unless the pinned
version changes.

Anything that keeps the full tier from being available or running —
the platform has no pinned release, the download or verification failed, the
server has not finished starting yet, or a running server dies mid-session —
is a **degradation**, not a failure the editor surfaces as broken: the editor
falls back to the baseline tier automatically, a non-blocking notice explains
why, and typing, saving, and running a plugin all keep working. The editor
also keeps checking in the background and upgrades itself back to the full
tier on its own if the language server later becomes available, with no
action needed from the plugin author.

### Running a draft

The run panel below the editor runs the command as currently written,
including unsaved changes, without saving anything: the draft's sources and
manifest text go straight into the run request and are never written to the
plugin's row. Its capability grant, though, is **not** whatever the edited
manifest's `capabilities` array asks for — it is always the plugin's stored
`granted_capabilities`, intersected against whichever manifest capability
list actually governs the run (the draft's if one is supplied). Both the
panel's normal run and the editor's draft run pass through one function that
computes the grant this same way, so a draft cannot reach a capability the
plugin was never granted just by asking for it in the edited manifest.
Draft runs also skip the "plugin must be enabled" check the panel's normal
run enforces, so a disabled plugin's draft still runs from the editor.

A command the manifest marks `destructive` runs the same two-step way it does
in the plugin panel, and the editor's Dry run checkbox cannot turn that off:
the first press always runs a dry run, and only a second, explicit Apply
against the projected counts writes anything to the loaded save. Apply
re-runs the exact draft the preview ran, not whatever the buffers hold by
then, so editing the code after a preview and pressing Apply cannot smuggle
different code past the preview — it applies what you saw, and a fresh
preview is needed for the new text.

### Bundled plugins are read-only

The editor's Save button is disabled whenever the opened plugin is bundled,
and the save handler refuses the write server-side too, for the same reason:
every bundled plugin's manifest and sources are overwritten from the
compiled-in copy on every application startup, so an edit made through the
editor would simply vanish the next time the app launched. Only a plugin's
`enabled` flag and its granted capabilities survive that startup reseed.

The draft runner is disabled for a bundled plugin as well, and the draft
handler refuses it server-side too — this one is a privilege boundary rather
than a durability one. A draft run executes sources supplied by the caller
under the row's stored grant, and a bundled row's grant may include
`save.raw`, which exists only so that code compiled into the application can
use it. Running an edited script against that grant would hand raw save
access to code that was never shipped in the binary, so a bundled row's
draft is refused outright rather than run with a reduced grant. A bundled
plugin still opens in the editor and can be read in full; it just cannot be
saved or draft-run. Its commands run normally from the plugin panel, from
the sources the application shipped.

### Bundle size

Measured from `npm run build` in `ui/` (SvelteKit's static adapter, hashes
will differ on any other build):

- Total built output (`ui_build/`): 354,892,569 bytes (~338.5 MiB), of which
  the JS/CSS asset tree (`ui_build/_app/`) is 38,999,261 bytes (~37.2 MiB) —
  the rest is game-data, wiki content, and other static assets unrelated to
  this feature.
- The `/plugins/editor` route is its own lazy-loaded chunk: the root entry
  modules reference it only inside a dynamic `import()` behind the router's
  path table, never as a direct import, so it is not fetched until that route
  is visited. Its own node chunk is 19,991 bytes; a second chunk
  (8,318 bytes) is shared only between it and the `/plugins` panel (the
  editor store, the agreement-warning check, and the shared run-result view).
  Together, the editor-only code no other route pays for is about 27.6 KB.
  That is up from about 16 KB (9,429 + 6,745 bytes) before multi-file
  plugins, the tier probe, and the language-server client shipped: the LSP
  client, the add/delete-file UI, and the baseline/full tier switch together
  add roughly 12 KB, split as +10,562 bytes on the route's own chunk and
  +1,573 bytes on the chunk it shares with the `/plugins` panel. Both editor
  chunks remain a fraction of the ~37.2 MiB JS/CSS asset tree.
- **`monaco-editor` is not a cost the plugin editor introduces** —
  `ui/src/routes/editor/+page.svelte` uses it through the same shared
  `Monaco` component, and every reference to the `monaco-editor` package
  anywhere in the codebase, including the plugin editor's, is a type-only
  import that produces no runtime code. The actual editor engine is never
  part of the Vite bundle at all: `Monaco.svelte` loads it at runtime via
  `@monaco-editor/loader`, which injects a `<script>` tag pointing at
  `https://cdn.jsdelivr.net/npm/monaco-editor@0.55.1/min/vs` — the only trace
  of "monaco-editor" anywhere in the built output is that URL string, sitting
  in the shared vendor chunk alongside unrelated code used by 35 other
  routes, not in either chunk the plugin editor owns. So there is no
  "Monaco chunk" to measure, and the plugin editor route adds no new heavy
  dependency to the web bundle.

That CDN fetch is a runtime dependency, not a build-time one: the editor's
*chrome* — Monaco itself, on either `/editor` or `/plugins/editor` — will not
open without network access to `cdn.jsdelivr.net`, on every deployment,
including a fully offline desktop install; nothing in this codebase serves
`monaco-editor` locally as a fallback. This is a property of
`@monaco-editor/loader`'s default configuration
(`ui/node_modules/@monaco-editor/loader/lib/es/config/index.js:1-5`), and it
is not specific to this route — the save editor at
`ui/src/routes/editor/+page.svelte` has the same requirement through the
same `Monaco.svelte` component. It is unrelated to the syntax
check described above: that check runs Lua's own parser locally inside the
Rust host and needs no network at all, so a plugin's syntax and manifest are
still validated offline — it is specifically the editor's on-screen chrome
that needs the network, not the checks it runs against your code.
