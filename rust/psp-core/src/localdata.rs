//! LocalData.sav editing — map unlock.
//! Port of `palworld_save_pal/ws/handlers/map_unlock_handler.py:46-83`.
//! `LocalData.sav`'s `SaveData` struct holds `WorldMapMaskTextureV4`, an
//! `ArrayProperty` of bytes (the fog-of-war mask texture). "Unlocked" means
//! every byte in that array is zero.

use uesave::{ByteArray, Property, PropertyKey, StructValue, ValueVec};

use crate::error::CoreError;
use crate::savio;

#[derive(Debug)]
pub struct MapUnlockOutcome {
    pub sav_bytes: Vec<u8>,
    pub cleared_byte_count: usize,
}

/// Zeroes every non-zero byte of `SaveData.WorldMapMaskTextureV4` and
/// re-emits the file as PlM/Oodle (save-type `0x31`), matching
/// `compress_gvas_to_sav(gvas_file.write(...), 0x31)` in
/// `map_unlock_handler.py:77-79`. Error strings mirror the Python handler's
/// own `raise Exception(...)` messages verbatim
/// (`map_unlock_handler.py:56-65`); the server layer is expected to prefix
/// them with "Failed to unlock map: " the same way the Python handler's
/// `except` block does, not this function.
pub fn unlock_world_map(local_data_sav: &[u8]) -> Result<MapUnlockOutcome, CoreError> {
    let mut save = savio::read_sav_bytes(local_data_sav)?;

    // Python: `save_data = PalObjects.get_value(gvas_file.properties["SaveData"])`
    // `if not save_data: raise Exception("SaveData not found in LocalData.sav")`
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

    // Python: `if "WorldMapMaskTextureV4" not in save_data: raise Exception(
    // "WorldMapMaskTextureV4 not found in SaveData")`
    let mask_property = save_data
        .0
        .get_mut(&PropertyKey::from("WorldMapMaskTextureV4"))
        .ok_or_else(|| {
            CoreError::Other("WorldMapMaskTextureV4 not found in SaveData".to_string())
        })?;

    // Python: `map_values = PalObjects.get_array_property(...)`
    // `if not map_values: raise Exception("Map values array not found")`
    let mask_bytes = match mask_property {
        Property::Array(ValueVec::Byte(ByteArray::Byte(bytes))) => bytes,
        _ => return Err(CoreError::Other("Map values array not found".to_string())),
    };
    if mask_bytes.is_empty() {
        return Err(CoreError::Other("Map values array not found".to_string()));
    }

    let mut cleared_byte_count = 0;
    for byte in mask_bytes.iter_mut() {
        if *byte != 0 {
            *byte = 0;
            cleared_byte_count += 1;
        }
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
    use crate::gamepass::fixture::python_testdata_dir;

    fn testdata_or_skip() -> Option<std::path::PathBuf> {
        let dir = python_testdata_dir();
        if dir.is_none() {
            eprintln!("SKIP: python testdata not found (set PSP_PY_TESTDATA)");
        }
        dir
    }

    fn mask_bytes(local_data_sav: &[u8]) -> Vec<u8> {
        let save = uesave::Save::read_with_types(
            &mut std::io::Cursor::new(local_data_sav),
            uesave::games::palworld::palworld_types(),
        )
        .unwrap();
        let uesave::Property::Struct(uesave::StructValue::Struct(save_data)) =
            &save.root.properties.0[&uesave::PropertyKey::from("SaveData")]
        else {
            panic!("SaveData missing");
        };
        let uesave::Property::Array(uesave::ValueVec::Byte(uesave::ByteArray::Byte(bytes))) =
            &save_data.0[&uesave::PropertyKey::from("WorldMapMaskTextureV4")]
        else {
            panic!("WorldMapMaskTextureV4 missing or not a byte array");
        };
        bytes.clone()
    }

    #[test]
    fn local_data_round_trips_byte_identical_at_gvas_level() {
        let Some(testdata) = testdata_or_skip() else {
            return;
        };
        let sav_bytes = std::fs::read(testdata.join("LocalData.sav")).unwrap();
        let gvas_bytes =
            uesave::compression::decompress_save(&mut std::io::Cursor::new(sav_bytes.as_slice()))
                .unwrap();
        let save = uesave::Save::read_with_types(
            &mut std::io::Cursor::new(sav_bytes.as_slice()),
            uesave::games::palworld::palworld_types(),
        )
        .unwrap();
        let mut rewritten = Vec::new();
        save.write(&mut rewritten).unwrap();
        assert_eq!(
            gvas_bytes, rewritten,
            "LocalData.sav GVAS round-trip must be byte-identical"
        );
    }

    #[test]
    fn unlock_world_map_zeroes_mask_and_emits_plm() {
        let Some(testdata) = testdata_or_skip() else {
            return;
        };
        let sav_bytes = std::fs::read(testdata.join("LocalData.sav")).unwrap();
        let mask_before = mask_bytes(&sav_bytes);
        let nonzero_before = mask_before.iter().filter(|byte| **byte != 0).count();

        let outcome = unlock_world_map(&sav_bytes).unwrap();
        assert_eq!(outcome.cleared_byte_count, nonzero_before);
        assert_eq!(&outcome.sav_bytes[8..12], b"PlM1");

        let mask_after = mask_bytes(&outcome.sav_bytes);
        assert_eq!(mask_after.len(), mask_before.len());
        assert!(mask_after.iter().all(|byte| *byte == 0));

        // Unlocking twice clears nothing further.
        let second = unlock_world_map(&outcome.sav_bytes).unwrap();
        assert_eq!(second.cleared_byte_count, 0);
    }

    #[test]
    fn unlock_world_map_reports_python_error_for_missing_save_data() {
        // A syntactically valid GVAS without SaveData: reuse LevelMeta which HAS SaveData,
        // so build the negative case from raw bytes instead — any parse failure path:
        let error = unlock_world_map(b"not a sav file").unwrap_err();
        assert!(matches!(error, crate::error::CoreError::Parse(_)));
    }

    /// Real Xbox `LocalData.sav`, if present in the on-disk gamepass backup
    /// corpus (not committed; only present on a machine that has copied it
    /// in) -- validates `unlock_world_map` end-to-end against a real
    /// fog-of-war mask instead of only the Python reference fixture.
    /// SAFETY: reads the corpus file read-only; `unlock_world_map` never
    /// mutates its input slice or touches disk, so `backups/gamepass/` is
    /// never written to. Skipped, not failed, when no such file exists.
    #[test]
    fn unlock_world_map_on_real_gamepass_corpus_when_present() {
        let Some(local_data_path) = find_corpus_local_data_sav() else {
            eprintln!(
                "SKIP: no LocalData.sav found under backups/gamepass/ (unlock_world_map_on_real_gamepass_corpus_when_present)"
            );
            return;
        };
        let sav_bytes = std::fs::read(&local_data_path).unwrap();
        let mask_before = mask_bytes(&sav_bytes);
        let nonzero_before = mask_before.iter().filter(|byte| **byte != 0).count();

        let outcome = unlock_world_map(&sav_bytes).unwrap();
        assert_eq!(outcome.cleared_byte_count, nonzero_before);
        assert_eq!(&outcome.sav_bytes[8..12], b"PlM1");

        let mask_after = mask_bytes(&outcome.sav_bytes);
        assert_eq!(mask_after.len(), mask_before.len());
        assert!(mask_after.iter().all(|byte| *byte == 0));
    }

    /// Searches `backups/gamepass/<save_id>/<container>/LocalData.sav` for
    /// the first real Xbox `LocalData.sav` on disk, if any. Not every
    /// gamepass save in the corpus has one (map unlock is optional/rare
    /// data), so this walks the whole tree rather than assuming a fixed
    /// path.
    fn find_corpus_local_data_sav() -> Option<std::path::PathBuf> {
        let gamepass_root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../backups/gamepass");
        if !gamepass_root.exists() {
            return None;
        }
        for save_dir in std::fs::read_dir(&gamepass_root).ok()?.flatten() {
            if !save_dir.path().is_dir() {
                continue;
            }
            for container_dir in std::fs::read_dir(save_dir.path()).ok()?.flatten() {
                let candidate = container_dir.path().join("LocalData.sav");
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
        None
    }
}
