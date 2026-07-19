use psp_core::gamedata::GameData;
use psp_core::progress::null_progress;
use psp_core::session::{PlayerFileData, SaveKind, SaveSession};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use uuid::Uuid;

/// Loads the committed rich `v1_relics` fixture (10 players) as the corpus
/// under test. Not env-gated and never skips: panics on failure, since a
/// missing or broken checked-in fixture is a repo problem.
#[allow(dead_code)]
pub fn load_corpus_session() -> SaveSession {
    load_fixture_session("v1_relics")
}

/// Loads a committed fixture save from `tests/fixtures/saves/<name>/`. Never
/// env-gated, so tests built on it always run; panics on failure, since a
/// missing or broken checked-in fixture is a repo problem, not a skip.
pub fn load_fixture_session(name: &str) -> SaveSession {
    let save_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/saves")
        .join(name);
    let level_sav_bytes =
        std::fs::read(save_dir.join("Level.sav")).expect("read fixture Level.sav");
    let level_meta_bytes = std::fs::read(save_dir.join("LevelMeta.sav")).ok();

    let mut player_file_refs: BTreeMap<Uuid, PlayerFileData> = BTreeMap::new();
    if let Ok(entries) = std::fs::read_dir(save_dir.join("Players")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "sav") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let (uid_part, is_dps) = match stem.strip_suffix("_dps") {
                Some(base) => (base, true),
                None => (stem, false),
            };
            let Ok(uid) = uid_part.parse::<Uuid>() else {
                continue;
            };
            let file_ref = player_file_refs
                .entry(uid)
                .or_insert(PlayerFileData::Paths {
                    sav: None,
                    dps: None,
                });
            if let PlayerFileData::Paths { sav, dps } = file_ref {
                if is_dps {
                    *dps = Some(path);
                } else {
                    *sav = Some(path);
                }
            }
        }
    }

    SaveSession::load(
        SaveKind::Steam {
            level_path: save_dir.join("Level.sav"),
        },
        save_dir.to_string_lossy().into_owned(),
        "steam",
        &level_sav_bytes,
        level_meta_bytes.as_deref(),
        None,
        player_file_refs,
        None,
        true,
        &psp_core::progress::null_progress(),
    )
    .expect("load fixture session")
}

/// The player's `.sav` as JSON, for structural assertions the DTO does not
/// expose. Serialized through the real writer and re-parsed, so anything these
/// helpers see is genuinely what would land on disk.
#[allow(dead_code)]
pub fn player_sav_json(session: &SaveSession, player_id: Uuid) -> serde_json::Value {
    let player_files = session.player_sav_bytes().expect("serialize player savs");
    let (sav_bytes, _dps) = player_files.get(&player_id).expect("player is loaded");
    let save = psp_core::savio::read_sav_bytes(sav_bytes).expect("parse player sav");
    serde_json::to_value(&save).expect("player sav to json")
}

/// Finds the property whose key is `name` -- uesave serializes a `PropertyKey`
/// as `<name>_<index>`, so the `_` guard is what keeps
/// `RelicObtainForInstanceFlag` from matching `RelicObtainForInstanceFlagByType`.
#[allow(dead_code)]
fn find<'a>(v: &'a serde_json::Value, name: &str) -> Option<&'a serde_json::Value> {
    match v {
        serde_json::Value::Object(m) => {
            for (k, val) in m {
                if k.starts_with(name) && k[name.len()..].starts_with('_') {
                    return Some(val);
                }
                if let Some(hit) = find(val, name) {
                    return Some(hit);
                }
            }
            None
        }
        serde_json::Value::Array(a) => a.iter().find_map(|x| find(x, name)),
        _ => None,
    }
}

/// The legacy flat `RelicObtainForInstanceFlag` map: every key flagged `true`.
#[allow(dead_code)]
pub fn relic_flat_flags(sav: &serde_json::Value) -> BTreeSet<String> {
    let Some(entries) = find(sav, "RelicObtainForInstanceFlag").and_then(|v| v.as_array()) else {
        return BTreeSet::new();
    };
    // Guard against `find` having walked into `...FlagByType`, whose entries are
    // `Type`/`Flags` structs rather than map `key`/`value` pairs.
    for entry in entries {
        assert!(
            entry.get("key").is_some() && entry.get("value").is_some(),
            "relic_flat_flags matched the wrong property: entries are not key/value \
             map entries but {entry}"
        );
    }
    entries
        .iter()
        .filter(|e| e["value"].as_bool() == Some(true))
        .filter_map(|e| e["key"].as_str().map(str::to_string))
        .collect()
}

