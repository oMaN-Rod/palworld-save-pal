//! Linux `browser-mode` launcher: runs the embedded psp-server and opens a
//! small Tauri control-panel window at the SPA's `/browser-mode` route. That
//! page shows the live boot sequence, auto-opens the system browser once the
//! server answers, and offers Open/Copy/Quit buttons (Quit arrives as the
//! `shutdown` WS message, observed via `ServerHandle::shutdown_requested`).
//! The heavy editor UI never renders in the webview — it runs in the user's
//! browser at full engine speed.
//!
//! Compiled only when the `browser-mode` cargo feature is on AND the target is
//! Linux — everywhere else `webview_app` runs and the normal Tauri window
//! behavior is untouched.

use std::io::{ErrorKind, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use psp_server::ServerConfig;
use tauri::{Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};

use crate::{repo_root, SERVER_PORT};

/// Same identifier as tauri.conf.json: browser-mode and the webview build share
/// one per-user data dir (~/.local/share/<identifier>) — one database and one
/// backups/ root regardless of which build launched it.
const APP_IDENTIFIER: &str = "com.palworldsavepal.desktop";

const HTTP_PROBE_TIMEOUT: Duration = Duration::from_secs(15);

/// Holds the running embedded server so the exit handler can shut it down.
struct EmbeddedServer(Mutex<Option<psp_server::ServerHandle>>);

// ---------------------------------------------------------------------------
// Asset resolution — mirrors webview_app::resolve_asset_dirs without a Tauri
// AppHandle. Bundled runs (AppImage/deb) keep Tauri's `usr/lib/<productName>`
// layout; dev runs fall back to the repo's ui_build/ + data/.
// ---------------------------------------------------------------------------

struct AssetDirs {
    ui_dir: PathBuf,
    data_dir: PathBuf,
    db_path: PathBuf,
}

/// Tauri's Linux bundler places resources under `usr/lib/<productName>`; the
/// directory name varies with the bundle config (the browser-mode AppImage
/// overrides productName), so probe entries instead of hardcoding it. The
/// AppImage runtime exports `APPDIR` (the mounted squashfs root); a deb
/// install keeps the same `usr/` layout on disk relative to the binary.
fn bundled_resource_root() -> Option<PathBuf> {
    let mut usr_dirs: Vec<PathBuf> = Vec::new();
    if let Ok(appdir) = std::env::var("APPDIR") {
        usr_dirs.push(Path::new(&appdir).join("usr"));
    }
    if let Ok(exe) = std::env::current_exe() {
        // Binary lives at <root>/usr/bin/psp → resources under <root>/usr/lib/.
        if let Some(usr) = exe.parent().and_then(Path::parent) {
            usr_dirs.push(usr.to_path_buf());
        }
    }
    for usr in usr_dirs {
        let entries = std::fs::read_dir(usr.join("lib")).ok()?;
        for entry in entries.flatten() {
            let candidate = entry.path();
            let is_bundle = candidate.join("ui").join("index.html").is_file()
                && candidate.join("data").join("json").is_dir();
            if is_bundle {
                return Some(candidate);
            }
        }
    }
    None
}

/// XDG data dir + app identifier — the same location tauri's `app_data_dir()`
/// resolves on Linux, so both builds share state.
fn user_data_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        if !xdg.is_empty() && Path::new(&xdg).is_absolute() {
            return PathBuf::from(xdg).join(APP_IDENTIFIER);
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    Path::new(&home).join(".local/share").join(APP_IDENTIFIER)
}

fn resolve_asset_dirs() -> Result<AssetDirs> {
    if let Some(root) = bundled_resource_root() {
        let app_data = user_data_dir();
        std::fs::create_dir_all(&app_data)
            .with_context(|| format!("creating the app data dir {}", app_data.display()))?;
        // psp-server resolves backups/ and open_folder("psp_root") against
        // PSP_APP_ROOT; keep it pointed at the writable per-user dir.
        std::env::set_var("PSP_APP_ROOT", &app_data);
        return Ok(AssetDirs {
            ui_dir: root.join("ui"),
            data_dir: root.join("data"),
            db_path: app_data.join("psp-rs.db"),
        });
    }

    // Dev checkout: same layout the webview build uses (ui_build/ + data/ at
    // the repo root, state kept in-tree).
    let root = repo_root().context("locating the repo root for a dev browser-mode run")?;
    anyhow::ensure!(
        root.join("ui_build").join("index.html").is_file(),
        "ui_build/index.html not found — run scripts/build-ui-desktop.sh (or easyrun.sh --browser) first"
    );
    std::env::set_var("PSP_APP_ROOT", &root);
    Ok(AssetDirs {
        ui_dir: root.join("ui_build"),
        data_dir: root.join("data"),
        db_path: root.join("psp-rs.db"),
    })
}

// ---------------------------------------------------------------------------
// Loopback HTTP probing — std-only, no HTTP client dependency. Only ever
// pointed at 127.0.0.1, where TLS is impossible.
// ---------------------------------------------------------------------------

/// Blocking HTTP/1.1 GET returning `(status, raw response text)` — headers
/// plus the first body bytes, enough to check a status code or a marker
/// string in the served HTML.
fn http_get(addr: SocketAddr, path: &str) -> Option<(u16, String)> {
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    stream.set_write_timeout(Some(Duration::from_secs(2))).ok()?;
    let request = format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).ok()?;

    let mut response = Vec::with_capacity(8 * 1024);
    let mut chunk = [0u8; 4 * 1024];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                response.extend_from_slice(&chunk[..n]);
                if response.len() >= 8 * 1024 {
                    break;
                }
            }
            Err(ref error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    let text = String::from_utf8_lossy(&response).into_owned();
    let status = text.split_whitespace().nth(1)?.parse().ok()?;
    Some((status, text))
}

