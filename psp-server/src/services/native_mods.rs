//! Native-server mod management via the game's own `Mods/PalModSettings.ini`:
//! a `[PalModSettings]` section holding `bGlobalEnableMod`, an optional
//! `WorkshopRootDir`, and one repeated `ActiveModList=<PackageName>` line per
//! enabled mod. Mods themselves live in per-mod directories, each carrying an
//! `Info.json`. PalServer.exe reads the file through the Unreal config system,
//! which only sees keys under the section header and rewrites the file from its
//! own state at launch — so anything written outside the header is discarded.
use std::path::{Path, PathBuf};

use psp_db::servers::ServerRecord;
use serde_json::Value;

/// Palworld Steam app id for workshop content (not the dedicated-server app id).
const WORKSHOP_APP_ID: &str = "1623730";

const MOD_SECTION_HEADER: &str = "[PalModSettings]";
const DEFAULT_CONFIG_VERSION: &str = "1.0";

#[derive(Debug, Clone, PartialEq)]
pub struct PalModSettings {
    pub enabled: bool,
    pub active_mods: Vec<String>,
    pub workshop_root_dir: String,
}

pub fn palmodsettings_path(install_path: &str) -> PathBuf {
    Path::new(install_path)
        .join("Mods")
        .join("PalModSettings.ini")
}

pub fn local_workshop_path(install_path: &str) -> PathBuf {
    Path::new(install_path).join("Mods").join("Workshop")
}

/// Everything in the section, including the keys this module does not own.
struct RawModSettings {
    settings: PalModSettings,
    config_version: String,
    other_keys: Vec<String>,
}

fn read_raw_palmodsettings(install_path: &str) -> RawModSettings {
    let mut raw = RawModSettings {
        settings: PalModSettings {
            enabled: false,
            active_mods: Vec::new(),
            workshop_root_dir: String::new(),
        },
        config_version: DEFAULT_CONFIG_VERSION.to_string(),
        other_keys: Vec::new(),
    };
    let Ok(contents) = std::fs::read_to_string(palmodsettings_path(install_path)) else {
        return raw;
    };
    // Files written before the header was emitted start straight in at the keys,
    // so the leading run counts as in-section.
    let mut in_section = true;
    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_section = line.eq_ignore_ascii_case(MOD_SECTION_HEADER);
            continue;
        }
        if !in_section || line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "bGlobalEnableMod" => raw.settings.enabled = value.eq_ignore_ascii_case("true"),
            "ActiveModList" if !value.is_empty() => {
                raw.settings.active_mods.push(value.to_string());
            }
            "WorkshopRootDir" => raw.settings.workshop_root_dir = value.to_string(),
            "ConfigVersion" if !value.is_empty() => raw.config_version = value.to_string(),
            _ => raw.other_keys.push(line.to_string()),
        }
    }
    raw
}

pub fn read_palmodsettings(install_path: &str) -> PalModSettings {
    read_raw_palmodsettings(install_path).settings
}

pub fn write_palmodsettings(
    install_path: &str,
    enabled: bool,
    active_mods: &[String],
    workshop_root_dir: &str,
) -> std::io::Result<()> {
    let existing = read_raw_palmodsettings(install_path);
    let mods_dir = Path::new(install_path).join("Mods");
    std::fs::create_dir_all(&mods_dir)?;
    let mut lines = vec![
        MOD_SECTION_HEADER.to_string(),
        format!("ConfigVersion={}", existing.config_version),
        format!(
            "bGlobalEnableMod={}",
            if enabled { "true" } else { "false" }
        ),
    ];
    if !workshop_root_dir.is_empty() {
        lines.push(format!("WorkshopRootDir={workshop_root_dir}"));
    }
    for package in active_mods {
        lines.push(format!("ActiveModList={package}"));
    }
    lines.extend(existing.other_keys);
    std::fs::write(palmodsettings_path(install_path), lines.join("\n") + "\n")
}

