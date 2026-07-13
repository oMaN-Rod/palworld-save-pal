//! Native PalServer.exe lifecycle: steamcmd install, process spawn/stop, and
//! status/stats via sysinfo.
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use chrono::TimeZone;
use psp_db::servers::ServerRecord;
use serde_json::Value;
use sysinfo::{Pid, ProcessStatus, System};

use super::palworld_api::PalworldApiClient;
use super::{
    native_config, native_mods, python_isoformat, round_to, ServerProcessStatus, ServiceError,
};

pub const STEAMCMD_ZIP_URL: &str = "https://steamcdn-a.akamaihd.net/client/installer/steamcmd.zip";
pub const PALWORLD_APP_ID: &str = "2394010";

/// One shared `System` so successive polls have a previous sample to compute CPU
/// deltas against; the first poll for a pid therefore reports ~0.
fn system() -> &'static Mutex<System> {
    static SYSTEM: OnceLock<Mutex<System>> = OnceLock::new();
    SYSTEM.get_or_init(|| Mutex::new(System::new()))
}

pub fn default_steamcmd_dir() -> PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    Path::new(&home).join("SteamCMD")
}

/// PATH lookup, then <drive>:\SteamCMD and <drive>:\steamcmd on every drive.
pub fn find_steamcmd() -> Option<String> {
    if let Ok(path_variable) = std::env::var("PATH") {
        for directory in std::env::split_paths(&path_variable) {
            let candidate = directory.join("steamcmd.exe");
            if candidate.exists() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }
    for drive_letter in "CDEFGHIJKLMNOPQRSTUVWXYZ".chars() {
        for directory_name in ["SteamCMD", "steamcmd"] {
            let candidate = PathBuf::from(format!("{drive_letter}:\\"))
                .join(directory_name)
                .join("steamcmd.exe");
            if candidate.exists() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }
    None
}

pub async fn ensure_steamcmd(steamcmd_dir: &Path) -> Result<PathBuf, ServiceError> {
    let steamcmd_exe = steamcmd_dir.join("steamcmd.exe");
    if steamcmd_exe.exists() {
        return Ok(steamcmd_exe);
    }
    std::fs::create_dir_all(steamcmd_dir)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|error| ServiceError::Http(error.to_string()))?;
    let response = client
        .get(STEAMCMD_ZIP_URL)
        .send()
        .await
        .map_err(|error| ServiceError::Http(error.to_string()))?
        .error_for_status()
        .map_err(|error| ServiceError::Http(error.to_string()))?;
    let zip_bytes = response
        .bytes()
        .await
        .map_err(|error| ServiceError::Http(error.to_string()))?;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes.to_vec()))
        .map_err(|error| ServiceError::Other(format!("invalid steamcmd zip: {error}")))?;
    archive
        .extract(steamcmd_dir)
        .map_err(|error| ServiceError::Other(format!("steamcmd extract failed: {error}")))?;
    Ok(steamcmd_exe)
}

pub fn steamcmd_install_args(install_dir: &str) -> Vec<String> {
    vec![
        "+login".to_string(),
        "anonymous".to_string(),
        "+force_install_dir".to_string(),
        install_dir.to_string(),
        "+app_update".to_string(),
        PALWORLD_APP_ID.to_string(),
        "validate".to_string(),
        "+quit".to_string(),
    ]
}

/// SteamCMD's exit codes are unreliable, so success is judged by PalServer.exe
/// existing afterwards. The 1800 s cap covers a cold full download of the app; on
/// timeout the child is killed rather than left running detached.
pub async fn install_server(steamcmd_exe: &str, install_dir: &str) -> bool {
    if std::fs::create_dir_all(install_dir).is_err() {
        return false;
    }
    let spawned = tokio::process::Command::new(steamcmd_exe)
        .args(steamcmd_install_args(install_dir))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    if let Ok(mut child) = spawned {
        let wait_result =
            tokio::time::timeout(std::time::Duration::from_secs(1800), child.wait()).await;
        if wait_result.is_err() {
            // child.kill() awaits the process's actual exit, so this also reaps it.
            let _ = child.kill().await;
        }
    }
    Path::new(install_dir).join("PalServer.exe").exists()
}

