mod common;

use psp_core::domain::blueprint::{BaseBlueprint, BlueprintHeader, CaptureOptions, SCHEMA_VERSION};
use psp_core::domain::blueprint::{capture, gvas};
use psp_core::ue::{
    Byte, MapEntry, PalStruct, Properties, Property, PropertyKey, StructValue,
};

/// How many of the blueprint's payloads are still TYPED Palworld structs
/// rather than opaque byte arrays. Byte-identical round trips prove nothing
/// here: every consumer in `capture`/`transform`/`scrub` pattern-matches
/// `StructValue::Game(PalStruct::..)`, so a decode that loses the type view
/// yields a blueprint that is silently inert.
#[derive(Debug, PartialEq, Eq)]
struct TypedCounts {
    models: usize,
    concrete_models: usize,
    works: usize,
    characters: usize,
}

fn raw_data_of(properties: &Properties) -> Option<&StructValue> {
    match properties.0.get(&PropertyKey::from("RawData")) {
        Some(Property::Struct(value)) => Some(value),
        _ => None,
    }
}

fn nested_raw_data<'a>(properties: &'a Properties, field: &str) -> Option<&'a StructValue> {
    let nested = properties
        .0
        .get(&PropertyKey::from(field))
        .and_then(psp_core::props::struct_props)?;
    raw_data_of(nested)
}

fn struct_value_raw_data(value: &StructValue) -> Option<&StructValue> {
    match value {
        StructValue::Struct(properties) => raw_data_of(properties),
        _ => None,
    }
}

fn entry_raw_data(entry: &MapEntry) -> Option<&StructValue> {
    psp_core::props::struct_props(&entry.value).and_then(raw_data_of)
}

fn typed_counts(blueprint: &BaseBlueprint) -> TypedCounts {
    TypedCounts {
        models: blueprint
            .structures
            .iter()
            .filter(|s| {
                matches!(
                    nested_raw_data(&s.properties, "Model"),
                    Some(StructValue::Game(PalStruct::MapModel(_)))
                )
            })
            .count(),
        concrete_models: blueprint
            .structures
            .iter()
            .filter(|s| {
                matches!(
                    nested_raw_data(&s.properties, "ConcreteModel"),
                    Some(StructValue::Game(PalStruct::MapConcreteModel(_)))
                )
            })
            .count(),
        works: blueprint
            .works
            .iter()
            .filter(|w| {
                matches!(struct_value_raw_data(w), Some(StructValue::Game(PalStruct::Work(_))))
            })
            .count(),
        characters: blueprint
            .characters
            .iter()
            .filter(|e| {
                matches!(
                    entry_raw_data(e),
                    Some(StructValue::Game(PalStruct::CharacterData(_)))
                )
            })
            .count(),
    }
}

fn captured_fixture_blueprint() -> BaseBlueprint {
    let session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);
    capture::capture(&session, base_id, CaptureOptions::full(), "Home").expect("capture")
}

fn empty_blueprint() -> BaseBlueprint {
    let source_header = common::load_fixture_session("v1_relics").level.header.clone();
    BaseBlueprint {
        source_header,
        header: BlueprintHeader {
            schema_version: SCHEMA_VERSION,
            game_data_version: "test".to_string(),
            uesave_struct_version: "test".to_string(),
            manifest: CaptureOptions::blueprint(),
            name: "Test Base".to_string(),
            source_world: "world1".to_string(),
            source_base: "Home".to_string(),
            created_at: 0,
            structure_count: 0,
            footprint_radius: 3500.0,
            anchor_height_above_terrain: 0.0,
        },
        base_camp: None,
        structures: Vec::new(),
        item_containers: Vec::new(),
        character_containers: Vec::new(),
        characters: Vec::new(),
        works: Vec::new(),
        dynamic_items: Vec::new(),
    }
}

