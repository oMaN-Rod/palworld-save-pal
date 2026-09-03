//! Native shell-open handlers; they hand paths and URLs to the host OS, so they
//! live in the transport crate rather than the platform-agnostic message layer.

use std::path::{Path, PathBuf};

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;

#[derive(Debug, serde::Deserialize)]
pub struct OpenFolderData {
    pub folder_type: String,
}

/// `app_root` is the writable base dir — the desktop shell exports `PSP_APP_ROOT`
/// pointing at a per-user dir, which is where `backups/` is written.
pub fn folder_path_for(folder_type: &str, app_root: &Path) -> Option<PathBuf> {
    match folder_type {
        "backups" => Some(app_root.join("backups")),
        "steam" => Some(crate::desktop_dialogs::steam_save_root()),
        "gamepass" => Some(crate::desktop_dialogs::gamepass_save_root()),
        "psp_root" => Some(app_root.to_path_buf()),
        _ => None,
    }
}

pub fn browser_url_from(host_and_port: &str) -> String {
    let (host, port) = match host_and_port.rsplit_once(':') {
        Some((host, port)) => (host, port),
        None => (host_and_port, ""),
    };
    let host = if host == "127.0.0.1" {
        "localhost"
    } else {
        host
    };
    format!("http://{host}:{port}")
}

/// Opens with NO response frame on success; a missing folder answers `warning`.
pub async fn handle_open_folder(
    data: OpenFolderData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if !ctx.app.config.desktop_mode {
        // `open_folder` is a desktop-only message: stay silent rather than
        // emit a frame a web client would never see the button for.
        return Ok(());
    }
    let app_root = psp_core::paths::app_root();
    let resolved = folder_path_for(&data.folder_type, &app_root);
    match resolved {
        Some(folder_path) if folder_path.exists() => {
            opener::open(&folder_path).map_err(|open_error| {
                HandlerError::Other(format!(
                    "Failed to open folder {}: {open_error}",
                    folder_path.display()
                ))
            })?;
        }
        Some(folder_path) => {
            ctx.emitter.emit(
                MessageType::Warning,
                &format!("Folder not found: {}", folder_path.display()),
            );
        }
        None => {
            ctx.emitter.emit(
                MessageType::Warning,
                &format!("Folder not found: {}", data.folder_type),
            );
        }
    }
    Ok(())
}

/// Active in BOTH desktop and web mode, unlike `open_folder`.
pub async fn handle_open_in_browser(
    data: String,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let url = browser_url_from(&data);
    opener::open(&url).map_err(|open_error| {
        HandlerError::Other(format!("Failed to open browser: {open_error}"))
    })?;
    ctx.emitter
        .emit(MessageType::OpenInBrowser, &"Browser opened successfully");
    Ok(())
}

