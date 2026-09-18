/// UE4SS's own `mods.txt`, for a target whose file is missing.
pub const UE4SS_DEFAULT_MODS_TXT: &str = "CheatManagerEnablerMod : 0\r\nConsoleCommandsMod : 0\r\nConsoleEnablerMod : 0\r\nSplitScreenMod : 0\r\nLineTraceMod : 0\r\nBPML_GenericFunctions : 1\r\nBPModLoaderMod : 1\r\n\r\n\r\n\r\n\r\n; Built-in keybinds, do not move up!\r\nKeybinds : 1\r\n";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModsTxtLine {
    Entry { name: String, enabled: bool },
    Other(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModsTxt {
    pub bom: bool,
    pub lines: Vec<ModsTxtLine>,
}

const BOM: char = '\u{feff}';
const KEYBINDS_COMMENT: &str = "; Built-in keybinds";
const KEYBINDS_ENTRY: &str = "Keybinds";
const LOADER_ENTRY: &str = "BPModLoaderMod";

impl ModsTxt {
    pub fn parse(text: &str) -> ModsTxt {
        let bom = text.starts_with(BOM);
        let body = text.strip_prefix(BOM).unwrap_or(text);
        // `"".split('\n')` yields one empty piece, which would otherwise become a
        // blank line the file never had.
        if body.is_empty() {
            return ModsTxt {
                bom,
                lines: Vec::new(),
            };
        }
        let mut lines = Vec::new();
        for raw in body.split('\n') {
            let line = raw.strip_suffix('\r').unwrap_or(raw);
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with("//") {
                lines.push(ModsTxtLine::Other(line.to_string()));
                continue;
            }
            match trimmed.split_once(':') {
                Some((name, state)) => lines.push(ModsTxtLine::Entry {
                    name: name.trim().to_string(),
                    enabled: state.trim() != "0",
                }),
                None => lines.push(ModsTxtLine::Entry {
                    name: trimmed.to_string(),
                    enabled: true,
                }),
            }
        }
        // `split` yields a trailing empty piece when the text ends in a newline.
        if body.ends_with('\n') {
            lines.pop();
        }
        ModsTxt { bom, lines }
    }

    pub fn entries(&self) -> Vec<(String, bool)> {
        self.lines
            .iter()
            .filter_map(|l| match l {
                ModsTxtLine::Entry { name, enabled } => Some((name.clone(), *enabled)),
                ModsTxtLine::Other(_) => None,
            })
            .collect()
    }

    pub fn set_managed(&mut self, managed: &[(String, bool)], purge: &[String]) {
        let drop_name = |name: &str| {
            managed.iter().any(|(m, _)| m.eq_ignore_ascii_case(name))
                || purge.iter().any(|p| p.eq_ignore_ascii_case(name))
        };
        self.lines
            .retain(|l| !matches!(l, ModsTxtLine::Entry { name, .. } if drop_name(name)));

        let block: Vec<ModsTxtLine> = managed
            .iter()
            .map(|(name, enabled)| ModsTxtLine::Entry {
                name: name.clone(),
                enabled: *enabled,
            })
            .collect();

        let after_loader = self
            .lines
            .iter()
            .rposition(|l| matches!(l, ModsTxtLine::Entry { name, .. } if name.eq_ignore_ascii_case(LOADER_ENTRY)))
            .map(|i| i + 1);
        let before_comment = self.lines.iter().position(
            |l| matches!(l, ModsTxtLine::Other(s) if s.trim_start().starts_with(KEYBINDS_COMMENT)),
        );
        let before_keybinds = self
            .lines
            .iter()
            .position(|l| matches!(l, ModsTxtLine::Entry { name, .. } if name.eq_ignore_ascii_case(KEYBINDS_ENTRY)));
        let at = after_loader
            .or(before_comment)
            .or(before_keybinds)
            .unwrap_or(self.lines.len());
        self.lines.splice(at..at, block);
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        if self.bom {
            out.push(BOM);
        }
        for line in &self.lines {
            match line {
                ModsTxtLine::Entry { name, enabled } => {
                    out.push_str(name);
                    out.push_str(" : ");
                    out.push(if *enabled { '1' } else { '0' });
                }
                ModsTxtLine::Other(s) => out.push_str(s),
            }
            out.push_str("\r\n");
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    const SAMPLE: &str = "\u{feff}CheatManagerEnablerMod : 1\r\nActorDumperMod : 0\r\nBPModLoaderMod : 1\r\nBPML_GenericFunctions : 1\r\n\r\n; Built-in keybinds, do not move up!\r\nKeybinds : 1\r\n";

    #[test]
    fn parses_entries_comments_bom_and_bare_lines() {
        let m = ModsTxt::parse("\u{feff}A : 1\n; c\n// d\nBare\n\nB:0\n");
        assert!(m.bom);
        assert_eq!(
            m.entries(),
            vec![
                ("A".to_string(), true),
                ("Bare".to_string(), true),
                ("B".to_string(), false)
            ]
        );
        assert!(matches!(&m.lines[1], ModsTxtLine::Other(s) if s == "; c"));
        assert!(matches!(&m.lines[4], ModsTxtLine::Other(s) if s.is_empty()));
    }

    #[test]
    fn render_uses_crlf_trailing_newline_and_bom() {
        let m = ModsTxt::parse(SAMPLE);
        assert_eq!(m.render(), SAMPLE);
        let m = ModsTxt::parse("A : 1\nB : 0");
        assert_eq!(m.render(), "A : 1\r\nB : 0\r\n");
    }

    #[test]
    fn managed_block_goes_after_last_bpmodloader() {
        let mut m = ModsTxt::parse(SAMPLE);
        m.set_managed(&[("CoolMod".into(), true), ("OtherMod".into(), false)], &[]);
        let names: Vec<String> = m.entries().into_iter().map(|(n, _)| n).collect();
        assert_eq!(
            names,
            [
                "CheatManagerEnablerMod",
                "ActorDumperMod",
                "BPModLoaderMod",
                "CoolMod",
                "OtherMod",
                "BPML_GenericFunctions",
                "Keybinds"
            ]
        );
        assert!(m
            .render()
            .ends_with("; Built-in keybinds, do not move up!\r\nKeybinds : 1\r\n"));
    }

    #[test]
    fn managed_block_goes_before_keybinds_comment_without_bpmodloader() {
        let mut m = ModsTxt::parse("A : 1\n; Built-in keybinds, do not move up!\nKeybinds : 1\n");
        m.set_managed(&[("CoolMod".into(), true)], &[]);
        assert_eq!(
            m.render(),
            "A : 1\r\nCoolMod : 1\r\n; Built-in keybinds, do not move up!\r\nKeybinds : 1\r\n"
        );
    }

    #[test]
    fn managed_block_goes_before_bare_keybinds_entry_or_appends() {
        let mut m = ModsTxt::parse("A : 1\nKeybinds : 1\n");
        m.set_managed(&[("CoolMod".into(), true)], &[]);
        assert_eq!(m.render(), "A : 1\r\nCoolMod : 1\r\nKeybinds : 1\r\n");
        let mut m = ModsTxt::parse("A : 1\n");
        m.set_managed(&[("CoolMod".into(), false)], &[]);
        assert_eq!(m.render(), "A : 1\r\nCoolMod : 0\r\n");
        let mut m = ModsTxt::parse("");
        m.set_managed(&[("CoolMod".into(), true)], &[]);
        assert_eq!(m.render(), "CoolMod : 1\r\n");
    }

    #[test]
    fn set_managed_replaces_existing_lines_case_insensitively_and_purges() {
        let mut m = ModsTxt::parse("coolmod : 0\nStale : 1\nBPModLoaderMod : 1\nKeybinds : 1\n");
        m.set_managed(
            &[("CoolMod".into(), true)],
            &["Stale".into(), "Cool Mod Display".into()],
        );
        assert_eq!(
            m.render(),
            "BPModLoaderMod : 1\r\nCoolMod : 1\r\nKeybinds : 1\r\n"
        );
    }

    #[test]
    fn default_mods_txt_round_trips_and_seeds_after_bpmodloader() {
        let mut m = ModsTxt::parse(UE4SS_DEFAULT_MODS_TXT);
        assert_eq!(m.render(), UE4SS_DEFAULT_MODS_TXT);
        m.set_managed(&[("CoolMod".into(), true)], &[]);
        let names: Vec<String> = m.entries().into_iter().map(|(n, _)| n).collect();
        let loader = names.iter().position(|n| n == "BPModLoaderMod").unwrap();
        let cool = names.iter().position(|n| n == "CoolMod").unwrap();
        assert_eq!(cool, loader + 1);
    }

    #[test]
    fn rerunning_set_managed_is_idempotent() {
        let mut m = ModsTxt::parse(SAMPLE);
        m.set_managed(&[("CoolMod".into(), true)], &[]);
        let once = m.render();
        m.set_managed(&[("CoolMod".into(), true)], &[]);
        assert_eq!(m.render(), once);
    }
}
