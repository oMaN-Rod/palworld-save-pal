use std::io::Write;
use std::path::{Component, Path, PathBuf};

use super::super::digest;
use super::MoveStrategy;

pub const STAGE_SUFFIX: &str = ".psnew";
pub const NEW_COPY_SUFFIX: &str = ".new";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveOutcome {
    SameVolume,
    CrossVolume,
}

fn with_suffix(dest: &Path, suffix: &str) -> PathBuf {
    let mut name = dest.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    dest.with_file_name(name)
}

/// The staging path sits beside the destination, in the same directory, because a
/// rename is only atomic within one filesystem.
pub fn staged_path(dest: &Path) -> PathBuf {
    with_suffix(dest, STAGE_SUFFIX)
}

/// Where the new version of a preserved file is written, beside the user's edit.
pub fn new_copy_path(dest: &Path) -> PathBuf {
    with_suffix(dest, NEW_COPY_SUFFIX)
}

fn parent_of(path: &Path) -> std::io::Result<&Path> {
    path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{} has no parent directory", path.display()),
        )
    })
}

/// Writes through a staging file and renames it over the destination, so a reader
/// sees the whole old file or the whole new one and never a partial write. The
/// stage file is removed on every path out, including failure: a leftover is what
/// recovery looks for, so one left by a successful call would be misread.
fn finish_stage(staged: &Path, dest: &Path) -> std::io::Result<()> {
    match std::fs::rename(staged, dest) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = std::fs::remove_file(staged);
            Err(error)
        }
    }
}

pub fn copy_staged(source: &Path, dest: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(parent_of(dest)?)?;
    let staged = staged_path(dest);
    let outcome = (|| -> std::io::Result<()> {
        std::fs::copy(source, &staged)?;
        std::fs::OpenOptions::new()
            .write(true)
            .open(&staged)?
            .sync_all()
    })();
    if let Err(error) = outcome {
        let _ = std::fs::remove_file(&staged);
        return Err(error);
    }
    finish_stage(&staged, dest)
}

pub fn write_staged(bytes: &[u8], dest: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(parent_of(dest)?)?;
    let staged = staged_path(dest);
    let outcome = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::create(&staged)?;
        file.write_all(bytes)?;
        file.sync_all()
    })();
    if let Err(error) = outcome {
        let _ = std::fs::remove_file(&staged);
        return Err(error);
    }
    finish_stage(&staged, dest)
}

/// Carries the file as it is on disk, user edits included. Across volumes the
/// source is deleted only after the copy's hash matches the source's *current*
/// hash, so a file that changed underneath is caught before anything is lost.
pub fn move_file(from: &Path, to: &Path, strategy: MoveStrategy) -> std::io::Result<MoveOutcome> {
    if from == to {
        return Ok(MoveOutcome::SameVolume);
    }
    std::fs::create_dir_all(parent_of(to)?)?;
    if strategy == MoveStrategy::Auto {
        // Any failure falls through to copy-verify-delete, which also covers
        // the cross-device case this is really testing for.
        if std::fs::rename(from, to).is_ok() {
            return Ok(MoveOutcome::SameVolume);
        }
    }
    let staged = staged_path(to);
    let outcome = (|| -> std::io::Result<()> {
        std::fs::copy(from, &staged)?;
        std::fs::OpenOptions::new()
            .write(true)
            .open(&staged)?
            .sync_all()?;
        let copied = digest::hash_file(&staged)?;
        let current = digest::hash_file(from)?;
        if copied != current {
            return Err(std::io::Error::other(format!(
                "{} changed while it was being moved",
                from.display()
            )));
        }
        Ok(())
    })();
    if let Err(error) = outcome {
        let _ = std::fs::remove_file(&staged);
        return Err(error);
    }
    finish_stage(&staged, to)?;
    std::fs::remove_file(from)?;
    Ok(MoveOutcome::CrossVolume)
}

/// `true` when a file was there and is now gone. A path that is already absent is
/// success, because apply has to converge when it is re-run.
pub fn remove_if_present(path: &Path) -> std::io::Result<bool> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

pub fn clear_stage(dest: &Path) -> std::io::Result<bool> {
    remove_if_present(&staged_path(dest))
}

