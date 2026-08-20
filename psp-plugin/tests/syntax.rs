use psp_plugin::syntax::{check, SyntaxError};

#[test]
fn a_well_formed_chunk_reports_no_error() {
    assert_eq!(check("local a = 1\nreturn a\n"), None);
}

#[test]
fn an_empty_source_reports_no_error() {
    assert_eq!(check(""), None);
}

#[test]
fn a_parse_error_reports_its_line_and_a_prefix_free_message() {
    let error = check("local a = 1\nlocal b = = 2\nreturn a\n").expect("this source does not parse");
    assert_eq!(error.line, Some(2));
    assert!(
        !error.message.starts_with("psp:"),
        "the chunk-name prefix must be stripped, got {:?}",
        error.message
    );
    assert!(
        error.message.contains("unexpected symbol"),
        "expected Lua's own diagnostic, got {:?}",
        error.message
    );
}

#[test]
fn an_unterminated_block_is_blamed_on_the_line_lua_blames() {
    let error = check("function f()\n  return 1\n").expect("this source does not parse");
    assert_eq!(error.line, Some(3));
}

#[test]
fn a_chunk_that_parses_is_never_executed() {
    assert_eq!(check("error('this must not be raised')"), None);
}

#[test]
fn a_chunk_that_parses_but_would_loop_forever_is_never_executed() {
    assert_eq!(check("while true do end"), None);
}

#[test]
fn the_error_type_serialises_line_and_message() {
    let error = SyntaxError { line: Some(7), message: "unexpected symbol".to_string() };
    let json = serde_json::to_value(&error).unwrap();
    assert_eq!(json, serde_json::json!({ "line": 7, "message": "unexpected symbol" }));
}

#[test]
fn a_positionless_message_serialises_line_as_null() {
    let error = SyntaxError { line: None, message: "no position".to_string() };
    let json = serde_json::to_value(&error).unwrap();
    assert_eq!(json, serde_json::json!({ "line": null, "message": "no position" }));
}
