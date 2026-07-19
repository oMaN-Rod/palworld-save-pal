//! Guild lookup, detail loading, updates, and deletion.

use std::collections::HashMap;

use crate::dto::container::{CharacterContainerDto, ItemContainerDto};
use crate::dto::guild::{BaseDto, GuildDto, GuildLabResearchInfo};
use crate::dto::ordered_map::OrderedMap;
use crate::dto::pal::PalDto;
use crate::dto::player::WorldMapPointDto;
use crate::error::CoreError;
use crate::gamedata::GameData;
use crate::props;
use crate::session::SaveSession;
use crate::ue::{Properties, Property, PropertyKey, StructValue};

use super::{containers, guild_tail, pal, world};

/// From a `BaseCampSaveData` entry: `(group_id_belong_to, WorkerDirector
/// container_id)`.
///
/// `uesave` registers `BaseCampSaveData.WorkerDirector.RawData` as a generic
/// `Struct(None)` hint, which it never decodes, so the property arrives as a
/// raw byte array. `palbin::worker_director_container_id` bounds-checks and
/// parses that fixed 118-byte layout.
pub fn base_guild_and_container(entry: &crate::ue::MapEntry) -> Option<(uuid::Uuid, uuid::Uuid)> {
    let value_properties = props::struct_props(&entry.value)?;
    let raw_data = props::get(value_properties, &["RawData"])?;
    let crate::ue::Property::Struct(crate::ue::StructValue::Game(crate::ue::PalStruct::BaseCamp(base_camp))) = raw_data else {
        return None;
    };
    let guild_id = props::guid_to_uuid(&base_camp.group_id_belong_to);

    let worker_director_blob = props::get(value_properties, &["WorkerDirector", "RawData"])
        .and_then(props::as_byte_array)?;
    let container_id = crate::palbin::worker_director_container_id(worker_director_blob).ok()?;

    Some((guild_id, container_id))
}

/// The guild whose member list contains `player_id`, via a cached
/// `player uid -> guild id` map built on first call.
///
/// A guild group whose tail doesn't decode contributes no entries rather than
/// aborting the scan: save data is untrusted, so malformed entries are skipped.
pub fn find_player_guild_id(
    session: &mut crate::session::SaveSession,
    player_id: uuid::Uuid,
) -> Result<Option<uuid::Uuid>, crate::error::CoreError> {
    if session.caches.player_guild_map.is_none() {
        let mut player_guild_map = std::collections::HashMap::new();
        for entry in super::world::group_map(&session.level)? {
            if super::guild_tail::entry_group_type(entry).as_deref() != Some("EPalGroupType::Guild")
            {
                continue;
            }
            let Some(guild_id) = crate::props::as_uuid(&entry.key) else {
                continue;
            };
            let Some(group_data) = super::guild_tail::entry_group_data(entry) else {
                continue;
            };
            let Some(guild) = super::guild_tail::as_guild(group_data) else {
                continue;
            };
            for player_uid in super::guild_tail::guild_player_uids(guild) {
                player_guild_map.insert(player_uid, guild_id);
            }
        }
        session.caches.player_guild_map = Some(player_guild_map);
    }
    Ok(session
        .caches
        .player_guild_map
        .as_ref()
        .and_then(|map| map.get(&player_id).copied()))
}

pub fn guild_entry_index(
    session: &SaveSession,
    guild_id: uuid::Uuid,
) -> Result<Option<usize>, CoreError> {
    Ok(world::group_map(&session.level)?
        .iter()
        .position(|entry| props::as_uuid(&entry.key) == Some(guild_id)))
}

/// `GuildExtraSaveDataMap` is optional in the save format, so a `None` result
/// means either "no such map" or "no such guild in it".
pub fn guild_extra_entry_index(
    session: &SaveSession,
    guild_id: uuid::Uuid,
) -> Result<Option<usize>, CoreError> {
    let entries = world::guild_extra_map(&session.level)?;
    Ok(entries.and_then(|entries| {
        entries
            .iter()
            .position(|entry| props::as_uuid(&entry.key) == Some(guild_id))
    }))
}

/// `GuildExtraSaveDataMap[i].Lab.RawData`, typed as `PalGuildLab`.
fn guild_extra_lab(
    session: &SaveSession,
    extra_index: usize,
) -> Option<&crate::ue::games::palworld::PalGuildLab> {
    let entries = world::guild_extra_map(&session.level).ok().flatten()?;
    let value_props = props::struct_props(&entries.get(extra_index)?.value)?;
    let lab_props = props::struct_props(value_props.0.get(&PropertyKey::from("Lab"))?)?;
    match lab_props.0.get(&PropertyKey::from("RawData"))? {
        Property::Struct(StructValue::Game(crate::ue::PalStruct::GuildLab(lab))) => Some(lab),
        _ => None,
    }
}

/// The guild chest's container id, from
/// `GuildExtraSaveDataMap[i].GuildItemStorage.RawData.container_id`.
fn guild_chest_container_id(session: &SaveSession, extra_index: usize) -> Option<uuid::Uuid> {
    let entries = world::guild_extra_map(&session.level).ok().flatten()?;
    let value_props = props::struct_props(&entries.get(extra_index)?.value)?;
    let storage_props =
        props::struct_props(value_props.0.get(&PropertyKey::from("GuildItemStorage"))?)?;
    match storage_props.0.get(&PropertyKey::from("RawData"))? {
        Property::Struct(StructValue::Game(crate::ue::PalStruct::GuildItemStorage(storage))) => {
            Some(props::guid_to_uuid(&storage.container_id))
        }
        _ => None,
    }
}

/// The container a base pal sits in, via `SaveParameter.SlotId.ContainerId.ID`.
///
/// Only the `"SlotId"` spelling is accepted here -- deliberately narrower than
/// `pal::read_save_parameter_dto`, which also falls back to `"SlotID"`.
pub(crate) fn base_container_membership(save_parameter: &Properties) -> Option<uuid::Uuid> {
    let slot = pal::param(save_parameter, "SlotId").and_then(props::struct_props)?;
    slot.0
        .get(&PropertyKey::from("ContainerId"))
        .and_then(props::struct_props)
        .and_then(|container| container.0.get(&PropertyKey::from("ID")))
        .and_then(props::as_uuid)
}

