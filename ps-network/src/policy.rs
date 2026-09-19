//! Peer classification and the allow/deny decision engine.
//!
//! The listener binds broadly and every request is judged here, per peer —
//! so switching listen mode never rebinds a socket. Loopback is always the
//! trusted operator seat; everything else must pass the allowlists, whose
//! empty-list fallback is selected by the listen mode and `AllowMode`;
//! writes additionally pass the write allowlist; and a configured auth
//! scope can demand a PIN session first. An address explicitly listed in
//! an allowlist is admitted regardless of the listen mode's default
//! audience (except `Localhost`, which stays loopback-only, matching its
//! loopback-only listener).
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use crate::config::{AuthScope, ListenMode, NetworkConfig};

/// A parsed CIDR block (or single address, as /32 or /128).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IpNet {
    addr: IpAddr,
    prefix: u8,
}

impl IpNet {
    pub fn parse(s: &str) -> Option<IpNet> {
        let s = s.trim();
        let (addr, prefix): (&str, Option<u8>) = match s.split_once('/') {
            Some((addr, prefix)) => (addr, prefix.parse::<u8>().ok()),
            None => (s, None),
        };
        let addr: IpAddr = addr.parse().ok()?;
        match addr {
            IpAddr::V4(addr) => {
                let prefix = prefix.unwrap_or(32);
                if prefix > 32 {
                    return None;
                }
                Some(IpNet {
                    addr: IpAddr::V4(Ipv4Addr::from(u32::from(addr) & mask_v4(prefix))),
                    prefix,
                })
            }
            IpAddr::V6(addr) => {
                let prefix = prefix.unwrap_or(128);
                if prefix > 128 {
                    return None;
                }
                Some(IpNet {
                    addr: IpAddr::V6(Ipv6Addr::from(u128::from(addr) & mask_v6(prefix))),
                    prefix,
                })
            }
        }
    }

    pub fn contains(&self, ip: IpAddr) -> bool {
        let ip = canonical(ip);
        match (self.addr, ip) {
            (IpAddr::V4(net), IpAddr::V4(ip)) => {
                self.prefix == 0
                    || u32::from(net) >> (32 - self.prefix) == u32::from(ip) >> (32 - self.prefix)
            }
            (IpAddr::V6(net), IpAddr::V6(ip)) => {
                self.prefix == 0
                    || u128::from(net) >> (128 - self.prefix)
                        == u128::from(ip) >> (128 - self.prefix)
            }
            _ => false,
        }
    }
}

fn mask_v4(prefix: u8) -> u32 {
    if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    }
}

fn mask_v6(prefix: u8) -> u128 {
    if prefix == 0 {
        0
    } else {
        u128::MAX << (128 - prefix)
    }
}

/// v4-mapped v6 (::ffff:1.2.3.4) collapses to the v4 form so rules written
/// for one family match the other.
pub fn canonical(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => IpAddr::V4(v4),
            None => IpAddr::V6(v6),
        },
        v4 => v4,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerClass {
    Loopback,
    PrivateLan,
    /// Tailscale's CGNAT range (100.64.0.0/10), incl. the 4via6 spellings.
    Tailnet,
    LinkLocal,
    Public,
}

pub fn classify(ip: IpAddr) -> PeerClass {
    let ip = canonical(ip);
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            if o[0] == 127 {
                PeerClass::Loopback
            } else if o[0] == 169 && o[1] == 254 {
                PeerClass::LinkLocal
            } else if o[0] == 100 && (o[1] & 0b1100_0000) == 0b0100_0000 {
                PeerClass::Tailnet
            } else if o[0] == 10
                || (o[0] == 172 && (o[1] & 0xf0) == 16)
                || (o[0] == 192 && o[1] == 168)
            {
                PeerClass::PrivateLan
            } else {
                PeerClass::Public
            }
        }
        IpAddr::V6(v6) => {
            let seg = v6.segments();
            if v6.is_loopback() {
                PeerClass::Loopback
            } else if (seg[0] & 0xffc0) == 0xfe80 {
                PeerClass::LinkLocal
            } else if (seg[0] & 0xfe00) == 0xfc00 {
                PeerClass::PrivateLan
            } else {
                PeerClass::Public
            }
        }
    }
}

