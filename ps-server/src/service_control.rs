//! In-tool switching between standalone and background service — the
//! "change it later from within the tool" affordance for the install-time
//! mode question. The Network page asks these endpoints to (un)register the
//! same service definitions the install scripts create, then the process
//! exits gracefully so the new arrangement takes over the port.
//!
//! Definitions this module must stay in step with: the systemd unit and
//! launchd plist written by install.sh, and the Scheduled Task written by
//! install.ps1 (name `PalStudio`, agent label `app.palstudio.server`).
use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;

use crate::AppState;

/// Set by the service definitions (systemd `Environment=`, launchd
/// `EnvironmentVariables`) so a service-run process can recognize itself.
/// The Windows task cannot carry per-task env vars, so Windows relies on
/// querying the task instead.
pub const SERVICE_MARKER_ENV: &str = "PALSTUDIO_SERVICE";

const SYSTEMD_UNIT_NAME: &str = "palstudio.service";
const LAUNCHD_LABEL: &str = "app.palstudio.server";
const LAUNCHD_PLIST: &str = "Library/LaunchAgents/app.palstudio.server.plist";
const WINDOWS_TASK: &str = "PalStudio";
const ADMIN_TOKEN_ENV: &str = "PS_ADMIN_TOKEN";
const SERVICE_USER: &str = "palstudio";
const SERVICE_GROUP: &str = "palstudio";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceManager {
    SystemdUser,
    SystemdSystem,
    Launchd,
    TaskScheduler,
}

impl ServiceManager {
    fn detect() -> Option<ServiceManager> {
        if cfg!(target_os = "linux") {
            if which_systemctl() {
                // Root cannot use `systemctl --user` without a session bus;
                // it manages the system instance instead.
                if am_root() {
                    Some(ServiceManager::SystemdSystem)
                } else {
                    Some(ServiceManager::SystemdUser)
                }
            } else {
                None
            }
        } else if cfg!(target_os = "macos") {
            which_version_ok("launchctl").then_some(ServiceManager::Launchd)
        } else if cfg!(target_os = "windows") {
            which_version_ok("schtasks.exe").then_some(ServiceManager::TaskScheduler)
        } else {
            None
        }
    }

    fn name(self) -> &'static str {
        match self {
            ServiceManager::SystemdUser | ServiceManager::SystemdSystem => "systemd",
            ServiceManager::Launchd => "launchd",
            ServiceManager::TaskScheduler => "Task Scheduler",
        }
    }
}

/// `systemctl` lives at /usr/bin or /bin and takes `--version` without a
/// running daemon, so this is a cheap presence probe.
fn which_systemctl() -> bool {
    which_version_ok("systemctl")
}

fn which_version_ok(program: &str) -> bool {
    std::process::Command::new(program)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn am_root() -> bool {
    #[cfg(unix)]
    {
        nix_uid() == 0
    }
    #[cfg(not(unix))]
    {
        false
    }
}

#[cfg(unix)]
fn nix_uid() -> u32 {
    // No libc dependency needed for one syscall: read it off /proc, and
    // fall back to the `id` utility.
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if let Some(rest) = line.strip_prefix("Uid:") {
                if let Some(first) = rest.split_whitespace().next() {
                    if let Ok(uid) = first.parse::<u32>() {
                        return uid;
                    }
                }
            }
        }
    }
    std::process::Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|out| String::from_utf8_lossy(&out.stdout).trim().parse().ok())
        .unwrap_or(u32::MAX)
}

/// Whether this process was started by a service definition we control.
pub fn running_as_service() -> bool {
    std::env::var(SERVICE_MARKER_ENV).is_ok_and(|v| v == "1")
}

/// Reconstructs this process's command line (`argv` sans argv[0]) so service
/// definitions restart exactly what the operator is running — except that a
/// hand-launched `webapp` (localhost-clamped tier) graduates to `serve`,
/// because a background service is by definition a hosted context.
fn current_args() -> Vec<String> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|first| first == "webapp") {
        args[0] = "serve".into();
    }
    args
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0u8, |difference, (a, b)| difference | (a ^ b))
        == 0
}

