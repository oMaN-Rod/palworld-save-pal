//! Linux launcher: a single binary that runs the embedded psp-server and, based
//! on the persisted launch mode, either shows the normal Tauri editor window
//! (`Mode::Desktop`), runs headless behind a system tray with the editor in the
//! user's browser (`Mode::Browser`), or — on first run (`Mode::Unset`) — shows
//! a `/mode-select` overlay asking the user to choose.
//!
//! The choice is persisted to `mode.json` (see `mode.rs`) and can be changed
//! later from the tray ("Switch to Desktop Mode") or from Settings
//! ("Display mode"). Cross-mode switches relaunch the process, because a webview
//! cannot be hot-swapped into an already-running headless Tauri runtime; the
//! first-run pivot (Unset → a concrete mode) happens in-process.
//!
//! Tray reachability: built on Tauri's `tray-icon` support, which on Linux
//! goes through libappindicator's StatusNotifierItem spec. Desktops that host
//! StatusNotifier (KDE Plasma, Ubuntu's GNOME via its AppIndicator extension,
//! XFCE/MATE/Cinnamon with their SNI applets) show the icon. Vanilla GNOME and
//! some tiling-WM setups ship no such host, and libappindicator has no
//! legacy-XEmbed fallback — there the icon simply never appears (creation
//! still succeeds, silently). The app keeps running headless regardless: the
//! browser is auto-opened once ready, and Quit also works via the `shutdown`
//! WS message in addition to the tray.

use std::io::{ErrorKind, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use std::os::unix::process::CommandExt;

use anyhow::Result;
use psp_server::{ModeEvent, ServerConfig};
use tauri::image::Image;
use tauri::menu::{MenuBuilder, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};

use crate::mode::Mode;
use crate::{repo_root, SERVER_PORT};

const HTTP_PROBE_TIMEOUT: Duration = Duration::from_secs(15);

// ---------------------------------------------------------------------------
// Asset + state resolution
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct AssetDirs {
    ui_dir: PathBuf,
    data_dir: PathBuf,
    db_path: PathBuf,
}

/// Serves bundled resources and keeps mutable state in the per-user app data
/// dir, mirroring the Windows/macOS launcher. Unpackaged runs use the repo's
/// ui_build/ + data/ directly.
fn resolve_asset_dirs(app: &tauri::AppHandle) -> Result<AssetDirs> {
    if let Ok(resource_dir) = app.path().resource_dir() {
        let bundled_ui = resource_dir.join("ui");
        if bundled_ui.join("index.html").is_file() {
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;
            std::env::set_var("PSP_APP_ROOT", &app_data_dir);
            return Ok(AssetDirs {
                ui_dir: bundled_ui,
                data_dir: resource_dir.join("data"),
                db_path: app_data_dir.join("psp-rs.db"),
            });
        }
    }
    let repo_root = repo_root()?;
    anyhow::ensure!(
        tauri::is_dev() || repo_root.join("ui_build").join("index.html").is_file(),
        "ui_build/index.html not found — run scripts/build-ui-desktop before `cargo run -p psp-desktop`, from the repo root"
    );
    std::env::set_var("PSP_APP_ROOT", &repo_root);
    Ok(AssetDirs {
        ui_dir: repo_root.join("ui_build"),
        data_dir: repo_root.join("data"),
        db_path: repo_root.join("psp-rs.db"),
    })
}

// ---------------------------------------------------------------------------
// Loopback HTTP probing — std-only, no HTTP client dependency
// ---------------------------------------------------------------------------

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

fn bind_addr() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), SERVER_PORT)
}

fn url_for(addr: SocketAddr) -> String {
    format!("http://localhost:{}", addr.port())
}

fn start_server_blocking(assets: &AssetDirs) -> Result<psp_server::ServerHandle> {
    let config = ServerConfig {
        host: IpAddr::V4(Ipv4Addr::LOCALHOST),
        port: SERVER_PORT,
        ui_dir: assets.ui_dir.clone(),
        data_dir: assets.data_dir.clone(),
        db_path: assets.db_path.clone(),
        desktop_mode: true,
    };
    let server = match tauri::async_runtime::block_on(psp_server::start_server(config)) {
        Ok(handle) => handle,
        Err(error) if is_addr_in_use(&error) => handle_port_busy(error, bind_addr()),
        Err(error) => return Err(error.context("starting the embedded server")),
    };
    tracing::info!("embedded server listening on {}", server.addr);
    wait_until_responding(server.addr)?;
    Ok(server)
}

