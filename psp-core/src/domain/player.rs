//! Player lazy load, full detail dump, write-back, and deletion.

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

/// The save stores stat names as Japanese strings; this maps them to the
/// English keys the DTO exposes. The last 12 arrived with Palworld 1.0 and are
/// granted by relics; an older save simply has no row for them.
pub const STATUS_NAME_MAP: [(&str, &str); 18] = [
    ("最大HP", "max_hp"),
    ("最大SP", "max_sp"),
    ("攻撃力", "attack"),
    ("所持重量", "weight"),
    ("捕獲率", "capture_rate"),
    ("作業速度", "work_speed"),
    ("空腹率低減", "hunger_reduction"),
    ("泳ぎ速度", "swim_speed"),
    ("食料腐敗低減", "food_decay_reduction"),
    ("ジャンプ力", "jump_power"),
    ("滑空速度", "glider_speed"),
    ("崖登り速度", "climb_speed"),
    ("状態異常耐性", "status_ailment_resist"),
    ("経験値ボーナス", "exp_bonus"),
    ("虹パッシブ率", "rainbow_passive_rate"),
    ("移動速度アップ", "move_speed"),
    ("パルスフィアホーミング", "sphere_homing"),
    ("スタミナ消費軽減", "stamina_reduction"),
];
/// `STATUS_NAME_MAP` minus `capture_rate`, which the extended stat list has no
/// entry for.
pub const EX_STATUS_NAME_MAP: [(&str, &str); 5] = [
    ("最大HP", "max_hp"),
    ("最大SP", "max_sp"),
    ("攻撃力", "attack"),
    ("所持重量", "weight"),
    ("作業速度", "work_speed"),
];

/// .NET/Palworld ticks -> an ISO-8601 string.
///
/// Delegates the tick math to `summary::ticks_to_datetime`. Do not recompute it
/// as `ticks as f64 / 10_000_000.0`: `f64` cannot hold a tick count precisely,
/// which silently corrupts any date past roughly year 1000.
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

pub(crate) fn container_id_from(save_data: &Properties, name: &str) -> Option<uuid::Uuid> {
    props::struct_props(save_data.0.get(&PropertyKey::from(name))?)
        .and_then(|inner| inner.0.get(&PropertyKey::from("ID")))
        .and_then(props::as_uuid)
}

/// The keys of a `Name -> Bool` unlock-flag map whose value is `true`.
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

/// One `english_name -> StatusPoint` entry per list element whose `StatusName`
/// resolves through `name_map`. A `"None"` or unrecognized row is skipped rather
/// than treated as an error: save data is untrusted input.
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

/// Lazily loads a player's `.sav`/`.dps` on first access, then dumps their full
/// DTO. `None` when the player has no file reference or no matching
/// character-map entry.
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

    // Reuse the `.sav` summary extraction already parsed, if it is still
    // cached, rather than reading and parsing the same file twice.
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

    // Warms `caches.player_guild_map`, which `build_player_dto` reads but --
    // taking `&SaveSession` -- cannot populate itself.
    if let Some(guild_id) = super::guild::find_player_guild_id(session, player_id)? {
        session.loaded_guilds.insert(guild_id);
        if let Some(summary) = session.guild_summaries.get_mut(&guild_id) {
            summary.loaded = true;
        }
    }

    build_player_dto(session, game_data, player_id)
}

/// Rebuilds the `PlayerDto` for an already-loaded player, without re-running the
/// lazy-load machinery. `None` when the player isn't in `loaded_players` or its
/// character-map entry has vanished.
///
/// `guild_id` comes from `session.caches.player_guild_map`, which this function
/// -- taking `&SaveSession` -- cannot populate, so it relies on that cache
/// already being warm.
pub fn build_player_dto(
    session: &SaveSession,
    game_data: &GameData,
    player_id: uuid::Uuid,
) -> Result<Option<PlayerDto>, CoreError> {
    let Some(loaded) = session.loaded_players.get(&player_id) else {
        return Ok(None);
    };

    // --- character-map side ---
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
            // A nameless player renders as this ninja-emoji placeholder. It is
            // deliberately NOT the sheep placeholder `PlayerSummary::nickname`
            // uses, and `apply_player_dto` treats this exact string as "no
            // nickname" on write-back.
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

    // --- player .sav side ---
    let save_data = save_data_props(&loaded.sav)?;
    // Older saves spell this key `inventoryInfo`; newer ones `InventoryInfo`.
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

    // A player with no `RecordData` at all reads as empty lists and 0.
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
    let effigy_possess_num = record_data
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

    // Populated only when a `_dps.sav` exists for this player; `None` (JSON
    // `null`) is a legitimate wire shape otherwise.
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

    // No zero-tick guard here (unlike `PlayerSummary::last_online_time`): 0
    // ticks legitimately serializes as the year-1 epoch. A missing or mistyped
    // `Timestamp` resolves to `None` rather than panicking.
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
            .get(&PropertyKey::from("bossTechnologyPoint")) // lowercase b: the save's own spelling
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
        effigy_possess_num,
        location,
        last_online_time,
        dps,
    }))
}

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

