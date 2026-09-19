use ps_core::domain::blueprint::pst;
use ps_core::domain::blueprint::validate::Severity;

/// zstd's frame magic, which is how a .pstbase is told from a .json.
const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];

#[test]
fn a_zstd_frame_is_recognized_as_a_pstbase() {
    assert!(pst::envelope::is_pstbase(&ZSTD_MAGIC));
    assert!(!pst::envelope::is_pstbase(br#"{"base_camp":{}}"#));
    assert!(!pst::envelope::is_pstbase(&[]));
}

#[test]
fn plain_json_decodes_to_its_own_tree() {
    let decoded = pst::envelope::decode(br#"{"base_camp_level":35}"#).expect("decodes");
    assert_eq!(decoded["base_camp_level"], serde_json::json!(35));
}

/// CBOR byte strings and JSON integer arrays must produce the same tree, so that
/// nothing downstream has to know which encoding it came from.
#[test]
fn cbor_byte_strings_normalize_to_integer_arrays() {
    let cbor_payload = {
        let value = serde_json::json!({ "trailing_bytes": [1, 0, 0, 0] });
        // Round-trip through CBOR so the bytes arrive as a CBOR byte string.
        let mut buffer = Vec::new();
        ciborium::into_writer(&value, &mut buffer).expect("cbor encodes");
        buffer
    };
    let from_cbor = pst::envelope::decode_cbor(&cbor_payload).expect("decodes");
    let from_json =
        pst::envelope::decode(br#"{"trailing_bytes":[1,0,0,0]}"#).expect("decodes");
    assert_eq!(from_cbor, from_json);
}

/// Some real `.json` exports (e.g. Purple Lake 3) wrap byte arrays as
/// `{"~b": "<base64>"}` instead of inlining them as an integer array. This must
/// normalize to the same tree as the inline form. `AAAAAA==` is the base64 this
/// sample actually uses for a four-zero-byte `trailing_bytes`.
#[test]
fn base64_wrapped_bytes_normalize_to_integer_arrays() {
    let wrapped =
        pst::envelope::decode(br#"{"trailing_bytes":{"~b":"AAAAAA=="}}"#).expect("decodes");
    let inline =
        pst::envelope::decode(br#"{"trailing_bytes":[0,0,0,0]}"#).expect("decodes");
    assert_eq!(wrapped, inline);
}

/// An object with a `~b` key alongside other keys is an ordinary object that happens
/// to have a field named `~b`, not the base64-wrapped-bytes marker -- it must pass
/// through unchanged rather than being mistaken for one.
#[test]
fn a_multi_key_object_containing_tilde_b_is_left_alone() {
    let decoded =
        pst::envelope::decode(br#"{"~b":"AAAAAA==","other":1}"#).expect("decodes");
    assert_eq!(decoded, serde_json::json!({"~b": "AAAAAA==", "other": 1}));
}

#[test]
fn an_oversized_payload_is_refused_rather_than_exhausting_memory() {
    // A frame claiming far more than the cap must fail before allocating it. Either
    // our own cap refuses it, or ruzstd refuses the synthetic frame first for its own
    // reasons (it is not a fully valid frame past the size header) — both are a refusal
    // rather than an attempt to allocate the claimed size, which is what this guards.
    let bomb = pst::envelope::test_support::zstd_frame_claiming(
        pst::envelope::MAX_DECOMPRESSED_BYTES + 1,
    );
    let err = pst::envelope::decode(&bomb).expect_err("refuses an oversized payload");
    let message = err.to_string();
    assert!(
        message.contains("too large") || message.contains("zstd decode failed"),
        "the error should name the size problem or ruzstd's own refusal, got: {err}"
    );
}

#[test]
fn a_file_that_is_neither_encoding_is_refused() {
    assert!(pst::envelope::decode(b"not a blueprint").is_err());
}

/// Reads a fixture, transparently gunzipping it if only a `.gz` sibling is committed.
fn fixture(name: &str) -> Vec<u8> {
    let dir =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/pst");
    let gz_path = dir.join(format!("{name}.gz"));
    if gz_path.exists() {
        let compressed = std::fs::read(&gz_path)
            .unwrap_or_else(|e| panic!("read fixture {}: {e}", gz_path.display()));
        let mut decoded = Vec::new();
        std::io::Read::read_to_end(
            &mut flate2::read::GzDecoder::new(compressed.as_slice()),
            &mut decoded,
        )
        .unwrap_or_else(|e| panic!("gunzip fixture {}: {e}", gz_path.display()));
        return decoded;
    }
    let path = dir.join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read fixture {}: {e}", path.display()))
}

/// The two encodings of one base must normalize to the identical tree. This is the
/// contract that lets one assembler serve both.
#[test]
fn both_fixture_encodings_decode_to_the_same_tree() {
    let from_json = pst::envelope::decode(&fixture("v1_relics_base.json")).expect("json decodes");
    let from_pstbase =
        pst::envelope::decode(&fixture("v1_relics_base.pstbase")).expect("pstbase decodes");
    assert_eq!(from_json, from_pstbase);
}

#[test]
fn the_fixture_carries_the_sections_the_assembler_needs() {
    let payload = pst::envelope::decode(&fixture("v1_relics_base.json")).expect("decodes");
    for section in ["base_camp", "base_camp_level", "map_objects"] {
        assert!(payload.get(section).is_some(), "fixture is missing {section}");
    }
}

#[test]
fn importing_the_fixture_produces_structures_at_relative_positions() {
    let imported = pst::import(&fixture("v1_relics_base.json"), "v1_relics_base").expect("imports");
    assert!(
        !imported.blueprint.structures.is_empty(),
        "the fixture base has structures"
    );
    assert_eq!(
        imported.blueprint.header.structure_count as usize,
        imported.blueprint.structures.len()
    );
    // Relative, not absolute: a base camp's own structures sit near its anchor, never
    // at the world coordinates PST stored.
    let furthest = imported
        .blueprint
        .structures
        .iter()
        .map(|s| s.relative_transform.translation.x.0.abs())
        .fold(0.0f64, f64::max);
    assert!(furthest < 100_000.0, "structures should be anchor-relative, got {furthest}");
}

#[test]
fn an_import_declares_a_full_manifest_and_an_unnamed_source() {
    let imported = pst::import(&fixture("v1_relics_base.json"), "v1_relics_base").expect("imports");
    let header = &imported.blueprint.header;
    assert_eq!(header.schema_version, 1, "no schema bump");
    assert_eq!(header.manifest, ps_core::domain::blueprint::CaptureOptions::full());
    assert_eq!(header.name, "v1_relics_base");
    assert_eq!(header.source_world, "", "identity is scrubbed");
    assert_eq!(header.source_base, "");
}

/// PalStudio's blueprint has nowhere to put the guild's base level.
#[test]
fn the_base_camp_level_is_dropped_with_a_warning() {
    let imported = pst::import(&fixture("v1_relics_base.json"), "v1_relics_base").expect("imports");
    let warning = imported
        .findings
        .iter()
        .find(|f| f.code == "pst.base_camp_level_dropped")
        .expect("base camp level is reported as dropped");
    assert!(matches!(warning.severity, Severity::Warning));
}

/// These files are shared publicly; an import must carry no stranger's identity.
#[test]
fn an_import_is_scrubbed_like_a_native_capture() {
    use ps_core::ue::{PalStruct, Property, PropertyKey, StructValue};

    let imported = pst::import(&fixture("v1_relics_base.json"), "v1_relics_base").expect("imports");
    for structure in &imported.blueprint.structures {
        let Some(model) = structure.properties.0.get(&PropertyKey::from("Model")) else {
            continue;
        };
        let Some(model) = ps_core::props::struct_props(model) else { continue };
        if let Some(Property::Struct(StructValue::Game(PalStruct::MapModel(raw)))) =
            model.0.get(&PropertyKey::from("RawData"))
        {
            assert!(raw.build_player_uid.is_nil(), "build_player_uid must be scrubbed");
            assert!(raw.group_id_belong_to.is_nil(), "group_id_belong_to must be scrubbed");
        }
    }
}

/// A payload PST itself would call outdated is refused, not half-imported.
#[test]
fn an_outdated_payload_is_refused() {
    let err = pst::import(br#"{"base_camp":{},"map_objects":[]}"#, "old").expect_err("refuses");
    assert!(
        err.to_string().contains("re-export"),
        "the error should tell the user to re-export, got: {err}"
    );
}
