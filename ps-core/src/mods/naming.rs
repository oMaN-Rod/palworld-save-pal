pub const GAME_PATH_SEGMENTS: &[&str] = &[
    "pal",
    "content",
    "paks",
    "~mods",
    "logicmods",
    "binaries",
    "win64",
    "wingdk",
    "ue4ss",
    "mods",
    "palschema",
];

pub const PALSCHEMA_FOLDERS: &[&str] = &[
    "resources",
    "enums",
    "pals",
    "npcs",
    "items",
    "skins",
    "appearance",
    "buildings",
    "raw",
    "blueprints",
    "helpguide",
    "spawns",
    "translations",
    "paks",
    "unique",
];

const FORBIDDEN_NAMES: &[&str] = &[
    "pal",
    "palworld",
    "mods",
    "win64",
    "wingdk",
    "binaries",
    "content",
    "paks",
    "~mods",
    "logicmods",
    "ue4ss",
    "palschema",
    "plugins",
    "scripts",
    "nativemods",
    "blueprints",
    "translations",
    "steam",
    "(steam)",
    "xbox",
    "(xbox)",
    "gdk",
    "(gdk)",
    "gamepass",
    "(gamepass)",
    "swapjson",
    "alterconfig",
    "release",
    "build",
    "dist",
    "dlls",
    "shared",
    "framework",
];

const FORBIDDEN_PHRASES: &[&str] = &[
    "mods folder",
    "mod folder",
    "ue4ss mods",
    "palschema mods",
    "mods directory",
];

const PLATFORM_WORDS: &[&str] = &["gamepass", "steam", "gdk", "xbox", "singleplayer", "sp"];

const UE4SS_MARKERS: &[&str] = &["scripts", "dlls", "enabled.txt"];

pub fn is_forbidden_name(name: &str) -> bool {
    let lower = name.trim().to_ascii_lowercase();
    if lower.is_empty() || FORBIDDEN_NAMES.contains(&lower.as_str()) {
        return true;
    }
    // A name that is a path, or that names a parent directory, cannot be a folder
    // name: joined onto a base directory it would escape it.
    if lower == "." || lower == ".." || lower.contains(['/', '\\', ':', '\0']) {
        return true;
    }
    // A control character in a name is a line break in every marker file that
    // lists one, and those formats have no escape.
    if lower.chars().any(char::is_control) {
        return true;
    }
    if FORBIDDEN_PHRASES.iter().any(|p| lower.contains(p)) {
        return true;
    }
    lower.len() >= 32 && lower.chars().all(|c| c.is_ascii_hexdigit())
}

fn is_year(token: &str) -> bool {
    token
        .parse::<u32>()
        .is_ok_and(|y| (2020..=2038).contains(&y))
}

fn archive_stem(file_name: &str) -> &str {
    match file_name.rsplit_once('.') {
        Some((stem, ext))
            if matches!(
                ext.to_ascii_lowercase().as_str(),
                "zip" | "7z" | "rar" | "pak"
            ) =>
        {
            stem
        }
        _ => file_name,
    }
}

fn is_id_token(token: &str) -> bool {
    !token.is_empty() && token.chars().all(|c| c.is_ascii_digit()) && !is_year(token)
}

/// Index range of the trailing run of numeric, non-year tokens — the
/// `<modid> <fileid>` pair Nexus appends to download names.
fn trailing_numeric_run(tokens: &[&str]) -> Option<(usize, usize)> {
    let last = tokens.iter().rposition(|t| is_id_token(t))?;
    let mut start = last;
    while start > 0 && is_id_token(tokens[start - 1]) {
        start -= 1;
    }
    Some((start, last))
}

pub fn nexus_ids_from_archive_name(file_name: &str) -> (Option<u32>, Option<String>) {
    let tokens: Vec<&str> = archive_stem(file_name).split_whitespace().collect();
    let Some((start, last)) = trailing_numeric_run(&tokens) else {
        return (None, None);
    };
    let run = &tokens[start..=last];
    (
        run.first().and_then(|t| t.parse::<u32>().ok()),
        run.get(1).map(|t| t.to_string()),
    )
}

/// Strips a Nexus download suffix (`<name> <modid> <fileid> <timestamp> <hash>`)
/// and trailing platform words from an archive file name.
pub fn clean_archive_name(file_name: &str) -> String {
    let tokens: Vec<&str> = archive_stem(file_name).split_whitespace().collect();
    let cut = trailing_numeric_run(&tokens)
        .map(|(start, _)| start)
        .unwrap_or(tokens.len());
    let mut kept: Vec<&str> = tokens[..cut].to_vec();
    while let Some(last) = kept.last() {
        if PLATFORM_WORDS.contains(&last.to_ascii_lowercase().as_str()) {
            kept.pop();
        } else {
            break;
        }
    }
    kept.join(" ").trim().to_string()
}

fn segments(path: &str) -> Vec<&str> {
    path.split('/').filter(|s| !s.is_empty()).collect()
}

