mod support;

use std::collections::{BTreeMap, BTreeSet};

use psp_plugin::host::api_def::{api_definition, ApiField, ApiType};
use psp_plugin::manifest::Capability;
use psp_plugin::Access;
use psp_plugin::status::RunStatus;

/// Unlike `install_globals_body`'s match arms, this list has no compiler-enforced tie to `Capability`; a new variant not added here silently drops out of this test's coverage.
const ALL_CAPABILITIES: &[Capability] = &[
    Capability::SaveRead,
    Capability::SaveWrite,
    Capability::SaveRaw,
    Capability::Players,
    Capability::GameData,
    Capability::UiDialog,
    Capability::Storage,
    Capability::Log,
];

fn all_capabilities_harness() -> support::Harness {
    support::harness(ALL_CAPABILITIES)
}

fn read_only_harness() -> support::Harness {
    support::harness(&[Capability::SaveRead, Capability::GameData])
}

/// Every player row that is not answered from the session's own summary is
/// gated on `players`, so probing the whole table needs it granted.
fn player_read_harness() -> support::Harness {
    support::harness(&[Capability::SaveRead, Capability::GameData, Capability::Players])
}

/// Catches a function registered outside the shared consts: `save_write` extends `save_read`'s table and `install_ctx` sets fields directly, so neither goes through `register_table`.
#[test]
fn every_global_lua_can_see_is_described_and_every_described_global_exists() {
    let mut h = all_capabilities_harness();
    let (status, value) = h.run(
        "local seen = {}
         for _, name in ipairs({'save','raw','gamedata','progress','ctx','log','storage','ui'}) do
           local t = _G[name]
           if type(t) == 'table' then
             for key, v in pairs(t) do
               seen[#seen+1] = name .. '.' .. key
             end
           end
         end
         table.sort(seen)
         return table.concat(seen, ',')",
    );
    assert_eq!(status, RunStatus::Ok);

    let live: BTreeSet<String> = value
        .expect("a string")
        .split(',')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();

    let described: BTreeSet<String> = api_definition()
        .globals
        .iter()
        .flat_map(|g| {
            let functions = g.functions.iter().map(move |f| format!("{}.{}", g.name, f.name));
            let fields = g.fields.iter().map(move |f| format!("{}.{}", g.name, f.name));
            functions.chain(fields)
        })
        .collect();

    let undescribed: Vec<&String> = live.difference(&described).collect();
    let phantom: Vec<&String> = described.difference(&live).collect();

    assert!(
        undescribed.is_empty(),
        "these exist in Lua but are not described (an author gets no completion for them): {undescribed:?}"
    );
    assert!(
        phantom.is_empty(),
        "these are described but do not exist in Lua (an author would be offered code that fails): {phantom:?}"
    );
}

#[test]
fn a_denied_capability_installs_none_of_its_functions() {
    let mut h = read_only_harness();
    let (status, value) = h.run("return tostring(save.unlock_private_chests) .. ',' .. tostring(raw)");
    assert_eq!(status, RunStatus::Ok);
    let value = value.expect("a string");
    let parts: Vec<&str> = value.split(',').collect();
    assert_eq!(parts[0], "nil", "a SaveWrite function must be absent: {value}");
    assert_eq!(parts[1], "nil", "the raw table must be absent: {value}");
}

/// Hand-written per handle type: the API definition describes what a handle exposes, not how a script first comes to hold one, so this can't be derived from it.
fn acquire_snippet(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "player" => "for h in save.players() do H = h break end",
        "pal" => "for h in save.pals() do H = h break end",
        "guild" => "for h in save.guilds() do H = h break end",
        "base" => "for h in save.bases() do H = h break end",
        "container" => "for h in save.containers() do H = h break end",
        "map_object" => "for h in save.map_objects() do H = h break end",
        "slot" => {
            "for c in save.containers() do \
               for s in c.slots() do H = s break end \
               if H then break end \
             end"
        }
        _ => return None,
    })
}

/// Lua's own `type()` calls every number `"number"`, which would let a field
/// declared `Integer` hand back a float unnoticed. Probe scripts report
/// `math.type()` for numbers instead, so these are its names: an `Integer`
/// admits only an integer, while a `Number` admits either, matching how LuaLS
/// treats `integer` as a subtype of `number`.
const REFINED_TYPE_HELPER: &str = "local function refined_type(v)\n\
                                   \x20 if type(v) == 'number' then return math.type(v) end\n\
                                   \x20 return type(v)\n\
                                   end\n";

fn allowed_lua_types(ty: &ApiType) -> Vec<&'static str> {
    match ty {
        ApiType::Nil => vec!["nil"],
        ApiType::Boolean => vec!["boolean"],
        ApiType::Integer => vec!["integer"],
        ApiType::Number => vec!["integer", "float"],
        ApiType::String => vec!["string"],
        ApiType::Table | ApiType::List(_) | ApiType::Map { .. } => vec!["table"],
        ApiType::Handle(_) => vec!["userdata"],
        ApiType::Iterator(_) => vec!["function"],
        ApiType::Any => {
            vec!["nil", "boolean", "integer", "float", "string", "table", "function", "userdata", "thread"]
        }
        ApiType::Union(members) => members.iter().flat_map(allowed_lua_types).collect(),
        ApiType::Multi(_) => panic!("Multi describes a function's returns, never a field or parameter"),
    }
}

fn field_allows_nil(ty: &ApiType) -> bool {
    allowed_lua_types(ty).contains(&"nil")
}

