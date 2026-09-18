//! Latest Palworld version, read from the version-titled announcements Pocketpair
//! posts on the game's Steam news feed. The dedicated server app has no feed.
use std::time::{Duration, Instant};

use serde_json::Value;

const NEWS_URL: &str = "https://api.steampowered.com/ISteamNews/GetNewsForApp/v2/?appid=1623730&count=20&maxlength=1&feeds=steam_community_announcements";
const FRESH_FOR: Duration = Duration::from_secs(3600);
const RETRY_AFTER: Duration = Duration::from_secs(300);

pub struct LatestVersion {
    http: reqwest::Client,
    cached: tokio::sync::Mutex<Option<(Instant, Option<String>)>>,
}

impl Default for LatestVersion {
    fn default() -> Self {
        Self::new()
    }
}

impl LatestVersion {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("building reqwest client cannot fail with static config");
        Self {
            http,
            cached: Default::default(),
        }
    }

    /// The server listing is polled, so a failed lookup is cached too, for less time.
    pub async fn get(&self) -> Option<String> {
        let mut cached = self.cached.lock().await;
        if let Some((fetched_at, version)) = cached.as_ref() {
            let ttl = if version.is_some() {
                FRESH_FOR
            } else {
                RETRY_AFTER
            };
            if fetched_at.elapsed() < ttl {
                return version.clone();
            }
        }
        let version = self.fetch().await;
        *cached = Some((Instant::now(), version.clone()));
        version
    }

    async fn fetch(&self) -> Option<String> {
        let news: Value = self
            .http
            .get(NEWS_URL)
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .json()
            .await
            .ok()?;
        newest_announced_version(&news)
    }
}

/// Steam returns items newest first.
pub fn newest_announced_version(news: &Value) -> Option<String> {
    news.get("appnews")?
        .get("newsitems")?
        .as_array()?
        .iter()
        .filter_map(|item| item.get("title")?.as_str())
        .find_map(version_in_title)
}

fn version_in_title(title: &str) -> Option<String> {
    title
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '.'))
        .filter_map(|word| word.strip_prefix('v'))
        .map(|number| number.trim_end_matches('.'))
        .find(|number| {
            number.contains('.')
                && number
                    .split('.')
                    .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
        })
        .map(|number| format!("v{number}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_in_title_finds_the_version_token() {
        assert_eq!(
            version_in_title("v1.0.4: Balance Adjustments & Bug Fixes").as_deref(),
            Some("v1.0.4")
        );
        assert_eq!(
            version_in_title("v1.0.2.101103:Bug fixes").as_deref(),
            Some("v1.0.2.101103")
        );
        assert_eq!(
            version_in_title("Palworld v1.0 - Official Release Changelog").as_deref(),
            Some("v1.0")
        );
        assert_eq!(version_in_title("Palworld Global Popularity Poll"), None);
        assert_eq!(version_in_title("Very vivid visuals v2"), None);
    }

    #[test]
    fn newest_announced_version_skips_items_without_a_version() {
        let news = serde_json::json!({"appnews": {"newsitems": [
            {"title": "Palworld Global Popularity Poll 2026 is LIVE!"},
            {"title": "v1.0.4: Balance Adjustments & Bug Fixes"},
            {"title": "v1.0.3: Balance Adjustments & Bug Fixes"}
        ]}});
        assert_eq!(newest_announced_version(&news).as_deref(), Some("v1.0.4"));
        assert_eq!(newest_announced_version(&serde_json::json!({})), None);
    }
}
