use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use ps_core::mods::{
    build_plan, normalize_physical_path, resolve_layout, DeployPlan, DesiredFile, DiskState,
    JournalHints, LayoutOverrides, PlanEntry, PreflightError, RecordedFile, Role, RouteKind,
    TargetKind, TargetLayout,
};
use ps_db::mod_deployments::DeploymentFile;
use ps_db::mod_targets::ModTarget;

use super::super::layout::{self, LayoutResolveError};
use super::super::paths::LibraryPaths;
use super::backup::{self, BackupSet};
use super::desired::{self, DesiredSet};
use super::markers::{self, MarkerWrite};
use super::write;
use super::{hash_if_present, recover, ApplyOptions};

#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    #[error("the game or server for {0} is running")]
    TargetLocked(String),
    #[error("another apply is already running on {0}")]
    ApplyInProgress(String),
    #[error("target {0} has no active profile")]
    NoActiveProfile(String),
    #[error(transparent)]
    Preflight(#[from] PreflightError),
    #[error(transparent)]
    Desired(#[from] desired::DesiredError),
    #[error(transparent)]
    Layout(#[from] LayoutResolveError),
    #[error(transparent)]
    Db(#[from] ps_db::DbError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// A failure after one or more replaced occupants had already been moved
    /// into `backup_dir`, where they stay.
    #[error("{source} (after moving {} replaced file(s) into {backup_dir})", .moved.len())]
    ReplacePartial {
        moved: Vec<String>,
        backup_dir: String,
        source: Box<ApplyError>,
    },
}

/// Whether the filesystem this process writes to folds case. A property of the
/// host, not of the target: a Docker server's files live in host directories.
pub(super) const CASE_INSENSITIVE: bool = cfg!(any(windows, target_os = "macos"));

/// The key two spellings of one file share. `Path` equality is still
/// case-sensitive on Windows, so it cannot be used for this.
pub(super) fn path_key(path: &str) -> String {
    normalize_physical_path(path, CASE_INSENSITIVE)
}

/// The hash a moved file's destination row records. A journal written before
/// `recorded_hash` existed carries only the on-disk hash.
pub(super) fn moved_row_hash<'a>(hash: &'a str, recorded_hash: &'a str) -> &'a str {
    if recorded_hash.is_empty() {
        hash
    } else {
        recorded_hash
    }
}

fn partial_replace(moved: Vec<String>, backup_dir: &Path, source: ApplyError) -> ApplyError {
    if moved.is_empty() {
        return source;
    }
    ApplyError::ReplacePartial {
        moved,
        backup_dir: backup_dir.to_string_lossy().into_owned(),
        source: Box::new(source),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JournalledMarker {
    pub path: String,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JournalledNewCopy {
    pub path: String,
    pub hash: String,
    pub mod_version_id: String,
    pub rel_path: String,
}

/// Everything recovery needs, and nothing it has to re-derive. The expected hash
/// of every write is here, which is what lets a file the app wrote be recognised
/// as app-written no matter which instruction a crash interrupted.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JournalledPlan {
    pub plan: DeployPlan,
    pub markers: Vec<JournalledMarker>,
    pub backups: Vec<backup::BackupEntry>,
    #[serde(default)]
    pub new_copies: Vec<JournalledNewCopy>,
    pub op_id: String,
    pub stamp: String,
}

#[derive(Debug, Default)]
pub struct ApplyOutcome {
    pub plan: DeployPlan,
    pub mid_apply: bool,
    pub failed: Vec<String>,
    pub preserved: Vec<String>,
    /// The `.new` copies beside preserved files that now hold the new version.
    pub new_copies: Vec<String>,
    /// `.new` paths holding a file the deployer does not own, left untouched.
    pub skipped_new_copies: Vec<String>,
    pub backup_dir: Option<String>,
    /// Why execution stopped, when it stopped. A filesystem failure part way
    /// through is a `mid_apply` outcome with the paths that still disagree, not
    /// an error that discards them — but the caller still has to be told what
    /// went wrong, and told it in a form it can match on, or the partial result
    /// reads as a decision rather than a fault.
    pub error: Option<ApplyError>,
}

fn recorded_files(
    rows: Vec<ps_db::mod_deployments::DeploymentFile>,
    layout: &TargetLayout,
    default_layout: &TargetLayout,
    desired: &[DesiredFile],
) -> Vec<RecordedFile> {
    // A preserved copy is settled after execution, unless a version now ships a
    // real file at its path: then it is the app's file in the way, and the plan
    // has to be able to replace it.
    let destinations: HashSet<String> = desired.iter().map(|d| path_key(&d.path)).collect();
    rows.into_iter()
        .filter(|row| match row.role.as_str() {
            "shared_marker" => false,
            PRESERVED_COPY => destinations.contains(&path_key(&row.path)),
            _ => true,
        })
        .map(|row| {
            let mut kind = kind_for_path(layout, default_layout, Path::new(&row.path));
            // A server target's directories come entirely from overrides (see
            // `server_layout_overrides`), so a relocation there is always one
            // override replacing another — never an override appearing where
            // there was none — and neither layout above sits under the other.
            // The desired set is the only remaining source of truth for a row
            // in that state. The runtime directories contain other kinds'
            // bases, so a row classified by them is confirmed against the
            // desired set too.
            if matches!(
                kind,
                RouteKind::Passthrough | RouteKind::Binaries | RouteKind::Ue4ssCore
            ) {
                if let (Some(mv), Some(rel)) =
                    (row.mod_version_id.as_deref(), row.rel_path.as_deref())
                {
                    if let Some(found) = desired_kind_for(desired, mv, rel) {
                        kind = found;
                    }
                }
            }
            RecordedFile {
                kind,
                path: row.path,
                mod_version_id: row.mod_version_id,
                rel_path: row.rel_path,
                hash: row.hash,
                role: role_of(&row.role),
            }
        })
        .collect()
}

/// A fall-through only, never a precedence override: path classification is
/// the only signal that separates two recorded rows sharing one
/// `(mod_version_id, rel_path)` under two different route kinds at once (see
/// `one_rel_path_under_two_route_kinds_is_two_keys` in `ps-core`), so an
/// ambiguous match here must leave the row `Passthrough` rather than guess.
fn desired_kind_for(
    desired: &[DesiredFile],
    mod_version_id: &str,
    rel_path: &str,
) -> Option<RouteKind> {
    let mut matches = desired
        .iter()
        .filter(|d| d.mod_version_id == mod_version_id && d.rel_path == rel_path)
        .map(|d| d.kind);
    let first = matches.next()?;
    match matches.next() {
        None => Some(first),
        Some(_) => None,
    }
}

/// The layout a target would resolve to with no overrides at all. A row
/// recorded before an override moved one of its directories still sits under
/// this one, which is what keeps it classifiable after the move.
fn default_layout_for(target: &ModTarget) -> Result<TargetLayout, LayoutResolveError> {
    let mut spec = layout::spec_for(target)?;
    spec.overrides = LayoutOverrides::default();
    Ok(resolve_layout(&spec)?)
}

fn bases_of(layout: &TargetLayout) -> [(Option<&Path>, RouteKind); 8] {
    [
        (layout.palschema_mods_dir.as_deref(), RouteKind::PalSchema),
        (layout.ue4ss_mods_dir.as_deref(), RouteKind::Ue4ss),
        (Some(layout.paks_mods_dir.as_path()), RouteKind::Pak),
        (Some(layout.logicmods_dir.as_path()), RouteKind::LogicMods),
        (layout.nativemods_dir.as_deref(), RouteKind::NativeDll),
        (layout.workshop_local_dir.as_deref(), RouteKind::Workshop),
        (layout.ue4ss_dir.as_deref(), RouteKind::Ue4ssCore),
        (layout.binaries_dir.as_deref(), RouteKind::Binaries),
    ]
}

/// Removes the directories a plan's removals and moves left empty, within the
/// layout's mod directories.
pub(super) fn prune_vacated_dirs(plan: &DeployPlan, layout: &TargetLayout) {
    let candidates: Vec<&Path> = plan
        .entries
        .iter()
        .filter_map(|entry| match entry {
            PlanEntry::Remove { path } | PlanEntry::RemovePreserve { path, .. } => Some(path),
            PlanEntry::Move { from, .. } => Some(from),
            _ => None,
        })
        .filter_map(|path| Path::new(path).parent())
        .collect();
    if candidates.is_empty() {
        return;
    }
    let bases: Vec<&Path> = bases_of(layout)
        .into_iter()
        .filter_map(|(base, _)| base)
        .collect();
    write::prune_empty_dirs(&candidates, &bases);
}

/// `deployment_files` stores no route kind, and `RecordedFile` needs one for the
/// move key: a wrong kind stops a move matching and silently degrades it into a
/// remove plus an add, which backs the user's edit up instead of carrying it. The
/// kind is recoverable from the path, because every kind has its own base
/// directory — longest match wins, since `palschema_mods_dir` sits inside
/// `ue4ss_mods_dir`.
///
/// A row is checked against both the current layout and the default,
/// no-override one: a target that has since gained an override for the very
/// directory a file was recorded under would otherwise reclassify that row as
/// `Passthrough` the moment the override is set, which is exactly the
/// relocation this deployer is supposed to carry as a move.
///
/// Compared as normalised keys on a component boundary, so a respelled root still
/// classifies and `…/Mods` never claims `…/ModsExtra`.
fn kind_for_path(layout: &TargetLayout, default_layout: &TargetLayout, path: &Path) -> RouteKind {
    let key = path_key(&path.to_string_lossy());
    let mut best: Option<(usize, RouteKind)> = None;
    for (base, kind) in bases_of(layout).into_iter().chain(bases_of(default_layout)) {
        let Some(base) = base else { continue };
        let base_key = path_key(&base.to_string_lossy());
        if base_key.is_empty() {
            continue;
        }
        let under = key == base_key
            || key
                .strip_prefix(&base_key)
                .is_some_and(|rest| base_key.ends_with('/') || rest.starts_with('/'));
        if under {
            let depth = base_key.split('/').filter(|s| !s.is_empty()).count();
            if best.map(|(d, _)| depth > d).unwrap_or(true) {
                best = Some((depth, kind));
            }
        }
    }
    best.map(|(_, kind)| kind).unwrap_or(RouteKind::Passthrough)
}

pub(super) fn role_of(role: &str) -> Role {
    match role {
        "companion" => Role::Companion,
        "marker" => Role::Marker,
        "shared_marker" => Role::SharedMarker,
        PRESERVED_COPY => Role::PreservedCopy,
        _ => Role::File,
    }
}

const PRESERVED_COPY: &str = "preserved_copy";

fn role_name(role: Role) -> &'static str {
    match role {
        Role::File => "file",
        Role::Companion => "companion",
        Role::Marker => "marker",
        Role::SharedMarker => "shared_marker",
        Role::PreservedCopy => PRESERVED_COPY,
    }
}

/// Hashes every path the plan could touch. `build_plan` reads a missing key as
/// "no file there", which would turn a recorded file into an unconditional
/// `Remove`, so this map has to be complete rather than convenient. A path that
/// exists but cannot be read is not "no file there" either, which is why every
/// read here goes through `hash_if_present`.
fn disk_state(desired: &DesiredSet, recorded: &[RecordedFile]) -> Result<DiskState, ApplyError> {
    let mut hashes: HashMap<String, Option<String>> = HashMap::new();
    let mut note = |path: &str| -> Result<(), ApplyError> {
        hashes.insert(path.to_string(), hash_if_present(Path::new(path))?);
        Ok(())
    };
    for file in &desired.files {
        note(&file.path)?;
    }
    for row in recorded {
        note(&row.path)?;
    }
    Ok(DiskState { hashes })
}

/// The plan a still-open journal describes, or `None` when there is no journal
/// or its contents cannot be read. A journal nobody can parse carries no
/// information, so it holds nothing back: treating it as fatal instead would
/// leave the target refusing every future apply with no way out.
pub(super) async fn journalled_plan(
    db: &dyn ps_db::DbDriver,
    target_id: &str,
) -> Result<Option<JournalledPlan>, ApplyError> {
    Ok(ps_db::mod_deployments::journal_of(db, target_id)
        .await?
        .as_ref()
        .and_then(parse_journal))
}

fn parse_journal(row: &ps_db::mod_deployments::ApplyJournal) -> Option<JournalledPlan> {
    serde_json::from_str::<JournalledPlan>(&row.plan).ok()
}

/// The second half of "app-written": the hash the last journal intended to put
/// at each path. A file the app wrote but did not get to record still matches
/// one of these, which is what stops a crash turning the app's own file into
/// something that looks like a user edit.
pub(super) fn hints_of(journalled: Option<&JournalledPlan>) -> JournalHints {
    let Some(journalled) = journalled else {
        return JournalHints::default();
    };
    let mut intended = HashMap::new();
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
                intended.insert(path.clone(), expected_hash.clone());
            }
            PlanEntry::Move {
                to,
                hash,
                recorded_hash,
                ..
            } => {
                intended.insert(to.clone(), moved_row_hash(hash, recorded_hash).to_string());
            }
            _ => {}
        }
    }
    for marker in &journalled.markers {
        intended.insert(marker.path.clone(), marker.hash.clone());
    }
    JournalHints { intended }
}

/// A destination the app does not own and which already holds a file is a
/// refusal, not an overwrite. Checked against the desired set and the recorded
/// rows rather than against a plan, so it runs *before* the plan is built: a
/// refusal that arrives after the diff is a refusal from somewhere else.
///
/// Ownership is checked before this: a selection that routes two versions to
/// one path must be refused as that, whatever already sits on disk there.
fn occupancy(
    desired: &[DesiredFile],
    recorded: &[RecordedFile],
    disk: &DiskState,
) -> Result<(), PreflightError> {
    let owned: HashSet<String> = recorded.iter().map(|r| path_key(&r.path)).collect();
    // `DiskState` is keyed by the exact strings `desired::build` produced, so it
    // is looked up by those rather than by key.
    let mut occupied: Vec<String> = desired
        .iter()
        .filter(|d| !owned.contains(&path_key(&d.path)))
        .filter(|d| disk.hashes.get(&d.path).is_some_and(Option::is_some))
        .map(|d| d.path.clone())
        .collect();
    if occupied.is_empty() {
        Ok(())
    } else {
        occupied.sort();
        occupied.dedup();
        Err(PreflightError::UnmanagedOccupant { paths: occupied })
    }
}

/// One desired owner per destination, decided from the desired set alone, so it
/// answers the same whatever is on disk.
fn ownership(desired: &[DesiredFile]) -> Result<(), PreflightError> {
    let mut owners: BTreeMap<String, Vec<&DesiredFile>> = BTreeMap::new();
    for file in desired {
        owners.entry(path_key(&file.path)).or_default().push(file);
    }
    match owners.into_values().find(|files| files.len() > 1) {
        Some(files) => Err(PreflightError::DestinationConflict {
            path: files[0].path.clone(),
            mod_version_ids: files.iter().map(|f| f.mod_version_id.clone()).collect(),
        }),
        None => Ok(()),
    }
}

/// Everything the diff is computed from, read once. `apply` needs all of it
/// after the plan as well — the recorded rows for the occupancy answer, the
/// desired set for the roles and hashes a plan entry does not carry.
struct PlanInputs {
    layout: TargetLayout,
    desired: DesiredSet,
    recorded: Vec<RecordedFile>,
    disk: DiskState,
}

async fn plan_inputs(
    db: &dyn ps_db::DbDriver,
    target: &ModTarget,
    profile_id: &str,
) -> Result<PlanInputs, ApplyError> {
    let layout = layout::layout_for(target)?;
    let desired = desired::build(db, target, profile_id).await?;
    let recorded = recorded_files(
        ps_db::mod_deployments::files_of(db, &target.id).await?,
        &layout,
        &default_layout_for(target)?,
        &desired.files,
    );
    let disk = disk_state(&desired, &recorded)?;
    Ok(PlanInputs {
        layout,
        desired,
        recorded,
        disk,
    })
}

async fn plan_with_hints(
    db: &dyn ps_db::DbDriver,
    target: &ModTarget,
    profile_id: &str,
    hints: &JournalHints,
) -> Result<(DeployPlan, PlanInputs), ApplyError> {
    let inputs = plan_inputs(db, target, profile_id).await?;
    ownership(&inputs.desired.files)?;
    occupancy(&inputs.desired.files, &inputs.recorded, &inputs.disk)?;
    let plan = build_plan(
        &inputs.desired.files,
        &inputs.recorded,
        &inputs.disk,
        hints,
        CASE_INSENSITIVE,
    )?;
    Ok((plan, inputs))
}

/// Absolute, and made only of a prefix, a root and plain names: `Path` equality
/// never resolves `.` or `..`, so anything else is refused before it is compared.
fn is_plain_absolute(path: &Path) -> bool {
    path.is_absolute()
        && path.components().all(|c| {
            matches!(
                c,
                std::path::Component::Prefix(_)
                    | std::path::Component::RootDir
                    | std::path::Component::Normal(_)
            )
        })
}

/// Moves each listed unmanaged occupant into this operation's backup set so the
/// plan sees its destination free. A path qualifies only when it is one of the
/// desired set's destinations, has no `deployment_files` row, and holds a file:
/// replace exists for files the app does not own, and must never move one it
/// does or reach anywhere else on disk. Nothing moves unless the plan would then
/// pass preflight, so a refusal still leaves the target untouched.
struct Replaced {
    set: BackupSet,
    moved: Vec<String>,
}

#[allow(clippy::too_many_arguments)]
async fn take_occupants(
    db: &dyn ps_db::DbDriver,
    paths: &LibraryPaths,
    target: &ModTarget,
    profile_id: &str,
    hints: &JournalHints,
    options: &ApplyOptions<'_>,
    stamp: &str,
    op_id: &str,
) -> Result<Option<Replaced>, ApplyError> {
    if options.replace_occupants.is_empty() {
        return Ok(None);
    }
    let rows = ps_db::mod_deployments::files_of(db, &target.id).await?;
    let inputs = plan_inputs(db, target, profile_id).await?;
    let mut chosen: Vec<&DesiredFile> = Vec::new();
    for listed in options.replace_occupants {
        if !is_plain_absolute(listed) {
            continue;
        }
        let listed_key = path_key(&listed.to_string_lossy());
        if rows.iter().any(|row| path_key(&row.path) == listed_key) {
            continue;
        }
        let Some(destination) = inputs
            .desired
            .files
            .iter()
            .find(|d| path_key(&d.path) == listed_key)
        else {
            continue;
        };
        if hash_if_present(listed)?.is_none() {
            continue;
        }
        if !chosen.iter().any(|c| path_key(&c.path) == listed_key) {
            chosen.push(destination);
        }
    }
    if chosen.is_empty() {
        return Ok(None);
    }

    let mut disk = inputs.disk.clone();
    for destination in &chosen {
        disk.hashes.insert(destination.path.clone(), None);
    }
    ownership(&inputs.desired.files)?;
    occupancy(&inputs.desired.files, &inputs.recorded, &disk)?;
    build_plan(
        &inputs.desired.files,
        &inputs.recorded,
        &disk,
        hints,
        CASE_INSENSITIVE,
    )?;

    let mut set = BackupSet::open(paths, &target.id, stamp, op_id)?;
    let mut moved = Vec::new();
    for destination in chosen {
        if let Err(error) = set.take(Path::new(&destination.path), options.move_strategy) {
            return Err(partial_replace(moved, set.dir(), error.into()));
        }
        moved.push(destination.path.clone());
    }
    Ok(Some(Replaced { set, moved }))
}

pub async fn plan_for(
    db: &dyn ps_db::DbDriver,
    target: &ModTarget,
    profile_id: &str,
) -> Result<(DeployPlan, DesiredSet, TargetLayout), ApplyError> {
    let hints = hints_of(journalled_plan(db, &target.id).await?.as_ref());
    let (plan, inputs) = plan_with_hints(db, target, profile_id, &hints).await?;
    Ok((plan, inputs.desired, inputs.layout))
}

pub async fn apply(
    db: &dyn ps_db::DbDriver,
    paths: &LibraryPaths,
    target: &ModTarget,
    options: &ApplyOptions<'_>,
) -> Result<ApplyOutcome, ApplyError> {
    // Held for the whole apply: a second run would read this one's live journal
    // as a crash and recover it while it is still being written.
    let Some(lock) = super::try_lock_target(paths, target) else {
        return Err(ApplyError::ApplyInProgress(target.id.clone()));
    };
    apply_locked(db, paths, target, &lock, options).await
}

/// `apply` under the target's apply lock, already taken by a caller that must
/// keep it past the apply. Any other guard is refused as `ApplyInProgress`.
pub async fn apply_locked(
    db: &dyn ps_db::DbDriver,
    paths: &LibraryPaths,
    target: &ModTarget,
    lock: &tokio::sync::OwnedMutexGuard<()>,
    options: &ApplyOptions<'_>,
) -> Result<ApplyOutcome, ApplyError> {
    if !super::holds_target(paths, target, lock) {
        return Err(ApplyError::ApplyInProgress(target.id.clone()));
    }
    if options.running.is_running(target) {
        return Err(ApplyError::TargetLocked(target.id.clone()));
    }

    // A journal left by a previous run is read before recovery gets to close it:
    // its intended hashes are what keep a file the crashed run wrote from being
    // read as a user edit, and its operation id names the backup set this run
    // continues rather than orphaning. Recovery then deals with the journal
    // before anything else, so the plan below is computed against a reconciled
    // target. A target recovery could not fully reconcile must not then be
    // planned against and written to as if it were clean, so that case returns
    // here rather than falling through.
    //
    // Those hints cannot change *this* plan, and the proof is worth writing down
    // because it is what a later change would quietly break. `build_plan`
    // consults a hint only through `app_written`, so a hint can only decide
    // anything when `on_disk == intended` and `on_disk != row.hash`. That is
    // precisely the condition each recovery arm upserts the row under, and
    // precisely the condition `reconcile` refuses to close the journal on — so
    // by the time the early return below has been passed, every hint agrees with
    // a row that already says the same thing. The wiring stays because both
    // halves of that proof are things a later plan may change: the early return,
    // and the exact condition each arm records on.
    // A journal row that cannot be parsed yields no hints; the second read is
    // what still notices it, so that recovery can close it.
    let journal = ps_db::mod_deployments::journal_of(db, &target.id).await?;
    let resumed = journal.as_ref().and_then(parse_journal);
    if journal.is_some() {
        let unresolved = recover::recover(db, paths, target, options).await?;
        if !unresolved.is_empty() {
            return Ok(ApplyOutcome {
                plan: DeployPlan::default(),
                mid_apply: true,
                failed: unresolved,
                ..Default::default()
            });
        }
    }

    let op_id = resumed
        .as_ref()
        .map(|j| j.op_id.clone())
        .unwrap_or_else(backup::new_op_id);
    let stamp = resumed
        .as_ref()
        .map(|j| j.stamp.clone())
        .or_else(|| options.stamp.map(str::to_string))
        .unwrap_or_else(backup::utc_stamp);

    let profile = ps_db::mod_profiles::active_for_target(db, &target.id)
        .await?
        .ok_or_else(|| ApplyError::NoActiveProfile(target.id.clone()))?;
    let hints = hints_of(resumed.as_ref());
    let replaced = take_occupants(
        db,
        paths,
        target,
        &profile.id,
        &hints,
        options,
        &stamp,
        &op_id,
    )
    .await?;
    let backup_dir = replaced.as_ref().map(|r| r.set.dir().to_path_buf());
    let moved = replaced
        .as_ref()
        .map(|r| r.moved.clone())
        .unwrap_or_default();
    let result = apply_plan(
        db,
        paths,
        target,
        options,
        &profile.id,
        &hints,
        &op_id,
        &stamp,
        replaced.map(|r| r.set),
    )
    .await;
    match (result, backup_dir) {
        (Err(error), Some(dir)) => Err(partial_replace(moved, &dir, error)),
        (result, _) => result,
    }
}

#[allow(clippy::too_many_arguments)]
async fn apply_plan(
    db: &dyn ps_db::DbDriver,
    paths: &LibraryPaths,
    target: &ModTarget,
    options: &ApplyOptions<'_>,
    profile_id: &str,
    hints: &JournalHints,
    op_id: &str,
    stamp: &str,
    replaced: Option<BackupSet>,
) -> Result<ApplyOutcome, ApplyError> {
    let (plan, inputs) = plan_with_hints(db, target, profile_id, hints).await?;
    let PlanInputs {
        layout,
        desired,
        recorded,
        ..
    } = inputs;

    let marker_writes = markers::plan_markers(&layout, &desired)?;

    // The shared markers are the one thing an apply overwrites without an
    // occupancy check and without ever classifying it as a user edit, so the
    // user's own copy is snapshotted before the app first writes either of them.
    // A target whose markers do not exist yet has nothing to snapshot, and an
    // empty backup set would offer the user a restore action over nothing.
    let recorded_markers: HashSet<String> = ps_db::mod_deployments::shared_markers(db, &target.id)
        .await?
        .iter()
        .map(|row| path_key(&row.path))
        .collect();
    let mut snapshots: Vec<&Path> = Vec::new();
    for marker in &marker_writes {
        if !recorded_markers.contains(&path_key(&marker.path.to_string_lossy()))
            && hash_if_present(&marker.path)?.is_some()
        {
            snapshots.push(&marker.path);
        }
    }
    let needs_backups = !snapshots.is_empty()
        || plan
            .entries
            .iter()
            .any(|e| matches!(e, PlanEntry::RemovePreserve { .. }));
    let mut set = match replaced {
        Some(set) => Some(set),
        None if needs_backups => Some(BackupSet::open(paths, &target.id, stamp, op_id)?),
        None => None,
    };

    // Backups happen before the journal so their entries can be journalled, which
    // is what makes the journal the authoritative map after a crash.
    let mut preserved_moves: Vec<(String, backup::BackupEntry)> = Vec::new();
    if let Some(set) = set.as_mut() {
        for marker in snapshots {
            // A resumed apply reopens the set it already snapshotted into.
            let taken = set
                .entries()
                .iter()
                .any(|e| Path::new(&e.original_path) == marker);
            if !taken {
                set.copy_in(marker)?;
            }
        }
        for entry in &plan.entries {
            if let PlanEntry::RemovePreserve { path, .. } = entry {
                let file = Path::new(path);
                if hash_if_present(file)?.is_some() {
                    let recorded_entry = set.take(file, options.move_strategy)?;
                    preserved_moves.push((path.clone(), recorded_entry));
                }
            }
        }
        set.write_index()?;
    }

    let rows = ps_db::mod_deployments::files_of(db, &target.id).await?;
    let (planned_copies, mut skipped_new_copies) = plan_new_copies(&plan, &desired, &rows);

    let journalled = JournalledPlan {
        plan: plan.clone(),
        markers: marker_writes
            .iter()
            .map(|m| JournalledMarker {
                path: m.path.to_string_lossy().into_owned(),
                hash: m.hash.clone(),
            })
            .collect(),
        backups: preserved_moves.iter().map(|(_, e)| e.clone()).collect(),
        new_copies: planned_copies.iter().map(|p| p.copy.clone()).collect(),
        op_id: op_id.to_string(),
        stamp: stamp.to_string(),
    };
    ps_db::mod_deployments::open_journal(
        db,
        &target.id,
        op_id,
        &serde_json::to_string(&journalled).map_err(std::io::Error::other)?,
    )
    .await?;

    // A write that fails part way through is a partial result, not a lost one:
    // the journal stays open, reconciliation still runs, and the caller is handed
    // the paths that still disagree together with the failure that stopped the
    // run. Only a filesystem failure is a partial result, though — a database
    // fault is not a half-applied target and must not read as one.
    let mut preserved: Vec<String> = Vec::new();
    let mut new_copies: Vec<String> = Vec::new();
    let executed = match execute(
        db,
        target,
        &layout,
        &plan,
        &marker_writes,
        options,
        &desired,
        &recorded,
        &mut preserved,
    )
    .await
    {
        Ok(()) => {
            settle_new_copies(
                db,
                target,
                &plan,
                &desired,
                &planned_copies,
                &mut new_copies,
                &mut skipped_new_copies,
            )
            .await
        }
        Err(error) => Err(error),
    };

    let failed = match recover::reconcile(db, target, &journalled).await {
        Ok(failed) => failed,
        // Reconciliation itself only fails on the database now, since an
        // unreadable path is one of its answers rather than an error. The journal
        // is still open, so the next apply reconciles again; the failure that
        // stopped this run is the more useful one to report.
        Err(error) => return Err(executed.err().unwrap_or(error)),
    };
    let failure = match executed {
        Ok(()) => None,
        Err(ApplyError::Io(error)) => Some(ApplyError::Io(error)),
        Err(other) => return Err(other),
    };
    if failed.is_empty() && failure.is_none() {
        ps_db::mod_deployments::close_journal(db, &target.id).await?;
        if let Err(error) =
            super::super::library::forget_released(db, &target.id, &desired.workshop_released).await
        {
            tracing::warn!(%error, target_id = %target.id, "failed to forget released workshop packages");
        }
    }
    Ok(ApplyOutcome {
        plan,
        mid_apply: !failed.is_empty() || failure.is_some(),
        failed,
        preserved,
        new_copies,
        skipped_new_copies,
        backup_dir: set.map(|s| s.dir().to_string_lossy().into_owned()),
        error: failure,
    })
}

/// Removals and preserves first, then moves, then adds and replaces, then
/// markers last: a moved-from source has to be gone before anything later
/// could mistake it for still being available, and a marker's managed-mod
/// listing has to reflect every route write that already happened. Each
/// file's row is written immediately after its own filesystem step rather
/// than batched at the end, because a crash between the two is exactly what
/// recovery's hash comparison exists to resolve.
#[allow(clippy::too_many_arguments)]
async fn execute(
    db: &dyn ps_db::DbDriver,
    target: &ModTarget,
    layout: &TargetLayout,
    plan: &DeployPlan,
    marker_writes: &[MarkerWrite],
    options: &ApplyOptions<'_>,
    desired: &DesiredSet,
    recorded: &[RecordedFile],
    preserved: &mut Vec<String>,
) -> Result<(), ApplyError> {
    for entry in &plan.entries {
        match entry {
            PlanEntry::RemovePreserve { path, .. } => {
                // The file was already moved into the backup set before the
                // journal; all that is left is the row.
                ps_db::mod_deployments::forget(db, &target.id, &[path.as_str()]).await?;
            }
            PlanEntry::Remove { path } => {
                write::clear_stage(Path::new(path))?;
                write::remove_if_present(Path::new(path))?;
                ps_db::mod_deployments::forget(db, &target.id, &[path.as_str()]).await?;
            }
            _ => {}
        }
    }
    for entry in &plan.entries {
        if let PlanEntry::Move {
            from,
            to,
            mod_version_id,
            rel_path,
            hash,
            role,
            recorded_hash,
        } = entry
        {
            // The source's stage path is swept as well as the destination's: a
            // `.psnew` a crashed run left beside the old location is the one
            // piece of debris nothing else on this branch would ever reach.
            write::clear_stage(Path::new(from))?;
            write::clear_stage(Path::new(to))?;
            write::move_file(Path::new(from), Path::new(to), options.move_strategy)?;
            ps_db::mod_deployments::forget(db, &target.id, &[from.as_str()]).await?;
            record_recovered(
                db,
                target,
                to,
                mod_version_id,
                rel_path,
                moved_row_hash(hash, recorded_hash),
                *role,
            )
            .await?;
        }
    }
    for entry in &plan.entries {
        match entry {
            PlanEntry::Add {
                path,
                source,
                expected_hash,
                mod_version_id,
                rel_path,
                role,
            }
            | PlanEntry::Replace {
                path,
                source,
                expected_hash,
                mod_version_id,
                rel_path,
                role,
            } => {
                write::copy_staged(Path::new(source), Path::new(path))?;
                record_recovered(
                    db,
                    target,
                    recorded_spelling(recorded, path),
                    mod_version_id,
                    rel_path,
                    expected_hash,
                    *role,
                )
                .await?;
            }
            // `Keep`, `Preserve`, and `Reattribute` write no file content of
            // their own, so a stage file a crashed run left beside one of
            // these paths would otherwise never be touched again; sweeping it
            // here is what keeps a completed apply from leaving debris.
            PlanEntry::Preserve { path, .. } => {
                write::clear_stage(Path::new(path))?;
                preserved.push(path.clone());
            }
            PlanEntry::Reattribute {
                path,
                mod_version_id,
                rel_path,
            } => {
                write::clear_stage(Path::new(path))?;
                // `build_plan` only emits `Reattribute` when the file on disk
                // already matches the desired file's `expected_hash` (that is
                // what makes it a reattribution and not a `Replace`), so that
                // is the hash to record — never the row's old one, which is
                // exactly the value that made this a reattribution in the
                // first place, and never an absent row's default. The role
                // comes from the same place: a marker or a pak's companion
                // whose bytes did not change is still a marker or a companion.
                if let Some(d) = desired::file_at(&desired.files, Path::new(path)) {
                    record_recovered(
                        db,
                        target,
                        recorded_spelling(recorded, path),
                        mod_version_id,
                        rel_path,
                        &d.expected_hash,
                        d.role,
                    )
                    .await?;
                }
            }
            PlanEntry::Keep { path } => {
                write::clear_stage(Path::new(path))?;
            }
            _ => {}
        }
    }
    prune_vacated_dirs(plan, layout);
    let marker_rows = ps_db::mod_deployments::shared_markers(db, &target.id).await?;
    for marker in marker_writes {
        write::write_staged(&marker.bytes, &marker.path)?;
        let path = marker.path.to_string_lossy();
        let key = path_key(&path);
        let path = marker_rows
            .iter()
            .find(|row| path_key(&row.path) == key)
            .map_or(path.as_ref(), |row| row.path.as_str());
        ps_db::mod_deployments::record(
            db,
            &[ps_db::mod_deployments::NewDeploymentFile {
                target_id: target.id.clone(),
                path: path.to_string(),
                mod_version_id: None,
                hash: marker.hash.clone(),
                role: "shared_marker".to_string(),
                rel_path: None,
            }],
        )
        .await?;
    }
    // A shared marker this apply no longer writes loses its row but keeps its
    // file, which still holds lines the app does not manage. The exception is the
    // `PalModSettings.ini` a Docker server target was once given: no container
    // reads it, so it goes while it still holds the app's own bytes.
    let planned: HashSet<String> = marker_writes
        .iter()
        .map(|marker| path_key(&marker.path.to_string_lossy()))
        .collect();
    let docker = layout::spec_for(target).is_ok_and(|spec| spec.kind == TargetKind::DockerServer);
    for row in ps_db::mod_deployments::shared_markers(db, &target.id).await? {
        if planned.contains(&path_key(&row.path)) {
            continue;
        }
        let unread_ini = docker
            && Path::new(&row.path).file_name().is_some_and(|name| {
                name.to_string_lossy()
                    .eq_ignore_ascii_case("PalModSettings.ini")
            });
        if unread_ini {
            release(db, target, &row).await?;
        } else {
            ps_db::mod_deployments::forget(db, &target.id, &[row.path.as_str()]).await?;
        }
    }
    Ok(())
}

/// A `.new` copy the deployer means to write beside a preserved file, and the
/// library file it is copied from.
struct PlannedCopy {
    copy: JournalledNewCopy,
    source: String,
}

fn row_at<'a>(rows: &'a [DeploymentFile], path: &str) -> Option<&'a DeploymentFile> {
    let key = path_key(path);
    rows.iter().find(|row| path_key(&row.path) == key)
}

