//! LocalData.sav editing — map unlock.
//!
//! `LocalData.sav`'s `SaveData` struct stores the fog-of-war mask in one of two
//! shapes, and which one is present depends on the game version:
//!
//! - **pre-1.0**: `WorldMapMaskTextureV4`, a single `ArrayProperty` of bytes —
//!   the one and only mask, covering Palpagos.
//! - **1.0+**: `WorldMapUISaveDataMap`, a `TMap<FName, FPalWorldMapUISaveData>`
//!   keyed by map name (`MainMap`, `Tree`), each value a struct holding a
//!   `MaskTextureData` byte array. 1.0 added the World Tree, which is a second,
//!   separate map with its own mask.
//!
//! On every real 1.0 save inspected the legacy property is **absent entirely**,
//! so the two shapes are disjoint in practice — but this handles both being
//! present, since neither costs anything to clear.
//!
//! "Unlocked" means every byte of every mask present is zero.

use crate::ue::{ByteArray, Property, PropertyKey, StructValue, ValueVec};

use crate::error::CoreError;
use crate::savio;

#[derive(Debug)]
pub struct MapUnlockOutcome {
    pub sav_bytes: Vec<u8>,
    pub cleared_byte_count: usize,
}

/// Zeroes a mask array in place, returning how many non-zero bytes it cleared.
fn zero_mask(mask_bytes: &mut [u8]) -> usize {
    let mut cleared = 0;
    for byte in mask_bytes.iter_mut() {
        if *byte != 0 {
            *byte = 0;
            cleared += 1;
        }
    }
    cleared
}

