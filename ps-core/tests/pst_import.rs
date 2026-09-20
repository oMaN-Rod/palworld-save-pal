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

mod common;

use ps_core::domain::blueprint::{capture, BlueprintStructure, CaptureOptions};
use std::collections::BTreeSet;

/// The third-party exporter drops these object types from its own payload by design; its
/// source applies this same rule in three separate places when it builds `map_objects`:
/// plain `PalBooth`/`ItemBooth`, and any `PalEgg*` that is neither hatching nor an
/// incubator. No conversion could ever produce them on the imported side, so every
/// comparison below subtracts exactly what this rule accounts for -- and nothing else --
/// before comparing the two roads.
fn exporter_excludes(map_object_id: &str) -> bool {
    map_object_id == "PalBooth"
        || map_object_id == "ItemBooth"
        || (map_object_id.starts_with("PalEgg")
            && !map_object_id.contains("Hatching")
            && !map_object_id.contains("Incubator"))
}

/// The centrepiece. A PST export of a base and a native capture of that same base are
/// two roads to the same object; where they disagree, the mapping table is wrong.
///
/// This is what catches a systematic fault such as palworld-save-tools and uesave
/// disagreeing on FGuid byte order, which would otherwise surface as scrambled guids in
/// a user's save long after the cause.
///
/// `capture_unscrubbed` needs `ps-core`'s `test-fixtures` feature; this workspace already
/// enables it for every integration test via the `[dev-dependencies]` self-dependency in
/// `ps-core/Cargo.toml`, but run with `--features test-fixtures` explicitly too:
/// `cargo test -p ps-core --features test-fixtures --test pst_import`.
#[test]
fn a_pst_import_matches_a_native_capture_of_the_same_base() {
    let session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);
    let captured = capture::capture_unscrubbed(&session, base_id, CaptureOptions::full(), "Home")
        .expect("native capture");
    let imported = pst::import(&fixture("v1_relics_base.json"), "Home")
        .expect("pst import")
        .blueprint;

    // Every structure the exporter's own rule would drop, set aside before comparing --
    // not a tolerance, but matching the payload the import actually received.
    let expected_structures: Vec<&BlueprintStructure> =
        captured.structures.iter().filter(|s| !exporter_excludes(&s.map_object_id)).collect();
    assert_eq!(
        imported.structures.len(),
        expected_structures.len(),
        "both roads must find the same structures, once the exporter's own exclusions are set aside"
    );

    let ids = |bp: &ps_core::domain::blueprint::BaseBlueprint| -> Vec<String> {
        let mut out: Vec<String> =
            bp.structures.iter().map(|s| s.map_object_id.clone()).collect();
        out.sort();
        out
    };
    let mut expected_ids: Vec<String> =
        expected_structures.iter().map(|s| s.map_object_id.clone()).collect();
    expected_ids.sort();
    assert_eq!(
        ids(&imported),
        expected_ids,
        "the same structure types, in the same counts, once excluded types are set aside"
    );

    // Relative transforms are computed identically by both paths, so they must agree to
    // within float noise rather than merely being "close".
    let mut captured_by_id: Vec<_> = expected_structures
        .iter()
        .map(|s| (s.map_object_id.clone(), s.relative_transform.clone()))
        .collect();
    captured_by_id.sort_by(|a, b| a.0.cmp(&b.0));
    let mut imported_by_id: Vec<_> = imported
        .structures
        .iter()
        .map(|s| (s.map_object_id.clone(), s.relative_transform.clone()))
        .collect();
    imported_by_id.sort_by(|a, b| a.0.cmp(&b.0));

    for ((id, want), (_, got)) in captured_by_id.iter().zip(&imported_by_id) {
        for (axis, want, got) in [
            ("x", want.translation.x.0, got.translation.x.0),
            ("y", want.translation.y.0, got.translation.y.0),
            ("z", want.translation.z.0, got.translation.z.0),
        ] {
            assert!(
                (want - got).abs() < 0.01,
                "{id} relative {axis}: captured {want}, imported {got}"
            );
        }
    }
}

/// Guid-bearing collections must line up exactly. A byte-order fault shows here as two
/// disjoint sets rather than as a near miss.
///
/// The exporter's exclusion rule (see `exporter_excludes`) removes item containers reachable
/// *only* through an excluded structure; a container still reached by some other,
/// non-excluded structure must still be present, so this does not merely filter the
/// captured side -- it subtracts exactly the ids the exclusion accounts for.
///
/// Work entries get no such treatment: the exporter's exclusion rule only ever filters
/// `map_objects`, and a booth's own `EPalWorkableType::Booth` work record still travels in
/// the payload even though the booth's map object does not. So the two sides' work ids
/// must match exactly, with nothing subtracted.
#[test]
fn a_pst_import_carries_the_same_container_and_work_ids_as_a_capture() {
    let session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);
    let captured = capture::capture_unscrubbed(&session, base_id, CaptureOptions::full(), "Home")
        .expect("native capture");
    let imported = pst::import(&fixture("v1_relics_base.json"), "Home")
        .expect("pst import")
        .blueprint;

    let container_ids = |bp: &ps_core::domain::blueprint::BaseBlueprint| -> BTreeSet<uuid::Uuid> {
        bp.item_containers.iter().filter_map(capture::container_entry_id).collect()
    };
    let excluded_container_ids: BTreeSet<uuid::Uuid> = captured
        .structures
        .iter()
        .filter(|s| exporter_excludes(&s.map_object_id))
        .flat_map(|s| capture::module_target_container_ids(&s.properties).0)
        .collect();
    let included_container_ids: BTreeSet<uuid::Uuid> = captured
        .structures
        .iter()
        .filter(|s| !exporter_excludes(&s.map_object_id))
        .flat_map(|s| capture::module_target_container_ids(&s.properties).0)
        .collect();
    let exclusively_excluded_container_ids: BTreeSet<uuid::Uuid> =
        excluded_container_ids.difference(&included_container_ids).copied().collect();
    let expected_container_ids: BTreeSet<uuid::Uuid> = container_ids(&captured)
        .difference(&exclusively_excluded_container_ids)
        .copied()
        .collect();
    assert_eq!(
        container_ids(&imported),
        expected_container_ids,
        "item container ids must match exactly, once containers reachable only through an \
         excluded structure are set aside"
    );

    let work_ids = |bp: &ps_core::domain::blueprint::BaseBlueprint| -> BTreeSet<uuid::Uuid> {
        bp.works.iter().filter_map(capture::work_base_id).collect()
    };
    assert_eq!(work_ids(&imported), work_ids(&captured), "work ids must match exactly");
}