/// Service-control is a local privileged control plane. Loopback alone is not
/// an authorization boundary, so callers must also present either the
/// in-process PIN session or a separately provisioned high-entropy token.
fn control_authorized(
    peer: SocketAddr,
    headers: &HeaderMap,
    runtime: &crate::network::NetworkRuntime,
) -> bool {
    if !ps_network::canonical(peer.ip()).is_loopback() {
        return false;
    }
    if runtime.session_valid(headers) {
        return true;
    }
    let Some(expected) = std::env::var(ADMIN_TOKEN_ENV).ok() else {
        return false;
    };
    if expected.len() < 32 || expected.bytes().any(|byte| byte.is_ascii_control()) {
        return false;
    }
    let Some(value) = headers.get(axum::http::header::AUTHORIZATION) else {
        return false;
    };
    let Ok(value) = value.to_str() else {
        return false;
    };
    let Some(provided) = value.strip_prefix("Bearer ") else {
        return false;
    };
    constant_time_equal(provided.as_bytes(), expected.as_bytes())
}

fn desktop_binary_path() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let install_dir = exe.parent()?.parent()?;
    let name = if cfg!(windows) {
        "palstudio-desktop.exe"
    } else {
        "palstudio-desktop"
    };
    let path = install_dir.join("bin").join(name);
    path.is_file().then_some(path)
}

fn install_dir() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| {
            exe.parent()
                .and_then(|bin| bin.parent())
                .map(std::path::Path::to_path_buf)
        })
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(std::path::PathBuf::from)
}

// ---------------------------------------------------------------------------
// Service definition builders (pure — unit tested)
// ---------------------------------------------------------------------------

/// Quotes one token for a systemd `ExecStart=` line. Rejecting controls and
/// specifier characters is important: unit files are not shell scripts, but
/// systemd still interprets escapes and `%` specifiers.
fn systemd_quote(token: &str) -> Result<String, String> {
    if token.is_empty() {
        return Ok(String::from("\"\""));
    }
    if token.chars().any(char::is_control) || token.contains('%') {
        return Err("service arguments may not contain control characters or '%'".into());
    }
    let escaped = token.replace('\\', "\\\\").replace('"', "\\\"");
    Ok(format!("\"{escaped}\""))
}

fn secure_line_value(value: &str, kind: &str) -> Result<(), String> {
    if value.is_empty() || value.chars().any(char::is_control) || value.contains('%') {
        return Err(format!("invalid {kind} for service definition"));
    }
    Ok(())
}

pub fn systemd_unit_contents(
    exe: &str,
    args: &[String],
    working_dir: &str,
    user_unit: bool,
) -> Result<String, String> {
    systemd_unit_contents_with_data_dir(exe, args, working_dir, working_dir, user_unit)
}

pub fn systemd_unit_contents_with_data_dir(
    exe: &str,
    args: &[String],
    working_dir: &str,
    data_dir: &str,
    user_unit: bool,
) -> Result<String, String> {
    secure_line_value(exe, "executable path")?;
    secure_line_value(working_dir, "working directory")?;
    secure_line_value(data_dir, "data directory")?;
    let exec = std::iter::once(Ok(systemd_quote(exe)?))
        .chain(args.iter().map(|a| systemd_quote(a)))
        .collect::<Result<Vec<_>, _>>()?
        .join(" ");
    let quoted_dir = systemd_quote(working_dir)?;
    let quoted_data_dir = systemd_quote(data_dir)?;
    let install_target = if user_unit {
        "default.target"
    } else {
        "multi-user.target"
    };
    let user_lines = if user_unit {
        String::new()
    } else {
        format!("User={SERVICE_USER}\nGroup={SERVICE_GROUP}\n")
    };
    let protect_home = if user_unit { "read-only" } else { "true" };
    Ok(format!(
        "[Unit]\nDescription=PalStudio server\nAfter=network.target\n\n\
         [Service]\nEnvironment={SERVICE_MARKER_ENV}=1\n\
         {user_lines}\
         ExecStart={exec}\n\
         WorkingDirectory={quoted_dir}\n\
         ReadWritePaths={quoted_data_dir}\n\
         UMask=0077\n\
         NoNewPrivileges=true\nPrivateTmp=true\n\
         ProtectSystem=strict\nProtectHome={protect_home}\n\
         ProtectKernelTunables=true\nProtectKernelModules=true\n\
         ProtectControlGroups=true\n\
         RestrictSUIDSGID=true\nLockPersonality=true\n\
         PrivateDevices=true\nRestrictAddressFamilies=AF_UNIX AF_INET AF_INET6\n\
         CapabilityBoundingSet=\n\
         Restart=on-failure\nRestartSec=5\n\n\
         [Install]\nWantedBy={install_target}\n",
        exec = exec,
        user_lines = user_lines,
        quoted_data_dir = quoted_data_dir,
        protect_home = protect_home,
        install_target = install_target,
    ))
}

