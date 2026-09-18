//! Archives a client sends as a sequence of chunks, staged under
//! `{app_dir}/downloads/.uploads` and moved into `{app_dir}/downloads` once
//! their size and hash check out.
use std::collections::HashMap;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use ps_app::emitter::{Emitter, WeakEmitter};
use ps_core::mods::normalize_physical_path;

pub const CHUNK_SIZE: usize = 1024 * 1024;
pub const MAX_UPLOAD_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub const IDLE_LIMIT: Duration = Duration::from_secs(600);
pub const SWEEP_EVERY: Duration = Duration::from_secs(30);
pub const MAX_PENDING_UPLOADS: usize = 8;
const MAX_NAME_BYTES: usize = 200;
pub const PROFILE_EXTENSION: &str = "psmods";
pub const EXTENSIONS: &[&str] = &[
    "zip",
    "7z",
    "rar",
    "tar",
    "gz",
    "tgz",
    "pak",
    "lua",
    "dll",
    PROFILE_EXTENSION,
];

#[derive(Debug, thiserror::Error)]
pub enum UploadError {
    #[error("that file name cannot be used")]
    InvalidName,
    #[error("that file type cannot be uploaded")]
    UnsupportedType,
    #[error("an upload needs a size")]
    InvalidSize,
    #[error("uploads are limited to {MAX_UPLOAD_BYTES} bytes")]
    TooLarge,
    #[error("sha256 must be 64 hex characters")]
    InvalidHash,
    #[error("at most {MAX_PENDING_UPLOADS} uploads may be in progress")]
    TooManyUploads,
    #[error("no upload {0}")]
    NotFound(String),
    #[error("expected chunk {expected}")]
    OutOfOrder { expected: u64 },
    #[error("a chunk may hold at most {CHUNK_SIZE} bytes")]
    ChunkTooLarge,
    #[error("more bytes than the upload declared")]
    SizeExceeded,
    #[error("received {received} of {expected} bytes")]
    SizeMismatch { expected: u64, received: u64 },
    #[error("the uploaded bytes do not match their hash")]
    HashMismatch,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl UploadError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidName => "invalid_name",
            Self::UnsupportedType => "unsupported_type",
            Self::InvalidSize => "invalid_size",
            Self::TooLarge => "upload_too_large",
            Self::InvalidHash => "invalid_hash",
            Self::TooManyUploads => "too_many_uploads",
            Self::NotFound(_) => "upload_not_found",
            Self::OutOfOrder { .. } => "chunk_out_of_order",
            Self::ChunkTooLarge => "chunk_too_large",
            Self::SizeExceeded => "size_exceeded",
            Self::SizeMismatch { .. } => "size_mismatch",
            Self::HashMismatch => "hash_mismatch",
            Self::Io(_) => "io",
        }
    }
}

struct Upload {
    name: String,
    size: u64,
    sha256: String,
    received: u64,
    next_seq: u64,
    hasher: Sha256,
    file: std::fs::File,
    last_activity: Instant,
    owner: WeakEmitter,
}

pub struct UploadStore {
    downloads: PathBuf,
    uploads: Mutex<HashMap<String, Upload>>,
    clock: Box<dyn Fn() -> Instant + Send + Sync>,
}

impl UploadStore {
    pub fn new(app_dir: &Path) -> Self {
        Self::with_clock(app_dir, Box::new(Instant::now))
    }

    pub fn with_clock(app_dir: &Path, clock: Box<dyn Fn() -> Instant + Send + Sync>) -> Self {
        let downloads = app_dir.join("downloads");
        let store = Self {
            downloads: std::path::absolute(&downloads).unwrap_or(downloads),
            uploads: Mutex::new(HashMap::new()),
            clock,
        };
        if let Err(error) = store.clear_pending() {
            tracing::warn!(%error, dir = %store.downloads.display(),
                "could not clear leftover partial uploads and imports");
        }
        store
    }

    pub fn downloads_dir(&self) -> &Path {
        &self.downloads
    }

    pub fn imports_dir(&self) -> PathBuf {
        self.downloads.join(".imports")
    }

