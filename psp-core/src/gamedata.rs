use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::OnceLock;

use serde_json::Value;

use crate::error::CoreError;

/// Memoized index over `pals.json`'s top-level keys, built once per `GameData`
/// instance. Without it, every pal decoded from a save re-scans all ~600 keys —
/// crippling on a save with a large Dimensional Palbox (thousands of slots).
#[derive(Debug, Default)]
pub(crate) struct PalLookup {
    /// Exact-case `pals.json` keys. Must stay exact-case:
    /// `domain::pal::format_character_key`'s boss-prefix check is
    /// case-sensitive.
    pub keys: HashSet<String>,
    /// Lowercased key -> the exact-case `pals.json` key, for
    /// `domain::pal::pal_data_for`'s case-insensitive lookup.
    pub lower_to_canonical: HashMap<String, String>,
}

/// In-memory copy of the `data/json/` tree. Loaded once at startup and shared
/// via `Arc` (read-only). Keys are extension-less forward-slash relative paths,
/// e.g. "pals", "l10n/en/pals", "ui/en".
#[derive(Debug)]
pub struct GameData {
    entries: HashMap<String, Value>,
    version: String,
    pal_lookup: OnceLock<PalLookup>,
}

impl GameData {
    /// Loads every *.json file under `data_dir`, recursing into `l10n/` and
    /// `ui/`.
    pub fn load(data_dir: &Path) -> Result<Self, CoreError> {
        let mut entries = HashMap::new();
        load_json_directory(data_dir, data_dir, &mut entries)?;
        Ok(Self {
            entries,
            version: env!("CARGO_PKG_VERSION").to_string(),
            pal_lookup: OnceLock::new(),
        })
    }

    /// Looks a file up by its extension-less path, case-insensitively.
    ///
    /// The l10n directories carry the game's casing (`es-MX`, `zh-Hans`), but the app's
    /// locale codes are lowercase (`es-mx`, `zh-hans`), so an exact-case lookup silently
    /// resolved to nothing for four languages -- across every table, not just one.
    ///
    /// This is about FILE keys only. The pal catalog's keys *inside* `pals.json` remain
    /// case-sensitive on purpose: `pal_lookup` below relies on exact casing to decide
    /// whether a `BOSS_`-prefixed id names a real catalog entry.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries.get(&key.to_lowercase())
    }

    /// App version for the `get_version` message.
    pub fn version(&self) -> &str {
        &self.version
    }

    pub(crate) fn pal_lookup(&self) -> &PalLookup {
        self.pal_lookup.get_or_init(|| {
            let mut keys = HashSet::new();
            let mut lower_to_canonical = HashMap::new();
            if let Some(pals) = self.get("pals").and_then(Value::as_object) {
                for key in pals.keys() {
                    keys.insert(key.clone());
                    lower_to_canonical.insert(key.to_lowercase(), key.clone());
                }
            }
            PalLookup {
                keys,
                lower_to_canonical,
            }
        })
    }
}

fn load_json_directory(
    root: &Path,
    directory: &Path,
    entries: &mut HashMap<String, Value>,
) -> Result<(), CoreError> {
    for dir_entry in std::fs::read_dir(directory)? {
        let path = dir_entry?.path();
        if path.is_dir() {
            load_json_directory(root, &path, entries)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            let text = std::fs::read_to_string(&path)?;
            let value: Value = serde_json::from_str(&text).map_err(|parse_error| {
                CoreError::Parse(format!("{}: {parse_error}", path.display()))
            })?;
            // Lower-cased so a mixed-case directory on disk (`l10n/es-MX/`) still answers
            // the lowercase locale code the app sends (`es-mx`). See `GameData::get`.
            let key = path
                .strip_prefix(root)
                .expect("path is under root by construction")
                .with_extension("")
                .to_string_lossy()
                .replace('\\', "/")
                .to_lowercase();
            entries.insert(key, value);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::GameData;
    use std::fs;

    #[test]
    fn loads_nested_json_tree_and_serves_by_key() {
        let temp_dir = tempfile::tempdir().unwrap();
        fs::write(
            temp_dir.path().join("pals.json"),
            r#"{"PinkCat": {"code_name": "PinkCat"}}"#,
        )
        .unwrap();
        fs::create_dir_all(temp_dir.path().join("l10n/en")).unwrap();
        fs::write(
            temp_dir.path().join("l10n/en/pals.json"),
            r#"{"PinkCat": {"localized_name": "Cattiva"}}"#,
        )
        .unwrap();
        fs::create_dir_all(temp_dir.path().join("ui")).unwrap();
        fs::write(
            temp_dir.path().join("ui/en.json"),
            r#"{"health": "Health"}"#,
        )
        .unwrap();

        let game_data = GameData::load(temp_dir.path()).unwrap();

        assert_eq!(
            game_data.get("pals").unwrap()["PinkCat"]["code_name"],
            "PinkCat"
        );
        assert_eq!(
            game_data.get("l10n/en/pals").unwrap()["PinkCat"]["localized_name"],
            "Cattiva"
        );
        assert_eq!(game_data.get("ui/en").unwrap()["health"], "Health");
        assert!(game_data.get("does_not_exist").is_none());
        assert_eq!(game_data.version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn loads_the_real_repo_data_dir() {
        // <repo>/psp-core → <repo>/data/json
        let repo_json_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/json");
        let game_data = GameData::load(&repo_json_dir).unwrap();
        for key in [
            "pals",
            "active_skills",
            "items",
            "l10n/en/pals",
            "l10n/en/ui",
            "ui/en",
            "exp",
            "bosses",
            "relics",
        ] {
            assert!(game_data.get(key).is_some(), "missing game data key {key}");
        }
    }

    #[test]
    fn invalid_json_is_a_parse_error_naming_the_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        fs::write(temp_dir.path().join("broken.json"), "{ not json").unwrap();
        let error = GameData::load(temp_dir.path()).unwrap_err();
        assert!(error.to_string().contains("broken.json"), "got: {error}");
    }
}