/// Both encodings must import identically; otherwise one of them is lossy.
#[test]
fn both_encodings_import_to_the_same_blueprint() {
    let from_json = pst::import(&fixture("v1_relics_base.json"), "Home").expect("json").blueprint;
    let from_pstbase =
        pst::import(&fixture("v1_relics_base.pstbase"), "Home").expect("pstbase").blueprint;
    assert_eq!(from_json.structures.len(), from_pstbase.structures.len());
    assert_eq!(from_json.header.footprint_radius, from_pstbase.header.footprint_radius);
}

/// Leniency must never cost integrity: a structure whose container was dropped is
/// dropped too, because at placement it would dereference something that is not there.
#[test]
fn a_structure_whose_container_is_missing_is_dropped_with_a_warning() {
    let mut imported = pst::import(&fixture("v1_relics_base.json"), "Home").expect("imports");
    let before = imported.blueprint.structures.len();

    // Remove every item container, stranding whatever referenced them.
    imported.blueprint.item_containers.clear();
    let mut findings = Vec::new();
    ps_core::domain::blueprint::pst::reconcile::reconcile(&mut imported.blueprint, &mut findings);

    assert!(
        imported.blueprint.structures.len() < before,
        "structures referencing a missing container must be dropped"
    );
    let finding = findings
        .iter()
        .find(|f| f.code == "pst.structure_dropped_missing_container")
        .expect("the drop is reported");
    assert!(
        finding.message.contains(&(before - imported.blueprint.structures.len()).to_string()),
        "the finding should carry a count, got: {}",
        finding.message
    );
}

/// One finding per rule, not one per object: the corpus has an 8,120-object sample.
#[test]
fn findings_are_aggregated_rather_than_one_per_object() {
    let mut imported = pst::import(&fixture("v1_relics_base.json"), "Home").expect("imports");
    imported.blueprint.item_containers.clear();
    let mut findings = Vec::new();
    ps_core::domain::blueprint::pst::reconcile::reconcile(&mut imported.blueprint, &mut findings);

    let drops = findings
        .iter()
        .filter(|f| f.code == "pst.structure_dropped_missing_container")
        .count();
    assert_eq!(drops, 1, "one aggregated finding per rule");
}

/// A blueprint with nothing left in it is a failure, not a silent empty import.
#[test]
fn a_blueprint_reduced_to_nothing_is_blocking() {
    let mut imported = pst::import(&fixture("v1_relics_base.json"), "Home").expect("imports");
    imported.blueprint.structures.clear();
    let mut findings = Vec::new();
    ps_core::domain::blueprint::pst::reconcile::reconcile(&mut imported.blueprint, &mut findings);

    assert!(
        findings.iter().any(|f| f.code == "pst.no_structures"
            && matches!(f.severity, Severity::Blocking)),
        "an empty blueprint must block"
    );
}

/// The happy path must stay clean, or the rules are too eager.
#[test]
fn a_healthy_import_produces_no_blocking_findings() {
    let imported = pst::import(&fixture("v1_relics_base.json"), "Home").expect("imports");
    let blocking: Vec<_> = imported
        .findings
        .iter()
        .filter(|f| matches!(f.severity, Severity::Blocking))
        .collect();
    assert!(blocking.is_empty(), "unexpected blocking findings: {blocking:?}");
}

/// A cleared slot must actually stop referencing the missing dynamic item, not merely
/// get reported -- placement would otherwise dereference a `DynamicItemSaveData` entry
/// that this same pass just decided did not exist.
#[test]
fn a_container_slot_pointing_at_a_missing_dynamic_item_is_cleared_with_a_warning() {
    let mut imported = pst::import(&fixture("v1_relics_base.json"), "Home").expect("imports");
    let referenced_before: usize = imported
        .blueprint
        .item_containers
        .iter()
        .map(|entry| capture::container_slot_dynamic_item_ids(entry).len())
        .sum();
    assert!(referenced_before > 0, "fixture must exercise at least one occupied slot");

    imported.blueprint.dynamic_items.clear();
    let mut findings = Vec::new();
    pst::reconcile::reconcile(&mut imported.blueprint, &mut findings);

    findings
        .iter()
        .find(|f| f.code == "pst.slot_cleared_missing_dynamic_item")
        .expect("the clear is reported");

    let referenced_after: usize = imported
        .blueprint
        .item_containers
        .iter()
        .map(|entry| capture::container_slot_dynamic_item_ids(entry).len())
        .sum();
    assert_eq!(referenced_after, 0, "no slot may still reference a dropped dynamic item");
}
