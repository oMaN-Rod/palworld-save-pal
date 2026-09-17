//! Optional UPnP port mapping (feature `upnp`).
//!
//! This is the one part of the network policy that MODIFIES THE ROUTER
//! instead of PalStudio. It exists because some users on locked-down LANs
//! genuinely have no tailscale option, but it is off by default, marked
//! "highly not recommended" in the UI, and never enabled implicitly: a
//! router-level hole for a save editor is strictly worse than a tailnet.
#![cfg(feature = "upnp")]

use std::net::{SocketAddr, ToSocketAddrs};

/// Best-effort local LAN address (the classic no-packet UDP connect trick);
/// UPnP mappings must point at a routable host address, not loopback.
fn local_lan_addr(port: u16) -> Option<SocketAddr> {
    let target: SocketAddr = "8.8.8.8:53".to_socket_addrs().ok()?.next()?;
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect(target).ok()?;
    let mut addr = socket.local_addr().ok()?;
    addr.set_port(port);
    Some(addr)
}

/// Map `external -> port` on the first IGD that answers.
///
/// The description identifies the hole in router UIs so it can be traced
/// back and cleaned up. Lease duration is 24h (0 = infinite is rude); the
/// mapping is not re-created across PalStudio restarts unless the user
/// re-applies it from the Network page.
pub async fn map_port(port: u16, description: &str) -> Result<(), String> {
    let gateway = igd_next::aio::tokio::search_gateway(Default::default())
        .await
        .map_err(|e| format!("no UPnP gateway found: {e}"))?;
    let local = local_lan_addr(port)
        .ok_or_else(|| "could not determine this machine's LAN address".to_owned())?;
    gateway
        .add_port(
            igd_next::PortMappingProtocol::TCP,
            port,
            local,
            24 * 60 * 60,
            description,
        )
        .await
        .map_err(|e| format!("router refused the mapping: {e}"))?;
    tracing::info!(port, "UPnP port mapping established ({description})");
    Ok(())
}

/// Remove a mapping previously created by `map_port`.
pub async fn unmap_port(port: u16) -> Result<(), String> {
    let gateway = igd_next::aio::tokio::search_gateway(Default::default())
        .await
        .map_err(|e| format!("no UPnP gateway found: {e}"))?;
    gateway
        .remove_port(igd_next::PortMappingProtocol::TCP, port)
        .await
        .map_err(|e| format!("could not remove the mapping: {e}"))?;
    Ok(())
}

/// One-shot availability probe for the Network page.
pub async fn probe() -> bool {
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        igd_next::aio::tokio::search_gateway(Default::default())
            .await
            .is_ok()
    })
    .await
    .is_ok_and(|found| found)
}
