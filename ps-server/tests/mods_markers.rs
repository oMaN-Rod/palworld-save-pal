use std::path::Path;

use ps_core::mods::{Platform, TargetKind, TargetSpec, Ue4ssMode};
use ps_server::services::mods::deploy::{desired::DesiredSet, markers};

fn layout(root: &Path) -> ps_core::mods::TargetLayout {
    ps_core::mods::resolve_layout(&TargetSpec {
        kind: TargetKind::Client,
        root: root.to_string_lossy().into_owned(),
        platform: Platform::Win64,
        ue4ss_mode: Ue4ssMode::Standard,
        overrides: Default::default(),
    })
    .unwrap()
}

fn set(ue4ss: &[(&str, bool)], workshop: &[&str]) -> DesiredSet {
    let entries: Vec<(String, bool)> = ue4ss.iter().map(|(n, e)| ((*n).to_string(), *e)).collect();
    DesiredSet {
        files: Vec::new(),
        ue4ss_purge: entries
            .iter()
            .filter(|(_, e)| !e)
            .map(|(n, _)| n.clone())
            .collect(),
        ue4ss_entries: entries,
        workshop_active: workshop.iter().map(|s| (*s).to_string()).collect(),
        workshop_managed: workshop.iter().map(|s| (*s).to_string()).collect(),
        workshop_released: Vec::new(),
        seed_ue4ss_mods_txt: false,
    }
}

#[test]
fn mods_txt_lists_the_managed_entries_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let l = layout(dir.path());
    let marker = markers::mods_txt(&l, &set(&[("Alpha", true), ("Beta", false)], &[]))
        .unwrap()
        .unwrap();
    assert_eq!(marker.path, *l.mods_txt.as_ref().unwrap());
    assert!(marker.contents.contains("Alpha : 1"), "{}", marker.contents);
    assert!(
        marker.contents.contains("Beta : 0"),
        "a disabled mod is listed as off, not omitted: {}",
        marker.contents
    );
    assert_eq!(
        marker.hash,
        ps_server::services::mods::digest::hash_bytes(marker.contents.as_bytes())
    );
}

#[test]
fn an_existing_mods_txt_keeps_every_line_the_codec_does_not_manage() {
    let dir = tempfile::tempdir().unwrap();
    let l = layout(dir.path());
    let path = l.mods_txt.clone().unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        "; a comment the user wrote\r\nSomeoneElsesMod : 1\r\nAlpha : 0\r\n",
    )
    .unwrap();

    let marker = markers::mods_txt(&l, &set(&[("Alpha", true)], &[]))
        .unwrap()
        .unwrap();
    assert!(
        marker.contents.contains("; a comment the user wrote"),
        "comments survive: {}",
        marker.contents
    );
    assert!(
        marker.contents.contains("SomeoneElsesMod : 1"),
        "an entry for a mod the app does not know survives: {}",
        marker.contents
    );
    assert!(marker.contents.contains("Alpha : 1"), "{}", marker.contents);
    assert_eq!(
        marker.contents.matches("Alpha").count(),
        1,
        "and the stale line is replaced, not duplicated: {}",
        marker.contents
    );
}

#[test]
fn palmodsettings_lists_the_active_packages_and_raises_the_global_flag() {
    let dir = tempfile::tempdir().unwrap();
    let l = layout(dir.path());
    let marker = markers::palmodsettings(&l, &set(&[], &["CoolPack", "OtherPack"]))
        .unwrap()
        .unwrap();
    assert!(
        marker.contents.contains("bGlobalEnableMod=true"),
        "{}",
        marker.contents
    );
    assert!(
        marker.contents.contains("ActiveModList=CoolPack"),
        "{}",
        marker.contents
    );
    assert!(
        marker.contents.contains("ActiveModList=OtherPack"),
        "{}",
        marker.contents
    );
}

#[test]
fn an_existing_palmodsettings_keeps_its_unknown_keys() {
    let dir = tempfile::tempdir().unwrap();
    let l = layout(dir.path());
    let path = l.palmodsettings_ini.clone().unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        "[PalModSettings]\nbGlobalEnableMod=False\nSomeFutureKey=17\nActiveModList=GonePack\n",
    )
    .unwrap();

    let mut desired = set(&[], &["CoolPack"]);
    desired.workshop_managed.push("GonePack".to_string());
    let marker = markers::palmodsettings(&l, &desired).unwrap().unwrap();
    assert!(
        marker.contents.contains("SomeFutureKey=17"),
        "an unknown key survives: {}",
        marker.contents
    );
    assert!(
        !marker.contents.contains("GonePack"),
        "and a package no longer active is dropped: {}",
        marker.contents
    );
    assert!(marker.contents.contains("bGlobalEnableMod=true"));
}