/// The verdict for one peer under one config. Computed per connection (HTTP
/// middleware) or once per WS upgrade, then carried with the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerAcl {
    pub can_connect: bool,
    pub can_write: bool,
    pub auth_required: bool,
}

impl PeerAcl {
    pub(crate) fn denied() -> PeerAcl {
        PeerAcl {
            can_connect: false,
            can_write: false,
            auth_required: false,
        }
    }
}

/// Evaluate a peer against the whole policy.
///
/// Fail-closed rule: if the auth scope demands a PIN (NetworkOnly/Always)
/// but no PIN is configured, non-loopback peers are refused outright rather
/// than silently admitted without the protection the operator asked for.
///
/// Allowlists widen, listen modes narrow: an address explicitly listed in
/// the connect (or write) allowlist is admitted regardless of the listen
/// mode's default audience — the operator typed it, so it connects. The
/// only exception is `Localhost`, which never admits non-loopback peers
/// (its listener is loopback-only, so a listed external address would be
/// unreachable anyway). An EMPTY list falls back to the mode-selected
/// default audience (`AllowMode`).
pub fn evaluate(config: &NetworkConfig, peer: IpAddr) -> PeerAcl {
    let peer = canonical(peer);
    let is_loopback = classify(peer) == PeerClass::Loopback;

    if is_loopback {
        let auth_required = config.auth.scope == AuthScope::Always && config.auth.pin.is_some();
        return PeerAcl {
            can_connect: true,
            can_write: true,
            auth_required,
        };
    }

    if let Some(verdict) = auth_fail_closed(config) {
        return verdict;
    }

    let listed_connect = matches_any(&config.allow.connect, peer);
    let listed_write = matches_any(&config.allow.write, peer);

    let admitted_by_listing =
        (listed_connect || listed_write) && config.listen != ListenMode::Localhost;
    let admitted_by_default = config.allow.connect.is_empty()
        && empty_connect_admits(config.allow.mode)
        && mode_admits(config.listen, peer);
    if !admitted_by_listing && !admitted_by_default {
        return PeerAcl::denied();
    }

    let can_write =
        listed_write || (config.allow.write.is_empty() && empty_write_admits(config.allow.mode));

    PeerAcl {
        can_connect: true,
        can_write,
        auth_required: auth_required(config),
    }
}

/// Evaluate a peer that reached us through a trusted local proxy (Tailscale
/// Funnel forwards arrive on loopback carrying `X-Forwarded-For`).
///
/// The proxy is an exposure the operator toggled separately from the listen
/// mode, so the mode's default audience does not apply — but the allowlists
/// absolutely do: a non-empty connect list means "exactly these addresses"
/// on this path too, which is what makes removing an IP take effect for
/// funnel clients. A proxied peer is never the trusted loopback seat, even
/// when the forwarded address claims to be loopback.
pub fn evaluate_forwarded(config: &NetworkConfig, client: IpAddr) -> PeerAcl {
    let client = canonical(client);
    if classify(client) == PeerClass::Loopback {
        // A real operator on loopback connects directly, not through the
        // funnel proxy; a forwarded loopback claim is not the trusted seat.
        return PeerAcl::denied();
    }

    if let Some(verdict) = auth_fail_closed(config) {
        return verdict;
    }

    let listed_connect = matches_any(&config.allow.connect, client);
    let listed_write = matches_any(&config.allow.write, client);
    if !(listed_connect
        || listed_write
        || (config.allow.connect.is_empty() && empty_connect_admits(config.allow.mode)))
    {
        return PeerAcl::denied();
    }

    let can_write =
        listed_write || (config.allow.write.is_empty() && empty_write_admits(config.allow.mode));

    PeerAcl {
        can_connect: true,
        can_write,
        auth_required: auth_required(config),
    }
}

