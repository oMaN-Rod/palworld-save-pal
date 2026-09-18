//! Listing of the asset paths inside a Palworld `.pak`, read from its footer
//! and index only: the full directory index for versions 10-11, and the
//! legacy per-file index for versions 3-9. Nothing here decompresses file
//! data or touches the filesystem: callers hand in any `Read + Seek`, so this
//! module compiles for wasm and is exercised entirely from in-memory
//! fixtures.

use std::io::{Read, Seek, SeekFrom};

const MAGIC: u32 = 0x5A6F_12E1;
const MAX_INDEX_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PATH_BYTES: usize = 1024;
const MAX_FILE_COUNT: usize = 200_000;
const MAX_TOTAL_PATH_BYTES: u64 = 2 * MAX_INDEX_BYTES;
const DELETED_FLAG: u8 = 0b10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Layout {
    V3,
    V4,
    V5,
    V6,
    V7,
    V8A,
    V8B,
    V9,
    V10,
    V11,
}

impl Layout {
    /// Newest first, so a v8B footer is tried before the shorter v8A one.
    const PROBE_ORDER: [Layout; 10] = [
        Layout::V11,
        Layout::V10,
        Layout::V9,
        Layout::V8B,
        Layout::V8A,
        Layout::V7,
        Layout::V6,
        Layout::V5,
        Layout::V4,
        Layout::V3,
    ];

    fn major(self) -> u32 {
        match self {
            Layout::V3 => 3,
            Layout::V4 => 4,
            Layout::V5 => 5,
            Layout::V6 => 6,
            Layout::V7 => 7,
            Layout::V8A | Layout::V8B => 8,
            Layout::V9 => 9,
            Layout::V10 => 10,
            Layout::V11 => 11,
        }
    }

    fn footer_len(self) -> u64 {
        let mut len = 4 + 4 + 8 + 8 + 20;
        if self.major() >= 7 {
            len += 16;
        }
        if self.major() >= 4 {
            len += 1;
        }
        match self {
            Layout::V8A => len += 4 * 32,
            Layout::V9 => len += 1 + 5 * 32,
            Layout::V8B | Layout::V10 | Layout::V11 => len += 5 * 32,
            _ => {}
        }
        len
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PakListing {
    pub version: u32,
    pub mount_point: String,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PakIndexError {
    #[error("not a pak file")]
    NotAPak,
    #[error("unsupported pak version {0}")]
    UnsupportedVersion(u32),
    #[error("pak index is encrypted")]
    Encrypted,
    #[error("pak has no full directory index")]
    NoDirectoryIndex,
    #[error("malformed pak index: {0}")]
    Malformed(&'static str),
    #[error("{0}")]
    Io(String),
}

fn io_err(e: std::io::Error) -> PakIndexError {
    PakIndexError::Io(e.to_string())
}

/// A cursor over an already-bounded byte slice, so every read is checked
/// against what is actually there before anything is decoded.
struct Buf<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Buf<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], PakIndexError> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or(PakIndexError::Malformed("length overflow"))?;
        if end > self.data.len() {
            return Err(PakIndexError::Malformed("read past end of index"));
        }
        let slice = &self.data[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8, PakIndexError> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32, PakIndexError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("4 bytes"),
        ))
    }

    fn i32(&mut self) -> Result<i32, PakIndexError> {
        Ok(i32::from_le_bytes(
            self.take(4)?.try_into().expect("4 bytes"),
        ))
    }

    fn u64(&mut self) -> Result<u64, PakIndexError> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().expect("8 bytes"),
        ))
    }

    fn u128(&mut self) -> Result<u128, PakIndexError> {
        Ok(u128::from_le_bytes(
            self.take(16)?.try_into().expect("16 bytes"),
        ))
    }

    fn fstring(&mut self) -> Result<String, PakIndexError> {
        let len = self.i32()?;
        if len == 0 {
            return Ok(String::new());
        }
        if len == i32::MIN {
            return Err(PakIndexError::Malformed("fstring length overflow"));
        }
        if len > 0 {
            let n = len as usize;
            let bytes = self.take(n)?;
            Ok(String::from_utf8_lossy(&bytes[..n - 1]).into_owned())
        } else {
            let units = (-len) as usize;
            let byte_len = units
                .checked_mul(2)
                .ok_or(PakIndexError::Malformed("fstring length overflow"))?;
            let bytes = self.take(byte_len)?;
            let mut units: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            units.pop();
            Ok(String::from_utf16_lossy(&units))
        }
    }
}

fn read_bounded<R: Read + Seek>(
    reader: &mut R,
    offset: u64,
    size: u64,
    total_len: u64,
) -> Result<Vec<u8>, PakIndexError> {
    if size > MAX_INDEX_BYTES {
        return Err(PakIndexError::Malformed("index exceeds the size cap"));
    }
    let end = offset
        .checked_add(size)
        .ok_or(PakIndexError::Malformed("index offset overflows"))?;
    if end > total_len {
        return Err(PakIndexError::Malformed("index lies outside the file"));
    }
    reader.seek(SeekFrom::Start(offset)).map_err(io_err)?;
    let mut buf = vec![0u8; size as usize];
    reader.read_exact(&mut buf).map_err(io_err)?;
    Ok(buf)
}

