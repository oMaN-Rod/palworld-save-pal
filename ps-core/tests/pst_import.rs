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

/// A structure's unique identity, for pairing two roads' output to the *same* structure
/// rather than merely the same type -- `map_object_id` is a type name shared by every
/// wall or floor of that kind, so sorting or zipping on it alone can pair up two
/// different structures that happen to share a type and never notice.
fn model_instance_id(properties: &ps_core::ue::Properties) -> Option<uuid::Uuid> {
    let model = properties
        .0
        .get(&ps_core::ue::PropertyKey::from("Model"))
        .and_then(ps_core::props::struct_props)?;
    match model.0.get(&ps_core::ue::PropertyKey::from("RawData"))? {
        ps_core::ue::Property::Struct(ps_core::ue::StructValue::Game(
            ps_core::ue::PalStruct::MapModel(raw),
        )) => Some(ps_core::props::guid_to_uuid(&raw.instance_id)),
        _ => None,
    }
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
    // within float noise rather than merely being "close". Paired by model instance id
    // (unique per structure) rather than `map_object_id` (a type name shared by every
    // structure of that kind), and the id itself is asserted equal inside the zip so a
    // divergent ordering between the two roads fails as exactly that -- an ordering
    // mismatch -- rather than a fabricated transform mismatch between two unrelated
    // same-typed structures.
    let mut captured_by_id: Vec<_> = expected_structures
        .iter()
        .filter_map(|s| {
            model_instance_id(&s.properties)
                .map(|id| (id, s.map_object_id.clone(), s.relative_transform.clone()))
        })
        .collect();
    captured_by_id.sort_by_key(|(id, _, _)| *id);
    let mut imported_by_id: Vec<_> = imported
        .structures
        .iter()
        .filter_map(|s| {
            model_instance_id(&s.properties)
                .map(|id| (id, s.map_object_id.clone(), s.relative_transform.clone()))
        })
        .collect();
    imported_by_id.sort_by_key(|(id, _, _)| *id);
    assert_eq!(
        captured_by_id.len(),
        expected_structures.len(),
        "every expected structure must carry a model instance id"
    );
    assert_eq!(
        imported_by_id.len(),
        imported.structures.len(),
        "every imported structure must carry a model instance id"
    );

    for ((want_id, id, want), (got_id, _, got)) in captured_by_id.iter().zip(&imported_by_id) {
        assert_eq!(
            want_id, got_id,
            "{id} at this position: captured instance {want_id}, imported instance {got_id} -- \
             the two roads disagree on which structure this is, not merely its transform"
        );
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

    // Structure count and footprint radius alone would pass even if the two encodings
    // normalized to different structures wearing the same totals. Compare the actual
    // per-structure geometry, paired by model instance id rather than the shared type
    // name `map_object_id`, so a real divergence in "all encodings normalize to one
    // representation" cannot hide behind two equal-looking numbers.
    let sorted_geometry = |bp: &ps_core::domain::blueprint::BaseBlueprint| -> Vec<_> {
        let mut geometry: Vec<_> = bp
            .structures
            .iter()
            .filter_map(|s| {
                model_instance_id(&s.properties)
                    .map(|id| (id, s.map_object_id.clone(), s.relative_transform.clone()))
            })
            .collect();
        geometry.sort_by_key(|(id, _, _)| *id);
        geometry.into_iter().map(|(_, map_object_id, transform)| (map_object_id, transform)).collect()
    };
    let json_geometry = sorted_geometry(&from_json);
    let pstbase_geometry = sorted_geometry(&from_pstbase);
    assert_eq!(json_geometry.len(), from_json.structures.len());
    assert_eq!(pstbase_geometry.len(), from_pstbase.structures.len());
    assert_eq!(
        json_geometry, pstbase_geometry,
        "both encodings must produce the same structure types at the same relative transforms"
    );
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
    ps_core::domain::blueprint::pst::reconcile::reconcile(&mut imported.blueprint, before, &mut findings);

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

    let remaining_item_targets: usize = imported
        .blueprint
        .structures
        .iter()
        .map(|s| capture::module_target_container_ids(&s.properties).0.len())
        .sum();
    assert_eq!(
        remaining_item_targets, 0,
        "no surviving structure may still reference a missing item container"
    );
}

/// One finding per rule, not one per object: the corpus has an 8,120-object sample.
#[test]
fn findings_are_aggregated_rather_than_one_per_object() {
    let mut imported = pst::import(&fixture("v1_relics_base.json"), "Home").expect("imports");
    let before = imported.blueprint.structures.len();
    imported.blueprint.item_containers.clear();
    let mut findings = Vec::new();
    ps_core::domain::blueprint::pst::reconcile::reconcile(&mut imported.blueprint, before, &mut findings);

    let drops = findings
        .iter()
        .filter(|f| f.code == "pst.structure_dropped_missing_container")
        .count();
    assert_eq!(drops, 1, "one aggregated finding per rule");
}

/// A base that had structures, none of which survived, is a failure -- leniency lost
/// everything the source actually had.
#[test]
fn a_blueprint_reduced_to_nothing_is_blocking() {
    let mut imported = pst::import(&fixture("v1_relics_base.json"), "Home").expect("imports");
    let before = imported.blueprint.structures.len();
    assert!(before > 0, "fixture must start with structures for this test to mean anything");
    imported.blueprint.structures.clear();
    let mut findings = Vec::new();
    ps_core::domain::blueprint::pst::reconcile::reconcile(&mut imported.blueprint, before, &mut findings);

    assert!(
        findings.iter().any(|f| f.code == "pst.no_structures"
            && matches!(f.severity, Severity::Blocking)),
        "losing every structure the source had must block"
    );
}

/// A base that had no structures to begin with is not a failure: it is a faithfully
/// imported empty base (e.g. a genuinely empty corpus sample). It must warn, not block,
/// so the import still succeeds.
#[test]
fn a_source_with_no_structures_at_all_only_warns() {
    let mut imported = pst::import(&fixture("v1_relics_base.json"), "Home").expect("imports");
    imported.blueprint.structures.clear();
    let mut findings = Vec::new();
    ps_core::domain::blueprint::pst::reconcile::reconcile(&mut imported.blueprint, 0, &mut findings);

    let finding = findings
        .iter()
        .find(|f| f.code == "pst.no_structures")
        .expect("an empty result is still reported");
    assert!(
        matches!(finding.severity, Severity::Warning),
        "a source that never had structures must not block, got {:?}",
        finding.severity
    );
    assert!(
        !findings.iter().any(|f| matches!(f.severity, Severity::Blocking)),
        "nothing about a genuinely empty source should block the import"
    );
}

/// The regression this discriminator exists to catch: a source that genuinely held map
/// objects, every one of which failed to decode, must not be indistinguishable from a
/// source that never had any. This is the real corpus failure mode (a dialect gap took
/// three full samples to zero structures) and it must still block after the fix.
#[test]
fn a_payload_whose_structures_all_fail_to_decode_blocks_the_import() {
    let mut payload = pst::envelope::decode(&fixture("v1_relics_base.json")).expect("decodes");
    let map_objects = payload
        .get_mut("map_objects")
        .and_then(serde_json::Value::as_array_mut)
        .expect("fixture has a map_objects array");
    let original_count = map_objects.len();
    assert!(original_count > 0, "fixture must carry real map objects for this test to mean anything");
    for entry in map_objects.iter_mut() {
        *entry = serde_json::Value::Null;
    }
    let bytes = serde_json::to_vec(&payload).expect("re-serializes");

    let imported = pst::import(&bytes, "Home").expect("the envelope itself still decodes");

    assert!(imported.blueprint.structures.is_empty(), "every structure must have failed to decode");
    let structure_dropped: Vec<_> =
        imported.findings.iter().filter(|f| f.code == "pst.structure_dropped").collect();
    assert_eq!(
        structure_dropped.len(),
        1,
        "one aggregated finding per rule, not one per corrupted map object"
    );
    assert!(
        structure_dropped[0].message.contains(&original_count.to_string()),
        "the aggregated finding should carry the count, got: {}",
        structure_dropped[0].message
    );

    let finding = imported
        .findings
        .iter()
        .find(|f| f.code == "pst.no_structures")
        .expect("the empty result is still reported");
    assert!(
        matches!(finding.severity, Severity::Blocking),
        "a source that had structures which all failed to decode must block, got {:?}",
        finding.severity
    );
}

/// Pins the boundary from the other side: some decode failures among survivors must not
/// trip the empty-result rule at all.
#[test]
fn a_payload_whose_structures_partially_fail_to_decode_does_not_block() {
    let mut payload = pst::envelope::decode(&fixture("v1_relics_base.json")).expect("decodes");
    let map_objects = payload
        .get_mut("map_objects")
        .and_then(serde_json::Value::as_array_mut)
        .expect("fixture has a map_objects array");
    assert!(map_objects.len() > 1, "fixture must carry more than one map object for this test");
    map_objects[0] = serde_json::Value::Null;
    let bytes = serde_json::to_vec(&payload).expect("re-serializes");

    let imported = pst::import(&bytes, "Home").expect("imports");

    assert!(!imported.blueprint.structures.is_empty(), "the surviving structures must remain");
    assert!(
        imported.findings.iter().any(|f| f.code == "pst.structure_dropped"),
        "the corrupted entry must still be reported"
    );
    assert!(
        !imported.findings.iter().any(|f| f.code == "pst.no_structures"),
        "structures survived, so the empty-result rule must not fire at all"
    );
}

/// The other boundary: a source that never had any map objects to begin with must warn,
/// not block, exercised through the real import path rather than `reconcile` directly.
#[test]
fn a_payload_with_no_map_objects_at_all_only_warns() {
    let mut payload = pst::envelope::decode(&fixture("v1_relics_base.json")).expect("decodes");
    payload["map_objects"] = serde_json::Value::Array(Vec::new());
    let bytes = serde_json::to_vec(&payload).expect("re-serializes");

    let imported = pst::import(&bytes, "Home").expect("imports");

    assert!(imported.blueprint.structures.is_empty());
    let finding = imported
        .findings
        .iter()
        .find(|f| f.code == "pst.no_structures")
        .expect("the empty result is still reported");
    assert!(
        matches!(finding.severity, Severity::Warning),
        "a source with no map objects at all must not block, got {:?}",
        finding.severity
    );
    assert!(
        !imported.findings.iter().any(|f| matches!(f.severity, Severity::Blocking)),
        "nothing about a genuinely empty source should block the import"
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
    let source_structure_count = imported.blueprint.structures.len();
    let mut findings = Vec::new();
    pst::reconcile::reconcile(&mut imported.blueprint, source_structure_count, &mut findings);

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

/// Pins the schema priming for the two structs `assemble` re-types from a sibling field
/// (see the module doc on `pst::assemble`): `PalMapConcreteModelModule::module_type` and
/// `PalWork::work_type` both come back empty from the generic decode and are patched in
/// afterward. If the psbp encoder does not know these variants exist, or the decoder does
/// not prime them the same way, that patched-in type tag is exactly the kind of thing an
/// encode/decode cycle could silently drop -- writing back a blueprint that will not parse.
#[test]
fn an_import_round_trips_through_gvas_encoding_unchanged() {
    use ps_core::domain::blueprint::gvas;
    use ps_core::ue::{PalStruct, Property, PropertyKey, StructValue};

    let imported = pst::import(&fixture("v1_relics_base.json"), "Home").expect("imports").blueprint;

    let bytes = gvas::to_psbp_bytes(&imported).expect("psbp encode");
    let restored = gvas::from_psbp_bytes(&bytes).expect("psbp decode");

    assert_eq!(restored.structures.len(), imported.structures.len());
    assert_eq!(restored.header.name, imported.header.name);
    assert_eq!(restored.header.footprint_radius, imported.header.footprint_radius);

    fn module_types(bp: &ps_core::domain::blueprint::BaseBlueprint) -> Vec<String> {
        let mut out = Vec::new();
        for structure in &bp.structures {
            let Some(concrete) = structure
                .properties
                .0
                .get(&PropertyKey::from("ConcreteModel"))
                .and_then(ps_core::props::struct_props)
            else {
                continue;
            };
            let Some(entries) = concrete
                .0
                .get(&PropertyKey::from("ModuleMap"))
                .and_then(ps_core::props::map_entries)
            else {
                continue;
            };
            for entry in entries {
                let Some(props) = ps_core::props::struct_props(&entry.value) else { continue };
                if let Some(Property::Struct(StructValue::Game(PalStruct::MapConcreteModelModule(
                    raw,
                )))) = props.0.get(&PropertyKey::from("RawData"))
                {
                    out.push(raw.module_type.clone());
                }
            }
        }
        out
    }
    let imported_module_types = module_types(&imported);
    assert!(!imported_module_types.is_empty(), "fixture must exercise typed concrete-model modules");
    assert_eq!(
        module_types(&restored),
        imported_module_types,
        "module_type must survive an encode/decode cycle"
    );
    assert!(
        module_types(&restored).iter().all(|t| !t.is_empty()),
        "module_type must not come back blanked"
    );

    fn work_types(bp: &ps_core::domain::blueprint::BaseBlueprint) -> Vec<String> {
        bp.works
            .iter()
            .filter_map(|w| {
                let StructValue::Struct(props) = w else { return None };
                match props.0.get(&PropertyKey::from("RawData"))? {
                    Property::Struct(StructValue::Game(PalStruct::Work(raw))) => {
                        Some(raw.work_type.clone())
                    }
                    _ => None,
                }
            })
            .collect()
    }
    let imported_work_types = work_types(&imported);
    assert!(!imported_work_types.is_empty(), "fixture must exercise typed works");
    assert_eq!(
        work_types(&restored),
        imported_work_types,
        "work_type must survive an encode/decode cycle"
    );
    assert!(
        work_types(&restored).iter().all(|t| !t.is_empty()),
        "work_type must not come back blanked"
    );
}

/// The headline invariant, verified through the real pipeline rather than by hand:
/// anything `import` returns must be something `place` can safely write. A blueprint
/// import that quietly produces something `place` refuses would otherwise only be caught
/// by a human clicking through the UI.
#[test]
fn an_imported_blueprint_places_cleanly_into_a_fixture_save() {
    use ps_core::domain::blueprint::place::{self, PlacementRequest};
    use ps_core::domain::blueprint::validate::{Anchor, PlacementMode};

    let mut session = common::load_fixture_session("v1_relics");
    let imported = pst::import(&fixture("v1_relics_base.json"), "Home").expect("imports").blueprint;
    assert!(!imported.structures.is_empty(), "the fixture base has structures");

    let guild_id = common::fixture_guild_id(&session);
    let owner = common::fixture_player_uid(&session);
    let bases_before = common::base_count(&session);
    let objects_before = common::map_object_count(&session);

    let request = PlacementRequest {
        anchor: Anchor { x: 400_000.0, y: 400_000.0, z: 1000.0, yaw_radians: 0.0 },
        mode: PlacementMode::NewBase { guild_id },
        owner_player_uid: owner,
        override_warnings: true,
    };

    let result = place::place(&mut session, &imported, &request, &common::game_data())
        .expect("an imported blueprint must place cleanly into a fixture save");

    assert_eq!(common::base_count(&session), bases_before + 1, "placement must add one base");
    assert_eq!(
        result.structures_placed as usize,
        imported.structures.len(),
        "every imported structure must be placed"
    );
    assert_eq!(
        common::map_object_count(&session),
        objects_before + imported.structures.len(),
        "every placed structure must reach MapObjectSaveData"
    );
}

/// The 9 real PST exports. Env-gated because they are large and live outside the repo:
///
///     PST_CORPUS_DIR=/o/tmp/pstbases cargo test -p ps-core --features test-fixtures \
///         --test pst_import the_corpus -- --nocapture
///
/// Reports rather than failing on warnings: the point is to discover which adapter
/// variants real blueprints exercise. Blocking findings and hard errors do fail.
///
/// Peak working set observed while importing the full 9-file corpus in one process,
/// dominated by the ~92 MB `megabase.json` sample: ~526 MB (measured with
/// `Get-Process`'s `PeakWorkingSet64` wrapping the compiled test binary).
#[test]
fn the_corpus_imports_without_blocking_findings() {
    let Ok(dir) = std::env::var("PST_CORPUS_DIR") else {
        eprintln!("PST_CORPUS_DIR unset; skipping the corpus test");
        return;
    };

    let mut failures = Vec::new();
    let mut warned = 0usize;
    for entry in std::fs::read_dir(&dir).expect("read corpus dir").flatten() {
        let path = entry.path();
        let is_blueprint = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e == "json" || e == "pstbase");
        if !is_blueprint {
            continue;
        }
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let bytes = std::fs::read(&path).expect("read corpus file");

        match pst::import(&bytes, &name) {
            Err(e) => failures.push(format!("{name}: hard error: {e}")),
            Ok(imported) => {
                for finding in &imported.findings {
                    match finding.severity {
                        Severity::Blocking => {
                            failures.push(format!("{name}: blocking {}: {}", finding.code, finding.message));
                        }
                        Severity::Warning => {
                            warned += 1;
                            eprintln!("{name}: warning {}: {}", finding.code, finding.message);
                        }
                    }
                }
                eprintln!(
                    "{name}: {} structures, {} containers, {} works",
                    imported.blueprint.structures.len(),
                    imported.blueprint.item_containers.len(),
                    imported.blueprint.works.len()
                );
            }
        }
    }

    eprintln!("corpus: {warned} warning(s) across all samples");
    assert!(failures.is_empty(), "corpus failures:\n{}", failures.join("\n"));
}
