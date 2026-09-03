//! Persisted Linux launch mode: which shell (`desktop` webview vs `browser`
//! headless + tray) the app runs in. Stored as a tiny text file in the app data
//! dir so the Rust shell can read it before any window is created — the SPA
//! cannot decide this because the decision of *whether to open a webview
//! window* happens in `setup`, before the UI loads.

use std::path::{Path, PathBuf};

/// The two selectable Linux run modes. `Unset` means no saved preference
/// (first run) — the `/mode-select` overlay is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Desktop,
    Browser,
    Unset,
}

impl Mode {
    /// Wire/disk value. `Unset` has no value (it is the absence of a choice).
    pub fn as_str(self) -> Option<&'static str> {
        match self {
            Mode::Desktop => Some("desktop"),
            Mode::Browser => Some("browser"),
            Mode::Unset => None,
        }
    }

    pub fn parse(s: &str) -> Mode {
        match s.trim() {
            "desktop" => Mode::Desktop,
            "browser" => Mode::Browser,
            // Corrupt/unknown content is treated as a first run rather than
            // erroring the whole app out.
            _ => Mode::Unset,
        }
    }
}

const MODE_FILE: &str = "mode.json";

/// Path of the persisted mode file for this app data dir.
pub fn mode_file_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(MODE_FILE)
}

/// Read the persisted mode. Missing or unreadable/corrupt → `Unset`.
pub fn load(app_data_dir: &Path) -> Mode {
    std::fs::read_to_string(mode_file_path(app_data_dir))
        .map(|s| Mode::parse(&s))
        .unwrap_or(Mode::Unset)
}

/// Persist a concrete mode choice. `Unset` cannot be saved (it means "no
/// choice made"); attempting to save it removes any existing file instead so a
/// future launch re-prompts.
pub fn save(app_data_dir: &Path, mode: Mode) -> std::io::Result<()> {
    match mode {
        Mode::Unset => {
            let _ = std::fs::remove_file(mode_file_path(app_data_dir));
            Ok(())
        }
        Mode::Desktop | Mode::Browser => {
            std::fs::create_dir_all(app_data_dir)?;
            std::fs::write(
                mode_file_path(app_data_dir),
                mode.as_str().expect("concrete mode has a value"),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("psp-mode-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn missing_file_is_unset() {
        let dir = temp_dir("missing");
        assert_eq!(load(&dir), Mode::Unset);
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = temp_dir("roundtrip");
        save(&dir, Mode::Browser).expect("save browser");
        assert_eq!(load(&dir), Mode::Browser);
        save(&dir, Mode::Desktop).expect("save desktop");
        assert_eq!(load(&dir), Mode::Desktop);
    }

    #[test]
    fn corrupt_content_is_unset() {
        let dir = temp_dir("corrupt");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(mode_file_path(&dir), "not-a-mode").unwrap();
        assert_eq!(load(&dir), Mode::Unset);
    }

    #[test]
    fn saving_unset_removes_the_choice() {
        let dir = temp_dir("unset-removes");
        save(&dir, Mode::Browser).unwrap();
        save(&dir, Mode::Unset).unwrap();
        assert_eq!(load(&dir), Mode::Unset);
    }
}
