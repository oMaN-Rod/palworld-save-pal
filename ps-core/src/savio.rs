//! Palworld `.sav` byte I/O — the compressed-layer bookends around a
//! `crate::ue::Save`.

use crate::error::CoreError;
use crate::ue::compression::{CompressionHeader, MagicBytes};

/// The save-type byte a `PlM` container carries.
const PLM_SAVE_TYPE: u8 = 0x31;

/// Reads a Palworld `.sav` byte payload (PlM/Oodle-compressed GVAS, or plain
/// GVAS). Reads MUST go through `session::parse_palworld_save`: it installs
/// `crate::ue::games::palworld::palworld_types()`, without which the Palworld
/// RawData codecs (guild tail, character/item containers, ...) parse as opaque
/// bytes and every typed accessor comes back empty.
pub fn read_sav_bytes(bytes: &[u8]) -> Result<crate::ue::Save, CoreError> {
    if crate::oodle::is_installed() {
        if let Some(gvas) = plm_payload(bytes)? {
            return crate::session::parse_palworld_save(&gvas);
        }
    }
    crate::session::parse_palworld_save(bytes)
}

/// Writes a `crate::ue::Save` back to its `.sav` byte payload. `uesave`'s Oodle
/// compressor emits the `PlM` magic and the `0x31` save-type byte the game
/// expects, with Mermaid/Normal settings.
pub fn write_sav_bytes(save: &crate::ue::Save) -> Result<Vec<u8>, CoreError> {
    if crate::oodle::is_installed() {
        return write_plm_bytes(save);
    }
    let mut buffer = Vec::new();
    save.write_plm(&mut buffer)
        .map_err(|error| CoreError::Other(error.to_string()))?;
    Ok(buffer)
}

/// The `PlM` bookends spelled out in Rust so a host-lent codec can stand in for
/// the linked one -- `uesave`'s own `write_plm`/`decompress_save` reach for
/// ooz-rs directly, which the wasm32 build has none of. Both halves are gated on
/// `oodle::is_installed`, so a bridge owns the whole container or none of it.
fn write_plm_bytes(save: &crate::ue::Save) -> Result<Vec<u8>, CoreError> {
    let gvas = write_gvas_bytes(save)?;
    let compressed = crate::oodle::compress(&gvas)?;

    let header = CompressionHeader {
        uncompressed_len: gvas.len() as u32,
        compressed_len: compressed.len() as u32,
        magic_bytes: MagicBytes::PLM,
        save_type: PLM_SAVE_TYPE,
        data_offset: 12,
    };
    let mut out = Vec::with_capacity(header.data_offset + compressed.len());
    header
        .write(&mut out)
        .map_err(|error| CoreError::Other(error.to_string()))?;
    out.extend_from_slice(&compressed);
    Ok(out)
}

/// `Some(gvas)` when `bytes` is a `PlM` container, `None` for every other shape
/// (plain GVAS, PlZ, CNK) -- those `uesave` decodes without an Oodle codec, so
/// they are left to it.
fn plm_payload(bytes: &[u8]) -> Result<Option<Vec<u8>>, CoreError> {
    let header = CompressionHeader::read(&mut &bytes[..])
        .map_err(|error| CoreError::Parse(error.to_string()))?;
    let Some(header) = header.filter(|header| header.magic_bytes == MagicBytes::PLM) else {
        return Ok(None);
    };

    let end = header.data_offset + header.compressed_len as usize;
    if end > bytes.len() {
        return Err(CoreError::Parse(format!(
            "PlM container declares {} compressed bytes but only {} follow its header",
            header.compressed_len,
            bytes.len() - header.data_offset.min(bytes.len())
        )));
    }
    let payload = crate::oodle::decompress(
        &bytes[header.data_offset..end],
        header.uncompressed_len as usize,
    )?;
    if payload.len() != header.uncompressed_len as usize {
        return Err(CoreError::Parse(format!(
            "PlM container declares {} uncompressed bytes, the codec produced {}",
            header.uncompressed_len,
            payload.len()
        )));
    }
    Ok(Some(payload))
}

