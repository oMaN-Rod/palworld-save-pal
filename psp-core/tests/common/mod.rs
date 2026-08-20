use psp_core::domain::blueprint::validate::Anchor;
use psp_core::gamedata::GameData;
use psp_core::progress::null_progress;
use psp_core::session::{PlayerFileData, SaveKind, SaveSession};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use uuid::Uuid;

/// `GameData` is not reachable from a `SaveSession`; every domain consumer threads it
/// explicitly, so every test that reaches a domain call needs this.
#[allow(dead_code)]
pub fn game_data() -> GameData {
    let json_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/json");
    GameData::load(&json_dir).expect("data dir")
}

/// The nil uid is excluded deliberately: it is what scrubbing writes, and a leak scan
/// that included it would match every zeroed guid in the file and never fail.
#[allow(dead_code)]
pub fn all_player_uids(session: &SaveSession) -> Vec<Uuid> {
    let mut uids: BTreeSet<Uuid> = session.player_file_refs.keys().copied().collect();
    if let Ok(entries) = psp_core::domain::world::character_map(&session.level) {
        uids.extend(entries.iter().filter_map(psp_core::domain::world::entry_player_uid));
    }
    uids.remove(&Uuid::nil());
    uids.into_iter().collect()
}

/// Guild identity is invisible to `all_player_uids` -- it is not a player uid -- so a
/// leak scan built on that helper alone can never see it.
#[allow(dead_code)]
pub fn all_group_ids(session: &SaveSession) -> Vec<Uuid> {
    let mut ids: BTreeSet<Uuid> = psp_core::domain::world::group_map(&session.level)
        .map(|entries| entries.iter().filter_map(|entry| psp_core::props::as_uuid(&entry.key)).collect())
        .unwrap_or_default();
    ids.remove(&Uuid::nil());
    ids.into_iter().collect()
}

/// The committed rich `v1_relics` fixture (10 players), as the corpus under test.
#[allow(dead_code)]
pub fn load_corpus_session() -> SaveSession {
    load_fixture_session("v1_relics")
}

/// Loads a committed fixture save from `tests/fixtures/saves/<name>/`. Never env-gated;
/// panics on failure, since a missing or broken checked-in fixture is a repo problem.
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

/// The player's `.sav` as JSON. Serialized through the real writer and re-parsed, so
/// anything these helpers see is genuinely what would land on disk.
#[allow(dead_code)]
pub fn player_sav_json(session: &SaveSession, player_id: Uuid) -> serde_json::Value {
    let player_files = session.player_sav_bytes().expect("serialize player savs");
    let (sav_bytes, _dps) = player_files.get(&player_id).expect("player is loaded");
    let save = psp_core::savio::read_sav_bytes(sav_bytes).expect("parse player sav");
    serde_json::to_value(&save).expect("player sav to json")
}

/// uesave serializes a `PropertyKey` as `<name>_<index>`, so the `_` guard is what keeps
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
    // Guards against `find` having walked into `...FlagByType` instead.
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

/// On-disk-ordered `Vec` variant of `relic_flat_flags`, which catches a silent reorder
/// that a `BTreeSet` comparison would not.
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

/// On-disk-ordered `Vec` variant of `relic_by_type_flags`. See `relic_flat_flags_ordered`.
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

/// The first fixture player that actually carries the 1.0 by-type relic structures.
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

/// `(slot count, occupied slot count)` for one `CharacterContainerSaveData` entry. Slot
/// count is the container's capacity, independent of how many pals sit in it.
#[allow(dead_code)]
pub fn container_slot_census(entry: &psp_core::ue::MapEntry) -> (usize, usize) {
    use psp_core::ue::{PalStruct, Property, PropertyKey, StructValue};

    let Some(value_props) = psp_core::props::struct_props(&entry.value) else { return (0, 0) };
    let Some(slots) =
        psp_core::props::get(value_props, &["Slots"]).and_then(psp_core::props::struct_values)
    else {
        return (0, 0);
    };
    let mut total = 0;
    let mut occupied = 0;
    for slot in slots {
        let StructValue::Struct(slot_props) = slot else { continue };
        if let Some(Property::Struct(StructValue::Game(PalStruct::CharacterContainer(raw)))) =
            slot_props.0.get(&PropertyKey::from("RawData"))
        {
            total += 1;
            if !psp_core::props::guid_to_uuid(&raw.instance_id).is_nil() {
                occupied += 1;
            }
        }
    }
    (total, occupied)
}

