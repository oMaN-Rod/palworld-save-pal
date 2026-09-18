//! The native server's `Mods/PalModSettings.ini` bootstrap, and locating the
//! Steam workshop content directory a server reads workshop mods from.
use std::path::{Path, PathBuf};

use ps_core::mods::palmodsettings::PalModSettings;
use ps_db::servers::ServerRecord;

use super::ini_text;

/// Palworld Steam app id for workshop content (not the dedicated-server app id).
pub(crate) const WORKSHOP_APP_ID: &str = "1623730";

fn palmodsettings_path(install_path: &str) -> PathBuf {
    Path::new(install_path)
        .join("Mods")
        .join("PalModSettings.ini")
}

#[derive(Debug, thiserror::Error)]
pub enum ModSettingsError {
    #[error("{} cannot be read: {source}", .path.display())]
    Unreadable {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{} cannot be written: {source}", .path.display())]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// The server loads no mods unless `Mods/Workshop` and `PalModSettings.ini` both
/// exist, so both are created before start, and `WorkshopRootDir` follows the
/// record whenever the record's workshop directory has moved. A file that exists
/// but cannot be decoded is reported rather than overwritten with defaults, and a
/// rewrite keeps the file's encoding.
pub fn ensure_mod_settings(record: &ServerRecord) -> Result<(), ModSettingsError> {
    let install = Path::new(&record.install_path);
    let path = palmodsettings_path(&record.install_path);
    let write_error = |source| ModSettingsError::Write {
        path: path.clone(),
        source,
    };
    std::fs::create_dir_all(install.join("Mods").join("Workshop")).map_err(write_error)?;
    let existing = ini_text::read(&path).map_err(|source| ModSettingsError::Unreadable {
        path: path.clone(),
        source,
    })?;
    let mut settings = existing
        .as_ref()
        .map(|file| PalModSettings::parse(&file.text))
        .unwrap_or_else(|| PalModSettings {
            enabled: true,
            ..PalModSettings::default()
        });
    let wanted = record.workshop_dir.as_str();
    let case_insensitive = cfg!(any(windows, target_os = "macos"));
    let same_dir = ps_core::mods::normalize_physical_path(&settings.workshop_root_dir, case_insensitive)
        == ps_core::mods::normalize_physical_path(wanted, case_insensitive);
    if existing.is_some() && (wanted.is_empty() || same_dir) {
        return Ok(());
    }
    if !wanted.is_empty() {
        settings.workshop_root_dir = wanted.to_string();
    }
    let encoding = existing.map(|file| file.encoding).unwrap_or_default();
    super::deploy::write::write_staged(&ini_text::encode(&settings.render(), encoding), &path)
        .map_err(write_error)
}

/// The Steam workshop content dir (app 1623730): every Steam library first,
/// then a guess at common folder names on each drive.
pub fn find_steam_workshop_dir() -> Option<String> {
    find_steam_workshop_dir_in(
        &super::detect::steam_library_dirs(),
        scan_drives_for_workshop_dir,
    )
}

pub fn find_steam_workshop_dir_in(
    libraries: &[PathBuf],
    fallback: impl FnOnce() -> Option<String>,
) -> Option<String> {
    libraries
        .iter()
        .map(|library| {
            library
                .join("steamapps")
                .join("workshop")
                .join("content")
                .join(WORKSHOP_APP_ID)
        })
        .find(|candidate| candidate.is_dir())
        .map(|dir| ps_core::mods::native_separators(&dir.to_string_lossy(), cfg!(windows)))
        .or_else(fallback)
}

fn scan_drives_for_workshop_dir() -> Option<String> {
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

    fn native_record(install_path: &Path, workshop_dir: &str) -> ServerRecord {
        let mut record = crate::services::docker::test_support::docker_record();
        record.server_type = "native".to_string();
        record.install_path = install_path.to_string_lossy().into_owned();
        record.workshop_dir = workshop_dir.to_string();
        record
    }

    fn ini(install: &Path) -> PathBuf {
        install.join("Mods").join("PalModSettings.ini")
    }

    fn steam_listing(steam: &Path, library: &Path) {
        let vdf = steam.join("steamapps").join("libraryfolders.vdf");
        std::fs::create_dir_all(vdf.parent().unwrap()).unwrap();
        std::fs::write(
            vdf,
            format!(
                "\"libraryfolders\"\n{{\n    \"0\"\n    {{\n        \"path\"        \"{}\"\n    }}\n}}\n",
                library.to_string_lossy().replace('\\', "\\\\")
            ),
        )
        .unwrap();
    }

    #[test]
    fn the_workshop_dir_is_found_through_a_steam_library_outside_program_files() {
        let scratch = tempfile::tempdir().unwrap();
        let steam = scratch.path().join("Programs").join("Steam");
        let library = scratch.path().join("Games").join("SteamLibrary");
        steam_listing(&steam, &library);
        let content = library
            .join("steamapps")
            .join("workshop")
            .join("content")
            .join(WORKSHOP_APP_ID);
        std::fs::create_dir_all(&content).unwrap();

        let libraries = super::super::detect::steam_library_dirs_in(&[steam]);
        let found =
            find_steam_workshop_dir_in(&libraries, || panic!("the drive scan must not run"));

        assert_eq!(found.as_deref(), Some(&*content.to_string_lossy()));
    }

    #[test]
    fn the_drive_scan_runs_when_no_library_has_the_workshop_dir() {
        let scratch = tempfile::tempdir().unwrap();
        let steam = scratch.path().join("Programs").join("Steam");
        steam_listing(&steam, &steam);

        let libraries = super::super::detect::steam_library_dirs_in(&[steam]);
        let found = find_steam_workshop_dir_in(&libraries, || Some("from the drive scan".into()));

        assert_eq!(found.as_deref(), Some("from the drive scan"));
    }

    #[test]
    fn seeds_a_missing_file_with_modding_enabled() {
        let scratch = tempfile::tempdir().unwrap();
        let record = native_record(scratch.path(), "D:/workshop/content/1623730");
        ensure_mod_settings(&record).unwrap();
        assert!(scratch.path().join("Mods").join("Workshop").is_dir());
        assert_eq!(
            std::fs::read_to_string(ini(scratch.path())).unwrap(),
            "[PalModSettings]\nConfigVersion=1.0\nbGlobalEnableMod=true\nWorkshopRootDir=D:/workshop/content/1623730\n"
        );
    }

    #[test]
    fn resyncs_a_moved_workshop_dir_and_keeps_the_selection() {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(scratch.path().join("Mods")).unwrap();
        std::fs::write(
            ini(scratch.path()),
            "[PalModSettings]\nConfigVersion=1.0\nbGlobalEnableMod=false\nWorkshopRootDir=D:/old/1623730\nActiveModList=PackA\n",
        )
        .unwrap();
        ensure_mod_settings(&native_record(scratch.path(), "E:/other/1623730")).unwrap();
        let settings =
            PalModSettings::parse(&std::fs::read_to_string(ini(scratch.path())).unwrap());
        assert_eq!(settings.workshop_root_dir, "E:/other/1623730");
        assert_eq!(settings.active_mods, vec!["PackA".to_string()]);
        assert!(!settings.enabled);
    }

    #[test]
    fn leaves_an_unchanged_file_byte_identical() {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(scratch.path().join("Mods")).unwrap();
        let original = "bGlobalEnableMod=True\r\nWorkshopRootDir=D:\\w\\1623730\r\nActiveModList=PackA\r\n";
        std::fs::write(ini(scratch.path()), original).unwrap();
        ensure_mod_settings(&native_record(scratch.path(), "D:\\w\\1623730")).unwrap();
        ensure_mod_settings(&native_record(scratch.path(), "")).unwrap();
        assert_eq!(std::fs::read_to_string(ini(scratch.path())).unwrap(), original);
    }

    #[test]
    fn a_workshop_dir_spelled_with_other_separators_is_not_rewritten() {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(scratch.path().join("Mods")).unwrap();
        let original = "bGlobalEnableMod=True\r\nWorkshopRootDir=D:\\w\\1623730\\\r\n";
        std::fs::write(ini(scratch.path()), original).unwrap();
        ensure_mod_settings(&native_record(scratch.path(), "D:/w/1623730")).unwrap();
        assert_eq!(std::fs::read_to_string(ini(scratch.path())).unwrap(), original);
        assert_eq!(
            std::fs::read_dir(scratch.path().join("Mods")).unwrap().count(),
            2,
            "only Workshop and the ini, nothing staged beside it"
        );
    }

    #[test]
    fn preserves_unknown_lines_when_rewriting() {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(scratch.path().join("Mods")).unwrap();
        std::fs::write(
            ini(scratch.path()),
            "[PalModSettings]\nConfigVersion=2.5\nDeleteModList=OldPack\nbNeedShowErrorOnNextStart=true\n",
        )
        .unwrap();
        ensure_mod_settings(&native_record(scratch.path(), "D:/w/1623730")).unwrap();
        assert_eq!(
            std::fs::read_to_string(ini(scratch.path())).unwrap(),
            "[PalModSettings]\nConfigVersion=2.5\nbGlobalEnableMod=false\nWorkshopRootDir=D:/w/1623730\nDeleteModList=OldPack\nbNeedShowErrorOnNextStart=true\n"
        );
    }

    #[test]
    fn an_unreadable_file_is_reported_not_overwritten() {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(scratch.path().join("Mods")).unwrap();
        for bytes in [&[0xC3, 0x28, b'A'][..], &[0xFF, 0xFE, b'A'][..]] {
            std::fs::write(ini(scratch.path()), bytes).unwrap();
            let outcome = ensure_mod_settings(&native_record(scratch.path(), "D:/w/1623730"));
            assert!(
                matches!(outcome, Err(ModSettingsError::Unreadable { .. })),
                "{outcome:?}"
            );
            assert_eq!(std::fs::read(ini(scratch.path())).unwrap(), bytes);
        }
    }

    #[test]
    fn a_utf16_file_is_rewritten_in_utf16_with_its_mark_and_unknown_lines() {
        use super::ini_text::{decode, encode, IniEncoding};
        let scratch = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(scratch.path().join("Mods")).unwrap();
        let original = "[PalModSettings]\r\nbGlobalEnableMod=True\r\nWorkshopRootDir=C:\\Users\\José\\old\r\nActiveModList=SteamThing\r\nDeleteModList=OldPack\r\n";
        std::fs::write(ini(scratch.path()), encode(original, IniEncoding::Utf16Le)).unwrap();

        ensure_mod_settings(&native_record(scratch.path(), "C:\\Users\\José\\old")).unwrap();
        assert_eq!(
            std::fs::read(ini(scratch.path())).unwrap(),
            encode(original, IniEncoding::Utf16Le),
            "an unchanged UTF-16 file is left alone"
        );

        ensure_mod_settings(&native_record(scratch.path(), "D:\\Jörg\\new")).unwrap();
        let bytes = std::fs::read(ini(scratch.path())).unwrap();
        assert_eq!(&bytes[..2], &[0xFF, 0xFE]);
        let decoded = decode(&bytes).unwrap();
        assert_eq!(decoded.encoding, IniEncoding::Utf16Le);
        let settings = PalModSettings::parse(&decoded.text);
        assert_eq!(settings.workshop_root_dir, "D:\\Jörg\\new");
        assert_eq!(settings.active_mods, vec!["SteamThing".to_string()]);
        assert!(
            decoded.text.contains("DeleteModList=OldPack"),
            "{}",
            decoded.text
        );
    }
}
