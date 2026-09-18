use std::path::{Path, PathBuf};

use ps_core::mods::backup_key;

use super::super::digest;
use super::super::paths::LibraryPaths;
use super::write;
use super::MoveStrategy;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BackupEntry {
    pub original_path: String,
    pub backup_key: String,
    pub hash: String,
}

/// One apply's backup set. Reserved by the operation id rather than by the clock,
/// so two applies in the same second cannot share one, while a retry resuming a
/// journal reuses the journaled id and therefore continues the same set.
pub struct BackupSet {
    dir: PathBuf,
    entries: Vec<BackupEntry>,
    next_ordinal: u32,
}

pub fn set_dir(paths: &LibraryPaths, target_id: &str, stamp: &str, op_id: &str) -> PathBuf {
    paths
        .backups_dir(target_id)
        .join(format!("{stamp}-{op_id}"))
}

/// Eight bytes from the OS random number generator as sixteen lowercase hex
/// characters.
pub fn new_op_id() -> String {
    // rand 0.9's `OsRng` is a `TryRngCore`, not an `RngCore`; `fill_bytes` on it
    // does not satisfy its trait bounds. Verified against this workspace's lock.
    use rand::TryRngCore;
    let mut bytes = [0u8; 8];
    rand::rngs::OsRng
        .try_fill_bytes(&mut bytes)
        .expect("the OS random number generator is available");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn utc_stamp() -> String {
    chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string()
}

impl BackupSet {
    pub fn open(
        paths: &LibraryPaths,
        target_id: &str,
        stamp: &str,
        op_id: &str,
    ) -> std::io::Result<Self> {
        let dir = set_dir(paths, target_id, stamp, op_id);
        std::fs::create_dir_all(&dir)?;
        // A resumed set continues its ordinals; restarting them would write a
        // second file over the first one this set already holds.
        let entries: Vec<BackupEntry> = std::fs::read_to_string(dir.join("index.json"))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        let next_ordinal = entries.len() as u32;
        Ok(BackupSet {
            dir,
            entries,
            next_ordinal,
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn entries(&self) -> &[BackupEntry] {
        &self.entries
    }

    /// Moves the file into the set. Used for a `RemovePreserve`: the file is
    /// leaving the target, and the point of the set is that it is recoverable.
    pub fn take(
        &mut self,
        original: &Path,
        strategy: MoveStrategy,
    ) -> std::io::Result<BackupEntry> {
        let key = backup_key(self.next_ordinal, &original.to_string_lossy());
        self.place(original, key, Some(strategy))
    }

    /// Redoes a take the journal already named, under the key the journal
    /// recorded. A fresh ordinal would leave the journal's key pointing at a
    /// file that was never written.
    pub fn take_at(
        &mut self,
        original: &Path,
        key: &str,
        strategy: MoveStrategy,
    ) -> std::io::Result<BackupEntry> {
        self.place(original, key.to_string(), Some(strategy))
    }

    /// Copies the file into the set, leaving the original. Used for the snapshot
    /// of the two shared markers before the first apply on a target.
    pub fn copy_in(&mut self, original: &Path) -> std::io::Result<BackupEntry> {
        let key = backup_key(self.next_ordinal, &original.to_string_lossy());
        self.place(original, key, None)
    }

    /// Adds every entry the journal names that the set's own index has lost. The
    /// journal is the authoritative map after a crash, but a take redone during
    /// recovery exists only in the set, so the index has to carry both.
    pub fn adopt(&mut self, journalled: &[BackupEntry]) -> std::io::Result<()> {
        for entry in journalled {
            if !self
                .entries
                .iter()
                .any(|e| e.backup_key == entry.backup_key)
            {
                self.entries.push(entry.clone());
            }
        }
        self.next_ordinal = self.next_ordinal.max(self.entries.len() as u32);
        self.write_index()
    }

    /// The index is written for each file *before* that file moves, and once per
    /// file rather than once per set. A crash can then leave an entry naming a
    /// file still sitting at its original path, which loses nothing and is
    /// visible; the other order leaves a blob in the set that nothing maps back
    /// to, which is the one way this directory can hold unrecoverable data.
    fn place(
        &mut self,
        original: &Path,
        key: String,
        move_with: Option<MoveStrategy>,
    ) -> std::io::Result<BackupEntry> {
        let hash = digest::hash_file(original)?;
        let destination = self.dir.join(&key);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let entry = BackupEntry {
            original_path: original.to_string_lossy().into_owned(),
            backup_key: key,
            hash,
        };
        match self
            .entries
            .iter_mut()
            .find(|e| e.backup_key == entry.backup_key)
        {
            Some(existing) => *existing = entry.clone(),
            None => self.entries.push(entry.clone()),
        }
        self.next_ordinal = self.next_ordinal.max(self.entries.len() as u32);
        self.write_index()?;
        // The same copy-verify-delete discipline as any other move out of the
        // game directory: this is the one category of file the deployer promises
        // never to destroy, so an unflushed copy followed by a delete will not do.
        match move_with {
            Some(strategy) => {
                write::move_file(original, &destination, strategy)?;
            }
            None => write::copy_staged(original, &destination)?,
        }
        Ok(entry)
    }

    pub fn write_index(&self) -> std::io::Result<()> {
        Self::rewrite_index_from(&self.dir, &self.entries)
    }

    /// After a crash the journal is the authoritative map, so recovery rewrites
    /// the index from it rather than from whatever the set happens to hold.
    ///
    /// Staged like every other write the deployer makes. A plain `fs::write`
    /// truncates first, so a crash inside it would lose the mapping for every
    /// blob in the set — and this is now written once per file, which would be
    /// that many chances to take the whole index with it.
    pub fn rewrite_index_from(dir: &Path, entries: &[BackupEntry]) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let json = serde_json::to_string_pretty(entries).map_err(std::io::Error::other)?;
        write::write_staged(json.as_bytes(), &dir.join("index.json"))
    }
}
