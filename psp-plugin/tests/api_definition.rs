mod support;

use std::collections::{BTreeMap, BTreeSet};

use psp_plugin::host::api_def::{api_definition, ApiType};
use psp_plugin::manifest::Capability;
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
