use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;

use crate::error::CoreError;

/// In-memory copy of the `data/json/` tree. Loaded once at startup and shared
/// via `Arc` (read-only). Keys are extension-less forward-slash relative paths,
/// e.g. "pals", "l10n/en/pals", "ui/en".
#[derive(Debug)]
pub struct GameData {
    entries: HashMap<String, Value>,
    version: String,
}

impl GameData {
    /// Loads every *.json file under `data_dir` (the `data/json` directory),
    /// including `l10n/` and `ui/` subtrees.
    pub fn load(data_dir: &Path) -> Result<Self, CoreError> {
        let mut entries = HashMap::new();
        load_json_directory(data_dir, data_dir, &mut entries)?;
        Ok(Self {
            entries,
            version: env!("CARGO_PKG_VERSION").to_string(),
        })
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries.get(key)
    }

    /// App version for the `get_version` message.
    pub fn version(&self) -> &str {
        &self.version
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
            let key = path
                .strip_prefix(root)
                .expect("path is under root by construction")
                .with_extension("")
                .to_string_lossy()
                .replace('\\', "/");
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
        // <repo>/rust/psp-core → <repo>/data/json
        let repo_json_dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/json");
        let game_data = GameData::load(&repo_json_dir).unwrap();
        for key in [
            "pals",
            "active_skills",
            "items",
            "l10n/en/pals",
            "l10n/en/ui",
            "ui/en",
            "exp",
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