struct Footer {
    layout: Layout,
    encrypted: bool,
    frozen: bool,
    index_offset: u64,
    index_size: u64,
}

fn read_tail<R: Read + Seek>(
    reader: &mut R,
    total_len: u64,
    from_end: u64,
    len: usize,
) -> Result<Vec<u8>, PakIndexError> {
    reader
        .seek(SeekFrom::Start(total_len - from_end))
        .map_err(io_err)?;
    let mut bytes = vec![0u8; len];
    reader.read_exact(&mut bytes).map_err(io_err)?;
    Ok(bytes)
}

fn parse_footer(layout: Layout, bytes: &[u8]) -> Result<Option<Footer>, PakIndexError> {
    let mut f = Buf::new(bytes);
    if layout.major() >= 7 {
        f.u128()?;
    }
    let encrypted = layout.major() >= 4 && f.u8()? != 0;
    if f.u32()? != MAGIC || f.u32()? != layout.major() {
        return Ok(None);
    }
    let index_offset = f.u64()?;
    let index_size = f.u64()?;
    f.take(20)?;
    let frozen = layout == Layout::V9 && f.u8()? != 0;
    Ok(Some(Footer {
        layout,
        encrypted,
        frozen,
        index_offset,
        index_size,
    }))
}

/// Where a footer of each distinct length keeps its magic, counted back from the end.
const MAGIC_FROM_END: [u64; 4] = [204, 205, 172, 44];

fn read_footer<R: Read + Seek>(reader: &mut R, total_len: u64) -> Result<Footer, PakIndexError> {
    for layout in Layout::PROBE_ORDER {
        let len = layout.footer_len();
        if total_len < len {
            continue;
        }
        let bytes = read_tail(reader, total_len, len, len as usize)?;
        if let Some(footer) = parse_footer(layout, &bytes)? {
            return Ok(footer);
        }
    }
    for from_end in MAGIC_FROM_END {
        if total_len < from_end {
            continue;
        }
        let bytes = read_tail(reader, total_len, from_end, 8)?;
        if u32::from_le_bytes(bytes[..4].try_into().expect("4 bytes")) == MAGIC {
            let version = u32::from_le_bytes(bytes[4..].try_into().expect("4 bytes"));
            return Err(PakIndexError::UnsupportedVersion(version));
        }
    }
    Err(PakIndexError::NotAPak)
}

fn push_path(
    files: &mut Vec<String>,
    total_path_bytes: &mut u64,
    path: String,
) -> Result<(), PakIndexError> {
    if path.len() > MAX_PATH_BYTES {
        return Err(PakIndexError::Malformed("path exceeds the path cap"));
    }
    if files.len() >= MAX_FILE_COUNT {
        return Err(PakIndexError::Malformed("file count exceeds the cap"));
    }
    *total_path_bytes = total_path_bytes
        .checked_add(path.len() as u64)
        .ok_or(PakIndexError::Malformed("path bytes overflow"))?;
    if *total_path_bytes > MAX_TOTAL_PATH_BYTES {
        return Err(PakIndexError::Malformed("total path bytes exceed the cap"));
    }
    files.push(path);
    Ok(())
}

fn read_directory_listing<R: Read + Seek>(
    reader: &mut R,
    primary: &[u8],
    total_len: u64,
) -> Result<(String, Vec<String>), PakIndexError> {
    let mut p = Buf::new(primary);
    let mount_point = p.fstring()?;
    let _entry_count = p.u32()?;
    let _path_hash_seed = p.u64()?;
    if p.u32()? != 0 {
        let _offset = p.u64()?;
        let _size = p.u64()?;
        let _hash = p.take(20)?;
    }
    if p.u32()? == 0 {
        return Err(PakIndexError::NoDirectoryIndex);
    }
    let dir_offset = p.u64()?;
    let dir_size = p.u64()?;
    let _dir_hash = p.take(20)?;

    let directory = read_bounded(reader, dir_offset, dir_size, total_len)?;
    let mut d = Buf::new(&directory);
    let dir_count = d.u32()?;
    let mut files = Vec::new();
    let mut total_path_bytes: u64 = 0;
    for _ in 0..dir_count {
        let dir_name = d.fstring()?;
        let stripped = dir_name.trim_start_matches('/');
        if stripped.len() > MAX_PATH_BYTES {
            return Err(PakIndexError::Malformed(
                "directory name exceeds the path cap",
            ));
        }
        let file_count = d.u32()?;
        for _ in 0..file_count {
            let file_name = d.fstring()?;
            let entry_offset = d.i32()?;
            if entry_offset == i32::MIN {
                continue;
            }
            push_path(
                &mut files,
                &mut total_path_bytes,
                format!("{stripped}{file_name}"),
            )?;
        }
    }

    Ok((mount_point, files))
}

