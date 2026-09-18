/// Canonical string form of an **absolute** path, for use as a stable key.
///
/// Relative input is out of contract: a leading `..` has nothing to resolve
/// against and is dropped, so two different relative paths can collapse
/// together. Callers pass paths built by the layout resolver, which are absolute.
///
/// A leading `//` is preserved, because on Windows it introduces a UNC share. A
/// POSIX caller that joins paths by string concatenation could produce a doubled
/// leading slash meaning the same place as a single one; collapse that where the
/// platform is known, at the point a target root is taken in.
pub fn normalize_physical_path(path: &str, case_insensitive: bool) -> String {
    let unified = path.replace('\\', "/");
    let unc = unified.starts_with("//");
    let absolute = unified.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for seg in unified.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                // A drive letter or root is never popped.
                if parts.last().is_some_and(|p| !p.ends_with(':')) {
                    parts.pop();
                }
            }
            s => parts.push(s),
        }
    }
    let mut out = String::new();
    if unc {
        out.push_str("//");
    } else if absolute {
        out.push('/');
    }
    out.push_str(&parts.join("/"));
    if case_insensitive {
        out.to_lowercase()
    } else {
        out
    }
}

/// A host path in the host's own separator. Only a Windows host rewrites, since
/// a backslash is an ordinary filename character everywhere else.
pub fn native_separators(path: &str, windows_host: bool) -> String {
    if windows_host {
        path.replace('/', "\\")
    } else {
        path.to_string()
    }
}

/// A relative key naming one file inside a backup set. Never absolute, so
/// joining it onto the set's directory cannot discard that directory.
pub fn backup_key(ordinal: u32, original_path: &str) -> String {
    let unified = original_path.replace('\\', "/");
    let name = unified
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("");
    // `.` and `..` resolve away on join and would take the ordinal directory
    // with them, putting one set's files on top of another's.
    let name = match name {
        "" | "." | ".." => "file",
        other => other,
    };
    format!("{ordinal:04}/{name}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_separators_case_and_dots() {
        assert_eq!(
            normalize_physical_path("D:\\Games\\Palworld\\Pal\\..\\Pal\\Content\\", true),
            "d:/games/palworld/pal/content"
        );
        assert_eq!(
            normalize_physical_path("/srv//palstudio/./servers/x/mods", false),
            "/srv/palstudio/servers/x/mods"
        );
        assert_eq!(normalize_physical_path("C:/A/B", false), "C:/A/B");
        assert_eq!(
            normalize_physical_path("//server/share/dir/", true),
            "//server/share/dir"
        );
    }

    #[test]
    fn native_separators_backslash_only_on_a_windows_host() {
        assert_eq!(
            native_separators(r"D:\Programs\Steam\steamapps/common/Palworld", true),
            r"D:\Programs\Steam\steamapps\common\Palworld"
        );
        assert_eq!(
            native_separators("//server/share/dir", true),
            r"\\server\share\dir"
        );
        assert_eq!(
            native_separators(r"C:\Already\Native", true),
            r"C:\Already\Native"
        );
        assert_eq!(
            native_separators("/srv/palstudio/servers/a", false),
            "/srv/palstudio/servers/a"
        );
        assert_eq!(
            native_separators(r"/home/u/odd\name", false),
            r"/home/u/odd\name"
        );
    }

    #[test]
    fn dotdot_never_escapes_root() {
        assert_eq!(normalize_physical_path("/a/../../b", false), "/b");
        assert_eq!(normalize_physical_path("C:/../x", true), "c:/x");
    }

    #[test]
    fn backup_keys_are_relative_and_unique_by_ordinal() {
        assert_eq!(
            backup_key(7, "D:/CustomMods/Foo/config.lua"),
            "0007/config.lua"
        );
        assert_eq!(backup_key(8, "C:\\Other\\config.lua"), "0008/config.lua");
        assert_eq!(backup_key(0, "/x/y/"), "0000/y");
    }

    #[test]
    fn backup_keys_never_escape_their_ordinal_directory() {
        for path in ["/a/b/..", "/a/b/.", "/", ""] {
            assert_eq!(backup_key(3, path), "0003/file", "{path}");
        }
    }
}
