//! Materialises a plugin's stored sources on disk as a `lua-language-server`
//! workspace: the generated `psp.lua` annotations, a `.luarc.json`, and one
//! file per source key.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

const META_FILE: &str = "psp.lua";
const LUARC_FILE: &str = ".luarc.json";

/// `inject-field` and `missing-fields` are disabled because the generated
/// `psp.lua` trips 25 of them against itself — a file the plugin author did
/// not write and cannot edit. Type resolution is unaffected.
const LUARC: &str = r#"{
  "runtime": { "version": "Lua 5.4", "path": ["?.lua", "?/init.lua"] },
  "diagnostics": { "disable": ["lowercase-global", "inject-field", "missing-fields"] },
  "workspace": { "checkThirdParty": false, "library": ["psp.lua"] }
}
"#;

/// Mirrors the bounds the save handler already applies to a stored key, so a
/// path that reached the database can never be one this refuses on length.
const MAX_SOURCE_PATH_LEN: usize = 240;
const MAX_SOURCE_PATH_SEGMENTS: usize = 16;
const MAX_PLUGIN_ID_LEN: usize = 128;

/// Budgeted against the *resolved* workspace root, never against the stored
/// key alone. `std::fs` gives an absolute Windows path the `\\?\` prefix,
/// which replaces the 260-character `MAX_PATH` ceiling with the ~32767
/// namespace limit; unix stays at `PATH_MAX`.
const MAX_ABSOLUTE_PATH_LEN: usize = if cfg!(windows) { 32_000 } else { 4_000 };

/// Writes `sources` into `root/plugin_id`, replacing whatever `.lua` files
/// were there before, and returns the absolute workspace directory.
///
/// The whole map is re-synced on every call, so a key the author deleted is
/// gone from disk afterwards. That is what lets the delete handler work purely
/// on the stored map and never unlink a client-supplied path itself.
pub fn materialise(
    root: &Path,
    plugin_id: &str,
    sources: &BTreeMap<String, String>,
) -> Result<PathBuf, String> {
    if !is_safe_plugin_id(plugin_id) {
        return Err(format!("{plugin_id:?} is not a valid plugin id"));
    }

    let root = std::path::absolute(root)
        .map_err(|error| format!("could not resolve the workspace root: {error}"))?;
    let workspace = root.join(plugin_id);
    let workspace_len = workspace.as_os_str().len();

    check_paths_fit(workspace_len, sources)?;
    check_no_case_collision(sources)?;

    std::fs::create_dir_all(&workspace)
        .map_err(|error| format!("could not create the plugin workspace: {error}"))?;
    clear_lua_files(&workspace)
        .map_err(|error| format!("could not clear the plugin workspace: {error}"))?;

    write_file(
        &workspace.join(META_FILE),
        &psp_plugin::lua_meta(&psp_plugin::api_definition()),
    )?;
    write_file(&workspace.join(LUARC_FILE), LUARC)?;

    for (path, source) in sources {
        let target = resolve(&workspace, path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("could not create the directory for {path:?}: {error}"))?;
        }
        write_file(&target, source)?;
    }

    Ok(workspace)
}

fn write_file(path: &Path, contents: &str) -> Result<(), String> {
    std::fs::write(path, contents)
        .map_err(|error| format!("could not write {}: {error}", path.display()))
}

/// Joins segment by segment rather than handing the whole key to `join`, so a
/// forward slash never has to survive a platform's separator handling.
fn resolve(workspace: &Path, path: &str) -> PathBuf {
    let mut target = workspace.to_path_buf();
    for segment in path.split('/') {
        target.push(segment);
    }
    target
}

fn check_paths_fit(workspace_len: usize, sources: &BTreeMap<String, String>) -> Result<(), String> {
    let budget = MAX_ABSOLUTE_PATH_LEN.saturating_sub(workspace_len + 1);
    if budget < META_FILE.len().max(LUARC_FILE.len()) {
        return Err(format!(
            "the workspace root is {workspace_len} characters, leaving no room for a source file"
        ));
    }
    for path in sources.keys() {
        if !is_safe_source_path(path) {
            return Err(format!("{path:?} is not a valid plugin source path"));
        }
        if path.len() > budget {
            return Err(format!(
                "{path:?} does not fit under a workspace root of {workspace_len} characters"
            ));
        }
    }
    Ok(())
}

