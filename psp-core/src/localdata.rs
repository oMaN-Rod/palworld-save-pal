//! LocalData.sav editing — map unlock.
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

/// Zeroes every non-zero byte of `SaveData.WorldMapMaskTextureV4` and re-emits
/// the file as PlM/Oodle. Error strings reach the UI unprefixed; the server
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

    let mask_property = save_data
        .0
        .get_mut(&PropertyKey::from("WorldMapMaskTextureV4"))
        .ok_or_else(|| {
            CoreError::Other("WorldMapMaskTextureV4 not found in SaveData".to_string())
        })?;

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

    /// Resolves the testdata dir only when it actually holds a `LocalData.sav`,
    /// so a partial checkout skips rather than panicking on the read below.
    fn testdata_local_data_or_skip() -> Option<std::path::PathBuf> {
        let Some(dir) = python_testdata_dir() else {
            eprintln!("SKIP: python testdata not found (set PSP_PY_TESTDATA)");
            return None;
        };
        if !dir.join("LocalData.sav").exists() {
            eprintln!("SKIP: python testdata has no LocalData.sav (partial layout)");
            return None;
        }
        Some(dir)
    }

    /// A real committed gamepass `LevelMeta.sav`. It carries a `SaveData`
    /// struct, so the hermetic test below can graft a synthetic mask into a
    /// real GVAS tree; the corpus has no `LocalData.sav`.
    fn corpus_level_meta_path() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../backups/gamepass/000900000487F3B6_0000000000000000000000006B210A9C_20260325231642/4F64BAB699AE4B4A97A5862116E07C6D/LevelMeta.sav",
        )
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
        let Some(testdata) = testdata_local_data_or_skip() else {
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
        let Some(testdata) = testdata_local_data_or_skip() else {
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
        let error = unlock_world_map(b"not a sav file").unwrap_err();
        assert!(matches!(error, crate::error::CoreError::Parse(_)));
    }

    /// Hermetic test of the zeroing/count/re-emit path, needing no
    /// `LocalData.sav` fixture. The grafted mask `[1, 2, 3, 0, 4]` mixes
    /// non-zero and already-zero bytes so the cleared count is discriminating.
    #[test]
    fn unlock_world_map_zeroes_synthetic_mask_grafted_into_real_savedata() {
        let level_meta_path = corpus_level_meta_path();
        if !level_meta_path.exists() {
            eprintln!(
                "SKIP: corpus LevelMeta.sav not found ({})",
                level_meta_path.display()
            );
            return;
        }
        let meta_bytes = std::fs::read(&level_meta_path).unwrap();

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
            uesave::PropertyTagPartial {
                id: None,
                data: uesave::PropertyTagDataPartial::Array(Box::new(
                    uesave::PropertyTagDataPartial::Byte(None),
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

    /// End-to-end against a real Xbox `LocalData.sav` from the on-disk gamepass
    /// corpus, when one is present. The corpus file is only ever read;
    /// `unlock_world_map` touches no disk.
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

    /// First `backups/gamepass/<save_id>/<container>/LocalData.sav` on disk, if
    /// any — not every save in the corpus has one.
    fn find_corpus_local_data_sav() -> Option<std::path::PathBuf> {
        let gamepass_root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../backups/gamepass");
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
