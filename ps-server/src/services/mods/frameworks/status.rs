//! Detecting which frameworks a target has installed, their versions, and any
//! hazards (dual UE4SS installs, workshop proxy DLLs, a legacy Amity folder).
use std::path::Path;

use ps_core::mods::{
    native_separators, parse_workshop_info, resolve_layout, FrameworkKey, TargetKind, TargetLayout,
    Ue4ssMode,
};
use ps_db::mod_profiles::TargetFramework;
use ps_db::mod_targets::ModTarget;

use crate::services::mods::detect::{detect_ue4ss, HAZARD_UE4SS_DUAL_INSTANCE};
use crate::services::mods::layout::{self, LayoutResolveError};

pub const HAZARD_WORKSHOP_PROXY_DLL: &str = "workshop_proxy_dll";
pub const HAZARD_AMITY_LEGACY_FOLDER: &str = "amity_legacy_folder";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Installed {
    pub present: bool,
    pub version: Option<String>,
    pub managed: bool,
    pub mod_version_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Hazard {
    pub code: String,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detection {
    pub ue4ss_mode: Ue4ssMode,
    pub hazards: Vec<Hazard>,
    pub installed: Vec<(FrameworkKey, Installed)>,
}

#[derive(Debug, thiserror::Error)]
pub enum StatusError {
    #[error(transparent)]
    Db(#[from] ps_db::DbError),
    #[error(transparent)]
    Layout(#[from] LayoutResolveError),
    #[error("framework detection does not apply to a Docker server")]
    Docker,
}

/// Resolves the target's layout with `mode` in force, substituting `Standard`
/// for `None` so the UE4SS paths exist to probe even on a target that has
/// nothing installed yet.
pub fn probe_layout(
    target: &ModTarget,
    mode: Ue4ssMode,
) -> Result<TargetLayout, LayoutResolveError> {
    let mut spec = layout::spec_for(target)?;
    spec.ue4ss_mode = if mode == Ue4ssMode::None {
        Ue4ssMode::Standard
    } else {
        mode
    };
    Ok(resolve_layout(&spec)?)
}

fn native(path: &Path) -> String {
    native_separators(&path.to_string_lossy(), cfg!(windows))
}

fn slot_for(slots: &[TargetFramework], key: FrameworkKey) -> Option<&TargetFramework> {
    slots.iter().find(|slot| slot.framework == key.as_str())
}

async fn slot_version(
    db: &dyn ps_db::DbDriver,
    slot: Option<&TargetFramework>,
) -> Result<Option<String>, ps_db::DbError> {
    match slot {
        None => Ok(None),
        Some(slot) => Ok(ps_db::mod_library::get_version(db, &slot.mod_version_id)
            .await?
            .map(|row| row.version)),
    }
}

fn trimmed_non_empty(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn dwmapi_modified_version(binaries_dir: &Path) -> Option<String> {
    let modified = std::fs::metadata(binaries_dir.join("dwmapi.dll"))
        .ok()?
        .modified()
        .ok()?;
    Some(
        chrono::DateTime::<chrono::Utc>::from(modified)
            .format("%d.%m.%Y")
            .to_string(),
    )
}

fn workshop_info_version(root: &Path) -> String {
    let info_path = root
        .join("Mods")
        .join("NativeMods")
        .join("UE4SS")
        .join("Info.json");
    std::fs::read_to_string(&info_path)
        .ok()
        .and_then(|text| parse_workshop_info(&text).ok())
        .map(|info| info.version)
        .filter(|version| !version.is_empty())
        .unwrap_or_else(|| "Workshop".to_string())
}

async fn ue4ss_installed(
    db: &dyn ps_db::DbDriver,
    layout: &TargetLayout,
    mode: Ue4ssMode,
    slots: &[TargetFramework],
) -> Result<Installed, ps_db::DbError> {
    let slot = slot_for(slots, FrameworkKey::Ue4ss);
    let present = mode != Ue4ssMode::None;
    let mut version = slot_version(db, slot).await?;
    if version.is_none() {
        version = layout
            .ue4ss_dir
            .as_deref()
            .and_then(|dir| trimmed_non_empty(&dir.join("ue4ss.version")));
    }
    if version.is_none() {
        version = layout
            .binaries_dir
            .as_deref()
            .and_then(dwmapi_modified_version);
    }
    if version.is_none() && mode == Ue4ssMode::Workshop {
        version = Some(workshop_info_version(&layout.root));
    }
    Ok(Installed {
        present,
        version,
        managed: slot.is_some(),
        mod_version_id: slot.map(|slot| slot.mod_version_id.clone()),
    })
}

async fn palschema_installed(
    db: &dyn ps_db::DbDriver,
    layout: &TargetLayout,
    slots: &[TargetFramework],
) -> Result<Installed, ps_db::DbError> {
    let slot = slot_for(slots, FrameworkKey::PalSchema);
    let base = layout
        .ue4ss_mods_dir
        .as_deref()
        .map(|dir| dir.join("PalSchema"));
    let files_present = base.as_deref().is_some_and(|base| {
        base.join("dlls").join("main.dll").is_file()
            || base.join("scripts").join("main.lua").is_file()
            || base.join("main.lua").is_file()
    });
    let mut version = slot_version(db, slot).await?;
    if version.is_none() {
        version = base
            .as_deref()
            .and_then(|base| trimmed_non_empty(&base.join("palschema.version")));
    }
    Ok(Installed {
        present: files_present || slot.is_some(),
        version,
        managed: slot.is_some(),
        mod_version_id: slot.map(|slot| slot.mod_version_id.clone()),
    })
}

async fn amity_installed(
    db: &dyn ps_db::DbDriver,
    layout: &TargetLayout,
    slots: &[TargetFramework],
) -> Result<Installed, ps_db::DbError> {
    let slot = slot_for(slots, FrameworkKey::Amity);
    let base = layout
        .ue4ss_mods_dir
        .as_deref()
        .map(|dir| dir.join("PSAmity"));
    let files_present = base
        .as_deref()
        .is_some_and(|base| base.join("dlls").join("main.dll").is_file());
    let version = slot_version(db, slot).await?;
    Ok(Installed {
        present: files_present || slot.is_some(),
        version,
        managed: slot.is_some(),
        mod_version_id: slot.map(|slot| slot.mod_version_id.clone()),
    })
}

fn workshop_proxy_hazard(layout: &TargetLayout) -> Option<Hazard> {
    let binaries_dir = layout.binaries_dir.as_deref()?;
    let paths: Vec<String> = ["dwmapi.dll", "xinput1_3.dll"]
        .into_iter()
        .map(|name| binaries_dir.join(name))
        .filter(|path| path.is_file())
        .map(|path| native(&path))
        .collect();
    (!paths.is_empty()).then_some(Hazard {
        code: HAZARD_WORKSHOP_PROXY_DLL.to_string(),
        paths,
    })
}

fn amity_legacy_hazard(layout: &TargetLayout) -> Option<Hazard> {
    let legacy = layout.ue4ss_mods_dir.as_deref()?.join("PSPAmity");
    let metadata = std::fs::symlink_metadata(&legacy).ok()?;
    metadata.is_dir().then(|| Hazard {
        code: HAZARD_AMITY_LEGACY_FOLDER.to_string(),
        paths: vec![native(&legacy)],
    })
}

pub async fn detect(
    db: &dyn ps_db::DbDriver,
    target: &ModTarget,
) -> Result<Detection, StatusError> {
    let spec = layout::spec_for(target)?;
    if spec.kind == TargetKind::DockerServer {
        return Err(StatusError::Docker);
    }
    let (mode, dual) = detect_ue4ss(Path::new(&target.root_path), spec.platform);
    let mut hazards: Vec<Hazard> = dual
        .iter()
        .map(|_| Hazard {
            code: HAZARD_UE4SS_DUAL_INSTANCE.to_string(),
            paths: Vec::new(),
        })
        .collect();

    let layout = probe_layout(target, mode)?;
    let slots = ps_db::mod_profiles::frameworks_of(db, &target.id).await?;

    let ue4ss = ue4ss_installed(db, &layout, mode, &slots).await?;
    let palschema = palschema_installed(db, &layout, &slots).await?;
    let amity = amity_installed(db, &layout, &slots).await?;

    if mode == Ue4ssMode::Workshop {
        if let Some(hazard) = workshop_proxy_hazard(&layout) {
            hazards.push(hazard);
        }
    }
    if let Some(hazard) = amity_legacy_hazard(&layout) {
        hazards.push(hazard);
    }

    Ok(Detection {
        ue4ss_mode: mode,
        hazards,
        installed: vec![
            (FrameworkKey::Ue4ss, ue4ss),
            (FrameworkKey::PalSchema, palschema),
            (FrameworkKey::Amity, amity),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ps_db::mod_targets::NewModTarget;

    fn write(path: &Path, body: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    struct Fixture {
        db: ps_db::SqlxSqliteDriver,
        target: ModTarget,
        root: std::path::PathBuf,
        _dir: tempfile::TempDir,
    }

    async fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let db =
            ps_db::SqlxSqliteDriver::new(ps_db::open(&dir.path().join("test.db")).await.unwrap());
        let root = dir.path().join("game");
        write(
            &root.join("Pal/Binaries/Win64/Palworld-Win64-Shipping.exe"),
            b"x",
        );
        std::fs::create_dir_all(root.join("Pal/Content/Paks")).unwrap();
        let target = ps_db::mod_targets::upsert(
            &db,
            &NewModTarget {
                id: "client-steam".to_string(),
                kind: "client".to_string(),
                name: "Steam".to_string(),
                root_path: root.to_string_lossy().into_owned(),
                platform: "win64".to_string(),
                ue4ss_mode: "none".to_string(),
                layout_overrides: "{}".to_string(),
                detected: "{}".to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        Fixture {
            db,
            target,
            root,
            _dir: dir,
        }
    }

    fn installed_of(detection: &Detection, key: FrameworkKey) -> &Installed {
        &detection
            .installed
            .iter()
            .find(|(k, _)| *k == key)
            .unwrap()
            .1
    }

    #[tokio::test]
    async fn nothing_installed_reports_absent_and_no_hazards() {
        let fixture = fixture().await;

        let detection = detect(&fixture.db, &fixture.target).await.unwrap();

        assert_eq!(detection.ue4ss_mode, Ue4ssMode::None);
        for key in FrameworkKey::ALL {
            let installed = installed_of(&detection, key);
            assert!(!installed.present, "{key:?}");
            assert_eq!(installed.version, None, "{key:?}");
        }
        assert!(detection.hazards.is_empty());
    }

    #[tokio::test]
    async fn standard_ue4ss_with_version_file_reports_that_version() {
        let fixture = fixture().await;
        write(&fixture.root.join("Pal/Binaries/Win64/dwmapi.dll"), b"x");
        write(
            &fixture.root.join("Pal/Binaries/Win64/ue4ss/ue4ss.version"),
            b"abc\n",
        );

        let detection = detect(&fixture.db, &fixture.target).await.unwrap();

        let ue4ss = installed_of(&detection, FrameworkKey::Ue4ss);
        assert!(ue4ss.present);
        assert_eq!(ue4ss.version, Some("abc".to_string()));
        assert!(!ue4ss.managed);
    }

    #[tokio::test]
    async fn standard_ue4ss_without_version_file_falls_back_to_dll_mtime() {
        let fixture = fixture().await;
        write(&fixture.root.join("Pal/Binaries/Win64/dwmapi.dll"), b"x");

        let detection = detect(&fixture.db, &fixture.target).await.unwrap();

        let ue4ss = installed_of(&detection, FrameworkKey::Ue4ss);
        assert!(ue4ss.present);
        let version = ue4ss.version.as_deref().unwrap();
        assert_eq!(version.len(), 10, "{version}");
        assert_eq!(version.as_bytes()[2], b'.', "{version}");
        assert_eq!(version.as_bytes()[5], b'.', "{version}");
    }

    #[tokio::test]
    async fn workshop_info_json_reports_workshop_mode_and_its_version() {
        let fixture = fixture().await;
        write(
            &fixture.root.join("Mods/NativeMods/UE4SS/Info.json"),
            br#"{"PackageName":"UE4SS","Version":"1.2"}"#,
        );

        let detection = detect(&fixture.db, &fixture.target).await.unwrap();

        assert_eq!(detection.ue4ss_mode, Ue4ssMode::Workshop);
        let ue4ss = installed_of(&detection, FrameworkKey::Ue4ss);
        assert!(ue4ss.present);
        assert_eq!(ue4ss.version, Some("1.2".to_string()));
    }

    #[tokio::test]
    async fn dual_install_hazards_include_the_workshop_proxy_dll() {
        let fixture = fixture().await;
        write(&fixture.root.join("Pal/Binaries/Win64/dwmapi.dll"), b"x");
        write(
            &fixture.root.join("Mods/NativeMods/UE4SS/Info.json"),
            br#"{"PackageName":"UE4SS","Version":"1.2"}"#,
        );
        write(
            &fixture.root.join("Mods/PalModSettings.ini"),
            b"[PalModSettings]\nbGlobalEnableMod=True\nActiveModList=UE4SS\n",
        );

        let detection = detect(&fixture.db, &fixture.target).await.unwrap();

        assert_eq!(detection.ue4ss_mode, Ue4ssMode::Workshop);
        let codes: Vec<&str> = detection
            .hazards
            .iter()
            .map(|hazard| hazard.code.as_str())
            .collect();
        assert!(codes.contains(&HAZARD_UE4SS_DUAL_INSTANCE), "{codes:?}");
        assert!(codes.contains(&HAZARD_WORKSHOP_PROXY_DLL), "{codes:?}");
        let proxy = detection
            .hazards
            .iter()
            .find(|hazard| hazard.code == HAZARD_WORKSHOP_PROXY_DLL)
            .unwrap();
        assert!(proxy.paths.iter().all(|path| path.ends_with("dwmapi.dll")));
    }

    #[tokio::test]
    async fn palschema_present_with_a_legacy_amity_folder_is_a_hazard() {
        let fixture = fixture().await;
        write(&fixture.root.join("Pal/Binaries/Win64/dwmapi.dll"), b"x");
        write(
            &fixture
                .root
                .join("Pal/Binaries/Win64/ue4ss/Mods/PalSchema/dlls/main.dll"),
            b"x",
        );
        write(
            &fixture
                .root
                .join("Pal/Binaries/Win64/ue4ss/Mods/PSPAmity/marker.txt"),
            b"x",
        );

        let detection = detect(&fixture.db, &fixture.target).await.unwrap();

        let palschema = installed_of(&detection, FrameworkKey::PalSchema);
        assert!(palschema.present);
        assert_eq!(palschema.version, None);
        let codes: Vec<&str> = detection
            .hazards
            .iter()
            .map(|hazard| hazard.code.as_str())
            .collect();
        assert!(codes.contains(&HAZARD_AMITY_LEGACY_FOLDER), "{codes:?}");
    }

    #[tokio::test]
    async fn a_symlinked_psp_amity_folder_is_not_a_hazard() {
        let fixture = fixture().await;
        write(&fixture.root.join("Pal/Binaries/Win64/dwmapi.dll"), b"x");
        let mods_dir = fixture.root.join("Pal/Binaries/Win64/ue4ss/Mods");
        std::fs::create_dir_all(&mods_dir).unwrap();
        let outside = fixture._dir.path().join("outside-amity");
        std::fs::create_dir_all(&outside).unwrap();
        let link = mods_dir.join("PSPAmity");
        #[cfg(windows)]
        let created = std::os::windows::fs::symlink_dir(&outside, &link).is_ok();
        #[cfg(unix)]
        let created = std::os::unix::fs::symlink(&outside, &link).is_ok();
        if !created {
            eprintln!("skipping: could not create a symlink in this environment");
            return;
        }

        let detection = detect(&fixture.db, &fixture.target).await.unwrap();

        let codes: Vec<&str> = detection
            .hazards
            .iter()
            .map(|hazard| hazard.code.as_str())
            .collect();
        assert!(!codes.contains(&HAZARD_AMITY_LEGACY_FOLDER), "{codes:?}");
    }

    #[tokio::test]
    async fn a_stored_framework_version_is_reported_as_managed() {
        let fixture = fixture().await;
        let paths = crate::services::mods::LibraryPaths::new(fixture._dir.path());
        let scratch = tempfile::tempdir().unwrap();
        write(&scratch.path().join("dlls/main.dll"), b"x");
        let manifest = ps_core::mods::InstallManifest {
            folder_name: "amity".to_string(),
            display_name: "PSAmity".to_string(),
            mod_type: ps_core::mods::ModType::Framework,
            version: "0.2.0".to_string(),
            routes: vec![ps_core::mods::FileRoute {
                archive_path: "dlls/main.dll".to_string(),
                rel_path: "dlls/main.dll".to_string(),
                kind: ps_core::mods::RouteKind::Ue4ss,
            }],
            decisions: Vec::new(),
            platform_filtered: None,
            source: ps_core::mods::SourceHint::default(),
        };
        let stored = crate::services::mods::library::store(
            &fixture.db,
            &paths,
            &crate::services::mods::library::StoreRequest {
                mod_id: "framework-amity",
                manifest: &manifest,
                extracted_root: scratch.path(),
                archive: None,
                source_kind: "framework",
                source_ref: "{}",
                custom_name: None,
            },
        )
        .await
        .unwrap();
        ps_db::mod_profiles::set_framework(
            &fixture.db,
            &fixture.target.id,
            "amity",
            &stored.version.id,
        )
        .await
        .unwrap();

        let detection = detect(&fixture.db, &fixture.target).await.unwrap();

        let amity = installed_of(&detection, FrameworkKey::Amity);
        assert!(amity.managed);
        assert_eq!(amity.version, Some("0.2.0".to_string()));
        assert_eq!(amity.mod_version_id, Some(stored.version.id));
    }
}