/// Unlike the global test above: a handle's fields are dispatched through an `__index` function (`host/handle.rs::install_metatables`), not a table, with no `__pairs`, so Lua cannot enumerate a handle's real field set from the outside, and an unmatched field name also resolves to plain `nil` (see `host_read.rs::an_unknown_field_reads_as_nil_rather_than_erroring`). So this can only prove that described non-nil fields are in fact non-nil on a live instance -- it cannot prove the reverse, that nothing Lua exposes is missing from the description, for a nilable field.
#[test]
fn every_described_handle_field_or_method_resolves_correctly_on_a_live_instance() {
    let def = api_definition();

    let mut script = String::from(REFINED_TYPE_HELPER);
    script.push_str("local results = {}\n");
    for handle in &def.handles {
        let acquire = acquire_snippet(handle.name).unwrap_or_else(|| {
            panic!(
                "no fixture acquisition strategy is known for handle type \"{}\"; add one to \
                 acquire_snippet",
                handle.name
            )
        });
        script.push_str("do\n  local H\n  ");
        script.push_str(acquire);
        script.push_str("\n  local out = {}\n");
        script.push_str("  if H == nil then\n    out[#out+1] = 'NOHANDLE'\n  else\n");
        script.push_str("    out[#out+1] = 'FOUND'\n");
        for field in handle.fields {
            script.push_str(&format!("    out[#out+1] = '{n}=' .. refined_type(H['{n}'])\n", n = field.name));
        }
        for method in handle.methods {
            script.push_str(&format!("    out[#out+1] = '{n}=' .. refined_type(H['{n}'])\n", n = method.name));
        }
        script.push_str("  end\n");
        script.push_str(&format!("  results[#results+1] = '{n}:' .. table.concat(out, ',')\n", n = handle.name));
        script.push_str("end\n");
    }
    script.push_str("return table.concat(results, ';')");

    let mut h = all_capabilities_harness();
    let (status, value) = h.run(&script);
    assert_eq!(status, RunStatus::Ok, "the probe script must run cleanly: {value:?}");
    let value = value.expect("a string");

    let mut no_reachable_instance: Vec<String> = Vec::new();

    for section in value.split(';') {
        let (kind, rest) = section.split_once(':').expect("kind:payload");
        let handle = def.handles.iter().find(|h| h.name == kind).expect("a described handle");

        let mut parts = rest.split(',');
        let found = parts.next().expect("at least a status entry");
        if found == "NOHANDLE" {
            no_reachable_instance.push(kind.to_string());
            continue;
        }
        assert_eq!(found, "FOUND", "unexpected probe status for {kind}: {found}");

        let seen: BTreeMap<&str, &str> = parts
            .map(|entry| entry.split_once('=').unwrap_or_else(|| panic!("expected name=type, got {entry}")))
            .collect();

        for field in handle.fields {
            let lua_type = *seen.get(field.name).unwrap_or_else(|| {
                panic!(
                    "{kind}.{} was not probed by the generated script -- the generator and this \
                     assertion loop drifted",
                    field.name
                )
            });
            let allowed = allowed_lua_types(&field.ty);
            assert!(
                allowed.contains(&lua_type),
                "{kind}.{} is described as {:?} but Lua resolved it as {lua_type}",
                field.name,
                field.ty
            );
            if !field_allows_nil(&field.ty) {
                assert_ne!(
                    lua_type, "nil",
                    "{kind}.{} is described as never nil but resolved to nil on a live instance \
                     -- this may be a phantom field the description invented",
                    field.name
                );
            }
        }

        for method in handle.methods {
            let lua_type = *seen.get(method.name).unwrap_or_else(|| {
                panic!(
                    "{kind}.{} was not probed by the generated script -- the generator and this \
                     assertion loop drifted",
                    method.name
                )
            });
            assert_eq!(
                lua_type, "function",
                "{kind}.{} is described as a method but Lua resolved it as {lua_type}, not a \
                 function -- with every capability granted this must always be callable",
                method.name
            );
        }
    }

    assert!(
        no_reachable_instance.is_empty(),
        "the fixture has no reachable instance of these described handle types, so their \
         fields could not be probed at all: {no_reachable_instance:?}"
    );
}

#[test]
fn every_pal_field_row_appears_in_the_api_definition() {
    let definition = api_definition();
    let handle = definition
        .handles
        .iter()
        .find(|h| h.name == "pal")
        .expect("the pal handle must be described");

    for spec in psp_plugin::PAL_FIELDS {
        let described = handle.fields.iter().find(|f| f.name == spec.name);
        assert!(described.is_some(), "{} is in the table but not the definition", spec.name);
        let described = described.expect("checked above");
        assert_eq!(described.ty, spec.ty, "{} disagrees on type", spec.name);
        assert_eq!(described.access, spec.access, "{} disagrees on access", spec.name);
    }
}

#[test]
fn every_described_pal_field_exists_in_the_table() {
    let definition = api_definition();
    let handle = definition.handles.iter().find(|h| h.name == "pal").expect("pal");

    for field in handle.fields {
        assert!(
            psp_plugin::PAL_FIELDS.iter().any(|s| s.name == field.name),
            "{} is described but has no table row",
            field.name
        );
    }
}

#[test]
fn every_player_field_row_appears_in_the_api_definition() {
    let definition = api_definition();
    let handle = definition
        .handles
        .iter()
        .find(|h| h.name == "player")
        .expect("the player handle must be described");

    for spec in psp_plugin::PLAYER_FIELDS {
        let described = handle.fields.iter().find(|f| f.name == spec.name);
        assert!(described.is_some(), "{} is in the table but not the definition", spec.name);
        let described = described.expect("checked above");
        assert_eq!(described.ty, spec.ty, "{} disagrees on type", spec.name);
        assert_eq!(described.access, spec.access, "{} disagrees on access", spec.name);
    }
}

#[test]
fn every_described_player_field_exists_in_the_table() {
    let definition = api_definition();
    let handle = definition.handles.iter().find(|h| h.name == "player").expect("player");

    for field in handle.fields {
        assert!(
            psp_plugin::PLAYER_FIELDS.iter().any(|s| s.name == field.name),
            "{} is described but has no table row",
            field.name
        );
    }
}

