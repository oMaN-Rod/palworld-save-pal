use std::collections::HashSet;

use super::analyzer::{analyze_flags, detect_mod_type, file_paths, ArchiveFlags};
use super::modinfo::{ModInfoJson, WorkshopInfo};
use super::naming::{
    clean_archive_name, detect_folder_name, is_forbidden_name, nexus_ids_from_archive_name,
    PALSCHEMA_FOLDERS,
};
use super::types::{
    ArchiveEntry, Decision, FileRoute, InstallManifest, ModType, Platform, RouteKind, SourceHint,
    TargetKind,
};

/// `detect_mod_type` reports `Workshop` for any archive carrying an `Info.json`,
/// but Workshop routing happens only when `workshop_info` is supplied. A caller
/// that does not parse `Info.json` therefore gets a Workshop mod type with
/// heuristically routed files and no Workshop route, so a caller that branches on
/// `mod_type` must supply `workshop_info` whenever the archive has an `Info.json`.
pub struct AnalyzeInput<'a> {
    pub entries: &'a [ArchiveEntry],
    pub archive_name: &'a str,
    pub target_platform: Platform,
    /// Accepted for the next plan's use; routing does not read it.
    pub target_kind: TargetKind,
    pub modinfo: Option<&'a ModInfoJson>,
    pub workshop_info: Option<&'a WorkshopInfo>,
    pub custom_name: Option<&'a str>,
}

const STEAM_TAGS: &[&str] = &["(steam)", "steam", "win64"];
const XBOX_TAGS: &[&str] = &[
    "(xbox)",
    "xbox",
    "(gdk)",
    "gdk",
    "wingdk",
    "(gamepass)",
    "gamepass",
];
const WRAPPER_SEGMENTS: &[&str] = &[
    "(steam)",
    "steam",
    "(xbox)",
    "xbox",
    "(gdk)",
    "gdk",
    "(gamepass)",
    "gamepass",
    "win64",
    "wingdk",
    "release",
    "build",
    "dist",
];
const WRAPPER_PHRASES: &[&str] = &[
    "mods folder",
    "mod folder",
    "ue4ss mods",
    "palschema mods",
    "mods directory",
];
const MARKER_SEGMENTS: &[&str] = &["mods", "ue4ss", "palschema"];
const PASSTHROUGH_ALLOWED: &[&str] = &[
    "pal/content/paks/~mods/",
    "pal/content/paks/logicmods/",
    "pal/binaries/win64/ue4ss/",
    "pal/binaries/win64/mods/",
    "pal/binaries/win64/nativemods/",
    "pal/binaries/wingdk/ue4ss/",
    "pal/binaries/wingdk/mods/",
    "pal/binaries/wingdk/nativemods/",
];
const DOC_EXTENSIONS: &[&str] = &["md", "url", "png", "jpg", "jpeg", "gif", "pdf"];
const FUNCTIONAL_TXT: &[&str] = &["enabled.txt", "mod.txt", "info.txt"];
const DOC_TXT_STEMS: &[&str] = &[
    "readme",
    "read me",
    "install",
    "changelog",
    "license",
    "licence",
];
const FALLBACK_VERSION: &str = "unversioned";

fn segs(path: &str) -> Vec<&str> {
    path.split('/').filter(|s| !s.is_empty()).collect()
}

fn lower_segs(path: &str) -> Vec<String> {
    segs(path)
        .into_iter()
        .map(|s| s.to_ascii_lowercase())
        .collect()
}

fn ext(leaf: &str) -> String {
    leaf.rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default()
}

fn is_wrapper(seg: &str) -> bool {
    WRAPPER_SEGMENTS.contains(&seg) || WRAPPER_PHRASES.iter().any(|p| seg.contains(p))
}

fn has_numeric_prefix(seg: &str) -> bool {
    seg.len() > 4 && seg.as_bytes()[..3].iter().all(u8::is_ascii_digit) && seg.as_bytes()[3] == b'_'
}

fn is_config_like(stem: &str) -> bool {
    let s = stem.to_ascii_lowercase();
    s.starts_with("config") || s.starts_with("setting")
}

/// The tag sets for the target's own platform and the other one. Returning both
/// avoids identifying a side by comparing slice addresses, which a promoted
/// `const` does not guarantee.
fn platform_sides(platform: Platform) -> (&'static [&'static str], &'static [&'static str]) {
    match platform {
        Platform::WinGdk => (XBOX_TAGS, STEAM_TAGS),
        Platform::Win64 | Platform::Linux | Platform::Mac => (STEAM_TAGS, XBOX_TAGS),
    }
}

fn ue4ss_roots(paths: &[String]) -> Vec<String> {
    let mut roots: Vec<String> = Vec::new();
    for path in paths {
        let s = segs(path);
        let lower = lower_segs(path);
        let idx = lower.iter().enumerate().rposition(|(i, seg)| {
            let is_dir = i + 1 < lower.len();
            (is_dir && (seg == "scripts" || seg == "dlls")) || (!is_dir && seg == "enabled.txt")
        });
        let Some(idx) = idx else { continue };
        if idx == 0 || is_forbidden_name(s[idx - 1]) {
            continue;
        }
        let root = s[idx - 1];
        if !roots.iter().any(|r| r.eq_ignore_ascii_case(root)) {
            roots.push(root.to_string());
        }
    }
    roots.sort();
    roots
}