/// Only an empty path, or one still holding the bytes its own `preserved_copy`
/// row recorded, is the deployer's to write. A file with no row, or one whose
/// bytes have changed since, belongs to the user.
fn may_write_copy(rows: &[DeploymentFile], path: &str) -> bool {
    match (hash_if_present(Path::new(path)), row_at(rows, path)) {
        (Ok(None), None) => true,
        (Ok(None), Some(row)) => row.role == PRESERVED_COPY,
        (Ok(Some(on_disk)), Some(row)) => row.role == PRESERVED_COPY && row.hash == on_disk,
        _ => false,
    }
}

fn plan_new_copies(
    plan: &DeployPlan,
    desired: &DesiredSet,
    rows: &[DeploymentFile],
) -> (Vec<PlannedCopy>, Vec<String>) {
    let mut planned = Vec::new();
    let mut skipped = Vec::new();
    for entry in &plan.entries {
        let PlanEntry::Preserve {
            path,
            source,
            expected_hash,
            mod_version_id,
            rel_path,
            ..
        } = entry
        else {
            continue;
        };
        if !Path::new(source).is_file() {
            continue;
        }
        let base = row_at(rows, path).map_or(path.as_str(), |row| row.path.as_str());
        let copy = write::new_copy_path(Path::new(base))
            .to_string_lossy()
            .into_owned();
        let key = path_key(&copy);
        if desired.files.iter().any(|d| path_key(&d.path) == key) {
            continue;
        }
        if !may_write_copy(rows, &copy) {
            skipped.push(copy);
            continue;
        }
        planned.push(PlannedCopy {
            copy: JournalledNewCopy {
                path: copy,
                hash: expected_hash.clone(),
                mod_version_id: mod_version_id.clone(),
                rel_path: format!("{rel_path}{}", write::NEW_COPY_SUFFIX),
            },
            source: source.clone(),
        });
    }
    (planned, skipped)
}

