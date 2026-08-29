//! Local game-data discovery.
//!
//! The game only writes its bridge file when launched with
//! `-output-gamedata`; on Linux the write lands inside the Proton prefix.
//! Candidate paths:
//!
//! * Windows client: `%LOCALAPPDATA%\Pal\Saved\PalGameDataBridge\GameData.json`
//! * Linux client (Proton):
//!   `<library>/steamapps/compatdata/1623730/pfx/drive_c/users/steamuser/
//!    AppData/Local/Pal/Saved/PalGameDataBridge/GameData.json`
//! * Native PalServer: `<library>/steamapps/common/PalServer/Pal/Saved/
//!   PalGameDataBridge/GameData.json`
use std::path::{Path, PathBuf};

use crate::PALWORLD_APPID;

/// One discovery result: where the file would be, and whether it is there.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct GameDataCandidate {
    pub path: PathBuf,
    pub exists: bool,
    /// "proton", "local" (Windows %LOCALAPPDATA%), or "palserver".
    pub origin: &'static str,
}

/// Steam install roots for the current platform. Linux covers the six spots
/// seen in the wild (standard, ~/.steam symlinks, flatpak, Steam Deck).
pub fn steam_base_roots() -> Vec<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let mut roots = Vec::new();
        for var in ["ProgramFiles(x86)", "ProgramFiles", "LOCALAPPDATA"] {
            if let Ok(value) = std::env::var(var) {
                if !value.is_empty() {
                    roots.push(PathBuf::from(value).join("Steam"));
                }
            }
        }
        roots
    }
    #[cfg(not(target_os = "windows"))]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        if home.is_empty() {
            return Vec::new();
        }
        let home = PathBuf::from(home);
        vec![
            home.join(".local/share/Steam"),
            home.join(".steam/steam"),
            home.join(".steam/root"),
            home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
            PathBuf::from("/home/deck/.local/share/Steam"),
            home.join(".steam/steam/steamapps"),
        ]
    }
}

/// Library folders declared by `libraryfolders.vdf`, including the root the
/// file was found under. Values are the `"path"` entries with Windows
/// backslashes normalized; a torn or missing file simply yields the roots.
pub fn steam_libraries() -> Vec<PathBuf> {
    let mut libraries: Vec<PathBuf> = Vec::new();
    for root in steam_base_roots() {
        libraries.push(root.clone());
        let vdf = root.join("steamapps/libraryfolders.vdf");
        let Ok(text) = std::fs::read_to_string(&vdf) else {
            continue;
        };
        for path in vdf_paths(&text) {
            let normalized = if cfg!(windows) {
                path
            } else {
                path.replace('\\', "/")
            };
            libraries.push(PathBuf::from(normalized));
        }
    }
    dedupe_existing(libraries)
}

/// Extracts `"path" "value"` pairs from a KeyValues file without pulling in a
/// VDF parser. Splitting on the quote character alternates separator /
/// quoted-content, so the quoted tokens are the odd-indexed fields; a `"path"`
/// token is followed immediately by its value token.
fn vdf_paths(text: &str) -> impl Iterator<Item = String> + '_ {
    let tokens: Vec<&str> = text.split('"').collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 1;
    while i + 2 < tokens.len() {
        if tokens[i] == "path" {
            out.push(tokens[i + 2].to_string());
            i += 4;
        } else {
            i += 2;
        }
    }
    out.into_iter()
}

fn dedupe_existing(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen: Vec<PathBuf> = Vec::new();
    for path in paths {
        if path.as_os_str().is_empty() || seen.contains(&path) {
            continue;
        }
        seen.push(path);
    }
    seen
}

/// All `GameData.json` locations worth probing on this machine.
pub fn game_data_candidates() -> Vec<GameDataCandidate> {
    let mut out = Vec::new();
    #[cfg(target_os = "windows")]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            if !local.is_empty() {
                out.push(candidate(
                    PathBuf::from(&local)
                        .join("Pal")
                        .join("Saved")
                        .join("PalGameDataBridge")
                        .join("GameData.json"),
                    "local",
                ));
            }
        }
    }
    for library in steam_libraries() {
        let steamapps = if library.ends_with("steamapps") {
            library.clone()
        } else {
            library.join("steamapps")
        };
        out.push(candidate(
            steamapps
                .join("compatdata")
                .join(PALWORLD_APPID.to_string())
                .join("pfx/drive_c/users/steamuser/AppData/Local/Pal/Saved/PalGameDataBridge/GameData.json"),
            "proton",
        ));
        out.push(candidate(
            steamapps
                .join("common")
                .join("PalServer")
                .join("Pal/Saved/PalGameDataBridge/GameData.json"),
            "palserver",
        ));
    }
    out
}

fn candidate(path: PathBuf, origin: &'static str) -> GameDataCandidate {
    let exists = path.is_file();
    GameDataCandidate {
        path,
        exists,
        origin,
    }
}

/// First existing bridge file, preferring the Proton client path (the common
/// case), then the native PalServer.
pub fn find_game_data() -> Option<PathBuf> {
    game_data_candidates()
        .into_iter()
        .find(|candidate| candidate.exists)
        .map(|candidate| candidate.path)
}

/// True when `path` sits inside a Proton prefix for Palworld — used by the
/// UI to explain *why* the path looks the way it does.
pub fn is_proton_path(path: &Path) -> bool {
    path.components().any(|component| {
        component.as_os_str().to_string_lossy() == format!("compatdata")
    }) && path
        .components()
        .any(|component| component.as_os_str().to_string_lossy() == PALWORLD_APPID.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vdf_paths_extracts_quoted_path_entries() {
        let vdf = r#"
"libraryfolders"
{
    "0" { "path" "C:\\SteamLibrary" }
    "1" { "path"    "/mnt/games/SteamLibrary"  "label" "external" }
}
"#;
        let paths: Vec<String> = vdf_paths(vdf).collect();
        assert_eq!(paths, vec![r"C:\\SteamLibrary", "/mnt/games/SteamLibrary"]);
    }

    #[test]
    fn dedupe_existing_drops_empty_and_duplicate_paths() {
        let merged = dedupe_existing(vec![
            PathBuf::new(),
            PathBuf::from("/a"),
            PathBuf::from("/a"),
            PathBuf::from("/b"),
        ]);
        assert_eq!(merged, vec![PathBuf::from("/a"), PathBuf::from("/b")]);
    }

    #[test]
    fn proton_paths_are_recognized() {
        let path = Path::new("/home/deck/.local/share/Steam/steamapps/compatdata/1623730/pfx");
        assert!(is_proton_path(path));
        assert!(!is_proton_path(Path::new("/home/deck/.local/share/Steam/steamapps/common")));
    }
}
