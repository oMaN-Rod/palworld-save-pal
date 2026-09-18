//! Comparing what a bound target's active profile expects against what Amity
//! reports as loaded. `classify` is pure; everything else here is the impure
//! half that reads the database and holds the last result per target.
use std::collections::BTreeMap;

use ps_core::mods::{InstallManifest, ModType, RouteKind};
use ps_db::mod_targets::ModTarget;
use ps_db::DbError;
use serde_json::Value;

const DEFAULT_UE4SS_BUILTINS: &[&str] = &[
    "BPModLoaderMod",
    "BPML_GenericFunctions",
    "Keybinds",
    "ConsoleCommandsMod",
    "ConsoleEnablerMod",
    "CheatManagerEnablerMod",
    "LineTraceMod",
    "SplitScreenMod",
    "shared",
];

const FRAMEWORK_UE4SS_MOD_ID: &str = "framework-ue4ss";

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerifyStatus {
    Missing,
    Unexpected,
    Verified,
    Unknown,
}

impl VerifyStatus {
    fn rank(self) -> u8 {
        match self {
            VerifyStatus::Missing => 0,
            VerifyStatus::Unexpected => 1,
            VerifyStatus::Verified => 2,
            VerifyStatus::Unknown => 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ModVerification {
    pub mod_id: Option<String>,
    pub name: String,
    pub kind: String,
    pub status: VerifyStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expected {
    pub mod_id: String,
    pub name: String,
    pub kind: String,
    pub observable: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoadedMod {
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Resolution {
    pub complete: bool,
    pub missing: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct TargetVerification {
    pub target_id: String,
    pub instance_id: String,
    pub live: bool,
    pub checked_at: String,
    pub build_info: Option<Value>,
    pub resolution: Resolution,
    pub status: Vec<ModVerification>,
}

/// Compares the active profile's expectations against what Amity reports as
/// loaded. `builtins` are UE4SS's own always-loaded mods, never reported as
/// unexpected; `ue4ss_answered` is whether `get_loaded_mods` answered at all,
/// which is what proves the UE4SS framework slot itself is running.
pub fn classify(
    expected: &[Expected],
    loaded: &[LoadedMod],
    builtins: &[String],
    ue4ss_answered: bool,
) -> Vec<ModVerification> {
    let mut out = Vec::with_capacity(expected.len());

    for entry in expected {
        let status = if entry.mod_id == FRAMEWORK_UE4SS_MOD_ID {
            if ue4ss_answered {
                VerifyStatus::Verified
            } else {
                VerifyStatus::Missing
            }
        } else if !entry.observable {
            VerifyStatus::Unknown
        } else if loaded
            .iter()
            .any(|l| l.enabled && l.name.eq_ignore_ascii_case(&entry.name))
        {
            VerifyStatus::Verified
        } else {
            VerifyStatus::Missing
        };
        out.push(ModVerification {
            mod_id: Some(entry.mod_id.clone()),
            name: entry.name.clone(),
            kind: entry.kind.clone(),
            status,
        });
    }

    for l in loaded {
        if !l.enabled {
            continue;
        }
        let known = expected
            .iter()
            .any(|e| e.name.eq_ignore_ascii_case(&l.name))
            || builtins.iter().any(|b| b.eq_ignore_ascii_case(&l.name))
            || l.name.eq_ignore_ascii_case("PSAmity");
        if known {
            continue;
        }
        out.push(ModVerification {
            mod_id: None,
            name: l.name.clone(),
            kind: "ue4ss".to_string(),
            status: VerifyStatus::Unexpected,
        });
    }

    out.sort_by(|a, b| (a.status.rank(), &a.name).cmp(&(b.status.rank(), &b.name)));
    out
}

pub fn parse_loaded(value: &Value) -> Vec<LoadedMod> {
    value
        .get("ue4ss")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let name = item.get("name")?.as_str()?.to_string();
                    let enabled = item.get("enabled")?.as_bool()?;
                    Some(LoadedMod { name, enabled })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn parse_resolution(value: &Value) -> Resolution {
    let complete = value
        .get("complete")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let missing = value
        .get("missing")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    Resolution { complete, missing }
}

/// The active profile's enabled mods and framework slots for a target,
/// translated into what Amity should be reporting as loaded. Returns the
/// UE4SS built-ins to exempt from "unexpected" alongside the expectations,
/// since both come from the same UE4SS framework version.
pub async fn expected_for_target(
    db: &dyn ps_db::DbDriver,
    target: &ModTarget,
) -> Result<(Vec<Expected>, Vec<String>), DbError> {
    let mut expected = Vec::new();
    let mut builtins: Option<Vec<String>> = None;

    let Some(profile) = ps_db::mod_profiles::active_for_target(db, &target.id).await? else {
        return Ok((expected, default_builtins()));
    };

    for entry in ps_db::mod_profiles::mods_of(db, &profile.id).await? {
        if !entry.enabled {
            continue;
        }
        let version = match &entry.mod_version_id {
            Some(version_id) => ps_db::mod_library::get_version(db, version_id).await?,
            None => ps_db::mod_library::current_version(db, &entry.mod_id).await?,
        };
        let Some(version) = version else { continue };
        let Ok(manifest) = serde_json::from_str::<InstallManifest>(&version.manifest) else {
            continue;
        };
        let Some(row) = ps_db::mod_library::get_mod(db, &entry.mod_id).await? else {
            continue;
        };
        let observable = matches!(manifest.mod_type, ModType::Ue4ss | ModType::Hybrid);
        let name = if observable {
            manifest.folder_name.clone()
        } else {
            row.custom_name.clone().unwrap_or_else(|| row.name.clone())
        };
        expected.push(Expected {
            mod_id: row.id.clone(),
            name,
            kind: row.mod_type.clone(),
            observable,
        });
    }

    for framework in ps_db::mod_profiles::frameworks_of(db, &target.id).await? {
        match framework.framework.as_str() {
            "amity" => expected.push(Expected {
                mod_id: "framework-amity".to_string(),
                name: "PSAmity".to_string(),
                kind: "framework".to_string(),
                observable: true,
            }),
            "palschema" => expected.push(Expected {
                mod_id: "framework-palschema".to_string(),
                name: "PalSchema".to_string(),
                kind: "framework".to_string(),
                observable: true,
            }),
            "ue4ss" => {
                expected.push(Expected {
                    mod_id: FRAMEWORK_UE4SS_MOD_ID.to_string(),
                    name: "UE4SS".to_string(),
                    kind: "framework".to_string(),
                    observable: false,
                });
                if let Some(version) =
                    ps_db::mod_library::get_version(db, &framework.mod_version_id).await?
                {
                    if let Ok(manifest) = serde_json::from_str::<InstallManifest>(&version.manifest)
                    {
                        let mut roots: Vec<String> = manifest
                            .routes
                            .iter()
                            .filter(|r| r.kind == RouteKind::Ue4ss)
                            .filter_map(|r| r.rel_path.split('/').next())
                            .map(str::to_string)
                            .collect();
                        roots.sort();
                        roots.dedup();
                        if !roots.is_empty() {
                            builtins = Some(roots);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    Ok((expected, builtins.unwrap_or_else(default_builtins)))
}

fn default_builtins() -> Vec<String> {
    DEFAULT_UE4SS_BUILTINS
        .iter()
        .map(|s| s.to_string())
        .collect()
}

/// The last verification result per target, published by `run_verifier` and
/// read by the `mod_verification_*` handlers. A target that loses its bridge
/// connection keeps its last result here with `live: false`, rather than
/// disappearing.
pub struct VerificationStore {
    tx: tokio::sync::watch::Sender<BTreeMap<String, TargetVerification>>,
}

impl VerificationStore {
    pub fn new() -> Self {
        let (tx, _rx) = tokio::sync::watch::channel(BTreeMap::new());
        Self { tx }
    }

    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<BTreeMap<String, TargetVerification>> {
        self.tx.subscribe()
    }

    pub fn get(&self, target_id: &str) -> Option<TargetVerification> {
        self.tx.borrow().get(target_id).cloned()
    }

    pub fn publish(&self, verification: TargetVerification) {
        self.tx.send_if_modified(|map| {
            let changed = map.get(&verification.target_id) != Some(&verification);
            if changed {
                map.insert(verification.target_id.clone(), verification.clone());
            }
            changed
        });
    }

    pub fn mark_offline(&self, target_id: &str) {
        self.tx
            .send_if_modified(|map| match map.get_mut(target_id) {
                Some(entry) if entry.live => {
                    entry.live = false;
                    true
                }
                _ => false,
            });
    }

    /// Drops a removed target's entry. Subscribers only ever push entries
    /// present in the map, so removing one silently needs no special case.
    pub fn remove(&self, target_id: &str) {
        self.tx
            .send_if_modified(|map| map.remove(target_id).is_some());
    }
}

impl Default for VerificationStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expected(mod_id: &str, name: &str, kind: &str, observable: bool) -> Expected {
        Expected {
            mod_id: mod_id.to_string(),
            name: name.to_string(),
            kind: kind.to_string(),
            observable,
        }
    }

    fn loaded(name: &str, enabled: bool) -> LoadedMod {
        LoadedMod {
            name: name.to_string(),
            enabled,
        }
    }

    fn builtins() -> Vec<String> {
        vec!["BPModLoaderMod".to_string()]
    }

    #[test]
    fn an_observable_mod_loaded_and_enabled_is_verified() {
        let expected = vec![expected("coolmod", "CoolMod", "ue4ss", true)];
        let loaded = vec![loaded("CoolMod", true)];
        let result = classify(&expected, &loaded, &builtins(), true);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].status, VerifyStatus::Verified);
        assert_eq!(result[0].mod_id.as_deref(), Some("coolmod"));
    }

    #[test]
    fn matching_is_case_insensitive() {
        let expected = vec![expected("coolmod", "CoolMod", "ue4ss", true)];
        let loaded = vec![loaded("coolmod", true)];
        let result = classify(&expected, &loaded, &builtins(), true);
        assert_eq!(result[0].status, VerifyStatus::Verified);
    }

    #[test]
    fn a_disabled_or_absent_loaded_entry_is_missing() {
        let expected = vec![expected("coolmod", "CoolMod", "ue4ss", true)];
        let disabled = vec![loaded("CoolMod", false)];
        assert_eq!(
            classify(&expected, &disabled, &builtins(), true)[0].status,
            VerifyStatus::Missing
        );
        assert_eq!(
            classify(&expected, &[], &builtins(), true)[0].status,
            VerifyStatus::Missing
        );
    }

    #[test]
    fn an_unobservable_mod_is_always_unknown() {
        let expected = vec![expected("schema-thing", "SchemaThing", "palschema", false)];
        assert_eq!(
            classify(&expected, &[], &builtins(), true)[0].status,
            VerifyStatus::Unknown
        );
        let loaded = vec![loaded("SchemaThing", true)];
        assert_eq!(
            classify(&expected, &loaded, &builtins(), true)[0].status,
            VerifyStatus::Unknown
        );
    }

    #[test]
    fn a_loaded_enabled_stray_mod_is_unexpected_with_no_mod_id() {
        let expected = Vec::new();
        let loaded = vec![
            loaded("Stray", true),
            loaded("BPModLoaderMod", true),
            loaded("Stray2", false),
        ];
        let result = classify(&expected, &loaded, &builtins(), true);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "Stray");
        assert_eq!(result[0].mod_id, None);
        assert_eq!(result[0].kind, "ue4ss");
        assert_eq!(result[0].status, VerifyStatus::Unexpected);
    }

    #[test]
    fn a_loaded_psamity_is_never_unexpected() {
        let expected = Vec::new();
        let loaded = vec![loaded("PSAmity", true)];
        let result = classify(&expected, &loaded, &builtins(), true);
        assert!(result.is_empty());
    }

    #[test]
    fn the_ue4ss_framework_slot_is_verified_only_when_it_answered() {
        let expected = vec![expected("framework-ue4ss", "UE4SS", "framework", false)];
        assert_eq!(
            classify(&expected, &[], &builtins(), true)[0].status,
            VerifyStatus::Verified
        );
        assert_eq!(
            classify(&expected, &[], &builtins(), false)[0].status,
            VerifyStatus::Missing
        );
    }

    #[test]
    fn results_are_ordered_missing_then_unexpected_then_verified_then_unknown_by_name() {
        let expected = vec![
            expected("z-mod", "ZMod", "ue4ss", true),
            expected("a-mod", "AMod", "ue4ss", true),
            expected("schema", "Schema", "palschema", false),
        ];
        let loaded = vec![loaded("AMod", true), loaded("Intruder", true)];
        let result = classify(&expected, &loaded, &builtins(), true);
        let names: Vec<&str> = result.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, vec!["ZMod", "Intruder", "AMod", "Schema"]);
        let statuses: Vec<VerifyStatus> = result.iter().map(|m| m.status).collect();
        assert_eq!(
            statuses,
            vec![
                VerifyStatus::Missing,
                VerifyStatus::Unexpected,
                VerifyStatus::Verified,
                VerifyStatus::Unknown,
            ]
        );
    }

    fn verification(target_id: &str) -> TargetVerification {
        TargetVerification {
            target_id: target_id.to_string(),
            instance_id: "auto:1".to_string(),
            live: true,
            checked_at: "2026-09-15T00:00:00Z".to_string(),
            build_info: None,
            resolution: Resolution {
                complete: true,
                missing: Vec::new(),
            },
            status: Vec::new(),
        }
    }

    #[test]
    fn remove_drops_the_targets_entry_without_leaving_a_trace() {
        let store = VerificationStore::new();
        store.publish(verification("t1"));
        assert!(store.get("t1").is_some());
        store.remove("t1");
        assert!(store.get("t1").is_none());
    }

    #[test]
    fn removing_an_absent_target_is_a_no_op() {
        let store = VerificationStore::new();
        store.remove("nope");
        assert!(store.get("nope").is_none());
    }
}
