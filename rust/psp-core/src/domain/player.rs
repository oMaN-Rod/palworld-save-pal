//! Player lazy load and full detail dump -- port of
//! `LoadingMixin.load_player_on_demand` (`game/mixins/loading.py`),
//! `AppState.get_player_details` (`state.py`), and `Player`'s computed dump
//! (`game/player.py`).
//!
//! Deviation from the brief: the brief's reference code imports a
//! `session::FileRef` type with `.sav`/`.dps` fields and a hand-rolled
//! `file_ref_bytes` helper. Neither exists -- Task 2 already built this
//! exact thing as `session::PlayerFileData`, with `sav_bytes()`/`dps_bytes()`
//! methods that already resolve the Paths-vs-Bytes distinction. This module
//! uses that real type instead of reinventing it.

use crate::dto::container::CharacterContainerDto;
use crate::dto::ordered_map::OrderedMap;
use crate::dto::pal::PalDto;
use crate::dto::player::{PlayerDto, WorldMapPointDto};
use crate::dto::summary::{self, IsoDateTime};
use crate::error::CoreError;
use crate::gamedata::GameData;
use crate::progress::ProgressSink;
use crate::props;
use crate::session::{parse_palworld_save, LoadedPlayer, SaveSession, WorldCaches};
use chrono::Timelike;
use uesave::{Properties, Property, PropertyKey, StructValue, ValueVec};

use super::{containers, pal, world};

/// `PalObjects.StatusNameMap` (`pal_objects.py`) -- Japanese `StatusName` ->
/// English key, for `Player.status_point_list`.
pub const STATUS_NAME_MAP: [(&str, &str); 6] = [
    ("最大HP", "max_hp"),
    ("最大SP", "max_sp"),
    ("攻撃力", "attack"),
    ("所持重量", "weight"),
    ("捕獲率", "capture_rate"),
    ("作業速度", "work_speed"),
];
/// `PalObjects.ExStatusNameMap` (`pal_objects.py`) -- same as
/// `STATUS_NAME_MAP` minus capture_rate, for `Player.ext_status_point_list`.
pub const EX_STATUS_NAME_MAP: [(&str, &str); 5] = [
    ("最大HP", "max_hp"),
    ("最大SP", "max_sp"),
    ("攻撃力", "attack"),
    ("所持重量", "weight"),
    ("作業速度", "work_speed"),
];

/// .NET/Palworld ticks -> Python `datetime.isoformat()` string
/// (`game/player.py::Player.last_online_time` + FastAPI's default `datetime`
/// JSON encoding).
///
/// Deviation from the brief: the brief's reference implementation
/// recomputed `ticks as f64 / 10_000_000.0` from scratch -- the exact lossy
/// `u64`-straight-to-`f64` shortcut `dto::summary::ticks_to_datetime`'s own
/// doc comment documents as a real, previously-shipped parity bug (silently
/// corrupts any date past ~year 1000). Rather than reintroduce that bug a
/// second time in a second location, this delegates to the already
/// precision-verified `ticks_to_datetime` (a 500,000-sample fuzz match
/// against real CPython) and only duplicates the small, parity-risk-free
/// final string-formatting step (mirrors `dto::summary::IsoDateTime`'s own
/// `Serialize` impl).
pub fn ticks_to_isoformat(ticks: u64) -> String {
    let datetime = summary::ticks_to_datetime(ticks).unwrap_or_else(|| {
        chrono::NaiveDate::from_ymd_opt(1, 1, 1)
            .expect("year 1 is a valid NaiveDate")
            .and_hms_opt(0, 0, 0)
            .expect("midnight is a valid time")
    });
    let microseconds = datetime.time().nanosecond() / 1_000;
    if microseconds == 0 {
        datetime.format("%Y-%m-%dT%H:%M:%S").to_string()
    } else {
        format!("{}.{microseconds:06}", datetime.format("%Y-%m-%dT%H:%M:%S"))
    }
}

pub(crate) fn save_data_props(player_sav: &uesave::Save) -> Result<&Properties, CoreError> {
    props::struct_props(
        player_sav
            .root
            .properties
            .0
            .get(&PropertyKey::from("SaveData"))
            .ok_or_else(|| CoreError::Parse("player SaveData missing".into()))?,
    )
    .ok_or_else(|| CoreError::Parse("player SaveData not a struct".into()))
}

/// `PalObjects.get_nested(self._save_data, name, "value", "ID")`
/// (`Player.pal_box_id`/`Player.otomo_container_id`).
pub(crate) fn container_id_from(save_data: &Properties, name: &str) -> Option<uuid::Uuid> {
    props::struct_props(save_data.0.get(&PropertyKey::from(name))?)
        .and_then(|inner| inner.0.get(&PropertyKey::from("ID")))
        .and_then(props::as_uuid)
}