/// Exit the whole app when a client sends the `shutdown` WS message — the
/// Quit affordance for browser-mode setups with no visible tray. Each server
/// owns its watch channel, so every start (first run, Open recovery, restart)
/// re-arms the watcher. Flags a deliberate quit so the ServiceMode exit-guard
/// lets it through; `RunEvent::Exit` then drains the server gracefully.
fn watch_shutdown_requests(app: &tauri::AppHandle, server: &psp_server::ServerHandle) {
    let app = app.clone();
    let mut requests = server.shutdown_requested.clone();
    tauri::async_runtime::spawn(async move {
        while requests.changed().await.is_ok() {
            if *requests.borrow() {
                tracing::info!("shutdown requested over the WebSocket — quitting");
                app.state::<UserQuitting>().0.store(true, Ordering::SeqCst);
                app.exit(0);
            }
        }
    });
}

    /// Port 7257 is occupied by another PSP instance: open it in the browser and
/// leave (single-instance-like). If a non-PSP app owns the port, error out —
    /// the UI's WebSocket URL is baked to 7257 and cannot silently move.
fn handle_port_busy(error: anyhow::Error, addr: SocketAddr) -> ! {
    if let Some((200, body)) = http_get(addr, "/") {
        if body.contains("Palworld") {
            let url = format!("http://localhost:{}", addr.port());
            let _ = opener::open(&url);
            eprintln!(
                "psp: another instance is already serving at {url} — opened it, nothing new started."
            );
            std::process::exit(0);
        }
    }
    eprintln!("psp: {error:#}");
    eprintln!(
        "port {} is occupied by another application. The UI's WebSocket URL is baked to \
         127.0.0.1:{0} at build time, so psp cannot pick a different port — free the port \
         and start again",
        addr.port()
    );
    std::process::exit(1);
}

fn open_url(url: &str) {
    match opener::open(url) {
        Ok(()) => tracing::info!("opened {url} in the default browser"),
        Err(error) => tracing::error!("could not open {url}: {error}"),
    }
}

// ---------------------------------------------------------------------------
// Window helpers (shared by modes)
// ---------------------------------------------------------------------------

fn server_url() -> String {
    format!("http://127.0.0.1:{SERVER_PORT}")
}

/// The editor webview — the `Mode::Desktop` experience (same as Windows/macOS).
fn open_desktop_window(app: &tauri::AppHandle) -> Result<()> {
    // WebKitGTK often keeps the webview blank on virtual GPUs; default the
    // DMABUF renderer off unless the user chose otherwise.
    if let Ok(current) = std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER") {
        let _ = current;
    } else {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }

    let allow_dev_server = cfg!(debug_assertions) && tauri::is_dev();
    let webview_url = if allow_dev_server {
        app.config()
            .build
            .dev_url
            .clone()
            .unwrap_or_else(|| server_url().parse().expect("valid server url"))
    } else {
        server_url().parse().expect("valid server url")
    };
    tracing::info!("webview loading {}", webview_url);

    WebviewWindowBuilder::new(app, "main", WebviewUrl::External(webview_url))
        .title(format!("Palworld Save Pal v{}", env!("CARGO_PKG_VERSION")))
        .inner_size(1366.0, 768.0)
        .min_inner_size(1366.0, 768.0)
        .maximized(true)
        // Show only once the editor UI reports it finished bootstrapping (see
        // the `ready` WS listener in `run`), so the user never sees the blank
        // WebKitGTK flash while the SPA loads in the background.
        .visible(false)
        .disable_drag_drop_handler()
        .build()?;
    Ok(())
}

/// The first-run `/mode-select` overlay. Bare shell, asks Desktop vs Browser.
fn open_mode_select_window(app: &tauri::AppHandle) -> Result<()> {
    let url: tauri::Url = format!("{}/mode-select", server_url()).parse()?;
    WebviewWindowBuilder::new(app, "mode-select", WebviewUrl::External(url))
        .title("Choose Display Mode — Palworld Save Pal")
        .inner_size(520.0, 640.0)
        .resizable(false)
        .center()
        .build()?;
    Ok(())
}

