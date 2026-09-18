use std::cmp::Ordering;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::mods::{compare_versions, is_newer};

pub const ARCHIVE_EXTENSIONS: &[&str] = &["zip", "7z", "rar"];
pub const IOSTORE_SUFFIX: &str = "+iostore";
const MAX_NAME_BYTES: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FileCategory {
    Main,
    Update,
    Optional,
    OldVersion,
    Miscellaneous,
    Removed,
    Archived,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct ModFile {
    pub file_id: u32,
    pub name: String,
    #[serde(default)]
    pub version: String,
    pub category: FileCategory,
    #[serde(default)]
    pub date: i64,
    #[serde(default, deserialize_with = "size_from_any")]
    pub size_in_bytes: Option<u64>,
    pub uri: String,
    #[serde(default, deserialize_with = "flag_from_any")]
    pub primary: bool,
    #[serde(default)]
    pub description: Option<String>,
}

fn size_from_any<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u64>, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::String(text) => text.trim().parse().ok(),
        Value::Number(number) => number.as_u64(),
        _ => None,
    })
}

fn flag_from_any<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::Bool(flag) => flag,
        Value::Number(number) => number.as_i64().is_some_and(|n| n != 0),
        _ => false,
    })
}

pub fn latest_file(files: &[ModFile]) -> Option<&ModFile> {
    files
        .iter()
        .filter(|file| matches!(file.category, FileCategory::Main | FileCategory::Update))
        .max_by_key(|file| file.file_id)
}

