use ps_core::mods::{Platform, TargetKind, Ue4ssMode};
use ps_db::mod_targets::ModTarget;
use ps_server::services::mods::layout;

fn target(kind: &str, platform: &str, mode: &str, overrides: &str) -> ModTarget {
    ModTarget {
        id: "client-steam".to_string(),
        kind: kind.to_string(),
        server_id: None,
        name: "Steam".to_string(),
        root_path: "C:/Games/Palworld".to_string(),
        platform: platform.to_string(),
        ue4ss_mode: mode.to_string(),
        layout_overrides: overrides.to_string(),
        detected: "{}".to_string(),
        last_scanned_at: None,
        created_at: "2026-01-01T00:00:00".to_string(),
        updated_at: "2026-01-01T00:00:00".to_string(),
    }
}

#[test]
fn a_client_row_resolves_to_the_standard_layout() {
    let resolved = layout::layout_for(&target("client", "win64", "standard", "{}")).unwrap();
    assert_eq!(resolved.root, std::path::Path::new("C:/Games/Palworld"));
    assert!(
        resolved
            .ue4ss_mods_dir
            .as_ref()
            .unwrap()
            .ends_with("ue4ss/Mods"),
        "{:?}",
        resolved.ue4ss_mods_dir
    );
    assert!(resolved.paks_mods_dir.ends_with("Pal/Content/Paks/~mods"));
    assert!(resolved
        .logicmods_dir
        .ends_with("Pal/Content/Paks/LogicMods"));
}

#[test]
fn the_three_enum_columns_parse_every_spelling_the_database_stores() {
    for (text, expected) in [
        ("client", TargetKind::Client),
        ("native_server", TargetKind::NativeServer),
        ("docker_server", TargetKind::DockerServer),
    ] {
        let spec = layout::spec_for(&target(text, "win64", "none", "{}")).unwrap();
        assert_eq!(spec.kind, expected, "{text}");
    }
    for (text, expected) in [
        ("win64", Platform::Win64),
        ("wingdk", Platform::WinGdk),
        ("linux", Platform::Linux),
        ("mac", Platform::Mac),
    ] {
        let spec = layout::spec_for(&target("client", text, "none", "{}")).unwrap();
        assert_eq!(spec.platform, expected, "{text}");
    }
    for (text, expected) in [
        ("workshop", Ue4ssMode::Workshop),
        ("standard", Ue4ssMode::Standard),
        ("none", Ue4ssMode::None),
    ] {
        let spec = layout::spec_for(&target("client", "win64", text, "{}")).unwrap();
        assert_eq!(spec.ue4ss_mode, expected, "{text}");
    }
}

#[test]
fn an_unknown_enum_value_is_an_error_naming_the_value() {
    let kind = layout::spec_for(&target("potato", "win64", "none", "{}"));
    assert!(matches!(
        kind,
        Err(layout::LayoutResolveError::UnknownKind(ref v)) if v == "potato"
    ));
    assert!(matches!(
        layout::spec_for(&target("client", "amiga", "none", "{}")),
        Err(layout::LayoutResolveError::UnknownPlatform(_))
    ));
    assert!(matches!(
        layout::spec_for(&target("client", "win64", "sideways", "{}")),
        Err(layout::LayoutResolveError::UnknownMode(_))
    ));
}

#[test]
fn an_override_replaces_a_base_and_carries_its_derived_paths_with_it() {
    let resolved = layout::layout_for(&target(
        "client",
        "win64",
        "standard",
        r#"{"ue4ss_mods_dir":"D:/Elsewhere/Mods"}"#,
    ))
    .unwrap();
    assert_eq!(
        resolved.ue4ss_mods_dir.as_deref(),
        Some(std::path::Path::new("D:/Elsewhere/Mods"))
    );
    assert_eq!(
        resolved.mods_txt.as_deref(),
        Some(std::path::Path::new("D:/Elsewhere/Mods/mods.txt")),
        "mods.txt is derived from the final base, so it moves with the override"
    );
    assert!(
        resolved
            .palschema_mods_dir
            .as_ref()
            .unwrap()
            .starts_with("D:/Elsewhere/Mods"),
        "{:?}",
        resolved.palschema_mods_dir
    );
}

#[test]
fn overriding_a_derived_path_is_refused() {
    for bad in [
        r#"{"mods_txt":"D:/x/mods.txt"}"#,
        r#"{"palschema_mods_dir":"D:/x"}"#,
        r#"{"executable":"D:/x/Pal.exe"}"#,
    ] {
        let result = layout::layout_for(&target("client", "win64", "standard", bad));
        assert!(
            matches!(result, Err(layout::LayoutResolveError::BadOverrides(_))),
            "{bad} should have been refused, got {result:?}"
        );
    }
}

