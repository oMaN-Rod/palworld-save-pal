//! The persisted network policy for PalStudio itself (not the Palworld game
//! servers the tool manages). Everything here is plain data so it can be
//! stored as JSON in the `meta` table, seeded from install-time env vars, and
//! edited from the in-app Network page.
//!
//! Enforcement model (Sunshine-inspired, strict by default):
//! - The listener binds broadly, but **listen mode is enforced per peer**:
//!   `Localhost` rejects everything non-loopback, `Lan` rejects public peers,
//!   `Tailscale` only accepts loopback + the tailnet's CGNAT range (and,
//!   when detected, the machine's own tailscale IPs' peers), `Wan` accepts
//!   any peer — modes never require a socket rebind.
//! - Loopback is always allowed to connect AND write; it is the trusted
//!   operator seat (the desktop app and the launcher run there).
//! - `allow.connect` narrows who may talk to us at all; `allow.write`
//!   narrows who may mutate saves/settings (reads stay available).
//! - `auth` gates non-loopback (or, if the user insists, all) peers behind
//!   a PIN; sessions are short-lived in-memory tokens issued by the server.

use serde::{Deserialize, Serialize};

/// Where PalStudio accepts connections from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ListenMode {
    /// Loopback only (default; what a local tool should be).
    #[default]
    Localhost,
    /// Loopback + RFC1918/ULA private ranges.
    Lan,
    /// Loopback + tailnet CGNAT range (100.64.0.0/10) — reachable only via
    /// the tailnet, never from the raw internet.
    Tailscale,
    /// Any peer; pair with allowlists + PIN or a reverse proxy.
    Wan,
}

impl ListenMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "localhost" => Some(ListenMode::Localhost),
            "lan" => Some(ListenMode::Lan),
            "tailscale" => Some(ListenMode::Tailscale),
            "wan" => Some(ListenMode::Wan),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ListenMode::Localhost => "localhost",
            ListenMode::Lan => "lan",
            ListenMode::Tailscale => "tailscale",
            ListenMode::Wan => "wan",
        }
    }
}

/// When the PIN is demanded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AuthScope {
    /// No PIN anywhere (only sensible with localhost listen mode).
    #[default]
    Never,
    /// PIN for every non-loopback peer — the recommended posture when
    /// listening on lan/wan/tailscale.
    NetworkOnly,
    /// PIN even on loopback (kiosk-style paranoia).
    Always,
}

impl AuthScope {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "never" | "off" | "none" => Some(AuthScope::Never),
            "network" | "network_only" | "networkonly" => Some(AuthScope::NetworkOnly),
            "always" => Some(AuthScope::Always),
            _ => None,
        }
    }
}

/// Salted PBKDF2-HMAC-SHA256 parameters + verifier. The PIN itself is never
/// stored; `verify` runs the same KDF over the candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinHash {
    /// Hex-encoded 16-byte salt.
    pub salt: String,
    /// Hex-encoded derived key.
    pub hash: String,
    pub iterations: u32,
}

impl PinHash {
    pub fn generate(pin: &str) -> Self {
        use rand::RngCore;
        let mut salt = [0u8; 16];
        rand::rng().fill_bytes(&mut salt);
        let iterations = crate::auth::PIN_ITERATIONS;
        let hash = crate::auth::pbkdf2_hmac_sha256(pin.as_bytes(), &salt, iterations);
        PinHash {
            salt: crate::auth::hex(&salt),
            hash: crate::auth::hex(&hash),
            iterations,
        }
    }

    pub fn matches(&self, pin: &str) -> bool {
        let Some(salt) = crate::auth::unhex(&self.salt) else {
            return false;
        };
        let candidate = crate::auth::pbkdf2_hmac_sha256(pin.as_bytes(), &salt, self.iterations);
        crate::auth::constant_time_eq(
            &candidate,
            &crate::auth::unhex(&self.hash).unwrap_or_default(),
        )
    }
}

/// CIDR allowlists. Empty `connect` means "the listen mode's default
/// audience"; empty `write` means "anyone allowed to connect may write"
/// (loopback always can). The fields always serialize — the Network page's
/// DTO renders them unconditionally, and a missing key there reads as
/// `undefined` in the browser.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AllowRules {
    #[serde(default)]
    pub connect: Vec<String>,
    #[serde(default)]
    pub write: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthConfig {
    #[serde(default)]
    pub scope: AuthScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<PinHash>,
    /// Session lifetime in seconds (default 12h — long enough for a work
    /// session, short enough that a leaked token ages out).
    #[serde(default = "default_session_ttl")]
    pub session_ttl_secs: u64,
}