/// Siblings of install_path first, then <steamcmd_dir>/steamapps/common/PalServer.
pub fn find_existing_server(steamcmd_path: &str, install_path: &str) -> Option<String> {
    if !install_path.is_empty() {
        if let Some(base_dir) = Path::new(install_path).parent() {
            if let Ok(entries) = std::fs::read_dir(base_dir) {
                for entry in entries.flatten() {
                    let candidate = entry.path();
                    if candidate == Path::new(install_path) {
                        continue;
                    }
                    if candidate.join("PalServer.exe").exists() {
                        return Some(candidate.to_string_lossy().to_string());
                    }
                }
            }
        }
    }
    if !steamcmd_path.is_empty() {
        let steamcmd_dir = if steamcmd_path.ends_with(".exe") {
            Path::new(steamcmd_path).parent().map(Path::to_path_buf)
        } else {
            Some(PathBuf::from(steamcmd_path))
        };
        if let Some(directory) = steamcmd_dir {
            let default_install = directory.join("steamapps").join("common").join("PalServer");
            if default_install.join("PalServer.exe").exists() {
                return Some(default_install.to_string_lossy().to_string());
            }
        }
    }
    None
}

/// Clones an existing install's binaries and content. `Saved` (world data,
/// config) and `steamapps` (install metadata) are skipped so the new server does
/// not inherit the source's saves or Steam bookkeeping.
pub fn copy_server_base(source: &Path, dest: &Path) -> std::io::Result<()> {
    const EXCLUDED_DIRS: [&str; 2] = ["Saved", "steamapps"];
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let name = entry.file_name();
        let source_path = entry.path();
        let dest_path = dest.join(&name);
        if source_path.is_dir() {
            if EXCLUDED_DIRS.contains(&name.to_string_lossy().as_ref()) {
                continue;
            }
            copy_server_base(&source_path, &dest_path)?;
        } else {
            std::fs::copy(&source_path, &dest_path)?;
        }
    }
    Ok(())
}

/// Installs only when PalServer.exe is absent: copy from an existing install when
/// one is given, otherwise steamcmd. Writes PalWorldSettings.ini either way.
pub async fn create_native_server(record: &ServerRecord, source_server_path: Option<&str>) -> bool {
    let install_path = record.install_path.clone();
    if install_path.is_empty() {
        return false;
    }
    let exe_present = Path::new(&install_path).join("PalServer.exe").exists();
    let usable_source = source_server_path
        .filter(|source| Path::new(source).join("PalServer.exe").exists())
        .map(str::to_string);
    if exe_present {
        // already installed, nothing to copy
    } else if let Some(source) = usable_source {
        let dest = install_path.clone();
        let copy_result = tokio::task::spawn_blocking(move || {
            copy_server_base(Path::new(&source), Path::new(&dest))
        })
        .await;
        if !matches!(copy_result, Ok(Ok(()))) {
            return false;
        }
    } else {
        if record.steamcmd_path.is_empty() {
            return false;
        }
        if !install_server(&record.steamcmd_path, &install_path).await {
            return false;
        }
    }
    if std::fs::create_dir_all(native_config::config_dir(&install_path)).is_err() {
        return false;
    }
    native_config::write_palworld_settings(record).is_ok()
}

/// Rewrites the ini files first, since PalServer.exe only reads them at launch.
/// On Windows the child gets its own process group so a Ctrl-C to psp-server does
/// not take the game server down with it.
pub fn start_server_process(record: &ServerRecord) -> Option<u32> {
    let exe_path = Path::new(&record.install_path).join("PalServer.exe");
    if !exe_path.exists() {
        return None;
    }
    if native_config::write_palworld_settings(record).is_err() {
        return None;
    }
    if native_mods::ensure_mod_settings(record).is_err() {
        return None;
    }
    let mut command = std::process::Command::new(&exe_path);
    command
        .arg(format!("-port={}", record.game_port))
        .arg("-useperfthreads")
        .arg("-NoAsyncLoadingThread")
        .arg("-UseMultithreadForDS");
    for extra_arg in record.launch_args.split_whitespace() {
        command.arg(extra_arg);
    }
    command
        .current_dir(&record.install_path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        command.creation_flags(CREATE_NEW_PROCESS_GROUP);
    }
    command.spawn().ok().map(|child| child.id())
}

fn pid_alive(pid: u32) -> bool {
    let mut system = system().lock().unwrap();
    system.refresh_processes(
        sysinfo::ProcessesToUpdate::Some(&[Pid::from_u32(pid)]),
        true,
    );
    system
        .process(Pid::from_u32(pid))
        .map(|process| !matches!(process.status(), ProcessStatus::Zombie))
        .unwrap_or(false)
}

