//! Readers for the Palworld binary blobs `uesave` keeps opaque: the guild tail
//! inside `PalGroupData::remaining_data` and the `WorkerDirector` RawData byte
//! array. All multi-byte integers in these blobs are little-endian.

use crate::error::CoreError;
use crate::ue::games::palworld::PalTransform;
use crate::ue::{Double, Quat, Vector};
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

    /// Bytes already consumed — lets callers report trailing-byte errors at the
    /// exact offset.
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
            // length < 0 means unit_count >= 1, so units is non-empty and this
            // pop cannot panic.
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

/// The permutation between a guid's 16 raw on-disk bytes and its RFC 4122
/// display bytes — see [`BlobReader::read_uuid`]. An involution, so the same
/// call converts either way.
pub fn shuffle_guid_bytes(b: [u8; 16]) -> [u8; 16] {
    [
        b[3], b[2], b[1], b[0], b[7], b[6], b[5], b[4], b[11], b[10], b[9], b[8], b[15], b[14],
        b[13], b[12],
    ]
}

/// A guid's raw on-disk byte encoding, for matching or substituting guids
/// inside an opaque blob.
pub fn guid_bytes(id: Uuid) -> [u8; 16] {
    shuffle_guid_bytes(*id.as_bytes())
}

/// Inverse of [`guid_bytes`].
pub fn guid_bytes_to_uuid(raw: [u8; 16]) -> Uuid {
    Uuid::from_bytes(shuffle_guid_bytes(raw))
}

/// Adds a field name to a leaf read's error, so a truncated save reports which
/// field failed in addition to `take`'s byte offset.
fn describe_field<T>(field: &'static str, result: Result<T, CoreError>) -> Result<T, CoreError> {
    result.map_err(|err| match err {
        CoreError::Parse(msg) => CoreError::Parse(format!("{field}: {msg}")),
        other => other,
    })
}

/// `WorkerDirector` RawData is a fixed 118-byte layout, concatenated in order:
/// `id: guid` (16), `spawn_transform: FTransform` (10 doubles = 80),
/// `current_order_type: u8` (1), `current_battle_type: u8` (1),
/// `container_id: guid` (16), `trailing_bytes` (4) — putting `container_id` at
/// offset 98. Any other length is corrupt.
pub const WORKER_DIRECTOR_BLOB_LEN: usize = 118;
const WORKER_DIRECTOR_CONTAINER_ID_OFFSET: usize = 98;

pub fn worker_director_container_id(raw_data: &[u8]) -> Result<Uuid, CoreError> {
    if raw_data.len() != WORKER_DIRECTOR_BLOB_LEN {
        return Err(CoreError::Parse(format!(
            "WorkerDirector raw data must be exactly {WORKER_DIRECTOR_BLOB_LEN} byte(s), got {}",
            raw_data.len()
        )));
    }
    let mut reader = BlobReader::new(&raw_data[WORKER_DIRECTOR_CONTAINER_ID_OFFSET..]);
    describe_field("container_id", reader.read_uuid())
}

/// The whole `WorkerDirector` blob, for the two fields a placed base has to
/// retarget: `container_id`, which otherwise still names the source save's
/// worker container, and `spawn_transform`, which otherwise still sends the
/// base's workers to the coordinates it was captured at. `id` names the base
/// camp the director belongs to.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkerDirector {
    pub id: Uuid,
    pub spawn_transform: PalTransform,
    pub current_order_type: u8,
    pub current_battle_type: u8,
    pub container_id: Uuid,
    pub trailing_bytes: [u8; 4],
}

pub fn read_worker_director(raw_data: &[u8]) -> Result<WorkerDirector, CoreError> {
    if raw_data.len() != WORKER_DIRECTOR_BLOB_LEN {
        return Err(CoreError::Parse(format!(
            "WorkerDirector raw data must be exactly {WORKER_DIRECTOR_BLOB_LEN} byte(s), got {}",
            raw_data.len()
        )));
    }
    let mut reader = BlobReader::new(raw_data);
    let id = describe_field("id", reader.read_uuid())?;
    let spawn_transform = describe_field("spawn_transform", read_transform(&mut reader))?;
    let current_order_type = describe_field("current_order_type", reader.read_u8())?;
    let current_battle_type = describe_field("current_battle_type", reader.read_u8())?;
    let container_id = describe_field("container_id", reader.read_uuid())?;
    let mut trailing_bytes = [0u8; 4];
    trailing_bytes.copy_from_slice(describe_field("trailing_bytes", reader.take(4))?);
    Ok(WorkerDirector {
        id,
        spawn_transform,
        current_order_type,
        current_battle_type,
        container_id,
        trailing_bytes,
    })
}

