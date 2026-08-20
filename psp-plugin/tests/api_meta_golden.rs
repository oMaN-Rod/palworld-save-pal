use psp_plugin::host::api_def::api_definition;

#[test]
fn the_meta_file_declares_globals_handles_and_capabilities() {
    let meta = psp_plugin::lua_meta(&api_definition());

    assert!(meta.starts_with("---@meta"), "LuaLS requires the meta marker");
    assert!(meta.contains("---@class guild"), "handles become classes: {meta:.400}");
    assert!(
        meta.contains("---@field chest_container_id string|nil"),
        "a nullable field must keep its nil in the annotation"
    );
    assert!(
        meta.contains("function save.unlock_private_chests()"),
        "write-half functions appear in the meta file"
    );
    assert!(
        meta.contains("save.raw"),
        "the capability a function needs must be visible to a reader"
    );
}

#[test]
fn the_meta_file_matches_its_golden() {
    let generated = psp_plugin::lua_meta(&api_definition());
    let golden = include_str!("fixtures/psp.lua");
    assert_eq!(
        generated, golden,
        "the API surface changed; regenerate the golden and review the diff"
    );
}
