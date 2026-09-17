//! Tailscale presence detection and Funnel orchestration via the `tailscale`
//! CLI. No LocalAPI client, no extra dependencies — every machine running a
//! tailnet node has the CLI next to the daemon, and every operation here
//! degrades to "unavailable" without it.
//!
//! SECURITY NOTE surfaced to the UI: Funnel traffic is proxied by the local
//! tailscale daemon and therefore reaches PalStudio from a loopback/CGNAT
//! source we cannot distinguish from the operator's own machine. Enabling
//! Funnel with `funnel_enabled` must always be paired with a PIN
//! (AuthScope::NetworkOnly or Always) — the Network page says so in loud
//! words, and `funnel_status` reports the pairing problem when it sees it.
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const CLI_TIMEOUT: Duration = Duration::from_secs(10);
const TAILSCALE_PATHS: &[&str] = &[
    "/usr/local/bin/tailscale",
    "/opt/homebrew/bin/tailscale",
    "/usr/bin/tailscale",
    "/bin/tailscale",
];

/// What we could learn about tailscale on this machine.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TailscaleStatus {
    /// The CLI exists and answered.
    pub available: bool,
    /// This node's tailscale IPv4 addresses (CGNAT range).
    pub ipv4: Vec<IpAddr>,
    /// IPv4 addresses reported for authenticated remote tailnet peers.
    pub peer_ipv4: Vec<IpAddr>,
    /// `tailscale status` says we're logged in to a tailnet.
    pub logged_in: bool,
    /// The last `funnel` subcommand output, for the UI to show verbatim.
    pub last_funnel_output: Option<String>,
}

fn tailscale_program() -> Option<PathBuf> {
    let mut candidates = TAILSCALE_PATHS
        .iter()
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    if cfg!(windows) {
        candidates.push(PathBuf::from("C:\\Program Files\\Tailscale\\tailscale.exe"));
        candidates.push(PathBuf::from(
            "C:\\Program Files (x86)\\Tailscale\\tailscale.exe",
        ));
    }
    candidates.into_iter().find(|path| trusted_executable(path))
}

fn trusted_executable(path: &Path) -> bool {
    let Ok(canonical) = std::fs::canonicalize(path) else {
        return false;
    };
    let Ok(metadata) = std::fs::symlink_metadata(&canonical) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        use std::os::unix::fs::PermissionsExt;
        if metadata.uid() != 0 || metadata.permissions().mode() & 0o022 != 0 {
            return false;
        }
        let mut ancestor = canonical.parent();
        while let Some(path) = ancestor {
            let Ok(metadata) = std::fs::symlink_metadata(path) else {
                return false;
            };
            if metadata.uid() != 0 || metadata.permissions().mode() & 0o022 != 0 {
                return false;
            }
            if path.parent().is_none() {
                break;
            }
            ancestor = path.parent();
        }
    }
    true
}