fn strip_wrappers<'a>(parts: &[&'a str], folder_name: &str) -> Vec<&'a str> {
    let mut start = 0;
    while start + 1 < parts.len() && is_wrapper(&parts[start].to_ascii_lowercase()) {
        start += 1;
    }
    let mut rest: Vec<&str> = parts[start..].to_vec();
    let dirs = rest.len().saturating_sub(1);
    if let Some(i) = (0..dirs)
        .rev()
        .find(|&i| MARKER_SEGMENTS.contains(&rest[i].to_ascii_lowercase().as_str()))
    {
        let mut cut = i + 1;
        if rest[i].eq_ignore_ascii_case("palschema")
            && cut < dirs
            && rest[cut].eq_ignore_ascii_case("mods")
        {
            cut += 1;
        }
        rest = rest[cut..].to_vec();
    }
    if rest.len() > 1 && rest[0].eq_ignore_ascii_case(folder_name) {
        rest = rest[1..].to_vec();
    }
    rest
}

fn game_tree_subpath(parts: &[&str], platform: Platform) -> Option<String> {
    let lower: Vec<String> = parts.iter().map(|s| s.to_ascii_lowercase()).collect();
    let idx = (0..lower.len().saturating_sub(2)).rev().find(|&i| {
        lower[i] == "pal" && (lower[i + 1] == "content" || lower[i + 1] == "binaries")
    })?;
    let mut out: Vec<String> = parts[idx..].iter().map(|s| s.to_string()).collect();
    for seg in out.iter_mut() {
        let l = seg.to_ascii_lowercase();
        match (l.as_str(), platform) {
            ("win64", Platform::WinGdk) => *seg = "WinGDK".to_string(),
            ("wingdk", Platform::Win64 | Platform::Linux | Platform::Mac) => {
                *seg = "Win64".to_string()
            }
            _ => {}
        }
    }
    Some(out.join("/"))
}

/// Documentation and app metadata that must never be deployed.
fn is_doc(rest: &[&str]) -> bool {
    let leaf = rest.last().copied().unwrap_or("");
    let e = ext(leaf);
    if DOC_EXTENSIONS.contains(&e.as_str()) || leaf.eq_ignore_ascii_case("modinfo.json") {
        return true;
    }
    if e != "txt" || FUNCTIONAL_TXT.contains(&leaf.to_ascii_lowercase().as_str()) {
        return false;
    }
    let stem = leaf
        .rsplit_once('.')
        .map(|(s, _)| s)
        .unwrap_or(leaf)
        .to_ascii_lowercase();
    rest.len() == 1 || DOC_TXT_STEMS.iter().any(|d| stem.contains(d))
}

/// A route may only name a location inside its kind's base directory. Anything
/// else — a parent segment, an absolute path, a Windows separator or drive — is
/// refused rather than routed, because the deployer resolves `rel_path` against
/// that base and a parent segment would escape it.
pub(crate) fn is_contained(rel_path: &str) -> bool {
    // Windows strips a trailing dot or space from the final component, so `".. "`
    // and `"...."` resolve to a parent directory even though neither equals `".."`.
    let usable = |s: &str| {
        !s.is_empty()
            && !s.ends_with('.')
            && !s.ends_with(' ')
            && !s.chars().all(|c| c == '.')
            && !s.contains(':')
    };
    !rel_path.is_empty()
        && !rel_path.starts_with('/')
        && !rel_path.contains('\\')
        && rel_path.split('/').all(usable)
}

/// The type of an archive the ladder could not classify. The routes actually
/// emitted are the only evidence left, and an archive that routed nothing has
/// none at all.
fn type_of_routes(routes: &[FileRoute]) -> ModType {
    let Some(first) = routes.first().map(|r| r.kind) else {
        return ModType::Pak;
    };
    if routes.iter().any(|r| r.kind != first) {
        return ModType::Hybrid;
    }
    match first {
        RouteKind::Ue4ss => ModType::Ue4ss,
        RouteKind::PalSchema => ModType::PalSchema,
        RouteKind::LogicMods => ModType::LogicMods,
        RouteKind::Pak => ModType::Pak,
        _ => ModType::Hybrid,
    }
}

fn primary_kind(flags: &ArchiveFlags) -> Option<RouteKind> {
    if flags.has_lua || flags.has_dll {
        Some(RouteKind::Ue4ss)
    } else if flags.has_palschema_json {
        Some(RouteKind::PalSchema)
    } else {
        None
    }
}