fn xml_escape(value: &str) -> Result<String, String> {
    if value.chars().any(char::is_control) {
        return Err("service XML values may not contain control characters".into());
    }
    Ok(value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;"))
}

pub fn launchd_plist_contents(
    exe: &str,
    args: &[String],
    working_dir: &str,
) -> Result<String, String> {
    let mut program_args = format!("    <string>{}</string>\n", xml_escape(exe)?);
    for arg in args {
        program_args.push_str(&format!("    <string>{}</string>\n", xml_escape(arg)?));
    }
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n<dict>\n\
         \x20 <key>Label</key>\n\x20 <string>{LAUNCHD_LABEL}</string>\n\
         \x20 <key>ProgramArguments</key>\n\x20 <array>\n{program_args}\x20 </array>\n\
         \x20 <key>WorkingDirectory</key>\n\x20 <string>{working_dir}</string>\n\
         \x20 <key>EnvironmentVariables</key>\n\x20 <dict>\n\
         \x20   <key>{SERVICE_MARKER_ENV}</key>\n\x20   <string>1</string>\n\
         \x20 </dict>\n\
         \x20 <key>RunAtLoad</key>\n\x20 <true/>\n\
         \x20 <key>KeepAlive</key>\n\x20 <true/>\n\
         \x20 <key>StandardOutPath</key>\n\x20 <string>{log_path}</string>\n\
         \x20 <key>StandardErrorPath</key>\n\x20 <string>{log_path}</string>\n\
         </dict>\n</plist>\n",
        working_dir = xml_escape(working_dir)?,
        log_path = xml_escape(&format!("{working_dir}/palstudio.log"))?,
    ))
}

/// One PowerShell-single-quoted literal.
fn ps_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn windows_quote_arg(value: &str) -> Result<String, String> {
    if value.chars().any(char::is_control) {
        return Err("Task Scheduler arguments may not contain control characters".into());
    }
    if value.is_empty() {
        return Ok("\"\"".into());
    }
    if !value.chars().any(|ch| ch.is_whitespace() || ch == '"') {
        return Ok(value.to_owned());
    }
    let mut out = String::from("\"");
    let mut backslashes = 0usize;
    for ch in value.chars() {
        if ch == '\\' {
            backslashes += 1;
        } else if ch == '"' {
            out.push_str(&"\\".repeat(backslashes * 2 + 1));
            out.push('"');
            backslashes = 0;
        } else {
            out.push_str(&"\\".repeat(backslashes));
            out.push(ch);
            backslashes = 0;
        }
    }
    out.push_str(&"\\".repeat(backslashes * 2));
    out.push('"');
    Ok(out)
}

pub fn windows_register_command(
    exe: &str,
    args: &[String],
    working_dir: &str,
) -> Result<Vec<String>, String> {
    let argument = args
        .iter()
        .map(|a| windows_quote_arg(a))
        .collect::<Result<Vec<_>, _>>()?
        .join(" ");
    secure_line_value(exe, "executable path")?;
    secure_line_value(working_dir, "working directory")?;
    let script = format!(
        "$action = New-ScheduledTaskAction -Execute {exe} -Argument {argument} -WorkingDirectory {working_dir}; \
         $trigger = New-ScheduledTaskTrigger -AtLogOn; \
         $settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries \
           -RestartCount 3 -RestartInterval (New-TimeSpan -Minutes 1) -MultipleInstances IgnoreNew \
           -ExecutionTimeLimit (New-TimeSpan -Days 3650) -StartWhenAvailable; \
         Register-ScheduledTask -TaskName {task} -Action $action -Trigger $trigger -Settings $settings -Force | Out-Null; \
         Start-ScheduledTask -TaskName {task}",
        exe = ps_quote(exe),
        argument = ps_quote(&argument),
        working_dir = ps_quote(working_dir),
        task = ps_quote(WINDOWS_TASK),
    );
    Ok(vec![
        "-NoProfile".into(),
        "-NonInteractive".into(),
        "-Command".into(),
        script,
    ])
}