fn run_tailscale(args: &[&str]) -> Option<String> {
    let output = run_capture(args).ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

/// Detect tailscale addresses. `tailscale ip -4` prints one address per
/// line; anything unparsable is skipped rather than fatal.
pub fn detect() -> TailscaleStatus {
    let Some(ips) = run_tailscale(&["ip", "-4"]) else {
        return TailscaleStatus::default();
    };
    let ipv4 = ips
        .lines()
        .map(str::trim)
        .filter_map(|line| line.parse::<IpAddr>().ok())
        .filter(|ip| {
            matches!(
                crate::policy::classify(*ip),
                crate::policy::PeerClass::Tailnet
            )
        })
        .collect();
    let status_json = run_tailscale(&["status", "--json"]);
    let peer_ipv4 = status_json
        .as_deref()
        .map(parse_peer_ipv4)
        .unwrap_or_default();
    let logged_in = status_json.as_deref().is_some_and(status_json_is_logged_in)
        || (status_json.is_none()
            && run_tailscale(&["status"]).is_some_and(|text| {
                let text = text.to_ascii_lowercase();
                !text.contains("logged out") && !text.contains("needs login")
            }));
    TailscaleStatus {
        available: true,
        ipv4,
        peer_ipv4,
        logged_in,
        last_funnel_output: None,
    }
}

/// Enable or disable `tailscale funnel` for our port.
///
/// `tailscale funnel --bg <port>` persists the mapping in the node's serve
/// config, so PalStudio only needs to toggle it — there is no state to keep
/// in sync while running, and disabling on shutdown is *not* wanted (the
/// operator chose the exposure; a PalStudio restart must not silently
/// re-close it).
pub fn set_funnel(enabled: bool, port: u16) -> Result<Option<String>, String> {
    if port == 0 {
        return Err("Funnel port must be between 1 and 65535".into());
    }
    let current = funnel_probe();
    if !current.available {
        return Err("Tailscale Funnel status is unavailable".into());
    }
    if current.on
        && !funnel_targets_are_owned(&current, port)
        && !funnel_targets_are_local(&current)
    {
        return Err(
            "refusing to change Funnel because an unrelated target is configured; remove it manually first"
                .into(),
        );
    }
    // `funnel reset` is the subcommand spelling; the old `--reset` flag
    // was dropped from the CLI. It clears the whole funnel config, with
    // no per-entry removal spelling stable across versions.
    let port_arg = port.to_string();
    let args: Vec<&str> = if enabled {
        // --yes skips the interactive confirmation newer CLIs would
        // otherwise demand (or fail on) without a terminal.
        vec!["funnel", "--bg", "--yes", &port_arg]
    } else {
        // `funnel reset` is the subcommand spelling; the old `--reset` flag
        // was dropped from the CLI. It clears the whole funnel config, with
        // no per-entry removal spelling stable across versions.
        vec!["funnel", "reset"]
    };
    let output = run_capture(&args);
    // Older CLI versions predate --yes; one retry without it keeps them
    // working instead of hard-failing the toggle.
    let output = match output {
        Ok(out) if out.status.success() => Ok(out),
        Ok(out) if enabled && unknown_flag(&out).is_some_and(|flag| flag == "yes") => {
            run_capture(&["funnel", "--bg", &port_arg])
        }
        other => other,
    };
    let output = output.map_err(|error| format!("could not run the tailscale CLI: {error}"))?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .trim()
    .to_owned();
    if !output.status.success() {
        return Err(if text.is_empty() {
            format!("tailscale funnel exited with {}", output.status)
        } else {
            text
        });
    }
    Ok((!text.is_empty()).then_some(text))
}

/// The `-flag` name an "flag provided but not defined" complaint mentions,
/// when the CLI's usage error names one.
fn unknown_flag(output: &std::process::Output) -> Option<String> {
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    text.split("flag provided but not defined:")
        .nth(1)?
        .trim()
        .split_whitespace()
        .next()
        .map(|flag| flag.trim_start_matches('-').to_owned())
}

fn run_capture(args: &[&str]) -> std::io::Result<std::process::Output> {
    let program = tailscale_program().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "trusted tailscale executable not found",
        )
    })?;
    let mut child = Command::new(program)
        .args(args)
        .env("LC_ALL", "C")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let deadline = Instant::now() + CLI_TIMEOUT;
    loop {
        if child.try_wait()?.is_some() {
            return child.wait_with_output();
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "tailscale command timed out",
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn parse_peer_ipv4(raw: &str) -> Vec<IpAddr> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return Vec::new();
    };
    value
        .get("Peer")
        .and_then(serde_json::Value::as_object)
        .into_iter()
        .flat_map(|peers| peers.values())
        .flat_map(|peer| {
            peer.get("TailscaleIPs")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flat_map(|ips| ips.iter())
        })
        .filter_map(serde_json::Value::as_str)
        .filter_map(|ip| ip.parse::<IpAddr>().ok())
        .filter(|ip| matches!(ip, IpAddr::V4(_)))
        .filter(|ip| {
            matches!(
                crate::policy::classify(*ip),
                crate::policy::PeerClass::Tailnet
            )
        })
        .collect()
}

fn status_json_is_logged_in(raw: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .and_then(|value| {
            value
                .get("BackendState")
                .and_then(serde_json::Value::as_str)
                .map(|state| state.eq_ignore_ascii_case("Running"))
        })
        .unwrap_or(false)
}

fn funnel_targets_are_owned(probe: &FunnelProbe, port: u16) -> bool {
    if !probe.on {
        return true;
    }
    !probe.targets.is_empty()
        && probe
            .targets
            .iter()
            .all(|target| funnel_target_port(target) == Some(port))
}

/// Returns true only when every parsed Funnel target is a literal loopback
/// HTTP endpoint for the requested port. Callers use this before deciding
/// that an existing mapping already points at PalStudio.
pub fn funnel_owns_local_port(probe: &FunnelProbe, port: u16) -> bool {
    funnel_targets_are_owned(probe, port)
}

fn funnel_targets_are_local(probe: &FunnelProbe) -> bool {
    !probe.targets.is_empty()
        && probe
            .targets
            .iter()
            .all(|target| funnel_target_port(target).is_some())
}

