use std::path::{Path, PathBuf};

use ps_core::mods::{PalModSettings, Platform, Ue4ssMode};

pub const HAZARD_UE4SS_DUAL_INSTANCE: &str = "ue4ss_dual_instance";

const MAX_PARENT_WALK: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DetectedTarget {
    pub root: String,
    pub platform: String,
    pub ue4ss_mode: String,
    pub hazards: Vec<String>,
    pub source: &'static str,
}

/// Pulls every `"path"` value out of a `libraryfolders.vdf`. Deliberately a text
/// scan rather than a vdf parser: the only key this needs is `path`, the format
/// quotes every value, and a scan cannot be broken by a nesting change between
/// Steam versions. Escaped backslashes are unescaped, which is how the file
/// stores Windows paths.
pub fn steam_library_paths(libraryfolders_vdf: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in libraryfolders_vdf.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("\"path\"") else {
            continue;
        };
        let mut parts = rest.split('"').filter(|p| !p.trim().is_empty());
        if let Some(value) = parts.next() {
            let unescaped = value.replace("\\\\", "\\");
            if !unescaped.is_empty() {
                out.push(unescaped);
            }
        }
    }
    out
}

fn binaries_candidates(root: &Path) -> [(PathBuf, Platform); 2] {
    [
        (root.join("Pal/Binaries/Win64"), Platform::Win64),
        (root.join("Pal/Binaries/WinGDK"), Platform::WinGdk),
    ]
}

fn has_executable(binaries: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(binaries) else {
        return false;
    };
    entries.filter_map(Result::ok).any(|entry| {
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        name.ends_with(".exe") || name.ends_with("-shipping")
    })
}

/// `Pal/Content/Paks` plus an executable under one of the binaries directories.
/// The paks directory alone is not enough: an extracted mod archive can contain
/// that path, and the analyzer's passthrough rules depend on the two being distinguishable.
pub fn looks_like_install(root: &Path) -> bool {
    root.join("Pal/Content/Paks").is_dir()
        && binaries_candidates(root)
            .iter()
            .any(|(dir, _)| has_executable(dir))
}

pub fn platform_of_install(root: &Path) -> Option<Platform> {
    binaries_candidates(root)
        .into_iter()
        .find(|(dir, _)| has_executable(dir))
        .map(|(_, platform)| platform)
}

/// Walks up at most six parents looking for an install, so a user who picks
/// `…/Palworld/Pal/Binaries/Win64` still lands on the root.
pub fn normalise_root(picked: &Path) -> Option<PathBuf> {
    // A relative pick would resolve against the process working directory, and
    // popping a one-component relative path leaves the empty path, which tests
    // `Pal/Content/Paks` relative to wherever the process happens to be running.
    // An install picked that way would store an empty root and move with the cwd.
    if !picked.is_absolute() {
        return None;
    }
    let mut current = picked.to_path_buf();
    for _ in 0..=MAX_PARENT_WALK {
        if looks_like_install(&current) {
            return Some(current);
        }
        if !current.pop() || current.as_os_str().is_empty() {
            break;
        }
    }
    None
}

/// Standard is `dwmapi.dll` or `ue4ss/UE4SS.dll` beside the
/// game executable, Workshop is anything under `Mods/NativeMods/UE4SS`. Standard
/// wins unless `PalModSettings.ini` both enables modding and lists UE4SS. Both
/// present is always the hazard, whichever wins, because running both crashes the
/// game.
pub fn detect_ue4ss(root: &Path, platform: Platform) -> (Ue4ssMode, Vec<String>) {
    let binaries = match platform {
        Platform::WinGdk => root.join("Pal/Binaries/WinGDK"),
        _ => root.join("Pal/Binaries/Win64"),
    };
    let standard =
        binaries.join("dwmapi.dll").is_file() || binaries.join("ue4ss/UE4SS.dll").is_file();
    let workshop_dir = root.join("Mods/NativeMods/UE4SS");
    let workshop = workshop_dir
        .read_dir()
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false);

    let mut hazards = Vec::new();
    if standard && workshop {
        hazards.push(HAZARD_UE4SS_DUAL_INSTANCE.to_string());
    }

    let settings_prefers_workshop = super::ini_text::read(&root.join("Mods/PalModSettings.ini"))
        .ok()
        .flatten()
        .map(|file| {
            let settings = PalModSettings::parse(&file.text);
            settings.enabled
                && settings
                    .active_mods
                    .iter()
                    .any(|name| name.to_ascii_lowercase().starts_with("ue4ss"))
        })
        .unwrap_or(false);

    let mode = match (standard, workshop) {
        (true, true) if settings_prefers_workshop => Ue4ssMode::Workshop,
        (true, _) => Ue4ssMode::Standard,
        (false, true) => Ue4ssMode::Workshop,
        (false, false) => Ue4ssMode::None,
    };
    (mode, hazards)
}

