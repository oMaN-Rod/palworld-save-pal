//! In-memory sav↔json conversion (uesave JSON schema).
//! Serves the convert_sav_file WS handler (serialization.py:86-105 is the Python
//! equivalent, but the JSON schema here is uesave's — documented breaking change,
//! spec §4).

use std::io::Cursor;

use crate::error::CoreError;

pub fn sav_to_json_string(sav_bytes: &[u8]) -> Result<String, CoreError> {
    let save = uesave::Save::read_with_types(
        &mut Cursor::new(sav_bytes),
        uesave::games::palworld::palworld_types(),
    )
    .map_err(|error| CoreError::Parse(error.to_string()))?;
    serde_json::to_string(&save).map_err(|error| CoreError::Other(error.to_string()))
}

pub fn json_to_sav_bytes(json_bytes: &[u8]) -> Result<Vec<u8>, CoreError> {
    let save: uesave::Save =
        serde_json::from_slice(json_bytes).map_err(|error| CoreError::Parse(error.to_string()))?;
    let mut sav_bytes = Vec::new();
    save.write_compressed(
        &mut sav_bytes,
        uesave::compression::CompressionFormat::Oodle,
    )
    .map_err(|error| CoreError::Parse(error.to_string()))?;
    Ok(sav_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gamepass::fixture::python_testdata_dir;

    #[test]
    fn sav_json_sav_round_trip_preserves_gvas_bytes() {
        let testdata = match python_testdata_dir() {
            Some(dir) => dir,
            None => {
                eprintln!("SKIP: python testdata not found (set PSP_PY_TESTDATA)");
                return;
            }
        };
        let sav_bytes =
            std::fs::read(testdata.join("00000000000000000000000000000001.sav")).unwrap();

        let json = sav_to_json_string(&sav_bytes).unwrap();
        assert!(json.starts_with('{'));
        assert!(!json.contains('\n')); // minified, like Python's minify=True

        let rebuilt_sav = json_to_sav_bytes(json.as_bytes()).unwrap();
        assert_eq!(&rebuilt_sav[8..12], b"PlM1");

        // GVAS payloads identical after the full round trip.
        let original_gvas =
            uesave::compression::decompress_save(&mut std::io::Cursor::new(sav_bytes.as_slice()))
                .unwrap();
        let rebuilt_gvas =
            uesave::compression::decompress_save(&mut std::io::Cursor::new(rebuilt_sav.as_slice()))
                .unwrap();
        assert_eq!(original_gvas, rebuilt_gvas);
    }

    #[test]
    fn json_to_sav_rejects_invalid_json() {
        let error = json_to_sav_bytes(b"{ not json").unwrap_err();
        assert!(matches!(error, crate::error::CoreError::Parse(_)));
    }

    #[test]
    fn corpus_level_meta_round_trip() {
        // Use real gamepass LevelMeta.sav from the on-disk corpus
        let corpus_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../backups/gamepass/000900000487F3B6_0000000000000000000000006B210A9C_20260325231642/4F64BAB699AE4B4A97A5862116E07C6D/LevelMeta.sav",
        );
        if !corpus_path.exists() {
            eprintln!("SKIP: corpus LevelMeta.sav not found at {corpus_path:?}");
            return;
        }

        let sav_bytes = std::fs::read(&corpus_path).unwrap();

        // Convert to JSON
        let json = sav_to_json_string(&sav_bytes).unwrap();
        assert!(json.starts_with('{'), "JSON should start with '{{");
        assert!(
            !json.contains('\n'),
            "JSON should be minified (no newlines)"
        );

        // Convert back to sav
        let rebuilt_sav = json_to_sav_bytes(json.as_bytes()).unwrap();
        assert_eq!(
            &rebuilt_sav[8..12],
            b"PlM1",
            "rebuilt sav should have PlM1 at offset 8"
        );

        // Verify GVAS payloads are identical (full round-trip)
        let original_gvas =
            uesave::compression::decompress_save(&mut std::io::Cursor::new(sav_bytes.as_slice()))
                .unwrap();
        let rebuilt_gvas =
            uesave::compression::decompress_save(&mut std::io::Cursor::new(rebuilt_sav.as_slice()))
                .unwrap();
        assert_eq!(
            original_gvas, rebuilt_gvas,
            "GVAS payloads should be identical after round-trip"
        );
    }
}
