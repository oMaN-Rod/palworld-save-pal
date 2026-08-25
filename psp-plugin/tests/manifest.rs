use psp_plugin::manifest::{Capability, Manifest, ManifestError, ParamValue};

const GOOD: &str = r#"{
  "id": "pst.cleanup",
  "api_version": 1,
  "name": "Save Cleanup",
  "version": "1.0.0",
  "author": "Palworld Save Pal",
  "license": "GPL-3.0-only",
  "entry": "main.lua",
  "capabilities": ["save.read", "save.write", "log"],
  "commands": [
    {
      "id": "delete_empty_guilds",
      "title": "Delete Empty Guilds",
      "description": "Removes guilds that have no remaining members.",
      "destructive": true,
      "params": []
    },
    {
      "id": "delete_inactive_players",
      "title": "Delete Inactive Players",
      "destructive": true,
      "params": [
        { "id": "days", "type": "int", "label": "Inactive for (days)",
          "default": 30, "min": 1, "max": 3650 }
      ]
    }
  ]
}"#;

const UNBOUNDED_INT_COMMAND: &str = r#"[{"id": "run", "title": "A", "params": [
    {"id": "n", "type": "int", "label": "N"}
]}]"#;

fn with(field: &str, value: &str) -> String {
    let parsed: serde_json::Value = serde_json::from_str(GOOD).expect("fixture parses");
    let mut object = parsed.as_object().expect("fixture is an object").clone();
    object.insert(
        field.to_string(),
        serde_json::from_str(value).expect("replacement parses"),
    );
    serde_json::to_string(&object).expect("re-serializes")
}

#[test]
fn a_well_formed_manifest_parses() {
    let manifest = Manifest::parse(GOOD).expect("should parse");
    assert_eq!(manifest.id, "pst.cleanup");
    assert_eq!(manifest.entry, "main.lua");
    assert_eq!(manifest.commands.len(), 2);
    assert!(manifest.grants(Capability::SaveWrite));
    assert!(!manifest.grants(Capability::SaveRaw));
}

#[test]
fn an_unsupported_api_version_is_refused_with_both_numbers() {
    let error = Manifest::parse(&with("api_version", "7")).unwrap_err();
    assert_eq!(
        error,
        ManifestError::UnsupportedApiVersion { found: 7, supported: 1 }
    );
}

#[test]
fn any_manifest_may_request_raw_access() {
    let raw = with("capabilities", r#"["save.read", "save.raw"]"#);
    let manifest = Manifest::parse(&raw).expect("save.raw is available to every plugin");
    assert!(manifest.capabilities.contains(&Capability::SaveRaw));
}

#[test]
fn write_without_read_is_refused() {
    let write_only = with("capabilities", r#"["save.write"]"#);
    assert_eq!(
        Manifest::parse(&write_only).unwrap_err(),
        ManifestError::WriteRequiresRead
    );
}

#[test]
fn a_duplicate_capability_is_refused() {
    let dupe = with("capabilities", r#"["log", "log"]"#);
    assert!(matches!(
        Manifest::parse(&dupe).unwrap_err(),
        ManifestError::DuplicateCapability(_)
    ));
}

#[test]
fn an_unknown_capability_is_refused_rather_than_ignored() {
    let unknown = with("capabilities", r#"["filesystem"]"#);
    assert!(matches!(
        Manifest::parse(&unknown).unwrap_err(),
        ManifestError::Malformed(_)
    ));
}

#[test]
fn plugin_ids_are_restricted_to_a_safe_alphabet() {
    for bad in ["", "Has Caps", "trailing.", ".leading", "sla/sh", "..", "a..b"] {
        let json = with("id", &serde_json::to_string(bad).expect("quotes"));
        assert!(
            matches!(Manifest::parse(&json), Err(ManifestError::InvalidId(_))),
            "id {bad:?} should have been refused"
        );
    }
    for good in ["pst.cleanup", "a", "my_plugin", "my-plugin", "a.b.c", "x1.y2"] {
        let json = with("id", &serde_json::to_string(good).expect("quotes"));
        Manifest::parse(&json)
            .unwrap_or_else(|error| panic!("id {good:?} should parse, got {error:?}"));
    }
}

#[test]
fn the_entry_must_be_a_plain_lua_filename() {
    for bad in ["../escape.lua", "sub/dir.lua", "back\\slash.lua", "/abs.lua", "main.txt", ""] {
        let json = with("entry", &serde_json::to_string(bad).expect("quotes"));
        assert!(
            matches!(Manifest::parse(&json), Err(ManifestError::InvalidEntry(_))),
            "entry {bad:?} should have been refused"
        );
    }
}

#[test]
fn command_ids_must_be_callable_lua_globals() {
    for bad in ["", "1st", "has-dash", "has.dot", "end", "while", "nil"] {
        let commands = format!(
            r#"[{{"id": {}, "title": "T"}}]"#,
            serde_json::to_string(bad).expect("quotes")
        );
        let json = with("commands", &commands);
        assert!(
            matches!(Manifest::parse(&json), Err(ManifestError::InvalidCommandId(_))),
            "command id {bad:?} should have been refused"
        );
    }
}

#[test]
fn duplicate_command_ids_are_refused() {
    let commands = r#"[{"id": "run", "title": "A"}, {"id": "run", "title": "B"}]"#;
    assert!(matches!(
        Manifest::parse(&with("commands", commands)).unwrap_err(),
        ManifestError::DuplicateCommandId(_)
    ));
}

#[test]
fn a_param_range_must_not_be_inverted() {
    let commands = r#"[{"id": "run", "title": "A", "params": [
        {"id": "n", "type": "int", "label": "N", "min": 10, "max": 1}
    ]}]"#;
    assert!(matches!(
        Manifest::parse(&with("commands", commands)).unwrap_err(),
        ManifestError::InvalidParam { .. }
    ));
}

#[test]
fn param_ids_must_not_be_lua_reserved_words() {
    for bad in ["end", "function", "nil", "local", "while"] {
        let commands = format!(
            r#"[{{"id": "run", "title": "A", "params": [
                {{"id": {}, "type": "bool", "label": "L"}}
            ]}}]"#,
            serde_json::to_string(bad).expect("quotes")
        );
        assert!(
            matches!(
                Manifest::parse(&with("commands", &commands)).unwrap_err(),
                ManifestError::InvalidParam { .. }
            ),
            "param id {bad:?} should have been refused"
        );
    }
}

