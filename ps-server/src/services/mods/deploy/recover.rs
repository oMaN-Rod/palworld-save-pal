use std::path::Path;

use ps_core::mods::{DesiredFile, PlanEntry, Role};
use ps_db::mod_targets::ModTarget;

use super::super::paths::LibraryPaths;
use super::apply::{ApplyError, JournalledPlan};
use super::backup::BackupSet;
use super::desired;
use super::write;
use super::{hash_if_present, ApplyOptions};

type Rows = [ps_db::mod_deployments::DeploymentFile];

/// The hash of a file recovery is sure about, and `None` for anything else —
/// absent, unreadable, or a directory where a file was expected.
///
/// Every use of this is a decision recovery declines to take: a row it does not
/// upsert, a file it does not touch. Declining is always safe, and it leaves the
/// path for `reconcile` to report by name, which is the answer a run that did
/// not finish owes its caller. Propagating instead would make an unreadable path
/// a raw error out of every future apply on the target, with no list of what was
/// wrong.
fn known(path: &Path) -> Option<String> {
    hash_if_present(path).ok().flatten()
}

/// Finds a recorded row by normalised key rather than by the string `ps_db`
/// itself matches on. A journalled path and the row it names can differ by
/// separator convention — a docker target's forward-slashed `root_path` — or,
/// on a case-insensitive filesystem, by case alone, and `ps_db::forget`/`file_at`
/// would then silently match nothing, leaving a row recovery meant to remove
/// or update untouched.
fn row_for<'a>(rows: &'a Rows, path: &str) -> Option<&'a ps_db::mod_deployments::DeploymentFile> {
    let key = super::apply::path_key(path);
    rows.iter().find(|r| super::apply::path_key(&r.path) == key)
}

/// The spelling `deployment_files` already holds for this path, or the
/// journalled one when it holds none. `ps_db::record` upserts on the path as SQL
/// text, so writing the journal's spelling of a path a row spells differently
/// would insert a second row for one file instead of updating the first.
fn record_path<'a>(rows: &'a Rows, path: &'a str) -> &'a str {
    row_for(rows, path).map(|r| r.path.as_str()).unwrap_or(path)
}

/// The desired set a `Keep` or `Reattribute` path was checked against. Neither
/// entry carries a hash of its own, so restoring a row that went missing for one
/// has no other source for the hash, role and version it needs. Recomputed fresh
/// rather than trusted from the journal, since it is cheap and it is the same
/// set this apply is about to plan against anyway.
async fn desired_files(
    db: &dyn ps_db::DbDriver,
    target: &ModTarget,
) -> Result<Vec<DesiredFile>, ApplyError> {
    match ps_db::mod_profiles::active_for_target(db, &target.id).await? {
        Some(profile) => Ok(desired::build(db, target, &profile.id)
            .await
            .map(|set| set.files)
            .unwrap_or_default()),
        None => Ok(Vec::new()),
    }
}

