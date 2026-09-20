//! The persisted network policy for PalStudio itself (not the Palworld game
//! servers the tool manages). Everything here is plain data so it can be
//! stored as JSON in the `meta` table, seeded from install-time env vars, and
//! edited from the in-app Network page.
//!
//! Enforcement model (Sunshine-inspired, strict by default):
//! - The listener binds broadly, but **listen mode shapes the default
//!   audience enforced per peer**: `Localhost` rejects everything
//!   non-loopback, `Lan` defaults to private ranges, `Tailscale` defaults to
//!   the tailnet's CGNAT range (100.64.0.0/10), `Wan` defaults to any peer —
//!   modes never require a socket rebind.
//! - Loopback is always allowed to connect AND write; it is the trusted
//!   operator seat (the desktop app and the launcher run there).
//! - `allow.connect` decides who may talk to us at all; `allow.write`
//!   decides who may mutate saves/settings (reads stay available). A
//!   LISTED address is admitted regardless of the listen mode's default
//!   audience — the operator typed it — while an EMPTY list falls back to
//!   that audience as selected by `AllowMode`.
//! - `auth` gates non-loopback (or, if the user insists, all) peers behind
//!   a PIN; sessions are short-lived in-memory tokens issued by the server.

use serde::{Deserialize, Serialize};

pub const MIN_SESSION_TTL_SECS: u64 = 60;
pub const MAX_SESSION_TTL_SECS: u64 = 30 * 24 * 60 * 60;
pub const MIN_PIN_CHARS: usize = 4;
pub const MAX_PIN_CHARS: usize = 128;

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
        let Some(expected) = crate::auth::unhex(&self.hash) else {
            return false;
        };
        if salt.len() != 16
            || expected.len() != 32
            || !(crate::auth::MIN_PIN_ITERATIONS..=crate::auth::MAX_PIN_ITERATIONS)
                .contains(&self.iterations)
        {
            return false;
        }
        let candidate = crate::auth::pbkdf2_hmac_sha256(pin.as_bytes(), &salt, self.iterations);
        crate::auth::constant_time_eq(&candidate, &expected)
    }
}

/// CIDR allowlists. A listed address is admitted regardless of the listen
/// mode's default audience (except under `Localhost`, whose listener is
/// loopback-only anyway); an empty list falls back to that audience as
/// shaped by `AllowMode`. The fields always serialize — the Network page's
/// DTO renders them unconditionally, and a missing key there reads as
/// `undefined` in the browser.
/// What an EMPTY allowlist grants. The mode only decides the fallback when a
/// list is empty; a non-empty list always means "exactly these addresses",
/// and loopback is always trusted regardless.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AllowMode {
    /// Empty connect list: anyone the listen mode admits may view AND edit.
    Open,
    /// The legacy default: anyone admitted may view; edits require listing.
    #[default]
    Balanced,
    /// Empty lists admit nobody but loopback — addresses must be listed both
    /// to connect and to edit.
    Strict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AllowRules {
    #[serde(default)]
    pub connect: Vec<String>,
    #[serde(default)]
    pub write: Vec<String>,
    #[serde(default)]
    pub mode: AllowMode,
}

/// How remote (non-loopback) peers may load the app and its assets. The
/// local operator seat — a DIRECT loopback connection — is exempt from every
/// variant; a Funnel-forwarded loopback socket is judged as its remote
/// client, not as the operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AssetTransport {
    /// HTTPS only (default): cleartext requests from non-loopback peers are
    /// refused. Serve HTTPS natively, publish via Tailscale Funnel, or front
    /// the port with an HTTPS proxy.
    #[default]
    Https,
    /// HTTPS or HTTP: cleartext is accepted for remote peers too. Saving
    /// this answers with a security warning — saves, settings, and the PIN
    /// travel unencrypted on the wire.
    HttpsHttp,
    /// Loopback only: remote peers are refused regardless of transport;
    /// asset streaming is limited to this machine.
    Loopback,
}