/// True when the peer is explicitly named by either allowlist — the state
/// that makes an address admitted regardless of the listen-mode audience.
pub fn explicitly_listed(config: &NetworkConfig, peer: IpAddr) -> bool {
    let peer = canonical(peer);
    matches_any(&config.allow.connect, peer) || matches_any(&config.allow.write, peer)
}

/// The operator asked for PIN protection that was never configured — refuse
/// instead of quietly exposing the tool.
fn auth_fail_closed(config: &NetworkConfig) -> Option<PeerAcl> {
    (auth_required(config) && config.auth.pin.is_none()).then(PeerAcl::denied)
}

fn auth_required(config: &NetworkConfig) -> bool {
    match config.auth.scope {
        AuthScope::Never => false,
        AuthScope::NetworkOnly | AuthScope::Always => true,
    }
}

fn matches_any(rules: &[String], peer: IpAddr) -> bool {
    rules
        .iter()
        .filter_map(|r| IpNet::parse(r))
        .any(|net| net.contains(peer))
}

fn empty_connect_admits(mode: crate::config::AllowMode) -> bool {
    matches!(
        mode,
        crate::config::AllowMode::Open | crate::config::AllowMode::Balanced
    )
}

fn empty_write_admits(mode: crate::config::AllowMode) -> bool {
    mode == crate::config::AllowMode::Open
}

fn mode_admits(mode: ListenMode, peer: IpAddr) -> bool {
    match mode {
        ListenMode::Localhost => false, // non-loopback peers already returned
        ListenMode::Lan => {
            matches!(classify(peer), PeerClass::PrivateLan | PeerClass::LinkLocal)
        }
        ListenMode::Tailscale => matches!(classify(peer), PeerClass::Tailnet),
        ListenMode::Wan => true,
    }
}