/// `Lib/util.lua` and `lib/util.lua` are distinct map keys but one file on a
/// case-insensitive filesystem, so writing both would silently keep whichever
/// came last. Refusing the pair is the only outcome that behaves the same on
/// every platform, and folding the name instead would break the host's own
/// case-sensitive `require` resolution.
fn check_no_case_collision(sources: &BTreeMap<String, String>) -> Result<(), String> {
    // Seeded with the two generated files so a source key cannot shadow them.
    // `psp.lua` is what `.luarc.json` names as the workspace library: an
    // author's own `psp.lua` would replace the whole host API annotation set
    // and turn every `psp.*` call in their plugin into a false diagnostic.
    let mut files: HashMap<String, &str> = HashMap::from([
        (META_FILE.to_string(), META_FILE),
        (LUARC_FILE.to_string(), LUARC_FILE),
    ]);
    let mut directories: HashSet<String> = HashSet::new();

    for path in sources.keys() {
        let folded = path.to_ascii_lowercase();
        if folded == META_FILE || folded == LUARC_FILE {
            return Err(format!(
                "{path:?} is generated into the workspace and cannot also be a plugin source"
            ));
        }
        if let Some(existing) = files.insert(folded.clone(), path.as_str()) {
            return Err(format!(
                "{path:?} and {existing:?} name the same file on a case-insensitive filesystem"
            ));
        }
        let segments: Vec<&str> = folded.split('/').collect();
        let mut prefix = String::new();
        for segment in &segments[..segments.len().saturating_sub(1)] {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(segment);
            directories.insert(prefix.clone());
        }
    }

    match directories
        .iter()
        .find(|folded| files.contains_key(*folded))
    {
        Some(clash) => Err(format!(
            "{clash:?} is used as both a source file and a directory"
        )),
        None => Ok(()),
    }
}

/// Removes every `.lua` file under `dir` and prunes the directories that empty
/// out, leaving the language server's own log files alone.
fn clear_lua_files(dir: &Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            clear_lua_files(&path)?;
            if std::fs::read_dir(&path)?.next().is_none() {
                std::fs::remove_dir(&path)?;
            }
        } else if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("lua"))
        {
            std::fs::remove_file(&path)?;
        }
    }
    Ok(())
}

fn is_safe_plugin_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_PLUGIN_ID_LEN
        && !id.ends_with('.')
        && !id.ends_with(' ')
        && !id.trim_matches('.').is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        && !is_reserved_device_name(id)
}

/// Restates the client's `validNewPath` rule server-side: the key becomes a
/// real filesystem path here, so nothing about it may be taken on trust. The
/// `.lua` suffix is required because `clear_lua_files` sweeps by extension —
/// a key with any other suffix would survive its own deletion.
fn is_safe_source_path(path: &str) -> bool {
    if path.is_empty() || path.starts_with('/') || path.contains('\\') {
        return false;
    }
    if path.len() > MAX_SOURCE_PATH_LEN || !path.is_ascii() || !path.ends_with(".lua") {
        return false;
    }
    let bytes = path.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return false;
    }
    if path.chars().any(|c| c.is_control()) {
        return false;
    }
    let segments: Vec<&str> = path.split('/').collect();
    if segments.len() > MAX_SOURCE_PATH_SEGMENTS {
        return false;
    }
    segments.iter().all(|segment| {
        !segment.is_empty()
            // `resolve` pushes segment by segment, and `PathBuf::push` throws
            // the whole buffer away when the component it is given carries a
            // prefix: `lib/C:evil.lua` would land on `C:evil.lua`, outside the
            // workspace entirely. A drive letter has to be refused wherever in
            // the key it appears, not only at its start.
            && !segment.contains(':')
            && *segment != "."
            && *segment != ".."
            && !segment.ends_with('.')
            && !segment.ends_with(' ')
            && !is_reserved_device_name(segment)
    })
}

