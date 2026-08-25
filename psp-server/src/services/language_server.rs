//! Acquisition of the `lua-language-server` binary that backs the plugin
//! editor's full tier: download the pinned release, verify its digest, and
//! extract it into an install root.
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::ServiceError;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HostOs {
    Windows,
    Linux,
    Macos,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HostArch {
    X86_64,
    Aarch64,
}

#[derive(Clone, Copy)]
pub struct Release {
    pub os: HostOs,
    pub arch: HostArch,
    pub version: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub exe_relative: &'static str,
}

const VERSION: &str = "3.19.1";

/// The single table of pinned releases. `release_for_host` selects one entry
/// from this by matching `os`/`arch` against the host, never by position, so
/// reordering this table cannot pair a release's URL/digest with the wrong
/// platform. `all_releases` exposes the whole table so a test can walk every
/// pin without depending on which host it runs on.
const RELEASES: &[Release] = &[
    Release {
        os: HostOs::Windows,
        arch: HostArch::X86_64,
        version: VERSION,
        url: concat!(
            "https://github.com/LuaLS/lua-language-server/releases/download/3.19.1/",
            "lua-language-server-3.19.1-win32-x64.zip"
        ),
        sha256: "fdb9a59108cf62517813c97fa5549b0e16d1ef0688306bac728b08434db7e4cd",
        exe_relative: "bin/lua-language-server.exe",
    },
    Release {
        os: HostOs::Linux,
        arch: HostArch::X86_64,
        version: VERSION,
        url: concat!(
            "https://github.com/LuaLS/lua-language-server/releases/download/3.19.1/",
            "lua-language-server-3.19.1-linux-x64.tar.gz"
        ),
        sha256: "e9235d2d72ef55bc41cf8c99cda2ed64777682024b4bb81f5dea425060c5cbb8",
        exe_relative: "bin/lua-language-server",
    },
    Release {
        os: HostOs::Linux,
        arch: HostArch::Aarch64,
        version: VERSION,
        url: concat!(
            "https://github.com/LuaLS/lua-language-server/releases/download/3.19.1/",
            "lua-language-server-3.19.1-linux-arm64.tar.gz"
        ),
        sha256: "abd2572e8fc929dc838a81ffb8473c5bce0bf39bfe8edb4b120b3b623176ce83",
        exe_relative: "bin/lua-language-server",
    },
    Release {
        os: HostOs::Macos,
        arch: HostArch::Aarch64,
        version: VERSION,
        url: concat!(
            "https://github.com/LuaLS/lua-language-server/releases/download/3.19.1/",
            "lua-language-server-3.19.1-darwin-arm64.tar.gz"
        ),
        sha256: "0bc077f4447f076b4c92c14e9fd303f5b569eda2ec74b4dca2b55f75fae2e90c",
        exe_relative: "bin/lua-language-server",
    },
    Release {
        os: HostOs::Macos,
        arch: HostArch::X86_64,
        version: VERSION,
        url: concat!(
            "https://github.com/LuaLS/lua-language-server/releases/download/3.19.1/",
            "lua-language-server-3.19.1-darwin-x64.tar.gz"
        ),
        sha256: "eb373c159cbe556711d7cd316315de2dce969bfd54b31edb7eb9cab2937f2cca",
        exe_relative: "bin/lua-language-server",
    },
];

/// Every pinned release, independent of host — lets a test assert every
/// digest in one run instead of only the one `release_for_host` would pick.
pub fn all_releases() -> &'static [Release] {
    RELEASES
}

/// The pinned release for the running host, or `None` off the supported matrix
/// (windows/x86_64, linux/x86_64, linux/aarch64, macos/x86_64, macos/aarch64).
pub fn release_for_host() -> Option<Release> {
    let os = if cfg!(target_os = "windows") {
        HostOs::Windows
    } else if cfg!(target_os = "linux") {
        HostOs::Linux
    } else if cfg!(target_os = "macos") {
        HostOs::Macos
    } else {
        return None;
    };
    let arch = if cfg!(target_arch = "x86_64") {
        HostArch::X86_64
    } else if cfg!(target_arch = "aarch64") {
        HostArch::Aarch64
    } else {
        return None;
    };
    RELEASES
        .iter()
        .find(|release| release.os == os && release.arch == arch)
        .copied()
}

/// Written into `root` alongside the extracted release so a version bump can
/// be detected without re-hashing or re-downloading anything.
const VERSION_STAMP_FILE: &str = ".lua-language-server-version";

pub fn installed_exe(root: &Path) -> Option<PathBuf> {
    let release = release_for_host()?;
    let exe = root.join(release.exe_relative);
    exe.exists().then_some(exe)
}

/// Whether `root` holds a stamp matching `version` — distinct from
/// `installed_exe`, which only checks the executable is present, so that
/// re-pinning a newer `Release::version` reaches hosts that already have an
/// older one installed.
fn installed_version_matches(root: &Path, version: &str) -> bool {
    std::fs::read_to_string(root.join(VERSION_STAMP_FILE))
        .map(|content| content.trim() == version)
        .unwrap_or(false)
}

pub fn verify_sha256(bytes: &[u8], expected: &str) -> bool {
    let computed = format!("{:x}", Sha256::digest(bytes));
    computed.to_ascii_lowercase() == expected.to_ascii_lowercase()
}

/// Downloads, verifies, and extracts the pinned release for this host.
/// Returns the existing executable without downloading when a matching
/// version is already installed under `root`.
pub async fn ensure(root: &Path) -> Result<PathBuf, ServiceError> {
    let release = release_for_host().ok_or_else(|| {
        ServiceError::Other("no lua-language-server release is pinned for this platform".into())
    })?;
    ensure_release(root, &release).await
}