/// Route every file in an archive to a location inside its kind's base directory.
///
/// Every `rel_path` in the returned manifest is a contained relative path: no
/// parent segment, no leading separator, no Windows separator or drive letter. A
/// file that cannot be placed that way is reported in `Decision::UnplacedFiles`
/// instead of routed, so a deployer may join a route onto its base directory
/// without re-checking. Archive contents are untrusted; this is the only place
/// that containment is decided.
pub fn build_manifest(input: &AnalyzeInput) -> InstallManifest {
    let all_paths = file_paths(input.entries);

    let (paths, platform_filtered) = {
        let has = |tags: &[&str]| {
            all_paths
                .iter()
                .any(|p| lower_segs(p).iter().any(|s| tags.contains(&s.as_str())))
        };
        if has(STEAM_TAGS) && has(XBOX_TAGS) {
            let (_, other) = platform_sides(input.target_platform);
            let kept: Vec<String> = all_paths
                .iter()
                .filter(|p| !lower_segs(p).iter().any(|s| other.contains(&s.as_str())))
                .cloned()
                .collect();
            (kept, Some(input.target_platform))
        } else {
            (all_paths.clone(), None)
        }
    };

    let flags = analyze_flags(&paths);
    let ladder = detect_mod_type(&flags);
    let cleaned = clean_archive_name(input.archive_name);
    let (nexus_mod_id, nexus_file_id) = nexus_ids_from_archive_name(input.archive_name);

    if let Some(info) = input
        .workshop_info
        .filter(|w| !is_forbidden_name(&w.package_name))
    {
        let package = info.package_name.clone();
        let root_dir: Option<String> = paths.iter().find_map(|p| {
            let s = segs(p);
            (s.len() == 2 && s[1].eq_ignore_ascii_case("info.json")).then(|| s[0].to_string())
        });
        let mut routes = Vec::new();
        let mut unplaced = Vec::new();
        for p in &paths {
            let s = segs(p);
            let rest: Vec<&str> = match &root_dir {
                Some(r) if s.len() > 1 && s[0].eq_ignore_ascii_case(r) => s[1..].to_vec(),
                _ => s,
            };
            let rel_path = format!("{package}/{}", rest.join("/"));
            if !is_contained(&rel_path) {
                unplaced.push(p.clone());
                continue;
            }
            routes.push(FileRoute {
                archive_path: p.clone(),
                rel_path,
                kind: RouteKind::Workshop,
            });
        }
        return InstallManifest {
            folder_name: package.clone(),
            display_name: if info.mod_name.is_empty() {
                package.clone()
            } else {
                info.mod_name.clone()
            },
            mod_type: ModType::Workshop,
            version: if info.version.is_empty() {
                FALLBACK_VERSION.into()
            } else {
                info.version.clone()
            },
            routes,
            decisions: if unplaced.is_empty() {
                Vec::new()
            } else {
                vec![Decision::UnplacedFiles { files: unplaced }]
            },
            platform_filtered,
            source: SourceHint {
                nexus_mod_id,
                nexus_file_id,
                workshop_package: Some(package),
                version: Some(info.version.clone()).filter(|v| !v.is_empty()),
                author: Some(info.author.clone()).filter(|a| !a.is_empty()),
                server_capable: Some(info.install_rules.iter().any(|rule| rule.is_server)),
            },
        };
    }

    let custom_routes: Vec<(String, RouteKind)> = input
        .modinfo
        .map(|m| {
            m.routes
                .iter()
                .filter(|r| !matches!(r.kind, RouteKind::Binaries | RouteKind::Ue4ssCore))
                .map(|r| (r.path.to_ascii_lowercase(), r.kind))
                .collect()
        })
        .unwrap_or_default();

    let folder_name = [
        input.modinfo.and_then(|m| m.name.clone()),
        detect_folder_name(&paths),
        Some(cleaned.clone()),
    ]
    .into_iter()
    .flatten()
    .find(|n| !is_forbidden_name(n))
    .unwrap_or_else(|| "mod".to_string());

    let display_name = input
        .custom_name
        .map(str::to_string)
        .or_else(|| input.modinfo.and_then(|m| m.name.clone()))
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| {
            if cleaned.is_empty() {
                folder_name.clone()
            } else {
                cleaned.clone()
            }
        });

    let roots = ue4ss_roots(&paths);
    let archive_has_scripts_dir = paths
        .iter()
        .any(|p| lower_segs(p).iter().rev().skip(1).any(|s| s == "scripts"));
    let pak_stems: HashSet<String> = paths
        .iter()
        .filter(|p| ext(segs(p).last().unwrap_or(&"")) == "pak")
        .filter_map(|p| {
            segs(p)
                .last()
                .and_then(|l| l.rsplit_once('.'))
                .map(|(s, _)| s.to_ascii_lowercase())
        })
        .collect();
    let primary = primary_kind(&flags);

    let mut routes = Vec::new();
    let mut decisions = Vec::new();
    let mut unplaced = Vec::new();

    for path in &paths {
        let parts = segs(path);
        let lower = lower_segs(path);
        let leaf = *parts.last().unwrap_or(&"");
        let leaf_lower = leaf.to_ascii_lowercase();
        let e = ext(leaf);
        let original_dirs = &lower[..lower.len().saturating_sub(1)];

        if let Some(sub) = game_tree_subpath(&parts, input.target_platform) {
            let rest: Vec<&str> = sub.split('/').collect();
            if !is_doc(&rest) {
                // A game-relative path may only reach the directories mods are
                // loaded from, never the engine's own files or the executable.
                let sub_lower = sub.to_ascii_lowercase();
                let allowed = PASSTHROUGH_ALLOWED.iter().any(|p| sub_lower.starts_with(p));
                if allowed && is_contained(&sub) {
                    routes.push(FileRoute {
                        archive_path: path.clone(),
                        rel_path: sub,
                        kind: RouteKind::Passthrough,
                    });
                } else {
                    unplaced.push(path.clone());
                }
            }
            continue;
        }

        let mut rest = strip_wrappers(&parts, &folder_name);
        let wrapper_stripped = rest.len() < parts.len();
        // A bundle folder above a UE4SS root (`Pack/ModA/Scripts/…`) is dropped by
        // re-anchoring on the root; the search stops at the first `scripts`/`dlls`
        // so an internal folder that repeats the mod name is left alone.
        {
            let dirs_lower: Vec<String> = rest[..rest.len().saturating_sub(1)]
                .iter()
                .map(|s| s.to_ascii_lowercase())
                .collect();
            let code_idx = dirs_lower
                .iter()
                .position(|d| d == "scripts" || d == "dlls")
                .unwrap_or(dirs_lower.len());
            if let Some(i) =
                (0..code_idx).find(|&i| roots.iter().any(|r| r.eq_ignore_ascii_case(rest[i])))
            {
                rest = rest[i..].to_vec();
            }
        }
        let rest_dirs: Vec<String> = rest[..rest.len().saturating_sub(1)]
            .iter()
            .map(|s| s.to_ascii_lowercase())
            .collect();
        let first_is_root = rest.len() > 1 && roots.iter().any(|r| r.eq_ignore_ascii_case(rest[0]));
        let in_logicmods = original_dirs.iter().any(|d| d == "logicmods");
        // A segment named `palschema` is proof. A bare `paks`/`resources`/`items`
        // folder is only schema data when the archive actually carries schema
        // json, or a pak mod shipped as `Mod/Paks/x.pak` would route into the
        // PalSchema tree and never load.
        let schema_owned = original_dirs.iter().any(|d| d.contains("palschema"))
            || (flags.has_palschema_json
                && original_dirs
                    .iter()
                    .any(|d| PALSCHEMA_FOLDERS.contains(&d.as_str())));

        let custom = custom_routes
            .iter()
            .find(|(p, _)| *p == path.to_ascii_lowercase())
            .map(|(_, k)| *k);
        let kind = match custom {
            Some(k) => k,
            None => match e.as_str() {
                "pak" | "ucas" | "utoc" => {
                    let stem = leaf
                        .rsplit_once('.')
                        .map(|(s, _)| s.to_ascii_lowercase())
                        .unwrap_or_default();
                    if e != "pak" && !pak_stems.contains(&stem) {
                        RouteKind::Companion
                    } else if in_logicmods {
                        RouteKind::LogicMods
                    } else if schema_owned {
                        RouteKind::PalSchema
                    } else {
                        RouteKind::Pak
                    }
                }
                "lua" | "dll" => RouteKind::Ue4ss,
                // `rest_dirs` is always a suffix of `original_dirs`, so a bare
                // schema-folder name here is already covered by `schema_owned`
                // (and its json-evidence gate) with no separate check needed.
                _ if schema_owned => RouteKind::PalSchema,
                _ if rest_dirs.iter().any(|d| d == "scripts" || d == "dlls")
                    || first_is_root
                    || leaf_lower == "enabled.txt" =>
                {
                    RouteKind::Ue4ss
                }
                _ if wrapper_stripped => primary.unwrap_or(RouteKind::Passthrough),
                _ => RouteKind::Passthrough,
            },
        };

        let rel_path = match kind {
            RouteKind::Ue4ss => {
                if first_is_root {
                    rest.join("/")
                } else if rest.len() == 1
                    && e == "lua"
                    && !archive_has_scripts_dir
                    && !is_config_like(leaf.rsplit_once('.').map(|(s, _)| s).unwrap_or(leaf))
                {
                    format!("{folder_name}/Scripts/{leaf}")
                } else {
                    format!("{folder_name}/{}", rest.join("/"))
                }
            }
            RouteKind::PalSchema => {
                // Anchor on the first schema folder: the segment before it is the
                // schema mod's own folder (kept, prefix and all); with none, the
                // archive's folder name is prepended.
                let schema_idx = rest_dirs
                    .iter()
                    .position(|d| PALSCHEMA_FOLDERS.contains(&d.as_str()));
                match schema_idx {
                    Some(i) if i > 0 => rest[i - 1..].join("/"),
                    Some(_) => format!("{folder_name}/{}", rest.join("/")),
                    None if rest.len() > 1
                        && (has_numeric_prefix(rest[0]) || !rest[0].contains('.')) =>
                    {
                        rest.join("/")
                    }
                    None => format!("{folder_name}/{}", rest.join("/")),
                }
            }
            RouteKind::Pak | RouteKind::LogicMods => {
                if e == "pak" && custom.is_none() && !in_logicmods {
                    decisions.push(Decision::PakDestination {
                        file: leaf.to_string(),
                        default: RouteKind::Pak,
                    });
                }
                // A declared route is used as given, even when its target
                // happens to carry a documentation extension.
                if custom.is_none() && is_doc(&rest) {
                    continue;
                }
                leaf.to_string()
            }
            RouteKind::Companion | RouteKind::Binaries | RouteKind::Ue4ssCore => {
                unplaced.push(path.clone());
                continue;
            }
            RouteKind::Passthrough => {
                if !is_doc(&rest) {
                    unplaced.push(path.clone());
                }
                continue;
            }
            RouteKind::NativeDll | RouteKind::Workshop | RouteKind::Framework => {
                format!("{folder_name}/{}", rest.join("/"))
            }
        };
        if !is_contained(&rel_path) {
            unplaced.push(path.clone());
            continue;
        }
        routes.push(FileRoute {
            archive_path: path.clone(),
            rel_path,
            kind,
        });
    }

    if roots.len() > 1 {
        decisions.push(Decision::MultipleUe4ssRoots {
            roots: roots.clone(),
        });
    }
    if !unplaced.is_empty() {
        decisions.push(Decision::UnplacedFiles { files: unplaced });
    }
    if routes.is_empty() && decisions.is_empty() {
        decisions.push(Decision::UnplacedFiles {
            files: paths.clone(),
        });
    }

    InstallManifest {
        folder_name,
        display_name,
        mod_type: ladder.unwrap_or_else(|| type_of_routes(&routes)),
        version: input
            .modinfo
            .and_then(|m| m.version.clone())
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| FALLBACK_VERSION.to_string()),
        routes,
        decisions,
        platform_filtered,
        source: SourceHint {
            nexus_mod_id,
            nexus_file_id,
            workshop_package: None,
            version: input.modinfo.and_then(|m| m.version.clone()),
            author: None,
            server_capable: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::{Decision, ModType, Platform, RouteKind, TargetKind};

    fn entries(items: &[&str]) -> Vec<ArchiveEntry> {
        items
            .iter()
            .map(|p| ArchiveEntry {
                path: p.to_string(),
                size: 1,
            })
            .collect()
    }

    fn input<'a>(entries: &'a [ArchiveEntry], name: &'a str) -> AnalyzeInput<'a> {
        AnalyzeInput {
            entries,
            archive_name: name,
            target_platform: Platform::Win64,
            target_kind: TargetKind::Client,
            modinfo: None,
            workshop_info: None,
            custom_name: None,
        }
    }

    fn route<'a>(m: &'a InstallManifest, archive_path: &str) -> &'a FileRoute {
        m.routes
            .iter()
            .find(|r| r.archive_path == archive_path)
            .unwrap_or_else(|| panic!("no route for {archive_path}: {:?}", m.routes))
    }

    #[test]
    fn plain_ue4ss_mod_keeps_folder() {
        let e = entries(&[
            "CoolMod/Scripts/main.lua",
            "CoolMod/enabled.txt",
            "CoolMod/readme.md",
        ]);
        let m = build_manifest(&input(&e, "CoolMod.zip"));
        assert_eq!(m.mod_type, ModType::Ue4ss);
        assert_eq!(m.folder_name, "CoolMod");
        assert_eq!(
            route(&m, "CoolMod/Scripts/main.lua").rel_path,
            "CoolMod/Scripts/main.lua"
        );
        assert_eq!(route(&m, "CoolMod/Scripts/main.lua").kind, RouteKind::Ue4ss);
        assert_eq!(
            route(&m, "CoolMod/enabled.txt").rel_path,
            "CoolMod/enabled.txt"
        );
        assert_eq!(route(&m, "CoolMod/readme.md").rel_path, "CoolMod/readme.md");
        assert!(m.decisions.is_empty());
    }

    #[test]
    fn nested_same_name_folder_is_not_flattened() {
        let e = entries(&[
            "CoolMod/Scripts/CoolMod/util.lua",
            "CoolMod/Scripts/main.lua",
        ]);
        let m = build_manifest(&input(&e, "x.zip"));
        assert_eq!(
            route(&m, "CoolMod/Scripts/CoolMod/util.lua").rel_path,
            "CoolMod/Scripts/CoolMod/util.lua"
        );
    }

    #[test]
    fn wrapper_dirs_and_ue4ss_mods_prefix_are_stripped() {
        let e = entries(&["(Steam)/UE4SS Mods Folder/ue4ss/Mods/CoolMod/Scripts/main.lua"]);
        let m = build_manifest(&input(&e, "x.zip"));
        assert_eq!(m.folder_name, "CoolMod");
        assert_eq!(
            route(
                &m,
                "(Steam)/UE4SS Mods Folder/ue4ss/Mods/CoolMod/Scripts/main.lua"
            )
            .rel_path,
            "CoolMod/Scripts/main.lua"
        );
    }

    #[test]
    fn full_game_tree_routes_by_game_relative_subpath() {
        let e = entries(&["Palworld/Pal/Binaries/Win64/ue4ss/Mods/CoolMod/Scripts/main.lua"]);
        let mut i = input(&e, "x.zip");
        i.target_platform = Platform::WinGdk;
        let m = build_manifest(&i);
        let r = route(
            &m,
            "Palworld/Pal/Binaries/Win64/ue4ss/Mods/CoolMod/Scripts/main.lua",
        );
        assert_eq!(r.kind, RouteKind::Passthrough);
        assert_eq!(
            r.rel_path,
            "Pal/Binaries/WinGDK/ue4ss/Mods/CoolMod/Scripts/main.lua"
        );
    }

    #[test]
    fn dual_platform_archive_keeps_only_target_side() {
        let e = entries(&[
            "(Steam)/Cool_P.pak",
            "(Xbox)/Cool_P.pak",
            "(Xbox)/Cool_P.utoc",
            "(Xbox)/Cool_P.ucas",
        ]);
        let m = build_manifest(&input(&e, "Cool 100 2 2026-01-01T00-00Z abc.zip"));
        assert_eq!(m.platform_filtered, Some(Platform::Win64));
        assert_eq!(m.routes.len(), 1);
        assert_eq!(m.routes[0].archive_path, "(Steam)/Cool_P.pak");
        assert_eq!(m.source.nexus_mod_id, Some(100));
        assert_eq!(m.source.nexus_file_id.as_deref(), Some("2"));
    }

    #[test]
    fn a_custom_name_changes_only_the_display_name() {
        use crate::mods::ids::{mod_id, ModIdInput};

        for (items, archive) in [
            (
                &["QualityOfLife/Scripts/main.lua"][..],
                "Quality Of Life.zip",
            ),
            (&["main.lua", "config.lua"][..], "Cool Mod.zip"),
        ] {
            let e = entries(items);
            let plain = build_manifest(&input(&e, archive));
            let mut named_input = input(&e, archive);
            named_input.custom_name = Some("My Renamed Mod");
            let named = build_manifest(&named_input);

            assert_eq!(named.display_name, "My Renamed Mod");
            assert_eq!(named.folder_name, plain.folder_name);
            assert_eq!(named.routes, plain.routes);
            let id = |m: &InstallManifest| {
                mod_id(&ModIdInput::Local {
                    folder: &m.folder_name,
                    mod_type: m.mod_type,
                })
            };
            assert_eq!(id(&named), id(&plain));
        }
    }

    #[test]
    fn bare_root_lua_is_wrapped_into_scripts_unless_config_like() {
        let e = entries(&["main.lua", "config.lua"]);
        let m = build_manifest(&input(&e, "Cool Mod.zip"));
        assert_eq!(m.folder_name, "Cool Mod");
        assert_eq!(route(&m, "main.lua").rel_path, "Cool Mod/Scripts/main.lua");
        assert_eq!(route(&m, "config.lua").rel_path, "Cool Mod/config.lua");
    }

    #[test]
    fn palschema_numeric_prefix_is_kept_and_schema_dirs_get_folder() {
        let e = entries(&["000_Foo/pals/a.json"]);
        let m = build_manifest(&input(&e, "x.zip"));
        assert_eq!(m.mod_type, ModType::PalSchema);
        assert_eq!(
            route(&m, "000_Foo/pals/a.json").rel_path,
            "000_Foo/pals/a.json"
        );
        let e = entries(&["pals/a.json", "translations/en.json"]);
        let m = build_manifest(&input(&e, "Schema Thing.zip"));
        assert_eq!(
            route(&m, "pals/a.json").rel_path,
            "Schema Thing/pals/a.json"
        );
    }

    #[test]
    fn dlls_only_is_ue4ss() {
        let e = entries(&["CppMod/dlls/main.dll"]);
        let m = build_manifest(&input(&e, "x.zip"));
        assert_eq!(m.mod_type, ModType::Ue4ss);
        assert_eq!(
            route(&m, "CppMod/dlls/main.dll").rel_path,
            "CppMod/dlls/main.dll"
        );
    }

    #[test]
    fn paks_get_destination_decision_and_companions_follow() {
        let e = entries(&[
            "Cool_P.pak",
            "Cool_P.utoc",
            "Cool_P.ucas",
            "screenshot.png",
            "readme.txt",
        ]);
        let m = build_manifest(&input(&e, "x.zip"));
        assert_eq!(m.mod_type, ModType::Pak);
        assert_eq!(m.routes.len(), 3);
        assert_eq!(route(&m, "Cool_P.utoc").kind, RouteKind::Pak);
        assert_eq!(route(&m, "Cool_P.utoc").rel_path, "Cool_P.utoc");
        assert!(
            matches!(m.decisions[0], Decision::PakDestination { ref file, default: RouteKind::Pak } if file == "Cool_P.pak")
        );
    }

    #[test]
    fn a_paks_folder_alone_does_not_make_a_pak_schema_owned() {
        let e = entries(&["MyMod/Paks/Cool_P.pak"]);
        let m = build_manifest(&input(&e, "x.zip"));
        assert_eq!(m.mod_type, ModType::Pak);
        assert_eq!(route(&m, "MyMod/Paks/Cool_P.pak").kind, RouteKind::Pak);
        assert_eq!(route(&m, "MyMod/Paks/Cool_P.pak").rel_path, "Cool_P.pak");
        assert!(matches!(m.decisions[0], Decision::PakDestination { .. }));
    }

    #[test]
    fn a_resources_folder_alone_stays_with_its_ue4ss_mod() {
        let e = entries(&["CoolMod/Scripts/main.lua", "CoolMod/resources/icon.png"]);
        let m = build_manifest(&input(&e, "x.zip"));
        assert_eq!(
            route(&m, "CoolMod/resources/icon.png").kind,
            RouteKind::Ue4ss
        );
        assert_eq!(
            route(&m, "CoolMod/resources/icon.png").rel_path,
            "CoolMod/resources/icon.png"
        );
    }

    #[test]
    fn logicmods_folder_routes_pak_without_decision() {
        let e = entries(&["LogicMods/Cool_P.pak"]);
        let m = build_manifest(&input(&e, "x.zip"));
        assert_eq!(route(&m, "LogicMods/Cool_P.pak").kind, RouteKind::LogicMods);
        assert!(m.decisions.is_empty());
    }

    #[test]
    fn multiple_ue4ss_roots_are_kept_and_flagged() {
        let e = entries(&[
            "Bundle/ModA/Scripts/main.lua",
            "Bundle/ModB/Scripts/main.lua",
        ]);
        let m = build_manifest(&input(&e, "x.zip"));
        assert_eq!(
            route(&m, "Bundle/ModA/Scripts/main.lua").rel_path,
            "ModA/Scripts/main.lua"
        );
        assert_eq!(
            route(&m, "Bundle/ModB/Scripts/main.lua").rel_path,
            "ModB/Scripts/main.lua"
        );
        assert!(m
            .decisions
            .iter()
            .any(|d| matches!(d, Decision::MultipleUe4ssRoots { roots } if roots.len() == 2)));
    }

    #[test]
    fn hybrid_routes_each_component_and_names_by_ue4ss_root() {
        let e = entries(&[
            "Pack/CoolMod/Scripts/main.lua",
            "Pack/SchemaPart/pals/a.json",
            "Pack/Cool_P.pak",
        ]);
        let m = build_manifest(&input(&e, "x.zip"));
        assert_eq!(m.mod_type, ModType::Hybrid);
        assert_eq!(m.folder_name, "CoolMod");
        assert_eq!(
            route(&m, "Pack/SchemaPart/pals/a.json").kind,
            RouteKind::PalSchema
        );
        assert_eq!(
            route(&m, "Pack/SchemaPart/pals/a.json").rel_path,
            "SchemaPart/pals/a.json"
        );
        assert_eq!(route(&m, "Pack/Cool_P.pak").kind, RouteKind::Pak);
    }

    #[test]
    fn modinfo_routes_override_heuristics() {
        let mi = crate::mods::parse_modinfo(
            r#"{"name":"Declared","routes":[{"path":"stuff/thing.bin","kind":"ue4ss"}]}"#,
        )
        .unwrap();
        let e = entries(&["stuff/thing.bin", "modinfo.json"]);
        let mut i = input(&e, "x.zip");
        i.modinfo = Some(&mi);
        let m = build_manifest(&i);
        assert_eq!(m.folder_name, "Declared");
        assert_eq!(route(&m, "stuff/thing.bin").kind, RouteKind::Ue4ss);
        assert_eq!(
            route(&m, "stuff/thing.bin").rel_path,
            "Declared/stuff/thing.bin"
        );
    }

    #[test]
    fn modinfo_cannot_declare_framework_only_kinds() {
        let mi = crate::mods::parse_modinfo(
            r#"{"name":"Sneaky","routes":[{"path":"dwmapi.dll","kind":"binaries"},{"path":"UE4SS.dll","kind":"ue4sscore"}]}"#,
        )
        .unwrap();
        let e = entries(&["dwmapi.dll", "UE4SS.dll", "modinfo.json"]);
        let mut i = input(&e, "x.zip");
        i.modinfo = Some(&mi);
        let m = build_manifest(&i);
        assert!(
            m.routes
                .iter()
                .all(|r| !matches!(r.kind, RouteKind::Binaries | RouteKind::Ue4ssCore)),
            "{:?}",
            m.routes
        );
    }

    #[test]
    fn workshop_package_routes_whole_package_under_package_name() {
        let wi = crate::mods::parse_workshop_info(
            r#"{"PackageName":"CoolPkg","ModName":"Cool","Version":"3"}"#,
        )
        .unwrap();
        let e = entries(&["CoolPkg/Info.json", "CoolPkg/Mods/Cool/Scripts/main.lua"]);
        let mut i = input(&e, "x.zip");
        i.workshop_info = Some(&wi);
        let m = build_manifest(&i);
        assert_eq!(m.mod_type, ModType::Workshop);
        assert_eq!(m.folder_name, "CoolPkg");
        assert_eq!(m.version, "3");
        assert_eq!(route(&m, "CoolPkg/Info.json").kind, RouteKind::Workshop);
        assert_eq!(route(&m, "CoolPkg/Info.json").rel_path, "CoolPkg/Info.json");
        assert_eq!(
            route(&m, "CoolPkg/Mods/Cool/Scripts/main.lua").rel_path,
            "CoolPkg/Mods/Cool/Scripts/main.lua"
        );
    }

    #[test]
    fn a_workshop_package_records_whether_any_install_rule_is_for_a_server() {
        let e = entries(&["CoolPkg/Info.json", "CoolPkg/Mods/Cool/Scripts/main.lua"]);
        let server_capable = |json: &str| {
            let wi = crate::mods::parse_workshop_info(json).unwrap();
            let mut i = input(&e, "x.zip");
            i.workshop_info = Some(&wi);
            build_manifest(&i).source.server_capable
        };
        assert_eq!(
            server_capable(r#"{"PackageName":"CoolPkg","InstallRule":[{"Type":"Lua"}]}"#),
            Some(false)
        );
        assert_eq!(
            server_capable(
                r#"{"PackageName":"CoolPkg","InstallRule":[{"Type":"UE4SS"},{"Type":"UE4SS","IsServer":true}]}"#
            ),
            Some(true)
        );
        assert_eq!(server_capable(r#"{"PackageName":"CoolPkg"}"#), Some(false));

        let plain = entries(&["CoolMod/Scripts/main.lua"]);
        assert_eq!(
            build_manifest(&input(&plain, "x.zip"))
                .source
                .server_capable,
            None
        );
    }

    #[test]
    fn a_declared_pak_route_survives_a_documentation_extension() {
        let mi = crate::mods::parse_modinfo(
            r#"{"name":"Declared","routes":[{"path":"Art/Map_P.png","kind":"pak"}]}"#,
        )
        .unwrap();
        let e = entries(&["Art/Map_P.png"]);
        let mut i = input(&e, "x.zip");
        i.modinfo = Some(&mi);
        let m = build_manifest(&i);
        assert_eq!(route(&m, "Art/Map_P.png").kind, RouteKind::Pak);
        assert_eq!(route(&m, "Art/Map_P.png").rel_path, "Map_P.png");
    }

    #[test]
    fn an_undecided_archive_takes_the_type_of_its_declared_routes() {
        let mi = crate::mods::parse_modinfo(
            r#"{"name":"Declared","routes":[{"path":"stuff/thing.bin","kind":"ue4ss"}]}"#,
        )
        .unwrap();
        let e = entries(&["stuff/thing.bin", "modinfo.json"]);
        let mut i = input(&e, "x.zip");
        i.modinfo = Some(&mi);
        let m = build_manifest(&i);
        assert_eq!(m.mod_type, ModType::Ue4ss);
    }

    #[test]
    fn an_undecided_archive_of_declared_paks_is_a_pak_mod() {
        let mi = crate::mods::parse_modinfo(
            r#"{"name":"Declared","routes":[{"path":"stuff/thing.bin","kind":"pak"}]}"#,
        )
        .unwrap();
        let e = entries(&["stuff/thing.bin", "modinfo.json"]);
        let mut i = input(&e, "x.zip");
        i.modinfo = Some(&mi);
        let m = build_manifest(&i);
        assert_eq!(m.mod_type, ModType::Pak);
    }

    #[test]
    fn an_archive_with_nothing_to_install_always_says_so() {
        let e = entries(&["readme.md", "shot.png"]);
        let m = build_manifest(&input(&e, "x.zip"));
        assert!(m.routes.is_empty());
        assert!(!m.decisions.is_empty());
    }

    #[test]
    fn a_game_tree_archive_cannot_reach_the_games_own_pak_containers() {
        let e = entries(&[
            "Palworld/Pal/Content/Paks/Pal-WindowsNoEditor.pak",
            "Palworld/Pal/Content/Paks/~mods/Cool_P.pak",
        ]);
        let m = build_manifest(&input(&e, "x.zip"));
        assert_eq!(m.routes.len(), 1);
        assert_eq!(
            m.routes[0].archive_path,
            "Palworld/Pal/Content/Paks/~mods/Cool_P.pak"
        );
        assert!(m.decisions.iter().any(|d| matches!(
            d,
            Decision::UnplacedFiles { files }
                if files == &vec!["Palworld/Pal/Content/Paks/Pal-WindowsNoEditor.pak".to_string()]
        )));
    }

    #[test]
    fn a_game_tree_archive_cannot_target_the_executable() {
        let e = entries(&[
            "Palworld/Pal/Binaries/Win64/Palworld-Win64-Shipping.exe",
            "Palworld/Pal/Binaries/Win64/ue4ss/Mods/CoolMod/Scripts/main.lua",
        ]);
        let m = build_manifest(&input(&e, "x.zip"));
        assert_eq!(m.routes.len(), 1);
        assert_eq!(
            m.routes[0].archive_path,
            "Palworld/Pal/Binaries/Win64/ue4ss/Mods/CoolMod/Scripts/main.lua"
        );
        assert!(m
            .decisions
            .iter()
            .any(|d| matches!(d, Decision::UnplacedFiles { .. })));
    }

    #[test]
    fn traversal_entries_are_never_routed() {
        for entry in [
            "../../../../Windows/Temp/x.lua",
            "a/../../x.lua",
            "..\\..\\x.lua",
            "/abs/x.lua",
            "CoolMod/Scripts/.. ",
            "CoolMod/Scripts/....",
            "CoolMod/Scripts/x.lua.",
        ] {
            let e = entries(&[entry]);
            let m = build_manifest(&input(&e, "x.zip"));
            assert!(
                m.routes.iter().all(|r| is_contained(&r.rel_path)),
                "{entry}: {:?}",
                m.routes
            );
        }
    }

    #[test]
    fn a_workshop_package_cannot_name_a_parent_directory() {
        let wi =
            crate::mods::parse_workshop_info(r#"{"PackageName":"..","ModName":"Evil"}"#).unwrap();
        let e = entries(&["../Info.json", "../evil.lua"]);
        let mut i = input(&e, "x.zip");
        i.workshop_info = Some(&wi);
        let m = build_manifest(&i);
        assert!(
            m.routes.iter().all(|r| is_contained(&r.rel_path)),
            "{:?}",
            m.routes
        );
    }

    #[test]
    fn unrecognised_archive_is_unplaced() {
        // `notes.txt` at the archive root is documentation and is dropped silently;
        // `data.bin` has nowhere to go.
        let e = entries(&["data.bin", "notes.txt"]);
        let m = build_manifest(&input(&e, "x.zip"));
        assert!(m.routes.is_empty());
        assert!(
            matches!(&m.decisions[0], Decision::UnplacedFiles { files } if files == &vec!["data.bin".to_string()])
        );
    }
}
