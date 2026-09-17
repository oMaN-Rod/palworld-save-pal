//! First-run provisioning: fetches the web UI and game-data tree from the
//! GitHub release matching this binary's version.
//!
//! A published crate cannot embed the bun-built UI or the 28 MB game-data
//! tree, and `ps_server` hard-fails without `data/json` while silently 404ing
//! without the UI — so before the first start we lay both out under the data
//! home from the release's server bundle (the same archive the install
//! scripts use).
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use sha2::{Digest, Sha256};

/// Where the versioned UI and game data live during a normal install.
pub struct AssetPaths {
    pub ui_dir: PathBuf,
    pub data_dir: PathBuf,
}

const DEFAULT_REPO: &str = "oMaN-Rod/palworld-save-pal";

/// The server-bundle platform token used in release asset names, e.g.
/// `palstudio-v1.4.2-server-linux-x86_64.tar.gz`.
fn asset_platform() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Some("linux-x86_64"),
        ("linux", "aarch64") => Some("linux-aarch64"),
        ("macos", "x86_64") => Some("macos-x86_64"),
        ("macos", "aarch64") => Some("macos-aarch64"),
        ("windows", "x86_64") => Some("windows-x64"),
        _ => None,
    }
}

/// Per-user root for `ui/`, `data/` and the database — the conventional data
/// dir on each platform (%APPDATA%, ~/Library/Application Support,
/// XDG_DATA_HOME) without pulling in a directory-crate dependency.
pub fn default_data_home() -> Result<PathBuf> {
    fn under(base: PathBuf) -> PathBuf {
        base.join("palstudio")
    }
    if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA")
            .map(|v| under(PathBuf::from(v)))
            .ok_or_else(|| anyhow!("APPDATA is not set; pass --data-home"))
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(|v| under(PathBuf::from(v).join("Library/Application Support")))
            .ok_or_else(|| anyhow!("HOME is not set; pass --data-home"))
    } else {
        match std::env::var_os("XDG_DATA_HOME") {
            Some(v) if !v.is_empty() => Ok(under(PathBuf::from(v))),
            _ => std::env::var_os("HOME")
                .map(|v| under(PathBuf::from(v).join(".local/share")))
                .ok_or_else(|| anyhow!("neither XDG_DATA_HOME nor HOME is set; pass --data-home")),
        }
    }
}