#[test]
fn an_empty_overrides_column_is_treated_as_no_overrides() {
    // The column defaults to '{}' but a hand-edited row may hold an empty string.
    for text in ["{}", ""] {
        let resolved = layout::layout_for(&target("client", "win64", "standard", text));
        assert!(resolved.is_ok(), "{text:?} -> {resolved:?}");
    }
}

#[test]
fn a_docker_target_takes_its_four_mod_directories_from_overrides() {
    // A docker target's directories are host paths from the server record, so the
    // override is the whole story. Without them the resolver still produces
    // root-relative paths, which is why the backfill always writes the overrides.
    let bare = layout::layout_for(&target("docker_server", "linux", "none", "{}")).unwrap();
    assert!(
        bare.paks_mods_dir.starts_with("C:/Games/Palworld"),
        "{:?}",
        bare.paks_mods_dir
    );

    let resolved = layout::layout_for(&target(
        "docker_server",
        "linux",
        "none",
        r#"{"ue4ss_mods_dir":"C:/ps/servers/a/mods","logicmods_dir":"C:/ps/servers/a/logicmods","nativemods_dir":"C:/ps/servers/a/nativemods","paks_mods_dir":"C:/ps/servers/a/paks"}"#,
    ))
    .unwrap();
    assert_eq!(
        resolved.ue4ss_mods_dir.as_deref(),
        Some(std::path::Path::new("C:/ps/servers/a/mods"))
    );
    assert_eq!(
        resolved.paks_mods_dir,
        std::path::Path::new("C:/ps/servers/a/paks")
    );
    assert_eq!(
        resolved.logicmods_dir,
        std::path::Path::new("C:/ps/servers/a/logicmods")
    );
    assert_eq!(
        resolved.nativemods_dir.as_deref(),
        Some(std::path::Path::new("C:/ps/servers/a/nativemods"))
    );
    assert_eq!(
        resolved.mods_txt.as_deref(),
        Some(std::path::Path::new("C:/ps/servers/a/mods/mods.txt")),
        "and the derived marker path follows the override"
    );
}

#[test]
fn a_server_target_resolves_as_docker_or_native_by_its_platform() {
    let docker = layout::spec_for(&target("server", "linux", "none", "{}")).unwrap();
    assert_eq!(docker.kind, TargetKind::DockerServer);
    let resolved = layout::layout_for(&target("server", "linux", "none", "{}")).unwrap();
    assert_eq!(resolved.palmodsettings_ini, None);
    assert_eq!(resolved.workshop_local_dir, None);

    let native = layout::spec_for(&target("server", "win64", "none", "{}")).unwrap();
    assert_eq!(native.kind, TargetKind::NativeServer);
    let resolved = layout::layout_for(&target("server", "win64", "none", "{}")).unwrap();
    assert_eq!(
        resolved.palmodsettings_ini.as_deref(),
        Some(std::path::Path::new("C:/Games/Palworld/Mods/PalModSettings.ini"))
    );
    assert!(resolved.workshop_local_dir.is_some());
}

#[test]
fn a_stored_root_with_mixed_separators_resolves_with_the_hosts_own() {
    let mut row = target(
        "client",
        "win64",
        "standard",
        r#"{"paks_mods_dir":"D:/Custom/Paks"}"#,
    );
    row.root_path = r"D:\Games\Steam\steamapps/common/Palworld".to_string();

    let spec = layout::spec_for(&row).unwrap();
    let resolved = layout::layout_for(&row).unwrap();

    if cfg!(windows) {
        assert_eq!(spec.root, r"D:\Games\Steam\steamapps\common\Palworld");
        assert_eq!(
            spec.overrides.paks_mods_dir.as_deref(),
            Some(r"D:\Custom\Paks")
        );
        for path in [
            Some(resolved.root.as_path()),
            Some(resolved.paks_mods_dir.as_path()),
            Some(resolved.logicmods_dir.as_path()),
            resolved.ue4ss_mods_dir.as_deref(),
            resolved.mods_txt.as_deref(),
            resolved.palmodsettings_ini.as_deref(),
            resolved.workshop_local_dir.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            assert!(!path.to_string_lossy().contains('/'), "{path:?}");
        }
    } else {
        assert_eq!(spec.root, row.root_path);
        assert_eq!(
            spec.overrides.paks_mods_dir.as_deref(),
            Some("D:/Custom/Paks")
        );
    }
}