fn read_legacy_listing(
    primary: &[u8],
    layout: Layout,
) -> Result<(String, Vec<String>), PakIndexError> {
    let mut p = Buf::new(primary);
    let mount_point = p.fstring()?;
    let count = p.u32()? as usize;
    if count > MAX_FILE_COUNT {
        return Err(PakIndexError::Malformed("file count exceeds the cap"));
    }
    let mut files = Vec::new();
    let mut total_path_bytes: u64 = 0;
    for _ in 0..count {
        let path = p.fstring()?;
        let _offset = p.u64()?;
        let _size = p.u64()?;
        let _uncompressed = p.u64()?;
        let compression = if layout == Layout::V8A {
            u32::from(p.u8()?)
        } else {
            p.u32()?
        };
        let _hash = p.take(20)?;
        if compression != 0 {
            let blocks = p.u32()? as usize;
            let block_bytes = blocks
                .checked_mul(16)
                .ok_or(PakIndexError::Malformed("block count overflow"))?;
            p.take(block_bytes)?;
        }
        let flags = p.u8()?;
        let _block_size = p.u32()?;
        if layout.major() >= 6 && flags & DELETED_FLAG != 0 {
            continue;
        }
        push_path(&mut files, &mut total_path_bytes, path)?;
    }
    Ok((mount_point, files))
}

pub fn read_pak_listing<R: Read + Seek>(reader: &mut R) -> Result<PakListing, PakIndexError> {
    let total_len = reader.seek(SeekFrom::End(0)).map_err(io_err)?;
    let footer = read_footer(reader, total_len)?;
    if footer.encrypted {
        return Err(PakIndexError::Encrypted);
    }
    if footer.frozen {
        return Err(PakIndexError::UnsupportedVersion(9));
    }
    let primary = read_bounded(reader, footer.index_offset, footer.index_size, total_len)?;
    let (mount_point, files) = if footer.layout.major() >= 10 {
        read_directory_listing(reader, &primary, total_len)?
    } else {
        read_legacy_listing(&primary, footer.layout)?
    };
    Ok(PakListing {
        version: footer.layout.major(),
        mount_point,
        files,
    })
}

/// Canonical key for one asset, so two mods that ship the same game asset
/// collide on the same string regardless of case or which sidecar extension
/// they carry.
pub fn asset_key(mount_point: &str, file: &str) -> String {
    let mut mount = mount_point;
    while let Some(rest) = mount.strip_prefix("../") {
        mount = rest;
    }
    let mount = mount.trim_end_matches('/');
    let file = file.trim_start_matches('/');
    let joined = if mount.is_empty() {
        file.to_string()
    } else {
        format!("{mount}/{file}")
    };
    let lower = joined.to_lowercase();
    for ext in [".uexp", ".ubulk", ".uptnl"] {
        if let Some(stripped) = lower.strip_suffix(ext) {
            return format!("{stripped}.uasset");
        }
    }
    lower
}

#[cfg(any(test, feature = "test-fixtures"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestPakVersion {
    V3,
    V4,
    V5,
    V6,
    V7,
    V8A,
    V8B,
    V9,
    V10,
    V11,
}

#[cfg(any(test, feature = "test-fixtures"))]
impl TestPakVersion {
    fn layout(self) -> Layout {
        match self {
            TestPakVersion::V3 => Layout::V3,
            TestPakVersion::V4 => Layout::V4,
            TestPakVersion::V5 => Layout::V5,
            TestPakVersion::V6 => Layout::V6,
            TestPakVersion::V7 => Layout::V7,
            TestPakVersion::V8A => Layout::V8A,
            TestPakVersion::V8B => Layout::V8B,
            TestPakVersion::V9 => Layout::V9,
            TestPakVersion::V10 => Layout::V10,
            TestPakVersion::V11 => Layout::V11,
        }
    }
}

/// One legacy index record; `blocks > 0` writes a compressed entry with that
/// many zeroed blocks.
#[cfg(any(test, feature = "test-fixtures"))]
#[derive(Debug, Clone, Copy)]
pub struct TestEntry<'a> {
    pub path: &'a str,
    pub blocks: u32,
    pub flags: u8,
}

#[cfg(any(test, feature = "test-fixtures"))]
fn write_fstring(buf: &mut Vec<u8>, s: &str) {
    if s.is_empty() {
        buf.extend_from_slice(&0i32.to_le_bytes());
        return;
    }
    let bytes = s.as_bytes();
    let len = (bytes.len() + 1) as i32;
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(bytes);
    buf.push(0);
}

