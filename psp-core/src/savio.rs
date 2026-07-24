//! Palworld `.sav` byte I/O — the compressed-layer bookends around a
//! `crate::ue::Save`.

use crate::error::CoreError;

/// Reads a Palworld `.sav` byte payload (PlM/Oodle-compressed GVAS, or plain
/// GVAS). Reads MUST go through `session::parse_palworld_save`: it installs
/// `crate::ue::games::palworld::palworld_types()`, without which the Palworld
/// RawData codecs (guild tail, character/item containers, ...) parse as opaque
/// bytes and every typed accessor comes back empty.
pub fn read_sav_bytes(bytes: &[u8]) -> Result<crate::ue::Save, CoreError> {
    crate::session::parse_palworld_save(bytes)
}

/// Writes a `crate::ue::Save` back to its `.sav` byte payload. `uesave`'s Oodle
/// compressor emits the `PlM` magic and the `0x31` save-type byte the game
/// expects, with Mermaid/Normal settings.
pub fn write_sav_bytes(save: &crate::ue::Save) -> Result<Vec<u8>, CoreError> {
    let mut buffer = Vec::new();
    save.write_plm(&mut buffer)
        .map_err(|error| CoreError::Other(error.to_string()))?;
    Ok(buffer)
}

/// Parses already-decompressed GVAS bytes. Like `read_sav_bytes`, this MUST go
/// through `session::parse_palworld_save`: it installs the Palworld type
/// registry, without which the RawData codecs parse as opaque bytes.
pub fn read_gvas_bytes(bytes: &[u8]) -> Result<crate::ue::Save, CoreError> {
    crate::session::parse_palworld_save(bytes)
}

/// Serializes a `Save` to uncompressed GVAS, leaving compression to the caller.
/// The web worker compresses with a vendored `ooz.wasm`; native callers use
/// `write_sav_bytes`, which emits PlM/Oodle directly.
pub fn write_gvas_bytes(save: &crate::ue::Save) -> Result<Vec<u8>, CoreError> {
    let mut buffer = Vec::new();
    save.write(&mut buffer)
        .map_err(|error| CoreError::Other(error.to_string()))?;
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::player::{container_id_from, save_data_props};
    use crate::gamepass::fixture::reference_saves_dir;

    /// The GVAS pair must round-trip a save without touching compression:
    /// `write_gvas_bytes` produces uncompressed bytes distinct from the `.sav`
    /// payload, and reading them back through `read_gvas_bytes` yields usable
    /// typed output (a resolvable container id), with a second write matching
    /// the first byte-for-byte.
    #[test]
    fn gvas_pair_round_trips_without_compression() {
        let sav = std::fs::read(
            reference_saves_dir().join("00000000000000000000000000000001.sav"),
        )
        .expect("committed fixture is readable");
        let save = read_sav_bytes(&sav).expect("fixture parses");

        let gvas = write_gvas_bytes(&save).expect("serializes to GVAS");
        assert!(!gvas.is_empty(), "GVAS output is non-empty");
        assert_ne!(gvas, sav, "GVAS is the uncompressed form, not the .sav bytes");

        let reparsed = read_gvas_bytes(&gvas).expect("GVAS re-parses");
        let regvas = write_gvas_bytes(&reparsed).expect("re-serializes");
        assert_eq!(gvas, regvas, "GVAS round-trips byte-for-byte");

        // Smoke check that parsing yields usable typed output, not just bytes
        // that happen to round-trip.
        let save_data = save_data_props(&reparsed).expect("player SaveData present");
        assert!(
            container_id_from(save_data, "OtomoCharacterContainerId").is_some()
                || container_id_from(save_data, "PalStorageContainerId").is_some(),
            "typed container id readable after read_gvas_bytes"
        );
    }
}