/// The 1.0 `RelicObtainForInstanceFlagByType` array, as relic type -> `true` flags.
#[allow(dead_code)]
pub fn relic_by_type_flags(sav: &serde_json::Value) -> BTreeMap<String, BTreeSet<String>> {
    let mut out = BTreeMap::new();
    let Some(entries) = find(sav, "RelicObtainForInstanceFlagByType").and_then(|v| v.as_array())
    else {
        return out;
    };
    for entry in entries {
        let Some(ty) = find(entry, "Type").and_then(|v| v.as_str()) else {
            continue;
        };
        let flags: BTreeSet<String> = find(entry, "Flags")
            .and_then(|v| v.as_array())
            .map(|f| {
                f.iter()
                    .filter(|e| e["value"].as_bool() == Some(true))
                    .filter_map(|e| e["key"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        out.insert(ty.to_string(), flags);
    }
    out
}

#[allow(dead_code)]
pub fn relic_possess_num(sav: &serde_json::Value) -> i64 {
    find(sav, "RelicPossessNum")
        .and_then(|v| v.as_i64())
        .unwrap_or(0)
}

#[allow(dead_code)]
pub fn relic_possess_num_map(sav: &serde_json::Value) -> BTreeMap<String, i64> {
    find(sav, "RelicPossessNumMap")
        .and_then(|v| v.as_array())
        .map(|entries| {
            entries
                .iter()
                .filter_map(|e| Some((e["key"].as_str()?.to_string(), e["value"].as_i64()?)))
                .collect()
        })
        .unwrap_or_default()
}

#[allow(dead_code)]
pub fn relic_bonus_exp_table_index(sav: &serde_json::Value) -> i64 {
    find(sav, "RelicBonusExpTableIndex")
        .and_then(|v| v.as_i64())
        .unwrap_or(0)
}

/// Ordered variant of `relic_flat_flags`: an on-disk-ordered `Vec` rather than a
/// `BTreeSet`. `relic_flat_flags`'s set comparison would pass even if a write silently
/// sorted or reordered the map -- this is the helper that actually catches that.
#[allow(dead_code)]
pub fn relic_flat_flags_ordered(sav: &serde_json::Value) -> Vec<String> {
    let Some(entries) = find(sav, "RelicObtainForInstanceFlag").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    entries
        .iter()
        .filter(|e| e["value"].as_bool() == Some(true))
        .filter_map(|e| e["key"].as_str().map(str::to_string))
        .collect()
}

/// Ordered variant of `relic_by_type_flags`: each type's `Flags` as an on-disk-ordered
/// `Vec` rather than a `BTreeSet`. See `relic_flat_flags_ordered`.
#[allow(dead_code)]
pub fn relic_by_type_flags_ordered(sav: &serde_json::Value) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    let Some(entries) = find(sav, "RelicObtainForInstanceFlagByType").and_then(|v| v.as_array())
    else {
        return out;
    };
    for entry in entries {
        let Some(ty) = find(entry, "Type").and_then(|v| v.as_str()) else {
            continue;
        };
        let flags: Vec<String> = find(entry, "Flags")
            .and_then(|v| v.as_array())
            .map(|f| {
                f.iter()
                    .filter(|e| e["value"].as_bool() == Some(true))
                    .filter_map(|e| e["key"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        out.insert(ty.to_string(), flags);
    }
    out
}

/// The first fixture player that actually carries the 1.0 by-type relic
/// structures. Loads each player's details, since only a loaded player has a
/// `.sav` to inspect.
#[allow(dead_code)]
pub fn first_player_with_relics(session: &mut SaveSession, data: &GameData) -> Uuid {
    let ids: Vec<Uuid> = session.player_file_refs.keys().copied().collect();
    for id in ids {
        if psp_core::domain::player::get_player_details(session, data, id, &null_progress())
            .ok()
            .flatten()
            .is_none()
        {
            continue;
        }
        let sav = player_sav_json(session, id);
        if !relic_by_type_flags(&sav).is_empty() {
            return id;
        }
    }
    panic!("no fixture player carries by-type relic structures");
}

/// The relic type every effigy grants; the only one the legacy flat fields mirror.
#[allow(dead_code)]
pub const CAPTURE_POWER_RELIC: &str = "EPalRelicType::CapturePower";

/// The first fixture player carrying a *non*-CapturePower relic type with at least one
/// true flag. Only such a player can witness a regression that wipes the other relic
/// types' flag sets: an effigy unlock touches CapturePower alone, so every other type's
/// flags must survive the write byte for byte.
#[allow(dead_code)]
pub fn first_player_with_non_capture_power_relics(
    session: &mut SaveSession,
    data: &GameData,
) -> Uuid {
    let ids: Vec<Uuid> = session.player_file_refs.keys().copied().collect();
    for id in ids {
        if psp_core::domain::player::get_player_details(session, data, id, &null_progress())
            .ok()
            .flatten()
            .is_none()
        {
            continue;
        }
        let sav = player_sav_json(session, id);
        let has_other = relic_by_type_flags(&sav)
            .iter()
            .any(|(ty, flags)| ty != CAPTURE_POWER_RELIC && !flags.is_empty());
        if has_other {
            return id;
        }
    }
    panic!("no fixture player carries a non-CapturePower relic type with flags");
}