#[cfg(any(test, feature = "test-fixtures"))]
fn test_footer(
    layout: Layout,
    magic: u32,
    encrypted: u8,
    frozen: u8,
    index_offset: u64,
    index_size: u64,
) -> Vec<u8> {
    let mut footer = Vec::with_capacity(layout.footer_len() as usize);
    if layout.major() >= 7 {
        footer.extend_from_slice(&0u128.to_le_bytes());
    }
    if layout.major() >= 4 {
        footer.push(encrypted);
    }
    footer.extend_from_slice(&magic.to_le_bytes());
    footer.extend_from_slice(&layout.major().to_le_bytes());
    footer.extend_from_slice(&index_offset.to_le_bytes());
    footer.extend_from_slice(&index_size.to_le_bytes());
    footer.extend_from_slice(&[0u8; 20]);
    match layout {
        Layout::V8A => footer.extend_from_slice(&[0u8; 4 * 32]),
        Layout::V9 => {
            footer.push(frozen);
            footer.extend_from_slice(&[0u8; 5 * 32]);
        }
        Layout::V8B | Layout::V10 | Layout::V11 => footer.extend_from_slice(&[0u8; 5 * 32]),
        _ => {}
    }
    assert_eq!(footer.len() as u64, layout.footer_len());
    footer
}

#[cfg(any(test, feature = "test-fixtures"))]
fn directory_pak(layout: Layout, mount_point: &str, files: &[&str]) -> Vec<u8> {
    let mut dirs: Vec<(String, Vec<&str>)> = Vec::new();
    for &path in files {
        let (dir, name) = match path.rfind('/') {
            Some(i) => (format!("/{}", &path[..=i]), &path[i + 1..]),
            None => ("/".to_string(), path),
        };
        match dirs.iter_mut().find(|(d, _)| *d == dir) {
            Some((_, names)) => names.push(name),
            None => dirs.push((dir, vec![name])),
        }
    }

    let mut directory = Vec::new();
    directory.extend_from_slice(&(dirs.len() as u32).to_le_bytes());
    for (dir, names) in &dirs {
        write_fstring(&mut directory, dir);
        directory.extend_from_slice(&(names.len() as u32).to_le_bytes());
        for name in names {
            write_fstring(&mut directory, name);
            directory.extend_from_slice(&0i32.to_le_bytes());
        }
    }

    let mut primary = Vec::new();
    write_fstring(&mut primary, mount_point);
    primary.extend_from_slice(&(files.len() as u32).to_le_bytes());
    primary.extend_from_slice(&0u64.to_le_bytes());
    primary.extend_from_slice(&0u32.to_le_bytes());
    primary.extend_from_slice(&1u32.to_le_bytes());
    let dir_offset = primary.len() as u64 + 8 + 8 + 20 + 4 + 4;
    primary.extend_from_slice(&dir_offset.to_le_bytes());
    primary.extend_from_slice(&(directory.len() as u64).to_le_bytes());
    primary.extend_from_slice(&[0u8; 20]);
    primary.extend_from_slice(&0u32.to_le_bytes());
    primary.extend_from_slice(&0u32.to_le_bytes());

    let mut out = primary.clone();
    out.extend_from_slice(&directory);
    out.extend_from_slice(&test_footer(layout, MAGIC, 0, 0, 0, primary.len() as u64));
    out
}

/// A pak with no data section in any supported version.
#[cfg(any(test, feature = "test-fixtures"))]
pub fn test_pak_version(version: TestPakVersion, mount_point: &str, files: &[&str]) -> Vec<u8> {
    match version {
        TestPakVersion::V10 | TestPakVersion::V11 => {
            directory_pak(version.layout(), mount_point, files)
        }
        _ => {
            let entries: Vec<TestEntry<'_>> = files
                .iter()
                .map(|&path| TestEntry {
                    path,
                    blocks: 0,
                    flags: 0,
                })
                .collect();
            test_legacy_pak(version, mount_point, &entries)
        }
    }
}

#[cfg(any(test, feature = "test-fixtures"))]
pub fn test_pak(mount_point: &str, files: &[&str]) -> Vec<u8> {
    test_pak_version(TestPakVersion::V11, mount_point, files)
}