fn is_reserved_device_name(segment: &str) -> bool {
    const RESERVED: &[&str] = &[
        "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
        "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
    ];
    let stem = segment.split('.').next().unwrap_or(segment);
    RESERVED.iter().any(|name| stem.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(path: &str) -> BTreeMap<String, String> {
        BTreeMap::from([(path.to_string(), "return {}".to_string())])
    }

    #[test]
    fn a_plugin_id_that_could_leave_the_root_is_refused() {
        let dir = tempfile::tempdir().expect("a temp dir");
        for id in ["", ".", "..", "a/b", "a\\b", "con", "user.demo."] {
            assert!(
                materialise(dir.path(), id, &BTreeMap::new()).is_err(),
                "{id:?} must be refused"
            );
        }
    }

    #[test]
    fn two_keys_differing_only_by_case_are_refused_rather_than_clobbered() {
        let mut sources = one("lib/util.lua");
        sources.insert("Lib/util.lua".to_string(), "return {}".to_string());
        let dir = tempfile::tempdir().expect("a temp dir");
        assert!(materialise(dir.path(), "user.demo", &sources).is_err());
    }

    #[test]
    fn a_key_used_as_both_a_file_and_a_directory_is_refused() {
        let mut sources = one("lib.lua");
        sources.insert("lib.lua/util.lua".to_string(), "return {}".to_string());
        let dir = tempfile::tempdir().expect("a temp dir");
        assert!(materialise(dir.path(), "user.demo", &sources).is_err());
    }

    #[test]
    fn a_root_that_leaves_no_room_for_a_key_is_refused() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let workspace_len = std::path::absolute(dir.path())
            .expect("absolute")
            .join("user.demo")
            .as_os_str()
            .len();
        // No key long enough to overflow a realistic root can also clear
        // `is_safe_source_path`, so the budget is exercised by shrinking the
        // room the root leaves rather than by an unreachable key length.
        assert!(check_paths_fit(MAX_ABSOLUTE_PATH_LEN, &one("main.lua")).is_err());
        assert!(check_paths_fit(MAX_ABSOLUTE_PATH_LEN - 8, &one("main.lua")).is_err());
        assert!(check_paths_fit(MAX_ABSOLUTE_PATH_LEN - 250, &one("main.lua")).is_ok());
        assert!(check_paths_fit(workspace_len, &one("main.lua")).is_ok());
    }

    #[test]
    fn the_workspace_returned_is_absolute() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let workspace =
            materialise(dir.path(), "user.demo", &one("main.lua")).expect("materialise");
        assert!(workspace.is_absolute());
    }

    #[test]
    fn a_drive_letter_in_any_segment_is_refused() {
        let dir = tempfile::tempdir().expect("a temp dir");
        for path in [
            "lib/C:evil.lua",
            "lib/C:/evil.lua",
            "C:evil.lua",
            "a/b/c:d.lua",
        ] {
            // Asserted against the rule and not only against `materialise`,
            // because `resolve` lands these on `C:evil.lua` and the write
            // there happens to fail for an unprivileged process — a
            // permission error is not the same guarantee as a refusal.
            assert!(!is_safe_source_path(path), "{path:?} must be refused");
            assert!(
                materialise(dir.path(), "user.demo", &one(path)).is_err(),
                "{path:?} must be refused"
            );
        }
        // The escape being guarded against, stated outright: `PathBuf::push`
        // discards the whole buffer when the component carries a prefix.
        #[cfg(windows)]
        assert_eq!(
            resolve(Path::new(r"O:\ws"), "lib/C:evil.lua"),
            PathBuf::from("C:evil.lua")
        );
    }

    #[test]
    fn a_source_named_like_a_generated_file_is_refused() {
        let dir = tempfile::tempdir().expect("a temp dir");
        for path in ["psp.lua", "PSP.lua"] {
            assert!(
                materialise(dir.path(), "user.demo", &one(path)).is_err(),
                "{path:?} would replace the annotations .luarc.json points at"
            );
        }
        let mut nested = one("main.lua");
        nested.insert("psp.lua/x.lua".to_string(), "return {}".to_string());
        assert!(materialise(dir.path(), "user.demo", &nested).is_err());
    }

    #[test]
    fn the_generated_meta_file_is_the_one_that_survives() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let workspace =
            materialise(dir.path(), "user.demo", &one("main.lua")).expect("materialise");
        assert_eq!(
            std::fs::read_to_string(workspace.join(META_FILE)).expect("psp.lua"),
            psp_plugin::lua_meta(&psp_plugin::api_definition())
        );
    }

    #[test]
    fn a_language_server_log_file_survives_a_re_sync() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let workspace = materialise(dir.path(), "user.demo", &one("main.lua")).expect("first");
        let log = workspace.join("service.log");
        std::fs::write(&log, "log").expect("write a log");

        materialise(dir.path(), "user.demo", &one("main.lua")).expect("second");
        assert!(
            log.exists(),
            "the workspace doubles as the server's --logpath, so the sweep must not take its logs"
        );
    }
}
