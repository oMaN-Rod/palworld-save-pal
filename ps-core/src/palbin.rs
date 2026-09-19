//! A cursor over opaque byte blobs, and the raw <-> display guid byte permutation
//! Palworld save data uses, for the callers that still substitute guids inside
//! blobs `uesave` does not model as typed structs. All multi-byte integers read
//! here are little-endian.

use crate::error::CoreError;
use uuid::Uuid;

/// Cursor over an opaque byte blob. Every read is bounds-checked against the
/// remaining bytes; a truncated or maliciously long declared length produces a
/// `CoreError::Parse` naming the offset, never a panic.
pub struct BlobReader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> BlobReader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    pub fn is_at_end(&self) -> bool {
        self.position == self.bytes.len()
    }

    /// Bytes already consumed, so callers can report trailing-byte errors at the exact offset.
    pub fn position(&self) -> usize {
        self.position
    }

    /// Bounds-checked slice of the next `count` bytes. `position + count` uses
    /// `checked_add` so a `count` read straight out of the blob can never wrap
    /// or index past the end of `bytes`.
    fn take(&mut self, count: usize) -> Result<&'a [u8], CoreError> {
        let end = self
            .position
            .checked_add(count)
            .filter(|&end| end <= self.bytes.len())
            .ok_or_else(|| {
                CoreError::Parse(format!(
                    "unexpected end of blob: need {count} more byte(s) at offset {} \
                     (blob is {} byte(s) long)",
                    self.position,
                    self.bytes.len()
                ))
            })?;
        let slice = &self.bytes[self.position..end];
        self.position = end;
        Ok(slice)
    }

    pub fn skip(&mut self, count: usize) -> Result<(), CoreError> {
        self.take(count).map(|_| ())
    }

    pub fn read_u8(&mut self) -> Result<u8, CoreError> {
        Ok(self.take(1)?[0])
    }

    pub fn read_u32(&mut self) -> Result<u32, CoreError> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes(
            bytes.try_into().expect("take(4) yields 4 bytes"),
        ))
    }

    pub fn read_i32(&mut self) -> Result<i32, CoreError> {
        let bytes = self.take(4)?;
        Ok(i32::from_le_bytes(
            bytes.try_into().expect("take(4) yields 4 bytes"),
        ))
    }

    pub fn read_i64(&mut self) -> Result<i64, CoreError> {
        let bytes = self.take(8)?;
        Ok(i64::from_le_bytes(
            bytes.try_into().expect("take(8) yields 8 bytes"),
        ))
    }

    pub fn read_f64(&mut self) -> Result<f64, CoreError> {
        let bytes = self.take(8)?;
        Ok(f64::from_le_bytes(
            bytes.try_into().expect("take(8) yields 8 bytes"),
        ))
    }

    /// Palworld guid: 16 raw bytes shuffled into RFC 4122 display order —
    /// raw `[b0..b15]` -> display `[b3,b2,b1,b0, b7,b6, b5,b4, b11,b10, b9,b8, b15,b14,b13,b12]`.
    /// The permutation is an involution, so the same shuffle converts display
    /// order back to raw order.
    pub fn read_uuid(&mut self) -> Result<Uuid, CoreError> {
        let b = self.take(16)?;
        Ok(Uuid::from_bytes([
            b[3], b[2], b[1], b[0], b[7], b[6], b[5], b[4], b[11], b[10], b[9], b[8], b[15], b[14],
            b[13], b[12],
        ]))
    }

    /// Unreal fstring: `i32` length prefix.
    /// * `0` -> empty string, no bytes follow.
    /// * `> 0` -> that many UTF-8 bytes, the last being the NUL terminator.
    /// * `< 0` -> `|length|` UTF-16LE code units, the last being the NUL.
    ///
    /// The terminator is dropped unconditionally, not checked for.
    pub fn read_string(&mut self) -> Result<String, CoreError> {
        let length = self.read_i32()?;
        if length == 0 {
            return Ok(String::new());
        }
        if length < 0 {
            let unit_count = length.unsigned_abs() as usize;
            let byte_count = unit_count.checked_mul(2).ok_or_else(|| {
                CoreError::Parse(format!(
                    "fstring length overflow: {unit_count} utf-16 code unit(s) at offset {}",
                    self.position
                ))
            })?;
            let raw = self.take(byte_count)?;
            let mut units: Vec<u16> = raw
                .chunks_exact(2)
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .collect();
            // length < 0 means unit_count >= 1, so this pop cannot panic.
            units.pop();
            Ok(String::from_utf16_lossy(&units))
        } else {
            let byte_count = length as usize;
            let raw = self.take(byte_count)?;
            // length > 0 here, so byte_count >= 1 and raw is non-empty.
            let without_terminator = &raw[..raw.len() - 1];
            Ok(String::from_utf8_lossy(without_terminator).into_owned())
        }
    }

    /// Unreal `TArray`: `u32` element count followed by that many elements.
    /// A hostile count cannot cause unbounded work: the `Result` collect
    /// short-circuits on the first element that runs out of bytes, so
    /// iterations are bounded by the blob's length, not the declared count.
    pub fn read_tarray<T>(
        &mut self,
        mut read_element: impl FnMut(&mut Self) -> Result<T, CoreError>,
    ) -> Result<Vec<T>, CoreError> {
        let count = self.read_u32()?;
        (0..count).map(|_| read_element(self)).collect()
    }
}