/// Each field is applied only when `Some`. `Err` when `player_id` isn't loaded.
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
    // This path does not go through `apply_player_dto`, so it must register the
    // `bossTechnologyPoint` schema itself or the resave fails. Idempotent.
    if wrote_boss_points {
        ensure_boss_technology_point_schema(&mut loaded.sav);
    }
    Ok(())
}

/// `_game_data` is unused here; the whole `update_*` family shares one uniform
/// `(session, game_data, modified, progress)` signature.
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

/// The five item containers live one level deeper, under
/// `InventoryInfo`/`inventoryInfo` (the older spelling), unlike
/// `PalStorageContainerId`/`OtomoCharacterContainerId`, which sit directly on
/// `SaveData`.
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

/// Applies every writable field from `dto` onto the player's character-map
/// `SaveParameter` bag and their own `.sav`'s `SaveData`/`RecordData`.
///
/// Container edits are routed by the ids resolved from the player's OWN
/// `InventoryInfo`, never by `dto.<container>.id`. A client-supplied id would
/// let a forged payload redirect the write onto an arbitrary container anywhere
/// in the save -- another player's inventory, a base's storage chest.
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
    // Player edits write `SanityValue`, but a player's raw SaveParameter may
    // carry no schema for that path, and uesave refuses to serialize an
    // unschema'd property -- `level_sav_bytes()` would then fail on every player
    // edit. Prime the shared per-path schemas pals already rely on.
    pal::ensure_pal_property_schemas(&mut session.level);
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
        // Some saves spell HP as "HP"; only "Hp" is written, so drop the stale
        // key rather than leave two disagreeing values in the entry.
        save_parameter.0.shift_remove(&PropertyKey::from("HP"));
        save_parameter.insert("FullStomach", props::float_property(dto.stomach as f32));
        save_parameter.insert("SanityValue", props::float_property(dto.sanity as f32));
        // The default placeholder nickname removes the property entirely rather
        // than persisting the placeholder itself.
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
            // One {QuestName, BlockIndex: 0, IntegerMap: {}, StringMap: {}}
            // struct per current mission.
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
        // The three writes above can each land on a property the save carries no
        // schema for; register those now. Each needs `&mut loaded.sav` for its
        // `.schemas` table, so the `save_data` borrow must already have ended.
        ensure_boss_technology_point_schema(&mut loaded.sav);
        ensure_player_quest_array_schemas(&mut loaded.sav);
        // Unlock flags apply only when the caller supplied a value.
        if let Some(points) = &dto.unlocked_fast_travel_points {
            // Fast travel has no relic counter of any kind; the delta is deliberately dropped.
            apply_unlock_flags(&mut loaded.sav, "FastTravelPointUnlockFlag", points);
        }
        if let Some(effigies) = &dto.collected_effigies {
            let delta = apply_unlock_flags(&mut loaded.sav, "RelicObtainForInstanceFlag", effigies);
            apply_effigy_relic_counters(&mut loaded.sav, effigies, delta);
        }
    }
    // Resolve every container id from the player's own save data first, then
    // apply. Common MUST be applied before essential: essential's
    // `AdditionalInventory_` resize targets the already-applied common container.
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

