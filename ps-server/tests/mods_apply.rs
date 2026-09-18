use std::path::Path;

use ps_core::mods::PlanEntry;
use ps_server::services::mods::deploy::{apply, ApplyOptions, MoveStrategy, RunningCheck};
use ps_server::services::mods::LibraryPaths;

mod common;
use common::mods::*;

struct AlwaysRunning;
impl RunningCheck for AlwaysRunning {
    fn is_running(&self, _t: &ps_db::mod_targets::ModTarget) -> bool {
        true
    }
}

fn ue4ss_dir(root: &Path) -> std::path::PathBuf {
    root.join("Pal/Binaries/Win64/ue4ss/Mods")
}

#[tokio::test]
async fn applying_an_enabled_mod_writes_its_files_and_records_them() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[("ue4ss", "CoolMod/Scripts/main.lua", b"print('hi')")],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;

    let outcome = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(!outcome.mid_apply, "{:?}", outcome.failed);

    let deployed = ue4ss_dir(&root).join("CoolMod/Scripts/main.lua");
    assert_eq!(std::fs::read_to_string(&deployed).unwrap(), "print('hi')");

    let rows = ps_db::mod_deployments::files_of(&db, &target.id)
        .await
        .unwrap();
    assert!(
        rows.iter()
            .any(|r| Path::new(&r.path) == deployed.as_path()),
        "{rows:?}"
    );
    assert!(
        ps_db::mod_deployments::journal_of(&db, &target.id)
            .await
            .unwrap()
            .is_none(),
        "a clean apply closes its journal"
    );
    // The marker was written too.
    assert!(ue4ss_dir(&root).join("mods.txt").is_file());
}

#[tokio::test]
async fn an_apply_already_running_on_the_target_refuses_a_second_until_it_finishes() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[("ue4ss", "CoolMod/Scripts/main.lua", b"print('hi')")],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;
    let deployed = ue4ss_dir(&root).join("CoolMod/Scripts/main.lua");

    let held = ps_server::services::mods::deploy::try_lock_target(&paths, &target)
        .expect("nothing else holds this target's lock");
    let refused = apply::apply(&db, &paths, &target, &ApplyOptions::default()).await;
    assert!(
        matches!(&refused, Err(apply::ApplyError::ApplyInProgress(id)) if id == &target.id),
        "{:?}",
        refused.as_ref().err().map(ToString::to_string)
    );
    assert!(!deployed.exists(), "a refused apply writes nothing");

    drop(held);
    let outcome = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(!outcome.mid_apply, "{:?}", outcome.failed);
    assert!(deployed.is_file());
    assert!(
        ps_server::services::mods::deploy::try_lock_target(&paths, &target).is_some(),
        "the lock is released when the apply returns"
    );
}

fn lock_target(id: &str, root: &Path) -> ps_db::mod_targets::ModTarget {
    ps_db::mod_targets::ModTarget {
        id: id.to_string(),
        kind: "server".to_string(),
        server_id: None,
        name: "Server".to_string(),
        root_path: root.to_string_lossy().into_owned(),
        platform: "win64".to_string(),
        ue4ss_mode: "none".to_string(),
        layout_overrides: "{}".to_string(),
        detected: "{}".to_string(),
        last_scanned_at: None,
        created_at: String::new(),
        updated_at: String::new(),
    }
}

#[test]
fn the_apply_lock_is_shared_within_a_library_and_separate_across_libraries() {
    use ps_server::services::mods::deploy::try_lock_target;
    let dir = tempfile::tempdir().unwrap();
    let one = LibraryPaths::new(&dir.path().join("one"));
    let two = LibraryPaths::new(&dir.path().join("two"));
    let target = lock_target("server-1", &dir.path().join("root"));

    let held = try_lock_target(&one, &target).expect("nothing holds it yet");
    assert!(
        try_lock_target(&one, &target).is_none(),
        "the same id under one library blocks"
    );
    assert!(
        try_lock_target(&two, &target).is_some(),
        "the same id under another library does not"
    );
    drop(held);
}

#[test]
fn a_target_whose_root_moves_keeps_its_apply_lock() {
    use ps_server::services::mods::deploy::try_lock_target;
    let dir = tempfile::tempdir().unwrap();
    let library = LibraryPaths::new(dir.path());
    let before = lock_target("server-1", &dir.path().join("old-install"));
    let after = lock_target("server-1", &dir.path().join("new-install"));

    let _held = try_lock_target(&library, &before).expect("nothing holds it yet");
    assert!(
        try_lock_target(&library, &after).is_none(),
        "an apply that read the moved root must still wait for one that read the old root"
    );
}

#[tokio::test]
async fn applying_twice_changes_nothing_the_second_time() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[("ue4ss", "CoolMod/Scripts/main.lua", b"print('hi')")],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;

    apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    let second = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();

    assert!(
        !second.plan.entries.is_empty(),
        "an empty plan is not the same as a plan of nothing but Keep"
    );
    assert!(
        second
            .plan
            .entries
            .iter()
            .all(|e| matches!(e, PlanEntry::Keep { .. })),
        "the second apply has nothing to do: {:?}",
        second.plan.entries
    );
    assert!(!second.mid_apply);

    let deployed = ue4ss_dir(&root).join("CoolMod/Scripts/main.lua");
    assert_eq!(
        std::fs::read_to_string(&deployed).unwrap(),
        "print('hi')",
        "a no-op apply must not have touched the file's content"
    );
    let rows = ps_db::mod_deployments::files_of(&db, &target.id)
        .await
        .unwrap();
    let row = rows
        .iter()
        .find(|r| Path::new(&r.path) == deployed.as_path())
        .expect("the row survives a no-op apply");
    assert_eq!(
        row.hash,
        ps_server::services::mods::digest::hash_bytes(b"print('hi')"),
        "and its recorded hash still matches what is actually deployed"
    );
}

#[tokio::test]
async fn disabling_a_mod_removes_its_files_and_its_rows() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[("ue4ss", "CoolMod/Scripts/main.lua", b"print('hi')")],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;
    apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    let deployed = ue4ss_dir(&root).join("CoolMod/Scripts/main.lua");
    assert!(deployed.is_file());

    enable(&db, &target.id, "coolmod-ue4ss", None, false).await;
    apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();

    assert!(!deployed.exists(), "a disabled mod's files are removed");
    assert!(
        ps_db::mod_deployments::files_of(&db, &target.id)
            .await
            .unwrap()
            .iter()
            .all(|r| !r.path.ends_with("main.lua")),
        "and so are its rows"
    );
    let marker = std::fs::read_to_string(ue4ss_dir(&root).join("mods.txt")).unwrap();
    assert!(marker.contains("CoolMod : 0"), "{marker}");
}

#[tokio::test]
async fn a_user_edit_is_preserved_rather_than_overwritten() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[("ue4ss", "CoolMod/config.lua", b"defaults")],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;
    apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();

    // The user edits the deployed config, then a new version arrives.
    let deployed = ue4ss_dir(&root).join("CoolMod/config.lua");
    std::fs::write(&deployed, b"the user's settings").unwrap();
    let second = install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "2.0",
        &[("ue4ss", "CoolMod/config.lua", b"new defaults")],
    )
    .await;
    enable(
        &db,
        &target.id,
        "coolmod-ue4ss",
        Some(&second.version.id),
        true,
    )
    .await;

    let outcome = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(&deployed).unwrap(),
        "the user's settings",
        "an upgrade must not overwrite a file the user edited"
    );
    assert!(
        outcome
            .preserved
            .iter()
            .any(|p| Path::new(p) == deployed.as_path()),
        "and the result says so: {:?}",
        outcome.preserved
    );
}

#[tokio::test]
async fn a_dropped_file_the_user_edited_goes_to_the_backup_set_not_the_bin() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[
            ("ue4ss", "CoolMod/main.lua", b"main"),
            ("ue4ss", "CoolMod/config.lua", b"defaults"),
        ],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;
    apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();

    let config = ue4ss_dir(&root).join("CoolMod/config.lua");
    std::fs::write(&config, b"the user's settings").unwrap();

    // Version 2 no longer ships config.lua at all.
    let second = install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "2.0",
        &[("ue4ss", "CoolMod/main.lua", b"main v2")],
    )
    .await;
    enable(
        &db,
        &target.id,
        "coolmod-ue4ss",
        Some(&second.version.id),
        true,
    )
    .await;

    let outcome = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();

    assert!(!config.exists(), "the dropped file is gone from the target");
    let backup_dir = outcome.backup_dir.expect("a backup set was created");
    let index: Vec<ps_server::services::mods::deploy::backup::BackupEntry> = serde_json::from_str(
        &std::fs::read_to_string(Path::new(&backup_dir).join("index.json")).unwrap(),
    )
    .unwrap();
    let entry = index
        .iter()
        .find(|e| Path::new(&e.original_path) == config.as_path())
        .expect("the edited file is in the index");
    assert_eq!(
        std::fs::read_to_string(Path::new(&backup_dir).join(&entry.backup_key)).unwrap(),
        "the user's settings",
        "the user's edit is recoverable, not destroyed"
    );
}

#[tokio::test]
async fn an_unmanaged_file_in_the_way_is_refused_before_anything_is_written() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[("ue4ss", "CoolMod/main.lua", b"from the library")],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;

    let occupied = ue4ss_dir(&root).join("CoolMod/main.lua");
    std::fs::create_dir_all(occupied.parent().unwrap()).unwrap();
    std::fs::write(&occupied, b"something the user put here").unwrap();

    let failed = apply::apply(&db, &paths, &target, &ApplyOptions::default()).await;
    match failed {
        Err(apply::ApplyError::Preflight(
            ps_core::mods::PreflightError::UnmanagedOccupant { ref paths },
        )) => assert!(
            paths.iter().any(|p| Path::new(p) == occupied.as_path()),
            "the refusal has to name the contested path: {paths:?}"
        ),
        ref other => panic!("expected an unmanaged occupant refusal, got {other:?}"),
    }
    assert_eq!(
        std::fs::read_to_string(&occupied).unwrap(),
        "something the user put here",
        "and nothing was written"
    );
    assert!(
        ps_db::mod_deployments::journal_of(&db, &target.id)
            .await
            .unwrap()
            .is_none(),
        "the refusal happens before the journal"
    );
}

#[tokio::test]
async fn a_running_game_refuses_the_apply() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;

    let options = ApplyOptions {
        running: &AlwaysRunning,
        move_strategy: MoveStrategy::Auto,
        stamp: None,
        replace_occupants: &[],
    };
    let failed = apply::apply(&db, &paths, &target, &options).await;
    assert!(
        matches!(failed, Err(apply::ApplyError::TargetLocked(_))),
        "{failed:?}"
    );
}

#[tokio::test]
async fn a_crash_between_the_rename_and_the_row_is_recovered_on_the_next_apply() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[("ue4ss", "CoolMod/main.lua", b"print('hi')")],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;
    apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();

    // Simulate the crash: the file is on disk and correct, the journal still names
    // it, and the row is gone. This is exactly the rename-succeeded-row-failed case.
    let deployed = ue4ss_dir(&root).join("CoolMod/main.lua");
    let plan = apply::plan_for(&db, &target, &format!("{}/default", target.id))
        .await
        .unwrap()
        .0;
    let journalled = serde_json::json!({
        "plan": plan,
        "markers": [],
        "backups": [],
        "op_id": "0123456789abcdef",
        "stamp": "20260913-120000",
    });
    ps_db::mod_deployments::forget(&db, &target.id, &[deployed.to_string_lossy().as_ref()])
        .await
        .unwrap();
    ps_db::mod_deployments::open_journal(
        &db,
        &target.id,
        "0123456789abcdef",
        &journalled.to_string(),
    )
    .await
    .unwrap();

    let outcome = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(
        !outcome.mid_apply,
        "recovery closed the journal: {:?}",
        outcome.failed
    );
    assert_eq!(
        std::fs::read_to_string(&deployed).unwrap(),
        "print('hi')",
        "the file was already correct and must not have been rewritten from scratch"
    );
    assert!(
        ps_db::mod_deployments::files_of(&db, &target.id)
            .await
            .unwrap()
            .iter()
            .any(|r| Path::new(&r.path) == deployed.as_path()),
        "and the row it was missing is back"
    );
}