/// The permutation between a guid's 16 raw on-disk bytes and its RFC 4122 display
/// bytes (see [`BlobReader::read_uuid`]). An involution: the same call converts either way.
pub fn shuffle_guid_bytes(b: [u8; 16]) -> [u8; 16] {
    [
        b[3], b[2], b[1], b[0], b[7], b[6], b[5], b[4], b[11], b[10], b[9], b[8], b[15], b[14],
        b[13], b[12],
    ]
}

/// A guid's raw on-disk byte encoding, for matching or substituting guids inside a blob.
pub fn guid_bytes(id: Uuid) -> [u8; 16] {
    shuffle_guid_bytes(*id.as_bytes())
}

pub fn guid_bytes_to_uuid(raw: [u8; 16]) -> Uuid {
    Uuid::from_bytes(shuffle_guid_bytes(raw))
}

#[cfg(test)]
pub(crate) mod test_bytes {
    /// Test-only writer, the exact inverse of `BlobReader`.
    #[derive(Default)]
    pub struct BlobWriter {
        pub bytes: Vec<u8>,
    }

    impl BlobWriter {
        pub fn write_raw(&mut self, raw: &[u8]) {
            self.bytes.extend_from_slice(raw);
        }
        pub fn write_u8(&mut self, value: u8) {
            self.bytes.push(value);
        }
        pub fn write_u32(&mut self, value: u32) {
            self.write_raw(&value.to_le_bytes());
        }
        pub fn write_i32(&mut self, value: i32) {
            self.write_raw(&value.to_le_bytes());
        }
        /// ASCII fstring: length includes the trailing NUL
        pub fn write_string(&mut self, text: &str) {
            assert!(text.is_ascii());
            self.write_i32(text.len() as i32 + 1);
            self.write_raw(text.as_bytes());
            self.write_u8(0);
        }
    }

    pub use super::shuffle_guid_bytes;
}

#[cfg(test)]
mod tests {
    use super::test_bytes::*;
    use super::*;

    #[test]
    fn test_read_uuid_uses_mixed_endian_byte_order() {
        let raw: Vec<u8> = (0u8..16).collect();
        let parsed = BlobReader::new(&raw).read_uuid().unwrap();
        assert_eq!("03020100-0706-0504-0b0a-09080f0e0d0c", parsed.to_string());
    }

    #[test]
    fn test_read_string_ascii_and_utf16() {
        let mut ascii = BlobWriter::default();
        ascii.write_string("Guild Name");
        assert_eq!(
            "Guild Name",
            BlobReader::new(&ascii.bytes).read_string().unwrap()
        );

        // UTF-16LE: negative length, includes trailing NUL code unit
        let mut utf16 = BlobWriter::default();
        utf16.write_i32(-3);
        utf16.write_raw(&[0x42, 0x30, 0x44, 0x30, 0x00, 0x00]); // "あい\0"
        assert_eq!("あい", BlobReader::new(&utf16.bytes).read_string().unwrap());

        let mut empty = BlobWriter::default();
        empty.write_i32(0);
        assert_eq!("", BlobReader::new(&empty.bytes).read_string().unwrap());
    }

    #[test]
    fn test_read_string_rejects_truncated_ascii_body() {
        // length prefix claims 10 bytes follow, but none do
        let mut writer = BlobWriter::default();
        writer.write_i32(10);
        assert!(BlobReader::new(&writer.bytes).read_string().is_err());
    }

    #[test]
    fn test_read_string_rejects_absurd_utf16_length() {
        // negative length claims i32::MIN code units; must not panic computing
        // unit_count * 2 nor attempt the allocation.
        let mut writer = BlobWriter::default();
        writer.write_i32(i32::MIN);
        assert!(BlobReader::new(&writer.bytes).read_string().is_err());
    }

    #[test]
    fn test_blob_reader_skip_and_reads_reject_truncated_input() {
        assert!(BlobReader::new(&[]).skip(1).is_err());
        assert!(BlobReader::new(&[]).read_u8().is_err());
        assert!(BlobReader::new(&[0, 0, 0]).read_u32().is_err());
        assert!(BlobReader::new(&[0, 0, 0]).read_i32().is_err());
        assert!(BlobReader::new(&[0; 7]).read_i64().is_err());
        assert!(BlobReader::new(&[0; 15]).read_uuid().is_err());
    }

    #[test]
    fn test_read_tarray_rejects_oversized_count_without_panicking() {
        // count claims ~4 billion guid elements; must error cleanly on the first short
        // element read rather than iterating that many times.
        let mut writer = BlobWriter::default();
        writer.write_u32(u32::MAX);
        let mut reader = BlobReader::new(&writer.bytes);
        let result = reader.read_tarray(BlobReader::read_uuid);
        assert!(result.is_err());
    }

    #[test]
    fn test_guid_bytes_matches_the_reader_byte_order() {
        let raw: [u8; 16] = std::array::from_fn(|i| i as u8);
        let parsed = BlobReader::new(&raw).read_uuid().unwrap();
        assert_eq!(guid_bytes(parsed), raw);
        assert_eq!(guid_bytes_to_uuid(raw), parsed);
    }
}
