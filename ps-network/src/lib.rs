//! PalStudio's own network policy crate.
//!
//! Distinct from the Palworld *game server* management the tool offers:
//! this crate is about who may talk to the PalStudio HTTP/WS server itself —
//! listen modes, IP allowlists (connect vs write), PIN sessions, tailscale
//! detection and Funnel toggling, and (optionally, behind the `upnp`
//! feature) UPnP port mapping.
//!
//! Layering: `config` is the persisted policy (JSON in the meta table),
//! `policy::evaluate` turns (config, peer IP) into a `PeerAcl` verdict, and
//! `auth` issues/validates the PIN sessions that satisfy `auth_required`.
//! The server crate wires these into axum middleware and the WS dispatcher.
pub mod auth;
pub mod config;
pub mod policy;
pub mod tailscale;
#[cfg(feature = "upnp")]
pub mod upnp;

pub use config::{
    validate_pin, AllowMode, AllowRules, AuthConfig, AuthScope, ConfigError, ListenMode,
    NetworkConfig, NetworkTier, PinHash, MAX_PIN_CHARS, MAX_SESSION_TTL_SECS, MIN_PIN_CHARS,
    MIN_SESSION_TTL_SECS,
};
pub use policy::{canonical, classify, default_audience, evaluate, PeerAcl, PeerClass};
