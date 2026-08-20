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

fn allowed_lua_types(ty: &ApiType) -> Vec<&'static str> {
    match ty {
        ApiType::Nil => vec!["nil"],
        ApiType::Boolean => vec!["boolean"],
        ApiType::Integer | ApiType::Number => vec!["number"],
        ApiType::String => vec!["string"],
        ApiType::Table => vec!["table"],
        ApiType::Handle(_) => vec!["userdata"],
        ApiType::Iterator(_) => vec!["function"],
        ApiType::Any => vec!["nil", "boolean", "number", "string", "table", "function", "userdata", "thread"],
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

    let mut script = String::from("local results = {}\n");
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
            script.push_str(&format!("    out[#out+1] = '{n}=' .. type(H['{n}'])\n", n = field.name));
        }
        for method in handle.methods {
            script.push_str(&format!("    out[#out+1] = '{n}=' .. type(H['{n}'])\n", n = method.name));
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