/// Writes `points` back onto a `StatusPoint` list.
///
/// A key with no row is APPENDED when its value is positive: the game creates a row
/// lazily, only once a rank is bought, so an absent row means rank 0 and setting such a
/// stat would otherwise silently do nothing. A key with no row and a value of 0 appends
/// nothing -- see the comment on that branch.
///
/// `drop_none_rows` first prunes `"None"`/unrecognized rows -- set for
/// `GotStatusPointList`, deliberately NOT for `GotExStatusPointList`, whose rows
/// are left as-is. A `points` key matching no `name_map` entry is skipped rather
/// than treated as an error: the DTO is untrusted input.
///
/// The prune pass advances its index even on a removal, so among a run of
/// consecutive removable rows only every other one is dropped. This quirk is
/// load-bearing for byte-compatible saves -- `Vec::retain` would remove all of
/// them and produce a different save. See the tests pinning both cases.
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
        // `index` advances on every step, INCLUDING after a removal -- the row
        // that shifts into the vacated slot is skipped. Intentional; see this
        // function's doc comment.
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
        let clamped = (*point_value).clamp(i32::MIN as i64, i32::MAX as i64) as i32;

        let existing = values.iter_mut().find(|value| match value {
            StructValue::Struct(status_props) => {
                status_props
                    .0
                    .get(&PropertyKey::from("StatusName"))
                    .and_then(props::as_str)
                    == Some(*japanese_name)
            }
            _ => false,
        });

        match existing {
            Some(StructValue::Struct(status_props)) => {
                status_props.insert("StatusPoint", props::int_property(clamped));
            }
            _ => {
                // No row. The game creates one lazily, only once a rank is bought, so an
                // absent row means rank 0 -- and a rank-0 stat must NOT create one. The UI
                // sends every relic key on save, so appending zeros here would add rows the
                // game never wrote to every file that passes through the editor.
                if clamped > 0 {
                    let mut status_props = Properties::default();
                    status_props.insert("StatusName", props::name_property(japanese_name));
                    status_props.insert("StatusPoint", props::int_property(clamped));
                    values.push(StructValue::Struct(status_props));
                }
            }
        }
    }
}

/// How a flag-map write changed the set of `true` keys. Both directions matter:
/// the worldmap UI un-collects an effigy on a single click, so a caller that only
/// counted additions would let an off/on toggle cycle ratchet a counter upward.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct FlagDelta {
    /// Keys in the new set that were not already `true`.
    added: usize,
    /// Keys that were `true` and are not in the new set.
    removed: usize,
}

