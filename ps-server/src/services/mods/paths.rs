use std::path::{Path, PathBuf};

use ps_core::mods::RouteKind;

/// Every library path, rooted at `{app_root}/mods`. The app root is a
/// constructor argument rather than an environment read, so a test can point the
/// whole library at a temp dir.
#[derive(Debug, Clone)]
pub struct LibraryPaths {
    root: PathBuf,
}

impl LibraryPaths {
    pub fn new(app_root: &Path) -> Self {
        Self {
            root: app_root.join("mods"),
        }
    }

    pub fn from_env() -> Self {
        Self::new(&ps_core::paths::app_root())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn mod_dir(&self, mod_id: &str) -> PathBuf {
        self.root.join(mod_id)
    }

    pub fn version_dir(&self, mod_id: &str, version: &str) -> PathBuf {
        self.mod_dir(mod_id).join(sanitize_version(version))
    }

    pub fn archives_dir(&self, mod_id: &str) -> PathBuf {
        self.mod_dir(mod_id).join("archives")
    }

    /// `_backups` cannot collide with a mod id: `ps_core::mods::mod_id` trims
    /// leading underscores from every slug it produces.
    pub fn backups_dir(&self, target_id: &str) -> PathBuf {
        self.root.join("_backups").join(target_id)
    }

    /// The library copy of one routed file, under the version directory this mod
    /// version actually recorded. Always prefer this over `route_path` when a
    /// `mod_versions` row is in hand: the recorded directory is the truth, and a
    /// directory recomputed from the version string is only the same when no
    /// collision forced a different name.
    pub fn route_path_in(version_dir: &Path, kind: RouteKind, rel_path: &str) -> PathBuf {
        let mut path = version_dir.join(kind_segment(kind));
        for segment in rel_path.split('/').filter(|s| !s.is_empty()) {
            path.push(segment);
        }
        path
    }

    /// The path a routed file *would* take for a freshly chosen version directory.
    /// Correct only when the version's slug was free; `route_path_in` with the
    /// recorded `library_dir` is correct always.
    pub fn route_path(
        &self,
        mod_id: &str,
        version: &str,
        kind: RouteKind,
        rel_path: &str,
    ) -> PathBuf {
        Self::route_path_in(&self.version_dir(mod_id, version), kind, rel_path)
    }
}

pub fn kind_segment(kind: RouteKind) -> &'static str {
    match kind {
        RouteKind::Ue4ss => "ue4ss",
        RouteKind::PalSchema => "palschema",
        RouteKind::Pak => "pak",
        RouteKind::LogicMods => "logicmods",
        RouteKind::NativeDll => "nativedll",
        RouteKind::Workshop => "workshop",
        RouteKind::Companion => "companion",
        RouteKind::Passthrough => "passthrough",
        RouteKind::Framework => "framework",
        RouteKind::Binaries => "binaries",
        RouteKind::Ue4ssCore => "ue4sscore",
    }
}