/// `Player._get_unlock_flags` (`game/player.py`): map of Name->Bool, keep
/// only the keys whose value is truthy.
fn unlock_flag_keys(record_data: &Properties, flag_name: &str) -> Vec<String> {
    let Some(entries) = record_data
        .0
        .get(&PropertyKey::from(flag_name))
        .and_then(props::map_entries)
    else {
        return Vec::new();
    };
    entries
        .iter()
        .filter(|entry| props::as_bool(&entry.value).unwrap_or(false))
        .filter_map(|entry| props::as_str(&entry.key).map(str::to_string))
        .collect()
}

/// `Player.status_point_list`/`Player.ext_status_point_list`
/// (`game/player.py`): one `english_name -> StatusPoint` entry per list
/// element whose `StatusName` resolves through `name_map`.
///
/// Deliberate, documented divergence for `ext_status_point_list`: Python's
/// own getter for that field has NO `"None"`/unrecognized-name guard at all
/// (unlike `status_point_list`, which explicitly skips `"None"` rows) --
/// `PalObjects.ExStatusNameMap[japanese_name]` would raise `KeyError`
/// outright on an entry Python doesn't expect. Real
/// `GotExStatusPointList`/`GotStatusPointList` data (see
/// `pal_objects.py::PalSaveParameter`) is always constructed with exactly
/// the known Japanese names, so this is not expected to differ from Python
/// on any real save; applying the same defensive skip to both lists here,
/// rather than reproducing Python's crash-on-unrecognized-name for
/// `ext_status_point_list` specifically, follows this port's own
/// established, already-precedented policy of skipping a malformed/
/// unrecognized entry instead of panicking on untrusted save data (see
/// `domain::pal::read_save_parameter_dto`'s `work_suitability` loop for the
/// same reasoning applied to an analogous "Python would crash here" case).
fn status_points(
    save_parameter: &Properties,
    list_name: &str,
    name_map: &[(&str, &str)],
) -> OrderedMap<String, i64> {
    let mut points = OrderedMap::new();
    let Some(values) = save_parameter
        .0
        .get(&PropertyKey::from(list_name))
        .and_then(props::struct_values)
    else {
        return points;
    };
    for value in values {
        let StructValue::Struct(status_props) = value else {
            continue;
        };
        let Some(status_name) = status_props
            .0
            .get(&PropertyKey::from("StatusName"))
            .and_then(props::as_str)
        else {
            continue;
        };
        if status_name == "None" {
            continue;
        }
        let Some((_, english)) = name_map
            .iter()
            .find(|(japanese, _)| *japanese == status_name)
        else {
            continue;
        };
        let point = status_props
            .0
            .get(&PropertyKey::from("StatusPoint"))
            .and_then(props::as_i32)
            .unwrap_or(0) as i64;
        points.insert(english.to_string(), point);
    }
    points
}

/// Port of `AppState.get_player_details` + `LoadingMixin.load_player_on_demand`
/// (`state.py` / `game/mixins/loading.py`). Returns `None` when the player
/// has no file reference or no matching character-map entry -- Python logs a
/// warning and returns `None` in both cases (the handler layer, Task 13,
/// turns that into the `{"error": ...}` WS payload).
pub fn get_player_details(
    session: &mut SaveSession,
    game_data: &GameData,
    player_id: uuid::Uuid,
    progress: &ProgressSink,
) -> Result<Option<PlayerDto>, CoreError> {
    if session.loaded_players.contains_key(&player_id) {
        return build_player_dto(session, game_data, player_id);
    }
    if !session.player_file_refs.contains_key(&player_id) {
        return Ok(None);
    }

    let display_name = session
        .player_summaries
        .get(&player_id)
        .map(|summary| summary.nickname.clone())
        .unwrap_or_else(|| player_id.to_string()[..8].to_string());
    progress(&format!("Loading player {display_name}..."));

    // `cached_sav_gvas = self._player_gvas_sav_cache.pop(player_id, None)`
    // (loading.py): reuse the already-parsed `.sav` Task 5's summary
    // extraction stashed away, skipping a second read+parse of the same
    // file entirely, when present.
    let player_sav = match session.player_sav_cache.remove(&player_id) {
        Some(cached) => cached,
        None => {
            let Some(file_ref) = session.player_file_refs.get(&player_id) else {
                return Ok(None);
            };
            let Some(sav_bytes) = file_ref.sav_bytes()? else {
                return Ok(None);
            };
            parse_palworld_save(&sav_bytes)?
        }
    };
    let player_dps = {
        let Some(file_ref) = session.player_file_refs.get(&player_id) else {
            return Ok(None);
        };
        match file_ref.dps_bytes()? {
            Some(dps_bytes) => Some(parse_palworld_save(&dps_bytes)?),
            None => None,
        }
    };

    // `player_entry = None; for entry in ...: if is_player(entry) and
    // PlayerUId == player_id: player_entry = entry; break` (loading.py).
    let has_entry = world::character_map(&session.level)?.iter().any(|entry| {
        world::entry_is_player(entry) && world::entry_player_uid(entry) == Some(player_id)
    });
    if !has_entry {
        return Ok(None);
    }

    progress("Loading pals...");
    session.loaded_players.insert(
        player_id,
        LoadedPlayer {
            uid: player_id,
            sav: player_sav,
            dps: player_dps,
        },
    );
    if let Some(summary) = session.player_summaries.get_mut(&player_id) {
        summary.loaded = true;
    }

    // Guild becomes discoverable via the guild map cache; the full nested
    // `GuildDto` build is Task 8 scope. Mirrors the summary-loaded side
    // effect of Python's `_load_guild_by_id` without constructing the full
    // `Guild` domain object this task doesn't have.
    if let Some(guild_id) = super::guild::find_player_guild_id(session, player_id)? {
        session.loaded_guilds.insert(guild_id);
        if let Some(summary) = session.guild_summaries.get_mut(&guild_id) {
            summary.loaded = true;
        }
    }

    build_player_dto(session, game_data, player_id)
}

