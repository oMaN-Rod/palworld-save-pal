//! Pure rules behind mod conflict detection: JSONC cleanup, the PalSchema
//! rows a file edits, which paks overwrite the same asset, and which
//! frameworks a mod or Workshop dependency needs. Nothing here reads a file,
//! a process, or the network; callers hand in text and values already read.

use serde::Serialize;

use super::frameworks::FrameworkKey;
use super::types::{FileRoute, InstallManifest, ModType, RouteKind};

const GAMEPASS_PAK_DIRS: &[&str] = &["pal/content/paks/~mods/", "pal/content/paks/logicmods/"];

fn ends_with_ci(value: &str, suffix: &str) -> bool {
    value.len() >= suffix.len() && value[value.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
}

/// The full `rel_path` minus its extension, lowercased so two routes that
/// differ only in case still match.
fn path_stem_ci(rel_path: &str) -> String {
    match rel_path.rfind('.') {
        Some(index) => rel_path[..index].to_ascii_lowercase(),
        None => rel_path.to_ascii_lowercase(),
    }
}

/// Whether `route` is a pak this Game Pass/overlap logic must consider: a
/// `Pak`/`LogicMods` route ending `.pak`, or a game-tree archive dropped
/// straight into the game's own `~mods`/`LogicMods` pak directories.
pub fn is_pak_route(route: &FileRoute) -> bool {
    match route.kind {
        RouteKind::Pak | RouteKind::LogicMods => ends_with_ci(&route.rel_path, ".pak"),
        RouteKind::Passthrough => {
            let lower = route.rel_path.to_ascii_lowercase();
            lower.ends_with(".pak") && GAMEPASS_PAK_DIRS.iter().any(|dir| lower.starts_with(dir))
        }
        _ => false,
    }
}

/// The `.utoc` route that makes `route` an IoStore pak, matched on the full
/// `rel_path` (not just the file's basename) so paks of the same name in
/// different directories are never confused for siblings. `route` only
/// counts as IoStore when a matching `.ucas` route exists too.
pub fn iostore_sibling<'a>(
    manifest: &'a InstallManifest,
    route: &FileRoute,
) -> Option<&'a FileRoute> {
    if !is_pak_route(route) {
        return None;
    }
    let base = path_stem_ci(&route.rel_path);
    let sibling_with_ext = |extension: &str| {
        manifest.routes.iter().find(|other| {
            other.kind == route.kind
                && ends_with_ci(&other.rel_path, extension)
                && path_stem_ci(&other.rel_path) == base
        })
    };
    let utoc = sibling_with_ext(".utoc")?;
    sibling_with_ext(".ucas")?;
    Some(utoc)
}

/// Strips a PalSchema `.json`/`.jsonc` file down to parseable JSON: drops a
/// leading BOM, removes `//` and `/* */` comments outside string literals,
/// and drops a trailing comma before `}` or `]`. Newlines inside a removed
/// comment are kept so line numbers in the source still line up.
pub fn strip_jsonc(text: &str) -> String {
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let without_comments = strip_comments(text);
    strip_trailing_commas(&without_comments)
}

fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if c == '\\' {
                if let Some(escaped) = chars.next() {
                    out.push(escaped);
                }
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                out.push(c);
            }
            '/' if chars.peek() == Some(&'/') => {
                chars.next();
                for next in chars.by_ref() {
                    if next == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut prev_star = false;
                for next in chars.by_ref() {
                    if prev_star && next == '/' {
                        break;
                    }
                    if next == '\n' {
                        out.push('\n');
                    }
                    prev_star = next == '*';
                }
            }
            _ => out.push(c),
        }
    }
    out
}