/// Replaces the flag map's entries, every key set to `true`, and reports how the
/// `true` set changed (see `FlagDelta`).
///
/// This function does not touch any relic counter. `RelicPossessNum` is an
/// effigy-only concern and is handled by `apply_effigy_relic_counters`; folding it
/// in here is what made fast-travel unlocks increment a relic count.
///
/// A real save can legitimately carry a `RecordData` with no `flag_name` key. It
/// is created here, preceded by an `ensure_schema` at the
/// `SaveData.RecordData.<name>` path, because uesave's writer refuses to serialize
/// a property with no registered schema.
fn apply_unlock_flags(
    player_sav: &mut uesave::Save,
    flag_name: &str,
    keys: &[String],
) -> FlagDelta {
    let record_data_key = PropertyKey::from("RecordData");
    let flag_key = PropertyKey::from(flag_name);

    let has_record_data = save_data_props(player_sav)
        .ok()
        .and_then(|save_data| save_data.0.get(&record_data_key))
        .and_then(props::struct_props)
        .is_some();
    if !has_record_data {
        return FlagDelta::default();
    }

    let flag_already_present = save_data_props(player_sav)
        .ok()
        .and_then(|save_data| save_data.0.get(&record_data_key))
        .and_then(props::struct_props)
        .map(|record_data| record_data.0.contains_key(&flag_key))
        .unwrap_or(false);

    if !flag_already_present {
        if let Some(prefix) = props::schema_prefix_ending_with(player_sav, "RecordData") {
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
    }

    let Ok(save_data) = save_data_props_mut(player_sav) else {
        return FlagDelta::default();
    };
    let Some(record_data) = save_data
        .0
        .get_mut(&record_data_key)
        .and_then(props::struct_props_mut)
    else {
        return FlagDelta::default();
    };

    // Which keys were already true? Anything else in `keys` is newly unlocked, and any
    // of these missing from `keys` is being un-set by this write.
    let previously_true: std::collections::BTreeSet<String> = record_data
        .0
        .get(&flag_key)
        .and_then(props::map_entries)
        .map(|entries| {
            entries
                .iter()
                .filter(|entry| props::as_bool(&entry.value).unwrap_or(false))
                .filter_map(|entry| props::as_str(&entry.key).map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    // `keys` is untrusted input and may repeat a key; dedupe so a duplicate cannot be
    // counted as two additions (the map write below collapses it to one entry anyway).
    let now_true: std::collections::BTreeSet<&str> = keys.iter().map(String::as_str).collect();

    let delta = FlagDelta {
        added: now_true
            .iter()
            .filter(|key| !previously_true.contains(**key))
            .count(),
        removed: previously_true
            .iter()
            .filter(|key| !now_true.contains(key.as_str()))
            .count(),
    };

    let entries: Vec<uesave::MapEntry> = keys
        .iter()
        .map(|key| uesave::MapEntry {
            key: props::name_property(key),
            value: props::bool_property(true),
        })
        .collect();
    record_data.insert(flag_name, Property::Map(entries));

    delta
}

/// The relic type every effigy grants. Pre-1.0, effigies were the *only* relic, so
/// 1.0 kept the legacy flat fields as CapturePower-only mirrors and put the other
/// relic types exclusively in the by-type structures.
const CAPTURE_POWER_RELIC: &str = "EPalRelicType::CapturePower";

/// A relic type key. The save stores these as `EnumProperty`, but read `Name`/`Str`
/// too rather than silently skipping an entry we would then duplicate.
fn relic_type_name(property: &Property) -> Option<&str> {
    props::as_enum(property).or_else(|| props::as_str(property))
}

/// Applies `delta` to `RelicPossessNum` -- the count of *unspent* effigy relics the
/// player holds -- and, on a Palworld 1.0 save, keeps the structures the game
/// actually reads in agreement with it.
///
/// The counter moves by `added - removed`, floored at 0. Un-collecting matters
/// because the worldmap UI splices an effigy out of the set on a single click: were
/// only additions counted, an off/on toggle cycle would leave the flags identical
/// but ratchet the counter up by one every time. Removal giving a relic back also
/// makes this symmetric with the frontend, which already decrements the inventory
/// `Relic` item when an effigy is un-collected.
///
/// The floor is not cosmetic: a relic already spent on a rank cannot be un-spent, so
/// a real save holds fewer unspent relics than it has effigy flags, and un-collecting
/// them all must stop at 0 rather than go negative.
///
/// An unchanged resave has `added == removed == 0` and so leaves the counter alone.
///
/// A 1.0 save carries `RelicObtainForInstanceFlagByType`, `RelicPossessNumMap` and
/// `RelicBonusExpTableIndex` alongside the legacy pair, and
/// `bCaptureCompletionRelicFixupDone` is already `true`, so the game's one-time
/// migration has run and will never re-derive them from our legacy writes. These
/// invariants hold in every real 1.0 save examined, and are restored here:
///   flat flags              == the CapturePower by-type flag set
///   RelicPossessNum         == RelicPossessNumMap[CapturePower]
///   RelicBonusExpTableIndex == total true flags across ALL by-type entries
///
/// There is deliberately no `delta.is_zero()` early return: a save whose structures
/// are already out of sync must get repaired on resave, even when the edit changed no
/// flags.
///
/// A pre-1.0 save has none of the three, and every update below is conditional on
/// the property already existing, so we never invent them. `RelicPossessNum` itself
/// is the one property we may create -- but only when the delta is positive, so an
/// unchanged resave of a pre-1.0 save stays a strict no-op, and a removal-only edit
/// never conjures a `0` into a save that never carried the field.
fn apply_effigy_relic_counters(
    player_sav: &mut uesave::Save,
    effigies: &[String],
    delta: FlagDelta,
) {
    // Relics given back on removal, netted against those granted by new collections.
    let net = delta.added as i64 - delta.removed as i64;
    let record_data_key = PropertyKey::from("RecordData");
    let relic_key = PropertyKey::from("RelicPossessNum");
    let by_type_key = PropertyKey::from("RelicObtainForInstanceFlagByType");
    let possess_map_key = PropertyKey::from("RelicPossessNumMap");
    let exp_index_key = PropertyKey::from("RelicBonusExpTableIndex");

    let relic_already_present = save_data_props(player_sav)
        .ok()
        .and_then(|save_data| save_data.0.get(&record_data_key))
        .and_then(props::struct_props)
        .map(|record_data| record_data.0.contains_key(&relic_key))
        .unwrap_or(false);

    // A save predating the field carries no `RelicPossessNum`. Creating it needs an
    // `ensure_schema` first, because uesave's writer refuses to serialize a property
    // with no registered schema.
    if !relic_already_present && net > 0 {
        if let Some(prefix) = props::schema_prefix_ending_with(player_sav, "RecordData") {
            props::ensure_schema(
                player_sav,
                format!("{prefix}RecordData.RelicPossessNum"),
                uesave::PropertyTagPartial {
                    id: None,
                    data: uesave::PropertyTagDataPartial::Other(uesave::PropertyType::IntProperty),
                },
            );
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

    // Unspent effigy relics: moves by the net of collections and un-collections, floored
    // at 0 -- a spent relic cannot be un-spent, so the count must never go negative.
    // Writing a 0 into a save that never had the property would be inventing a field,
    // so don't.
    let possess = if relic_already_present || net > 0 {
        let current = record_data
            .0
            .get(&relic_key)
            .and_then(props::as_i32)
            .unwrap_or(0);
        let possess = (current as i64 + net).clamp(0, i32::MAX as i64) as i32;
        record_data.insert("RelicPossessNum", props::int_property(possess));
        Some(possess)
    } else {
        None
    };

    // Mirror the flat flags into the CapturePower by-type entry, when present.
    if let Some(by_type) = record_data
        .0
        .get_mut(&by_type_key)
        .and_then(props::struct_values_mut)
    {
        for value in by_type.iter_mut() {
            let StructValue::Struct(entry) = value else {
                continue;
            };
            let is_capture_power = entry
                .0
                .get(&PropertyKey::from("Type"))
                .and_then(relic_type_name)
                == Some(CAPTURE_POWER_RELIC);
            if !is_capture_power {
                continue;
            }
            let flags: Vec<uesave::MapEntry> = effigies
                .iter()
                .map(|key| uesave::MapEntry {
                    key: props::name_property(key),
                    value: props::bool_property(true),
                })
                .collect();
            entry.insert("Flags", Property::Map(flags));
            break;
        }
    }

    // RelicPossessNumMap[CapturePower] mirrors the scalar, when present. `possess` is
    // `None` only when the legacy scalar is absent *and* nothing was newly collected --
    // there is no value to mirror then, and writing the `0` default would zero a real
    // map entry. (Unreachable in every real save examined, where the scalar always
    // exists alongside the map, but the map write must not depend on that.)
    if let Some(possess) = possess {
        if let Some(entries) = record_data
            .0
            .get_mut(&possess_map_key)
            .and_then(props::map_entries_mut)
        {
            match entries
                .iter_mut()
                .find(|entry| relic_type_name(&entry.key) == Some(CAPTURE_POWER_RELIC))
            {
                Some(entry) => entry.value = props::int_property(possess),
                // The map's declared key type is EnumProperty, so a fresh key must be one
                // too -- a NameProperty here would not read back as a relic type.
                None => entries.push(uesave::MapEntry {
                    key: props::enum_property(CAPTURE_POWER_RELIC),
                    value: props::int_property(possess),
                }),
            }
        }
    }

    // RelicBonusExpTableIndex counts every by-type flag, not just CapturePower.
    if record_data.0.contains_key(&exp_index_key) {
        let total: i32 = record_data
            .0
            .get(&by_type_key)
            .and_then(props::struct_values)
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| match value {
                        StructValue::Struct(entry) => entry.0.get(&PropertyKey::from("Flags")),
                        _ => None,
                    })
                    .filter_map(props::map_entries)
                    .flatten()
                    .filter(|entry| props::as_bool(&entry.value).unwrap_or(false))
                    .count() as i32
            })
            .unwrap_or(0);
        record_data.insert("RelicBonusExpTableIndex", props::int_property(total));
    }
}

/// Registers the `SaveData.bossTechnologyPoint` schema when the player's `.sav`
/// carries none, so the unconditional write survives `uesave::Save::write` --
/// which rejects any property with no schema at its exact dotted path.
///
/// A save predating the field has `SaveData.TechnologyPoint` but not
/// `bossTechnologyPoint`, so the prefix is derived from that sibling. The
/// `.TechnologyPoint` match is unambiguous: `bossTechnologyPoint` is preceded by
/// `s`, not `.`. A `.sav` with no `TechnologyPoint` schema at all is a silent
/// no-op; the writer surfaces a clear error rather than this panicking.
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

/// Same missing-schema gap as `ensure_boss_technology_point_schema`, for the two
/// quest arrays: a player who has never started or completed a quest carries no
/// schema for either, and the writer rejects the unconditional write.
///
/// `uesave` looks each struct-array element field up at a flat
/// `<ArrayPath>.<FieldName>` path, so `OrderedQuestArray`'s four element fields
/// each need their own schema entry, not just the array itself.
fn ensure_player_quest_array_schemas(player_sav: &mut uesave::Save) {
    use uesave::{PropertyTagDataPartial, PropertyTagPartial, PropertyType, StructType};

    let Some(prefix) = props::schema_prefix_ending_with(player_sav, ".TechnologyPoint") else {
        return;
    };
    let tag = |data: PropertyTagDataPartial| PropertyTagPartial { id: None, data };
    let path = |name: &str| format!("{prefix}.{name}");

    props::ensure_schema(
        player_sav,
        path("CompletedQuestArray"),
        tag(PropertyTagDataPartial::Array(Box::new(
            PropertyTagDataPartial::Other(PropertyType::NameProperty),
        ))),
    );

    props::ensure_schema(
        player_sav,
        path("OrderedQuestArray"),
        tag(PropertyTagDataPartial::Array(Box::new(
            PropertyTagDataPartial::Struct {
                struct_type: StructType::Struct(Some("PalOrderedQuestSaveData".to_string())),
                id: uesave::FGuid::nil(),
            },
        ))),
    );
    props::ensure_schema(
        player_sav,
        path("OrderedQuestArray.QuestName"),
        tag(PropertyTagDataPartial::Other(PropertyType::NameProperty)),
    );
    props::ensure_schema(
        player_sav,
        path("OrderedQuestArray.BlockIndex"),
        tag(PropertyTagDataPartial::Other(PropertyType::IntProperty)),
    );
    props::ensure_schema(
        player_sav,
        path("OrderedQuestArray.IntegerMap"),
        tag(PropertyTagDataPartial::Map {
            key_type: Box::new(PropertyTagDataPartial::Other(PropertyType::NameProperty)),
            value_type: Box::new(PropertyTagDataPartial::Other(PropertyType::IntProperty)),
        }),
    );
    props::ensure_schema(
        player_sav,
        path("OrderedQuestArray.StringMap"),
        tag(PropertyTagDataPartial::Map {
            key_type: Box::new(PropertyTagDataPartial::Other(PropertyType::NameProperty)),
            value_type: Box::new(PropertyTagDataPartial::Other(PropertyType::StrProperty)),
        }),
    );
}

/// `Ok(false)` when the player is their guild's admin -- an admin cannot be
/// deleted, and nothing is touched. `Err` when `player_id` isn't loaded, before
/// any mutation.
///
/// The guild lookup is scoped to already-LOADED guilds. A player whose guild was
/// never loaded this session is treated as guildless: no admin check, no
/// guild-handle removal.
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
            let guild = super::guild_tail::as_guild(group_data)
                .ok_or_else(|| CoreError::Parse("guild group data untyped".into()))?;
            (
                guild.guild_name.clone(),
                super::guild_tail::guild_player_uids(guild).first().copied(),
            )
        };
        if admin_uid == Some(player_id) {
            return Ok(false);
        }
        progress(&format!(
            "Deleting player {nickname} from guild {guild_name}"
        ));
        // Drop the player's own character handle and their member row. uesave
        // re-serializes the structured guild on save, so removing the row in
        // place is the whole write -- no blob re-encode.
        let entries = world::group_map_mut(&mut session.level)?;
        if let Some(group_data) = super::guild_tail::entry_group_data_mut(&mut entries[entry_index])
        {
            group_data.individual_character_handle_ids.retain(|handle| {
                props::guid_to_uuid(&handle.instance_id) != player_id
                    && props::guid_to_uuid(&handle.guid) != player_id
            });
            if let Some(guild) = super::guild_tail::as_guild_mut(group_data) {
                super::guild_tail::remove_player(guild, player_id);
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

/// Deletes every pal the player's own pal box and party reference, then the
/// player's character-map entry, file ref, and `LoadedPlayer`. Returns the five
/// item-container ids and two character-container ids the CALLER must still
/// delete -- both `delete_player` and `guild::delete_guild_and_players` use this.
///
/// Does NOT remove the deleted pals' guild character handles, so their
/// `individual_character_handle_ids` entries are left dangling in the guild
/// tail. (Base pals, deleted via `pal::delete_guild_pals`, DO get theirs
/// cleaned up -- the asymmetry is intentional.)
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

    /// Among a run of CONSECUTIVE "None" rows, only every other one is dropped:
    /// the prune pass advances its index even after a removal. Pinned because a
    /// `Vec::retain` "fix" would drop all of them and change the written save.
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
            "of two CONSECUTIVE \"None\" rows, only the first is removed -- \
             the prune pass skips the row that shifts into the vacated slot"
        );
    }

    /// Contrast case: non-consecutive "None" rows are both removed -- there is
    /// no shift for them to hide behind. Bounds the skip to exactly the
    /// consecutive case.
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