/// Zeroes every non-zero byte of EVERY fog mask in `SaveData` — the legacy
/// `WorldMapMaskTextureV4` when present, plus the `MaskTextureData` of every
/// entry of the 1.0 `WorldMapUISaveDataMap` (which covers both `MainMap` and the
/// World Tree) — and re-emits the file as PlM/Oodle. `cleared_byte_count` sums
/// across all of them.
///
/// Neither structure being present is an error: that is an unrecognised save,
/// not a fully-unlocked one. Error strings reach the UI unprefixed; the server
/// layer adds the "Failed to unlock map: " prefix.
pub fn unlock_world_map(local_data_sav: &[u8]) -> Result<MapUnlockOutcome, CoreError> {
    let mut save = savio::read_sav_bytes(local_data_sav)?;

    let save_data = match save
        .root
        .properties
        .0
        .get_mut(&PropertyKey::from("SaveData"))
    {
        Some(Property::Struct(StructValue::Struct(properties))) => properties,
        _ => {
            return Err(CoreError::Other(
                "SaveData not found in LocalData.sav".to_string(),
            ))
        }
    };

    let mut cleared_byte_count = 0;
    let mut found_a_mask = false;

    // Legacy pre-1.0 single mask.
    if let Some(Property::Array(ValueVec::Byte(ByteArray::Byte(mask_bytes)))) = save_data
        .0
        .get_mut(&PropertyKey::from("WorldMapMaskTextureV4"))
    {
        if !mask_bytes.is_empty() {
            found_a_mask = true;
            cleared_byte_count += zero_mask(mask_bytes);
        }
    }

    // 1.0 per-map masks. Keys are `Name`s ("MainMap", "Tree"); each value is a
    // bare user struct whose only field is the `MaskTextureData` byte array.
    // Every entry is cleared regardless of its key, so a future third map area
    // is unlocked without a code change.
    if let Some(Property::Map(entries)) = save_data
        .0
        .get_mut(&PropertyKey::from("WorldMapUISaveDataMap"))
    {
        for entry in entries.iter_mut() {
            let Some(value_properties) = crate::props::struct_props_mut(&mut entry.value) else {
                continue;
            };
            let Some(Property::Array(ValueVec::Byte(ByteArray::Byte(mask_bytes)))) =
                value_properties
                    .0
                    .get_mut(&PropertyKey::from("MaskTextureData"))
            else {
                continue;
            };
            if mask_bytes.is_empty() {
                continue;
            }
            found_a_mask = true;
            cleared_byte_count += zero_mask(mask_bytes);
        }
    }

    if !found_a_mask {
        return Err(CoreError::Other(
            "No non-empty WorldMapMaskTextureV4 or WorldMapUISaveDataMap mask found in SaveData"
                .to_string(),
        ));
    }

    let sav_bytes = savio::write_sav_bytes(&save)?;

    Ok(MapUnlockOutcome {
        sav_bytes,
        cleared_byte_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gamepass::fixture::reference_saves_dir;

    /// A real, git-tracked `LevelMeta.sav` (the world1 fixture, also used by
    /// `psp-server/tests/phase4_ws.rs`). It carries a `SaveData` struct, so the
    /// hermetic tests below can graft a synthetic mask into a real GVAS tree.
    /// Committed, so tests using it run unconditionally on a clean checkout / CI.
    fn fixture_level_meta_path() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/saves/world1/LevelMeta.sav")
    }

    fn mask_bytes(local_data_sav: &[u8]) -> Vec<u8> {
        let save = crate::ue::SaveReader::new()
            .game::<crate::ue::Palworld>()
            .types(crate::ue::games::palworld::palworld_types())
            .read(std::io::Cursor::new(local_data_sav))
            .unwrap();
        let crate::ue::Property::Struct(crate::ue::StructValue::Struct(save_data)) =
            &save.root.properties.0[&crate::ue::PropertyKey::from("SaveData")]
        else {
            panic!("SaveData missing");
        };
        let crate::ue::Property::Array(crate::ue::ValueVec::Byte(crate::ue::ByteArray::Byte(bytes))) =
            &save_data.0[&crate::ue::PropertyKey::from("WorldMapMaskTextureV4")]
        else {
            panic!("WorldMapMaskTextureV4 missing or not a byte array");
        };
        bytes.clone()
    }

    #[test]
    fn local_data_round_trips_byte_identical_at_gvas_level() {
        let sav_bytes = std::fs::read(reference_saves_dir().join("LocalData.sav")).unwrap();
        let gvas_bytes =
            crate::ue::compression::decompress_save(&mut std::io::Cursor::new(sav_bytes.as_slice()))
                .unwrap();
        let save = crate::ue::SaveReader::new()
            .game::<crate::ue::Palworld>()
            .types(crate::ue::games::palworld::palworld_types())
            .read(std::io::Cursor::new(sav_bytes.as_slice()))
            .unwrap();
        let mut rewritten = Vec::new();
        save.write(&mut rewritten).unwrap();
        assert_eq!(
            gvas_bytes, rewritten,
            "LocalData.sav GVAS round-trip must be byte-identical"
        );
    }

    #[test]
    fn unlock_world_map_reports_python_error_for_missing_save_data() {
        let error = unlock_world_map(b"not a sav file").unwrap_err();
        assert!(matches!(error, crate::error::CoreError::Parse(_)));
    }

    /// Hermetic test of the zeroing/count/re-emit path, needing no
    /// `LocalData.sav` fixture. The grafted mask `[1, 2, 3, 0, 4]` mixes
    /// non-zero and already-zero bytes so the cleared count is discriminating.
    #[test]
    fn unlock_world_map_zeroes_synthetic_mask_grafted_into_real_savedata() {
        let meta_bytes = std::fs::read(fixture_level_meta_path()).unwrap();

        // The schema must be registered for the brand-new property or the
        // writer fails with MissingPropertySchema.
        let mut save = savio::read_sav_bytes(&meta_bytes).unwrap();
        {
            let save_data =
                crate::props::get_mut(&mut save.root.properties, &["SaveData"]).unwrap();
            let save_data = crate::props::struct_props_mut(save_data).unwrap();
            save_data.insert(
                "WorldMapMaskTextureV4",
                Property::Array(ValueVec::Byte(ByteArray::Byte(vec![1, 2, 3, 0, 4]))),
            );
        }
        crate::props::ensure_schema(
            &mut save,
            "SaveData.WorldMapMaskTextureV4".to_string(),
            crate::ue::PropertyTagPartial {
                id: None,
                data: crate::ue::PropertyTagDataPartial::Array(Box::new(
                    crate::ue::PropertyTagDataPartial::Byte(None),
                )),
            },
        );
        let grafted_sav = savio::write_sav_bytes(&save).unwrap();
        assert_eq!(mask_bytes(&grafted_sav), vec![1, 2, 3, 0, 4]);

        let outcome = unlock_world_map(&grafted_sav).unwrap();
        assert_eq!(
            outcome.cleared_byte_count, 4,
            "only the four non-zero bytes (1, 2, 3, 4) are cleared; the already-zero byte is not counted"
        );
        assert_eq!(&outcome.sav_bytes[8..12], b"PlM1");

        let mask_after = mask_bytes(&outcome.sav_bytes);
        assert_eq!(mask_after, vec![0, 0, 0, 0, 0]);
    }

    /// Grafts a 1.0-shaped `WorldMapUISaveDataMap` into a real `SaveData` tree.
    /// Mirrors the exact shape observed in real 1.0 saves: a `Property::Map`
    /// whose keys are `Property::Name` ("MainMap"/"Tree") and whose values are
    /// bare user structs carrying a single `MaskTextureData` byte array.
    fn graft_world_map_ui_save_data_map(
        save: &mut crate::ue::Save,
        entries: &[(&str, Vec<u8>)],
        legacy_mask: Option<Vec<u8>>,
    ) {
        {
            let save_data =
                crate::props::get_mut(&mut save.root.properties, &["SaveData"]).unwrap();
            let save_data = crate::props::struct_props_mut(save_data).unwrap();
            if let Some(legacy) = legacy_mask {
                save_data.insert(
                    "WorldMapMaskTextureV4",
                    Property::Array(ValueVec::Byte(ByteArray::Byte(legacy))),
                );
            }
            let map_entries = entries
                .iter()
                .map(|(name, bytes)| {
                    let mut value = crate::ue::Properties::default();
                    value.insert(
                        "MaskTextureData",
                        Property::Array(ValueVec::Byte(ByteArray::Byte(bytes.clone()))),
                    );
                    crate::ue::MapEntry {
                        key: Property::Name((*name).to_string()),
                        value: Property::Struct(StructValue::Struct(value)),
                    }
                })
                .collect();
            save_data.insert("WorldMapUISaveDataMap", Property::Map(map_entries));
        }

        crate::props::ensure_schema(
            save,
            "SaveData.WorldMapMaskTextureV4".to_string(),
            crate::ue::PropertyTagPartial {
                id: None,
                data: crate::ue::PropertyTagDataPartial::Array(Box::new(
                    crate::ue::PropertyTagDataPartial::Byte(None),
                )),
            },
        );
        // Observed in real 1.0 saves: the map's value struct is an anonymous
        // (`StructType::Struct(None)`) inline struct, and its inner field's
        // schema path is the map path + ".MaskTextureData" — no ".Value" and no
        // entry index segment.
        crate::props::ensure_schema(
            save,
            "SaveData.WorldMapUISaveDataMap".to_string(),
            crate::ue::PropertyTagPartial {
                id: None,
                data: crate::ue::PropertyTagDataPartial::Map {
                    key_type: Box::new(crate::ue::PropertyTagDataPartial::Other(
                        crate::ue::PropertyType::NameProperty,
                    )),
                    value_type: Box::new(crate::ue::PropertyTagDataPartial::Struct {
                        struct_type: crate::ue::StructType::Struct(None),
                        id: crate::ue::FGuid::default(),
                    }),
                },
            },
        );
        crate::props::ensure_schema(
            save,
            "SaveData.WorldMapUISaveDataMap.MaskTextureData".to_string(),
            crate::ue::PropertyTagPartial {
                id: None,
                data: crate::ue::PropertyTagDataPartial::Array(Box::new(
                    crate::ue::PropertyTagDataPartial::Byte(None),
                )),
            },
        );
    }

    /// Reads back every `WorldMapUISaveDataMap` entry as (name, mask bytes).
    fn ui_map_masks(local_data_sav: &[u8]) -> Vec<(String, Vec<u8>)> {
        let save = savio::read_sav_bytes(local_data_sav).unwrap();
        let Some(Property::Struct(StructValue::Struct(save_data))) =
            save.root.properties.0.get(&PropertyKey::from("SaveData"))
        else {
            panic!("SaveData missing");
        };
        let Some(Property::Map(entries)) =
            save_data.0.get(&PropertyKey::from("WorldMapUISaveDataMap"))
        else {
            panic!("WorldMapUISaveDataMap missing or not a Map");
        };
        entries
            .iter()
            .map(|entry| {
                let Property::Name(name) = &entry.key else {
                    panic!("map key is not a Name");
                };
                let bytes = crate::props::get_in(&entry.value, &["MaskTextureData"])
                    .and_then(crate::props::as_byte_array)
                    .expect("MaskTextureData byte array");
                (name.clone(), bytes.to_vec())
            })
            .collect()
    }

    fn fixture_level_meta_bytes() -> Vec<u8> {
        std::fs::read(fixture_level_meta_path()).unwrap()
    }

    /// A 1.0-only save: `WorldMapUISaveDataMap` present, legacy field ABSENT.
    /// Every entry's mask must end up zeroed and the legacy field's absence must
    /// not be an error.
    #[test]
    fn unlock_world_map_zeroes_every_world_map_ui_save_data_map_entry() {
        let meta_bytes = fixture_level_meta_bytes();
        let mut save = savio::read_sav_bytes(&meta_bytes).unwrap();
        graft_world_map_ui_save_data_map(
            &mut save,
            &[("MainMap", vec![1, 2, 3, 0, 4]), ("Tree", vec![0, 9, 9, 0])],
            None,
        );
        let grafted_sav = savio::write_sav_bytes(&save).unwrap();

        let outcome = unlock_world_map(&grafted_sav).unwrap();
        assert_eq!(
            outcome.cleared_byte_count, 6,
            "4 non-zero MainMap bytes + 2 non-zero Tree bytes"
        );
        assert_eq!(&outcome.sav_bytes[8..12], b"PlM1");

        let masks = ui_map_masks(&outcome.sav_bytes);
        assert_eq!(masks.len(), 2);
        assert_eq!(masks[0], ("MainMap".to_string(), vec![0, 0, 0, 0, 0]));
        assert_eq!(masks[1], ("Tree".to_string(), vec![0, 0, 0, 0]));

        // Unlocking twice clears nothing further.
        assert_eq!(
            unlock_world_map(&outcome.sav_bytes)
                .unwrap()
                .cleared_byte_count,
            0
        );
    }

    /// A save carrying BOTH shapes: the cleared count sums across all of them.
    #[test]
    fn unlock_world_map_sums_cleared_bytes_across_legacy_and_ui_map() {
        let meta_bytes = fixture_level_meta_bytes();
        let mut save = savio::read_sav_bytes(&meta_bytes).unwrap();
        graft_world_map_ui_save_data_map(
            &mut save,
            &[("MainMap", vec![5, 0]), ("Tree", vec![7, 7, 7])],
            Some(vec![1, 2, 3, 0, 4]),
        );
        let grafted_sav = savio::write_sav_bytes(&save).unwrap();

        let outcome = unlock_world_map(&grafted_sav).unwrap();
        assert_eq!(
            outcome.cleared_byte_count,
            4 + 1 + 3,
            "4 legacy + 1 MainMap + 3 Tree non-zero bytes"
        );
        assert_eq!(mask_bytes(&outcome.sav_bytes), vec![0, 0, 0, 0, 0]);
        for (_, bytes) in ui_map_masks(&outcome.sav_bytes) {
            assert!(bytes.iter().all(|byte| *byte == 0));
        }
    }

    /// Neither mask structure present — a genuinely unrecognised save, which
    /// must still error rather than silently succeed. Asserts on the error
    /// MESSAGE, not just the `CoreError::Other` variant, since the "SaveData
    /// not found" branch returns that same variant and would falsely pass a
    /// variant-only check.
    #[test]
    fn unlock_world_map_errors_when_no_mask_structure_exists() {
        let meta_bytes = fixture_level_meta_bytes();
        // The fixture LevelMeta.sav has a SaveData struct but no mask of either
        // shape, so it stands in for an unrecognised save as-is.
        let error = unlock_world_map(&meta_bytes).unwrap_err();
        let crate::error::CoreError::Other(message) = error else {
            panic!("expected CoreError::Other, got {error:?}");
        };
        assert_eq!(
            message,
            "No non-empty WorldMapMaskTextureV4 or WorldMapUISaveDataMap mask found in SaveData"
        );
    }

}