/// Returns the existing layout, or downloads it into `data_home` first.
pub async fn ensure_assets(data_home: &Path) -> Result<AssetPaths> {
    let ui_dir = data_home.join("ui");
    let data_dir = data_home.join("data");
    let ui_present = ui_dir.join("index.html").is_file();
    let data_present = data_dir.join("json").is_dir();
    if ui_present && data_present {
        return Ok(AssetPaths { ui_dir, data_dir });
    }

    let repo = std::env::var("PALSTUDIO_REPO").unwrap_or_else(|_| DEFAULT_REPO.to_owned());
    let tag = format!("v{}", env!("CARGO_PKG_VERSION"));
    let platform = asset_platform().ok_or_else(|| {
        anyhow!(
            "no prebuilt release bundle for {}-{}; install the UI and game \
             data manually via --ui-dir/--data-dir",
            std::env::consts::OS,
            std::env::consts::ARCH
        )
    })?;
    let bundle_name = format!("palstudio-{tag}-server-{platform}.tar.gz");
    let checksums_name = format!("palstudio-{tag}-server-checksums.txt");

    let client = reqwest::Client::builder()
        .user_agent(concat!("palstudio/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let release: serde_json::Value = {
        let resp = client
            .get(format!(
                "https://api.github.com/repos/{repo}/releases/tags/{tag}"
            ))
            .send()
            .await
            .map_err(|error| anyhow!("release {tag} lookup on {repo} failed: {error}"))?;
        resp.error_for_status()
            .map_err(|error| anyhow!("release {tag} lookup on {repo} failed: {error}"))?
            .json()
            .await
            .context("could not parse the release metadata")?
    };

    let asset_url = |name: &str| -> Option<String> {
        release["assets"]
            .as_array()?
            .iter()
            .find(|asset| asset["name"].as_str() == Some(name))
            .and_then(|asset| asset["browser_download_url"].as_str())
            .map(str::to_owned)
    };
    let bundle_url = asset_url(&bundle_name).ok_or_else(|| {
        anyhow!(
            "release {tag} has no {bundle_name}; download it from \
             https://github.com/{repo}/releases/tag/{tag} or install via the \
             install script"
        )
    })?;

    tracing::info!("provisioning first-run assets from {repo} {tag} (~25 MB)");
    let bundle = client
        .get(&bundle_url)
        .send()
        .await
        .and_then(|resp| resp.error_for_status())
        .map_err(|error| anyhow!("downloading {bundle_name}: {error}"))?
        .bytes()
        .await
        .context("downloaded bundle is truncated")?;

    // The checksum asset is how the install scripts verify too; a release
    // built without it still works, we just skip verification.
    if let Some(checksums_url) = asset_url(&checksums_name) {
        let checksums = client
            .get(&checksums_url)
            .send()
            .await
            .and_then(|resp| resp.error_for_status())
            .map_err(|error| anyhow!("downloading {checksums_name}: {error}"))?
            .text()
            .await?;
        let expected = parse_checksum(&checksums, &bundle_name)
            .ok_or_else(|| anyhow!("{checksums_name} has no entry for {bundle_name}"))?;
        let actual = hex(&Sha256::digest(&bundle));
        if !expected.eq_ignore_ascii_case(&actual) {
            return Err(anyhow!(
                "checksum mismatch for {bundle_name}: expected {expected}, got {actual}"
            ));
        }
    } else {
        tracing::warn!("release carries no {checksums_name}; skipping verification");
    }

    extract_assets(&bundle, data_home)
        .with_context(|| format!("extracting {bundle_name} into {}", data_home.display()))?;
    if !ui_dir.join("index.html").is_file() || !data_dir.join("json").is_dir() {
        return Err(anyhow!("{bundle_name} did not contain ui/ and data/"));
    }
    Ok(AssetPaths { ui_dir, data_dir })
}

/// Maps a sha256sum-style `<hash>  <filename>` body to the hash for `name`.
fn parse_checksum(body: &str, name: &str) -> Option<String> {
    body.lines().find_map(|line| {
        let (hash, file) = line.split_once(|c: char| c.is_whitespace())?;
        let file = file.trim();
        (file == name).then(|| hash.trim().to_owned())
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Pulls `palstudio/ui/**` and `palstudio/data/**` out of the bundle, staging
/// first so a failed extract never replaces a working layout.
fn extract_assets(bundle: &[u8], data_home: &Path) -> Result<()> {
    let stage = data_home.join(".provision-stage");
    let _ = std::fs::remove_dir_all(&stage);
    std::fs::create_dir_all(&stage)?;

    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(bundle));
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();
        let Some(rest) = strip_bundle_prefix(&path) else {
            continue; // bin/ and README.txt are not needed here
        };
        if !safe_member(&rest) {
            return Err(anyhow!(
                "refusing bundle member outside ui/ or data/: {}",
                path.display()
            ));
        }
        let dest = stage.join(rest);
        if entry.header().entry_type().is_dir() {
            std::fs::create_dir_all(&dest)?;
        } else {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            entry.unpack(&dest)?;
        }
    }

    for dir in ["ui", "data"] {
        let staged = stage.join(dir);
        if !staged.is_dir() {
            continue;
        }
        let live = data_home.join(dir);
        let retire = data_home.join(format!(".{dir}.retired"));
        let _ = std::fs::remove_dir_all(&retire);
        if live.exists() && std::fs::rename(&live, &retire).is_err() {
            std::fs::remove_dir_all(&live)
                .with_context(|| format!("replacing {}", live.display()))?;
        }
        std::fs::rename(&staged, &live)
            .with_context(|| format!("moving {} into place", live.display()))?;
        let _ = std::fs::remove_dir_all(&retire);
    }
    let _ = std::fs::remove_dir_all(&stage);
    Ok(())
}

/// `palstudio/ui/x` → `ui/x`, matching the bundle layout the release job packs.
fn strip_bundle_prefix(path: &Path) -> Option<PathBuf> {
    let mut components = path.components();
    match components.next() {
        Some(std::path::Component::Normal(first)) if first == "palstudio" => {
            Some(components.as_path().to_path_buf())
        }
        _ => None,
    }
}

/// Defense against `../`, absolute or rooted members inside a malicious or
/// corrupted archive: everything under ui/ and data/ must be relative and
/// purely normal path components. (The tar writer rejects `..` when creating
/// archives, but a hostile file needs no cooperation from our writer.)
fn safe_member(rest: &Path) -> bool {
    rest.components()
        .all(|c| matches!(c, std::path::Component::Normal(_)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;

    #[test]
    fn parses_the_hash_for_the_named_asset() {
        let body = "1111111111111111111111111111111111111111111111111111111111111111  other.tar.gz\n\
                    abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789  palstudio-v1.2.3-server-linux-x86_64.tar.gz\n";
        assert_eq!(
            parse_checksum(body, "palstudio-v1.2.3-server-linux-x86_64.tar.gz").as_deref(),
            Some("abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789")
        );
        assert_eq!(parse_checksum(body, "missing.tar.gz"), None);
    }

    #[test]
    fn hex_is_lowercase_sha256() {
        assert_eq!(
            hex(&Sha256::digest(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn bundle_prefix_strips_only_the_leading_dir() {
        assert_eq!(
            strip_bundle_prefix(Path::new("palstudio/ui/_app/immutable/x.js")),
            Some(PathBuf::from("ui/_app/immutable/x.js"))
        );
        assert_eq!(
            strip_bundle_prefix(Path::new("palstudio/README.txt")),
            Some(PathBuf::from("README.txt"))
        );
        assert_eq!(strip_bundle_prefix(Path::new("ui/index.html")), None);
    }

    #[test]
    fn extract_lays_out_ui_and_data_and_ignores_the_rest() {
        // Build a real bundle in memory with the release job's layout.
        let mut raw = Vec::new();
        {
            let mut builder =
                tar::Builder::new(GzEncoder::new(&mut raw, flate2::Compression::default()));
            let add = |builder: &mut tar::Builder<GzEncoder<&mut Vec<u8>>>,
                       path: &str,
                       contents: &[u8]| {
                let mut header = tar::Header::new_gnu();
                header.set_size(contents.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                builder.append_data(&mut header, path, contents).unwrap();
            };
            add(&mut builder, "palstudio/ui/index.html", b"<html></html>");
            add(&mut builder, "palstudio/data/json/game.json", b"{}");
            add(&mut builder, "palstudio/bin/palstudio", b"elf");
            add(&mut builder, "palstudio/README.txt", b"readme");
            builder.into_inner().unwrap().finish().unwrap();
        }

        let home = tempfile::tempdir().expect("tempdir");
        // A stale ui/ from a previous partial run must be retired, not merged.
        std::fs::create_dir_all(home.path().join("ui")).unwrap();
        std::fs::write(home.path().join("ui/stale.txt"), b"stale").unwrap();

        extract_assets(&raw, home.path()).expect("extraction succeeds");

        assert_eq!(
            std::fs::read(home.path().join("ui/index.html")).unwrap(),
            b"<html></html>"
        );
        assert!(!home.path().join("ui/stale.txt").exists());
        assert_eq!(
            std::fs::read(home.path().join("data/json/game.json")).unwrap(),
            b"{}"
        );
        assert!(!home.path().join("bin").exists());
        assert!(!home.path().join("README.txt").exists());
        assert!(!home.path().join(".provision-stage").exists());
    }

    #[test]
    fn safe_member_rejects_traversal_and_absolute_paths() {
        assert!(safe_member(Path::new("ui/_app/immutable/x.js")));
        assert!(safe_member(Path::new("data/json/l10n/de/x.json")));
        assert!(!safe_member(Path::new("ui/../../escape")));
        assert!(!safe_member(Path::new("../escape")));
        assert!(!safe_member(Path::new("/etc/passwd")));
        assert!(!safe_member(Path::new("./dotdir")));
    }
}