impl Default for AuthConfig {
    fn default() -> Self {
        AuthConfig {
            scope: AuthScope::Never,
            pin: None,
            session_ttl_secs: default_session_ttl(),
        }
    }
}

fn default_session_ttl() -> u64 {
    12 * 60 * 60
}

/// The whole policy. Versioned for forward-compatible migrations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkConfig {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub listen: ListenMode,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub allow: AllowRules,
    #[serde(default)]
    pub auth: AuthConfig,
    /// UPnP port mapping: opt-in, off by default, and discouraged — punching
    /// a hole in the router's firewall for a save editor is exactly the kind
    /// of thing the rest of this config tries to avoid. Tailscale is the
    /// recommended remote path.
    #[serde(default)]
    pub upnp_enabled: bool,
    /// Ask the tailscale CLI to publish the port via `tailscale funnel`
    /// when available. No-op (surfaced as unavailable) without tailscale.
    #[serde(default)]
    pub funnel_enabled: bool,
}

fn default_version() -> u32 {
    1
}

fn default_port() -> u16 {
    5174
}

impl Default for NetworkConfig {
    fn default() -> Self {
        NetworkConfig {
            version: default_version(),
            listen: ListenMode::default(),
            port: default_port(),
            allow: AllowRules::default(),
            auth: AuthConfig::default(),
            upnp_enabled: false,
            funnel_enabled: false,
        }
    }
}

impl NetworkConfig {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("NetworkConfig serializes")
    }

    pub fn from_json(raw: &str) -> Result<Self, ConfigError> {
        serde_json::from_str(raw).map_err(ConfigError::Parse)
    }

    /// Merges install-time env overrides (`PS_LISTEN`, `PS_PORT`, `PS_PIN`)
    /// on top of this config. Used only when the operator asked for them —
    /// i.e. the install script wrote them into the service definition or a
    /// container environment.
    pub fn apply_env(self) -> Self {
        let mut config = self;
        if let Ok(mode) = std::env::var("PS_LISTEN") {
            if let Some(mode) = ListenMode::parse(&mode) {
                config.listen = mode;
            }
        }
        if let Ok(port) = std::env::var("PS_PORT") {
            if let Ok(port) = port.trim().parse::<u16>() {
                config.port = port;
            }
        }
        if let Ok(pin) = std::env::var("PS_PIN") {
            let pin = pin.trim();
            if !pin.is_empty() {
                config.auth.pin = Some(PinHash::generate(pin));
                if config.auth.scope == AuthScope::Never && config.listen != ListenMode::Localhost {
                    config.auth.scope = AuthScope::NetworkOnly;
                }
            }
        }
        config
    }
}

/// Which runtime context the server is running in — decides how much of the
/// network policy is surfaced and enforced:
///
/// - `Desktop`      — the Tauri app: localhost by construction, no network
///                    settings at all.
/// - `LocalWebapp`  — a hand-launched `palstudio webapp` (AppImage, bundle,
///                    interactive picker): a local tool. Hard-clamped to
///                    localhost; only the port is editable.
/// - `Hosted`       — `palstudio serve`/`host`, background services, and
///                    containers: the full policy surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum NetworkTier {
    #[default]
    Hosted,
    #[serde(alias = "desktop")]
    Desktop,
    LocalWebapp,
}

impl NetworkTier {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "hosted" | "host" | "serve" => Some(NetworkTier::Hosted),
            "desktop" => Some(NetworkTier::Desktop),
            "local" | "webapp" | "localwebapp" | "local_webapp" => Some(NetworkTier::LocalWebapp),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            NetworkTier::Hosted => "hosted",
            NetworkTier::Desktop => "desktop",
            NetworkTier::LocalWebapp => "localwebapp",
        }
    }
}

