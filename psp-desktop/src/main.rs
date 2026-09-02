#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Two launchers share this crate:
//! - `webview_app` (default): the Tauri webview desktop app on every OS;
//! - `browser_mode` (Linux + the `browser-mode` cargo feature): a terminal
//!   launcher that runs the same embedded server, opens the system browser
//!   instead of a webview window, and quits on `q`/Ctrl+C.
//!
//! The feature is inert on Windows/macOS, so `--features browser-mode` there
//! still produces the normal webview build.

use std::path::PathBuf;

/// Port of the embedded server in both launchers; the desktop UI's baked
/// WebSocket URL (`PUBLIC_WS_URL=127.0.0.1:5174/ws`) depends on it.
pub(crate) const SERVER_PORT: u16 = 5174;

#[cfg(all(feature = "browser-mode", target_os = "linux"))]
mod browser_mode;

#[cfg(not(all(feature = "browser-mode", target_os = "linux")))]
mod webview_app;

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

#[cfg(all(feature = "browser-mode", target_os = "linux"))]
fn main() {
    browser_mode::run();
}

#[cfg(not(all(feature = "browser-mode", target_os = "linux")))]
fn main() {
    webview_app::run();
}
