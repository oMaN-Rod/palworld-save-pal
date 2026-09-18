//! Reads and writes an Unreal config file in the encoding it already has. Unreal
//! writes a config as UTF-16LE with a byte-order mark once it holds a non-ASCII
//! character, so a plain UTF-8 read would refuse a perfectly valid file.
use std::io::{Error, ErrorKind};
use std::path::Path;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum IniEncoding {
    #[default]
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IniText {
    pub text: String,
    pub encoding: IniEncoding,
}

const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];
const UTF16LE_BOM: &[u8] = &[0xFF, 0xFE];
const UTF16BE_BOM: &[u8] = &[0xFE, 0xFF];

fn utf16(body: &[u8], unit: fn([u8; 2]) -> u16) -> std::io::Result<String> {
    if !body.len().is_multiple_of(2) {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "UTF-16 text has an odd number of bytes",
        ));
    }
    let units: Vec<u16> = body
        .chunks_exact(2)
        .map(|pair| unit([pair[0], pair[1]]))
        .collect();
    String::from_utf16(&units).map_err(|error| Error::new(ErrorKind::InvalidData, error))
}

fn utf8(body: &[u8]) -> std::io::Result<String> {
    String::from_utf8(body.to_vec()).map_err(|error| Error::new(ErrorKind::InvalidData, error))
}

/// Text with no byte-order mark is read as UTF-8; anything that does not decode
/// is an error rather than an empty file.
pub fn decode(bytes: &[u8]) -> std::io::Result<IniText> {
    let (text, encoding) = if let Some(body) = bytes.strip_prefix(UTF8_BOM) {
        (utf8(body)?, IniEncoding::Utf8Bom)
    } else if let Some(body) = bytes.strip_prefix(UTF16LE_BOM) {
        (utf16(body, u16::from_le_bytes)?, IniEncoding::Utf16Le)
    } else if let Some(body) = bytes.strip_prefix(UTF16BE_BOM) {
        (utf16(body, u16::from_be_bytes)?, IniEncoding::Utf16Be)
    } else {
        (utf8(bytes)?, IniEncoding::Utf8)
    };
    Ok(IniText { text, encoding })
}

pub fn encode(text: &str, encoding: IniEncoding) -> Vec<u8> {
    match encoding {
        IniEncoding::Utf8 => text.as_bytes().to_vec(),
        IniEncoding::Utf8Bom => [UTF8_BOM, text.as_bytes()].concat(),
        IniEncoding::Utf16Le => UTF16LE_BOM
            .iter()
            .copied()
            .chain(text.encode_utf16().flat_map(u16::to_le_bytes))
            .collect(),
        IniEncoding::Utf16Be => UTF16BE_BOM
            .iter()
            .copied()
            .chain(text.encode_utf16().flat_map(u16::to_be_bytes))
            .collect(),
    }
}

/// `None` when the file does not exist.
pub fn read(path: &Path) -> std::io::Result<Option<IniText>> {
    let named = |error: Error| Error::new(error.kind(), format!("{}: {error}", path.display()));
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(named(error)),
    };
    decode(&bytes).map(Some).map_err(named)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "[PalModSettings]\r\nWorkshopRootDir=C:\\Users\\José\\w\r\n";

    #[test]
    fn every_encoding_round_trips_with_its_mark() {
        for encoding in [
            IniEncoding::Utf8,
            IniEncoding::Utf8Bom,
            IniEncoding::Utf16Le,
            IniEncoding::Utf16Be,
        ] {
            let bytes = encode(TEXT, encoding);
            let decoded = decode(&bytes).unwrap();
            assert_eq!(decoded.text, TEXT, "{encoding:?}");
            assert_eq!(decoded.encoding, encoding);
        }
        assert_eq!(
            &encode(TEXT, IniEncoding::Utf16Le)[..4],
            &[0xFF, 0xFE, b'[', 0]
        );
    }

    #[test]
    fn undecodable_bytes_are_an_error() {
        assert!(decode(&[0xC3, 0x28, b'A']).is_err());
        assert!(decode(&[0xFF, 0xFE, b'A']).is_err());
        assert!(decode(&[0xFF, 0xFE, 0x00, 0xD8]).is_err());
    }

    #[test]
    fn a_missing_file_reads_as_none() {
        let scratch = tempfile::tempdir().unwrap();
        assert_eq!(read(&scratch.path().join("absent.ini")).unwrap(), None);
    }
}