fn strip_trailing_commas(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut in_string = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_string {
            out.push(c);
            if c == '\\' && i + 1 < chars.len() {
                out.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if c == '"' {
            in_string = true;
            out.push(c);
            i += 1;
            continue;
        }
        if c == ',' {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j < chars.len() && (chars[j] == '}' || chars[j] == ']') {
                i += 1;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

const ROW_CATEGORIES: &[&str] = &[
    "appearance",
    "buildings",
    "enums",
    "helpguide",
    "items",
    "pals",
    "skins",
    "spawns",
    "translations",
];

/// The PalSchema rows a `{folder}/{category}/…/{file}.json` file edits, read
/// from its parsed content. `raw` and `blueprints` key by their nested table
/// name; the flat categories key by their top-level entry. A file directly
/// under the folder, or in a category outside both sets, has no rows.
pub fn palschema_row_keys(rel_path: &str, text: &str) -> Result<Vec<String>, serde_json::Error> {
    let stripped = strip_jsonc(text);
    let value: serde_json::Value = serde_json::from_str(&stripped)?;

    let mut keys = Vec::new();
    let normalized = rel_path.replace('\\', "/");
    let segments: Vec<&str> = normalized.trim_start_matches('/').split('/').collect();
    if segments.len() >= 3 {
        let category = segments[1].to_ascii_lowercase();
        if let Some(obj) = value.as_object() {
            if category == "raw" || category == "blueprints" {
                for (top, v) in obj {
                    if let Some(sub) = v.as_object() {
                        for sub_key in sub.keys() {
                            keys.push(format!("{top}::{sub_key}"));
                        }
                    }
                }
            } else if ROW_CATEGORIES.contains(&category.as_str()) {
                for (top, v) in obj {
                    if v.is_object() {
                        keys.push(format!("{category}::{top}"));
                    }
                }
            }
        }
    }

    keys.sort();
    keys.dedup();
    Ok(keys)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PakSource {
    Library,
    Disk,
}

/// `path` is a display path under the game's `Paks` directory, set only for a
/// pak found on disk; it is never joined onto a filesystem path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct PakRef {
    pub mod_id: Option<String>,
    pub file: String,
    pub source: PakSource,
    pub path: Option<String>,
}

impl PakRef {
    pub fn library(mod_id: impl Into<String>, file: impl Into<String>) -> Self {
        Self {
            mod_id: Some(mod_id.into()),
            file: file.into(),
            source: PakSource::Library,
            path: None,
        }
    }

    pub fn disk(mod_id: Option<String>, file: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            mod_id,
            file: file.into(),
            source: PakSource::Disk,
            path: Some(path.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PakOverlap {
    pub paks: Vec<PakRef>,
    pub winner: PakRef,
    pub assets: Vec<String>,
    pub asset_count: usize,
}

/// Sort key that ranks a pak's override priority: file name last,
/// ASCII case-insensitively, ties broken by mod id then by disk path.
fn override_rank(pak: &PakRef) -> (String, Option<&str>, Option<&str>) {
    (
        pak.file.to_ascii_lowercase(),
        pak.mod_id.as_deref(),
        pak.path.as_deref(),
    )
}

/// Groups the paks that share an asset by the exact set of paks sharing it,
/// so mods A+B overlapping on one set of assets and A+C on another produce
/// two separate groups. `paks` holds each enabled pak and its asset keys.
pub fn group_overlaps(paks: &[(PakRef, Vec<String>)]) -> Vec<PakOverlap> {
    use std::collections::BTreeMap;

    let mut asset_owners: BTreeMap<String, Vec<PakRef>> = BTreeMap::new();
    for (pak, assets) in paks {
        for asset in assets {
            let owners = asset_owners.entry(asset.clone()).or_default();
            if !owners.contains(pak) {
                owners.push(pak.clone());
            }
        }
    }

    let mut groups: BTreeMap<Vec<PakRef>, Vec<String>> = BTreeMap::new();
    for (asset, mut owners) in asset_owners {
        if owners.len() < 2 {
            continue;
        }
        owners.sort_by(|a, b| override_rank(a).cmp(&override_rank(b)));
        groups.entry(owners).or_default().push(asset);
    }

    groups
        .into_iter()
        .map(|(paks, mut assets)| {
            assets.sort();
            let asset_count = assets.len();
            assets.truncate(100);
            let winner = paks.last().cloned().expect("a group has at least 2 paks");
            PakOverlap {
                paks,
                winner,
                assets,
                asset_count,
            }
        })
        .collect()
}

/// The frameworks a mod of this type needs installed and enabled.
/// `LogicMods` rely on UE4SS's `BPModLoaderMod`, so it needs UE4SS too.
pub fn required_frameworks(mod_type: ModType) -> &'static [FrameworkKey] {
    match mod_type {
        ModType::Ue4ss | ModType::Hybrid | ModType::LogicMods => {
            const UE4SS: [FrameworkKey; 1] = [FrameworkKey::Ue4ss];
            &UE4SS
        }
        ModType::PalSchema => {
            const BOTH: [FrameworkKey; 2] = [FrameworkKey::Ue4ss, FrameworkKey::PalSchema];
            &BOTH
        }
        ModType::Pak | ModType::NativeDll | ModType::Workshop | ModType::Framework => &[],
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dependency {
    Framework(FrameworkKey),
    Package(String),
}

/// Reads one entry of a Workshop `Info.json`'s `Dependencies` array.
pub fn workshop_dependency(value: &str) -> Dependency {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("ue4ss") {
        Dependency::Framework(FrameworkKey::Ue4ss)
    } else if trimmed.eq_ignore_ascii_case("palschema") {
        Dependency::Framework(FrameworkKey::PalSchema)
    } else {
        Dependency::Package(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_jsonc_drops_a_leading_bom() {
        let input = "\u{FEFF}{\"a\":1}";
        assert_eq!(strip_jsonc(input), "{\"a\":1}");
    }

    #[test]
    fn strip_jsonc_removes_comments_outside_strings() {
        let input = "{\n  // a line comment\n  \"a\": 1, /* a block\n comment */ \"b\": 2\n}";
        let stripped = strip_jsonc(input);
        let value: serde_json::Value = serde_json::from_str(&stripped).unwrap();
        assert_eq!(value["a"], 1);
        assert_eq!(value["b"], 2);
    }

    #[test]
    fn strip_jsonc_keeps_comment_like_text_inside_strings() {
        let input = r#"{"a": "//not a comment", "b": "a /* b */ c", "c": "http://x"}"#;
        let stripped = strip_jsonc(input);
        let value: serde_json::Value = serde_json::from_str(&stripped).unwrap();
        assert_eq!(value["a"], "//not a comment");
        assert_eq!(value["b"], "a /* b */ c");
        assert_eq!(value["c"], "http://x");
    }

    #[test]
    fn strip_jsonc_keeps_an_escaped_quote_before_a_trailing_comment() {
        let input = "{\"a\": \"x\\\"y\"} // trailing comment";
        let stripped = strip_jsonc(input);
        let value: serde_json::Value = serde_json::from_str(&stripped).unwrap();
        assert_eq!(value["a"], "x\"y");
    }

    #[test]
    fn strip_jsonc_removes_a_trailing_comma_before_close_brace_and_bracket() {
        let input = "{\"a\": [1, 2,], \"b\": 1,}";
        let stripped = strip_jsonc(input);
        let value: serde_json::Value = serde_json::from_str(&stripped).unwrap();
        assert_eq!(value["a"], serde_json::json!([1, 2]));
        assert_eq!(value["b"], 1);
    }

    #[test]
    fn strip_jsonc_preserves_line_count() {
        let input = "{\n  // c1\n  \"a\": 1,\n  /* c2\n c3 */\n  \"b\": 2\n}";
        let stripped = strip_jsonc(input);
        assert_eq!(stripped.lines().count(), input.lines().count());
    }

    #[test]
    fn strip_jsonc_returns_on_an_unterminated_string() {
        let input = "{\"a\": \"abc";
        assert_eq!(strip_jsonc(input), input);
    }

    #[test]
    fn strip_jsonc_returns_on_an_unterminated_block_comment() {
        let input = "{\"a\": 1} /* abc";
        assert_eq!(strip_jsonc(input).trim(), "{\"a\": 1}");
    }

    #[test]
    fn raw_file_gives_table_and_row_keys() {
        let text = r#"{"DT_A": {"R1": {}, "R2": {}}}"#;
        let keys = palschema_row_keys("MyMod/raw/DT_A.json", text).unwrap();
        assert_eq!(keys, vec!["DT_A::R1".to_string(), "DT_A::R2".to_string()]);
    }

    #[test]
    fn items_jsonc_with_a_comment_gives_a_category_key() {
        let text = "{\n  // a bow\n  \"MOD_Bow_Meteor\": {}\n}";
        let keys = palschema_row_keys("MyMod/items/bow.jsonc", text).unwrap();
        assert_eq!(keys, vec!["items::MOD_Bow_Meteor".to_string()]);
    }

    #[test]
    fn blueprints_file_gives_row_keys() {
        let text = r#"{"BP_A": {"X": {}}}"#;
        let keys = palschema_row_keys("MyMod/blueprints/BP_A.json", text).unwrap();
        assert_eq!(keys, vec!["BP_A::X".to_string()]);
    }

    #[test]
    fn a_file_directly_under_the_folder_gives_no_keys() {
        let text = r#"{"a": {}}"#;
        let keys = palschema_row_keys("MyMod/root.json", text).unwrap();
        assert!(keys.is_empty());
    }

    #[test]
    fn an_unlisted_category_gives_no_keys() {
        let text = r#"{"a": {}}"#;
        let keys = palschema_row_keys("MyMod/unknown/file.json", text).unwrap();
        assert!(keys.is_empty());
    }

    #[test]
    fn a_non_object_value_is_skipped() {
        let text = r#"{"MOD_A": {}, "MOD_B": 5}"#;
        let keys = palschema_row_keys("MyMod/items/file.json", text).unwrap();
        assert_eq!(keys, vec!["items::MOD_A".to_string()]);
    }

    #[test]
    fn invalid_json_is_an_error() {
        assert!(palschema_row_keys("MyMod/items/file.json", "{not json").is_err());
    }

    #[test]
    fn a_leading_separator_does_not_shift_the_category() {
        let text = r#"{"MOD_Bow": {}}"#;
        let keys = palschema_row_keys("/MyMod/items/x.json", text).unwrap();
        assert_eq!(keys, vec!["items::MOD_Bow".to_string()]);
    }

    #[test]
    fn a_backslash_separated_path_still_splits_into_a_category() {
        let text = r#"{"MOD_Bow": {}}"#;
        let keys = palschema_row_keys("MyMod\\items\\x.json", text).unwrap();
        assert_eq!(keys, vec!["items::MOD_Bow".to_string()]);
    }

    #[test]
    fn a_raw_category_matches_case_insensitively() {
        let text = r#"{"DT_A": {"R1": {}}}"#;
        let keys = palschema_row_keys("MyMod/Raw/x.json", text).unwrap();
        assert_eq!(keys, vec!["DT_A::R1".to_string()]);
    }

    #[test]
    fn a_flat_category_matches_case_insensitively() {
        let text = r#"{"MOD_Bow": {}}"#;
        let canonical = palschema_row_keys("MyMod/items/x.json", text).unwrap();
        let mixed_case = palschema_row_keys("MyMod/Items/x.json", text).unwrap();
        assert_eq!(mixed_case, canonical);
    }

    #[test]
    fn keys_come_back_sorted() {
        let text = r#"{"Z_Table": {"R": {}}, "A_Table": {"R": {}}}"#;
        let keys = palschema_row_keys("MyMod/raw/file.json", text).unwrap();
        assert_eq!(
            keys,
            vec!["A_Table::R".to_string(), "Z_Table::R".to_string()]
        );
    }

    fn pak_ref(mod_id: &str, file: &str) -> PakRef {
        PakRef::library(mod_id, file)
    }

    fn assets(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_library_ref_and_a_disk_ref_serialize_their_source_and_path() {
        assert_eq!(
            serde_json::to_value(PakRef::library("moda", "A_P.pak")).unwrap(),
            serde_json::json!({"mod_id": "moda", "file": "A_P.pak", "source": "library", "path": null})
        );
        assert_eq!(
            serde_json::to_value(PakRef::disk(None, "X.pak", "LogicMods/X.pak")).unwrap(),
            serde_json::json!({"mod_id": null, "file": "X.pak", "source": "disk", "path": "LogicMods/X.pak"})
        );
    }

    #[test]
    fn two_untracked_paks_with_one_file_name_stay_apart() {
        let a = PakRef::disk(None, "Same_P.pak", "~mods/Same_P.pak");
        let b = PakRef::disk(None, "Same_P.pak", "LogicMods/Same_P.pak");
        let groups = group_overlaps(&[(a.clone(), assets(&["x"])), (b.clone(), assets(&["x"]))]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].paks.len(), 2);
        assert_eq!(groups[0].winner, a, "the later path breaks a full tie");
    }

    #[test]
    fn an_untracked_pak_wins_by_file_name_like_any_other() {
        let library = PakRef::library("moda", "A_P.pak");
        let untracked = PakRef::disk(None, "zzz_Patch_P.pak", "~mods/zzz_Patch_P.pak");
        let groups = group_overlaps(&[
            (library, assets(&["x"])),
            (untracked.clone(), assets(&["x"])),
        ]);
        assert_eq!(groups[0].winner, untracked);
    }

    #[test]
    fn groups_by_the_exact_set_of_sharing_paks() {
        let a = pak_ref("modA", "A_P.pak");
        let b = pak_ref("modB", "B_P.pak");
        let c = pak_ref("modC", "C_P.pak");
        let paks = vec![
            (a.clone(), assets(&["x1", "x2", "x3"])),
            (b.clone(), assets(&["x1", "x2"])),
            (c.clone(), assets(&["x3"])),
        ];
        let overlaps = group_overlaps(&paks);
        assert_eq!(overlaps.len(), 2);

        let ab = overlaps
            .iter()
            .find(|o| o.paks.len() == 2)
            .expect("a two-pak group");
        assert_eq!(ab.asset_count, 2);
        assert_eq!(ab.assets, assets(&["x1", "x2"]));

        let ac = overlaps
            .iter()
            .find(|o| o.paks.iter().any(|p| p.mod_id.as_deref() == Some("modC")))
            .expect("a group containing modC");
        assert_eq!(ac.asset_count, 1);
        assert_eq!(ac.assets, assets(&["x3"]));
    }

    #[test]
    fn a_zzz_prefixed_pak_wins_override_priority() {
        let a = pak_ref("modA", "A_P.pak");
        let z = pak_ref("modB", "zzz_b_P.pak");
        let paks = vec![
            (a.clone(), assets(&["shared"])),
            (z.clone(), assets(&["shared"])),
        ];
        let overlaps = group_overlaps(&paks);
        assert_eq!(overlaps.len(), 1);
        assert_eq!(overlaps[0].winner, z);
    }

    #[test]
    fn a_file_name_tie_breaks_on_the_later_mod_id() {
        let a = pak_ref("aaa", "same_P.pak");
        let b = pak_ref("bbb", "same_P.pak");
        let paks = vec![
            (a.clone(), assets(&["shared"])),
            (b.clone(), assets(&["shared"])),
        ];
        let overlaps = group_overlaps(&paks);
        assert_eq!(overlaps[0].winner, b);
    }

    #[test]
    fn a_case_folded_file_name_tie_still_breaks_on_the_later_mod_id() {
        let a = pak_ref("modA", "A_P.pak");
        let b = pak_ref("modB", "a_p.pak");
        let set1 = vec![
            (a.clone(), assets(&["shared"])),
            (b.clone(), assets(&["shared"])),
        ];
        let set2 = vec![
            (b.clone(), assets(&["shared"])),
            (a.clone(), assets(&["shared"])),
        ];
        assert_eq!(group_overlaps(&set1)[0].winner, b);
        assert_eq!(group_overlaps(&set2)[0].winner, b);
    }

    #[test]
    fn assets_are_capped_at_one_hundred_but_the_count_is_exact() {
        let a = pak_ref("modA", "A_P.pak");
        let b = pak_ref("modB", "B_P.pak");
        let all: Vec<String> = (0..150).map(|i| format!("asset_{i:03}")).collect();
        let paks = vec![(a.clone(), all.clone()), (b.clone(), all.clone())];
        let overlaps = group_overlaps(&paks);
        assert_eq!(overlaps.len(), 1);
        assert_eq!(overlaps[0].asset_count, 150);
        assert_eq!(overlaps[0].assets.len(), 100);
        assert_eq!(overlaps[0].assets[0], "asset_000");
    }

    #[test]
    fn a_single_pak_asset_yields_no_overlap() {
        let a = pak_ref("modA", "A_P.pak");
        let paks = vec![(a, assets(&["solo"]))];
        assert!(group_overlaps(&paks).is_empty());
    }

    #[test]
    fn output_order_is_deterministic_across_input_orders() {
        let a = pak_ref("modA", "A_P.pak");
        let b = pak_ref("modB", "B_P.pak");
        let c = pak_ref("modC", "C_P.pak");
        let set1 = vec![
            (a.clone(), assets(&["x1"])),
            (b.clone(), assets(&["x1", "y1"])),
            (c.clone(), assets(&["y1"])),
        ];
        let set2 = vec![
            (c.clone(), assets(&["y1"])),
            (a.clone(), assets(&["x1"])),
            (b.clone(), assets(&["x1", "y1"])),
        ];
        assert_eq!(group_overlaps(&set1), group_overlaps(&set2));
    }

    #[test]
    fn required_frameworks_matches_each_mod_type() {
        assert_eq!(required_frameworks(ModType::Ue4ss), [FrameworkKey::Ue4ss]);
        assert_eq!(required_frameworks(ModType::Hybrid), [FrameworkKey::Ue4ss]);
        assert_eq!(
            required_frameworks(ModType::LogicMods),
            [FrameworkKey::Ue4ss]
        );
        assert_eq!(
            required_frameworks(ModType::PalSchema),
            [FrameworkKey::Ue4ss, FrameworkKey::PalSchema]
        );
        assert_eq!(required_frameworks(ModType::Pak), []);
        assert_eq!(required_frameworks(ModType::NativeDll), []);
        assert_eq!(required_frameworks(ModType::Workshop), []);
        assert_eq!(required_frameworks(ModType::Framework), []);
    }

    fn route(kind: RouteKind, rel_path: &str) -> FileRoute {
        FileRoute {
            archive_path: rel_path.to_string(),
            rel_path: rel_path.to_string(),
            kind,
        }
    }

    #[test]
    fn a_pak_route_ending_pak_is_a_pak_route() {
        assert!(is_pak_route(&route(RouteKind::Pak, "Cool_P.pak")));
        assert!(is_pak_route(&route(RouteKind::LogicMods, "Cool_P.pak")));
        assert!(!is_pak_route(&route(RouteKind::Pak, "Cool_P.utoc")));
        assert!(!is_pak_route(&route(RouteKind::Ue4ss, "Cool_P.pak")));
    }

    #[test]
    fn a_game_tree_passthrough_pak_is_a_pak_route() {
        assert!(is_pak_route(&route(
            RouteKind::Passthrough,
            "Pal/Content/Paks/~mods/Cool_P.pak"
        )));
        assert!(is_pak_route(&route(
            RouteKind::Passthrough,
            "PAL/CONTENT/PAKS/LOGICMODS/Cool_P.pak"
        )));
        assert!(!is_pak_route(&route(
            RouteKind::Passthrough,
            "Pal/Binaries/Win64/ue4ss/dwmapi.dll"
        )));
        assert!(!is_pak_route(&route(
            RouteKind::Passthrough,
            "Pal/Content/Paks/Pal-WindowsNoEditor.pak"
        )));
    }

    #[test]
    fn iostore_sibling_requires_both_utoc_and_ucas() {
        let pak = route(RouteKind::Pak, "Cool_P.pak");
        let utoc = route(RouteKind::Pak, "Cool_P.utoc");
        let ucas = route(RouteKind::Pak, "Cool_P.ucas");

        let utoc_only = InstallManifest {
            routes: vec![pak.clone(), utoc.clone()],
            ..test_manifest()
        };
        assert!(iostore_sibling(&utoc_only, &pak).is_none());

        let both = InstallManifest {
            routes: vec![pak.clone(), utoc.clone(), ucas],
            ..test_manifest()
        };
        assert_eq!(iostore_sibling(&both, &pak), Some(&both.routes[1]));
    }

    #[test]
    fn iostore_sibling_matches_the_full_path_not_the_basename() {
        let pak = route(RouteKind::Pak, "A/X_P.pak");
        let utoc = route(RouteKind::Pak, "B/X_P.utoc");
        let ucas = route(RouteKind::Pak, "B/X_P.ucas");
        let manifest = InstallManifest {
            routes: vec![pak.clone(), utoc, ucas],
            ..test_manifest()
        };
        assert!(iostore_sibling(&manifest, &pak).is_none());
    }

    #[test]
    fn iostore_sibling_ignores_a_sibling_of_a_different_kind() {
        let pak = route(RouteKind::Pak, "Cool_P.pak");
        let utoc = route(RouteKind::LogicMods, "Cool_P.utoc");
        let ucas = route(RouteKind::LogicMods, "Cool_P.ucas");
        let manifest = InstallManifest {
            routes: vec![pak.clone(), utoc, ucas],
            ..test_manifest()
        };
        assert!(iostore_sibling(&manifest, &pak).is_none());
    }

    fn test_manifest() -> InstallManifest {
        InstallManifest {
            folder_name: "Mod".to_string(),
            display_name: "Mod".to_string(),
            mod_type: ModType::Pak,
            version: "1.0".to_string(),
            routes: Vec::new(),
            decisions: Vec::new(),
            platform_filtered: None,
            source: Default::default(),
        }
    }

    #[test]
    fn workshop_dependency_recognizes_frameworks_case_insensitively() {
        assert_eq!(
            workshop_dependency("UE4SS"),
            Dependency::Framework(FrameworkKey::Ue4ss)
        );
        assert_eq!(
            workshop_dependency("ue4ss"),
            Dependency::Framework(FrameworkKey::Ue4ss)
        );
        assert_eq!(
            workshop_dependency(" PalSchema "),
            Dependency::Framework(FrameworkKey::PalSchema)
        );
        assert_eq!(
            workshop_dependency("SomePackage"),
            Dependency::Package("SomePackage".to_string())
        );
    }
}