#[test]
fn nothing_is_produced_for_a_marker_the_layout_does_not_have() {
    let dir = tempfile::tempdir().unwrap();
    let mut l = layout(dir.path());
    l.mods_txt = None;
    l.palmodsettings_ini = None;
    assert!(markers::mods_txt(&l, &set(&[("Alpha", true)], &[]))
        .unwrap()
        .is_none());
    assert!(markers::palmodsettings(&l, &set(&[], &["CoolPack"]))
        .unwrap()
        .is_none());
    assert!(
        markers::plan_markers(&l, &set(&[("Alpha", true)], &["CoolPack"]))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn plan_markers_returns_both_when_both_apply() {
    let dir = tempfile::tempdir().unwrap();
    let l = layout(dir.path());
    let planned = markers::plan_markers(&l, &set(&[("Alpha", true)], &["CoolPack"])).unwrap();
    assert_eq!(planned.len(), 2, "{planned:?}");
    assert!(planned.iter().any(|m| m.path.ends_with("mods.txt")));
    assert!(planned
        .iter()
        .any(|m| m.path.ends_with("PalModSettings.ini")));
}

#[test]
fn a_package_the_library_does_not_manage_keeps_its_active_line() {
    let dir = tempfile::tempdir().unwrap();
    let l = layout(dir.path());
    let path = l.palmodsettings_ini.clone().unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        "[PalModSettings]\nbGlobalEnableMod=False\nActiveModList=SteamThing\nActiveModList=OldPack\n",
    )
    .unwrap();

    let mut desired = set(&[], &["CoolPack"]);
    desired.workshop_managed.push("OldPack".to_string());
    let marker = markers::palmodsettings(&l, &desired).unwrap().unwrap();
    let settings = ps_core::mods::PalModSettings::parse(&marker.contents);
    assert_eq!(
        settings.active_mods,
        vec!["SteamThing".to_string(), "CoolPack".to_string()],
        "{}",
        marker.contents
    );
}

#[test]
fn an_undecodable_palmodsettings_fails_instead_of_being_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let l = layout(dir.path());
    let path = l.palmodsettings_ini.clone().unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, [0xC3, 0x28, b'A']).unwrap();

    assert!(markers::palmodsettings(&l, &set(&[], &["CoolPack"])).is_err());
    assert!(markers::plan_markers(&l, &set(&[], &["CoolPack"])).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), [0xC3, 0x28, b'A']);
}

#[test]
fn a_utf16_palmodsettings_is_planned_back_in_utf16() {
    use ps_server::services::mods::ini_text::{decode, encode, IniEncoding};
    let dir = tempfile::tempdir().unwrap();
    let l = layout(dir.path());
    let path = l.palmodsettings_ini.clone().unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        encode(
            "[PalModSettings]\r\nWorkshopRootDir=C:\\Users\\José\\w\r\nSomeFutureKey=17\r\nActiveModList=SteamThing\r\n",
            IniEncoding::Utf16Le,
        ),
    )
    .unwrap();

    let marker = markers::palmodsettings(&l, &set(&[], &["CoolPack"]))
        .unwrap()
        .unwrap();
    assert_eq!(&marker.bytes[..2], &[0xFF, 0xFE]);
    assert_eq!(
        marker.hash,
        ps_server::services::mods::digest::hash_bytes(&marker.bytes)
    );
    let decoded = decode(&marker.bytes).unwrap();
    assert_eq!(decoded.encoding, IniEncoding::Utf16Le);
    assert_eq!(decoded.text, marker.contents);
    for line in [
        "SomeFutureKey=17",
        "WorkshopRootDir=C:\\Users\\José\\w",
        "ActiveModList=SteamThing",
        "ActiveModList=CoolPack",
    ] {
        assert!(decoded.text.contains(line), "{line} in {}", decoded.text);
    }
}

#[test]
fn a_utf16_mods_txt_is_planned_back_in_utf16() {
    use ps_server::services::mods::ini_text::{decode, encode, IniEncoding};
    let dir = tempfile::tempdir().unwrap();
    let l = layout(dir.path());
    let path = l.mods_txt.clone().unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        encode(
            "; José's list\r\nSomeoneElsesMod : 1\r\n",
            IniEncoding::Utf16Le,
        ),
    )
    .unwrap();

    let marker = markers::mods_txt(&l, &set(&[("Alpha", true)], &[]))
        .unwrap()
        .unwrap();
    assert_eq!(&marker.bytes[..2], &[0xFF, 0xFE]);
    assert_eq!(
        marker.hash,
        ps_server::services::mods::digest::hash_bytes(&marker.bytes)
    );
    let decoded = decode(&marker.bytes).unwrap();
    assert_eq!(decoded.encoding, IniEncoding::Utf16Le);
    assert_eq!(decoded.text, marker.contents);
    for line in ["; José's list", "SomeoneElsesMod : 1", "Alpha : 1"] {
        assert!(decoded.text.contains(line), "{line} in {}", decoded.text);
    }
}

#[test]
fn an_undecodable_mods_txt_fails_instead_of_being_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let l = layout(dir.path());
    let path = l.mods_txt.clone().unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, [0xFF, 0xFE, b'A']).unwrap();

    assert!(markers::mods_txt(&l, &set(&[("Alpha", true)], &[])).is_err());
    assert!(markers::plan_markers(&l, &set(&[("Alpha", true)], &[])).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), [0xFF, 0xFE, b'A']);
}
