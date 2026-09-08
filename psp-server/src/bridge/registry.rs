use psp_db::amity_instances::AmityInstance;

use super::endpoint::DiscoveredEndpoint;
use super::service::BridgeTarget;

pub const ACTIVE_INSTANCE_KEY: &str = "amity.active_instance";

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceEntry {
    pub id: String,
    pub source: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub live: bool,
}

fn auto_target(endpoint: &DiscoveredEndpoint) -> BridgeTarget {
    BridgeTarget {
        id: format!("auto:{}", endpoint.pid),
        name: endpoint.name.clone(),
        host: "127.0.0.1".to_string(),
        port: endpoint.port,
        token: endpoint.token.clone(),
    }
}

fn saved_target(instance: &AmityInstance) -> Option<BridgeTarget> {
    Some(BridgeTarget {
        id: format!("saved:{}", instance.id),
        name: instance.name.clone(),
        host: instance.host.clone(),
        port: u16::try_from(instance.port).ok()?,
        token: instance.token.clone(),
    })
}

pub fn merge_instances(discovered: &[DiscoveredEndpoint], saved: &[AmityInstance]) -> Vec<InstanceEntry> {
    let auto = discovered.iter().map(|endpoint| InstanceEntry {
        id: format!("auto:{}", endpoint.pid),
        source: "auto".to_string(),
        name: endpoint.name.clone(),
        host: "127.0.0.1".to_string(),
        port: endpoint.port,
        live: true,
    });

    let stored = saved.iter().filter_map(|instance| {
        Some(InstanceEntry {
            id: format!("saved:{}", instance.id),
            source: "saved".to_string(),
            name: instance.name.clone(),
            host: instance.host.clone(),
            port: u16::try_from(instance.port).ok()?,
            live: false,
        })
    });

    auto.chain(stored).collect()
}

pub fn target_for(
    id: &str,
    discovered: &[DiscoveredEndpoint],
    saved: &[AmityInstance],
) -> Option<BridgeTarget> {
    let (source, rest) = id.split_once(':')?;
    match source {
        "auto" => {
            let pid: u32 = rest.parse().ok()?;
            discovered.iter().find(|e| e.pid == pid).map(auto_target)
        }
        "saved" => {
            let row: i64 = rest.parse().ok()?;
            saved.iter().find(|i| i.id == row).and_then(saved_target)
        }
        _ => None,
    }
}

pub fn default_target(discovered: &[DiscoveredEndpoint]) -> Option<BridgeTarget> {
    discovered.first().map(auto_target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn discovered(pid: u32, name: &str, port: u16) -> DiscoveredEndpoint {
        DiscoveredEndpoint {
            pid,
            name: name.to_string(),
            port,
            token: format!("token-{pid}"),
            bind: "127.0.0.1".to_string(),
        }
    }

    fn saved(id: i64, name: &str, host: &str, port: i64) -> AmityInstance {
        AmityInstance {
            id,
            name: name.to_string(),
            host: host.to_string(),
            port,
            token: format!("token-saved-{id}"),
        }
    }

    #[test]
    fn merge_lists_auto_entries_before_saved_ones() {
        let entries = merge_instances(
            &[discovered(11, "Solo", 52104)],
            &[saved(1, "Remote", "10.0.0.14", 8788)],
        );
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, "auto:11");
        assert_eq!(entries[0].source, "auto");
        assert_eq!(entries[0].host, "127.0.0.1");
        assert_eq!(entries[0].port, 52104);
        assert!(entries[0].live);
        assert_eq!(entries[1].id, "saved:1");
        assert_eq!(entries[1].source, "saved");
        assert_eq!(entries[1].host, "10.0.0.14");
        assert!(!entries[1].live);
    }

    #[test]
    fn merge_handles_both_lists_being_empty() {
        assert!(merge_instances(&[], &[]).is_empty());
    }

    #[test]
    fn target_for_resolves_an_auto_id_to_loopback() {
        let target = target_for("auto:11", &[discovered(11, "Solo", 52104)], &[]).unwrap();
        assert_eq!(target.id, "auto:11");
        assert_eq!(target.name, "Solo");
        assert_eq!(target.host, "127.0.0.1");
        assert_eq!(target.port, 52104);
        assert_eq!(target.token, "token-11");
    }

    #[test]
    fn target_for_resolves_a_saved_id_to_its_host() {
        let target = target_for("saved:1", &[], &[saved(1, "Remote", "10.0.0.14", 8788)]).unwrap();
        assert_eq!(target.host, "10.0.0.14");
        assert_eq!(target.port, 8788);
        assert_eq!(target.token, "token-saved-1");
    }

    #[test]
    fn target_for_returns_none_for_unknown_or_malformed_ids() {
        let live = [discovered(11, "Solo", 52104)];
        assert!(target_for("auto:99", &live, &[]).is_none());
        assert!(target_for("saved:99", &live, &[]).is_none());
        assert!(target_for("nonsense", &live, &[]).is_none());
        assert!(target_for("auto:not-a-number", &live, &[]).is_none());
        assert!(target_for("", &live, &[]).is_none());
    }

    #[test]
    fn default_target_picks_the_first_discovered_instance() {
        let target = default_target(&[discovered(11, "Solo", 52104), discovered(30, "Server", 8788)]).unwrap();
        assert_eq!(target.id, "auto:11");
    }

    #[test]
    fn default_target_is_none_with_nothing_discovered() {
        assert!(default_target(&[]).is_none());
    }

    #[test]
    fn a_saved_port_outside_u16_does_not_resolve() {
        assert!(target_for("saved:1", &[], &[saved(1, "Bad", "10.0.0.1", 70000)]).is_none());
    }
}