/// Writes each planned copy that is still the deployer's to write, then releases
/// every recorded copy whose base file is no longer preserved: deleted while it
/// still holds the recorded bytes, otherwise left to the user and only its row
/// forgotten. A copy the user changed beside a file that is still preserved loses
/// its row too, or the row would pin its version for good.
async fn settle_new_copies(
    db: &dyn ps_db::DbDriver,
    target: &ModTarget,
    plan: &DeployPlan,
    desired: &DesiredSet,
    planned: &[PlannedCopy],
    written: &mut Vec<String>,
    skipped: &mut Vec<String>,
) -> Result<(), ApplyError> {
    let rows = ps_db::mod_deployments::files_of(db, &target.id).await?;
    for PlannedCopy { copy, source } in planned {
        if !may_write_copy(&rows, &copy.path) {
            skipped.push(copy.path.clone());
            continue;
        }
        let row = row_at(&rows, &copy.path);
        let current = row.is_some_and(|r| r.hash == copy.hash)
            && hash_if_present(Path::new(&copy.path))?.as_deref() == Some(copy.hash.as_str());
        if !current {
            write::copy_staged(Path::new(source), Path::new(&copy.path))?;
        }
        record_recovered(
            db,
            target,
            row.map_or(copy.path.as_str(), |r| r.path.as_str()),
            &copy.mod_version_id,
            &copy.rel_path,
            &copy.hash,
            Role::PreservedCopy,
        )
        .await?;
        written.push(copy.path.clone());
    }

    let preserved: HashSet<String> = plan
        .entries
        .iter()
        .filter_map(|entry| match entry {
            PlanEntry::Preserve { path, .. } => Some(path_key(path)),
            _ => None,
        })
        .collect();
    let settled: HashSet<String> = written
        .iter()
        .map(|path| path_key(path))
        .chain(desired.files.iter().map(|d| path_key(&d.path)))
        .collect();
    for row in rows.iter().filter(|row| row.role == PRESERVED_COPY) {
        if settled.contains(&path_key(&row.path)) {
            continue;
        }
        let Some(base) = row.path.strip_suffix(write::NEW_COPY_SUFFIX) else {
            continue;
        };
        if !preserved.contains(&path_key(base)) {
            release(db, target, row).await?;
            continue;
        }
        if let Ok(Some(on_disk)) = hash_if_present(Path::new(&row.path)) {
            if on_disk != row.hash {
                ps_db::mod_deployments::forget(db, &target.id, &[row.path.as_str()]).await?;
            }
        }
    }
    Ok(())
}

