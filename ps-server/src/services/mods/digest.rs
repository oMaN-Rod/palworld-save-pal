use std::io::Read;
use std::path::Path;

/// Streams the file: a pak is routinely hundreds of megabytes and the deployer
/// hashes every one on every apply.
pub fn hash_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0u8; 128 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

/// Total bytes of every file under `root`. A missing directory is zero rather
/// than an error, because the UI asks for the size of a library entry that may
/// have been deleted.
pub fn dir_size(root: &Path) -> std::io::Result<u64> {
    if !root.exists() {
        return Ok(0);
    }
    let mut total = 0u64;
    for entry in walkdir::WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(std::io::Error::other)?;
        if entry.file_type().is_file() {
            total += entry.metadata().map_err(std::io::Error::other)?.len();
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashing_a_file_matches_hashing_its_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.bin");
        let body = b"the quick brown fox";
        std::fs::write(&path, body).unwrap();
        assert_eq!(hash_file(&path).unwrap(), hash_bytes(body));
    }

    #[test]
    fn the_hash_is_lowercase_hex_of_the_expected_width() {
        let digest = hash_bytes(b"x");
        assert_eq!(digest.len(), 64);
        assert!(digest
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn different_bytes_hash_differently_and_equal_bytes_match() {
        assert_ne!(hash_bytes(b"a"), hash_bytes(b"b"));
        assert_eq!(hash_bytes(b"same"), hash_bytes(b"same"));
    }

    #[test]
    fn an_empty_file_hashes_rather_than_failing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty");
        std::fs::write(&path, b"").unwrap();
        assert_eq!(hash_file(&path).unwrap(), hash_bytes(b""));
    }

    #[test]
    fn a_file_larger_than_one_buffer_hashes_correctly() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.bin");
        let body: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(&path, &body).unwrap();
        assert_eq!(
            hash_file(&path).unwrap(),
            hash_bytes(&body),
            "streaming must agree with one-shot across buffer boundaries"
        );
    }

    #[test]
    fn dir_size_counts_files_recursively_and_ignores_directories() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("a/b")).unwrap();
        std::fs::write(dir.path().join("a/one"), b"12345").unwrap();
        std::fs::write(dir.path().join("a/b/two"), b"123").unwrap();
        assert_eq!(dir_size(dir.path()).unwrap(), 8);
    }

    #[test]
    fn dir_size_of_a_missing_directory_is_zero() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(dir_size(&dir.path().join("nope")).unwrap(), 0);
    }
}