#[tokio::test]
async fn a_leftover_stage_file_is_overwritten_by_a_genuine_write() {
    // This exercises `write::copy_staged`'s own overwrite of a stale `.psnew`
    // during a real `Add`, not anything specific to journal recovery: the plan
    // here is a first-ever install, so the file's own write step runs and
    // clobbers whatever was already at the stage path.
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[("ue4ss", "CoolMod/main.lua", b"print('hi')")],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;

    let deployed = ue4ss_dir(&root).join("CoolMod/main.lua");
    let staged = ps_server::services::mods::deploy::write::staged_path(&deployed);
    std::fs::create_dir_all(staged.parent().unwrap()).unwrap();
    std::fs::write(&staged, b"half a file from a crashed run").unwrap();

    apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(
        !staged.exists(),
        "an interrupted write leaves a stage file, and it must not survive"
    );
    assert_eq!(std::fs::read_to_string(&deployed).unwrap(), "print('hi')");
}

#[tokio::test]
async fn a_leftover_stage_file_beside_an_already_correct_file_is_swept() {
    // Unlike the test above, this file is already correctly deployed, so the
    // second apply's plan for it is `Keep` — which performs no filesystem step
    // of its own. Nothing but a dedicated sweep can remove a stage file left
    // beside a `Keep` path.
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[("ue4ss", "CoolMod/main.lua", b"print('hi')")],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;
    apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();

    let deployed = ue4ss_dir(&root).join("CoolMod/main.lua");
    let staged = ps_server::services::mods::deploy::write::staged_path(&deployed);
    std::fs::write(&staged, b"half a file from a crashed run").unwrap();

    let outcome = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(
        outcome.plan.entries.iter().any(
            |e| matches!(e, PlanEntry::Keep { path } if Path::new(path) == deployed.as_path())
        ),
        "this run's plan must actually Keep the swept path: {:?}",
        outcome.plan.entries
    );
    assert!(
        !staged.exists(),
        "a stage file beside a Keep path must still be swept"
    );
    assert_eq!(std::fs::read_to_string(&deployed).unwrap(), "print('hi')");
}

#[tokio::test]
async fn a_cross_volume_move_carries_an_edited_file_intact() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[("ue4ss", "CoolMod/config.lua", b"defaults")],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;

    let options = ApplyOptions {
        running: &ps_server::services::mods::deploy::NeverRunning,
        move_strategy: MoveStrategy::ForceCrossVolume,
        stamp: None,
        replace_occupants: &[],
    };
    apply::apply(&db, &paths, &target, &options).await.unwrap();

    // The user edits it, then the UE4SS mods directory moves.
    let old = ue4ss_dir(&root).join("CoolMod/config.lua");
    std::fs::write(&old, b"the user's settings").unwrap();
    let moved_dir = dir.path().join("Elsewhere/Mods");
    ps_db::mod_targets::set_layout_overrides(
        &db,
        &target.id,
        &serde_json::json!({ "ue4ss_mods_dir": moved_dir.to_string_lossy() }).to_string(),
    )
    .await
    .unwrap();
    let moved_target = ps_db::mod_targets::get(&db, &target.id)
        .await
        .unwrap()
        .unwrap();

    let outcome = apply::apply(&db, &paths, &moved_target, &options)
        .await
        .unwrap();
    assert!(!outcome.mid_apply, "{:?}", outcome.failed);
    let new = moved_dir.join("CoolMod/config.lua");
    assert_eq!(
        std::fs::read_to_string(&new).unwrap(),
        "the user's settings",
        "a move carries the file as it is on disk, edits included"
    );
    assert!(!old.exists(), "and the source is gone");
    let rows = ps_db::mod_deployments::files_of(&db, &target.id)
        .await
        .unwrap();
    assert!(
        rows.iter().any(|r| Path::new(&r.path) == new.as_path()),
        "the row followed the file: {rows:?}"
    );
}

#[tokio::test]
async fn two_mods_wanting_one_path_is_refused_by_name() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    let mut version_ids = Vec::new();
    for id in ["a-pak", "b-pak"] {
        let stored =
            install_fixture_mod(&db, &paths, id, "1.0", &[("pak", "Shared_P.pak", b"pak")]).await;
        version_ids.push(stored.version.id);
        enable(&db, &target.id, id, None, true).await;
    }

    let failed = apply::apply(&db, &paths, &target, &ApplyOptions::default()).await;
    match failed {
        Err(apply::ApplyError::Preflight(ps_core::mods::PreflightError::DestinationConflict {
            path,
            mod_version_ids,
        })) => {
            assert!(path.ends_with("Shared_P.pak"), "{path}");
            assert_eq!(mod_version_ids.len(), 2, "{mod_version_ids:?}");
            for expected in &version_ids {
                assert!(
                    mod_version_ids.contains(expected),
                    "{mod_version_ids:?} is missing {expected}"
                );
            }
        }
        other => panic!("expected a destination conflict naming the shared path, got {other:?}"),
    }
}

#[tokio::test]
async fn a_relocated_server_mods_directory_is_carried_as_a_move_not_a_backup() {
    // A server target's directories come entirely from `layout_overrides` (see
    // `ps_db::mod_targets::server_layout_overrides`), so a relocation there is
    // always one override replacing another, never a bare default gaining an
    // override for the first time. Neither the current layout nor the
    // no-override default layout sits under the other in that case, which is
    // exactly the gap `desired_kind_for`'s fall-through closes.
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("server-root");
    std::fs::create_dir_all(&root).unwrap();
    let old_dir = dir.path().join("binds/a/mods");
    let target = ps_db::mod_targets::upsert(
        &db,
        &ps_db::mod_targets::NewModTarget {
            id: "server-1".to_string(),
            kind: "native_server".to_string(),
            name: "Server".to_string(),
            root_path: root.to_string_lossy().into_owned(),
            platform: "win64".to_string(),
            ue4ss_mode: "standard".to_string(),
            layout_overrides: serde_json::json!({ "ue4ss_mods_dir": old_dir.to_string_lossy() })
                .to_string(),
            detected: "{}".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[("ue4ss", "CoolMod/config.lua", b"defaults")],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;
    apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();

    let old = old_dir.join("CoolMod/config.lua");
    assert!(old.is_file(), "{old:?}");
    std::fs::write(&old, b"the user's settings").unwrap();

    let new_dir = dir.path().join("binds/b/mods");
    ps_db::mod_targets::set_layout_overrides(
        &db,
        &target.id,
        &serde_json::json!({ "ue4ss_mods_dir": new_dir.to_string_lossy() }).to_string(),
    )
    .await
    .unwrap();
    let moved_target = ps_db::mod_targets::get(&db, &target.id)
        .await
        .unwrap()
        .unwrap();

    let outcome = apply::apply(&db, &paths, &moved_target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(!outcome.mid_apply, "{:?}", outcome.failed);
    assert!(
        outcome
            .plan
            .entries
            .iter()
            .any(|e| matches!(e, PlanEntry::Move { .. })),
        "a relocation between two overrides must be a move, not a backup and re-fetch: {:?}",
        outcome.plan.entries
    );
    let new = new_dir.join("CoolMod/config.lua");
    assert_eq!(
        std::fs::read_to_string(&new).unwrap(),
        "the user's settings",
        "the edit made between the two applies must be carried, not lost to a fresh add"
    );
    assert!(!old.exists(), "and the old location is gone");
    let rows = ps_db::mod_deployments::files_of(&db, &target.id)
        .await
        .unwrap();
    assert!(
        rows.iter().any(|r| Path::new(&r.path) == new.as_path()),
        "the row followed the file: {rows:?}"
    );

    let again = apply::apply(&db, &paths, &moved_target, &ApplyOptions::default())
        .await
        .unwrap();
    assert_carried_edit_is_preserved(&again, &new);
}

/// After a move carried an edit, the next plan must still see the edit as the
/// user's: `Preserve`, with the user's bytes untouched.
fn assert_carried_edit_is_preserved(outcome: &apply::ApplyOutcome, path: &Path) {
    assert!(!outcome.mid_apply && outcome.error.is_none(), "{outcome:?}");
    assert!(
        outcome
            .plan
            .entries
            .iter()
            .any(|e| matches!(e, PlanEntry::Preserve { path: p, .. } if Path::new(p) == path)),
        "the carried edit must be preserved, not replaced: {:?}",
        outcome.plan.entries
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), "the user's settings");
}

#[tokio::test]
async fn a_crash_recovered_move_of_an_edited_file_stays_the_users() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[("ue4ss", "CoolMod/config.lua", b"defaults")],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;
    apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();

    let old = ue4ss_dir(&root).join("CoolMod/config.lua");
    std::fs::write(&old, b"the user's settings").unwrap();
    let moved_dir = dir.path().join("Elsewhere/Mods");
    ps_db::mod_targets::set_layout_overrides(
        &db,
        &target.id,
        &serde_json::json!({ "ue4ss_mods_dir": moved_dir.to_string_lossy() }).to_string(),
    )
    .await
    .unwrap();
    let moved_target = ps_db::mod_targets::get(&db, &target.id)
        .await
        .unwrap()
        .unwrap();
    let plan = apply::plan_for(&db, &moved_target, &format!("{}/default", target.id))
        .await
        .unwrap()
        .0;
    let journalled = serde_json::json!({
        "plan": plan,
        "markers": [],
        "backups": [],
        "op_id": "0123456789abcdef",
        "stamp": "20260913-120000",
    });
    ps_db::mod_deployments::open_journal(
        &db,
        &target.id,
        "0123456789abcdef",
        &journalled.to_string(),
    )
    .await
    .unwrap();
    // The crash came after the rename and before either row changed.
    let new = moved_dir.join("CoolMod/config.lua");
    std::fs::create_dir_all(new.parent().unwrap()).unwrap();
    std::fs::rename(&old, &new).unwrap();

    let recovered = apply::apply(&db, &paths, &moved_target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(ps_db::mod_deployments::journal_of(&db, &target.id)
        .await
        .unwrap()
        .is_none());
    assert_carried_edit_is_preserved(&recovered, &new);

    let again = apply::apply(&db, &paths, &moved_target, &ApplyOptions::default())
        .await
        .unwrap();
    assert_carried_edit_is_preserved(&again, &new);
}

#[cfg(any(windows, target_os = "macos"))]
#[tokio::test]
async fn a_recorded_file_respelled_only_in_case_is_still_the_apps_own() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[("ue4ss", "CoolMod/config.lua", b"defaults")],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;
    apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    let recorded = ue4ss_dir(&root).join("CoolMod/config.lua");
    std::fs::write(&recorded, b"the user's settings").unwrap();

    let renamed = install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "2.0",
        &[("ue4ss", "coolmod/config.lua", b"new defaults")],
    )
    .await;
    enable(
        &db,
        &target.id,
        "coolmod-ue4ss",
        Some(&renamed.version.id),
        true,
    )
    .await;
    let respelled = ue4ss_dir(&root).join("coolmod/config.lua");
    let listed = [respelled.clone()];
    let options = ApplyOptions {
        replace_occupants: &listed,
        ..ApplyOptions::default()
    };
    let outcome = apply::apply(&db, &paths, &target, &options)
        .await
        .unwrap_or_else(|e| panic!("a case-only respelling is not an occupant: {e}"));
    assert!(outcome.error.is_none(), "{:?}", outcome.error);
    assert!(
        outcome.backup_dir.is_none(),
        "replace moved the app's own file: {:?}",
        outcome.backup_dir
    );
    assert_eq!(
        std::fs::read_to_string(&respelled).unwrap(),
        "the user's settings"
    );

    std::fs::write(&respelled, b"defaults").unwrap();
    let outcome = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(outcome.error.is_none(), "{:?}", outcome.error);
    assert_eq!(std::fs::read_to_string(&respelled).unwrap(), "new defaults");
    let rows: Vec<_> = ps_db::mod_deployments::files_of(&db, &target.id)
        .await
        .unwrap()
        .into_iter()
        .filter(|r| r.path.to_lowercase().ends_with("config.lua"))
        .collect();
    assert_eq!(rows.len(), 1, "one file, one row: {rows:?}");
    assert_eq!(
        Path::new(&rows[0].path),
        recorded.as_path(),
        "the row keeps the spelling the directory on disk still has"
    );
    assert_eq!(rows[0].mod_version_id.as_deref(), Some(renamed.version.id.as_str()));

    use ps_server::services::mods::scan::{scan, FileState};
    let report = scan(&db, &target).await.unwrap();
    let config: Vec<_> = report
        .files
        .iter()
        .filter(|f| f.path.to_lowercase().ends_with("config.lua"))
        .collect();
    assert_eq!(config.len(), 1, "{config:?}");
    assert_eq!(config[0].state, FileState::ManagedIntact, "{config:?}");
}

