//! GitHub release resolution and the bundled Amity fallback.

use super::source::{Channel, FrameworkSource, Progress, Release, ReleaseAsset, SourceError};
use ps_core::mods::FrameworkKey;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};

pub const UE4SS_REPO: &str = "Okaetsu/RE-UE4SS";
pub const PALSCHEMA_REPO: &str = "Okaetsu/PalSchema";
pub const MAX_DOWNLOAD: u64 = 256 * 1024 * 1024;
const API_BASE: &str = "https://api.github.com";
const WEB_BASE: &str = "https://github.com";
const EXPERIMENTAL_TAG: &str = "experimental-palworld";

fn repo_of(key: FrameworkKey) -> Option<&'static str> {
    match key {
        FrameworkKey::Ue4ss => Some(UE4SS_REPO),
        FrameworkKey::PalSchema => Some(PALSCHEMA_REPO),
        FrameworkKey::Amity => None,
    }
}

/// `2026-08-28T20:18:46Z` → (`20260828`, `28.08.2026`).
fn date_parts(updated_at: &str) -> Option<(String, String)> {
    let date = updated_at.get(..10)?;
    let mut parts = date.split('-');
    let (y, m, d) = (parts.next()?, parts.next()?, parts.next()?);
    Some((format!("{y}{m}{d}"), format!("{d}.{m}.{y}")))
}

pub fn pick_release(
    key: FrameworkKey,
    channel: Channel,
    dev: bool,
    release: &serde_json::Value,
) -> Option<Release> {
    let repo = repo_of(key)?;
    let tag = release.get("tag_name")?.as_str()?.to_string();
    let assets = release.get("assets")?.as_array()?;
    let asset = assets.iter().find(|asset| {
        let Some(name) = asset.get("name").and_then(|n| n.as_str()) else {
            return false;
        };
        let lower = name.to_ascii_lowercase();
        match key {
            FrameworkKey::Ue4ss => {
                name.starts_with("UE4SS-Palworld")
                    && lower.ends_with(".zip")
                    && lower.contains("zdev") == dev
            }
            FrameworkKey::PalSchema => {
                name.starts_with("PalSchema")
                    && lower.ends_with(".zip")
                    && lower.contains("_dev") == dev
            }
            FrameworkKey::Amity => false,
        }
    })?;
    let name = asset.get("name")?.as_str()?.to_string();
    let url = asset.get("browser_download_url")?.as_str()?.to_string();
    let size = asset.get("size").and_then(|s| s.as_u64());
    let dates = asset
        .get("updated_at")
        .and_then(|u| u.as_str())
        .and_then(date_parts);
    let (version, display) = match key {
        FrameworkKey::Ue4ss => {
            let base = match channel {
                Channel::Latest => tag.clone(),
                Channel::Experimental => format!("experimental-{}", dates.as_ref()?.0),
            };
            let version = if dev { format!("{base}-zdev") } else { base };
            let display = match &dates {
                Some((_, shown)) => format!("{tag} ({shown})"),
                None => version.clone(),
            };
            (version, display)
        }
        _ => {
            let version = if dev {
                format!("{tag}-dev")
            } else {
                tag.clone()
            };
            (version.clone(), version)
        }
    };
    Some(Release {
        key,
        tag,
        version,
        display,
        asset: ReleaseAsset { name, url, size },
        origin: "github",
        repo: Some(repo),
    })
}

/// The tag in a `…/releases/tag/{tag}` redirect.
pub fn tag_from_location(location: &str) -> Option<String> {
    let (_, rest) = location.split_once("/releases/tag/")?;
    let tag = rest.split(['?', '#', '/']).next()?;
    (!tag.is_empty()).then(|| tag.to_string())
}

pub fn fallback_release(
    key: FrameworkKey,
    dev: bool,
    tag: &str,
    web_base: &str,
) -> Option<Release> {
    let repo = repo_of(key)?;
    let (name, version) = match key {
        FrameworkKey::Ue4ss => (
            format!(
                "UE4SS-Palworld-g{tag}{}.zip",
                if dev { "-zDev" } else { "" }
            ),
            if dev {
                format!("{tag}-zdev")
            } else {
                tag.to_string()
            },
        ),
        _ => (
            format!("PalSchema_{tag}{}.zip", if dev { "_Dev" } else { "" }),
            if dev {
                format!("{tag}-dev")
            } else {
                tag.to_string()
            },
        ),
    };
    let url = format!("{web_base}/{repo}/releases/download/{tag}/{name}");
    Some(Release {
        key,
        tag: tag.to_string(),
        display: version.clone(),
        version,
        asset: ReleaseAsset {
            name,
            url,
            size: None,
        },
        origin: "github",
        repo: Some(repo),
    })
}

