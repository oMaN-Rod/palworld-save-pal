//! Dedicated-server REST client for Signal.
//!
//! Palworld dedicated servers expose plain HTTP under `/v1/api/...` and
//! authenticate with basic auth whose user is always the literal `admin` and
//! whose password is the server's AdminPassword. Signal uses two endpoints:
//!
//! * `GET /v1/api/players` — the players-only feed (capability probe)
//! * `GET /v1/api/game-data` — the rich world feed (`ActorData` array)
//!
//! Timeouts: 5s for the probe, 8s for the rich feed.
use std::time::Duration;

use serde_json::Value;

pub const PLAYERS_TIMEOUT: Duration = Duration::from_secs(5);
pub const GAME_DATA_TIMEOUT: Duration = Duration::from_secs(8);

/// The classification a read walks away with. `Err` is reserved for caller
/// bugs; everything the server can do wrong is a variant.
#[derive(Debug, Clone, PartialEq)]
pub enum RestRead {
    /// 2xx with a parsed body.
    Ok(Value),
    /// 401 — wrong or missing AdminPassword.
    Unauthorized,
    /// 404/400 — the server predates the game-data endpoint.
    NotSupported,
    /// Any other non-2xx status.
    Status(u16),
    /// Transport failure: connect refused, DNS, timeout, TLS.
    Transport,
    /// 2xx but torn, empty, or unparseable — "a hiccup, never a verdict".
    Torn,
}

#[derive(Debug, Clone)]
pub struct SignalRestClient {
    http: reqwest::Client,
}

impl Default for SignalRestClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Normalizes a user-supplied base into something the client can call:
/// trims trailing slashes, gives bare `host:port` inputs the `http://` they
/// obviously meant, and refuses everything that is not an http(s) address.
pub fn normalize_base(base: &str) -> Option<String> {
    let trimmed = base.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    if let Some(rest) = trimmed
        .strip_prefix("http://")
        .or_else(|| trimmed.strip_prefix("https://"))
    {
        return if rest.is_empty() { None } else { Some(trimmed.to_string()) };
    }
    // A bare host:port has no scheme separator and exactly one colon.
    let looks_like_host_port = !trimmed.contains("://")
        && trimmed.matches(':').count() <= 1
        && !trimmed.contains('/')
        && !trimmed.chars().any(char::is_whitespace)
        // When a colon is present the tail must be a real port, so scheme
        // look-alikes ("javascript:alert(1)") are refused, not guessed.
        && trimmed
            .rsplit_once(':')
            .map(|(_, port)| port.is_empty() || port.parse::<u16>().is_ok())
            .unwrap_or(true);
    if looks_like_host_port {
        Some(format!("http://{trimmed}"))
    } else {
        None
    }
}

impl SignalRestClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    async fn get(&self, url: &str, password: &str, timeout: Duration) -> RestRead {
        let response = match tokio::time::timeout(
            timeout,
            self.http
                .get(url)
                .basic_auth("admin", Some(password))
                .header("Accept", "application/json")
                .send(),
        )
        .await
        {
            Ok(Ok(response)) => response,
            Ok(Err(_)) | Err(_) => return RestRead::Transport,
        };
        match response.status().as_u16() {
            200..=299 => {}
            401 => return RestRead::Unauthorized,
            404 | 400 => return RestRead::NotSupported,
            status => return RestRead::Status(status),
        }
        let body = match response.text().await {
            Ok(body) => body,
            Err(_) => return RestRead::Torn,
        };
        let trimmed = body.trim();
        if trimmed.is_empty() {
            return RestRead::Torn;
        }
        match serde_json::from_str::<Value>(trimmed) {
            Ok(value) => RestRead::Ok(value),
            Err(_) => RestRead::Torn,
        }
    }

    /// `GET {base}/v1/api/players`
    pub async fn players(&self, base: &str, password: &str) -> RestRead {
        self.get(&format!("{base}/v1/api/players"), password, PLAYERS_TIMEOUT)
            .await
    }

    /// `GET {base}/v1/api/game-data`
    pub async fn game_data(&self, base: &str, password: &str) -> RestRead {
        self.get(
            &format!("{base}/v1/api/game-data"),
            password,
            GAME_DATA_TIMEOUT,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_base_trims_slashes_and_keeps_schemes() {
        assert_eq!(
            normalize_base("http://pal.example:8212/"),
            Some("http://pal.example:8212".to_string())
        );
        assert_eq!(
            normalize_base("https://pal.example/palserver"),
            Some("https://pal.example/palserver".to_string())
        );
    }

    #[test]
    fn normalize_base_gives_bare_host_port_the_obvious_scheme() {
        assert_eq!(
            normalize_base("pal.example:8212"),
            Some("http://pal.example:8212".to_string())
        );
        assert_eq!(
            normalize_base("127.0.0.1:8212"),
            Some("http://127.0.0.1:8212".to_string())
        );
    }

    #[test]
    fn normalize_base_refuses_non_http_addresses() {
        assert_eq!(normalize_base(""), None);
        assert_eq!(normalize_base("   "), None);
        assert_eq!(normalize_base("ftp://pal.example"), None);
        assert_eq!(normalize_base("not a url"), None);
        assert_eq!(normalize_base("javascript:alert(1)"), None);
    }
}
