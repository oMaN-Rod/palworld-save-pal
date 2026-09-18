use super::naming::PALSCHEMA_FOLDERS;
use super::types::{ArchiveEntry, ModType};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ArchiveFlags {
    pub has_lua: bool,
    pub has_dll: bool,
    pub has_pak: bool,
    pub has_json: bool,
    pub has_data_json: bool,
    pub in_logicmods: bool,
    pub has_palschema_folder: bool,
    pub has_palschema_json: bool,
    pub has_info_json: bool,
    pub has_modinfo: bool,
}

const METADATA_JSON: &[&str] = &[
    "info.json",
    "modinfo.json",
    "manifest.json",
    "metadata.json",
];

pub fn file_paths(entries: &[ArchiveEntry]) -> Vec<String> {
    entries
        .iter()
        .filter(|e| !e.path.ends_with('/'))
        .filter(|e| {
            e.path
                .rsplit('/')
                .next()
                .is_some_and(|leaf| leaf.contains('.'))
        })
        .map(|e| e.path.clone())
        .collect()
}

fn ext_of(path: &str) -> String {
    path.rsplit('/')
        .next()
        .and_then(|leaf| leaf.rsplit_once('.'))
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default()
}

pub fn analyze_flags(paths: &[String]) -> ArchiveFlags {
    let mut f = ArchiveFlags::default();
    for path in paths {
        let lower = path.to_ascii_lowercase();
        let segs: Vec<&str> = lower.split('/').filter(|s| !s.is_empty()).collect();
        let leaf = segs.last().copied().unwrap_or("");
        let dirs = &segs[..segs.len().saturating_sub(1)];
        match ext_of(&lower).as_str() {
            "lua" => f.has_lua = true,
            "dll" => f.has_dll = true,
            "pak" => f.has_pak = true,
            "json" | "jsonc" => {
                f.has_json = true;
                if leaf == "info.json" {
                    f.has_info_json = true;
                } else if leaf == "modinfo.json" {
                    f.has_modinfo = true;
                }
                if !METADATA_JSON.contains(&leaf) {
                    f.has_data_json = true;
                }
                let under_code = dirs.iter().any(|d| *d == "scripts" || *d == "dlls");
                let schema_dir = dirs
                    .iter()
                    .any(|d| PALSCHEMA_FOLDERS.contains(d) || d.contains("palschema"));
                if !METADATA_JSON.contains(&leaf) && !under_code && schema_dir {
                    f.has_palschema_json = true;
                }
            }
            _ => {}
        }
        if dirs.contains(&"logicmods") {
            f.in_logicmods = true;
        }
        if dirs.iter().any(|d| d.contains("palschema")) {
            f.has_palschema_folder = true;
        }
    }
    f
}

pub fn detect_mod_type(f: &ArchiveFlags) -> Option<ModType> {
    let ue4ss = f.has_lua || f.has_dll;
    let palschema = f.has_palschema_json;
    if f.has_info_json {
        return Some(ModType::Workshop);
    }
    if (ue4ss && palschema) || (ue4ss && f.has_pak) {
        return Some(ModType::Hybrid);
    }
    if palschema {
        return Some(ModType::PalSchema);
    }
    if ue4ss {
        return Some(ModType::Ue4ss);
    }
    if f.has_pak && f.in_logicmods {
        return Some(ModType::LogicMods);
    }
    if f.has_pak {
        return Some(ModType::Pak);
    }
    if f.has_data_json {
        return Some(ModType::PalSchema);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::ArchiveEntry;

    fn p(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn file_paths_drops_dirs_and_extensionless_leaves() {
        let entries = vec![
            ArchiveEntry {
                path: "CoolMod/".into(),
                size: 0,
            },
            ArchiveEntry {
                path: "CoolMod/Scripts/main.lua".into(),
                size: 10,
            },
            ArchiveEntry {
                path: "CoolMod/LICENSE".into(),
                size: 5,
            },
            ArchiveEntry {
                path: "CoolMod/enabled.txt".into(),
                size: 0,
            },
        ];
        assert_eq!(
            file_paths(&entries),
            p(&["CoolMod/Scripts/main.lua", "CoolMod/enabled.txt"])
        );
    }

    #[test]
    fn palschema_json_requires_schema_folder_or_palschema_segment() {
        let f = analyze_flags(&p(&["X/pals/dog.json"]));
        assert!(f.has_palschema_json);
        let f = analyze_flags(&p(&["X/modinfo.json"]));
        assert!(!f.has_palschema_json && f.has_json && !f.has_data_json && f.has_modinfo);
        let f = analyze_flags(&p(&["X/Scripts/config.json"]));
        assert!(!f.has_palschema_json);
        let f = analyze_flags(&p(&["PalSchema/mods/X/whatever.jsonc"]));
        assert!(f.has_palschema_json && f.has_palschema_folder);
    }

    #[test]
    fn info_json_and_logicmods_flags() {
        let f = analyze_flags(&p(&["Pkg/Info.json", "Pkg/LogicMods/Cool_P.pak"]));
        assert!(f.has_info_json && f.in_logicmods && f.has_pak);
    }

    #[test]
    fn ladder() {
        let t = |paths: &[&str]| detect_mod_type(&analyze_flags(&p(paths)));
        assert_eq!(t(&["Pkg/Info.json", "Pkg/x.lua"]), Some(ModType::Workshop));
        assert_eq!(
            t(&["M/Scripts/main.lua", "M/pals/x.json"]),
            Some(ModType::Hybrid)
        );
        assert_eq!(
            t(&["M/Scripts/main.lua", "Cool_P.pak"]),
            Some(ModType::Hybrid)
        );
        assert_eq!(t(&["M/pals/x.json"]), Some(ModType::PalSchema));
        assert_eq!(t(&["M/dlls/main.dll"]), Some(ModType::Ue4ss));
        assert_eq!(t(&["M/Scripts/main.lua"]), Some(ModType::Ue4ss));
        assert_eq!(t(&["LogicMods/Cool_P.pak"]), Some(ModType::LogicMods));
        assert_eq!(
            t(&["Cool_P.pak", "Cool_P.utoc", "Cool_P.ucas"]),
            Some(ModType::Pak)
        );
        assert_eq!(t(&["M/weird.json"]), Some(ModType::PalSchema));
        assert_eq!(t(&["modinfo.json", "thing.bin"]), None);
        assert_eq!(t(&["readme.md", "shot.png"]), None);
    }
}
