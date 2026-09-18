//! The `.psmods` share format: a zip holding `profile.json` and, optionally,
//! the archives its entries were installed from under `archives/`.
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use zip::result::ZipError;
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

use super::uploads::valid_name;

pub const FORMAT: &str = "palstudio-profile";
pub const FORMAT_VERSION: u32 = 1;
const PROFILE_JSON: &str = "profile.json";
const ARCHIVES_PREFIX: &str = "archives/";
const MAX_PROFILE_JSON_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SharedProfile {
    pub format: String,
    pub format_version: u32,
    pub name: String,
    pub exported_at: String,
    pub target_kind: String,
    pub target_platform: String,
    pub entries: Vec<SharedEntry>,
    pub frameworks: Vec<SharedFramework>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SharedEntry {
    pub mod_id: String,
    pub name: String,
    pub mod_type: String,
    pub source_kind: String,
    pub version: String,
    pub mod_version_id: String,
    pub enabled: bool,
    pub load_order: i64,
    /// Zip entry name under `archives/`, when the archive was included.
    pub archive: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SharedFramework {
    pub framework: String,
    pub mod_id: String,
    pub version: String,
    pub mod_version_id: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ShareError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("not a readable zip: {0}")]
    Zip(String),
    #[error("not a PalStudio profile: {0}")]
    Invalid(String),
    #[error("unsupported profile format {format} version {format_version}")]
    UnsupportedFormat { format: String, format_version: u32 },
    #[error("unsafe archive entry {0}")]
    UnsafeEntry(String),
}

impl ShareError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io(_) => "io",
            Self::Zip(_) | Self::Invalid(_) | Self::UnsafeEntry(_) => "invalid_archive",
            Self::UnsupportedFormat { .. } => "unsupported_format",
        }
    }
}

#[derive(serde::Deserialize)]
struct FormatHeader {
    format: String,
    format_version: u32,
}

pub fn archive_entry_name(mod_version_id: &str, file_name: &str) -> String {
    let version: String = mod_version_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let file_name = file_name.rsplit(['/', '\\']).next().unwrap_or_default();
    format!("{ARCHIVES_PREFIX}{version}/{file_name}")
}

/// Writes `{dest}.part` and renames it over `dest` only once the zip is
/// complete. `archives` pairs each zip entry name with its source file.
pub fn write_psmods(
    dest: &Path,
    profile: &SharedProfile,
    archives: &[(String, PathBuf)],
) -> Result<(), ShareError> {
    let part = dest.with_extension("psmods.part");
    let written = write_zip(&part, profile, archives)
        .and_then(|()| std::fs::rename(&part, dest).map_err(ShareError::from));
    if written.is_err() {
        if let Err(error) = std::fs::remove_file(&part) {
            if error.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(%error, path = %part.display(), "could not delete a partial export");
            }
        }
    }
    written
}

fn write_zip(
    part: &Path,
    profile: &SharedProfile,
    archives: &[(String, PathBuf)],
) -> Result<(), ShareError> {
    let json = serde_json::to_vec_pretty(profile).map_err(std::io::Error::from)?;
    let mut writer = zip::ZipWriter::new(std::fs::File::create(part)?);
    writer
        .start_file(
            PROFILE_JSON,
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
        )
        .map_err(zip_error)?;
    writer.write_all(&json)?;
    for (name, source) in archives {
        let mut file = std::fs::File::open(source)?;
        let large = file.metadata()?.len() >= u64::from(u32::MAX);
        writer
            .start_file(
                name.as_str(),
                SimpleFileOptions::default()
                    .compression_method(CompressionMethod::Stored)
                    .large_file(large),
            )
            .map_err(zip_error)?;
        std::io::copy(&mut file, &mut writer)?;
    }
    writer.finish().map_err(zip_error)?.sync_all()?;
    Ok(())
}

