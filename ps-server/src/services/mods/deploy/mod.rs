//! Making a target's filesystem match its active profile. The decisions are
//! `ps_core::mods::build_plan`'s; everything here is the part that touches disk.

pub mod apply;
pub mod backup;
pub mod desired;
pub mod markers;
pub mod recover;
pub mod write;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use ps_db::mod_targets::ModTarget;

use super::digest;

static APPLY_LOCKS: OnceLock<std::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>> =
    OnceLock::new();

/// The per-target apply lock, or `None` while another apply holds it. Keyed by
/// the library root as well as the target id: a process has one library per
/// database, so the key is the target within its database, and it stays the
/// same while a relocation moves the target's own root.
pub fn try_lock_target(
    paths: &super::LibraryPaths,
    target: &ModTarget,
) -> Option<tokio::sync::OwnedMutexGuard<()>> {
    lock_of(paths, target).try_lock_owned().ok()
}

/// Whether `guard` is the apply lock of this target.
pub fn holds_target(
    paths: &super::LibraryPaths,
    target: &ModTarget,
    guard: &tokio::sync::OwnedMutexGuard<()>,
) -> bool {
    Arc::ptr_eq(
        tokio::sync::OwnedMutexGuard::mutex(guard),
        &lock_of(paths, target),
    )
}

fn lock_of(paths: &super::LibraryPaths, target: &ModTarget) -> Arc<tokio::sync::Mutex<()>> {
    let key = format!(
        "{}\u{0}{}",
        ps_core::mods::normalize_physical_path(
            &paths.root().to_string_lossy(),
            cfg!(any(windows, target_os = "macos"))
        ),
        target.id
    );
    let lock = APPLY_LOCKS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .entry(key)
        .or_default()
        .clone();
    lock
}

/// The hash of the file at `path`, or `None` when nothing is there.
///
/// Everything that reads the target's disk goes through here, because the two
/// ways of getting this wrong both end in lost data: reading an existing file as
/// absent turns a recorded file into an unconditional `Remove` and lets a write
/// past the occupancy gate, so a metadata error is propagated rather than
/// guessed at, and a directory where a file belongs is named rather than
/// reported as an access denial from inside a rename.
pub(crate) fn hash_if_present(path: &Path) -> std::io::Result<Option<String>> {
    match std::fs::metadata(path) {
        Ok(meta) if meta.is_file() => Ok(Some(digest::hash_file(path)?)),
        Ok(_) => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{} is a directory, not a file", path.display()),
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Whether the game or server owning this target is running. A trait because the
/// real answer depends on a process list and a container daemon, and no deployer
/// test can depend on what happens to be running on the machine.
pub trait RunningCheck: Send + Sync {
    fn is_running(&self, target: &ModTarget) -> bool;
}

pub struct NeverRunning;

impl RunningCheck for NeverRunning {
    fn is_running(&self, _target: &ModTarget) -> bool {
        false
    }
}

/// Forcing the cross-volume branch is the only way to exercise it: two volumes
/// cannot be arranged inside a temp directory, and the branch carries the
/// copy-verify-delete logic that a same-volume rename never reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveStrategy {
    Auto,
    ForceCrossVolume,
}

pub struct ApplyOptions<'a> {
    pub running: &'a dyn RunningCheck,
    pub move_strategy: MoveStrategy,
    /// Overrides the backup set's human-ordering stamp. The `op_id` is what
    /// actually reserves a set, so this exists only so a test can hold the stamp
    /// still and prove the id is doing the work.
    pub stamp: Option<&'a str>,
    /// Unmanaged files the caller has agreed to move into this operation's
    /// backup set so the mod's own files can take their place.
    pub replace_occupants: &'a [std::path::PathBuf],
}

impl Default for ApplyOptions<'static> {
    fn default() -> Self {
        ApplyOptions {
            running: &NeverRunning,
            move_strategy: MoveStrategy::Auto,
            stamp: None,
            replace_occupants: &[],
        }
    }
}
