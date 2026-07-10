//! Guild-lookup helpers shared by pal-summary extraction (`domain::pal`,
//! Task 5) and guild detail loading (Task 8).

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
use uesave::{Properties, Property, PropertyKey, StructValue};

use super::{containers, guild_tail, pal, world};

/// From a `BaseCampSaveData` entry: `(group_id_belong_to, WorkerDirector
/// container_id)`. Python paths: `value.RawData.value.group_id_belong_to`
/// and `value.WorkerDirector.value.RawData.value.container_id`
/// (`game/mixins/loading.py`'s `_load_base_camps`,
/// `game/mixins/summaries.py`'s `_build_base_container_map`).
///
/// Deviation from the brief: the brief's version of this function matched on
/// `Property::Struct(StructValue::PalWorkerDirector(director))` and read
/// `director.container_id`. Neither that variant nor that struct exists in
/// `uesave-rs`. The API-shape checkpoint the brief called out was
/// necessary but insufficient -- the real gap is one level up: `../uesave-
/// rs/uesave/src/games/palworld/mod.rs` registers
/// `worldSaveData.BaseCampSaveData.WorkerDirector.RawData` in its
/// `struct_hints` list as a generic `StructType::Struct(None)`, and
/// `is_pal_struct_type` (same file) does not recognize `Struct(None)` as
/// Palworld-embedded data -- so `process_property_for_read` never attempts
/// to decode that byte array at all. The property survives parsing as a
/// plain, undecoded `Property::Array(ValueVec::Byte(ByteArray::Byte(bytes)))`,
/// not any `StructValue` variant, typed or otherwise. Phase 1 already solved
/// exactly this for `domain::summaries::guild_worker_container_ids`:
/// `palbin::worker_director_container_id` is a bounds-checked, already-
/// tested parser for this exact fixed 118-byte layout
/// (`palworld_save_tools/rawdata/worker_director.py`'s `decode_bytes`) --
/// this function reuses it rather than reinventing a byte parser or
/// depending on a struct that doesn't exist.
pub fn base_guild_and_container(entry: &uesave::MapEntry) -> Option<(uuid::Uuid, uuid::Uuid)> {
    let value_properties = props::struct_props(&entry.value)?;
    let raw_data = props::get(value_properties, &["RawData"])?;
    let uesave::Property::Struct(uesave::StructValue::PalBaseCamp(base_camp)) = raw_data else {
        return None;
    };
    let guild_id = props::guid_to_uuid(&base_camp.group_id_belong_to);

    let worker_director_blob = props::get(value_properties, &["WorkerDirector", "RawData"])
        .and_then(props::as_byte_array)?;
    let container_id = crate::palbin::worker_director_container_id(worker_director_blob).ok()?;

    Some((guild_id, container_id))
}