/// The player counterpart of the pal read-back test below, and load-bearing for
/// the same reason: `player_index` answers seven rows from a hand-written
/// `player_field` arm that short-circuits before the table is consulted, so a
/// row's declared type and the value Lua actually receives can disagree with
/// nothing objecting. It also proves every row the summary does not carry is
/// reachable at all -- each of those costs a lazy load of the player's own
/// `.sav`, and a row that failed to load would read nil here.
#[test]
fn every_player_field_row_reads_back_at_its_declared_type() {
    let mut script = String::from(REFINED_TYPE_HELPER);
    script.push_str(
        "local target\n\
         for p in save.players() do target = p break end\n\
         assert(target ~= nil, 'the fixture must hold a player')\n\
         local out = {}\n",
    );
    for spec in psp_plugin::PLAYER_FIELDS {
        script.push_str(&format!("out[#out+1] = '{n}=' .. refined_type(target['{n}'])\n", n = spec.name));
    }
    script.push_str("return table.concat(out, ',')");

    let mut h = player_read_harness();
    let (status, value) = h.run(&script);
    assert_eq!(status, RunStatus::Ok, "the probe script must run cleanly: {value:?}");
    let value = value.expect("a string");

    let seen: BTreeMap<&str, &str> = value
        .split(',')
        .map(|entry| entry.split_once('=').unwrap_or_else(|| panic!("expected name=type, got {entry}")))
        .collect();

    let mut read_as_nil: Vec<&str> = Vec::new();
    for spec in psp_plugin::PLAYER_FIELDS {
        let lua_type = *seen.get(spec.name).unwrap_or_else(|| {
            panic!("{} was not probed -- the generator and this loop drifted", spec.name)
        });
        assert!(
            allowed_lua_types(&spec.ty).contains(&lua_type),
            "player.{} is declared {:?} but Lua resolved it as {lua_type}",
            spec.name,
            spec.ty
        );
        if lua_type == "nil" {
            read_as_nil.push(spec.name);
        }
    }

    assert_eq!(psp_plugin::PLAYER_FIELDS.len(), seen.len(), "every row must be probed exactly once");
    assert_eq!(
        read_as_nil,
        Vec::<&str>::new(),
        "every row has a value on the fixture player, including the nilable ones -- a row \
         reading nil here needs explaining, not exempting"
    );
}

/// The two agreement tests above are structural once the description is
/// generated from the table: they compare the table to itself. This one is
/// not. `pal_index` answers most rows from a hand-written `pal_field` arm that
/// short-circuits before the table is ever consulted, so a row's declared type
/// and the value Lua actually receives can disagree with nothing objecting.
#[test]
fn every_pal_field_row_reads_back_at_its_declared_type() {
    let mut script = String::from(REFINED_TYPE_HELPER);
    script.push_str(
        "local target\n\
         for p in save.pals() do target = p break end\n\
         assert(target ~= nil, 'the fixture must hold a pal')\n\
         local out = {}\n",
    );
    for spec in psp_plugin::PAL_FIELDS {
        script.push_str(&format!("out[#out+1] = '{n}=' .. refined_type(target['{n}'])\n", n = spec.name));
    }
    script.push_str("return table.concat(out, ',')");

    let mut h = read_only_harness();
    let (status, value) = h.run(&script);
    assert_eq!(status, RunStatus::Ok, "the probe script must run cleanly: {value:?}");
    let value = value.expect("a string");

    let seen: BTreeMap<&str, &str> = value
        .split(',')
        .map(|entry| entry.split_once('=').unwrap_or_else(|| panic!("expected name=type, got {entry}")))
        .collect();

    let mut read_as_nil: Vec<&str> = Vec::new();
    for spec in psp_plugin::PAL_FIELDS {
        let lua_type = *seen.get(spec.name).unwrap_or_else(|| {
            panic!("{} was not probed -- the generator and this loop drifted", spec.name)
        });
        assert!(
            allowed_lua_types(&spec.ty).contains(&lua_type),
            "pal.{} is declared {:?} but Lua resolved it as {lua_type}",
            spec.name,
            spec.ty
        );
        if lua_type == "nil" {
            read_as_nil.push(spec.name);
        }
    }

    assert_eq!(
        psp_plugin::PAL_FIELDS.len(),
        seen.len(),
        "every row must be probed exactly once"
    );
    assert_eq!(
        read_as_nil,
        ["nickname", "guild_id", "base_id"],
        "only rows genuinely absent on the fixture pal may read nil, and each of those must \
         admit nil in its declared type"
    );
}

#[test]
fn every_guild_field_row_appears_in_the_api_definition() {
    let definition = api_definition();
    let handle = definition
        .handles
        .iter()
        .find(|h| h.name == "guild")
        .expect("the guild handle must be described");

    for spec in psp_plugin::GUILD_FIELDS {
        let described = handle.fields.iter().find(|f| f.name == spec.name);
        assert!(described.is_some(), "{} is in the table but not the definition", spec.name);
        let described = described.expect("checked above");
        assert_eq!(described.ty, spec.ty, "{} disagrees on type", spec.name);
        assert_eq!(described.access, spec.access, "{} disagrees on access", spec.name);
    }
}

#[test]
fn every_described_guild_field_exists_in_the_table() {
    let definition = api_definition();
    let handle = definition.handles.iter().find(|h| h.name == "guild").expect("guild");

    for field in handle.fields {
        assert!(
            psp_plugin::GUILD_FIELDS.iter().any(|s| s.name == field.name),
            "{} is described but has no table row",
            field.name
        );
    }
}

#[test]
fn every_base_field_row_appears_in_the_api_definition() {
    let definition = api_definition();
    let handle = definition
        .handles
        .iter()
        .find(|h| h.name == "base")
        .expect("the base handle must be described");

    for spec in psp_plugin::BASE_FIELDS {
        let described = handle.fields.iter().find(|f| f.name == spec.name);
        assert!(described.is_some(), "{} is in the table but not the definition", spec.name);
        let described = described.expect("checked above");
        assert_eq!(described.ty, spec.ty, "{} disagrees on type", spec.name);
        assert_eq!(described.access, spec.access, "{} disagrees on access", spec.name);
    }
}