fn atomic_write(path: &std::path::Path, contents: &str, mode: u32) -> Result<(), String> {
    if let Ok(metadata) = std::fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return Err(format!(
                "refusing to replace non-regular file {}",
                path.display()
            ));
        }
    }
    let parent = path
        .parent()
        .ok_or_else(|| format!("service path has no parent: {}", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "service-definition".into());
    let temp = parent.join(format!(".{file_name}.tmp-{}", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|error| format!("could not create {}: {error}", temp.display()))?;
    use std::io::Write;
    if let Err(error) = file
        .write_all(contents.as_bytes())
        .and_then(|_| file.sync_all())
    {
        let _ = std::fs::remove_file(&temp);
        return Err(format!("could not write {}: {error}", temp.display()));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(mode))
            .map_err(|error| format!("could not set permissions on {}: {error}", temp.display()))?;
    }
    if let Err(error) = std::fs::rename(&temp, path) {
        let _ = std::fs::remove_file(&temp);
        return Err(format!("could not install {}: {error}", path.display()));
    }
    Ok(())
}

fn service_account_secure() -> bool {
    if !cfg!(target_os = "linux") {
        return true;
    }
    let Ok(output) = std::process::Command::new("getent")
        .args(["passwd", SERVICE_USER])
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let fields = String::from_utf8_lossy(&output.stdout)
        .trim()
        .split(':')
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if fields.len() < 7 || fields[0] != SERVICE_USER || fields[2] == "0" {
        return false;
    }
    if !matches!(fields[6].as_str(), "/usr/sbin/nologin" | "/sbin/nologin") {
        return false;
    }
    let Ok(group) = std::process::Command::new("getent")
        .args(["group", SERVICE_GROUP])
        .output()
    else {
        return false;
    };
    if !group.status.success() {
        return false;
    }
    String::from_utf8_lossy(&group.stdout)
        .trim()
        .split(':')
        .nth(2)
        .and_then(|gid| gid.parse::<u32>().ok())
        .is_some_and(|gid| gid != 0)
}

fn data_dir_from_args(args: &[String], fallback: &str) -> String {
    args.windows(2)
        .find(|pair| pair[0] == "--db")
        .and_then(|pair| std::path::Path::new(&pair[1]).parent())
        .map(|path| path.to_string_lossy().into_owned())
        .filter(|path| !path.is_empty())
        .unwrap_or_else(|| fallback.to_owned())
}

// ---------------------------------------------------------------------------
// Install / uninstall (blocking CLI work)
// ---------------------------------------------------------------------------

fn run(argv: &[&str]) -> Result<(), String> {
    let output = std::process::Command::new(argv[0])
        .args(&argv[1..])
        .output()
        .map_err(|e| format!("could not run {}: {e}", argv[0]))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{}: {}",
            argv[0],
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn service_installed(manager: Option<ServiceManager>) -> bool {
    match manager {
        Some(ServiceManager::SystemdUser) => home_dir()
            .map(|home| {
                std::path::Path::new(&home)
                    .join(".config/systemd/user")
                    .join(SYSTEMD_UNIT_NAME)
                    .is_file()
            })
            .unwrap_or(false),
        Some(ServiceManager::SystemdSystem) => std::path::Path::new("/etc/systemd/system")
            .join(SYSTEMD_UNIT_NAME)
            .is_file(),
        Some(ServiceManager::Launchd) => home_dir()
            .map(|home| home.join(LAUNCHD_PLIST).is_file())
            .unwrap_or(false),
        Some(ServiceManager::TaskScheduler) => std::process::Command::new("schtasks.exe")
            .args(["/Query", "/TN", WINDOWS_TASK])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success()),
        None => false,
    }
}

