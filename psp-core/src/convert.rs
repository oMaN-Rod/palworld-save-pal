//! In-memory sav<->json conversion. The JSON shape is `uesave`'s own schema.

use std::io::Cursor;

use crate::error::CoreError;

pub fn sav_to_json_string(sav_bytes: &[u8]) -> Result<String, CoreError> {
    let save = crate::ue::SaveReader::new()
        .game::<crate::ue::Palworld>()
        .types(crate::ue::games::palworld::palworld_types())
        .read(Cursor::new(sav_bytes))
        .map_err(|error| CoreError::Parse(error.to_string()))?;
    serde_json::to_string(&save).map_err(|error| CoreError::Other(error.to_string()))
}

pub fn json_to_sav_bytes(json_bytes: &[u8]) -> Result<Vec<u8>, CoreError> {
    let save: crate::ue::Save =
        serde_json::from_slice(json_bytes).map_err(|error| CoreError::Parse(error.to_string()))?;
    let mut sav_bytes = Vec::new();
    save.write_plm(&mut sav_bytes)
        .map_err(|error| CoreError::Parse(error.to_string()))?;
    Ok(sav_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gamepass::fixture::reference_saves_dir;

    #[test]
    fn sav_json_sav_round_trip_preserves_gvas_bytes() {
        let testdata = reference_saves_dir();
        let sav_bytes =
            std::fs::read(testdata.join("00000000000000000000000000000001.sav")).unwrap();

        let json = sav_to_json_string(&sav_bytes).unwrap();
        assert!(json.starts_with('{'));
        assert!(!json.contains('\n')); // minified

        let rebuilt_sav = json_to_sav_bytes(json.as_bytes()).unwrap();
        assert_eq!(&rebuilt_sav[8..12], b"PlM1");

        let original_gvas =
            crate::ue::compression::decompress_save(&mut std::io::Cursor::new(sav_bytes.as_slice()))
                .unwrap();
        let rebuilt_gvas =
            crate::ue::compression::decompress_save(&mut std::io::Cursor::new(rebuilt_sav.as_slice()))
                .unwrap();
        assert_eq!(original_gvas, rebuilt_gvas);
    }

    #[test]
    fn json_to_sav_rejects_invalid_json() {
        let error = json_to_sav_bytes(b"{ not json").unwrap_err();
        assert!(matches!(error, crate::error::CoreError::Parse(_)));
    }

    #[test]
    fn level_meta_round_trip_preserves_gvas_bytes() {
        let sav_bytes =
            std::fs::read(crate::gamepass::fixture::reference_saves_dir().join("LevelMeta.sav"))
                .unwrap();

        let json = sav_to_json_string(&sav_bytes).unwrap();
        assert!(json.starts_with('{'), "JSON should start with '{{");
        assert!(
            !json.contains('\n'),
            "JSON should be minified (no newlines)"
        );

        let rebuilt_sav = json_to_sav_bytes(json.as_bytes()).unwrap();
        assert_eq!(
            &rebuilt_sav[8..12],
            b"PlM1",
            "rebuilt sav should have PlM1 at offset 8"
        );

        let original_gvas =
            crate::ue::compression::decompress_save(&mut std::io::Cursor::new(sav_bytes.as_slice()))
                .unwrap();
        let rebuilt_gvas =
            crate::ue::compression::decompress_save(&mut std::io::Cursor::new(rebuilt_sav.as_slice()))
                .unwrap();
        assert_eq!(
            original_gvas, rebuilt_gvas,
            "GVAS payloads should be identical after round-trip"
        );
    }
}