/// Returns true only when every parsed Funnel target is a literal loopback
/// HTTP endpoint. This is used when cleaning up a stale PalStudio mapping
/// whose old port is no longer present in the persisted configuration.
pub fn funnel_owns_local_targets(probe: &FunnelProbe) -> bool {
    funnel_targets_are_local(probe)
}

/// Only literal loopback HTTP targets are considered PalStudio-owned. A
/// hostname, credentials-bearing URL, or public address must never be treated
/// as safe to overwrite or remove during a port reconciliation.
fn funnel_target_port(target: &str) -> Option<u16> {
    let (scheme, rest) = target.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("http") {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next()?;
    if authority.contains('@') {
        return None;
    }
    let address = authority.parse::<SocketAddr>().ok()?;
    address.ip().is_loopback().then_some(address.port())
}

/// Structured live funnel state for the UI: `tailscale funnel status --json`
/// reports the serve config; `AllowFunnel` keys with `true` are the hostnames
/// published to the internet.
#[derive(Debug, Clone, Default)]
pub struct FunnelProbe {
    pub available: bool,
    pub on: bool,
    /// Published funnel URLs (e.g. `https://machine.tailnet.ts.net`).
    pub urls: Vec<String>,
    /// What each URL forwards to (e.g. `http://127.0.0.1:5174`).
    pub targets: Vec<String>,
    /// Verbatim `tailscale funnel status` text for display.
    pub text: Option<String>,
}

/// Ask the CLI what it thinks about the current funnel state, parsed into a
/// probe plus the human-readable text. Version-tolerant: a CLI without
/// `--json` support falls back to text with a crude on/off heuristic.
pub fn funnel_probe() -> FunnelProbe {
    let mut probe = FunnelProbe::default();
    let json = run_tailscale(&["funnel", "status", "--json"]);
    let text = run_tailscale(&["funnel", "status"]);
    probe.available = json.is_some() || text.is_some();
    if let Some(raw) = json {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(allow) = value.get("AllowFunnel").and_then(|v| v.as_object()) {
                for (host, allowed) in allow {
                    if allowed.as_bool().unwrap_or(false) {
                        probe.on = true;
                        // Keys carry the port ("host:443"); the bare HTTPS
                        // host is what an operator types in a browser.
                        let host = host.trim_end_matches(":443");
                        probe.urls.push(format!("https://{host}"));
                    }
                }
            }
            if let Some(web) = value.get("Web").and_then(|v| v.as_object()) {
                for (_, site) in web {
                    if let Some(handlers) = site.get("Handlers").and_then(|v| v.as_object()) {
                        for (_, handler) in handlers {
                            if let Some(proxy) = handler.get("Proxy").and_then(|v| v.as_str()) {
                                probe.targets.push(proxy.to_owned());
                            }
                        }
                    }
                }
            }
        }
    }
    if probe.urls.is_empty() && probe.targets.is_empty() {
        // No structured answer (older CLI): "Funnel on" appears in the text
        // exactly when something is published.
        probe.on = text.as_deref().is_some_and(|t| t.contains("Funnel on"));
    }
    probe.text = text.filter(|t| !t.trim().is_empty());
    probe
}

/// Ask the CLI what it thinks about the current funnel state.
pub fn funnel_status() -> Option<String> {
    run_tailscale(&["funnel", "status"])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_without_tailscale_reports_unavailable() {
        // CI and dev machines without tailscale must get a clean "absent".
        let status = detect();
        if !which_tailscale() {
            assert!(!status.available);
            assert!(status.ipv4.is_empty());
        }
    }

    #[test]
    fn funnel_ownership_requires_literal_loopback_and_the_requested_port() {
        let local = FunnelProbe {
            available: true,
            on: true,
            targets: vec!["http://127.0.0.1:5174".into()],
            ..FunnelProbe::default()
        };
        assert!(funnel_owns_local_port(&local, 5174));
        assert!(funnel_owns_local_targets(&local));
        assert!(!funnel_owns_local_port(&local, 5175));

        let public = FunnelProbe {
            available: true,
            on: true,
            targets: vec!["http://192.168.1.20:5174".into()],
            ..FunnelProbe::default()
        };
        assert!(!funnel_owns_local_port(&public, 5174));
        assert!(!funnel_owns_local_targets(&public));
    }

    fn which_tailscale() -> bool {
        run_capture(&["version"]).is_ok_and(|o| o.status.success())
    }
}