impl AssetTransport {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "https" => Some(AssetTransport::Https),
            "https-http" | "http" | "https_http" => Some(AssetTransport::HttpsHttp),
            "loopback" => Some(AssetTransport::Loopback),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            AssetTransport::Https => "https",
            AssetTransport::HttpsHttp => "https-http",
            AssetTransport::Loopback => "loopback",
        }
    }
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
    /// Serve the port over HTTPS with a self-signed certificate generated
    /// (once) beside the database. Mutually exclusive with Funnel, which
    /// forwards to this port over plain HTTP.
    #[serde(default)]
    pub https_enabled: bool,
    /// How remote peers may load the app and its assets (HTTPS by default).
    #[serde(default)]
    pub asset_transport: AssetTransport,
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
            https_enabled: false,
            asset_transport: AssetTransport::Https,
        }
    }
}

impl NetworkConfig {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("NetworkConfig serializes")
    }

    pub fn from_json(raw: &str) -> Result<Self, ConfigError> {
        let config: Self = serde_json::from_str(raw).map_err(ConfigError::Parse)?;
        config.validate()?;
        Ok(config)
    }

    /// Parses without validating. Loading a stored config needs this so a
    /// row saved by an older build can be normalized before the current
    /// invariants would reject it.
    pub fn from_json_lenient(raw: &str) -> Result<Self, ConfigError> {
        serde_json::from_str(raw).map_err(ConfigError::Parse)
    }

    /// Adjusts a stored config so it satisfies the current invariants,
    /// always failing toward less exposure. Returns one human-readable note
    /// per adjustment; an empty result means nothing changed.
    pub fn normalize_legacy(&mut self) -> Vec<String> {
        let mut notes = Vec::new();
        if self.funnel_enabled && self.auth.scope != AuthScope::Always {
            self.funnel_enabled = false;
            notes.push(
                "Tailscale Funnel disabled: it now requires authentication scope 'always'".into(),
            );
        }
        if self.auth.scope != AuthScope::Never && self.auth.pin.is_none() {
            self.auth.scope = AuthScope::Never;
            notes.push("authentication disabled: no PIN was configured".into());
        }
        if self.https_enabled && self.funnel_enabled {
            // Fail toward less exposure: Funnel off keeps the port private
            // while native HTTPS keeps it encrypted.
            self.funnel_enabled = false;
            notes.push(
                "Tailscale Funnel disabled: native HTTPS and Funnel are mutually exclusive".into(),
            );
        }
        notes
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.port == 0 {
            return Err(ConfigError::Invalid(
                "port must be between 1 and 65535".into(),
            ));
        }
        if !(MIN_SESSION_TTL_SECS..=MAX_SESSION_TTL_SECS).contains(&self.auth.session_ttl_secs) {
            return Err(ConfigError::Invalid(format!(
                "session_ttl_secs must be between {MIN_SESSION_TTL_SECS} and {MAX_SESSION_TTL_SECS}"
            )));
        }
        for (name, rules) in [
            ("allow.connect", &self.allow.connect),
            ("allow.write", &self.allow.write),
        ] {
            if rules.len() > 256 {
                return Err(ConfigError::Invalid(format!(
                    "{name} contains too many entries"
                )));
            }
            for rule in rules {
                if rule.len() > 64 || rule.chars().any(char::is_control) {
                    return Err(ConfigError::Invalid(format!(
                        "{name} contains an invalid entry"
                    )));
                }
                if crate::policy::IpNet::parse(rule).is_none() {
                    return Err(ConfigError::Invalid(format!(
                        "{name} contains an invalid IP or CIDR entry"
                    )));
                }
            }
        }
        if let Some(pin) = &self.auth.pin {
            let salt = crate::auth::unhex(&pin.salt);
            let hash = crate::auth::unhex(&pin.hash);
            if salt.as_ref().is_none_or(|value| value.len() != 16)
                || hash.as_ref().is_none_or(|value| value.len() != 32)
                || !(crate::auth::MIN_PIN_ITERATIONS..=crate::auth::MAX_PIN_ITERATIONS)
                    .contains(&pin.iterations)
            {
                return Err(ConfigError::Invalid(
                    "PIN hash parameters are invalid".into(),
                ));
            }
        }
        if self.auth.scope != AuthScope::Never && self.auth.pin.is_none() {
            return Err(ConfigError::Invalid(
                "a PIN is required when authentication is enabled".into(),
            ));
        }
        if self.funnel_enabled && self.auth.scope != AuthScope::Always {
            return Err(ConfigError::Invalid(
                "Tailscale Funnel requires AuthScope::Always".into(),
            ));
        }
        if self.https_enabled && self.funnel_enabled {
            return Err(ConfigError::Invalid(
                "native HTTPS and Tailscale Funnel cannot both be enabled — Funnel forwards \
                 to this port over plain HTTP"
                    .into(),
            ));
        }
        Ok(())
    }

    /// Merges install-time env overrides (`PS_LISTEN`, `PS_PORT`, `PS_PIN`)
    /// on top of this config. Used only when the operator asked for them —
    /// i.e. the install script wrote them into the service definition or a
    /// container environment.
    pub fn apply_env(self) -> Result<Self, ConfigError> {
        let mut config = self;
        if let Ok(mode) = std::env::var("PS_LISTEN") {
            config.listen = ListenMode::parse(&mode).ok_or_else(|| {
                ConfigError::Invalid("PS_LISTEN must be localhost, lan, tailscale, or wan".into())
            })?;
        }
        if let Ok(port) = std::env::var("PS_PORT") {
            config.port = port
                .trim()
                .parse::<u16>()
                .map_err(|_| ConfigError::Invalid("PS_PORT must be between 1 and 65535".into()))?;
        }
        if let Ok(pin) = std::env::var("PS_PIN") {
            if pin.is_empty() {
                config.auth.pin = None;
            } else {
                validate_pin(&pin)?;
                config.auth.pin = Some(PinHash::generate(&pin));
                if config.auth.scope == AuthScope::Never && config.listen != ListenMode::Localhost
                {
                    config.auth.scope = AuthScope::NetworkOnly;
                }
            }
        }
        if let Ok(flag) = std::env::var("PS_HTTPS") {
            config.https_enabled = flag.trim().eq_ignore_ascii_case("1")
                || flag.trim().eq_ignore_ascii_case("true");
            if config.https_enabled {
                // An operator asking for HTTPS via env cannot also keep
                // Funnel pointed at the now-TLS port.
                config.funnel_enabled = false;
            }
        }
        if let Ok(transport) = std::env::var("PS_ASSET_TRANSPORT") {
            config.asset_transport = AssetTransport::parse(&transport).ok_or_else(|| {
                ConfigError::Invalid(
                    "PS_ASSET_TRANSPORT must be https, https-http, or loopback".into(),
                )
            })?;
        }
        config.validate()?;
        Ok(config)
    }
}

