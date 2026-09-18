use std::path::Path;

use ps_server::services::mods::deploy::backup::{self, BackupSet};
use ps_server::services::mods::deploy::MoveStrategy;
use ps_server::services::mods::LibraryPaths;

fn write(path: &Path, body: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

#[test]
fn a_set_is_named_for_its_stamp_and_operation() {
    let dir = tempfile::tempdir().unwrap();
    let paths = LibraryPaths::new(dir.path());
    let set = backup::set_dir(
        &paths,
        "client-steam",
        "20260913-120000",
        "0123456789abcdef",
    );
    assert!(set.ends_with("20260913-120000-0123456789abcdef"), "{set:?}");
    assert!(
        set.starts_with(paths.backups_dir("client-steam")),
        "{set:?}"
    );
}

#[test]
fn two_operations_in_the_same_second_get_different_sets() {
    let dir = tempfile::tempdir().unwrap();
    let paths = LibraryPaths::new(dir.path());
    let a = backup::set_dir(&paths, "t", "20260913-120000", &backup::new_op_id());
    let b = backup::set_dir(&paths, "t", "20260913-120000", &backup::new_op_id());
    assert_ne!(a, b, "the operation id reserves the set, not the clock");
}

#[test]
fn an_operation_id_is_sixteen_hex_characters() {
    let id = backup::new_op_id();
    assert_eq!(id.len(), 16, "{id}");
    assert!(
        id.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
        "{id}"
    );
    assert_ne!(id, backup::new_op_id(), "and it is not a constant");
}

#[test]
fn taking_a_file_moves_it_under_a_key_and_leaves_the_original_gone() {
    let dir = tempfile::tempdir().unwrap();
    let paths = LibraryPaths::new(dir.path());
    let original = dir
        .path()
        .join("game/Pal/Binaries/Win64/ue4ss/Mods/CoolMod/config.lua");
    write(&original, b"the user's edit");

    let mut set = BackupSet::open(&paths, "client-steam", "20260913-120000", "abc").unwrap();
    let entry = set.take(&original, MoveStrategy::Auto).unwrap();

    assert_eq!(entry.original_path, original.to_string_lossy());
    assert_eq!(entry.backup_key, "0000/config.lua");
    assert!(!original.exists(), "take moves, it does not copy");
    assert_eq!(
        std::fs::read_to_string(set.dir().join("0000/config.lua")).unwrap(),
        "the user's edit"
    );
    assert_eq!(
        entry.hash,
        ps_server::services::mods::digest::hash_bytes(b"the user's edit")
    );
}

#[test]
fn two_files_with_one_name_do_not_collide() {
    let dir = tempfile::tempdir().unwrap();
    let paths = LibraryPaths::new(dir.path());
    let a = dir.path().join("game/ModA/config.lua");
    let b = dir.path().join("game/ModB/config.lua");
    write(&a, b"A");
    write(&b, b"B");

    let mut set = BackupSet::open(&paths, "t", "20260913-120000", "abc").unwrap();
    let first = set.take(&a, MoveStrategy::Auto).unwrap();
    let second = set.take(&b, MoveStrategy::Auto).unwrap();

    assert_ne!(first.backup_key, second.backup_key);
    assert_eq!(first.backup_key, "0000/config.lua");
    assert_eq!(second.backup_key, "0001/config.lua");
    assert_eq!(
        std::fs::read_to_string(set.dir().join(&first.backup_key)).unwrap(),
        "A"
    );
    assert_eq!(
        std::fs::read_to_string(set.dir().join(&second.backup_key)).unwrap(),
        "B"
    );
}

#[test]
fn an_absolute_original_path_cannot_escape_the_set() {
    let dir = tempfile::tempdir().unwrap();
    let paths = LibraryPaths::new(dir.path());
    let original = dir.path().join("game/deep/nested/thing.pak");
    write(&original, b"pak");

    let mut set = BackupSet::open(&paths, "t", "20260913-120000", "abc").unwrap();
    let entry = set.take(&original, MoveStrategy::Auto).unwrap();
    let stored = set.dir().join(&entry.backup_key);
    assert!(
        stored.starts_with(set.dir()),
        "a backup must land inside its set: {stored:?}"
    );
    assert!(stored.is_file());
}

#[test]
fn copy_in_leaves_the_original_where_it_is() {
    let dir = tempfile::tempdir().unwrap();
    let paths = LibraryPaths::new(dir.path());
    let marker = dir.path().join("game/ue4ss/Mods/mods.txt");
    write(&marker, b"CoolMod : 1\n");

    let mut set = BackupSet::open(&paths, "t", "20260913-120000", "abc").unwrap();
    let entry = set.copy_in(&marker).unwrap();
    assert!(
        marker.is_file(),
        "a marker snapshot must not move the marker"
    );
    assert_eq!(
        std::fs::read_to_string(set.dir().join(&entry.backup_key)).unwrap(),
        "CoolMod : 1\n"
    );
}

#[test]
fn a_cross_volume_take_leaves_the_bytes_intact_and_nothing_staged() {
    let dir = tempfile::tempdir().unwrap();
    let paths = LibraryPaths::new(dir.path());
    let original = dir.path().join("game/ModA/config.lua");
    write(&original, b"the user's edit");

    let mut set = BackupSet::open(&paths, "t", "20260913-120000", "abc").unwrap();
    let entry = set
        .take(&original, MoveStrategy::ForceCrossVolume)
        .unwrap();

    let stored = set.dir().join(&entry.backup_key);
    assert_eq!(
        std::fs::read_to_string(&stored).unwrap(),
        "the user's edit",
        "the one file the deployer promises never to destroy has to survive the \
         branch that copies instead of renaming"
    );
    assert!(!original.exists(), "and the original is only then removed");
    assert!(
        !ps_server::services::mods::deploy::write::staged_path(&stored).exists(),
        "the copy is staged and renamed, and the stage never survives"
    );
}

#[test]
fn the_index_names_a_backup_as_soon_as_it_is_taken() {
    let dir = tempfile::tempdir().unwrap();
    let paths = LibraryPaths::new(dir.path());
    let a = dir.path().join("game/ModA/config.lua");
    write(&a, b"A");

    let mut set = BackupSet::open(&paths, "t", "20260913-120000", "abc").unwrap();
    let entry = set.take(&a, MoveStrategy::Auto).unwrap();

    // No `write_index` call: between the move and the index the mapping from the
    // blob back to its original path would exist nowhere the restore can read.
    let read_back: Vec<backup::BackupEntry> = serde_json::from_str(
        &std::fs::read_to_string(set.dir().join("index.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(read_back, vec![entry]);
}

#[test]
fn the_index_lists_every_entry_and_can_be_rewritten() {
    let dir = tempfile::tempdir().unwrap();
    let paths = LibraryPaths::new(dir.path());
    let a = dir.path().join("game/ModA/config.lua");
    write(&a, b"A");

    let mut set = BackupSet::open(&paths, "t", "20260913-120000", "abc").unwrap();
    let entry = set.take(&a, MoveStrategy::Auto).unwrap();
    set.write_index().unwrap();

    let index = set.dir().join("index.json");
    let read_back: Vec<backup::BackupEntry> =
        serde_json::from_str(&std::fs::read_to_string(&index).unwrap()).unwrap();
    assert_eq!(read_back, vec![entry.clone()]);

    // Recovery rewrites the index from the journal, which is the authority.
    let from_journal = vec![
        entry.clone(),
        backup::BackupEntry {
            original_path: "C:/game/ModB/other.lua".to_string(),
            backup_key: "0001/other.lua".to_string(),
            hash: "deadbeef".to_string(),
        },
    ];
    BackupSet::rewrite_index_from(set.dir(), &from_journal).unwrap();
    let read_back: Vec<backup::BackupEntry> =
        serde_json::from_str(&std::fs::read_to_string(&index).unwrap()).unwrap();
    assert_eq!(read_back, from_journal);
}

#[test]
fn reopening_a_set_continues_its_ordinals() {
    let dir = tempfile::tempdir().unwrap();
    let paths = LibraryPaths::new(dir.path());
    let a = dir.path().join("game/ModA/config.lua");
    let b = dir.path().join("game/ModB/config.lua");
    write(&a, b"A");
    write(&b, b"B");

    let mut set = BackupSet::open(&paths, "t", "20260913-120000", "abc").unwrap();
    set.take(&a, MoveStrategy::Auto).unwrap();
    set.write_index().unwrap();
    drop(set);

    // A retry resuming a journal reuses the same op id and therefore the same set.
    let mut again = BackupSet::open(&paths, "t", "20260913-120000", "abc").unwrap();
    let second = again.take(&b, MoveStrategy::Auto).unwrap();
    assert_eq!(
        second.backup_key, "0001/config.lua",
        "a resumed set must not restart its ordinals and overwrite the first file"
    );
    assert_eq!(
        std::fs::read_to_string(again.dir().join("0000/config.lua")).unwrap(),
        "A"
    );
}
