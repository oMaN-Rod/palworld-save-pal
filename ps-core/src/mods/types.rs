use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModType {
    Ue4ss,
    PalSchema,
    Pak,
    LogicMods,
    NativeDll,
    Workshop,
    Hybrid,
    Framework,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RouteKind {
    Ue4ss,
    PalSchema,
    Pak,
    LogicMods,
    NativeDll,
    Workshop,
    /// A pak's `.ucas`/`.utoc` siblings are routed with the pak under its own
    /// kind, so a route never carries this kind; it exists for the deployment
    /// role a companion file takes. `Role::Companion` is set by the deployer from
    /// the file extension.
    Companion,
    Passthrough,
    Framework,
    /// Framework-only: never produced by analysis nor accepted from `modinfo.json`.
    Binaries,
    /// Framework-only: never produced by analysis nor accepted from `modinfo.json`.
    Ue4ssCore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Win64,
    WinGdk,
    Linux,
    Mac,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    Client,
    NativeServer,
    DockerServer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Ue4ssMode {
    Workshop,
    Standard,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    File,
    Companion,
    Marker,
    SharedMarker,
    /// The new version's copy of a file, written as `{name}.new` beside a
    /// deployed file the user edited.
    PreservedCopy,
}

/// One entry from an archive listing. `path` uses `/` separators exactly as
/// the archive stores it; directory entries end with `/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveEntry {
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRoute {
    pub archive_path: String,
    /// Relative to the route kind's base directory on any target
    /// (e.g. `CoolMod/Scripts/main.lua` under the UE4SS mods dir), and
    /// guaranteed contained within it by `build_manifest`.
    pub rel_path: String,
    pub kind: RouteKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Decision {
    PakDestination {
        file: String,
        default: RouteKind,
    },
    MultipleUe4ssRoots {
        roots: Vec<String>,
    },
    UnplacedFiles {
        files: Vec<String>,
    },
    NameConflict {
        proposed: String,
        existing_mod_id: String,
    },
    NexusVariant {
        existing_mod_id: String,
        file_name: String,
    },
}

/// The Nexus fields are a guess read from the archive's file name, not
/// provenance: a local archive named `MyMod 3.zip` yields a mod id of 3.
/// Authoritative provenance comes from the download record, and a mod identifier
/// must not be derived from these fields alone.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceHint {
    #[serde(default)]
    pub nexus_mod_id: Option<u32>,
    #[serde(default)]
    pub nexus_file_id: Option<String>,
    #[serde(default)]
    pub workshop_package: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    /// Whether any of a Workshop package's `InstallRule`s sets `IsServer`; a
    /// package with none deploys nothing on a dedicated server. `None` when the
    /// manifest predates the field or the mod is not a Workshop package.
    #[serde(default)]
    pub server_capable: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallManifest {
    pub folder_name: String,
    pub display_name: String,
    pub mod_type: ModType,
    pub version: String,
    pub routes: Vec<FileRoute>,
    #[serde(default)]
    pub decisions: Vec<Decision>,
    #[serde(default)]
    pub platform_filtered: Option<Platform>,
    #[serde(default)]
    pub source: SourceHint,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enums_serialize_snake_case() {
        assert_eq!(
            serde_json::to_string(&ModType::LogicMods).unwrap(),
            "\"logicmods\""
        );
        assert_eq!(
            serde_json::to_string(&RouteKind::NativeDll).unwrap(),
            "\"nativedll\""
        );
        assert_eq!(
            serde_json::to_string(&Platform::WinGdk).unwrap(),
            "\"wingdk\""
        );
        assert_eq!(serde_json::to_string(&Ue4ssMode::None).unwrap(), "\"none\"");
        assert_eq!(
            serde_json::to_string(&Role::SharedMarker).unwrap(),
            "\"shared_marker\""
        );
        assert_eq!(
            serde_json::to_string(&Role::PreservedCopy).unwrap(),
            "\"preserved_copy\""
        );
    }

    #[test]
    fn framework_only_kinds_serialize_lowercase() {
        assert_eq!(
            serde_json::to_string(&RouteKind::Binaries).unwrap(),
            "\"binaries\""
        );
        assert_eq!(
            serde_json::to_string(&RouteKind::Ue4ssCore).unwrap(),
            "\"ue4sscore\""
        );
    }

    #[test]
    fn manifest_round_trips_through_json() {
        let manifest = InstallManifest {
            folder_name: "CoolMod".into(),
            display_name: "Cool Mod".into(),
            mod_type: ModType::Ue4ss,
            version: "1.0".into(),
            routes: vec![FileRoute {
                archive_path: "CoolMod/Scripts/main.lua".into(),
                rel_path: "CoolMod/Scripts/main.lua".into(),
                kind: RouteKind::Ue4ss,
            }],
            decisions: vec![Decision::PakDestination {
                file: "x.pak".into(),
                default: RouteKind::Pak,
            }],
            platform_filtered: Some(Platform::Win64),
            source: SourceHint::default(),
        };
        let json = serde_json::to_string(&manifest).unwrap();
        let back: InstallManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(back, manifest);
    }
}
