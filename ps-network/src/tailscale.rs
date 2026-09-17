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
use std::net::IpAddr;
use std::process::Command;

/// What we could learn about tailscale on this machine.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TailscaleStatus {
    /// The CLI exists and answered.
    pub available: bool,
    /// This node's tailscale IPv4 addresses (CGNAT range).
    pub ipv4: Vec<IpAddr>,
    /// `tailscale status` says we're logged in to a tailnet.
    pub logged_in: bool,
    /// The last `funnel` subcommand output, for the UI to show verbatim.
    pub last_funnel_output: Option<String>,
}

fn run_tailscale(args: &[&str]) -> Option<String> {
    let output = Command::new("tailscale")
        .args(args)
        .env("LC_ALL", "C")
        .output()
        .ok()?;
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
    let logged_in = run_tailscale(&["status"]).is_some();
    TailscaleStatus {
        available: true,
        ipv4,
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
        Ok(out)
            if enabled && unknown_flag(&out).is_some_and(|flag| flag == "yes") =>
        {
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
    Command::new("tailscale")
        .args(args)
        .env("LC_ALL", "C")
        .output()
}

/// Structured live funnel state for the UI: `tailscale funnel status --json`
/// reports the serve config; `AllowFunnel` keys with `true` are the hostnames
/// published to the internet.
#[derive(Debug, Clone, Default)]
pub struct FunnelProbe {
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
    let text = run_tailscale(&["funnel", "status"]);
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

    fn which_tailscale() -> bool {
        Command::new("tailscale")
            .arg("version")
            .output()
            .is_ok_and(|o| o.status.success())
    }
}