#[cfg(any(windows, target_os = "macos"))]
#[tokio::test]
async fn a_case_respelled_write_recovered_after_a_crash_keeps_one_row() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[("ue4ss", "CoolMod/config.lua", b"defaults")],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;
    apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    let recorded = ue4ss_dir(&root).join("CoolMod/config.lua");

    let renamed = install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "2.0",
        &[("ue4ss", "coolmod/config.lua", b"new defaults")],
    )
    .await;
    enable(
        &db,
        &target.id,
        "coolmod-ue4ss",
        Some(&renamed.version.id),
        true,
    )
    .await;
    let plan = apply::plan_for(&db, &target, &format!("{}/default", target.id))
        .await
        .unwrap()
        .0;
    assert!(
        plan.entries
            .iter()
            .any(|e| matches!(e, PlanEntry::Replace { .. })),
        "{:?}",
        plan.entries
    );
    let journalled = serde_json::json!({
        "plan": plan,
        "markers": [],
        "backups": [],
        "op_id": "0123456789abcdef",
        "stamp": "20260913-120000",
    });
    ps_db::mod_deployments::open_journal(
        &db,
        &target.id,
        "0123456789abcdef",
        &journalled.to_string(),
    )
    .await
    .unwrap();
    // The write landed under the new spelling; its row never did.
    std::fs::write(ue4ss_dir(&root).join("coolmod/config.lua"), b"new defaults").unwrap();

    let outcome = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(!outcome.mid_apply && outcome.error.is_none(), "{outcome:?}");
    let rows: Vec<_> = ps_db::mod_deployments::files_of(&db, &target.id)
        .await
        .unwrap()
        .into_iter()
        .filter(|r| r.path.to_lowercase().ends_with("config.lua"))
        .collect();
    assert_eq!(rows.len(), 1, "recovery updated the row it had: {rows:?}");
    assert_eq!(Path::new(&rows[0].path), recorded.as_path());
    assert_eq!(rows[0].mod_version_id.as_deref(), Some(renamed.version.id.as_str()));

    enable(&db, &target.id, "coolmod-ue4ss", Some(&renamed.version.id), false).await;
    let removed = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(removed.error.is_none(), "{removed:?}");
    assert!(
        !removed
            .plan
            .entries
            .iter()
            .any(|e| matches!(e, PlanEntry::RemovePreserve { .. })),
        "the app's own file is not preserved: {:?}",
        removed.plan.entries
    );
    assert!(removed.backup_dir.is_none(), "{:?}", removed.backup_dir);
    assert!(!recorded.exists());
}

#[tokio::test]
async fn a_stale_row_after_a_crashed_replace_is_corrected_by_recovery() {
    // The `mod_version_id` is deliberately held constant across both applies:
    // a version change would classify the second plan as `Reattribute`, which
    // corrects a stale row through a completely different path (the `Reattribute`
    // arm in `execute`, fixed separately) and would not isolate this fix. What
    // changes here is the library's own copy of the file -- `desired::build`
    // reads its hash fresh on every apply, so the same `mod_version_id` can
    // legitimately want different bytes without ever being reattributed.
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    let stored = install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[("ue4ss", "CoolMod/main.lua", b"v1")],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;
    apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();

    let deployed = ue4ss_dir(&root).join("CoolMod/main.lua");
    let library_file = LibraryPaths::route_path_in(
        Path::new(&stored.version.library_dir),
        ps_core::mods::RouteKind::Ue4ss,
        "CoolMod/main.lua",
    );
    std::fs::write(&library_file, b"v2").unwrap();

    let plan = apply::plan_for(&db, &target, &format!("{}/default", target.id))
        .await
        .unwrap()
        .0;
    assert!(
        plan.entries.iter().any(|e| matches!(
            e,
            PlanEntry::Replace { mod_version_id, .. } if mod_version_id == &stored.version.id
        )),
        "{:?}",
        plan.entries
    );

    // Simulate the crash: the plan's file-write step (a `Replace`) finished --
    // the rename landed v2's bytes -- but the row-write step that should have
    // followed it never ran, so the row is still v1's: present, but stale, not
    // missing.
    std::fs::write(&deployed, b"v2").unwrap();
    let journalled = serde_json::json!({
        "plan": plan,
        "markers": [],
        "backups": [],
        "op_id": "fedcba9876543210",
        "stamp": "20260913-120000",
    });
    ps_db::mod_deployments::open_journal(
        &db,
        &target.id,
        "fedcba9876543210",
        &journalled.to_string(),
    )
    .await
    .unwrap();

    let outcome = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(!outcome.mid_apply, "{:?}", outcome.failed);
    assert_eq!(
        std::fs::read_to_string(&deployed).unwrap(),
        "v2",
        "must not have been rewritten from scratch"
    );

    let rows = ps_db::mod_deployments::files_of(&db, &target.id)
        .await
        .unwrap();
    let row = rows
        .iter()
        .find(|r| Path::new(&r.path) == deployed.as_path())
        .expect("the row is still there");
    assert_eq!(
        row.hash,
        ps_server::services::mods::digest::hash_bytes(b"v2"),
        "the stale row's hash must be corrected to what is actually on disk, \
         or a later apply will misread this file as a user edit forever"
    );
}

#[tokio::test]
async fn recovery_finishes_a_move_that_reached_the_destination() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(&db, &paths, "coolmod-ue4ss", "1.0", &[
        ("ue4ss", "CoolMod/config.lua", b"defaults"),
    ])
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;
    apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();

    let from = ue4ss_dir(&root).join("CoolMod/config.lua");
    let to = dir.path().join("Elsewhere/Mods/CoolMod/config.lua");
    std::fs::create_dir_all(to.parent().unwrap()).unwrap();
    std::fs::rename(&from, &to).unwrap();

    // The journal says a move was planned; the destination exists, the source does
    // not, and the row still names the source. That is the completed-but-unrecorded
    // state, and recovery must simply move the row.
    let hash = ps_server::services::mods::digest::hash_file(&to).unwrap();
    let journalled = serde_json::json!({
        "plan": { "entries": [ {
            "op": "move",
            "from": from.to_string_lossy(),
            "to": to.to_string_lossy(),
            "mod_version_id": "coolmod-ue4ss@1.0",
            "rel_path": "CoolMod/config.lua",
            "hash": hash,
            "role": "file",
        } ] },
        "markers": [],
        "backups": [],
        "op_id": "0123456789abcdef",
        "stamp": "20260913-120000",
    });
    ps_db::mod_deployments::open_journal(
        &db,
        &target.id,
        "0123456789abcdef",
        &journalled.to_string(),
    )
    .await
    .unwrap();

    let failed = ps_server::services::mods::deploy::recover::recover(
        &db,
        &paths,
        &target,
        &ApplyOptions::default(),
    )
    .await
    .unwrap();
    assert!(failed.is_empty(), "{failed:?}");
    let rows = ps_db::mod_deployments::files_of(&db, &target.id).await.unwrap();
    assert!(
        rows.iter().any(|r| Path::new(&r.path) == to.as_path()),
        "the row followed the file: {rows:?}"
    );
    assert!(
        !rows.iter().any(|r| Path::new(&r.path) == from.as_path()),
        "and no longer names the source"
    );
}

#[tokio::test]
async fn recovery_prunes_the_folder_an_interrupted_removal_emptied() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[("ue4ss", "CoolMod/Scripts/main.lua", b"print('hi')")],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;
    apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();

    let removed = ue4ss_dir(&root).join("CoolMod/Scripts/main.lua");
    std::fs::remove_file(&removed).unwrap();
    let journalled = serde_json::json!({
        "plan": { "entries": [ { "op": "remove", "path": removed.to_string_lossy() } ] },
        "markers": [],
        "backups": [],
        "op_id": "0123456789abcdef",
        "stamp": "20260913-120000",
    });
    ps_db::mod_deployments::open_journal(
        &db,
        &target.id,
        "0123456789abcdef",
        &journalled.to_string(),
    )
    .await
    .unwrap();

    let failed = ps_server::services::mods::deploy::recover::recover(
        &db,
        &paths,
        &target,
        &ApplyOptions::default(),
    )
    .await
    .unwrap();
    assert!(failed.is_empty(), "{failed:?}");
    assert!(!ue4ss_dir(&root).join("CoolMod").exists());
    assert!(ue4ss_dir(&root).is_dir());
}

#[tokio::test]
async fn recovery_deletes_a_stage_file_from_an_interrupted_write() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;

    let dest = ue4ss_dir(&root).join("CoolMod/main.lua");
    let staged = ps_server::services::mods::deploy::write::staged_path(&dest);
    std::fs::create_dir_all(staged.parent().unwrap()).unwrap();
    std::fs::write(&staged, b"half written").unwrap();

    let journalled = serde_json::json!({
        "plan": { "entries": [ {
            "op": "add",
            "path": dest.to_string_lossy(),
            "source": "unused",
            "expected_hash": "nothing matches this",
            "mod_version_id": "coolmod-ue4ss@1.0",
            "rel_path": "CoolMod/main.lua",
            "role": "file",
        } ] },
        "markers": [],
        "backups": [],
        "op_id": "0123456789abcdef",
        "stamp": "20260913-120000",
    });
    ps_db::mod_deployments::open_journal(
        &db,
        &target.id,
        "0123456789abcdef",
        &journalled.to_string(),
    )
    .await
    .unwrap();

    ps_server::services::mods::deploy::recover::recover(
        &db,
        &paths,
        &target,
        &ApplyOptions::default(),
    )
    .await
    .unwrap();
    assert!(!staged.exists(), "the stage file is always removed");
    assert!(
        !dest.exists(),
        "and the destination is untouched, because the rename never happened"
    );
}

