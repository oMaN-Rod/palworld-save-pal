mod support;

use psp_plugin::manifest::Capability;
use psp_plugin::status::RunStatus;

const MANIFEST: &str = r#"{
  "id": "test.plugin", "api_version": 1, "name": "Test", "version": "1.0.0",
  "entry": "main.lua",
  "capabilities": ["save.read", "save.write", "log"],
  "commands": [
    { "id": "count_guilds", "title": "Count Guilds" },
    { "id": "needs_arg", "title": "Needs Arg",
      "params": [{ "id": "n", "type": "int", "label": "N", "default": 3, "min": 1, "max": 10 }] },
    { "id": "returns_nothing", "title": "Returns Nothing" },
    { "id": "explodes", "title": "Explodes" }
  ]
}"#;

const SOURCE: &str = r#"
function count_guilds()
  local n = 0
  for _ in save.guilds() do n = n + 1 end
  log.info('counted ' .. n)
  return { summary = 'Found ' .. n .. ' guilds', counts = { guilds = n } }
end

function needs_arg()
  return { summary = 'n was ' .. tostring(ctx.args.n) }
end

function returns_nothing() end

function explodes() error('deliberate failure') end
"#;

#[test]
fn a_command_runs_and_returns_its_summary_and_counts() {
    let outcome = support::run(MANIFEST, SOURCE, "count_guilds", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok);
    let summary = outcome.summary.expect("a summary comes back");
    assert!(summary.starts_with("Found "), "got {summary}");
    assert!(outcome.counts.contains_key("guilds"));
    assert_eq!(outcome.log.len(), 1);
}

#[test]
fn a_declared_default_reaches_ctx_args() {
    let outcome = support::run(MANIFEST, SOURCE, "needs_arg", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok);
    assert_eq!(outcome.summary.as_deref(), Some("n was 3"));
}

#[test]
fn a_supplied_argument_overrides_the_default() {
    let outcome = support::run(MANIFEST, SOURCE, "needs_arg", serde_json::json!({"n": 9}), false);
    assert_eq!(outcome.summary.as_deref(), Some("n was 9"));
}

#[test]
fn an_out_of_range_argument_fails_before_the_script_runs() {
    let outcome = support::run(MANIFEST, SOURCE, "needs_arg", serde_json::json!({"n": 99}), false);
    match outcome.status {
        RunStatus::Error(message) => assert!(message.contains("n"), "got {message}"),
        other => panic!("expected Error, got {other:?}"),
    }
    assert!(outcome.log.is_empty(), "the script must not have run at all");
}

#[test]
fn a_command_returning_nothing_still_succeeds() {
    let outcome = support::run(MANIFEST, SOURCE, "returns_nothing", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok);
    assert_eq!(outcome.summary, None);
}

#[test]
fn a_script_error_is_reported_with_its_message() {
    let outcome = support::run(MANIFEST, SOURCE, "explodes", serde_json::json!({}), false);
    match outcome.status {
        RunStatus::Error(message) => assert!(message.contains("deliberate failure")),
        other => panic!("expected Error, got {other:?}"),
    }
}

#[test]
fn an_unknown_command_is_refused_without_loading_the_script() {
    let outcome = support::run(MANIFEST, SOURCE, "no_such_command", serde_json::json!({}), false);
    assert!(matches!(outcome.status, RunStatus::Error(_)));
}

#[test]
fn a_command_the_manifest_declares_but_the_script_omits_is_a_clear_error() {
    let manifest: serde_json::Value = serde_json::from_str(MANIFEST).expect("parses");
    let outcome = support::run(
        &manifest.to_string(),
        "-- no functions defined at all",
        "count_guilds",
        serde_json::json!({}),
        false,
    );
    match outcome.status {
        RunStatus::Error(message) => assert!(
            message.contains("count_guilds"),
            "the error must name the missing function: {message}"
        ),
        other => panic!("expected Error, got {other:?}"),
    }
}

