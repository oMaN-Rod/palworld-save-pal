use super::types::{ModType, TargetKind};

pub enum ModIdInput<'a> {
    Nexus {
        nexus_mod_id: u32,
        variant: Option<&'a str>,
    },
    Local {
        folder: &'a str,
        mod_type: ModType,
    },
    Framework {
        key: &'a str,
    },
}

/// Lowercase; a run of characters outside `[a-z0-9._-]` becomes one `_`, and
/// adds nothing where a separator already sits beside it; leading and trailing
/// `_` are trimmed. Separators written in the input are preserved as they are.
/// A name with no usable character at all slugs to `_` plus the hex codepoints
/// of the original rather than to the empty string, which every such name would
/// share. Trimming means an ordinary slug never starts with `_`, so the two
/// forms cannot collide; `u` never appears in a hex digit, so it separates the
/// codepoints unambiguously.
pub fn slugify(input: &str) -> String {
    let lowered: String = input.chars().flat_map(char::to_lowercase).collect();
    let mut out = String::with_capacity(lowered.len());
    let mut pending_sep = false;
    for ch in lowered.chars() {
        let separator = matches!(ch, '.' | '-' | '_');
        if ch.is_ascii_alphanumeric() || separator {
            let beside_separator = separator || matches!(out.chars().last(), Some('.' | '-' | '_'));
            if pending_sep && !out.is_empty() && !beside_separator {
                out.push('_');
            }
            pending_sep = false;
            out.push(ch);
        } else {
            pending_sep = true;
        }
    }
    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        let hex: String = lowered
            .chars()
            .map(|ch| format!("u{:x}", ch as u32))
            .collect();
        return format!("_{hex}");
    }
    trimmed.to_string()
}

fn type_slug(mod_type: ModType) -> &'static str {
    match mod_type {
        ModType::Ue4ss => "ue4ss",
        ModType::PalSchema => "palschema",
        ModType::Pak => "pak",
        ModType::LogicMods => "logicmods",
        ModType::NativeDll => "nativedll",
        ModType::Workshop => "workshop",
        ModType::Hybrid => "hybrid",
        ModType::Framework => "framework",
    }
}

pub fn mod_id(input: &ModIdInput) -> String {
    match input {
        ModIdInput::Nexus {
            nexus_mod_id,
            variant: None,
        } => format!("nexus-{nexus_mod_id}"),
        ModIdInput::Nexus {
            nexus_mod_id,
            variant: Some(variant),
        } => {
            format!("nexus-{nexus_mod_id}-{}", slugify(variant))
        }
        ModIdInput::Local { folder, mod_type } => {
            format!("{}-{}", slugify(folder), type_slug(*mod_type))
        }
        ModIdInput::Framework { key } => format!("framework-{}", slugify(key)),
    }
}

pub fn target_id(kind: TargetKind, discriminator: &str) -> String {
    let prefix = match kind {
        TargetKind::Client => "client",
        TargetKind::NativeServer | TargetKind::DockerServer => "server",
    };
    format!("{prefix}-{}", slugify(discriminator))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_lowercases_and_replaces_disallowed_chars() {
        assert_eq!(slugify("Cool Mod (Steam) v2!"), "cool_mod_steam_v2");
        assert_eq!(slugify("a:b/c\\d"), "a_b_c_d");
        assert_eq!(slugify("__lead__"), "lead");
        assert_eq!(
            slugify("keep.dots-and_underscores"),
            "keep.dots-and_underscores"
        );
    }

    #[test]
    fn nexus_ids_ignore_file_id_and_accept_variants() {
        assert_eq!(
            mod_id(&ModIdInput::Nexus {
                nexus_mod_id: 1827,
                variant: None
            }),
            "nexus-1827"
        );
        assert_eq!(
            mod_id(&ModIdInput::Nexus {
                nexus_mod_id: 1827,
                variant: Some("HD Textures")
            }),
            "nexus-1827-hd_textures"
        );
    }

    #[test]
    fn local_ids_combine_folder_and_type() {
        assert_eq!(
            mod_id(&ModIdInput::Local {
                folder: "CoolMod",
                mod_type: ModType::Ue4ss
            }),
            "coolmod-ue4ss"
        );
        assert_eq!(
            mod_id(&ModIdInput::Local {
                folder: "Big Pak_P",
                mod_type: ModType::LogicMods
            }),
            "big_pak_p-logicmods"
        );
    }

    #[test]
    fn framework_and_target_ids() {
        assert_eq!(
            mod_id(&ModIdInput::Framework { key: "ue4ss" }),
            "framework-ue4ss"
        );
        assert_eq!(
            target_id(TargetKind::Client, "Steam Main"),
            "client-steam_main"
        );
        assert_eq!(target_id(TargetKind::NativeServer, "7"), "server-7");
        assert_eq!(target_id(TargetKind::DockerServer, "7"), "server-7");
    }

    #[test]
    fn disallowed_runs_never_double_an_existing_separator() {
        assert_eq!(slugify("a!_b"), "a_b");
        assert_eq!(slugify("Cool_ Mod (v2)"), "cool_mod_v2");
        assert_eq!(slugify("dash-  case"), "dash-case");
        assert_eq!(slugify("trail! "), "trail");
    }

    #[test]
    fn names_with_no_usable_characters_stay_distinct() {
        assert_eq!(slugify("日本語モッド"), "_u65e5u672cu8a9eu30e2u30c3u30c9");
        assert_ne!(slugify("日本語モッド"), slugify("中文模组"));
        assert!(!slugify("😀").is_empty());
        // An ordinary slug is trimmed of leading `_`, so it can never be
        // mistaken for the fallback form.
        assert_ne!(slugify("u1f600"), slugify("😀"));
        assert!(!slugify("__lead__").starts_with('_'));
        assert_ne!(
            mod_id(&ModIdInput::Local {
                folder: "日本語モッド",
                mod_type: ModType::Pak
            }),
            mod_id(&ModIdInput::Local {
                folder: "中文模组",
                mod_type: ModType::Pak
            })
        );
    }

    #[test]
    fn ids_never_contain_colons_or_separators() {
        for id in [
            mod_id(&ModIdInput::Local {
                folder: "C:\\weird/name",
                mod_type: ModType::Pak,
            }),
            target_id(TargetKind::Client, "D:/Games"),
        ] {
            assert!(
                !id.contains(':') && !id.contains('/') && !id.contains('\\'),
                "{id}"
            );
        }
    }
}