fn close_window(app: &tauri::AppHandle, label: &str) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.close();
    }
}

/// Reveal the editor window once it has finished bootstrapping. No-op if the
/// window isn't present (e.g. Browser mode, where there is no desktop window).
fn show_desktop_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Safety net: if the editor UI never reports ready (a hung boot, or a listener
/// race), fall back to showing the hidden window after `timeout` so the user is
/// never left staring at nothing.
fn schedule_show_fallback(app: tauri::AppHandle, timeout: Duration) {
    std::thread::spawn(move || {
        std::thread::sleep(timeout);
        let show = app.clone();
        let _ = app.run_on_main_thread(move || show_desktop_window(&show));
    });
}

// ---------------------------------------------------------------------------
// Tray (Browser mode)
// ---------------------------------------------------------------------------

const MENU_OPEN: &str = "open";
const MENU_OPEN_LABEL: &str = "Open Editor";
const MENU_TO_DESKTOP: &str = "to_desktop";
const MENU_TO_DESKTOP_LABEL: &str = "Switch to Desktop Mode";
const MENU_RESTART: &str = "restart";
const MENU_RESTART_LABEL: &str = "Restart";
const MENU_QUIT: &str = "quit";
const MENU_QUIT_LABEL: &str = "Quit";

fn tray_icon() -> Option<Image<'static>> {
    let png = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/icons/icon.png"));
    let decoded = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?;
    let rgba = decoded.to_rgba8();
    Image::new_owned(rgba.to_vec(), rgba.width(), rgba.height()).into()
}

/// Build the tray with Open / Switch-to-Desktop / Restart / Quit. A missing tray
/// host is logged, not fatal — the browser is already open and the process stays
/// as the service.
fn build_tray(app: &tauri::AppHandle, assets: &AssetDirs) {
    let build_item = |id: &str, text: &str| -> Result<MenuItem<tauri::Wry>> {
        MenuItem::with_id(app, id, text, true, Option::<&str>::None).map_err(Into::into)
    };
    let open = match build_item(MENU_OPEN, MENU_OPEN_LABEL) {
        Ok(i) => i,
        Err(e) => return warn_tray_fail(e),
    };
    let to_desktop = match build_item(MENU_TO_DESKTOP, MENU_TO_DESKTOP_LABEL) {
        Ok(i) => i,
        Err(e) => return warn_tray_fail(e),
    };
    let restart = match build_item(MENU_RESTART, MENU_RESTART_LABEL) {
        Ok(i) => i,
        Err(e) => return warn_tray_fail(e),
    };
    let quit = match build_item(MENU_QUIT, MENU_QUIT_LABEL) {
        Ok(i) => i,
        Err(e) => return warn_tray_fail(e),
    };
    let menu = match MenuBuilder::new(app)
        .items(&[&open, &to_desktop, &restart, &quit])
        .build()
    {
        Ok(m) => m,
        Err(e) => return warn_tray_fail(e),
    };

    let mut builder = TrayIconBuilder::with_id("psp-browser-mode")
        .menu(&menu)
        .tooltip(format!(
            "Palworld Save Pal (browser mode) — http://localhost:{SERVER_PORT}",
        ));
    if let Some(icon) = tray_icon() {
        builder = builder.icon(icon);
    }

    let assets = assets.clone();
    let result = builder
        .on_menu_event(move |app, event| match event.id().as_ref() {
            MENU_OPEN => tray_open(app, &assets),
            MENU_TO_DESKTOP => switch_display_mode(app, Mode::Desktop),
            MENU_RESTART => tray_restart(app, assets.clone()),
            MENU_QUIT => {
                tracing::info!("quit requested from the tray");
                app.state::<UserQuitting>().0.store(true, Ordering::SeqCst);
                app.exit(0)
            }
            _ => {}
        })
        .build(app);

    if let Err(error) = result {
        warn_tray_fail(error);
    }
}