/// Registers the service definition (from this process's own command line)
/// and starts it. The caller exits shortly after, freeing the port for the
/// service's restart attempts.
fn install_service(manager: ServiceManager) -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("could not determine this binary's path: {e}"))?
        .to_string_lossy()
        .into_owned();
    let args = current_args();
    let workdir = install_dir().to_string_lossy().into_owned();
    let data_dir = data_dir_from_args(&args, &workdir);

    match manager {
        ServiceManager::SystemdUser => {
            let Some(home) = home_dir() else {
                return Err("HOME is not set".into());
            };
            let unit_dir = home.join(".config/systemd/user");
            let unit_path = unit_dir.join(SYSTEMD_UNIT_NAME);
            let unit = systemd_unit_contents_with_data_dir(&exe, &args, &workdir, &data_dir, true)?;
            atomic_write(&unit_path, &unit, 0o600)?;
            run(&["systemctl", "--user", "daemon-reload"])?;
            // --now starts immediately; while this process still holds the
            // port the unit enters its restart backoff and takes over the
            // moment we exit.
            run(&["systemctl", "--user", "enable", "--now", SYSTEMD_UNIT_NAME])
        }
        ServiceManager::SystemdSystem => {
            if !am_root() {
                return Err("system service installation requires root".into());
            }
            if !service_account_secure() {
                return Err(format!(
                    "required locked service account '{}' is missing; install-server.sh must create it first",
                    SERVICE_USER
                ));
            }
            let unit_path = std::path::Path::new("/etc/systemd/system").join(SYSTEMD_UNIT_NAME);
            let unit =
                systemd_unit_contents_with_data_dir(&exe, &args, &workdir, &data_dir, false)?;
            atomic_write(&unit_path, &unit, 0o644)?;
            run(&["systemctl", "daemon-reload"])?;
            run(&["systemctl", "enable", "--now", SYSTEMD_UNIT_NAME])
        }
        ServiceManager::Launchd => {
            let Some(home) = home_dir() else {
                return Err("HOME is not set".into());
            };
            let plist_path = home.join(LAUNCHD_PLIST);
            let plist = launchd_plist_contents(&exe, &args, &workdir)?;
            atomic_write(&plist_path, &plist, 0o600)?;
            // Out with any previous agent, in with the new definition.
            let uid = nix_uid().to_string();
            let _ = run(&[
                "launchctl",
                "bootout",
                &format!("gui/{uid}"),
                &plist_path.to_string_lossy(),
            ]);
            run(&[
                "launchctl",
                "bootstrap",
                &format!("gui/{uid}"),
                &plist_path.to_string_lossy(),
            ])
        }
        ServiceManager::TaskScheduler => {
            let command = windows_register_command(&exe, &args, &workdir)?;
            let referenced: Vec<&str> = command.iter().map(String::as_str).collect();
            let mut argv = vec!["powershell.exe"];
            argv.extend(referenced);
            run(&argv)
        }
    }
}

