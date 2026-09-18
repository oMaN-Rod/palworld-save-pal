use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::types::{Platform, RouteKind, TargetKind, Ue4ssMode};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct LayoutOverrides {
    pub binaries_dir: Option<String>,
    pub ue4ss_dir: Option<String>,
    pub ue4ss_mods_dir: Option<String>,
    pub paks_mods_dir: Option<String>,
    pub logicmods_dir: Option<String>,
    pub nativemods_dir: Option<String>,
    pub workshop_local_dir: Option<String>,
    pub palmodsettings_ini: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetSpec {
    pub kind: TargetKind,
    pub root: String,
    pub platform: Platform,
    pub ue4ss_mode: Ue4ssMode,
    #[serde(default)]
    pub overrides: LayoutOverrides,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetLayout {
    pub root: PathBuf,
    pub binaries_dir: Option<PathBuf>,
    pub executable: Option<PathBuf>,
    pub ue4ss_dir: Option<PathBuf>,
    pub ue4ss_mods_dir: Option<PathBuf>,
    pub mods_txt: Option<PathBuf>,
    pub paks_mods_dir: PathBuf,
    pub logicmods_dir: PathBuf,
    pub palschema_mods_dir: Option<PathBuf>,
    pub nativemods_dir: Option<PathBuf>,
    pub palmodsettings_ini: Option<PathBuf>,
    pub workshop_local_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LayoutError {
    #[error("platform not supported yet: {0:?}")]
    UnsupportedPlatform(Platform),
    #[error("layout requires an override for {0}")]
    MissingOverride(&'static str),
}

fn pick(override_: &Option<String>, default: Option<PathBuf>) -> Option<PathBuf> {
    override_.as_ref().map(PathBuf::from).or(default)
}

pub fn resolve_layout(spec: &TargetSpec) -> Result<TargetLayout, LayoutError> {
    let root = PathBuf::from(&spec.root);
    let o = &spec.overrides;

    let (binaries_default, exe_name): (Option<PathBuf>, Option<String>) =
        match (spec.kind, spec.platform) {
            (_, Platform::Mac) => return Err(LayoutError::UnsupportedPlatform(Platform::Mac)),
            (TargetKind::DockerServer, _) => (None, None),
            (TargetKind::Client, Platform::WinGdk) => (
                Some(root.join("Pal").join("Binaries").join("WinGDK")),
                Some("Palworld-WinGDK-Shipping.exe".into()),
            ),
            (TargetKind::Client, _) => (
                Some(root.join("Pal").join("Binaries").join("Win64")),
                Some("Palworld-Win64-Shipping.exe".into()),
            ),
            (TargetKind::NativeServer, _) => (
                Some(root.join("Pal").join("Binaries").join("Win64")),
                Some("PalServer-Win64-Shipping.exe".into()),
            ),
        };
    let binaries_dir = pick(&o.binaries_dir, binaries_default);
    let executable = match (&binaries_dir, exe_name) {
        (Some(b), Some(n)) => Some(b.join(n)),
        _ => None,
    };

    let ue4ss_default = match (spec.kind, spec.ue4ss_mode) {
        (TargetKind::DockerServer, _) | (_, Ue4ssMode::None) => None,
        (TargetKind::Client, Ue4ssMode::Workshop) => {
            Some(root.join("Mods").join("NativeMods").join("UE4SS"))
        }
        (_, _) => binaries_dir.as_ref().map(|b| b.join("ue4ss")),
    };
    let ue4ss_dir = pick(&o.ue4ss_dir, ue4ss_default);

    let ue4ss_mods_default = match spec.kind {
        TargetKind::DockerServer => Some(root.join("mods")),
        TargetKind::NativeServer => binaries_dir.as_ref().map(|b| b.join("ue4ss").join("Mods")),
        TargetKind::Client => ue4ss_dir.as_ref().map(|u| u.join("Mods")),
    };
    let ue4ss_mods_dir = pick(&o.ue4ss_mods_dir, ue4ss_mods_default);

    let paks_default = match spec.kind {
        TargetKind::DockerServer => root.join("paks"),
        _ => root.join("Pal").join("Content").join("Paks").join("~mods"),
    };
    let logic_default = match spec.kind {
        TargetKind::DockerServer => root.join("logicmods"),
        _ => root
            .join("Pal")
            .join("Content")
            .join("Paks")
            .join("LogicMods"),
    };
    let native_default = match spec.kind {
        TargetKind::DockerServer => Some(root.join("nativemods")),
        TargetKind::NativeServer => binaries_dir.as_ref().map(|b| b.join("NativeMods")),
        TargetKind::Client => None,
    };
    let workshop_default = match spec.kind {
        TargetKind::DockerServer => None,
        _ => Some(root.join("Mods").join("Workshop")),
    };
    let ini_default = match spec.kind {
        TargetKind::DockerServer => None,
        _ => Some(root.join("Mods").join("PalModSettings.ini")),
    };

    Ok(TargetLayout {
        mods_txt: ue4ss_mods_dir.as_ref().map(|m| m.join("mods.txt")),
        palschema_mods_dir: ue4ss_mods_dir
            .as_ref()
            .map(|m| m.join("PalSchema").join("mods")),
        root,
        binaries_dir,
        executable,
        ue4ss_dir,
        ue4ss_mods_dir,
        paks_mods_dir: o
            .paks_mods_dir
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or(paks_default),
        logicmods_dir: o
            .logicmods_dir
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or(logic_default),
        nativemods_dir: pick(&o.nativemods_dir, native_default),
        palmodsettings_ini: pick(&o.palmodsettings_ini, ini_default),
        workshop_local_dir: pick(&o.workshop_local_dir, workshop_default),
    })
}

impl TargetLayout {
    pub fn base_for(&self, kind: RouteKind) -> Option<&Path> {
        match kind {
            RouteKind::Ue4ss => self.ue4ss_mods_dir.as_deref(),
            RouteKind::PalSchema => self.palschema_mods_dir.as_deref(),
            RouteKind::Pak => Some(self.paks_mods_dir.as_path()),
            RouteKind::LogicMods => Some(self.logicmods_dir.as_path()),
            RouteKind::NativeDll => self.nativemods_dir.as_deref(),
            RouteKind::Workshop => self.workshop_local_dir.as_deref(),
            RouteKind::Passthrough => Some(self.root.as_path()),
            RouteKind::Binaries => self.binaries_dir.as_deref(),
            RouteKind::Ue4ssCore => self.ue4ss_dir.as_deref(),
            RouteKind::Companion | RouteKind::Framework => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn spec(kind: TargetKind, platform: Platform, mode: Ue4ssMode) -> TargetSpec {
        TargetSpec {
            kind,
            root: "/g".into(),
            platform,
            ue4ss_mode: mode,
            overrides: LayoutOverrides::default(),
        }
    }

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn client_standard_win64() {
        let l = resolve_layout(&spec(
            TargetKind::Client,
            Platform::Win64,
            Ue4ssMode::Standard,
        ))
        .unwrap();
        assert_eq!(l.binaries_dir, Some(p("/g/Pal/Binaries/Win64")));
        assert_eq!(
            l.executable,
            Some(p("/g/Pal/Binaries/Win64/Palworld-Win64-Shipping.exe"))
        );
        assert_eq!(l.ue4ss_dir, Some(p("/g/Pal/Binaries/Win64/ue4ss")));
        assert_eq!(
            l.ue4ss_mods_dir,
            Some(p("/g/Pal/Binaries/Win64/ue4ss/Mods"))
        );
        assert_eq!(
            l.mods_txt,
            Some(p("/g/Pal/Binaries/Win64/ue4ss/Mods/mods.txt"))
        );
        assert_eq!(
            l.palschema_mods_dir,
            Some(p("/g/Pal/Binaries/Win64/ue4ss/Mods/PalSchema/mods"))
        );
        assert_eq!(l.paks_mods_dir, p("/g/Pal/Content/Paks/~mods"));
        assert_eq!(l.logicmods_dir, p("/g/Pal/Content/Paks/LogicMods"));
        assert_eq!(l.palmodsettings_ini, Some(p("/g/Mods/PalModSettings.ini")));
        assert_eq!(l.workshop_local_dir, Some(p("/g/Mods/Workshop")));
        assert_eq!(l.nativemods_dir, None);
    }

    #[test]
    fn client_workshop_mode_and_wingdk() {
        let l = resolve_layout(&spec(
            TargetKind::Client,
            Platform::WinGdk,
            Ue4ssMode::Workshop,
        ))
        .unwrap();
        assert_eq!(l.binaries_dir, Some(p("/g/Pal/Binaries/WinGDK")));
        assert_eq!(
            l.executable,
            Some(p("/g/Pal/Binaries/WinGDK/Palworld-WinGDK-Shipping.exe"))
        );
        assert_eq!(l.ue4ss_dir, Some(p("/g/Mods/NativeMods/UE4SS")));
        assert_eq!(
            l.mods_txt,
            Some(p("/g/Mods/NativeMods/UE4SS/Mods/mods.txt"))
        );
    }

    #[test]
    fn client_without_ue4ss_has_no_ue4ss_paths() {
        let l =
            resolve_layout(&spec(TargetKind::Client, Platform::Linux, Ue4ssMode::None)).unwrap();
        assert_eq!(l.ue4ss_dir, None);
        assert_eq!(l.ue4ss_mods_dir, None);
        assert_eq!(l.mods_txt, None);
        assert_eq!(l.palschema_mods_dir, None);
        assert_eq!(l.paks_mods_dir, p("/g/Pal/Content/Paks/~mods"));
    }

    #[test]
    fn mac_is_unsupported_for_now() {
        assert_eq!(
            resolve_layout(&spec(TargetKind::Client, Platform::Mac, Ue4ssMode::None)).unwrap_err(),
            LayoutError::UnsupportedPlatform(Platform::Mac)
        );
    }

    #[test]
    fn native_server_loads_ue4ss_mods_beside_the_runtime() {
        let l = resolve_layout(&spec(
            TargetKind::NativeServer,
            Platform::Win64,
            Ue4ssMode::Standard,
        ))
        .unwrap();
        assert_eq!(
            l.executable,
            Some(p("/g/Pal/Binaries/Win64/PalServer-Win64-Shipping.exe"))
        );
        assert_eq!(
            l.ue4ss_mods_dir,
            Some(p("/g/Pal/Binaries/Win64/ue4ss/Mods"))
        );
        assert_eq!(
            l.mods_txt,
            Some(p("/g/Pal/Binaries/Win64/ue4ss/Mods/mods.txt"))
        );
        assert_eq!(
            l.nativemods_dir,
            Some(p("/g/Pal/Binaries/Win64/NativeMods"))
        );
        assert_eq!(l.workshop_local_dir, Some(p("/g/Mods/Workshop")));
    }

    #[test]
    fn docker_server_defaults_to_host_dirs() {
        let l = resolve_layout(&spec(
            TargetKind::DockerServer,
            Platform::Linux,
            Ue4ssMode::None,
        ))
        .unwrap();
        assert_eq!(l.binaries_dir, None);
        assert_eq!(l.ue4ss_mods_dir, Some(p("/g/mods")));
        assert_eq!(l.mods_txt, Some(p("/g/mods/mods.txt")));
        assert_eq!(l.paks_mods_dir, p("/g/paks"));
        assert_eq!(l.logicmods_dir, p("/g/logicmods"));
        assert_eq!(l.nativemods_dir, Some(p("/g/nativemods")));
        assert_eq!(l.palmodsettings_ini, None);
    }

    #[test]
    fn overrides_replace_bases_and_derived_paths_follow() {
        let mut s = spec(TargetKind::DockerServer, Platform::Linux, Ue4ssMode::None);
        s.overrides.ue4ss_mods_dir = Some("/elsewhere/Mods".into());
        s.overrides.paks_mods_dir = Some("D:/CustomPaks".into());
        let l = resolve_layout(&s).unwrap();
        assert_eq!(l.mods_txt, Some(p("/elsewhere/Mods/mods.txt")));
        assert_eq!(
            l.palschema_mods_dir,
            Some(p("/elsewhere/Mods/PalSchema/mods"))
        );
        assert_eq!(l.paks_mods_dir, p("D:/CustomPaks"));
    }

    #[test]
    fn overriding_a_derived_key_is_rejected_at_parse_time() {
        let err = serde_json::from_str::<LayoutOverrides>(r#"{"mods_txt":"/x"}"#).unwrap_err();
        assert!(err.to_string().contains("unknown field"));
    }

    #[test]
    fn base_for_maps_route_kinds() {
        let l = resolve_layout(&spec(
            TargetKind::Client,
            Platform::Win64,
            Ue4ssMode::Standard,
        ))
        .unwrap();
        assert_eq!(l.base_for(RouteKind::Ue4ss), l.ue4ss_mods_dir.as_deref());
        assert_eq!(
            l.base_for(RouteKind::PalSchema),
            l.palschema_mods_dir.as_deref()
        );
        assert_eq!(l.base_for(RouteKind::Pak), Some(l.paks_mods_dir.as_path()));
        assert_eq!(
            l.base_for(RouteKind::LogicMods),
            Some(l.logicmods_dir.as_path())
        );
        assert_eq!(l.base_for(RouteKind::Passthrough), Some(l.root.as_path()));
        assert_eq!(
            l.base_for(RouteKind::Workshop),
            l.workshop_local_dir.as_deref()
        );
        assert_eq!(l.base_for(RouteKind::NativeDll), None);
        assert_eq!(l.base_for(RouteKind::Framework), None);
    }

    #[test]
    fn framework_kinds_resolve_to_binaries_and_the_ue4ss_runtime() {
        let l = resolve_layout(&spec(
            TargetKind::Client,
            Platform::Win64,
            Ue4ssMode::Standard,
        ))
        .unwrap();
        assert_eq!(
            l.base_for(RouteKind::Binaries),
            Some(Path::new("/g/Pal/Binaries/Win64"))
        );
        assert_eq!(
            l.base_for(RouteKind::Ue4ssCore),
            Some(Path::new("/g/Pal/Binaries/Win64/ue4ss"))
        );
        let none =
            resolve_layout(&spec(TargetKind::Client, Platform::Win64, Ue4ssMode::None)).unwrap();
        assert_eq!(none.base_for(RouteKind::Ue4ssCore), None);
        let docker = resolve_layout(&spec(
            TargetKind::DockerServer,
            Platform::Linux,
            Ue4ssMode::None,
        ))
        .unwrap();
        assert_eq!(docker.base_for(RouteKind::Binaries), None);
    }
}
