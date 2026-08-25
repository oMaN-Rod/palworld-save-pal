use psp_plugin::manifest::{Manifest, ManifestError};

/// Every test builds one manifest around a `ui` body, so the surrounding
/// commands and params stay constant and only the view under test varies.
fn manifest_with_view(ui: &str) -> Result<Manifest, ManifestError> {
    manifest_with(ui, r#"["save.read"]"#)
}

fn manifest_with(ui: &str, capabilities: &str) -> Result<Manifest, ManifestError> {
    let json = format!(
        r#"{{
            "id": "test.plugin",
            "api_version": 1,
            "name": "Test",
            "version": "1.0.0",
            "entry": "main.lua",
            "capabilities": {capabilities},
            "commands": [
                {{ "id": "scan", "title": "Scan", "params": [
                    {{ "id": "who", "type": "entity", "label": "Who", "entity": "player" }},
                    {{ "id": "min_level", "type": "int", "label": "Min", "default": 1 }}
                ]}},
                {{ "id": "fix", "title": "Fix", "destructive": true, "params": [
                    {{ "id": "ids", "type": "multiselect", "label": "Ids", "default": [] }}
                ]}}
            ],
            "ui": {ui}
        }}"#
    );
    Manifest::parse(&json)
}

const GOOD_VIEW: &str = r#"[
    { "title": "Filters", "columns": 2, "widgets": [
        { "type": "entity_select", "id": "who", "entity": "player", "label": "Player" },
        { "type": "number_input", "id": "min_level", "label": "Minimum level" }
    ]},
    { "columns": 1, "widgets": [
        { "type": "button", "label": "Scan", "command": "scan", "span": "full" },
        { "type": "table", "id": "rows", "from": "scan", "path": "pals",
          "columns": ["name", "level"], "selectable": true },
        { "type": "button", "label": "Fix selected", "command": "fix", "args": { "ids": "rows.selection" } }
    ]}
]"#;

#[test]
fn a_manifest_with_no_ui_parses_and_carries_an_empty_view() {
    let json = r#"{
        "id": "test.plugin",
        "api_version": 1,
        "name": "Test",
        "version": "1.0.0",
        "entry": "main.lua",
        "commands": [{ "id": "go", "title": "Go" }]
    }"#;
    let manifest = Manifest::parse(json).expect("an SP1 manifest must still parse");
    assert!(manifest.ui.is_empty(), "a plugin with no ui declares no sections");
}

#[test]
fn the_spec_example_view_parses_whole() {
    let manifest = manifest_with_view(GOOD_VIEW).expect("the example view must parse");
    assert_eq!(manifest.ui.len(), 2);
    assert_eq!(manifest.ui[0].title.as_deref(), Some("Filters"));
    assert_eq!(manifest.ui[0].columns, 2);
    assert_eq!(manifest.ui[0].widgets.len(), 2);
    assert_eq!(manifest.ui[1].title, None);
    assert_eq!(manifest.ui[1].columns, 1);

    let table = &manifest.ui[1].widgets[1];
    assert_eq!(table.widget_type, "table");
    assert_eq!(table.id.as_deref(), Some("rows"));
    assert_eq!(table.from.as_deref(), Some("scan"));
    assert_eq!(table.path.as_deref(), Some("pals"));
    assert_eq!(table.columns, vec!["name".to_string(), "level".to_string()]);
    assert!(table.selectable);

    let fix = &manifest.ui[1].widgets[2];
    assert_eq!(fix.args.get("ids").map(String::as_str), Some("rows.selection"));
}

/// A section defaults to one column, so the commonest view needs no `columns`.
#[test]
fn a_section_without_columns_is_one_column() {
    let manifest = manifest_with_view(r#"[{ "widgets": [{ "type": "text", "from": "scan" }] }]"#)
        .expect("a section may omit columns");
    assert_eq!(manifest.ui[0].columns, 1);
}

#[test]
fn a_section_column_count_outside_one_to_three_is_refused() {
    for columns in ["0", "4", "17"] {
        let ui = format!(r#"[{{ "columns": {columns}, "widgets": [] }}]"#);
        let Err(error) = manifest_with_view(&ui) else {
            panic!("{columns} columns must be refused");
        };
        let message = error.to_string();
        assert!(message.contains("1, 2 or 3"), "the message must say what is allowed: {message}");
    }
}

#[test]
fn an_input_widget_whose_id_names_no_param_is_refused() {
    let ui = r#"[{ "widgets": [{ "type": "number_input", "id": "nonesuch", "label": "X" }] }]"#;
    let Err(error) = manifest_with_view(ui) else {
        panic!("an input widget bound to nothing must be refused");
    };
    let message = error.to_string();
    assert!(message.contains("nonesuch"), "the message must name the widget: {message}");
}

#[test]
fn an_input_widget_with_no_id_is_refused() {
    let ui = r#"[{ "widgets": [{ "type": "text_input", "label": "X" }] }]"#;
    assert!(manifest_with_view(ui).is_err(), "an input widget with no id feeds nothing");
}

#[test]
fn a_duplicate_widget_id_is_refused() {
    let ui = r#"[{ "widgets": [
        { "type": "number_input", "id": "min_level", "label": "A" },
        { "type": "number_input", "id": "min_level", "label": "B" }
    ]}]"#;
    let Err(error) = manifest_with_view(ui) else { panic!("a duplicate widget id must be refused") };
    assert!(error.to_string().contains("min_level"), "{error}");
}