pub fn validate_pin(pin: &str) -> Result<(), ConfigError> {
    let length = pin.chars().count();
    if !(MIN_PIN_CHARS..=MAX_PIN_CHARS).contains(&length) {
        return Err(ConfigError::Invalid(format!(
            "PIN length must be between {MIN_PIN_CHARS} and {MAX_PIN_CHARS} characters"
        )));
    }
    if pin.chars().any(char::is_control) {
        return Err(ConfigError::Invalid(
            "PIN must not contain control characters".into(),
        ));
    }
    Ok(())
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
            https_enabled: false,
            asset_transport: AssetTransport::Https,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("could not parse network config JSON: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("invalid network config: {0}")]
    Invalid(String),
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
        assert!(!config.https_enabled);
        assert_eq!(config.asset_transport, AssetTransport::Https);
    }

    #[test]
    fn native_https_and_funnel_are_mutually_exclusive() {
        let mut config = NetworkConfig::default();
        config.listen = ListenMode::Lan;
        config.auth.scope = AuthScope::Always;
        config.auth.pin = Some(PinHash::generate("1234"));
        config.funnel_enabled = true;
        config.https_enabled = true;
        assert!(config.validate().is_err());

        // Normalization disables the exposure (Funnel), keeping encryption.
        let mut notes = config.normalize_legacy();
        assert!(!config.funnel_enabled);
        assert!(config.https_enabled);
        assert!(notes.pop().unwrap().contains("mutually exclusive"));
    }

    #[test]
    fn asset_transport_round_trips_and_defaults_for_older_stored_rows() {
        let config = NetworkConfig {
            listen: ListenMode::Lan,
            asset_transport: AssetTransport::HttpsHttp,
            ..NetworkConfig::default()
        };
        let parsed = NetworkConfig::from_json(&config.to_json()).unwrap();
        assert_eq!(parsed.asset_transport, AssetTransport::HttpsHttp);
        assert_eq!(AssetTransport::parse("loopback"), Some(AssetTransport::Loopback));

        // A row saved before the field existed keeps the HTTPS default.
        let legacy = NetworkConfig::from_json(r#"{"listen":"lan","port":9000}"#).unwrap();
        assert_eq!(legacy.asset_transport, AssetTransport::Https);
        assert!(!legacy.https_enabled);
    }

    #[test]
    fn json_round_trips_and_tolerates_missing_fields() {
        let config = NetworkConfig {
            listen: ListenMode::Tailscale,
            allow: AllowRules {
                connect: vec!["100.64.1.5/32".into()],
                write: vec![],
                ..Default::default()
            },
            ..NetworkConfig::default()
        };
        let parsed = NetworkConfig::from_json(&config.to_json()).unwrap();
        assert_eq!(parsed, config);

        // A minimal hand-written doc (install script, docker env) parses.
        let minimal = NetworkConfig::from_json(r#"{"listen":"lan","port":9000}"#).unwrap();
        assert_eq!(minimal.listen, ListenMode::Lan);
        assert_eq!(minimal.port, 9000);

        // Configs saved before the allowlist mode existed keep their exact
        // legacy semantics: view-all, edit-nobody-unless-listed.
        let legacy = NetworkConfig::from_json(
            r#"{"listen":"lan","port":9000,"allow":{"connect":[],"write":[]}}"#,
        )
        .unwrap();
        assert_eq!(legacy.allow.mode, AllowMode::Balanced);
        assert_eq!(minimal.auth.scope, AuthScope::Never);
    }

    #[test]
    fn env_overrides_apply_and_pin_upgrades_scope() {
        let config = NetworkConfig::default();
        std::env::set_var("PS_LISTEN", "lan");
        std::env::set_var("PS_PORT", "9100");
        std::env::set_var("PS_PIN", "1234");
        let merged = config.apply_env().unwrap();
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
                ..Default::default()
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
    fn legacy_funnel_with_auth_off_is_normalized_not_rejected() {
        // Saved by builds before Funnel required AuthScope::Always: Funnel
        // on with authentication off (a leftover PIN hash is harmless).
        let legacy = NetworkConfig {
            listen: ListenMode::Localhost,
            port: 5174,
            auth: AuthConfig {
                scope: AuthScope::Never,
                pin: Some(PinHash::generate("1234")),
                session_ttl_secs: NetworkConfig::default().auth.session_ttl_secs,
            },
            funnel_enabled: true,
            ..NetworkConfig::default()
        };
        let raw = legacy.to_json();
        // The current invariants reject that row on a strict parse…
        assert!(NetworkConfig::from_json(&raw).is_err());
        // …so loading normalizes it toward less exposure instead of failing.
        let mut loaded = NetworkConfig::from_json_lenient(&raw).unwrap();
        let notes = loaded.normalize_legacy();
        assert!(loaded.validate().is_ok());
        assert!(!loaded.funnel_enabled);
        assert_eq!(notes.len(), 1);
        assert!(notes[0].to_lowercase().contains("funnel"));
    }

    #[test]
    fn legacy_auth_without_pin_normalizes_to_auth_off() {
        let legacy = NetworkConfig {
            auth: AuthConfig {
                scope: AuthScope::NetworkOnly,
                pin: None,
                session_ttl_secs: NetworkConfig::default().auth.session_ttl_secs,
            },
            ..NetworkConfig::default()
        };
        let raw = legacy.to_json();
        assert!(NetworkConfig::from_json(&raw).is_err());
        let mut loaded = NetworkConfig::from_json_lenient(&raw).unwrap();
        let notes = loaded.normalize_legacy();
        assert!(loaded.validate().is_ok());
        assert_eq!(loaded.auth.scope, AuthScope::Never);
        assert_eq!(notes.len(), 1);
    }

    #[test]
    fn current_configs_pass_through_normalization_untouched() {
        let mut config = NetworkConfig::default();
        assert!(config.normalize_legacy().is_empty());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn pin_hash_verifies_only_the_right_pin() {
        let hash = PinHash::generate("4321");
        assert!(hash.matches("4321"));
        assert!(!hash.matches("4322"));
        assert!(!hash.matches(""));
    }
}