/// The implementation behind `ensure`, parameterized on the release so tests
/// can exercise the digest-mismatch path against a real download.
pub async fn ensure_release(root: &Path, release: &Release) -> Result<PathBuf, ServiceError> {
    let exe = root.join(release.exe_relative);
    if exe.exists() && installed_version_matches(root, release.version) {
        return Ok(exe);
    }

    let bytes = download_and_verify(release).await?;

    let partial_dir = unique_partial_dir(root);
    if let Err(error) = stage_release(&partial_dir, &bytes, release) {
        let _ = std::fs::remove_dir_all(&partial_dir);
        return Err(error);
    }

    if root.exists() {
        if let Err(error) = std::fs::remove_dir_all(root) {
            let _ = std::fs::remove_dir_all(&partial_dir);
            return Err(ServiceError::Io(error));
        }
    }
    if let Err(error) = std::fs::rename(&partial_dir, root) {
        let _ = std::fs::remove_dir_all(&partial_dir);
        return Err(ServiceError::Io(error));
    }

    Ok(root.join(release.exe_relative))
}

async fn download_and_verify(release: &Release) -> Result<Vec<u8>, ServiceError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|error| ServiceError::Http(error.to_string()))?;
    let response = client
        .get(release.url)
        .send()
        .await
        .map_err(|error| ServiceError::Http(error.to_string()))?
        .error_for_status()
        .map_err(|error| ServiceError::Http(error.to_string()))?;
    let bytes = response
        .bytes()
        .await
        .map_err(|error| ServiceError::Http(error.to_string()))?;

    if !verify_sha256(&bytes, release.sha256) {
        return Err(ServiceError::Other(format!(
            "digest mismatch for {}: expected {}",
            release.url, release.sha256
        )));
    }

    Ok(bytes.to_vec())
}

/// Extracts `bytes` into `partial_dir`, sets the executable bit, and writes
/// the version stamp — everything the eventual rename onto `root` needs to
/// already be true of `partial_dir`.
fn stage_release(partial_dir: &Path, bytes: &[u8], release: &Release) -> Result<(), ServiceError> {
    std::fs::create_dir_all(partial_dir)?;
    extract_archive(bytes, release.url, partial_dir)?;

    let extracted_exe = partial_dir.join(release.exe_relative);
    if !extracted_exe.exists() {
        return Err(ServiceError::Other(format!(
            "expected executable missing after extraction: {}",
            release.exe_relative
        )));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&extracted_exe)?.permissions();
        permissions.set_mode(permissions.mode() | 0o111);
        std::fs::set_permissions(&extracted_exe, permissions)?;
    }

    std::fs::write(partial_dir.join(VERSION_STAMP_FILE), release.version)?;
    Ok(())
}

/// A sibling of `root`, never a child of it, so removing/renaming `root` at
/// the end of `ensure_release` cannot also destroy the extraction in
/// progress. The process id and a monotonic counter make the name unique per
/// call, so concurrent calls never share a path and a leftover from a failed
/// or interrupted run can never block a fresh attempt.
fn unique_partial_dir(root: &Path) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let pid = std::process::id();
    let counter = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let name = root
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "lua-language-server".to_string());
    let unique = format!(".{name}.{pid}.{counter}.partial");
    match root.parent() {
        Some(parent) => parent.join(unique),
        None => PathBuf::from(unique),
    }
}

/// The archive extracts flat: its top level is `bin/`, `main.lua`, `meta/`, and
/// so on, with no version-prefixed directory to strip.
fn extract_archive(bytes: &[u8], url: &str, dest: &Path) -> Result<(), ServiceError> {
    if url.ends_with(".zip") {
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|error| {
            ServiceError::Other(format!("invalid lua-language-server zip: {error}"))
        })?;
        archive.extract(dest).map_err(|error| {
            ServiceError::Other(format!("lua-language-server extract failed: {error}"))
        })?;
        Ok(())
    } else if url.ends_with(".tar.gz") {
        let decoder = flate2::read::GzDecoder::new(std::io::Cursor::new(bytes));
        let mut archive = tar::Archive::new(decoder);
        archive.unpack(dest).map_err(|error| {
            ServiceError::Other(format!("lua-language-server extract failed: {error}"))
        })?;
        Ok(())
    } else {
        Err(ServiceError::Other(format!(
            "unsupported archive extension for {url}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_archive_rejects_an_unknown_extension() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let result = extract_archive(b"", "https://example.com/asset.rar", dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn installed_version_matches_requires_an_exact_stamp() {
        let dir = tempfile::tempdir().expect("a temp dir");
        assert!(!installed_version_matches(dir.path(), "3.19.1"));

        std::fs::write(dir.path().join(VERSION_STAMP_FILE), "3.19.0").expect("write stamp");
        assert!(!installed_version_matches(dir.path(), "3.19.1"));

        std::fs::write(dir.path().join(VERSION_STAMP_FILE), "3.19.1").expect("write stamp");
        assert!(installed_version_matches(dir.path(), "3.19.1"));
    }

    #[test]
    fn unique_partial_dir_never_repeats_across_calls() {
        let root = Path::new("/tmp/lua-language-server");
        let first = unique_partial_dir(root);
        let second = unique_partial_dir(root);
        assert_ne!(first, second);
        assert_eq!(first.parent(), root.parent());
        assert_eq!(second.parent(), root.parent());
    }
}
