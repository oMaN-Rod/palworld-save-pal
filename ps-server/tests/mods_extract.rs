use std::io::Write;
use std::path::{Path, PathBuf};

use ps_server::services::mods::extract::{self, ArchiveFormat};

fn scratch() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

/// A zip with one text file and one nested file, written with the `zip` crate so
/// the test needs no fixture binary.
fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let file = std::fs::File::create(path).unwrap();
    let mut writer = zip::ZipWriter::new(file);
    let options: zip::write::FileOptions<'_, ()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, body) in entries {
        writer.start_file(*name, options).unwrap();
        writer.write_all(body).unwrap();
    }
    writer.finish().unwrap();
}

fn write_tar_gz(path: &Path, entries: &[(&str, &[u8])]) {
    let file = std::fs::File::create(path).unwrap();
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    for (name, body) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(body.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, name, *body).unwrap();
    }
    builder.into_inner().unwrap().finish().unwrap();
}

fn sorted_paths(entries: &[ps_core::mods::ArchiveEntry]) -> Vec<String> {
    let mut v: Vec<String> = entries.iter().map(|e| e.path.clone()).collect();
    v.sort();
    v
}

#[test]
fn a_zip_is_sniffed_listed_and_extracted() {
    let dir = scratch();
    let archive = dir.path().join("CoolMod-1.0.zip");
    write_zip(
        &archive,
        &[
            ("CoolMod/Scripts/main.lua", b"print('hi')"),
            ("CoolMod/enabled.txt", b""),
        ],
    );

    assert_eq!(extract::sniff_format(&archive).unwrap(), ArchiveFormat::Zip);

    let entries = extract::list_entries(&archive).unwrap();
    assert_eq!(
        sorted_paths(&entries),
        vec![
            "CoolMod/Scripts/main.lua".to_string(),
            "CoolMod/enabled.txt".to_string(),
        ]
    );
    let main = entries
        .iter()
        .find(|e| e.path.ends_with("main.lua"))
        .unwrap();
    assert_eq!(main.size, 11, "size is the uncompressed size");

    let out = extract::extract(&archive).unwrap();
    assert_eq!(sorted_paths(&out.entries), sorted_paths(&entries));
    assert_eq!(
        std::fs::read_to_string(out.path().join("CoolMod/Scripts/main.lua")).unwrap(),
        "print('hi')"
    );
}

#[test]
fn the_extension_is_not_the_format() {
    let dir = scratch();
    // A real 7z archive that claims to be a zip. Built by compressing with
    // sevenz_rust2 so the test needs no fixture binary.
    let source = dir.path().join("src");
    std::fs::create_dir_all(source.join("CoolMod")).unwrap();
    std::fs::write(source.join("CoolMod/main.lua"), b"print('7z')").unwrap();
    let archive = dir.path().join("CoolMod-1.0.zip");
    sevenz_rust2::compress_to_path(&source, &archive).unwrap();

    assert_eq!(
        extract::sniff_format(&archive).unwrap(),
        ArchiveFormat::SevenZ,
        "sniffed from magic bytes, not from .zip"
    );
    let entries = extract::list_entries(&archive).unwrap();
    assert!(
        entries.iter().any(|e| e.path.ends_with("main.lua")),
        "{:?}",
        sorted_paths(&entries)
    );
    let out = extract::extract(&archive).unwrap();
    let found: Vec<PathBuf> = walkdir::WalkDir::new(out.path())
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_path_buf())
        .collect();
    assert_eq!(found.len(), 1, "{found:?}");
}

#[test]
fn a_tar_gz_round_trips() {
    let dir = scratch();
    let archive = dir.path().join("mod.tar.gz");
    write_tar_gz(&archive, &[("CoolMod/main.lua", b"print('tar')")]);
    assert_eq!(
        extract::sniff_format(&archive).unwrap(),
        ArchiveFormat::TarGz
    );
    let out = extract::extract(&archive).unwrap();
    assert_eq!(
        std::fs::read_to_string(out.path().join("CoolMod/main.lua")).unwrap(),
        "print('tar')"
    );
}

#[test]
fn a_bare_pak_is_not_an_archive() {
    let dir = scratch();
    let pak = dir.path().join("CoolMod_P.pak");
    std::fs::write(&pak, b"\x00\x00\x00\x00not an archive").unwrap();
    assert_eq!(
        extract::sniff_format(&pak).unwrap(),
        ArchiveFormat::Unknown,
        "a loose pak is handled by the caller, not by the extractor"
    );
    assert!(matches!(
        extract::list_entries(&pak),
        Err(extract::ExtractError::UnsupportedFormat)
    ));
}