#[tokio::test]
async fn an_unreconciled_entry_keeps_the_journal_open() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[("ue4ss", "CoolMod/config.lua", b"defaults")],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;
    apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();

    // A journalled `Move` with different content at both ends. §7.1.2 hands this
    // one to a human: recovery leaves both files, keeps the row on the source and
    // reports the destination. Nothing the deployer can do settles it, so this is
    // what a journal that stays open now looks like -- and, unlike an entry whose
    // write simply never started, it is not something the next apply converges.
    let from = ue4ss_dir(&root).join("CoolMod/config.lua");
    let to = dir.path().join("Elsewhere/Mods/CoolMod/config.lua");
    common::mods::write(&to, b"something else entirely");
    let journalled = serde_json::json!({
        "plan": { "entries": [ {
            "op": "move",
            "from": from.to_string_lossy(),
            "to": to.to_string_lossy(),
            "mod_version_id": "coolmod-ue4ss@1.0",
            "rel_path": "CoolMod/config.lua",
            "hash": ps_server::services::mods::digest::hash_bytes(b"defaults"),
            "role": "file",
        } ] },
        "markers": [],
        "backups": [],
        "op_id": "0123456789abcdef",
        "stamp": "20260913-120000",
    });
    ps_db::mod_deployments::open_journal(
        &db,
        &target.id,
        "0123456789abcdef",
        &journalled.to_string(),
    )
    .await
    .unwrap();

    // Driven through `apply`, not through `recover` alone: the point is that the
    // caller is told, and told which path.
    let outcome = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(outcome.mid_apply, "{outcome:?}");
    assert!(
        outcome.failed.iter().any(|p| Path::new(p) == to.as_path()),
        "the refusal has to name the path a human must settle: {:?}",
        outcome.failed
    );
    assert!(
        ps_db::mod_deployments::journal_of(&db, &target.id)
            .await
            .unwrap()
            .is_some(),
        "an entry that cannot be reconciled must keep the journal open"
    );
    assert_eq!(
        std::fs::read_to_string(&from).unwrap(),
        "defaults",
        "and nothing was written while it was unresolved"
    );
}

#[tokio::test]
async fn an_interrupted_move_of_an_edited_file_is_completed_not_reported() {
    // The state a `Move` exists for: the source carries the user's edit, so its
    // bytes differ from its own row's hash as a matter of course (`build_plan`
    // takes the entry's hash from disk, never from the row). Asking the source to
    // match its row would make every interrupted move of an edited file an
    // unreconcilable entry, and the target would never accept another apply.
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[("ue4ss", "CoolMod/config.lua", b"defaults")],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;
    apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();

    let old = ue4ss_dir(&root).join("CoolMod/config.lua");
    std::fs::write(&old, b"the user's settings").unwrap();
    let moved_dir = dir.path().join("Elsewhere/Mods");
    ps_db::mod_targets::set_layout_overrides(
        &db,
        &target.id,
        &serde_json::json!({ "ue4ss_mods_dir": moved_dir.to_string_lossy() }).to_string(),
    )
    .await
    .unwrap();
    let moved_target = ps_db::mod_targets::get(&db, &target.id)
        .await
        .unwrap()
        .unwrap();

    // The plan a real apply would journal, journalled, and then nothing done:
    // the process died between opening the journal and the rename.
    let plan = apply::plan_for(&db, &moved_target, &format!("{}/default", target.id))
        .await
        .unwrap()
        .0;
    assert!(
        plan.entries
            .iter()
            .any(|e| matches!(e, PlanEntry::Move { .. })),
        "{:?}",
        plan.entries
    );
    let journalled = serde_json::json!({
        "plan": plan,
        "markers": [],
        "backups": [],
        "op_id": "0123456789abcdef",
        "stamp": "20260913-120000",
    });
    ps_db::mod_deployments::open_journal(
        &db,
        &target.id,
        "0123456789abcdef",
        &journalled.to_string(),
    )
    .await
    .unwrap();

    let outcome = apply::apply(&db, &paths, &moved_target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(
        !outcome.mid_apply,
        "an un-started move of an edited file must not wedge the target: {:?} {:?}",
        outcome.failed, outcome.error
    );
    let new = moved_dir.join("CoolMod/config.lua");
    assert_eq!(
        std::fs::read_to_string(&new).unwrap(),
        "the user's settings",
        "the retry finishes the move and carries the edit"
    );
    assert!(!old.exists(), "and the source is gone");
    let rows = ps_db::mod_deployments::files_of(&db, &target.id)
        .await
        .unwrap();
    assert!(
        rows.iter().any(|r| Path::new(&r.path) == new.as_path()),
        "the row followed the file: {rows:?}"
    );
    assert!(
        ps_db::mod_deployments::journal_of(&db, &target.id)
            .await
            .unwrap()
            .is_none(),
        "and the journal closed"
    );
}

#[tokio::test]
async fn a_first_apply_over_the_users_own_marker_is_not_held_open_by_it() {
    // The marker on disk holds the user's own bytes, which are by construction
    // not the bytes the app intended to write. The journal explains nothing about
    // that file, so it must not keep the journal open -- and this path is
    // reachable for every target whose owner ever hand-edited `mods.txt`.
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[("ue4ss", "CoolMod/main.lua", b"main")],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;

    let mods_txt = ue4ss_dir(&root).join("mods.txt");
    common::mods::write(&mods_txt, b"; the user's own file\r\nSomeOtherMod : 1\r\n");

    // A journal from a first apply whose execute died before the marker loop: the
    // marker is journalled with the hash the app meant to write, and the file on
    // disk is still the user's.
    let journalled = serde_json::json!({
        "plan": { "entries": [] },
        "markers": [ {
            "path": mods_txt.to_string_lossy(),
            "hash": "the hash the app meant to write",
        } ],
        "backups": [],
        "op_id": "0123456789abcdef",
        "stamp": "20260913-120000",
    });
    ps_db::mod_deployments::open_journal(
        &db,
        &target.id,
        "0123456789abcdef",
        &journalled.to_string(),
    )
    .await
    .unwrap();

    let outcome = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(
        !outcome.mid_apply,
        "a marker holding the user's own bytes is not something the journal \
         explains, and must not wedge the target: {:?} {:?}",
        outcome.failed, outcome.error
    );
    let written = std::fs::read_to_string(&mods_txt).unwrap();
    assert!(
        written.contains("CoolMod") && written.contains("SomeOtherMod"),
        "the retry wrote the marker, merging rather than replacing: {written}"
    );
    assert!(
        ps_db::mod_deployments::journal_of(&db, &target.id)
            .await
            .unwrap()
            .is_none(),
        "and the journal closed"
    );
}

#[tokio::test]
async fn a_path_reconciliation_cannot_read_is_reported_rather_than_thrown() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;

    // A directory where the journal says a file should be. Nothing can hash it,
    // and the answer §7.1 step 6 asks for is a named path, not an IO error that
    // tells the caller nothing about which path or how far the apply got.
    let dest = ue4ss_dir(&root).join("Ghost/main.lua");
    std::fs::create_dir_all(&dest).unwrap();
    let journalled = serde_json::json!({
        "plan": { "entries": [ {
            "op": "add",
            "path": dest.to_string_lossy(),
            "source": "unused",
            "expected_hash": "a hash nothing on disk has",
            "mod_version_id": "ghost-ue4ss@1.0",
            "rel_path": "Ghost/main.lua",
            "role": "file",
        } ] },
        "markers": [],
        "backups": [],
        "op_id": "0123456789abcdef",
        "stamp": "20260913-120000",
    });
    ps_db::mod_deployments::open_journal(
        &db,
        &target.id,
        "0123456789abcdef",
        &journalled.to_string(),
    )
    .await
    .unwrap();

    let outcome = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .expect("an unreadable path is an outcome, not an error");
    assert!(outcome.mid_apply);
    assert!(
        outcome.failed.iter().any(|p| Path::new(p) == dest.as_path()),
        "{:?}",
        outcome.failed
    );
}

#[tokio::test]
async fn recovery_applies_a_reattribution_with_the_desired_role_and_hash() {
    // The second `Reattribute` site: the one `recover` owns. `execute`'s is
    // covered by `an_upgrade_that_changes_no_bytes_keeps_each_row_role_and_hash`,
    // and neither site can stand in for the other.
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[("ue4ss", "CoolMod/enabled.txt", b"")],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;
    apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();

    // Version 2 ships the same bytes, so the plan is a reattribution; the journal
    // names it and the row update never ran.
    let second = install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "2.0",
        &[("ue4ss", "CoolMod/enabled.txt", b"")],
    )
    .await;
    enable(
        &db,
        &target.id,
        "coolmod-ue4ss",
        Some(&second.version.id),
        true,
    )
    .await;
    let marker = ue4ss_dir(&root).join("CoolMod/enabled.txt");
    let journalled = serde_json::json!({
        "plan": { "entries": [ {
            "op": "reattribute",
            "path": marker.to_string_lossy(),
            "mod_version_id": second.version.id,
            "rel_path": "CoolMod/enabled.txt",
        } ] },
        "markers": [],
        "backups": [],
        "op_id": "0123456789abcdef",
        "stamp": "20260913-120000",
    });
    ps_db::mod_deployments::open_journal(
        &db,
        &target.id,
        "0123456789abcdef",
        &journalled.to_string(),
    )
    .await
    .unwrap();

    let failed = ps_server::services::mods::deploy::recover::recover(
        &db,
        &paths,
        &target,
        &ApplyOptions::default(),
    )
    .await
    .unwrap();
    assert!(failed.is_empty(), "{failed:?}");

    let rows = ps_db::mod_deployments::files_of(&db, &target.id)
        .await
        .unwrap();
    let row = rows
        .iter()
        .find(|r| Path::new(&r.path) == marker.as_path())
        .expect("the marker's row");
    assert_eq!(row.mod_version_id.as_deref(), Some(second.version.id.as_str()));
    assert_eq!(
        row.role, "marker",
        "recovery must not rewrite a marker row to a plain file either"
    );
    assert_eq!(
        row.hash,
        ps_server::services::mods::digest::hash_bytes(b""),
        "and the hash recorded is the desired file's, not a stale one"
    );
}

#[tokio::test]
async fn a_partly_executed_apply_is_finished_by_the_next_one() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[
            ("ue4ss", "CoolMod/one.lua", b"one"),
            ("ue4ss", "CoolMod/two.lua", b"two"),
        ],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;

    // The plan a real apply journals, journalled exactly as `apply` journals it,
    // with only the first of its two writes performed and neither row written:
    // the state a process that died part way through execution leaves behind.
    let plan = apply::plan_for(&db, &target, &format!("{}/default", target.id))
        .await
        .unwrap()
        .0;
    assert_eq!(plan.entries.len(), 2, "{:?}", plan.entries);
    let journalled = serde_json::json!({
        "plan": plan,
        "markers": [],
        "backups": [],
        "op_id": "0123456789abcdef",
        "stamp": "20260913-120000",
    });
    ps_db::mod_deployments::open_journal(
        &db,
        &target.id,
        "0123456789abcdef",
        &journalled.to_string(),
    )
    .await
    .unwrap();
    let one = ue4ss_dir(&root).join("CoolMod/one.lua");
    let two = ue4ss_dir(&root).join("CoolMod/two.lua");
    common::mods::write(&one, b"one");

    let outcome = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();

    assert!(
        !outcome.mid_apply,
        "a retry has to finish the remainder, not refuse forever: {:?} {:?}",
        outcome.failed, outcome.error
    );
    assert_eq!(
        std::fs::read_to_string(&two).unwrap(),
        "two",
        "the write that never happened is what the retry is for"
    );
    assert_eq!(
        std::fs::read_to_string(&one).unwrap(),
        "one",
        "and the write that did happen is left alone"
    );
    let rows = ps_db::mod_deployments::files_of(&db, &target.id)
        .await
        .unwrap();
    for (file, body) in [(&one, &b"one"[..]), (&two, &b"two"[..])] {
        let row = rows
            .iter()
            .find(|r| Path::new(&r.path) == file.as_path())
            .unwrap_or_else(|| panic!("no row for {file:?}: {rows:?}"));
        assert_eq!(
            row.hash,
            ps_server::services::mods::digest::hash_bytes(body),
            "the row has to match what is actually on disk"
        );
    }
    assert!(
        ps_db::mod_deployments::journal_of(&db, &target.id)
            .await
            .unwrap()
            .is_none(),
        "and the journal closes, or the target is wedged for good"
    );
}