#[test]
fn every_described_base_field_exists_in_the_table() {
    let definition = api_definition();
    let handle = definition.handles.iter().find(|h| h.name == "base").expect("base");

    for field in handle.fields {
        assert!(
            psp_plugin::BASE_FIELDS.iter().any(|s| s.name == field.name),
            "{} is described but has no table row",
            field.name
        );
    }
}

/// The guild counterpart of the pal and player read-back tests, and load-bearing
/// for the same reason: the two agreement tests above compare the table to
/// itself, while this one compares it to what Lua actually receives. Three of
/// this handle's rows are answered from the cached `GuildDto` and five from the
/// session's guild summary, so a row's declared type and its value can disagree
/// with nothing objecting.
#[test]
fn every_guild_field_row_reads_back_at_its_declared_type() {
    let mut script = String::from(REFINED_TYPE_HELPER);
    script.push_str(
        "local target\n\
         for g in save.guilds() do target = g break end\n\
         assert(target ~= nil, 'the fixture must hold a guild')\n\
         local out = {}\n",
    );
    for spec in psp_plugin::GUILD_FIELDS {
        script.push_str(&format!("out[#out+1] = '{n}=' .. refined_type(target['{n}'])\n", n = spec.name));
    }
    script.push_str("return table.concat(out, ',')");

    let mut h = read_only_harness();
    let (status, value) = h.run(&script);
    assert_eq!(status, RunStatus::Ok, "the probe script must run cleanly: {value:?}");
    let value = value.expect("a string");

    let seen: BTreeMap<&str, &str> = value
        .split(',')
        .map(|entry| entry.split_once('=').unwrap_or_else(|| panic!("expected name=type, got {entry}")))
        .collect();

    let mut read_as_nil: Vec<&str> = Vec::new();
    for spec in psp_plugin::GUILD_FIELDS {
        let lua_type = *seen.get(spec.name).unwrap_or_else(|| {
            panic!("{} was not probed -- the generator and this loop drifted", spec.name)
        });
        assert!(
            allowed_lua_types(&spec.ty).contains(&lua_type),
            "guild.{} is declared {:?} but Lua resolved it as {lua_type}",
            spec.name,
            spec.ty
        );
        if lua_type == "nil" {
            read_as_nil.push(spec.name);
        }
    }

    assert_eq!(psp_plugin::GUILD_FIELDS.len(), seen.len(), "every row must be probed exactly once");
    assert_eq!(
        read_as_nil,
        Vec::<&str>::new(),
        "every row has a value on the fixture guild, including the nilable ones -- a row \
         reading nil here needs explaining, not exempting"
    );
}

/// The base counterpart. Two of this handle's rows are answered from the
/// cached `BaseDto` and five straight off the base's `BaseCampSaveData` entry,
/// which is exactly the split a declared type can drift across.
#[test]
fn every_base_field_row_reads_back_at_its_declared_type() {
    let mut script = String::from(REFINED_TYPE_HELPER);
    script.push_str(
        "local target\n\
         for b in save.bases() do target = b break end\n\
         assert(target ~= nil, 'the fixture must hold a base')\n\
         local out = {}\n",
    );
    for spec in psp_plugin::BASE_FIELDS {
        script.push_str(&format!("out[#out+1] = '{n}=' .. refined_type(target['{n}'])\n", n = spec.name));
    }
    script.push_str("return table.concat(out, ',')");

    let mut h = read_only_harness();
    let (status, value) = h.run(&script);
    assert_eq!(status, RunStatus::Ok, "the probe script must run cleanly: {value:?}");
    let value = value.expect("a string");

    let seen: BTreeMap<&str, &str> = value
        .split(',')
        .map(|entry| entry.split_once('=').unwrap_or_else(|| panic!("expected name=type, got {entry}")))
        .collect();

    let mut read_as_nil: Vec<&str> = Vec::new();
    for spec in psp_plugin::BASE_FIELDS {
        let lua_type = *seen.get(spec.name).unwrap_or_else(|| {
            panic!("{} was not probed -- the generator and this loop drifted", spec.name)
        });
        assert!(
            allowed_lua_types(&spec.ty).contains(&lua_type),
            "base.{} is declared {:?} but Lua resolved it as {lua_type}",
            spec.name,
            spec.ty
        );
        if lua_type == "nil" {
            read_as_nil.push(spec.name);
        }
    }

    assert_eq!(psp_plugin::BASE_FIELDS.len(), seen.len(), "every row must be probed exactly once");
    assert_eq!(
        read_as_nil,
        Vec::<&str>::new(),
        "every row has a value on the fixture base, including the nilable ones -- a row \
         reading nil here needs explaining, not exempting"
    );
}

#[test]
fn every_container_field_row_appears_in_the_api_definition() {
    let definition = api_definition();
    let handle = definition
        .handles
        .iter()
        .find(|h| h.name == "container")
        .expect("the container handle must be described");

    for spec in psp_plugin::CONTAINER_FIELDS {
        let described = handle.fields.iter().find(|f| f.name == spec.name);
        assert!(described.is_some(), "{} is in the table but not the definition", spec.name);
        let described = described.expect("checked above");
        assert_eq!(described.ty, spec.ty, "{} disagrees on type", spec.name);
        assert_eq!(described.access, spec.access, "{} disagrees on access", spec.name);
    }
}

#[test]
fn every_described_container_field_exists_in_the_table() {
    let definition = api_definition();
    let handle = definition.handles.iter().find(|h| h.name == "container").expect("container");

    for field in handle.fields {
        assert!(
            psp_plugin::CONTAINER_FIELDS.iter().any(|s| s.name == field.name),
            "{} is described but has no table row",
            field.name
        );
    }
}

