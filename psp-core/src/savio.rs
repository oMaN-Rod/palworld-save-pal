//! Palworld `.sav` byte I/O — the compressed-layer bookends around a
//! `uesave::Save`.

use crate::error::CoreError;

/// Reads a Palworld `.sav` byte payload (PlM/Oodle-compressed GVAS, or plain
/// GVAS). Reads MUST go through `session::parse_palworld_save`: it installs
/// `uesave::games::palworld::palworld_types()`, without which the Palworld
/// RawData codecs (guild tail, character/item containers, ...) parse as opaque
/// bytes and every typed accessor comes back empty.
pub fn read_sav_bytes(bytes: &[u8]) -> Result<uesave::Save, CoreError> {
    crate::session::parse_palworld_save(bytes)
}

/// Writes a `uesave::Save` back to its `.sav` byte payload. `uesave`'s Oodle
/// compressor emits the `PlM` magic and the `0x31` save-type byte the game
/// expects, with Mermaid/Normal settings.
pub fn write_sav_bytes(save: &uesave::Save) -> Result<Vec<u8>, CoreError> {
    let mut buffer = Vec::new();
    save.write_compressed(&mut buffer, uesave::compression::CompressionFormat::Oodle)
        .map_err(|error| CoreError::Other(error.to_string()))?;
    Ok(buffer)
}
