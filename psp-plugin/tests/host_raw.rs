mod support;

use psp_plugin::manifest::Capability;
use psp_plugin::status::RunStatus;

/// Runs `body` as a command's Lua source and returns its JSON-converted table result,
/// so a test can index the return value the way the raw host itself does.
fn run_script(body: &str) -> serde_json::Value {
    let manifest = r#"{
      "id": "test.raw_scalar_array", "api_version": 1, "name": "Test", "version": "1.0.0",
      "entry": "main.lua",
      "capabilities": ["save.raw"],
      "commands": [{ "id": "run", "title": "Run" }]
    }"#;
    let source = format!("function run()\n{body}\nend\n");
    let outcome = support::run(manifest, &source, "run", serde_json::json!({}), false);
    match outcome.status {
        RunStatus::Ok => {}
        other => panic!("script failed: {other:?}"),
    }
    outcome.result.expect("the script must return a table")
}

#[test]
fn a_top_level_key_can_be_deleted_in_one_line() {
    let mut h = support::harness(&[Capability::SaveRaw]);
    let (status, value) = h.run(
        "local had = raw.exists('level', 'worldSaveData.GroupSaveDataMap')
         local gone = raw.delete('level', 'worldSaveData.GroupSaveDataMap')
         local still = raw.exists('level', 'worldSaveData.GroupSaveDataMap')
         return tostring(had) .. ',' .. tostring(gone) .. ',' .. tostring(still)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("true,true,false"));
}

#[test]
fn raw_get_errors_on_an_unresolvable_path_rather_than_returning_nil() {
    let mut h = support::harness(&[Capability::SaveRaw]);
    let (status, value) = h.run("return tostring(pcall(raw.get, 'level', 'worldSaveData.NoSuchKeyAtAll'))");
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("false"));
}

#[test]
fn raw_len_errors_on_an_unresolvable_path_rather_than_returning_nil() {
    let mut h = support::harness(&[Capability::SaveRaw]);
    let (status, value) = h.run("return tostring(pcall(raw.len, 'level', 'worldSaveData.NoSuchKeyAtAll'))");
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("false"));
}

#[test]
fn raw_exists_and_raw_kind_probe_without_erroring_on_a_missing_path() {
    let mut h = support::harness(&[Capability::SaveRaw]);
    let (status, value) = h.run(
        "local exists = raw.exists('level', 'worldSaveData.NoSuchKeyAtAll')
         local kind = raw.kind('level', 'worldSaveData.NoSuchKeyAtAll')
         return tostring(exists) .. ',' .. tostring(kind)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("false,nil"));
}

#[test]
fn raw_kind_reports_struct_map_and_scalar_shapes() {
    let mut h = support::harness(&[Capability::SaveRaw]);
    let (status, value) = h.run(
        "local struct_kind = raw.kind('level', 'worldSaveData')
         local map_kind = raw.kind('level', 'worldSaveData.CharacterSaveParameterMap')
         local scalar_kind = raw.kind(
             'level',
             'worldSaveData.CharacterSaveParameterMap[0].value.RawData.SaveParameter.Level'
         )
         return struct_kind .. ',' .. map_kind .. ',' .. scalar_kind",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("struct,map,scalar"));
}

#[test]
fn deleting_an_absent_key_reports_false_rather_than_erroring() {
    let mut h = support::harness(&[Capability::SaveRaw]);
    let (status, value) = h.run("return tostring(raw.delete('level', 'worldSaveData.Nope'))");
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("false"));
}

#[test]
fn a_visit_can_count_and_remove_by_key_name() {
    let mut h = support::harness(&[Capability::SaveRaw]);
    let (status, value) = h.run(
        "local stats = raw.visit('level', 'worldSaveData', function(node)
             if node.key == 'SkinName' then return 'remove' end
         end)
         return tostring(stats.visited > 0) .. ',' .. tostring(stats.stopped)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("true,false"));
}