/// The base camp with the most placed structures. `v1_relics` has 15 base camps and the
/// first is nearly empty, so keying on entry order would make capture assertions vacuous.
/// Ties break on the uuid string for a stable choice across runs.
#[allow(dead_code)]
pub fn fixture_base_id(session: &SaveSession) -> Uuid {
    use psp_core::ue::{PalStruct, Property, PropertyKey, StructValue};

    let mut counts: std::collections::HashMap<Uuid, usize> = std::collections::HashMap::new();
    let map_objects = psp_core::domain::world::map_object_values(&session.level)
        .expect("map object values")
        .expect("the fixture must have MapObjectSaveData");
    for value in map_objects {
        let StructValue::Struct(object_props) = value else { continue };
        let Some(model) = object_props
            .0
            .get(&PropertyKey::from("Model"))
            .and_then(psp_core::props::struct_props)
        else {
            continue;
        };
        let Some(Property::Struct(StructValue::Game(PalStruct::MapModel(raw)))) =
            model.0.get(&PropertyKey::from("RawData"))
        else {
            continue;
        };
        let base_id = psp_core::props::guid_to_uuid(&raw.base_camp_id_belong_to);
        if base_id.is_nil() {
            continue;
        }
        *counts.entry(base_id).or_default() += 1;
    }

    let base_camps = psp_core::domain::world::base_camp_map(&session.level)
        .expect("base camp map")
        .expect("the fixture must have BaseCampSaveData");
    let known: std::collections::HashSet<Uuid> = base_camps
        .iter()
        .filter_map(|entry| psp_core::props::as_uuid(&entry.key))
        .collect();

    let (base_id, count) = counts
        .into_iter()
        .filter(|(id, _)| known.contains(id))
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.to_string().cmp(&b.0.to_string())))
        .expect("the fixture must have a base camp with structures");
    assert!(count > 1, "the chosen fixture base must have real structures, got {count}");
    base_id
}

/// The first fixture player carrying a *non*-CapturePower relic type with at least one
/// true flag -- the only kind that can witness a regression wiping other relic types.
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

/// `base_id`'s `BaseCampSaveData.RawData`, the typed struct backing its
/// group membership, transform and area range.
fn fixture_base_camp_raw(
    session: &SaveSession,
    base_id: Uuid,
) -> psp_core::ue::games::palworld::PalBaseCamp {
    use psp_core::ue::{PalStruct, Property, PropertyKey, StructValue};

    let base_camps = psp_core::domain::world::base_camp_map(&session.level)
        .expect("base camp map")
        .expect("the fixture must have BaseCampSaveData");
    let entry = base_camps
        .iter()
        .find(|entry| psp_core::props::as_uuid(&entry.key) == Some(base_id))
        .expect("fixture base camp entry exists");
    let value_props = psp_core::props::struct_props(&entry.value).expect("base camp value");
    match value_props.0.get(&PropertyKey::from("RawData")) {
        Some(Property::Struct(StructValue::Game(PalStruct::BaseCamp(raw)))) => (**raw).clone(),
        _ => panic!("fixture base camp missing typed RawData"),
    }
}

/// The base camp's `group_id_belong_to` -- the guild that owns it.
#[allow(dead_code)]
pub fn fixture_guild_id(session: &SaveSession) -> Uuid {
    let base_id = fixture_base_id(session);
    psp_core::props::guid_to_uuid(&fixture_base_camp_raw(session, base_id).group_id_belong_to)
}

/// An `Anchor` built from `base_id`'s raw `transform`, so a placement test can
/// target the exact spot an existing base already occupies.
#[allow(dead_code)]
pub fn fixture_base_anchor(session: &SaveSession, base_id: Uuid) -> Anchor {
    let raw = fixture_base_camp_raw(session, base_id);
    let rotation = &raw.transform.rotation;
    Anchor {
        x: raw.transform.translation.x.0,
        y: raw.transform.translation.y.0,
        z: raw.transform.translation.z.0,
        yaw_radians: 2.0 * rotation.z.0.atan2(rotation.w.0),
    }
}

/// The guild that owns each base camp, one entry per base camp, read straight off
/// `BaseCampSaveData` -- independent of the code under test.
#[allow(dead_code)]
pub fn base_camp_guild_ids(session: &SaveSession) -> Vec<Uuid> {
    use psp_core::ue::{PalStruct, Property, PropertyKey, StructValue};

    psp_core::domain::world::base_camp_map(&session.level)
        .expect("base camp map")
        .expect("the fixture must have BaseCampSaveData")
        .iter()
        .filter_map(|entry| {
            let value_props = psp_core::props::struct_props(&entry.value)?;
            match value_props.0.get(&PropertyKey::from("RawData")) {
                Some(Property::Struct(StructValue::Game(PalStruct::BaseCamp(raw)))) => {
                    Some(psp_core::props::guid_to_uuid(&raw.group_id_belong_to))
                }
                _ => None,
            }
        })
        .collect()
}

/// Overwrites a base camp's `area_range`; the fixture's bases are all a uniform 3500 cm,
/// which cannot express a tighter-footprint merge case on its own.
#[allow(dead_code)]
pub fn set_base_area_range(session: &mut SaveSession, base_id: Uuid, area_range: f32) {
    use psp_core::ue::{PalStruct, Property, PropertyKey, StructValue};

    let entries = psp_core::domain::world::base_camp_map_mut(&mut session.level)
        .expect("base camp map")
        .expect("the fixture must have BaseCampSaveData");
    let entry = entries
        .iter_mut()
        .find(|entry| psp_core::props::as_uuid(&entry.key) == Some(base_id))
        .expect("fixture base camp entry exists");
    let value_props = psp_core::props::struct_props_mut(&mut entry.value).expect("base camp value");
    match value_props.0.get_mut(&PropertyKey::from("RawData")) {
        Some(Property::Struct(StructValue::Game(PalStruct::BaseCamp(raw)))) => {
            raw.area_range = area_range;
        }
        _ => panic!("fixture base camp missing typed RawData"),
    }
}

