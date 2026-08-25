mod support;

use std::collections::BTreeMap;

use psp_plugin::status::RunStatus;

const MANIFEST: &str = r#"{
  "id": "test.modules",
  "api_version": 1,
  "name": "Modules",
  "version": "1.0.0",
  "entry": "main.lua",
  "capabilities": [],
  "commands": [{ "id": "go", "title": "Go" }]
}"#;

fn sources(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

#[test]
fn a_required_module_returns_its_value_to_the_entry() {
    let outcome = support::run_multi(
        MANIFEST,
        sources(&[
            ("main.lua", "local util = require('lib.util')\nfunction go() return util.greet() end"),
            ("lib/util.lua", "return { greet = function() return 'hello' end }"),
        ]),
        "go",
        serde_json::json!({}),
        false,
    );
    assert_eq!(outcome.status, RunStatus::Ok);
    assert_eq!(outcome.summary.as_deref(), Some("hello"));
}

#[test]
fn a_module_is_evaluated_once_however_many_times_it_is_required() {
    let outcome = support::run_multi(
        MANIFEST,
        sources(&[
            (
                "main.lua",
                "require('counter') require('counter')\n\
                 function go() return tostring(require('counter').n) end",
            ),
            ("counter.lua", "local t = { n = 0 } t.n = t.n + 1 return t"),
        ]),
        "go",
        serde_json::json!({}),
        false,
    );
    assert_eq!(outcome.status, RunStatus::Ok);
    assert_eq!(outcome.summary.as_deref(), Some("1"));
}

#[test]
fn requiring_a_module_that_is_not_in_sources_fails_with_its_name() {
    let outcome = support::run_multi(
        MANIFEST,
        sources(&[("main.lua", "require('nope')\nfunction go() return 'unreachable' end")]),
        "go",
        serde_json::json!({}),
        false,
    );
    match outcome.status {
        RunStatus::Error(message) => assert!(
            message.contains("nope"),
            "the error must name the missing module, got {message:?}"
        ),
        other => panic!("expected an error, got {other:?}"),
    }
}

#[test]
fn a_circular_require_is_reported_rather_than_looping() {
    let outcome = support::run_multi(
        MANIFEST,
        sources(&[
            ("main.lua", "require('a')\nfunction go() return 'unreachable' end"),
            ("a.lua", "require('b') return {}"),
            ("b.lua", "require('a') return {}"),
        ]),
        "go",
        serde_json::json!({}),
        false,
    );
    match outcome.status {
        RunStatus::Error(message) => assert!(
            message.contains("circular"),
            "the error must say the require is circular, got {message:?}"
        ),
        other => panic!("expected an error, got {other:?}"),
    }
}

#[test]
fn a_syntax_error_inside_a_module_names_that_module_not_the_entry() {
    let outcome = support::run_multi(
        MANIFEST,
        sources(&[
            ("main.lua", "require('broken')\nfunction go() return 'unreachable' end"),
            ("broken.lua", "local = = ="),
        ]),
        "go",
        serde_json::json!({}),
        false,
    );
    match outcome.status {
        RunStatus::Error(message) => assert!(
            message.contains("broken.lua"),
            "the error must name broken.lua, got {message:?}"
        ),
        other => panic!("expected an error, got {other:?}"),
    }
}

#[test]
fn a_module_returning_nothing_is_cached_as_true() {
    let outcome = support::run_multi(
        MANIFEST,
        sources(&[
            ("main.lua", "function go() return tostring(require('bare')) end"),
            ("bare.lua", "local unused = 1"),
        ]),
        "go",
        serde_json::json!({}),
        false,
    );
    assert_eq!(outcome.status, RunStatus::Ok);
    assert_eq!(outcome.summary.as_deref(), Some("true"));
}

#[test]
fn the_package_library_is_still_absent() {
    let outcome = support::run_multi(
        MANIFEST,
        sources(&[(
            "main.lua",
            "function go() return tostring(package) .. ',' .. tostring(loadfile) end",
        )]),
        "go",
        serde_json::json!({}),
        false,
    );
    assert_eq!(outcome.status, RunStatus::Ok);
    assert_eq!(outcome.summary.as_deref(), Some("nil,nil"));
}

#[test]
fn require_rejects_a_non_string_name() {
    let outcome = support::run_multi(
        MANIFEST,
        sources(&[("main.lua", "require(42)\nfunction go() return 'unreachable' end")]),
        "go",
        serde_json::json!({}),
        false,
    );
    match outcome.status {
        RunStatus::Error(message) => assert!(
            message.contains("module name string"),
            "the error must say require needs a string name, got {message:?}"
        ),
        other => panic!("expected an error, got {other:?}"),
    }
}

#[test]
fn a_module_that_raised_can_be_required_again_without_being_poisoned_or_cached() {
    let outcome = support::run_multi(
        MANIFEST,
        sources(&[
            (
                "main.lua",
                "local ok1, err1 = pcall(require, 'bad')\n\
                 local ok2, err2 = pcall(require, 'bad')\n\
                 function go()\n\
                   local circular1 = tostring(err1):find('circular') ~= nil\n\
                   local circular2 = tostring(err2):find('circular') ~= nil\n\
                   return tostring(ok1) .. ',' .. tostring(ok2) .. ',' \
                       .. tostring(circular1) .. ',' .. tostring(circular2)\n\
                 end",
            ),
            ("bad.lua", "error('boom')"),
        ]),
        "go",
        serde_json::json!({}),
        false,
    );
    assert_eq!(outcome.status, RunStatus::Ok);
    assert_eq!(
        outcome.summary.as_deref(),
        Some("false,false,false,false"),
        "both attempts must fail the same way — neither cached as success nor \
         misreported as a circular require"
    );
}
