//! Cross-save player transfer -- port of `game/player_transfer.py`'s
//! `transfer_player` and its six private helpers
//! (`_transfer_character_entry`, `_transfer_player_containers`,
//! `_transfer_tech`/`_transfer_appearance`, `_transfer_inventory`,
//! `_transfer_pals`/`_copy_pal_container_slots`/`_update_guild_pal_handles`,
//! `_transfer_guild`/`_create_guild_for_player`, `_sync_timestamps`).
//!
//! Reconciliation with the brief: the brief's assumed interface
//! (`CharacterMapEntry`/`GroupMapEntry`/`GroupPlayerInfo`/`ContainerEntry`
//! wrapper types with typed inherent methods, `PlayerGvasFiles::container_ids`,
//! `SaveSession::player_gvas`, ...) does NOT exist in this codebase. There are
//! no typed wrapper structs over `uesave::MapEntry`; Phase 2 navigates the raw
//! `Property` tree functionally through `psp_core::props` and the `domain::*`
//! helpers. This module is implemented directly against that real navigation
//! layer -- reusing `domain::world`'s character/group/container accessors and
//! entry helpers, `domain::player`'s `save_data_props`/`container_id_from`,
//! `domain::pal`'s `param`, `domain::guild`'s `find_player_guild_id`, and
//! `domain::guild_tail`'s `GuildTail` raw-tail codec -- rather than inventing a
//! typed-wrapper abstraction. The control flow, the `progress(...)` strings,
//! and the rejection messages are reproduced 1:1 from `player_transfer.py`.
//!
//! The Python guild `RawData` dict is split across two Rust representations:
//! `group_id`/`individual_character_handle_ids` are typed fields on
//! `uesave::games::palworld::PalGroupData`, while `players`/`guild_name`/
//! `admin_player_uid`/`base_ids`/`base_camp_level`/
//! `map_object_instance_ids_base_camp_points` live in `PalGroupData.
//! remaining_data`, decoded/re-encoded by `domain::guild_tail::GuildTail`. Each
//! Python `raw_data["..."]` access below is translated to whichever of the two
//! actually carries that field (see each helper).

use std::collections::HashSet;

use uesave::games::palworld::PalInstanceId;
use uesave::{MapEntry, Properties, Property, PropertyKey, Save, StructValue, ValueVec};
use uuid::Uuid;

use crate::domain::guild_tail::{self, GuildPlayerInfo};
use crate::domain::{guild, pal, player, world};
use crate::error::CoreError;
use crate::progress::ProgressSink;
use crate::props;
use crate::session::{parse_palworld_save, LoadedPlayer, SaveSession};

/// Which of a player's sub-trees the transfer touches. Mirrors
/// `transfer_player`'s five boolean parameters (`transfer_character`/
/// `transfer_inventory`/`transfer_pals`/`transfer_tech`/`transfer_appearance`),
/// each defaulting to `True` in Python.
#[derive(Debug, Clone)]
pub struct TransferOptions {
    pub transfer_character: bool,
    pub transfer_inventory: bool,
    pub transfer_pals: bool,
    pub transfer_tech: bool,
    pub transfer_appearance: bool,
}