/// A committed real `WorldOption.sav`, parsed through the production reader. Real files
/// carry all 119 settings, so a test reading one back is reading the game's own values.
#[allow(dead_code)]
pub fn load_world_option_fixture(name: &str) -> psp_core::ue::Save {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/world_option")
        .join(format!("{name}.sav"));
    let bytes = std::fs::read(&path).expect("read committed WorldOption fixture");
    let mut save = psp_core::savio::read_sav_bytes(&bytes).expect("parse WorldOption fixture");
    psp_core::domain::world_option::ensure_world_option_schemas(&mut save);
    save
}

/// Gives `session` a real `WorldOption.sav`. The save fixtures ship none, so
/// without this every limit check reads `None` and can never fire.
#[allow(dead_code)]
pub fn attach_world_option(session: &mut SaveSession, name: &str) {
    session.world_option = Some(load_world_option_fixture(name));
    session.world_option_dirty = false;
}

/// Writes `key = value` through the real `domain::world_option` schema/patch surface,
/// attaching a real `WorldOption` fixture first if the save didn't carry one.
#[allow(dead_code)]
pub fn set_world_option_int(session: &mut SaveSession, key: &str, value: i32) {
    use psp_core::domain::world_option;

    if session.world_option.is_none() {
        attach_world_option(session, "save_files");
    }
    let save = session.world_option.as_mut().expect("world_option present");
    world_option::ensure_world_option_schemas(save);

    let patch = world_option::WorldOptionPatch { key: key.to_string(), value: serde_json::json!(value) };
    world_option::apply_patch(save, &[patch]).expect("apply world option patch");
}

/// The admin of the guild that owns `fixture_base_id`'s base -- the natural
/// owner for a placement made into that guild.
#[allow(dead_code)]
pub fn fixture_player_uid(session: &SaveSession) -> Uuid {
    let guild_id = fixture_guild_id(session);
    let entries = psp_core::domain::world::group_map(&session.level).expect("group map");
    let entry = entries
        .iter()
        .find(|entry| psp_core::props::as_uuid(&entry.key) == Some(guild_id))
        .expect("the fixture guild has a GroupSaveDataMap entry");
    let group_data =
        psp_core::domain::guild_tail::entry_group_data(entry).expect("guild group data");
    let guild = psp_core::domain::guild_tail::as_guild(group_data).expect("group is a guild");
    let players = psp_core::domain::guild_tail::guild_player_uids(guild);
    *players.first().expect("the fixture guild must have a member")
}

/// How many base camps the save holds, read straight off `BaseCampSaveData`.
#[allow(dead_code)]
pub fn base_count(session: &SaveSession) -> usize {
    psp_core::domain::world::base_camp_map(&session.level)
        .expect("base camp map")
        .map(Vec::len)
        .unwrap_or(0)
}

/// How many elements `MapObjectSaveData` holds, including any that carry no
/// typed `Model.RawData` and so contribute no instance id.
#[allow(dead_code)]
pub fn map_object_count(session: &SaveSession) -> usize {
    psp_core::domain::world::map_object_values(&session.level)
        .expect("map object values")
        .map(Vec::len)
        .unwrap_or(0)
}

/// Every placed structure's `Model.RawData.instance_id`, in on-disk order.
#[allow(dead_code)]
pub fn all_map_object_instance_ids(session: &SaveSession) -> Vec<Uuid> {
    use psp_core::ue::{PalStruct, Property, PropertyKey, StructValue};

    let Ok(Some(values)) = psp_core::domain::world::map_object_values(&session.level) else {
        return Vec::new();
    };
    values
        .iter()
        .filter_map(|value| {
            let StructValue::Struct(object_props) = value else { return None };
            let model = object_props
                .0
                .get(&PropertyKey::from("Model"))
                .and_then(psp_core::props::struct_props)?;
            match model.0.get(&PropertyKey::from("RawData")) {
                Some(Property::Struct(StructValue::Game(PalStruct::MapModel(raw)))) => {
                    Some(psp_core::props::guid_to_uuid(&raw.instance_id))
                }
                _ => None,
            }
        })
        .collect()
}

/// A `WorldOption` setting's current value, read back through the same
/// `domain::world_option` surface the app uses.
#[allow(dead_code)]
pub fn world_option_int(session: &SaveSession, key: &str) -> Option<i64> {
    psp_core::domain::world_option::read_settings(session.world_option.as_ref()?)
        .into_iter()
        .find(|entry| entry.key == key)?
        .value
        .as_i64()
}
