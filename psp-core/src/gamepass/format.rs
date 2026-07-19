//! Binary codec for wgs `containers.index` files and their `container.<seq>` file lists.
//! All integers are little-endian; all strings are UTF-16LE.

use std::io::{Read, Write};
use std::path::Path;

use crate::dto::ordered_map::OrderedMap;
use crate::error::CoreError;

pub const CONTAINER_INDEX_VERSION: u32 = 0xE;
pub const CONTAINER_FILE_LIST_VERSION: u32 = 4;

const FILETIME_UNIX_EPOCH_TICKS: u64 = 116_444_736_000_000_000;

/// Windows FILETIME: 100 ns ticks since 1601-01-01 UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Filetime(pub u64);

impl Filetime {
    pub fn from_unix_seconds(seconds: f64) -> Self {
        Filetime((seconds * 10_000_000.0) as u64 + FILETIME_UNIX_EPOCH_TICKS)
    }

    pub fn to_unix_seconds(&self) -> f64 {
        (self.0 as f64 - FILETIME_UNIX_EPOCH_TICKS as f64) / 10_000_000.0
    }

    pub fn now() -> Self {
        let since_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        Self::from_unix_seconds(since_epoch.as_secs_f64())
    }
}

/// Directory / blob file name for a GUID: uppercase hex of the GUID's mixed-endian
/// (`bytes_le`) byte order, which is what Windows writes on disk.
pub fn guid_file_name(id: &uuid::Uuid) -> String {
    id.to_bytes_le()
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect()
}

fn read_u32(reader: &mut impl Read) -> Result<u32, CoreError> {
    let mut buffer = [0u8; 4];
    reader.read_exact(&mut buffer)?;
    Ok(u32::from_le_bytes(buffer))
}

fn read_u64(reader: &mut impl Read) -> Result<u64, CoreError> {
    let mut buffer = [0u8; 8];
    reader.read_exact(&mut buffer)?;
    Ok(u64::from_le_bytes(buffer))
}

fn read_u8(reader: &mut impl Read) -> Result<u8, CoreError> {
    let mut buffer = [0u8; 1];
    reader.read_exact(&mut buffer)?;
    Ok(buffer[0])
}

/// u32 code-unit count + UTF-16LE payload.
fn read_utf16_string(reader: &mut impl Read) -> Result<String, CoreError> {
    let unit_count = read_u32(reader)? as usize;
    if unit_count == 0 {
        return Ok(String::new());
    }
    let mut bytes = vec![0u8; unit_count * 2];
    reader.read_exact(&mut bytes)?;
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    String::from_utf16(&units)
        .map_err(|error| CoreError::Parse(format!("invalid UTF-16 container string: {error}")))
}

fn write_utf16_string(writer: &mut impl Write, value: &str) -> Result<(), CoreError> {
    let units: Vec<u16> = value.encode_utf16().collect();
    writer.write_all(&(units.len() as u32).to_le_bytes())?;
    for unit in units {
        writer.write_all(&unit.to_le_bytes())?;
    }
    Ok(())
}

/// Fixed-width `char_count` UTF-16LE code units, NUL-padded; trailing NULs are
/// stripped on read.
fn read_utf16_fixed(reader: &mut impl Read, char_count: usize) -> Result<String, CoreError> {
    let mut bytes = vec![0u8; char_count * 2];
    reader.read_exact(&mut bytes)?;
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    let value = String::from_utf16(&units)
        .map_err(|error| CoreError::Parse(format!("invalid UTF-16 container string: {error}")))?;
    Ok(value.trim_end_matches('\0').to_string())
}

fn write_utf16_fixed(
    writer: &mut impl Write,
    value: &str,
    char_count: usize,
) -> Result<(), CoreError> {
    let units: Vec<u16> = value.encode_utf16().collect();
    let byte_len = char_count * 2;
    if units.len() * 2 > byte_len {
        return Err(CoreError::Parse(format!(
            "container file name too long for fixed {char_count}-char field: {value}"
        )));
    }
    for unit in &units {
        writer.write_all(&unit.to_le_bytes())?;
    }
    let padding = byte_len - units.len() * 2;
    writer.write_all(&vec![0u8; padding])?;
    Ok(())
}

/// Reads a fixed 64-char UTF-16LE file name field (128 bytes).
pub(crate) fn read_utf16_fixed_64(reader: &mut impl Read) -> Result<String, CoreError> {
    read_utf16_fixed(reader, 64)
}