/// The server will not load mods unless Mods/Workshop and PalModSettings.ini
/// exist, so both are created before start and WorkshopRootDir is re-synced
/// whenever the record's workshop_dir has moved.
pub fn ensure_mod_settings(record: &ServerRecord) -> std::io::Result<()> {
    let install_path = record.install_path.as_str();
    std::fs::create_dir_all(local_workshop_path(install_path))?;
    if !palmodsettings_path(install_path).exists() {
        return write_palmodsettings(install_path, true, &[], &record.workshop_dir);
    }
    if !record.workshop_dir.is_empty() {
        let settings = read_palmodsettings(install_path);
        if settings.workshop_root_dir != record.workshop_dir {
            return write_palmodsettings(
                install_path,
                settings.enabled,
                &settings.active_mods,
                &record.workshop_dir,
            );
        }
    }
    Ok(())
}

/// Reads a mod directory's Info.json. `mod_type` comes from the first InstallRule:
/// an unrecognized Type passes through lowercased, and only a missing or empty
/// InstallRule yields "unknown". `is_server` is true when any rule opts in with
/// `IsServer`; without one the game deploys no payload on a dedicated server.
pub fn parse_info_json(mod_dir: &Path) -> Option<Value> {
    let contents = std::fs::read_to_string(mod_dir.join("Info.json")).ok()?;
    let data: Value = serde_json::from_str(&contents).ok()?;
    let install_rules = data.get("InstallRule").and_then(Value::as_array);
    let is_server = install_rules.is_some_and(|rules| {
        rules.iter().any(|rule| {
            rule.get("IsServer")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
    });
    let mod_type = match install_rules.and_then(|rules| rules.first()) {
        Some(rule) => {
            let rule_type = rule
                .get("Type")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_ascii_lowercase();
            match rule_type.as_str() {
                "ue4ss" => "ue4ss".to_string(),
                "lua" => "lua".to_string(),
                "palschema" => "palschema".to_string(),
                "logicmods" => "logic".to_string(),
                "paks" => "paks".to_string(),
                _ => rule_type,
            }
        }
        None => "unknown".to_string(),
    };
    let text = |key: &str| {
        data.get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let dependencies = match data.get("Dependencies") {
        Some(Value::Array(items)) => Value::Array(items.clone()),
        _ => Value::Array(Vec::new()),
    };
    Some(serde_json::json!({
        "package_name": text("PackageName"),
        "display_name": text("ModName"),
        "mod_version": text("Version"),
        "mod_author": text("Author"),
        "mod_type": mod_type,
        "is_server": is_server,
        "dependencies": dependencies
    }))
}

pub fn list_workshop_mods(workshop_dir: &str, source: &str) -> Vec<Value> {
    if workshop_dir.is_empty() {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(workshop_dir) else {
        return Vec::new();
    };
    let mut mods = Vec::new();
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let Some(info) = parse_info_json(&entry.path()) else {
            continue;
        };
        let package_name = info["package_name"].as_str().unwrap_or("");
        if package_name.is_empty() {
            continue;
        }
        mods.push(serde_json::json!({
            "mod_name": package_name,
            "display_name": info["display_name"],
            "mod_type": info["mod_type"],
            "mod_version": info["mod_version"],
            "mod_author": info["mod_author"],
            "is_server": info["is_server"],
            "source": source,
            "enabled": false
        }));
    }
    mods
}

/// Merges the Steam workshop dir and the local Mods/Workshop dir, flagging each
/// against ActiveModList. Packages listed as active but present in neither
/// directory are still reported, as source "config".
pub fn list_native_server_mods(record: &ServerRecord) -> Vec<Value> {
    let install_path = record.install_path.as_str();
    let settings = read_palmodsettings(install_path);
    let active: std::collections::HashSet<&String> = settings.active_mods.iter().collect();

    let mut all_mods = Vec::new();
    if !record.workshop_dir.is_empty() {
        all_mods.extend(list_workshop_mods(&record.workshop_dir, "workshop"));
    }
    all_mods.extend(list_workshop_mods(
        &local_workshop_path(install_path).to_string_lossy(),
        "local",
    ));

    let mut seen = std::collections::HashSet::new();
    for entry in &mut all_mods {
        let name = entry["mod_name"].as_str().unwrap_or("").to_string();
        entry["enabled"] = Value::Bool(active.contains(&name));
        seen.insert(name);
    }
    for package in &settings.active_mods {
        if !seen.contains(package) {
            all_mods.push(serde_json::json!({
                "mod_name": package,
                "display_name": package,
                "mod_type": "unknown",
                "mod_version": "",
                "mod_author": "",
                "is_server": true,
                "source": "config",
                "enabled": true
            }));
        }
    }
    all_mods
}

pub fn toggle_native_mod(
    install_path: &str,
    package_name: &str,
    enabled: bool,
) -> std::io::Result<()> {
    let settings = read_palmodsettings(install_path);
    let mut active_mods = settings.active_mods;
    if enabled && !active_mods.iter().any(|entry| entry == package_name) {
        active_mods.push(package_name.to_string());
    } else if !enabled {
        active_mods.retain(|entry| entry != package_name);
    }
    write_palmodsettings(
        install_path,
        settings.enabled || enabled,
        &active_mods,
        &settings.workshop_root_dir,
    )
}

/// ActiveModList keys on the PackageName from Info.json, which need not match the
/// directory the mod was installed into.
pub fn install_native_workshop_mod(install_path: &str, mod_name: &str, mod_zip_b64: &str) -> bool {
    let mod_dir = local_workshop_path(install_path).join(mod_name);
    if crate::services::docker_mods::extract_base64_zip(mod_zip_b64, &mod_dir).is_err() {
        return false;
    }
    let package_name = parse_info_json(&mod_dir)
        .and_then(|info| info["package_name"].as_str().map(str::to_string))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| mod_name.to_string());
    toggle_native_mod(install_path, &package_name, true).is_ok()
}

/// Drive-root scan for the Steam workshop content dir (app 1623730).
pub fn find_steam_workshop_dir() -> Option<String> {
    let steam_patterns = [
        format!("Program Files (x86){}Steam", std::path::MAIN_SEPARATOR),
        format!("Programs{}Steam", std::path::MAIN_SEPARATOR),
        "Steam".to_string(),
        "SteamLibrary".to_string(),
        format!("Program Files{}Steam", std::path::MAIN_SEPARATOR),
    ];
    for drive_letter in "CDEFGHIJKLMNOPQRSTUVWXYZ".chars() {
        for pattern in &steam_patterns {
            let candidate = PathBuf::from(format!("{drive_letter}:\\"))
                .join(pattern)
                .join("steamapps")
                .join("workshop")
                .join("content")
                .join(WORKSHOP_APP_ID);
            if candidate.is_dir() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const INFO_JSON: &str = r#"{
        "PackageName": "CoolPackage",
        "ModName": "Cool Mod",
        "Version": "1.2.0",
        "Author": "someone",
        "InstallRule": [{"Type": "LogicMods"}],
        "Dependencies": null
    }"#;

    fn native_record(install_path: &str, workshop_dir: &str) -> psp_db::servers::ServerRecord {
        let mut record = crate::services::docker::test_support::docker_record();
        record.server_type = "native".to_string();
        record.install_path = install_path.to_string();
        record.workshop_dir = workshop_dir.to_string();
        record
    }

    #[test]
    fn palmodsettings_round_trips() {
        let scratch = tempfile::tempdir().unwrap();
        let install = scratch.path().to_string_lossy().to_string();
        write_palmodsettings(
            &install,
            true,
            &["PackA".to_string(), "PackB".to_string()],
            "C:/steam/workshop/content/1623730",
        )
        .unwrap();
        let contents = std::fs::read_to_string(palmodsettings_path(&install)).unwrap();
        assert_eq!(
            contents,
            "[PalModSettings]\nConfigVersion=1.0\nbGlobalEnableMod=true\nWorkshopRootDir=C:/steam/workshop/content/1623730\nActiveModList=PackA\nActiveModList=PackB\n"
        );
        let settings = read_palmodsettings(&install);
        assert!(settings.enabled);
        assert_eq!(settings.active_mods, vec!["PackA", "PackB"]);
        assert_eq!(
            settings.workshop_root_dir,
            "C:/steam/workshop/content/1623730"
        );
    }

    #[test]
    fn read_palmodsettings_defaults_when_missing() {
        let scratch = tempfile::tempdir().unwrap();
        let settings = read_palmodsettings(&scratch.path().to_string_lossy());
        assert!(!settings.enabled);
        assert!(settings.active_mods.is_empty());
        assert_eq!(settings.workshop_root_dir, "");
    }

    /// A mod is server-capable only if some InstallRule opts in with IsServer;
    /// the rest install on a client but deploy no payload on a dedicated server.
    #[test]
    fn parse_info_json_reports_server_capability_from_any_install_rule() {
        let client_only = tempfile::tempdir().unwrap();
        std::fs::write(client_only.path().join("Info.json"), INFO_JSON).unwrap();
        assert_eq!(
            parse_info_json(client_only.path()).unwrap()["is_server"],
            false
        );

        // UE4SS ships a client rule and a server rule; the server one counts.
        let dual = tempfile::tempdir().unwrap();
        std::fs::write(
            dual.path().join("Info.json"),
            serde_json::json!({
                "PackageName": "P",
                "InstallRule": [
                    {"Type": "UE4SS", "Targets": ["."]},
                    {"Type": "UE4SS", "IsServer": true, "Targets": ["."]}
                ]
            })
            .to_string(),
        )
        .unwrap();
        assert_eq!(parse_info_json(dual.path()).unwrap()["is_server"], true);

        let no_rules = tempfile::tempdir().unwrap();
        std::fs::write(
            no_rules.path().join("Info.json"),
            serde_json::json!({"PackageName": "P"}).to_string(),
        )
        .unwrap();
        assert_eq!(parse_info_json(no_rules.path()).unwrap()["is_server"], false);
    }

    #[test]
    fn list_native_server_mods_flags_client_only_mods() {
        let scratch = tempfile::tempdir().unwrap();
        let install = scratch.path().join("server");
        let workshop = scratch.path().join("workshop");
        let client_only = workshop.join("111");
        std::fs::create_dir_all(&client_only).unwrap();
        std::fs::write(client_only.join("Info.json"), INFO_JSON).unwrap();
        let server_ready = workshop.join("222");
        std::fs::create_dir_all(&server_ready).unwrap();
        std::fs::write(
            server_ready.join("Info.json"),
            serde_json::json!({
                "PackageName": "ServerPack",
                "InstallRule": [{"Type": "Paks", "IsServer": true}]
            })
            .to_string(),
        )
        .unwrap();
        let record = native_record(
            &install.to_string_lossy(),
            &workshop.to_string_lossy(),
        );
        let mods = list_native_server_mods(&record);
        let flag = |name: &str| {
            mods.iter()
                .find(|entry| entry["mod_name"] == name)
                .map(|entry| entry["is_server"].clone())
        };
        assert_eq!(flag("CoolPackage"), Some(Value::Bool(false)));
        assert_eq!(flag("ServerPack"), Some(Value::Bool(true)));
    }

    /// A package named in ActiveModList but absent from disk cannot be judged,
    /// so it must not be reported as client-only.
    #[test]
    fn list_native_server_mods_reports_config_only_packages_as_server_capable() {
        let scratch = tempfile::tempdir().unwrap();
        let install = scratch.path().to_string_lossy().to_string();
        write_palmodsettings(&install, true, &["GhostPackage".to_string()], "").unwrap();
        let record = native_record(&install, "");
        let mods = list_native_server_mods(&record);
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0]["source"], "config");
        assert_eq!(mods[0]["is_server"], true);
    }

    #[test]
    fn parse_info_json_maps_install_rule_type() {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::write(scratch.path().join("Info.json"), INFO_JSON).unwrap();
        let info = parse_info_json(scratch.path()).unwrap();
        assert_eq!(
            info,
            serde_json::json!({
                "package_name": "CoolPackage",
                "display_name": "Cool Mod",
                "mod_version": "1.2.0",
                "mod_author": "someone",
                "mod_type": "logic",
                "is_server": false,
                "dependencies": []
            })
        );
    }

    #[test]
    fn parse_info_json_mod_type_install_rule_edge_cases() {
        // InstallRule present but the first rule has no Type: mod_type is the
        // empty string, NOT "unknown".
        let no_type = tempfile::tempdir().unwrap();
        std::fs::write(
            no_type.path().join("Info.json"),
            serde_json::json!({"PackageName": "P", "InstallRule": [{}]}).to_string(),
        )
        .unwrap();
        assert_eq!(parse_info_json(no_type.path()).unwrap()["mod_type"], "");

        // Missing/empty InstallRule falls back to the "unknown" default.
        let missing = tempfile::tempdir().unwrap();
        std::fs::write(
            missing.path().join("Info.json"),
            serde_json::json!({"PackageName": "P"}).to_string(),
        )
        .unwrap();
        assert_eq!(
            parse_info_json(missing.path()).unwrap()["mod_type"],
            "unknown"
        );

        let empty = tempfile::tempdir().unwrap();
        std::fs::write(
            empty.path().join("Info.json"),
            serde_json::json!({"PackageName": "P", "InstallRule": []}).to_string(),
        )
        .unwrap();
        assert_eq!(
            parse_info_json(empty.path()).unwrap()["mod_type"],
            "unknown"
        );

        // An unmapped Type passes through lowercased.
        let other = tempfile::tempdir().unwrap();
        std::fs::write(
            other.path().join("Info.json"),
            serde_json::json!({"PackageName": "P", "InstallRule": [{"Type": "Custom"}]})
                .to_string(),
        )
        .unwrap();
        assert_eq!(parse_info_json(other.path()).unwrap()["mod_type"], "custom");
    }

    #[test]
    fn list_native_server_mods_merges_sources_and_marks_enabled() {
        let scratch = tempfile::tempdir().unwrap();
        let install = scratch.path().join("server");
        let workshop = scratch.path().join("workshop");
        let workshop_mod = workshop.join("123456");
        std::fs::create_dir_all(&workshop_mod).unwrap();
        std::fs::write(workshop_mod.join("Info.json"), INFO_JSON).unwrap();
        let install_text = install.to_string_lossy().to_string();
        // Enable CoolPackage plus a package that exists only in config
        write_palmodsettings(
            &install_text,
            true,
            &["CoolPackage".to_string(), "GhostPackage".to_string()],
            &workshop.to_string_lossy(),
        )
        .unwrap();
        let record = native_record(&install_text, &workshop.to_string_lossy());
        let mods = list_native_server_mods(&record);
        assert_eq!(mods.len(), 2);
        assert_eq!(mods[0]["mod_name"], "CoolPackage");
        assert_eq!(mods[0]["source"], "workshop");
        assert_eq!(mods[0]["enabled"], true);
        assert_eq!(
            mods[1],
            serde_json::json!({
                "mod_name": "GhostPackage",
                "display_name": "GhostPackage",
                "mod_type": "unknown",
                "mod_version": "",
                "mod_author": "",
                "is_server": true,
                "source": "config",
                "enabled": true
            })
        );
    }

    #[test]
    fn toggle_native_mod_adds_and_removes_active_entries() {
        let scratch = tempfile::tempdir().unwrap();
        let install = scratch.path().to_string_lossy().to_string();
        write_palmodsettings(&install, true, &[], "").unwrap();
        toggle_native_mod(&install, "PackA", true).unwrap();
        assert_eq!(read_palmodsettings(&install).active_mods, vec!["PackA"]);
        toggle_native_mod(&install, "PackA", false).unwrap();
        assert!(read_palmodsettings(&install).active_mods.is_empty());
    }

    /// The keys only take effect under the [PalModSettings] header; without it
    /// PalServer.exe rewrites the file to bare defaults on the next launch and
    /// every enabled mod silently turns itself off.
    #[test]
    fn read_palmodsettings_ignores_keys_outside_the_mod_section() {
        let scratch = tempfile::tempdir().unwrap();
        let install = scratch.path().to_string_lossy().to_string();
        std::fs::create_dir_all(Path::new(&install).join("Mods")).unwrap();
        std::fs::write(
            palmodsettings_path(&install),
            "[PalModSettings]\nbGlobalEnableMod=true\nActiveModList=PackA\n\
             [Other]\nActiveModList=NotMine\nWorkshopRootDir=D:/wrong\n",
        )
        .unwrap();
        let settings = read_palmodsettings(&install);
        assert!(settings.enabled);
        assert_eq!(settings.active_mods, vec!["PackA"]);
        assert_eq!(settings.workshop_root_dir, "");
    }

    /// Files written by earlier builds have no header at all; they are still
    /// read so a user's mod selection survives the upgrade.
    #[test]
    fn read_palmodsettings_accepts_headerless_legacy_files() {
        let scratch = tempfile::tempdir().unwrap();
        let install = scratch.path().to_string_lossy().to_string();
        std::fs::create_dir_all(Path::new(&install).join("Mods")).unwrap();
        std::fs::write(
            palmodsettings_path(&install),
            "bGlobalEnableMod=true\nActiveModList=PackA\n",
        )
        .unwrap();
        let settings = read_palmodsettings(&install);
        assert!(settings.enabled);
        assert_eq!(settings.active_mods, vec!["PackA"]);
    }

    /// ConfigVersion and the game's own bookkeeping keys are round-tripped;
    /// dropping DeleteModList would strand mods the game meant to uninstall.
    #[test]
    fn write_palmodsettings_preserves_config_version_and_unknown_keys() {
        let scratch = tempfile::tempdir().unwrap();
        let install = scratch.path().to_string_lossy().to_string();
        std::fs::create_dir_all(Path::new(&install).join("Mods")).unwrap();
        std::fs::write(
            palmodsettings_path(&install),
            "[PalModSettings]\nConfigVersion=2.5\nDeleteModList=OldPack\n\
             bNeedShowErrorOnNextStart=true\n",
        )
        .unwrap();
        write_palmodsettings(&install, true, &["PackA".to_string()], "").unwrap();
        assert_eq!(
            std::fs::read_to_string(palmodsettings_path(&install)).unwrap(),
            "[PalModSettings]\nConfigVersion=2.5\nbGlobalEnableMod=true\nActiveModList=PackA\n\
             DeleteModList=OldPack\nbNeedShowErrorOnNextStart=true\n"
        );
    }

    /// PalServer.exe resets bGlobalEnableMod to false whenever it rewrites the
    /// file, and nothing else in PSP ever sets it back, so enabling a mod has to
    /// raise the global flag or the selection stays inert.
    #[test]
    fn toggle_native_mod_reenables_the_global_flag() {
        let scratch = tempfile::tempdir().unwrap();
        let install = scratch.path().to_string_lossy().to_string();
        write_palmodsettings(&install, false, &[], "").unwrap();
        toggle_native_mod(&install, "PackA", true).unwrap();
        assert!(read_palmodsettings(&install).enabled);
    }

    #[test]
    fn install_native_workshop_mod_extracts_and_activates_package_name() {
        let scratch = tempfile::tempdir().unwrap();
        let install = scratch.path().to_string_lossy().to_string();
        let zip_b64 = crate::services::docker_mods::zip_fixture::base64_zip(&[
            ("Info.json", INFO_JSON),
            ("payload/mod.pak", "PAK"),
        ]);
        assert!(install_native_workshop_mod(&install, "cool-mod", &zip_b64));
        assert!(local_workshop_path(&install)
            .join("cool-mod")
            .join("payload")
            .join("mod.pak")
            .exists());
        // ActiveModList uses PackageName from Info.json, not the zip name
        assert_eq!(
            read_palmodsettings(&install).active_mods,
            vec!["CoolPackage"]
        );
    }

    #[test]
    fn ensure_mod_settings_creates_structure_and_syncs_workshop_dir() {
        let scratch = tempfile::tempdir().unwrap();
        let install = scratch.path().to_string_lossy().to_string();
        let record = native_record(&install, "D:/workshop/content/1623730");
        ensure_mod_settings(&record).unwrap();
        assert!(local_workshop_path(&install).is_dir());
        let settings = read_palmodsettings(&install);
        assert!(settings.enabled);
        assert_eq!(settings.workshop_root_dir, "D:/workshop/content/1623730");
        // Changing workshop_dir on the record re-syncs the ini
        let moved = native_record(&install, "E:/other/1623730");
        ensure_mod_settings(&moved).unwrap();
        assert_eq!(
            read_palmodsettings(&install).workshop_root_dir,
            "E:/other/1623730"
        );
    }
}
