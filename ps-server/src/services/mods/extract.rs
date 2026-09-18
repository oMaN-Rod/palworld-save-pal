use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path};

use ps_core::mods::ArchiveEntry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    Zip,
    SevenZ,
    Rar,
    TarGz,
    Tar,
    Unknown,
}

#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    #[error("unsupported archive format")]
    UnsupportedFormat,
    #[error("{0} is needed to read this archive and was not found on PATH")]
    MissingTool(&'static str),
    #[error("archive is empty")]
    Empty,
    #[error("archive wrote outside its destination: {0:?}")]
    Escaped(Vec<String>),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Archive(String),
}

pub struct Extracted {
    dir: tempfile::TempDir,
    payload: std::path::PathBuf,
    pub entries: Vec<ArchiveEntry>,
}

impl Extracted {
    /// The directory the archive's contents were written into. Deliberately a
    /// child of the temp directory, so that anything an archive writes outside it
    /// lands somewhere this code can see.
    pub fn path(&self) -> &Path {
        &self.payload
    }

    /// The temp directory holding the payload. Kept so the caller can confirm the
    /// payload is all that was written.
    pub fn enclosing_dir(&self) -> &Path {
        self.dir.path()
    }
}

pub fn sniff_format(path: &Path) -> Result<ArchiveFormat, ExtractError> {
    let mut file = File::open(path)?;
    let mut head = [0u8; 8];
    let read = read_at_most(&mut file, &mut head)?;
    let head = &head[..read];
    if head.starts_with(b"PK\x03\x04")
        || head.starts_with(b"PK\x05\x06")
        || head.starts_with(b"PK\x07\x08")
    {
        return Ok(ArchiveFormat::Zip);
    }
    if head.starts_with(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]) {
        return Ok(ArchiveFormat::SevenZ);
    }
    if head.starts_with(b"Rar!\x1A\x07") {
        return Ok(ArchiveFormat::Rar);
    }
    if head.starts_with(&[0x1F, 0x8B]) {
        return Ok(ArchiveFormat::TarGz);
    }
    let mut ustar = [0u8; 5];
    if file.seek(SeekFrom::Start(257)).is_ok()
        && read_at_most(&mut file, &mut ustar)? == 5
        && &ustar == b"ustar"
    {
        return Ok(ArchiveFormat::Tar);
    }
    Ok(ArchiveFormat::Unknown)
}

fn read_at_most(file: &mut File, buf: &mut [u8]) -> Result<usize, ExtractError> {
    let mut filled = 0usize;
    while filled < buf.len() {
        match file.read(&mut buf[filled..])? {
            0 => break,
            n => filled += n,
        }
    }
    Ok(filled)
}

pub fn list_entries(path: &Path) -> Result<Vec<ArchiveEntry>, ExtractError> {
    match sniff_format(path)? {
        ArchiveFormat::Zip => list_zip(path),
        ArchiveFormat::SevenZ | ArchiveFormat::Rar | ArchiveFormat::TarGz | ArchiveFormat::Tar => {
            // These formats are streamed, not indexed, so the only honest listing
            // is the one extraction produces, and the temp tree is then dropped.
            // A caller that will extract anyway must call `extract` once and read
            // `Extracted::entries` instead of calling this first.
            Ok(extract(path)?.entries)
        }
        ArchiveFormat::Unknown => Err(ExtractError::UnsupportedFormat),
    }
}

fn list_zip(path: &Path) -> Result<Vec<ArchiveEntry>, ExtractError> {
    let file = File::open(path)?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| ExtractError::Archive(e.to_string()))?;
    let mut entries = Vec::new();
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|e| ExtractError::Archive(e.to_string()))?;
        if entry.is_dir() {
            continue;
        }
        // `enclosed_name` both rejects an escaping entry and normalises one that
        // merely looks like it escapes: `CoolMod/../evil.lua` resolves to
        // `evil.lua`, which is where extraction writes it. Reporting the raw name
        // instead would make this listing disagree with `extract` about what the
        // archive holds, so a preview would show a path the install never produces.
        let Some(contained) = entry.enclosed_name() else {
            continue;
        };
        let resolved = lexical_rel(&contained);
        if resolved.as_os_str().is_empty() {
            continue;
        }
        entries.push(ArchiveEntry {
            path: rel_to_slash(&resolved),
            size: entry.size(),
        });
    }
    Ok(entries)
}

/// Unpacks into `{temp}/payload`, never into `{temp}` itself. The extra level is
/// what makes an escape observable: a crate or external tool that writes `../x`
/// lands in `{temp}`, which `escaped_entries` then finds. Unpacking at the top
/// level would put such a file outside the temp directory entirely, where nothing
/// here could see it and the `TempDir` drop would not remove it.
pub fn extract(path: &Path) -> Result<Extracted, ExtractError> {
    let dir = tempfile::tempdir()?;
    let payload = dir.path().join("payload");
    std::fs::create_dir_all(&payload)?;
    match sniff_format(path)? {
        ArchiveFormat::Zip => extract_zip(path, &payload)?,
        ArchiveFormat::SevenZ => {
            // A containment refusal inside sevenz-rust2 is indistinguishable from a
            // codec error, so the external tool may be handed an archive that was
            // refused for being unsafe. `escaped_entries` below is what covers that.
            if sevenz_rust2::decompress_file(path, &payload).is_err() {
                external_extract(path, &payload, "7z")?;
            }
        }
        ArchiveFormat::Rar => external_extract(path, &payload, "unrar")?,
        ArchiveFormat::TarGz => {
            let file = File::open(path)?;
            let decoder = flate2::read::GzDecoder::new(file);
            tar::Archive::new(decoder)
                .unpack(&payload)
                .map_err(|e| ExtractError::Archive(e.to_string()))?;
        }
        ArchiveFormat::Tar => {
            let file = File::open(path)?;
            tar::Archive::new(file)
                .unpack(&payload)
                .map_err(|e| ExtractError::Archive(e.to_string()))?;
        }
        ArchiveFormat::Unknown => return Err(ExtractError::UnsupportedFormat),
    }
    let escaped = escaped_entries(dir.path(), &payload)?;
    if !escaped.is_empty() {
        return Err(ExtractError::Escaped(escaped));
    }
    let entries = walk_extracted(&payload)?;
    if entries.is_empty() {
        return Err(ExtractError::Empty);
    }
    Ok(Extracted {
        dir,
        payload,
        entries,
    })
}