#[tokio::test]
async fn a_retry_writes_into_the_backup_set_the_journal_already_reserved() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[
            ("ue4ss", "CoolMod/main.lua", b"main"),
            ("ue4ss", "CoolMod/config.lua", b"defaults"),
        ],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;
    apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();

    let config = ue4ss_dir(&root).join("CoolMod/config.lua");
    std::fs::write(&config, b"the user's settings").unwrap();
    let second = install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "2.0",
        &[("ue4ss", "CoolMod/main.lua", b"main v2")],
    )
    .await;
    enable(
        &db,
        &target.id,
        "coolmod-ue4ss",
        Some(&second.version.id),
        true,
    )
    .await;

    // A journal from the attempt that died, naming the set that attempt
    // reserved. Its one entry is already reconciled, so recovery closes it and
    // this run proceeds -- into the same set, because the set belongs to the
    // operation rather than to the attempt.
    let main = ue4ss_dir(&root).join("CoolMod/main.lua");
    let journalled = serde_json::json!({
        "plan": { "entries": [ { "op": "keep", "path": main.to_string_lossy() } ] },
        "markers": [],
        "backups": [],
        "op_id": "aaaabbbbccccdddd",
        "stamp": "20260101-000000",
    });
    ps_db::mod_deployments::open_journal(
        &db,
        &target.id,
        "aaaabbbbccccdddd",
        &journalled.to_string(),
    )
    .await
    .unwrap();

    let options = ApplyOptions {
        running: &ps_server::services::mods::deploy::NeverRunning,
        move_strategy: MoveStrategy::Auto,
        stamp: Some("20261231-235959"),
        replace_occupants: &[],
    };
    let outcome = apply::apply(&db, &paths, &target, &options).await.unwrap();
    assert!(!outcome.mid_apply, "{:?}", outcome.failed);

    let backup_dir = outcome.backup_dir.expect("a backup set was created");
    assert!(
        Path::new(&backup_dir).ends_with("20260101-000000-aaaabbbbccccdddd"),
        "a retry continues the journalled operation's set rather than opening a \
         second one and orphaning the first: {backup_dir}"
    );
    let index: Vec<ps_server::services::mods::deploy::backup::BackupEntry> = serde_json::from_str(
        &std::fs::read_to_string(Path::new(&backup_dir).join("index.json")).unwrap(),
    )
    .unwrap();
    assert!(
        index
            .iter()
            .any(|e| Path::new(&e.original_path) == config.as_path()),
        "and the preserved file lands in it: {index:?}"
    );
}

#[tokio::test]
async fn the_hash_a_journal_intended_still_reads_as_app_written() {
    // Property 4's second clause: "app-written" means the on-disk hash matches
    // the recorded hash *or* the hash the last journal intended to write.
    // Without the journal being consulted, a file the app itself renamed into
    // place moments before the crash is indistinguishable from a file the user
    // edited, and the next apply preserves it -- refusing to update a mod,
    // permanently, on the strength of the app's own write.
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    let stored = install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[("ue4ss", "CoolMod/main.lua", b"v1")],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;
    apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();

    let deployed = ue4ss_dir(&root).join("CoolMod/main.lua");
    let library_file = LibraryPaths::route_path_in(
        Path::new(&stored.version.library_dir),
        ps_core::mods::RouteKind::Ue4ss,
        "CoolMod/main.lua",
    );

    // The crashed run: it planned v2, its rename landed v2's bytes, and the row
    // write that should have followed never ran, so the row still says v1.
    std::fs::write(&library_file, b"v2").unwrap();
    let plan = apply::plan_for(&db, &target, &format!("{}/default", target.id))
        .await
        .unwrap()
        .0;
    let journalled = serde_json::json!({
        "plan": plan,
        "markers": [],
        "backups": [],
        "op_id": "0123456789abcdef",
        "stamp": "20260913-120000",
    });
    ps_db::mod_deployments::open_journal(
        &db,
        &target.id,
        "0123456789abcdef",
        &journalled.to_string(),
    )
    .await
    .unwrap();
    std::fs::write(&deployed, b"v2").unwrap();

    // And the library has moved on again, so what is on disk matches neither the
    // row nor what is wanted: the journal is the only thing that explains it.
    std::fs::write(&library_file, b"v3").unwrap();
    let plan = apply::plan_for(&db, &target, &format!("{}/default", target.id))
        .await
        .unwrap()
        .0;
    assert!(
        plan.entries.iter().any(|e| matches!(
            e,
            PlanEntry::Replace { path, .. } if Path::new(path) == deployed.as_path()
        )),
        "the journal's intended hash makes this the app's own file, so it is \
         replaced rather than preserved: {:?}",
        plan.entries
    );
}

#[tokio::test]
async fn an_upgrade_that_changes_no_bytes_keeps_each_row_role_and_hash() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[
            ("ue4ss", "CoolMod/main.lua", b"unchanged"),
            ("ue4ss", "CoolMod/enabled.txt", b""),
        ],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;
    apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();

    // Version 2 ships the same bytes, which is what makes this a reattribution
    // rather than a replace.
    let second = install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "2.0",
        &[
            ("ue4ss", "CoolMod/main.lua", b"unchanged"),
            ("ue4ss", "CoolMod/enabled.txt", b""),
        ],
    )
    .await;
    enable(
        &db,
        &target.id,
        "coolmod-ue4ss",
        Some(&second.version.id),
        true,
    )
    .await;

    let outcome = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    let marker = ue4ss_dir(&root).join("CoolMod/enabled.txt");
    let main = ue4ss_dir(&root).join("CoolMod/main.lua");
    for file in [&marker, &main] {
        assert!(
            outcome.plan.entries.iter().any(|e| matches!(
                e,
                PlanEntry::Reattribute { path, .. } if Path::new(path) == file.as_path()
            )),
            "{file:?} is not reattributed: {:?}",
            outcome.plan.entries
        );
    }

    let rows = ps_db::mod_deployments::files_of(&db, &target.id)
        .await
        .unwrap();
    let row_for = |file: &Path| {
        rows.iter()
            .find(|r| Path::new(&r.path) == file)
            .unwrap_or_else(|| panic!("no row for {file:?}: {rows:?}"))
            .clone()
    };
    for (file, role, body) in [
        (&marker, "marker", &b""[..]),
        (&main, "file", &b"unchanged"[..]),
    ] {
        let row = row_for(file);
        assert_eq!(
            row.role, role,
            "a reattribution changes which version owns a file, not what the file is"
        );
        assert_eq!(
            row.mod_version_id.as_deref(),
            Some(second.version.id.as_str()),
            "{file:?}"
        );
        assert_eq!(
            row.hash,
            ps_server::services::mods::digest::hash_bytes(body),
            "and the hash recorded is the one the file actually has"
        );
    }
}

#[tokio::test]
async fn the_users_own_shared_markers_are_kept_before_the_app_first_writes_them() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[("ue4ss", "CoolMod/main.lua", b"main")],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;

    // The user's hand-built mods.txt, which this apply is about to rewrite.
    let mods_txt = ue4ss_dir(&root).join("mods.txt");
    common::mods::write(&mods_txt, b"; the user's own file\r\nSomeOtherMod : 1\r\n");

    let outcome = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(!outcome.mid_apply, "{:?}", outcome.failed);

    let backup_dir = outcome
        .backup_dir
        .expect("the first apply on a target snapshots the shared markers");
    let index: Vec<ps_server::services::mods::deploy::backup::BackupEntry> = serde_json::from_str(
        &std::fs::read_to_string(Path::new(&backup_dir).join("index.json")).unwrap(),
    )
    .unwrap();
    let entry = index
        .iter()
        .find(|e| Path::new(&e.original_path) == mods_txt.as_path())
        .unwrap_or_else(|| panic!("mods.txt is not in the snapshot: {index:?}"));
    assert_eq!(
        std::fs::read_to_string(Path::new(&backup_dir).join(&entry.backup_key)).unwrap(),
        "; the user's own file\r\nSomeOtherMod : 1\r\n",
        "the snapshot is the file as the user had it, not as the codec rendered it"
    );
    assert!(mods_txt.is_file(), "and the marker itself stays where it is");
    assert_eq!(
        index.len(),
        1,
        "every marker the layout has, and nothing else: {index:?}"
    );

    // Only before the app first writes them. A second apply has rows for the
    // markers, so it must not snapshot the app's own rendering over the user's.
    let again = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(
        again.backup_dir.is_none(),
        "a later apply with nothing to preserve opens no set at all: {:?}",
        again.backup_dir
    );
    let index: Vec<ps_server::services::mods::deploy::backup::BackupEntry> = serde_json::from_str(
        &std::fs::read_to_string(Path::new(&backup_dir).join("index.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        index.len(),
        1,
        "and the snapshot is taken exactly once: {index:?}"
    );
}

#[tokio::test]
async fn a_first_apply_with_no_marker_of_the_users_opens_no_backup_set() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[("ue4ss", "CoolMod/main.lua", b"main")],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;

    let outcome = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(
        outcome.backup_dir.is_none(),
        "a set holding nothing would offer the user a restore over nothing: {:?}",
        outcome.backup_dir
    );
}

fn beside(path: &Path) -> std::path::PathBuf {
    let mut name = path.file_name().unwrap().to_os_string();
    name.push(".new");
    path.with_file_name(name)
}

async fn copy_row(
    db: &ps_db::SqlxSqliteDriver,
    target_id: &str,
    path: &Path,
) -> Option<ps_db::mod_deployments::DeploymentFile> {
    ps_db::mod_deployments::files_of(db, target_id)
        .await
        .unwrap()
        .into_iter()
        .find(|r| Path::new(&r.path) == path)
}

/// Deploys `defaults`, lets the user edit it, and enables a version shipping
/// `body` for the same file. Returns the deployed path and the new version id.
async fn an_edited_file_meets_a_new_version(
    db: &ps_db::SqlxSqliteDriver,
    paths: &LibraryPaths,
    target: &ps_db::mod_targets::ModTarget,
    root: &Path,
    body: &[u8],
) -> (std::path::PathBuf, String) {
    install_fixture_mod(
        db,
        paths,
        "coolmod-ue4ss",
        "1.0",
        &[("ue4ss", "CoolMod/config.lua", b"defaults")],
    )
    .await;
    enable(db, &target.id, "coolmod-ue4ss", None, true).await;
    apply::apply(db, paths, target, &ApplyOptions::default())
        .await
        .unwrap();
    let deployed = ue4ss_dir(root).join("CoolMod/config.lua");
    std::fs::write(&deployed, b"the user's settings").unwrap();
    let version = upgrade(db, paths, target, "2.0", body).await;
    (deployed, version)
}

async fn upgrade(
    db: &ps_db::SqlxSqliteDriver,
    paths: &LibraryPaths,
    target: &ps_db::mod_targets::ModTarget,
    version: &str,
    body: &[u8],
) -> String {
    let stored = install_fixture_mod(
        db,
        paths,
        "coolmod-ue4ss",
        version,
        &[("ue4ss", "CoolMod/config.lua", body)],
    )
    .await;
    enable(
        db,
        &target.id,
        "coolmod-ue4ss",
        Some(&stored.version.id),
        true,
    )
    .await;
    stored.version.id
}