fn warn_tray_fail(error: impl std::fmt::Display) {
    tracing::warn!(
        "no system tray available on this desktop ({error}); the service is still running — \
         reopen the editor at http://localhost:{SERVER_PORT}, quit via pkill psp or the tray/Quit entry"
    );
}

fn tray_open(app: &tauri::AppHandle, assets: &AssetDirs) {
    if let Some(addr) = server_addr(app) {
        open_url(&url_for(addr));
        return;
    }
    // Server is down (e.g. a crash): boot it fresh, then open. Runs on a plain
    // std thread because start_server_blocking blocks on the tauri runtime and
    // must not be called from inside a tokio worker.
    let app = app.clone();
    let assets = assets.clone();
    std::thread::spawn(move || {
        // One starter at a time: a concurrent start would lose the port race
        // and take the whole process down via the port-busy exit.
        if !try_begin_server_start(&app) {
            tracing::warn!("a server start is already in progress");
            return;
        }
        match start_server_blocking(&assets) {
            Ok(server) => {
                watch_shutdown_requests(&app, &server);
                let addr = server.addr;
                set_server_handle(&app, Some(server));
                open_url(&url_for(addr));
            }
            Err(error) => tracing::error!("Open Editor could not start the server: {error}"),
        }
        end_server_start(&app);
    });
}

fn tray_restart(app: &tauri::AppHandle, assets: AssetDirs) {
    // Runs on a plain std thread: both start_server_blocking and
    // tauri::async_runtime::block_on(server.shutdown()) block on the tauri
    // runtime and panic if invoked from inside a tokio worker.
    let app = app.clone();
    std::thread::spawn(move || {
        if !try_begin_server_start(&app) {
            tracing::warn!("a server restart is already in progress");
            return;
        }
        let taken = set_server_handle(&app, None);
        if let Some(server) = taken {
            tracing::info!("restart: shutting down the current server");
            tauri::async_runtime::block_on(server.shutdown());
        }
        match start_server_blocking(&assets) {
            Ok(server) => {
                watch_shutdown_requests(&app, &server);
                let addr = server.addr;
                set_server_handle(&app, Some(server));
                tracing::info!("restart complete — reopened {}", url_for(addr));
                open_url(&url_for(addr));
            }
            Err(error) => tracing::error!("restart failed: {error:#}"),
        }
        end_server_start(&app);
    });
}

// ---------------------------------------------------------------------------
// Managed state + server handle access
// ---------------------------------------------------------------------------

struct ServerState(Mutex<Option<psp_server::ServerHandle>>);

/// Set to `true` when the process should keep running with no windows (Browser
/// mode, or after a first-run pivot to Browser) — a last-window-close must then
/// be ignored instead of exiting.
struct ServiceMode(AtomicBool);

/// Set to `true` when the user deliberately quits from the tray, so the
/// `ServiceMode` exit-guard lets the exit through instead of swallowing it.
struct UserQuitting(AtomicBool);

/// Held while a tray-triggered server start runs, so two rapid tray actions
/// can't race each other into `handle_port_busy`'s process exit.
struct ServerStarting(AtomicBool);

/// Claims the single server-start slot. False means another start is already
/// running and this one must not proceed.
fn try_begin_server_start(app: &tauri::AppHandle) -> bool {
    !app.state::<ServerStarting>().0.swap(true, Ordering::SeqCst)
}

fn end_server_start(app: &tauri::AppHandle) {
    app.state::<ServerStarting>()
        .0
        .store(false, Ordering::SeqCst);
}

fn server_addr(app: &tauri::AppHandle) -> Option<SocketAddr> {
    app.state::<ServerState>()
        .0
        .lock()
        .expect("server state mutex poisoned")
        .as_ref()
        .map(|server| server.addr)
}

fn set_server_handle(app: &tauri::AppHandle, server: Option<psp_server::ServerHandle>) -> Option<psp_server::ServerHandle> {
    std::mem::replace(
        &mut *app
            .state::<ServerState>()
            .0
            .lock()
            .expect("server state mutex poisoned"),
        server,
    )
}

fn is_service_mode(app: &tauri::AppHandle) -> bool {
    app.state::<ServiceMode>().0.load(Ordering::SeqCst)
}

fn user_wants_quit(app: &tauri::AppHandle) -> bool {
    app.state::<UserQuitting>().0.load(Ordering::SeqCst)
}