#[test]
fn the_header_round_trips_through_json() {
    let original = empty_blueprint();

    let json = serde_json::to_string(&original.header).expect("header must serialize to json");
    let restored: BlueprintHeader =
        serde_json::from_str(&json).expect("header must parse back from json");

    assert_eq!(restored.name, "Test Base");
    assert_eq!(restored.schema_version, SCHEMA_VERSION);
    assert_eq!(restored.footprint_radius, 3500.0);
    assert_eq!(restored.manifest, original.header.manifest);
}

#[test]
fn a_newer_schema_version_is_refused() {
    let mut blueprint = empty_blueprint();
    blueprint.header.schema_version = SCHEMA_VERSION + 1;

    let result = blueprint.check_schema_version();

    assert!(
        result.is_err(),
        "a blueprint from a newer schema must be refused, not silently misread"
    );
}

#[test]
fn a_manifest_promising_contents_it_lacks_is_refused() {
    let mut blueprint = empty_blueprint();
    blueprint.header.manifest.container_contents = true;
    blueprint.header.structure_count = 1;

    let result = blueprint.check_manifest_consistency();

    assert!(
        result.is_err(),
        "a manifest advertising a layer absent from the payload must be refused"
    );
}

#[test]
fn presets_select_the_documented_layers() {
    let blueprint = CaptureOptions::blueprint();
    assert!(blueprint.production_config, "blueprint preset keeps recipes");
    assert!(!blueprint.container_contents, "blueprint preset drops contents");

    let full = CaptureOptions::full();
    assert!(full.container_contents, "full preset keeps contents");
    assert!(full.worker_pals, "full preset keeps worker pals");
}

#[test]
fn a_manifest_claiming_no_state_layers_is_consistent_when_empty() {
    // Kills an implementation that ignores manifest and wrongly rejects if structure_count > 0.
    let mut blueprint = empty_blueprint();
    blueprint.header.structure_count = 1;

    let result = blueprint.check_manifest_consistency();

    assert!(
        result.is_ok(),
        "a blueprint with structures but no claimed state layers must be valid"
    );
}

#[test]
fn an_older_or_current_schema_version_is_accepted() {
    // Kills an implementation using != instead of > that rejects older blueprints.
    let current = empty_blueprint();
    assert_eq!(
        current.header.schema_version, SCHEMA_VERSION,
        "setup: empty blueprint has current schema"
    );
    assert!(
        current.check_schema_version().is_ok(),
        "current schema version must be accepted"
    );

    let mut older = empty_blueprint();
    older.header.schema_version = SCHEMA_VERSION - 1;
    assert!(
        older.check_schema_version().is_ok(),
        "older schema version must be accepted and migrated"
    );
}

/// The regression that motivated the Task 3/6 amendment. `MapObjectId` is a
/// `Property::Name`; under a naive untagged serde derive it came back as
/// `Property::Byte(Label(..))`, silently corrupting every structure's type.
/// Empty-collection round trips pass vacuously, so this asserts on real data.
#[test]
fn json_round_trip_preserves_property_variants_not_just_values() {
    let session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);
    let original =
        capture::capture(&session, base_id, CaptureOptions::full(), "Home").expect("capture");

    let restored = gvas::from_json(&gvas::to_json(&original).expect("json encode"))
        .expect("json decode");

    assert!(
        !original.structures.is_empty(),
        "setup: the fixture must capture structures, or this round trip passes vacuously"
    );
    assert_eq!(restored.structures.len(), original.structures.len());
    for (a, b) in original.structures.iter().zip(restored.structures.iter()) {
        assert_eq!(a.map_object_id, b.map_object_id, "structure type must survive");
        assert!(
            psp_core::domain::blueprint::capture::map_object_id_is_name_property(&b.properties),
            "MapObjectId must still be a Property::Name, not collapsed to another variant"
        );
    }
}

