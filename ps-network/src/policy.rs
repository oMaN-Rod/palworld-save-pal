//! Peer classification and the allow/deny decision engine.
//!
//! The listener binds broadly and every request is judged here, per peer —
//! so switching listen mode never rebinds a socket. Loopback is always the
//! trusted operator seat; everything else must pass the mode gate, then the
//! optional connect allowlist; writes additionally pass the write allowlist;
//! and a configured auth scope can demand a PIN session first.
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

    if !mode_admits(config.listen, peer) {
        return PeerAcl::denied();
    }

    let pin_configured = config.auth.pin.is_some();
    let auth_required = match config.auth.scope {
        AuthScope::Never => false,
        AuthScope::NetworkOnly | AuthScope::Always => true,
    };
    if auth_required && !pin_configured {
        // Operator asked for protection they never configured — refuse
        // instead of quietly exposing the tool.
        return PeerAcl::denied();
    }

    let matches_any = |rules: &[String]| {
        rules
            .iter()
            .filter_map(|r| IpNet::parse(r))
            .any(|net| net.contains(peer))
    };

    let can_connect = config.allow.connect.is_empty() || matches_any(&config.allow.connect);
    if !can_connect {
        return PeerAcl::denied();
    }

    let can_write = config.allow.write.is_empty() || matches_any(&config.allow.write);

    PeerAcl {
        can_connect: true,
        can_write,
        auth_required,
    }
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
    use crate::config::{AllowRules, AuthConfig, PinHash};

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
            },
            ..NetworkConfig::default()
        };
        let editor = evaluate(&config, ip("192.168.1.50"));
        assert!(editor.can_connect && editor.can_write);
        let reader = evaluate(&config, ip("10.0.0.5"));
        assert!(reader.can_connect && !reader.can_write);
    }
}
