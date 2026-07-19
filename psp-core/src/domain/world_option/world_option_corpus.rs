//! Round-trip and drift guards over the real WorldOption corpus.
//!
//! The corpus is 7 committed fixtures under `tests/fixtures/world_option/`, copied
//! from real saves. It spans 89..=119 keys (observed counts: 89, 108, 119, 119, 119,
//! 119, 119): real saves are SPARSE, and `Version` (always 101) does not discriminate.
//! Real saves in the wild have been seen as low as 87 keys, though no fixture here
//! goes that low.

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::{
    apply_patch, ensure_world_option_schemas, kind_for, read_settings, settings_schema_path,
    tag_for, WoKind, WorldOptionPatch, WORLD_OPTION_SETTINGS,
};

/// The 7 committed `tests/fixtures/world_option/*.sav` fixtures, sorted.
///
/// Panics if the fixture directory is missing or does not contain exactly 7 `.sav`
/// files, so a broken checkout fails loudly instead of silently skipping tests.
fn corpus() -> Vec<PathBuf> {
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/world_option");

    let mut found: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("WorldOption fixture dir missing: {dir:?}: {e}"))
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "sav"))
        .collect();
    found.sort();

    assert_eq!(
        found.len(),
        7,
        "expected exactly 7 WorldOption fixtures in {dir:?}, found {}: {found:?}",
        found.len()
    );

    found
}

fn decompress(bytes: &[u8]) -> Vec<u8> {
    crate::ue::compression::decompress_save(&mut std::io::Cursor::new(bytes)).unwrap()
}

#[test]
fn empty_patch_round_trip_preserves_gvas_payload_byte_for_byte() {
    let files = corpus();

    for path in &files {
        let original_bytes = std::fs::read(path).unwrap();
        let mut save = crate::savio::read_sav_bytes(&original_bytes).unwrap();

        // Priming must not alter the payload: it only records schemas.
        ensure_world_option_schemas(&mut save);
        assert!(
            !apply_patch(&mut save, &[]).unwrap(),
            "empty patch must not be dirty: {path:?}"
        );

        let rewritten = crate::savio::write_sav_bytes(&save).unwrap();
        assert_eq!(
            decompress(&original_bytes),
            decompress(&rewritten),
            "GVAS payload changed for {path:?}"
        );
    }
}

#[test]
fn corpus_spans_sparse_and_full_saves() {
    let files = corpus();

    let mut counts: Vec<usize> = files
        .iter()
        .map(|path| {
            let save = crate::savio::read_sav_bytes(&std::fs::read(path).unwrap()).unwrap();
            read_settings(&save).len()
        })
        .collect();
    counts.sort_unstable();

    // Guards the premise of the whole feature: if every file had 119 keys, the
    // add-absent-key path would be dead code and untested by this corpus.
    assert!(
        *counts.first().unwrap() < 119,
        "expected at least one sparse save, got {counts:?}"
    );
    assert_eq!(*counts.last().unwrap(), 119, "expected a full 119-key save");
}

#[test]
fn world_option_table_matches_corpus() {
    let files = corpus();

    // path suffix -> serialized tag, unioned across the corpus.
    let mut recorded: BTreeMap<String, String> = BTreeMap::new();
    for path in &files {
        let save = crate::savio::read_sav_bytes(&std::fs::read(path).unwrap()).unwrap();
        let json = serde_json::to_value(&save).unwrap();
        let schemas = json["schemas"]["schemas"].as_object().unwrap();
        for (schema_path, tag) in schemas {
            let Some(key) = schema_path.strip_prefix("OptionWorldData.Settings.") else {
                continue;
            };
            recorded.insert(key.to_string(), serde_json::to_string(tag).unwrap());
        }
    }

    let table: BTreeMap<&str, ()> = WORLD_OPTION_SETTINGS.iter().map(|(k, _)| (*k, ())).collect();

    let missing: Vec<&String> = recorded.keys().filter(|k| !table.contains_key(k.as_str())).collect();
    assert!(
        missing.is_empty(),
        "corpus has settings absent from WORLD_OPTION_SETTINGS (did Palworld add settings?): {missing:?}"
    );

    let extra: Vec<&&str> = table.keys().filter(|k| !recorded.contains_key(**k)).collect();
    assert!(extra.is_empty(), "table has settings the corpus never records: {extra:?}");

    // Each table tag must equal what the real files record.
    for (key, kind) in WORLD_OPTION_SETTINGS {
        let expected = serde_json::to_string(&tag_for(*kind)).unwrap();
        let actual = &recorded[*key];
        assert_eq!(
            &expected, actual,
            "tag mismatch for {key} at {}",
            settings_schema_path(key)
        );
    }
}

