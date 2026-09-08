use std::path::{Path, PathBuf};

use sysinfo::{Pid, ProcessStatus, System};

use super::protocol::{BridgeEndpointFile, BRIDGE_PROTOCOL_VERSION};

pub fn default_endpoint_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("PSP_BRIDGE_ENDPOINT_PATH") {
        return Some(PathBuf::from(path));
    }
    let local_app_data = std::env::var("LOCALAPPDATA").ok()?;
    Some(
        Path::new(&local_app_data)
            .join("Pal")
            .join("Saved")
            .join("PSPAmity")
            .join("endpoint.json"),
    )
}

pub fn read_endpoint(path: &Path, liveness: &dyn Fn(u32) -> bool) -> Option<BridgeEndpointFile> {
    let bytes = std::fs::read(path).ok()?;
    let endpoint: BridgeEndpointFile = serde_json::from_slice(&bytes).ok()?;
    if endpoint.protocol_version != BRIDGE_PROTOCOL_VERSION {
        return None;
    }
    if !liveness(endpoint.pid) {
        return None;
    }
    Some(endpoint)
}

pub fn sysinfo_liveness(pid: u32) -> bool {
    let mut system = System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[Pid::from_u32(pid)]), true);
    system
        .process(Pid::from_u32(pid))
        .map(|process| !matches!(process.status(), ProcessStatus::Zombie))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, json: &str) -> PathBuf {
        let path = dir.join("endpoint.json");
        std::fs::write(&path, json).unwrap();
        path
    }

    #[test]
    fn missing_file_reads_as_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("endpoint.json");
        assert!(read_endpoint(&path, &|_| true).is_none());
    }

    #[test]
    fn malformed_json_reads_as_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path(), "not json");
        assert!(read_endpoint(&path, &|_| true).is_none());
    }

    #[test]
    fn wrong_protocol_version_reads_as_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(
            dir.path(),
            r#"{"protocolVersion":2,"port":1234,"token":"t","pid":1,"startedAt":"now"}"#,
        );
        assert!(read_endpoint(&path, &|_| true).is_none());
    }

    #[test]
    fn dead_pid_reads_as_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(
            dir.path(),
            r#"{"protocolVersion":1,"port":1234,"token":"t","pid":1,"startedAt":"now"}"#,
        );
        assert!(read_endpoint(&path, &|_| false).is_none());
    }

    #[test]
    fn live_valid_file_parses() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(
            dir.path(),
            r#"{"protocolVersion":1,"port":1234,"token":"t","pid":1,"startedAt":"now"}"#,
        );
        let endpoint = read_endpoint(&path, &|_| true).expect("should parse");
        assert_eq!(endpoint.port, 1234);
        assert_eq!(endpoint.token, "t");
    }
}
