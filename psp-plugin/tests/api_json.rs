use psp_plugin::host::api_def::api_definition;

#[test]
fn the_definition_serialises_with_the_field_names_the_editor_expects() {
    let json = serde_json::to_value(api_definition()).expect("serialises");

    let globals = json.get("globals").expect("globals").as_array().expect("array");
    assert!(!globals.is_empty());

    let save = globals
        .iter()
        .find(|g| g.get("name").and_then(|n| n.as_str()) == Some("save"))
        .expect("save is present");
    assert!(save.get("capability").is_some(), "capability travels to the editor");
    assert!(save.get("functions").is_some(), "functions travel to the editor");

    let handles = json.get("handles").expect("handles").as_array().expect("array");
    assert!(
        handles.iter().any(|h| h.get("name").and_then(|n| n.as_str()) == Some("guild")),
        "handle types travel to the editor"
    );
}

/// Only this test inspects `type`/`returns` shape, so it is what catches a revert of `ApiType`'s adjacently-tagged serde attribute.
#[test]
fn api_type_serialises_adjacently_tagged() {
    let json = serde_json::to_value(api_definition()).expect("serialises");

    let gamedata = json["globals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["name"] == "gamedata")
        .expect("gamedata is present");
    let is_valid_item = gamedata["functions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "is_valid_item")
        .expect("is_valid_item is present");
    let id_param_type = &is_valid_item["params"][0]["type"];
    assert_eq!(
        id_param_type,
        &serde_json::json!({ "kind": "string" }),
        "a unit variant must serialise as its kind alone, with no companion value key: {id_param_type}"
    );

    let save = json["globals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["name"] == "save")
        .expect("save is present");
    let players = save["functions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "players")
        .expect("players is present");
    assert_eq!(
        players["returns"],
        serde_json::json!({ "kind": "iterator", "value": "player" }),
        "a newtype variant must carry both its kind and its payload: {}",
        players["returns"]
    );

    let guild = json["handles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["name"] == "guild")
        .expect("guild is present");
    let chest_id = guild["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "chest_container_id")
        .expect("chest_container_id is present");
    assert_eq!(
        chest_id["type"],
        serde_json::json!({
            "kind": "union",
            "value": [{ "kind": "string" }, { "kind": "nil" }]
        }),
        "a Union must carry its members as independently tagged values, not bare strings: {}",
        chest_id["type"]
    );
}

/// The editor mirrors `access` as a string-literal union, so the exact
/// spelling serde emits is the contract between the two and nothing else pins
/// it -- a rename here would type-check on both sides and still not match.
#[test]
fn a_fields_access_serialises_as_the_editor_spells_it() {
    let json = serde_json::to_value(api_definition()).expect("serialises");

    let pal = json["handles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["name"] == "pal")
        .expect("pal is present");
    let fields = pal["fields"].as_array().unwrap();

    let writable = fields.iter().find(|f| f["name"] == "nickname").expect("nickname is present");
    assert_eq!(writable["access"], serde_json::json!("read_write"), "{}", writable["access"]);

    let read_only = fields.iter().find(|f| f["name"] == "instance_id").expect("instance_id is present");
    assert_eq!(read_only["access"], serde_json::json!("read_only"), "{}", read_only["access"]);
}