/// `_find_player_guild_id` / the player-guild lookup (`game/mixins/loading.py`).
/// Python branches on whether `self._player_guild_map_cache` happens to
/// already be populated (a fast cached path that yields a single result) vs.
/// a full fallback scan of every `EPalGroupType::Guild` group's `players`
/// list -- but both branches converge on the exact same answer (the guild
/// whose player list contains `player_id`), since the cache itself is only
/// ever built BY that same fallback scan (`_build_player_guild_index`). This
/// function reproduces that converged answer directly: build the full
/// `player uid -> guild id` map once (caching it in `session.caches.
/// player_guild_map`, mirroring the Python cache's role), then look up
/// `player_id` in it. A guild-type group whose tail fails to parse
/// contributes no entries rather than aborting the whole scan, matching this
/// port's "skip malformed, don't panic" policy for untrusted save data.
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
            let Ok(tail) = super::guild_tail::GuildTail::parse(&group_data.remaining_data) else {
                continue;
            };
            for player in &tail.players {
                player_guild_map.insert(player.player_uid, guild_id);
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

// ============================================================================
// Guild details, lab research, guild updates (Task 8) -- port of
// `_load_guild_by_id`/`_load_bases_for_guild` (`game/mixins/loading.py`),
// `Guild`/`Base`'s dumps (`game/guild.py`, `game/base.py`), and
// `Guild.update_lab_research` (`guild.py:205-219`). `Guild.update_from`
// (`guild.py:221-241`, wired up here as `apply_guild_dto`) is Task 10 scope
// -- it depends on `apply_base_dto`/`apply_item_container_dto`, which don't
// exist yet; adding a stub here would just have to be replaced, so it is
// left out of this commit entirely (see this task's report).
// ============================================================================

pub fn guild_entry_index(
    session: &SaveSession,
    guild_id: uuid::Uuid,
) -> Result<Option<usize>, CoreError> {
    Ok(world::group_map(&session.level)?
        .iter()
        .position(|entry| props::as_uuid(&entry.key) == Some(guild_id)))
}

/// Deviation from the brief: the brief's version did
/// `world::guild_extra_map(&session.level)?.iter().position(...)`, which does
/// not compile against the real signature (`GuildExtraSaveDataMap` is
/// optional, so `world::guild_extra_map` returns
/// `Result<Option<&Vec<MapEntry>>, CoreError>`, not `Result<&Vec<MapEntry>,
/// CoreError>` -- see `world.rs`'s own doc comment on why the three optional
/// maps get this treatment). `?` alone leaves an `Option<&Vec<MapEntry>>`,
/// which has no `.iter()` that walks its *entries*; `.and_then` is needed to
/// reach inside it first.
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

/// `Guild._load_lab_research`'s raw-data lookup (`guild.py:176-203`):
/// `guild_extra_data.value.Lab.value.RawData.value`, typed as `PalGuildLab`.
///
/// Deviation from the brief: the brief's version did
/// `world::guild_extra_map(&session.level).ok()?`, which leaves an
/// `Option<Option<&Vec<MapEntry>>>` collapsed by a single `?` into
/// `Option<&Vec<MapEntry>>` -- one level short of the `&Vec<MapEntry>`
/// `.get(extra_index)` needs (`Option` has no `.get()` method; that's
/// `Vec`'s). `.ok().flatten()?` collapses both the `Result` and the
/// `Option<Option<_>>` in one step.
fn guild_extra_lab(
    session: &SaveSession,
    extra_index: usize,
) -> Option<&uesave::games::palworld::PalGuildLab> {
    let entries = world::guild_extra_map(&session.level).ok().flatten()?;
    let value_props = props::struct_props(&entries.get(extra_index)?.value)?;
    let lab_props = props::struct_props(value_props.0.get(&PropertyKey::from("Lab"))?)?;
    match lab_props.0.get(&PropertyKey::from("RawData"))? {
        Property::Struct(StructValue::PalGuildLab(lab)) => Some(lab),
        _ => None,
    }
}

/// `Guild.container_id`'s raw-data lookup (`guild.py:91-105`):
/// `guild_extra_data.value.GuildItemStorage.value.RawData.value.container_id`.
/// Same `.ok().flatten()?` fix as `guild_extra_lab` above.
fn guild_chest_container_id(session: &SaveSession, extra_index: usize) -> Option<uuid::Uuid> {
    let entries = world::guild_extra_map(&session.level).ok().flatten()?;
    let value_props = props::struct_props(&entries.get(extra_index)?.value)?;
    let storage_props =
        props::struct_props(value_props.0.get(&PropertyKey::from("GuildItemStorage"))?)?;
    match storage_props.0.get(&PropertyKey::from("RawData"))? {
        Property::Struct(StructValue::PalGuildItemStorage(storage)) => {
            Some(props::guid_to_uuid(&storage.container_id))
        }
        _ => None,
    }
}

/// Base-container pal membership, matching `_load_pals_for_container`
/// (`loading.py:317-346`) exactly: `slot_id = save_parameter.get("SlotId")`
/// checks ONLY `"SlotId"` -- unlike `Pal.storage_slot`/`storage_id`'s getter
/// (and `read_save_parameter_dto`, and the brief's own reference code for
/// this function, which matched via `pal_dto.storage_id ==
/// worker_container_id`), there is no `"SlotID"` fallback here. Every one of
/// world1's 11 pals spells this property `"SlotId"` (0 use `"SlotID"`), so
/// the two approaches happen to agree on this port's real-save fixtures; see
/// this task's report for why the narrower, Python-literal check is used
/// anyway.
pub(crate) fn base_container_membership(save_parameter: &Properties) -> Option<uuid::Uuid> {
    let slot = pal::param(save_parameter, "SlotId").and_then(props::struct_props)?;
    slot.0
        .get(&PropertyKey::from("ContainerId"))
        .and_then(props::struct_props)
        .and_then(|container| container.0.get(&PropertyKey::from("ID")))
        .and_then(props::as_uuid)
}

/// `_get_map_object_index`/`_build_map_object_index` (`indexing.py:41-58`):
/// groups every `MapObjectSaveData` element by
/// `Model.RawData.value.base_camp_id_belong_to`, so `_load_bases_for_guild`
/// can look a base's map objects up in O(1) instead of re-scanning
/// `MapObjectSaveData` once per base.
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
        let Some(Property::Struct(StructValue::PalMapModel(model))) =
            model_props.0.get(&PropertyKey::from("RawData"))
        else {
            continue;
        };
        let base_id = props::guid_to_uuid(&model.base_camp_id_belong_to);
        index.entry(base_id).or_default().push(object_props);
    }
    index
}

/// Extracts `target_container_id` from an ItemContainer module's typed
/// `RawData` (`base.py:214-219`'s
/// `module["value"]["RawData"]["value"]["target_container_id"]`).
///
/// Deviation from the brief: the brief matched
/// `Property::Struct(StructValue::PalMapConcreteModelModule(module))` and
/// then matched enum variants directly on `module`, as if
/// `PalMapConcreteModelModule` were itself the enum. The real `uesave-rs`
/// shape (`map_concrete_model_module.rs`) is one level deeper:
/// `PalMapConcreteModelModule` is a plain struct (`module_type`, `data`,
/// `custom_version_data`), and the per-module-type enum
/// (`PalMapConcreteModelModuleData`, with the `ItemContainer {
/// target_container_id, .. }` variant this needs) lives at `module.data`.
fn module_target_container_id(raw_data: &Property) -> Option<uuid::Uuid> {
    let Property::Struct(StructValue::PalMapConcreteModelModule(module)) = raw_data else {
        return None;
    };
    match &module.data {
        uesave::games::palworld::PalMapConcreteModelModuleData::ItemContainer {
            target_container_id,
            ..
        } => Some(props::guid_to_uuid(target_container_id)),
        _ => None,
    }
}

