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
    let wrote_boss_points = boss_technology_points.is_some();
    if let Some(boss_points) = boss_technology_points {
        save_data.insert(
            "bossTechnologyPoint",
            props::int_property(boss_points.clamp(i32::MIN as i64, i32::MAX as i64) as i32),
        );
    }
    // Same schema gap Task 12 closed for the `update_players`/`apply_player_dto`
    // path: `bossTechnologyPoint` (an IntProperty) is absent from older player
    // saves' `PropertySchemas`, so inserting it here without registering a
    // schema makes a later `player_sav_bytes()`/`download`/`save_modded` resave
    // fail `missing property schema for path: SaveData.bossTechnologyPoint`.
    // `set_technology_data` reaches this fn WITHOUT going through
    // `apply_player_dto`, so it must register the schema itself. Idempotent
    // no-op when the schema already exists; guarded on actually having written
    // the property. Surfaced by the Task-15 parity sequence's
    // set_technology_data -> download_save_file.
    if wrote_boss_points {
        ensure_boss_technology_point_schema(&mut loaded.sav);
    }
    Ok(())
}

/// Port of `PlayerOpsMixin.update_players` (`player_ops.py`).
///
/// `_game_data` is currently unused by this call chain (`apply_player_dto`'s
/// own internals need no `GameData` -- see
/// `containers::apply_item_container_dto`'s doc comment on why that's true
/// all the way down). Kept, not removed: this port's whole `update_*` family
/// (`pal::update_pals`/`update_dps_pals`, `guild::update_guilds`, this
/// function) shares one uniform `(session, game_data, modified, progress)`
/// public signature -- `update_pals` genuinely needs it. See this task's
/// report.
pub fn update_players(
    session: &mut SaveSession,
    _game_data: &GameData,
    modified_players: &OrderedMap<uuid::Uuid, PlayerDto>,
    progress: &crate::progress::ProgressSink,
) -> Result<(), CoreError> {
    for (player_id, dto) in modified_players.iter() {
        progress(&format!("Updating player {}", dto.nickname));
        apply_player_dto(session, *player_id, dto)?;
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
///
/// No `game_data: &GameData` parameter -- see
/// `containers::apply_item_container_dto`'s doc comment.
fn apply_player_dto(
    session: &mut SaveSession,
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
        {
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
        }
        // `bossTechnologyPoint` is written unconditionally above, but saves
        // generated before that field existed carry no schema for it, and
        // uesave's writer refuses to serialize a schema-less property
        // (`MissingPropertySchema`). Register one now -- see this function's
        // own note above and `ensure_boss_technology_point_schema`. Must run
        // after the `save_data` borrow of `loaded.sav` ends (this call needs
        // `&mut loaded.sav` for its `.schemas` table).
        ensure_boss_technology_point_schema(&mut loaded.sav);
        // unlock flags: only when the caller actually supplied a value
        // (player.py's `case ... : if value is not None: setattr(...)`).
        // Needs `&mut loaded.sav` (not just its `SaveData` `Properties`) so it
        // can register a schema for a brand-new flag Map -- see
        // `apply_unlock_flags`'s own doc comment; the `save_data` borrow
        // above must therefore already have ended.
        if let Some(points) = &dto.unlocked_fast_travel_points {
            apply_unlock_flags(&mut loaded.sav, "FastTravelPointUnlockFlag", points);
        }
        if let Some(effigies) = &dto.collected_effigies {
            apply_unlock_flags(&mut loaded.sav, "RelicObtainForInstanceFlag", effigies);
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
        containers::apply_item_container_dto(session, container_id, container, None)?;
    }
    if let (Some(container_id), Some(container)) = (essential_id, &dto.essential_container) {
        containers::apply_item_container_dto(session, container_id, container, common_id)?;
    }
    if let (Some(container_id), Some(container)) = (weapon_id, &dto.weapon_load_out_container) {
        containers::apply_item_container_dto(session, container_id, container, None)?;
    }
    if let (Some(container_id), Some(container)) = (armor_id, &dto.player_equipment_armor_container)
    {
        containers::apply_item_container_dto(session, container_id, container, None)?;
    }
    if let (Some(container_id), Some(container)) = (food_id, &dto.food_equip_container) {
        containers::apply_item_container_dto(session, container_id, container, None)?;
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
///
/// **A genuine, newly-found Python bug, reproduced deliberately for save-file
/// byte parity, not on the known list:** `status_point_list`'s setter
/// (`player.py`) drops "None"/unrecognized rows with `for item in
/// status_point_list: ... status_point_list.remove(item)` -- a classic
/// mutate-while-iterating bug. `list.remove(item)` shifts every later
/// element left by one, but the `for` loop's own internal position counter
/// still advances by one on the NEXT step regardless, so the element that
/// just shifted into the just-vacated slot is silently skipped. With two or
/// more *consecutive* matching rows, only every OTHER one actually gets
/// removed. Verified against real `.venv` CPython (see this task's report
/// for the exact script and output) with this exact shape: four rows
/// `[real, "None", "None", real]` reduce to THREE rows
/// `[real, "None", real]`, not two -- one "None" row survives. The block
/// below reproduces that exact index-advances-regardless-of-removal
/// semantics (not `Vec::retain`, which would correctly remove ALL matching
/// rows in one pass and would disagree, byte-for-byte, with what real
/// Python actually writes for this input).
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
        // Python: `for item in status_point_list: if <predicate>:
        // status_point_list.remove(item)`. `index` here plays the role of
        // Python's own internal for-loop position counter: it advances by
        // exactly one on every step, REGARDLESS of whether a removal just
        // happened -- reproducing the skip, not avoiding it.
        let mut index = 0;
        while index < values.len() {
            let should_remove = match &values[index] {
                StructValue::Struct(status_props) => status_props
                    .0
                    .get(&PropertyKey::from("StatusName"))
                    .and_then(props::as_str)
                    .map(|name| name == "None")
                    .unwrap_or(true), // "StatusName" not in item
                _ => false,
            };
            if should_remove {
                values.remove(index);
            }
            index += 1;
        }
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
/// **Fixed to match Python, not narrowed:** when `RecordData` doesn't yet
/// carry `flag_name` at all, Python creates a fresh
/// `PalObjects.MapProperty("NameProperty", "BoolProperty")` rather than
/// no-op'ing -- and this IS a real, reachable shape, not a hypothetical:
/// `tests/fixtures/saves/world1`'s own real player `8C2F1930` has NO
/// `RelicObtainForInstanceFlag` key under `RecordData` at all (verified
/// empirically -- see this task's report), i.e. a legitimately key-less
/// save, not merely a theoretical edge case. `props::ensure_schema`
/// registers the `Map<NameProperty, BoolProperty>` schema `uesave`'s writer
/// needs before it can serialize a brand-new `Map` property (see that
/// function's own doc comment) at the SAME dotted path pattern this same
/// player's own sibling `RecordData` fields already carry
/// (`SaveData.RecordData.<name>`, confirmed against the real, already-parsed
/// schema table for this exact fixture player -- see this task's report),
/// derived via `schema_prefix_ending_with` off `RecordData` itself (which,
/// unlike the specific flag, is always present here -- this function already
/// returns early otherwise). The unconditional `insert` below then creates
/// (or overwrites) the property either way, matching Python's own
/// unconditional `self._record_data[flag_name]["value"] = [...]` regardless
/// of whether the property was just freshly created. `RelicPossessNum` still
/// increments unconditionally, matching Python's own unconditional
/// increment -- and gets the exact same "register a schema for a brand-new
/// property before writing it" treatment when IT is the one that's missing
/// (this fixture's own real player `8C2F1930` has a
/// `RelicObtainForInstanceFlag`-less RecordData that ALSO has no
/// `RelicPossessNum` yet -- caught empirically with a temporary
/// `uesave::Save::write` round trip during this task's own verification,
/// which failed with `MissingPropertySchema("...RelicPossessNum")` before
/// this second `ensure_schema` call was added; see this task's report).
fn apply_unlock_flags(player_sav: &mut uesave::Save, flag_name: &str, keys: &[String]) {
    let record_data_key = PropertyKey::from("RecordData");
    let flag_key = PropertyKey::from(flag_name);
    let relic_key = PropertyKey::from("RelicPossessNum");

    let has_record_data = save_data_props(player_sav)
        .ok()
        .and_then(|save_data| save_data.0.get(&record_data_key))
        .and_then(props::struct_props)
        .is_some();
    if !has_record_data {
        return;
    }

    let record_data_contains = |key: &PropertyKey| {
        save_data_props(player_sav)
            .ok()
            .and_then(|save_data| save_data.0.get(&record_data_key))
            .and_then(props::struct_props)
            .map(|record_data| record_data.0.contains_key(key))
            .unwrap_or(false)
    };
    let flag_already_present = record_data_contains(&flag_key);
    let relic_already_present = record_data_contains(&relic_key);

    if !flag_already_present || !relic_already_present {
        if let Some(prefix) = props::schema_prefix_ending_with(player_sav, "RecordData") {
            if !flag_already_present {
                props::ensure_schema(
                    player_sav,
                    format!("{prefix}RecordData.{flag_name}"),
                    uesave::PropertyTagPartial {
                        id: None,
                        data: uesave::PropertyTagDataPartial::Map {
                            key_type: Box::new(uesave::PropertyTagDataPartial::Other(
                                uesave::PropertyType::NameProperty,
                            )),
                            value_type: Box::new(uesave::PropertyTagDataPartial::Other(
                                uesave::PropertyType::BoolProperty,
                            )),
                        },
                    },
                );
            }
            if !relic_already_present {
                props::ensure_schema(
                    player_sav,
                    format!("{prefix}RecordData.RelicPossessNum"),
                    uesave::PropertyTagPartial {
                        id: None,
                        data: uesave::PropertyTagDataPartial::Other(
                            uesave::PropertyType::IntProperty,
                        ),
                    },
                );
            }
        }
    }

    let Ok(save_data) = save_data_props_mut(player_sav) else {
        return;
    };
    let Some(record_data) = save_data
        .0
        .get_mut(&record_data_key)
        .and_then(props::struct_props_mut)
    else {
        return;
    };
    let entries: Vec<uesave::MapEntry> = keys
        .iter()
        .map(|key| uesave::MapEntry {
            key: props::name_property(key),
            value: props::bool_property(true),
        })
        .collect();
    record_data.insert(flag_name, Property::Map(entries));
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

/// Registers the `SaveData.bossTechnologyPoint` schema when the player's
/// `.sav` doesn't already carry one, so `apply_player_dto`'s unconditional
/// `bossTechnologyPoint` write survives a real `uesave::Save::write`.
///
/// **Why this exists (Task 10 → Task 12 carry-forward).** `apply_player_dto`
/// always writes `SaveData.bossTechnologyPoint` (an `IntProperty`, mirroring
/// Python `Player.technologies`'s setter, which sets both `TechnologyPoint`
/// and `bossTechnologyPoint`). Every value edit in this port stays in memory
/// until Task 12's save-out compresses the tree back through
/// `uesave::Save::write`, and uesave's writer looks a property's schema up by
/// its exact dotted scope path and returns `MissingPropertySchema` when none
/// was recorded (see `props::ensure_schema`'s own doc comment). A save
/// generated before `bossTechnologyPoint` existed -- e.g. the committed
/// fixture `tests/fixtures/saves/world1`'s real player `8C2F1930`, which
/// carries `SaveData.TechnologyPoint` but NO `SaveData.bossTechnologyPoint`
/// schema (verified empirically) -- would therefore fail the resave with
/// `missing property schema for path: SaveData.bossTechnologyPoint`. This was
/// invisible before Task 12 because Task 10 was the first code to insert the
/// property but nothing yet round-tripped an edited player `.sav` through the
/// writer.
///
/// The schema is copied from the always-present sibling `TechnologyPoint`
/// (same `IntProperty` shape, same `SaveData` scope): `schema_prefix_ending_
/// with(".TechnologyPoint")` yields the `SaveData` prefix (`bossTechnology
/// Point` does NOT end with `.TechnologyPoint` -- the char before is `s`, not
/// `.` -- so the match is unambiguous), and `ensure_schema` records
/// `SaveData.bossTechnologyPoint` only when it's genuinely absent (a no-op on
/// a newer save that already has it). Silent no-op when a malformed `.sav`
/// has no `TechnologyPoint` schema at all -- the writer would then surface
/// the same clear error, never a panic.
fn ensure_boss_technology_point_schema(player_sav: &mut uesave::Save) {
    if let Some(prefix) = props::schema_prefix_ending_with(player_sav, ".TechnologyPoint") {
        props::ensure_schema(
            player_sav,
            format!("{prefix}.bossTechnologyPoint"),
            uesave::PropertyTagPartial {
                id: None,
                data: uesave::PropertyTagDataPartial::Other(uesave::PropertyType::IntProperty),
            },
        );
    }
}

// ============================================================================
// delete_player (Task 11) -- port of `PlayerOpsMixin.delete_player`/
// `_delete_player_and_pals` (`player_ops.py`) and `Guild.delete_player`
// (`guild.py:159-170`).
// ============================================================================

/// Port of `delete_player` (`player_ops.py`). `Ok(false)` when the player is
/// their guild's admin (nothing deleted, matching Python's early `return
/// False`); `Err` when `player_id` was never loaded (matching Python's
/// `raise ValueError` before any mutation, `player_ops.py:29-31`).
///
/// **Guild lookup is scoped to already-LOADED guilds, not the brief's
/// unscoped `find_player_guild_id`.** Python's `player_guild =
/// self._player_guild(player_id)` (`save_manager.py`) iterates
/// `self._guilds.values()` ONLY -- guilds already lazily loaded this
/// session -- not every guild the raw save records (that broader scan is a
/// DIFFERENT method, `_find_player_guild_id`, used only by the loading
/// path to auto-load a player's guild on demand). A player whose guild was
/// never loaded this session is therefore treated as guildless by
/// `delete_player` itself: no admin check, no guild-handle removal, even if
/// the raw save data says the player belongs to a guild. Reproduced here by
/// filtering `find_player_guild_id`'s (unscoped) answer through
/// `session.loaded_guilds`. The brief's own reference code used
/// `find_player_guild_id` unfiltered, which would run the admin check
/// against ANY guild the raw save happens to record for this player,
/// loaded or not this session -- a real, observable behavioral difference
/// from Python (an admin deletion the brief's version would refuse, real
/// Python would actually allow, if that guild was never separately loaded
/// first). See this task's report.
pub fn delete_player(
    session: &mut SaveSession,
    game_data: &GameData,
    player_id: uuid::Uuid,
    progress: &ProgressSink,
) -> Result<bool, CoreError> {
    if !session.loaded_players.contains_key(&player_id) {
        return Err(CoreError::Other(format!(
            "Player {player_id} not found in the save file."
        )));
    }
    let details = build_player_dto(session, game_data, player_id)?.ok_or_else(|| {
        CoreError::Other(format!("Player {player_id} not found in the save file."))
    })?;
    let nickname = details.nickname.clone();

    let player_guild_id = super::guild::find_player_guild_id(session, player_id)?
        .filter(|guild_id| session.loaded_guilds.contains(guild_id));

    if let Some(guild_id) = player_guild_id {
        let entry_index = super::guild::guild_entry_index(session, guild_id)?
            .ok_or(CoreError::GuildNotFound(guild_id))?;
        let (guild_name, admin_uid) = {
            let entries = world::group_map(&session.level)?;
            let group_data = super::guild_tail::entry_group_data(&entries[entry_index])
                .ok_or_else(|| CoreError::Parse("guild group data untyped".into()))?;
            let tail = super::guild_tail::GuildTail::parse(&group_data.remaining_data)?;
            (
                tail.guild_name.clone(),
                tail.players.first().map(|player| player.player_uid),
            )
        };
        if admin_uid == Some(player_id) {
            return Ok(false); // player_ops.py:34-40
        }
        progress(&format!(
            "Deleting player {nickname} from guild {guild_name}"
        ));
        // `Guild.delete_player` (guild.py:159-170): drop the player's own
        // character handle and their `players` row.
        let entries = world::group_map_mut(&mut session.level)?;
        if let Some(group_data) = super::guild_tail::entry_group_data_mut(&mut entries[entry_index])
        {
            group_data.individual_character_handle_ids.retain(|handle| {
                props::guid_to_uuid(&handle.instance_id) != player_id
                    && props::guid_to_uuid(&handle.guid) != player_id
            });
            if let Ok(mut tail) = super::guild_tail::GuildTail::parse(&group_data.remaining_data) {
                tail.players.retain(|player| player.player_uid != player_id);
                group_data.remaining_data = tail.to_bytes();
            }
        }
    }

    let (item_container_ids, character_container_ids) =
        delete_player_and_pals_for_guild(session, game_data, player_id, &details, progress)?;

    progress(&format!("Deleting map objects of player {nickname}"));
    if let Some(values) = world::map_object_values_mut(&mut session.level)? {
        values.retain(|map_object| {
            !super::guild::should_delete_map_object(map_object, None, &[player_id])
        });
    }

    progress(&format!("Deleting item containers of player {nickname}"));
    super::containers::delete_item_containers(session, &item_container_ids)?;

    progress(&format!(
        "Deleting character containers of player {nickname}"
    ));
    super::containers::delete_character_containers(session, &character_container_ids)?;
    Ok(true)
}

/// Port of `_delete_player_and_pals` (`player_ops.py`). Deletes every pal
/// this player's OWN pal box + party containers reference, directly by
/// instance id, then the player's own character-map entry, file ref, and
/// `LoadedPlayer`. Returns the five item-container ids and two
/// character-container ids (pal box + party) the caller still needs to
/// delete afterward -- matching Python's own return value.
///
/// Named `..._for_guild` (not just `delete_player_and_pals`) because both
/// `delete_player` above and `guild::delete_guild_and_players` call this
/// same function -- exactly like Python's single `_delete_player_and_pals`
/// is shared by `delete_player` and `delete_guild_and_players`
/// (`guild_ops.py:57-65`).
///
/// **Deliberately does NOT remove any per-pal guild character handle -- a
/// newly-found Python gap, not on the known list, reproduced for byte
/// parity (required: a dangling `individual_character_handle_ids` entry is
/// a real, observable difference in the written save's guild tail).**
/// `_delete_player_and_pals` calls `self._delete_pal_by_id(pal_id)` directly
/// for every box/party pal (`pal_ops.py`'s `_delete_pal_by_id`, which only
/// pops `self._pals`/`CharacterSaveParameterMap`) -- NEVER `Player.
/// delete_pal` (which DOES call `self._guild.delete_character_handle
/// (pal_id)`, `player.py:614-619`, and IS what `PalOpsMixin.
/// delete_player_pals` -- Task 9's own single-pal-delete op -- uses
/// instead). So deleting a player through `delete_player`/
/// `delete_guild_and_players` leaves that player's own pals'
/// `individual_character_handle_ids` entries dangling in the guild's raw
/// tail bytes. Contrast: base pals deleted via `delete_guild_and_players`
/// DO get their handle cleaned up, because that path calls `PalOpsMixin.
/// delete_guild_pals` -> `Guild.delete_base_pal`, which DOES call
/// `delete_character_handle` (`guild.py:143-146`) -- this asymmetry
/// (player pals: dangling handle; base pals: cleaned up) is a real Python
/// behavior, not a port artifact. See this task's report.
pub(crate) fn delete_player_and_pals_for_guild(
    session: &mut SaveSession,
    _game_data: &GameData,
    player_id: uuid::Uuid,
    details: &PlayerDto,
    progress: &crate::progress::ProgressSink,
) -> Result<(Vec<uuid::Uuid>, Vec<uuid::Uuid>), CoreError> {
    let nickname = &details.nickname;
    progress(&format!(
        "Deleting player {nickname} with {} pals",
        details.pals.len()
    ));

    let item_container_ids: Vec<uuid::Uuid> = [
        &details.common_container,
        &details.essential_container,
        &details.weapon_load_out_container,
        &details.player_equipment_armor_container,
        &details.food_equip_container,
    ]
    .into_iter()
    .flatten()
    .map(|container| container.id)
    .collect();
    let character_container_ids: Vec<uuid::Uuid> = [details.otomo_container_id, details.pal_box_id]
        .into_iter()
        .flatten()
        .collect();

    let box_pal_ids: Vec<uuid::Uuid> = details
        .pal_box
        .as_ref()
        .map(|container| {
            container
                .slots
                .iter()
                .filter_map(|slot| slot.pal_id)
                .collect()
        })
        .unwrap_or_default();
    progress(&format!(
        "Deleting {} pals of player {nickname} from PalBox",
        box_pal_ids.len()
    ));
    for pal_id in box_pal_ids {
        super::pal::delete_pal_entry(session, pal_id);
    }

    let party_pal_ids: Vec<uuid::Uuid> = details
        .party
        .as_ref()
        .map(|container| {
            container
                .slots
                .iter()
                .filter_map(|slot| slot.pal_id)
                .collect()
        })
        .unwrap_or_default();
    progress(&format!(
        "Deleting {} pals of player {nickname} from Party",
        party_pal_ids.len()
    ));
    for pal_id in party_pal_ids {
        super::pal::delete_pal_entry(session, pal_id);
    }

    session.loaded_players.remove(&player_id);
    world::character_map_mut(&mut session.level)?.retain(|entry| {
        !(world::entry_is_player(entry) && world::entry_player_uid(entry) == Some(player_id))
    });
    session.invalidate_performance_caches();
    session.player_file_refs.remove(&player_id);
    Ok((item_container_ids, character_container_ids))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status_point_struct(name: &str, point: i32) -> StructValue {
        let mut status_props = Properties::default();
        status_props.insert("StatusName", props::name_property(name));
        status_props.insert("StatusPoint", props::int_property(point));
        StructValue::Struct(status_props)
    }

    fn names_of(save_parameter: &Properties, list_name: &str) -> Vec<String> {
        props::struct_values(save_parameter.0.get(&PropertyKey::from(list_name)).unwrap())
            .unwrap()
            .iter()
            .map(|value| {
                let StructValue::Struct(status_props) = value else {
                    panic!("expected a struct row");
                };
                status_props
                    .0
                    .get(&PropertyKey::from("StatusName"))
                    .and_then(props::as_str)
                    .unwrap()
                    .to_string()
            })
            .collect()
    }

    /// A newly-found Python bug (not on the PARITY-BUG-1/2 list -- see this
    /// task's report): `Player.status_point_list`'s setter mutates the list
    /// while iterating it, which skips every OTHER row among a run of
    /// consecutive "None"/unrecognized-name matches. Reproduced deliberately
    /// (not fixed with `Vec::retain`) for save-file byte parity. This exact
    /// four-row shape (`real, "None", "None", real`) was independently
    /// verified against real `.venv` CPython to reduce to THREE rows, not
    /// two -- see this task's report for the script and its output.
    #[test]
    fn apply_status_points_reproduces_pythons_consecutive_none_row_skip() {
        let mut save_parameter = Properties::default();
        save_parameter.insert(
            "GotStatusPointList",
            Property::Array(ValueVec::Struct(vec![
                status_point_struct("最大HP", 0),
                status_point_struct("None", 0),
                status_point_struct("None", 0),
                status_point_struct("攻撃力", 0),
            ])),
        );

        apply_status_points(
            &mut save_parameter,
            "GotStatusPointList",
            &OrderedMap::new(),
            &STATUS_NAME_MAP,
            true,
        );

        assert_eq!(
            names_of(&save_parameter, "GotStatusPointList"),
            vec![
                "最大HP".to_string(),
                "None".to_string(),
                "攻撃力".to_string(),
            ],
            "PYTHON BUG (reproduced deliberately, see this task's report): \
             of two CONSECUTIVE \"None\" rows, only the first is removed -- \
             `list.remove(item)` while iterating skips the row that shifts \
             into the just-vacated position, matching real CPython's \
             observed output for this exact input"
        );
    }

    /// Contrast case: non-consecutive "None" rows are NOT protected by the
    /// skip (there's no shift to hide behind) -- both are correctly removed,
    /// same as `Vec::retain` would produce. Proves the reproduction is
    /// exactly the skip, not an over-broad "keep more None rows" bug.
    #[test]
    fn apply_status_points_removes_non_consecutive_none_rows_normally() {
        let mut save_parameter = Properties::default();
        save_parameter.insert(
            "GotStatusPointList",
            Property::Array(ValueVec::Struct(vec![
                status_point_struct("最大HP", 0),
                status_point_struct("None", 0),
                status_point_struct("攻撃力", 0),
                status_point_struct("None", 0),
                status_point_struct("所持重量", 0),
            ])),
        );

        apply_status_points(
            &mut save_parameter,
            "GotStatusPointList",
            &OrderedMap::new(),
            &STATUS_NAME_MAP,
            true,
        );

        assert_eq!(
            names_of(&save_parameter, "GotStatusPointList"),
            vec![
                "最大HP".to_string(),
                "攻撃力".to_string(),
                "所持重量".to_string(),
            ]
        );
    }
}