/// A version string becomes a directory name. It comes from an archive name, so
/// it is slugged; a string with nothing usable in it becomes `unversioned`
/// rather than the empty string, which every such version would share.
pub fn sanitize_version(version: &str) -> String {
    let slug = ps_core::mods::slugify(version);
    // `slugify` keeps `.` as written, so ".", "..", "..." and any other run of
    // dots survive it. Each of those names the directory's own parent or itself
    // once the filesystem resolves it, which would point a version directory at
    // the mod directory and make a later delete take every other version with it.
    // `_` is what an empty input slugs to, and a leading `_u` is the
    // hex-codepoint form `slugify` falls back to when nothing is usable.
    if slug.is_empty() || slug == "_" || slug.starts_with("_u") || slug.chars().all(|c| c == '.') {
        return "unversioned".to_string();
    }
    let cleaned = slug.replace("..", "_");
    if ps_core::mods::is_forbidden_name(&cleaned) {
        "unversioned".to_string()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths() -> LibraryPaths {
        LibraryPaths::new(Path::new("C:/ps/data"))
    }

    #[test]
    fn the_library_sits_under_the_app_root() {
        let p = paths();
        assert_eq!(p.root(), Path::new("C:/ps/data/mods"));
        assert_eq!(
            p.mod_dir("coolmod-ue4ss"),
            Path::new("C:/ps/data/mods/coolmod-ue4ss")
        );
        assert_eq!(
            p.version_dir("coolmod-ue4ss", "1.0"),
            Path::new("C:/ps/data/mods/coolmod-ue4ss/1.0")
        );
        assert_eq!(
            p.archives_dir("coolmod-ue4ss"),
            Path::new("C:/ps/data/mods/coolmod-ue4ss/archives")
        );
        assert_eq!(
            p.backups_dir("client-steam"),
            Path::new("C:/ps/data/mods/_backups/client-steam")
        );
    }

    #[test]
    fn a_route_is_stored_under_its_kind() {
        let p = paths();
        assert_eq!(
            p.route_path("m", "1.0", RouteKind::Ue4ss, "CoolMod/Scripts/main.lua"),
            Path::new("C:/ps/data/mods/m/1.0/ue4ss/CoolMod/Scripts/main.lua")
        );
        assert_eq!(
            p.route_path("m", "1.0", RouteKind::Pak, "CoolMod_P.pak"),
            Path::new("C:/ps/data/mods/m/1.0/pak/CoolMod_P.pak")
        );
    }

    #[test]
    fn two_kinds_sharing_a_rel_path_do_not_collide() {
        let p = paths();
        assert_ne!(
            p.route_path("m", "1.0", RouteKind::Pak, "X_P.pak"),
            p.route_path("m", "1.0", RouteKind::Passthrough, "X_P.pak"),
            "the kind segment is what keeps these apart"
        );
    }

    #[test]
    fn every_route_kind_has_a_distinct_segment() {
        let kinds = [
            RouteKind::Ue4ss,
            RouteKind::PalSchema,
            RouteKind::Pak,
            RouteKind::LogicMods,
            RouteKind::NativeDll,
            RouteKind::Workshop,
            RouteKind::Companion,
            RouteKind::Passthrough,
            RouteKind::Framework,
            RouteKind::Binaries,
            RouteKind::Ue4ssCore,
        ];
        let segments: Vec<&str> = kinds.iter().copied().map(kind_segment).collect();
        let mut sorted = segments.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), kinds.len(), "{segments:?}");
        assert!(
            segments.iter().all(|s| !s.is_empty()
                && s.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')),
            "{segments:?}"
        );
    }

    #[test]
    fn a_version_that_cannot_be_a_directory_name_is_made_into_one() {
        assert_eq!(sanitize_version("1.0"), "1.0");
        assert_eq!(sanitize_version("v2.1-beta"), "v2.1-beta");
        assert_eq!(sanitize_version("1.0 (Steam)"), "1.0_steam");
        assert_eq!(sanitize_version(""), "unversioned");
        assert_eq!(sanitize_version("   "), "unversioned");
        assert!(!sanitize_version("../../etc").contains(".."));
        assert!(!sanitize_version("a/b").contains('/'));
        assert!(!sanitize_version("C:\\x").contains(':'));
    }
}

#[cfg(test)]
mod dot_version_tests {
    use super::*;

    #[test]
    fn a_version_made_only_of_dots_never_names_a_directory() {
        for version in [".", "..", "...", "....", "./.", ". . ."] {
            let sanitized = sanitize_version(version);
            assert!(
                !sanitized.chars().all(|c| c == '.'),
                "{version:?} sanitized to {sanitized:?}, which resolves to a parent or to itself"
            );
            assert!(!sanitized.contains(".."), "{version:?} -> {sanitized:?}");
            let dir = LibraryPaths::new(Path::new("C:/ps/data")).version_dir("m", version);
            assert_ne!(
                dir,
                Path::new("C:/ps/data/mods/m"),
                "{version:?} pointed the version directory at the mod directory"
            );
            assert_ne!(
                dir,
                Path::new("C:/ps/data/mods"),
                "{version:?} escaped further still"
            );
        }
    }
}