#[test]
fn an_ungranted_capability_is_absent_from_the_environment() {
    let manifest: serde_json::Value = serde_json::from_str(MANIFEST).expect("parses");
    let mut manifest = manifest.as_object().expect("object").clone();
    manifest.insert("capabilities".into(), serde_json::json!(["save.read"]));
    let outcome = support::run(
        &serde_json::to_string(&manifest).expect("serializes"),
        "function count_guilds() return { summary = type(log) } end",
        "count_guilds",
        serde_json::json!({}),
        false,
    );
    assert_eq!(outcome.status, RunStatus::Ok);
    assert_eq!(outcome.summary.as_deref(), Some("nil"));
}

#[test]
fn a_syntax_error_in_the_entry_file_is_reported_before_any_command_runs() {
    let outcome = support::run(MANIFEST, "function count_guilds( end", "count_guilds",
        serde_json::json!({}), false);
    assert!(matches!(outcome.status, RunStatus::Error(_)));
}

#[test]
fn a_dry_run_reaches_the_script_as_ctx_dry_run() {
    let outcome = support::run(
        MANIFEST,
        "function count_guilds() return { summary = tostring(ctx.dry_run) } end",
        "count_guilds",
        serde_json::json!({}),
        true,
    );
    assert_eq!(outcome.summary.as_deref(), Some("true"));
}

#[test]
fn a_runaway_command_times_out_and_still_returns_its_log() {
    let outcome = support::run_with_timeout(
        MANIFEST,
        "function count_guilds() log.info('starting') while true do end end",
        "count_guilds",
        serde_json::json!({}),
        250,
    );
    assert_eq!(outcome.status, RunStatus::Timeout);
    assert_eq!(outcome.log.len(), 1, "the log written before the timeout survives");
}

const STORAGE_MANIFEST: &str = r#"{
  "id": "test.storage_timeout", "api_version": 1, "name": "Storage Timeout", "version": "1.0.0",
  "entry": "main.lua",
  "capabilities": ["storage"],
  "commands": [ { "id": "spin", "title": "Spin" } ]
}"#;

#[test]
fn a_buffered_storage_write_survives_a_non_ok_status() {
    let outcome = support::run_with_timeout(
        STORAGE_MANIFEST,
        "function spin() storage.set('k', 'v') while true do end end",
        "spin",
        serde_json::json!({}),
        250,
    );
    assert_eq!(outcome.status, RunStatus::Timeout);
    assert_eq!(outcome.storage_writes, vec![("k".to_string(), "v".to_string())]);
}

const DEEP_TABLE_MANIFEST: &str = r#"{
  "id": "test.deep_table", "api_version": 1, "name": "Deep Table", "version": "1.0.0",
  "entry": "main.lua",
  "capabilities": [],
  "commands": [ { "id": "build_deep", "title": "Build Deep" } ]
}"#;

const DEEP_TABLE_SOURCE: &str = r#"
function build_deep()
  local root = {}
  local cur = root
  for i = 1, 60 do
    cur.child = {}
    cur = cur.child
  end
  cur.leaf = true
  return root
end
"#;

#[test]
fn a_result_table_the_converter_cannot_afford_becomes_ok_with_no_result() {
    let outcome = support::run_with_memory(
        DEEP_TABLE_MANIFEST,
        DEEP_TABLE_SOURCE,
        "build_deep",
        serde_json::json!({}),
        24_576,
    );
    assert_eq!(outcome.status, RunStatus::Ok);
    assert_eq!(outcome.result, None);
}

const NO_RAW_MANIFEST: &str = r#"{
  "id": "test.no_raw", "api_version": 1, "name": "No Raw", "version": "1.0.0",
  "entry": "main.lua",
  "capabilities": [],
  "commands": [ { "id": "check_raw", "title": "Check Raw" } ]
}"#;

#[test]
fn a_granted_capability_the_manifest_does_not_declare_is_not_installed() {
    let outcome = support::run_with_granted(
        NO_RAW_MANIFEST,
        "function check_raw() return { summary = type(raw) } end",
        "check_raw",
        serde_json::json!({}),
        &[Capability::SaveRaw],
    );
    assert_eq!(outcome.status, RunStatus::Ok);
    assert_eq!(outcome.summary.as_deref(), Some("nil"));
}
