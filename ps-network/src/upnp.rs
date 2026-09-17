//! Optional UPnP port mapping (feature `upnp`).
//!
//! This is the one part of the network policy that MODIFIES THE ROUTER
//! instead of PalStudio. It exists because some users on locked-down LANs
//! genuinely have no tailscale option, but it is off by default, marked
//! "highly not recommended" in the UI, and never enabled implicitly: a
//! router-level hole for a save editor is strictly worse than a tailnet.
#![cfg(feature = "upnp")]

use std::net::{SocketAddr, ToSocketAddrs};
use std::time::Duration;

const MAPPING_LEASE_SECS: u32 = 12 * 60 * 60;
const MAPPING_DESCRIPTION: &str = "PalStudio";
const MAX_MAPPING_SCAN: u32 = 512;

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
/// back and cleaned up. The finite lease is renewed by the server's periodic
/// reconciler and is never enabled implicitly.
pub async fn map_port(port: u16, description: &str) -> Result<(), String> {
    if port == 0 || description.is_empty() || description.len() > 64 {
        return Err("invalid UPnP mapping parameters".into());
    }
    if description.chars().any(char::is_control) {
        return Err("UPnP mapping description contains control characters".into());
    }
    let gateway = tokio::time::timeout(
        Duration::from_secs(10),
        igd_next::aio::tokio::search_gateway(Default::default()),
    )
    .await
    .map_err(|_| "UPnP gateway search timed out".to_owned())?
    .map_err(|e| format!("no UPnP gateway found: {e}"))?;
    let local = local_lan_addr(port)
        .ok_or_else(|| "could not determine this machine's LAN address".to_owned())?;
    if !matches!(
        crate::policy::classify(local.ip()),
        crate::policy::PeerClass::PrivateLan | crate::policy::PeerClass::LinkLocal
    ) {
        return Err("UPnP refused to map a non-LAN local address".into());
    }
    tokio::time::timeout(
        Duration::from_secs(10),
        gateway.add_port(
            igd_next::PortMappingProtocol::TCP,
            port,
            local,
            MAPPING_LEASE_SECS,
            description,
        ),
    )
    .await
    .map_err(|_| "UPnP mapping request timed out".to_owned())?
    .map_err(|e| format!("router refused the mapping: {e}"))?;
    tracing::info!(port, "UPnP port mapping established ({description})");
    Ok(())
}

/// Remove a mapping previously created by `map_port`.
pub async fn unmap_port(port: u16) -> Result<(), String> {
    unmap_owned_port(port, MAPPING_DESCRIPTION).await
}

/// Remove only entries created by this application. UPnP has no portable
/// ownership token, so the exact description, protocol, and local port are
/// checked before deletion. The internal address is deliberately not part of
/// the match: DHCP can change it between a port change and cleanup.
pub async fn unmap_owned_port(port: u16, description: &str) -> Result<(), String> {
    if port == 0 || description.is_empty() {
        return Err("invalid UPnP mapping parameters".into());
    }
    let gateway = tokio::time::timeout(
        Duration::from_secs(10),
        igd_next::aio::tokio::search_gateway(Default::default()),
    )
    .await
    .map_err(|_| "UPnP gateway search timed out".to_owned())?
    .map_err(|e| format!("no UPnP gateway found: {e}"))?;
    let mut removed = false;
    for index in 0..MAX_MAPPING_SCAN {
        let entry = match tokio::time::timeout(
            Duration::from_secs(3),
            gateway.get_generic_port_mapping_entry(index),
        )
        .await
        {
            Ok(Ok(entry)) => entry,
            Ok(Err(_)) => break,
            Err(_) => return Err("UPnP mapping scan timed out".into()),
        };
        if entry.protocol == igd_next::PortMappingProtocol::TCP
            && entry.external_port == port
            && entry.internal_port == port
            && entry.port_mapping_description == description
        {
            tokio::time::timeout(
                Duration::from_secs(10),
                gateway.remove_port(igd_next::PortMappingProtocol::TCP, port),
            )
            .await
            .map_err(|_| "UPnP unmapping request timed out".to_owned())?
            .map_err(|e| format!("could not remove the mapping: {e}"))?;
            removed = true;
            break;
        }
    }
    if removed {
        tracing::info!(port, "owned UPnP port mapping removed");
    }
    Ok(())
}

/// Remove all PalStudio-owned mappings, including stale mappings left by a
/// previous port. This never deletes another application's port entry.
pub async fn remove_owned_mappings(description: &str) -> Result<(), String> {
    remove_owned_mappings_except(description, None).await
}

/// Remove owned mappings except the currently desired external port. IGDs
/// compact their tables immediately after a delete, so the scan index does
/// not advance on a deletion or stale entries would be skipped.
pub async fn remove_owned_mappings_except(
    description: &str,
    keep_port: Option<u16>,
) -> Result<(), String> {
    if description.is_empty() {
        return Err("invalid UPnP mapping description".into());
    }
    let gateway = tokio::time::timeout(
        Duration::from_secs(10),
        igd_next::aio::tokio::search_gateway(Default::default()),
    )
    .await
    .map_err(|_| "UPnP gateway search timed out".to_owned())?
    .map_err(|e| format!("no UPnP gateway found: {e}"))?;
    let mut removed = 0u32;
    let mut index = 0u32;
    let mut inspected = 0u32;
    while inspected < MAX_MAPPING_SCAN {
        let entry = match tokio::time::timeout(
            Duration::from_secs(3),
            gateway.get_generic_port_mapping_entry(index),
        )
        .await
        {
            Ok(Ok(entry)) => entry,
            Ok(Err(_)) => break,
            Err(_) => return Err("UPnP mapping scan timed out".into()),
        };
        inspected = inspected.saturating_add(1);
        if entry.protocol == igd_next::PortMappingProtocol::TCP
            && entry.port_mapping_description == description
            && entry.internal_port == entry.external_port
            && keep_port != Some(entry.external_port)
        {
            tokio::time::timeout(
                Duration::from_secs(10),
                gateway.remove_port(igd_next::PortMappingProtocol::TCP, entry.external_port),
            )
            .await
            .map_err(|_| "UPnP unmapping request timed out".to_owned())?
            .map_err(|e| format!("could not remove an owned mapping: {e}"))?;
            removed = removed.saturating_add(1);
        } else {
            index = index.saturating_add(1);
        }
    }
    if removed > 0 {
        tracing::info!(removed, "stale PalStudio UPnP mappings removed");
    }
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