fn marker_parent(paths: &[String], markers: &[&str]) -> Option<String> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for path in paths {
        let segs = segments(path);
        let lower: Vec<String> = segs.iter().map(|s| s.to_ascii_lowercase()).collect();
        let Some(idx) = lower.iter().rposition(|s| markers.contains(&s.as_str())) else {
            continue;
        };
        if idx == 0 {
            continue;
        }
        let parent = segs[idx - 1];
        if is_forbidden_name(parent) {
            continue;
        }
        match counts
            .iter_mut()
            .find(|(n, _)| n.eq_ignore_ascii_case(parent))
        {
            Some((_, c)) => *c += 1,
            None => counts.push((parent.to_string(), 1)),
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    counts.into_iter().next().map(|(n, _)| n)
}

/// Strategy order: UE4SS marker parent (so hybrids get the name `mods.txt`
/// needs), PalSchema folder parent, first non-forbidden non-leaf segment, pak stem.
pub fn detect_folder_name(paths: &[String]) -> Option<String> {
    if let Some(name) = marker_parent(paths, UE4SS_MARKERS) {
        return Some(name);
    }
    if let Some(name) = marker_parent(paths, PALSCHEMA_FOLDERS) {
        return Some(name);
    }
    for path in paths {
        let segs = segments(path);
        if segs.len() < 2 {
            continue;
        }
        // A schema data folder names the data inside a mod, never the mod.
        if let Some(seg) = segs[..segs.len() - 1].iter().find(|s| {
            !is_forbidden_name(s) && !PALSCHEMA_FOLDERS.contains(&s.to_ascii_lowercase().as_str())
        }) {
            return Some((*seg).to_string());
        }
    }
    paths.iter().find_map(|p| {
        let leaf = segments(p).last()?.to_string();
        let (stem, ext) = leaf.rsplit_once('.')?;
        (ext.eq_ignore_ascii_case("pak") && !is_forbidden_name(stem)).then(|| stem.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn clean_archive_name_strips_nexus_suffix_and_platform_words() {
        assert_eq!(
            clean_archive_name("Fishing Pond HR (Palschema) 2631 3 2026-07-11T11-06Z vcf5 (1).zip"),
            "Fishing Pond HR (Palschema)"
        );
        assert_eq!(
            clean_archive_name("Quality Of Life 4599 1 2026-07-30T23-44Z QXTyhgimX.zip"),
            "Quality Of Life"
        );
        assert_eq!(clean_archive_name("CoolMod Steam.7z"), "CoolMod");
        assert_eq!(clean_archive_name("Year 2024 Mod.zip"), "Year 2024 Mod");
        assert_eq!(clean_archive_name("plain.rar"), "plain");
    }

    #[test]
    fn nexus_ids_come_from_the_trailing_numeric_run() {
        assert_eq!(
            nexus_ids_from_archive_name("Cool 100 2 2026-01-01T00-00Z abc.zip"),
            (Some(100), Some("2".into()))
        );
        assert_eq!(
            nexus_ids_from_archive_name("Quality Of Life 4599 1 2026-07-30T23-44Z QXTyhgimX.zip"),
            (Some(4599), Some("1".into()))
        );
        assert_eq!(
            nexus_ids_from_archive_name("Year 2024 Mod.zip"),
            (None, None)
        );
        assert_eq!(nexus_ids_from_archive_name("Solo 77.zip"), (Some(77), None));
    }

    #[test]
    fn forbidden_names() {
        for name in [
            "pal",
            "Mods",
            "~mods",
            "(Steam)",
            "WinGDK",
            "UE4SS Mods Folder",
            "release",
            "Framework",
        ] {
            assert!(is_forbidden_name(name), "{name}");
        }
        for name in [".", "..", "a/b", "a\\b", "C:", "x\0y", "A\r\nOtherMod : 1"] {
            assert!(is_forbidden_name(name), "{name}");
        }
        assert!(is_forbidden_name("A\r\nOtherMod"));
        assert!(!is_forbidden_name("CoolMod"));
        assert!(is_forbidden_name("0123456789abcdef0123456789abcdef"));
    }

    #[test]
    fn ue4ss_marker_parent_wins_for_hybrids() {
        let p = paths(&[
            "Bundle/UE4SSName/Scripts/main.lua",
            "Bundle/SchemaName/pals/x.json",
        ]);
        assert_eq!(detect_folder_name(&p).as_deref(), Some("UE4SSName"));
    }

    #[test]
    fn palschema_parent_when_no_ue4ss_markers() {
        let p = paths(&["SchemaName/pals/x.json", "SchemaName/translations/en.json"]);
        assert_eq!(detect_folder_name(&p).as_deref(), Some("SchemaName"));
    }

    #[test]
    fn dlls_only_archive_is_named_by_dll_parent() {
        let p = paths(&["Mods/CppMod/dlls/main.dll", "Mods/CppMod/enabled.txt"]);
        assert_eq!(detect_folder_name(&p).as_deref(), Some("CppMod"));
    }

    #[test]
    fn first_non_forbidden_non_leaf_segment_otherwise() {
        let p = paths(&["(Steam)/Pal/Content/Paks/~mods/Cool_P.pak"]);
        assert_eq!(detect_folder_name(&p).as_deref(), Some("Cool_P"));
        let p = paths(&["Wrapper/Inner/readme.txt", "Wrapper/Inner/data.bin"]);
        assert_eq!(detect_folder_name(&p).as_deref(), Some("Wrapper"));
    }

    #[test]
    fn nested_same_name_uses_last_marker() {
        let p = paths(&["CoolMod/CoolMod/Scripts/main.lua"]);
        assert_eq!(detect_folder_name(&p).as_deref(), Some("CoolMod"));
    }

    #[test]
    fn returns_none_when_nothing_usable() {
        assert_eq!(detect_folder_name(&paths(&["readme.txt"])), None);
    }

    #[test]
    fn bare_schema_folders_do_not_name_the_mod() {
        let p = paths(&["pals/a.json", "translations/en.json"]);
        assert_eq!(detect_folder_name(&p), None);
    }
}