impl NetworkConfig {
    /// The effective policy for a hand-launched local webapp: localhost
    /// only, no PIN, no exposure features, no allowlists — the port (and
    /// nothing else) survives. The stored config is NOT mutated, so a later
    /// switch to a hosted context sees the operator's original policy.
    pub fn clamped_for_local_webapp(&self) -> NetworkConfig {
        NetworkConfig {
            version: self.version,
            listen: ListenMode::Localhost,
            port: self.port,
            allow: AllowRules::default(),
            auth: AuthConfig::default(),
            upnp_enabled: false,
            funnel_enabled: false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("could not parse network config JSON: {0}")]
    Parse(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_strict() {
        let config = NetworkConfig::default();
        assert_eq!(config.listen, ListenMode::Localhost);
        assert_eq!(config.port, 5174);
        assert_eq!(config.auth.scope, AuthScope::Never);
        assert!(!config.upnp_enabled);
        assert!(!config.funnel_enabled);
    }

    #[test]
    fn json_round_trips_and_tolerates_missing_fields() {
        let config = NetworkConfig {
            listen: ListenMode::Tailscale,
            allow: AllowRules {
                connect: vec!["100.64.1.5/32".into()],
                write: vec![],
            },
            ..NetworkConfig::default()
        };
        let parsed = NetworkConfig::from_json(&config.to_json()).unwrap();
        assert_eq!(parsed, config);

        // A minimal hand-written doc (install script, docker env) parses.
        let minimal = NetworkConfig::from_json(r#"{"listen":"lan","port":9000}"#).unwrap();
        assert_eq!(minimal.listen, ListenMode::Lan);
        assert_eq!(minimal.port, 9000);
        assert_eq!(minimal.auth.scope, AuthScope::Never);
    }

    #[test]
    fn env_overrides_apply_and_pin_upgrades_scope() {
        let config = NetworkConfig::default();
        std::env::set_var("PS_LISTEN", "lan");
        std::env::set_var("PS_PORT", "9100");
        std::env::set_var("PS_PIN", "1234");
        let merged = config.apply_env();
        std::env::remove_var("PS_LISTEN");
        std::env::remove_var("PS_PORT");
        std::env::remove_var("PS_PIN");
        assert_eq!(merged.listen, ListenMode::Lan);
        assert_eq!(merged.port, 9100);
        assert_eq!(merged.auth.scope, AuthScope::NetworkOnly);
        assert!(merged.auth.pin.unwrap().matches("1234"));
    }

    #[test]
    fn listen_mode_parses_friendly_spellings() {
        assert_eq!(ListenMode::parse("Localhost"), Some(ListenMode::Localhost));
        assert_eq!(
            ListenMode::parse(" TAILSCALE "),
            Some(ListenMode::Tailscale)
        );
        assert_eq!(ListenMode::parse("vpn"), None);
        assert_eq!(
            AuthScope::parse("network_only"),
            Some(AuthScope::NetworkOnly)
        );
    }

    #[test]
    fn local_webapp_clamp_forces_localhost_and_strips_everything_but_port() {
        let stored = NetworkConfig {
            listen: ListenMode::Wan,
            port: 9000,
            allow: AllowRules {
                connect: vec!["0.0.0.0/0".into()],
                write: vec!["10.0.0.0/8".into()],
            },
            auth: AuthConfig {
                scope: AuthScope::NetworkOnly,
                pin: Some(PinHash::generate("1234")),
                session_ttl_secs: 60,
            },
            upnp_enabled: true,
            funnel_enabled: true,
            ..NetworkConfig::default()
        };
        let clamped = stored.clamped_for_local_webapp();
        assert_eq!(clamped.listen, ListenMode::Localhost);
        assert_eq!(clamped.port, 9000, "the port survives");
        assert!(clamped.allow.connect.is_empty());
        assert!(clamped.allow.write.is_empty());
        assert_eq!(clamped.auth.scope, AuthScope::Never);
        assert!(clamped.auth.pin.is_none());
        assert!(!clamped.upnp_enabled);
        assert!(!clamped.funnel_enabled);
        // The stored policy is untouched for a later hosted run.
        assert_eq!(stored.listen, ListenMode::Wan);
        assert!(stored.auth.pin.is_some());
    }

    #[test]
    fn tier_parses_friendly_spellings() {
        assert_eq!(NetworkTier::parse("webapp"), Some(NetworkTier::LocalWebapp));
        assert_eq!(NetworkTier::parse("HOSTED"), Some(NetworkTier::Hosted));
        assert_eq!(NetworkTier::parse("desktop"), Some(NetworkTier::Desktop));
        assert_eq!(NetworkTier::parse("other"), None);
        assert_eq!(NetworkTier::default(), NetworkTier::Hosted);
    }

    #[test]
    fn pin_hash_verifies_only_the_right_pin() {
        let hash = PinHash::generate("4321");
        assert!(hash.matches("4321"));
        assert!(!hash.matches("4322"));
        assert!(!hash.matches(""));
    }
}