/// Groups every `MapObjectSaveData` element by its
/// `Model.RawData.base_camp_id_belong_to`, so a base's map objects resolve in
/// O(1) instead of rescanning the whole array once per base.
fn map_object_properties_by_base_id(
    map_objects: &[StructValue],
) -> HashMap<uuid::Uuid, Vec<&Properties>> {
    let mut index: HashMap<uuid::Uuid, Vec<&Properties>> = HashMap::new();
    for map_object in map_objects {
        let StructValue::Struct(object_props) = map_object else {
            continue;
        };
        let Some(model_props) = object_props
            .0
            .get(&PropertyKey::from("Model"))
            .and_then(props::struct_props)
        else {
            continue;
        };
        let Some(Property::Struct(StructValue::Game(crate::ue::PalStruct::MapModel(model)))) =
            model_props.0.get(&PropertyKey::from("RawData"))
        else {
            continue;
        };
        let base_id = props::guid_to_uuid(&model.base_camp_id_belong_to);
        index.entry(base_id).or_default().push(object_props);
    }
    index
}

/// `target_container_id` from an ItemContainer module's typed `RawData`.
fn module_target_container_id(raw_data: &Property) -> Option<uuid::Uuid> {
    let Property::Struct(StructValue::Game(crate::ue::PalStruct::MapConcreteModelModule(module))) = raw_data else {
        return None;
    };
    match &module.data {
        crate::ue::games::palworld::PalMapConcreteModelModuleData::ItemContainer {
            target_container_id,
            ..
        } => Some(props::guid_to_uuid(target_container_id)),
        _ => None,
    }
}

/// Loads a guild's full detail DTO. `None` when the guild id doesn't resolve,
/// or when it has no `GuildExtraSaveDataMap` entry (a guild without one cannot
/// be loaded at all).
pub fn get_guild_details(
    session: &mut SaveSession,
    game_data: &GameData,
    guild_id: uuid::Uuid,
) -> Result<Option<GuildDto>, CoreError> {
    let Some(entry_index) = guild_entry_index(session, guild_id)? else {
        return Ok(None);
    };
    let Some(extra_index) = guild_extra_entry_index(session, guild_id)? else {
        return Ok(None);
    };

    let Some(dto) = build_guild_dto(session, game_data, guild_id, entry_index, extra_index)? else {
        return Ok(None);
    };

    session.loaded_guilds.insert(guild_id);
    if let Some(summary) = session.guild_summaries.get_mut(&guild_id) {
        summary.loaded = true;
    }

    Ok(Some(dto))
}

fn build_guild_dto(
    session: &SaveSession,
    game_data: &GameData,
    guild_id: uuid::Uuid,
    entry_index: usize,
    extra_index: usize,
) -> Result<Option<GuildDto>, CoreError> {
    let (guild_name, base_camp_level, players, admin_player_uid) = {
        let entries = world::group_map(&session.level)?;
        let Some(group_data) = guild_tail::entry_group_data(&entries[entry_index]) else {
            return Ok(None);
        };
        let Some(guild) = guild_tail::as_guild(group_data) else {
            return Ok(None);
        };
        let players: Vec<uuid::Uuid> = guild_tail::guild_player_uids(guild);
        // The admin is the first member row in the guild tail; an empty member
        // list is a normal "no admin" case, not an error.
        let admin = players.first().copied();
        (
            guild.guild_name.clone(),
            guild.base_camp_level,
            players,
            admin,
        )
    };

    let lab_research: Vec<GuildLabResearchInfo> = guild_extra_lab(session, extra_index)
        .map(|lab| {
            lab.research_info
                .iter()
                .map(|info| GuildLabResearchInfo {
                    research_id: info.research_id.clone(),
                    work_amount: info.work_amount as f64,
                })
                .collect()
        })
        .unwrap_or_default();

    let mut caches_scratch = crate::session::WorldCaches::default();
    let container_id = guild_chest_container_id(session, extra_index);
    let guild_chest = container_id.and_then(|chest_id| {
        containers::read_item_container(
            &session.level,
            &mut caches_scratch,
            game_data,
            chest_id,
            "GuildChest",
            Some("GuildChest".to_string()),
        )
    });

    let map_object_index = world::map_object_values(&session.level)?
        .map(|values| map_object_properties_by_base_id(values))
        .unwrap_or_default();
    let empty_map_objects: Vec<&Properties> = Vec::new();
    let character_container_index = world::build_character_container_index(&session.level);
    let base_camp_entries: &[crate::ue::MapEntry] = world::base_camp_map(&session.level)?
        .map(|entries| entries.as_slice())
        .unwrap_or(&[]);

    let base_entries_info: Vec<(uuid::Uuid, uuid::Uuid)> = base_camp_entries
        .iter()
        .filter_map(|base_entry| {
            let base_id = props::as_uuid(&base_entry.key)?;
            let (owner_guild, worker_container_id) = base_guild_and_container(base_entry)?;
            (owner_guild == guild_id).then_some((base_id, worker_container_id))
        })
        .collect();

    let mut bases: OrderedMap<uuid::Uuid, BaseDto> = OrderedMap::new();
    for (base_id, worker_container_id) in base_entries_info {
        let Some(container_entry_index) = character_container_index.get(&worker_container_id)
        else {
            continue;
        };
        let Some(container_view) =
            containers::read_character_container(&session.level, *container_entry_index)
        else {
            continue;
        };

        let mut base_pals: OrderedMap<uuid::Uuid, PalDto> = OrderedMap::new();
        for pal_entry in world::character_map(&session.level)? {
            if world::entry_is_player(pal_entry) {
                continue;
            }
            let Some(save_parameter) = world::entry_save_parameter(pal_entry) else {
                continue;
            };
            if base_container_membership(save_parameter) != Some(worker_container_id) {
                continue;
            }
            let Some(pal_dto) = pal::pal_dto_from_entry(pal_entry, game_data) else {
                continue;
            };
            base_pals.insert(pal_dto.instance_id, pal_dto);
        }

        let base_entry = base_camp_entries
            .iter()
            .find(|entry| props::as_uuid(&entry.key) == Some(base_id));
        let (base_name, area_range, location) = base_entry
            .and_then(|entry| props::struct_props(&entry.value))
            .and_then(|value_props| value_props.0.get(&PropertyKey::from("RawData")))
            .map(|raw_data| match raw_data {
                Property::Struct(StructValue::Game(crate::ue::PalStruct::BaseCamp(base_camp))) => (
                    Some(base_camp.name.clone()),
                    Some(base_camp.area_range as f64),
                    Some(WorldMapPointDto {
                        x: base_camp.transform.translation.x.0,
                        y: base_camp.transform.translation.y.0,
                        z: base_camp.transform.translation.z.0,
                    }),
                ),
                _ => (None, None, None),
            })
            .unwrap_or((None, None, None));

        // A base's storage containers are its map objects that carry an
        // ItemContainer module.
        let mut storage_containers: OrderedMap<uuid::Uuid, ItemContainerDto> = OrderedMap::new();
        let base_map_objects = map_object_index.get(&base_id).unwrap_or(&empty_map_objects);
        for object_props in base_map_objects.iter().copied() {
            let map_object_id = object_props
                .0
                .get(&PropertyKey::from("MapObjectId"))
                .and_then(props::as_str)
                .map(str::to_string);
            let Some(concrete_props) = object_props
                .0
                .get(&PropertyKey::from("ConcreteModel"))
                .and_then(props::struct_props)
            else {
                continue;
            };
            let Some(module_entries) = concrete_props
                .0
                .get(&PropertyKey::from("ModuleMap"))
                .and_then(props::map_entries)
            else {
                continue;
            };
            for module in module_entries {
                if props::as_str(&module.key)
                    != Some("EPalMapObjectConcreteModelModuleType::ItemContainer")
                {
                    continue;
                }
                let Some(module_props) = props::struct_props(&module.value) else {
                    continue;
                };
                let Some(target_container_id) = module_props
                    .0
                    .get(&PropertyKey::from("RawData"))
                    .and_then(module_target_container_id)
                else {
                    continue;
                };
                if let Some(container_dto) = containers::read_item_container(
                    &session.level,
                    &mut caches_scratch,
                    game_data,
                    target_container_id,
                    "BaseContainer",
                    map_object_id.clone(),
                ) {
                    storage_containers.insert(target_container_id, container_dto);
                }
            }
        }

        bases.insert(
            base_id,
            BaseDto {
                pals: base_pals,
                container_id: Some(worker_container_id),
                slot_count: Some(container_view.size),
                storage_containers,
                pal_container: Some(CharacterContainerDto {
                    id: worker_container_id,
                    player_uid: props::EMPTY_UUID,
                    r#type: "Base".to_string(),
                    size: container_view.size,
                    slots: container_view.slots,
                }),
                id: base_id,
                name: base_name,
                location,
                area_range,
            },
        );
    }

    Ok(Some(GuildDto {
        bases: Some(bases),
        guild_chest,
        lab_research: Some(lab_research.clone()),
        name: Some(guild_name),
        base_camp_level: Some(base_camp_level),
        id: Some(guild_id),
        admin_player_uid,
        players,
        container_id,
        lab_research_data: lab_research,
    }))
}