    pub fn begin(
        &self,
        name: &str,
        size: u64,
        sha256: &str,
        owner: &Emitter,
    ) -> Result<(String, usize), UploadError> {
        if !valid_name(name) {
            return Err(UploadError::InvalidName);
        }
        let supported = Path::new(name)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str()));
        if !supported {
            return Err(UploadError::UnsupportedType);
        }
        if size == 0 {
            return Err(UploadError::InvalidSize);
        }
        if size > MAX_UPLOAD_BYTES {
            return Err(UploadError::TooLarge);
        }
        if sha256.len() != 64 || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(UploadError::InvalidHash);
        }
        let id = uuid::Uuid::new_v4().to_string();
        let mut uploads = self.lock();
        if uploads.len() >= MAX_PENDING_UPLOADS {
            return Err(UploadError::TooManyUploads);
        }
        std::fs::create_dir_all(self.pending_dir())?;
        let file = std::fs::File::create(self.pending_file(&id))?;
        let upload = Upload {
            name: name.to_string(),
            size,
            sha256: sha256.to_ascii_lowercase(),
            received: 0,
            next_seq: 0,
            hasher: Sha256::new(),
            file,
            last_activity: (self.clock)(),
            owner: owner.downgrade(),
        };
        uploads.insert(id.clone(), upload);
        Ok((id, CHUNK_SIZE))
    }

    /// Returns the bytes received so far. Overshooting the declared size or
    /// failing to write discards the upload, since a partial write leaves the
    /// file ahead of the hash.
    pub fn chunk(&self, id: &str, seq: u64, bytes: &[u8]) -> Result<u64, UploadError> {
        let mut uploads = self.lock();
        let upload = uploads
            .get_mut(id)
            .ok_or_else(|| UploadError::NotFound(id.to_string()))?;
        if seq != upload.next_seq {
            return Err(UploadError::OutOfOrder {
                expected: upload.next_seq,
            });
        }
        if bytes.len() > CHUNK_SIZE {
            return Err(UploadError::ChunkTooLarge);
        }
        let len = bytes.len() as u64;
        if upload.received + len > upload.size {
            uploads.remove(id);
            self.delete_pending(id);
            return Err(UploadError::SizeExceeded);
        }
        if let Err(error) = upload.file.write_all(bytes) {
            uploads.remove(id);
            self.delete_pending(id);
            return Err(error.into());
        }
        upload.hasher.update(bytes);
        upload.received += len;
        upload.next_seq += 1;
        upload.last_activity = (self.clock)();
        Ok(upload.received)
    }

    /// The lock is held through the rename so two uploads of one name cannot
    /// pick the same destination.
    pub fn end(&self, id: &str) -> Result<PathBuf, UploadError> {
        let mut uploads = self.lock();
        let Upload {
            name,
            size,
            sha256,
            received,
            hasher,
            mut file,
            ..
        } = uploads
            .remove(id)
            .ok_or_else(|| UploadError::NotFound(id.to_string()))?;
        let flushed = file.flush();
        drop(file);
        if let Err(error) = flushed {
            self.delete_pending(id);
            return Err(error.into());
        }
        if received != size {
            self.delete_pending(id);
            return Err(UploadError::SizeMismatch {
                expected: size,
                received,
            });
        }
        if format!("{:x}", hasher.finalize()) != sha256 {
            self.delete_pending(id);
            return Err(UploadError::HashMismatch);
        }
        let destination = free_destination(&self.downloads, &name);
        if let Err(error) = std::fs::rename(self.pending_file(id), &destination) {
            self.delete_pending(id);
            return Err(error.into());
        }
        Ok(destination)
    }

    /// Returns the ids of the uploads it removed.
    pub fn sweep(&self) -> Vec<String> {
        let now = (self.clock)();
        let mut uploads = self.lock();
        let expired: Vec<String> = uploads
            .iter()
            .filter(|(_, upload)| {
                !upload.owner.is_connected()
                    || now.saturating_duration_since(upload.last_activity) >= IDLE_LIMIT
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in &expired {
            uploads.remove(id);
            self.delete_pending(id);
        }
        expired
    }

    /// True only for an existing file directly inside the downloads directory.
    pub fn is_upload_path(&self, path: &str) -> bool {
        let candidate = Path::new(path);
        if !candidate.is_absolute()
            || candidate
                .components()
                .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
        {
            return false;
        }
        let Some(parent) = candidate.parent() else {
            return false;
        };
        let in_downloads = if cfg!(windows) {
            normalize_physical_path(&parent.to_string_lossy(), true)
                == normalize_physical_path(&self.downloads.to_string_lossy(), true)
        } else {
            parent == self.downloads.as_path()
        };
        in_downloads && candidate.is_file()
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, Upload>> {
        self.uploads.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn pending_dir(&self) -> PathBuf {
        self.downloads.join(".uploads")
    }

    fn pending_file(&self, id: &str) -> PathBuf {
        self.pending_dir().join(id)
    }

    fn clear_pending(&self) -> std::io::Result<()> {
        let pending = self.pending_dir();
        std::fs::create_dir_all(&pending)?;
        clear_dir(&pending)?;
        match clear_dir(&self.imports_dir()) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            result => result,
        }
    }

    fn delete_pending(&self, id: &str) {
        let path = self.pending_file(id);
        if let Err(error) = std::fs::remove_file(&path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(%error, path = %path.display(), "could not delete a partial upload");
            }
        }
    }
}

/// Ends by itself once the store is dropped with the server.
pub async fn sweep_while_alive(store: std::sync::Weak<UploadStore>) {
    let every = std::env::var("PS_UPLOAD_SWEEP_SECS")
        .ok()
        .and_then(|seconds| seconds.parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .map(Duration::from_secs)
        .unwrap_or(SWEEP_EVERY);
    let mut ticks = tokio::time::interval(every);
    loop {
        ticks.tick().await;
        let Some(store) = store.upgrade() else {
            return;
        };
        store.sweep();
    }
}

pub(crate) fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME_BYTES
        && name != ".."
        && !name.starts_with('.')
        && !name.contains(['/', '\\', ':'])
        && !name.chars().any(char::is_control)
}

fn clear_dir(dir: &Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            std::fs::remove_dir_all(&path)?;
        } else {
            std::fs::remove_file(&path)?;
        }
    }
    Ok(())
}

