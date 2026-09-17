//! Transport-agnostic view of PalStudio's own network policy.
//!
//! The real implementation (`ps_network::policy`) lives in the server crate;
//! ps-app must stay dependency-free for wasm and the CI transport guard, so
//! the verdict is mirrored here as plain data and the runtime is injected as
//! a trait object. `None` on `AppState` means "no policy installed" — the
//! wasm/browser transport and unit tests get unrestricted access.
use std::net::IpAddr;
use std::sync::Arc;

/// Mirror of `ps_network::policy::PeerAcl`; duplicated rather than shared so
/// this crate needs no new dependencies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionAcl {
    pub can_connect: bool,
    pub can_write: bool,
    pub auth_required: bool,
}

impl ConnectionAcl {
    /// What a connection gets when no policy is installed (wasm transport,
    /// unit tests): everything. The server always installs a real policy.
    pub fn unrestricted() -> ConnectionAcl {
        ConnectionAcl {
            can_connect: true,
            can_write: true,
            auth_required: false,
        }
    }
}

/// Installed by ps-server; consulted at WS upgrade to stamp the connection's
/// write permission (HTTP enforcement happens in axum middleware).
pub trait NetworkPolicy: Send + Sync {
    fn acl_for(&self, peer: IpAddr) -> ConnectionAcl;
    fn has_valid_session(&self, token: &str) -> bool;
    /// Changes to listen/auth/write policy invalidate existing WebSocket
    /// connections. Implementations without a mutable policy keep generation
    /// zero for wasm and test transports.
    fn policy_generation(&self) -> u64 {
        0
    }
}

/// Convenience for call sites that only have `Option<Arc<dyn NetworkPolicy>>`.
pub fn acl_for(app_policy: &Option<Arc<dyn NetworkPolicy>>, peer: IpAddr) -> ConnectionAcl {
    app_policy
        .as_ref()
        .map(|policy| policy.acl_for(peer))
        .unwrap_or_else(ConnectionAcl::unrestricted)
}

pub fn policy_generation(app_policy: &Option<Arc<dyn NetworkPolicy>>) -> u64 {
    app_policy
        .as_ref()
        .map(|policy| policy.policy_generation())
        .unwrap_or(0)
}