#[test]
fn json_round_trip_survives_a_re_encode_to_gvas_bytes() {
    let session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);
    let original =
        capture::capture(&session, base_id, CaptureOptions::full(), "Home").expect("capture");

    let direct = gvas::to_psp_bytes(&original).expect("psp encode");
    let via_json = gvas::from_json(&gvas::to_json(&original).expect("json encode"))
        .expect("json decode");
    let round_tripped = gvas::to_psp_bytes(&via_json).expect("psp encode after json");

    assert_eq!(
        direct, round_tripped,
        "a json round trip must not change the bytes the blueprint encodes to"
    );
}

#[test]
fn psp_round_trip_preserves_the_blueprint() {
    let session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);
    let original =
        capture::capture(&session, base_id, CaptureOptions::full(), "Home").expect("capture");

    let bytes = gvas::to_psp_bytes(&original).expect("blueprint must serialize to psp bytes");
    let restored = gvas::from_psp_bytes(&bytes).expect("psp bytes must parse back");

    assert_eq!(restored.header.name, original.header.name);
    assert_eq!(restored.structures.len(), original.structures.len());
    assert_eq!(restored.structures[0].map_object_id, original.structures[0].map_object_id);
    assert_eq!(
        restored.structures[0].relative_transform.translation.x.0,
        original.structures[0].relative_transform.translation.x.0
    );
}

#[test]
fn psp_and_json_encodings_agree() {
    let original = captured_fixture_blueprint();

    let from_psp =
        gvas::from_psp_bytes(&gvas::to_psp_bytes(&original).expect("psp encode")).expect("psp decode");
    let from_json =
        gvas::from_json(&gvas::to_json(&original).expect("json encode")).expect("json decode");

    assert_eq!(from_psp.structures.len(), from_json.structures.len());
    assert_eq!(from_psp.header.manifest, from_json.header.manifest);
    // Lengths and the manifest agreed even while `.psp` was decoding every
    // payload as an opaque byte array. The type view is what actually differs.
    assert_eq!(
        typed_counts(&from_psp),
        typed_counts(&from_json),
        "both encodings must decode to the same typed Palworld structs, not just the same counts of rows"
    );
}

/// A blueprint decoded from `.psp` must be as PLACEABLE as one decoded from
/// JSON: every `RawData` still a typed Palworld struct, so `capture`'s and
/// `transform`'s `StructValue::Game(..)` matches still fire.
#[test]
fn a_psp_round_trip_keeps_every_typed_palworld_struct() {
    let original = captured_fixture_blueprint();
    let expected = typed_counts(&original);

    assert!(
        expected.models > 0 && expected.concrete_models > 0 && expected.works > 0 && expected.characters > 0,
        "setup: the fixture must carry typed structs of every kind, got {expected:?}"
    );

    let from_psp =
        gvas::from_psp_bytes(&gvas::to_psp_bytes(&original).expect("psp encode")).expect("psp decode");
    let from_json =
        gvas::from_json(&gvas::to_json(&original).expect("json encode")).expect("json decode");

    assert_eq!(typed_counts(&from_json), expected, "json decode must preserve typed structs");
    assert_eq!(typed_counts(&from_psp), expected, "psp decode must preserve typed structs");

    assert!(
        capture::first_build_player_uid(&from_psp).is_some(),
        "a psp-decoded blueprint must still expose its typed Model.RawData"
    );
    assert_eq!(
        capture::first_build_player_uid(&from_psp),
        capture::first_build_player_uid(&from_json),
    );
}

