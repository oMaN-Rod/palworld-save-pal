# Plugin API reference

This is the plugin reference for Palworld Save Pal. It covers the manifest,
command model, UI contract, capability model, host API, and sandbox rules.
Use it when you need the exact behavior of the runtime.

## Table of contents

- [Quick start](#quick-start)
- [Manifest schema](#manifest-schema)
- [Plugin-defined interfaces](#plugin-defined-interfaces)
- [Capabilities](#capabilities)
- [The host API](#the-host-api)
- [The API definition](#the-api-definition)
- [The mutation-during-iteration rule](#the-mutation-during-iteration-rule)
- [Sandbox limits and terminating statuses](#sandbox-limits-and-terminating-statuses)
- [Multi-file plugins and `require`](#multi-file-plugins-and-require)
- [Worked example: `delete_empty_guilds`](#worked-example-delete_empty_guilds)
- [The plugin editor](#the-plugin-editor)

## Quick start

A plugin is a `manifest.json` plus one or more `.lua` source files. It can be
installed from the UI either as a single bare `.lua` file or as a `.zip`
containing `manifest.json` and its sources.

### Minimal plugin

```json
{
  "id": "example.cleanup",
  "api_version": 1,
  "name": "Example Cleanup",
  "version": "1.0.0",
  "entry": "main.lua",
  "capabilities": ["log"],
  "commands": [
    {
      "id": "hello",
      "title": "Hello",
      "params": []
    }
  ]
}
```

```lua
function hello()
  log.info("Hello from the plugin")
end
```

### Core rules

- The manifest is the plugin contract.
- Every command must have a matching top-level Lua function.
- Capabilities are explicit and enforced before the script runs.
- `ui` is data only. It arranges commands, it does not execute code.
- The sandbox is strict. Read the limits and the mutation rules carefully.

## At a glance

| Topic | What matters most |
|---|---|
| Manifest | Defines the plugin id, entry file, capabilities, and commands. |
| Commands | Each command must exist as a top-level Lua function with the same name. |
| Params | Values are validated and defaulted before a command runs. |
| Capabilities | These are the permission gates for save access, raw access, storage, UI, logs, and game data. |
| UI | JSON only. It builds layout, not logic. |
| Sandbox | Scripts run in a restricted Lua environment with strict limits. |
| Mutations | Writes and structural changes follow explicit validation and dry-run rules. |

### Common patterns

```lua
function list_players()
  for player in save.players() do
    log.info(player.name .. " (uid: " .. tostring(player.uid) .. ")")
  end
end
```

```lua
function hello_with_args()
  local count = ctx.args.count or 1
  log.info("Processing " .. tostring(count) .. " items")
end
```

```lua
function save_last_run()
  storage.set("last_run", ctx.now)
  log.info("Saved timestamp: " .. tostring(storage.get("last_run")))
end
```

Use the typed save API whenever possible. Use `raw` only when you truly need direct access to data the typed model cannot reach.

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
| `id` | string | yes | 1-64 characters, lowercase letters, digits, and single `.` `_` `-` separators. Cannot start or end with a separator, and separators cannot repeat (`a..b` is invalid). This is also the plugin's row key - an install with an id that already names a bundled plugin is refused. |
| `api_version` | integer | yes | Must equal the runtime's supported version (`1` today). A manifest declaring any other value is refused before anything is parsed further. |
| `name` | string | yes | Non-empty after trimming. |
| `version` | string | yes | Non-empty after trimming. Free-form (not semver-checked). |
| `author` | string | no | Free-form. |
| `license` | string | no | Free-form. |
| `entry` | string | yes | A plain `.lua` filename - no `/`, `\`, or `:`, not `.` or `..`, must end in `.lua` and be longer than just `.lua`. Must name a key present in the plugin's `sources`. |
| `capabilities` | array of strings | no (defaults to none) | Each must be one of the eight names in the capability table below. No duplicates. `save.raw` is refused unless the plugin's origin is bundled (see below). `save.write` is refused unless `save.read` is also declared. |
| `commands` | array of command objects | no (defaults to none) | See below. |

### Command objects

| Field | Type | Required | Rule |
|---|---|---|---|
| `id` | string | yes | Must be usable as a Lua global identifier (starts with a letter or `_`, then letters/digits/`_`) and must not be a Lua reserved word. The script must define a top-level `function <id>() ... end` with this exact name - the runtime looks it up as a global by that name when the command runs. No two commands on one manifest may share an id. |
| `title` | string | yes | Shown in the UI. |
| `description` | string | no | Shown in the UI. Should describe only what the command does - see the worked example below. |
| `destructive` | bool | no (default `false`) | Purely descriptive; the runtime does not gate anything on it. The UI uses it to decide whether to offer a dry run. |
| `params` | array of param objects | no (defaults to none) | See below. |

### Param objects

| Field | Type | Required | Rule |
|---|---|---|---|
| `id` | string | yes | A valid Lua identifier, not a reserved word, unique within the command. This becomes the key under `ctx.args`. |
| `type` | string | yes | One of `int`, `float`, `string`, `bool`, `enum`, `entity`, `multiselect`. |
| `label` | string | yes | Shown in the UI. |
| `description` | string | no | Shown in the UI. |
| `default` | JSON value | no | Used when the caller omits (or sends JSON `null` for) this argument. Its JSON type must match `type` (an integer for `int` - a float with a fractional part or out of `i64` range is rejected; a string that is one of `options` for `enum`; a string for `entity`; an array of strings for `multiselect`; etc). If there is no default and the caller supplies nothing, the run is refused before the script starts. |
| `min` / `max` | number | no | Inclusive bounds, checked for `int` and `float` only. `min` must not exceed `max`. |
| `options` | array of strings | only for `enum`; optional for `multiselect` | Must be non-empty for an `enum` param. For `multiselect`, declaring `options` constrains which strings are accepted; leaving it empty is the common case - see below. The supplied (or default) value must be one of these, compared as an exact string match. |
| `entity` | string | only for `entity` | One of `pal`, `player`, `guild`, `base` - which kind of entity this parameter's value identifies. Required and validated at install time for an `entity` param; ignored for every other type. |

Two of these types are not scalars in the way `int` or `string` are:

- **`entity`** takes an entity's id as a string, and its `entity` field says which
  kind of entity that id names. An empty string is the conventional "any" -
  `pst.repair`'s `owner` parameter defaults to `""` and its script reads that as
  "every player" rather than one in particular.
- **`multiselect`** takes an array of strings. Most of the time it declares no
  `options`, because its whole purpose is to receive a selection made over a
  result no manifest could enumerate in advance - the ids a plugin's own
  `table` widget just showed the user, for instance. Its `default` must be an
  array (typically `[]`), never a bare string.

Every SP1 param declaration - `int`, `float`, `string`, `bool`, `enum` - is
unchanged by either addition.

Argument coercion (`run_plugin_command`'s `args`) happens once, before the
script runs: every declared param is resolved from the supplied JSON object,
falls back to its default, is type- and range-checked, and is written into
`ctx.args`. An argument key the command does not declare is refused outright -
there is no silent pass-through of extra fields.

### Bare `.lua` install

Installing a single `.lua` file (rather than a `.zip`) synthesises a manifest
for it:

- The script must define a top-level `function main()`. A plain source-text
  scan looks for this - not a full Lua parse - so it can be fooled by a
  `function main` inside a comment or string, but not by common formatting.
- `id` is the filename's stem, slugified (lowercased, runs of non-alphanumeric
  characters collapsed to a single `-`, trimmed, capped at 64 characters).
- `capabilities` is always empty. A bare-`.lua` install has no way to request
  any capability - if the script needs `log`, `gamedata`, or save access, ship
  it as a `.zip` with an explicit manifest instead.
- `commands` is exactly one entry: `{ "id": "main", "title": "Main" }`.

## Plugin-defined interfaces

Every plugin already gets a form generated from its `commands` and their
`params` - one field per param, one button per command. A `ui` section lets a
plugin arrange those same commands into something more purposeful: a scan
that feeds a table, a table the user picks rows from, a button that acts on
the pick. It does this by describing widgets, not by writing any.

**A view is data, never code.** A plugin's `ui` is JSON - sections and
widgets and the field names below, nothing else. There is no HTML, no CSS,
no JavaScript, and no expression language, and no string a plugin supplies
is ever rendered as markup or evaluated as a program. If the vocabulary
below can't build the widget you want, that's a host limitation to raise,
not a gap to route a string through - an escape hatch here would mean a
sandboxed script drawing arbitrary markup into the host's own UI, which is
exactly the hole the sandbox exists to close everywhere else.

**`ui` is optional, and does not change how commands run.** A plugin with no
`ui` renders and runs exactly as it did before this feature existed: the
generated command form, one field per param. Deleting a `ui` block from a
plugin that has one is equally harmless - its commands still exist, still
validate arguments the same way, and still run the same way when invoked
from the generated form. `ui` only changes how a plugin's commands are
*arranged*; it has no say in how they are *run*.

### The shape

`ui` is an array of sections. A section has an optional `title`, a `columns`
of `1`, `2` or `3` (default `1`), and an array of `widgets`. That's the
entire grammar - sections hold widgets, and a widget holds nothing further.
Any widget may set `"span": "full"` to take its section's full width
regardless of `columns`, which is how the worked example's "Scan" button
sits under three columns of inputs instead of trying to share a column with
one of them.

### The ten widget types

Six are inputs, three are outputs, and one is an action. Every field below
is a JSON field of the widget object; a field a given type doesn't use is
simply absent.

| Type | Kind | Fields |
|---|---|---|
| `entity_select` | input | `id`, `label`, `entity` (one of `pal`, `player`, `guild`, `base`) |
| `text_input` | input | `id`, `label` |
| `number_input` | input | `id`, `label` |
| `toggle` | input | `id`, `label` |
| `select` | input | `id`, `label` |
| `multiselect` | input | `id`, `label` |
| `table` | output | `from`, `path`, `columns`, `selectable`, `id` (required if `selectable`) |
| `list` | output | `from`, `path`, `label` |
| `text` | output | `from` and `path`, or a literal `text` |
| `button` | action | `label`, `command`, `args` |

An input widget's `label` is optional - omit it and the widget falls back to
the label the parameter itself declares. The bounds on a `number_input`, the
options a `select` or `multiselect` offers, and whether a value is required
all come from the command's own `param` declaration too; the widget names
which param it is, nothing more. An `entity_select` reads the loaded save to
populate itself, so a manifest that uses one must declare `save.read`.

### How inputs reach a command

An input widget's `id` names the parameter it feeds - on **every** command
that declares a parameter by that id, not just the one nearest it in the
layout. That's deliberate: in the worked example below, the `max_level` and
`max_rank` number inputs sit in the "Scan" section, but `fix_illegal_pals`
also declares params called `max_level` and `max_rank`, so the same two
inputs drive the fix as well as the scan, with nothing in the manifest
saying so twice.

### How results reach outputs

A `table`, `list`, or `text` widget can read `from` a command and a `path`
into what that command returned. The whole table a command's Lua function
returns is available this way - `summary` and `counts` are pulled out of it
separately for the host's own display, but they never leave the table the
script built, so `"path": "summary"` reads the same field the host's own
summary line does. `path` is a dotted walk (`"counts.illegal"` reaches a
nested field); a path that finds nothing renders an empty widget rather than
an error, so a command that hasn't run yet or returned a differently-shaped
result never breaks the page around it.

### A table's rows

A selectable table's rows need an identity that survives a re-render, and
nothing in a command's returned row forces one to exist - this is the one
convention in the format that isn't spelled out anywhere in the JSON. A
row's id is the first of `id`, `instance_id` or `uid` it carries as a
non-empty string, checked in that order; a row with none of those falls back
to its own index in the returned array. `pst.repair`'s scan rows below carry
`instance_id`, which is why that's what ends up in a selection.

### Chaining widgets together with `args`

A `button`'s `args` pulls a value out of another widget when it runs its
command: `"args": { "ids": "rows.selection" }` sends the table widget `rows`'
current selection as the `ids` argument. The only two reference forms that
parse are `<widget>.selection` (a table's checked row ids) and
`<widget>.value` (an input widget's current value); nothing else does -
there's no arithmetic, no string building, no reaching into a result
directly. If a plugin needs to hand a command something more elaborate than
one other widget's value or selection, that value belongs in the command's
own default or in `ctx.args`, not in `args`.

### What the host owns

A `ui` section never bypasses the destructive-command rule that governs
every other way of running a command: a button that runs a `destructive`
command always previews first (see `ctx.dry_run` below), and the host draws
its own Apply / Cancel bar over the prediction - the same bar and the same
preview a command run from the generated form gets. A plugin cannot declare
a widget that skips this; there is no field for it.

### Limits

A `table` or `list` widget renders at most 500 rows and says how many rows
actually exist - "Showing 500 of 3,214" - rather than silently truncating.
An `entity_select` is capped the same way: it offers at most 500 entities
and reports the true total alongside them. Both are render caps, chosen so
a wide save doesn't hang a browser tab; they are unrelated to the 150,000
JSON node cap documented under `gamedata` below, which limits what a single
`gamedata.get` call may build, not how many rows a widget draws.

### What is refused at install, and what degrades at render

Two different pieces of code police a view, at two different times. The
manifest parser (`validate_view`) runs once, at install, and refuses the
*whole manifest* if a view is malformed - nothing installs. The browser's
own normalizer runs every time a valid view is opened, and instead of
failing, drops just the one widget or field that doesn't make sense and
carries on - partly so a plugin written against a newer host's widget
vocabulary still opens, with the unfamiliar parts simply missing, rather
than refusing outright.

| Refused at install (the manifest never installs) | Degrades at render (the view opens; the offending piece is dropped) |
|---|---|
| a section's `columns` is not `1`, `2` or `3` | a section that isn't a JSON object is skipped |
| a widget `id` isn't a valid Lua identifier, or is reused elsewhere in the view | a widget that isn't a JSON object is skipped |
| a `span` is present and isn't `"full"` | an unrecognized widget `type` is skipped |
| an input widget has no `id`, or its `id` names no parameter any command declares | an `entity_select` with a missing or unrecognized `entity` is skipped |
| an `entity_select` appears without the manifest declaring `save.read` | a `from` naming a command that no longer exists is skipped |
| a selectable `table` has no `id` | a `button` whose `command` names no known command is skipped |
| a `from` names no command the manifest declares | a `path` that resolves to nothing renders as an empty widget, not an error |
| a `button` has no `command`, or its `command` names no declared command | a section's `columns` outside `1`-`3` falls back to `1` instead of being refused |
| `args` appears on a non-`button`, names a key the button's command doesn't declare, or has a value that isn't `<widget>.selection`/`<widget>.value` naming a widget in the view | |

### Worked example: `pst.repair`'s view

From the bundled `pst.repair` plugin
(`psp-app/src/bundled/pst.repair/manifest.json`), whose two commands are
described below:

```json
"ui": [
  {
    "title": "Scan",
    "columns": 3,
    "widgets": [
      { "type": "entity_select", "id": "owner", "entity": "player", "label": "Owner" },
      { "type": "number_input", "id": "max_level", "label": "Maximum level" },
      { "type": "number_input", "id": "max_rank", "label": "Maximum condensing rank" },
      { "type": "button", "label": "Scan", "command": "scan_illegal_pals", "span": "full" }
    ]
  },
  {
    "title": "Illegal pals",
    "columns": 1,
    "widgets": [
      { "type": "text", "from": "scan_illegal_pals", "path": "summary" },
      { "type": "table", "id": "rows", "from": "scan_illegal_pals", "path": "pals",
        "columns": ["name", "level", "rank", "problems"], "selectable": true },
      { "type": "button", "label": "Fix selected", "command": "fix_illegal_pals",
        "args": { "ids": "rows.selection" } }
    ]
  }
]
```

- The **"Scan" section** lays three inputs and a button across three columns:
  an `entity_select` bound to `owner` (so the user can narrow the scan to one
  player, or leave it at the "Any" that an empty string means), and two
  `number_input`s bound to `max_level` and `max_rank`. The button spans the
  full width beneath them and runs `scan_illegal_pals` with whatever the
  three inputs currently hold.
- The **"Illegal pals" section** is where the scan's result lands. The `text`
  widget shows `scan_illegal_pals`'s own `summary` field verbatim. The
  `table` widget reads that same result's `pals` array, renders the four
  named columns, and - because `selectable` is `true` and it declares an
  `id` of `rows` - tracks which rows the user has checked.
- The **"Fix selected" button** runs `fix_illegal_pals` with `ids` set to
  `rows.selection`: exactly the `instance_id`s of the checked rows. Because
  `fix_illegal_pals` also declares `max_level` and `max_rank` params, the
  same two number inputs from the Scan section feed it too, so a user who
  adjusts the threshold and fixes a selection doesn't need to re-enter it.
  `fix_illegal_pals` is `destructive`, so pressing this button previews the
  clamp before anything is written, exactly as it would from the generated
  form.

## Capabilities

A capability that is not both declared in the manifest *and* granted at run
time installs no global at all - a script that references it fails with "field
X is nil" at the point of use, not with a capability error. This is
deliberate: the set of installed globals is exactly the set of things the
script is actually allowed to touch.

| Capability | Wire name | Installs |
|---|---|---|
| Save read | `save.read` | The `save` global's read half: `save.info()`, `save.players()`, `save.pals()`, `save.guilds()`, `save.bases()`, `save.containers()`, and every handle field read - except the `player` fields that are not answered from the save's player summary, which need `players` as well (see below). |
| Save write | `save.write` | The `save` global's write half: `delete_where` on the player/guild/pal iterators, `save.clear_slots_where()`, every handle's mutating methods (`player.delete()`, `pal.delete()`, `slot.clear()`, ...), and assignment to a `pal`, `player`, `guild`, `base` or `slot` handle's writable fields. Requires `save.read` to also be declared - the manifest is refused otherwise. |
| Save raw | `save.raw` | The `raw` global (`raw.get`/`exists`/`kind`/`set`/`delete`/`len`/`visit`). **Bundled plugins only in v1** - see below. |
| Players | `players` | Installs nothing of its own. Gates two things: the `player:<uid>` and `player_dps:<uid>` raw targets - `raw.get`/`raw.set`/etc. against a per-player scope refuse with an error unless this is also granted, and `raw` itself still needs `save.raw` - and reading any `player` handle field that is not answered from the save's own player summary. It is not a blanket gate on everything about a player: a player's entry in the level save (their `Exp`, `Level`, `FullStomach` and stat lists) is also reachable through the `level` raw target, which needs `save.raw` but not this. |
| Game data | `gamedata` | The `gamedata` global (`gamedata.is_valid_item()`, `gamedata.is_valid_pal()`, `gamedata.version()`, `gamedata.catalogs()`, `gamedata.keys()`, `gamedata.get()`). |
| UI dialog | `ui.dialog` | The `ui` global (`ui.confirm()`). |
| Storage | `storage` | The `storage` global (`storage.get()`, `storage.set()`). |
| Log | `log` | The `log` global (`log.info()`, `log.warn()`, `log.error()`). |

### Capability cheat sheet

Use this to decide what a plugin needs:

- `save.read` for reading save data and iterating players, pals, guilds, and bases.
- `save.write` when the command edits the save.
- `players` for player-scoped data that is not covered by the summary read path.
- `gamedata` for catalog lookups and validation.
- `storage` for small persistent plugin data.
- `ui.dialog` for user confirmation dialogs.
- `log` for returning debug and status messages.
- `save.raw` only for rare cases where the typed API is not enough.

`progress` and `ctx` are installed unconditionally for every run - reporting
progress and reading run metadata cannot leak anything, so neither needs a
capability.

A capability also has to appear in the manifest's own `capabilities` array to
take effect: `run_plugin_command`'s `granted` list is intersected against the
manifest's declared capabilities before installation, so a caller cannot grant
a plugin more than it declared it wants.

### `save.raw` is powerful, and the risk is yours to accept

Any plugin may declare `save.raw`. Read this before you do.

Every other capability writes through a typed model that validates what it is
given. `save.write` can make changes you did not want, but the result is always
a structurally valid save. **`raw.set` and `raw.delete` write untyped values at
arbitrary paths with no schema behind them, so a mistake can produce a save the
game refuses to load.** There is no undo inside a plugin run.

Three things follow from that:

- **A plugin only gets `save.raw` if you grant it.** The capability is declared
  in the manifest and granted at install time; the grant is where you decide
  whether you trust this code with unvalidated write access.
- **Back up the save before running a `save.raw` plugin you did not write.**
  Especially one from someone else.
- **Prefer the typed API where it can do the job.** `save.players()`,
  `save.pals()`, `save.containers()` and friends validate their writes and keep
  handles coherent. Reach for `raw` when the typed API genuinely cannot reach
  what you need - not as a shortcut around learning it.

Raw access is the main road for porting PalworldSaveTools' own functions, most
of which read and write the level tree directly rather than through a typed
model. That is why it is available rather than withheld.

If you publish a plugin that declares `save.raw`, say so plainly in its
description. Someone installing it is being asked to trust you with their save
file.

## The host API

### `ctx` - always installed

A plain table, rebuilt once per run before the script's command function is
called:

| Field | Type | Meaning |
|---|---|---|
| `ctx.dry_run` | bool | Whether this run is a dry run. Every mutating host function checks this itself - a script does not need to branch on it except to change its own wording (see the worked example). Branching the *logic* on it means maintaining two implementations of one count, which is how the two drift apart; reach for a bulk form whose predicate pass is mode-independent instead. |
| `ctx.api_version` | integer | The manifest's declared `api_version`. |
| `ctx.plugin_id` | string | The manifest's `id`. |
| `ctx.command_id` | string | The command id this run invoked. |
| `ctx.now` | integer | Unix seconds, sampled once before the script runs. Lua's `os` library is excluded from the sandbox, so this is the only clock a script has. |
| `ctx.args` | table | The command's declared parameters, coerced and defaulted, keyed by their `id`. |

### `progress` - always installed

- `progress.report(message, fraction)` - `fraction` is optional; when given it
  must be a finite number in `[0.0, 1.0]` or the call errors. Reports go to the
  same progress sink `psp-core`'s own domain calls use, so a consumer can see
  more updates than the script explicitly sent - treat it as "more updates
  than expected," not a 1:1 count with `report` calls.

### `log` - requires `log`

- `log.info(message)`, `log.warn(message)`, `log.error(message)` - each
  appends one line to the run's log, returned to the caller alongside the
  result. Capped at 1000 lines; the 1000th line is replaced with a single
  truncation notice and every call after that is a silent no-op.

### `storage` - requires `storage`

Per-plugin key/value persistence across runs.

- `storage.get(key) -> string|nil`
- `storage.set(key, value)` - `key` is capped at 128 bytes, `value` at 64 KiB;
  either limit is a hard error. Writes are buffered in memory for the run and
  only persisted by the caller once the run finishes with an `ok` status -
  a failed or timed-out run's storage writes are discarded.

### `ui` - requires `ui.dialog`

- `ui.confirm(message) -> bool` - under a dry run this always returns `true`
  without prompting anything (a dry run never actually needs confirmation,
  since nothing is written). On a real run it calls back into the host's
  confirmation channel if one was supplied, and returns `false` if not.

### `gamedata` - requires `gamedata`

- `gamedata.is_valid_item(id) -> bool` - `id` is the item's internal catalog
  id (e.g. `Wood`), matched exactly, never a display name.
- `gamedata.is_valid_pal(id) -> bool` - `id` is the pal's internal catalog id
  (e.g. `CuteFox`), matched case-insensitively (a save's `character_id` casing
  does not always match the catalog).
- `gamedata.version() -> string`
- `gamedata.catalogs() -> string[]` - the name of every top-level catalog the
  loaded game data ships, sorted. A catalog name is extension-less and
  lowercase (`pals`, `items`) - never `pals.json` or `Pals`. Catalog names are
  matched case-insensitively wherever `keys` or `get` take one below. The game
  data also loads two nested subtrees - the game's locale strings and the
  application's own interface strings - and neither is listed here.
- `gamedata.keys(catalog) -> string[]|nil` - the named catalog's top-level
  keys. `nil` if no catalog by that name exists. A catalog that exists but is
  not a JSON object answers an empty table rather than `nil` - five of the
  loaded game data's 33 top-level catalogs (`camps`, `eggs_spawners`,
  `kinship_peach`, `presets`, `skill_fruits`) are JSON arrays and hit this
  today. Unlike the catalog name - and unlike `is_valid_item` and
  `is_valid_pal`, which both fold case - a catalog's own keys are
  case-sensitive, and the two halves do not meet:
  `gamedata.is_valid_pal('cutefox')` is `true` while
  `gamedata.get('pals', 'cutefox')` is `nil`. An id that passes a validity
  check is not thereby a key you can look one up with; take the key from
  `gamedata.keys(catalog)`.
- `gamedata.get(catalog) -> any|nil`, `gamedata.get(catalog, key) -> any|nil`
  - the whole catalog, or one entry of it when `key` is given. `nil` if the
  catalog (or, with a key, the entry) does not exist - and also if the stored
  value there is JSON `null`; nothing distinguishes "absent" from "present
  and null" once it has crossed into Lua.

Every value `gamedata` hands back - from `keys` or `get` alike - is a
snapshot: a fresh value each call, and a fresh table when the value is one
(the same rule field reads follow - see below). Mutating what you get back
changes nothing in the loaded game data.

#### Examples: looping through game data

The sandbox does not provide a console `print` function. To print values from
a command, declare `log` in the manifest and use `log.info`, `log.warn`, or
`log.error`; the messages are returned with the run. These examples use
`ipairs` for the sorted arrays returned by `catalogs` and `keys`, and `pairs`
for the object tables returned by `get`.

List every available catalog:

```lua
function list_catalogs()
  log.info("Game data version: " .. gamedata.version())

  for _, catalog in ipairs(gamedata.catalogs()) do
    log.info(catalog)
  end
end
```

List the top-level keys in one catalog without loading the catalog's values:

```lua
function list_pal_ids()
  local keys = gamedata.keys("pals")
  if not keys then
    log.warn("The pals catalog is not loaded")
    return
  end

  for _, key in ipairs(keys) do
    log.info(key)
  end
end
```

Fetch and print one entry. The key must use the exact spelling returned by
`gamedata.keys`; validity checks and lookups do not use the same casing rules:

```lua
function show_first_pal()
  local keys = gamedata.keys("pals")
  if not keys or #keys == 0 then
    log.warn("The pals catalog is empty or not loaded")
    return
  end

  local pal = gamedata.get("pals", keys[1])
  for field, value in pairs(pal) do
    log.info(string.format("%s = %s", field, tostring(value)))
  end
end
```

For nested data, this bounded walker prints object fields and array entries
without turning a large catalog into one enormous log. It also avoids relying
on table key order for object data:

```lua
local function dump(value, path, depth)
  if depth > 2 or type(value) ~= "table" then
    log.info(string.format("%s = %s", path, tostring(value)))
    return
  end

  for key, child in pairs(value) do
    dump(child, path .. "." .. tostring(key), depth + 1)
  end
end

function inspect_first_item()
  local keys = gamedata.keys("items")
  if not keys or #keys == 0 then
    log.warn("The items catalog is empty or not loaded")
    return
  end

  dump(gamedata.get("items", keys[1]), "items." .. keys[1], 0)
end
```

Each call to `gamedata.get` returns a new snapshot. Changing `pal` or the
value passed to `dump` in these examples would not change the loaded game
data. A manifest for any of these commands needs at least
`"capabilities": ["gamedata", "log"]`.

**A `gamedata.get` can refuse.** Building a value as one Lua table is capped
at 150,000 JSON nodes - one per array element or object entry, counted over
the whole tree - and a fetch past that cap errors, naming what was asked for
and the count against the limit, rather than building a table nobody could
use. The cap is one rule for the whole call: a single entry is measured the
same way a whole catalog is, so a keyed fetch can refuse too. The figure in
the message is that node count over the whole tree, which is not the number
of keys `gamedata.keys` lists.

Measured against the currently loaded game data, **every catalog and every
entry fits**, so no fetch refuses today:

- `gamedata.get('breeding')` (106,942 nodes) is the largest whole catalog,
  and its largest single entry,
  `gamedata.get('breeding', 'child_to_parents_formula')` (102,226 nodes),
  fetches on its own too.
- `gamedata.get('breeding_distance')` (92,720 nodes) is the next largest
  catalog, then `gamedata.get('pals')` (51,680 nodes).

The cap leaves the largest catalog room to grow by about 40% before it would
begin refusing. It is not a budget for the whole data set: each call builds
one table, and the 33 catalogs total roughly 390,000 nodes between them, so
loading every catalog means 33 separate fetches rather than one.

### `save` - read half requires `save.read`

- `save.info() -> { world_name, save_id, player_count, guild_count, pal_count }`
- `save.players()`, `save.pals()`, `save.guilds()`, `save.bases()`,
  `save.containers()`, `save.map_objects()` - each returns a stateless Lua
  iterator over handles, for use as `for x in save.players() do ... end`.

Every handle exposes fields through `__index` (a plain field read, not a
method call - `player.name`, not `player.name()`):

| Handle | Fields |
|---|---|
| `player` | `uid`, `name`, `level`, `guild_id`, `pal_count`, `last_online` (ISO string or nil), `last_online_ts` (Unix seconds or nil), `instance_id`, `exp`, `hp`, `stomach`, `sanity`, `technology_points`, `boss_technology_points`, `effigy_possess_num`, `pal_box_id`, `otomo_container_id`, eight table-valued fields: `technologies`, `completed_missions`, `current_missions`, `unlocked_fast_travel_points`, `collected_effigies`, `defeated_bosses` (lists of id strings) and `status_point_list`, `ext_status_point_list` (points keyed by stat name), plus `pals` (a `player.pals()` iterator factory) |
| `pal` | `instance_id`, `character_id`, `character_key`, `nickname`, `owner_uid`, `guild_id`, `base_id`, `gender`, `level`, `hp`, `max_hp`, `rank`, `exp`, `talent_hp`, `talent_shot`, `talent_defense`, `rank_hp`, `rank_attack`, `rank_defense`, `rank_craftspeed`, `is_boss`, `is_lucky`, `is_awakened`, `is_imported`, `is_predator`, `is_tower`, `is_sick`, `group_id`, `stomach`, `sanity`, `friendship_point`, `storage_id`, `storage_slot`, and four table-valued fields: `learned_skills`, `active_skills`, `passive_skills` (lists of catalog id strings) and `work_suitability` (ranks keyed by work type) |
| `guild` | `id`, `name`, `admin_uid`, `player_count`, `base_count`, `level`, `pal_count`, `chest_container_id` (a container id string, or `nil` when the guild has no chest) |
| `base` | `id`, `guild_id`, `name`, `area_range` (the radius of the base's working area, in world units), `x`, `y`, `z`. Everything but `id` reads `nil` on a base whose record in the save could not be read - the iterator hands out a handle for any entry that has an id, without checking there is anything behind it. |
| `container` | `id`, `slot_count`, `slots` (a `container.slots()` iterator factory). Nothing on this handle is assignable - see below. |
| `slot` | `index`, `item_id` (nil for an empty slot), `count` |
| `map_object` | `id`, `instance_id`, `base_id`, `guild_id`, `build_player_uid`, `hp`, `max_hp`, `kind` |

A `map_object` handle covers built structures, chests and resource nodes.
`id` and `instance_id` answer two different questions: `id` is the
`MapObjectId` asset name shared by every instance of a kind - every
`PalBoxV2` in the world reads the same `id` - while `instance_id` is the
UUID of this one object and nothing else shares it. A predicate that wants
to act on one specific object needs `instance_id`; one that wants to act on
a kind of object needs `id`. `base_id`, `guild_id` and `build_player_uid`
read `nil` when the object has none of the corresponding kind - never the
save's own zero guid - and `kind` is the concrete model type name the
object was built from. Of the eight fields, only `hp` and `build_player_uid`
are assignable; the rest are read-only.

An unresolvable field name returns `nil` rather than erroring - reading a
typo'd field looks like reading an absent one. A field that *is* real can
still raise, though, and one class of read does: every `player` field that is
not answered from the save's player summary is served out of that player's full
DTO, and building that DTO reads the player's own `.sav`. If that file cannot be
read, the field raises - whichever of the two files the field itself comes from.
That is deliberate - `nil` already means "this field has no value", and
answering `nil` for "your save could not be read" would let a plugin quietly
compute from a wrong answer.

Most of a `player`'s fields are answered from a summary the save already holds.
The ones that are not - everything past `last_online_ts` in the table above -
come from that player's full DTO, built the first time one of them is touched and
then kept for the rest of the run. Building it reads the player's own `.sav` from
disk, which is what makes those fields cost something: reading `uid`, `name`,
`level`, `guild_id`, `pal_count` or the two `last_online` fields across every
player costs nothing extra; reading `exp` does.

The DTO draws on two files, and which one a field comes from matters for what
follows:

- **From the player's own `.sav`** - `technologies`, `technology_points`,
  `boss_technology_points`, `completed_missions`, `current_missions`,
  `unlocked_fast_travel_points`, `collected_effigies`, `defeated_bosses`,
  `effigy_possess_num`, `pal_box_id`, `otomo_container_id`.
- **From that player's entry in the level save** - `instance_id`, `exp`, `hp`,
  `stomach`, `sanity`, `status_point_list`, `ext_status_point_list`.

**All eighteen need the `players` capability, not just `save.read`.** None was
reachable under `save.read` before these fields existed, but not for the same
reason: the eleven needed `raw` with a `player:<uid>` target, which `players`
already gates, while the seven needed `raw` with the `level` target, which needs
only `save.raw`.

Gating both sets the same way is a deliberate choice, and for those seven it is
not the bar they came from. `save.raw` is an escape hatch for reaching what the
typed API cannot, not a sensitivity tier - those fields required it because no
typed accessor existed, never because anyone judged them sensitive. Making you
request the most dangerous capability in the system to read a player's stomach
value would push authors to ask for raw access for benign reads and users to
grant it, which is the opposite of what that capability's warnings are for.
`save.read` plus `players` is the right bar for per-player data of either origin.

A `pal` handle's `stomach` and `sanity` are the same numbers about a different
subject, and they need only `save.read`. That is not an oversight: `players`
gates data about a person - the account behind a save entry, what they have
unlocked, where they have been - and a pal's condition is data about a creature
in the world, which `save.read` has always covered. `pal.owner_uid` and
`pal.guild_id` were reachable under `save.read` before any of these fields
existed, for the same reason.

Reading one of the eighteen without `players` raises and names the capability.
The summary-backed fields are unaffected and need only `save.read`. `psp.lua`
says which is which, in each field's own entry.

*Writing* one of those fields needs only `save.write` - rewriting a player's
data, both their level-save entry and their own `.sav`, is what the write half
has always done. So a plugin holding `save.read` and `save.write` can set
`player.exp` but cannot read it back; declare `players` too if it needs to.

### `save` - write half requires `save.write` (and therefore also `save.read`)

Adds mutating methods onto the same handles, plus a bulk `delete_where` on the
three entity iterators that support it:

- `player.delete() -> bool` - refuses (returns `false`, does not error) rather
  than deleting a guild admin.
- `pal.delete() -> bool`
- `guild.delete() -> bool` - also deletes every loaded member's player entity.
- `base.delete() -> bool` - also deletes the base's worker pals.
- `slot.clear()` - **structural**, not an in-place empty: it removes the raw
  slot entry rather than overwriting it, so every later slot in that container
  shifts down one position. This invalidates the `container.slots()` iterator
  that produced the handle, exactly like any other structural write - see the
  mutation rule below.
- `container.set_slot_count(n) -> bool` - resizes the container to hold `n`
  slots. Returns `true` when it resized, `false` when it **refused** because
  an occupied slot falls outside the new capacity - a refusal writes nothing.
  Raises a catchable error if the container id no longer resolves. Growing
  raises the capacity without creating slot entries: the raw `Slots` array is
  sparse, holding an entry only for an occupied slot, so an empty slot is
  represented by absence, the same representation `slot.clear()` produces.
  A *successful* call is **structural**, exactly like `slot.clear()` - it
  bumps the mutation epoch and invalidates every live handle and iterator, so
  reading a field off the same handle right afterward raises; re-fetch it
  instead. A refusal does not bump the epoch and leaves every handle valid.
- `save.players():delete_where(fn(player) -> bool)`,
  `save.guilds():delete_where(fn(guild) -> bool)`,
  `save.pals():delete_where(fn(pal) -> bool)`,
  `save.map_objects():delete_where(fn(map_object) -> bool)` - collects a
  snapshot of every id first, calls `fn` once per id with a handle (no save
  mutation may happen from inside the predicate - see below), then deletes
  every id the predicate returned truthy for. Returns two integers: `removed,
  unresolved`. `unresolved` counts ids the predicate selected but that the
  host itself could not resolve at apply time (e.g. an already-dangling
  reference) - a different thing from the predicate choosing to skip an id
  itself, and worth keeping as a separate count in a command's own summary
  (see the worked example).
- `save.clear_slots_where(fn(slot) -> bool) -> cleared, examined` - the slot
  counterpart to `delete_where`, and the only practical way to clear more than
  one slot in a run. Walks every container once, calls `fn` with a slot handle
  while nothing has been mutated yet, then applies every selected clear
  afterwards, grouped so each container is written once. `examined` is the
  number of slots the predicate saw, `cleared` the number it selected. It hangs
  off `save` rather than off `save.containers()` because that iterator is a
  plain function value, which Lua gives you nowhere to attach a method to.
- `save.unlock_private_chests() -> integer` - clears the ownership lock
  (`private_lock_player_uid`) on every locked private chest and item booth in
  the world, and returns the number of locks **actually cleared**; a chest
  that was already unlocked is not counted. Deliberately does **not** touch
  `PasswordLock` module state (its `password`/`player_infos` fields) - that is
  a separate access-control mechanism, and clearing it is a larger change than
  a user asking to unlock chests has consented to. It is **structural**, and
  bumps the mutation epoch, exactly like `slot.clear()` and
  `container.set_slot_count()` - but only when the count is non-zero; a run
  that clears nothing (including any dry run, which never writes) leaves every
  handle and iterator valid.
- `save.remove_orphaned_works() -> integer` - removes every `WorkSaveData`
  entry whose owning map object no longer exists, returning how many were
  removed. Structural on a non-zero result, the same way
  `save.unlock_private_chests()` is.
- `save.remove_orphaned_dynamic_items() -> integer` - removes every
  `DynamicItemSaveData` entry that no item-container slot, dropped item, item
  booth trade or damage-drop table still points at, returning how many were
  removed. Structural on a non-zero result.
- `save.delete_dps_pals(player_uid, indexes) -> integer` - empties the given
  slot indexes of one player's dimensional storage in place: it nils the
  slot's instance id and resets its parameter bag to an unused slot's shape,
  the same way the slot got there in the first place, without changing the
  storage array's length. Returns how many of the given indexes were valid.
  Requires `players`, in addition to `save.write`.

Every write method's dry-run behaviour is: validate what it can, bump an
internal count, and return without changing anything. `slot.clear()` under a
dry run is a documented no-op, which is a trap for any loop that clears until a
container is empty: the loop's exit condition never arrives and the run times
out instead. `save.clear_slots_where` has no such split - its predicate pass is
identical in both modes and only the apply phase is skipped - so prefer it over
hand-rolling the loop, and reach for `ctx.dry_run` branching only if you must.

#### Writing a handle's fields

The writable fields of a `pal`, `player`, `guild`, `base` or `slot` handle are
written by assigning to them, not by calling a setter:

```lua
pal.level = 60
pal.talent_hp = 100
pal.nickname = "Sparky"

player.level = 50
player.technology_points = 200
player.name = "Ada"

guild.name = "The Guild"
guild.level = 12

base.name = "Main Camp"
base.area_range = 3500

slot.count = 5
slot.item_id = "Wood"
```

An out-of-range value raises rather than being clamped, and the message names
the field. Assigning a field that does not exist raises too, so a typo is an
error rather than a silent no-op. Not every readable field is writable -
identity fields (a pal's `instance_id`, `owner_uid`, `guild_id`, `base_id`; a
player's `uid`; a base's `id` and `guild_id`), derived ones (`player.pal_count`,
`guild.player_count`) and fields the game itself recomputes whenever the pal is
saved (`pal.hp`, `pal.stomach`, ...) are read-only, and assigning one raises
saying so. A base's `x`, `y` and `z` are read-only for a blunter reason:
nothing in this app writes a base's position at all, so there is no write path
to offer - and moving a base is not merely a missing setter, since its placed
structures and working pals carry their own coordinates that nothing relates
back to the base's. The authority on which is which is `psp.lua`: a
type-annotation file generated from the same host API definition these docs
describe, in which every read-only field says so in its own entry. It says so in
prose only: LuaLS has no read-only field modifier, so an assignment to one
type-checks like any other and is refused only once the command runs. The file
is written into the plugin's workspace only where the editor's full tier runs,
since it exists for that tier's language server to read - on the baseline tier
there is not even that entry to read, and the runtime error on a refused
assignment is all there is.

Assignment is non-structural: every live handle and iterator stays valid, and a
read straight afterwards sees the new value. That holds for a player too - a
player write deliberately leaves the player's own item containers out of what it
writes back, so nothing a `container` or `slot` handle points at can move under
it. A guild write does the same with the guild's bases and its shared chest, and
a base write with the base's storage containers: the save's own writers would
otherwise walk each of those into a container rewrite that no handle or iterator
could see coming. A `slot` write is non-structural for a blunter reason: it
overwrites that one raw slot entry in place and touches no other slot, so a
`container.slots()` iterator stays walkable across it - unlike `slot.clear()`,
which removes the entry and does not.

Some fields are refused not because the value is wrong but because the change is
**structural** - it adds or removes an entry, which invalidates every live
handle and iterator. Those stay calls, and the refusal names the call to use:

- `container.slot_count` is read-only; resizing is `container.set_slot_count(n)`.
  Nothing at all on a `container` handle can be assigned.
- `slot.item_id` cannot be assigned `"None"`: that is the one value the save
  reads as "delete this slot". Use `slot.clear()`.
- `slot.count` cannot be assigned zero or less, for the same reason - a slot
  holding none of its item is an empty slot.
- `slot.index` is read-only: moving a slot means removing and re-adding it.

`slot.item_id` also refuses `nil` and `""`, but not because either is
structural - neither removes anything. Both read back as an empty slot without
emptying one: the entry would still be there, holding an item with no id. They
are refused because emptying a slot is what they mean, and `slot.clear()` is
what does it.

`slot.item_id` must also name an item the loaded game data knows. The match is
case-insensitive, because save ids and the catalog do not agree on casing, and
the id reaches the save exactly as you wrote it. If no catalog is loaded the
check is skipped rather than refusing everything - an unavailable catalog is not
evidence that an id is wrong.

Finally, `slot.item_id` is refused on a slot carrying a per-item record - a
weapon's durability and remaining rounds, an egg's pal, armour's condition. That
record names its own item and nothing here can rewrite it, so re-pointing only
the slot would leave the two disagreeing. A `slot.count` write, on the other
hand, keeps the record exactly as it was: the record is carried across the write
untouched, not rebuilt and not dropped.

A few values are refused rather than written because the save itself reads them
as "leave this alone", so the assignment would quietly do nothing: an empty
`guild.name` or `base.name`, and a `guild.level` of zero. Assigning either
writable `base` field is refused outright on a base whose record could not be
read - the same base whose `name`, `area_range`, `x`, `y` and `z` all read
`nil` - since there would be nothing for the write to land in.

**Under a dry run** an assignment is validated and counted like the write
methods above, but it does not simply return - the new value goes into the
run's own cache for that handle, so every later read in that same run sees it,
which is what lets a preview compute from values it has itself assigned. Nothing
reaches the save, and the cache is discarded when the run ends. A `slot`
assignment behaves the same way from the outside, though it gets there
differently: on a real run it lands on the save at the point of assignment
rather than being held and flushed later, so the read afterwards is a read of
the save.

One asymmetry to know about, which predates field assignment: **a dry run never
invalidates handles**, because no structural operation performs its mutation
under one. So a dry run can call `slot.clear()` and then keep reading that slot,
where a real run would raise. A previewed clear does at least drop whatever the
same run had previewed assigning to that slot, so the two do not contradict each
other - the read falls back to what the save holds.

**Reads are snapshots, not live views.** A field read hands back a copy - for a
table field, a fresh table each time - so mutating what you read changes
nothing. Write the whole value back:

```lua
-- mutates a copy; does nothing
table.insert(pal.active_skills, "EPalWazaID::FireBall")

-- correct
local skills = pal.active_skills
table.insert(skills, "EPalWazaID::FireBall")
pal.active_skills = skills
```

The three skill fields are checked entry by entry against the game's own skill
catalogs, so their ids have to be spelled exactly as the catalog spells them -
`"EPalWazaID::FireBall"`, not `"FireBall"` and not `"fireball"` - and the
refusal names the entry it rejected. `work_suitability` is not catalog-backed:
its keys are checked against a fixed set of work types the host knows, and a
key outside that set is refused with the whole accepted set listed for you.

A player's two stat-point maps (`status_point_list`, `ext_status_point_list`)
work the same way, and their key sets are not identical: `capture_rate` is a
base stat with no extended-stat row. A key outside either set is refused rather
than dropped - writing the save itself would drop it silently.

Those two maps are a genuine replacement, and the host has to work for it: the
save's own writer *merges*, so a key left out of the assigned table would keep
whatever it already held. Every key is therefore written, with the ones you left
out set to zero - the only way the save can say "no points spent", since a stat
row cannot be removed once it exists. A stat the save has never carried and that
you leave out stays absent and reads back `nil`, so assigning `{}` clears every
stat that has a row and leaves the rest alone. The read agrees with that before
and after the change is written, which is the point.

Three of a player's fields refuse a value that would not read back as what was
written. `player.name` refuses an empty string and refuses the exact
placeholder the save uses for a nameless player, because writing either one
makes the name read back as a generated placeholder instead. `player.stomach`
and `player.sanity` are stored as 32-bit floats, so a value outside that range
is refused rather than written as an infinity.

A player's `technologies`, `completed_missions` and `current_missions` are not
checked against anything: no part of the save-writing path compares those names
to the game's own lists, so any string is accepted and stored as given.

`player.collected_effigies` moves `player.effigy_possess_num` as a side effect,
by the number of keys newly collected minus the number un-collected, floored at
zero. `effigy_possess_num` counts *unspent* effigies rather than collected ones,
so the two are not a running total of each other: replacing a 58-key set with a
2-key one takes a count of 3 down to 0, not to -53.

### `raw` - requires `save.raw`

Direct GVAS tree access, available to any plugin that declares `save.raw`.
Per-player targets also require `players`. Writes here bypass schema
validation and can produce a save the game cannot load - see the capability
notes above before using it.

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

- `"level"` - the level file's GVAS root.
- `"player:<uid>"` - one player's own `.sav` tree. Requires `players`.
- `"player_dps:<uid>"` - that player's `_dps.sav` tree. Requires `players`.

`path` is a dotted/indexed address:
`worldSaveData.CharacterSaveParameterMap[3].value.RawData.SaveParameter.IsPlayer`
- a segment is a property key, or `[n]` for a map/array index.

**`[n]` also steps into a scalar array's own elements**, not only into an
array of structs. An array whose elements are themselves scalars - a `Name`,
`Str` or `Enum` array, an array of integers of any width, a `Float` or
`Double` array, a `Bool` array, or a `Byte`/`Label` array - has each element
individually addressable by index, the same way `SaveParameterArray[3]`
addresses one struct element of that array. `raw.get`, `raw.set`,
`raw.delete`, `raw.len` and `raw.kind` all reach an element this way, and a
`raw.visit` walk descends into one the same way it descends into a struct
array, handing the callback one node per element with its own `path`. `raw.kind`
reports an individual element as `"scalar"` and the array itself as
`"array"`, exactly as it does for an array of structs. **`raw.set` writes one
element at a time and can never write a whole array in one call** - the path
has to name an element, not the array, so replacing an array's contents means
setting each element (or deleting and re-adding elements) one at a time
rather than assigning a new array wholesale.

Error discipline: `raw.get` and `raw.len` **error** when `path` does not
resolve to any node at all (a typo'd segment or an out-of-range index) - a
literal path string is exactly the kind of typo that needs to fail loudly at
the point of use. `raw.exists` and `raw.kind` are the deliberate probes for
"is anything here" and "what shape is it", and never error on an unresolved
path - they answer `false`/`nil`. `raw.delete` returns `false` (not an error)
when nothing was there; scripts rely on that as a meaningful outcome, not an
error case.

`raw.visit`'s callback receives one table per visited node:

| Field | Meaning |
|---|---|
| `node.key` | The node's own key/field name, or `nil` at the root. |
| `node.value` | The node's scalar value, or `nil` if it is not a scalar (a struct/map/array/opaque node). |
| `node.path` | A ready-made, re-parseable path string for this node, usable directly with `raw.get`/`raw.set`/`raw.delete` - or `nil` when the node cannot be faithfully addressed (an unfaithful path would be worse than none). |
| `node.index` | The node's index within its parent array/map, or `nil`. |
| `node.depth` | Depth from the walk's starting path. |
| `node.kind` | `"scalar"`, `"struct"`, `"map"`, `"array"`, `"entry"`, or `"opaque"`. |

The callback returns one of `"keep"` (default - anything other than `"remove"`
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
these functions with Lua - not maintained separately from it - describing
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
directions - everything Lua exposes is described, and everything described
actually exists in Lua. A drift between the two fails that test, not a
review.

Handle fields get a further check: every field this document lists under a
handle is probed against a live handle from a real save fixture and confirmed
to resolve without error, with a value whose Lua type matches what's
described. That check only runs one way, though. It cannot also prove the
reverse - that nothing a handle answers goes undescribed - because a handle's
fields are dispatched through an `__index` function rather than a plain
table, with no `__pairs` to enumerate them, so Lua has no way to ask a handle
what it holds, only to ask about one name at a time. An unrecognised field
name comes back as plain `nil`, indistinguishable from a legitimately-nilable
field that just happens to be absent on this instance - so a phantom field
only gets caught here if the description also claims it is never nil. In
short: a field listed under a handle above is proven to exist with the type
stated; a name not listed there is not thereby proven absent.

## The mutation-during-iteration rule

A **handle** (returned by `save.players()`, `player.pals()`,
`save.containers()`, etc.) carries the mutation epoch that was current when it
was created. Every structural change - anything that adds or removes an
entity, including `slot.clear()` - bumps that epoch. A handle whose epoch no
longer matches the run's current epoch is refused on its next field read or
method call with:

> this handle was invalidated by a change made during iteration; use a bulk
> form such as `delete_where`

This is why `for p in save.players() do if should_delete(p) then p.delete()
end end` is unsafe in general (deleting one player invalidates every handle
still live from that same iterator, including the one just used) and why the
bulk forms exist: `delete_where` does its own predicate pass first - no
mutation happens until every id has been decided - then applies the deletions
in a single pass afterward, once no live handle from the original iterator is
still in use. `save.clear_slots_where` does the same for item slots.

Restarting the walk after each single write is the shape to avoid, even though
it is the only one the per-handle methods alone permit. It is quadratic in
matches, and for slots it is worse than that: every `slot.clear()` also drops
the world's container index, so each restart pays to rebuild that index across
the whole save. On a large world it does not finish. If you find yourself
writing one, the bulk form you need is missing - say so rather than working
around it.

**`raw` writes trip this too, and across scope boundaries.** `raw.delete`
bumps the same `mutation_epoch` on a real (non-dry) run, but only when it
actually removes something; a removing `raw.visit` bumps it the same way,
once, after the walk finishes if it removed at least one node. `raw.set` does
not bump it - overwriting a scalar in place moves nothing, so handles and
iterators stay valid. The epoch itself is a single counter for the whole run,
not scoped to whichever `target` the raw call touched, so a `raw.delete`
against `player:<uid>` invalidates every live handle and iterator for the rest
of that run - including a `save.players()` iterator still walking the level
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
`delete_where` uses internally - finish reading everything first, then issue
the raw writes from a plain table with no live handle held across them:

```lua
local uids = {}
for player in save.players() do uids[#uids + 1] = player.uid end
for _, uid in ipairs(uids) do
  raw.delete("player:" .. uid, "SaveData.SomeArray")
end
```

`fix_missions` in the bundled `pst.reset` plugin uses exactly this shape -
see below.

Collecting first is worth reaching for even where nothing is invalidated and
the loop would run correctly: assigning a `pal` field drops the host's pal
snapshot, and the next step of a live `save.pals()` iterator rebuilds that
snapshot from every pal in the save, so a walk that assigns as it goes rebuilds
it once per pal where the two-pass shape rebuilds it not at all.

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
| Cancelled | `"cancelled"` | The run was cancelled. **Not reachable in v1** - the `Cancel` registry and `cancel_plugin_run` message exist and are tested, but no client ever holds a run's id while it is still in flight (it first appears in the terminal result frame), so nothing can ever actually name a run to cancel. The panel shows the control disabled with an explanation instead of pretending it works. |
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
repository ships** - see spec §20.3.

The sandbox opens exactly six Lua libraries: the base library, `coroutine`,
`math`, `string`, `table`, and `utf8`. `io`, `os`, `package`, and `debug` are
never opened. `load`, `loadfile`, `dofile`, and `xpcall` are removed even
though the base library installs them (`xpcall`'s message handler runs with
interrupt hooks disabled, so neither the deadline nor cancellation could ever
reach a handler that hangs). `pcall` stays and is safe to use. Precompiled Lua
bytecode is refused at load - only text source is accepted. A script's
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
A table deeper than 32 levels or wider than 150,000 total nodes fails the
conversion - the run still succeeds, but with no `result`.

Under a dry run specifically, every mutating host function (`raw.set`,
`pal.delete`, `players():delete_where()`, ...) also bumps its own count key
(for example `"guilds.delete_where"`) the moment it runs, independently of
whatever the script itself returns. Assigning a handle field does the same,
under the handle's name followed by the field name - `pal.level = 60` bumps
`"pal.level"`, `guild.name = "x"` bumps `"guild.name"` - once per assignment
the host accepts, so a refused assignment contributes nothing and assigning the
same field twice counts twice.

The outcome's final `counts` map is the union of those host-bumped keys and
the script's own returned `counts` table - so a dry-run caller can see keys
the script never set itself (see the worked example below, where a live run
showed both `guilds` and `guilds.delete_where` in the same `counts` map). A
real (non-dry) run never gets these host-bumped keys - mutating functions
report what they did through their own return values instead, so a real run's
`counts` reflects only what the script itself put there.

## Multi-file plugins and `require`

A plugin's sources are not limited to the single file named by `entry`. Any
other file in `sources` is pulled in only when something calls `require` for
it - nothing scans the source map and runs every file it finds.

`require` is implemented by the host itself, not by Lua's own `package`
library - that library is one of the ones never opened (see the sandbox
limits below), so there is no `loadlib`, no C searcher, and no filesystem
`package.path`/`package.cpath` for a script to point anywhere. `load`,
`loadfile`, and `dofile` stay `nil` as well. `require` is the only way to pull
in another file, and it resolves exclusively against the plugin's own
`sources` - there is no way to reach another plugin's files, the filesystem,
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
  and returns a plain boolean - no mutation happens inside it, satisfying the
  mutation-during-iteration rule; `delete_where` performs the actual deletes
  itself, afterward, in its own pass.
- `removed` is how many guilds were actually deleted; `unresolved` is
  `delete_where`'s own second return - guilds the predicate selected that the
  host could not resolve at apply time (a dangling reference), kept separate
  from anything the predicate itself chose to spare so a summary can report
  "the script chose to leave this alone" and "this could not be resolved at
  all" as two different things.
- The returned table's `summary` and `counts` are both lifted onto the run
  outcome directly (and the whole table is also available as `result`), so a
  UI panel can render either the summary text or the structured counts
  without parsing the other.

## The `pst.cleanup` plugin

Bundled at `psp-app/src/bundled/pst.cleanup/` (`psp-app/src/bundled/pst.cleanup/main.lua`),
`pst.cleanup` ("Save Cleanup") ports PalworldSaveTools' Delete family: thirteen
commands that remove dead or invalid save entries, as against resetting
regenerable state (`pst.reset`) or clamping a value back into range
(`pst.repair`). The manifest declares `save.read`, `save.write`, `save.raw`,
`players`, `gamedata`, and `log`: `save.raw` and `players` because several
commands address a `player:<uid>` or `player_dps:<uid>` raw target directly,
`gamedata` because several check an id against a loaded catalog, and `log`
for `remove_invalid_items_from_save`, the only command here that reports
unknown ids through `log.warn` rather than folding them into its result.

| id | title | params | does |
|---|---|---|---|
| `delete_all_skins` | Delete All Skins | none | Clears every applied-skin field in the world and clears each player's own stored skin-inventory record. Skins are cosmetic, so nothing else about the save changes. |
| `delete_imported_pals` | Delete Imported Pals | none | Removes every pal flagged DNA-imported, from the world and from each player's dimensional storage. |
| `delete_empty_guilds` | Delete Empty Guilds | none | Removes guilds with no remaining members. This is the command quoted in the worked example above. |
| `delete_inactive_players` | Delete Inactive Players | `days` (default `30`, `1..=3650`) | Removes players whose `last_online_ts` is older than `now - days`. Guild admins are never removed. |
| `fix_all_negative_timestamps` | Fix Future Timestamps | none | Clamps a `last_online` timestamp that sits in the future back to the world's current time. |
| `remove_invalid_pals_from_save` | Remove Invalid Pals | none | Removes pals whose species id is not in the game's pal catalog, from the world and from each player's dimensional storage. Bosses, lucky pals and predators are kept. |
| `remove_invalid_items_from_save` | Remove Invalid Items | none | Clears item slots whose item id is not in the game's item catalog. |
| `remove_invalid_passives_from_save` | Remove Invalid Passive Skills | none | Removes passive skills that are not in the game's passive-skill catalog, from pals in the world and in each player's dimensional storage. |
| `delete_duplicated_players` | Delete Duplicated Players | none | Where one player uid has more than one character record, keeps the most recently online one and removes the rest. |
| `delete_inactive_bases` | Delete Inactive Bases | `mode` (`inactive` / `below level` / `both`, default `inactive`), `days` (default `30`), `level` (default `10`) | Removes bases whose guild members all fail the chosen filter. |
| `delete_non_base_map_objects` | Delete Structures Outside Bases | none | Removes structures whose `base_id` is set but does not resolve to a base, and the work entries that referenced them. |
| `delete_invalid_structure_map_objects` | Delete Invalid Structures | none | Removes structures whose `id` is not in the game's building catalog, and the work entries that referenced them. Treasure boxes, resource nodes, dropped items and death bags are kept. |
| `delete_unreferenced_data` | Delete Unreferenced Data | none | Removes ownerless pals, clears stale structure-builder references, and removes orphaned work records and unreferenced item records. |

### Behavioural notes

- **`remove_invalid_pals_from_save` keeps bosses, lucky pals and predators.**
  A boss or predator's `character_id` carries a `BOSS_` or `PREDATOR_`
  prefix the pal catalog does not list under, so a straight catalog lookup
  would misread every one of them as invalid; both prefixes are checked
  directly instead of trusting the catalog for them. A lucky pal is not
  flagged `is_boss`, but its `character_id` carries the same `BOSS_` prefix a
  real boss's does, for the same reason - `is_lucky` covers exactly that gap.
- **`delete_non_base_map_objects` only removes an object whose base id is set
  but unresolvable.** An object with no base id at all is not an orphan -
  it is world content with no base to belong to in the first place: a
  treasure box, a resource node, a dropped item. Those are never touched by
  this command, only structures that name a base that no longer exists.
- **`delete_invalid_structure_map_objects` spares treasure boxes, resource
  nodes, dropped items and death bags.** The building catalog is not a
  superset of everything a save's structure list holds - those four are
  legitimate world content with no catalog entry, and are recognised by id
  rather than mistaken for invalid structures. The catalog match itself is
  case-insensitive, since several real structure ids differ from their
  catalog key only in case.
- **`delete_inactive_bases` skips a base whose guild has no readable
  members, counting it separately rather than deleting it.** A guild with no
  member found here means unknown, not inactive: a member's own `.sav` might
  simply be missing from the save bundle. Such a base is left alone and
  counted under its own `skipped_unknown`, not folded into the removed count
  or silently treated as empty.
- **`delete_duplicated_players` leaves guild membership records alone.** It
  deletes the discarded character entry from `CharacterSaveParameterMap`
  directly, but the guild roster's own copy of that player's membership
  lives in a typed struct `raw` cannot reach - see "Data unreachable through
  raw" below. Which record survives is decided by `last_online_ts` (most
  recent wins); a tie, including two missing timestamps, keeps the lower map
  index, so the outcome never depends on visit order.
- **`delete_all_skins` also clears each player's stored skin inventory.** A
  player's `SkinInventoryInfo` block exists in every player's save whether or
  not a skin was ever applied, unlike the `SkinName` and
  `SkinAppliedCharacterId` fields it clears everywhere else, which only exist
  where a skin actually is - so it is counted separately rather than folded
  into the same count.
- **`fix_all_negative_timestamps` only clamps the copy `save.players()` reads
  from.** A player's `last_online` timestamp also has a duplicate copy in the
  guild roster's own member-info block, for guild membership display; that
  copy is unreachable through `raw` for the same reason guild rosters always
  are (see below), so this command cannot update it and does not claim to.
- **`delete_unreferenced_data` cannot use `save.pals():delete_where` for its
  ownerless-pal sweep.** That bulk form's apply phase requires the pal's
  owning player or guild base to still resolve, which is exactly what an
  ownerless pal fails by definition. It deletes the
  `CharacterSaveParameterMap` entry directly instead, the same way
  `delete_duplicated_players` does, since a pal with no owner and no base
  worker slot needs nothing else resolved to remove it.

### Data unreachable through raw

Guild rosters (`GroupSaveDataMap`) decode to a typed Rust struct, not the
generic property bag `raw.*` walks - every entry's `RawData` reports
`raw.kind` as `"opaque"`, and an opaque node has no children no matter how
its path is written. That is why `delete_duplicated_players` leaves the
discarded record's guild-membership entry in place, and why
`delete_unreferenced_data` does not cascade a removed pal's or player's
membership out of its guild: neither command has anything to call and
nothing to walk that would reach it. This is not unique to this plugin - see
the note under `pst.tools` below.

`save.pals()` (and so `save.pals():delete_where`) covers the world only:
pals held in a player's own dimensional storage are a separate save file,
`player_dps:<uid>`, reached only through `raw.*` or
`save.delete_dps_pals()`. Coverage of dimensional storage varies by command:

| Command | Dimensional storage |
|---|---|
| `delete_all_skins` | Not touched - only world skin fields and each player's skin-inventory record. |
| `delete_imported_pals` | Swept, via `save.delete_dps_pals()` against every player whose storage has an imported-pal slot. |
| `delete_empty_guilds` | Not applicable. |
| `delete_inactive_players` | Not applicable. |
| `fix_all_negative_timestamps` | Not touched. |
| `remove_invalid_pals_from_save` | Swept, the same way as `delete_imported_pals`. |
| `remove_invalid_items_from_save` | Not applicable - dimensional storage holds pals, not item slots. |
| `remove_invalid_passives_from_save` | Swept - each stored pal's `PassiveSkillList` is filtered in place. |
| `delete_duplicated_players` | Not touched. |
| `delete_inactive_bases` | Not applicable. |
| `delete_non_base_map_objects` | Not applicable. |
| `delete_invalid_structure_map_objects` | Not applicable. |
| `delete_unreferenced_data` | Not touched - the ownerless-pal sweep only reaches the world's `CharacterSaveParameterMap`. |

A player missing a `_dps.sav` entirely - dimensional storage is unlocked
separately from the rest of the game - is treated as empty storage rather
than an error by every command that sweeps it.

## The `pst.reset` plugin

Bundled alongside `pst.cleanup` (`psp-app/src/bundled/pst.reset/`), `pst.reset`
("World Reset") ports PalworldSaveTools' Reset family: seven commands that
each clear save state the game regenerates on next load. Every command but
one is a single `raw.delete` against one key under the level's
`worldSaveData`; the manifest declares `save.read`, `save.raw`, and
`players` - `players` because `fix_missions` addresses `player:<uid>` raw
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
  how many of the two keys were actually present and removed (0, 1, or 2) -
  not how many deletes were attempted.
- **`fix_missions` is filed under Reset despite its name.** It clears every
  player's completed-quest list so the game re-offers those missions, which is
  a reset of quest-completion state, not a repair of a broken value - hence
  its home in this family rather than Fix. The command id is `fix_missions`;
  the title shown in the UI is "Reset Missions".
- **`fix_missions` reports a count rather than writing a log file.** This port
  has no file system access from inside the sandbox, so it returns a count of
  affected players in the result instead (`counts.players`).
- Unlike the other six commands, `fix_missions` touches per-player files
  rather than the level, and does it with the two-pass collect-then-delete
  shape described in the mutation-during-iteration rule above - a `raw.delete`
  against one player's file bumps the same run-wide mutation epoch a
  `save.players()` iterator over the level relies on, so every uid has to be
  collected before any delete runs.

Bundled command ids are not guaranteed stable across releases - that is only
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
such a struct as `"opaque"`, and an opaque node has no children - no `raw`
path expression can reach inside one, no matter how it is written. That data
is not undecoded, though - only unreachable through `raw`; a command can still
read or write it through a host method bound onto a typed field or handle, the
same way `save.unlock_private_chests()` and `guild.chest_container_id` do for
the map-object lock and the guild chest id.

This is not unique to those two - the guild-tail data behind
`GroupSaveDataMap` (member online timestamps, base ownership, and more) is
opaque to `raw` for the identical reason, and any future command reaching for
it through `raw` will hit the same wall.

## The `pst.repair` plugin

Bundled at `psp-app/src/bundled/pst.repair/`, `pst.repair` ("Save Repair")
ports PalworldSaveTools' Fix family onto the plugin API, and is the plugin
the "Worked example" above quotes the view of. It has two commands:
`scan_illegal_pals`, which lists pals whose level or condensing rank is
above the legal maximum and changes nothing, and `fix_illegal_pals`, which
clamps the level and condensing rank of a selected set of pals down to that
maximum.

**This is the shape to reach for whenever a command's output is what the
next command's input should be:** a scan, a pick, then an apply, with a
`ui` view wiring the three steps together - the scan's table feeds the pick,
and the pick feeds the apply's `ids` argument. Without a view, a plugin has
no way to hand a user's row selection to a second command at all, so the
only shape left is a single `fix_all_*` command with no selection -
everything eligible gets fixed, whether the user wanted all of it touched or
not. That shape is sometimes the right one, but it is worth avoiding
whenever a selection is what the user actually wants, which is exactly the
case a scan-then-fix pair exists to serve.

**`scan_illegal_pals` checks level and condensing rank against two
thresholds** - `max_level` and `max_rank`, both parameters with defaults of
`60` and `4` - **and nothing else.** In particular, it does not check:

- **Talents.** A talent outside `0..=100` is a third kind of illegal value,
  but the host's own pal writer already refuses one out of range, so nothing
  a plugin could write would ever put a talent back in range, and no
  scan could find one to report.
- **An unknown species.** A pal whose species id no longer resolves against
  the loaded game data is a different kind of broken save state than an
  out-of-range number, and there is no legal value to clamp it down to -
  clamping only makes sense for a value with a valid range, which "which
  species this is" does not have.

## The plugin editor

The plugins page is master-detail: selecting a plugin from the list opens its
detail pane, which has a Run tab and, for a user-installed plugin only, a Code
tab that opens the editor - a bundled plugin's pane has no Code tab (see
"Bundled plugins are read-only" below). "New plugin" prompts for a name,
derives an id from it (lowercased, runs of non-alphanumeric characters
collapsed to a single `-`, leading/trailing `-` trimmed, and cut to the 64
characters a manifest id may hold, trimming a `-` the cut lands on), creates
the plugin with a one-command Lua scaffold and `log` as its only capability,
and opens its detail pane on the Code tab directly.

### Files, the manifest tab, and canonical storage

The editor shows one tab per file: `manifest.json` first, then every entry in
`sources`, alphabetically. The `manifest.json` tab is not the row's raw
stored text - it is the parsed manifest re-serialised as indented JSON for
editing, and on save it is parsed again and stored back through Rust's own
`serde_json` serialisation (compact, key order fixed by the struct
definition), not as whatever text was typed. Saving a `.lua` tab writes that
file's text into the plugin's `sources` map unchanged.

A manifest whose `id` does not match the plugin it is being saved into is
refused. The id keys the row, its storage, and its capability grant, so a
manifest declaring a different one would leave a running command's
`ctx.plugin_id` disagreeing with everything the host looks up by it.

A refused save - a manifest that will not parse, one that asks for a
capability its origin may not have, a mismatched id, or a bundled plugin -
answers under `save_plugin_source` with an `error` string rather than as a
transport-level error, and creating a plugin answers under `create_plugin`
the same way. The editor keeps the unsaved buffer and shows the message as a
toast; nothing is lost and no other tab is disturbed.

### Adding and deleting source files

The editor is not limited to editing files a plugin already has - it can add
a new `.lua` source and delete an existing one, which is how a multi-file
plugin using `require` grows past its `entry` file. A new file's path must be
relative, use forward slashes only, and end in `.lua`; the editor checks this
as the name is typed, but that check is convenience, not the boundary - the
same validation runs again on the server for every add, regardless of what
the client sent, and rejects an absolute path, a `..` or `.` segment, a
backslash, a drive-letter prefix, and similar attempts to write outside the
plugin's own source map. `manifest.json` cannot be deleted, and neither can
whichever file the manifest currently names as `entry`.

### Syntax and manifest checks

Every edit (debounced) is checked: a `.lua` tab is parsed - not run - with
`luaL_loadbufferx`, the exact same call the runtime uses to load a script
before invoking a command, so a syntax error the editor flags is a syntax
error a real run would also hit. This holds on every deployment, because
desktop, the standalone server, and the web build all dispatch
`CheckPluginSyntax` (and every other plugin message) through the same
`psp-app` handler table - desktop embeds `psp-server` directly, and `psp-web`
imports the identical `dispatch` function `psp-server` uses. The
`manifest.json` tab is checked by parsing it as a manifest under the plugin's
real origin (bundled manifests are judged more permissively than user ones),
using the same `Manifest::parse` the install and save paths use.

### Two tiers: baseline and full

The editor's checks and assists come in two tiers.

The **baseline** tier works on every deployment the editor runs on - desktop,
a self-hosted Docker deployment, and the web build alike. It is the syntax
check described above, plus the completion, hover, and signature help
described next, all generated from the host API definition and filtered by
the plugin's granted capabilities.

The **full** tier adds `lua-language-server` on top of the baseline, and only
where that binary can run: desktop and a self-hosted Docker deployment. It
adds type inference, go-to-definition, find-references, rename, and
diagnostics beyond what a syntax-only parse can report. It is not available
in the web build - a language server is a native process a browser has
nowhere to run - so the web editor never attempts it and shows a visible,
non-blocking notice explaining why, while continuing to work on the baseline
tier. See below for how the full tier is acquired and what happens when it
is not available.

### Completions, hover and signature help

Typing in a `.lua` tab offers completion, hover, and signature help for the
host API's **global tables and their members** - the globals themselves, and
what follows the dot on `save.`, `log.`, `ui.`, and the rest - generated from
the same `ApiDefinition` the Rust host builds from its own function
registrations, not maintained separately. Each entry is filtered against the
plugin's granted capabilities: a global or function gated on a capability the
plugin was not granted is left out of every list, exactly as if it did not
exist. This grant is the plugin's **stored** `granted_capabilities`, fetched
when the editor opens - editing the manifest's `capabilities` array does not
change what the editor offers until the manifest is saved and the plugin is
reopened.

Completions stop at the global tables. A value held in a variable gets none:
inside `for p in save.players() do`, typing `p.` offers nothing, because
knowing that `p` holds a player handle means inferring the type of an
expression, and the editor does no type inference. Handle types - player,
pal, guild, container - are exactly where that shows: they are fully
described in the `ApiDefinition` and rendered in the `---@meta` output
(`psp.lua`, written into the workspace alongside your sources when the full
tier starts), so `lua-language-server` reading that file does offer their
fields, but the editor's own completion lists only ever start from a global
name.

**`delete_where` is not offered as a completion, on any plugin, regardless of
capability.** It is not a gap in the capability filter - it is a gap in what
the API definition describes at all. `save.players()`, `save.pals()`, and
`save.guilds()` each return a plain Lua function value, and `delete_where` is
installed on that function's own `__index` metatable, not on any global table
or handle the generator walks. Nothing built from `ApiDefinition` - the
editor's completions included - knows it exists. This is a known gap, not a
broken install: code that calls `save.players():delete_where(...)` is correct
Lua and runs exactly as documented above; the editor simply has nothing to
suggest for it.

### The command/function agreement warning

The editor cross-checks the manifest's `commands` array against the entry
file's top-level function definitions and warns both ways: a command with no
matching function ("will fail"), and a function with no matching command
("nothing can run it"). This mirrors what the runtime actually does when a
command runs - it looks the command id up as a Lua *global* with
`lua_getglobal` - so the check only recognises a definition that lands in a
global of that exact name: `function name() ... end` at the start of a line,
or `name = function() ... end`, both counted even inside a long-bracket
string (the check is a line scan, not a parse, so it errs toward warning
rather than missing something). A `local function run()`, `function
M.run()`, or `function M:run()` binds no global the runtime's
`lua_getglobal` lookup - or this check - can see, so none of those satisfy a
command of the same name; the editor will warn "no global function of that
name" even though the code parses and the name reads correctly to a person.

### The language server: download on first use, and graceful degradation

On desktop and Docker, `lua-language-server` is downloaded on first use
rather than shipped inside the application - the editor opens on the
baseline tier immediately and upgrades to the full tier once the binary is
in place. The download is a pinned release (about 4.5 MB, platform-specific)
verified against a pinned SHA-256 digest before anything from it is
installed; a checksum that does not match leaves nothing on disk, rather
than installing an unverified binary. Once installed for a given version, it
is reused on every later launch - nothing is re-downloaded unless the pinned
version changes.

Anything that keeps the full tier from being available or running -
the platform has no pinned release, the download or verification failed, the
server has not finished starting yet, or a running server dies mid-session -
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
manifest's `capabilities` array asks for - it is always the plugin's stored
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
different code past the preview - it applies what you saw, and a fresh
preview is needed for the new text.

### Bundled plugins are read-only

The editor's Save button is disabled whenever the opened plugin is bundled,
and the save handler refuses the write server-side too, for the same reason:
every bundled plugin's manifest and sources are overwritten from the
compiled-in copy on every application startup, so an edit made through the
editor would simply vanish the next time the app launched. Only a plugin's
`enabled` flag and its granted capabilities survive that startup reseed.

The draft runner is disabled for a bundled plugin as well, and the draft
handler refuses it server-side too - this one is a privilege boundary rather
than a durability one. A draft run executes sources supplied by the caller
under the row's stored grant, and a bundled row's grant may include
`save.raw`, which exists only so that code compiled into the application can
use it. Running an edited script against that grant would hand raw save
access to code that was never shipped in the binary, so a bundled row's
draft is refused outright rather than run with a reduced grant. A bundled
plugin's detail pane has no Code tab at all, so none of this is reachable
from the plugins page's own navigation; the refusals above still hold in the
editor and its host handlers regardless. Its commands run normally from the
plugin panel's Run tab, from the sources the application shipped.

### Bundle size

Measured from `npm run build` in `ui/` (SvelteKit's static adapter, hashes
will differ on any other build):

- Total built output (`ui_build/`): 354,892,569 bytes (~338.5 MiB), of which
  the JS/CSS asset tree (`ui_build/_app/`) is 38,999,261 bytes (~37.2 MiB) -
  the rest is game-data, wiki content, and other static assets unrelated to
  this feature.
- The editor is no longer its own lazy-loaded route. Before the plugins page
  became master-detail, `/plugins/editor` was a separate route fetched only
  when visited; now `CodePane.svelte` and its helpers are imported directly
  by the plugin detail page (`/plugins/[id]`) alongside the Run tab, so they
  are part of that route's own chunk instead of a route fetched on demand.
  The byte breakdown this section used to give was measured against that old
  route and no longer describes the current layout; it would take a fresh
  build to re-measure. What that old measurement did establish, and which the
  restructuring does not change, is that Monaco itself - the largest cost by
  far - is never part of the bundle at all (see below).
- **`monaco-editor` is not a cost the plugin editor introduces** -
  `ui/src/routes/editor/+page.svelte` uses it through the same shared
  `Monaco` component, and every reference to the `monaco-editor` package
  anywhere in the codebase, including the plugin editor's, is a type-only
  import that produces no runtime code. The actual editor engine is never
  part of the Vite bundle at all: `Monaco.svelte` loads it at runtime via
  `@monaco-editor/loader`, which injects a `<script>` tag pointing at
  `https://cdn.jsdelivr.net/npm/monaco-editor@0.55.1/min/vs` - the only trace
  of "monaco-editor" anywhere in the built output is that URL string, sitting
  in the shared vendor chunk alongside unrelated code used by 35 other
  routes, not in the plugin editor's own code. So there is no "Monaco chunk"
  to measure, and the plugin editor adds no new heavy dependency to the web
  bundle.

That CDN fetch is a runtime dependency, not a build-time one: the editor's
*chrome* - Monaco itself, on either the save editor (`/editor`) or a
plugin's Code tab - will not open without network access to
`cdn.jsdelivr.net`, on every deployment, including a fully offline desktop
install; nothing in this codebase serves `monaco-editor` locally as a
fallback. This is a property of `@monaco-editor/loader`'s default
configuration
(`ui/node_modules/@monaco-editor/loader/lib/es/config/index.js:1-5`), and it
is not specific to the plugin editor - the save editor at
`ui/src/routes/editor/+page.svelte` has the same requirement through the
same `Monaco.svelte` component. It is unrelated to the syntax
check described above: that check runs Lua's own parser locally inside the
Rust host and needs no network at all, so a plugin's syntax and manifest are
still validated offline - it is specifically the editor's on-screen chrome
that needs the network, not the checks it runs against your code.
