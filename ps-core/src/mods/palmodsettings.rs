//! Palworld's own `Mods/PalModSettings.ini`: a `[PalModSettings]` section holding
//! `bGlobalEnableMod`, an optional `WorkshopRootDir`, and one repeated
//! `ActiveModList=<PackageName>` line per enabled mod. PalServer reads it through the
//! Unreal config system, which only sees keys under the section header and rewrites the
//! file from its own state at launch. That is why unknown keys inside the section are
//! preserved and why writing the list re-raises `bGlobalEnableMod`: the server sets it
//! back to false on its own.

const SECTION_HEADER: &str = "[PalModSettings]";
const DEFAULT_CONFIG_VERSION: &str = "1.0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PalModSettings {
    pub enabled: bool,
    pub active_mods: Vec<String>,
    pub workshop_root_dir: String,
    pub config_version: String,
    pub other_lines: Vec<String>,
}

impl Default for PalModSettings {
    fn default() -> Self {
        PalModSettings {
            enabled: false,
            active_mods: Vec::new(),
            workshop_root_dir: String::new(),
            config_version: DEFAULT_CONFIG_VERSION.to_string(),
            other_lines: Vec::new(),
        }
    }
}

impl PalModSettings {
    pub fn parse(text: &str) -> PalModSettings {
        let mut s = PalModSettings::default();
        // Files written before the header existed start straight in at the
        // keys, so the leading run counts as in-section.
        let mut in_section = true;
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('[') && line.ends_with(']') {
                in_section = line.eq_ignore_ascii_case(SECTION_HEADER);
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
                "bGlobalEnableMod" => s.enabled = value.eq_ignore_ascii_case("true"),
                "ActiveModList" if !value.is_empty() => s.active_mods.push(value.to_string()),
                "WorkshopRootDir" => s.workshop_root_dir = value.to_string(),
                "ConfigVersion" if !value.is_empty() => s.config_version = value.to_string(),
                _ => s.other_lines.push(line.to_string()),
            }
        }
        s
    }

    /// Raises the global enable flag on every call, because the server clears
    /// that flag when it rewrites the file. Do not implement disabling a mod by
    /// passing a reduced list: that would switch modding back on while the user is
    /// turning a mod off.
    pub fn with_active(mut self, active: &[String]) -> PalModSettings {
        self.enabled = true;
        self.active_mods = active.to_vec();
        self
    }

    /// Like `with_active`, but only for the packages the app manages: an entry
    /// not named in `managed` (a Steam-subscribed package, say) is kept in
    /// place, a managed entry survives only while it is in `active`, and every
    /// `active` package ends up listed exactly once. Names compare exactly as
    /// `ActiveModList` stores them.
    pub fn with_managed_active(mut self, active: &[String], managed: &[String]) -> PalModSettings {
        self.enabled = true;
        let mut kept: Vec<String> = Vec::with_capacity(self.active_mods.len() + active.len());
        for entry in self.active_mods {
            let is_active = active.contains(&entry);
            if is_active && kept.contains(&entry) {
                continue;
            }
            if is_active || !managed.contains(&entry) {
                kept.push(entry);
            }
        }
        for package in active {
            if !kept.contains(package) {
                kept.push(package.clone());
            }
        }
        self.active_mods = kept;
        self
    }

    pub fn render(&self) -> String {
        let mut lines = vec![
            SECTION_HEADER.to_string(),
            format!("ConfigVersion={}", self.config_version),
            format!(
                "bGlobalEnableMod={}",
                if self.enabled { "true" } else { "false" }
            ),
        ];
        if !self.workshop_root_dir.is_empty() {
            lines.push(format!("WorkshopRootDir={}", self.workshop_root_dir));
        }
        lines.extend(
            self.active_mods
                .iter()
                .map(|p| format!("ActiveModList={p}")),
        );
        lines.extend(self.other_lines.iter().cloned());
        lines.join("\n") + "\n"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parses_section_keys_and_keeps_unknown_lines() {
        let s = PalModSettings::parse(
            "[PalModSettings]\nConfigVersion=2.0\nbGlobalEnableMod=False\nWorkshopRootDir=C:/W\nActiveModList=A\nActiveModList=B\nDeleteModList=Z\nbNeedShowErrorOnNextStart=True\n[Other]\nIgnored=1\n",
        );
        assert!(!s.enabled);
        assert_eq!(s.active_mods, vec!["A".to_string(), "B".to_string()]);
        assert_eq!(s.workshop_root_dir, "C:/W");
        assert_eq!(s.config_version, "2.0");
        assert_eq!(
            s.other_lines,
            vec![
                "DeleteModList=Z".to_string(),
                "bNeedShowErrorOnNextStart=True".to_string()
            ]
        );
    }

    #[test]
    fn headerless_legacy_file_counts_as_in_section() {
        let s = PalModSettings::parse("bGlobalEnableMod=true\nActiveModList=A\n");
        assert!(s.enabled);
        assert_eq!(s.active_mods, vec!["A".to_string()]);
        assert_eq!(s.config_version, "1.0");
    }

    #[test]
    fn empty_text_gives_defaults() {
        let s = PalModSettings::parse("");
        assert!(!s.enabled && s.active_mods.is_empty() && s.workshop_root_dir.is_empty());
        assert_eq!(s.config_version, "1.0");
    }

    #[test]
    fn with_active_reraises_global_enable() {
        let s = PalModSettings::parse("bGlobalEnableMod=False\nActiveModList=Old\n")
            .with_active(&["New".into()]);
        assert!(s.enabled);
        assert_eq!(s.active_mods, vec!["New".to_string()]);
    }

    fn names(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn with_managed_active_keeps_an_unmanaged_entry() {
        let s = PalModSettings::parse("ActiveModList=SteamThing
ActiveModList=Mine
")
            .with_managed_active(&[], &names(&["Mine"]));
        assert_eq!(s.active_mods, names(&["SteamThing"]));
        assert!(s.enabled);
    }

    #[test]
    fn with_managed_active_removes_a_managed_disabled_entry() {
        let s = PalModSettings::parse("ActiveModList=Off
ActiveModList=SteamThing
")
            .with_managed_active(&names(&["On"]), &names(&["On", "Off"]));
        assert_eq!(s.active_mods, names(&["SteamThing", "On"]));
    }

    #[test]
    fn with_managed_active_adds_a_managed_enabled_entry_once() {
        let s = PalModSettings::parse("ActiveModList=On
ActiveModList=On
")
            .with_managed_active(&names(&["On", "New"]), &names(&["On", "New"]));
        assert_eq!(s.active_mods, names(&["On", "New"]));
        let again = PalModSettings::parse(&s.render())
            .with_managed_active(&names(&["On", "New"]), &names(&["On", "New"]));
        assert_eq!(again.active_mods, names(&["On", "New"]));
    }

    #[test]
    fn with_managed_active_preserves_the_order_of_kept_entries() {
        let s = PalModSettings::parse(
            "ActiveModList=Zeta
ActiveModList=Gone
ActiveModList=Alpha
ActiveModList=Mine
ActiveModList=Beta
",
        )
        .with_managed_active(&names(&["Mine", "Added"]), &names(&["Mine", "Gone", "Added"]));
        assert_eq!(s.active_mods, names(&["Zeta", "Alpha", "Mine", "Beta", "Added"]));
    }

    #[test]
    fn with_managed_active_compares_names_exactly() {
        let s = PalModSettings::parse("ActiveModList=coolpack
")
            .with_managed_active(&[], &names(&["CoolPack"]));
        assert_eq!(s.active_mods, names(&["coolpack"]));
    }

    #[test]
    fn render_matches_existing_writer_layout() {
        let s = PalModSettings {
            enabled: true,
            active_mods: vec!["A".into(), "B".into()],
            workshop_root_dir: "C:/W".into(),
            config_version: "2.0".into(),
            other_lines: vec!["DeleteModList=Z".into()],
        };
        assert_eq!(
            s.render(),
            "[PalModSettings]\nConfigVersion=2.0\nbGlobalEnableMod=true\nWorkshopRootDir=C:/W\nActiveModList=A\nActiveModList=B\nDeleteModList=Z\n"
        );
        let s = PalModSettings::parse("").with_active(&[]);
        assert_eq!(
            s.render(),
            "[PalModSettings]\nConfigVersion=1.0\nbGlobalEnableMod=true\n"
        );
    }

    #[test]
    fn parse_render_parse_is_stable() {
        let text = "[PalModSettings]\nConfigVersion=1.0\nbGlobalEnableMod=true\nActiveModList=A\nDeleteModList=Z\n";
        let once = PalModSettings::parse(text);
        assert_eq!(PalModSettings::parse(&once.render()), once);
    }
}