#[test]
fn every_slot_field_row_appears_in_the_api_definition() {
    let definition = api_definition();
    let handle = definition
        .handles
        .iter()
        .find(|h| h.name == "slot")
        .expect("the slot handle must be described");

    for spec in psp_plugin::SLOT_FIELDS {
        let described = handle.fields.iter().find(|f| f.name == spec.name);
        assert!(described.is_some(), "{} is in the table but not the definition", spec.name);
        let described = described.expect("checked above");
        assert_eq!(described.ty, spec.ty, "{} disagrees on type", spec.name);
        assert_eq!(described.access, spec.access, "{} disagrees on access", spec.name);
    }
}

#[test]
fn every_described_slot_field_exists_in_the_table() {
    let definition = api_definition();
    let handle = definition.handles.iter().find(|h| h.name == "slot").expect("slot");

    for field in handle.fields {
        assert!(
            psp_plugin::SLOT_FIELDS.iter().any(|s| s.name == field.name),
            "{} is described but has no table row",
            field.name
        );
    }
}

/// The container counterpart of the four read-back tests above. Its two rows
/// are answered from different places -- one off the handle, one off the
/// container the save holds -- which is exactly the split a declared type can
/// drift across.
#[test]
fn every_container_field_row_reads_back_at_its_declared_type() {
    let mut script = String::from(REFINED_TYPE_HELPER);
    script.push_str(
        "local target\n\
         for c in save.containers() do target = c break end\n\
         assert(target ~= nil, 'the fixture must hold a container')\n\
         local out = {}\n",
    );
    for spec in psp_plugin::CONTAINER_FIELDS {
        script.push_str(&format!("out[#out+1] = '{n}=' .. refined_type(target['{n}'])\n", n = spec.name));
    }
    script.push_str("return table.concat(out, ',')");

    let mut h = read_only_harness();
    let (status, value) = h.run(&script);
    assert_eq!(status, RunStatus::Ok, "the probe script must run cleanly: {value:?}");
    let value = value.expect("a string");

    let seen: BTreeMap<&str, &str> = value
        .split(',')
        .map(|entry| entry.split_once('=').unwrap_or_else(|| panic!("expected name=type, got {entry}")))
        .collect();

    let mut read_as_nil: Vec<&str> = Vec::new();
    for spec in psp_plugin::CONTAINER_FIELDS {
        let lua_type = *seen.get(spec.name).unwrap_or_else(|| {
            panic!("{} was not probed -- the generator and this loop drifted", spec.name)
        });
        assert!(
            allowed_lua_types(&spec.ty).contains(&lua_type),
            "container.{} is declared {:?} but Lua resolved it as {lua_type}",
            spec.name,
            spec.ty
        );
        if lua_type == "nil" {
            read_as_nil.push(spec.name);
        }
    }

    assert_eq!(psp_plugin::CONTAINER_FIELDS.len(), seen.len(), "every row must be probed exactly once");
    assert_eq!(
        read_as_nil,
        Vec::<&str>::new(),
        "every row has a value on the fixture container, including the nilable one -- a row \
         reading nil here needs explaining, not exempting"
    );
}

/// The slot counterpart. Every row is answered off the slot the save holds, and
/// the fixture's first slot is occupied, so none of them may read nil.
#[test]
fn every_slot_field_row_reads_back_at_its_declared_type() {
    let mut script = String::from(REFINED_TYPE_HELPER);
    script.push_str(
        "local target\n\
         for c in save.containers() do\n\
         \x20 for s in c.slots() do if s.item_id ~= nil then target = s break end end\n\
         \x20 if target then break end\n\
         end\n\
         assert(target ~= nil, 'the fixture must hold an occupied slot')\n\
         local out = {}\n",
    );
    for spec in psp_plugin::SLOT_FIELDS {
        script.push_str(&format!("out[#out+1] = '{n}=' .. refined_type(target['{n}'])\n", n = spec.name));
    }
    script.push_str("return table.concat(out, ',')");

    let mut h = read_only_harness();
    let (status, value) = h.run(&script);
    assert_eq!(status, RunStatus::Ok, "the probe script must run cleanly: {value:?}");
    let value = value.expect("a string");

    let seen: BTreeMap<&str, &str> = value
        .split(',')
        .map(|entry| entry.split_once('=').unwrap_or_else(|| panic!("expected name=type, got {entry}")))
        .collect();

    let mut read_as_nil: Vec<&str> = Vec::new();
    for spec in psp_plugin::SLOT_FIELDS {
        let lua_type = *seen.get(spec.name).unwrap_or_else(|| {
            panic!("{} was not probed -- the generator and this loop drifted", spec.name)
        });
        assert!(
            allowed_lua_types(&spec.ty).contains(&lua_type),
            "slot.{} is declared {:?} but Lua resolved it as {lua_type}",
            spec.name,
            spec.ty
        );
        if lua_type == "nil" {
            read_as_nil.push(spec.name);
        }
    }

    assert_eq!(psp_plugin::SLOT_FIELDS.len(), seen.len(), "every row must be probed exactly once");
    assert_eq!(
        read_as_nil,
        Vec::<&str>::new(),
        "every row has a value on an occupied fixture slot, including the nilable one -- a row \
         reading nil here needs explaining, not exempting"
    );
}

/// Whether `message` names `field` as a word of its own, rather than merely
/// containing its letters. A plain substring test is vacuous for a short name:
/// `exp` sits inside "expected", which every type refusal already says, so
/// `player.exp` would pass no matter what name its validator quoted.
fn names_the_field(message: &str, field: &str) -> bool {
    let boundary = |byte: Option<u8>| !byte.is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_');
    message.match_indices(field).any(|(at, _)| {
        let bytes = message.as_bytes();
        boundary(at.checked_sub(1).map(|before| bytes[before]))
            && boundary(bytes.get(at + field.len()).copied())
    })
}

/// A value whose Lua type the row's declared type does not admit, as a Lua
/// literal. `None` when the row admits all four probes -- a type wide enough
/// that nothing is wrong-typed for it, which is a row this probe cannot reach.
fn wrong_typed_literal(ty: &ApiType) -> Option<&'static str> {
    let allowed = allowed_lua_types(ty);
    [("true", "boolean"), ("'psp_probe'", "string"), ("1.5", "float"), ("{}", "table")]
        .into_iter()
        .find(|(_, lua_type)| !allowed.contains(lua_type))
        .map(|(literal, _)| literal)
}