/// Fully replaces the guild's lab `research_info`, leaving
/// `current_research_id` and `trailing_bytes` alone. Writes only into
/// `GuildExtraSaveDataMap`'s `Lab.RawData`, never the guild tail.
///
/// `Err(GuildNotFound)` only when the guild id itself is not loaded. Once it
/// resolves, every other failure (missing extra entry, missing `Lab`, untyped
/// `Lab.RawData`) is a silent no-op. `work_amount` narrows to `f32` because the
/// save persists it as IEEE-754 single precision.
pub fn update_lab_research(
    session: &mut SaveSession,
    guild_id: uuid::Uuid,
    research_updates: &[GuildLabResearchInfo],
) -> Result<(), CoreError> {
    if !session.loaded_guilds.contains(&guild_id) {
        return Err(CoreError::GuildNotFound(guild_id));
    }
    let Some(extra_index) = guild_extra_entry_index(session, guild_id)? else {
        return Err(CoreError::GuildNotFound(guild_id));
    };
    let Some(entries) = world::guild_extra_map_mut(&mut session.level)? else {
        return Err(CoreError::GuildNotFound(guild_id));
    };
    let Some(entry) = entries.get_mut(extra_index) else {
        return Err(CoreError::GuildNotFound(guild_id));
    };
    let Some(value_props) = props::struct_props_mut(&mut entry.value) else {
        return Ok(());
    };
    let Some(lab_props) = value_props
        .0
        .get_mut(&PropertyKey::from("Lab"))
        .and_then(props::struct_props_mut)
    else {
        return Ok(());
    };
    let Some(Property::Struct(StructValue::Game(crate::ue::PalStruct::GuildLab(lab)))) =
        lab_props.0.get_mut(&PropertyKey::from("RawData"))
    else {
        return Ok(());
    };
    lab.research_info = research_updates
        .iter()
        .map(|info| crate::ue::games::palworld::PalLabResearchInfo {
            research_id: info.research_id.clone(),
            work_amount: info.work_amount as f32,
        })
        .collect();
    Ok(())
}

/// The item-container ids that genuinely belong to `base_id`'s own storage.
/// `containers::apply_base_dto` uses this to reject a client-supplied
/// `storage_containers` key that doesn't belong to this base. An unresolvable
/// `base_id` yields an empty set rather than an error.
pub(crate) fn base_storage_container_ids(
    session: &SaveSession,
    base_id: uuid::Uuid,
) -> std::collections::HashSet<uuid::Uuid> {
    let mut ids = std::collections::HashSet::new();
    let Ok(Some(map_objects)) = world::map_object_values(&session.level) else {
        return ids;
    };
    let index = map_object_properties_by_base_id(map_objects);
    let Some(objects) = index.get(&base_id) else {
        return ids;
    };
    for object_props in objects {
        let Some(concrete_props) = object_props
            .0
            .get(&PropertyKey::from("ConcreteModel"))
            .and_then(props::struct_props)
        else {
            continue;
        };
        let Some(module_entries) = concrete_props
            .0
            .get(&PropertyKey::from("ModuleMap"))
            .and_then(props::map_entries)
        else {
            continue;
        };
        for module in module_entries {
            if props::as_str(&module.key)
                != Some("EPalMapObjectConcreteModelModuleType::ItemContainer")
            {
                continue;
            }
            let Some(module_props) = props::struct_props(&module.value) else {
                continue;
            };
            if let Some(target_id) = module_props
                .0
                .get(&PropertyKey::from("RawData"))
                .and_then(module_target_container_id)
            {
                ids.insert(target_id);
            }
        }
    }
    ids
}

/// The guild chest's container id as resolved from the save itself. Guild-chest
/// edits route through this, never through the client-supplied
/// `GuildDto::guild_chest.id`, so a forged id cannot redirect the write.
pub(crate) fn guild_chest_id(session: &SaveSession, guild_id: uuid::Uuid) -> Option<uuid::Uuid> {
    let extra_index = guild_extra_entry_index(session, guild_id).ok().flatten()?;
    guild_chest_container_id(session, extra_index)
}

