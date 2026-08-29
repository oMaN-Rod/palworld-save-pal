//! In-memory sav<->json conversion. The JSON shape is `uesave`'s own schema.
//!
//! Both directions go through `savio`, never `uesave`'s own Oodle: on wasm32
//! that codec is not linked, and a `PlM` container would fail with
//! "Compression support not enabled" on a target that has a bridge lent to it.

use crate::error::CoreError;

pub fn sav_to_json_string(sav_bytes: &[u8]) -> Result<String, CoreError> {
    let save = crate::savio::read_sav_bytes(sav_bytes)?;
    serde_json::to_string(&save).map_err(|error| CoreError::Other(error.to_string()))
}

pub fn json_to_sav_bytes(json_bytes: &[u8]) -> Result<Vec<u8>, CoreError> {
    let save: crate::ue::Save =
        serde_json::from_slice(json_bytes).map_err(|error| CoreError::Parse(error.to_string()))?;
    crate::savio::write_sav_bytes(&save)
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

    /// The web build links no Oodle codec and lends one instead, so a `PlM`
    /// container has to be written and read through that bridge. Going to
    /// `uesave`'s own compressor there fails with "Compression support not
    /// enabled", which is what the browser's raw editor used to hit.
    #[test]
    fn conversion_uses_a_lent_codec_when_one_is_installed() {
        crate::oodle::set_bridge(
            |data| Ok([b"OOZ".as_slice(), data].concat()),
            |payload, _| Ok(payload[3..].to_vec()),
        );
        let sav_bytes =
            std::fs::read(reference_saves_dir().join("00000000000000000000000000000001.sav"))
                .unwrap();

        let json = sav_to_json_string(&sav_bytes).unwrap();
        let rebuilt = json_to_sav_bytes(json.as_bytes()).unwrap();

        assert_eq!(&rebuilt[8..12], b"PlM1");
        assert_eq!(
            &rebuilt[12..15],
            b"OOZ",
            "the container was not compressed by the lent codec"
        );
        assert_eq!(
            sav_to_json_string(&rebuilt).unwrap(),
            json,
            "reading back through the lent codec lost the save"
        );
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