// ---------------------------------------------------------------------------
// Mode switching
// ---------------------------------------------------------------------------

// Globals set once at startup so the SET_MODE listener task and tray closure
// can reach state/UI from any thread.
static ASSETS: std::sync::OnceLock<AssetDirs> = std::sync::OnceLock::new();
static MODE_FILE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
static CURRENT_MODE: std::sync::OnceLock<Mutex<Mode>> = std::sync::OnceLock::new();

/// Detach and relaunch this executable so a new mode takes effect (a webview
/// can't be swapped into a running headless runtime). Works from the AppImage's
/// mounted AppRun. The current process exits shortly after.
fn relaunch(app: &tauri::AppHandle) {
    // The parent must actually die so the child can bind the port; flag the exit
    // so the ServiceMode exit-guard lets it through.
    app.state::<UserQuitting>().0.store(true, Ordering::SeqCst);
    let exe = std::env::current_exe().expect("current exe path");
    tracing::info!("relaunching in the new mode: {}", exe.display());
    // SAFETY: pre_exec runs in the child before exec, single-threaded there.
    let _ = unsafe {
        Command::new(&exe)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            // setsid: start a new session so the relaunch survives this process
            // exiting and never belongs to this terminal.
            .pre_exec(|| {
                libc::setsid();
                Ok(())
            })
            .spawn()
    };
    app.exit(0);
}

/// Apply a requested mode: persist it, then either pivot in-process (first-run
/// Unset → concrete mode) or relaunch (cross-mode switch from a running mode).
fn apply_mode_requested(app: &tauri::AppHandle, requested: Mode) {
    let current = *CURRENT_MODE
        .get()
        .expect("current mode set at startup")
        .lock()
        .expect("mode mutex poisoned");
    if requested == current {
        return;
    }

    let mode_file = MODE_FILE.get().expect("mode file set at startup");
    if let Err(error) = crate::mode::save(mode_file, requested) {
        tracing::error!("could not persist display mode {requested:?}: {error}");
        return;
    }

    // The choice is now committed, so the process must believe it too: without
    // this, a later `set_mode` re-enters the first-run pivot branch below and
    // builds a second tray / leaves ServiceMode stale. The relaunch path
    // re-reads mode.json in the child anyway; updating here keeps both paths
    // consistent and lets the UI query the live mode.
    *CURRENT_MODE
        .get()
        .expect("current mode set at startup")
        .lock()
        .expect("mode mutex poisoned") = requested;
    psp_server::set_display_mode(requested.as_str().map(str::to_string));

    // First-run pivot: no committed mode yet, so swap windows in-process.
    // Window close/build must run on the main UI thread — the SET_MODE handler
    // runs on a background task, so dispatch there.
    if current == Mode::Unset {
        let app2 = app.clone();
        app.run_on_main_thread(move || match requested {
            Mode::Desktop => {
                close_window(&app2, "mode-select");
                // Explicit for coherence with the Browser arm: Desktop owns the
                // window lifecycle, so closing the window should quit.
                app2.state::<ServiceMode>().0.store(false, Ordering::SeqCst);
                if let Err(error) = open_desktop_window(&app2) {
                    tracing::error!("could not open the editor window: {error}");
                }
            }
            Mode::Browser => {
                close_window(&app2, "mode-select");
                app2.state::<ServiceMode>().0.store(true, Ordering::SeqCst);
                build_tray(&app2, ASSETS.get().expect("assets set at startup"));
                if let Some(addr) = server_addr(&app2) {
                    open_url(&url_for(addr));
                }
            }
            Mode::Unset => unreachable!("requested is always concrete"),
        })
        .expect("first-run pivot dispatch to the main thread");
        return;
    }

    // Cross-mode switch from a running mode: relaunch so the new shell boots
    // from a clean runtime.
    relaunch(app);
}

/// The SET_MODE WS message (first-run overlay or Settings "Display mode").
/// Runs on a background task; window/tray builders marshal to the main thread.
fn handle_mode_event(app: &tauri::AppHandle, event: ModeEvent) {
    let requested = match event.mode.as_str() {
        "desktop" => Mode::Desktop,
        "browser" => Mode::Browser,
        other => {
            tracing::warn!("ignoring unknown display mode from the UI: {other}");
            return;
        }
    };
    apply_mode_requested(app, requested);
}