/// Walks the journalled plan entry by entry and brings records and disk back
/// into agreement. The rule underneath every arm: a file whose hash matches
/// what the journal intended to write is app-written, whichever instruction
/// the crash interrupted, so no file the app wrote can later be mistaken for a
/// user edit. Returns the paths that still disagree; an empty list is what
/// lets the journal close, via `reconcile` below.
pub async fn recover(
    db: &dyn ps_db::DbDriver,
    paths: &LibraryPaths,
    target: &ModTarget,
    options: &ApplyOptions<'_>,
) -> Result<Vec<String>, ApplyError> {
    if ps_db::mod_deployments::journal_of(db, &target.id)
        .await?
        .is_none()
    {
        return Ok(Vec::new());
    }
    // A journal whose contents cannot be read names nothing to reconcile, and a
    // journal that can never be reconciled would refuse every future apply on
    // this target. Closing it costs the hints it was carrying, which can only
    // make a later classification more cautious, never less.
    let Some(journalled) = super::apply::journalled_plan(db, &target.id).await? else {
        ps_db::mod_deployments::close_journal(db, &target.id).await?;
        return Ok(Vec::new());
    };

    let desired = desired_files(db, target).await?;
    let rows = ps_db::mod_deployments::files_of(db, &target.id).await?;

    for entry in &journalled.plan.entries {
        match entry {
            PlanEntry::Add {
                path,
                expected_hash,
                mod_version_id,
                rel_path,
                role,
                ..
            }
            | PlanEntry::Replace {
                path,
                expected_hash,
                mod_version_id,
                rel_path,
                role,
                ..
            } => {
                write::clear_stage(Path::new(path))?;
                if known(Path::new(path)).as_deref() == Some(expected_hash.as_str()) {
                    super::apply::record_recovered(
                        db,
                        target,
                        record_path(&rows, path),
                        mod_version_id,
                        rel_path,
                        expected_hash,
                        *role,
                    )
                    .await?;
                }
            }
            PlanEntry::Move {
                from,
                to,
                mod_version_id,
                rel_path,
                hash,
                role,
                recorded_hash,
            } => {
                let row_hash = super::apply::moved_row_hash(hash, recorded_hash);
                write::clear_stage(Path::new(to))?;
                // Both ends have to be readable before either can be acted on:
                // reading an unreadable source as absent would take the branch
                // that forgets the source's row while the source is still there.
                let (Ok(source), Ok(destination)) = (
                    hash_if_present(Path::new(from)),
                    hash_if_present(Path::new(to)),
                ) else {
                    continue;
                };
                match (source, destination) {
                    (Some(_), None) => {}
                    (None, Some(_)) => {
                        if let Some(row) = row_for(&rows, from) {
                            ps_db::mod_deployments::forget(db, &target.id, &[row.path.as_str()])
                                .await?;
                        }
                        super::apply::record_recovered(
                            db,
                            target,
                            record_path(&rows, to),
                            mod_version_id,
                            rel_path,
                            row_hash,
                            *role,
                        )
                        .await?;
                    }
                    (Some(source_hash), Some(destination_hash)) => {
                        if source_hash == destination_hash {
                            write::remove_if_present(Path::new(from))?;
                            if let Some(row) = row_for(&rows, from) {
                                ps_db::mod_deployments::forget(
                                    db,
                                    &target.id,
                                    &[row.path.as_str()],
                                )
                                .await?;
                            }
                            super::apply::record_recovered(
                                db,
                                target,
                                record_path(&rows, to),
                                mod_version_id,
                                rel_path,
                                row_hash,
                                *role,
                            )
                            .await?;
                        }
                        // Different content at both ends is drift: the row stays
                        // on the source and reconciliation reports the destination.
                    }
                    (None, None) => {}
                }
            }
            // The row goes only when the file is positively known to be gone. A
            // file that exists but cannot be stat'd must not be read as absent:
            // dropping its row abandons it, and for a `RemovePreserve` that is a
            // user's edited file left with no row, no backup entry and no journal.
            PlanEntry::Remove { path } | PlanEntry::RemovePreserve { path, .. } => {
                if matches!(hash_if_present(Path::new(path)), Ok(None)) {
                    if let Some(row) = row_for(&rows, path) {
                        ps_db::mod_deployments::forget(db, &target.id, &[row.path.as_str()])
                            .await?;
                    }
                }
            }
            // The attribution the crashed run was going to write, applied now.
            // The hash comes from the desired file whenever the file on disk is
            // still that file — which is the condition `build_plan` emitted this
            // entry under — and otherwise from the row, so a file edited since
            // the crash keeps a hash that says so and is preserved rather than
            // silently claimed as the app's own.
            PlanEntry::Reattribute {
                path,
                mod_version_id,
                rel_path,
            } => {
                let existing = row_for(&rows, path);
                let on_disk = known(Path::new(path));
                let wanted = desired::file_at(&desired, Path::new(path))
                    .filter(|d| on_disk.as_deref() == Some(d.expected_hash.as_str()))
                    .map(|d| (d.expected_hash.clone(), d.role))
                    .or_else(|| existing.map(|r| (r.hash.clone(), super::apply::role_of(&r.role))));
                if let Some((hash, role)) = wanted {
                    super::apply::record_recovered(
                        db,
                        target,
                        record_path(&rows, path),
                        mod_version_id,
                        rel_path,
                        &hash,
                        role,
                    )
                    .await?;
                }
            }
            // `Keep` carries no hash of its own, unlike every write entry, so
            // its row can only be restored against the desired file it was
            // checked against when the crashed plan was built. A path that
            // has since fallen out of the active profile is left for
            // `reconcile` to report rather than guessed at.
            PlanEntry::Keep { path } => {
                let missing_row = row_for(&rows, path).is_none();
                if missing_row {
                    if let Some(desired_file) = desired::file_at(&desired, Path::new(path)) {
                        if known(Path::new(path)).as_deref()
                            == Some(desired_file.expected_hash.as_str())
                        {
                            super::apply::record_recovered(
                                db,
                                target,
                                path,
                                &desired_file.mod_version_id,
                                &desired_file.rel_path,
                                &desired_file.expected_hash,
                                desired_file.role,
                            )
                            .await?;
                        }
                    }
                }
            }
            PlanEntry::Preserve { .. } => {}
        }
    }

    // A `RemovePreserve` whose source is still there did not complete; redo it,
    // then rewrite the index from the journal either way, because the journal
    // is the authoritative map after a crash, not whatever the set happens to
    // hold. An entry whose source is already gone was already taken before the
    // journal was opened (see `apply::apply`); all that was left for it was the
    // row, and the loop above just handled that through the shared
    // `Remove`/`RemovePreserve` arm.
    if !journalled.backups.is_empty() {
        let mut set = BackupSet::open(paths, &target.id, &journalled.stamp, &journalled.op_id)?;
        set.adopt(&journalled.backups)?;
        for backup in &journalled.backups {
            let original = Path::new(&backup.original_path);
            if known(original).is_some() {
                // Under the key the journal already published, not a fresh
                // ordinal: the restore action reads the index, and an entry
                // whose key names a file that was never written is worse than
                // no entry at all.
                set.take_at(original, &backup.backup_key, options.move_strategy)?;
                if let Some(row) = row_for(&rows, &backup.original_path) {
                    ps_db::mod_deployments::forget(db, &target.id, &[row.path.as_str()]).await?;
                }
            }
        }
    }

    for marker in &journalled.markers {
        let path = Path::new(&marker.path);
        write::clear_stage(path)?;
        if known(path).as_deref() == Some(marker.hash.as_str()) {
            ps_db::mod_deployments::record(
                db,
                &[ps_db::mod_deployments::NewDeploymentFile {
                    target_id: target.id.clone(),
                    path: record_path(&rows, &marker.path).to_string(),
                    mod_version_id: None,
                    hash: marker.hash.clone(),
                    role: "shared_marker".to_string(),
                    rel_path: None,
                }],
            )
            .await?;
        }
    }

    for copy in &journalled.new_copies {
        let path = Path::new(&copy.path);
        write::clear_stage(path)?;
        if known(path).as_deref() == Some(copy.hash.as_str()) {
            super::apply::record_recovered(
                db,
                target,
                record_path(&rows, &copy.path),
                &copy.mod_version_id,
                &copy.rel_path,
                &copy.hash,
                Role::PreservedCopy,
            )
            .await?;
        }
    }

    match super::super::layout::layout_for(target) {
        Ok(layout) => super::apply::prune_vacated_dirs(&journalled.plan, &layout),
        Err(error) => {
            tracing::debug!(%error, target_id = %target.id, "skipped pruning emptied directories");
        }
    }

    let failed = reconcile(db, target, &journalled).await?;
    if failed.is_empty() {
        ps_db::mod_deployments::close_journal(db, &target.id).await?;
    }
    Ok(failed)
}

