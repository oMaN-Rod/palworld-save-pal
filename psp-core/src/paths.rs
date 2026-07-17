//! Resolution of the writable "app root" — the base dir for `backups/`,
//! `servers/`, and the open-folder targets.
//!
//! The desktop shell can't `chdir` into its per-user data dir: the AppImage's
//! bundled WebKit spawns its helper processes via a path relative to the current
//! working directory, so changing the cwd makes those helpers unresolvable and
//! the webview crashes. Instead the shell exports `PSP_APP_ROOT`, and every
//! cwd-relative consumer resolves against it.

use std::ffi::OsString;
use std::path::PathBuf;

/// `PSP_APP_ROOT` when the desktop shell exported it, else the process cwd
/// (web-server mode, tests, and unpackaged runs that never set it).
pub fn app_root() -> PathBuf {
    resolve_app_root(std::env::var_os("PSP_APP_ROOT"))
}

fn resolve_app_root(configured: Option<OsString>) -> PathBuf {
    match configured {
        Some(value) if !value.is_empty() => PathBuf::from(value),
        _ => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_configured_root_when_set() {
        assert_eq!(
            resolve_app_root(Some(OsString::from("/writable/app-data"))),
            PathBuf::from("/writable/app-data")
        );
    }

    #[test]
    fn falls_back_to_cwd_when_unset() {
        let expected = std::env::current_dir().expect("cwd available");
        assert_eq!(resolve_app_root(None), expected);
    }

    #[test]
    fn falls_back_to_cwd_when_empty() {
        // An exported-but-empty PSP_APP_ROOT must NOT become PathBuf("") (which
        // would resolve every backup/server path cwd-relative again).
        let expected = std::env::current_dir().expect("cwd available");
        assert_eq!(resolve_app_root(Some(OsString::new())), expected);
    }
}