#[test]
fn a_visit_callback_that_errors_surfaces_the_error_not_a_crash() {
    let mut h = support::harness(&[Capability::SaveRaw]);
    let (status, _) = h.run(
        "raw.visit('level', 'worldSaveData', function(node) error('from the callback') end)",
    );
    match status {
        RunStatus::Error(message) => assert!(message.contains("from the callback"), "got {message}"),
        other => panic!("expected Error, got {other:?}"),
    }
}

#[test]
fn a_visit_is_interrupted_by_the_deadline() {
    let mut h = support::harness_with_timeout(&[Capability::SaveRaw], 250);
    let (status, _) = h.run(
        "while true do raw.visit('level', 'worldSaveData', function() end) end",
    );
    assert_eq!(status, RunStatus::Timeout);
}

#[test]
fn a_malformed_path_is_a_lua_error_not_a_silent_nil() {
    let mut h = support::harness(&[Capability::SaveRaw]);
    let (status, value) = h.run("return tostring(pcall(raw.len, 'level', 'a..b'))");
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("false"));
}

#[test]
fn an_unknown_target_is_a_lua_error() {
    let mut h = support::harness(&[Capability::SaveRaw]);
    let (status, value) = h.run("return tostring(pcall(raw.len, 'moon', 'worldSaveData'))");
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("false"));
}

#[test]
fn the_raw_global_is_absent_when_the_capability_was_not_granted() {
    let mut h = support::harness(&[]);
    let (status, value) = h.run("return type(raw)");
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("nil"));
}

#[test]
fn a_dry_run_makes_every_raw_mutation_a_counting_no_op() {
    let mut h = support::harness_dry(&[Capability::SaveRaw]);
    let (status, value) = h.run(
        "local reported = raw.delete('level', 'worldSaveData.GroupSaveDataMap')
         local still = raw.len('level', 'worldSaveData.GroupSaveDataMap') ~= nil
         return tostring(reported) .. ',' .. tostring(still)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("true,true"));
}

#[test]
fn a_player_scope_reaches_that_players_own_tree() {
    let mut h = support::harness(&[Capability::SaveRaw, Capability::Players]);
    let uid = h.a_player_uid();
    // TowerBossDefeatFlag is a map every player schema carries; SaveData itself is a struct, so raw.len on it would be nil.
    let (status, value) = h.run(&format!(
        "return tostring(raw.len('player:{uid}', 'SaveData.RecordData.TowerBossDefeatFlag') ~= nil)"
    ));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("true"));
}

#[test]
fn a_nested_visit_is_refused_with_a_clean_error() {
    let mut h = support::harness(&[Capability::SaveRaw]);
    let (status, value) = h.run(
        "local inner_ok = true
         raw.visit('level', 'worldSaveData', function()
             local ok = pcall(raw.visit, 'level', 'worldSaveData', function() end)
             if not ok then inner_ok = false end
             return 'stop'
         end)
         return tostring(inner_ok)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("false"), "a nested visit must be refused");
}

