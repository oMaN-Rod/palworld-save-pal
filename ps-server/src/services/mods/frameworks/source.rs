//! The seam between framework installation and where releases come from.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    Latest,
    Experimental,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseAsset {
    pub name: String,
    pub url: String,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub key: ps_core::mods::FrameworkKey,
    pub tag: String,
    pub version: String,
    pub display: String,
    pub asset: ReleaseAsset,
    pub origin: &'static str,
    pub repo: Option<&'static str>,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum SourceError {
    #[error("GitHub's rate limit is reached; try again later")]
    RateLimited,
    #[error("the release has no matching asset")]
    NoAsset,
    #[error("no bundled Amity package is available")]
    Unavailable,
    #[error("the download is larger than {0} bytes")]
    TooLarge(u64),
    #[error("{0}")]
    Network(String),
    #[error("{0}")]
    Io(String),
}

impl From<std::io::Error> for SourceError {
    fn from(error: std::io::Error) -> Self {
        SourceError::Io(error.to_string())
    }
}

impl SourceError {
    pub fn code(&self) -> &'static str {
        match self {
            SourceError::RateLimited => "rate_limited",
            SourceError::NoAsset => "no_release_asset",
            SourceError::Unavailable => "framework_unavailable",
            SourceError::TooLarge(_) => "download_too_large",
            SourceError::Network(_) => "network",
            SourceError::Io(_) => "io",
        }
    }
}

pub type Progress<'a> = &'a (dyn Fn(u64, Option<u64>) + Send + Sync);

#[async_trait::async_trait]
pub trait FrameworkSource: Send + Sync {
    async fn latest(
        &self,
        key: ps_core::mods::FrameworkKey,
        channel: Channel,
        dev: bool,
    ) -> Result<Release, SourceError>;

    async fn fetch(
        &self,
        release: &Release,
        dest: &std::path::Path,
        progress: Progress<'_>,
    ) -> Result<(), SourceError>;
}