/// `_load_guild_by_id` + `_load_bases_for_guild` + `Guild`'s dump
/// (`game/mixins/loading.py:203-346`, `game/guild.py`, `game/base.py`).
/// `None` when the guild id doesn't resolve at all, or when its
/// `GuildExtraSaveDataMap` entry is missing (`loading.py:222-224`: "Guild
/// extra save data not found for guild %s" -> the guild does not load).
///
/// Deviation from the brief: session mutation (`loaded_guilds`/
/// `guild_summaries[..].loaded`) is split out of the read-heavy DTO build,
/// mirroring Task 7's `get_player_details`/`build_player_dto` split -- the
/// brief's version interleaved a `&mut SaveSession` through the whole
/// function body, which either fights the borrow checker once bases/pals
/// start borrowing `session.level` while other code paths want to mutate
/// other `SaveSession` fields, or forces every helper to also take `&mut`
/// for no reason. `build_guild_dto` below takes `&SaveSession` and does
/// nothing but read; `get_guild_details` does the two field writes after it
/// returns, once no borrow of `session.level` is outstanding.
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

    // loading.py:232-238: cached membership, then (after bases finish
    // loading in Python; here, after the whole DTO -- including bases --
    // has already been built) the summary's `loaded` flip. The two Python
    // statements straddle `_load_bases_for_guild`; folding both builds into
    // one pure function above and flipping both flags together afterward is
    // an unobservable reordering (nothing in this task reads either flag
    // mid-build).
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
        let tail = guild_tail::GuildTail::parse(&group_data.remaining_data)?;
        let players: Vec<uuid::Uuid> = tail.players.iter().map(|p| p.player_uid).collect();
        // guild.py:76-77 (`self.players[0] if self.players else None`). Note:
        // `Guild.players` itself raises `UnboundLocalError` in real Python
        // when the raw player list is empty -- a genuine Python bug found
        // while porting this, NOT on the PARITY-BUG list, and NOT
        // reproduced here (see this task's report). An empty `players` here
        // is a normal, non-panicking "no admin" case.
        let admin = players.first().copied();
        (
            tail.guild_name.clone(),
            tail.base_camp_level,
            players,
            admin,
        )
    };

    // lab research (guild.py:176-203, `lab_research`/`lab_research_data`).
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

    // guild chest (guild.py:91-105, 243-263).
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

    // bases (loading.py:244-315).
    let map_object_index = world::map_object_values(&session.level)?
        .map(|values| map_object_properties_by_base_id(values))
        .unwrap_or_default();
    let empty_map_objects: Vec<&Properties> = Vec::new();
    let character_container_index = world::build_character_container_index(&session.level);
    let base_camp_entries: &[uesave::MapEntry] = world::base_camp_map(&session.level)?
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
            continue; // loading.py:291-293
        };
        let Some(container_view) =
            containers::read_character_container(&session.level, *container_entry_index)
        else {
            continue;
        };

        // base pals (loading.py:317-346).
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

        // base name / area_range / location (base.py's computed fields).
        let base_entry = base_camp_entries
            .iter()
            .find(|entry| props::as_uuid(&entry.key) == Some(base_id));
        let (base_name, area_range, location) = base_entry
            .and_then(|entry| props::struct_props(&entry.value))
            .and_then(|value_props| value_props.0.get(&PropertyKey::from("RawData")))
            .map(|raw_data| match raw_data {
                Property::Struct(StructValue::PalBaseCamp(base_camp)) => (
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

        // storage containers: base map objects with an ItemContainer module
        // (indexing.py:41-58 + base.py:196-228).
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

/// `Guild.update_lab_research` (`guild.py:205-219`): full replacement of
/// `research_info`; `current_research_id` and `trailing_bytes` untouched.
/// Never touches `GroupSaveDataMap`'s raw guild-tail bytes -- this writes
/// only into `GuildExtraSaveDataMap`'s `Lab.RawData` (a separate typed
/// struct); see this task's report for the byte-identical proof that an
/// untouched guild's raw tail survives a `get_guild_details`/
/// `update_lab_research` call unchanged.
///
/// `Err(GuildNotFound)` only when the guild id itself was never loaded (this
/// port's stand-in for Python's `self._guilds[guild_id]` lookup, which would
/// raise `KeyError` before `update_lab_research` is ever called on an
/// unloaded guild -- Task 13's WS handler owns that guard in the real
/// pipeline). Once the guild id itself resolves, every other failure --
/// missing/malformed `GuildExtraSaveDataMap` entry, missing `Lab`, an
/// untyped `Lab.RawData` -- is a silent no-op (`Ok(())`), matching Python's
/// own `if not self._lab_raw_data: logger.error(...); return` (a log
/// message, not an exception).
///
/// Deviation from the brief: the brief's version (a) indexed straight into
/// `world::guild_extra_map_mut(&mut session.level)?` as if it returned
/// `&mut Vec<MapEntry>` (it returns `Result<Option<&mut Vec<MapEntry>>,
/// CoreError>` -- optional maps again, see `guild_extra_entry_index`'s doc
/// comment above), and (b) assigned `info.work_amount` (an `f64` on
/// `GuildLabResearchInfo`) directly into `PalLabResearchInfo.work_amount`
/// (an `f32` in the real `uesave-rs` struct) with no cast, which does not
/// type-check. Both are fixed below; `as f32` matches Python's own
/// unavoidable narrowing (the persisted bytes are IEEE-754 single-precision,
/// `PalLabResearchInfo::write`'s `ar.write_f32::<LE>`; Python's `float` is a
/// double up until the moment `struct.pack`s it into the save).
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
    let Some(Property::Struct(StructValue::PalGuildLab(lab))) =
        lab_props.0.get_mut(&PropertyKey::from("RawData"))
    else {
        return Ok(());
    };
    lab.research_info = research_updates
        .iter()
        .map(|info| uesave::games::palworld::PalLabResearchInfo {
            research_id: info.research_id.clone(),
            work_amount: info.work_amount as f32,
        })
        .collect();
    Ok(())
}

// ============================================================================
// Guild update (Task 10) -- port of `GuildOpsMixin.update_guilds`
// (`guild_ops.py`) and `Guild.update_from` (`guild.py:221-241`).
// ============================================================================

/// The set of item-container ids that genuinely belong to `base_id`'s own
/// storage (`Base._load_storage_containers`'s own enumeration,
/// `base.py:196-228`): every `MapObjectSaveData` element for this base whose
/// `ConcreteModel.ModuleMap` carries an `ItemContainer` module, resolved to
/// its `target_container_id`. Used by `containers::apply_base_dto` to reject
/// a `storage_containers` map key that doesn't actually belong to this base
/// -- see that function's own doc comment for why this membership check is
/// load-bearing, not decorative. A `base_id` this port can't resolve at all
/// yields an empty set (matches the caller's own "skip, never panic" policy
/// for a not-found base).
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

/// `Guild.container_id`'s resolution (`guild.py:91-105`), reused by
/// `apply_guild_dto` so a guild-chest edit routes through the SESSION's own
/// resolved container id, never `guildDTO.guild_chest.id` (a client-supplied
/// value) -- same "never trust the payload's own id for routing" rule
/// `player::apply_player_dto`'s doc comment establishes for a player's five
/// containers.
pub(crate) fn guild_chest_id(session: &SaveSession, guild_id: uuid::Uuid) -> Option<uuid::Uuid> {
    let extra_index = guild_extra_entry_index(session, guild_id).ok().flatten()?;
    guild_chest_container_id(session, extra_index)
}

/// Port of `GuildOpsMixin.update_guilds` (`guild_ops.py`): progress message
/// names the guild's UUID (`guild_ops.py:113-114`), not its name.
///
/// `_game_data` is currently unused by this call chain (`apply_guild_dto`'s
/// own internals need no `GameData` -- see `apply_item_container_dto`'s doc
/// comment on why that's true all the way down). Kept, not removed: this
/// port's whole `update_*` family (`pal::update_pals`/`update_dps_pals`,
/// this function, `player::update_players`) shares one uniform
/// `(session, game_data, modified, progress)` public signature, matching
/// this task's own established convention -- `update_pals` genuinely needs
/// it. Changing only this one public entry point's shape for an internal
/// implementation detail would be a bigger, less obviously safe edit than
/// this task's review asked for.
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

/// Port of `Guild.update_from` (`guild.py:221-241`). No `game_data:
/// &GameData` parameter -- see `apply_item_container_dto`'s doc comment.
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
        let mut tail = guild_tail::GuildTail::parse(&group_data.remaining_data)?;
        // `if guildDTO.name:` -- Python truthiness (None AND "" both skip).
        if let Some(name) = &dto.name {
            if !name.is_empty() {
                tail.guild_name = name.clone();
            }
        }
        // `if guildDTO.base_camp_level:` -- 0 is falsy, mirror that.
        if let Some(level) = dto.base_camp_level {
            if level != 0 {
                tail.base_camp_level = level;
            }
        }
        group_data.remaining_data = tail.to_bytes();
    }
    if let Some(bases) = &dto.bases {
        for (base_id, base_dto) in bases.iter() {
            super::containers::apply_base_dto(session, *base_id, base_dto)?;
        }
    }
    // `if guildDTO.guild_chest and self.guild_chest is not None:` -- both
    // sides collapse to "the guild's real chest resolves", checked via
    // `guild_chest_id` rather than trusting `dto.guild_chest.id`.
    if dto.guild_chest.is_some() {
        if let Some(chest_id) = guild_chest_id(session, guild_id) {
            if let Some(chest_dto) = &dto.guild_chest {
                super::containers::apply_item_container_dto(session, chest_id, chest_dto, None)?;
            }
        }
    }
    Ok(())
}