/// `start_server` binds before returning, but the control window must not
/// open against a dead server — poll `GET /` until it actually answers 200.
fn wait_until_responding(addr: SocketAddr) -> Result<()> {
    let deadline = Instant::now() + HTTP_PROBE_TIMEOUT;
    loop {
        if matches!(http_get(addr, "/"), Some((200, _))) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            anyhow::bail!(
                "server on {addr} did not answer HTTP within {}s",
                HTTP_PROBE_TIMEOUT.as_secs()
            );
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn is_addr_in_use(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|e| e.kind() == ErrorKind::AddrInUse)
    })
}

// ---------------------------------------------------------------------------
// Launcher flow
// ---------------------------------------------------------------------------

/// Port 5174 is occupied. The desktop UI's WebSocket URL is baked to
/// `127.0.0.1:5174` at build time, so silently picking another port would
/// serve a UI that cannot connect. Instead: if the port already serves PSP,
/// just open it in the browser and leave (single-instance-like behavior);
/// otherwise surface a clear error. Either way this process ends here.
fn handle_port_busy(error: anyhow::Error, addr: SocketAddr) -> ! {
    if let Some((200, body)) = http_get(addr, "/") {
        if body.contains("Palworld") {
            let url = format!("http://localhost:{}", addr.port());
            let _ = opener::open(&url);
            println!(
                "browser-mode: another PSP instance is already serving at {url} — opened it in your browser, nothing new started."
            );
            std::process::exit(0);
        }
    }
    eprintln!("browser-mode: {error:#}");
    eprintln!(
        "port {} is occupied by another application. The desktop UI's WebSocket URL is baked to \
         127.0.0.1:{0} at build time, so browser-mode cannot pick a different port — free the \
         port (or stop the other app) and start again",
        addr.port()
    );
    std::process::exit(1);
}

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        .manage(EmbeddedServer(Mutex::new(None)))
        .setup(|app| {
            let app_handle = app.handle().clone();
            let assets = resolve_asset_dirs()?;

            let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), SERVER_PORT);
            let server = match tauri::async_runtime::block_on(psp_server::start_server(
                ServerConfig {
                    host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                    port: SERVER_PORT,
                    ui_dir: assets.ui_dir,
                    data_dir: assets.data_dir,
                    db_path: assets.db_path,
                    desktop_mode: true,
                },
            )) {
                Ok(handle) => handle,
                Err(error) if is_addr_in_use(&error) => handle_port_busy(error, bind_addr),
                Err(error) => return Err(error.context("starting the embedded server").into()),
            };
            tracing::info!("browser-mode server listening on {}", server.addr);
            wait_until_responding(server.addr)?;

            // Quit button on the control page → `shutdown` WS message → the
            // watch below fires → exit the run loop → RunEvent::Exit shuts the
            // server down gracefully.
            let mut shutdown_requested = server.shutdown_requested.clone();
            let shutdown_watcher = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                if shutdown_requested.changed().await.is_ok() {
                    tracing::info!("shutdown requested by the control panel");
                    shutdown_watcher.exit(0);
                }
            });

            app_handle
                .state::<EmbeddedServer>()
                .0
                .lock()
                .expect("server state mutex poisoned")
                .replace(server);

            // The control panel is served by the embedded server itself, so it
            // can only load once the server answers — the page's boot sequence
            // (WS connect → version handshake) runs live from there.
            let control_url: tauri::Url = format!("http://{}/browser-mode", bind_addr).parse()?;
            WebviewWindowBuilder::new(
                &app_handle,
                "browser-mode",
                WebviewUrl::External(control_url),
            )
            .title(format!(
                "Palworld Save Pal — Browser Mode v{}",
                env!("CARGO_PKG_VERSION")
            ))
            .inner_size(480.0, 560.0)
            .resizable(false)
            .center()
            .build()?;

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to build the PSP browser-mode app")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                // The server runs in-process: closing the control window is
                // the whole app quitting, so drain it gracefully.
                let taken = app
                    .state::<EmbeddedServer>()
                    .0
                    .lock()
                    .expect("server state mutex poisoned")
                    .take();
                if let Some(server) = taken {
                    tauri::async_runtime::block_on(server.shutdown());
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::{http_get, user_data_dir};

    #[test]
    fn http_get_reports_the_status_of_a_local_server() {
        // Spin a one-shot listener that answers any request with 418.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
        let addr = listener.local_addr().expect("local addr");
        let responder = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                use std::io::{Read, Write};
                let mut scratch = [0u8; 1024];
                let _ = stream.read(&mut scratch);
                let _ = stream
                    .write_all(b"HTTP/1.1 418 I'm a teapot\r\nContent-Length: 0\r\n\r\n");
            }
        });
        let (status, _) = http_get(addr, "/").expect("a response");
        assert_eq!(status, 418);
        responder.join().expect("responder thread");
    }

    #[test]
    fn user_data_dir_uses_xdg_when_absolute() {
        // Temporarily point XDG_DATA_HOME at an absolute temp path and confirm
        // the identifier is appended. Restored in all paths (single-threaded
        // test execution within one process makes this safe enough).
        let original = std::env::var("XDG_DATA_HOME").ok();
        let temp = std::env::temp_dir().join("psp-browser-mode-test");
        std::env::set_var("XDG_DATA_HOME", &temp);
        assert_eq!(user_data_dir(), temp.join(super::APP_IDENTIFIER));
        match original {
            Some(value) => std::env::set_var("XDG_DATA_HOME", value),
            None => std::env::remove_var("XDG_DATA_HOME"),
        }
    }
}
