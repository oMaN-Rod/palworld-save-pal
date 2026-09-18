use ps_core::mods::{
    resolve_layout, LayoutError, LayoutOverrides, Platform, TargetKind, TargetLayout, TargetSpec,
    Ue4ssMode,
};
use ps_db::mod_targets::ModTarget;

#[derive(Debug, thiserror::Error)]
pub enum LayoutResolveError {
    #[error("unknown target kind {0}")]
    UnknownKind(String),
    #[error("unknown platform {0}")]
    UnknownPlatform(String),
    #[error("unknown ue4ss mode {0}")]
    UnknownMode(String),
    #[error("layout_overrides is not valid JSON for this target: {0}")]
    BadOverrides(String),
    #[error(transparent)]
    Layout(#[from] LayoutError),
}

/// The spellings are the ones `serde` emits for these enums, which is what the
/// `mod_targets` columns hold. A mismatch here is a runtime error on a real target
/// and nothing the compiler can catch.
fn kind_of(text: &str, platform: &str) -> Result<TargetKind, LayoutResolveError> {
    match text {
        "client" => Ok(TargetKind::Client),
        "native_server" => Ok(TargetKind::NativeServer),
        "docker_server" => Ok(TargetKind::DockerServer),
        // `ensure_server_targets` writes the bare kind `server` for both server
        // flavours and gives only a Docker server the `linux` platform. Resolving
        // a Docker server as native would place `PalModSettings.ini` and the
        // Workshop directory where no container bind reaches.
        "server" if platform == "linux" => Ok(TargetKind::DockerServer),
        "server" => Ok(TargetKind::NativeServer),
        other => Err(LayoutResolveError::UnknownKind(other.to_string())),
    }
}

fn platform_of(text: &str) -> Result<Platform, LayoutResolveError> {
    match text {
        "win64" => Ok(Platform::Win64),
        "wingdk" => Ok(Platform::WinGdk),
        "linux" => Ok(Platform::Linux),
        "mac" => Ok(Platform::Mac),
        other => Err(LayoutResolveError::UnknownPlatform(other.to_string())),
    }
}

fn mode_of(text: &str) -> Result<Ue4ssMode, LayoutResolveError> {
    match text {
        "workshop" => Ok(Ue4ssMode::Workshop),
        "standard" => Ok(Ue4ssMode::Standard),
        "none" => Ok(Ue4ssMode::None),
        other => Err(LayoutResolveError::UnknownMode(other.to_string())),
    }
}

/// `LayoutOverrides` is `deny_unknown_fields`, so naming a derived path such as
/// `mods_txt` fails here rather than being ignored.
fn overrides_of(json: &str) -> Result<LayoutOverrides, LayoutResolveError> {
    let text = json.trim();
    if text.is_empty() {
        return Ok(LayoutOverrides::default());
    }
    serde_json::from_str(text).map_err(|e| LayoutResolveError::BadOverrides(e.to_string()))
}

fn native(path: &str) -> String {
    ps_core::mods::native_separators(path, cfg!(windows))
}

/// Stored rows can hold a root or override spelled with mixed separators, and
/// every layout path inherits that spelling, so both are taken in native.
pub fn spec_for(target: &ModTarget) -> Result<TargetSpec, LayoutResolveError> {
    let mut overrides = overrides_of(&target.layout_overrides)?;
    for dir in [
        &mut overrides.binaries_dir,
        &mut overrides.ue4ss_dir,
        &mut overrides.ue4ss_mods_dir,
        &mut overrides.paks_mods_dir,
        &mut overrides.logicmods_dir,
        &mut overrides.nativemods_dir,
        &mut overrides.workshop_local_dir,
        &mut overrides.palmodsettings_ini,
    ]
    .into_iter()
    .flatten()
    {
        *dir = native(dir);
    }
    Ok(TargetSpec {
        kind: kind_of(&target.kind, &target.platform)?,
        root: native(&target.root_path),
        platform: platform_of(&target.platform)?,
        ue4ss_mode: mode_of(&target.ue4ss_mode)?,
        overrides,
    })
}

pub fn layout_for(target: &ModTarget) -> Result<TargetLayout, LayoutResolveError> {
    Ok(resolve_layout(&spec_for(target)?)?)
}
