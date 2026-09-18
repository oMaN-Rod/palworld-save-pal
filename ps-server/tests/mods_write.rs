use std::path::Path;

use ps_server::services::mods::deploy::write::{self, MoveOutcome};
use ps_server::services::mods::deploy::MoveStrategy;

fn scratch() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn put(path: &Path, body: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

#[test]
fn a_staged_copy_lands_at_the_destination_and_leaves_no_stage_file() {
    let dir = scratch();
    let source = dir.path().join("library/main.lua");
    let dest = dir.path().join("game/Mods/CoolMod/main.lua");
    put(&source, b"print('hi')");

    write::copy_staged(&source, &dest).unwrap();

    assert_eq!(std::fs::read_to_string(&dest).unwrap(), "print('hi')");
    assert!(
        !write::staged_path(&dest).exists(),
        "a successful write leaves no .psnew behind, or recovery cannot tell them apart"
    );
    assert!(source.is_file(), "the library copy is not consumed");
}

#[test]
fn a_staged_copy_over_an_existing_file_replaces_it_whole() {
    let dir = scratch();
    let source = dir.path().join("library/main.lua");
    let dest = dir.path().join("game/main.lua");
    put(&source, b"new");
    put(&dest, b"the much longer old contents");

    write::copy_staged(&source, &dest).unwrap();
    assert_eq!(
        std::fs::read_to_string(&dest).unwrap(),
        "new",
        "no trailing bytes of the old file survive"
    );
}

#[test]
fn writing_bytes_creates_missing_directories() {
    let dir = scratch();
    let dest = dir.path().join("game/deeply/nested/mods.txt");
    write::write_staged(b"CoolMod : 1\n", &dest).unwrap();
    assert_eq!(std::fs::read_to_string(&dest).unwrap(), "CoolMod : 1\n");
    assert!(!write::staged_path(&dest).exists());
}

#[test]
fn the_stage_path_is_the_destination_plus_a_suffix_in_the_same_directory() {
    let staged = write::staged_path(Path::new("C:/game/Mods/CoolMod/main.lua"));
    assert_eq!(
        staged.parent(),
        Path::new("C:/game/Mods/CoolMod/main.lua").parent()
    );
    assert!(
        staged.to_string_lossy().ends_with(write::STAGE_SUFFIX),
        "{staged:?}"
    );
}

#[test]
fn a_same_volume_move_renames_and_carries_the_bytes() {
    let dir = scratch();
    let from = dir.path().join("game/old/CoolMod/config.lua");
    let to = dir.path().join("game/new/001_CoolMod/config.lua");
    put(&from, b"the user's edited config");

    let outcome = write::move_file(&from, &to, MoveStrategy::Auto).unwrap();
    assert_eq!(outcome, MoveOutcome::SameVolume);
    assert!(!from.exists());
    assert_eq!(
        std::fs::read_to_string(&to).unwrap(),
        "the user's edited config",
        "a move carries the file as it is on disk, edits included"
    );
}

#[test]
fn a_forced_cross_volume_move_copies_verifies_and_deletes() {
    let dir = scratch();
    let from = dir.path().join("game/old/CoolMod/config.lua");
    let to = dir.path().join("game/new/CoolMod/config.lua");
    put(&from, b"the user's edited config");

    let outcome = write::move_file(&from, &to, MoveStrategy::ForceCrossVolume).unwrap();
    assert_eq!(
        outcome,
        MoveOutcome::CrossVolume,
        "the strategy must actually take the other branch, or the branch is untested"
    );
    assert!(
        !from.exists(),
        "the source is deleted only after the copy verifies"
    );
    assert_eq!(
        std::fs::read_to_string(&to).unwrap(),
        "the user's edited config"
    );
    assert!(!write::staged_path(&to).exists());
}

#[test]
fn a_move_onto_itself_is_a_no_op_rather_than_a_deletion() {
    let dir = scratch();
    let path = dir.path().join("game/CoolMod/config.lua");
    put(&path, b"contents");
    write::move_file(&path, &path, MoveStrategy::Auto).unwrap();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "contents",
        "copy-verify-delete onto the same path must not delete the file it just wrote"
    );
    write::move_file(&path, &path, MoveStrategy::ForceCrossVolume).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "contents");
}

#[test]
fn removing_a_file_that_is_already_gone_succeeds() {
    let dir = scratch();
    let path = dir.path().join("game/CoolMod/main.lua");
    put(&path, b"x");
    assert!(write::remove_if_present(&path).unwrap());
    assert!(
        !write::remove_if_present(&path).unwrap(),
        "and says it removed nothing the second time"
    );
}

#[test]
fn clearing_a_stage_file_reports_whether_there_was_one() {
    let dir = scratch();
    let dest = dir.path().join("game/main.lua");
    assert!(!write::clear_stage(&dest).unwrap());
    put(&write::staged_path(&dest), b"half written");
    assert!(write::clear_stage(&dest).unwrap());
    assert!(!write::staged_path(&dest).exists());
}