/// `transfer_player`'s two failure shapes. `Rejected` is a SOFT rejection --
/// Python returns `{"error": msg}` (a normal WS response), NOT the WS error
/// frame -- so the handler layer maps this to `{"error": msg}`, not the
/// hard-error type. `Core` carries a genuine parse/IO failure.
#[derive(Debug, thiserror::Error)]
pub enum TransferError {
    #[error("{0}")]
    Rejected(String),
    #[error(transparent)]
    Core(#[from] CoreError),
}

/// Port of `player_transfer.py::transfer_player`. `Ok(())` == Python's
/// `{"success": True}`; `Err(TransferError::Rejected(msg))` == Python's
/// `{"error": msg}`. `target_player_uid == None` is spawn mode (Python's
/// `spawn_mode = target_player_uid is None`, then `target_player_uid =
/// source_player_uid`).
///
/// `source` and `target` are always distinct `SaveSession`s (Rust's borrow
/// checker forbids passing the same object as two `&mut`), which is why every
/// helper below can hold an immutable borrow of `source` and a mutable borrow
/// of `target` at once.
pub fn transfer_player(
    source: &mut SaveSession,
    target: &mut SaveSession,
    source_player_uid: Uuid,
    target_player_uid: Option<Uuid>,
    options: &TransferOptions,
    progress: &ProgressSink,
) -> Result<(), TransferError> {
    let spawn_mode = target_player_uid.is_none();
    let target_player_uid = target_player_uid.unwrap_or(source_player_uid);

    // --- Validation (player_transfer.py:173-207) ---
    progress("Validating players...");

    if !source.player_file_refs.contains_key(&source_player_uid) {
        return Err(TransferError::Rejected(format!(
            "Source player {source_player_uid} not found."
        )));
    }
    if !spawn_mode && !target.player_file_refs.contains_key(&target_player_uid) {
        return Err(TransferError::Rejected(format!(
            "Target player {target_player_uid} not found."
        )));
    }

    // `if source_player_uid not in source._player_gvas_files: await
    // source.load_player_on_demand(...)`. `loaded_players` is this port's
    // `_player_gvas_files` (the parsed GVAS, not the `_loaded_players` id set).
    ensure_player_gvas_loaded(source, source_player_uid)?;
    if !source.loaded_players.contains_key(&source_player_uid) {
        return Err(TransferError::Rejected(
            "Failed to load source player save file.".into(),
        ));
    }

    // `source_save_data = get_value(source_gvas.sav.properties["SaveData"])`;
    // `if not source_save_data: return {"error": "Source player SaveData is
    // missing or invalid."}`. Then the instance id.
    let source_instance_id = {
        let gvas = source
            .loaded_players
            .get(&source_player_uid)
            .expect("just ensured loaded");
        let save_data = match player::save_data_props(&gvas.sav) {
            Ok(save_data) => save_data,
            Err(_) => {
                return Err(TransferError::Rejected(
                    "Source player SaveData is missing or invalid.".into(),
                ))
            }
        };
        save_data_instance_id(save_data).unwrap_or(props::EMPTY_UUID)
    };

    // `source_level = (source_summary.level or 1) if source_summary else 1`.
    // `x or 1` treats both `None` and `0` as falsy -> 1.
    let source_level = source
        .player_summaries
        .get(&source_player_uid)
        .and_then(|summary| summary.level)
        .filter(|&level| level != 0)
        .unwrap_or(1);
    if source_level < 2 {
        return Err(TransferError::Rejected(format!(
            "Source player must be at least level 2 (current: {source_level})."
        )));
    }

    // --- Prepare target (player_transfer.py:208-236) ---
    let target_instance_id = if spawn_mode {
        progress("Spawning player into target save...");
        // Python deep-copies the in-memory source GVAS. `uesave::Save` is not
        // `Clone`, so this instead re-parses an independent copy from the
        // source player's own `.sav`/`_dps.sav` bytes -- identical to the
        // freshly-loaded source tree, since nothing edits the source between
        // load and here.
        let Some(file_ref) = source.player_file_refs.get(&source_player_uid).cloned() else {
            return Err(TransferError::Rejected(
                "Failed to load source player save file.".into(),
            ));
        };
        let Some(sav_bytes) = file_ref.sav_bytes()? else {
            return Err(TransferError::Rejected(
                "Failed to load source player save file.".into(),
            ));
        };
        let sav = parse_palworld_save(&sav_bytes)?;
        let dps = match file_ref.dps_bytes()? {
            Some(dps_bytes) => Some(parse_palworld_save(&dps_bytes)?),
            None => None,
        };
        target.loaded_players.insert(
            target_player_uid,
            LoadedPlayer {
                uid: target_player_uid,
                sav,
                dps,
            },
        );
        target.player_file_refs.insert(target_player_uid, file_ref);
        source_instance_id
    } else {
        ensure_player_gvas_loaded(target, target_player_uid)?;
        let Some(gvas) = target.loaded_players.get(&target_player_uid) else {
            return Err(TransferError::Rejected(
                "Failed to load target player save file.".into(),
            ));
        };
        let save_data = match player::save_data_props(&gvas.sav) {
            Ok(save_data) => save_data,
            Err(_) => {
                return Err(TransferError::Rejected(
                    "Target player SaveData is missing or invalid.".into(),
                ))
            }
        };
        save_data_instance_id(save_data).unwrap_or(props::EMPTY_UUID)
    };

    // --- Execute transfer steps (player_transfer.py:238-289) ---
    if options.transfer_character {
        progress("Transferring character data...");
        transfer_character_entry(
            source,
            target,
            source_player_uid,
            source_instance_id,
            target_player_uid,
            target_instance_id,
        )?;
        progress("Transferring player containers...");
        transfer_player_containers(source, target, source_player_uid)?;
    }
    if options.transfer_tech && !spawn_mode {
        progress("Transferring technology and recipes...");
        transfer_tech(source, source_player_uid, target, target_player_uid)?;
    }
    if options.transfer_appearance && !spawn_mode {
        progress("Transferring appearance data...");
        transfer_appearance(source, source_player_uid, target, target_player_uid)?;
    }
    if options.transfer_inventory && !spawn_mode {
        progress("Transferring inventory...");
        transfer_inventory(source, source_player_uid, target, target_player_uid)?;
    }
    if options.transfer_pals {
        progress("Transferring pals...");
        transfer_pals(source, source_player_uid, target, target_player_uid)?;
    }

    progress("Updating guild membership...");
    transfer_guild(source, source_player_uid, target, target_player_uid)?;

    progress("Syncing timestamps...");
    sync_timestamps(target, target_player_uid)?;

    progress("Rebuilding caches...");
    target.rebuild_player_caches()?;

    progress("Transfer complete!");
    Ok(())
}

// ---------------------------------------------------------------------------
// GVAS loading (player_transfer.py's `load_player_on_demand` dependency)
// ---------------------------------------------------------------------------

/// The GVAS-parsing half of `LoadingMixin.load_player_on_demand`
/// (`game/mixins/loading.py:82-141`) -- enough for the transfer, which reads
/// the raw character/group/container maps directly and never needs the pal
/// `Player` domain object Python's full loader also builds. Loads the player's
/// `.sav` (reusing the summary-extraction sav cache when present, exactly as
/// `player::get_player_details` does) and its `_dps.sav` companion into
/// `loaded_players`. A no-op when already loaded or the player has no file
/// reference (matching Python's early returns).
///
/// `pub(crate)`: Task 3E-4's `domain::uid_swap::SaveSession::swap_player_uids`
/// reuses this exact helper for its own "load both players on demand" step
/// (`player_swap.py::_validate_swap_players`'s `load_player_on_demand`
/// calls) rather than re-implementing GVAS lazy-loading a second time.
/// `SaveSession::ensure_player_loaded` (`session.rs`) re-exposes this same
/// helper as `pub`, for WS handlers that need to force-load a real,
/// eagerly-confirmed-present player before calling `domain::player::
/// build_player_dto` (which only resolves already-loaded players) — see that
/// method's own doc comment for the parity gap it closes.
pub(crate) fn ensure_player_gvas_loaded(
    session: &mut SaveSession,
    uid: Uuid,
) -> Result<(), CoreError> {
    if session.loaded_players.contains_key(&uid) {
        return Ok(());
    }
    if !session.player_file_refs.contains_key(&uid) {
        return Ok(());
    }
    let sav = match session.player_sav_cache.remove(&uid) {
        Some(cached) => cached,
        None => {
            let file_ref = session
                .player_file_refs
                .get(&uid)
                .expect("checked present above");
            let Some(sav_bytes) = file_ref.sav_bytes()? else {
                return Ok(());
            };
            parse_palworld_save(&sav_bytes)?
        }
    };
    let dps = {
        let file_ref = session
            .player_file_refs
            .get(&uid)
            .expect("checked present above");
        match file_ref.dps_bytes()? {
            Some(dps_bytes) => Some(parse_palworld_save(&dps_bytes)?),
            None => None,
        }
    };
    session
        .loaded_players
        .insert(uid, LoadedPlayer { uid, sav, dps });
    Ok(())
}

// ---------------------------------------------------------------------------
// Small navigation helpers (raw-MapEntry, no typed wrappers)
// ---------------------------------------------------------------------------

/// A container map entry's `key.ID` -- item- and character-container maps both
/// key this way (`_find_character_container`/`_find_item_container`'s
/// `container["key"]["ID"]`).
fn container_entry_id(entry: &MapEntry) -> Option<Uuid> {
    props::get(props::struct_props(&entry.key)?, &["ID"]).and_then(props::as_uuid)
}

/// `source_save_data["IndividualId"]["value"]["InstanceId"]`
/// (player_transfer.py:204-206).
///
/// `pub(crate)`: reused by Task 3E-4's `domain::uid_swap` for the identical
/// `IndividualId.InstanceId` read `player_swap.py:92-97` needs.
pub(crate) fn save_data_instance_id(save_data: &Properties) -> Option<Uuid> {
    props::get(save_data, &["IndividualId", "InstanceId"]).and_then(props::as_uuid)
}

// ---------------------------------------------------------------------------
// Transfer: character entry (player_transfer.py:298-339)
// ---------------------------------------------------------------------------

fn transfer_character_entry(
    source: &SaveSession,
    target: &mut SaveSession,
    source_uid: Uuid,
    source_instance_id: Uuid,
    target_uid: Uuid,
    target_instance_id: Uuid,
) -> Result<(), CoreError> {
    let source_entry = world::character_map(&source.level)?
        .iter()
        .find(|entry| {
            world::entry_is_player(entry)
                && world::entry_player_uid(entry) == Some(source_uid)
                && world::entry_instance_id(entry) == Some(source_instance_id)
        })
        .cloned();
    let Some(source_entry) = source_entry else {
        tracing::warn!("Source character entry not found in CharacterSaveParameterMap");
        return Ok(());
    };

    let entries = world::character_map_mut(&mut target.level)?;
    for entry in entries.iter_mut() {
        if world::entry_is_player(entry)
            && world::entry_player_uid(entry) == Some(target_uid)
            && world::entry_instance_id(entry) == Some(target_instance_id)
        {
            // `entry["value"] = copy.deepcopy(source_character["value"])`.
            entry.value = source_entry.value.clone();
            return Ok(());
        }
    }
    entries.push(source_entry);
    Ok(())
}

// ---------------------------------------------------------------------------
// Transfer: player containers (player_transfer.py:347-398)
// ---------------------------------------------------------------------------

/// `_get_player_container_ids` (player_transfer.py:101-141): the (up to) eight
/// container ids a player's SaveData references -- pal box + party at the top
/// level, the six inventory containers nested under `InventoryInfo` (capital
/// I, no lowercase fallback, matching Python's literal path tuples). Skips the
/// nil UUID.
fn player_container_ids(save_data: &Properties) -> HashSet<Uuid> {
    let mut ids = HashSet::new();
    for name in ["PalStorageContainerId", "OtomoCharacterContainerId"] {
        if let Some(id) = player::container_id_from(save_data, name) {
            if id != props::EMPTY_UUID {
                ids.insert(id);
            }
        }
    }
    if let Some(inventory_info) =
        props::get(save_data, &["InventoryInfo"]).and_then(props::struct_props)
    {
        for name in [
            "CommonContainerId",
            "DropSlotContainerId",
            "EssentialContainerId",
            "WeaponLoadOutContainerId",
            "PlayerEquipArmorContainerId",
            "FoodEquipContainerId",
        ] {
            if let Some(id) = player::container_id_from(inventory_info, name) {
                if id != props::EMPTY_UUID {
                    ids.insert(id);
                }
            }
        }
    }
    ids
}

/// `_copy_missing_containers` (player_transfer.py:372-398): append every source
/// container whose id is in `allowed` and not already present in the target.
fn copy_missing_containers(
    source_entries: &[MapEntry],
    target_entries: &mut Vec<MapEntry>,
    allowed: &HashSet<Uuid>,
) {
    let existing: HashSet<Uuid> = target_entries
        .iter()
        .filter_map(container_entry_id)
        .collect();
    for entry in source_entries {
        if let Some(id) = container_entry_id(entry) {
            if allowed.contains(&id) && !existing.contains(&id) {
                target_entries.push(entry.clone());
            }
        }
    }
}

fn transfer_player_containers(
    source: &SaveSession,
    target: &mut SaveSession,
    source_uid: Uuid,
) -> Result<(), CoreError> {
    let allowed = {
        let Some(gvas) = source.loaded_players.get(&source_uid) else {
            return Ok(());
        };
        match player::save_data_props(&gvas.sav) {
            Ok(save_data) => player_container_ids(save_data),
            Err(_) => return Ok(()),
        }
    };

    {
        let source_containers = world::character_container_map(&source.level)?;
        let target_containers = world::character_container_map_mut(&mut target.level)?;
        copy_missing_containers(source_containers, target_containers, &allowed);
    }
    {
        let source_containers = world::item_container_map(&source.level)?;
        let target_containers = world::item_container_map_mut(&mut target.level)?;
        copy_missing_containers(source_containers, target_containers, &allowed);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Transfer: technology / appearance (player_transfer.py:406-428)
// ---------------------------------------------------------------------------

/// Registers the `SaveData.bossTechnologyPoint` schema (an `IntProperty`)
/// copied off the always-present sibling `TechnologyPoint`, so a target that
/// never carried `bossTechnologyPoint` survives a later `Save::write`. Same
/// Phase-2 schema gap `player::apply_player_dto` closes; reproduced here since
/// `transfer_tech` can newly introduce that property on the target.
fn ensure_boss_tech_schema(sav: &mut Save) {
    if let Some(prefix) = props::schema_prefix_ending_with(sav, ".TechnologyPoint") {
        props::ensure_schema(
            sav,
            format!("{prefix}.bossTechnologyPoint"),
            uesave::PropertyTagPartial {
                id: None,
                data: uesave::PropertyTagDataPartial::Other(uesave::PropertyType::IntProperty),
            },
        );
    }
}

fn transfer_tech(
    source: &SaveSession,
    source_uid: Uuid,
    target: &mut SaveSession,
    target_uid: Uuid,
) -> Result<(), CoreError> {
    // Snapshot every source field first, so the source borrow is released
    // before the target is mutated.
    let (technology_point, boss_technology_point, unlocked_recipes, record_data) = {
        let Some(gvas) = source.loaded_players.get(&source_uid) else {
            return Ok(());
        };
        let Ok(save_data) = player::save_data_props(&gvas.sav) else {
            return Ok(());
        };
        (
            save_data
                .0
                .get(&PropertyKey::from("TechnologyPoint"))
                .cloned(),
            save_data
                .0
                .get(&PropertyKey::from("bossTechnologyPoint"))
                .cloned(),
            save_data
                .0
                .get(&PropertyKey::from("UnlockedRecipeTechnologyNames"))
                .cloned(),
            save_data.0.get(&PropertyKey::from("RecordData")).cloned(),
        )
    };

    let wrote_boss = boss_technology_point.is_some();
    {
        let Some(loaded) = target.loaded_players.get_mut(&target_uid) else {
            return Ok(());
        };
        let Ok(target_save_data) = player::save_data_props_mut(&mut loaded.sav) else {
            return Ok(());
        };

        // For each of the two point fields: copy source's value if present,
        // else zero the target's (only when the target already has it) --
        // `if key in source: target[key] = deepcopy(source[key]); elif key in
        // target: set_value(target[key], 0)`.
        for (key, source_value) in [
            ("TechnologyPoint", technology_point),
            ("bossTechnologyPoint", boss_technology_point),
        ] {
            match source_value {
                Some(value) => {
                    target_save_data.insert(key, value);
                }
                None => {
                    if target_save_data.0.contains_key(&PropertyKey::from(key)) {
                        target_save_data.insert(key, props::int_property(0));
                    }
                }
            }
        }
        if let Some(value) = unlocked_recipes {
            target_save_data.insert("UnlockedRecipeTechnologyNames", value);
        }
        // `if "RecordData" in source: target["RecordData"] = deepcopy(...);
        // elif "RecordData" in target: del target["RecordData"]`.
        match record_data {
            Some(value) => {
                target_save_data.insert("RecordData", value);
            }
            None => {
                target_save_data
                    .0
                    .shift_remove(&PropertyKey::from("RecordData"));
            }
        }
    }
    if wrote_boss {
        let loaded = target
            .loaded_players
            .get_mut(&target_uid)
            .expect("checked present above");
        ensure_boss_tech_schema(&mut loaded.sav);
    }
    Ok(())
}

fn transfer_appearance(
    source: &SaveSession,
    source_uid: Uuid,
    target: &mut SaveSession,
    target_uid: Uuid,
) -> Result<(), CoreError> {
    let make_data = {
        let Some(gvas) = source.loaded_players.get(&source_uid) else {
            return Ok(());
        };
        let Ok(save_data) = player::save_data_props(&gvas.sav) else {
            return Ok(());
        };
        save_data
            .0
            .get(&PropertyKey::from("PlayerCharacterMakeData"))
            .cloned()
    };
    if let Some(value) = make_data {
        let Some(loaded) = target.loaded_players.get_mut(&target_uid) else {
            return Ok(());
        };
        if let Ok(target_save_data) = player::save_data_props_mut(&mut loaded.sav) {
            target_save_data.insert("PlayerCharacterMakeData", value);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Transfer: inventory (player_transfer.py:436-480)
// ---------------------------------------------------------------------------

fn transfer_inventory(
    source: &SaveSession,
    source_uid: Uuid,
    target: &mut SaveSession,
    target_uid: Uuid,
) -> Result<(), CoreError> {
    // `_INVENTORY_CONTAINER_KEYS` (player_transfer.py:436-442), reproduced
    // literally: only the "main" path reaches through `InventoryInfo`; the
    // other four resolve against SaveData's top level (where these ids do NOT
    // live on a real save, so `_resolve_inventory_container_id` returns None
    // and Python skips them -- a faithful quirk, not a bug fixed here).
    let paths: [&[&str]; 5] = [
        &["InventoryInfo", "CommonContainerId", "ID"],
        &["CommonContainerId", "ID"],
        &["WeaponLoadOutContainerId", "ID"],
        &["PlayerEquipArmorContainerId", "ID"],
        &["FoodEquipContainerId", "ID"],
    ];

    let source_ids: Vec<Option<Uuid>> = {
        let Some(gvas) = source.loaded_players.get(&source_uid) else {
            return Ok(());
        };
        let Ok(save_data) = player::save_data_props(&gvas.sav) else {
            return Ok(());
        };
        paths
            .iter()
            .map(|path| props::get(save_data, path).and_then(props::as_uuid))
            .collect()
    };
    let target_ids: Vec<Option<Uuid>> = {
        let Some(gvas) = target.loaded_players.get(&target_uid) else {
            return Ok(());
        };
        let Ok(save_data) = player::save_data_props(&gvas.sav) else {
            return Ok(());
        };
        paths
            .iter()
            .map(|path| props::get(save_data, path).and_then(props::as_uuid))
            .collect()
    };

    for index in 0..paths.len() {
        let (Some(source_id), Some(target_id)) = (source_ids[index], target_ids[index]) else {
            continue;
        };
        let source_value = world::item_container_map(&source.level)?
            .iter()
            .find(|entry| container_entry_id(entry) == Some(source_id))
            .map(|entry| entry.value.clone());
        let Some(source_value) = source_value else {
            continue;
        };
        if let Some(entry) = world::item_container_map_mut(&mut target.level)?
            .iter_mut()
            .find(|entry| container_entry_id(entry) == Some(target_id))
        {
            // `target_container["value"] = copy.deepcopy(source_container["value"])`.
            entry.value = source_value;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Transfer: pals (player_transfer.py:488-644)
// ---------------------------------------------------------------------------

fn transfer_pals(
    source: &SaveSession,
    source_uid: Uuid,
    target: &mut SaveSession,
    target_uid: Uuid,
) -> Result<(), CoreError> {
    // `target_guild_id = _find_guild_id_for_player(target._group_save_data_map,
    // target_uid_str)` -- the guild containing the target, else the nil UUID.
    let target_guild_id =
        guild::find_player_guild_id(target, target_uid)?.unwrap_or(props::EMPTY_UUID);

    // Collect source pals (non-player, owned by source), retargeting ownership.
    let mut transferred: Vec<MapEntry> = Vec::new();
    for entry in world::character_map(&source.level)? {
        if world::entry_is_player(entry) {
            continue;
        }
        let Some(save_parameter) = world::entry_save_parameter(entry) else {
            continue;
        };
        if pal::param(save_parameter, "OwnerPlayerUId").and_then(props::as_uuid) != Some(source_uid)
        {
            continue;
        }
        transferred.push(entry.clone());
    }
    for pal_entry in transferred.iter_mut() {
        // `pal_raw["group_id"] = target_guild_id` -- PalCharacterData.group_id.
        if let Some(character_data) = world::entry_character_data_mut(pal_entry) {
            character_data.group_id = props::uuid_to_guid(target_guild_id);
        }
        if let Some(save_parameter) = world::entry_save_parameter_mut(pal_entry) {
            save_parameter.insert("OwnerPlayerUId", props::guid_property(target_uid));
            save_parameter
                .0
                .shift_remove(&PropertyKey::from("OldOwnerPlayerUIds"));
            save_parameter.0.shift_remove(&PropertyKey::from(
                "MapObjectConcreteInstanceIdAssignedToExpedition",
            ));
        }
    }

    copy_pal_container_slots(source, source_uid, target, target_uid, &mut transferred)?;

    // Instance ids captured before the move; needed by the guild-handle update.
    let transferred_ids: Vec<Uuid> = transferred
        .iter()
        .filter_map(world::entry_instance_id)
        .collect();

    // Replace the target player's own pals with the transferred set
    // (player_transfer.py:535-542's in-place `[:]` reassignment).
    {
        let entries = world::character_map_mut(&mut target.level)?;
        entries.retain(|entry| {
            world::entry_is_player(entry)
                || world::entry_save_parameter(entry)
                    .and_then(|save_parameter| pal::param(save_parameter, "OwnerPlayerUId"))
                    .and_then(props::as_uuid)
                    != Some(target_uid)
        });
        entries.extend(transferred);
    }

    update_guild_pal_handles(target, target_guild_id, &transferred_ids)?;
    Ok(())
}

/// `Slots.value.values` of the character container with id `container_id` --
/// `None` only when no such container entry exists (`_find_character_container`
/// returned None); an existing container with no `Slots` yields an empty vec,
/// matching Python's `.get("values", [])`.
fn character_container_slots(level: &Save, container_id: Uuid) -> Option<Vec<StructValue>> {
    let entry = world::character_container_map(level)
        .ok()?
        .iter()
        .find(|entry| container_entry_id(entry) == Some(container_id))?;
    let value_props = props::struct_props(&entry.value)?;
    Some(
        props::get(value_props, &["Slots"])
            .and_then(props::struct_values)
            .cloned()
            .unwrap_or_default(),
    )
}

/// Replaces the `Slots` array of the character container with id
/// `container_id`. Returns `false` when no such container entry exists (Python
/// `if not target_container: continue`).
fn set_character_container_slots(
    level: &mut Save,
    container_id: Uuid,
    slots: Vec<StructValue>,
) -> Result<bool, CoreError> {
    let entries = world::character_container_map_mut(level)?;
    let Some(entry) = entries
        .iter_mut()
        .find(|entry| container_entry_id(entry) == Some(container_id))
    else {
        return Ok(false);
    };
    let Some(value_props) = props::struct_props_mut(&mut entry.value) else {
        return Ok(false);
    };
    value_props.insert("Slots", Property::Array(ValueVec::Struct(slots)));
    Ok(true)
}

/// `_copy_pal_container_slots` (player_transfer.py:550-610): copy the pal box
/// and party slot arrays from source to target, then repoint every transferred
/// pal's `SlotId.ContainerId.ID` at the target container when the ids differ.
fn copy_pal_container_slots(
    source: &SaveSession,
    source_uid: Uuid,
    target: &mut SaveSession,
    target_uid: Uuid,
    transferred: &mut [MapEntry],
) -> Result<(), CoreError> {
    for container_key in ["PalStorageContainerId", "OtomoCharacterContainerId"] {
        let source_id = {
            let Some(gvas) = source.loaded_players.get(&source_uid) else {
                return Ok(());
            };
            let Ok(save_data) = player::save_data_props(&gvas.sav) else {
                return Ok(());
            };
            player::container_id_from(save_data, container_key)
        };
        let target_id = {
            let Some(gvas) = target.loaded_players.get(&target_uid) else {
                return Ok(());
            };
            let Ok(save_data) = player::save_data_props(&gvas.sav) else {
                return Ok(());
            };
            player::container_id_from(save_data, container_key)
        };
        let (Some(source_id), Some(target_id)) = (source_id, target_id) else {
            continue;
        };

        let Some(source_slots) = character_container_slots(&source.level, source_id) else {
            continue;
        };
        if !set_character_container_slots(&mut target.level, target_id, source_slots)? {
            continue;
        }

        // `if source_container_id != target_container_id:` repoint pals whose
        // slot still references the source container. Python reads only
        // `"SlotId"` here (no `"SlotID"` fallback), so this does too.
        if source_id != target_id {
            for pal_entry in transferred.iter_mut() {
                let Some(save_parameter) = world::entry_save_parameter_mut(pal_entry) else {
                    continue;
                };
                if let Some(id_property) =
                    props::get_mut(save_parameter, &["SlotId", "ContainerId", "ID"])
                {
                    if props::as_uuid(id_property) == Some(source_id) {
                        *id_property = props::guid_property(target_id);
                    }
                }
            }
        }
    }
    Ok(())
}

/// `_update_guild_pal_handles` (player_transfer.py:613-644): in the target's
/// own guild, drop any handle already pointing at a transferred pal, then
/// append a fresh `{guid: nil, instance_id}` handle per transferred pal.
/// `individual_character_handle_ids` is a typed `PalGroupData` field here (not
/// part of the raw guild tail).
fn update_guild_pal_handles(
    target: &mut SaveSession,
    target_guild_id: Uuid,
    transferred_ids: &[Uuid],
) -> Result<(), CoreError> {
    let transferred_set: HashSet<Uuid> = transferred_ids.iter().copied().collect();
    let groups = world::group_map_mut(&mut target.level)?;
    for entry in groups.iter_mut() {
        let Some(group_data) = guild_tail::entry_group_data_mut(entry) else {
            continue;
        };
        if props::guid_to_uuid(&group_data.group_id) != target_guild_id {
            continue;
        }
        group_data
            .individual_character_handle_ids
            .retain(|handle| !transferred_set.contains(&props::guid_to_uuid(&handle.instance_id)));
        for &instance_id in transferred_ids {
            group_data
                .individual_character_handle_ids
                .push(PalInstanceId {
                    guid: props::uuid_to_guid(props::EMPTY_UUID),
                    instance_id: props::uuid_to_guid(instance_id),
                });
        }
        break;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Transfer: guild membership (player_transfer.py:652-743)
// ---------------------------------------------------------------------------

/// The source player's guild-tail membership (`last_online_real_time`,
/// `player_name`), the two fields carried onto the transferred player's new
/// membership row. `None` when the source player is in no guild.
fn find_source_guild_member(
    source: &SaveSession,
    source_uid: Uuid,
) -> Result<Option<(i64, String)>, CoreError> {
    for entry in world::group_map(&source.level)? {
        if guild_tail::entry_group_type(entry).as_deref() != Some("EPalGroupType::Guild") {
            continue;
        }
        let Some(group_data) = guild_tail::entry_group_data(entry) else {
            continue;
        };
        let Some(guild) = guild_tail::as_guild(group_data) else {
            continue;
        };
        if let Some((last_online_real_time, player_name)) =
            guild_tail::find_player_membership(guild, source_uid)
        {
            return Ok(Some((last_online_real_time, player_name)));
        }
    }
    Ok(None)
}

fn transfer_guild(
    source: &SaveSession,
    source_uid: Uuid,
    target: &mut SaveSession,
    target_uid: Uuid,
) -> Result<(), CoreError> {
    let Some((last_online_real_time, player_name)) = find_source_guild_member(source, source_uid)?
    else {
        tracing::warn!("Source player has no guild membership to transfer");
        return Ok(());
    };
    // `source_guild_info["player_uid"] = target_uid_str` -- the retargeted row.
    let retargeted = GuildPlayerInfo {
        player_uid: target_uid,
        last_online_real_time,
        player_name,
    };

    // Try to update the target's existing guild membership: find the guild
    // whose tail already lists `target_uid`, drop that row, append the new one.
    let target_guild_position = {
        let groups = world::group_map(&target.level)?;
        groups.iter().position(|entry| {
            guild_tail::entry_group_type(entry).as_deref() == Some("EPalGroupType::Guild")
                && guild_tail::entry_group_data(entry)
                    .and_then(guild_tail::as_guild)
                    .map(|guild| guild_tail::guild_has_player(guild, target_uid))
                    .unwrap_or(false)
        })
    };
    if let Some(position) = target_guild_position {
        let groups = world::group_map_mut(&mut target.level)?;
        let group_data = guild_tail::entry_group_data_mut(&mut groups[position])
            .ok_or_else(|| CoreError::Parse("target guild group data untyped".into()))?;
        let guild = guild_tail::as_guild_mut(group_data)
            .ok_or_else(|| CoreError::Parse("target guild group data untyped".into()))?;
        // Drop the target's existing row and re-add the retargeted one,
        // preserving its role (PostUpdate guilds) so the members-with-roles
        // and role_permissions stay consistent.
        let existing_role = guild_tail::remove_player(guild, target_uid);
        guild_tail::push_player(guild, &retargeted, existing_role);
        return Ok(());
    }

    // Target player has no guild -- clone the source guild as a template and
    // reset it to a fresh single-member guild (`_create_guild_for_player`,
    // player_transfer.py:700-743).
    create_guild_for_player(source, source_uid, target, target_uid, retargeted)
}

fn create_guild_for_player(
    source: &SaveSession,
    source_uid: Uuid,
    target: &mut SaveSession,
    target_uid: Uuid,
    retargeted: GuildPlayerInfo,
) -> Result<(), CoreError> {
    let template = world::group_map(&source.level)?
        .iter()
        .find(|entry| {
            guild_tail::entry_group_type(entry).as_deref() == Some("EPalGroupType::Guild")
                && guild_tail::entry_group_data(entry)
                    .and_then(guild_tail::as_guild)
                    .map(|guild| guild_tail::guild_has_player(guild, source_uid))
                    .unwrap_or(false)
        })
        .cloned();
    let Some(mut new_guild) = template else {
        tracing::warn!("Could not find source guild to use as template");
        return Ok(());
    };

    let new_guild_id = Uuid::new_v4();
    // Python assigns the raw string uuid to `new_guild["key"]`; the correct
    // typed representation of a GroupSaveDataMap key is a Guid property, so
    // that is what is written here (Python's bare-string key is a latent bug
    // with no faithful, non-corrupting Rust analogue -- see this task's report).
    new_guild.key = props::guid_property(new_guild_id);
    if let Some(group_data) = guild_tail::entry_group_data_mut(&mut new_guild) {
        group_data.group_id = props::uuid_to_guid(new_guild_id);
        group_data.individual_character_handle_ids = Vec::new();
        if let Some(guild) = guild_tail::as_guild_mut(group_data) {
            // Preserve the source admin's role for the new single-member
            // guild (PostUpdate guilds carry per-player roles); harmless
            // `None` for PreUpdate.
            let source_role = guild_tail::player_role(guild, source_uid);
            guild_tail::reset_to_single_member(
                guild,
                "Transferred Guild",
                target_uid,
                &retargeted,
                source_role,
            );
        }
    }
    world::group_map_mut(&mut target.level)?.push(new_guild);
    Ok(())
}

// ---------------------------------------------------------------------------
// Transfer: timestamps (player_transfer.py:751-795)
// ---------------------------------------------------------------------------

fn sync_timestamps(target: &mut SaveSession, target_uid: Uuid) -> Result<(), CoreError> {
    // `world_tick = world_save["GameTimeSaveData"]["value"]
    // ["RealDateTimeTicks"]["value"]`; `if not world_tick: return` (0 is falsy).
    let world_tick = {
        let Ok(world_props) = world::world_props(&target.level) else {
            return Ok(());
        };
        match props::get(world_props, &["GameTimeSaveData", "RealDateTimeTicks"])
            .and_then(props::as_i64)
            .filter(|&ticks| ticks != 0)
        {
            Some(ticks) => ticks,
            None => return Ok(()),
        }
    };

    // CharacterSaveParameterMap: the target player's own entry. Python also
    // sets `raw_data["last_online_real_time"]` on the character RawData, but
    // that key is not part of `PalCharacterData`'s codec (`object`/
    // `unknown_bytes`/`group_id`/`trailing_bytes`), so Python's write there is
    // a no-op on the wire and has no Rust counterpart. `LastOnlineRealTime` in
    // the SaveParameter bag IS real and is updated (only when already present).
    {
        let entries = world::character_map_mut(&mut target.level)?;
        for entry in entries.iter_mut() {
            if !world::entry_is_player(entry) {
                continue;
            }
            if world::entry_player_uid(entry) == Some(target_uid) {
                if let Some(save_parameter) = world::entry_save_parameter_mut(entry) {
                    if save_parameter
                        .0
                        .contains_key(&PropertyKey::from("LastOnlineRealTime"))
                    {
                        save_parameter
                            .insert("LastOnlineRealTime", props::int64_property(world_tick));
                    }
                }
                break;
            }
        }
    }

    // Guild player list: every guild row for the target player (Python's loop
    // has no break, so all matches are updated).
    {
        let groups = world::group_map_mut(&mut target.level)?;
        for entry in groups.iter_mut() {
            if guild_tail::entry_group_type(entry).as_deref() != Some("EPalGroupType::Guild") {
                continue;
            }
            let Some(group_data) = guild_tail::entry_group_data_mut(entry) else {
                continue;
            };
            let Some(guild) = guild_tail::as_guild_mut(group_data) else {
                continue;
            };
            guild_tail::set_player_last_online(guild, target_uid, world_tick);
        }
    }
    Ok(())
}