// ============================================================================
// delete_player / delete_guild (Task 11) -- port of
// `GuildOpsMixin.delete_guild_and_players`/`_should_delete_map_object`
// (`guild_ops.py`) and `Guild.delete_player` (`guild.py:159-170`, wired up
// in `domain::player::delete_player`).
// ============================================================================

/// `_should_delete_map_object` (`guild_ops.py`): delete a `MapObjectSaveData`
/// element when `Model.RawData.group_id_belong_to == guild_id`, OR
/// `Model.RawData.build_player_uid` is one of `player_ids`, OR (ItemBooth
/// concrete models only) `private_lock_player_uid` or any
/// `trade_infos[].seller_player_uid` is one of `player_ids`.
///
/// **The `ConcreteModel`'s `ModuleMap`/`PasswordLock` branch is deliberately
/// NOT implemented here -- a newly-found Python bug, not on the known list,
/// reproduced by omission (required for byte parity: whether a given map
/// object survives a delete is directly observable in the written save).**
/// Python's own check reads `concrete_model_raw_data.get("ModuleMap", {})
/// .get("value", [])`, where `concrete_model_raw_data =
/// map_object["ConcreteModel"]["value"]["RawData"]["value"]` -- i.e. it
/// looks for a `"ModuleMap"` key INSIDE the decoded, per-object-type
/// `RawData` dict. But `palworld_save_tools/rawdata/map_object.py::decode`
/// populates `ModuleMap` as a SIBLING of `RawData`
/// (`map_object["ConcreteModel"]["value"]["ModuleMap"]["value"]`, walked in
/// its own separate loop right after `RawData` is decoded), never nested
/// inside it -- verified against every per-object-type decoder in
/// `rawdata/map_concrete_model.py` (`PalMapObjectItemBoothModel` et al.),
/// none of which ever produce a `"ModuleMap"` key of their own. Real
/// Python's `.get("ModuleMap", {})` therefore ALWAYS returns the
/// empty-dict default, and the PasswordLock loop body never executes for
/// any real save -- dead code. Reproduced here by never implementing that
/// branch at all (an unconditional non-match), rather than "fixing" it into
/// a functional check, which is what the brief's own reference code did:
/// it read `ModuleMap` from the CORRECT sibling location under this port's
/// own `uesave-rs`-typed shape (`concrete_props.0.get("ModuleMap")`,
/// alongside `RawData`, not nested inside it) -- accidentally MORE correct
/// than Python, and therefore byte-divergent from what real Python actually
/// deletes. See this task's report.
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
    let Some(Property::Struct(StructValue::PalMapModel(model))) =
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

    // ItemBooth edge cases (guild_ops.py:141-162): private lock owner, or
    // any trade-info seller.
    let Some(concrete_props) = object_props
        .0
        .get(&PropertyKey::from("ConcreteModel"))
        .and_then(props::struct_props)
    else {
        return false;
    };
    let Some(Property::Struct(StructValue::PalMapConcreteModel(concrete))) =
        concrete_props.0.get(&PropertyKey::from("RawData"))
    else {
        return false;
    };
    if let uesave::games::palworld::PalMapConcreteModelVariant::ItemBooth(booth) =
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