/// A value the row's declared type *does* admit but that little in a save
/// plausibly holds: past the end of a range, outside a catalog, not a key any
/// stat map carries. Derived from the declared type, with no per-row knowledge
/// -- which is what makes a row added later covered without anyone remembering.
///
/// A row that accepts it is not a failure. This exists to reach the refusals a
/// wrong type never gets to: ranges, domains, key sets, catalogs, and
/// `slot.count`'s structural one.
fn implausible_literal(ty: &ApiType) -> Option<&'static str> {
    Some(match ty {
        ApiType::Integer => "-987654321",
        ApiType::Number => "1e40",
        ApiType::String => "'psp_implausible_value'",
        ApiType::Boolean => "false",
        ApiType::List(_) => "{ 'psp_implausible_value' }",
        ApiType::Map { .. } => "{ psp_implausible_key = -987654321 }",
        ApiType::Table => "{ psp_implausible_key = -987654321 }",
        ApiType::Union(members) => {
            return members.iter().find(|member| !matches!(member, ApiType::Nil)).and_then(implausible_literal)
        }
        ApiType::Nil | ApiType::Handle(_) | ApiType::Iterator(_) | ApiType::Multi(_) | ApiType::Any => return None,
    })
}

/// Builds one script that assigns `literal_for(row)` to every described row on
/// every handle, each inside its own `pcall` so a refusal collects a message
/// instead of stopping the walk. Returns the rows it probed, in the order their
/// records come back, and the rows it could not build a literal for.
///
/// The walk comes off `api_definition()`, which is projected from the field
/// tables, so neither pass has a list of rows to keep in step with the code.
fn build_row_probe(
    literal_for: fn(&ApiType) -> Option<&'static str>,
) -> (Vec<ProbedRow>, Vec<String>, String) {
    let def = api_definition();
    let mut probed: Vec<ProbedRow> = Vec::new();
    let mut unprobeable: Vec<String> = Vec::new();
    let mut script = String::from("local out = {}\n");

    for handle in &def.handles {
        let acquire = acquire_snippet(handle.name).unwrap_or_else(|| {
            panic!("no fixture acquisition strategy is known for handle type {:?}", handle.name)
        });
        script.push_str("do\n  local H\n  ");
        script.push_str(acquire);
        script.push_str(&format!(
            "\n  if H == nil then error('no {} handle in the fixture') end\n",
            handle.name
        ));
        for field in handle.fields {
            let Some(literal) = literal_for(&field.ty) else {
                unprobeable.push(format!("{}.{} ({:?})", handle.name, field.name, field.ty));
                continue;
            };
            probed.push(ProbedRow { handle: handle.name, field: field.name, access: field.access });
            script.push_str(&format!(
                "  do local ok, err = pcall(function() H['{n}'] = {literal} end)\n\
                 \x20   out[#out+1] = tostring(ok) .. '\\t' .. tostring(err) end\n",
                n = field.name
            ));
        }
        script.push_str("end\n");
    }
    script.push_str("return table.concat(out, '\\n')");
    (probed, unprobeable, script)
}

/// One row a probe assigned to. `access` is carried so the second pass can put
/// its floor on the writable rows alone: a read-only row refuses whatever it is
/// handed, from a message `not_assignable` already derives, so it would prop up
/// that floor without contributing any signal.
struct ProbedRow {
    handle: &'static str,
    field: &'static str,
    access: Access,
}

impl ProbedRow {
    fn label(&self) -> String {
        format!("{}.{}", self.handle, self.field)
    }
}

/// Runs a built probe and returns one `(ok, message)` per row, in probe order.
fn run_row_probe(script: &str, expected: usize) -> Vec<(bool, String)> {
    let mut h = all_capabilities_harness();
    let (status, value) = h.run(script);
    assert_eq!(status, RunStatus::Ok, "the probe script must run cleanly: {value:?}");
    let value = value.expect("a string");
    let records: Vec<(bool, String)> = value
        .split('\n')
        .map(|record| {
            let (ok, message) = record
                .split_once('\t')
                .unwrap_or_else(|| panic!("expected ok<tab>message, got {record:?}"));
            (ok == "true", message.to_string())
        })
        .collect();
    assert_eq!(
        records.len(),
        expected,
        "the generated script and this assertion loop drifted: {expected} probes, {} records",
        records.len()
    );
    records
}

/// Every row's refusal has to name the row it refused, and nothing derives that
/// today: each validator writes its own field name into its own message as a
/// string literal (`expect_str("item_id", ...)`, `expect_bool("is_lucky", ...)`,
/// and so on across every handle). A renamed row keeps its validator -- the row
/// carries it -- but its message would go on quoting the old name, and an author
/// would be told to fix a field that no longer exists.
///
/// Rather than thread a name parameter through some fifty validators, this walks
/// the rows the API definition describes and provokes each one with a value its
/// declared type does not admit, so every row's *type* refusal is reached.
#[test]
fn every_rows_refusal_names_the_row_it_refused() {
    let (probed, unprobeable, script) = build_row_probe(wrong_typed_literal);
    let records = run_row_probe(&script, probed.len());

    let mut accepted: Vec<String> = Vec::new();
    let mut unnamed: Vec<String> = Vec::new();
    for (row, (ok, message)) in probed.iter().zip(&records) {
        if *ok {
            accepted.push(row.label());
        } else if !names_the_field(message, row.field) {
            unnamed.push(format!("{} was refused with {message:?}", row.label()));
        }
    }

    assert_eq!(
        accepted,
        Vec::<String>::new(),
        "these rows accepted a value their declared type does not admit, so nothing about their \
         refusal could be checked"
    );
    assert_eq!(
        unnamed,
        Vec::<String>::new(),
        "a refusal that does not name its row sends an author to fix a field that is not the one \
         they wrote"
    );
    assert_eq!(
        unprobeable,
        Vec::<String>::new(),
        "these rows admit every value this probe can offer, so their refusal is unreachable here \
         -- exclude them deliberately rather than leaving them silently unprobed"
    );

    // Read-only rows pass this trivially: their refusal comes from
    // `FieldSpec::not_assignable`, which already builds the message from the
    // row's own name. All of the signal is in the writable rows, whose
    // validators write the name out by hand -- so the floor is on those.
    let writable = api_definition()
        .handles
        .iter()
        .flat_map(|handle| handle.fields.iter())
        .filter(|field| field.access == Access::ReadWrite)
        .count();
    assert!(
        writable >= 40,
        "only {writable} writable rows exist to probe; the walk is not reaching the tables"
    );
    assert_eq!(
        probed.len(),
        api_definition().handles.iter().map(|handle| handle.fields.len()).sum::<usize>(),
        "every described row must be probed exactly once"
    );
}