/// The newest `PSAmity-UE4SS-{version}.zip` in `dir`.
pub fn bundled_amity(dir: &std::path::Path) -> Option<Release> {
    let entries = std::fs::read_dir(dir).ok()?;
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let version = name
                .strip_prefix("PSAmity-UE4SS-")?
                .strip_suffix(".zip")?
                .to_string();
            (!version.is_empty() && entry.path().is_file()).then(|| (version, name, entry.path()))
        })
        .max_by(|a, b| ps_core::mods::compare_versions(&a.0, &b.0))
        .map(|(version, name, path)| Release {
            key: FrameworkKey::Amity,
            tag: version.clone(),
            display: version.clone(),
            version,
            asset: ReleaseAsset {
                name,
                url: path.to_string_lossy().into_owned(),
                size: None,
            },
            origin: "bundled",
            repo: None,
        })
}

type CacheKey = (FrameworkKey, Channel, bool);
type CachedRelease = (std::time::Instant, Result<Release, SourceError>);

pub struct ReleaseSources {
    http: reqwest::Client,
    probe: reqwest::Client,
    download: reqwest::Client,
    api_base: String,
    web_base: String,
    bundle_dir: Option<std::path::PathBuf>,
    max_download: u64,
    chunk_timeout: std::time::Duration,
    cache: tokio::sync::Mutex<std::collections::HashMap<CacheKey, CachedRelease>>,
}

const FRESH_FOR: std::time::Duration = std::time::Duration::from_secs(600);
const RETRY_AFTER: std::time::Duration = std::time::Duration::from_secs(120);
const CHUNK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

fn default_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_str(&format!("PalStudio/{}", env!("CARGO_PKG_VERSION")))
            .expect("valid header value"),
    );
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/vnd.github+json"),
    );
    headers
}

fn append_part(dest: &std::path::Path) -> std::path::PathBuf {
    let mut name = dest.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    dest.with_file_name(name)
}

impl ReleaseSources {
    pub fn new(api_base: &str, web_base: &str, bundle_dir: Option<std::path::PathBuf>) -> Self {
        let http = reqwest::Client::builder()
            .default_headers(default_headers())
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("http client");
        let probe = reqwest::Client::builder()
            .default_headers(default_headers())
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("probe client");
        let download = reqwest::Client::builder()
            .default_headers(default_headers())
            .connect_timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("download client");
        Self {
            http,
            probe,
            download,
            api_base: api_base.trim_end_matches('/').to_string(),
            web_base: web_base.trim_end_matches('/').to_string(),
            bundle_dir,
            max_download: MAX_DOWNLOAD,
            chunk_timeout: CHUNK_TIMEOUT,
            cache: tokio::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    pub fn github(bundle_dir: Option<std::path::PathBuf>) -> Self {
        Self::new(API_BASE, WEB_BASE, bundle_dir)
    }

    pub fn with_max_download(mut self, bytes: u64) -> Self {
        self.max_download = bytes;
        self
    }

    pub fn with_chunk_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.chunk_timeout = timeout;
        self
    }

    /// A plain-http asset url is only trusted when the release source itself
    /// (`web_base`) is configured over http, as a local test server is.
    fn insecure_url(&self, url: &str) -> bool {
        url.starts_with("http://") && !self.web_base.starts_with("http://")
    }

    fn is_rate_limited(response: &reqwest::Response) -> bool {
        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return true;
        }
        response.status() == reqwest::StatusCode::FORBIDDEN
            && response
                .headers()
                .get("x-ratelimit-remaining")
                .and_then(|v| v.to_str().ok())
                == Some("0")
    }

    async fn resolve_latest(
        &self,
        key: FrameworkKey,
        channel: Channel,
        dev: bool,
    ) -> Result<Release, SourceError> {
        let repo = repo_of(key).expect("Amity is handled before this call");
        let url = match channel {
            Channel::Latest => format!("{}/repos/{repo}/releases/latest", self.api_base),
            Channel::Experimental => {
                format!(
                    "{}/repos/{repo}/releases/tags/{EXPERIMENTAL_TAG}",
                    self.api_base
                )
            }
        };
        let response = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|error| SourceError::Network(error.to_string()))?;

