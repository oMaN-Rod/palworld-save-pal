//! Cross-save player transfer.
//!
//! A guild's data is split across two representations: `group_id` and
//! `individual_character_handle_ids` are typed fields on
//! `crate::ue::games::palworld::PalGroupData`, while `players`, `guild_name`,
//! `admin_player_uid`, `base_ids`, `base_camp_level` and
//! `map_object_instance_ids_base_camp_points` live in the opaque
//! `PalGroupData.remaining_data` tail, decoded by `domain::guild_tail`.

use std::collections::HashSet;

use crate::ue::games::palworld::PalInstanceId;
use crate::ue::{MapEntry, Properties, Property, PropertyKey, Save, StructValue, ValueVec};
use uuid::Uuid;

use crate::domain::guild_tail::{self, GuildPlayerInfo};
use crate::domain::{guild, pal, player, world};
use crate::error::CoreError;
use crate::progress::ProgressSink;
use crate::props;
use crate::session::{parse_palworld_save, LoadedPlayer, SaveSession};

/// Which of a player's sub-trees the transfer touches.
#[derive(Debug, Clone)]
pub struct TransferOptions {
    pub transfer_character: bool,
    pub transfer_inventory: bool,
    pub transfer_pals: bool,
    pub transfer_tech: bool,
    pub transfer_appearance: bool,
}

/// `Rejected` is a SOFT rejection: the handler layer must map it to a normal
/// `{"error": msg}` WS response, not the WS error frame. `Core` carries a
/// genuine parse/IO failure.
#[derive(Debug, thiserror::Error)]
pub enum TransferError {
    #[error("{0}")]
    Rejected(String),
    #[error(transparent)]
    Core(#[from] CoreError),
}

/// `target_player_uid == None` means spawn mode: the source player is spawned
/// into the target save under its own uid.
///
/// `source` and `target` are always distinct `SaveSession`s — the borrow
/// checker forbids passing one object as two `&mut` — which is why every helper
/// below can hold `&source` and `&mut target` at once.
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

    ensure_player_gvas_loaded(source, source_player_uid)?;
    if !source.loaded_players.contains_key(&source_player_uid) {
        return Err(TransferError::Rejected(
            "Failed to load source player save file.".into(),
        ));
    }

    // A transfer grafts whole source subtrees onto the target -- character-map entry,
    // pals, containers -- carrying properties the target may have no tag for.
    props::merge_schemas(&mut target.level, &source.level);

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

    // A missing summary and a recorded level of 0 both mean level 1.
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

    let target_instance_id = if spawn_mode {
        progress("Spawning player into target save...");
        // `crate::ue::Save` is not `Clone`, so an independent copy of the source
        // player is obtained by re-parsing its own bytes.
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
            LoadedPlayer::new(target_player_uid, sav, dps),
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

/// Loads a player's `.sav` (reusing the summary-extraction sav cache when
/// present) and its `_dps.sav` companion into `loaded_players`. A no-op when
/// already loaded or the player has no file reference.
///
/// Re-exposed as `pub` by `SaveSession::ensure_player_loaded`.
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
        .insert(uid, LoadedPlayer::new(uid, sav, dps));
    Ok(())
}

/// A container map entry's `key.ID` — item- and character-container maps both
/// key this way.
fn container_entry_id(entry: &MapEntry) -> Option<Uuid> {
    props::get(props::struct_props(&entry.key)?, &["ID"]).and_then(props::as_uuid)
}

pub(crate) fn save_data_instance_id(save_data: &Properties) -> Option<Uuid> {
    props::get(save_data, &["IndividualId", "InstanceId"]).and_then(props::as_uuid)
}

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
            entry.value = source_entry.value.clone();
            return Ok(());
        }
    }
    entries.push(source_entry);
    Ok(())
}

/// The up to eight container ids a player's SaveData references: pal box and
/// party at the top level, the six inventory containers nested under
/// `InventoryInfo`. The nil UUID is skipped.
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

/// Appends every source container whose id is in `allowed` and not already
/// present in the target.
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

    // `RecordData` is grafted wholesale below; a target `.sav` carrying none of its
    // own has a tag for nothing beneath it.
    let source_sav_schemas = source
        .loaded_players
        .get(&source_uid)
        .map(|gvas| gvas.sav.schemas.clone());

    {
        let Some(loaded) = target.loaded_players.get_mut(&target_uid) else {
            return Ok(());
        };
        if let Some(schemas) = &source_sav_schemas {
            for (path, tag) in schemas.schemas().clone() {
                props::ensure_schema(&mut loaded.sav, path, tag);
            }
        }
    }

    let wrote_boss = boss_technology_point.is_some();
    {
        let Some(loaded) = target.loaded_players.get_mut(&target_uid) else {
            return Ok(());
        };
        let Ok(target_save_data) = player::save_data_props_mut(&mut loaded.sav) else {
            return Ok(());
        };

        // Copy the source's value when present, else zero the target's — but
        // only when the target already carries the field.
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
        // A source with no RecordData removes the target's outright.
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
        player::ensure_player_sav_schemas(&mut loaded.sav);
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

fn transfer_inventory(
    source: &SaveSession,
    source_uid: Uuid,
    target: &mut SaveSession,
    target_uid: Uuid,
) -> Result<(), CoreError> {
    // Only the first path reaches through `InventoryInfo`; the other four
    // resolve against SaveData's top level, where these ids do not live on a
    // real save, so they resolve to `None` and are skipped.
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
            entry.value = source_value;
        }
    }
    Ok(())
}

fn transfer_pals(
    source: &SaveSession,
    source_uid: Uuid,
    target: &mut SaveSession,
    target_uid: Uuid,
) -> Result<(), CoreError> {
    // The guild containing the target, else the nil UUID.
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

    // Replace the target player's own pals with the transferred set.
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

/// `Slots` of the character container with id `container_id`. `None` only when
/// no such container exists; an existing container with no `Slots` yields an
/// empty vec.
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
/// `container_id`. `false` when no such container entry exists.
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

/// Copies the pal box and party slot arrays from source to target, then
/// repoints every transferred pal's `SlotId.ContainerId.ID` at the target
/// container when the ids differ.
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

        // Repoint pals whose slot still references the source container. The
        // key is `SlotId`; there is no `SlotID` spelling on this path.
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

/// In the target's own guild, drops any handle already pointing at a
/// transferred pal, then appends a fresh `{guid: nil, instance_id}` handle per
/// transferred pal.
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

    // Target player has no guild: clone the source guild as a template and
    // reset it to a fresh single-member guild.
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
    // A GroupSaveDataMap key is a Guid property, not a string.
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

fn sync_timestamps(target: &mut SaveSession, target_uid: Uuid) -> Result<(), CoreError> {
    // A world tick of 0 means the save has no usable clock; nothing to sync.
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

    // `LastOnlineRealTime` lives in the SaveParameter bag, and is updated only
    // when the entry already carries it. The character RawData
    // (`PalCharacterData`) has no last-online field at all.
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

    // Every guild row for the target player is updated, not just the first.
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