/// Lets go of a recorded file the deployer no longer writes. The file is deleted
/// only while it still holds the bytes its row recorded; one somebody changed
/// since is left in place, and one that cannot be read keeps its row.
async fn release(
    db: &dyn ps_db::DbDriver,
    target: &ModTarget,
    row: &DeploymentFile,
) -> Result<(), ApplyError> {
    let file = Path::new(&row.path);
    match hash_if_present(file) {
        Ok(Some(on_disk)) if on_disk == row.hash => {
            write::remove_if_present(file)?;
        }
        Ok(_) => {}
        Err(_) => return Ok(()),
    }
    ps_db::mod_deployments::forget(db, &target.id, &[row.path.as_str()]).await?;
    Ok(())
}

/// The spelling `deployment_files` already holds for this file, or `path` when no
/// row names it. `ps_db::record` upserts on the exact text, so recording another
/// spelling would give one file two rows, and the spelling on disk is the old one:
/// a case-insensitive filesystem keeps the case a directory was created with.
fn recorded_spelling<'a>(recorded: &'a [RecordedFile], path: &'a str) -> &'a str {
    let key = path_key(path);
    recorded
        .iter()
        .find(|r| path_key(&r.path) == key)
        .map_or(path, |r| r.path.as_str())
}

/// Also called from `recover.rs`, which lives one module over: the row upsert
/// a recovered write needs is exactly the one a live write needs.
pub(super) async fn record_recovered(
    db: &dyn ps_db::DbDriver,
    target: &ModTarget,
    path: &str,
    mod_version_id: &str,
    rel_path: &str,
    hash: &str,
    role: Role,
) -> Result<(), ApplyError> {
    ps_db::mod_deployments::record(
        db,
        &[ps_db::mod_deployments::NewDeploymentFile {
            target_id: target.id.clone(),
            path: path.to_string(),
            mod_version_id: Some(mod_version_id.to_string()),
            hash: hash.to_string(),
            role: role_name(role).to_string(),
            rel_path: Some(rel_path.to_string()),
        }],
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod kind_for_path_tests {
    use super::*;
    use ps_core::mods::{Platform, TargetSpec, Ue4ssMode};

    #[test]
    fn a_sibling_sharing_the_bases_prefix_is_outside_it() {
        let layout = resolve_layout(&TargetSpec {
            kind: TargetKind::Client,
            root: "/g".to_string(),
            platform: Platform::Win64,
            ue4ss_mode: Ue4ssMode::Standard,
            overrides: Default::default(),
        })
        .unwrap();
        let mods = layout.ue4ss_mods_dir.clone().unwrap();
        let sibling = mods.with_file_name(format!(
            "{}Extra",
            mods.file_name().unwrap().to_string_lossy()
        ));

        assert_eq!(
            kind_for_path(&layout, &layout, &mods.join("x")),
            RouteKind::Ue4ss
        );
        assert_ne!(
            kind_for_path(&layout, &layout, &sibling.join("x")),
            RouteKind::Ue4ss,
            "{}",
            sibling.display()
        );
    }

    #[test]
    fn framework_kinds_are_recovered_from_the_binaries_and_ue4ss_runtime_dirs() {
        let layout = resolve_layout(&TargetSpec {
            kind: TargetKind::Client,
            root: "/g".to_string(),
            platform: Platform::Win64,
            ue4ss_mode: Ue4ssMode::Standard,
            overrides: Default::default(),
        })
        .unwrap();
        let root = Path::new("/g");

        assert_eq!(
            kind_for_path(
                &layout,
                &layout,
                &root.join("Pal/Binaries/Win64/dwmapi.dll")
            ),
            RouteKind::Binaries
        );
        assert_eq!(
            kind_for_path(
                &layout,
                &layout,
                &root.join("Pal/Binaries/Win64/ue4ss/UE4SS.dll")
            ),
            RouteKind::Ue4ssCore
        );
        assert_eq!(
            kind_for_path(
                &layout,
                &layout,
                &root.join("Pal/Binaries/Win64/ue4ss/Mods/X/a.lua")
            ),
            RouteKind::Ue4ss
        );
    }
}