#[test]
fn every_raw_function_survives_hostile_arguments() {
    let mut h = support::harness(&[Capability::SaveRaw]);
    let (status, value) = h.run(
        "local vals = { nil, true, 0, -1, 1/0, 0/0, '', 'x', {}, print }
         for _, fn in pairs({ raw.get, raw.exists, raw.kind, raw.set, raw.delete, raw.len, raw.visit }) do
           for i = 1, 10 do for j = 1, 10 do pcall(fn, vals[i], vals[j]) end end
         end
         return 'survived'",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("survived"));
}

#[test]
fn a_visit_node_exposes_a_path_that_round_trips_through_get_and_kind() {
    let mut h = support::harness(&[Capability::SaveRaw]);
    let (status, value) = h.run(
        "local checked = 0
         raw.visit('level', 'worldSaveData.CharacterSaveParameterMap[3].value.RawData.SaveParameter',
             function(node)
                 if node.kind == 'scalar' then
                     assert(raw.get('level', node.path) == node.value,
                         'node.path must resolve back to the same value the walk yielded: ' .. node.path)
                     assert(raw.kind('level', node.path) == 'scalar')
                     checked = checked + 1
                 end
             end
         )
         return tostring(checked > 0)",
    );
    assert_eq!(status, RunStatus::Ok, "{value:?}");
    assert_eq!(value.as_deref(), Some("true"));
}

#[test]
fn raw_set_is_callable_from_inside_a_visit_callback_and_the_write_is_visible_after() {
    let mut h = support::harness(&[Capability::SaveRaw]);
    let (status, value) = h.run(
        "local touched = {}
         raw.visit('level', 'worldSaveData.CharacterSaveParameterMap', function(node)
             if node.key == 'Exp' and node.value ~= 0 then
                 raw.set('level', node.path, 0)
                 table.insert(touched, node.path)
             end
         end)
         local all_zero = true
         for _, p in ipairs(touched) do
             if raw.get('level', p) ~= 0 then all_zero = false end
         end
         return tostring(#touched) .. ',' .. tostring(all_zero)",
    );
    assert_eq!(status, RunStatus::Ok, "{value:?}");
    let value = value.expect("the chunk returns a string");
    let (touched, all_zero) = value.split_once(',').expect("two comma-separated fields");
    let touched: i64 = touched.parse().expect("touched is an integer");
    assert!(touched > 0, "the corpus fixture must have at least one nonzero Exp field");
    assert_eq!(all_zero, "true", "every raw.set made from inside the callback must have taken");
}

#[test]
fn a_mid_walk_set_does_not_skip_or_duplicate_the_remaining_nodes() {
    let script = "local touched = 0
         local stats = raw.visit('level', 'worldSaveData.CharacterSaveParameterMap', function(node)
             if node.key == 'Exp' then
                 raw.set('level', node.path, node.value)
                 touched = touched + 1
             end
         end)
         return tostring(stats.visited) .. ',' .. tostring(touched) .. ',' .. tostring(stats.stopped)";
    let control_script = "local stats = raw.visit('level', 'worldSaveData.CharacterSaveParameterMap', function() end)
         return tostring(stats.visited)";

    let mut mutating = support::harness(&[Capability::SaveRaw]);
    let (mutating_status, mutating_value) = mutating.run(script);
    let mut control = support::harness(&[Capability::SaveRaw]);
    let (control_status, control_value) = control.run(control_script);

    assert_eq!(mutating_status, RunStatus::Ok, "{mutating_value:?}");
    assert_eq!(control_status, RunStatus::Ok, "{control_value:?}");

    let mutating_value = mutating_value.expect("the mutating chunk returns a string");
    let mut parts = mutating_value.split(',');
    let mutating_visited: &str = parts.next().expect("visited field");
    let touched: i64 = parts.next().expect("touched field").parse().expect("touched is an integer");
    let stopped = parts.next().expect("stopped field");

    assert!(touched > 0, "the corpus fixture must have at least one Exp field for this test to mean anything");
    assert_eq!(stopped, "false");
    assert_eq!(
        Some(mutating_visited), control_value.as_deref(),
        "a walk that patches nodes in place must visit exactly as many nodes as an unmutated control walk"
    );
}

#[test]
fn a_dry_run_visit_traverses_the_same_tree_as_a_real_run() {
    let script = "local stats = raw.visit('level', 'worldSaveData', function(node)
             if node.key == 'CharacterSaveParameterMap' then return 'remove' end
         end)
         return tostring(stats.visited) .. ',' .. tostring(stats.removed) .. ',' .. tostring(stats.stopped)";

    let mut dry = support::harness_dry(&[Capability::SaveRaw]);
    let (dry_status, dry_value) = dry.run(script);
    let mut real = support::harness(&[Capability::SaveRaw]);
    let (real_status, real_value) = real.run(script);

    assert_eq!(dry_status, RunStatus::Ok);
    assert_eq!(real_status, RunStatus::Ok);
    assert_eq!(
        dry_value, real_value,
        "a dry run must visit, remove and stop exactly where the real run would"
    );
}

#[test]
fn a_visit_result_reports_removal_errors() {
    let mut h = support::harness(&[Capability::SaveRaw]);
    let (status, value) = h.run(
        "local stats = raw.visit('level', 'worldSaveData', function(node)
             if node.key == 'SkinName' then return 'remove' end
         end)
         return tostring(stats.removal_errors)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("0"));
}