/// Removes the service definition WITHOUT stopping this process: the
/// handlers must be able to answer the HTTP request and exit gracefully
/// themselves. A systemd `disable` (no `--now`) keeps the started unit
/// running until its process exits — and a clean exit does not trip
/// `Restart=on-failure`; `launchctl disable` likewise only affects future
/// loads. The caller's delayed `request_exit` ends the job.
fn uninstall_service(manager: ServiceManager) -> Result<(), String> {
    match manager {
        ServiceManager::SystemdUser => {
            let Some(home) = home_dir() else {
                return Err("HOME is not set".into());
            };
            let unit_path = home.join(".config/systemd/user").join(SYSTEMD_UNIT_NAME);
            if unit_path.exists() {
                run(&["systemctl", "--user", "disable", SYSTEMD_UNIT_NAME])?;
                std::fs::remove_file(&unit_path).map_err(|e| e.to_string())?;
                run(&["systemctl", "--user", "daemon-reload"])?;
            }
            Ok(())
        }
        ServiceManager::SystemdSystem => {
            if !am_root() {
                return Err("system service removal requires root".into());
            }
            let unit_path = std::path::Path::new("/etc/systemd/system").join(SYSTEMD_UNIT_NAME);
            if unit_path.exists() {
                run(&["systemctl", "disable", SYSTEMD_UNIT_NAME])?;
                std::fs::remove_file(&unit_path).map_err(|e| e.to_string())?;
                run(&["systemctl", "daemon-reload"])?;
            }
            Ok(())
        }
        ServiceManager::Launchd => {
            let Some(home) = home_dir() else {
                return Err("HOME is not set".into());
            };
            let plist_path = home.join(LAUNCHD_PLIST);
            let uid = nix_uid().to_string();
            // Disable for future logins; the running instance ends when the
            // caller exits, and booting the (removed) plist again is then
            // impossible.
            run(&[
                "launchctl",
                "disable",
                &format!("gui/{uid}/{LAUNCHD_LABEL}"),
            ])?;
            if plist_path.exists() {
                std::fs::remove_file(&plist_path).map_err(|e| e.to_string())?;
            }
            Ok(())
        }
        ServiceManager::TaskScheduler => run(&[
            "powershell.exe",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &format!(
                "Unregister-ScheduledTask -TaskName {} -Confirm:$false",
                ps_quote(WINDOWS_TASK)
            ),
        ]),
    }
}

// ---------------------------------------------------------------------------
// REST
// ---------------------------------------------------------------------------

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/runtime", get(get_runtime).post(set_runtime))
}

async fn get_runtime(
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    axum::Extension(runtime): axum::Extension<Arc<crate::network::NetworkRuntime>>,
) -> Response {
    if !control_authorized(peer, &headers, &runtime) {
        return error_response(
            StatusCode::FORBIDDEN,
            "service control requires local authentication",
        );
    }
    let manager = ServiceManager::detect();
    let installed = tokio::task::spawn_blocking(move || service_installed(manager))
        .await
        .unwrap_or(false);
    (
        StatusCode::OK,
        Json(json!({
            "tier": runtime.tier().as_str(),
            "running_as_service": running_as_service(),
            "service_supported": manager.is_some(),
            "service_manager": manager.map(|m| m.name()),
            "service_installed": installed,
            "desktop_available": desktop_binary_path().is_some(),
        })),
    )
        .into_response()
}

#[derive(serde::Deserialize)]
struct RuntimeSwitch {
    mode: String,
}

async fn set_runtime(
    State(app): State<Arc<AppState>>,
    axum::Extension(runtime): axum::Extension<Arc<crate::network::NetworkRuntime>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(request): Json<RuntimeSwitch>,
) -> Response {
    if !control_authorized(peer, &headers, &runtime) {
        return error_response(
            StatusCode::FORBIDDEN,
            "service control requires local authentication",
        );
    }
    if app.config.desktop_mode {
        return error_response(
            StatusCode::FORBIDDEN,
            "the desktop app is standalone by design; run the install script to set up a service",
        );
    }
    // Runtime mode is a network setting: a locked public websuite run
    // cannot (un)register services any more than it can rebind the policy.
    if runtime.edits_locked() {
        return error_response(
            StatusCode::FORBIDDEN,
            "network settings are locked: this instance is hosted publicly without \
             --allow-network — restart `palstudio websuite` with --allow-network to switch \
             the runtime mode",
        );
    }
    let Some(manager) = ServiceManager::detect() else {
        return error_response(
            StatusCode::CONFLICT,
            "no supported service manager (systemd/launchd/Task Scheduler) on this machine",
        );
    };

    let want_service = match request.mode.as_str() {
        "service" => true,
        "standalone" => false,
        other => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("unknown mode '{other}'; use service|standalone"),
            )
        }
    };

    let outcome = tokio::task::spawn_blocking(move || {
        if want_service {
            install_service(manager)
        } else {
            uninstall_service(manager)
        }
    })
    .await
    .unwrap_or_else(|e| Err(format!("switch task failed: {e}")));

    match outcome {
        Ok(()) => {
            // Let the response flush, then hand the port to the new
            // arrangement: the service's restart backoff waits for our exit,
            // and standalone simply stays down until the user launches it.
            let runtime = Arc::clone(&runtime);
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                runtime.request_exit();
            });
            let message = if want_service {
                "background service installed and started; this instance exits in a moment and the service takes the port".to_owned()
            } else {
                "background service removed; run `palstudio` whenever you want PalStudio".to_owned()
            };
            (
                StatusCode::OK,
                Json(json!({ "ok": true, "message": message })),
            )
                .into_response()
        }
        Err(error) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("could not switch runtime mode: {error}"),
        ),
    }
}