/// Rebuilds the `PlayerDto` for an already-loaded player -- both
/// `get_player_details`'s own return path and, later, any Task 9/11 caller
/// that wants a fresh dump after an edit without re-running the lazy-load
/// machinery. `None` when the player isn't (yet) in `loaded_players`, or (an
/// untrusted-input guard, not expected on any player this port itself
/// loaded) its character-map entry has since vanished.
///
/// `guild_id` is read from `session.caches.player_guild_map`, which this
/// function -- taking `&SaveSession`, not `&mut` -- cannot itself populate;
/// it relies on that cache already being warm (`get_player_details` always
/// warms it via `guild::find_player_guild_id` before calling here).
pub fn build_player_dto(
    session: &SaveSession,
    game_data: &GameData,
    player_id: uuid::Uuid,
) -> Result<Option<PlayerDto>, CoreError> {
    let Some(loaded) = session.loaded_players.get(&player_id) else {
        return Ok(None);
    };

    // --- character-map side (game/player.py computed fields backed by
    // `_character_save`/`_save_parameter`) ---
    let entries = world::character_map(&session.level)?;
    let Some(entry) = entries
        .iter()
        .find(|e| world::entry_is_player(e) && world::entry_player_uid(e) == Some(player_id))
    else {
        return Ok(None);
    };
    let instance_id = world::entry_instance_id(entry);
    let save_parameter = world::entry_save_parameter(entry)
        .ok_or_else(|| CoreError::Parse("player save parameter missing".into()))?;

    let nickname = pal::param(save_parameter, "NickName")
        .and_then(props::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| {
            // `Player.nickname` (game/player.py): literal ninja emoji, NOT
            // the sheep/"Player (...)" fallback `PlayerSummary.nickname`
            // uses -- the two fallbacks are genuinely different Python
            // strings for two different classes.
            format!(
                "\u{1f977} ({})",
                player_id.to_string().split('-').next().unwrap_or("")
            )
        });
    let location =
        pal::param(save_parameter, "LastJumpedLocation").and_then(|property| match property {
            Property::Struct(StructValue::Vector(vector)) => Some(WorldMapPointDto {
                x: vector.x.0,
                y: vector.y.0,
                z: vector.z.0,
            }),
            _ => None,
        });

    // --- player .sav side (`_save_data`) ---
    let save_data = save_data_props(&loaded.sav)?;
    // Legacy key rename (player.py `_load_inventory`): "inventoryInfo" ->
    // "InventoryInfo". Python mutates the tree to migrate this in place;
    // this read-only accessor instead just checks which spelling is
    // present, producing the identical output value without the write side
    // effect (see this task's report).
    let inventory_key = if save_data
        .0
        .contains_key(&PropertyKey::from("InventoryInfo"))
    {
        "InventoryInfo"
    } else {
        "inventoryInfo"
    };
    let inventory_info = save_data
        .0
        .get(&PropertyKey::from(inventory_key))
        .and_then(props::struct_props);

    let mut caches_scratch = WorldCaches::default();
    let mut read_inventory = |id_key: &str, type_name: &str| {
        inventory_info
            .and_then(|info| {
                info.0
                    .get(&PropertyKey::from(id_key))
                    .and_then(props::struct_props)
                    .and_then(|inner| inner.0.get(&PropertyKey::from("ID")))
                    .and_then(props::as_uuid)
            })
            .and_then(|container_id| {
                containers::read_item_container(
                    &session.level,
                    &mut caches_scratch,
                    game_data,
                    container_id,
                    type_name,
                    None,
                )
            })
    };
    let common_container = read_inventory("CommonContainerId", "CommonContainer");
    let essential_container = read_inventory("EssentialContainerId", "EssentialContainer");
    let weapon_load_out_container =
        read_inventory("WeaponLoadOutContainerId", "WeaponLoadOutContainer");
    let player_equipment_armor_container =
        read_inventory("PlayerEquipArmorContainerId", "PlayerEquipArmorContainer");
    let food_equip_container = read_inventory("FoodEquipContainerId", "FoodEquipContainer");

    let pal_box_id = container_id_from(save_data, "PalStorageContainerId");
    let otomo_container_id = container_id_from(save_data, "OtomoCharacterContainerId");

    // RecordData (player.py `__init__` creates it empty when absent -- this
    // read-only accessor produces the same *output* (empty lists / 0)
    // without that write side effect; see this task's report).
    let record_data = save_data
        .0
        .get(&PropertyKey::from("RecordData"))
        .and_then(props::struct_props);
    let unlocked_fast_travel_points = record_data
        .map(|record| unlock_flag_keys(record, "FastTravelPointUnlockFlag"))
        .unwrap_or_default();
    let collected_effigies = record_data
        .map(|record| unlock_flag_keys(record, "RelicObtainForInstanceFlag"))
        .unwrap_or_default();
    let relic_possess_num = record_data
        .and_then(|record| record.0.get(&PropertyKey::from("RelicPossessNum")))
        .and_then(props::as_i32)
        .unwrap_or(0) as i64;

    let completed_missions = save_data
        .0
        .get(&PropertyKey::from("CompletedQuestArray"))
        .and_then(props::name_values)
        .cloned()
        .unwrap_or_default();
    let current_missions = save_data
        .0
        .get(&PropertyKey::from("OrderedQuestArray"))
        .and_then(props::struct_values)
        .map(|quests| {
            quests
                .iter()
                .filter_map(|quest| match quest {
                    StructValue::Struct(quest_props) => quest_props
                        .0
                        .get(&PropertyKey::from("QuestName"))
                        .and_then(props::as_str)
                        .filter(|name| *name != "None")
                        .map(str::to_string),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();

    // Pals owned by this player (loading.py `_load_player_pals_only` /
    // `_pal_belongs_to_player`): scan the whole character map, keep every
    // non-player entry whose OwnerPlayerUId matches.
    let mut player_pals: OrderedMap<uuid::Uuid, PalDto> = OrderedMap::new();
    for pal_entry in entries {
        if world::entry_is_player(pal_entry) {
            continue;
        }
        let Some(pal_dto) = pal::pal_dto_from_entry(pal_entry, game_data) else {
            continue;
        };
        if pal_dto.owner_uid == Some(player_id) {
            player_pals.insert(pal_dto.instance_id, pal_dto);
        }
    }

    // DPS pals (player.py `_load_dps`): only populated when a `_dps.sav`
    // file actually exists for this player -- `None` (JSON `null`) is a
    // real, legitimate wire shape otherwise (see `PlayerDto::dps`'s own doc
    // comment).
    let dps = loaded.dps.as_ref().map(|dps_sav| {
        let mut dps_pals: OrderedMap<i32, PalDto> = OrderedMap::new();
        if let Some(slots) = dps_sav
            .root
            .properties
            .0
            .get(&PropertyKey::from("SaveParameterArray"))
            .and_then(props::struct_values)
        {
            for (index, slot) in slots.iter().enumerate() {
                if let Some(dps_dto) = pal::pal_dto_from_dps_slot(slot, game_data) {
                    if dps_dto.character_id != "None" {
                        dps_pals.insert(index as i32, dps_dto);
                    }
                }
            }
        }
        dps_pals
    });

    // Character containers (pal box / party), via the level's own
    // (unshared, freshly built) container index -- see this function's own
    // doc comment on why it cannot reuse `session.caches`.
    let container_index = world::build_character_container_index(&session.level);
    let build_character_container = |container_id: Option<uuid::Uuid>, type_name: &str| {
        let id = container_id?;
        let entry_index = *container_index.get(&id)?;
        let view = containers::read_character_container(&session.level, entry_index)?;
        Some(CharacterContainerDto {
            id,
            player_uid: player_id,
            r#type: type_name.to_string(),
            size: view.size,
            slots: view.slots,
        })
    };
    let pal_box = build_character_container(pal_box_id, "PalBox");
    let party = build_character_container(otomo_container_id, "Party");

    // `Player.last_online_time` (game/player.py): unlike
    // `PlayerSummary.last_online_time` (`_parse_player_gvas_and_timestamp`),
    // this getter has NO zero-tick guard -- ticks of `0` legitimately
    // produces the year-1 epoch on the wire here, matching Python exactly.
    // A missing/mistyped `Timestamp` (Python indexes unconditionally, which
    // would raise) resolves to `None` instead of panicking, per this port's
    // untrusted-save-data policy.
    let last_online_time = props::get(&loaded.sav.root.properties, &["Timestamp"])
        .and_then(props::as_datetime_ticks)
        .and_then(summary::ticks_to_datetime)
        .map(IsoDateTime);

    let guild_id = session
        .caches
        .player_guild_map
        .as_ref()
        .and_then(|map| map.get(&player_id).copied());

    Ok(Some(PlayerDto {
        pals: player_pals,
        common_container,
        essential_container,
        weapon_load_out_container,
        player_equipment_armor_container,
        food_equip_container,
        pal_box,
        party,
        guild_id,
        uid: player_id,
        instance_id,
        nickname,
        level: pal::param(save_parameter, "Level")
            .and_then(props::as_byte_number)
            .unwrap_or(1) as i64,
        technologies: save_data
            .0
            .get(&PropertyKey::from("UnlockedRecipeTechnologyNames"))
            .and_then(props::name_values)
            .cloned()
            .unwrap_or_default(),
        technology_points: save_data
            .0
            .get(&PropertyKey::from("TechnologyPoint"))
            .and_then(props::as_i32)
            .unwrap_or(0) as i64,
        boss_technology_points: save_data
            .0
            .get(&PropertyKey::from("bossTechnologyPoint")) // lowercase b, matching Python's literal key
            .and_then(props::as_i32)
            .unwrap_or(0) as i64,
        exp: pal::param(save_parameter, "Exp")
            .and_then(props::as_i64)
            .unwrap_or(0),
        hp: pal::param(save_parameter, "Hp")
            .or_else(|| pal::param(save_parameter, "HP"))
            .and_then(props::fixed_point64)
            .unwrap_or(0),
        stomach: pal::param(save_parameter, "FullStomach")
            .and_then(props::as_f32)
            .unwrap_or(150.0) as f64,
        sanity: pal::param(save_parameter, "SanityValue")
            .and_then(props::as_f32)
            .unwrap_or(100.0) as f64,
        status_point_list: status_points(save_parameter, "GotStatusPointList", &STATUS_NAME_MAP),
        ext_status_point_list: status_points(
            save_parameter,
            "GotExStatusPointList",
            &EX_STATUS_NAME_MAP,
        ),
        pal_box_id,
        otomo_container_id,
        completed_missions,
        current_missions,
        unlocked_fast_travel_points: Some(unlocked_fast_travel_points),
        collected_effigies: Some(collected_effigies),
        relic_possess_num,
        location,
        last_online_time,
        dps,
    }))
}

// ============================================================================
// Save-file write-back (Task 10) -- port of `PlayerOpsMixin.update_players`/
// `update_player_technologies` (`player_ops.py`) and `Player.update_from`
// (`player.py`).
// ============================================================================

pub(crate) fn save_data_props_mut(
    player_sav: &mut uesave::Save,
) -> Result<&mut Properties, CoreError> {
    props::struct_props_mut(
        player_sav
            .root
            .properties
            .0
            .get_mut(&PropertyKey::from("SaveData"))
            .ok_or_else(|| CoreError::Parse("player SaveData missing".into()))?,
    )
    .ok_or_else(|| CoreError::Parse("player SaveData not a struct".into()))
}

/// Port of `PlayerOpsMixin.update_player_technologies` (`player_ops.py`):
/// each of the three fields is applied only when `Some` (Python's `if
/// technologies is not None: player.technologies = technologies`, etc.).
/// `Err` when `player_id` was never loaded, mirroring `self._players.get
/// (player_id); if not player: raise ValueError(...)`.
pub fn update_player_technologies(
    session: &mut SaveSession,
    player_id: uuid::Uuid,
    technologies: Option<&[String]>,
    technology_points: Option<i64>,
    boss_technology_points: Option<i64>,
) -> Result<(), CoreError> {
    let loaded = session.loaded_players.get_mut(&player_id).ok_or_else(|| {
        CoreError::Other(format!("Player {player_id} not found in the save file."))
    })?;
    let save_data = save_data_props_mut(&mut loaded.sav)?;
    if let Some(technology_names) = technologies {
        save_data.insert(
            "UnlockedRecipeTechnologyNames",
            props::name_array_property(technology_names.to_vec()),
        );
    }
    if let Some(points) = technology_points {
        save_data.insert(
            "TechnologyPoint",
            props::int_property(points.clamp(i32::MIN as i64, i32::MAX as i64) as i32),
        );
    }
    if let Some(boss_points) = boss_technology_points {
        save_data.insert(
            "bossTechnologyPoint",
            props::int_property(boss_points.clamp(i32::MIN as i64, i32::MAX as i64) as i32),
        );
    }
    Ok(())
}

/// Port of `PlayerOpsMixin.update_players` (`player_ops.py`).
pub fn update_players(
    session: &mut SaveSession,
    game_data: &GameData,
    modified_players: &OrderedMap<uuid::Uuid, PlayerDto>,
    progress: &crate::progress::ProgressSink,
) -> Result<(), CoreError> {
    for (player_id, dto) in modified_players.iter() {
        progress(&format!("Updating player {}", dto.nickname));
        apply_player_dto(session, game_data, *player_id, dto)?;
    }
    Ok(())
}

/// `Player.pal_box_id`/`Player.otomo_container_id`'s sibling lookups for the
/// five item containers (`Player._load_inventory`, `player.py`): all five
/// live nested one level deeper, under `InventoryInfo`/`inventoryInfo`
/// (legacy spelling, same fallback `build_player_dto`'s own `inventory_key`
/// resolution already established), unlike `PalStorageContainerId`/
/// `OtomoCharacterContainerId`, which sit directly on `SaveData`.
fn player_inventory_container_id(save_data: &Properties, id_key: &str) -> Option<uuid::Uuid> {
    let inventory_key = if save_data
        .0
        .contains_key(&PropertyKey::from("InventoryInfo"))
    {
        "InventoryInfo"
    } else {
        "inventoryInfo"
    };
    let inventory_info = save_data
        .0
        .get(&PropertyKey::from(inventory_key))
        .and_then(props::struct_props)?;
    container_id_from(inventory_info, id_key)
}

/// Port of `Player.update_from` (`player.py`). Applies every writable field
/// from `dto` onto the player's character-map `SaveParameter` bag and their
/// own `.sav`'s `SaveData`/`RecordData`, following Python's exact
/// remove-on-default / skip-on-`None` semantics per field (each block below
/// cites the Python setter it ports).
///
/// **Container routing is a deliberate, Critical-class fix over the brief.**
/// The brief's reference `apply_item_container_dto` looked up which
/// `ItemContainerSaveData` entry to mutate via the DTO's OWN `id` field --
/// i.e., a value the CLIENT supplies. Real Python never does this:
/// `self.common_container.update_from(value)` mutates `self.common_container`,
/// an object the SERVER already bound to a specific container id at player-
/// load time (`Player._load_inventory`); `ItemContainer.update_from`'s body
/// only ever inspects `other_container["slots"]`, never `other_container
/// ["id"]`, for routing. Routing an edit through a client-supplied id would
/// let a forged `common_container.id` in an `update_players` payload
/// redirect the mutation onto an ARBITRARY container elsewhere in the save
/// (another player's common container, a base's storage container, ...) --
/// the exact class of cross-entity hole Task 9's review already flagged
/// Critical for `delete_player_pals`. Fixed here by resolving each of the
/// five container ids from the PLAYER'S OWN `InventoryInfo` (the same
/// server-trusted source `build_player_dto` already reads them from) and
/// passing that resolved id into `apply_item_container_dto` explicitly,
/// which never reads `dto.id` for routing at all -- see this task's report.
fn apply_player_dto(
    session: &mut SaveSession,
    game_data: &GameData,
    player_id: uuid::Uuid,
    dto: &PlayerDto,
) -> Result<(), CoreError> {
    if !session.loaded_players.contains_key(&player_id) {
        return Err(CoreError::Other(format!(
            "Player {player_id} not found in the save file."
        )));
    }
    // --- character-map save parameter fields ---
    {
        let entries = world::character_map_mut(&mut session.level)?;
        let Some(entry) = entries
            .iter_mut()
            .find(|e| world::entry_is_player(e) && world::entry_player_uid(e) == Some(player_id))
        else {
            return Err(CoreError::Other(format!(
                "Player {player_id} not found in the save file."
            )));
        };
        let Some(save_parameter) = world::entry_save_parameter_mut(entry) else {
            return Err(CoreError::Parse("player save parameter missing".into()));
        };
        save_parameter.insert("Level", props::byte_property(dto.level.clamp(0, 255) as u8));
        save_parameter.insert("Exp", props::int64_property(dto.exp));
        save_parameter.insert("Hp", props::fixed_point64_property(dto.hp));
        // Legacy spelling cleanup: `hp.setter` (player.py) always writes
        // "Hp", never touching a stale "HP" key -- proactively removed here
        // for the same strictly-safer reason `apply_pal_dto` already
        // established for pals (see that function's own doc comment).
        save_parameter.0.shift_remove(&PropertyKey::from("HP"));
        save_parameter.insert("FullStomach", props::float_property(dto.stomach as f32));
        save_parameter.insert("SanityValue", props::float_property(dto.sanity as f32));
        // nickname (Player.nickname setter): default pattern removes the
        // property entirely.
        let default_pattern = format!(
            "\u{1f977} ({})",
            player_id.to_string().split('-').next().unwrap_or("")
        );
        if dto.nickname == default_pattern {
            save_parameter
                .0
                .shift_remove(&PropertyKey::from("NickName"));
        } else {
            save_parameter.insert("NickName", props::str_property(&dto.nickname));
        }
        apply_status_points(
            save_parameter,
            "GotStatusPointList",
            &dto.status_point_list,
            &STATUS_NAME_MAP,
            true,
        );
        apply_status_points(
            save_parameter,
            "GotExStatusPointList",
            &dto.ext_status_point_list,
            &EX_STATUS_NAME_MAP,
            false,
        );
    }
    // --- player .sav SaveData fields ---
    {
        let loaded = session.loaded_players.get_mut(&player_id).expect("checked");
        let save_data = save_data_props_mut(&mut loaded.sav)?;
        save_data.insert(
            "UnlockedRecipeTechnologyNames",
            props::name_array_property(dto.technologies.clone()),
        );
        save_data.insert(
            "TechnologyPoint",
            props::int_property(
                dto.technology_points
                    .clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            ),
        );
        save_data.insert(
            "bossTechnologyPoint",
            props::int_property(
                dto.boss_technology_points
                    .clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            ),
        );
        save_data.insert(
            "CompletedQuestArray",
            props::name_array_property(dto.completed_missions.clone()),
        );
        // OrderedQuestArray rebuild (pal_objects.py's `PalObjects.
        // OrderedQuestArray`/`OrderedQuest`): one {QuestName, BlockIndex: 0,
        // IntegerMap: {}, StringMap: {}} struct per current mission.
        let mut quest_structs = Vec::new();
        for quest_name in &dto.current_missions {
            let mut quest_props = Properties::default();
            quest_props.insert("QuestName", props::name_property(quest_name));
            quest_props.insert("BlockIndex", props::int_property(0));
            quest_props.insert("IntegerMap", Property::Map(vec![]));
            quest_props.insert("StringMap", Property::Map(vec![]));
            quest_structs.push(StructValue::Struct(quest_props));
        }
        save_data.insert(
            "OrderedQuestArray",
            Property::Array(ValueVec::Struct(quest_structs)),
        );
        // unlock flags: only when the caller actually supplied a value
        // (player.py's `case ... : if value is not None: setattr(...)`).
        if let Some(points) = &dto.unlocked_fast_travel_points {
            apply_unlock_flags(save_data, "FastTravelPointUnlockFlag", points);
        }
        if let Some(effigies) = &dto.collected_effigies {
            apply_unlock_flags(save_data, "RelicObtainForInstanceFlag", effigies);
        }
    }
    // --- containers: resolve every real id from the player's OWN save data
    // first (see this function's own doc comment on why `dto.<container>.id`
    // is never trusted for routing), THEN apply -- common before essential
    // matches `Player.update_from`'s real field order, which matters because
    // essential's `AdditionalInventory_` resize targets the ALREADY-applied
    // common container.
    let (common_id, essential_id, weapon_id, armor_id, food_id) = {
        let loaded = session.loaded_players.get(&player_id).expect("checked");
        let save_data = save_data_props(&loaded.sav)?;
        (
            player_inventory_container_id(save_data, "CommonContainerId"),
            player_inventory_container_id(save_data, "EssentialContainerId"),
            player_inventory_container_id(save_data, "WeaponLoadOutContainerId"),
            player_inventory_container_id(save_data, "PlayerEquipArmorContainerId"),
            player_inventory_container_id(save_data, "FoodEquipContainerId"),
        )
    };
    if let (Some(container_id), Some(container)) = (common_id, &dto.common_container) {
        containers::apply_item_container_dto(session, game_data, container_id, container, None)?;
    }
    if let (Some(container_id), Some(container)) = (essential_id, &dto.essential_container) {
        containers::apply_item_container_dto(
            session,
            game_data,
            container_id,
            container,
            common_id,
        )?;
    }
    if let (Some(container_id), Some(container)) = (weapon_id, &dto.weapon_load_out_container) {
        containers::apply_item_container_dto(session, game_data, container_id, container, None)?;
    }
    if let (Some(container_id), Some(container)) = (armor_id, &dto.player_equipment_armor_container)
    {
        containers::apply_item_container_dto(session, game_data, container_id, container, None)?;
    }
    if let (Some(container_id), Some(container)) = (food_id, &dto.food_equip_container) {
        containers::apply_item_container_dto(session, game_data, container_id, container, None)?;
    }
    Ok(())
}

/// `Player.status_point_list`/`ext_status_point_list` setters (`player.py`).
/// `status_point_list`'s setter first drops every "None"/unrecognized-name
/// row (`drop_none_rows: true`); `ext_status_point_list`'s setter has NO such
/// step (`drop_none_rows: false`) -- a real, intentional Python asymmetry
/// between the two setters, not an oversight (see this task's report).
///
/// A `points` key that doesn't match any of `name_map`'s six/five English
/// names is skipped (real Python's `reverse_status_map[status_name]` would
/// raise `KeyError` -- a malformed/adversarial DTO input this port declines
/// to crash on, per its established "skip untrusted input" policy).
fn apply_status_points(
    save_parameter: &mut Properties,
    list_name: &str,
    points: &OrderedMap<String, i64>,
    name_map: &[(&str, &str)],
    drop_none_rows: bool,
) {
    let Some(values) = save_parameter
        .0
        .get_mut(&PropertyKey::from(list_name))
        .and_then(props::struct_values_mut)
    else {
        return;
    };
    if drop_none_rows {
        values.retain(|value| {
            let StructValue::Struct(status_props) = value else {
                return true;
            };
            status_props
                .0
                .get(&PropertyKey::from("StatusName"))
                .and_then(props::as_str)
                .map(|name| name != "None")
                .unwrap_or(true)
        });
    }
    for (english_name, point_value) in points.iter() {
        let Some((japanese_name, _)) = name_map
            .iter()
            .find(|(_, english)| *english == english_name.as_str())
        else {
            continue;
        };
        for value in values.iter_mut() {
            let StructValue::Struct(status_props) = value else {
                continue;
            };
            if status_props
                .0
                .get(&PropertyKey::from("StatusName"))
                .and_then(props::as_str)
                == Some(*japanese_name)
            {
                status_props.insert(
                    "StatusPoint",
                    props::int_property(
                        (*point_value).clamp(i32::MIN as i64, i32::MAX as i64) as i32
                    ),
                );
                break;
            }
        }
    }
}

/// `Player._set_unlock_flags` (`player.py`): full replacement of the flag
/// map's entries (every key set to `true`) plus an unconditional
/// `RelicPossessNum += len(value)` -- yes, this grows on every save,
/// matching Python's own real, if odd, cumulative behavior exactly (not a
/// bug on the known list; see `player.py`'s own `relic_possess_num =
/// relic_possess_num + len(value)`).
///
/// Deviation from Python: when `RecordData` doesn't yet carry `flag_name` at
/// all (Python creates a fresh `PalObjects.MapProperty("NameProperty",
/// "BoolProperty")` in that case), this port skips writing the entries
/// rather than fabricating a brand-new `Map` property from scratch --
/// `uesave`'s writer needs a recorded key/value-type schema for a `Map`
/// property before it can serialize one (see `props::ensure_schema`'s own
/// doc comment), and `props.rs` has no `Map`-shaped constructor/schema
/// helper yet (Phase 2 has never needed one). Every real save this port has
/// tested against already carries both `FastTravelPointUnlockFlag` and
/// `RelicObtainForInstanceFlag` (a player who has never fast-traveled
/// anywhere is not realistic test data), so this narrow gap has no observed
/// real-save impact; flagged here rather than silently assumed away -- see
/// this task's report. `RelicPossessNum` still increments unconditionally,
/// matching Python's own unconditional increment regardless of whether the
/// flag map itself could be written.
fn apply_unlock_flags(save_data: &mut Properties, flag_name: &str, keys: &[String]) {
    if let Some(record_data) = save_data
        .0
        .get_mut(&PropertyKey::from("RecordData"))
        .and_then(props::struct_props_mut)
    {
        if record_data.0.contains_key(&PropertyKey::from(flag_name)) {
            let entries: Vec<uesave::MapEntry> = keys
                .iter()
                .map(|key| uesave::MapEntry {
                    key: props::name_property(key),
                    value: props::bool_property(true),
                })
                .collect();
            record_data.insert(flag_name, Property::Map(entries));
        }
        let current = record_data
            .0
            .get(&PropertyKey::from("RelicPossessNum"))
            .and_then(props::as_i32)
            .unwrap_or(0);
        record_data.insert(
            "RelicPossessNum",
            props::int_property(current.saturating_add(keys.len() as i32)),
        );
    }
}
