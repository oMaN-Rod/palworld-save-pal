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
use ring::signature::{UnparsedPublicKey, ED25519};
use sha2::{Digest, Sha256};

/// Where the versioned UI and game data live during a normal install.
pub struct AssetPaths {
    pub ui_dir: PathBuf,
    pub data_dir: PathBuf,
}

const DEFAULT_REPO: &str = "oMaN-Rod/palworld-save-pal";
const SIGNING_PUBLIC_KEY: &[u8; 32] = &[
    28, 163, 7, 28, 242, 168, 116, 229, 210, 190, 104, 92, 159, 94, 44, 123, 68, 38, 75, 85, 152,
    226, 47, 222, 127, 71, 31, 160, 31, 13, 81, 222,
];

/// The server-bundle platform token used in release asset names, e.g.
/// `palstudio-v1.5.0-server-linux-x86_64.tar.gz`.
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

async fn get_https(
    client: &reqwest::Client,
    url: &str,
    description: &str,
) -> Result<reqwest::Response> {
    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("{description} request failed"))?;
    if response.url().scheme() != "https" || response.status().is_redirection() {
        return Err(anyhow!("{description} did not complete over HTTPS"));
    }
    response
        .error_for_status()
        .with_context(|| format!("{description} returned an error status"))
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
    let signature_name = format!("{checksums_name}.sig");

    let client = reqwest::Client::builder()
        .user_agent(concat!("palstudio/", env!("CARGO_PKG_VERSION")))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.url().scheme() == "https" {
                attempt.follow()
            } else {
                attempt.stop()
            }
        }))
        .build()?;
    let release: serde_json::Value = {
        let url = format!("https://api.github.com/repos/{repo}/releases/tags/{tag}");
        get_https(&client, &url, &format!("release {tag} lookup on {repo}"))
            .await?
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
    let checksums_url = asset_url(&checksums_name)
        .ok_or_else(|| anyhow!("release {tag} has no signed checksum manifest {checksums_name}"))?;
    let signature_url = asset_url(&signature_name).ok_or_else(|| {
        anyhow!("release {tag} has no signed checksum signature {signature_name}")
    })?;
    for (name, url) in [
        (bundle_name.as_str(), bundle_url.as_str()),
        (checksums_name.as_str(), checksums_url.as_str()),
        (signature_name.as_str(), signature_url.as_str()),
    ] {
        let parsed = reqwest::Url::parse(url)
            .with_context(|| format!("release asset {name} has an invalid URL"))?;
        if parsed.scheme() != "https" || parsed.host_str().is_none() {
            return Err(anyhow!("release asset {name} did not use HTTPS"));
        }
    }

    tracing::info!("provisioning first-run assets from {repo} {tag} (~25 MB)");
    let bundle = get_https(&client, &bundle_url, &format!("downloading {bundle_name}"))
        .await?
        .bytes()
        .await
        .context("downloaded bundle is truncated")?;

    let checksums = get_https(
        &client,
        &checksums_url,
        &format!("downloading {checksums_name}"),
    )
    .await?
    .bytes()
    .await
    .with_context(|| format!("downloading {checksums_name}"))?;
    let signature = get_https(
        &client,
        &signature_url,
        &format!("downloading {signature_name}"),
    )
    .await?
    .bytes()
    .await
    .with_context(|| format!("downloading {signature_name}"))?;
    UnparsedPublicKey::new(&ED25519, SIGNING_PUBLIC_KEY)
        .verify(&checksums, &signature)
        .map_err(|_| anyhow!("signed checksum manifest verification failed"))?;
    let checksums_text =
        std::str::from_utf8(&checksums).context("signed checksum manifest is not UTF-8")?;
    let expected = parse_checksum(checksums_text, &bundle_name)
        .ok_or_else(|| anyhow!("{checksums_name} has no entry for {bundle_name}"))?;
    let actual = hex(&Sha256::digest(&bundle));
    if !expected.eq_ignore_ascii_case(&actual) {
        return Err(anyhow!(
            "checksum mismatch for {bundle_name}: expected {expected}, got {actual}"
        ));
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
    if data_home.exists() {
        let metadata = std::fs::symlink_metadata(data_home)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(anyhow!(
                "data home is not a private directory: {}",
                data_home.display()
            ));
        }
    } else {
        std::fs::create_dir_all(data_home)?;
    }
    set_mode(data_home, 0o700)?;
    let stage = data_home.join(".provision-stage");
    if stage.exists() {
        let metadata = std::fs::symlink_metadata(&stage)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(anyhow!(
                "provision staging path is unsafe: {}",
                stage.display()
            ));
        }
        std::fs::remove_dir_all(&stage)?;
    }
    std::fs::create_dir_all(&stage)?;

    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(bundle));
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();
        if !safe_member(&path) {
            return Err(anyhow!(
                "refusing unsafe archive member: {}",
                path.display()
            ));
        }
        let entry_type = entry.header().entry_type();
        if entry_type.is_symlink()
            || entry_type.is_hard_link()
            || !entry_type.is_file() && !entry_type.is_dir()
        {
            return Err(anyhow!(
                "archive links and special files are not permitted: {}",
                path.display()
            ));
        }
        if entry.header().mode()? & 0o6000 != 0 {
            return Err(anyhow!(
                "archive privileged mode is not permitted: {}",
                path.display()
            ));
        }
        let Some(rest) = strip_bundle_prefix(&path) else {
            continue; // bin/ and README.txt are not needed here
        };
        if !safe_member(&rest) {
            return Err(anyhow!(
                "refusing bundle member outside ui/ or data/: {}",
                path.display()
            ));
        }
        if !matches!(rest.components().next(), Some(std::path::Component::Normal(name)) if name == "ui" || name == "data")
        {
            continue;
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

    harden_tree(&stage)?;

    let mut retired = Vec::new();
    let mut installed = Vec::new();
    let swap_result: Result<()> = (|| {
        for dir in ["ui", "data"] {
            let staged = stage.join(dir);
            if !staged.is_dir() {
                continue;
            }
            let live = data_home.join(dir);
            let retire = data_home.join(format!(".{dir}.retired"));
            if retire.exists() {
                let metadata = std::fs::symlink_metadata(&retire)?;
                if !metadata.is_dir() || metadata.file_type().is_symlink() {
                    return Err(anyhow!("retirement path is unsafe: {}", retire.display()));
                }
                std::fs::remove_dir_all(&retire)?;
            }
            if live.exists() {
                let metadata = std::fs::symlink_metadata(&live)?;
                if !metadata.is_dir() || metadata.file_type().is_symlink() {
                    return Err(anyhow!("live asset path is unsafe: {}", live.display()));
                }
                std::fs::rename(&live, &retire)
                    .with_context(|| format!("retiring {}", live.display()))?;
                retired.push((live.clone(), retire.clone()));
            }
            std::fs::rename(&staged, &live)
                .with_context(|| format!("moving {} into place", live.display()))?;
            installed.push(live);
        }
        Ok(())
    })();
    if let Err(error) = swap_result {
        for live in installed.iter().rev() {
            let _ = std::fs::remove_dir_all(live);
        }
        for (old_live, old_retire) in retired.iter().rev() {
            if old_retire.exists() {
                let _ = std::fs::rename(old_retire, old_live);
            }
        }
        return Err(error);
    }
    for (_, retire) in &retired {
        if let Err(error) = std::fs::remove_dir_all(retire) {
            tracing::warn!(path = %retire.display(), %error, "could not remove retired asset directory");
        }
    }
    if let Err(error) = std::fs::remove_dir_all(&stage) {
        tracing::warn!(path = %stage.display(), %error, "could not remove provisioning staging directory");
    }
    Ok(())
}

fn harden_tree(root: &Path) -> Result<()> {
    for entry in walkdir(root)? {
        let metadata = std::fs::symlink_metadata(&entry)?;
        if metadata.file_type().is_symlink() {
            return Err(anyhow!(
                "extracted asset contains a symlink: {}",
                entry.display()
            ));
        }
        let permissions = if metadata.is_dir() { 0o700 } else { 0o600 };
        set_mode(&entry, permissions)?;
    }
    Ok(())
}

fn walkdir(root: &Path) -> Result<Vec<PathBuf>> {
    let mut entries = vec![root.to_path_buf()];
    let mut index = 0;
    while index < entries.len() {
        let current = entries[index].clone();
        if std::fs::symlink_metadata(&current)?.is_dir() {
            for child in std::fs::read_dir(&current)? {
                entries.push(child?.path());
            }
        }
        index += 1;
    }
    Ok(entries)
}

fn set_mode(path: &Path, mode: u32) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))?;
    }
    #[cfg(not(unix))]
    let _ = (path, mode);
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
fn safe_member(path: &Path) -> bool {
    path.components()
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