/// An `FTransform` as ten little-endian doubles: rotation quat, translation,
/// scale — the same field order `PalTransform` itself carries.
fn read_transform(reader: &mut BlobReader) -> Result<PalTransform, CoreError> {
    let mut next = || reader.read_f64().map(Double);
    Ok(PalTransform {
        rotation: Quat {
            x: next()?,
            y: next()?,
            z: next()?,
            w: next()?,
        },
        translation: Vector {
            x: next()?,
            y: next()?,
            z: next()?,
        },
        scale: Vector {
            x: next()?,
            y: next()?,
            z: next()?,
        },
    })
}

impl WorkerDirector {
    /// Exact inverse of [`read_worker_director`]. The layout is fixed, so the
    /// blob a placement writes back is the same length uesave read.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(WORKER_DIRECTOR_BLOB_LEN);
        bytes.extend_from_slice(&guid_bytes(self.id));
        let transform = &self.spawn_transform;
        for value in [
            transform.rotation.x.0,
            transform.rotation.y.0,
            transform.rotation.z.0,
            transform.rotation.w.0,
            transform.translation.x.0,
            transform.translation.y.0,
            transform.translation.z.0,
            transform.scale.x.0,
            transform.scale.y.0,
            transform.scale.z.0,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.push(self.current_order_type);
        bytes.push(self.current_battle_type);
        bytes.extend_from_slice(&guid_bytes(self.container_id));
        bytes.extend_from_slice(&self.trailing_bytes);
        bytes
    }
}

/// `BaseCampSaveData.WorkCollection` RawData: `own_id: guid` (16),
/// `work_ids: TArray<guid>` (`u32` count + 16 bytes each), then a 4-byte tail.
/// The base's works are named here as well as by each work's own entry, so a
/// remapped blueprint has to rewrite both.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkCollection {
    pub own_id: Uuid,
    pub work_ids: Vec<Uuid>,
    pub trailing_bytes: u32,
}

pub fn read_work_collection(raw_data: &[u8]) -> Result<WorkCollection, CoreError> {
    let mut reader = BlobReader::new(raw_data);
    let own_id = describe_field("own_id", reader.read_uuid())?;
    let work_ids = describe_field("work_ids", reader.read_tarray(BlobReader::read_uuid))?;
    let trailing_bytes = describe_field("trailing_bytes", reader.read_u32())?;
    if !reader.is_at_end() {
        return Err(CoreError::Parse(format!(
            "WorkCollection raw data has {} unread byte(s) after offset {}",
            raw_data.len() - reader.position(),
            reader.position()
        )));
    }
    Ok(WorkCollection {
        own_id,
        work_ids,
        trailing_bytes,
    })
}