/// Anything written under the temp directory but outside the payload. A
/// non-empty result means the extractor for that format wrote outside its
/// destination, which is a refusal rather than something to clean up and continue
/// from.
fn escaped_entries(root: &Path, payload: &Path) -> Result<Vec<String>, ExtractError> {
    let mut escaped = Vec::new();
    for found in walkdir::WalkDir::new(root).follow_links(false) {
        let found = found.map_err(|e| ExtractError::Archive(e.to_string()))?;
        if found.path() == root || found.path().starts_with(payload) {
            continue;
        }
        escaped.push(
            found
                .path()
                .strip_prefix(root)
                .unwrap_or(found.path())
                .to_string_lossy()
                .into_owned(),
        );
    }
    Ok(escaped)
}

/// `enclosed_name` is `zip`'s own containment check; an entry it rejects is
/// skipped rather than written, so a zip-slip archive yields a short tree instead
/// of a file outside the root.
fn extract_zip(path: &Path, out: &Path) -> Result<(), ExtractError> {
    let file = File::open(path)?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| ExtractError::Archive(e.to_string()))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| ExtractError::Archive(e.to_string()))?;
        let Some(relative) = entry.enclosed_name() else {
            continue;
        };
        let relative = lexical_rel(&relative);
        if relative.as_os_str().is_empty() {
            continue;
        }
        let destination = out.join(&relative);
        if entry.is_dir() {
            std::fs::create_dir_all(&destination)?;
            continue;
        }
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut sink = File::create(&destination)?;
        std::io::copy(&mut entry, &mut sink)?;
    }
    Ok(())
}

/// Tries every candidate rather than only the first that happens to be installed:
/// an `unrar` too old for RAR5 must still fall through to `7z`. A tool that ran
/// and failed is recorded and the next is tried.
fn external_extract(path: &Path, out: &Path, kind: &'static str) -> Result<(), ExtractError> {
    let candidates: &[&str] = match kind {
        "unrar" => &["unrar", "7z", "7za"],
        _ => &["7z", "7za"],
    };
    let mut failures: Vec<String> = Vec::new();
    for tool in candidates {
        let args: Vec<std::ffi::OsString> = if *tool == "unrar" {
            // unrar reads a destination with no trailing separator as a member
            // filter, which matches nothing and exits non-zero.
            let mut target = out.as_os_str().to_os_string();
            target.push(std::path::MAIN_SEPARATOR_STR);
            vec!["x".into(), "-y".into(), path.into(), target]
        } else {
            let mut target = std::ffi::OsString::from("-o");
            target.push(out);
            vec!["x".into(), "-y".into(), target, path.into()]
        };
        match std::process::Command::new(tool).args(&args).output() {
            Ok(output) if output.status.success() => return Ok(()),
            Ok(output) => failures.push(format!(
                "{tool}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )),
            Err(_) => continue,
        }
    }
    if failures.is_empty() {
        Err(ExtractError::MissingTool(kind))
    } else {
        Err(ExtractError::Archive(failures.join("; ")))
    }
}

/// Re-walks what was written. This is both the entry list for formats with no
/// cheap index and the containment backstop for the crates and tools that wrote
/// the tree themselves: anything that did not land under `root` is not reported,
/// and symlinks are skipped because an archive listing does not describe them.
fn walk_extracted(root: &Path) -> Result<Vec<ArchiveEntry>, ExtractError> {
    let mut entries = Vec::new();
    for found in walkdir::WalkDir::new(root).follow_links(false) {
        let found = found.map_err(|e| ExtractError::Archive(e.to_string()))?;
        if !found.file_type().is_file() {
            continue;
        }
        let Ok(relative) = found.path().strip_prefix(root) else {
            continue;
        };
        if relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        {
            continue;
        }
        let metadata = found
            .metadata()
            .map_err(|e| ExtractError::Archive(e.to_string()))?;
        entries.push(ArchiveEntry {
            path: rel_to_slash(relative),
            size: metadata.len(),
        });
    }
    Ok(entries)
}

/// Resolves `.` and `..` lexically, so a path is described the way the filesystem
/// will actually store it. `zip`'s `enclosed_name` permits an interior `..` when the
/// result still lands inside the destination, which means `CoolMod/../evil.lua` is
/// safe but is written as `evil.lua`; without this, the listing and the extraction
/// would describe the same entry by two different names.
fn lexical_rel(relative: &Path) -> std::path::PathBuf {
    let mut out = std::path::PathBuf::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) => return std::path::PathBuf::new(),
        }
    }
    out
}

fn rel_to_slash(relative: &Path) -> String {
    relative
        .components()
        .filter_map(|c| match c {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}
