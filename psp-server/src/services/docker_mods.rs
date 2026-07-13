//! UE4SS / logic / native-DLL mod file management for Docker servers. UE4SS
//! discovers mods as subdirectories and reads their enabled state from
//! `mods.txt`, one `<ModName> : <0|1>` line per mod.
use std::io::Read;
use std::path::Path;

use serde_json::Value;

use super::ServiceError;

/// Decodes a base64 zip and extracts it into `destination`. Entries whose paths
/// escape the destination (zip-slip) are skipped, since the archive is untrusted.
pub fn extract_base64_zip(mod_zip_b64: &str, destination: &Path) -> Result<(), ServiceError> {
    use base64::Engine;
    let zip_bytes = base64::engine::general_purpose::STANDARD
        .decode(mod_zip_b64)
        .map_err(|error| ServiceError::Other(format!("invalid base64 mod data: {error}")))?;
    std::fs::create_dir_all(destination)?;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes))
        .map_err(|error| ServiceError::Other(format!("invalid mod zip: {error}")))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| ServiceError::Other(format!("invalid mod zip entry: {error}")))?;
        let Some(relative_path) = entry.enclosed_name() else {
            continue; // zip-slip guard
        };
        let target = destination.join(relative_path);
        if entry.is_dir() {
            std::fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut contents = Vec::new();
            entry.read_to_end(&mut contents)?;
            std::fs::write(&target, contents)?;
        }
    }
    Ok(())
}

fn parse_mods_txt(mods_path: &str) -> Vec<(String, bool)> {
    let mods_txt = Path::new(mods_path).join("mods.txt");
    let Ok(contents) = std::fs::read_to_string(mods_txt) else {
        return Vec::new();
    };
    contents
        .lines()
        .filter_map(|line| {
            let (mod_name, state) = line.trim().split_once(" : ")?;
            Some((mod_name.trim().to_string(), state.trim() == "1"))
        })
        .collect()
}

pub fn list_ue4ss_mods(mods_path: &str) -> Vec<Value> {
    let enabled_map: std::collections::HashMap<String, bool> =
        parse_mods_txt(mods_path).into_iter().collect();
    let Ok(entries) = std::fs::read_dir(mods_path) else {
        return Vec::new();
    };
    let mut mods = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if entry.path().is_dir() && name != "shared" {
            mods.push(serde_json::json!({
                "mod_name": name,
                "mod_type": "ue4ss",
                "enabled": enabled_map.get(&name).copied().unwrap_or(false)
            }));
        }
    }
    mods
}

pub fn set_mod_enabled(mods_path: &str, mod_name: &str, enabled: bool) -> std::io::Result<()> {
    let mods_txt = Path::new(mods_path).join("mods.txt");
    let state = if enabled { "1" } else { "0" };
    let mut lines: Vec<String> = Vec::new();
    let mut found = false;
    if let Ok(contents) = std::fs::read_to_string(&mods_txt) {
        for line in contents.lines() {
            if line.trim().starts_with(&format!("{mod_name} : ")) {
                lines.push(format!("{mod_name} : {state}"));
                found = true;
            } else {
                lines.push(line.to_string());
            }
        }
    }
    if !found {
        lines.push(format!("{mod_name} : {state}"));
    }
    std::fs::write(&mods_txt, lines.join("\n") + "\n")
}

/// Extracts into `<target>/<mod_name>/` and enables it in `<target>/mods.txt`.
pub fn install_zip_mod(target_path: &str, mod_name: &str, mod_zip_b64: &str) -> bool {
    let mod_dir = Path::new(target_path).join(mod_name);
    if extract_base64_zip(mod_zip_b64, &mod_dir).is_err() {
        return false;
    }
    set_mod_enabled(target_path, mod_name, true).is_ok()
}

pub fn list_native_dll_mods(nativemods_path: &str) -> Vec<Value> {
    let Ok(entries) = std::fs::read_dir(nativemods_path) else {
        return Vec::new();
    };
    let mut mods = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let is_dll_file = path.is_file() && name.ends_with(".dll");
        let is_dll_dir = path.is_dir()
            && std::fs::read_dir(&path)
                .map(|inner| {
                    inner
                        .flatten()
                        .any(|file| file.file_name().to_string_lossy().ends_with(".dll"))
                })
                .unwrap_or(false);
        if is_dll_file || is_dll_dir {
            mods.push(serde_json::json!({
                "mod_name": name,
                "mod_type": "native",
                "enabled": true
            }));
        }
    }
    mods
}

pub fn install_native_dll_mod(nativemods_path: &str, mod_zip_b64: &str) -> bool {
    extract_base64_zip(mod_zip_b64, Path::new(nativemods_path)).is_ok()
}

#[cfg(test)]
pub(crate) mod zip_fixture {
    use std::io::Write;

