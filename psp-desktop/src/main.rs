#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Single launcher binary.
//!
//! - **Non-Linux** (Windows/macOS): the classic Tauri webview editor
//!   (`webview_app`), unchanged.
//! - **Linux**: a runtime-mode launcher (`linux_launcher`) that, based on the
//!   persisted choice in `mode.rs`, either shows the webview editor
//!   (`Mode::Desktop`), runs headless behind a system tray with the editor in
//!   the user's browser (`Mode::Browser`), or — on first run — shows the
//!   `/mode-select` overlay asking the user to choose.

use std::path::PathBuf;

/// Port of the embedded server in all modes; the desktop UI's baked WebSocket
/// URL (`PUBLIC_WS_URL=127.0.0.1:5174/ws`) depends on it.
pub(crate) const SERVER_PORT: u16 = 5174;

#[cfg(target_os = "linux")]
mod linux_launcher;
#[cfg(not(target_os = "linux"))]
mod webview_app;

mod mode;

/// Unpackaged runs resolve assets against the repo root. In debug builds that is
/// derived from the compile-time manifest path rather than the cwd, because
/// `tauri dev` runs the binary from the crate dir, not the repo root.
pub(crate) fn repo_root() -> anyhow::Result<PathBuf> {
    if cfg!(debug_assertions) {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let root = manifest_dir
            .parent()
            .ok_or_else(|| anyhow::anyhow!("psp-desktop manifest has no repo root above it"))?;
        return Ok(root.to_path_buf());
    }
    Ok(std::env::current_dir()?)
}

/// The per-user app data dir — the same location tauri's `app_data_dir()`
/// resolves on Linux (`$XDG_DATA_HOME/<id>` else `~/.local/share/<id>`), where
/// `mode.json` and the DB live. Read here (before any Tauri init) so the launch
/// mode can be chosen before the runtime sets up.
#[cfg(target_os = "linux")]
fn default_app_data_dir() -> PathBuf {
    const APP_IDENTIFIER: &str = "com.palworldsavepal.desktop";
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        if !xdg.is_empty() && PathBuf::from(&xdg).is_absolute() {
            return PathBuf::from(xdg).join(APP_IDENTIFIER);
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".local/share").join(APP_IDENTIFIER)
}

fn main() {
    #[cfg(target_os = "linux")]
    {
        let mode = mode::load(&default_app_data_dir());
        tracing::info!(?mode, "psp Linux launcher starting");
        linux_launcher::run(mode);
    }
    #[cfg(not(target_os = "linux"))]
    webview_app::run();
}