#[test]
fn a_dry_run_raw_set_never_writes_even_when_the_value_would_be_accepted() {
    let mut h = support::harness_dry(&[Capability::SaveRaw, Capability::Players]);
    let uid = h.a_player_uid();
    let (status, value) = h.run(&format!(
        "local before = raw.get('player:{uid}', 'SaveData.TechnologyPoint')
         raw.set('player:{uid}', 'SaveData.TechnologyPoint', before + 1)
         local after = raw.get('player:{uid}', 'SaveData.TechnologyPoint')
         return tostring(before) .. ',' .. tostring(after == before)"
    ));
    assert_eq!(status, RunStatus::Ok);
    let value = value.expect("the chunk returns a string");
    assert!(value.ends_with(",true"), "dry-run raw.set must not write: got {value}");
}

#[test]
fn raw_set_with_an_incompatible_value_errors_and_writes_nothing() {
    let mut h = support::harness(&[Capability::SaveRaw, Capability::Players]);
    let uid = h.a_player_uid();
    let (status, value) = h.run(&format!(
        "local before = raw.get('player:{uid}', 'SaveData.TechnologyPoint')
         local ok = pcall(raw.set, 'player:{uid}', 'SaveData.TechnologyPoint', 'not-a-number')
         local after = raw.get('player:{uid}', 'SaveData.TechnologyPoint')
         return tostring(ok) .. ',' .. tostring(before == after)"
    ));
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("false,true"));
}

#[test]
fn raw_set_on_a_pal_field_is_visible_through_the_domain_api_in_the_same_run() {
    let mut h = support::harness(&[Capability::SaveRead, Capability::SaveRaw]);
    let (status, value) = h.run(
        "local exp_by_id = {}
         for p in save.pals() do exp_by_id[p.instance_id] = p.exp end

         local count = raw.len('level', 'worldSaveData.CharacterSaveParameterMap')
         local id, index = nil, nil
         for i = 0, count - 1 do
           local this_id = raw.get('level', 'worldSaveData.CharacterSaveParameterMap[' .. i .. '].key.InstanceId')
           if exp_by_id[this_id] ~= nil then
             id, index = this_id, i
             break
           end
         end
         assert(id ~= nil, 'no pal entry found in CharacterSaveParameterMap')

         local before = exp_by_id[id]
         local target = 'worldSaveData.CharacterSaveParameterMap[' .. index .. '].value.RawData.SaveParameter.Exp'
         local new_exp = before + 12345
         raw.set('level', target, new_exp)

         local via_raw = raw.get('level', target)
         local via_domain
         for p in save.pals() do
           if p.instance_id == id then via_domain = p.exp end
         end

         return tostring(before) .. ',' .. tostring(new_exp) .. ',' .. tostring(via_raw) .. ',' .. tostring(via_domain)",
    );
    assert_eq!(status, RunStatus::Ok);
    let value = value.expect("the chunk returns a string");
    let parts: Vec<&str> = value.split(',').collect();
    assert_eq!(parts.len(), 4, "got {value}");
    let (before, new_exp, via_raw, via_domain) = (parts[0], parts[1], parts[2], parts[3]);
    assert_ne!(before, new_exp, "the fixture pal's exp did not change under the new value");
    assert_eq!(via_raw, new_exp, "raw.get must see the write it just made");
    assert_eq!(
        via_domain, new_exp,
        "pal.exp must agree with raw.get after a raw.set on the same run \
         -- got raw={via_raw} domain={via_domain}"
    );
}