#[tokio::test]
async fn a_preserved_file_gets_a_recorded_new_copy_beside_it() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    let (deployed, version) =
        an_edited_file_meets_a_new_version(&db, &paths, &target, &root, b"new defaults").await;

    let outcome = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(!outcome.mid_apply && outcome.error.is_none(), "{outcome:?}");

    let copy = beside(&deployed);
    assert_eq!(
        std::fs::read_to_string(&deployed).unwrap(),
        "the user's settings"
    );
    assert_eq!(std::fs::read_to_string(&copy).unwrap(), "new defaults");
    let row = copy_row(&db, &target.id, &copy)
        .await
        .expect("the copy is recorded");
    assert_eq!(row.role, "preserved_copy");
    assert_eq!(
        row.hash,
        ps_server::services::mods::digest::hash_bytes(b"new defaults")
    );
    assert_eq!(row.mod_version_id.as_deref(), Some(version.as_str()));
    assert_eq!(row.rel_path.as_deref(), Some("CoolMod/config.lua.new"));
    assert!(
        outcome
            .new_copies
            .iter()
            .any(|p| Path::new(p) == copy.as_path()),
        "{:?}",
        outcome.new_copies
    );
    assert!(outcome.skipped_new_copies.is_empty());
    assert!(ps_db::mod_deployments::journal_of(&db, &target.id)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn a_users_own_new_file_is_never_overwritten() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    let (deployed, _) =
        an_edited_file_meets_a_new_version(&db, &paths, &target, &root, b"new defaults").await;
    let copy = beside(&deployed);
    std::fs::write(&copy, b"the user's own spare copy").unwrap();

    let outcome = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(!outcome.mid_apply && outcome.error.is_none(), "{outcome:?}");
    assert_eq!(
        std::fs::read_to_string(&copy).unwrap(),
        "the user's own spare copy",
        "a path no row accounts for is not the deployer's to overwrite"
    );
    assert!(
        outcome
            .skipped_new_copies
            .iter()
            .any(|p| Path::new(p) == copy.as_path()),
        "{:?}",
        outcome.skipped_new_copies
    );
    assert!(outcome.new_copies.is_empty(), "{:?}", outcome.new_copies);
    assert!(copy_row(&db, &target.id, &copy).await.is_none());

    std::fs::write(&deployed, b"new defaults").unwrap();
    apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(&copy).unwrap(),
        "the user's own spare copy",
        "nor the deployer's to delete once the preserve resolves"
    );
}

#[tokio::test]
async fn an_app_owned_new_copy_is_refreshed_by_the_next_version() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    let (deployed, _) =
        an_edited_file_meets_a_new_version(&db, &paths, &target, &root, b"new defaults").await;
    apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();

    let third = upgrade(&db, &paths, &target, "3.0", b"newest defaults").await;
    let outcome = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(!outcome.mid_apply && outcome.error.is_none(), "{outcome:?}");

    let copy = beside(&deployed);
    assert_eq!(
        std::fs::read_to_string(&deployed).unwrap(),
        "the user's settings"
    );
    assert_eq!(std::fs::read_to_string(&copy).unwrap(), "newest defaults");
    let row = copy_row(&db, &target.id, &copy).await.unwrap();
    assert_eq!(
        row.hash,
        ps_server::services::mods::digest::hash_bytes(b"newest defaults")
    );
    assert_eq!(row.mod_version_id.as_deref(), Some(third.as_str()));
    assert!(outcome.skipped_new_copies.is_empty());
}

#[tokio::test]
async fn reverting_the_edit_sweeps_the_new_copy() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    let (deployed, _) =
        an_edited_file_meets_a_new_version(&db, &paths, &target, &root, b"new defaults").await;
    apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    let copy = beside(&deployed);
    assert!(copy.is_file());

    std::fs::write(&deployed, b"defaults").unwrap();
    let outcome = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(!outcome.mid_apply && outcome.error.is_none(), "{outcome:?}");
    assert_eq!(std::fs::read_to_string(&deployed).unwrap(), "new defaults");
    assert!(!copy.exists(), "the app's own copy is swept");
    assert!(copy_row(&db, &target.id, &copy).await.is_none());
    assert!(outcome.new_copies.is_empty());
}

#[tokio::test]
async fn a_new_copy_the_user_edited_is_released_not_deleted() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    let (deployed, _) =
        an_edited_file_meets_a_new_version(&db, &paths, &target, &root, b"new defaults").await;
    apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    let copy = beside(&deployed);
    std::fs::write(&copy, b"the user's merge").unwrap();

    std::fs::write(&deployed, b"defaults").unwrap();
    let outcome = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(!outcome.mid_apply && outcome.error.is_none(), "{outcome:?}");
    assert_eq!(
        std::fs::read_to_string(&copy).unwrap(),
        "the user's merge",
        "a copy the user changed is theirs now"
    );
    assert!(
        copy_row(&db, &target.id, &copy).await.is_none(),
        "and the deployer no longer claims it"
    );
}

#[tokio::test]
async fn a_crash_after_writing_a_new_copy_is_recorded_by_recovery() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    let (deployed, version) =
        an_edited_file_meets_a_new_version(&db, &paths, &target, &root, b"new defaults").await;

    let copy = beside(&deployed);
    let hash = ps_server::services::mods::digest::hash_bytes(b"new defaults");
    let plan = apply::plan_for(&db, &target, &format!("{}/default", target.id))
        .await
        .unwrap()
        .0;
    let journalled = serde_json::json!({
        "plan": plan,
        "markers": [],
        "backups": [],
        "new_copies": [{
            "path": copy.to_string_lossy(),
            "hash": hash,
            "mod_version_id": version,
            "rel_path": "CoolMod/config.lua.new",
        }],
        "op_id": "0123456789abcdef",
        "stamp": "20260913-120000",
    });
    std::fs::write(&copy, b"new defaults").unwrap();
    ps_db::mod_deployments::open_journal(
        &db,
        &target.id,
        "0123456789abcdef",
        &journalled.to_string(),
    )
    .await
    .unwrap();

    let outcome = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(!outcome.mid_apply && outcome.error.is_none(), "{outcome:?}");
    assert!(ps_db::mod_deployments::journal_of(&db, &target.id)
        .await
        .unwrap()
        .is_none());
    let row = copy_row(&db, &target.id, &copy)
        .await
        .expect("recovery recorded the copy the crashed run wrote");
    assert_eq!(row.role, "preserved_copy");
    assert_eq!(row.hash, hash);
    assert!(
        outcome.skipped_new_copies.is_empty(),
        "{:?}",
        outcome.skipped_new_copies
    );
}

fn palmodsettings_ini(root: &Path) -> std::path::PathBuf {
    root.join("Mods/PalModSettings.ini")
}

async fn two_workshop_packages(
    db: &ps_db::SqlxSqliteDriver,
    paths: &LibraryPaths,
) {
    install_fixture_mod(
        db,
        paths,
        "coolpack",
        "1.0",
        &[("workshop", "CoolPack/Info.json", b"{\"PackageName\":\"CoolPack\"}")],
    )
    .await;
    install_fixture_mod(
        db,
        paths,
        "oldpack",
        "1.0",
        &[("workshop", "OldPack/Info.json", b"{\"PackageName\":\"OldPack\"}")],
    )
    .await;
}

#[tokio::test]
async fn an_apply_keeps_the_active_line_of_a_package_the_library_does_not_manage() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    two_workshop_packages(&db, &paths).await;
    enable(&db, &target.id, "coolpack", None, true).await;
    enable(&db, &target.id, "oldpack", None, false).await;
    let ini = palmodsettings_ini(&root);
    common::mods::write(
        &ini,
        b"[PalModSettings]\nbGlobalEnableMod=False\nActiveModList=SteamThing\nActiveModList=OldPack\n",
    );

    let outcome = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(
        !outcome.mid_apply && outcome.error.is_none(),
        "{:?} {:?}",
        outcome.failed,
        outcome.error
    );
    let active = |ini: &Path| {
        ps_core::mods::PalModSettings::parse(&std::fs::read_to_string(ini).unwrap()).active_mods
    };
    assert_eq!(
        active(&ini),
        vec!["SteamThing".to_string(), "CoolPack".to_string()],
        "a managed package is added, a managed disabled one removed, and a Steam one kept"
    );

    enable(&db, &target.id, "coolpack", None, false).await;
    let outcome = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(outcome.error.is_none(), "{:?}", outcome.error);
    assert_eq!(active(&ini), vec!["SteamThing".to_string()]);
}

