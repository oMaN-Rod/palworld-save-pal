use std::collections::BTreeMap;

use psp_server::services::lsp_workspace::materialise;

fn sources() -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "main.lua".to_string(),
            "local u = require('lib.util')".to_string(),
        ),
        ("lib/util.lua".to_string(), "return {}".to_string()),
    ])
}

#[test]
fn a_workspace_holds_the_meta_file_the_luarc_and_every_source() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let workspace = materialise(dir.path(), "user.demo", &sources()).expect("materialise");

    let meta = std::fs::read_to_string(workspace.join("psp.lua")).expect("psp.lua");
    assert_eq!(
        meta,
        psp_plugin::lua_meta(&psp_plugin::api_definition()),
        "the meta file must be the generated one, verbatim"
    );

    let luarc: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(workspace.join(".luarc.json")).expect("luarc"),
    )
    .expect("valid json");
    assert_eq!(luarc["runtime"]["version"], "Lua 5.4");

    assert!(workspace.join("main.lua").exists());
    assert!(workspace.join("lib/util.lua").exists());
}

#[test]
fn a_source_path_escaping_the_workspace_is_refused() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let mut bad = sources();
    bad.insert("../escape.lua".to_string(), "return {}".to_string());
    assert!(materialise(dir.path(), "user.demo", &bad).is_err());
}

#[test]
fn materialising_twice_removes_a_source_that_is_gone() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let workspace = materialise(dir.path(), "user.demo", &sources()).expect("first");
    assert!(workspace.join("lib/util.lua").exists());

    let mut fewer = sources();
    fewer.remove("lib/util.lua");
    materialise(dir.path(), "user.demo", &fewer).expect("second");
    assert!(
        !workspace.join("lib/util.lua").exists(),
        "a stale source would keep producing diagnostics for a file the author deleted"
    );
}
