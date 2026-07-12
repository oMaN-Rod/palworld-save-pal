#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};

const SERVER_PORT: u16 = 5174;

/// Holds the running embedded server so the exit handler can shut it down.
struct EmbeddedServer(Mutex<Option<psp_server::ServerHandle>>);

struct AssetDirs {
    ui_dir: PathBuf,
    data_dir: PathBuf,
    db_path: PathBuf,
}

/// Unpackaged runs resolve assets against the repo root. In debug builds that is
/// derived from the compile-time manifest path rather than the cwd, because
/// `tauri dev` runs the binary from the crate dir, not the repo root.
fn repo_root() -> anyhow::Result<PathBuf> {
    if cfg!(debug_assertions) {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let root = manifest_dir
            .parent()
            .ok_or_else(|| anyhow::anyhow!("psp-desktop manifest has no repo root above it"))?;
        return Ok(root.to_path_buf());
    }
    Ok(std::env::current_dir()?)
}

/// Packaged app: serve bundled resources, keep mutable state (DB, backups/) in
/// the per-user app data dir. Unpackaged (`cargo tauri dev`, `cargo run`): use
/// the repo's ui_build/ and data/ directly.
fn resolve_asset_dirs(app: &tauri::AppHandle) -> anyhow::Result<AssetDirs> {
    if let Ok(resource_dir) = app.path().resource_dir() {
        let bundled_ui = resource_dir.join("ui");
        if bundled_ui.join("index.html").is_file() {
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;
            // backups/ and open_folder("psp_root") resolve against cwd
            // (psp-server convention); point it at the writable app data dir.
            std::env::set_current_dir(&app_data_dir)?;
            return Ok(AssetDirs {
                ui_dir: bundled_ui,
                data_dir: resource_dir.join("data"),
                db_path: app_data_dir.join("psp-rs.db"),
            });
        }
    }
    let repo_root = repo_root()?;
    // Under `tauri dev` the webview loads Vite, so no static build is needed.
    // ServeDir just 404s on the absent dir; nothing else reads ui_dir.
    anyhow::ensure!(
        tauri::is_dev() || repo_root.join("ui_build").join("index.html").is_file(),
        "ui_build/index.html not found — run scripts/build-ui-desktop before `cargo run -p psp-desktop`, from the repo root"
    );
    // backups/ and open_folder("psp_root") resolve against cwd; `tauri dev` runs
    // the binary from the crate dir, so pin it back to the repo root.
    std::env::set_current_dir(&repo_root)?;
    Ok(AssetDirs {
        ui_dir: repo_root.join("ui_build"),
        data_dir: repo_root.join("data"),
        db_path: repo_root.join("psp-rs.db"),
    })
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        // Replaces desktop.py's temp-dir lock file (desktop.py:273-381):
        // a second launch triggers this callback in the FIRST instance,
        // which focuses the existing window; the second process exits.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(main_window) = app.get_webview_window("main") {
                let _ = main_window.unminimize();
                let _ = main_window.set_focus();
            }
        }))
        .manage(EmbeddedServer(Mutex::new(None)))
        .setup(|app| {
            let app_handle = app.handle().clone();
            let asset_dirs = resolve_asset_dirs(&app_handle)?;

            let server_config = psp_server::ServerConfig {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: SERVER_PORT,
                ui_dir: asset_dirs.ui_dir,
                data_dir: asset_dirs.data_dir,
                db_path: asset_dirs.db_path,
                desktop_mode: true,
            };

            // start_server binds the listener before returning, so once this
            // await completes the URL below is live — no sleep, no polling
            // (replaces desktop.py:440's time.sleep(2)).
            let server_handle =
                tauri::async_runtime::block_on(psp_server::start_server(server_config))?;
            let server_url: tauri::Url = format!("http://{}", server_handle.addr).parse()?;
            tracing::info!("embedded server listening on {}", server_handle.addr);

            // Dev: load the Vite dev server so frontend edits hot-reload. It
            // proxies /api back to the embedded server, and PUBLIC_WS_URL is an
            // absolute host, so /ws still connects straight to it. Prod: load the
            // embedded server, which serves the static ui_build/.
            let webview_url = app
                .config()
                .build
                .dev_url
                .clone()
                .filter(|_| tauri::is_dev())
                .unwrap_or(server_url);
            tracing::info!("webview loading {}", webview_url);

            app_handle
                .state::<EmbeddedServer>()
                .0
                .lock()
                .expect("server state mutex poisoned")
                .replace(server_handle);

            // desktop.py:231-237 — title, 1366x768, min 1366x768.
            WebviewWindowBuilder::new(&app_handle, "main", WebviewUrl::External(webview_url))
                .title(format!("Palworld Save Pal v{}", env!("CARGO_PKG_VERSION")))
                .inner_size(1366.0, 768.0)
                .min_inner_size(1366.0, 768.0)
                .build()?;

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to build Palworld Save Pal desktop app")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                // Graceful shutdown (replaces desktop.py on_closed + psutil
                // child cleanup — the server is in-process, nothing to kill).
                let taken = app
                    .state::<EmbeddedServer>()
                    .0
                    .lock()
                    .expect("server state mutex poisoned")
                    .take();
                if let Some(server_handle) = taken {
                    tauri::async_runtime::block_on(server_handle.shutdown());
                }
            }
        });
}