fn error_response(status: StatusCode, message: &str) -> Response {
    crate::network::error_response(status, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn systemd_unit_quotes_paths_and_carries_the_marker() {
        let unit = systemd_unit_contents(
            "/opt/pal studio/bin/palstudio",
            &[
                "serve".into(),
                "--ui-dir".into(),
                "/opt/pal studio/ui".into(),
            ],
            "/opt/pal studio",
            true,
        )
        .unwrap();
        assert!(unit.contains("Environment=PALSTUDIO_SERVICE=1"));
        assert!(unit.contains(
            "ExecStart=\"/opt/pal studio/bin/palstudio\" \"serve\" \"--ui-dir\" \"/opt/pal studio/ui\""
        ));
        assert!(unit.contains("WorkingDirectory=\"/opt/pal studio\""));
        assert!(unit.contains("WantedBy=default.target"));
        let system =
            systemd_unit_contents("/opt/p/bin/palstudio", &["serve".into()], "/opt/p", false)
                .unwrap();
        assert!(system.contains("WantedBy=multi-user.target"));
        assert!(system.contains("User=palstudio"));
        assert!(system.contains("ProtectSystem=strict"));
        assert!(system.contains("CapabilityBoundingSet="));
        assert!(!system.contains("PS_PIN"));
    }

    #[test]
    fn launchd_plist_carries_marker_and_escapes_values() {
        let plist = launchd_plist_contents(
            "/usr/local/bin/palstudio",
            &["serve".into(), "--port".into(), "5174".into()],
            "/usr/local/share/palstudio",
        )
        .unwrap();
        assert!(plist.contains("<string>app.palstudio.server</string>"));
        assert!(plist.contains("<key>PALSTUDIO_SERVICE</key>"));
        assert!(plist.contains("<string>serve</string>"));
        assert!(plist.contains("<string>5174</string>"));
        let hostile = launchd_plist_contents(
            "/x&a<y>/palstudio",
            &["--name".into(), "<script>".into()],
            "/x&a<y>",
        )
        .unwrap();
        assert!(hostile.contains("<string>/x&amp;a&lt;y&gt;/palstudio</string>"));
        assert!(hostile.contains("<string>&lt;script&gt;</string>"));
        assert!(!hostile.contains("<script>"));
    }

    #[test]
    fn windows_register_command_builds_a_sane_script() {
        let command = windows_register_command(
            r"C:\Program Files\PalStudio\bin\palstudio.exe",
            &[
                "serve".into(),
                "--ui-dir".into(),
                r"C:\Program Files\PalStudio\ui".into(),
            ],
            r"C:\Program Files\PalStudio",
        )
        .unwrap();
        assert_eq!(command[0], "-NoProfile");
        let script = &command[3];
        assert!(script.contains(
            "New-ScheduledTaskAction -Execute 'C:\\Program Files\\PalStudio\\bin\\palstudio.exe'"
        ));
        assert!(script.contains("-Argument 'serve --ui-dir \"C:\\Program Files\\PalStudio\\ui\"'"));
        assert!(script.contains("Register-ScheduledTask -TaskName 'PalStudio'"));
        assert!(script.contains("Start-ScheduledTask -TaskName 'PalStudio'"));
    }

    #[test]
    fn ps_quote_doubles_single_quotes() {
        assert_eq!(ps_quote("it's"), "'it''s'");
        assert_eq!(ps_quote("plain"), "'plain'");
    }
}