fn platform_slug(platform: Platform) -> &'static str {
    match platform {
        Platform::Win64 => "win64",
        Platform::WinGdk => "wingdk",
        Platform::Linux => "linux",
        Platform::Mac => "mac",
    }
}

fn mode_slug(mode: Ue4ssMode) -> &'static str {
    match mode {
        Ue4ssMode::Workshop => "workshop",
        Ue4ssMode::Standard => "standard",
        Ue4ssMode::None => "none",
    }
}

pub fn describe(root: &Path, source: &'static str) -> Option<DetectedTarget> {
    let platform = platform_of_install(root)?;
    if !looks_like_install(root) {
        return None;
    }
    let (mode, hazards) = detect_ue4ss(root, platform);
    Some(DetectedTarget {
        root: ps_core::mods::native_separators(&root.to_string_lossy(), cfg!(windows)),
        platform: platform_slug(platform).to_string(),
        ue4ss_mode: mode_slug(mode).to_string(),
        hazards,
        source,
    })
}

/// The value named `name` in the listing `reg query` prints, e.g.
/// `    SteamPath    REG_SZ    d:/programs/steam`.
pub fn registry_string_value(reg_query_output: &str, name: &str) -> Option<String> {
    reg_query_output.lines().find_map(|line| {
        let (key, rest) = line.trim().split_once(char::is_whitespace)?;
        if !key.eq_ignore_ascii_case(name) {
            return None;
        }
        let (kind, value) = rest.trim_start().split_once(char::is_whitespace)?;
        let value = value.trim();
        (matches!(kind, "REG_SZ" | "REG_EXPAND_SZ") && !value.is_empty())
            .then(|| value.to_string())
    })
}

/// Where Steam says it is installed. Steam can live anywhere, so the
/// Program Files guesses below miss any install on another drive.
#[cfg(windows)]
fn registry_steam_roots() -> Vec<PathBuf> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    [
        (r"HKCU\Software\Valve\Steam", "SteamPath"),
        (r"HKLM\SOFTWARE\WOW6432Node\Valve\Steam", "InstallPath"),
        (r"HKLM\SOFTWARE\Valve\Steam", "InstallPath"),
    ]
    .into_iter()
    .filter_map(|(key, value)| {
        let output = std::process::Command::new("reg")
            .args(["query", key, "/v", value])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        registry_string_value(&String::from_utf8_lossy(&output.stdout), value).map(PathBuf::from)
    })
    .collect()
}

fn steam_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    #[cfg(windows)]
    {
        roots.extend(registry_steam_roots());
        for key in ["ProgramFiles(x86)", "ProgramFiles"] {
            if let Some(base) = std::env::var_os(key) {
                roots.push(PathBuf::from(base).join("Steam"));
            }
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        roots.push(home.join(".steam/steam"));
        roots.push(home.join(".local/share/Steam"));
        roots.push(home.join("Library/Application Support/Steam"));
    }
    roots
}

/// Every Palworld install reachable through Steam's own library list. Returns an
/// empty vector rather than an error when Steam is absent, because "no installs
/// found" is an ordinary outcome the caller presents as such.
pub fn find_installs() -> Vec<DetectedTarget> {
    find_installs_in(&steam_roots())
}

pub fn find_installs_in(steam_roots: &[PathBuf]) -> Vec<DetectedTarget> {
    let mut found: Vec<DetectedTarget> = Vec::new();
    for library in steam_library_dirs_in(steam_roots) {
        let candidate = library.join("steamapps").join("common").join("Palworld");
        if let Some(target) = describe(&candidate, "steam") {
            if !found.iter().any(|existing| existing.root == target.root) {
                found.push(target);
            }
        }
    }
    found
}

/// Every library folder Steam lists, each once, in native spelling.
pub fn steam_library_dirs() -> Vec<PathBuf> {
    steam_library_dirs_in(&steam_roots())
}

pub fn steam_library_dirs_in(steam_roots: &[PathBuf]) -> Vec<PathBuf> {
    let case_insensitive = cfg!(any(windows, target_os = "macos"));
    let mut seen = std::collections::HashSet::new();
    let mut libraries = Vec::new();
    for steam_root in steam_roots {
        let vdf = steam_root.join("steamapps").join("libraryfolders.vdf");
        let Ok(text) = std::fs::read_to_string(&vdf) else {
            continue;
        };
        for library in steam_library_paths(&text) {
            let library = ps_core::mods::native_separators(&library, cfg!(windows));
            if seen.insert(ps_core::mods::normalize_physical_path(
                &library,
                case_insensitive,
            )) {
                libraries.push(PathBuf::from(library));
            }
        }
    }
    libraries
}