#[tokio::test]
async fn an_apply_rewrites_a_utf16_palmodsettings_in_utf16() {
    use ps_server::services::mods::ini_text::{decode, encode, IniEncoding};
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    two_workshop_packages(&db, &paths).await;
    enable(&db, &target.id, "coolpack", None, true).await;
    let ini = palmodsettings_ini(&root);
    common::mods::write(
        &ini,
        &encode(
            "[PalModSettings]\r\nbGlobalEnableMod=True\r\nWorkshopRootDir=C:\\Users\\José\\steam\r\nSomeFutureKey=17\r\nActiveModList=SteamThing\r\n",
            IniEncoding::Utf16Le,
        ),
    );

    let outcome = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(outcome.error.is_none(), "{:?}", outcome.error);
    let written = std::fs::read(&ini).unwrap();
    assert_eq!(&written[..2], &[0xFF, 0xFE]);
    let decoded = decode(&written).unwrap();
    assert_eq!(decoded.encoding, IniEncoding::Utf16Le);
    assert!(decoded.text.contains("SomeFutureKey=17"), "{}", decoded.text);
    assert!(
        decoded.text.contains("WorkshopRootDir=C:\\Users\\José\\steam"),
        "{}",
        decoded.text
    );
    assert_eq!(
        ps_core::mods::PalModSettings::parse(&decoded.text).active_mods,
        vec!["SteamThing".to_string(), "CoolPack".to_string()]
    );

    let again = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(
        !again.mid_apply && again.error.is_none(),
        "{:?} {:?}",
        again.failed,
        again.error
    );
    assert_eq!(std::fs::read(&ini).unwrap(), written);
    assert!(ps_db::mod_deployments::journal_of(&db, &target.id)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn an_undecodable_palmodsettings_fails_the_apply_and_is_left_alone() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    two_workshop_packages(&db, &paths).await;
    enable(&db, &target.id, "coolpack", None, true).await;
    let ini = palmodsettings_ini(&root);
    common::mods::write(&ini, &[0xC3, 0x28, b'A']);

    let result = apply::apply(&db, &paths, &target, &ApplyOptions::default()).await;
    match result {
        Err(apply::ApplyError::Io(_)) => {}
        Ok(outcome) => assert!(
            matches!(outcome.error, Some(apply::ApplyError::Io(_))),
            "{:?}",
            outcome.error
        ),
        Err(other) => panic!("expected an io failure, got {other:?}"),
    }
    assert_eq!(std::fs::read(&ini).unwrap(), [0xC3, 0x28, b'A']);
}

async fn server_target(
    db: &ps_db::SqlxSqliteDriver,
    root: &Path,
    platform: &str,
) -> ps_db::mod_targets::ModTarget {
    std::fs::create_dir_all(root).unwrap();
    let dir = |name: &str| root.join(name).to_string_lossy().into_owned();
    let overrides = serde_json::json!({
        "ue4ss_mods_dir": dir("mods"),
        "logicmods_dir": dir("logicmods"),
        "nativemods_dir": dir("nativemods"),
        "paks_mods_dir": dir("paks"),
    });
    let target = ps_db::mod_targets::upsert(
        db,
        &ps_db::mod_targets::NewModTarget {
            id: "server-1".to_string(),
            kind: "server".to_string(),
            name: "Server".to_string(),
            root_path: root.to_string_lossy().into_owned(),
            platform: platform.to_string(),
            ue4ss_mode: "none".to_string(),
            layout_overrides: overrides.to_string(),
            detected: "{}".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    default_profile(db, &target.id).await;
    target
}

async fn apply_a_ue4ss_mod(
    db: &ps_db::SqlxSqliteDriver,
    paths: &LibraryPaths,
    target: &ps_db::mod_targets::ModTarget,
) -> apply::ApplyOutcome {
    install_fixture_mod(
        db,
        paths,
        "coolmod-ue4ss",
        "1.0",
        &[("ue4ss", "CoolMod/Scripts/main.lua", b"print('hi')")],
    )
    .await;
    enable(db, &target.id, "coolmod-ue4ss", None, true).await;
    let outcome = apply::apply(db, paths, target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(!outcome.mid_apply && outcome.error.is_none(), "{outcome:?}");
    outcome
}

#[tokio::test]
async fn a_docker_server_target_writes_no_palmodsettings() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("servers/alpha");
    let target = server_target(&db, &root, "linux").await;
    apply_a_ue4ss_mod(&db, &paths, &target).await;

    assert!(root.join("mods/CoolMod/Scripts/main.lua").is_file());
    assert!(root.join("mods/mods.txt").is_file());
    assert!(
        !palmodsettings_ini(&root).exists(),
        "no container reads an ini written here"
    );
    let rows = ps_db::mod_deployments::files_of(&db, &target.id)
        .await
        .unwrap();
    assert!(
        rows.iter().all(|r| !r.path.ends_with("PalModSettings.ini")),
        "{rows:?}"
    );
}

#[tokio::test]
async fn a_native_server_target_still_writes_palmodsettings() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("PalServer");
    let target = server_target(&db, &root, "win64").await;
    apply_a_ue4ss_mod(&db, &paths, &target).await;
    assert!(palmodsettings_ini(&root).is_file());
}

#[tokio::test]
async fn a_docker_targets_stale_palmodsettings_marker_is_released() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("servers/alpha");
    let target = server_target(&db, &root, "linux").await;
    let ini = palmodsettings_ini(&root);
    let stale_row = |hash: String| ps_db::mod_deployments::NewDeploymentFile {
        target_id: target.id.clone(),
        path: ini.to_string_lossy().into_owned(),
        mod_version_id: None,
        hash,
        role: "shared_marker".to_string(),
        rel_path: None,
    };
    let written = b"[PalModSettings]\nbGlobalEnableMod=True\n";
    common::mods::write(&ini, written);
    ps_db::mod_deployments::record(
        &db,
        &[stale_row(ps_server::services::mods::digest::hash_bytes(written))],
    )
    .await
    .unwrap();

    apply_a_ue4ss_mod(&db, &paths, &target).await;
    assert!(!ini.exists(), "the app's own unread ini is removed");
    let marker_rows = ps_db::mod_deployments::shared_markers(&db, &target.id)
        .await
        .unwrap();
    assert!(
        marker_rows
            .iter()
            .all(|r| !r.path.ends_with("PalModSettings.ini")),
        "{marker_rows:?}"
    );

    common::mods::write(&ini, b"[PalModSettings]\nbGlobalEnableMod=False\n");
    ps_db::mod_deployments::record(
        &db,
        &[stale_row(ps_server::services::mods::digest::hash_bytes(written))],
    )
    .await
    .unwrap();
    let outcome = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(outcome.error.is_none(), "{outcome:?}");
    assert_eq!(
        std::fs::read(&ini).unwrap(),
        b"[PalModSettings]\nbGlobalEnableMod=False\n",
        "an ini somebody else changed is left alone"
    );
    assert!(ps_db::mod_deployments::shared_markers(&db, &target.id)
        .await
        .unwrap()
        .iter()
        .all(|r| !r.path.ends_with("PalModSettings.ini")));
}

#[tokio::test]
async fn a_mods_txt_the_layout_no_longer_writes_keeps_its_file_and_loses_its_row() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    apply_a_ue4ss_mod(&db, &paths, &target).await;
    let old = ue4ss_dir(&root).join("mods.txt");
    let before = std::fs::read(&old).unwrap();

    let moved_dir = dir.path().join("Elsewhere/Mods");
    ps_db::mod_targets::set_layout_overrides(
        &db,
        &target.id,
        &serde_json::json!({ "ue4ss_mods_dir": moved_dir.to_string_lossy() }).to_string(),
    )
    .await
    .unwrap();
    let moved_target = ps_db::mod_targets::get(&db, &target.id)
        .await
        .unwrap()
        .unwrap();
    let outcome = apply::apply(&db, &paths, &moved_target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(!outcome.mid_apply && outcome.error.is_none(), "{outcome:?}");

    assert_eq!(
        std::fs::read(&old).unwrap(),
        before,
        "a mods.txt holds lines the app does not manage, so it is never removed"
    );
    assert!(moved_dir.join("mods.txt").is_file());
    let markers = ps_db::mod_deployments::shared_markers(&db, &target.id)
        .await
        .unwrap();
    assert!(
        markers.iter().all(|r| Path::new(&r.path) != old.as_path()),
        "{markers:?}"
    );
}

#[tokio::test]
async fn a_users_marker_at_a_moved_path_is_snapshotted_before_the_first_write_there() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    apply_a_ue4ss_mod(&db, &paths, &target).await;

    let moved_dir = dir.path().join("Elsewhere/Mods");
    let moved_txt = moved_dir.join("mods.txt");
    let users = b"; the user's own file\r\nSomeOtherMod : 1\r\n";
    common::mods::write(&moved_txt, users);
    ps_db::mod_targets::set_layout_overrides(
        &db,
        &target.id,
        &serde_json::json!({ "ue4ss_mods_dir": moved_dir.to_string_lossy() }).to_string(),
    )
    .await
    .unwrap();
    let moved_target = ps_db::mod_targets::get(&db, &target.id)
        .await
        .unwrap()
        .unwrap();
    let outcome = apply::apply(&db, &paths, &moved_target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(!outcome.mid_apply && outcome.error.is_none(), "{outcome:?}");

    let backup_dir = outcome
        .backup_dir
        .expect("the file already at the new marker path is snapshotted");
    let index: Vec<ps_server::services::mods::deploy::backup::BackupEntry> = serde_json::from_str(
        &std::fs::read_to_string(Path::new(&backup_dir).join("index.json")).unwrap(),
    )
    .unwrap();
    let entry = index
        .iter()
        .find(|e| Path::new(&e.original_path) == moved_txt.as_path())
        .unwrap_or_else(|| panic!("the moved mods.txt is not in the snapshot: {index:?}"));
    assert_eq!(
        std::fs::read(Path::new(&backup_dir).join(&entry.backup_key)).unwrap(),
        users
    );
}

#[tokio::test]
async fn a_new_copy_edited_beside_a_still_preserved_file_is_released() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    let (deployed, second) =
        an_edited_file_meets_a_new_version(&db, &paths, &target, &root, b"new defaults").await;
    apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    let copy = beside(&deployed);
    std::fs::write(&copy, b"the user's merge").unwrap();

    upgrade(&db, &paths, &target, "3.0", b"newest defaults").await;
    let outcome = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(!outcome.mid_apply && outcome.error.is_none(), "{outcome:?}");
    assert_eq!(
        std::fs::read_to_string(&deployed).unwrap(),
        "the user's settings"
    );
    assert_eq!(std::fs::read_to_string(&copy).unwrap(), "the user's merge");
    assert!(
        outcome
            .skipped_new_copies
            .iter()
            .any(|p| Path::new(p) == copy.as_path()),
        "{:?}",
        outcome.skipped_new_copies
    );
    assert!(
        copy_row(&db, &target.id, &copy).await.is_none(),
        "a copy the user took over is not the app's to keep a row for"
    );

    let usage = ps_db::mod_library::version_usage(&db, &second)
        .await
        .unwrap();
    assert!(!usage.is_in_use(), "{usage:?}");
    ps_server::services::mods::library::delete_version(&db, &second)
        .await
        .unwrap();
    assert_eq!(std::fs::read_to_string(&copy).unwrap(), "the user's merge");
}

#[tokio::test]
async fn a_version_shipping_a_real_new_file_replaces_the_apps_copy() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    let (deployed, _) =
        an_edited_file_meets_a_new_version(&db, &paths, &target, &root, b"new defaults").await;
    apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    let copy = beside(&deployed);
    assert!(copy_row(&db, &target.id, &copy).await.is_some());

    let third = install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "3.0",
        &[
            ("ue4ss", "CoolMod/config.lua", b"newest defaults"),
            ("ue4ss", "CoolMod/config.lua.new", b"shipped by the mod"),
        ],
    )
    .await;
    enable(
        &db,
        &target.id,
        "coolmod-ue4ss",
        Some(&third.version.id),
        true,
    )
    .await;
    let outcome = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap_or_else(|e| panic!("the app's own copy is not an occupant: {e}"));
    assert!(!outcome.mid_apply && outcome.error.is_none(), "{outcome:?}");
    assert_eq!(
        std::fs::read_to_string(&deployed).unwrap(),
        "the user's settings"
    );
    assert_eq!(std::fs::read_to_string(&copy).unwrap(), "shipped by the mod");
    let row = copy_row(&db, &target.id, &copy).await.unwrap();
    assert_eq!(row.role, "file");
    assert_eq!(row.mod_version_id.as_deref(), Some(third.version.id.as_str()));
    for listed in [&outcome.new_copies, &outcome.skipped_new_copies] {
        assert!(
            listed.iter().all(|p| Path::new(p) != copy.as_path()),
            "a file the mod ships is neither a copy nor a skipped copy: {listed:?}"
        );
    }
}

#[tokio::test]
async fn an_apply_rewrites_a_utf16_mods_txt_in_utf16() {
    use ps_server::services::mods::ini_text::{decode, encode, IniEncoding};
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    let mods_txt = ue4ss_dir(&root).join("mods.txt");
    common::mods::write(
        &mods_txt,
        &encode("; José's list\r\nKeybinds : 1\r\n", IniEncoding::Utf16Le),
    );

    apply_a_ue4ss_mod(&db, &paths, &target).await;
    let written = std::fs::read(&mods_txt).unwrap();
    let decoded = decode(&written).unwrap();
    assert_eq!(decoded.encoding, IniEncoding::Utf16Le);
    for line in ["; José's list", "Keybinds : 1", "CoolMod : 1"] {
        assert!(decoded.text.contains(line), "{line} in {}", decoded.text);
    }

    let again = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(!again.mid_apply && again.error.is_none(), "{again:?}");
    assert_eq!(std::fs::read(&mods_txt).unwrap(), written);
    assert!(ps_db::mod_deployments::journal_of(&db, &target.id)
        .await
        .unwrap()
        .is_none());
}

async fn adopt_subscribed(
    db: &ps_db::SqlxSqliteDriver,
    paths: &LibraryPaths,
    target: &ps_db::mod_targets::ModTarget,
    package: &str,
) -> ps_server::services::mods::adopt::Adopted {
    use ps_server::services::mods::{adopt, scan};
    let report = scan::scan(db, target).await.unwrap();
    let candidate = report
        .candidates
        .iter()
        .find(|c| c.name == package && c.source == scan::CandidateSource::SteamSubscribed)
        .unwrap_or_else(|| panic!("{package} is not offered: {:?}", report.candidates));
    adopt::adopt(db, paths, target, candidate).await.unwrap()
}

async fn apply_cleanly(
    db: &ps_db::SqlxSqliteDriver,
    paths: &LibraryPaths,
    target: &ps_db::mod_targets::ModTarget,
) {
    let outcome = apply::apply(db, paths, target, &ApplyOptions::default())
        .await
        .unwrap();
    assert!(
        !outcome.mid_apply && outcome.error.is_none(),
        "{:?} {:?}",
        outcome.failed,
        outcome.error
    );
}

fn active_lines(root: &Path) -> Vec<String> {
    ps_core::mods::PalModSettings::parse(
        &std::fs::read_to_string(palmodsettings_ini(root)).unwrap(),
    )
    .active_mods
}

