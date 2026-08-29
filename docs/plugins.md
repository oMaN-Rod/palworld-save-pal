# Plugins

Plugins add repeatable save-management commands to Palworld Save Pal. You can
install one written by someone else or create one in the built-in editor with
Lua.

This guide focuses on getting a plugin running safely. For exact schemas,
functions, limits, and bundled-plugin behavior, use the
[Plugin API reference](plugin-api.md).

## Choose your path

| I want to... | Start here |
|---|---|
| Use a plugin | [Install and run a plugin](#install-and-run-a-plugin) |
| Understand a permission request | [Review capabilities](#review-capabilities) |
| Build my first plugin | [Create your first plugin](#create-your-first-plugin) |
| Add parameters or a custom layout | [Grow the plugin](#grow-the-plugin) |
| Look up a Lua function or manifest rule | [Plugin API reference](plugin-api.md) |
| Fix an install or runtime error | [Troubleshooting](#troubleshooting) |

## Install and run a plugin

Plugins are distributed in two forms:

- A `.lua` file is useful for a command that needs no capabilities. It must
  define `function main()`.
- A `.zip` can include a manifest, multiple Lua files, commands, parameters,
  capabilities, and a custom interface. Most useful plugins use this form.

To install and run one:

1. Open **Plugins** and choose **Install**.
2. Select the `.lua` or `.zip` file.
3. Review the requested capabilities. Grant only what the plugin needs.
4. Load a save and select the plugin.
5. Choose a command, fill in its inputs, and run it.
6. Review the preview before applying a destructive command.

Back up an important save before trying an unfamiliar plugin. A plugin with
`save.write` can change save data; raw access is more powerful and bypasses the
typed API's validation.

### What is inside a plugin?

A typical plugin archive contains:

```text
manifest.json   # identity, commands, permissions, and optional layout
main.lua        # command implementation
```

The manifest is the contract between the plugin and the host. Each command id
must match a top-level Lua function of the same name.

## Review capabilities

Capabilities are permissions. A plugin can use a feature only when it declares
the capability and the user grants it.

| Capability | Allows |
|---|---|
| `save.read` | Read players, pals, guilds, bases, containers, and related save data. |
| `save.write` | Change save data. It also requires `save.read`. |
| `players` | Read player-scoped data not available from the normal save summary. |
| `gamedata` | Look up Palworld catalogs and validate ids. |
| `storage` | Store small values between runs for this plugin. |
| `ui.dialog` | Ask the user for confirmation. |
| `log` | Write status and diagnostic messages. |
| `save.raw` | Access untyped save paths. Reserved for bundled plugins in API v1. |

Use the smallest set that supports the plugin. Prefer the typed `save` API over
raw access: typed writes are validated, while an incorrect raw write can create
a save the game cannot load.

See [Capabilities](plugin-api.md#capabilities) for the complete enforcement
rules and [the host API](plugin-api.md#the-host-api) for the functions each
capability enables.

## Create your first plugin

The built-in editor supplies a working scaffold, syntax checks, completions,
and a draft runner.

1. Open **Plugins** and choose **New plugin**.
2. Enter **Hello Plugin**. The app creates the id `hello-plugin`, plus
   `manifest.json` and `main.lua`.
3. Replace the generated manifest and Lua with the example below.
4. Save both files.
5. Open **Run**, choose **Hello**, and run it.

`manifest.json`:

```json
{
  "id": "hello-plugin",
  "api_version": 1,
  "name": "Hello Plugin",
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

`main.lua`:

```lua
function hello()
  log.info("Hello from the plugin")
end
```

Three details make this work:

- `entry` points to the Lua file that the host loads.
- The command id `hello` matches the global function `hello`.
- The manifest requests `log`, which installs the `log` API for the run.

The editor can run unsaved changes as a draft. Drafts still use the plugin's
stored capability grant, so adding a capability to an unsaved manifest does not
silently grant it.

## Grow the plugin

Add one feature at a time. This keeps failures easy to locate and permission
requests easy to explain.

### 1. Add an input

Declare the parameter on the command:

```json
{
  "id": "hello",
  "title": "Hello",
  "params": [
    {
      "id": "name",
      "type": "string",
      "label": "Name",
      "default": "Pal Tamer"
    }
  ]
}
```

Read the validated value from `ctx.args`:

```lua
function hello()
  log.info("Hello, " .. ctx.args.name .. "!")
end
```

The generated command form is often all a plugin needs. Parameters support
integers, floats, strings, booleans, enums, entities, and multiple selections;
see [Param objects](plugin-api.md#param-objects).

### 2. Read save data

Add `save.read` to `capabilities`, then use the typed API:

```lua
function list_players()
  for player in save.players() do
    log.info(player.name .. " (uid: " .. tostring(player.uid) .. ")")
  end
end
```

Declare `list_players` as a command before running it. The manifest and Lua
must always agree.

### 3. Write save data safely

Add both `save.read` and `save.write`, mark commands that change data as
`destructive`, and use typed handle fields or methods. Destructive commands run
as a preview first; the user explicitly applies the projected changes.

```json
{
  "id": "apply_fix",
  "title": "Apply fix",
  "description": "Applies the selected fix to the loaded save.",
  "destructive": true,
  "params": []
}
```

Return useful counts from mutating commands so the preview explains what will
change. See the [worked cleanup example](plugin-api.md#worked-example-delete_empty_guilds)
for a complete dry-run-aware command.

### 4. Add a custom interface only when it helps

The host automatically creates a form from commands and parameters. Add a
`ui` section when the workflow benefits from structure, such as:

```text
scan → review results → select rows → preview changes → apply
```

Plugin interfaces are JSON data, not HTML or JavaScript. The host owns command
validation, destructive previews, and rendering. Start with the
[interface overview](plugin-api.md#plugin-defined-interfaces), then use the
[`pst.repair` view](plugin-api.md#worked-example-pstrepairs-view) as a complete
scan-and-fix example.

### 5. Split larger plugins into files

Keep `main.lua` as the entry point and load helpers with `require`. Module paths
must remain inside the plugin and use forward slashes. See
[Multi-file plugins and `require`](plugin-api.md#multi-file-plugins-and-require)
for resolution rules and examples.

## A practical development loop

1. Start with one command and the fewest capabilities possible.
2. Run it against a disposable or backed-up save.
3. Use `log` and returned counts to make behavior observable.
4. Add parameters before adding a custom interface.
5. Mark every save-changing command as destructive.
6. Test both the preview and the applied run.
7. Export the plugin to `.zip` when it is ready to share.

When adapting a bundled plugin, clone it first. Bundled plugins are read-only;
the clone is an editable user plugin with its own id and capability grant.

## Troubleshooting

### The plugin will not install

Check that:

- The archive root contains `manifest.json`.
- `entry` names an included `.lua` file.
- The manifest uses `api_version: 1`.
- The plugin id is lowercase and uses only supported separators.
- Command and parameter ids are valid Lua identifiers.
- A user plugin does not request `save.raw`.

The exact validation rules are in [Manifest schema](plugin-api.md#manifest-schema).

### A command is missing or fails immediately

The command id must match a top-level global function exactly. For a command
named `repair`, define `function repair() ... end`; a local function or
`function M.repair()` will not satisfy the runtime lookup.

### An API is `nil`

The associated capability must be both declared in the manifest and granted
to the installed plugin. Save the manifest, reopen the plugin, and review its
grants. Editor completions also reflect the stored grant rather than unsaved
capability changes.

### A destructive command only shows a preview

That is expected. Review the predicted counts, then choose **Apply**. The host
runs the previewed draft again when applying it.

### A large plugin times out

Report progress in long loops and avoid changing a collection while iterating
over it. Bulk pal updates also need their reads collected before the first
write. See [Sandbox limits](plugin-api.md#sandbox-limits-and-terminating-statuses),
[mutation during iteration](plugin-api.md#the-mutation-during-iteration-rule),
and [bulk pal loops](plugin-api.md#ordering-reads-and-writes-in-a-bulk-pal-loop).

## Reference map

Use the detailed reference when you need exact behavior:

- [Manifest, commands, and parameters](plugin-api.md#manifest-schema)
- [Plugin-defined interfaces](plugin-api.md#plugin-defined-interfaces)
- [Capabilities](plugin-api.md#capabilities)
- [Host API globals and handles](plugin-api.md#the-host-api)
- [Runtime-generated API definition](plugin-api.md#the-api-definition)
- [Sandbox limits and statuses](plugin-api.md#sandbox-limits-and-terminating-statuses)
- [Bundled plugin behavior](plugin-api.md#the-pstcleanup-plugin)
- [Plugin editor](plugin-api.md#the-plugin-editor)
