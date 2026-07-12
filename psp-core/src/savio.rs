//! Palworld `.sav` byte I/O — the compressed-layer bookends around a
//! `uesave::Save`.
//!
//! Port of the read/compress halves of Python's
//! `palworld_save_tools.palsav` (`decompress_sav_to_gvas` /
//! `compress_gvas_to_sav`) as they are used by
//! `SerializationMixin` (`game/mixins/serialization.py`): every save-out path
//! (`sav`, `player_gvas_files`, `level_meta_sav`, ...) funnels a GVAS tree
//! through `compress_gvas_to_sav(raw, 0x31)`, which emits the PlM/Oodle
//! container with save-type byte `0x31`.
//!
//! Phase 1 shipped `session::parse_palworld_save` (the read side); this
//! module is Phase 2's write side plus a thin read alias so callers have one
//! `savio::{read,write}_sav_bytes` pair for the compressed layer. The read
//! path MUST go through `parse_palworld_save` because it installs
//! `uesave::games::palworld::palworld_types()` — without that registry the
//! Palworld RawData codecs (guild tail, character/item containers, ...) parse
//! as opaque bytes and every typed accessor this port relies on comes back
//! empty.

use crate::error::CoreError;

/// Reads a Palworld `.sav` byte payload (PlM/Oodle-compressed GVAS, or plain
/// GVAS) into a `uesave::Save`. A thin alias over
/// `session::parse_palworld_save` so the compressed-layer read and write
/// helpers live together; see that function for the parse contract (loud
/// failure on unparseable properties, PlZ/CNK surfacing as `CoreError::Parse`).
pub fn read_sav_bytes(bytes: &[u8]) -> Result<uesave::Save, CoreError> {
    crate::session::parse_palworld_save(bytes)
}

/// Writes a `uesave::Save` back to its PlM/Oodle `.sav` byte payload,
/// mirroring Python `compress_gvas_to_sav(raw, 0x31)`. uesave's
/// `compress_save` emits the `PlM` magic and the `0x31` save-type byte
/// (verified: `uesave/src/compression.rs` `compress_save` builds a
/// `CompressionHeader { magic_bytes: PLM, save_type: 0x31, .. }`), and uses
/// the same `OodleCompressor::Mermaid` / `OodleLevel::Normal` settings Python
/// drives through the `ooz` C library — so this output can match Python's
/// byte-for-byte (proven at the GVAS layer by the resave gate in
/// `tests/resave_bytes.rs`, and at the compressed layer by Phase-2 parity
/// fixtures in Task 15).
pub fn write_sav_bytes(save: &uesave::Save) -> Result<Vec<u8>, CoreError> {
    let mut buffer = Vec::new();
    save.write_compressed(&mut buffer, uesave::compression::CompressionFormat::Oodle)
        .map_err(|error| CoreError::Other(error.to_string()))?;
    Ok(buffer)
}