use ps_core::mods::TargetKind;
use ps_db::mod_targets::ModTarget;

#[derive(Debug, thiserror::Error)]
pub enum RegisterError {
    #[error("no Palworld install at {0}")]
    NotAnInstall(String),
    #[error(transparent)]
    Db(#[from] ps_db::DbError),
}

pub fn detected_json(detected: &DetectedTarget) -> String {
    serde_json::json!({
        "source": detected.source,
        "platform": detected.platform,
        "ue4ss_mode": detected.ue4ss_mode,
        "hazards": detected.hazards,
    })
    .to_string()
}

/// The id is derived from the install's own directory name, so registering the
/// same install twice updates one row instead of creating a second target
/// pointing at one directory.
fn id_for(root: &str) -> String {
    let leaf = Path::new(root)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "client".to_string());
    ps_core::mods::target_id(TargetKind::Client, &leaf)
}

fn same_root(a: &str, b: &str) -> bool {
    let case_insensitive = cfg!(any(windows, target_os = "macos"));
    ps_core::mods::normalize_physical_path(a, case_insensitive)
        == ps_core::mods::normalize_physical_path(b, case_insensitive)
}

/// A free id for this install. Two Palworld installs on different drives share a
/// leaf directory name, so an id derived from the leaf alone would make the second
/// registration silently repoint the first target. An id already held by a
/// *different* root gets a numeric suffix; the same root keeps its id, which is
/// what makes re-registering idempotent.
async fn free_id_for(
    db: &dyn ps_db::DbDriver,
    root: &str,
) -> Result<(String, Option<ModTarget>), RegisterError> {
    let base = id_for(root);
    for attempt in 0..100u32 {
        let candidate = if attempt == 0 {
            base.clone()
        } else {
            format!("{base}-{}", attempt + 1)
        };
        match ps_db::mod_targets::get(db, &candidate).await? {
            None => return Ok((candidate, None)),
            Some(existing) if same_root(&existing.root_path, root) => {
                return Ok((candidate, Some(existing)))
            }
            Some(_) => continue,
        }
    }
    Err(RegisterError::NotAnInstall(root.to_string()))
}

pub async fn register(
    db: &dyn ps_db::DbDriver,
    detected: &DetectedTarget,
    name: &str,
) -> Result<ModTarget, RegisterError> {
    let (id, existing) = free_id_for(db, &detected.root).await?;
    let existed = existing.is_some();
    let target = ps_db::mod_targets::upsert(
        db,
        &ps_db::mod_targets::NewModTarget {
            id: id.clone(),
            kind: "client".to_string(),
            server_id: None,
            name: name.to_string(),
            root_path: ps_core::mods::native_separators(&detected.root, cfg!(windows)),
            platform: detected.platform.clone(),
            ue4ss_mode: detected.ue4ss_mode.clone(),
            // `upsert` replaces every column, so the user's overrides are carried
            // across. Re-running detection must not reset a path the user set.
            layout_overrides: existing
                .as_ref()
                .map(|row| row.layout_overrides.clone())
                .unwrap_or_else(|| "{}".to_string()),
            detected: detected_json(detected),
        },
    )
    .await?;
    if !existed {
        ps_db::mod_profiles::create(
            db,
            &ps_db::mod_profiles::NewProfile {
                id: format!("{id}/default"),
                target_id: id,
                name: "Default".to_string(),
                is_default: true,
            },
        )
        .await?;
    }
    Ok(target)
}

/// Re-reads the install and updates only what detection owns: the UE4SS mode and
/// the `detected` blob. `root_path` and `layout_overrides` belong to the user and
/// are never rewritten here.
pub async fn refresh(
    db: &dyn ps_db::DbDriver,
    target: &ModTarget,
) -> Result<ModTarget, RegisterError> {
    let root = PathBuf::from(&target.root_path);
    let described = describe(&root, "refresh")
        .ok_or_else(|| RegisterError::NotAnInstall(target.root_path.clone()))?;
    ps_db::mod_targets::set_detected(db, &target.id, &detected_json(&described)).await?;
    ps_db::mod_targets::set_ue4ss_mode(db, &target.id, &described.ue4ss_mode).await?;
    ps_db::mod_targets::mark_scanned(db, &target.id).await?;
    Ok(ps_db::mod_targets::get(db, &target.id)
        .await?
        .ok_or_else(|| ps_db::DbError::Other(format!("target {} vanished", target.id)))?)
}