#[test]
fn an_enum_param_must_offer_at_least_one_option() {
    let commands = r#"[{"id": "run", "title": "A", "params": [
        {"id": "pick", "type": "enum", "label": "Pick", "options": []}
    ]}]"#;
    assert!(matches!(
        Manifest::parse(&with("commands", commands)).unwrap_err(),
        ManifestError::InvalidParam { .. }
    ));
}

#[test]
fn supplied_arguments_are_coerced_and_clamped_against_the_declaration() {
    let manifest = Manifest::parse(GOOD).expect("should parse");
    let command = manifest.command("delete_inactive_players").expect("exists");

    let args = command
        .coerce_args(&serde_json::json!({ "days": 45 }))
        .expect("in-range value is accepted");
    assert_eq!(args, vec![("days".to_string(), ParamValue::Int(45))]);
}

#[test]
fn a_missing_argument_falls_back_to_its_declared_default() {
    let manifest = Manifest::parse(GOOD).expect("should parse");
    let command = manifest.command("delete_inactive_players").expect("exists");

    let args = command
        .coerce_args(&serde_json::json!({}))
        .expect("the default fills in");
    assert_eq!(args, vec![("days".to_string(), ParamValue::Int(30))]);
}

#[test]
fn an_out_of_range_argument_is_refused_not_clamped() {
    let manifest = Manifest::parse(GOOD).expect("should parse");
    let command = manifest.command("delete_inactive_players").expect("exists");

    assert!(matches!(
        command.coerce_args(&serde_json::json!({ "days": 99999 })).unwrap_err(),
        ManifestError::ArgumentOutOfRange { .. }
    ));
}

#[test]
fn an_argument_of_the_wrong_type_is_refused() {
    let manifest = Manifest::parse(GOOD).expect("should parse");
    let command = manifest.command("delete_inactive_players").expect("exists");

    assert!(matches!(
        command.coerce_args(&serde_json::json!({ "days": "thirty" })).unwrap_err(),
        ManifestError::ArgumentType { .. }
    ));
}

#[test]
fn an_undeclared_argument_is_refused_rather_than_passed_through() {
    let manifest = Manifest::parse(GOOD).expect("should parse");
    let command = manifest.command("delete_inactive_players").expect("exists");

    assert!(matches!(
        command
            .coerce_args(&serde_json::json!({ "days": 30, "sneaky": true }))
            .unwrap_err(),
        ManifestError::UndeclaredArgument(_)
    ));
}