/// The human-facing audience summary for the UI.
pub fn default_audience(mode: ListenMode) -> &'static str {
    match mode {
        ListenMode::Localhost => "this machine only (127.0.0.1)",
        ListenMode::Lan => "this machine and your LAN",
        ListenMode::Tailscale => "this machine and your tailnet",
        ListenMode::Wan => "anyone who can reach this port",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AllowMode, AllowRules, AuthConfig, PinHash};

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn cidr_parse_and_match() {
        let net = IpNet::parse("192.168.1.0/24").unwrap();
        assert!(net.contains(ip("192.168.1.77")));
        assert!(!net.contains(ip("192.168.2.1")));
        assert_eq!(net.addr, ip("192.168.1.0"));

        // Bare IPs become host routes.
        let host = IpNet::parse("100.64.1.5").unwrap();
        assert_eq!(host.prefix, 32);
        assert!(host.contains(ip("100.64.1.5")));
        assert!(!host.contains(ip("100.64.1.6")));

        assert!(IpNet::parse("::/0").unwrap().contains(ip("2001:db8::1")));
        // Family mismatch never matches (v4 peers collapse to v4 first).
        assert!(!IpNet::parse("::/0").unwrap().contains(ip("8.8.8.8")));
        assert!(!IpNet::parse("0.0.0.0/0")
            .unwrap()
            .contains(ip("2001:db8::1")));
        assert!(IpNet::parse("0.0.0.0/0").unwrap().contains(ip("8.8.8.8")));
        assert!(IpNet::parse("fc00::/7").unwrap().contains(ip("fd00::1")));
        assert_eq!(IpNet::parse("192.168.1.0/33"), None);
        assert_eq!(IpNet::parse("nope/24"), None);
    }

    #[test]
    fn v4_mapped_v6_peers_collapse() {
        assert_eq!(canonical(ip("::ffff:192.168.1.4")), ip("192.168.1.4"));
        assert_eq!(classify(ip("::ffff:127.0.0.1")), PeerClass::Loopback);
    }

    #[test]
    fn classification_covers_the_ranges_that_matter() {
        assert_eq!(classify(ip("127.0.0.1")), PeerClass::Loopback);
        assert_eq!(classify(ip("10.1.2.3")), PeerClass::PrivateLan);
        assert_eq!(classify(ip("172.16.0.1")), PeerClass::PrivateLan);
        assert_eq!(classify(ip("172.32.0.1")), PeerClass::Public);
        assert_eq!(classify(ip("192.168.0.1")), PeerClass::PrivateLan);
        assert_eq!(classify(ip("100.100.100.100")), PeerClass::Tailnet);
        assert_eq!(classify(ip("100.64.0.1")), PeerClass::Tailnet);
        assert_eq!(classify(ip("100.127.255.255")), PeerClass::Tailnet);
        // 100.128.0.1 sits just past the /10 — correctly not tailnet.
        assert_eq!(classify(ip("100.128.0.1")), PeerClass::Public);
        assert_eq!(classify(ip("100.160.0.1")), PeerClass::Public);
        assert_eq!(classify(ip("169.254.1.1")), PeerClass::LinkLocal);
        assert_eq!(classify(ip("8.8.8.8")), PeerClass::Public);
        assert_eq!(classify(ip("fd12::1")), PeerClass::PrivateLan);
        assert_eq!(classify(ip("fe80::1")), PeerClass::LinkLocal);
    }

    #[test]
    fn loopback_is_always_trusted() {
        for mode in [
            ListenMode::Localhost,
            ListenMode::Lan,
            ListenMode::Tailscale,
            ListenMode::Wan,
        ] {
            let config = NetworkConfig {
                listen: mode,
                allow: AllowRules {
                    connect: vec!["10.0.0.0/8".into()],
                    write: vec!["10.0.0.0/8".into()],
                    ..Default::default()
                },
                auth: AuthConfig {
                    scope: AuthScope::NetworkOnly,
                    pin: Some(PinHash::generate("1")),
                    ..AuthConfig::default()
                },
                ..NetworkConfig::default()
            };
            let acl = evaluate(&config, ip("127.0.0.1"));
            assert!(
                acl.can_connect && acl.can_write && !acl.auth_required,
                "{mode:?}"
            );
        }
    }

    #[test]
    fn localhost_mode_refuses_everything_else() {
        let config = NetworkConfig::default(); // Localhost
        assert!(!evaluate(&config, ip("192.168.1.4")).can_connect);
        assert!(!evaluate(&config, ip("100.64.1.4")).can_connect);
        assert!(!evaluate(&config, ip("8.8.8.8")).can_connect);
    }

    #[test]
    fn lan_mode_admits_private_not_tailnet_not_public() {
        let config = NetworkConfig {
            listen: ListenMode::Lan,
            ..NetworkConfig::default()
        };
        assert!(evaluate(&config, ip("192.168.1.4")).can_connect);
        assert!(!evaluate(&config, ip("100.64.1.4")).can_connect);
        assert!(!evaluate(&config, ip("8.8.8.8")).can_connect);
    }

    #[test]
    fn allow_mode_open_lets_empty_lists_grant_view_and_edits() {
        let config = NetworkConfig {
            listen: ListenMode::Lan,
            allow: AllowRules {
                mode: AllowMode::Open,
                ..Default::default()
            },
            ..NetworkConfig::default()
        };
        let peer = evaluate(&config, ip("192.168.1.4"));
        assert!(
            peer.can_connect && peer.can_write,
            "open: empty lists admit all"
        );
    }

    #[test]
    fn allow_mode_balanced_keeps_the_legacy_split() {
        let config = NetworkConfig {
            listen: ListenMode::Lan,
            allow: AllowRules {
                mode: AllowMode::Balanced,
                ..Default::default()
            },
            ..NetworkConfig::default()
        };
        let peer = evaluate(&config, ip("192.168.1.4"));
        assert!(peer.can_connect, "balanced: anyone admitted may view");
        assert!(!peer.can_write, "balanced: edits still require listing");
    }

    #[test]
    fn allow_mode_strict_locks_empty_lists_to_loopback() {
        let config = NetworkConfig {
            listen: ListenMode::Lan,
            allow: AllowRules {
                mode: AllowMode::Strict,
                ..Default::default()
            },
            ..NetworkConfig::default()
        };
        let peer = evaluate(&config, ip("192.168.1.4"));
        assert!(
            !peer.can_connect && !peer.can_write,
            "strict: unlisted peers are denied"
        );

        // Listing the address admits connecting; edits still need the write list.
        let listed = NetworkConfig {
            listen: ListenMode::Lan,
            allow: AllowRules {
                connect: vec!["192.168.1.4".into()],
                mode: AllowMode::Strict,
                ..Default::default()
            },
            ..NetworkConfig::default()
        };
        let admitted = evaluate(&listed, ip("192.168.1.4"));
        assert!(
            admitted.can_connect,
            "strict: a listed peer may connect and view"
        );
        assert!(!admitted.can_write, "strict: edits need the write list too");
    }

    #[test]
    fn listed_addresses_are_admitted_regardless_of_listen_mode() {
        // The operator typed the address; the listen mode's default audience
        // (here: LAN) must not veto it. Covers the "allowlisted tailnet IP
        // refused under lan" report.
        let config = NetworkConfig {
            listen: ListenMode::Lan,
            allow: AllowRules {
                connect: vec!["100.115.95.115".into()],
                mode: AllowMode::Strict,
                ..Default::default()
            },
            ..NetworkConfig::default()
        };
        let tailnet_peer = evaluate(&config, ip("100.115.95.115"));
        assert!(
            tailnet_peer.can_connect,
            "a listed tailnet IP connects under lan"
        );

        let listed_public = NetworkConfig {
            listen: ListenMode::Lan,
            allow: AllowRules {
                connect: vec!["203.0.113.9".into()],
                mode: AllowMode::Strict,
                ..Default::default()
            },
            ..NetworkConfig::default()
        };
        let public_peer = evaluate(&listed_public, ip("203.0.113.9"));
        assert!(
            public_peer.can_connect,
            "a listed public IP connects under lan"
        );

        // The mode still caps the DEFAULT (empty-list) audience: an unlisted
        // tailnet peer stays refused under lan even in open mode.
        let open = NetworkConfig {
            listen: ListenMode::Lan,
            allow: AllowRules {
                mode: AllowMode::Open,
                ..Default::default()
            },
            ..NetworkConfig::default()
        };
        assert!(!evaluate(&open, ip("100.115.95.115")).can_connect);
    }

    #[test]
    fn localhost_mode_stays_loopback_only_even_with_listings() {
        let config = NetworkConfig {
            listen: ListenMode::Localhost,
            allow: AllowRules {
                connect: vec!["192.168.1.4".into()],
                mode: AllowMode::Open,
                ..Default::default()
            },
            ..NetworkConfig::default()
        };
        assert!(!evaluate(&config, ip("192.168.1.4")).can_connect);
    }

    #[test]
    fn write_listing_implies_connect() {
        // Write ⊆ connect in every mode: naming someone as an editor must
        // not leave them unable to connect because of the mode's audience.
        let config = NetworkConfig {
            listen: ListenMode::Tailscale,
            allow: AllowRules {
                write: vec!["192.168.1.7".into()],
                mode: AllowMode::Balanced,
                ..Default::default()
            },
            ..NetworkConfig::default()
        };
        let editor = evaluate(&config, ip("192.168.1.7"));
        assert!(editor.can_connect && editor.can_write);
    }

    #[test]
    fn forwarded_peers_follow_the_allowlists_not_the_listen_mode() {
        // Funnel forwards arrive on loopback with X-Forwarded-For; the proxy
        // is its own exposure decision, so the mode's default audience does
        // not apply — but the allowlists do.
        let mut config = NetworkConfig {
            listen: ListenMode::Lan,
            auth: AuthConfig {
                scope: AuthScope::Always,
                pin: Some(PinHash::generate("1234")),
                ..AuthConfig::default()
            },
            ..NetworkConfig::default()
        };

        // Balanced + empty lists: anyone may view (the funnel posture).
        let anyone = evaluate_forwarded(&config, ip("203.0.113.9"));
        assert!(anyone.can_connect && anyone.auth_required);
        assert!(!anyone.can_write, "balanced: edits still require listing");

        // Strict + empty lists: nobody but the loopback seat.
        config.allow.mode = AllowMode::Strict;
        assert!(!evaluate_forwarded(&config, ip("203.0.113.9")).can_connect);

        // Listed in connect: admitted, read-only until write-listed.
        config.allow.connect = vec!["203.0.113.9".into()];
        let listed = evaluate_forwarded(&config, ip("203.0.113.9"));
        assert!(listed.can_connect && !listed.can_write && listed.auth_required);

        // Removal takes effect: a peer that is no longer listed is refused.
        config.allow.connect = vec!["198.51.100.1".into()];
        assert!(!evaluate_forwarded(&config, ip("203.0.113.9")).can_connect);
    }

    #[test]
    fn forwarded_loopback_claims_are_not_the_trusted_seat() {
        // A forwarded X-Forwarded-For claiming 127.0.0.1 is spoof-shaped,
        // not the local operator: never trust it.
        let config = NetworkConfig {
            listen: ListenMode::Wan,
            allow: AllowRules {
                connect: vec!["127.0.0.1".into()],
                mode: AllowMode::Strict,
                ..Default::default()
            },
            ..NetworkConfig::default()
        };
        assert!(!evaluate_forwarded(&config, ip("127.0.0.1")).can_connect);
        assert!(!evaluate_forwarded(&config, ip("::1")).can_connect);
    }

    #[test]
    fn tailscale_mode_admits_only_the_tailnet_range() {
        let config = NetworkConfig {
            listen: ListenMode::Tailscale,
            ..NetworkConfig::default()
        };
        assert!(evaluate(&config, ip("100.101.1.4")).can_connect);
        assert!(!evaluate(&config, ip("192.168.1.4")).can_connect);
        assert!(!evaluate(&config, ip("8.8.8.8")).can_connect);
    }

    #[test]
    fn wan_mode_still_honors_allowlists() {
        let config = NetworkConfig {
            listen: ListenMode::Wan,
            allow: AllowRules {
                connect: vec!["203.0.113.0/24".into()],
                write: vec!["203.0.113.7".into()],
                ..Default::default()
            },
            ..NetworkConfig::default()
        };
        let guest = evaluate(&config, ip("203.0.113.99"));
        assert!(guest.can_connect && !guest.can_write);
        let editor = evaluate(&config, ip("203.0.113.7"));
        assert!(editor.can_connect && editor.can_write);
        let stranger = evaluate(&config, ip("198.51.100.1"));
        assert!(!stranger.can_connect);
    }

    #[test]
    fn auth_scope_requires_pin_for_network_peers_only() {
        let mut config = NetworkConfig {
            listen: ListenMode::Lan,
            ..NetworkConfig::default()
        };
        config.auth.scope = AuthScope::NetworkOnly;
        config.auth.pin = Some(PinHash::generate("9999"));
        assert!(evaluate(&config, ip("192.168.1.9")).auth_required);
        assert!(!evaluate(&config, ip("127.0.0.1")).auth_required);

        config.auth.scope = AuthScope::Always;
        assert!(evaluate(&config, ip("127.0.0.1")).auth_required);
    }

    #[test]
    fn auth_without_configured_pin_fails_closed() {
        let mut config = NetworkConfig {
            listen: ListenMode::Lan,
            ..NetworkConfig::default()
        };
        config.auth.scope = AuthScope::NetworkOnly;
        assert!(config.auth.pin.is_none());
        assert!(!evaluate(&config, ip("192.168.1.9")).can_connect);
    }

    #[test]
    fn write_allowlist_is_a_subset_of_connect() {
        let config = NetworkConfig {
            listen: ListenMode::Lan,
            allow: AllowRules {
                connect: vec![],
                write: vec!["192.168.1.0/24".into()],
                ..Default::default()
            },
            ..NetworkConfig::default()
        };
        let editor = evaluate(&config, ip("192.168.1.50"));
        assert!(editor.can_connect && editor.can_write);
        let reader = evaluate(&config, ip("10.0.0.5"));
        assert!(reader.can_connect && !reader.can_write);
    }
}