#[test]
fn a_button_must_name_a_declared_command() {
    let missing = r#"[{ "widgets": [{ "type": "button", "label": "Go" }] }]"#;
    assert!(manifest_with_view(missing).is_err(), "a button with no command does nothing");

    let unknown = r#"[{ "widgets": [{ "type": "button", "label": "Go", "command": "nonesuch" }] }]"#;
    let Err(error) = manifest_with_view(unknown) else { panic!("an unknown command must be refused") };
    assert!(error.to_string().contains("nonesuch"), "{error}");
}

#[test]
fn a_from_must_name_a_declared_command() {
    let ui = r#"[{ "widgets": [{ "type": "table", "id": "rows", "from": "nonesuch" }] }]"#;
    let Err(error) = manifest_with_view(ui) else { panic!("an unknown from must be refused") };
    assert!(error.to_string().contains("nonesuch"), "{error}");
}

#[test]
fn an_args_entry_must_reference_a_param_of_its_own_command_and_a_widget_that_exists() {
    let unknown_param = r#"[{ "widgets": [
        { "type": "table", "id": "rows", "from": "scan", "selectable": true },
        { "type": "button", "label": "Fix", "command": "fix", "args": { "nonesuch": "rows.selection" } }
    ]}]"#;
    let Err(error) = manifest_with_view(unknown_param) else {
        panic!("an args key that is not a param must be refused")
    };
    assert!(error.to_string().contains("nonesuch"), "{error}");

    let unknown_widget = r#"[{ "widgets": [
        { "type": "button", "label": "Fix", "command": "fix", "args": { "ids": "ghost.selection" } }
    ]}]"#;
    let Err(error) = manifest_with_view(unknown_widget) else {
        panic!("an args value naming no widget must be refused")
    };
    assert!(error.to_string().contains("ghost"), "{error}");
}

/// The reference grammar is closed: no expression language, ever.
#[test]
fn an_args_value_outside_the_reference_grammar_is_refused() {
    for reference in ["rows", "rows.value()", "rows.selection.first", "1 + 1", "rows.total"] {
        let ui = format!(
            r#"[{{ "widgets": [
                {{ "type": "table", "id": "rows", "from": "scan", "selectable": true }},
                {{ "type": "button", "label": "Fix", "command": "fix", "args": {{ "ids": "{reference}" }} }}
            ]}}]"#
        );
        assert!(
            manifest_with_view(&ui).is_err(),
            "{reference:?} must not parse as a widget reference"
        );
    }
}

#[test]
fn both_halves_of_the_reference_grammar_are_accepted() {
    let ui = r#"[{ "widgets": [
        { "type": "table", "id": "rows", "from": "scan", "selectable": true },
        { "type": "number_input", "id": "min_level", "label": "Min" },
        { "type": "button", "label": "Fix", "command": "fix", "args": { "ids": "rows.selection" } }
    ]}]"#;
    assert!(manifest_with_view(ui).is_ok(), "a selection reference must be accepted");

    let value_ref = r#"[{ "widgets": [
        { "type": "number_input", "id": "min_level", "label": "Min" },
        { "type": "button", "label": "Fix", "command": "fix", "args": { "ids": "min_level.value" } }
    ]}]"#;
    assert!(manifest_with_view(value_ref).is_ok(), "a value reference must be accepted");
}

#[test]
fn a_selectable_table_must_have_an_id() {
    let ui = r#"[{ "widgets": [{ "type": "table", "from": "scan", "selectable": true }] }]"#;
    let Err(error) = manifest_with_view(ui) else {
        panic!("a selection nothing can reference must be refused")
    };
    assert!(error.to_string().contains("selectable"), "{error}");
}

#[test]
fn a_span_other_than_full_is_refused() {
    let ui = r#"[{ "widgets": [{ "type": "text", "from": "scan", "span": "half" }] }]"#;
    assert!(manifest_with_view(ui).is_err(), "span has exactly one legal value");
}

#[test]
fn an_entity_select_requires_save_read() {
    let ui = r#"[{ "widgets": [{ "type": "entity_select", "id": "who", "entity": "player", "label": "Who" }] }]"#;
    assert!(manifest_with(ui, r#"["save.read"]"#).is_ok());

    let Err(error) = manifest_with(ui, r#"["log"]"#) else {
        panic!("an entity_select without save.read must be refused at install")
    };
    let message = error.to_string();
    assert!(message.contains("save.read"), "the message must name the capability: {message}");
}

/// Forward compatibility: the widget vocabulary and the entity vocabulary can
/// grow host-side, so a manifest written against a newer host must still
/// install. The renderer skips what it does not know.
#[test]
fn an_unknown_widget_type_or_entity_still_installs() {
    let widget = r#"[{ "widgets": [{ "type": "sparkline", "id": "x", "from": "scan" }] }]"#;
    assert!(manifest_with_view(widget).is_ok(), "an unknown widget type must not block install");

    let entity = r#"[{ "widgets": [{ "type": "entity_select", "id": "who", "entity": "dragon", "label": "Who" }] }]"#;
    assert!(manifest_with_view(entity).is_ok(), "an unknown entity value must not block install");
}