/// Parses already-decompressed GVAS bytes. Like `read_sav_bytes`, this MUST go
/// through `session::parse_palworld_save` to install the Palworld type registry.
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

    /// wasm32 has no toolchain for ooz-rs's C++, so the web build compiles
    /// without the `oodle` feature and `write_plm` fails outright there. A host
    /// carrying its own Oodle codec -- the browser worker's `ooz.wasm` -- lends
    /// it through `oodle::set_bridge`, and from then on the bridge owns the PlM
    /// layer on BOTH sides: bytes written through it must read back through it,
    /// or a blueprint stored in the web library can never be loaded again.
    ///
    /// The stand-in codec here is the identity function, which no real Oodle
    /// decoder would accept -- that is what makes the read half prove routing
    /// rather than falling through to the linked codec.
    #[test]
    fn a_registered_bridge_owns_the_plm_layer_on_both_sides() {
        let sav = std::fs::read(
            reference_saves_dir().join("00000000000000000000000000000001.sav"),
        )
        .expect("committed fixture is readable");
        let save = read_sav_bytes(&sav).expect("fixture parses");
        let gvas = write_gvas_bytes(&save).expect("serializes to GVAS");

        crate::oodle::set_bridge(|data| Ok(data.to_vec()), |payload, _| Ok(payload.to_vec()));

        let written = write_sav_bytes(&save).expect("writes through the bridge");
        assert_eq!(&written[8..11], b"PlM", "the bridge writes Palworld's PlM container");
        assert_eq!(written[11], 0x31, "with the save type the game expects");
        assert_eq!(
            u32::from_le_bytes(written[0..4].try_into().unwrap()) as usize,
            gvas.len(),
            "the header states the uncompressed length"
        );
        assert_eq!(
            u32::from_le_bytes(written[4..8].try_into().unwrap()) as usize,
            written.len() - 12,
            "and the length of what the codec actually produced"
        );
        assert!(written[12..] == gvas[..], "the identity codec's output is the GVAS body");

        let reparsed = read_sav_bytes(&written).expect("reads back through the bridge");
        assert_eq!(
            write_gvas_bytes(&reparsed).expect("re-serializes"),
            gvas,
            "a bridge-written save round-trips through the bridge"
        );
    }

    /// The GVAS pair must round-trip a save without touching compression:
    /// `write_gvas_bytes` produces uncompressed bytes distinct from the `.sav` payload,
    /// reading them back yields usable typed output, and a second write matches
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

        // Smoke check that parsing yields usable typed output, not just round-tripping bytes.
        let save_data = save_data_props(&reparsed).expect("player SaveData present");
        assert!(
            container_id_from(save_data, "OtomoCharacterContainerId").is_some()
                || container_id_from(save_data, "PalStorageContainerId").is_some(),
            "typed container id readable after read_gvas_bytes"
        );
    }

    fn corpus_player_saves() -> Vec<(String, Vec<u8>)> {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/saves/v1_relics/Players");
        let mut files = Vec::new();
        for entry in std::fs::read_dir(&dir)
            .expect("the corpus fixture is checked in")
            .flatten()
        {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("sav") {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string();
            files.push((name, std::fs::read(&path).expect("read fixture player save")));
        }
        files.sort_by(|left, right| left.0.cmp(&right.0));
        files
    }

    /// A no-edit resave of a player `.sav` is NOT byte-identical to the game's own file:
    /// the game's Oodle encoder and this crate's spell the same payload differently, and
    /// the container's `compressed_len` moves in both directions. What must survive is the
    /// GVAS payload inside the container, which is what the game reads -- and every caller
    /// that loads a player without editing anything rewrites that file from this round
    /// trip. `decompress_save` rather than `plm_payload`: the latter routes through
    /// `oodle::decompress`, which answers only on a thread that has been lent a bridge,
    /// and a native test has none.
    #[test]
    fn untouched_player_sav_payload_survives_a_resave() {
        let files = corpus_player_saves();
        assert!(
            files.len() > 1,
            "the corpus fixture must carry several player saves"
        );
        assert!(
            files.iter().any(|(name, _)| name.ends_with("_dps.sav")),
            "the corpus fixture must carry a _dps.sav"
        );

        let mut recompressed = 0;
        for (name, original) in &files {
            let save = read_sav_bytes(original).unwrap_or_else(|e| panic!("{name} parses: {e}"));
            let rewritten =
                write_sav_bytes(&save).unwrap_or_else(|e| panic!("{name} writes back: {e}"));

            let before = crate::ue::compression::decompress_save(&mut &original[..])
                .unwrap_or_else(|e| panic!("{name} decompresses: {e}"));
            let after = crate::ue::compression::decompress_save(&mut &rewritten[..])
                .unwrap_or_else(|e| panic!("{name} resave decompresses: {e}"));

            assert_eq!(
                after.len(),
                before.len(),
                "{name}: the resaved payload changed length"
            );
            let first_diff = before.iter().zip(after.iter()).position(|(a, b)| a != b);
            assert_eq!(
                first_diff, None,
                "{name}: the resaved GVAS payload differs from the game's own"
            );
            if rewritten != *original {
                recompressed += 1;
            }
        }
        assert!(
            recompressed > 0,
            "every file resaved byte-identically, so this proves nothing a byte-level \
             comparison would not have caught"
        );
    }
}