pub fn installed_version_base(version: &str) -> &str {
    version.strip_suffix(IOSTORE_SUFFIX).unwrap_or(version)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateState {
    UpToDate,
    Available,
    Ignored,
}

pub fn update_state(installed: &str, latest: &str, ignored_version: Option<&str>) -> UpdateState {
    if !is_newer(latest, installed_version_base(installed)) {
        return UpdateState::UpToDate;
    }
    if ignored_version.is_some_and(|ignored| compare_versions(ignored, latest) == Ordering::Equal) {
        UpdateState::Ignored
    } else {
        UpdateState::Available
    }
}

pub fn archive_file_name(uri: &str, file_id: u32) -> String {
    let fallback = || format!("nexus_{file_id}.zip");
    let last = uri.rsplit(['/', '\\']).next().unwrap_or_default();
    let replaced: String = last
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = replaced.trim_matches(|c: char| c == ' ' || c == '.');
    let Some((stem, extension)) = cleaned.rsplit_once('.') else {
        return fallback();
    };
    let extension = extension.to_ascii_lowercase();
    if !ARCHIVE_EXTENSIONS.contains(&extension.as_str()) {
        return fallback();
    }
    let stem = stem.trim_end_matches([' ', '.']);
    if stem.is_empty() || is_reserved_windows_stem(stem) {
        return fallback();
    }
    let stem = truncate_on_char_boundary(stem, MAX_NAME_BYTES - extension.len() - 1);
    if stem.is_empty() {
        return fallback();
    }
    format!("{stem}.{extension}")
}

fn is_reserved_windows_stem(stem: &str) -> bool {
    let base = stem
        .split('.')
        .next()
        .unwrap_or(stem)
        .trim_end()
        .to_ascii_uppercase();
    let bytes = base.as_bytes();
    matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (bytes.len() == 4
            && (base.starts_with("COM") || base.starts_with("LPT"))
            && (b'1'..=b'9').contains(&bytes[3]))
}

fn truncate_on_char_boundary(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].trim_end_matches([' ', '.'])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn file(file_id: u32, category: FileCategory, version: &str) -> ModFile {
        ModFile {
            file_id,
            name: "Cool Mod".to_string(),
            version: version.to_string(),
            category,
            date: 0,
            size_in_bytes: None,
            uri: format!("Cool Mod-{file_id}.zip"),
            primary: false,
            description: None,
        }
    }

    #[test]
    fn a_graphql_mod_file_deserializes() {
        let parsed: ModFile = serde_json::from_value(json!({
            "fileId": 1, "name": "Enhanced Palworld Visuals", "version": "1.0",
            "category": "ARCHIVED", "date": 1705685026, "sizeInBytes": "27973",
            "uri": "Enhanced Palworld Visuals-1-1-0-1705685026.7z", "primary": 0, "description": ""
        }))
        .unwrap();
        assert_eq!(parsed.file_id, 1);
        assert_eq!(parsed.category, FileCategory::Archived);
        assert_eq!(parsed.size_in_bytes, Some(27973));
        assert!(!parsed.primary);
        let unknown: ModFile = serde_json::from_value(json!({
            "fileId": 2, "name": "x", "version": "1", "category": "SOMETHING_NEW",
            "sizeInBytes": null, "uri": "x.zip", "primary": 1
        }))
        .unwrap();
        assert_eq!(unknown.category, FileCategory::Unknown);
        assert!(unknown.primary);
        let out = serde_json::to_value(&parsed).unwrap();
        assert_eq!(out["file_id"], 1);
        assert_eq!(out["size_in_bytes"], 27973);
        assert_eq!(out["category"], "ARCHIVED");
    }

    #[test]
    fn the_latest_file_is_the_highest_main_or_update_id() {
        let files = vec![
            file(10, FileCategory::Main, "1.0"),
            file(30, FileCategory::Update, "1.2"),
            file(20, FileCategory::Main, "1.1"),
            file(40, FileCategory::Optional, "9.9"),
            file(50, FileCategory::Archived, "9.9"),
        ];
        assert_eq!(latest_file(&files).map(|f| f.file_id), Some(30));
        assert!(latest_file(&[file(1, FileCategory::Optional, "1")]).is_none());
    }

    #[test]
    fn update_state_strips_iostore_and_honours_the_ignored_version() {
        assert_eq!(update_state("1.0", "1.1", None), UpdateState::Available);
        assert_eq!(
            update_state("1.1+iostore", "1.1", None),
            UpdateState::UpToDate
        );
        assert_eq!(
            update_state("1.0+iostore", "1.1", None),
            UpdateState::Available
        );
        assert_eq!(
            update_state("1.0", "1.1", Some("1.1")),
            UpdateState::Ignored
        );
        assert_eq!(
            update_state("1.0", "1.1", Some("v1.1")),
            UpdateState::Ignored
        );
        assert_eq!(
            update_state("1.0", "1.2", Some("1.1")),
            UpdateState::Available
        );
        assert_eq!(update_state("v1.0", "1.0", None), UpdateState::UpToDate);
        assert_eq!(
            update_state("unversioned", "1.0", None),
            UpdateState::Available
        );
        assert_eq!(installed_version_base("2.0+iostore"), "2.0");
    }

    #[test]
    fn archive_names_are_sanitised() {
        assert_eq!(
            archive_file_name("Enhanced Palworld Visuals-1-1-0-1705685026.7z", 5),
            "Enhanced Palworld Visuals-1-1-0-1705685026.7z"
        );
        assert_eq!(archive_file_name("a/b\\Cool.7z", 5), "Cool.7z");
        assert_eq!(archive_file_name("..\\..\\evil.zip", 5), "evil.zip");
        assert_eq!(archive_file_name("Cool: Mod?.RAR", 5), "Cool_ Mod_.rar");
        assert_eq!(archive_file_name("name.zip.", 5), "name.zip");
        assert_eq!(archive_file_name("tab\there.zip", 5), "tab_here.zip");
        for fallback in [
            "readme.exe",
            "CON.zip",
            "com1.7z",
            " .zip",
            "",
            "noextension",
            "x.zip?y",
        ] {
            assert_eq!(
                archive_file_name(fallback, 5),
                "nexus_5.zip",
                "{fallback:?}"
            );
        }
        let long = format!("{}.zip", "é".repeat(300));
        let name = archive_file_name(&long, 5);
        assert!(name.len() <= 200 && name.ends_with(".zip"), "{name}");
    }
}