#[test]
fn editing_an_absent_key_on_a_sparse_save_writes_cleanly() {
    let files = corpus();

    // The sparsest file in the corpus: the one that actually exercises priming.
    let sparsest = files
        .iter()
        .min_by_key(|path| {
            let save = crate::savio::read_sav_bytes(&std::fs::read(path).unwrap()).unwrap();
            read_settings(&save).len()
        })
        .unwrap();

    let mut save = crate::savio::read_sav_bytes(&std::fs::read(sparsest).unwrap()).unwrap();
    ensure_world_option_schemas(&mut save);
    let before = read_settings(&save).len();

    // Pick a key this file genuinely lacks.
    let present: Vec<String> = read_settings(&save).into_iter().map(|e| e.key).collect();
    let absent = WORLD_OPTION_SETTINGS
        .iter()
        .map(|(k, _)| *k)
        .find(|k| !present.iter().any(|p| p == k))
        .expect("sparsest corpus save must be missing at least one setting");

    let dirty = apply_patch(
        &mut save,
        &[WorldOptionPatch {
            key: absent.to_string(),
            value: default_json_for(absent),
        }],
    )
    .unwrap();
    assert!(dirty);
    assert_eq!(read_settings(&save).len(), before + 1);

    // Must serialize: this is the MissingPropertySchema regression guard.
    let bytes = crate::savio::write_sav_bytes(&save).unwrap();
    let reparsed = crate::savio::read_sav_bytes(&bytes).unwrap();
    assert_eq!(read_settings(&reparsed).len(), before + 1);
}

/// Pins the gamepass container plumbing that Task 8 depends on: a WorldOption
/// container built by `fixture::build_wgs_tree` round-trips byte-for-byte through
/// `format::ContainerIndex` and `store::read_first_blob`.
#[test]
fn gamepass_fixture_round_trips_world_option_container() {
    let files = corpus();
    let world_option_bytes = std::fs::read(&files[0]).unwrap();

    let temp = tempfile::tempdir().unwrap();
    let save = crate::gamepass::fixture::SyntheticSave {
        save_id: "0123456789ABCDEF0123456789ABCDEF".to_string(),
        level_sav: b"LEVEL".to_vec(),
        level_meta: Some(b"META".to_vec()),
        local_data: None,
        world_option: Some(world_option_bytes.clone()),
        players: vec![],
    };
    let container_dir = crate::gamepass::fixture::build_wgs_tree(temp.path(), &[save]).unwrap();

    let index = crate::gamepass::format::ContainerIndex::read_from_dir(&container_dir).unwrap();
    let latest = index.latest_save_containers("0123456789ABCDEF0123456789ABCDEF");
    let entry = latest
        .get("WorldOption")
        .expect("WorldOption container must be discoverable");

    let (_seq, blob) = crate::gamepass::store::read_first_blob(&container_dir, entry)
        .unwrap()
        .unwrap();
    assert_eq!(blob, world_option_bytes);
}

/// A type-correct value for `key`, so the test doesn't depend on which key is absent.
fn default_json_for(key: &str) -> serde_json::Value {
    match kind_for(key).unwrap() {
        WoKind::Bool => serde_json::json!(true),
        WoKind::Int => serde_json::json!(1),
        WoKind::Float => serde_json::json!(1.0),
        WoKind::Str | WoKind::Name => serde_json::json!("x"),
        WoKind::Enum(name) => serde_json::json!(format!("{name}::None")),
        WoKind::EnumArray | WoKind::NameArray => serde_json::json!([]),
    }
}