/// The second half, and the one that reaches the refusals a wrong type never
/// gets to. A wrong-typed value only ever provokes a row's type check, so a
/// range, domain, key-set, catalog or structural message could quote a stale
/// name with the test above still green.
///
/// The assertion is conditional -- *if* the row refuses, the refusal must name
/// it -- which is what makes a derived second pass safe: a row that legitimately
/// accepts an implausible value is not a failure, and no per-row knowledge or
/// exception table is needed to tell the two apart.
#[test]
fn every_rows_second_refusal_also_names_the_row_it_refused() {
    let (probed, unprobeable, script) = build_row_probe(implausible_literal);
    let records = run_row_probe(&script, probed.len());

    let mut unnamed: Vec<String> = Vec::new();
    let mut refused_writable = 0usize;
    for (row, (ok, message)) in probed.iter().zip(&records) {
        if *ok {
            continue;
        }
        if row.access == Access::ReadWrite {
            refused_writable += 1;
        }
        if !names_the_field(message, row.field) {
            unnamed.push(format!("{} was refused with {message:?}", row.label()));
        }
    }

    assert_eq!(
        unnamed,
        Vec::<String>::new(),
        "a refusal that does not name its row sends an author to fix a field that is not the one \
         they wrote"
    );
    assert_eq!(
        unprobeable,
        Vec::<String>::new(),
        "these rows have no type-valid literal this probe can build, so nothing about their \
         second refusal could be checked -- exclude them deliberately rather than silently"
    );
    // Without a floor this pass would still be green if every row started
    // accepting everything, which is the shape it exists to notice.
    assert!(
        refused_writable >= 20,
        "only {refused_writable} writable rows refused an implausible value; this pass is no \
         longer reaching the range, domain, key-set and catalog refusals it exists for"
    );
}

/// Renders any value a row can hand back as one comparable string. Both sides
/// of the acceptance probe go through it, so the literal that was assigned and
/// the value read back afterwards are rendered by the same code, and nothing
/// about Lua's own number or table formatting has to be predicted in Rust.
const RENDER_HELPER: &str = "local function render(v)\n\
                             \x20 if type(v) ~= 'table' then return tostring(v) end\n\
                             \x20 local parts = {}\n\
                             \x20 local n = #v\n\
                             \x20 for i = 1, n do parts[#parts+1] = tostring(v[i]) end\n\
                             \x20 local keys = {}\n\
                             \x20 for k in pairs(v) do\n\
                             \x20   local positional = type(k) == 'number' and k >= 1 and k <= n and k % 1 == 0\n\
                             \x20   if not positional then keys[#keys+1] = tostring(k) end\n\
                             \x20 end\n\
                             \x20 table.sort(keys)\n\
                             \x20 for _, k in ipairs(keys) do parts[#parts+1] = k .. '=' .. tostring(v[k]) end\n\
                             \x20 return '{' .. table.concat(parts, ',') .. '}'\n\
                             end\n";

/// One accepted assignment: what the row was handed, and what it answered when
/// read straight back.
struct AcceptedRow {
    label: String,
    assigned: String,
    read_back: String,
}

/// Assigns `implausible_literal` to every **writable** row and, where the row
/// accepts it, reads the row straight back. Read-only rows are left out: they
/// refuse everything, so there is no acceptance on them to check.
fn build_acceptance_probe(
    literal_for: fn(&ApiType) -> Option<&'static str>,
) -> (Vec<ProbedRow>, Vec<String>, String) {
    let def = api_definition();
    let mut probed: Vec<ProbedRow> = Vec::new();
    let mut unprobeable: Vec<String> = Vec::new();
    let mut script = String::from("local out = {}\n");
    script.push_str(RENDER_HELPER);

    for handle in &def.handles {
        let acquire = acquire_snippet(handle.name).unwrap_or_else(|| {
            panic!("no fixture acquisition strategy is known for handle type {:?}", handle.name)
        });
        let writable: Vec<&ApiField> =
            handle.fields.iter().filter(|field| field.access == Access::ReadWrite).collect();
        if writable.is_empty() {
            continue;
        }
        script.push_str("do\n  local H\n  ");
        script.push_str(acquire);
        script.push_str(&format!(
            "\n  if H == nil then error('no {} handle in the fixture') end\n",
            handle.name
        ));
        for field in writable {
            let Some(literal) = literal_for(&field.ty) else {
                unprobeable.push(format!("{}.{} ({:?})", handle.name, field.name, field.ty));
                continue;
            };
            probed.push(ProbedRow { handle: handle.name, field: field.name, access: field.access });
            script.push_str(&format!(
                "  do\n\
                 \x20   local want = render({literal})\n\
                 \x20   local ok, err = pcall(function() H['{n}'] = {literal} end)\n\
                 \x20   local back = ''\n\
                 \x20   if ok then back = render(H['{n}']) end\n\
                 \x20   out[#out+1] = tostring(ok) .. '\\t' .. tostring(err) .. '\\t' .. want .. '\\t' .. back\n\
                 \x20 end\n",
                n = field.name
            ));
        }
        script.push_str("end\n");
    }
    script.push_str("return table.concat(out, '\\n')");
    (probed, unprobeable, script)
}