#[test]
fn a_memory_ceiling_that_starves_global_installation_is_an_error_not_an_abort() {
    // Reaching this test at all, rather than the process dying, is the assertion: an unprotected install_globals would abort here instead.
    let mut h = support::harness_with_memory(&[Capability::SaveRaw], 13_312);
    let (status, _) = h.run("return 'unreachable'");
    match status {
        RunStatus::Error(_) => {}
        other => panic!("expected an error from a starved global installation, got {other:?}"),
    }
}

#[test]
fn a_name_array_element_is_addressable_readable_and_writable() {
    let source = r#"
        local base = nil
        raw.visit('level', 'worldSaveData.CharacterSaveParameterMap', function(node)
            if base == nil and node.key == 'PassiveSkillList' then base = node.path end
        end)
        if base == nil then error('the fixture must carry a PassiveSkillList') end

        local n = raw.len('level', base)
        if n == nil or n == 0 then error('a name array must report its length') end

        local first = raw.get('level', base .. '[0]')
        raw.set('level', base .. '[0]', 'PSP_TEST_MARKER')
        local after = raw.get('level', base .. '[0]')
        raw.set('level', base .. '[0]', first)

        return { n = n, kind = raw.kind('level', base .. '[0]'),
                 first = first, after = after,
                 restored = raw.get('level', base .. '[0]') }
    "#;
    let result = run_script(source);
    assert!(result["n"].as_i64().unwrap_or(0) > 0);
    assert_eq!(result["after"], "PSP_TEST_MARKER");
    assert_eq!(result["restored"], result["first"]);
    assert_eq!(result["kind"], "scalar");
}

#[test]
fn writing_a_number_into_a_name_array_element_is_refused_rather_than_coerced() {
    let source = r#"
        local base = nil
        raw.visit('level', 'worldSaveData.CharacterSaveParameterMap', function(node)
            if base == nil and node.key == 'PassiveSkillList' and raw.len('level', node.path) > 0 then
                base = node.path
            end
        end)
        if base == nil then error('the fixture must carry a non-empty PassiveSkillList') end

        local before = raw.get('level', base .. '[0]')
        local ok, err = pcall(function() raw.set('level', base .. '[0]', 12345) end)
        return { ok = ok, err = tostring(err),
                 unchanged = raw.get('level', base .. '[0]') == before,
                 still_a_string = type(raw.get('level', base .. '[0]')) == 'string' }
    "#;
    let result = run_script(source);
    assert_eq!(result["ok"], false, "a number written into a name array must be refused");
    assert_eq!(result["unchanged"], true, "a refused write must leave the element alone");
    assert_eq!(result["still_a_string"], true, "the element must not be coerced to another type");
}

#[test]
fn deleting_a_name_array_element_shortens_the_array() {
    let source = r#"
        local base = nil
        raw.visit('level', 'worldSaveData.CharacterSaveParameterMap', function(node)
            if base == nil and node.key == 'PassiveSkillList' and raw.len('level', node.path) > 0 then
                base = node.path
            end
        end)
        if base == nil then error('the fixture must carry a non-empty PassiveSkillList') end
        local before = raw.len('level', base)
        raw.delete('level', base .. '[0]')
        return { before = before, after = raw.len('level', base) }
    "#;
    let result = run_script(source);
    assert_eq!(
        result["after"].as_i64().unwrap_or(-1),
        result["before"].as_i64().unwrap_or(0) - 1
    );
}

#[test]
fn a_walk_reaches_the_elements_of_a_name_array() {
    let source = r#"
        local seen = 0
        raw.visit('level', 'worldSaveData.CharacterSaveParameterMap', function(node)
            if node.index ~= nil and type(node.value) == 'string' then seen = seen + 1 end
        end)
        return { seen = seen }
    "#;
    assert!(run_script(source)["seen"].as_i64().unwrap_or(0) > 0);
}