    /// Builds a base64-encoded zip from (path, contents) entries.
    pub(crate) fn base64_zip(entries: &[(&str, &str)]) -> String {
        use base64::Engine;
        let mut buffer = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut buffer);
            let options = zip::write::SimpleFileOptions::default();
            for (path, contents) in entries {
                writer.start_file(*path, options).unwrap();
                writer.write_all(contents.as_bytes()).unwrap();
            }
            writer.finish().unwrap();
        }
        base64::engine::general_purpose::STANDARD.encode(buffer.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_ue4ss_mods_reads_dirs_and_mods_txt_state() {
        let scratch = tempfile::tempdir().unwrap();
        let mods_path = scratch.path().to_string_lossy().to_string();
        std::fs::create_dir(scratch.path().join("CoolMod")).unwrap();
        std::fs::create_dir(scratch.path().join("OtherMod")).unwrap();
        std::fs::create_dir(scratch.path().join("shared")).unwrap(); // excluded
        std::fs::write(
            scratch.path().join("mods.txt"),
            "CoolMod : 1\nOtherMod : 0\n",
        )
        .unwrap();
        let mut mods = list_ue4ss_mods(&mods_path);
        mods.sort_by_key(|entry| entry["mod_name"].as_str().unwrap().to_string());
        assert_eq!(
            mods,
            vec![
                serde_json::json!({"mod_name": "CoolMod", "mod_type": "ue4ss", "enabled": true}),
                serde_json::json!({"mod_name": "OtherMod", "mod_type": "ue4ss", "enabled": false}),
            ]
        );
    }

    #[test]
    fn set_mod_enabled_updates_existing_line_or_appends() {
        let scratch = tempfile::tempdir().unwrap();
        let mods_path = scratch.path().to_string_lossy().to_string();
        std::fs::write(scratch.path().join("mods.txt"), "CoolMod : 1\n").unwrap();
        set_mod_enabled(&mods_path, "CoolMod", false).unwrap();
        set_mod_enabled(&mods_path, "NewMod", true).unwrap();
        let contents = std::fs::read_to_string(scratch.path().join("mods.txt")).unwrap();
        assert_eq!(contents, "CoolMod : 0\nNewMod : 1\n");
    }

    #[test]
    fn install_zip_mod_extracts_into_named_dir_and_enables() {
        let scratch = tempfile::tempdir().unwrap();
        let mods_path = scratch.path().to_string_lossy().to_string();
        let zip_b64 = zip_fixture::base64_zip(&[("scripts/main.lua", "print('hi')")]);
        assert!(install_zip_mod(&mods_path, "CoolMod", &zip_b64));
        let script = scratch
            .path()
            .join("CoolMod")
            .join("scripts")
            .join("main.lua");
        assert_eq!(std::fs::read_to_string(script).unwrap(), "print('hi')");
        let mods_txt = std::fs::read_to_string(scratch.path().join("mods.txt")).unwrap();
        assert_eq!(mods_txt, "CoolMod : 1\n");
    }

    #[test]
    fn install_zip_mod_returns_false_on_bad_base64() {
        let scratch = tempfile::tempdir().unwrap();
        assert!(!install_zip_mod(
            &scratch.path().to_string_lossy(),
            "Broken",
            "not-base64!!"
        ));
    }

    #[test]
    fn list_native_dll_mods_finds_dlls_and_dll_dirs() {
        let scratch = tempfile::tempdir().unwrap();
        let path = scratch.path().to_string_lossy().to_string();
        std::fs::write(scratch.path().join("PalDefender.dll"), b"x").unwrap();
        std::fs::write(scratch.path().join(".hidden.dll"), b"x").unwrap(); // dot-skip
        std::fs::write(scratch.path().join("readme.txt"), b"x").unwrap(); // not a dll
        let config_dir = scratch.path().join("ConfigMod");
        std::fs::create_dir(&config_dir).unwrap();
        std::fs::write(config_dir.join("inner.dll"), b"x").unwrap();
        let empty_dir = scratch.path().join("EmptyDir");
        std::fs::create_dir(&empty_dir).unwrap();
        let mut mods = list_native_dll_mods(&path);
        mods.sort_by_key(|entry| entry["mod_name"].as_str().unwrap().to_string());
        assert_eq!(
            mods,
            vec![
                serde_json::json!({"mod_name": "ConfigMod", "mod_type": "native", "enabled": true}),
                serde_json::json!({"mod_name": "PalDefender.dll", "mod_type": "native", "enabled": true}),
            ]
        );
    }

    #[test]
    fn install_native_dll_mod_extracts_at_root() {
        let scratch = tempfile::tempdir().unwrap();
        let target = scratch.path().join("nativemods");
        let zip_b64 = zip_fixture::base64_zip(&[("Injector.dll", "MZ")]);
        assert!(install_native_dll_mod(&target.to_string_lossy(), &zip_b64));
        assert!(target.join("Injector.dll").exists());
    }
}