/// `Process::kill()` returning `true` only means the signal was delivered, not
/// that the process died, so poll for up to 10 s for the pid to actually vanish
/// before reporting failure.
async fn force_kill(pid: u32) -> bool {
    {
        let mut system = system().lock().unwrap();
        system.refresh_processes(
            sysinfo::ProcessesToUpdate::Some(&[Pid::from_u32(pid)]),
            true,
        );
        match system.process(Pid::from_u32(pid)) {
            Some(process) => {
                process.kill();
            }
            None => return true, // already gone
        }
    }
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if !pid_alive(pid) {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
}

/// Asks the server to shut itself down over the REST API first so it flushes the
/// world save; the 30 s wait covers that save. Falls back to a force kill.
pub async fn stop_server_process(record: &ServerRecord, api: &PalworldApiClient) -> bool {
    let Some(pid) = record.pid else {
        return false;
    };
    let pid = pid as u32;
    let shutdown_payload = serde_json::json!({"waittime": 5, "message": "Server shutting down..."});
    let shutdown_sent = api
        .rest_api_call(
            "127.0.0.1",
            record.rest_api_port as u16,
            &record.admin_password,
            "shutdown",
            "POST",
            Some(&shutdown_payload),
        )
        .await
        .is_ok();
    if shutdown_sent {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
        while tokio::time::Instant::now() < deadline {
            if !pid_alive(pid) {
                return true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }
    force_kill(pid).await
}

/// Running unless the pid is gone or a zombie; `started_at` is the process create
/// time in local time.
pub fn process_status(pid: Option<i64>) -> ServerProcessStatus {
    let Some(pid) = pid else {
        return ServerProcessStatus::exited();
    };
    let mut system = system().lock().unwrap();
    let target = Pid::from_u32(pid as u32);
    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[target]), true);
    if let Some(process) = system.process(target) {
        if !matches!(process.status(), ProcessStatus::Zombie) {
            let started_at = chrono::Local
                .timestamp_opt(process.start_time() as i64, 0)
                .single()
                .map(|start| python_isoformat(start.naive_local()));
            return ServerProcessStatus {
                status: "running".to_string(),
                running: true,
                started_at,
                health: None,
            };
        }
    }
    ServerProcessStatus::exited()
}

/// Aggregated over the whole process tree, because PalServer.exe is a launcher
/// that spawns the actual game server as a child. Native servers cannot report
/// per-process network I/O, so the net fields are always integer 0.
pub fn process_stats(pid: Option<i64>) -> Option<Value> {
    let root_pid = Pid::from_u32(pid? as u32);
    let mut system = system().lock().unwrap();
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    system.refresh_memory();
    system.process(root_pid)?;

    let mut tree: Vec<Pid> = vec![root_pid];
    let mut added = true;
    while added {
        added = false;
        for (candidate_pid, process) in system.processes() {
            if tree.contains(candidate_pid) {
                continue;
            }
            if let Some(parent) = process.parent() {
                if tree.contains(&parent) {
                    tree.push(*candidate_pid);
                    added = true;
                }
            }
        }
    }

    let mut total_cpu = 0.0f64;
    let mut total_memory_bytes = 0u64;
    let mut total_disk_read = 0u64;
    let mut total_disk_write = 0u64;
    for member in &tree {
        if let Some(process) = system.process(*member) {
            total_cpu += process.cpu_usage() as f64;
            total_memory_bytes += process.memory();
            let disk = process.disk_usage();
            total_disk_read += disk.total_read_bytes;
            total_disk_write += disk.total_written_bytes;
        }
    }
    let total_system_memory = system.total_memory();
    let mem_percent = if total_system_memory > 0 {
        (total_memory_bytes as f64 / total_system_memory as f64) * 100.0
    } else {
        0.0
    };

    const MB: f64 = 1024.0 * 1024.0;
    Some(serde_json::json!({
        "cpu_percent": round_to(total_cpu, 2),
        "mem_usage_mb": round_to(total_memory_bytes as f64 / MB, 1),
        "mem_limit_mb": round_to(total_system_memory as f64 / MB, 1),
        "mem_percent": round_to(mem_percent, 1),
        "net_rx_mb": 0,
        "net_tx_mb": 0,
        "disk_read_mb": round_to(total_disk_read as f64 / MB, 2),
        "disk_write_mb": round_to(total_disk_write as f64 / MB, 2)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steamcmd_install_args_match_python_command() {
        assert_eq!(
            steamcmd_install_args("D:/servers/world1"),
            vec![
                "+login".to_string(),
                "anonymous".to_string(),
                "+force_install_dir".to_string(),
                "D:/servers/world1".to_string(),
                "+app_update".to_string(),
                "2394010".to_string(),
                "validate".to_string(),
                "+quit".to_string(),
            ]
        );
    }

    #[test]
    fn process_status_without_pid_is_exited() {
        assert_eq!(
            process_status(None),
            crate::services::ServerProcessStatus::exited()
        );
    }

    #[test]
    fn process_status_for_dead_pid_is_exited() {
        // PIDs this large are never real on Windows or Linux test machines.
        assert_eq!(
            process_status(Some(0x7FFF_FF00)),
            crate::services::ServerProcessStatus::exited()
        );
    }

    #[test]
    fn process_status_for_this_test_process_is_running() {
        let own_pid = std::process::id() as i64;
        let status = process_status(Some(own_pid));
        assert_eq!(status.status, "running");
        assert!(status.running);
        assert!(status.started_at.is_some());
    }

    #[test]
    fn process_stats_without_pid_is_none() {
        assert!(process_stats(None).is_none());
        assert!(process_stats(Some(0x7FFF_FF00)).is_none());
    }

    #[test]
    fn process_stats_for_this_process_has_native_shape() {
        let stats = process_stats(Some(std::process::id() as i64)).unwrap();
        let keys: Vec<&str> = stats
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            vec![
                "cpu_percent",
                "mem_usage_mb",
                "mem_limit_mb",
                "mem_percent",
                "net_rx_mb",
                "net_tx_mb",
                "disk_read_mb",
                "disk_write_mb"
            ]
        );
        // Native servers report integer 0 for network I/O.
        assert_eq!(stats["net_rx_mb"], serde_json::json!(0));
        assert_eq!(stats["net_tx_mb"], serde_json::json!(0));
        assert!(stats["mem_usage_mb"].as_f64().unwrap() > 0.0);
    }

    #[test]
    fn copy_server_base_skips_saved_and_steamapps() {
        let scratch = tempfile::tempdir().unwrap();
        let source = scratch.path().join("source");
        std::fs::create_dir_all(source.join("Pal").join("Binaries")).unwrap();
        std::fs::write(source.join("PalServer.exe"), b"exe").unwrap();
        std::fs::create_dir_all(source.join("Pal").join("Saved").join("SaveGames")).unwrap();
        std::fs::create_dir_all(source.join("steamapps")).unwrap();
        let dest = scratch.path().join("dest");
        copy_server_base(&source, &dest).unwrap();
        assert!(dest.join("PalServer.exe").exists());
        assert!(dest.join("Pal").join("Binaries").exists());
        assert!(!dest.join("Pal").join("Saved").exists());
        assert!(!dest.join("steamapps").exists());
    }

    #[test]
    fn find_existing_server_checks_siblings_then_steamcmd_default() {
        let scratch = tempfile::tempdir().unwrap();
        let sibling = scratch.path().join("existing");
        std::fs::create_dir_all(&sibling).unwrap();
        std::fs::write(sibling.join("PalServer.exe"), b"exe").unwrap();
        let install = scratch.path().join("new-world");
        let found = find_existing_server("", &install.to_string_lossy());
        assert_eq!(found, Some(sibling.to_string_lossy().to_string()));

        let steamcmd_dir = scratch.path().join("steamcmd");
        let default_install = steamcmd_dir
            .join("steamapps")
            .join("common")
            .join("PalServer");
        std::fs::create_dir_all(&default_install).unwrap();
        std::fs::write(default_install.join("PalServer.exe"), b"exe").unwrap();
        let steamcmd_exe = steamcmd_dir.join("steamcmd.exe");
        let found = find_existing_server(&steamcmd_exe.to_string_lossy(), "");
        assert_eq!(found, Some(default_install.to_string_lossy().to_string()));
    }

    #[tokio::test]
    async fn create_native_server_skips_install_when_exe_present_and_writes_config() {
        let scratch = tempfile::tempdir().unwrap();
        let mut record = crate::services::docker::test_support::docker_record();
        record.server_type = "native".to_string();
        record.install_path = scratch.path().to_string_lossy().to_string();
        std::fs::write(scratch.path().join("PalServer.exe"), b"exe").unwrap();
        assert!(create_native_server(&record, None).await);
        assert!(scratch
            .path()
            .join("Pal")
            .join("Saved")
            .join("Config")
            .join("WindowsServer")
            .join("PalWorldSettings.ini")
            .exists());
    }

    #[tokio::test]
    async fn create_native_server_fails_without_exe_source_or_steamcmd() {
        let scratch = tempfile::tempdir().unwrap();
        let mut record = crate::services::docker::test_support::docker_record();
        record.server_type = "native".to_string();
        record.install_path = scratch.path().join("empty").to_string_lossy().to_string();
        record.steamcmd_path = String::new();
        assert!(!create_native_server(&record, None).await);
    }

    #[tokio::test]
    async fn stop_server_process_without_pid_returns_false() {
        let record = crate::services::docker::test_support::docker_record(); // pid: None
        let api = crate::services::palworld_api::PalworldApiClient::new();
        assert!(!stop_server_process(&record, &api).await);
    }
}