#[test]
fn trailing_garbage_after_the_central_directory_still_reads() {
    let dir = scratch();
    let archive = dir.path().join("CoolMod.zip");
    write_zip(&archive, &[("CoolMod/main.lua", b"print('hi')")]);
    let mut bytes = std::fs::read(&archive).unwrap();
    bytes.extend_from_slice(b"-- appended junk from a broken uploader --");
    std::fs::write(&archive, &bytes).unwrap();

    assert_eq!(extract::sniff_format(&archive).unwrap(), ArchiveFormat::Zip);
    let entries = extract::list_entries(&archive).unwrap();
    assert_eq!(entries.len(), 1, "{:?}", sorted_paths(&entries));
}

#[test]
fn entry_paths_are_normalised() {
    let dir = scratch();
    let archive = dir.path().join("CoolMod.zip");
    write_zip(
        &archive,
        &[
            ("./CoolMod/main.lua", b"a"),
            ("CoolMod/sub/", b""),
            ("CoolMod/sub/x.lua", b"b"),
        ],
    );
    let entries = extract::list_entries(&archive).unwrap();
    assert_eq!(
        sorted_paths(&entries),
        vec![
            "CoolMod/main.lua".to_string(),
            "CoolMod/sub/x.lua".to_string(),
        ],
        "the ./ prefix is stripped and the directory entry is dropped"
    );
}

#[test]
fn an_entry_that_would_escape_the_temp_dir_is_refused() {
    let dir = scratch();
    let archive = dir.path().join("evil.zip");
    // A safe entry rides along deliberately: an archive holding only the unsafe
    // entry extracts to nothing and returns `Empty`, so the assertions below
    // would never run and the test would pass for the wrong reason.
    write_zip(
        &archive,
        &[
            ("../../escaped.lua", b"pwned"),
            ("CoolMod/main.lua", b"print('safe')"),
        ],
    );

    let extracted = extract::extract(&archive).unwrap();
    let outside = dir.path().join("escaped.lua");
    assert!(
        !outside.exists(),
        "nothing may be written outside the temp dir"
    );
    assert!(
        !extracted
            .path()
            .parent()
            .unwrap()
            .join("escaped.lua")
            .exists(),
        "nor beside it"
    );
    assert_eq!(
        sorted_paths(&extracted.entries),
        vec!["CoolMod/main.lua".to_string()],
        "only the contained entry is extracted and reported"
    );
}

#[test]
fn an_empty_archive_is_an_error_not_an_empty_success() {
    let dir = scratch();
    let archive = dir.path().join("empty.zip");
    write_zip(&archive, &[]);
    assert!(matches!(
        extract::extract(&archive),
        Err(extract::ExtractError::Empty)
    ));
}

#[test]
fn the_payload_is_a_child_of_the_temp_dir_so_an_escape_is_observable() {
    let dir = scratch();
    let archive = dir.path().join("CoolMod.zip");
    write_zip(&archive, &[("CoolMod/main.lua", b"print('hi')")]);
    let out = extract::extract(&archive).unwrap();
    assert!(
        out.path().starts_with(out.enclosing_dir()),
        "the payload must sit under the temp dir"
    );
    assert_ne!(
        out.path(),
        out.enclosing_dir(),
        "and must not be the temp dir itself, or an escape would land outside it"
    );
}

#[test]
fn a_listing_never_claims_an_entry_extraction_would_skip() {
    let dir = scratch();
    let archive = dir.path().join("mixed.zip");
    write_zip(
        &archive,
        &[
            ("CoolMod/../evil.lua", b"pwned"),
            ("CoolMod/main.lua", b"print('safe')"),
        ],
    );
    let listed = sorted_paths(&extract::list_entries(&archive).unwrap());
    let extracted = sorted_paths(&extract::extract(&archive).unwrap().entries);
    assert_eq!(
        listed, extracted,
        "list_entries and extract must agree about what the archive holds"
    );
    // `CoolMod/../evil.lua` is not an escape: it resolves to `evil.lua`, which is
    // inside the destination, so both functions must report it under that resolved
    // name rather than either dropping it or quoting the archive's spelling.
    assert_eq!(
        listed,
        vec!["CoolMod/main.lua".to_string(), "evil.lua".to_string()]
    );
}