/// `_game_data` is unused here; the whole `update_*` family shares one uniform
/// `(session, game_data, modified, progress)` signature.
pub fn update_guilds(
    session: &mut SaveSession,
    _game_data: &GameData,
    modified_guilds: &crate::dto::ordered_map::OrderedMap<uuid::Uuid, GuildDto>,
    progress: &crate::progress::ProgressSink,
) -> Result<(), CoreError> {
    for (guild_id, dto) in modified_guilds.iter() {
        progress(&format!("Updating guild {guild_id}"));
        apply_guild_dto(session, *guild_id, dto)?;
    }
    Ok(())
}

pub fn apply_guild_dto(
    session: &mut SaveSession,
    guild_id: uuid::Uuid,
    dto: &GuildDto,
) -> Result<(), CoreError> {
    let Some(entry_index) = guild_entry_index(session, guild_id)? else {
        return Err(CoreError::GuildNotFound(guild_id));
    };
    {
        let entries = world::group_map_mut(&mut session.level)?;
        let Some(group_data) = guild_tail::entry_group_data_mut(&mut entries[entry_index]) else {
            return Err(CoreError::Parse("guild group data untyped".into()));
        };
        let Some(guild) = guild_tail::as_guild_mut(group_data) else {
            return Err(CoreError::Parse("guild group data untyped".into()));
        };
        // An absent OR empty name means "leave it alone", per this API's
        // contract.
        if let Some(name) = &dto.name {
            if !name.is_empty() {
                guild.guild_name = name.clone();
            }
        }
        // Likewise level 0 means "leave it alone". uesave re-serializes the
        // structured guild on save, so mutating the field in place is the whole
        // write -- no blob re-encode needed.
        if let Some(level) = dto.base_camp_level {
            if level != 0 {
                guild.base_camp_level = level;
            }
        }
    }
    if let Some(bases) = &dto.bases {
        for (base_id, base_dto) in bases.iter() {
            super::containers::apply_base_dto(session, *base_id, base_dto)?;
        }
    }
    // Route the chest edit through the id resolved from the save, not the
    // client-supplied `dto.guild_chest.id`.
    if dto.guild_chest.is_some() {
        if let Some(chest_id) = guild_chest_id(session, guild_id) {
            if let Some(chest_dto) = &dto.guild_chest {
                super::containers::apply_item_container_dto(session, chest_id, chest_dto, None)?;
            }
        }
    }
    Ok(())
}

/// Delete a `MapObjectSaveData` element when `Model.RawData.group_id_belong_to`
/// is `guild_id`, OR `Model.RawData.build_player_uid` is one of `player_ids`,
/// OR (ItemBooth concrete models only) `private_lock_player_uid` or any
/// `trade_infos[].seller_player_uid` is one of `player_ids`.
///
/// A `ConcreteModel.ModuleMap` `PasswordLock` module recording a target player
/// is deliberately NOT a delete trigger: the game never treats a lock record as
/// ownership, and honoring it here would destroy map objects other players
/// merely had access to.
pub(crate) fn should_delete_map_object(
    map_object: &StructValue,
    guild_id: Option<uuid::Uuid>,
    player_ids: &[uuid::Uuid],
) -> bool {
    let StructValue::Struct(object_props) = map_object else {
        return false;
    };
    let Some(model_props) = object_props
        .0
        .get(&PropertyKey::from("Model"))
        .and_then(props::struct_props)
    else {
        return false;
    };
    let Some(Property::Struct(StructValue::Game(crate::ue::PalStruct::MapModel(model)))) =
        model_props.0.get(&PropertyKey::from("RawData"))
    else {
        return false;
    };
    if let Some(target_guild) = guild_id {
        if props::guid_to_uuid(&model.group_id_belong_to) == target_guild {
            return true;
        }
    }
    if player_ids.contains(&props::guid_to_uuid(&model.build_player_uid)) {
        return true;
    }

    // ItemBooth edge cases: private lock owner, or any trade-info seller.
    let Some(concrete_props) = object_props
        .0
        .get(&PropertyKey::from("ConcreteModel"))
        .and_then(props::struct_props)
    else {
        return false;
    };
    let Some(Property::Struct(StructValue::Game(crate::ue::PalStruct::MapConcreteModel(concrete)))) =
        concrete_props.0.get(&PropertyKey::from("RawData"))
    else {
        return false;
    };
    if let crate::ue::games::palworld::PalMapConcreteModelVariant::ItemBooth(booth) =
        &concrete.model_data
    {
        if player_ids.contains(&props::guid_to_uuid(&booth.private_lock_player_uid)) {
            return true;
        }
        if booth
            .trade_infos
            .iter()
            .any(|trade| player_ids.contains(&props::guid_to_uuid(&trade.seller_player_uid)))
        {
            return true;
        }
    }
    false
}