/// The mirror of `implausible_literal`: the most ordinary value the declared
/// type admits, so that the rows whose ranges, domains and catalogs refuse an
/// implausible one still reach their `apply` and get read back. Derived from the
/// type with no per-row knowledge, exactly as the other two are.
///
/// A row that refuses this too is simply not covered by this pass; its refusal
/// is what the two refusal probes are for.
fn plausible_literal(ty: &ApiType) -> Option<&'static str> {
    Some(match ty {
        ApiType::Integer => "1",
        ApiType::Number => "1.0",
        ApiType::String => "'psp'",
        ApiType::Boolean => "true",
        ApiType::List(_) | ApiType::Map { .. } | ApiType::Table => "{}",
        ApiType::Union(members) => {
            return members.iter().find(|member| !matches!(member, ApiType::Nil)).and_then(plausible_literal)
        }
        ApiType::Nil | ApiType::Handle(_) | ApiType::Iterator(_) | ApiType::Multi(_) | ApiType::Any => return None,
    })
}

/// Rows whose read-back is deliberately not the value that was assigned, each
/// with the reason. Named rather than skipped quietly, and a stale entry is a
/// failure of its own.
const NOT_READ_BACK_AS_ASSIGNED: &[(&str, &str)] = &[
    (
        "player.status_point_list",
        "assigning a partial map is a replacement, not a merge: every stat the save already \
         carries a row for is written to zero rather than removed, since the save has no way to \
         record a stat with no row. So `{}` reads back as every existing stat at zero, which is \
         the documented behaviour and is pinned by fields_player.rs's own stat-point tests",
    ),
    (
        "player.ext_status_point_list",
        "the same replacement rule as status_point_list, over the extended stat set",
    ),
];

/// Every writable row's `apply` is `if let FieldValue::X(v) = value { ... }`
/// with a silent do-nothing else-branch. A row whose declared type, whose
/// `validate`'s accepted variants and whose `apply`'s matched variant ever
/// disagreed would report the write as accepted, mark the entry dirty, flush an
/// unchanged DTO and read back the old value -- with nothing failing anywhere.
///
/// The two refusal probes cover refusals in both directions and say nothing
/// about acceptance. This is the other side: for every row that *accepts* the
/// type-valid literal, the value read straight back has to be the value handed
/// in. Derived from the same walk, so a row added later is covered without
/// anyone remembering to.
#[test]
fn every_accepted_assignment_reads_back_as_what_was_assigned() {
    let mut accepted: Vec<AcceptedRow> = Vec::new();
    let mut unprobeable: Vec<String> = Vec::new();

    // Two passes over the same walk. The implausible literal reaches the rows
    // with no domain of their own; the plausible one reaches the rows whose
    // ranges, key sets and catalogs refuse the first. A row covered by neither
    // refuses every value this file can derive, and is left to the refusal
    // probes.
    for literal_for in [implausible_literal, plausible_literal] {
        let (probed, mut pass_unprobeable, script) = build_acceptance_probe(literal_for);
        let mut h = all_capabilities_harness();
        let (status, value) = h.run(&script);
        assert_eq!(status, RunStatus::Ok, "the probe script must run cleanly: {value:?}");
        let value = value.expect("a string");

        let records: Vec<Vec<&str>> =
            value.split('\n').map(|record| record.split('\t').collect()).collect();
        assert_eq!(
            records.len(),
            probed.len(),
            "the generated script and this assertion loop drifted: {} probes, {} records",
            probed.len(),
            records.len()
        );

        for (row, record) in probed.iter().zip(&records) {
            assert_eq!(record.len(), 4, "expected ok<tab>err<tab>want<tab>back, got {record:?}");
            if record[0] != "true" {
                continue;
            }
            accepted.push(AcceptedRow {
                label: row.label(),
                assigned: record[2].to_string(),
                read_back: record[3].to_string(),
            });
        }
        unprobeable.append(&mut pass_unprobeable);
    }

    let mut disagreed: Vec<String> = Vec::new();
    for row in &accepted {
        if NOT_READ_BACK_AS_ASSIGNED.iter().any(|(label, _)| *label == row.label) {
            continue;
        }
        if same_value(&row.assigned, &row.read_back) {
            continue;
        }
        disagreed.push(format!(
            "{} was assigned {} and read back {}",
            row.label, row.assigned, row.read_back
        ));
    }

    assert_eq!(
        disagreed,
        Vec::<String>::new(),
        "an accepted assignment that does not read back as itself is a row whose apply did not \
         apply: the write reported success and changed nothing"
    );
    assert_eq!(
        unprobeable,
        Vec::<String>::new(),
        "these writable rows have no type-valid literal this probe can build, so their \
         acceptance could not be checked -- exclude them deliberately rather than silently"
    );
    for (label, reason) in NOT_READ_BACK_AS_ASSIGNED {
        assert!(
            accepted.iter().any(|row| row.label == *label),
            "{label} is excluded from the read-back check ({reason}) but no longer accepts the \
             probe's literal, so the exclusion is stale"
        );
    }
    // Without a floor this would stay green on a probe that reached nothing. The
    // count is read-backs, not rows: a row each pass reaches is counted by each,
    // so it sits above the number of distinct rows covered.
    assert!(
        accepted.len() >= 40,
        "only {} read-backs succeeded across both passes; they are no longer reaching \
         enough of the apply side to be worth running",
        accepted.len()
    );
}

/// Lua renders `-987654321` as an integer and, once a row has narrowed it to a
/// float, as `-9.87654321e+08`. Those are one value, so numbers are compared as
/// numbers and everything else as text.
fn same_value(assigned: &str, read_back: &str) -> bool {
    if assigned == read_back {
        return true;
    }
    match (assigned.parse::<f64>(), read_back.parse::<f64>()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}
