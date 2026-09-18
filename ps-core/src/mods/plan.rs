use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::paths::normalize_physical_path;
use super::types::{Role, RouteKind};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesiredFile {
    pub path: String,
    pub mod_version_id: String,
    pub rel_path: String,
    pub kind: RouteKind,
    pub expected_hash: String,
    pub role: Role,
    pub source: String,
}

/// `kind` has to be carried over from the desired set that produced the file. No
/// `PlanEntry` variant carries it, so a deployer writing its records from plan
/// entries alone has no route kind to store — and a defaulted `kind` stops the
/// move key matching, which silently degrades every move into a remove plus an
/// add.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedFile {
    pub path: String,
    pub mod_version_id: Option<String>,
    pub rel_path: Option<String>,
    pub kind: RouteKind,
    pub hash: String,
    pub role: Role,
}

/// What is on the target now: the hash of each path, or `None` where the path
/// holds no file. A path absent from the map is read as holding no file, so the
/// caller must include every path the plan could touch (see [`build_plan`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiskState {
    pub hashes: HashMap<String, Option<String>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JournalHints {
    pub intended: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum PlanEntry {
    Keep {
        path: String,
    },
    Reattribute {
        path: String,
        mod_version_id: String,
        rel_path: String,
    },
    Replace {
        path: String,
        source: String,
        expected_hash: String,
        mod_version_id: String,
        rel_path: String,
        role: Role,
    },
    Preserve {
        path: String,
        source: String,
        expected_hash: String,
        mod_version_id: String,
        rel_path: String,
        role: Role,
    },
    Add {
        path: String,
        source: String,
        expected_hash: String,
        mod_version_id: String,
        rel_path: String,
        role: Role,
    },
    Move {
        from: String,
        to: String,
        mod_version_id: String,
        rel_path: String,
        /// The source's hash on disk, which is what arrives at `to`.
        hash: String,
        role: Role,
        /// The source row's hash, which the destination row takes over. A move
        /// carries a user's edit, and recording `hash` instead would make that
        /// edit read as app-written.
        #[serde(default)]
        recorded_hash: String,
    },
    Remove {
        path: String,
    },
    RemovePreserve {
        path: String,
        hash: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeployPlan {
    pub entries: Vec<PlanEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PreflightError {
    #[error("two mod versions resolve to {path}: {mod_version_ids:?}")]
    DestinationConflict {
        path: String,
        mod_version_ids: Vec<String>,
    },
    #[error("unmanaged files occupy destinations: {paths:?}")]
    UnmanagedOccupant { paths: Vec<String> },
}

impl DiskState {
    fn hash_of(&self, path: &str) -> Option<&str> {
        self.hashes.get(path).and_then(|h| h.as_deref())
    }
}

fn app_written(on_disk: &str, recorded: &str, journal: &JournalHints, path: &str) -> bool {
    on_disk == recorded || journal.intended.get(path).is_some_and(|h| h == on_disk)
}

fn write_entry(kind: &str, d: &DesiredFile) -> PlanEntry {
    let (path, source, expected_hash, mod_version_id, rel_path, role) = (
        d.path.clone(),
        d.source.clone(),
        d.expected_hash.clone(),
        d.mod_version_id.clone(),
        d.rel_path.clone(),
        d.role,
    );
    match kind {
        "add" => PlanEntry::Add {
            path,
            source,
            expected_hash,
            mod_version_id,
            rel_path,
            role,
        },
        "replace" => PlanEntry::Replace {
            path,
            source,
            expected_hash,
            mod_version_id,
            rel_path,
            role,
        },
        _ => PlanEntry::Preserve {
            path,
            source,
            expected_hash,
            mod_version_id,
            rel_path,
            role,
        },
    }
}

/// Classify every desired and recorded file into one plan entry each.
///
/// `disk` must carry a key for every desired path, every recorded path, and
/// every path a move could reach. A missing key reads as "no file there", which
/// turns a recorded file into an unconditional `Remove` and lets an `Add` past
/// the occupancy gate onto a path that is not in fact empty — so an incomplete
/// `DiskState` is the one way a caller can make this function lose a file.
///
/// Shared markers must be left out of `desired` entirely. They are filtered out
/// of `recorded` by a null mod version, but `DesiredFile` cannot express one, and
/// a placeholder id would let a marker be classified `Preserve` — which the
/// design forbids, since markers are generated per target rather than diffed.
///
/// Paths are matched after `normalize_physical_path`, so `case_insensitive` has to
/// describe the filesystem being written to, not the target's game platform.
/// `DiskState` is still looked up by the caller's own strings.
pub fn build_plan(
    desired: &[DesiredFile],
    recorded: &[RecordedFile],
    disk: &DiskState,
    journal: &JournalHints,
    case_insensitive: bool,
) -> Result<DeployPlan, PreflightError> {
    let key = |path: &str| normalize_physical_path(path, case_insensitive);

    // Ownership: one desired owner per path.
    let mut owners: HashMap<String, (&str, Vec<&str>)> = HashMap::new();
    for d in desired {
        owners
            .entry(key(&d.path))
            .or_insert_with(|| (d.path.as_str(), Vec::new()))
            .1
            .push(d.mod_version_id.as_str());
    }
    let mut conflicts: Vec<&(&str, Vec<&str>)> =
        owners.values().filter(|(_, v)| v.len() > 1).collect();
    conflicts.sort();
    if let Some((path, ids)) = conflicts.first() {
        return Err(PreflightError::DestinationConflict {
            path: path.to_string(),
            mod_version_ids: ids.iter().map(|s| s.to_string()).collect(),
        });
    }

    let recorded: Vec<&RecordedFile> = recorded
        .iter()
        .filter(|r| r.mod_version_id.is_some())
        .collect();
    let recorded_by_path: HashMap<String, &RecordedFile> =
        recorded.iter().map(|r| (key(&r.path), *r)).collect();
    let desired_by_path: HashMap<String, &DesiredFile> =
        desired.iter().map(|d| (key(&d.path), d)).collect();

    // Moves: same (mod_version, rel_path, kind), different path, source present,
    // destination unrecorded.
    let mut moves: Vec<PlanEntry> = Vec::new();
    let mut moved_from: HashSet<String> = HashSet::new();
    let mut moved_to: HashSet<String> = HashSet::new();
    for d in desired {
        let d_key = key(&d.path);
        if recorded_by_path.contains_key(&d_key) {
            continue;
        }
        let candidate = recorded.iter().find(|r| {
            let r_key = key(&r.path);
            r.mod_version_id.as_deref() == Some(d.mod_version_id.as_str())
                && r.rel_path.as_deref() == Some(d.rel_path.as_str())
                && r.kind == d.kind
                && r_key != d_key
                && !desired_by_path.contains_key(&r_key)
                && !moved_from.contains(&r_key)
                && disk.hash_of(&r.path).is_some()
        });
        if let Some(r) = candidate {
            let hash = disk.hash_of(&r.path).unwrap_or(&r.hash).to_string();
            moves.push(PlanEntry::Move {
                from: r.path.clone(),
                to: d.path.clone(),
                mod_version_id: d.mod_version_id.clone(),
                rel_path: d.rel_path.clone(),
                hash,
                role: d.role,
                recorded_hash: r.hash.clone(),
            });
            moved_from.insert(key(&r.path));
            moved_to.insert(d_key);
        }
    }

    // Occupancy: every unowned destination we will write must be empty.
    let mut occupied: Vec<String> = desired
        .iter()
        .filter(|d| !recorded_by_path.contains_key(&key(&d.path)))
        .filter(|d| disk.hash_of(&d.path).is_some())
        .map(|d| d.path.clone())
        .collect();
    occupied.sort();
    occupied.dedup();
    if !occupied.is_empty() {
        return Err(PreflightError::UnmanagedOccupant { paths: occupied });
    }

    let mut remove_preserve = Vec::new();
    let mut remove = Vec::new();
    for r in &recorded {
        let r_key = key(&r.path);
        if desired_by_path.contains_key(&r_key) || moved_from.contains(&r_key) {
            continue;
        }
        match disk.hash_of(&r.path) {
            None => remove.push(PlanEntry::Remove {
                path: r.path.clone(),
            }),
            Some(on_disk) if app_written(on_disk, &r.hash, journal, &r.path) => {
                remove.push(PlanEntry::Remove {
                    path: r.path.clone(),
                })
            }
            Some(on_disk) => remove_preserve.push(PlanEntry::RemovePreserve {
                path: r.path.clone(),
                hash: on_disk.to_string(),
            }),
        }
    }

    let mut writes = Vec::new();
    let mut reattributes = Vec::new();
    let mut keeps = Vec::new();
    for d in desired {
        let d_key = key(&d.path);
        if moved_to.contains(&d_key) {
            continue;
        }
        let Some(r) = recorded_by_path.get(&d_key) else {
            writes.push(write_entry("add", d));
            continue;
        };
        let Some(on_disk) = disk.hash_of(&d.path) else {
            writes.push(write_entry("add", d));
            continue;
        };
        if on_disk == d.expected_hash {
            if r.mod_version_id.as_deref() == Some(d.mod_version_id.as_str()) {
                keeps.push(PlanEntry::Keep {
                    path: d.path.clone(),
                });
            } else {
                reattributes.push(PlanEntry::Reattribute {
                    path: d.path.clone(),
                    mod_version_id: d.mod_version_id.clone(),
                    rel_path: d.rel_path.clone(),
                });
            }
        } else if app_written(on_disk, &r.hash, journal, &d.path) {
            writes.push(write_entry("replace", d));
        } else {
            writes.push(write_entry("preserve", d));
        }
    }

    let mut entries = Vec::new();
    entries.extend(remove_preserve);
    entries.extend(remove);
    entries.extend(moves);
    entries.extend(writes);
    entries.extend(reattributes);
    entries.extend(keeps);
    Ok(DeployPlan { entries })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::{Role, RouteKind};
    use pretty_assertions::assert_eq;

    fn desired(path: &str, mv: &str, rel: &str, hash: &str) -> DesiredFile {
        DesiredFile {
            path: path.into(),
            mod_version_id: mv.into(),
            rel_path: rel.into(),
            kind: RouteKind::Ue4ss,
            expected_hash: hash.into(),
            role: Role::File,
            source: format!("/lib/{rel}"),
        }
    }
    fn recorded(path: &str, mv: &str, rel: &str, hash: &str) -> RecordedFile {
        RecordedFile {
            path: path.into(),
            mod_version_id: Some(mv.into()),
            rel_path: Some(rel.into()),
            kind: RouteKind::Ue4ss,
            hash: hash.into(),
            role: Role::File,
        }
    }
    fn disk(pairs: &[(&str, Option<&str>)]) -> DiskState {
        DiskState {
            hashes: pairs
                .iter()
                .map(|(p, h)| (p.to_string(), h.map(str::to_string)))
                .collect(),
        }
    }
    fn no_journal() -> JournalHints {
        JournalHints {
            intended: HashMap::new(),
        }
    }
    fn kinds(plan: &DeployPlan) -> Vec<&'static str> {
        plan.entries
            .iter()
            .map(|e| match e {
                PlanEntry::Keep { .. } => "keep",
                PlanEntry::Reattribute { .. } => "reattribute",
                PlanEntry::Replace { .. } => "replace",
                PlanEntry::Preserve { .. } => "preserve",
                PlanEntry::Add { .. } => "add",
                PlanEntry::Move { .. } => "move",
                PlanEntry::Remove { .. } => "remove",
                PlanEntry::RemovePreserve { .. } => "remove_preserve",
            })
            .collect()
    }

    #[test]
    fn keep_when_recorded_and_hash_matches() {
        let d = [desired("/t/a", "m@1", "a", "h1")];
        let r = [recorded("/t/a", "m@1", "a", "h1")];
        let plan = build_plan(&d, &r, &disk(&[("/t/a", Some("h1"))]), &no_journal(), false).unwrap();
        assert_eq!(kinds(&plan), ["keep"]);
    }

    #[test]
    fn reattribute_when_identical_file_belongs_to_new_version() {
        let d = [desired("/t/a", "m@2", "a", "h1")];
        let r = [recorded("/t/a", "m@1", "a", "h1")];
        let plan = build_plan(&d, &r, &disk(&[("/t/a", Some("h1"))]), &no_journal(), false).unwrap();
        assert!(
            matches!(&plan.entries[0], PlanEntry::Reattribute { mod_version_id, .. } if mod_version_id == "m@2")
        );
    }

    #[test]
    fn replace_when_app_written_and_preserve_when_user_edited() {
        let d = [desired("/t/a", "m@2", "a", "h2")];
        let r = [recorded("/t/a", "m@1", "a", "h1")];
        let plan = build_plan(&d, &r, &disk(&[("/t/a", Some("h1"))]), &no_journal(), false).unwrap();
        assert_eq!(kinds(&plan), ["replace"]);
        let plan = build_plan(&d, &r, &disk(&[("/t/a", Some("edited"))]), &no_journal(), false).unwrap();
        assert_eq!(kinds(&plan), ["preserve"]);
    }

    #[test]
    fn journal_intended_hash_counts_as_app_written() {
        let d = [desired("/t/a", "m@2", "a", "h3")];
        let r = [recorded("/t/a", "m@1", "a", "h1")];
        let j = JournalHints {
            intended: [("/t/a".to_string(), "h2".to_string())]
                .into_iter()
                .collect(),
        };
        let plan = build_plan(&d, &r, &disk(&[("/t/a", Some("h2"))]), &j, false).unwrap();
        assert_eq!(kinds(&plan), ["replace"]);
    }

    #[test]
    fn add_when_unrecorded_or_recorded_but_missing_on_disk() {
        let d = [desired("/t/a", "m@1", "a", "h1")];
        let plan = build_plan(&d, &[], &disk(&[("/t/a", None)]), &no_journal(), false).unwrap();
        assert_eq!(kinds(&plan), ["add"]);
        let r = [recorded("/t/a", "m@1", "a", "h1")];
        let plan = build_plan(&d, &r, &disk(&[("/t/a", None)]), &no_journal(), false).unwrap();
        assert_eq!(kinds(&plan), ["add"]);
    }

    #[test]
    fn remove_and_remove_preserve() {
        let r = [
            recorded("/t/a", "m@1", "a", "h1"),
            recorded("/t/b", "m@1", "b", "h1"),
        ];
        let plan = build_plan(
            &[],
            &r,
            &disk(&[("/t/a", Some("h1")), ("/t/b", Some("edited"))]),
            &no_journal(),
            false,
        )
        .unwrap();
        assert_eq!(kinds(&plan), ["remove_preserve", "remove"]);
        assert!(
            matches!(&plan.entries[0], PlanEntry::RemovePreserve { path, hash } if path == "/t/b" && hash == "edited")
        );
    }

    #[test]
    fn move_when_same_version_and_rel_path_resolve_elsewhere() {
        let d = [desired("/t/002_Foo/x.json", "m@1", "Foo/x.json", "h1")];
        let r = [recorded("/t/001_Foo/x.json", "m@1", "Foo/x.json", "h1")];
        let plan = build_plan(
            &d,
            &r,
            &disk(&[
                ("/t/001_Foo/x.json", Some("edited")),
                ("/t/002_Foo/x.json", None),
            ]),
            &no_journal(),
            false,
        )
        .unwrap();
        assert_eq!(kinds(&plan), ["move"]);
        assert!(
            matches!(&plan.entries[0], PlanEntry::Move { from, to, hash, recorded_hash, .. } if from == "/t/001_Foo/x.json" && to == "/t/002_Foo/x.json" && hash == "edited" && recorded_hash == "h1")
        );
    }

    #[test]
    fn move_source_missing_falls_back_to_add() {
        let d = [desired("/t/new/x", "m@1", "x", "h1")];
        let r = [recorded("/t/old/x", "m@1", "x", "h1")];
        let plan = build_plan(
            &d,
            &r,
            &disk(&[("/t/old/x", None), ("/t/new/x", None)]),
            &no_journal(),
            false,
        )
        .unwrap();
        assert_eq!(kinds(&plan), ["remove", "add"]);
    }

    #[test]
    fn destination_conflict_is_rejected_before_anything_else() {
        let d = [
            desired("/t/a", "m@1", "a", "h1"),
            desired("/t/a", "n@1", "a", "h2"),
        ];
        let err = build_plan(&d, &[], &disk(&[("/t/a", None)]), &no_journal(), false).unwrap_err();
        assert!(
            matches!(err, PreflightError::DestinationConflict { ref path, ref mod_version_ids } if path == "/t/a" && mod_version_ids.len() == 2)
        );
    }

    #[test]
    fn unmanaged_occupant_blocks_add_and_move_destinations() {
        let d = [desired("/t/a", "m@1", "a", "h1")];
        let err =
            build_plan(&d, &[], &disk(&[("/t/a", Some("someone"))]), &no_journal(), false).unwrap_err();
        assert!(
            matches!(err, PreflightError::UnmanagedOccupant { ref paths } if paths == &vec!["/t/a".to_string()])
        );
        let d = [desired("/t/new/x", "m@1", "x", "h1")];
        let r = [recorded("/t/old/x", "m@1", "x", "h1")];
        let err = build_plan(
            &d,
            &r,
            &disk(&[("/t/old/x", Some("h1")), ("/t/new/x", Some("stranger"))]),
            &no_journal(),
            false,
        )
        .unwrap_err();
        assert!(matches!(err, PreflightError::UnmanagedOccupant { .. }));
    }

    #[test]
    fn two_moves_never_claim_the_same_source() {
        let d = [
            desired("/t/new/a/x", "m@1", "x", "h1"),
            desired("/t/new/b/x", "m@1", "x", "h1"),
        ];
        let r = [
            recorded("/t/old/a/x", "m@1", "x", "h1"),
            recorded("/t/old/b/x", "m@1", "x", "h1"),
        ];
        let plan = build_plan(
            &d,
            &r,
            &disk(&[
                ("/t/old/a/x", Some("h1")),
                ("/t/old/b/x", Some("h1")),
                ("/t/new/a/x", None),
                ("/t/new/b/x", None),
            ]),
            &no_journal(),
            false,
        )
        .unwrap();
        assert_eq!(kinds(&plan), ["move", "move"]);
        let mut sources: Vec<&str> = plan
            .entries
            .iter()
            .filter_map(|e| match e {
                PlanEntry::Move { from, .. } => Some(from.as_str()),
                _ => None,
            })
            .collect();
        sources.sort();
        assert_eq!(sources, ["/t/old/a/x", "/t/old/b/x"]);
    }

    #[test]
    fn one_rel_path_under_two_route_kinds_is_two_keys() {
        let mut d0 = desired("/t/paks/Cool_P.pak", "m@1", "Cool_P.pak", "h1");
        d0.kind = RouteKind::Pak;
        let mut d1 = desired("/t/logic/Cool_P.pak", "m@1", "Cool_P.pak", "h1");
        d1.kind = RouteKind::LogicMods;
        let mut r0 = recorded("/t/old_paks/Cool_P.pak", "m@1", "Cool_P.pak", "h1");
        r0.kind = RouteKind::Pak;
        let mut r1 = recorded("/t/old_logic/Cool_P.pak", "m@1", "Cool_P.pak", "h1");
        r1.kind = RouteKind::LogicMods;
        let plan = build_plan(
            &[d0, d1],
            &[r0, r1],
            &disk(&[
                ("/t/old_paks/Cool_P.pak", Some("h1")),
                ("/t/old_logic/Cool_P.pak", Some("h1")),
                ("/t/paks/Cool_P.pak", None),
                ("/t/logic/Cool_P.pak", None),
            ]),
            &no_journal(),
            false,
        )
        .unwrap();
        let moves: Vec<(&str, &str)> = plan
            .entries
            .iter()
            .filter_map(|e| match e {
                PlanEntry::Move { from, to, .. } => Some((from.as_str(), to.as_str())),
                _ => None,
            })
            .collect();
        assert!(
            moves.contains(&("/t/old_paks/Cool_P.pak", "/t/paks/Cool_P.pak")),
            "{moves:?}"
        );
        assert!(
            moves.contains(&("/t/old_logic/Cool_P.pak", "/t/logic/Cool_P.pak")),
            "{moves:?}"
        );
    }

    #[test]
    fn journal_intended_hash_also_makes_a_removal_app_written() {
        let r = [recorded("/t/a", "m@1", "a", "h1")];
        let j = JournalHints {
            intended: [("/t/a".to_string(), "h2".to_string())]
                .into_iter()
                .collect(),
        };
        let plan = build_plan(&[], &r, &disk(&[("/t/a", Some("h2"))]), &j, false).unwrap();
        assert_eq!(kinds(&plan), ["remove"]);
    }

    #[test]
    fn shared_markers_are_ignored() {
        let r = [RecordedFile {
            path: "/t/mods.txt".into(),
            mod_version_id: None,
            rel_path: None,
            kind: RouteKind::Ue4ss,
            hash: "h".into(),
            role: Role::SharedMarker,
        }];
        let plan = build_plan(
            &[],
            &r,
            &disk(&[("/t/mods.txt", Some("zzz"))]),
            &no_journal(),
            false,
        )
        .unwrap();
        assert!(plan.entries.is_empty());
    }

    #[test]
    fn entries_come_out_in_execution_order() {
        let d = [
            desired("/t/keep", "m@1", "keep", "h"),
            desired("/t/add", "m@1", "add", "h"),
            desired("/t/moved_to", "m@1", "mv", "h"),
        ];
        let r = [
            recorded("/t/keep", "m@1", "keep", "h"),
            recorded("/t/gone", "m@1", "gone", "h"),
            recorded("/t/moved_from", "m@1", "mv", "h"),
        ];
        let plan = build_plan(
            &d,
            &r,
            &disk(&[
                ("/t/keep", Some("h")),
                ("/t/add", None),
                ("/t/gone", Some("h")),
                ("/t/moved_from", Some("h")),
                ("/t/moved_to", None),
            ]),
            &no_journal(),
            false,
        )
        .unwrap();
        assert_eq!(kinds(&plan), ["remove", "move", "add", "keep"]);
    }

    #[test]
    fn a_recorded_path_spelled_with_other_separators_is_still_owned() {
        let d = [desired(r"C:\t\mods\a.lua", "m@1", "a.lua", "h1")];
        let r = [recorded("C:/t/mods/a.lua", "m@1", "a.lua", "h1")];
        let on_disk = disk(&[
            (r"C:\t\mods\a.lua", Some("h1")),
            ("C:/t/mods/a.lua", Some("h1")),
        ]);
        let plan = build_plan(&d, &r, &on_disk, &no_journal(), false).unwrap();
        assert_eq!(kinds(&plan), ["keep"]);
    }

    #[test]
    fn a_case_difference_is_owned_only_on_a_case_insensitive_filesystem() {
        let d = [desired(r"C:\t\coolmod\a.lua", "m@1", "a.lua", "h1")];
        let r = [recorded("C:/t/CoolMod/a.lua", "m@1", "a.lua", "h1")];
        let on_disk = disk(&[
            (r"C:\t\coolmod\a.lua", Some("h1")),
            ("C:/t/CoolMod/a.lua", Some("h1")),
        ]);
        let plan = build_plan(&d, &r, &on_disk, &no_journal(), true).unwrap();
        assert_eq!(kinds(&plan), ["keep"]);
        let err = build_plan(&d, &r, &on_disk, &no_journal(), false).unwrap_err();
        assert!(
            matches!(err, PreflightError::UnmanagedOccupant { ref paths } if paths == &vec![r"C:\t\coolmod\a.lua".to_string()]),
            "{err:?}"
        );
    }
}
