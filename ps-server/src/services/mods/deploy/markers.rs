use std::path::PathBuf;

use ps_core::mods::{ModsTxt, PalModSettings, TargetLayout};

use super::super::{digest, ini_text};
use super::desired::DesiredSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkerWrite {
    pub path: PathBuf,
    pub contents: String,
    /// `contents` in the encoding the file on disk already had: what is written,
    /// and what `hash` covers.
    pub bytes: Vec<u8>,
    pub hash: String,
}

fn marker(path: PathBuf, contents: String, bytes: Vec<u8>) -> MarkerWrite {
    let hash = digest::hash_bytes(&bytes);
    MarkerWrite {
        path,
        contents,
        bytes,
        hash,
    }
}

/// Rewritten whole from the desired set on every apply, never diffed: a marker
/// describes many mods at once and so belongs to no mod version. The merge is
/// non-destructive — every line the codec does not manage survives verbatim —
/// which is how a user's edit to a marker is preserved line by line. The file is
/// written back in the encoding it was saved in, and one that cannot be decoded
/// fails the apply rather than being rebuilt from nothing.
pub fn mods_txt(layout: &TargetLayout, set: &DesiredSet) -> std::io::Result<Option<MarkerWrite>> {
    let Some(path) = layout.mods_txt.clone() else {
        return Ok(None);
    };
    let existing = ini_text::read(&path)?;
    let encoding = existing
        .as_ref()
        .map(|file| file.encoding)
        .unwrap_or_default();
    let text = match existing {
        Some(file) => file.text,
        None if set.seed_ue4ss_mods_txt => ps_core::mods::UE4SS_DEFAULT_MODS_TXT.to_string(),
        None => String::new(),
    };
    let mut file = ModsTxt::parse(&text);
    file.set_managed(&set.ue4ss_entries, &set.ue4ss_purge);
    let contents = file.render();
    let bytes = ini_text::encode(&contents, encoding);
    Ok(Some(marker(path, contents, bytes)))
}

/// `with_managed_active` re-raises `bGlobalEnableMod` on every write, because the
/// server clears it when it rewrites the file, and touches only the packages the
/// library manages: a Steam-subscribed package's line is kept where it sits. A
/// file that exists but cannot be decoded fails the apply instead of being
/// replaced by one built from nothing.
pub fn palmodsettings(
    layout: &TargetLayout,
    set: &DesiredSet,
) -> std::io::Result<Option<MarkerWrite>> {
    let Some(path) = layout.palmodsettings_ini.clone() else {
        return Ok(None);
    };
    let existing = ini_text::read(&path)?;
    let encoding = existing
        .as_ref()
        .map(|file| file.encoding)
        .unwrap_or_default();
    let text = existing.map(|file| file.text).unwrap_or_default();
    let settings = PalModSettings::parse(&text)
        .with_managed_active(&set.workshop_active, &set.workshop_managed);
    let contents = settings.render();
    let bytes = ini_text::encode(&contents, encoding);
    Ok(Some(marker(path, contents, bytes)))
}

pub fn plan_markers(layout: &TargetLayout, set: &DesiredSet) -> std::io::Result<Vec<MarkerWrite>> {
    let mut planned: Vec<MarkerWrite> = mods_txt(layout, set)?.into_iter().collect();
    planned.extend(palmodsettings(layout, set)?);
    Ok(planned)
}

#[cfg(test)]
mod tests {
    use ps_core::mods::{Platform, TargetKind, TargetSpec, Ue4ssMode};

    use super::*;

    fn client_layout(root: &std::path::Path) -> TargetLayout {
        ps_core::mods::resolve_layout(&TargetSpec {
            kind: TargetKind::Client,
            root: root.to_string_lossy().into_owned(),
            platform: Platform::Win64,
            ue4ss_mode: Ue4ssMode::Standard,
            overrides: Default::default(),
        })
        .unwrap()
    }

    #[test]
    fn a_missing_mods_txt_seeds_from_the_ue4ss_default_when_a_slot_exists() {
        let dir = tempfile::tempdir().unwrap();
        let layout = client_layout(dir.path());
        let set = DesiredSet {
            seed_ue4ss_mods_txt: true,
            ue4ss_entries: vec![("CoolMod".into(), true)],
            ..Default::default()
        };

        let write = mods_txt(&layout, &set).unwrap().unwrap();

        assert!(write.contents.starts_with("CheatManagerEnablerMod : 0"));
        assert!(write.contents.contains("BPModLoaderMod : 1\r\nCoolMod : 1"));
    }

    #[test]
    fn without_the_flag_a_missing_mods_txt_starts_empty() {
        let dir = tempfile::tempdir().unwrap();
        let layout = client_layout(dir.path());
        let set = DesiredSet {
            seed_ue4ss_mods_txt: false,
            ue4ss_entries: vec![("CoolMod".into(), true)],
            ..Default::default()
        };

        let write = mods_txt(&layout, &set).unwrap().unwrap();

        assert_eq!(write.contents, "CoolMod : 1\r\n");
    }

    #[test]
    fn an_existing_file_on_disk_is_unaffected_by_the_flag() {
        let dir = tempfile::tempdir().unwrap();
        let layout = client_layout(dir.path());
        let path = layout.mods_txt.clone().unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "ExistingMod : 1\r\n").unwrap();
        let set = DesiredSet {
            seed_ue4ss_mods_txt: true,
            ue4ss_entries: vec![("CoolMod".into(), true)],
            ..Default::default()
        };

        let write = mods_txt(&layout, &set).unwrap().unwrap();

        assert_eq!(write.contents, "ExistingMod : 1\r\nCoolMod : 1\r\n");
    }
}