/// uesave picks `Byte::Byte(u8)` vs `Byte::Label(String)` from the tag's enum
/// type alone, but writes whichever variant it holds. A labelled byte tagged
/// as untyped therefore writes a string and reads back one byte, misaligning
/// the whole stream -- a blueprint that can never be loaded again.
#[test]
fn a_labelled_byte_property_survives_both_encodings() {
    let mut original = captured_fixture_blueprint();
    original.structures[0]
        .properties
        .insert("PspLabelledByte", Property::Byte(Byte::Label("EPal::Foo".to_string())));

    let from_psp =
        gvas::from_psp_bytes(&gvas::to_psp_bytes(&original).expect("psp encode")).expect("psp decode");
    let from_json =
        gvas::from_json(&gvas::to_json(&original).expect("json encode")).expect("json decode");

    for (label, restored) in [("psp", &from_psp), ("json", &from_json)] {
        let value = restored.structures[0]
            .properties
            .0
            .get(&PropertyKey::from("PspLabelledByte"));
        assert!(
            matches!(value, Some(Property::Byte(Byte::Label(l))) if l == "EPal::Foo"),
            "{label} decode must keep the byte labelled, got: {value:?}"
        );
    }
}

/// Two conflicting REAL observations of one path cannot both be encoded: the
/// single schema tag there decides how every value at that path decodes. A
/// `Name` silently returning as a `Str` is the failure class this format
/// exists to prevent, so encoding must refuse rather than pick a winner.
#[test]
fn two_irreconcilable_types_at_one_path_are_refused_at_encode_time() {
    let mut blueprint = captured_fixture_blueprint();
    assert!(blueprint.structures.len() > 1, "setup: need two structures to collide");
    blueprint.structures[0]
        .properties
        .insert("PspCollision", Property::Name("AAA".to_string()));
    blueprint.structures[1]
        .properties
        .insert("PspCollision", Property::Str("BBB".to_string()));

    let message = gvas::to_psp_bytes(&blueprint)
        .expect_err("a name/str collision at one path must not encode")
        .to_string();

    assert!(message.contains("PspCollision"), "the error must name the path, got: {message}");
    assert!(
        message.contains("NameProperty") && message.contains("StrProperty"),
        "the error must name both tags, got: {message}"
    );
}

/// The weak/strong half of the merge policy is load-bearing and must not
/// regress: the fixture's `ConcreteModel.RawData` is a typed
/// `PalMapConcreteModel` on some structures and an unparsed byte array on the
/// rest, and the typed observation has to win.
#[test]
fn an_unparsed_raw_data_sibling_does_not_erase_a_typed_one() {
    let original = captured_fixture_blueprint();
    let typed = typed_counts(&original);

    assert!(
        typed.concrete_models < original.structures.len(),
        "setup: the fixture must mix typed and raw ConcreteModel.RawData"
    );

    let restored =
        gvas::from_psp_bytes(&gvas::to_psp_bytes(&original).expect("psp encode")).expect("psp decode");

    assert_eq!(typed_counts(&restored).concrete_models, typed.concrete_models);
}

#[test]
fn a_file_without_the_magic_header_is_rejected_clearly() {
    let result = gvas::from_psp_bytes(b"not a blueprint at all");

    let message = result.expect_err("garbage must not parse").to_string();
    assert!(message.contains("blueprint"), "the error must name the format, got: {message}");
}

#[test]
fn a_container_written_by_a_newer_schema_is_refused() {
    let mut bytes = gvas::PSP_MAGIC.to_vec();
    bytes.extend_from_slice(&(SCHEMA_VERSION + 1).to_le_bytes());
    bytes.extend_from_slice(b"whatever body follows");

    let message = gvas::from_psp_bytes(&bytes)
        .expect_err("a newer container version must be refused, not misread")
        .to_string();

    assert!(
        message.contains("newer than supported"),
        "the error must say the file is too new, got: {message}"
    );
}

#[test]
fn json_from_a_save_that_is_not_a_blueprint_is_refused() {
    let original = empty_blueprint();
    let mut save = gvas::to_save(&original).expect("to_save");
    save.root.save_game_type = "PalWorldSaveGame".to_string();
    let json = serde_json::to_string(&save).expect("json encode");

    let message = gvas::from_json(&json)
        .expect_err("a foreign save must not decode as a blueprint")
        .to_string();

    assert!(
        message.contains("PspBaseBlueprint"),
        "the error must name the expected format marker, got: {message}"
    );
}