/// Port of `delete_guild_and_players` (`guild_ops.py`). `Err` when
/// `guild_id` was never loaded (matching Python's `guild =
/// self._guilds.get(guild_id); if not guild: raise ValueError(...)`, before
/// any mutation) -- checked directly via `session.loaded_guilds`, NOT by
/// calling `get_guild_details` first, which would lazily LOAD an unloaded
/// guild as a side effect that real Python's plain dict `.get` here never
/// has. Once confirmed loaded, `get_guild_details` is safe to call for its
/// `GuildDto` (its `loaded_guilds.insert`/`guild_summaries[..].loaded = true`
/// side effects are no-ops on an already-loaded guild).
///
/// **Never deletes the guild's own item-storage container (the "chest") --
/// a newly-found Python bug, not on the known list, reproduced for byte
/// parity, and a deliberate divergence from the brief.** The brief's own
/// reference code added an explicit extra `delete_item_containers(session,
/// &[chest_id])` call, justified as approximating a described Python
/// fallback. That justification does not hold up against
/// `_delete_item_containers`'s actual fallback condition (`guild_ops.py`):
/// the fallback only fires PER CONTAINER ID ALREADY IN THE CALLER'S OWN
/// LIST, when that specific id isn't found by direct lookup -- and
/// `container_ids_to_delete` here is built ONLY from player containers and
/// base storage containers (`base.storage_containers.keys()`); the guild's
/// own `container_id` (the chest) is NEVER added to that list at all, so
/// neither the primary lookup nor the fallback ever considers it. Real
/// Python therefore leaves the guild chest as a permanently orphaned
/// `ItemContainerSaveData` entry after `delete_guild_and_players` runs --
/// not fixed here. See this task's report.
///
/// **The second, buggy `_delete_item_containers(player_id,
/// container_ids_to_delete)` call in `guild_ops.py` is a THIRD newly-found
/// Python bug, NOT reproduced.** After the guild's players/bases are fully
/// processed, real Python calls `_delete_item_containers` TWICE: once with
/// `target_id=guild_id` (the real, correct pass), then AGAIN with
/// `target_id=player_id` -- a name that, at that point in the function, is
/// whatever value survives from the earlier `for player_id in
/// guild.players:` loop (the LAST guild member processed; an
/// `UnboundLocalError` if `guild.players` was ever empty). By the second
/// call, every id in `container_ids_to_delete` has ALREADY been removed by
/// the first call, so every one of them takes the FALLBACK branch this
/// time: a linear scan for any container whose `BelongInfo.GroupId` equals
/// that stale, unrelated player id, deleted on a match. This does nothing
/// to help delete the guild's own correct containers (already done by the
/// first call) and risks an unpredictable extra deletion this port declines
/// to gamble on reproducing; not implemented. See this task's report.
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

    // Map objects owned by the guild or its players (guild_ops.py:50-55).
    if let Some(values) = world::map_object_values_mut(&mut session.level)? {
        values.retain(|map_object| {
            !should_delete_map_object(map_object, Some(guild_id), &guild_players)
        });
    }

    let mut item_container_ids: Vec<uuid::Uuid> = Vec::new();
    let mut character_container_ids: Vec<uuid::Uuid> = Vec::new();

    // Every loaded guild player (guild_ops.py:57-65): an unloaded member is
    // skipped entirely, matching Python's `if player_id not in self._players:
    // continue`.
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

    // GuildExtraSaveDataMap entry (guild_ops.py:67-72).
    if let Some(entries) = world::guild_extra_map_mut(&mut session.level)? {
        entries.retain(|entry| props::as_uuid(&entry.key) != Some(guild_id));
    }

    // Every base (guild_ops.py:74-87).
    if let Some(bases) = &details.bases {
        for (base_id, base) in bases.iter() {
            progress(&format!("Deleting base {base_id}"));
            item_container_ids.extend(base.storage_containers.iter().map(|(id, _)| *id));
            if let Some(worker_container) = base.container_id {
                character_container_ids.push(worker_container);
            }
            let base_pal_ids: Vec<uuid::Uuid> = base.pals.iter().map(|(id, _)| *id).collect();
            // `Guild.delete_base_pal` (guild.py:143-146), via
            // `PalOpsMixin.delete_guild_pals` -- unlike the brief's own
            // reference code (a raw `delete_pal_entry` per id, which skips
            // the guild-handle cleanup `delete_base_pal` actually does),
            // this reuses the existing, already-reviewed Task 9 op so a
            // base pal's `individual_character_handle_ids` entry is removed
            // too, matching Python's real behavior for base pals exactly
            // (contrast with player pals -- see
            // `delete_player_and_pals_for_guild`'s own doc comment on why
            // THOSE are deliberately left dangling instead).
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

    // The group entry itself (guild_ops.py:98-104).
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
    use uesave::games::palworld::{PalBaseCamp, PalTransform};
    use uesave::{
        ByteArray, Double, MapEntry, Properties, Property, Quat, StructValue, ValueVec, Vector,
    };

    const GUILD_ID: &str = "33333333-3333-3333-3333-333333333333";
    const BASE_ID: &str = "44444444-4444-4444-4444-444444444444";
    const CONTAINER_ID: &str = "55555555-5555-5555-5555-555555555555";

    fn fguid(text: &str) -> uesave::FGuid {
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
            owner_map_object_instance_id: uesave::FGuid::nil(),
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
            Property::Struct(StructValue::PalBaseCamp(Box::new(camp))),
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
            owner_map_object_instance_id: uesave::FGuid::nil(),
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
            Property::Struct(StructValue::PalBaseCamp(Box::new(camp))),
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
    use uesave::games::palworld::PalGroupData;
    use uesave::{Header, MapEntry as UMapEntry, PackageVersion, PropertySchemas, Root, Save};

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

    fn guild_group_entry(guild_id: &str, tail: Vec<u8>) -> UMapEntry {
        let mut value_properties = Properties::default();
        value_properties.insert(
            "GroupType",
            Property::Enum("EPalGroupType::Guild".to_string()),
        );
        let group_data = PalGroupData {
            group_id: fguid(guild_id),
            group_name: String::new(),
            individual_character_handle_ids: vec![],
            remaining_data: tail,
        };
        value_properties.insert(
            "RawData",
            Property::Struct(StructValue::PalGroupData(group_data)),
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
        let tail = crate::palbin::test_bytes::guild_tail(
            3,
            "The Guild",
            "77777777-7777-7777-7777-777777777777",
            &[(PLAYER_ID, 0, "Tester")],
        );
        let mut session = session_with_group_map(vec![guild_group_entry(GUILD_ID, tail)]);

        let guild_id = find_player_guild_id(&mut session, PLAYER_ID.parse().unwrap()).unwrap();

        assert_eq!(guild_id, Some(GUILD_ID.parse().unwrap()));
        // The cache is now warm; a second lookup must return the same answer
        // without needing to re-scan (this only proves the answer stays
        // correct across calls -- the "no re-scan" half is a performance
        // claim this test does not attempt to measure).
        let guild_id_again =
            find_player_guild_id(&mut session, PLAYER_ID.parse().unwrap()).unwrap();
        assert_eq!(guild_id_again, Some(GUILD_ID.parse().unwrap()));
    }

    #[test]
    fn find_player_guild_id_returns_none_for_a_player_in_no_guild() {
        let tail = crate::palbin::test_bytes::guild_tail(
            1,
            "Other Guild",
            "77777777-7777-7777-7777-777777777777",
            &[("88888888-8888-8888-8888-888888888888", 0, "Someone Else")],
        );
        let mut session = session_with_group_map(vec![guild_group_entry(GUILD_ID, tail)]);

        let guild_id = find_player_guild_id(&mut session, PLAYER_ID.parse().unwrap()).unwrap();

        assert_eq!(guild_id, None);
    }

    /// A `GroupSaveDataMap` entry whose `GroupType` isn't `Guild` (an alliance,
    /// say) must never be scanned for a player match -- matching Python's own
    /// `if GroupType.from_value(group_type) != GroupType.GUILD: continue`.
    #[test]
    fn find_player_guild_id_ignores_non_guild_groups() {
        let mut value_properties = Properties::default();
        value_properties.insert(
            "GroupType",
            Property::Enum("EPalGroupType::Alliance".to_string()),
        );
        let tail = crate::palbin::test_bytes::guild_tail(
            1,
            "Alliance",
            "77777777-7777-7777-7777-777777777777",
            &[(PLAYER_ID, 0, "Tester")],
        );
        let group_data = PalGroupData {
            group_id: fguid(GUILD_ID),
            group_name: String::new(),
            individual_character_handle_ids: vec![],
            remaining_data: tail,
        };
        value_properties.insert(
            "RawData",
            Property::Struct(StructValue::PalGroupData(group_data)),
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

    /// `_load_pals_for_container` (`loading.py:317-346`) reads ONLY
    /// `"SlotId"` -- real save data's actual spelling (verified: 11/11
    /// world1 pals). Deliberate divergence from the brief, which matched
    /// base-container membership via `pal_dto.storage_id ==
    /// worker_container_id` (the DIFFERENT "SlotID"-first-then-"SlotId"-
    /// fallback rule `Pal.storage_id`'s getter uses) -- see this function's
    /// own doc comment and this task's report.
    #[test]
    fn base_container_membership_resolves_the_real_slot_id_spelling() {
        let container_id = uuid::Uuid::parse_str(CONTAINER_ID).unwrap();
        let save_parameter = slot_save_parameter("SlotId", container_id);

        assert_eq!(
            base_container_membership(&save_parameter),
            Some(container_id)
        );
    }

    /// The uppercase spelling `Pal.storage_id`'s getter checks FIRST must
    /// resolve to `None` here -- proving the two behaviors genuinely
    /// differ, not merely that one of them fails for an unrelated reason.
    #[test]
    fn base_container_membership_does_not_fall_back_to_slot_id_uppercase() {
        let container_id = uuid::Uuid::parse_str(CONTAINER_ID).unwrap();
        let save_parameter = slot_save_parameter("SlotID", container_id);

        assert_eq!(
            base_container_membership(&save_parameter),
            None,
            "loading.py's _load_pals_for_container has no \"SlotID\" fallback"
        );

        // Establish the contrast: `read_save_parameter_dto`'s
        // `Pal.storage_id`-equivalent getter DOES resolve this same
        // uppercase-spelled property.
        let mut with_character_id = save_parameter;
        with_character_id.insert("CharacterID", crate::props::name_property("SheepBall"));
        let json_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/json");
        let game_data = crate::gamedata::GameData::load(&json_dir).expect("data dir");
        let dto = super::pal::read_save_parameter_dto(
            &with_character_id,
            uuid::Uuid::nil(),
            false,
            &game_data,
        );
        assert_eq!(
            dto.storage_id, container_id,
            "Pal.storage_id's getter checks \"SlotID\" first"
        );
    }

    /// No slot property at all (neither spelling): a clean `None`, not a
    /// panic -- matches Python's `if not slot_id: ...; continue`.
    #[test]
    fn base_container_membership_returns_none_when_no_slot_property_present() {
        let save_parameter = Properties::default();
        assert!(base_container_membership(&save_parameter).is_none());
    }

    // ---- module_target_container_id ----

    #[test]
    fn module_target_container_id_resolves_the_item_container_variant() {
        use uesave::games::palworld::{PalMapConcreteModelModule, PalMapConcreteModelModuleData};

        let container_id = uuid::Uuid::parse_str(CONTAINER_ID).unwrap();
        let raw_data = Property::Struct(StructValue::PalMapConcreteModelModule(
            PalMapConcreteModelModule {
                module_type: "EPalMapObjectConcreteModelModuleType::ItemContainer".to_string(),
                data: PalMapConcreteModelModuleData::ItemContainer {
                    target_container_id: fguid(CONTAINER_ID),
                    slot_attribute_indexes: vec![],
                    all_slot_attribute: vec![],
                    drop_item_at_disposed: false,
                    usage_type: 0,
                    trailing_bytes: [0; 4],
                },
                custom_version_data: vec![],
            },
        ));

        assert_eq!(module_target_container_id(&raw_data), Some(container_id));
    }

    #[test]
    fn module_target_container_id_returns_none_for_a_non_item_container_module() {
        use uesave::games::palworld::{PalMapConcreteModelModule, PalMapConcreteModelModuleData};

        let raw_data = Property::Struct(StructValue::PalMapConcreteModelModule(
            PalMapConcreteModelModule {
                module_type: "EPalMapObjectConcreteModelModuleType::Energy".to_string(),
                data: PalMapConcreteModelModuleData::Energy,
                custom_version_data: vec![],
            },
        ));

        assert!(module_target_container_id(&raw_data).is_none());
        assert!(module_target_container_id(&Property::Bool(true)).is_none());
    }

    // ---- should_delete_map_object ----

    fn zero_map_model(
        group_id_belong_to: &str,
        build_player_uid: &str,
    ) -> uesave::games::palworld::PalMapModel {
        uesave::games::palworld::PalMapModel {
            instance_id: fguid("00000000-0000-0000-0000-000000000000"),
            concrete_model_instance_id: fguid("00000000-0000-0000-0000-000000000000"),
            base_camp_id_belong_to: fguid("00000000-0000-0000-0000-000000000000"),
            group_id_belong_to: fguid(group_id_belong_to),
            hp: uesave::games::palworld::PalMapObjectHp { current: 0, max: 0 },
            initial_transform_cache: zero_transform(),
            repair_work_id: fguid("00000000-0000-0000-0000-000000000000"),
            owner_spawner_level_object_instance_id: fguid("00000000-0000-0000-0000-000000000000"),
            owner_instance_id: fguid("00000000-0000-0000-0000-000000000000"),
            build_player_uid: fguid(build_player_uid),
            interact_restrict_type: 0,
            deterioration_damage: 0.0,
            stage_instance_id_belong_to: uesave::games::palworld::PalStageInstanceId {
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
            Property::Struct(StructValue::PalMapModel(Box::new(zero_map_model(
                group_id_belong_to,
                build_player_uid,
            )))),
        );
        let mut object_props = Properties::default();
        object_props.insert("Model", Property::Struct(StructValue::Struct(model_props)));
        if concrete_raw_data.is_some() || module_map.is_some() {
            let mut concrete_props = Properties::default();
            // Every real `MapObjectSaveData` element decodes `ConcreteModel.
            // RawData` unconditionally (`map_object.py::decode`), whatever
            // the object type -- an `Unknown`/`BaseModel` fallback when the
            // module_map-only caller doesn't care which concrete type this
            // is. Omitting `RawData` entirely (as an earlier revision of
            // this helper did) is not a shape any real save produces.
            let raw_data = concrete_raw_data.unwrap_or_else(|| {
                Property::Struct(StructValue::PalMapConcreteModel(Box::new(
                    uesave::games::palworld::PalMapConcreteModel {
                        instance_id: fguid(SDM_NIL),
                        model_instance_id: fguid(SDM_NIL),
                        concrete_model_type: "BaseModel".to_string(),
                        model_data: uesave::games::palworld::PalMapConcreteModelVariant::Unknown(
                            uesave::games::palworld::BaseModel {
                                trailing_bytes: vec![],
                            },
                        ),
                    },
                )))
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

    fn zero_item_and_num() -> uesave::games::palworld::PalItemAndNum {
        uesave::games::palworld::PalItemAndNum {
            item_id: uesave::games::palworld::PalItemId {
                static_id: String::new(),
                dynamic_id: uesave::games::palworld::PalDynamicId {
                    created_world_id: uesave::FGuid::nil(),
                    local_id_in_created_world: uesave::FGuid::nil(),
                },
            },
            num: 0,
        }
    }

    fn item_booth_concrete_model(private_lock_player_uid: &str, seller_uids: &[&str]) -> Property {
        use uesave::games::palworld::{
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
        Property::Struct(StructValue::PalMapConcreteModel(Box::new(
            uesave::games::palworld::PalMapConcreteModel {
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
        )))
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

    /// **The dead-code reproduction, pinned.** A map object with NO group/
    /// builder/item-booth match, but whose `ConcreteModel.ModuleMap` (the
    /// SIBLING location -- not nested inside `RawData`, matching the real
    /// property tree shape) carries a `PasswordLock` module recording the
    /// target player's uid, must NOT be deleted. If a "fixed" (functional)
    /// PasswordLock check were implemented instead of this reproduction,
    /// this assertion would flip to `true` and this test would fail --
    /// proving the omission is load-bearing, not a no-op no one would
    /// notice. See `should_delete_map_object`'s own doc comment for the
    /// Python-source citation this reproduces.
    #[test]
    fn should_delete_map_object_never_matches_via_password_lock_module_dead_code() {
        use uesave::games::palworld::{
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
                    try_success_cache: false,
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
                    Property::Struct(StructValue::PalMapConcreteModelModule(password_lock_module)),
                );
                properties
            })),
        };
        let object = map_object_with_model(SDM_NIL, SDM_NIL, None, Some(vec![module_entry]));

        assert!(
            !should_delete_map_object(&object, None, &[player]),
            "PasswordLock's player_infos must never be consulted -- real Python's \
             equivalent lookup targets the wrong (nested-in-RawData) location and \
             is unreachable dead code; a functional check here would diverge from \
             Python's actual byte-visible output"
        );
    }

    #[test]
    fn should_delete_map_object_returns_false_for_an_untyped_map_object() {
        assert!(!should_delete_map_object(
            &StructValue::Guid(uesave::FGuid::nil()),
            None,
            &[]
        ));
        let empty = StructValue::Struct(Properties::default());
        assert!(!should_delete_map_object(&empty, None, &[]));
    }
}