pub fn read_profile(path: &Path) -> Result<SharedProfile, ShareError> {
    let mut zip = open(path)?;
    let entry = match zip.by_name(PROFILE_JSON) {
        Ok(entry) => entry,
        Err(ZipError::FileNotFound) => {
            return Err(ShareError::Invalid(format!("{PROFILE_JSON} is missing")))
        }
        Err(error) => return Err(zip_error(error)),
    };
    let mut bytes = Vec::new();
    entry
        .take(MAX_PROFILE_JSON_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_PROFILE_JSON_BYTES {
        return Err(ShareError::Invalid(format!("{PROFILE_JSON} is too large")));
    }
    let invalid =
        |error: serde_json::Error| ShareError::Invalid(format!("{PROFILE_JSON}: {error}"));
    let header: FormatHeader = serde_json::from_slice(&bytes).map_err(invalid)?;
    if header.format != FORMAT || header.format_version != FORMAT_VERSION {
        return Err(ShareError::UnsupportedFormat {
            format: header.format,
            format_version: header.format_version,
        });
    }
    serde_json::from_slice(&bytes).map_err(invalid)
}

/// Copies one included archive to `out_dir/{file name}`. The name must be one
/// the profile lists; nothing from the zip decides where the file goes.
pub fn extract_archive(
    path: &Path,
    profile: &SharedProfile,
    entry_name: &str,
    out_dir: &Path,
) -> Result<PathBuf, ShareError> {
    let unsafe_entry = || ShareError::UnsafeEntry(entry_name.to_string());
    if !profile
        .entries
        .iter()
        .any(|entry| entry.archive.as_deref() == Some(entry_name))
    {
        return Err(unsafe_entry());
    }
    let Some(rest) = entry_name.strip_prefix(ARCHIVES_PREFIX) else {
        return Err(unsafe_entry());
    };
    if entry_name.contains('\\')
        || rest
            .split('/')
            .any(|segment| segment == ".." || segment == ".")
    {
        return Err(unsafe_entry());
    }
    let file_name = rest.rsplit('/').next().unwrap_or_default();
    if !valid_name(file_name) {
        return Err(unsafe_entry());
    }
    let mut zip = open(path)?;
    let mut entry = match zip.by_name(entry_name) {
        Ok(entry) => entry,
        Err(ZipError::FileNotFound) => {
            return Err(ShareError::Invalid(format!("{entry_name} is missing")))
        }
        Err(error) => return Err(zip_error(error)),
    };
    // The writer stores archives uncompressed; a compressed entry could inflate
    // far past its size, and zip only checks the CRC after everything is read.
    if entry.compression() != CompressionMethod::Stored {
        return Err(unsafe_entry());
    }
    let declared = entry.size();
    let dest = out_dir.join(file_name);
    let copied = std::fs::File::create(&dest).and_then(|mut file| {
        let written = std::io::copy(
            &mut (&mut entry).take(declared.saturating_add(1)),
            &mut file,
        )?;
        file.sync_all()?;
        Ok(written)
    });
    match copied {
        Ok(written) if written <= declared => Ok(dest),
        Ok(_) => {
            let _ = std::fs::remove_file(&dest);
            Err(ShareError::Invalid(format!(
                "{entry_name} holds more than its declared {declared} bytes"
            )))
        }
        Err(error) => {
            let _ = std::fs::remove_file(&dest);
            Err(error.into())
        }
    }
}

fn open(path: &Path) -> Result<zip::ZipArchive<std::fs::File>, ShareError> {
    zip::ZipArchive::new(std::fs::File::open(path)?).map_err(zip_error)
}

fn zip_error(error: ZipError) -> ShareError {
    ShareError::Zip(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn profile() -> SharedProfile {
        SharedProfile {
            format: FORMAT.to_string(),
            format_version: FORMAT_VERSION,
            name: "Co-op".to_string(),
            exported_at: "2026-09-14T12:00:00+00:00".to_string(),
            target_kind: "client".to_string(),
            target_platform: "win64".to_string(),
            entries: vec![SharedEntry {
                mod_id: "coolmod-ue4ss".to_string(),
                name: "CoolMod".to_string(),
                mod_type: "ue4ss".to_string(),
                source_kind: "local".to_string(),
                version: "1.0".to_string(),
                mod_version_id: "coolmod-ue4ss@1.0".to_string(),
                enabled: true,
                load_order: 0,
                archive: None,
            }],
            frameworks: vec![SharedFramework {
                framework: "ue4ss".to_string(),
                mod_id: "ue4ss".to_string(),
                version: "3.0.1".to_string(),
                mod_version_id: "ue4ss@3.0.1".to_string(),
            }],
        }
    }

    fn with_archive(name: &str) -> SharedProfile {
        let mut profile = profile();
        profile.entries[0].archive = Some(name.to_string());
        profile
    }

    fn write_raw_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let mut writer = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (name, body) in entries {
            writer
                .start_file(
                    *name,
                    SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
                )
                .unwrap();
            writer.write_all(body).unwrap();
        }
        writer.finish().unwrap();
    }

    fn part_of(dest: &Path) -> PathBuf {
        dest.with_extension("psmods.part")
    }

    #[test]
    fn a_compressed_archive_entry_is_refused_before_anything_is_written() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("bomb.psmods");
        let name = "archives/x/bomb.zip";
        write_raw_zip(
            &dest,
            &[(PROFILE_JSON, b"{}"), (name, &vec![0u8; 1024 * 1024])],
        );
        let out_dir = dir.path().join("out");
        std::fs::create_dir_all(&out_dir).unwrap();

        let result = extract_archive(&dest, &with_archive(name), name, &out_dir);

        assert!(
            matches!(&result, Err(ShareError::UnsafeEntry(entry)) if entry == name),
            "{result:?}"
        );
        assert_eq!(std::fs::read_dir(&out_dir).unwrap().count(), 0);
    }

    #[test]
    fn a_listed_archive_missing_from_the_zip_is_invalid() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("p.psmods");
        write_psmods(&dest, &profile(), &[]).unwrap();
        let name = "archives/x/absent.zip";
        let out_dir = dir.path().join("out");
        std::fs::create_dir_all(&out_dir).unwrap();

        let result = extract_archive(&dest, &with_archive(name), name, &out_dir);

        assert!(matches!(result, Err(ShareError::Invalid(_))), "{result:?}");
        assert_eq!(std::fs::read_dir(&out_dir).unwrap().count(), 0);
    }

    #[test]
    fn an_oversize_profile_json_is_invalid() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("big.psmods");
        let padding = vec![b' '; MAX_PROFILE_JSON_BYTES as usize + 1];
        write_raw_zip(&dest, &[(PROFILE_JSON, &padding)]);

        let result = read_profile(&dest);

        assert!(
            matches!(&result, Err(ShareError::Invalid(message)) if message.contains("too large")),
            "{result:?}"
        );
    }

    #[test]
    fn a_written_profile_reads_back_identically() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("p.psmods");

        write_psmods(&dest, &profile(), &[]).unwrap();

        assert_eq!(read_profile(&dest).unwrap(), profile());
        assert!(!part_of(&dest).exists());
    }

    #[test]
    fn included_archives_are_stored_under_their_entry_names() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("CoolMod-1.0.zip");
        let bytes: Vec<u8> = (0..70_000u32).map(|n| (n * 7 % 251) as u8).collect();
        std::fs::write(&source, &bytes).unwrap();
        let name = archive_entry_name("coolmod-ue4ss@1.0", "CoolMod-1.0.zip");
        assert_eq!(name, "archives/coolmod-ue4ss_1.0/CoolMod-1.0.zip");
        let profile = with_archive(&name);
        let dest = dir.path().join("p.psmods");

        write_psmods(&dest, &profile, &[(name.clone(), source)]).unwrap();

        let mut zip = zip::ZipArchive::new(std::fs::File::open(&dest).unwrap()).unwrap();
        let names: Vec<&str> = zip.file_names().collect();
        assert!(names.contains(&"profile.json"), "{names:?}");
        assert!(names.contains(&name.as_str()), "{names:?}");
        assert_eq!(
            zip.by_name(&name).unwrap().compression(),
            zip::CompressionMethod::Stored
        );
        let out_dir = dir.path().join("out");
        std::fs::create_dir_all(&out_dir).unwrap();
        let extracted = extract_archive(&dest, &profile, &name, &out_dir).unwrap();
        assert_eq!(extracted, out_dir.join("CoolMod-1.0.zip"));
        assert_eq!(std::fs::read(&extracted).unwrap(), bytes);
    }

    #[test]
    fn entry_names_sanitise_the_version_and_keep_only_the_file_name() {
        assert_eq!(
            archive_entry_name("a/b:c d@1", "C:\\mods\\../x/Cool.zip"),
            "archives/a_b_c_d_1/Cool.zip"
        );
    }

    #[test]
    fn extraction_only_accepts_names_the_profile_lists() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("evil.psmods");
        let evil = "archives/x/../../evil.zip";
        write_raw_zip(&dest, &[(PROFILE_JSON, b"{}"), (evil, b"payload")]);
        let out_dir = dir.path().join("a").join("b");
        std::fs::create_dir_all(&out_dir).unwrap();

        for (listed, requested) in [
            (evil, evil),
            ("archives/x/./evil.zip", "archives/x/./evil.zip"),
            ("archives\\x\\evil.zip", "archives\\x\\evil.zip"),
            ("other/evil.zip", "other/evil.zip"),
            ("archives/x/.evil.zip", "archives/x/.evil.zip"),
            ("archives/x/listed.zip", "archives/x/unlisted.zip"),
        ] {
            let result = extract_archive(&dest, &with_archive(listed), requested, &out_dir);
            assert!(
                matches!(result, Err(ShareError::UnsafeEntry(_))),
                "{requested}: {result:?}"
            );
        }
        assert!(!dir.path().join("evil.zip").exists());
        assert!(!dir.path().join("a").join("evil.zip").exists());
        assert_eq!(std::fs::read_dir(&out_dir).unwrap().count(), 0);
    }

    #[test]
    fn another_format_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        for (format, format_version) in [("other", FORMAT_VERSION), (FORMAT, 2)] {
            let dest = dir.path().join(format!("{format}-{format_version}.psmods"));
            let mut other = profile();
            other.format = format.to_string();
            other.format_version = format_version;
            write_psmods(&dest, &other, &[]).unwrap();

            let result = read_profile(&dest);
            assert!(
                matches!(
                    &result,
                    Err(ShareError::UnsupportedFormat { format: f, format_version: v })
                        if f == format && *v == format_version
                ),
                "{result:?}"
            );
            assert_eq!(result.unwrap_err().code(), "unsupported_format");
        }
    }

    #[test]
    fn a_missing_profile_json_is_invalid() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("empty.psmods");
        write_raw_zip(&dest, &[("readme.txt", b"hi")]);

        let result = read_profile(&dest);

        assert!(matches!(result, Err(ShareError::Invalid(_))), "{result:?}");
        assert_eq!(result.unwrap_err().code(), "invalid_archive");
    }

    #[test]
    fn writing_never_leaves_a_partial_file() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("p.psmods");
        let name = archive_entry_name("coolmod-ue4ss@1.0", "CoolMod-1.0.zip");

        let result = write_psmods(
            &dest,
            &with_archive(&name),
            &[(name, dir.path().join("missing.zip"))],
        );

        assert!(matches!(result, Err(ShareError::Io(_))), "{result:?}");
        assert!(!dest.exists());
        assert!(!part_of(&dest).exists());
    }
}
