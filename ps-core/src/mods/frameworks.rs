//! Framework release archives turned into install manifests.
use super::manifest::is_contained;
use super::types::{ArchiveEntry, FileRoute, InstallManifest, ModType, RouteKind, SourceHint};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FrameworkKey {
    Ue4ss,
    PalSchema,
    Amity,
}

impl FrameworkKey {
    pub const ALL: [FrameworkKey; 3] = [
        FrameworkKey::Ue4ss,
        FrameworkKey::PalSchema,
        FrameworkKey::Amity,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            FrameworkKey::Ue4ss => "ue4ss",
            FrameworkKey::PalSchema => "palschema",
            FrameworkKey::Amity => "amity",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|key| key.as_str() == text)
    }

    pub fn mod_id(self) -> String {
        format!("framework-{}", self.as_str())
    }

    pub fn display_name(self) -> &'static str {
        match self {
            FrameworkKey::Ue4ss => "UE4SS",
            FrameworkKey::PalSchema => "PalSchema",
            FrameworkKey::Amity => "PSAmity",
        }
    }
}

pub const UE4SS_VERSION_FILE: &str = "ue4ss.version";
pub const PALSCHEMA_VERSION_FILE: &str = "palschema.version";
const GENERATED_DIR: &str = ".palstudio";

#[derive(Debug)]
pub struct FrameworkPackage {
    pub manifest: InstallManifest,
    /// `(archive_path, contents)` the installer writes into the extracted tree
    /// before storing, so every route resolves to a real file.
    pub generated: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("the {framework} archive has no {missing}")]
pub struct FrameworkArchiveError {
    pub framework: &'static str,
    pub missing: &'static str,
}

/// The prefix of the entry that ends in `marker`: empty, or one wrapper directory.
fn prefix_of(paths: &[&str], marker: &str) -> Option<String> {
    let marker = marker.to_ascii_lowercase();
    paths
        .iter()
        .filter_map(|path| {
            let lower = path.to_ascii_lowercase();
            let prefix = lower.strip_suffix(&marker)?;
            let one_wrapper = prefix.is_empty()
                || (prefix.ends_with('/') && !prefix.trim_end_matches('/').contains('/'));
            one_wrapper.then(|| path[..prefix.len()].to_string())
        })
        .min_by_key(String::len)
}

/// `path` with `prefix` removed, case-insensitively, or `None` when `path`
/// does not start with `prefix` (a stray entry, or a same-length foreign one).
fn strip_prefix_ci<'a>(path: &'a str, prefix: &str) -> Option<&'a str> {
    if !path
        .to_ascii_lowercase()
        .starts_with(&prefix.to_ascii_lowercase())
    {
        return None;
    }
    path.get(prefix.len()..)
}

fn push(routes: &mut Vec<FileRoute>, archive_path: &str, kind: RouteKind, rel_path: String) {
    if is_contained(&rel_path) {
        routes.push(FileRoute {
            archive_path: archive_path.to_string(),
            rel_path,
            kind,
        });
    }
}