/// A version 3-9 pak whose legacy index sits at offset 0, directly followed by the footer.
#[cfg(any(test, feature = "test-fixtures"))]
pub fn test_legacy_pak(
    version: TestPakVersion,
    mount_point: &str,
    entries: &[TestEntry<'_>],
) -> Vec<u8> {
    let layout = version.layout();
    assert!(
        layout.major() < 10,
        "test_legacy_pak builds versions 3 through 9"
    );
    let index = legacy_index(layout, mount_point, entries);
    let mut out = index.clone();
    out.extend_from_slice(&test_footer(layout, MAGIC, 0, 0, 0, index.len() as u64));
    out
}

#[cfg(any(test, feature = "test-fixtures"))]
fn legacy_index(layout: Layout, mount_point: &str, entries: &[TestEntry<'_>]) -> Vec<u8> {
    let mut index = Vec::new();
    write_fstring(&mut index, mount_point);
    index.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for entry in entries {
        write_fstring(&mut index, entry.path);
        index.extend_from_slice(&0u64.to_le_bytes());
        index.extend_from_slice(&0u64.to_le_bytes());
        index.extend_from_slice(&0u64.to_le_bytes());
        let compression: u32 = u32::from(entry.blocks > 0);
        if layout == Layout::V8A {
            index.push(compression as u8);
        } else {
            index.extend_from_slice(&compression.to_le_bytes());
        }
        index.extend_from_slice(&[0u8; 20]);
        if compression != 0 {
            index.extend_from_slice(&entry.blocks.to_le_bytes());
            for _ in 0..entry.blocks {
                index.extend_from_slice(&[0u8; 16]);
            }
        }
        index.push(entry.flags);
        index.extend_from_slice(&0u32.to_le_bytes());
    }
    index
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn footer_bytes(
        magic: u32,
        version: u32,
        encrypted: u8,
        index_offset: u64,
        index_size: u64,
    ) -> Vec<u8> {
        let mut footer = Vec::with_capacity(221);
        footer.extend_from_slice(&0u128.to_le_bytes());
        footer.push(encrypted);
        footer.extend_from_slice(&magic.to_le_bytes());
        footer.extend_from_slice(&version.to_le_bytes());
        footer.extend_from_slice(&index_offset.to_le_bytes());
        footer.extend_from_slice(&index_size.to_le_bytes());
        footer.extend_from_slice(&[0u8; 20]);
        footer.extend_from_slice(&[0u8; 160]);
        footer
    }

    fn pak_with_directory(directory: &[u8]) -> Vec<u8> {
        let mut primary = Vec::new();
        write_fstring(&mut primary, "../../../");
        primary.extend_from_slice(&0u32.to_le_bytes());
        primary.extend_from_slice(&0u64.to_le_bytes());
        primary.extend_from_slice(&0u32.to_le_bytes());
        primary.extend_from_slice(&1u32.to_le_bytes());
        let dir_offset = primary.len() as u64 + 8 + 8 + 20;
        primary.extend_from_slice(&dir_offset.to_le_bytes());
        primary.extend_from_slice(&(directory.len() as u64).to_le_bytes());
        primary.extend_from_slice(&[0u8; 20]);

        let mut bytes = primary.clone();
        bytes.extend_from_slice(directory);
        bytes.extend_from_slice(&footer_bytes(MAGIC, 11, 0, 0, primary.len() as u64));
        bytes
    }

    fn write_fstring_utf16(buf: &mut Vec<u8>, s: &str) {
        let units: Vec<u16> = s.encode_utf16().chain(std::iter::once(0)).collect();
        let len = -(units.len() as i32);
        buf.extend_from_slice(&len.to_le_bytes());
        for u in units {
            buf.extend_from_slice(&u.to_le_bytes());
        }
    }

    #[test]
    fn round_trips_a_simple_pak() {
        let bytes = test_pak(
            "../../../",
            &[
                "Pal/Content/Pal/Blueprint/A.uasset",
                "Pal/Content/Pal/Blueprint/A.uexp",
            ],
        );
        let mut cursor = Cursor::new(bytes);
        let listing = read_pak_listing(&mut cursor).unwrap();
        assert_eq!(listing.version, 11);
        assert_eq!(listing.mount_point, "../../../");
        assert_eq!(
            listing.files,
            vec![
                "Pal/Content/Pal/Blueprint/A.uasset".to_string(),
                "Pal/Content/Pal/Blueprint/A.uexp".to_string(),
            ]
        );
    }

    #[test]
    fn rejects_a_buffer_shorter_than_a_footer() {
        let mut cursor = Cursor::new(vec![0u8; 100]);
        assert_eq!(read_pak_listing(&mut cursor), Err(PakIndexError::NotAPak));
    }

    #[test]
    fn rejects_a_bad_magic() {
        let mut bytes = vec![0u8; 50];
        bytes.extend_from_slice(&footer_bytes(0, 11, 0, 0, 0));
        let mut cursor = Cursor::new(bytes);
        assert_eq!(read_pak_listing(&mut cursor), Err(PakIndexError::NotAPak));
    }

    #[test]
    fn rejects_an_unsupported_version() {
        let mut bytes = vec![0u8; 50];
        bytes.extend_from_slice(&footer_bytes(MAGIC, 12, 0, 0, 0));
        let mut cursor = Cursor::new(bytes);
        assert_eq!(
            read_pak_listing(&mut cursor),
            Err(PakIndexError::UnsupportedVersion(12))
        );
    }

    const ALL_VERSIONS: [(TestPakVersion, u32); 10] = [
        (TestPakVersion::V3, 3),
        (TestPakVersion::V4, 4),
        (TestPakVersion::V5, 5),
        (TestPakVersion::V6, 6),
        (TestPakVersion::V7, 7),
        (TestPakVersion::V8A, 8),
        (TestPakVersion::V8B, 8),
        (TestPakVersion::V9, 9),
        (TestPakVersion::V10, 10),
        (TestPakVersion::V11, 11),
    ];

    #[test]
    fn every_supported_version_round_trips() {
        for (version, major) in ALL_VERSIONS {
            let bytes = test_pak_version(
                version,
                "../../../Pal/Content/Mods/X/",
                &["Blueprint/A.uasset", "Blueprint/A.uexp"],
            );
            let listing = read_pak_listing(&mut Cursor::new(bytes))
                .unwrap_or_else(|error| panic!("{version:?}: {error}"));
            assert_eq!(listing.version, major, "{version:?}");
            assert_eq!(
                listing.mount_point, "../../../Pal/Content/Mods/X/",
                "{version:?}"
            );
            assert_eq!(
                listing.files,
                vec![
                    "Blueprint/A.uasset".to_string(),
                    "Blueprint/A.uexp".to_string()
                ],
                "{version:?}"
            );
        }
    }

    /// Bytes written by hand in the layout a real v3 Workshop pak uses, so the
    /// reader is checked against the format rather than against its own fixture.
    #[test]
    fn reads_a_hand_written_v3_pak_with_a_compressed_entry() {
        let mut index = Vec::new();
        index.extend_from_slice(&10i32.to_le_bytes());
        index.extend_from_slice(b"../../../\0");
        index.extend_from_slice(&2u32.to_le_bytes());
        for (path, method, blocks) in [("Pal/A.uasset", 1u32, 2u32), ("Pal/A.uexp", 0, 0)] {
            let len = path.len() as i32 + 1;
            index.extend_from_slice(&len.to_le_bytes());
            index.extend_from_slice(path.as_bytes());
            index.push(0);
            index.extend_from_slice(&[0u8; 24]);
            index.extend_from_slice(&method.to_le_bytes());
            index.extend_from_slice(&[0u8; 20]);
            if method != 0 {
                index.extend_from_slice(&blocks.to_le_bytes());
                index.extend_from_slice(&vec![0u8; 16 * blocks as usize]);
            }
            index.push(0);
            index.extend_from_slice(&0x1_0000u32.to_le_bytes());
        }
        let mut bytes = index.clone();
        bytes.extend_from_slice(&MAGIC.to_le_bytes());
        bytes.extend_from_slice(&3u32.to_le_bytes());
        bytes.extend_from_slice(&0u64.to_le_bytes());
        bytes.extend_from_slice(&(index.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&[0u8; 20]);

        let listing = read_pak_listing(&mut Cursor::new(bytes)).unwrap();
        assert_eq!(listing.version, 3);
        assert_eq!(
            listing.files,
            vec!["Pal/A.uasset".to_string(), "Pal/A.uexp".to_string()]
        );
    }

    #[test]
    fn a_v8a_compression_slot_is_one_byte() {
        let bytes = test_legacy_pak(
            TestPakVersion::V8A,
            "../../../",
            &[
                TestEntry {
                    path: "Pal/A.uasset",
                    blocks: 3,
                    flags: 0,
                },
                TestEntry {
                    path: "Pal/B.uasset",
                    blocks: 0,
                    flags: 0,
                },
            ],
        );
        let listing = read_pak_listing(&mut Cursor::new(bytes)).unwrap();
        assert_eq!(
            listing.files,
            vec!["Pal/A.uasset".to_string(), "Pal/B.uasset".to_string()]
        );
    }

    #[test]
    fn a_delete_record_is_skipped_from_v6_and_kept_before() {
        let entries = [
            TestEntry {
                path: "Pal/Gone.uasset",
                blocks: 0,
                flags: DELETED_FLAG,
            },
            TestEntry {
                path: "Pal/Kept.uasset",
                blocks: 0,
                flags: 1,
            },
        ];
        let v6 = read_pak_listing(&mut Cursor::new(test_legacy_pak(
            TestPakVersion::V6,
            "../../../",
            &entries,
        )))
        .unwrap();
        assert_eq!(v6.files, vec!["Pal/Kept.uasset".to_string()]);
        let v5 = read_pak_listing(&mut Cursor::new(test_legacy_pak(
            TestPakVersion::V5,
            "../../../",
            &entries,
        )))
        .unwrap();
        assert_eq!(v5.files.len(), 2);
    }

    #[test]
    fn a_legacy_encrypted_index_is_encrypted() {
        let index = legacy_index(Layout::V4, "../../../", &[]);
        let mut bytes = index.clone();
        bytes.extend_from_slice(&test_footer(Layout::V4, MAGIC, 1, 0, 0, index.len() as u64));
        assert_eq!(
            read_pak_listing(&mut Cursor::new(bytes)),
            Err(PakIndexError::Encrypted)
        );
    }

    #[test]
    fn a_frozen_v9_index_is_unsupported() {
        let index = legacy_index(Layout::V9, "../../../", &[]);
        let mut bytes = index.clone();
        bytes.extend_from_slice(&test_footer(Layout::V9, MAGIC, 0, 1, 0, index.len() as u64));
        assert_eq!(
            read_pak_listing(&mut Cursor::new(bytes)),
            Err(PakIndexError::UnsupportedVersion(9))
        );
    }

    #[test]
    fn a_v2_footer_is_an_unsupported_version() {
        let mut bytes = vec![0u8; 64];
        bytes.extend_from_slice(&MAGIC.to_le_bytes());
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 36]);
        assert_eq!(
            read_pak_listing(&mut Cursor::new(bytes)),
            Err(PakIndexError::UnsupportedVersion(2))
        );
    }

    #[test]
    fn a_legacy_entry_count_over_the_cap_is_malformed() {
        let mut index = Vec::new();
        write_fstring(&mut index, "../../../");
        index.extend_from_slice(&((MAX_FILE_COUNT + 1) as u32).to_le_bytes());
        let mut bytes = index.clone();
        bytes.extend_from_slice(&test_footer(Layout::V3, MAGIC, 0, 0, 0, index.len() as u64));
        assert_eq!(
            read_pak_listing(&mut Cursor::new(bytes)),
            Err(PakIndexError::Malformed("file count exceeds the cap"))
        );
    }

    #[test]
    fn a_legacy_block_count_past_the_end_is_malformed() {
        let mut index = legacy_index(
            Layout::V7,
            "../../../",
            &[TestEntry {
                path: "Pal/A.uasset",
                blocks: 1,
                flags: 0,
            }],
        );
        let block_count_at = index.len() - (1 + 4) - 16 - 4;
        index[block_count_at..block_count_at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        let mut bytes = index.clone();
        bytes.extend_from_slice(&test_footer(Layout::V7, MAGIC, 0, 0, 0, index.len() as u64));
        assert!(matches!(
            read_pak_listing(&mut Cursor::new(bytes)),
            Err(PakIndexError::Malformed(_))
        ));
    }

    #[test]
    fn a_truncated_legacy_entry_is_malformed() {
        let index = legacy_index(
            Layout::V3,
            "../../../",
            &[TestEntry {
                path: "Pal/A.uasset",
                blocks: 0,
                flags: 0,
            }],
        );
        let short = &index[..index.len() - 3];
        let mut bytes = short.to_vec();
        bytes.extend_from_slice(&test_footer(Layout::V3, MAGIC, 0, 0, 0, short.len() as u64));
        assert!(matches!(
            read_pak_listing(&mut Cursor::new(bytes)),
            Err(PakIndexError::Malformed(_))
        ));
    }

    #[test]
    fn a_legacy_path_over_the_path_cap_is_malformed() {
        let long = "a".repeat(MAX_PATH_BYTES + 1);
        let bytes = test_legacy_pak(
            TestPakVersion::V3,
            "../../../",
            &[TestEntry {
                path: &long,
                blocks: 0,
                flags: 0,
            }],
        );
        assert_eq!(
            read_pak_listing(&mut Cursor::new(bytes)),
            Err(PakIndexError::Malformed("path exceeds the path cap"))
        );
    }

    #[test]
    fn rejects_an_encrypted_index() {
        let mut bytes = vec![0u8; 50];
        bytes.extend_from_slice(&footer_bytes(MAGIC, 11, 1, 0, 0));
        let mut cursor = Cursor::new(bytes);
        assert_eq!(read_pak_listing(&mut cursor), Err(PakIndexError::Encrypted));
    }

    #[test]
    fn rejects_a_pak_with_no_directory_index() {
        let mut primary = Vec::new();
        write_fstring(&mut primary, "../../../");
        primary.extend_from_slice(&0u32.to_le_bytes());
        primary.extend_from_slice(&0u64.to_le_bytes());
        primary.extend_from_slice(&0u32.to_le_bytes());
        primary.extend_from_slice(&0u32.to_le_bytes());

        let mut bytes = primary.clone();
        bytes.extend_from_slice(&footer_bytes(MAGIC, 11, 0, 0, primary.len() as u64));
        let mut cursor = Cursor::new(bytes);
        assert_eq!(
            read_pak_listing(&mut cursor),
            Err(PakIndexError::NoDirectoryIndex)
        );
    }

    #[test]
    fn rejects_a_directory_index_offset_past_the_end() {
        let mut primary = Vec::new();
        write_fstring(&mut primary, "../../../");
        primary.extend_from_slice(&0u32.to_le_bytes());
        primary.extend_from_slice(&0u64.to_le_bytes());
        primary.extend_from_slice(&0u32.to_le_bytes());
        primary.extend_from_slice(&1u32.to_le_bytes());
        primary.extend_from_slice(&999_999u64.to_le_bytes());
        primary.extend_from_slice(&10u64.to_le_bytes());
        primary.extend_from_slice(&[0u8; 20]);

        let mut bytes = primary.clone();
        bytes.extend_from_slice(&footer_bytes(MAGIC, 11, 0, 0, primary.len() as u64));
        let mut cursor = Cursor::new(bytes);
        assert!(matches!(
            read_pak_listing(&mut cursor),
            Err(PakIndexError::Malformed(_))
        ));
    }

    #[test]
    fn rejects_a_file_count_larger_than_the_remaining_bytes() {
        let mut directory = Vec::new();
        directory.extend_from_slice(&1u32.to_le_bytes());
        write_fstring(&mut directory, "/");
        directory.extend_from_slice(&u32::MAX.to_le_bytes());

        let bytes = pak_with_directory(&directory);
        let mut cursor = Cursor::new(bytes);
        assert!(matches!(
            read_pak_listing(&mut cursor),
            Err(PakIndexError::Malformed(_))
        ));
    }

    #[test]
    fn rejects_an_fstring_length_past_the_end() {
        let mut directory = Vec::new();
        directory.extend_from_slice(&1u32.to_le_bytes());
        directory.extend_from_slice(&100i32.to_le_bytes());

        let bytes = pak_with_directory(&directory);
        let mut cursor = Cursor::new(bytes);
        assert!(matches!(
            read_pak_listing(&mut cursor),
            Err(PakIndexError::Malformed(_))
        ));
    }

    #[test]
    fn decodes_a_utf16_file_name() {
        let mut directory = Vec::new();
        directory.extend_from_slice(&1u32.to_le_bytes());
        write_fstring(&mut directory, "/");
        directory.extend_from_slice(&1u32.to_le_bytes());
        write_fstring_utf16(&mut directory, "Ünïcode.uasset");
        directory.extend_from_slice(&0i32.to_le_bytes());

        let bytes = pak_with_directory(&directory);
        let mut cursor = Cursor::new(bytes);
        let listing = read_pak_listing(&mut cursor).unwrap();
        assert_eq!(listing.files, vec!["Ünïcode.uasset".to_string()]);
    }

    #[test]
    fn rejects_an_index_over_the_size_cap() {
        let mut bytes = vec![0u8; 50];
        bytes.extend_from_slice(&footer_bytes(MAGIC, 11, 0, 0, MAX_INDEX_BYTES + 1));
        let mut cursor = Cursor::new(bytes);
        assert_eq!(
            read_pak_listing(&mut cursor),
            Err(PakIndexError::Malformed("index exceeds the size cap"))
        );
    }

    #[test]
    fn rejects_a_directory_name_over_the_path_cap() {
        let long_name = "a".repeat(MAX_PATH_BYTES + 1);
        let mut directory = Vec::new();
        directory.extend_from_slice(&1u32.to_le_bytes());
        write_fstring(&mut directory, &long_name);
        directory.extend_from_slice(&0u32.to_le_bytes());

        let bytes = pak_with_directory(&directory);
        let mut cursor = Cursor::new(bytes);
        assert_eq!(
            read_pak_listing(&mut cursor),
            Err(PakIndexError::Malformed(
                "directory name exceeds the path cap"
            ))
        );
    }

    #[test]
    fn rejects_a_file_count_over_the_count_cap() {
        let mut directory = Vec::new();
        directory.extend_from_slice(&1u32.to_le_bytes());
        write_fstring(&mut directory, "/");
        let file_count = (MAX_FILE_COUNT + 1) as u32;
        directory.extend_from_slice(&file_count.to_le_bytes());
        for _ in 0..=MAX_FILE_COUNT {
            directory.extend_from_slice(&0i32.to_le_bytes());
            directory.extend_from_slice(&0i32.to_le_bytes());
        }

        let bytes = pak_with_directory(&directory);
        let mut cursor = Cursor::new(bytes);
        assert_eq!(
            read_pak_listing(&mut cursor),
            Err(PakIndexError::Malformed("file count exceeds the cap"))
        );
    }

    #[test]
    fn skips_a_pruned_entry() {
        let mut directory = Vec::new();
        directory.extend_from_slice(&1u32.to_le_bytes());
        write_fstring(&mut directory, "/");
        directory.extend_from_slice(&2u32.to_le_bytes());
        write_fstring(&mut directory, "kept.uasset");
        directory.extend_from_slice(&0i32.to_le_bytes());
        write_fstring(&mut directory, "pruned.uasset");
        directory.extend_from_slice(&i32::MIN.to_le_bytes());

        let bytes = pak_with_directory(&directory);
        let mut cursor = Cursor::new(bytes);
        let listing = read_pak_listing(&mut cursor).unwrap();
        assert_eq!(listing.files, vec!["kept.uasset".to_string()]);
    }

    #[test]
    fn asset_key_normalizes_common_extensions() {
        assert_eq!(
            asset_key("../../../", "Pal/Content/X.uexp"),
            "pal/content/x.uasset"
        );
        assert_eq!(
            asset_key("../../../", "Pal/Content/X.ubulk"),
            "pal/content/x.uasset"
        );
        assert_eq!(
            asset_key("../../../", "Pal/Content/X.uptnl"),
            "pal/content/x.uasset"
        );
        assert_eq!(
            asset_key("../../../", "Pal/Content/X.umap"),
            "pal/content/x.umap"
        );
    }
}
