use std::path::{Path, PathBuf};

use sysinfo::{Pid, ProcessStatus, System};

use super::protocol::{BridgeEndpointFile, BRIDGE_PROTOCOL_VERSION};

#[derive(Debug, Clone, PartialEq)]
pub struct DiscoveredEndpoint {
    pub pid: u32,
    pub name: String,
    pub port: u16,
    pub token: String,
    pub bind: String,
}

pub fn default_endpoint_dir() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("PS_BRIDGE_ENDPOINT_DIR") {
        return Some(PathBuf::from(path));
    }
    let local_app_data = std::env::var("LOCALAPPDATA").ok()?;
    Some(
        Path::new(&local_app_data)
            .join("Pal")
            .join("Saved")
            .join("PSAmity")
            .join("endpoints"),
    )
}

pub fn scan_endpoints(dir: &Path, liveness: &dyn Fn(u32) -> bool) -> Vec<DiscoveredEndpoint> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut found: Vec<DiscoveredEndpoint> = entries
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
        .filter_map(|entry| {
            let bytes = std::fs::read(entry.path()).ok()?;
            let file: BridgeEndpointFile = serde_json::from_slice(&bytes).ok()?;
            if file.protocol_version != BRIDGE_PROTOCOL_VERSION || !liveness(file.pid) {
                return None;
            }
            Some(DiscoveredEndpoint {
                pid: file.pid,
                name: if file.name.is_empty() {
                    "PSAmity".to_string()
                } else {
                    file.name
                },
                port: file.port,
                token: file.token,
                bind: file.bind,
            })
        })
        .collect();

    found.sort_by_key(|endpoint| endpoint.pid);
    found
}

pub fn sysinfo_liveness(pid: u32) -> bool {
    let mut system = System::new();
    system.refresh_processes(
        sysinfo::ProcessesToUpdate::Some(&[Pid::from_u32(pid)]),
        true,
    );
    system
        .process(Pid::from_u32(pid))
        .map(|process| !matches!(process.status(), ProcessStatus::Zombie))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, json: &str) {
        std::fs::write(dir.join(name), json).unwrap();
    }

    const LIVE: &str = r#"{"protocolVersion":2,"port":8788,"token":"t","name":"Solo",
        "bind":"127.0.0.1","pid":11,"startedAt":"now"}"#;

    #[test]
    fn missing_directory_scans_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(scan_endpoints(&dir.path().join("nope"), &|_| true).is_empty());
    }

    #[test]
    fn empty_directory_scans_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(scan_endpoints(dir.path(), &|_| true).is_empty());
    }

    #[test]
    fn live_file_parses_with_name_and_bind() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "11.json", LIVE);
        let found = scan_endpoints(dir.path(), &|_| true);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].pid, 11);
        assert_eq!(found[0].port, 8788);
        assert_eq!(found[0].token, "t");
        assert_eq!(found[0].name, "Solo");
        assert_eq!(found[0].bind, "127.0.0.1");
    }

    #[test]
    fn multiple_live_instances_are_returned_sorted_by_pid() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "30.json",
            r#"{"protocolVersion":2,"port":2,"token":"b","name":"B","bind":"0.0.0.0","pid":30,"startedAt":"n"}"#,
        );
        write(dir.path(), "11.json", LIVE);
        let found = scan_endpoints(dir.path(), &|_| true);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].pid, 11);
        assert_eq!(found[1].pid, 30);
    }

    #[test]
    fn dead_pid_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "11.json", LIVE);
        assert!(scan_endpoints(dir.path(), &|_| false).is_empty());
    }

    #[test]
    fn wrong_protocol_version_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "11.json",
            r#"{"protocolVersion":1,"port":1,"token":"t","pid":11,"startedAt":"n"}"#,
        );
        assert!(scan_endpoints(dir.path(), &|_| true).is_empty());
    }

    #[test]
    fn malformed_file_is_skipped_without_hiding_its_neighbours() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "9.json", "not json");
        write(dir.path(), "11.json", LIVE);
        let found = scan_endpoints(dir.path(), &|_| true);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].pid, 11);
    }

    #[test]
    fn non_json_entries_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "11.json.tmp", LIVE);
        assert!(scan_endpoints(dir.path(), &|_| true).is_empty());
    }

    #[test]
    fn missing_name_falls_back_to_a_placeholder() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "11.json",
            r#"{"protocolVersion":2,"port":1,"token":"t","pid":11,"startedAt":"n"}"#,
        );
        let found = scan_endpoints(dir.path(), &|_| true);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "PSAmity");
    }
}