/// Removes each candidate directory and then its ancestors while they are
/// empty and strictly inside one of `bases`, never a base itself. Removal is
/// non-recursive, so a directory still holding anything stops the climb.
pub fn prune_empty_dirs(candidates: &[&Path], bases: &[&Path]) {
    let key = |path: &Path| PathBuf::from(super::apply::path_key(&path.to_string_lossy()));
    // `Path::starts_with("")` is true for every path, so an empty base would
    // contain everything.
    let base_keys: Vec<PathBuf> = bases
        .iter()
        .map(|base| key(base))
        .filter(|base| !base.as_os_str().is_empty())
        .collect();
    let prunable = |dir: &Path| {
        if dir
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        {
            return false;
        }
        let dir = key(dir);
        !base_keys.contains(&dir) && base_keys.iter().any(|base| dir.starts_with(base))
    };

    let mut seen = std::collections::HashSet::new();
    let mut dirs: Vec<&Path> = candidates
        .iter()
        .copied()
        .filter(|dir| seen.insert(key(dir)))
        .collect();
    dirs.sort_by_key(|dir| std::cmp::Reverse(dir.components().count()));
    for dir in dirs {
        let mut current = Some(dir);
        while let Some(dir) = current.filter(|dir| prunable(dir)) {
            match std::fs::remove_dir(dir) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    tracing::debug!(%error, dir = %dir.display(), "left a deployment directory in place");
                    break;
                }
            }
            current = dir.parent();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emptied_directories_are_removed_up_to_the_base() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("Mods");
        let scripts = base.join("CoolMod").join("Scripts");
        std::fs::create_dir_all(&scripts).unwrap();

        prune_empty_dirs(&[scripts.as_path()], &[base.as_path()]);

        assert!(!base.join("CoolMod").exists());
        assert!(base.is_dir(), "the base itself survives");
    }

    #[test]
    fn a_directory_holding_a_file_stops_the_climb() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("Mods");
        let scripts = base.join("CoolMod").join("Scripts");
        std::fs::create_dir_all(&scripts).unwrap();
        std::fs::write(base.join("CoolMod").join("notes.txt"), b"mine").unwrap();

        prune_empty_dirs(&[scripts.as_path()], &[base.as_path()]);

        assert!(!scripts.exists());
        assert!(base.join("CoolMod").join("notes.txt").is_file());
    }

    #[test]
    fn a_nested_base_is_never_removed() {
        let dir = tempfile::tempdir().unwrap();
        let outer = dir.path().join("Mods");
        let inner = outer.join("PalSchema").join("mods");
        std::fs::create_dir_all(inner.join("A")).unwrap();

        let a = inner.join("A");
        prune_empty_dirs(&[a.as_path()], &[outer.as_path(), inner.as_path()]);

        assert!(!inner.join("A").exists());
        assert!(inner.is_dir());
    }

    #[test]
    fn directories_outside_every_base_are_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("Mods");
        let outside = dir.path().join("Other").join("Empty");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let dotted = base.join("..").join("Other").join("Empty");

        prune_empty_dirs(
            &[outside.as_path(), dotted.as_path(), base.as_path()],
            &[base.as_path()],
        );

        assert!(outside.is_dir());
        assert!(base.is_dir());
    }

    #[test]
    fn an_empty_base_contains_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("Mods");
        let inside = base.join("CoolMod");
        let outside = dir.path().join("Other").join("Empty");
        std::fs::create_dir_all(&inside).unwrap();
        std::fs::create_dir_all(&outside).unwrap();

        prune_empty_dirs(
            &[outside.as_path(), inside.as_path()],
            &[Path::new(""), base.as_path()],
        );

        assert!(outside.is_dir());
        assert!(!inside.exists());
        assert!(base.is_dir());
    }

    #[test]
    fn an_already_missing_directory_keeps_the_climb_going() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("Mods");
        std::fs::create_dir_all(base.join("CoolMod")).unwrap();
        let gone = base.join("CoolMod").join("Scripts");

        prune_empty_dirs(&[gone.as_path()], &[base.as_path()]);

        assert!(!base.join("CoolMod").exists());
        assert!(base.is_dir());
    }
}