pub fn framework_package(
    key: FrameworkKey,
    version: &str,
    entries: &[ArchiveEntry],
) -> Result<FrameworkPackage, FrameworkArchiveError> {
    let files: Vec<&str> = entries
        .iter()
        .map(|e| e.path.as_str())
        .filter(|p| !p.ends_with('/'))
        .collect();
    let missing = |missing| FrameworkArchiveError {
        framework: key.as_str(),
        missing,
    };
    let mut routes = Vec::new();
    let mut generated = Vec::new();

    let folder_name = match key {
        FrameworkKey::Ue4ss => {
            let prefix =
                prefix_of(&files, "ue4ss/UE4SS.dll").ok_or_else(|| missing("ue4ss/UE4SS.dll"))?;
            if !files
                .iter()
                .any(|p| p.eq_ignore_ascii_case(&format!("{prefix}dwmapi.dll")))
            {
                return Err(missing("dwmapi.dll"));
            }
            for path in &files {
                let Some(rest) = strip_prefix_ci(path, &prefix) else {
                    continue;
                };
                let lower = rest.to_ascii_lowercase();
                if lower == "dwmapi.dll" {
                    push(
                        &mut routes,
                        path,
                        RouteKind::Binaries,
                        "dwmapi.dll".to_string(),
                    );
                } else if let Some(inner) = lower.strip_prefix("ue4ss/mods/") {
                    if inner != "mods.txt" && inner != "mods.json" {
                        push(
                            &mut routes,
                            path,
                            RouteKind::Ue4ss,
                            rest["ue4ss/mods/".len()..].to_string(),
                        );
                    }
                } else if lower.starts_with("ue4ss/")
                    && lower != format!("ue4ss/{UE4SS_VERSION_FILE}")
                {
                    push(
                        &mut routes,
                        path,
                        RouteKind::Ue4ssCore,
                        rest["ue4ss/".len()..].to_string(),
                    );
                }
            }
            let generated_path = format!("{GENERATED_DIR}/{UE4SS_VERSION_FILE}");
            push(
                &mut routes,
                &generated_path,
                RouteKind::Ue4ssCore,
                UE4SS_VERSION_FILE.to_string(),
            );
            generated.push((generated_path, version.to_string()));
            "ue4ss"
        }
        FrameworkKey::PalSchema => {
            let prefix = prefix_of(&files, "PalSchema/dlls/main.dll")
                .ok_or_else(|| missing("PalSchema/dlls/main.dll"))?;
            for path in &files {
                let Some(rest) = strip_prefix_ci(path, &prefix) else {
                    continue;
                };
                let lower = rest.to_ascii_lowercase();
                let Some(inner) = lower.strip_prefix("palschema/") else {
                    continue;
                };
                if inner.starts_with("mods/") || inner == PALSCHEMA_VERSION_FILE {
                    continue;
                }
                push(
                    &mut routes,
                    path,
                    RouteKind::Ue4ss,
                    format!("PalSchema/{}", &rest["palschema/".len()..]),
                );
            }
            let generated_path = format!("{GENERATED_DIR}/{PALSCHEMA_VERSION_FILE}");
            push(
                &mut routes,
                &generated_path,
                RouteKind::Ue4ss,
                format!("PalSchema/{PALSCHEMA_VERSION_FILE}"),
            );
            generated.push((generated_path, version.to_string()));
            "PalSchema"
        }
        FrameworkKey::Amity => {
            let prefix = prefix_of(&files, "PSAmity/dlls/main.dll")
                .ok_or_else(|| missing("PSAmity/dlls/main.dll"))?;
            for path in &files {
                let Some(rest) = strip_prefix_ci(path, &prefix) else {
                    continue;
                };
                let lower = rest.to_ascii_lowercase();
                let Some(inner) = lower.strip_prefix("psamity/") else {
                    continue;
                };
                if inner == "psamity.ini" {
                    continue;
                }
                push(
                    &mut routes,
                    path,
                    RouteKind::Ue4ss,
                    format!("PSAmity/{}", &rest["psamity/".len()..]),
                );
            }
            "PSAmity"
        }
    };

    Ok(FrameworkPackage {
        manifest: InstallManifest {
            folder_name: folder_name.to_string(),
            display_name: key.display_name().to_string(),
            mod_type: ModType::Framework,
            version: version.to_string(),
            routes,
            decisions: Vec::new(),
            platform_filtered: None,
            source: SourceHint {
                version: Some(version.to_string()),
                ..SourceHint::default()
            },
        },
        generated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(paths: &[&str]) -> Vec<ArchiveEntry> {
        paths
            .iter()
            .map(|p| ArchiveEntry {
                path: p.to_string(),
                size: 1,
            })
            .collect()
    }

    fn kind_of(package: &FrameworkPackage, rel_path: &str) -> Option<RouteKind> {
        package
            .manifest
            .routes
            .iter()
            .find(|r| r.rel_path == rel_path)
            .map(|r| r.kind)
    }

    const UE4SS: &[&str] = &[
        "dwmapi.dll",
        "ue4ss/",
        "ue4ss/UE4SS.dll",
        "ue4ss/UE4SS-settings.ini",
        "ue4ss/UE4SS_SDK_Backends/UE4SS.json",
        "ue4ss/Mods/mods.txt",
        "ue4ss/Mods/mods.json",
        "ue4ss/Mods/BPModLoaderMod/Scripts/main.lua",
        "ue4ss/Mods/Keybinds/Scripts/main.lua",
    ];

    #[test]
    fn ue4ss_places_the_proxy_the_runtime_and_the_bundled_mods() {
        let package = framework_package(FrameworkKey::Ue4ss, "2281fa31", &entries(UE4SS)).unwrap();
        assert_eq!(kind_of(&package, "dwmapi.dll"), Some(RouteKind::Binaries));
        assert_eq!(kind_of(&package, "UE4SS.dll"), Some(RouteKind::Ue4ssCore));
        assert_eq!(
            kind_of(&package, "UE4SS_SDK_Backends/UE4SS.json"),
            Some(RouteKind::Ue4ssCore)
        );
        assert_eq!(
            kind_of(&package, "Keybinds/Scripts/main.lua"),
            Some(RouteKind::Ue4ss)
        );
        assert_eq!(kind_of(&package, "mods.txt"), None);
        assert_eq!(kind_of(&package, "mods.json"), None);
        assert_eq!(
            kind_of(&package, "ue4ss.version"),
            Some(RouteKind::Ue4ssCore)
        );
        assert_eq!(
            package.generated,
            [(
                ".palstudio/ue4ss.version".to_string(),
                "2281fa31".to_string()
            )]
        );
        assert_eq!(package.manifest.mod_type, ModType::Framework);
    }

    #[test]
    fn one_wrapper_directory_is_stripped() {
        let wrapped: Vec<String> = UE4SS
            .iter()
            .map(|p| format!("UE4SS-Palworld/{p}"))
            .collect();
        let refs: Vec<&str> = wrapped.iter().map(String::as_str).collect();
        let package = framework_package(FrameworkKey::Ue4ss, "v", &entries(&refs)).unwrap();
        assert_eq!(kind_of(&package, "dwmapi.dll"), Some(RouteKind::Binaries));
        let deep = framework_package(
            FrameworkKey::Ue4ss,
            "v",
            &entries(&["a/b/dwmapi.dll", "a/b/ue4ss/UE4SS.dll"]),
        );
        assert!(deep.is_err());
    }

    #[test]
    fn a_missing_required_file_is_an_error() {
        let error = framework_package(FrameworkKey::Ue4ss, "v", &entries(&["ue4ss/UE4SS.dll"]))
            .unwrap_err();
        assert_eq!(error.missing, "dwmapi.dll");
        assert!(framework_package(
            FrameworkKey::PalSchema,
            "v",
            &entries(&["PalSchema/enabled.txt"])
        )
        .is_err());
    }

    #[test]
    fn palschema_skips_user_mods_and_amity_skips_its_ini() {
        let palschema = framework_package(
            FrameworkKey::PalSchema,
            "0.6.71",
            &entries(&[
                "PalSchema/dlls/main.dll",
                "PalSchema/enabled.txt",
                "PalSchema/mods/Mine/x.json",
            ]),
        )
        .unwrap();
        assert_eq!(
            kind_of(&palschema, "PalSchema/enabled.txt"),
            Some(RouteKind::Ue4ss)
        );
        assert_eq!(kind_of(&palschema, "PalSchema/mods/Mine/x.json"), None);
        assert_eq!(
            kind_of(&palschema, "PalSchema/palschema.version"),
            Some(RouteKind::Ue4ss)
        );

        let amity = framework_package(
            FrameworkKey::Amity,
            "0.2.0",
            &entries(&[
                "PSAmity/dlls/main.dll",
                "PSAmity/enabled.txt",
                "PSAmity/PSAmity.ini",
            ]),
        )
        .unwrap();
        assert_eq!(
            kind_of(&amity, "PSAmity/dlls/main.dll"),
            Some(RouteKind::Ue4ss)
        );
        assert_eq!(kind_of(&amity, "PSAmity/PSAmity.ini"), None);
        assert!(amity.generated.is_empty());
    }

    #[test]
    fn an_uncontained_entry_is_never_routed() {
        let package = framework_package(
            FrameworkKey::Ue4ss,
            "v",
            &entries(&["dwmapi.dll", "ue4ss/UE4SS.dll", "ue4ss/../../evil.dll"]),
        )
        .unwrap();
        assert!(package
            .manifest
            .routes
            .iter()
            .all(|r| !r.rel_path.contains("..")));
    }

    #[test]
    fn stray_entries_outside_the_wrapper_are_skipped_without_panicking() {
        let mut wrapped: Vec<String> = UE4SS
            .iter()
            .map(|p| format!("UE4SS-Palworld/{p}"))
            .collect();
        wrapped.push("README.md".to_string());
        wrapped.push("ドキュメント.txt".to_string());
        let refs: Vec<&str> = wrapped.iter().map(String::as_str).collect();

        let package = framework_package(FrameworkKey::Ue4ss, "v", &entries(&refs)).unwrap();

        assert_eq!(kind_of(&package, "dwmapi.dll"), Some(RouteKind::Binaries));
        assert!(package
            .manifest
            .routes
            .iter()
            .all(|r| r.rel_path != "README.md" && r.rel_path != "ドキュメント.txt"));
    }

    #[test]
    fn a_same_length_foreign_prefix_is_not_routed() {
        let package = framework_package(
            FrameworkKey::Ue4ss,
            "v",
            &entries(&["abc/dwmapi.dll", "abc/ue4ss/UE4SS.dll", "xyz/ue4ss/foo.dll"]),
        )
        .unwrap();

        assert_eq!(kind_of(&package, "dwmapi.dll"), Some(RouteKind::Binaries));
        assert!(package
            .manifest
            .routes
            .iter()
            .all(|r| r.rel_path != "foo.dll"));
    }

    #[test]
    fn palschema_and_amity_skip_stray_entries_without_panicking() {
        let palschema = framework_package(
            FrameworkKey::PalSchema,
            "v",
            &entries(&[
                "SomeWrapper/PalSchema/dlls/main.dll",
                "README.md",
                "ドキュメント.txt",
            ]),
        )
        .unwrap();
        assert_eq!(
            kind_of(&palschema, "PalSchema/dlls/main.dll"),
            Some(RouteKind::Ue4ss)
        );
        assert!(palschema
            .manifest
            .routes
            .iter()
            .all(|r| r.rel_path != "README.md" && r.rel_path != "ドキュメント.txt"));

        let amity = framework_package(
            FrameworkKey::Amity,
            "v",
            &entries(&[
                "SomeWrapper/PSAmity/dlls/main.dll",
                "README.md",
                "ドキュメント.txt",
            ]),
        )
        .unwrap();
        assert_eq!(
            kind_of(&amity, "PSAmity/dlls/main.dll"),
            Some(RouteKind::Ue4ss)
        );
        assert!(amity
            .manifest
            .routes
            .iter()
            .all(|r| r.rel_path != "README.md" && r.rel_path != "ドキュメント.txt"));
    }

    #[test]
    fn keys_round_trip() {
        for key in FrameworkKey::ALL {
            assert_eq!(FrameworkKey::parse(key.as_str()), Some(key));
        }
        assert_eq!(FrameworkKey::PalSchema.mod_id(), "framework-palschema");
        assert_eq!(FrameworkKey::parse("nope"), None);
    }
}