impl WorkCollection {
    /// Exact inverse of [`read_work_collection`]. The length is written from
    /// `work_ids`, so the list may shrink or grow.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(24 + self.work_ids.len() * 16);
        bytes.extend_from_slice(&guid_bytes(self.own_id));
        bytes.extend_from_slice(&(self.work_ids.len() as u32).to_le_bytes());
        for id in &self.work_ids {
            bytes.extend_from_slice(&guid_bytes(*id));
        }
        bytes.extend_from_slice(&self.trailing_bytes.to_le_bytes());
        bytes
    }
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
        // negative length claims i32::MIN/-ish code units; must not panic
        // computing unit_count * 2 or attempting the allocation/read.
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
        // count claims ~4 billion guid elements; must error cleanly on the
        // first short element read rather than attempting to allocate or
        // iterate that many times unboundedly.
        let mut writer = BlobWriter::default();
        writer.write_u32(u32::MAX);
        let mut reader = BlobReader::new(&writer.bytes);
        let result = reader.read_tarray(BlobReader::read_uuid);
        assert!(result.is_err());
    }

    #[test]
    fn test_worker_director_container_id() {
        let container = "a1b2c3d4-0000-1111-2222-333344445555";
        let mut blob = vec![0u8; 118];
        let display = *container.parse::<uuid::Uuid>().unwrap().as_bytes();
        blob[98..114].copy_from_slice(&shuffle_guid_bytes(display));

        let parsed = worker_director_container_id(&blob).unwrap();
        assert_eq!(container, parsed.to_string());

        assert!(worker_director_container_id(&[0u8; 117]).is_err());
    }

    #[test]
    fn test_worker_director_container_id_rejects_empty_input() {
        assert!(worker_director_container_id(&[]).is_err());
    }

    /// The blob is hand-assembled from the layout the doc comment states, so
    /// the reader is checked against the documented field offsets rather than
    /// against its own writer.
    #[test]
    fn test_read_worker_director_reads_the_documented_layout() {
        let id: uuid::Uuid = "11111111-2222-3333-4444-555555555555".parse().unwrap();
        let container: uuid::Uuid = "a1b2c3d4-0000-1111-2222-333344445555".parse().unwrap();
        let doubles = [
            1.0, 2.0, 3.0, 4.0, -320856.5, 213349.875, -417.5, 5.0, 6.0, 7.0,
        ];

        let mut blob = Vec::new();
        blob.extend_from_slice(&guid_bytes(id));
        for value in doubles {
            blob.extend_from_slice(&f64::to_le_bytes(value));
        }
        blob.push(3);
        blob.push(9);
        blob.extend_from_slice(&guid_bytes(container));
        blob.extend_from_slice(&[7, 8, 9, 10]);
        assert_eq!(blob.len(), WORKER_DIRECTOR_BLOB_LEN);

        let director = read_worker_director(&blob).unwrap();
        assert_eq!(director.id, id);
        assert_eq!(director.container_id, container);
        assert_eq!(director.current_order_type, 3);
        assert_eq!(director.current_battle_type, 9);
        assert_eq!(director.trailing_bytes, [7, 8, 9, 10]);
        assert_eq!(director.spawn_transform.rotation.x.0, 1.0);
        assert_eq!(director.spawn_transform.rotation.w.0, 4.0);
        assert_eq!(director.spawn_transform.translation.x.0, -320856.5);
        assert_eq!(director.spawn_transform.translation.y.0, 213349.875);
        assert_eq!(director.spawn_transform.translation.z.0, -417.5);
        assert_eq!(director.spawn_transform.scale.z.0, 7.0);

        assert_eq!(director.to_bytes(), blob);
        // `container_id` must land at the offset the older reader assumes.
        assert_eq!(worker_director_container_id(&blob).unwrap(), container);
    }

    #[test]
    fn test_read_worker_director_rejects_wrong_lengths() {
        assert!(read_worker_director(&[]).is_err());
        assert!(read_worker_director(&[0u8; WORKER_DIRECTOR_BLOB_LEN - 1]).is_err());
        assert!(read_worker_director(&[0u8; WORKER_DIRECTOR_BLOB_LEN + 1]).is_err());
    }

    #[test]
    fn test_guid_bytes_matches_the_reader_byte_order() {
        let raw: [u8; 16] = std::array::from_fn(|i| i as u8);
        let parsed = BlobReader::new(&raw).read_uuid().unwrap();
        assert_eq!(guid_bytes(parsed), raw);
        assert_eq!(guid_bytes_to_uuid(raw), parsed);
    }

    #[test]
    fn test_work_collection_round_trips() {
        let collection = WorkCollection {
            own_id: "a1b2c3d4-0000-1111-2222-333344445555".parse().unwrap(),
            work_ids: vec![
                "11111111-2222-3333-4444-555555555555".parse().unwrap(),
                "66666666-7777-8888-9999-aaaaaaaaaaaa".parse().unwrap(),
            ],
            trailing_bytes: 0,
        };
        let bytes = collection.to_bytes();
        assert_eq!(bytes.len(), 16 + 4 + 2 * 16 + 4);
        assert_eq!(read_work_collection(&bytes).unwrap(), collection);
    }

    #[test]
    fn test_work_collection_rejects_truncated_and_trailing_input() {
        let collection = WorkCollection {
            own_id: uuid::Uuid::nil(),
            work_ids: vec![uuid::Uuid::nil()],
            trailing_bytes: 7,
        };
        let bytes = collection.to_bytes();
        assert!(read_work_collection(&bytes[..bytes.len() - 1]).is_err());

        let mut extra = bytes.clone();
        extra.push(0);
        assert!(read_work_collection(&extra).is_err());
        assert!(read_work_collection(&[]).is_err());
    }
}