/// Tray/`Switch to Desktop Mode` and Settings — a relaunch-based cross-mode
/// switch (the caller is already in a committed mode).
fn switch_display_mode(app: &tauri::AppHandle, requested: Mode) {
    apply_mode_requested(app, requested);
}

// ---------------------------------------------------------------------------
// Entry
// ---------------------------------------------------------------------------

pub fn run(mode: Mode) {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        .setup(move |app| {
            let app_handle = app.handle().clone();
            let assets = resolve_asset_dirs(&app_handle)?;
            let app_data_dir = app_handle.path().app_data_dir()?;
            let mode_file = crate::mode::mode_file_path(&app_data_dir);

            let server = start_server_blocking(&assets)?;
            watch_shutdown_requests(&app_handle, &server);

            CURRENT_MODE
                .set(Mutex::new(mode))
                .expect("CURRENT_MODE set once");
            ASSETS.set(assets.clone()).expect("ASSETS set once");
            MODE_FILE.set(mode_file).expect("MODE_FILE set once");
            // Publish the current display mode so the UI's Settings dialog can
            // show it and only offer switching where a shell supports it
            // (absent on Windows/macOS and the web build → control hidden).
            psp_server::set_display_mode(mode.as_str().map(str::to_string));
            app.manage(ServerState(Mutex::new(Some(server))));
            app.manage(ServiceMode(AtomicBool::new(matches!(mode, Mode::Browser))));
            app.manage(UserQuitting(AtomicBool::new(false)));
            app.manage(ServerStarting(AtomicBool::new(false)));

            // Listen for `set_mode` from the first-run overlay / Settings.
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<ModeEvent>();
            psp_server::set_mode_listener(tx);
            let listener = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                while let Some(event) = rx.recv().await {
                    handle_mode_event(&listener, event);
                }
            });

            // Reveal the hidden editor window once the UI reports it finished
            // bootstrapping (the `ready` WS message), so the user never sees the
            // blank WebKitGTK flash. Fall back so a hung boot can't strand it.
            let (ready_tx, mut ready_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
            psp_server::set_ready_listener(ready_tx);
            let ready_app = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                while ready_rx.recv().await.is_some() {
                    let show = ready_app.clone();
                    let _ = ready_app.run_on_main_thread(move || {
                        show_desktop_window(&show);
                    });
                }
            });
            schedule_show_fallback(app_handle.clone(), Duration::from_secs(12));

            // Enter the chosen (or unset) shell.
            match mode {
                Mode::Desktop => open_desktop_window(&app_handle)?,
                Mode::Browser => {
                    build_tray(&app_handle, &assets);
                    if let Some(addr) = server_addr(&app_handle) {
                        open_url(&url_for(addr));
                    }
                }
                Mode::Unset => open_mode_select_window(&app_handle)?,
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to build Palworld Save Pal desktop app")
        .run(|app, event| match event {
            // In Browser/headless mode there are no permanent windows; a
            // transient close (mode-select) must not take the service down.
            // A deliberate Quit from the tray is flagged and allowed through.
            RunEvent::ExitRequested { api, .. }
                if is_service_mode(app) && !user_wants_quit(app) =>
            {
                api.prevent_exit();
            }
            RunEvent::Exit => {
                let taken = set_server_handle(app, None);
                if let Some(server) = taken {
                    tauri::async_runtime::block_on(server.shutdown());
                }
            }
            _ => {}
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    #[test]
    fn http_get_reports_the_status_of_a_local_server() {
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
    fn tray_icon_decodes_from_the_bundled_png() {
        assert!(super::tray_icon().is_some());
    }

    #[test]
    fn url_for_maps_loopback_to_localhost() {
        assert_eq!(url_for(SocketAddr::from(([127, 0, 0, 1], SERVER_PORT))), "http://localhost:7257");
    }

    #[test]
    fn bind_addr_is_loopback_server_port() {
        assert_eq!(bind_addr(), SocketAddr::from(([127, 0, 0, 1], crate::SERVER_PORT)));
    }
}