/// Which journalled paths carry the app's own write on disk while the records do
/// not say so. Computed from a fresh read of both, never accumulated during the
/// walk above, so a retry reaches the same conclusion a first run would. An
/// empty result is what lets the journal close — closing it on anything else
/// would abandon the very state recovery exists to resolve.
///
/// The test is narrow on purpose: **the on-disk hash is the hash this journal
/// intended for that path, and no row carries it.** That is exactly the
/// condition every arm of `recover` upserts the row under, so a path still in
/// that state when reconciliation runs is one recovery tried to record and could
/// not — which is worth a journal. It is also exactly the condition under which
/// a journal hint can change a classification, so the journal is kept precisely
/// while it still explains something on disk.
///
/// Everything else converges on the next apply and must not hold the journal,
/// because a journal that cannot be closed wedges the target for good:
///
/// - the write never landed — no file, no row, and the next plan asks again;
/// - a file present that the intended hash does not explain — it is not ours,
///   and `build_plan`'s occupancy check refuses it by name next time, which
///   tells the user far more than a stuck journal does. A `Move` source still
///   carrying a user's edit and a shared marker still holding the user's own
///   bytes both live here, and both are ordinary rather than exceptional;
/// - a row whose file is absent — nothing on disk depends on the journal, the
///   spec forbids recovery from touching that row, and the next plan turns it
///   back into an `Add`.
pub async fn reconcile(
    db: &dyn ps_db::DbDriver,
    target: &ModTarget,
    journalled: &JournalledPlan,
) -> Result<Vec<String>, ApplyError> {
    let rows = ps_db::mod_deployments::files_of(db, &target.id).await?;
    let recorded = |path: &str| row_for(&rows, path);
    // A path reconciliation cannot read is a path it cannot vouch for, and the
    // answer a half-finished apply owes its caller is a list of paths rather
    // than an error: an unreadable path is reported like any other
    // disagreement, which keeps the journal *and* names the path.
    let unrecorded = |path: &str, on_disk_hash: &str, row_hash: &str| -> bool {
        match hash_if_present(Path::new(path)) {
            Err(_) => true,
            Ok(on_disk) => {
                on_disk.as_deref() == Some(on_disk_hash)
                    && recorded(path).map(|r| r.hash.as_str()) != Some(row_hash)
            }
        }
    };
    let unrecorded_write = |path: &str, intended: &str| unrecorded(path, intended, intended);

    let mut failed: Vec<String> = Vec::new();
    for entry in &journalled.plan.entries {
        match entry {
            PlanEntry::Add {
                path,
                expected_hash,
                ..
            }
            | PlanEntry::Replace {
                path,
                expected_hash,
                ..
            } => {
                if unrecorded_write(path, expected_hash) {
                    failed.push(path.clone());
                }
            }
            PlanEntry::Move {
                from,
                to,
                hash,
                recorded_hash,
                ..
            } => {
                // The destination is the only end the journal explains. A move
                // that never started leaves its source exactly where the source's
                // row still points, and its bytes are allowed to differ from that
                // row — carrying a user's edit is what a `Move` is *for*
                // (`build_plan` takes the entry's hash from disk, never from the
                // row), so asking the source to match its row would report every
                // interrupted move of an edited file forever.
                if unrecorded(to, hash, super::apply::moved_row_hash(hash, recorded_hash)) {
                    failed.push(to.clone());
                } else if drifted(from, to) {
                    // Both ends present with different content is the one state
                    // the design hands to a human: recovery leaves both, keeps
                    // the row on the source, and reports the destination.
                    failed.push(to.clone());
                }
            }
            // None of these four is a write, so none of them can leave the app's
            // own bytes somewhere unrecorded. A removal that has not happened is
            // a file with its row still on it and the next plan removes it again;
            // a `Keep`'s row hash is allowed to differ from disk, which is the
            // state that made it a `Keep`; and a `Preserve`'s entire content is
            // that the file and its row differ.
            PlanEntry::Remove { .. } | PlanEntry::RemovePreserve { .. } => {}
            PlanEntry::Keep { .. } | PlanEntry::Preserve { .. } => {}
            // `Reattribute` writes nothing either, but it is the one entry whose
            // whole job is a row update, so a row that is there and still names
            // the old version is a row update that did not run. A path with no
            // row at all is not reported: recovery cannot invent one, and the
            // next plan re-adds the file.
            PlanEntry::Reattribute {
                path,
                mod_version_id,
                ..
            } => {
                let stale = recorded(path)
                    .is_some_and(|r| r.mod_version_id.as_deref() != Some(mod_version_id));
                if stale {
                    failed.push(path.clone());
                }
            }
        }
    }
    for marker in &journalled.markers {
        if unrecorded_write(&marker.path, &marker.hash) {
            failed.push(marker.path.clone());
        }
    }
    for copy in &journalled.new_copies {
        if unrecorded_write(&copy.path, &copy.hash) {
            failed.push(copy.path.clone());
        }
    }
    failed.sort();
    failed.dedup();
    Ok(failed)
}

/// Two files at the two ends of a move with different content. A path that
/// cannot be read counts as drift for the same reason it counts as a
/// disagreement above: an answer nobody can check is not an answer.
fn drifted(from: &str, to: &str) -> bool {
    match (
        hash_if_present(Path::new(from)),
        hash_if_present(Path::new(to)),
    ) {
        (Ok(Some(source)), Ok(Some(destination))) => source != destination,
        (Err(_), _) | (_, Err(_)) => true,
        _ => false,
    }
}
