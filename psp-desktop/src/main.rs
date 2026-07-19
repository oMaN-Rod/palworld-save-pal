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
            // backups/, servers/ and open_folder("psp_root") resolve against
            // PSP_APP_ROOT; point it at the writable app data dir. Do NOT chdir
            // here: the AppImage's bundled WebKit spawns its helper processes via
            // a cwd-relative path, so changing the cwd crashes the webview.
            std::env::set_var("PSP_APP_ROOT", &app_data_dir);
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
    // backups/, servers/ and open_folder("psp_root") resolve against
    // PSP_APP_ROOT; `tauri dev` runs the binary from the crate dir, so anchor
    // them at the repo root.
    std::env::set_var("PSP_APP_ROOT", &repo_root);
    Ok(AssetDirs {
        ui_dir: repo_root.join("ui_build"),
        data_dir: repo_root.join("data"),
        db_path: repo_root.join("psp-rs.db"),
    })
}

/// Chooses the URL the webview loads. The Vite dev server (`dev_url`) is only
/// correct under `tauri dev`; every `--release` binary must load the embedded
/// server. `tauri::is_dev()` reports true for any binary NOT produced by
/// `cargo tauri build` (e.g. a bare `cargo build --release`), so callers gate
/// `allow_dev_server` on `cfg!(debug_assertions)` too — otherwise a release
/// binary loads a dev server that isn't running ("localhost refused").
fn choose_webview_url(
    dev_url: Option<tauri::Url>,
    server_url: tauri::Url,
    allow_dev_server: bool,
) -> tauri::Url {
    dev_url.filter(|_| allow_dev_server).unwrap_or(server_url)
}

/// WebKitGTK's DMABUF renderer leaves the WebView blank-white on many virtual
/// GPUs (VMs) and quirky driver combos — the page's JS still runs, nothing
/// paints. We default the renderer off on Linux so the window always shows,
/// but only when the user hasn't set `WEBKIT_DISABLE_DMABUF_RENDERER` themselves.
/// Returns the value to export, or `None` to leave the environment untouched.
#[cfg(any(target_os = "linux", test))]
fn dmabuf_disable_value(current: Option<std::ffi::OsString>) -> Option<&'static str> {
    match current {
        Some(_) => None,
        None => Some("1"),
    }
}

fn main() {
    // Must run before any WebKitGTK init (webview build), which reads this env
    // var when it spawns the web process. Keeps the WebView from rendering blank
    // on Linux VMs / virtual GPUs.
    #[cfg(target_os = "linux")]
    if let Some(value) = dmabuf_disable_value(std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER")) {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", value);
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        // Single instance: a second launch fires this callback in the FIRST
        // instance, which focuses its window; the second process then exits.
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
            // await completes the URL below is live — no sleep, no polling.
            let server_handle =
                tauri::async_runtime::block_on(psp_server::start_server(server_config))?;
            let server_url: tauri::Url = format!("http://{}", server_handle.addr).parse()?;
            tracing::info!("embedded server listening on {}", server_handle.addr);

            // Dev (`tauri dev`): load the Vite dev server so frontend edits
            // hot-reload; it proxies /api and /ws to the embedded server. Any
            // release build loads the embedded server, which serves ui_build/.
            let allow_dev_server = cfg!(debug_assertions) && tauri::is_dev();
            let webview_url = choose_webview_url(
                app.config().build.dev_url.clone(),
                server_url,
                allow_dev_server,
            );
            tracing::info!("webview loading {}", webview_url);

            app_handle
                .state::<EmbeddedServer>()
                .0
                .lock()
                .expect("server state mutex poisoned")
                .replace(server_handle);

            WebviewWindowBuilder::new(&app_handle, "main", WebviewUrl::External(webview_url))
                .title(format!("Palworld Save Pal v{}", env!("CARGO_PKG_VERSION")))
                .inner_size(1366.0, 768.0)
                .min_inner_size(1366.0, 768.0)
                // Tauri's OS-level drag-drop handler intercepts drag events and
                // breaks HTML5 drag-and-drop inside the webview (notably on
                // Windows/WebView2). Disable it so the presets list drag works.
                .disable_drag_drop_handler()
                .build()?;

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to build Palworld Save Pal desktop app")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                // The server runs in-process, so exiting just means awaiting its
                // graceful shutdown — there is no child process to kill.
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

#[cfg(test)]
mod tests {
    use super::{choose_webview_url, dmabuf_disable_value};

    fn url(s: &str) -> tauri::Url {
        s.parse().expect("valid url")
    }

    #[test]
    fn defaults_dmabuf_renderer_off_when_user_left_it_unset() {
        assert_eq!(dmabuf_disable_value(None), Some("1"));
    }

    #[test]
    fn respects_an_explicit_user_dmabuf_choice() {
        // User forcing it on (0) or off (1) must win — never clobbered.
        assert_eq!(dmabuf_disable_value(Some("0".into())), None);
        assert_eq!(dmabuf_disable_value(Some("1".into())), None);
    }

    #[test]
    fn dev_server_loaded_only_when_allowed() {
        let dev = url("http://localhost:5173/");
        let server = url("http://127.0.0.1:5174/");

        // `tauri dev` (debug + is_dev): load Vite.
        assert_eq!(
            choose_webview_url(Some(dev.clone()), server.clone(), true),
            dev
        );
        // Release binary (allow_dev_server false): NEVER the dev URL, even
        // though tauri.conf.json still carries a dev_url. This is the
        // "localhost refused" regression guard.
        assert_eq!(choose_webview_url(Some(dev), server.clone(), false), server);
        // No dev URL configured: always the embedded server.
        assert_eq!(choose_webview_url(None, server.clone(), true), server);
    }
}