/// Deletes a guild, its bases, its map objects, and its loaded members.
///
/// `Err` when `guild_id` isn't loaded. The check reads `session.loaded_guilds`
/// directly rather than calling `get_guild_details`, which would lazily load an
/// unloaded guild as a side effect.
///
/// Only player containers and base storage containers are collected for
/// deletion; the guild's own chest container is intentionally left behind as an
/// orphaned `ItemContainerSaveData` entry.
pub fn delete_guild_and_players(
    session: &mut SaveSession,
    game_data: &GameData,
    guild_id: uuid::Uuid,
    progress: &crate::progress::ProgressSink,
) -> Result<(), CoreError> {
    if !session.loaded_guilds.contains(&guild_id) {
        return Err(CoreError::Other(format!(
            "Guild {guild_id} not found in the save file."
        )));
    }
    let details = get_guild_details(session, game_data, guild_id)?
        .ok_or_else(|| CoreError::Other(format!("Guild {guild_id} not found in the save file.")))?;
    let guild_name = details.name.clone().unwrap_or_default();
    let guild_players = details.players.clone();
    progress(&format!(
        "Deleting guild {guild_name} with {} players",
        guild_players.len()
    ));

    if let Some(values) = world::map_object_values_mut(&mut session.level)? {
        values.retain(|map_object| {
            !should_delete_map_object(map_object, Some(guild_id), &guild_players)
        });
    }

    let mut item_container_ids: Vec<uuid::Uuid> = Vec::new();
    let mut character_container_ids: Vec<uuid::Uuid> = Vec::new();

    // An unloaded member is skipped entirely -- their containers and pals are
    // only knowable from their own loaded `.sav`.
    for player_uid in &guild_players {
        if !session.loaded_players.contains_key(player_uid) {
            continue;
        }
        let Some(player_details) =
            super::player::build_player_dto(session, game_data, *player_uid)?
        else {
            continue;
        };
        let (player_items, player_characters) = super::player::delete_player_and_pals_for_guild(
            session,
            game_data,
            *player_uid,
            &player_details,
            progress,
        )?;
        item_container_ids.extend(player_items);
        character_container_ids.extend(player_characters);
    }

    if let Some(entries) = world::guild_extra_map_mut(&mut session.level)? {
        entries.retain(|entry| props::as_uuid(&entry.key) != Some(guild_id));
    }

    if let Some(bases) = &details.bases {
        for (base_id, base) in bases.iter() {
            progress(&format!("Deleting base {base_id}"));
            item_container_ids.extend(base.storage_containers.iter().map(|(id, _)| *id));
            if let Some(worker_container) = base.container_id {
                character_container_ids.push(worker_container);
            }
            let base_pal_ids: Vec<uuid::Uuid> = base.pals.iter().map(|(id, _)| *id).collect();
            // `delete_guild_pals` (not a raw `delete_pal_entry` per id) so each
            // base pal's `individual_character_handle_ids` entry in the guild
            // tail is removed too.
            super::pal::delete_guild_pals(session, guild_id, *base_id, &base_pal_ids)?;
            if let Some(entries) = world::base_camp_map_mut(&mut session.level)? {
                entries.retain(|entry| props::as_uuid(&entry.key) != Some(*base_id));
            }
        }
    }

    progress(&format!("Deleting item containers of guild {guild_name}"));
    super::containers::delete_item_containers(session, &item_container_ids)?;

    progress(&format!(
        "Deleting character containers of guild {guild_name}"
    ));
    super::containers::delete_character_containers(session, &character_container_ids)?;

    world::group_map_mut(&mut session.level)?
        .retain(|entry| props::as_uuid(&entry.key) != Some(guild_id));
    session.loaded_guilds.remove(&guild_id);
    session.invalidate_performance_caches();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palbin::test_bytes::shuffle_guid_bytes;
    use crate::ue::games::palworld::{PalBaseCamp, PalTransform};
    use crate::ue::{
        ByteArray, Double, MapEntry, Properties, Property, Quat, StructValue, ValueVec, Vector,
    };

    const GUILD_ID: &str = "33333333-3333-3333-3333-333333333333";
    const BASE_ID: &str = "44444444-4444-4444-4444-444444444444";
    const CONTAINER_ID: &str = "55555555-5555-5555-5555-555555555555";

    fn fguid(text: &str) -> crate::ue::FGuid {
        serde_json::from_value(serde_json::Value::String(text.to_string())).unwrap()
    }

    fn guid_property(text: &str) -> Property {
        Property::Struct(StructValue::Guid(fguid(text)))
    }

    fn zero_transform() -> PalTransform {
        PalTransform {
            rotation: Quat {
                x: Double(0.0),
                y: Double(0.0),
                z: Double(0.0),
                w: Double(1.0),
            },
            translation: Vector {
                x: Double(0.0),
                y: Double(0.0),
                z: Double(0.0),
            },
            scale: Vector {
                x: Double(1.0),
                y: Double(1.0),
                z: Double(1.0),
            },
        }
    }

    fn worker_director_blob(container_id: &str) -> Vec<u8> {
        let mut blob = vec![0u8; 118];
        let display_bytes = *container_id.parse::<uuid::Uuid>().unwrap().as_bytes();
        blob[98..114].copy_from_slice(&shuffle_guid_bytes(display_bytes));
        blob
    }

    fn base_camp_entry(base_id: &str, guild_id: &str, worker_container_id: &str) -> MapEntry {
        let camp = PalBaseCamp {
            id: fguid(base_id),
            name: String::new(),
            state: 0,
            transform: zero_transform(),
            area_range: 0.0,
            group_id_belong_to: fguid(guild_id),
            fast_travel_local_transform: zero_transform(),
            owner_map_object_instance_id: crate::ue::FGuid::nil(),
            trailing_bytes: [0; 4],
        };
        let mut worker_properties = Properties::default();
        worker_properties.insert(
            "RawData",
            Property::Array(ValueVec::Byte(ByteArray::Byte(worker_director_blob(
                worker_container_id,
            )))),
        );
        let mut value_properties = Properties::default();
        value_properties.insert(
            "RawData",
            Property::Struct(StructValue::Game(crate::ue::PalStruct::BaseCamp(Box::new(camp)))),
        );
        value_properties.insert(
            "WorkerDirector",
            Property::Struct(StructValue::Struct(worker_properties)),
        );
        MapEntry {
            key: guid_property(base_id),
            value: Property::Struct(StructValue::Struct(value_properties)),
        }
    }

    #[test]
    fn base_guild_and_container_resolves_both_ids() {
        let entry = base_camp_entry(BASE_ID, GUILD_ID, CONTAINER_ID);

        let (guild_id, container_id) = base_guild_and_container(&entry).unwrap();

        assert_eq!(GUILD_ID, guild_id.to_string());
        assert_eq!(CONTAINER_ID, container_id.to_string());
    }

    #[test]
    fn base_guild_and_container_returns_none_when_raw_data_is_the_wrong_variant() {
        let mut value_properties = Properties::default();
        value_properties.insert("RawData", Property::Bool(true));
        let entry = MapEntry {
            key: guid_property(BASE_ID),
            value: Property::Struct(StructValue::Struct(value_properties)),
        };

        assert!(base_guild_and_container(&entry).is_none());
    }

    #[test]
    fn base_guild_and_container_returns_none_when_worker_director_blob_is_wrong_length() {
        let camp = PalBaseCamp {
            id: fguid(BASE_ID),
            name: String::new(),
            state: 0,
            transform: zero_transform(),
            area_range: 0.0,
            group_id_belong_to: fguid(GUILD_ID),
            fast_travel_local_transform: zero_transform(),
            owner_map_object_instance_id: crate::ue::FGuid::nil(),
            trailing_bytes: [0; 4],
        };
        let mut worker_properties = Properties::default();
        worker_properties.insert(
            "RawData",
            Property::Array(ValueVec::Byte(ByteArray::Byte(vec![0u8; 10]))),
        );
        let mut value_properties = Properties::default();
        value_properties.insert(
            "RawData",
            Property::Struct(StructValue::Game(crate::ue::PalStruct::BaseCamp(Box::new(camp)))),
        );
        value_properties.insert(
            "WorkerDirector",
            Property::Struct(StructValue::Struct(worker_properties)),
        );
        let entry = MapEntry {
            key: guid_property(BASE_ID),
            value: Property::Struct(StructValue::Struct(value_properties)),
        };

        assert!(base_guild_and_container(&entry).is_none());
    }

    // ---- find_player_guild_id ----

    use crate::session::{SaveKind, SaveSession};
    use crate::ue::games::palworld::PalGroupData;
    use crate::ue::{Header, MapEntry as UMapEntry, PackageVersion, PropertySchemas, Root, Save};

    fn minimal_save(properties: Properties) -> Save {
        Save {
            header: Header {
                magic: 0,
                save_game_version: 0,
                package_version: PackageVersion { ue4: 0, ue5: None },
                engine_version_major: 0,
                engine_version_minor: 0,
                engine_version_patch: 0,
                engine_version_build: 0,
                engine_version: String::new(),
                custom_version: None,
            },
            schemas: PropertySchemas::default(),
            root: Root {
                save_game_type: String::new(),
                properties,
            },
            extra: Vec::new(),
        }
    }

    fn guild_group_entry(
        guild_id: &str,
        guild: crate::ue::games::palworld::PalGuildGroup,
    ) -> UMapEntry {
        let mut value_properties = Properties::default();
        value_properties.insert(
            "GroupType",
            Property::Enum("EPalGroupType::Guild".to_string()),
        );
        let group_data = PalGroupData {
            group_id: fguid(guild_id),
            group_name: String::new(),
            individual_character_handle_ids: vec![],
            data: crate::ue::games::palworld::PalGroupVariant::Guild(guild),
        };
        value_properties.insert(
            "RawData",
            Property::Struct(StructValue::Game(crate::ue::PalStruct::GroupData(group_data))),
        );
        UMapEntry {
            key: guid_property(guild_id),
            value: Property::Struct(StructValue::Struct(value_properties)),
        }
    }

    fn session_with_group_map(entries: Vec<UMapEntry>) -> SaveSession {
        let mut world_save_data = Properties::default();
        world_save_data.insert("GroupSaveDataMap", Property::Map(entries));
        let mut root_properties = Properties::default();
        root_properties.insert(
            "worldSaveData",
            Property::Struct(StructValue::Struct(world_save_data)),
        );
        SaveSession::new_for_tests(SaveKind::InMemory, minimal_save(root_properties))
    }

    const PLAYER_ID: &str = "66666666-6666-6666-6666-666666666666";

    #[test]
    fn find_player_guild_id_locates_the_guild_owning_the_player() {
        let tail = guild_tail::pre_update_guild(
            3,
            "The Guild",
            "77777777-7777-7777-7777-777777777777".parse().unwrap(),
            &[(PLAYER_ID.parse().unwrap(), 0, "Tester")],
        );
        let mut session = session_with_group_map(vec![guild_group_entry(GUILD_ID, tail)]);

        let guild_id = find_player_guild_id(&mut session, PLAYER_ID.parse().unwrap()).unwrap();

        assert_eq!(guild_id, Some(GUILD_ID.parse().unwrap()));
        // A second lookup, now against the warm cache, must agree.
        let guild_id_again =
            find_player_guild_id(&mut session, PLAYER_ID.parse().unwrap()).unwrap();
        assert_eq!(guild_id_again, Some(GUILD_ID.parse().unwrap()));
    }

    #[test]
    fn find_player_guild_id_returns_none_for_a_player_in_no_guild() {
        let tail = guild_tail::pre_update_guild(
            1,
            "Other Guild",
            "77777777-7777-7777-7777-777777777777".parse().unwrap(),
            &[(
                "88888888-8888-8888-8888-888888888888".parse().unwrap(),
                0,
                "Someone Else",
            )],
        );
        let mut session = session_with_group_map(vec![guild_group_entry(GUILD_ID, tail)]);

        let guild_id = find_player_guild_id(&mut session, PLAYER_ID.parse().unwrap()).unwrap();

        assert_eq!(guild_id, None);
    }

    /// A `GroupSaveDataMap` entry whose `GroupType` isn't `Guild` (an alliance,
    /// say) must never be scanned for a player match.
    #[test]
    fn find_player_guild_id_ignores_non_guild_groups() {
        let mut value_properties = Properties::default();
        value_properties.insert(
            "GroupType",
            Property::Enum("EPalGroupType::Alliance".to_string()),
        );
        let group_data = PalGroupData {
            group_id: fguid(GUILD_ID),
            group_name: String::new(),
            individual_character_handle_ids: vec![],
            data: crate::ue::games::palworld::PalGroupVariant::Unknown {
                remaining_data: vec![],
            },
        };
        value_properties.insert(
            "RawData",
            Property::Struct(StructValue::Game(crate::ue::PalStruct::GroupData(group_data))),
        );
        let entry = UMapEntry {
            key: guid_property(GUILD_ID),
            value: Property::Struct(StructValue::Struct(value_properties)),
        };
        let mut session = session_with_group_map(vec![entry]);

        let guild_id = find_player_guild_id(&mut session, PLAYER_ID.parse().unwrap()).unwrap();

        assert_eq!(guild_id, None);
    }

    // ---- base_container_membership ----

    fn slot_save_parameter(slot_key: &str, container_id: uuid::Uuid) -> Properties {
        let mut container_struct = Properties::default();
        container_struct.insert("ID", crate::props::guid_property(container_id));
        let mut slot_struct = Properties::default();
        slot_struct.insert(
            "ContainerId",
            Property::Struct(StructValue::Struct(container_struct)),
        );
        slot_struct.insert("SlotIndex", crate::props::int_property(0));
        let mut save_parameter = Properties::default();
        save_parameter.insert(slot_key, Property::Struct(StructValue::Struct(slot_struct)));
        save_parameter
    }

    /// `"SlotId"` is real save data's actual spelling (11/11 world1 pals).
    #[test]
    fn base_container_membership_resolves_the_real_slot_id_spelling() {
        let container_id = uuid::Uuid::parse_str(CONTAINER_ID).unwrap();
        let save_parameter = slot_save_parameter("SlotId", container_id);

        assert_eq!(
            base_container_membership(&save_parameter),
            Some(container_id)
        );
    }

    /// The uppercase spelling `read_save_parameter_dto` accepts must resolve to
    /// `None` here -- the two lookups genuinely differ.
    #[test]
    fn base_container_membership_does_not_fall_back_to_slot_id_uppercase() {
        let container_id = uuid::Uuid::parse_str(CONTAINER_ID).unwrap();
        let save_parameter = slot_save_parameter("SlotID", container_id);

        assert_eq!(
            base_container_membership(&save_parameter),
            None,
            "base-container membership has no \"SlotID\" fallback"
        );

        // Contrast: `read_save_parameter_dto` DOES resolve the uppercase
        // spelling.
        let mut with_character_id = save_parameter;
        with_character_id.insert("CharacterID", crate::props::name_property("SheepBall"));
        let json_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/json");
        let game_data = crate::gamedata::GameData::load(&json_dir).expect("data dir");
        let dto = super::pal::read_save_parameter_dto(
            &with_character_id,
            uuid::Uuid::nil(),
            false,
            &game_data,
        );
        assert_eq!(
            dto.storage_id, container_id,
            "read_save_parameter_dto checks \"SlotID\" first"
        );
    }

    /// No slot property at all (neither spelling): a clean `None`, not a panic.
    #[test]
    fn base_container_membership_returns_none_when_no_slot_property_present() {
        let save_parameter = Properties::default();
        assert!(base_container_membership(&save_parameter).is_none());
    }

    // ---- module_target_container_id ----

    #[test]
    fn module_target_container_id_resolves_the_item_container_variant() {
        use crate::ue::games::palworld::{PalMapConcreteModelModule, PalMapConcreteModelModuleData};

        let container_id = uuid::Uuid::parse_str(CONTAINER_ID).unwrap();
        let raw_data = Property::Struct(StructValue::Game(crate::ue::PalStruct::MapConcreteModelModule(
            PalMapConcreteModelModule {
                module_type: "EPalMapObjectConcreteModelModuleType::ItemContainer".to_string(),
                data: PalMapConcreteModelModuleData::ItemContainer {
                    target_container_id: fguid(CONTAINER_ID),
                    slot_attribute_indexes: vec![],
                    all_slot_attribute: vec![],
                    drop_item_at_disposed: 0,
                    usage_type: 0,
                    trailing_bytes: [0; 4],
                },
                custom_version_data: vec![],
            },
        )));

        assert_eq!(module_target_container_id(&raw_data), Some(container_id));
    }

    #[test]
    fn module_target_container_id_returns_none_for_a_non_item_container_module() {
        use crate::ue::games::palworld::{PalMapConcreteModelModule, PalMapConcreteModelModuleData};

        let raw_data = Property::Struct(StructValue::Game(crate::ue::PalStruct::MapConcreteModelModule(
            PalMapConcreteModelModule {
                module_type: "EPalMapObjectConcreteModelModuleType::Energy".to_string(),
                data: PalMapConcreteModelModuleData::Energy,
                custom_version_data: vec![],
            },
        )));

        assert!(module_target_container_id(&raw_data).is_none());
        assert!(module_target_container_id(&Property::Bool(true)).is_none());
    }

    // ---- should_delete_map_object ----

    fn zero_map_model(
        group_id_belong_to: &str,
        build_player_uid: &str,
    ) -> crate::ue::games::palworld::PalMapModel {
        crate::ue::games::palworld::PalMapModel {
            instance_id: fguid("00000000-0000-0000-0000-000000000000"),
            concrete_model_instance_id: fguid("00000000-0000-0000-0000-000000000000"),
            base_camp_id_belong_to: fguid("00000000-0000-0000-0000-000000000000"),
            group_id_belong_to: fguid(group_id_belong_to),
            hp: crate::ue::games::palworld::PalMapObjectHp { current: 0, max: 0 },
            initial_transform_cache: zero_transform(),
            repair_work_id: fguid("00000000-0000-0000-0000-000000000000"),
            owner_spawner_level_object_instance_id: fguid("00000000-0000-0000-0000-000000000000"),
            owner_instance_id: fguid("00000000-0000-0000-0000-000000000000"),
            build_player_uid: fguid(build_player_uid),
            interact_restrict_type: 0,
            deterioration_damage: 0.0,
            stage_instance_id_belong_to: crate::ue::games::palworld::PalStageInstanceId {
                id: fguid("00000000-0000-0000-0000-000000000000"),
                valid: 0,
            },
            unknown_bytes: vec![],
        }
    }

    /// A `MapObjectSaveData` element with a real typed `Model.RawData`, and
    /// an optional `ConcreteModel.RawData` for the ItemBooth/PasswordLock
    /// cases.
    fn map_object_with_model(
        group_id_belong_to: &str,
        build_player_uid: &str,
        concrete_raw_data: Option<Property>,
        module_map: Option<Vec<MapEntry>>,
    ) -> StructValue {
        let mut model_props = Properties::default();
        model_props.insert(
            "RawData",
            Property::Struct(StructValue::Game(crate::ue::PalStruct::MapModel(Box::new(zero_map_model(
                group_id_belong_to,
                build_player_uid,
            ))))),
        );
        let mut object_props = Properties::default();
        object_props.insert("Model", Property::Struct(StructValue::Struct(model_props)));
        if concrete_raw_data.is_some() || module_map.is_some() {
            let mut concrete_props = Properties::default();
            // Every real `MapObjectSaveData` element carries a
            // `ConcreteModel.RawData` whatever its object type, so callers that
            // only care about `ModuleMap` still get a `BaseModel` fallback here
            // -- an element with no `RawData` at all is not a shape any real
            // save produces.
            let raw_data = concrete_raw_data.unwrap_or_else(|| {
                Property::Struct(StructValue::Game(crate::ue::PalStruct::MapConcreteModel(Box::new(
                    crate::ue::games::palworld::PalMapConcreteModel {
                        instance_id: fguid(SDM_NIL),
                        model_instance_id: fguid(SDM_NIL),
                        concrete_model_type: "BaseModel".to_string(),
                        model_data: crate::ue::games::palworld::PalMapConcreteModelVariant::Unknown(
                            crate::ue::games::palworld::BaseModel {
                                trailing_bytes: vec![],
                            },
                        ),
                    },
                ))))
            });
            concrete_props.insert("RawData", raw_data);
            if let Some(modules) = module_map {
                concrete_props.insert("ModuleMap", Property::Map(modules));
            }
            object_props.insert(
                "ConcreteModel",
                Property::Struct(StructValue::Struct(concrete_props)),
            );
        }
        StructValue::Struct(object_props)
    }

    fn zero_item_and_num() -> crate::ue::games::palworld::PalItemAndNum {
        crate::ue::games::palworld::PalItemAndNum {
            item_id: crate::ue::games::palworld::PalItemId {
                static_id: String::new(),
                dynamic_id: crate::ue::games::palworld::PalDynamicId {
                    created_world_id: crate::ue::FGuid::nil(),
                    local_id_in_created_world: crate::ue::FGuid::nil(),
                },
            },
            num: 0,
        }
    }

    fn item_booth_concrete_model(private_lock_player_uid: &str, seller_uids: &[&str]) -> Property {
        use crate::ue::games::palworld::{
            PalMapConcreteModelVariant, PalMapObjectItemBoothModel, PalMapObjectItemBoothTradeInfo,
        };
        let trade_infos = seller_uids
            .iter()
            .map(|seller| PalMapObjectItemBoothTradeInfo {
                product: zero_item_and_num(),
                cost: zero_item_and_num(),
                seller_player_uid: fguid(seller),
            })
            .collect();
        Property::Struct(StructValue::Game(crate::ue::PalStruct::MapConcreteModel(Box::new(
            crate::ue::games::palworld::PalMapConcreteModel {
                instance_id: fguid("00000000-0000-0000-0000-000000000000"),
                model_instance_id: fguid("00000000-0000-0000-0000-000000000000"),
                concrete_model_type: "PalMapObjectItemBoothModel".to_string(),
                model_data: PalMapConcreteModelVariant::ItemBooth(PalMapObjectItemBoothModel {
                    leading_bytes: [0; 4],
                    private_lock_player_uid: fguid(private_lock_player_uid),
                    trade_infos,
                    trailing_bytes: [0; 20],
                }),
            },
        ))))
    }

    const SDM_GUILD: &str = "10101010-0000-0000-0000-000000000000";
    const SDM_OTHER_GUILD: &str = "20202020-0000-0000-0000-000000000000";
    const SDM_PLAYER: &str = "30303030-0000-0000-0000-000000000000";
    const SDM_OTHER_PLAYER: &str = "40404040-0000-0000-0000-000000000000";
    const SDM_NIL: &str = "00000000-0000-0000-0000-000000000000";

    #[test]
    fn should_delete_map_object_matches_on_group_id_belong_to() {
        let guild_id: uuid::Uuid = SDM_GUILD.parse().unwrap();
        let object = map_object_with_model(SDM_GUILD, SDM_NIL, None, None);
        assert!(should_delete_map_object(&object, Some(guild_id), &[]));
        // A different target guild must not match.
        let other: uuid::Uuid = SDM_OTHER_GUILD.parse().unwrap();
        assert!(!should_delete_map_object(&object, Some(other), &[]));
        // No guild_id argument at all (player-only delete) must not match on
        // group ownership, regardless of the object's own group.
        assert!(!should_delete_map_object(&object, None, &[]));
    }

    #[test]
    fn should_delete_map_object_matches_on_build_player_uid() {
        let player: uuid::Uuid = SDM_PLAYER.parse().unwrap();
        let object = map_object_with_model(SDM_NIL, SDM_PLAYER, None, None);
        assert!(should_delete_map_object(&object, None, &[player]));
        let other: uuid::Uuid = SDM_OTHER_PLAYER.parse().unwrap();
        assert!(!should_delete_map_object(&object, None, &[other]));
    }

    #[test]
    fn should_delete_map_object_matches_on_item_booth_private_lock_owner() {
        let player: uuid::Uuid = SDM_PLAYER.parse().unwrap();
        let concrete = item_booth_concrete_model(SDM_PLAYER, &[]);
        let object = map_object_with_model(SDM_NIL, SDM_NIL, Some(concrete), None);
        assert!(should_delete_map_object(&object, None, &[player]));
    }

    #[test]
    fn should_delete_map_object_matches_on_item_booth_trade_seller() {
        let player: uuid::Uuid = SDM_PLAYER.parse().unwrap();
        let concrete = item_booth_concrete_model(SDM_NIL, &[SDM_OTHER_PLAYER, SDM_PLAYER]);
        let object = map_object_with_model(SDM_NIL, SDM_NIL, Some(concrete), None);
        assert!(should_delete_map_object(&object, None, &[player]));
    }

    /// A map object with no group/builder/item-booth match, but whose
    /// `ConcreteModel.ModuleMap` carries a `PasswordLock` module recording the
    /// target player's uid, must NOT be deleted -- a lock record is access, not
    /// ownership. Pins that the omission is load-bearing.
    #[test]
    fn should_delete_map_object_never_matches_via_password_lock_module_dead_code() {
        use crate::ue::games::palworld::{
            PalMapConcreteModelModule, PalMapConcreteModelModuleData, PalPlayerLockInfo,
        };
        let player: uuid::Uuid = SDM_PLAYER.parse().unwrap();
        let password_lock_module = PalMapConcreteModelModule {
            module_type: "EPalMapObjectConcreteModelModuleType::PasswordLock".to_string(),
            data: PalMapConcreteModelModuleData::PasswordLock {
                lock_state: 0,
                password: String::new(),
                player_infos: vec![PalPlayerLockInfo {
                    player_uid: fguid(SDM_PLAYER),
                    try_failed_count: 0,
                    try_success_cache: 0,
                }],
                trailing_bytes: [0; 4],
            },
            custom_version_data: vec![],
        };
        let module_entry = MapEntry {
            key: Property::Enum("EPalMapObjectConcreteModelModuleType::PasswordLock".to_string()),
            value: Property::Struct(StructValue::Struct({
                let mut properties = Properties::default();
                properties.insert(
                    "RawData",
                    Property::Struct(StructValue::Game(crate::ue::PalStruct::MapConcreteModelModule(password_lock_module))),
                );
                properties
            })),
        };
        let object = map_object_with_model(SDM_NIL, SDM_NIL, None, Some(vec![module_entry]));

        assert!(
            !should_delete_map_object(&object, None, &[player]),
            "PasswordLock's player_infos must never be consulted"
        );
    }

    #[test]
    fn should_delete_map_object_returns_false_for_an_untyped_map_object() {
        assert!(!should_delete_map_object(
            &StructValue::Guid(crate::ue::FGuid::nil()),
            None,
            &[]
        ));
        let empty = StructValue::Struct(Properties::default());
        assert!(!should_delete_map_object(&empty, None, &[]));
    }
}