/// `stem (n).ext`, counting from 2, when `name` is already taken.
fn free_destination(dir: &Path, name: &str) -> PathBuf {
    let taken = |path: &Path| std::fs::symlink_metadata(path).is_ok();
    let first = dir.join(name);
    if !taken(&first) {
        return first;
    }
    let path = Path::new(name);
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let extension = path.extension().unwrap_or_default().to_string_lossy();
    let mut n = 2u64;
    loop {
        let candidate = dir.join(format!("{stem} ({n}).{extension}"));
        if !taken(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(dir: &std::path::Path) -> (UploadStore, std::sync::Arc<std::sync::atomic::AtomicU64>) {
        let seconds = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let start = std::time::Instant::now();
        let tick = seconds.clone();
        let store = UploadStore::with_clock(
            dir,
            Box::new(move || {
                start
                    + std::time::Duration::from_secs(tick.load(std::sync::atomic::Ordering::SeqCst))
            }),
        );
        (store, seconds)
    }

    fn sha256_hex(bytes: &[u8]) -> String {
        use sha2::Digest;
        format!("{:x}", sha2::Sha256::digest(bytes))
    }

    fn pending_count(dir: &Path) -> usize {
        std::fs::read_dir(dir.join("downloads").join(".uploads"))
            .unwrap()
            .count()
    }

    fn upload(store: &UploadStore, owner: &Emitter, name: &str, bytes: &[u8]) -> PathBuf {
        let (id, _) = store
            .begin(name, bytes.len() as u64, &sha256_hex(bytes), owner)
            .unwrap();
        store.chunk(&id, 0, bytes).unwrap();
        store.end(&id).unwrap()
    }

    #[test]
    fn an_upload_in_two_chunks_lands_in_downloads_with_its_name() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _) = store(dir.path());
        let (owner, _receiver) = Emitter::test_channel();

        let (id, chunk_size) = store
            .begin("Cool Mod.zip", 6, &sha256_hex(b"abcdef"), &owner)
            .unwrap();
        assert!(uuid::Uuid::parse_str(&id).is_ok(), "{id}");
        assert_eq!(chunk_size, CHUNK_SIZE);
        assert_eq!(store.chunk(&id, 0, b"abc").unwrap(), 3);
        assert_eq!(store.chunk(&id, 1, b"def").unwrap(), 6);

        let path = store.end(&id).unwrap();
        assert_eq!(path, dir.path().join("downloads").join("Cool Mod.zip"));
        assert_eq!(std::fs::read(&path).unwrap(), b"abcdef");
        assert_eq!(pending_count(dir.path()), 0);
    }

    #[test]
    fn a_taken_name_gets_a_numbered_copy() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _) = store(dir.path());
        let (owner, _receiver) = Emitter::test_channel();

        let first = upload(&store, &owner, "a.zip", b"one");
        let second = upload(&store, &owner, "a.zip", b"two");

        assert_eq!(first, dir.path().join("downloads").join("a.zip"));
        assert_eq!(second, dir.path().join("downloads").join("a (2).zip"));
        assert_eq!(std::fs::read(&first).unwrap(), b"one");
        assert_eq!(std::fs::read(&second).unwrap(), b"two");
    }

    #[test]
    fn names_that_could_escape_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _) = store(dir.path());
        let (owner, _receiver) = Emitter::test_channel();
        let hash = sha256_hex(b"x");

        for name in [
            "../a.zip",
            "a/b.zip",
            "a\\b.zip",
            ".hidden.zip",
            "",
            "a\u{0}.zip",
        ] {
            let result = store.begin(name, 1, &hash, &owner);
            assert!(
                matches!(result, Err(UploadError::InvalidName)),
                "{name:?}: {result:?}"
            );
        }
        let result = store.begin("a.exe", 1, &hash, &owner);
        assert!(
            matches!(result, Err(UploadError::UnsupportedType)),
            "{result:?}"
        );
        assert_eq!(pending_count(dir.path()), 0);
    }

    #[test]
    fn sizes_and_hashes_are_checked_up_front() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _) = store(dir.path());
        let (owner, _receiver) = Emitter::test_channel();
        let hash = sha256_hex(b"x");

        let result = store.begin("a.zip", 0, &hash, &owner);
        assert!(
            matches!(result, Err(UploadError::InvalidSize)),
            "{result:?}"
        );
        let result = store.begin("a.zip", MAX_UPLOAD_BYTES + 1, &hash, &owner);
        assert!(matches!(result, Err(UploadError::TooLarge)), "{result:?}");
        let result = store.begin("a.zip", 1, "xyz", &owner);
        assert!(
            matches!(result, Err(UploadError::InvalidHash)),
            "{result:?}"
        );
        assert_eq!(pending_count(dir.path()), 0);
    }

    #[test]
    fn a_ninth_pending_upload_is_refused_until_one_ends() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _) = store(dir.path());
        let (owner, _receiver) = Emitter::test_channel();
        let hash = sha256_hex(b"abc");
        let ids: Vec<String> = (0..MAX_PENDING_UPLOADS)
            .map(|n| {
                store
                    .begin(&format!("{n}.zip"), 3, &hash, &owner)
                    .unwrap()
                    .0
            })
            .collect();

        let result = store.begin("extra.zip", 3, &hash, &owner);
        assert!(
            matches!(result, Err(UploadError::TooManyUploads)),
            "{result:?}"
        );
        assert_eq!(result.unwrap_err().code(), "too_many_uploads");
        assert_eq!(pending_count(dir.path()), MAX_PENDING_UPLOADS);

        store.chunk(&ids[0], 0, b"abc").unwrap();
        store.end(&ids[0]).unwrap();
        assert!(store.begin("extra.zip", 3, &hash, &owner).is_ok());
        assert_eq!(pending_count(dir.path()), MAX_PENDING_UPLOADS);
    }

    #[test]
    fn chunks_must_arrive_in_order_and_fit() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _) = store(dir.path());
        let (owner, _receiver) = Emitter::test_channel();
        let size = CHUNK_SIZE as u64 + 10;
        let (id, _) = store
            .begin("a.zip", size, &sha256_hex(b"x"), &owner)
            .unwrap();

        let result = store.chunk(&id, 1, b"abc");
        assert!(
            matches!(result, Err(UploadError::OutOfOrder { expected: 0 })),
            "{result:?}"
        );
        let result = store.chunk(&id, 0, &vec![0u8; CHUNK_SIZE + 1]);
        assert!(
            matches!(result, Err(UploadError::ChunkTooLarge)),
            "{result:?}"
        );
        assert_eq!(
            store.chunk(&id, 0, &vec![0u8; CHUNK_SIZE]).unwrap(),
            CHUNK_SIZE as u64
        );

        let result = store.chunk(&id, 1, &[0u8; 11]);
        assert!(
            matches!(result, Err(UploadError::SizeExceeded)),
            "{result:?}"
        );
        assert_eq!(pending_count(dir.path()), 0);
        let result = store.chunk(&id, 1, b"a");
        assert!(
            matches!(result, Err(UploadError::NotFound(_))),
            "{result:?}"
        );
        assert_eq!(result.unwrap_err().code(), "upload_not_found");
    }

    #[test]
    fn a_failed_write_discards_the_upload() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _) = store(dir.path());
        let (owner, _receiver) = Emitter::test_channel();
        let (id, _) = store
            .begin("a.zip", 6, &sha256_hex(b"abcdef"), &owner)
            .unwrap();
        let read_only = std::fs::File::open(store.pending_file(&id)).unwrap();
        store.lock().get_mut(&id).unwrap().file = read_only;

        let result = store.chunk(&id, 0, b"abc");
        assert!(matches!(result, Err(UploadError::Io(_))), "{result:?}");
        assert_eq!(pending_count(dir.path()), 0);
        let result = store.chunk(&id, 0, b"abc");
        assert!(
            matches!(result, Err(UploadError::NotFound(_))),
            "{result:?}"
        );
    }

    #[test]
    fn a_relative_app_dir_still_yields_absolute_upload_paths() {
        let cwd = std::env::current_dir().unwrap();
        let dir = tempfile::Builder::new()
            .prefix(".uploads-relative-")
            .tempdir_in(&cwd)
            .unwrap();
        let relative = PathBuf::from(dir.path().file_name().unwrap());
        assert!(relative.is_relative());
        let (store, _) = store(&relative);
        let (owner, _receiver) = Emitter::test_channel();

        let path = upload(&store, &owner, "a.zip", b"abc");

        assert!(path.is_absolute(), "{path:?}");
        assert!(path.starts_with(dir.path()), "{path:?}");
        assert!(store.is_upload_path(&path.to_string_lossy()), "{path:?}");
    }

    #[test]
    fn a_short_or_corrupt_upload_is_deleted_at_end() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _) = store(dir.path());
        let (owner, _receiver) = Emitter::test_channel();

        let (short, _) = store
            .begin("a.zip", 6, &sha256_hex(b"abcdef"), &owner)
            .unwrap();
        store.chunk(&short, 0, b"abc").unwrap();
        let result = store.end(&short);
        assert!(
            matches!(
                result,
                Err(UploadError::SizeMismatch {
                    expected: 6,
                    received: 3
                })
            ),
            "{result:?}"
        );
        assert_eq!(pending_count(dir.path()), 0);

        let (corrupt, _) = store
            .begin("b.zip", 6, &sha256_hex(b"zzzzzz"), &owner)
            .unwrap();
        store.chunk(&corrupt, 0, b"abcdef").unwrap();
        let result = store.end(&corrupt);
        assert!(
            matches!(result, Err(UploadError::HashMismatch)),
            "{result:?}"
        );
        assert_eq!(pending_count(dir.path()), 0);
        let finished: Vec<_> = std::fs::read_dir(dir.path().join("downloads"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .filter(|name| name != ".uploads")
            .collect();
        assert!(finished.is_empty(), "{finished:?}");
    }

    #[test]
    fn the_sweep_drops_idle_uploads_and_disconnected_owners() {
        let dir = tempfile::tempdir().unwrap();
        let (store, seconds) = store(dir.path());
        let hash = sha256_hex(b"abcdef");

        let (gone_owner, gone_receiver) = Emitter::test_channel();
        let (gone, _) = store.begin("gone.zip", 6, &hash, &gone_owner).unwrap();
        drop(gone_owner);
        drop(gone_receiver);
        let (live_owner, _live_receiver) = Emitter::test_channel();
        let (live, _) = store.begin("live.zip", 6, &hash, &live_owner).unwrap();

        assert_eq!(store.sweep(), vec![gone.clone()]);
        assert_eq!(pending_count(dir.path()), 1);
        assert_eq!(store.chunk(&live, 0, b"abc").unwrap(), 3);

        seconds.store(601, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(store.sweep(), vec![live.clone()]);
        assert_eq!(pending_count(dir.path()), 0);
        assert!(matches!(store.end(&gone), Err(UploadError::NotFound(_))));
        assert!(matches!(store.end(&live), Err(UploadError::NotFound(_))));
    }

    #[test]
    fn leftover_partials_are_cleared_on_startup() {
        let dir = tempfile::tempdir().unwrap();
        let pending = dir.path().join("downloads").join(".uploads");
        std::fs::create_dir_all(&pending).unwrap();
        std::fs::write(pending.join("stale"), b"partial").unwrap();

        let _store = UploadStore::new(dir.path());

        assert!(!pending.join("stale").exists());
        assert_eq!(pending_count(dir.path()), 0);
    }

    #[test]
    fn leftover_imports_are_cleared_on_startup() {
        let dir = tempfile::tempdir().unwrap();
        let downloads = dir.path().join("downloads");
        let imports = downloads.join(".imports");
        let extracted = imports.join("0f2c7a52-stale").join("CoolMod");
        std::fs::create_dir_all(&extracted).unwrap();
        std::fs::write(extracted.join("main.lua"), b"x").unwrap();
        std::fs::write(imports.join("stray"), b"partial").unwrap();
        std::fs::write(downloads.join("keep.zip"), b"finished").unwrap();

        let store = UploadStore::new(dir.path());

        assert_eq!(store.imports_dir(), imports);
        assert!(!imports.join("0f2c7a52-stale").exists());
        assert!(!imports.join("stray").exists());
        assert!(downloads.join("keep.zip").is_file());
    }

    #[cfg(not(windows))]
    #[test]
    fn a_backslash_in_a_directory_name_is_not_a_separator() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("app");
        let (store, _) = store(&app);
        let lookalike = dir.path().join("app\\downloads");
        std::fs::create_dir_all(&lookalike).unwrap();
        let file = lookalike.join("a.zip");
        std::fs::write(&file, b"abc").unwrap();

        assert_eq!(
            normalize_physical_path(&lookalike.to_string_lossy(), false),
            normalize_physical_path(&store.downloads_dir().to_string_lossy(), false),
        );
        assert!(!store.is_upload_path(&file.to_string_lossy()), "{file:?}");
    }

    #[test]
    fn only_finished_uploads_directly_in_downloads_count_as_upload_paths() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _) = store(dir.path());
        let (owner, _receiver) = Emitter::test_channel();
        let finished = upload(&store, &owner, "a.zip", b"abc");
        let (pending_id, _) = store
            .begin("b.zip", 3, &sha256_hex(b"abc"), &owner)
            .unwrap();
        let downloads = dir.path().join("downloads");
        std::fs::create_dir_all(downloads.join("sub")).unwrap();
        std::fs::write(downloads.join("sub").join("a.zip"), b"abc").unwrap();
        std::fs::create_dir_all(dir.path().join("mods")).unwrap();
        std::fs::write(dir.path().join("mods").join("a.zip"), b"abc").unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("a.zip"), b"abc").unwrap();

        assert!(store.is_upload_path(&finished.to_string_lossy()));
        for path in [
            downloads.join(".uploads").join(&pending_id),
            downloads.join("sub").join("a.zip"),
            downloads.join("..").join("mods").join("a.zip"),
            PathBuf::from("a.zip"),
            downloads.join("missing.zip"),
            dir.path().join("mods").join("a.zip"),
            outside.path().join("a.zip"),
        ] {
            assert!(!store.is_upload_path(&path.to_string_lossy()), "{path:?}");
        }
        if cfg!(windows) {
            let flipped = finished.to_string_lossy().replace('\\', "/").to_uppercase();
            assert!(store.is_upload_path(&flipped), "{flipped}");
        }
    }
}