        if Self::is_rate_limited(&response) {
            if channel == Channel::Experimental {
                return Err(SourceError::RateLimited);
            }
            let web_url = format!("{}/{repo}/releases/latest", self.web_base);
            let probe = self
                .probe
                .get(&web_url)
                .send()
                .await
                .map_err(|_| SourceError::RateLimited)?;
            if probe.status().is_redirection() {
                if let Some(tag) = probe
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|v| v.to_str().ok())
                    .and_then(tag_from_location)
                {
                    return fallback_release(key, dev, &tag, &self.web_base)
                        .ok_or(SourceError::RateLimited);
                }
            }
            return Err(SourceError::RateLimited);
        }

        let status = response.status();
        if !status.is_success() {
            return Err(SourceError::Network(format!("GitHub answered {status}")));
        }

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|error| SourceError::Network(error.to_string()))?;
        pick_release(key, channel, dev, &body).ok_or(SourceError::NoAsset)
    }
}

#[async_trait::async_trait]
impl FrameworkSource for ReleaseSources {
    async fn latest(
        &self,
        key: FrameworkKey,
        channel: Channel,
        dev: bool,
    ) -> Result<Release, SourceError> {
        if key == FrameworkKey::Amity {
            return self
                .bundle_dir
                .as_deref()
                .and_then(bundled_amity)
                .ok_or(SourceError::Unavailable);
        }
        let channel = if key == FrameworkKey::PalSchema {
            Channel::Latest
        } else {
            channel
        };
        let cache_key = (key, channel, dev);
        {
            let cache = self.cache.lock().await;
            if let Some((cached_at, result)) = cache.get(&cache_key) {
                let ttl = if result.is_ok() {
                    FRESH_FOR
                } else {
                    RETRY_AFTER
                };
                if cached_at.elapsed() < ttl {
                    return result.clone();
                }
            }
        }
        let result = self.resolve_latest(key, channel, dev).await;
        let mut cache = self.cache.lock().await;
        cache.insert(cache_key, (std::time::Instant::now(), result.clone()));
        result
    }

    async fn fetch(
        &self,
        release: &Release,
        dest: &std::path::Path,
        progress: Progress<'_>,
    ) -> Result<(), SourceError> {
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let part = append_part(dest);
        let result = self.fetch_into(release, &part, progress).await;
        match result {
            Ok(()) => match tokio::fs::rename(&part, dest).await {
                Ok(()) => Ok(()),
                Err(error) => {
                    let _ = tokio::fs::remove_file(&part).await;
                    Err(SourceError::Io(error.to_string()))
                }
            },
            Err(error) => {
                let _ = tokio::fs::remove_file(&part).await;
                Err(error)
            }
        }
    }
}

