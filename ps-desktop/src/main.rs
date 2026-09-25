#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::net::{IpAddr, Ipv4Addr};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tauri::{Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};

const SERVER_PORT: u16 = 5174;

const LEGACY_IDENTIFIER: &str = "com.palworldsavepal.desktop";

/// Holds the running embedded server so the exit handler can shut it down.
struct EmbeddedServer(Mutex<Option<ps_server::ServerHandle>>);

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
            .ok_or_else(|| anyhow::anyhow!("ps-desktop manifest has no repo root above it"))?;
        return Ok(root.to_path_buf());
    }
    Ok(std::env::current_dir()?)
}

/// The per-user dirs are named after the bundle identifier, so the old
/// identifier's dirs hold the database, backups and the webview profile.
/// Must run before anything creates the new dir or opens a webview.
fn adopt_legacy_dir(dir: &Path) {
    let legacy = dir.with_file_name(LEGACY_IDENTIFIER);
    if dir.exists() || !legacy.is_dir() {
        return;
    }
    match std::fs::rename(&legacy, dir) {
        Ok(()) => tracing::info!("moved {} to {}", legacy.display(), dir.display()),
        Err(error) => {
            tracing::warn!(%error, "could not move {} to {}", legacy.display(), dir.display())
        }
    }
}

/// Packaged app: serve bundled resources, keep mutable state in the per-user app
/// data dir. Unpackaged: use the repo's ui_build/ and data/ directly.
fn resolve_asset_dirs(app: &tauri::AppHandle) -> anyhow::Result<AssetDirs> {
    if let Ok(resource_dir) = app.path().resource_dir() {
        let bundled_ui = resource_dir.join("ui");
        if bundled_ui.join("index.html").is_file() {
            for dir in [app.path().app_data_dir(), app.path().app_local_data_dir()]
                .into_iter()
                .flatten()
            {
                adopt_legacy_dir(&dir);
            }
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;
            // backups/, servers/ and open_folder("ps_root") resolve against
            // PS_APP_ROOT; point it at the writable app data dir. Do NOT chdir
            // here: the AppImage's bundled WebKit spawns its helper processes via
            // a cwd-relative path, so changing the cwd crashes the webview.
            std::env::set_var("PS_APP_ROOT", &app_data_dir);
            return Ok(AssetDirs {
                ui_dir: bundled_ui,
                data_dir: resource_dir.join("data"),
                db_path: app_data_dir.join("ps-rs.db"),
            });
        }
    }
    let repo_root = repo_root()?;
    // Under `tauri dev` the webview loads Vite, so no static build is required.
    // The install bundle names the dir `ui` (it predates the desktop app);
    // `ui_build` is the repo checkout's spelling.
    let bundled_ui = ["ui_build", "ui"]
        .iter()
        .map(|name| repo_root.join(name))
        .find(|dir| dir.join("index.html").is_file());
    anyhow::ensure!(
        tauri::is_dev() || bundled_ui.is_some(),
        "no built UI found under {} (ui_build/ or ui/) — run scripts/build-ui-desktop before `cargo run -p ps-desktop`, from the repo root",
        repo_root.display()
    );
    std::env::set_var("PS_APP_ROOT", &repo_root);
    Ok(AssetDirs {
        ui_dir: bundled_ui.unwrap_or_else(|| repo_root.join("ui_build")),
        data_dir: repo_root.join("data"),
        db_path: repo_root.join("ps-rs.db"),
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

fn pip_path(main_path: &str) -> String {
    let first = main_path
        .trim_start_matches('/')
        .split('/')
        .next()
        .unwrap_or_default();
    let (language, region) = match first.split_once('-') {
        Some((language, region)) => (language, Some(region)),
        None => (first, None),
    };
    let is_locale = language.len() == 2
        && language.chars().all(|c| c.is_ascii_lowercase())
        && region.is_none_or(|r| {
            (2..=4).contains(&r.len()) && r.chars().all(|c| c.is_ascii_alphanumeric())
        });
    if is_locale {
        format!("/{first}/map")
    } else {
        "/map".to_string()
    }
}

/// Deliberately `async`: a synchronous command runs on the main thread, and
/// `WebviewWindowBuilder::build` there deadlocks on Windows waiting for an event
/// loop it is itself blocking — the window frame appears, the webview never
/// loads, and the app stops answering, so the blank window cannot even be
/// closed. Tauri documents `async` commands as the fix.
#[tauri::command]
async fn open_pip(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(pip) = app.get_webview_window("pip") {
        let _ = pip.unminimize();
        let _ = pip.set_focus();
        return Ok(());
    }
    let main = app.get_webview_window("main").ok_or("no main window")?;
    let mut url = main.url().map_err(|e| e.to_string())?;
    url.set_path(&pip_path(url.path()));
    url.set_query(Some("pip=1"));
    WebviewWindowBuilder::new(&app, "pip", WebviewUrl::External(url))
        .title("Live Map")
        .inner_size(360.0, 280.0)
        .min_inner_size(240.0, 180.0)
        .always_on_top(true)
        .decorations(true)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// WebKitGTK's DMABUF renderer leaves the WebView blank-white on many virtual
/// GPUs and driver combos; default it off on Linux unless the user already set
/// `WEBKIT_DISABLE_DMABUF_RENDERER` themselves.
#[cfg(any(target_os = "linux", test))]
fn dmabuf_disable_value(current: Option<std::ffi::OsString>) -> Option<&'static str> {
    match current {
        Some(_) => None,
        None => Some("1"),
    }
}

fn main() {
    // Must run before any WebKitGTK init, which reads this env var when it
    // spawns the web process.
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

            let server_config = ps_server::ServerConfig {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                // The desktop app's embedded server is fixed to its port;
                // network settings live in the server/webapp editions.
                port: Some(SERVER_PORT),
                ui_dir: asset_dirs.ui_dir,
                data_dir: asset_dirs.data_dir,
                db_path: asset_dirs.db_path,
                desktop_mode: true,
                hosted: false,
                websuite: false,
                allow_network_edits: true,
            };

            // start_server binds the listener before returning, so once this
            // await completes the URL below is live — no sleep, no polling.
            let server_handle =
                tauri::async_runtime::block_on(ps_server::start_server(server_config))?;
            let server_url: tauri::Url = format!("http://{}", server_handle.addr).parse()?;
            tracing::info!("embedded server listening on {}", server_handle.addr);

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
                .title(format!("PalStudio v{}", env!("CARGO_PKG_VERSION")))
                .inner_size(1366.0, 768.0)
                .min_inner_size(1366.0, 768.0)
                .maximized(true)
                .disable_drag_drop_handler()
                .build()?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![open_pip])
        .build(tauri::generate_context!())
        .expect("failed to build PalStudio desktop app")
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
    use super::{
        adopt_legacy_dir, choose_webview_url, dmabuf_disable_value, pip_path, LEGACY_IDENTIFIER,
    };

    fn url(s: &str) -> tauri::Url {
        s.parse().expect("valid url")
    }

    #[test]
    fn adopts_the_legacy_identifier_dir_when_the_new_one_is_absent() {
        let root = tempfile::tempdir().expect("a temp dir");
        let legacy = root.path().join(LEGACY_IDENTIFIER);
        std::fs::create_dir(&legacy).unwrap();
        std::fs::write(legacy.join("ps-rs.db"), b"db").unwrap();
        let current = root.path().join("com.palstudio.desktop");

        adopt_legacy_dir(&current);

        assert_eq!(std::fs::read(current.join("ps-rs.db")).unwrap(), b"db");
        assert!(!legacy.exists());
    }

    #[test]
    fn leaves_the_legacy_dir_alone_once_the_new_one_exists() {
        let root = tempfile::tempdir().expect("a temp dir");
        let legacy = root.path().join(LEGACY_IDENTIFIER);
        std::fs::create_dir(&legacy).unwrap();
        let current = root.path().join("com.palstudio.desktop");
        std::fs::create_dir(&current).unwrap();

        adopt_legacy_dir(&current);

        assert!(legacy.is_dir());
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

        assert_eq!(
            choose_webview_url(Some(dev.clone()), server.clone(), true),
            dev
        );
        // "localhost refused" regression guard: allow_dev_server false must never
        // pick the dev URL, even though tauri.conf.json still carries a dev_url.
        assert_eq!(choose_webview_url(Some(dev), server.clone(), false), server);
        assert_eq!(choose_webview_url(None, server.clone(), true), server);
    }

    #[test]
    fn pip_inherits_the_main_windows_locale_prefix() {
        assert_eq!(pip_path("/de/map"), "/de/map");
        assert_eq!(pip_path("/pt-br/wiki"), "/pt-br/map");
        assert_eq!(pip_path("/zh-hant/"), "/zh-hant/map");
        assert_eq!(pip_path("/map"), "/map");
        assert_eq!(pip_path("/"), "/map");
        assert_eq!(pip_path(""), "/map");
        assert_eq!(pip_path("/breeding"), "/map");
        assert_eq!(pip_path("/wiki/pal/lamball"), "/map");
    }
}