#[test]
fn an_integer_argument_at_the_i64_bounds_is_accepted_exactly() {
    let manifest =
        Manifest::parse(&with("commands", UNBOUNDED_INT_COMMAND)).expect("should parse");
    let command = manifest.command("run").expect("exists");

    let args = command
        .coerce_args(&serde_json::json!({ "n": i64::MAX }))
        .expect("i64::MAX fits in an i64");
    assert_eq!(args, vec![("n".to_string(), ParamValue::Int(i64::MAX))]);

    let args = command
        .coerce_args(&serde_json::json!({ "n": i64::MIN }))
        .expect("i64::MIN fits in an i64");
    assert_eq!(args, vec![("n".to_string(), ParamValue::Int(i64::MIN))]);
}

#[test]
fn an_integer_one_past_i64_max_is_refused_not_saturated() {
    let manifest =
        Manifest::parse(&with("commands", UNBOUNDED_INT_COMMAND)).expect("should parse");
    let command = manifest.command("run").expect("exists");

    let one_past_max: serde_json::Value =
        serde_json::from_str(r#"{"n": 9223372036854775808}"#).expect("fits in a u64");
    assert!(matches!(
        command.coerce_args(&one_past_max).unwrap_err(),
        ManifestError::ArgumentType { .. }
    ));
}

#[test]
fn a_huge_out_of_range_number_is_refused_not_saturated() {
    let manifest =
        Manifest::parse(&with("commands", UNBOUNDED_INT_COMMAND)).expect("should parse");
    let command = manifest.command("run").expect("exists");

    assert!(matches!(
        command.coerce_args(&serde_json::json!({ "n": 1e30 })).unwrap_err(),
        ManifestError::ArgumentType { .. }
    ));
    assert!(matches!(
        command.coerce_args(&serde_json::json!({ "n": -1e30 })).unwrap_err(),
        ManifestError::ArgumentType { .. }
    ));
}

#[test]
fn integers_at_and_past_the_f64_precision_boundary_are_accepted_exactly() {
    let manifest =
        Manifest::parse(&with("commands", UNBOUNDED_INT_COMMAND)).expect("should parse");
    let command = manifest.command("run").expect("exists");

    let two_pow_53: i64 = 1i64 << 53;
    let args = command
        .coerce_args(&serde_json::json!({ "n": two_pow_53 }))
        .expect("2^53 is exactly representable");
    assert_eq!(args, vec![("n".to_string(), ParamValue::Int(two_pow_53))]);

    let args = command
        .coerce_args(&serde_json::json!({ "n": two_pow_53 + 1 }))
        .expect("2^53 + 1 still fits exactly as a plain JSON integer");
    assert_eq!(
        args,
        vec![("n".to_string(), ParamValue::Int(two_pow_53 + 1))]
    );
}

#[test]
fn one_less_than_i64_min_collapses_to_i64_min_rather_than_erroring() {
    // serde_json parses this literal to an f64 before this crate sees a Value, and that f64 is bit-identical to i64::MIN.
    let manifest =
        Manifest::parse(&with("commands", UNBOUNDED_INT_COMMAND)).expect("should parse");
    let command = manifest.command("run").expect("exists");

    let one_past_min: serde_json::Value =
        serde_json::from_str(r#"{"n": -9223372036854775809}"#).expect("parses as an f64");
    let args = command
        .coerce_args(&one_past_min)
        .expect("collapses to i64::MIN");
    assert_eq!(args, vec![("n".to_string(), ParamValue::Int(i64::MIN))]);
}

fn manifest_with_param(param_json: &str) -> Result<psp_plugin::manifest::Manifest, psp_plugin::manifest::ManifestError> {
    let json = format!(
        r#"{{
            "id": "test.plugin",
            "api_version": 1,
            "name": "Test",
            "version": "1.0.0",
            "entry": "main.lua",
            "capabilities": ["save.read"],
            "commands": [
                {{ "id": "go", "title": "Go", "params": [{param_json}] }}
            ]
        }}"#
    );
    psp_plugin::manifest::Manifest::parse(&json)
}