impl ReleaseSources {
    async fn fetch_into(
        &self,
        release: &Release,
        part: &std::path::Path,
        progress: Progress<'_>,
    ) -> Result<(), SourceError> {
        if release.origin == "bundled" {
            let source = std::path::PathBuf::from(&release.asset.url);
            let dest = part.to_path_buf();
            let max = self.max_download;
            return tokio::task::spawn_blocking(move || -> Result<(), SourceError> {
                let metadata = std::fs::metadata(&source)?;
                if metadata.len() > max {
                    return Err(SourceError::TooLarge(max));
                }
                let mut src_file = std::fs::File::open(&source)?;
                let mut dst_file = std::fs::File::create(&dest)?;
                std::io::copy(&mut src_file, &mut dst_file)?;
                Ok(())
            })
            .await
            .map_err(|error| SourceError::Io(error.to_string()))?;
        }

        use tokio::io::AsyncWriteExt;

        if self.insecure_url(&release.asset.url) {
            return Err(SourceError::Network(
                "refusing to download over plain http".to_string(),
            ));
        }

        let response = self
            .download
            .get(&release.asset.url)
            .send()
            .await
            .map_err(|error| SourceError::Network(error.to_string()))?;
        let mut response = response
            .error_for_status()
            .map_err(|error| SourceError::Network(error.to_string()))?;
        let content_length = response.content_length();
        if let Some(len) = content_length {
            if len > self.max_download {
                return Err(SourceError::TooLarge(self.max_download));
            }
        }
        let mut file = tokio::fs::File::create(part).await?;
        let mut total: u64 = 0;
        loop {
            let chunk = match tokio::time::timeout(self.chunk_timeout, response.chunk()).await {
                Ok(Ok(Some(chunk))) => chunk,
                Ok(Ok(None)) => break,
                Ok(Err(error)) => return Err(SourceError::Network(error.to_string())),
                Err(_) => return Err(SourceError::Network("the download stalled".to_string())),
            };
            total += chunk.len() as u64;
            if total > self.max_download {
                return Err(SourceError::TooLarge(self.max_download));
            }
            file.write_all(&chunk).await?;
            progress(total, content_length);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn pick_release_ue4ss_latest() {
        let release = json!({
            "tag_name": "2281fa31",
            "assets": [
                {
                    "name": "UE4SS-Palworld-g2281fa31-zDev.zip",
                    "browser_download_url": "u-dev",
                    "size": 2,
                    "updated_at": "2026-09-03T21:05:04Z"
                },
                {
                    "name": "UE4SS-Palworld-g2281fa31.zip",
                    "browser_download_url": "u",
                    "size": 1,
                    "updated_at": "2026-09-03T21:04:41Z"
                }
            ]
        });
        let picked = pick_release(FrameworkKey::Ue4ss, Channel::Latest, false, &release).unwrap();
        assert_eq!(picked.version, "2281fa31");
        assert_eq!(picked.display, "2281fa31 (03.09.2026)");
        assert_eq!(picked.asset.url, "u");

        let dev = pick_release(FrameworkKey::Ue4ss, Channel::Latest, true, &release).unwrap();
        assert_eq!(dev.version, "2281fa31-zdev");
        assert_eq!(dev.asset.url, "u-dev");
    }

    #[test]
    fn pick_release_ue4ss_experimental() {
        let release = json!({
            "tag_name": "experimental-palworld",
            "assets": [
                {
                    "name": "UE4SS-Palworld.zip",
                    "browser_download_url": "e",
                    "size": 1,
                    "updated_at": "2026-08-28T20:18:46Z"
                }
            ]
        });
        let picked =
            pick_release(FrameworkKey::Ue4ss, Channel::Experimental, false, &release).unwrap();
        assert_eq!(picked.version, "experimental-20260828");
    }

    #[test]
    fn pick_release_palschema() {
        let release = json!({
            "tag_name": "0.6.71",
            "assets": [
                {"name": "PalSchema_0.6.71_Dev.zip", "browser_download_url": "d"},
                {"name": "PalSchema_0.6.71.zip", "browser_download_url": "p"}
            ]
        });
        let picked =
            pick_release(FrameworkKey::PalSchema, Channel::Latest, false, &release).unwrap();
        assert_eq!(picked.version, "0.6.71");
        assert_eq!(picked.asset.url, "p");

        let none_release = json!({
            "tag_name": "0.6.71",
            "assets": [
                {"name": "PalSchema_0.6.71_Dev.zip", "browser_download_url": "d"}
            ]
        });
        assert!(pick_release(
            FrameworkKey::PalSchema,
            Channel::Latest,
            false,
            &none_release
        )
        .is_none());
    }

    #[test]
    fn tag_from_location_extracts_the_release_tag() {
        assert_eq!(
            tag_from_location("https://github.com/Okaetsu/PalSchema/releases/tag/0.6.71"),
            Some("0.6.71".to_string())
        );
        assert_eq!(tag_from_location("/x/releases/latest"), None);
    }

    #[test]
    fn fallback_release_builds_the_download_url() {
        let release =
            fallback_release(FrameworkKey::Ue4ss, false, "2281fa31", "https://github.com").unwrap();
        assert_eq!(release.asset.name, "UE4SS-Palworld-g2281fa31.zip");
        assert_eq!(
            release.asset.url,
            "https://github.com/Okaetsu/RE-UE4SS/releases/download/2281fa31/UE4SS-Palworld-g2281fa31.zip"
        );
    }

    #[test]
    fn bundled_amity_picks_the_newest_version() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("PSAmity-UE4SS-0.1.0.zip"), b"a").unwrap();
        std::fs::write(dir.path().join("PSAmity-UE4SS-0.2.0.zip"), b"b").unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"c").unwrap();
        let release = bundled_amity(dir.path()).unwrap();
        assert_eq!(release.version, "0.2.0");
    }

    #[test]
    fn bundled_amity_is_none_for_empty_or_missing_dir() {
        let dir = tempfile::tempdir().unwrap();
        assert!(bundled_amity(dir.path()).is_none());
        assert!(bundled_amity(&dir.path().join("nope")).is_none());
    }

    async fn spawn(router: axum::Router) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn latest_and_fetch_ue4ss_caches_and_downloads() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let hits_for_route = hits.clone();
        let download_url = format!("http://{addr}/dl/UE4SS-Palworld-g2281fa31.zip");
        let router = axum::Router::new()
            .route(
                "/repos/Okaetsu/RE-UE4SS/releases/latest",
                axum::routing::get(move || {
                    let hits = hits_for_route.clone();
                    let download_url = download_url.clone();
                    async move {
                        hits.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        axum::Json(json!({
                            "tag_name": "2281fa31",
                            "assets": [{
                                "name": "UE4SS-Palworld-g2281fa31.zip",
                                "browser_download_url": download_url,
                                "size": 9,
                                "updated_at": "2026-09-03T21:04:41Z"
                            }]
                        }))
                    }
                }),
            )
            .route(
                "/dl/UE4SS-Palworld-g2281fa31.zip",
                axum::routing::get(|| async { b"zip-bytes".to_vec() }),
            );
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let base = format!("http://{addr}");

        let sources = ReleaseSources::new(&base, &base, None);
        let release = sources
            .latest(FrameworkKey::Ue4ss, Channel::Latest, false)
            .await
            .unwrap();
        assert_eq!(release.version, "2281fa31");

        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("out.zip");
        let progress_calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let calls = progress_calls.clone();
        let progress: Progress = &move |total, len| {
            calls.lock().unwrap().push((total, len));
        };
        sources.fetch(&release, &dest, progress).await.unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"zip-bytes");
        assert!(!dest.with_file_name("out.zip.part").exists());
        assert!(progress_calls
            .lock()
            .unwrap()
            .iter()
            .any(|(total, _)| *total == 9));

        sources
            .latest(FrameworkKey::Ue4ss, Channel::Latest, false)
            .await
            .unwrap();
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn palschema_falls_back_through_web_redirect_on_rate_limit() {
        let base = spawn(
            axum::Router::new()
                .route(
                    "/repos/Okaetsu/PalSchema/releases/latest",
                    axum::routing::get(|| async {
                        (
                            axum::http::StatusCode::FORBIDDEN,
                            [("x-ratelimit-remaining", "0")],
                            "rate limited",
                        )
                    }),
                )
                .route(
                    "/Okaetsu/PalSchema/releases/latest",
                    axum::routing::get(|| async {
                        (
                            axum::http::StatusCode::FOUND,
                            [(
                                axum::http::header::LOCATION,
                                "/Okaetsu/PalSchema/releases/tag/0.6.71",
                            )],
                        )
                    }),
                ),
        )
        .await;

        let sources = ReleaseSources::new(&base, &base, None);
        let release = sources
            .latest(FrameworkKey::PalSchema, Channel::Latest, false)
            .await
            .unwrap();
        assert_eq!(release.version, "0.6.71");
        assert_eq!(
            release.asset.url,
            format!("{base}/Okaetsu/PalSchema/releases/download/0.6.71/PalSchema_0.6.71.zip")
        );
    }

    #[tokio::test]
    async fn experimental_channel_has_no_fallback_on_rate_limit() {
        let base = spawn(axum::Router::new().route(
            "/repos/Okaetsu/RE-UE4SS/releases/tags/experimental-palworld",
            axum::routing::get(|| async { axum::http::StatusCode::TOO_MANY_REQUESTS }),
        ))
        .await;

        let sources = ReleaseSources::new(&base, &base, None);
        let error = sources
            .latest(FrameworkKey::Ue4ss, Channel::Experimental, false)
            .await
            .unwrap_err();
        assert!(matches!(error, SourceError::RateLimited));
    }

    #[tokio::test]
    async fn fetch_refuses_downloads_over_the_cap() {
        let base = spawn(axum::Router::new().route(
            "/dl/big.zip",
            axum::routing::get(|| async { vec![0u8; 16] }),
        ))
        .await;

        let sources = ReleaseSources::new(&base, &base, None).with_max_download(4);
        let release = Release {
            key: FrameworkKey::Ue4ss,
            tag: "t".to_string(),
            version: "t".to_string(),
            display: "t".to_string(),
            asset: ReleaseAsset {
                name: "big.zip".to_string(),
                url: format!("{base}/dl/big.zip"),
                size: None,
            },
            origin: "github",
            repo: Some(UE4SS_REPO),
        };
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("big.zip");
        let progress: Progress = &|_, _| {};
        let error = sources.fetch(&release, &dest, progress).await.unwrap_err();
        assert!(matches!(error, SourceError::TooLarge(4)));
        assert!(!dest.exists());
        assert!(!dest.with_file_name("big.zip.part").exists());
    }

    #[tokio::test]
    async fn amity_resolves_and_fetches_from_the_bundle_dir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("PSAmity-UE4SS-0.2.0.zip"), b"amity-bytes").unwrap();
        let sources = ReleaseSources::new(
            "http://localhost",
            "http://localhost",
            Some(dir.path().to_path_buf()),
        );
        let release = sources
            .latest(FrameworkKey::Amity, Channel::Latest, false)
            .await
            .unwrap();
        assert_eq!(release.version, "0.2.0");
        assert_eq!(release.origin, "bundled");

        let out_dir = tempfile::tempdir().unwrap();
        let dest = out_dir.path().join("out.zip");
        let progress: Progress = &|_, _| {};
        sources.fetch(&release, &dest, progress).await.unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"amity-bytes");
    }

    #[tokio::test]
    async fn a_stalled_chunk_times_out_and_leaves_no_part_file() {
        let base = spawn(axum::Router::new().route(
            "/dl/stall.zip",
            axum::routing::get(|| async {
                let body_stream = futures_util::stream::unfold(0u8, |state| async move {
                    if state == 0 {
                        Some((
                            Ok::<_, std::io::Error>(axum::body::Bytes::from_static(b"partial")),
                            1,
                        ))
                    } else {
                        std::future::pending().await
                    }
                });
                axum::body::Body::from_stream(body_stream)
            }),
        ))
        .await;

        let sources = ReleaseSources::new(&base, &base, None)
            .with_chunk_timeout(std::time::Duration::from_millis(200));
        let release = Release {
            key: FrameworkKey::Ue4ss,
            tag: "t".to_string(),
            version: "t".to_string(),
            display: "t".to_string(),
            asset: ReleaseAsset {
                name: "stall.zip".to_string(),
                url: format!("{base}/dl/stall.zip"),
                size: None,
            },
            origin: "github",
            repo: Some(UE4SS_REPO),
        };
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("stall.zip");
        let progress: Progress = &|_, _| {};
        let error = sources.fetch(&release, &dest, progress).await.unwrap_err();
        assert!(matches!(error, SourceError::Network(_)), "{error:?}");
        assert!(!dest.exists());
        assert!(!dest.with_file_name("stall.zip.part").exists());
    }

    #[tokio::test]
    async fn a_plain_http_download_url_is_refused_when_the_web_base_is_https() {
        let sources = ReleaseSources::github(None);
        let release = Release {
            key: FrameworkKey::Ue4ss,
            tag: "t".to_string(),
            version: "t".to_string(),
            display: "t".to_string(),
            asset: ReleaseAsset {
                name: "x.zip".to_string(),
                url: "http://evil.example/x.zip".to_string(),
                size: None,
            },
            origin: "github",
            repo: Some(UE4SS_REPO),
        };
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("x.zip");
        let progress: Progress = &|_, _| {};
        let error = sources.fetch(&release, &dest, progress).await.unwrap_err();
        assert!(matches!(error, SourceError::Network(_)), "{error:?}");
        assert!(!dest.exists());
    }

    #[tokio::test]
    async fn fetch_removes_the_part_file_when_the_final_rename_fails() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("PSAmity-UE4SS-0.2.0.zip"), b"amity-bytes").unwrap();
        let sources = ReleaseSources::new(
            "http://localhost",
            "http://localhost",
            Some(dir.path().to_path_buf()),
        );
        let release = sources
            .latest(FrameworkKey::Amity, Channel::Latest, false)
            .await
            .unwrap();

        let out_dir = tempfile::tempdir().unwrap();
        let dest = out_dir.path().join("out.zip");
        // A non-empty existing directory at `dest` makes the final rename fail
        // regardless of platform, without touching the pre-rename `fetch_into` step.
        std::fs::create_dir(&dest).unwrap();
        std::fs::write(dest.join("occupied.txt"), b"taken").unwrap();

        let progress: Progress = &|_, _| {};
        let error = sources.fetch(&release, &dest, progress).await.unwrap_err();
        assert!(matches!(error, SourceError::Io(_)));
        assert!(!dest.with_file_name("out.zip.part").exists());
    }
}