/// Writes a fixed 64-char UTF-16LE file name field (128 bytes).
pub(crate) fn write_utf16_fixed_64(writer: &mut impl Write, value: &str) -> Result<(), CoreError> {
    write_utf16_fixed(writer, value, 64)
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContainerEntry {
    pub container_name: String,
    pub cloud_id: String,
    pub seq: u8,
    pub flag: u32,
    pub container_uuid: uuid::Uuid,
    pub mtime: Filetime,
    pub size: u64,
}

impl ContainerEntry {
    pub fn read(reader: &mut impl Read) -> Result<Self, CoreError> {
        let container_name = read_utf16_string(reader)?;
        let container_name_repeated = read_utf16_string(reader)?;
        if container_name != container_name_repeated {
            return Err(CoreError::Parse(format!(
                "Container name mismatch: {container_name} != {container_name_repeated}"
            )));
        }
        let cloud_id = read_utf16_string(reader)?;
        let seq = read_u8(reader)?;
        let flag = read_u32(reader)?;
        if (cloud_id.is_empty() && flag & 4 == 0) || (!cloud_id.is_empty() && flag & 4 != 0) {
            return Err(CoreError::Parse(
                "Mismatch between cloud id and flag state".to_string(),
            ));
        }
        let mut uuid_bytes = [0u8; 16];
        reader.read_exact(&mut uuid_bytes)?;
        let container_uuid = uuid::Uuid::from_bytes(uuid_bytes);
        let mtime = Filetime(read_u64(reader)?);
        let _reserved = read_u64(reader)?;
        let size = read_u64(reader)?;
        Ok(ContainerEntry {
            container_name,
            cloud_id,
            seq,
            flag,
            container_uuid,
            mtime,
            size,
        })
    }

    pub fn write(&self, writer: &mut impl Write) -> Result<(), CoreError> {
        write_utf16_string(writer, &self.container_name)?;
        write_utf16_string(writer, &self.container_name)?;
        write_utf16_string(writer, &self.cloud_id)?;
        writer.write_all(&[self.seq])?;
        writer.write_all(&self.flag.to_le_bytes())?;
        writer.write_all(self.container_uuid.as_bytes())?;
        writer.write_all(&self.mtime.0.to_le_bytes())?;
        writer.write_all(&0u64.to_le_bytes())?;
        writer.write_all(&self.size.to_le_bytes())?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContainerIndex {
    pub flag1: u32,
    pub package_name: String,
    pub mtime: Filetime,
    pub flag2: u32,
    pub index_uuid: String,
    pub unknown: u64,
    pub containers: Vec<ContainerEntry>,
}

impl ContainerIndex {
    pub fn read(reader: &mut impl Read) -> Result<Self, CoreError> {
        let version = read_u32(reader)?;
        if version != CONTAINER_INDEX_VERSION {
            return Err(CoreError::Parse(format!(
                "Unsupported container index version: {version}"
            )));
        }
        let container_count = read_u32(reader)?;
        let flag1 = read_u32(reader)?;
        let package_name = read_utf16_string(reader)?;
        let mtime = Filetime(read_u64(reader)?);
        let flag2 = read_u32(reader)?;
        let index_uuid = read_utf16_string(reader)?;
        let unknown = read_u64(reader)?;
        let mut containers = Vec::with_capacity(container_count as usize);
        for _ in 0..container_count {
            containers.push(ContainerEntry::read(reader)?);
        }
        Ok(ContainerIndex {
            flag1,
            package_name,
            mtime,
            flag2,
            index_uuid,
            unknown,
            containers,
        })
    }

    pub fn read_from_dir(container_dir: &Path) -> Result<Self, CoreError> {
        let index_path = container_dir.join("containers.index");
        if !index_path.exists() {
            return Err(CoreError::Other(format!(
                "Container index not found: {}",
                index_path.display()
            )));
        }
        let bytes = std::fs::read(&index_path)?;
        Self::read(&mut std::io::Cursor::new(bytes))
    }

    pub fn write(&self, writer: &mut impl Write) -> Result<(), CoreError> {
        writer.write_all(&CONTAINER_INDEX_VERSION.to_le_bytes())?;
        writer.write_all(&(self.containers.len() as u32).to_le_bytes())?;
        writer.write_all(&self.flag1.to_le_bytes())?;
        write_utf16_string(writer, &self.package_name)?;
        writer.write_all(&self.mtime.0.to_le_bytes())?;
        writer.write_all(&self.flag2.to_le_bytes())?;
        write_utf16_string(writer, &self.index_uuid)?;
        writer.write_all(&self.unknown.to_le_bytes())?;
        for entry in &self.containers {
            entry.write(writer)?;
        }
        Ok(())
    }

    pub fn write_to_dir(&self, container_dir: &Path) -> Result<(), CoreError> {
        let mut buffer = Vec::new();
        self.write(&mut buffer)?;
        std::fs::write(container_dir.join("containers.index"), buffer)?;
        Ok(())
    }

    /// Latest container per key (`Level`, `LevelMeta`, `LocalData`, `WorldOption`,
    /// `Players-<HEX>`, `Players-<HEX>_dps`) for one save id. "Latest" is highest
    /// `seq`, breaking ties on newest `mtime`.
    pub fn latest_save_containers(&self, save_id: &str) -> OrderedMap<String, ContainerEntry> {
        let prefix = format!("{save_id}-");
        let mut latest: OrderedMap<String, ContainerEntry> = OrderedMap::new();
        for entry in &self.containers {
            if !entry.container_name.starts_with(&prefix) {
                continue;
            }
            let key = if entry.container_name.contains("Players-") {
                let player_suffix = entry
                    .container_name
                    .split("Players-")
                    .last()
                    .unwrap_or_default();
                format!("Players-{player_suffix}")
            } else if entry.container_name.contains("LocalData") {
                "LocalData".to_string()
            } else if entry.container_name.contains("LevelMeta") {
                "LevelMeta".to_string()
            } else if entry.container_name.contains("Level") {
                "Level".to_string()
            } else if entry.container_name.contains("WorldOption") {
                "WorldOption".to_string()
            } else {
                continue;
            };
            let replace = match latest.get(&key) {
                None => true,
                Some(current) => {
                    entry.seq > current.seq
                        || (entry.seq == current.seq && entry.mtime > current.mtime)
                }
            };
            if replace {
                latest.insert(key, entry.clone());
            }
        }
        latest
    }
}

/// One file inside a container: fixed-width name + blob loaded from a sibling
/// file named after `file_uuid`.
#[derive(Debug, Clone, PartialEq)]
pub struct ContainerBlob {
    pub name: String,
    pub file_uuid: uuid::Uuid,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContainerFileList {
    pub seq: u32,
    pub files: Vec<ContainerBlob>,
}

impl ContainerFileList {
    /// `list_path` must be `<container dir>/container.<seq>`; blob payloads are read
    /// from sibling files named by the file GUID. Missing blobs are skipped rather
    /// than erroring — a half-written container should still yield its other files.
    pub fn read_from_file(list_path: &Path) -> Result<Self, CoreError> {
        let file_name = list_path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let seq: u32 = file_name
            .strip_prefix("container.")
            .and_then(|suffix| suffix.parse().ok())
            .ok_or_else(|| {
                CoreError::Parse(format!(
                    "Invalid container file name: {}",
                    list_path.display()
                ))
            })?;
        let parent_dir = list_path.parent().unwrap_or_else(|| Path::new("."));

        let bytes = std::fs::read(list_path)?;
        let mut reader = std::io::Cursor::new(bytes);
        let version = read_u32(&mut reader)?;
        if version != CONTAINER_FILE_LIST_VERSION {
            return Err(CoreError::Parse(format!(
                "Unsupported container file version: {version}"
            )));
        }
        let file_count = read_u32(&mut reader)?;
        let mut files = Vec::with_capacity(file_count as usize);
        for _ in 0..file_count {
            let name = read_utf16_fixed_64(&mut reader)?;
            let mut cloud_uuid = [0u8; 16];
            reader.read_exact(&mut cloud_uuid)?; // cloud UUID: unused, always zeros on disk
            let mut uuid_bytes = [0u8; 16];
            reader.read_exact(&mut uuid_bytes)?;
            let file_uuid = uuid::Uuid::from_bytes(uuid_bytes);

            let blob_path = parent_dir.join(guid_file_name(&file_uuid));
            if !blob_path.exists() {
                continue;
            }
            let data = std::fs::read(&blob_path)?;
            files.push(ContainerBlob {
                name,
                file_uuid,
                data,
            });
        }
        Ok(ContainerFileList { seq, files })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use uuid::Uuid;

    fn sample_entry(name: &str, seq: u8, mtime: u64) -> ContainerEntry {
        ContainerEntry {
            container_name: name.to_string(),
            cloud_id: String::new(),
            seq,
            flag: 5,
            container_uuid: Uuid::from_u128(0x0102030405060708090a0b0c0d0e0f10),
            mtime: Filetime(mtime),
            size: 42,
        }
    }

    #[test]
    // The literal below groups digits as `<seconds>_<1e7-scaled fraction>` to make
    // the expected value legible against `from_unix_seconds`'s multiply-by-1e7
    // math, not in the thousands grouping clippy expects.
    #[allow(clippy::inconsistent_digit_grouping)]
    fn filetime_round_trips_unix_seconds() {
        let filetime = Filetime::from_unix_seconds(1720000000.5);
        assert_eq!(filetime.0, 1720000000_5000000 + 116444736000000000);
        assert!((filetime.to_unix_seconds() - 1720000000.5).abs() < 1e-6);
    }

    #[test]
    fn guid_file_name_uses_little_endian_hex_uppercase() {
        let id = Uuid::from_u128(0x0102030405060708090a0b0c0d0e0f10);
        assert_eq!(guid_file_name(&id), "0403020106050807090A0B0C0D0E0F10");
    }

    #[test]
    fn container_index_binary_round_trip() {
        let index = ContainerIndex {
            flag1: 0,
            package_name: "PocketpairInc.Palworld_ad4psfrxyesvt".to_string(),
            mtime: Filetime::from_unix_seconds(1720000000.0),
            flag2: 0,
            index_uuid: String::new(),
            unknown: 0,
            containers: vec![
                sample_entry("AAAA-Level", 1, 100),
                sample_entry("AAAA-LevelMeta", 1, 100),
            ],
        };
        let mut buffer = Vec::new();
        index.write(&mut buffer).unwrap();
        let round_tripped = ContainerIndex::read(&mut Cursor::new(&buffer)).unwrap();
        assert_eq!(index, round_tripped);
        // Header spot checks: version 0xE, count 2.
        assert_eq!(&buffer[0..4], &0xEu32.to_le_bytes());
        assert_eq!(&buffer[4..8], &2u32.to_le_bytes());
    }

    #[test]
    fn container_index_rejects_wrong_version() {
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&13u32.to_le_bytes());
        buffer.extend_from_slice(&0u32.to_le_bytes());
        let error = ContainerIndex::read(&mut Cursor::new(&buffer)).unwrap_err();
        assert_eq!(
            error.to_string(),
            "parse error: Unsupported container index version: 13"
        );
    }

    #[test]
    fn latest_save_containers_picks_highest_seq_then_newest_mtime() {
        let mut index = ContainerIndex {
            flag1: 0,
            package_name: String::new(),
            mtime: Filetime(0),
            flag2: 0,
            index_uuid: String::new(),
            unknown: 0,
            containers: vec![
                sample_entry("AAAA-Level", 1, 100),
                sample_entry("AAAA-Level", 2, 50), // higher seq wins despite older mtime
                sample_entry("AAAA-LevelMeta", 1, 100),
                sample_entry("AAAA-LevelMeta", 1, 200), // same seq: newer mtime wins
                sample_entry("AAAA-LocalData", 1, 1),
                sample_entry("AAAA-WorldOption", 1, 1),
                sample_entry("AAAA-Players-0123456789ABCDEF0123456789ABCDEF", 1, 1),
                sample_entry("AAAA-Players-0123456789ABCDEF0123456789ABCDEF_dps", 1, 1),
                sample_entry("AAAA-Unrelated", 1, 1), // unknown type: skipped
                sample_entry("BBBB-Level", 1, 1),     // different save: skipped
            ],
        };
        index.containers.push(sample_entry("EggTest", 9, 9)); // no dash prefix match
        let latest = index.latest_save_containers("AAAA");
        assert_eq!(latest.len(), 6);
        assert_eq!(latest.get("Level").unwrap().seq, 2);
        assert_eq!(latest.get("LevelMeta").unwrap().mtime, Filetime(200));
        assert!(latest.get("LocalData").is_some());
        assert!(latest.get("WorldOption").is_some());
        assert!(latest
            .get("Players-0123456789ABCDEF0123456789ABCDEF")
            .is_some());
        assert!(latest
            .get("Players-0123456789ABCDEF0123456789ABCDEF_dps")
            .is_some());
    }

}