#[test]
fn an_entity_param_must_name_a_known_entity_kind() {
    for kind in ["pal", "player", "guild", "base"] {
        let json = format!(r#"{{ "id": "who", "type": "entity", "label": "Who", "entity": "{kind}" }}"#);
        assert!(manifest_with_param(&json).is_ok(), "{kind} must be accepted");
    }

    let missing = manifest_with_param(r#"{ "id": "who", "type": "entity", "label": "Who" }"#);
    assert!(missing.is_err(), "an entity param with no entity kind must be refused");

    let unknown =
        manifest_with_param(r#"{ "id": "who", "type": "entity", "label": "Who", "entity": "dragon" }"#);
    let Err(error) = unknown else { panic!("an unknown entity kind must be refused") };
    let message = error.to_string();
    assert!(message.contains("pal"), "the message must list the kinds it accepts: {message}");
}

#[test]
fn an_entity_param_coerces_a_string_id() {
    let manifest =
        manifest_with_param(r#"{ "id": "who", "type": "entity", "label": "Who", "entity": "player" }"#)
            .expect("a valid entity param");
    let command = manifest.command("go").expect("the command");
    let coerced = command
        .coerce_args(&serde_json::json!({ "who": "abc-123" }))
        .expect("a string id must coerce");
    assert_eq!(coerced, vec![("who".to_string(), psp_plugin::manifest::ParamValue::Text("abc-123".to_string()))]);

    assert!(
        command.coerce_args(&serde_json::json!({ "who": 7 })).is_err(),
        "a number is not an entity id"
    );
}

#[test]
fn a_multiselect_param_coerces_an_array_of_strings() {
    let manifest = manifest_with_param(r#"{ "id": "ids", "type": "multiselect", "label": "Ids", "default": [] }"#)
        .expect("a valid multiselect param");
    let command = manifest.command("go").expect("the command");

    let coerced = command
        .coerce_args(&serde_json::json!({ "ids": ["a", "b"] }))
        .expect("an array of strings must coerce");
    assert_eq!(
        coerced,
        vec![(
            "ids".to_string(),
            psp_plugin::manifest::ParamValue::List(vec!["a".to_string(), "b".to_string()])
        )]
    );

    let empty = command
        .coerce_args(&serde_json::json!({ "ids": [] }))
        .expect("an empty selection is a legitimate value, not a missing one");
    assert_eq!(empty, vec![("ids".to_string(), psp_plugin::manifest::ParamValue::List(Vec::new()))]);

    assert!(
        command.coerce_args(&serde_json::json!({ "ids": "a" })).is_err(),
        "a bare string is not a multiselect value"
    );
    assert!(
        command.coerce_args(&serde_json::json!({ "ids": [1, 2] })).is_err(),
        "a multiselect carries strings"
    );
}

/// An unconstrained multiselect is the whole point -- it is fed from a table's
/// selection, whose contents no manifest can enumerate. Options constrain it
/// only when the manifest chose to declare them.
#[test]
fn a_multiselect_constrains_its_values_only_when_it_declares_options() {
    let open = manifest_with_param(r#"{ "id": "ids", "type": "multiselect", "label": "Ids", "default": [] }"#)
        .expect("an option-less multiselect is valid");
    assert!(open
        .command("go")
        .expect("the command")
        .coerce_args(&serde_json::json!({ "ids": ["anything"] }))
        .is_ok());

    let closed = manifest_with_param(
        r#"{ "id": "ids", "type": "multiselect", "label": "Ids", "options": ["a", "b"], "default": [] }"#,
    )
    .expect("a multiselect with options is valid");
    assert!(closed
        .command("go")
        .expect("the command")
        .coerce_args(&serde_json::json!({ "ids": ["c"] }))
        .is_err());
}

#[test]
fn a_multiselect_default_must_be_an_array_of_strings() {
    assert!(
        manifest_with_param(r#"{ "id": "ids", "type": "multiselect", "label": "Ids", "default": "a" }"#)
            .is_err(),
        "a string default is not an array"
    );
    assert!(
        manifest_with_param(r#"{ "id": "ids", "type": "multiselect", "label": "Ids", "default": [1] }"#)
            .is_err(),
        "a numeric element is not a string"
    );
}

/// The five SP1 types must be untouched by the two new ones.
#[test]
fn every_sp1_param_type_still_parses_and_coerces() {
    let json = r#"{
        "id": "test.plugin",
        "api_version": 1,
        "name": "Test",
        "version": "1.0.0",
        "entry": "main.lua",
        "commands": [
            { "id": "go", "title": "Go", "params": [
                { "id": "i", "type": "int", "label": "I", "default": 1 },
                { "id": "f", "type": "float", "label": "F", "default": 1.5 },
                { "id": "s", "type": "string", "label": "S", "default": "x" },
                { "id": "b", "type": "bool", "label": "B", "default": true },
                { "id": "e", "type": "enum", "label": "E", "options": ["a"], "default": "a" }
            ]}
        ]
    }"#;
    let manifest = psp_plugin::manifest::Manifest::parse(json).expect("the SP1 shape must still parse");
    let coerced = manifest
        .command("go")
        .expect("the command")
        .coerce_args(&serde_json::json!({}))
        .expect("every param has a default");
    assert_eq!(coerced.len(), 5);
}