/// Fired by the browser-mode control panel's Quit button: asks the embedded
/// server to begin a graceful shutdown. The embedding shell observes the
/// server stopping and exits — this handler never kills the process itself.
pub async fn handle_shutdown(
    _data: serde_json::Value,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if !crate::request_shutdown() {
        return Err(HandlerError::Other(
            "no shutdown channel is installed in this process".to_string(),
        ));
    }
    ctx.emitter.emit(MessageType::Shutdown, &"shutting down");
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct SetModeData {
    pub mode: String,
}

/// Fired by the first-run mode-select overlay or the Settings "Display mode"
/// control. Relays the request to the embedding shell (which persists the
/// choice to `mode.json` and pivots/relaunches); this handler only validates,
/// forwards, and acknowledges under the same wire type so `sendAndWait`
/// resolves. Replied under `SetMode`, not `Shutdown`/`Error`.
pub async fn handle_set_mode(
    data: SetModeData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    if data.mode != "desktop" && data.mode != "browser" {
        return Err(HandlerError::Other(format!(
            "Refusing unknown display mode: {}",
            data.mode
        )));
    }
    if !crate::emit_mode_event(crate::ModeEvent {
        mode: data.mode.clone(),
    }) {
        return Err(HandlerError::Other(
            "no mode listener is installed in this process".to_string(),
        ));
    }
    ctx.emitter.emit(MessageType::SetMode, &data.mode);
    Ok(())
}

/// Only http(s) URLs may be handed to `opener`; anything else (a `file://`
/// path, a `javascript:` payload, an arbitrary scheme) is refused so a WS
/// message can't coax the host into launching an unexpected handler.
fn is_openable_url(url: &str) -> bool {    url.starts_with("http://") || url.starts_with("https://")
}

/// Opens an external URL in the OS default browser. The Tauri webview drops
/// `<a target="_blank">` navigations, so desktop links route here instead;
/// `opener::open` hands the URL to the host, escaping the webview.
pub async fn handle_open_url(
    data: String,
    _ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let url = data.trim();
    if !is_openable_url(url) {
        return Err(HandlerError::Other(format!(
            "Refusing to open non-http(s) URL: {url}"
        )));
    }
    opener::open(url)
        .map_err(|open_error| HandlerError::Other(format!("Failed to open URL {url}: {open_error}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use psp_app::test_support::TestContext;

    #[test]
    fn is_openable_url_accepts_only_http_schemes() {
        assert!(is_openable_url("http://localhost:5173"));
        assert!(is_openable_url("https://github.com/oMaN-Rod/palworld-save-pal"));
        assert!(is_openable_url("https://buymeacoffee.com/i_am_o"));

        assert!(!is_openable_url("file:///etc/passwd"));
        assert!(!is_openable_url("javascript:alert(1)"));
        assert!(!is_openable_url("ftp://example.com"));
        assert!(!is_openable_url("github.com"));
        assert!(!is_openable_url(""));
    }

    #[tokio::test]
    async fn handle_open_url_rejects_non_http_scheme() {
        let mut test = TestContext::new(|_| {}).await;
        let mut ctx = HandlerCtx {
            session: &mut test.session,
            app: &test.app,
            emitter: &test.emitter,
            blueprints: &mut test.blueprints,
            attachment: None,
        };
        let result = handle_open_url("file:///etc/passwd".to_string(), &mut ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn set_mode_rejects_unknown_modes() {
        let mut test = TestContext::new(|_| {}).await;
        let mut ctx = HandlerCtx {
            session: &mut test.session,
            app: &test.app,
            emitter: &test.emitter,
            blueprints: &mut test.blueprints,
            attachment: None,
        };
        for bogus in ["", "terminal", "kiosk", "DESKTOP"] {
            let result = handle_set_mode(SetModeData { mode: bogus.to_string() }, &mut ctx).await;
            assert!(result.is_err(), "unexpected mode {bogus:?} must be refused");
        }
    }

    #[tokio::test]
    async fn set_mode_reports_missing_listener() {
        // No mode listener is installed in a unit-test process, so a valid
        // request must surface that as an error rather than pretending it
        // applied. (The shell's per-process listener is what makes it succeed
        // in the real app.)
        let mut test = TestContext::new(|_| {}).await;
        let mut ctx = HandlerCtx {
            session: &mut test.session,
            app: &test.app,
            emitter: &test.emitter,
            blueprints: &mut test.blueprints,
            attachment: None,
        };
        let result = handle_set_mode(SetModeData { mode: "browser".into() }, &mut ctx).await;
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod desktop_system_tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn folder_path_resolves_all_four_python_folder_types() {
        let app_root = Path::new("/opt/psp-data");
        assert_eq!(
            folder_path_for("backups", app_root),
            Some(app_root.join("backups"))
        );
        assert_eq!(
            folder_path_for("steam", app_root),
            Some(crate::desktop_dialogs::steam_save_root())
        );
        assert_eq!(
            folder_path_for("gamepass", app_root),
            Some(crate::desktop_dialogs::gamepass_save_root())
        );
        assert_eq!(
            folder_path_for("psp_root", app_root),
            Some(app_root.to_path_buf())
        );
        assert_eq!(folder_path_for("bogus", app_root), None);
    }

    #[test]
    fn browser_url_maps_loopback_to_localhost() {
        assert_eq!(browser_url_from("127.0.0.1:5174"), "http://localhost:5174");
        assert_eq!(browser_url_from("myhost:8080"), "http://myhost:8080");
    }
}