#[tokio::test]
async fn a_subscribed_package_toggles_only_its_own_active_line_and_no_steam_file() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(&dir.path().join("app"));
    let (target, root, content) = steam_client(&db, &dir.path().join("SteamLibrary")).await;
    let steam_before = tree_snapshot(&content);
    let pack = adopt_subscribed(&db, &paths, &target, "SteamPack").await;
    assert!(pack.enabled);

    apply_cleanly(&db, &paths, &target).await;
    assert_eq!(active_lines(&root), vec!["SteamThing", "SteamPack"]);

    enable(&db, &target.id, &pack.mod_id, None, false).await;
    apply_cleanly(&db, &paths, &target).await;
    assert_eq!(
        active_lines(&root),
        vec!["SteamThing"],
        "only its own line goes; the unadopted package's line stays"
    );
    assert_eq!(tree_snapshot(&content), steam_before);

    enable(&db, &target.id, &pack.mod_id, None, true).await;
    apply_cleanly(&db, &paths, &target).await;
    apply_cleanly(&db, &paths, &target).await;
    assert_eq!(
        active_lines(&root),
        vec!["SteamThing", "SteamPack"],
        "turned back on, listed exactly once"
    );

    assert_eq!(
        tree_snapshot(&content),
        steam_before,
        "no apply writes, moves or removes anything under Steam's directory"
    );
    let rows = ps_db::mod_deployments::files_of(&db, &target.id)
        .await
        .unwrap();
    assert!(
        rows.iter().all(|row| row.role == "shared_marker"
            && !Path::new(&row.path).starts_with(&content)),
        "{rows:?}"
    );
    let mut indexes = 0;
    for index in walkdir::WalkDir::new(paths.backups_dir(&target.id))
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name() == "index.json")
    {
        let entries: Vec<ps_server::services::mods::deploy::backup::BackupEntry> =
            serde_json::from_str(&std::fs::read_to_string(index.path()).unwrap()).unwrap();
        indexes += 1;
        let originals: Vec<&str> = entries.iter().map(|e| e.original_path.as_str()).collect();
        assert!(
            originals
                .iter()
                .all(|original| !Path::new(original).starts_with(&content)),
            "no Steam file is ever backed up: {originals:?}"
        );
    }
    assert!(indexes > 0, "the loop above read no backup index");
}

#[tokio::test]
async fn a_removed_subscribed_package_loses_its_line_once_and_then_steam_owns_it_again() {
    use ps_server::services::mods::library;
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(&dir.path().join("app"));
    let (target, root, content) = steam_client(&db, &dir.path().join("SteamLibrary")).await;
    let steam_before = tree_snapshot(&content);
    let pack = adopt_subscribed(&db, &paths, &target, "SteamPack").await;
    apply_cleanly(&db, &paths, &target).await;

    library::release_workshop_packages(&db, &pack.mod_id, std::slice::from_ref(&target.id))
        .await
        .unwrap();
    library::delete_mod(&db, &paths, &pack.mod_id).await.unwrap();
    apply_cleanly(&db, &paths, &target).await;
    assert_eq!(active_lines(&root), vec!["SteamThing"]);
    assert!(library::released_packages(&db, &target.id)
        .await
        .unwrap()
        .is_empty());

    common::mods::write(
        &palmodsettings_ini(&root),
        b"[PalModSettings]\nbGlobalEnableMod=True\nActiveModList=SteamThing\nActiveModList=SteamPack\n",
    );
    apply_cleanly(&db, &paths, &target).await;
    assert_eq!(
        active_lines(&root),
        vec!["SteamThing", "SteamPack"],
        "once its line is gone, a package the library no longer holds is Steam's again"
    );
    assert_eq!(tree_snapshot(&content), steam_before);
}

#[tokio::test]
async fn a_subscribed_package_another_target_holds_is_not_managed_here() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(&dir.path().join("app"));
    let (target, root, _content) = steam_client(&db, &dir.path().join("SteamLibrary")).await;
    let pack = adopt_subscribed(&db, &paths, &target, "SteamPack").await;

    let other_root = dir.path().join("Other");
    let other = ps_db::mod_targets::upsert(
        &db,
        &ps_db::mod_targets::NewModTarget {
            id: "client-other".to_string(),
            kind: "client".to_string(),
            name: "Other".to_string(),
            root_path: other_root.to_string_lossy().into_owned(),
            platform: "win64".to_string(),
            ue4ss_mode: "standard".to_string(),
            layout_overrides: "{}".to_string(),
            detected: "{}".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    default_profile(&db, &other.id).await;
    common::mods::write(
        &palmodsettings_ini(&other_root),
        b"[PalModSettings]\nbGlobalEnableMod=True\nActiveModList=SteamPack\n",
    );

    enable(&db, &target.id, &pack.mod_id, None, false).await;
    apply_cleanly(&db, &paths, &other).await;
    assert_eq!(
        active_lines(&other_root),
        vec!["SteamPack"],
        "Steam enabled it on a target whose profiles never took it over"
    );
    apply_cleanly(&db, &paths, &target).await;
    assert!(active_lines(&root).iter().all(|line| line != "SteamPack"));
}

async fn other_client(
    db: &ps_db::SqlxSqliteDriver,
    root: &Path,
) -> ps_db::mod_targets::ModTarget {
    let target = ps_db::mod_targets::upsert(
        db,
        &ps_db::mod_targets::NewModTarget {
            id: "client-other".to_string(),
            kind: "client".to_string(),
            name: "Other".to_string(),
            root_path: root.to_string_lossy().into_owned(),
            platform: "win64".to_string(),
            ue4ss_mode: "standard".to_string(),
            layout_overrides: "{}".to_string(),
            detected: "{}".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    default_profile(db, &target.id).await;
    target
}

#[tokio::test]
async fn an_installed_workshop_package_on_one_target_leaves_another_targets_steam_line_alone() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(&dir.path().join("app"));
    let root = dir.path().join("A");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolpack-workshop",
        "1.0",
        &[("workshop", "CoolPack/Info.json", br#"{"PackageName":"CoolPack"}"#)],
    )
    .await;
    enable(&db, &target.id, "coolpack-workshop", None, true).await;
    let other_root = dir.path().join("B");
    let other = other_client(&db, &other_root).await;
    common::mods::write(
        &palmodsettings_ini(&other_root),
        b"[PalModSettings]\nbGlobalEnableMod=True\nActiveModList=CoolPack\n",
    );

    apply_cleanly(&db, &paths, &target).await;
    assert_eq!(active_lines(&root), vec!["CoolPack"]);
    apply_cleanly(&db, &paths, &other).await;
    assert_eq!(
        active_lines(&other_root),
        vec!["CoolPack"],
        "B never took the package over, so its Steam-enabled line is not A's to strip"
    );
}

#[tokio::test]
async fn a_release_survives_a_failed_apply_and_the_next_clean_apply_uses_it() {
    use ps_server::services::mods::library;
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(&dir.path().join("app"));
    let (target, root, content) = steam_client(&db, &dir.path().join("SteamLibrary")).await;
    let steam_before = tree_snapshot(&content);
    let pack = adopt_subscribed(&db, &paths, &target, "SteamPack").await;
    apply_cleanly(&db, &paths, &target).await;
    library::release_workshop_packages(&db, &pack.mod_id, std::slice::from_ref(&target.id))
        .await
        .unwrap();
    library::delete_mod(&db, &paths, &pack.mod_id).await.unwrap();

    common::mods::write(&palmodsettings_ini(&root), &[0xC3, 0x28, b'A']);
    let failed = apply::apply(&db, &paths, &target, &ApplyOptions::default()).await;
    assert!(
        failed.is_err() || failed.as_ref().is_ok_and(|outcome| outcome.error.is_some()),
        "an undecodable settings file fails the apply"
    );
    assert_eq!(
        library::released_packages(&db, &target.id).await.unwrap(),
        vec!["SteamPack"]
    );

    common::mods::write(
        &palmodsettings_ini(&root),
        b"[PalModSettings]\nbGlobalEnableMod=True\nActiveModList=SteamThing\nActiveModList=SteamPack\n",
    );
    apply_cleanly(&db, &paths, &target).await;
    assert_eq!(active_lines(&root), vec!["SteamThing"]);
    assert!(library::released_packages(&db, &target.id)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(tree_snapshot(&content), steam_before);
}

#[tokio::test]
async fn each_holding_target_clears_only_its_own_release() {
    use ps_server::services::mods::library;
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(&dir.path().join("app"));
    let (target, _root, _content) = steam_client(&db, &dir.path().join("SteamLibrary")).await;
    let pack = adopt_subscribed(&db, &paths, &target, "SteamPack").await;
    let other_root = dir.path().join("Other");
    let other = other_client(&db, &other_root).await;
    common::mods::write(
        &palmodsettings_ini(&other_root),
        b"[PalModSettings]\nbGlobalEnableMod=True\nActiveModList=SteamPack\n",
    );
    library::release_workshop_packages(&db, &pack.mod_id, &[target.id.clone(), other.id.clone()])
        .await
        .unwrap();
    library::delete_mod(&db, &paths, &pack.mod_id).await.unwrap();

    apply_cleanly(&db, &paths, &target).await;
    assert!(library::released_packages(&db, &target.id)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        library::released_packages(&db, &other.id).await.unwrap(),
        vec!["SteamPack"],
        "another target's apply does not consume this target's release"
    );
    apply_cleanly(&db, &paths, &other).await;
    assert!(active_lines(&other_root).is_empty());
    assert!(library::released_packages(&db, &other.id)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn a_release_recorded_after_an_apply_planned_survives_that_apply() {
    use ps_server::services::mods::library;
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(&dir.path().join("app"));
    let target = client_target(&db, &dir.path().join("Palworld")).await;
    default_profile(&db, &target.id).await;
    for (mod_id, rel) in [("xpack-workshop", "XPack/Info.json"), ("ypack-workshop", "YPack/Info.json")] {
        install_fixture_mod(&db, &paths, mod_id, "1.0", &[("workshop", rel, b"{}")]).await;
    }
    library::release_workshop_packages(&db, "xpack-workshop", std::slice::from_ref(&target.id))
        .await
        .unwrap();
    let planned = library::released_packages(&db, &target.id).await.unwrap();
    assert_eq!(planned, vec!["XPack"]);

    library::release_workshop_packages(&db, "ypack-workshop", std::slice::from_ref(&target.id))
        .await
        .unwrap();
    library::forget_released(&db, &target.id, &planned)
        .await
        .unwrap();
    assert_eq!(
        library::released_packages(&db, &target.id).await.unwrap(),
        vec!["YPack"]
    );
}

#[tokio::test]
async fn rows_recorded_under_another_separator_spelling_are_not_recorded_twice() {
    let (db, dir) = db_and_dir().await;
    let paths = LibraryPaths::new(dir.path());
    let root = dir.path().join("Palworld");
    let target = client_target(&db, &root).await;
    default_profile(&db, &target.id).await;
    install_fixture_mod(
        &db,
        &paths,
        "coolmod-ue4ss",
        "1.0",
        &[("ue4ss", "CoolMod/Scripts/main.lua", b"print('hi')")],
    )
    .await;
    enable(&db, &target.id, "coolmod-ue4ss", None, true).await;
    apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();
    let before = ps_db::mod_deployments::files_of(&db, &target.id)
        .await
        .unwrap()
        .len();
    ps_db::DbDriver::execute(
        &db,
        "UPDATE deployment_files SET path = REPLACE(path, char(92), '/') WHERE target_id = ?1",
        &[target.id.as_str().into()],
    )
    .await
    .unwrap();

    let outcome = apply::apply(&db, &paths, &target, &ApplyOptions::default())
        .await
        .unwrap();

    assert!(!outcome.mid_apply, "{:?}", outcome.failed);
    let rows = ps_db::mod_deployments::files_of(&db, &target.id)
        .await
        .unwrap();
    assert_eq!(rows.len(), before, "{rows:?}");
}
