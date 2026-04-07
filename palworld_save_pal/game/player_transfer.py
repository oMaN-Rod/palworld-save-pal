import copy
from typing import Any, Callable, Dict, List, Optional, Set
from uuid import UUID

from palworld_save_pal.game.pal_objects import PalObjects
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)

EMPTY_UUID = "00000000-0000-0000-0000-000000000000"


# ---------------------------------------------------------------------------
# Helpers for navigating CharacterSaveParameterMap entries
# ---------------------------------------------------------------------------


def _get_save_parameter(entry: Dict) -> Optional[Dict]:
    try:
        return entry["value"]["RawData"]["value"]["object"]["SaveParameter"]["value"]
    except (KeyError, TypeError):
        return None


def _is_player_entry(entry: Dict) -> bool:
    save_param = _get_save_parameter(entry)
    if not save_param:
        return False
    return bool(PalObjects.get_value(save_param.get("IsPlayer", {})))


def _get_entry_instance_id(entry: Dict) -> Optional[str]:
    try:
        return str(PalObjects.get_guid(entry["key"]["InstanceId"])).lower()
    except (KeyError, TypeError):
        return None


def _get_entry_player_uid(entry: Dict) -> Optional[str]:
    try:
        return str(PalObjects.get_guid(entry["key"]["PlayerUId"])).lower()
    except (KeyError, TypeError):
        return None


def _get_owner_uid(save_param: Dict) -> Optional[str]:
    owner_data = save_param.get("OwnerPlayerUId")
    if not owner_data:
        return None
    owner_value = (
        PalObjects.get_value(owner_data)
        if isinstance(owner_data, dict)
        else owner_data
    )
    return str(owner_value).lower() if owner_value else None


# ---------------------------------------------------------------------------
# Helpers for guild and container lookups
# ---------------------------------------------------------------------------


def _find_guild_id_for_player(group_save_data_map: List, player_uid_str: str) -> str:
    for group_entry in group_save_data_map:
        try:
            raw_data = group_entry["value"]["RawData"]["value"]
            for player_info in raw_data.get("players", []):
                if str(player_info.get("player_uid", "")).lower() == player_uid_str:
                    return str(raw_data.get("group_id", EMPTY_UUID)).lower()
        except (KeyError, TypeError):
            continue
    return EMPTY_UUID


def _find_character_container(save_manager, container_id_str: str) -> Optional[Dict]:
    for container in save_manager._character_container_save_data:
        try:
            current_id = str(PalObjects.get_guid(container["key"]["ID"])).lower()
            if current_id == container_id_str:
                return container
        except (KeyError, TypeError):
            continue
    return None


def _find_item_container(save_manager, container_id_str: str) -> Optional[Dict]:
    for container in save_manager._item_container_save_data:
        try:
            current_id = str(PalObjects.get_guid(container["key"]["ID"])).lower()
            if current_id == container_id_str:
                return container
        except (KeyError, TypeError):
            continue
    return None


def _get_player_container_ids(save_data: Dict) -> Set[str]:
    """Extract all container IDs referenced by a player's SaveData."""
    container_ids = set()
    container_paths = [
        ("PalStorageContainerId", "value", "ID"),
        ("OtomoCharacterContainerId", "value", "ID"),
    ]
    inventory_container_paths = [
        ("CommonContainerId", "value", "ID"),
        ("DropSlotContainerId", "value", "ID"),
        ("EssentialContainerId", "value", "ID"),
        ("WeaponLoadOutContainerId", "value", "ID"),
        ("PlayerEquipArmorContainerId", "value", "ID"),
        ("FoodEquipContainerId", "value", "ID"),
    ]

    for path in container_paths:
        try:
            node = save_data
            for key in path:
                node = node[key]
            container_id = str(PalObjects.get_guid(node)).lower()
            if container_id and container_id != EMPTY_UUID:
                container_ids.add(container_id)
        except (KeyError, TypeError):
            continue

    # Inventory containers are nested under InventoryInfo
    inventory_info = save_data.get("InventoryInfo", {}).get("value", {})
    for path in inventory_container_paths:
        try:
            node = inventory_info
            for key in path:
                node = node[key]
            container_id = str(PalObjects.get_guid(node)).lower()
            if container_id and container_id != EMPTY_UUID:
                container_ids.add(container_id)
        except (KeyError, TypeError):
            continue

    return container_ids


# ---------------------------------------------------------------------------
# Main transfer orchestrator
# ---------------------------------------------------------------------------


async def transfer_player(
    source,
    target,
    source_player_uid: UUID,
    target_player_uid: Optional[UUID] = None,
    transfer_character: bool = True,
    transfer_inventory: bool = True,
    transfer_pals: bool = True,
    transfer_tech: bool = True,
    transfer_appearance: bool = True,
    ws_callback: Optional[Callable] = None,
) -> Dict[str, Any]:
    async def progress(msg: str):
        logger.info(msg)
        if ws_callback:
            await ws_callback(msg)

    spawn_mode = target_player_uid is None
    if spawn_mode:
        target_player_uid = source_player_uid

    source_uid_str = str(source_player_uid).lower()
    target_uid_str = str(target_player_uid).lower()

    # --- Validation ---

    await progress("Validating players...")

    if source_player_uid not in source._player_file_refs:
        return {"error": f"Source player {source_player_uid} not found."}

    if not spawn_mode and target_player_uid not in target._player_file_refs:
        return {"error": f"Target player {target_player_uid} not found."}

    # Load source GVAS
    if source_player_uid not in source._player_gvas_files:
        await source.load_player_on_demand(source_player_uid)

    source_gvas = source._player_gvas_files.get(source_player_uid)
    if not source_gvas:
        return {"error": "Failed to load source player save file."}

    source_save_data = PalObjects.get_value(source_gvas.sav.properties["SaveData"])

    source_summary = source._player_summaries.get(source_player_uid)
    source_level = (source_summary.level or 1) if source_summary else 1
    if source_level < 2:
        return {
            "error": f"Source player must be at least level 2 (current: {source_level})."
        }

    source_instance_id = str(
        PalObjects.get_guid(source_save_data["IndividualId"]["value"]["InstanceId"])
    ).lower()

    # --- Prepare target ---

    if spawn_mode:
        await progress("Spawning player into target save...")
        from palworld_save_pal.game.player import PlayerGvasFiles

        target._player_gvas_files[target_player_uid] = PlayerGvasFiles(
            sav=copy.deepcopy(source_gvas.sav),
            dps=copy.deepcopy(source_gvas.dps) if source_gvas.dps else None,
        )
        target._player_file_refs[target_player_uid] = copy.deepcopy(
            source._player_file_refs[source_player_uid]
        )
        target_gvas = target._player_gvas_files[target_player_uid]
        target_save_data = PalObjects.get_value(
            target_gvas.sav.properties["SaveData"]
        )
        target_instance_id = source_instance_id
    else:
        if target_player_uid not in target._player_gvas_files:
            await target.load_player_on_demand(target_player_uid)
        target_gvas = target._player_gvas_files.get(target_player_uid)
        if not target_gvas:
            return {"error": "Failed to load target player save file."}
        target_save_data = PalObjects.get_value(
            target_gvas.sav.properties["SaveData"]
        )
        target_instance_id = str(
            PalObjects.get_guid(
                target_save_data["IndividualId"]["value"]["InstanceId"]
            )
        ).lower()

    # --- Execute transfer steps ---

    if transfer_character:
        await progress("Transferring character data...")
        _transfer_character_entry(
            source, target,
            source_uid_str, source_instance_id,
            target_uid_str, target_instance_id,
        )

    if transfer_character:
        await progress("Transferring player containers...")
        _transfer_player_containers(source, target, source_save_data)

    if transfer_tech and not spawn_mode:
        await progress("Transferring technology and recipes...")
        _transfer_tech(source_save_data, target_save_data)

    if transfer_appearance and not spawn_mode:
        await progress("Transferring appearance data...")
        _transfer_appearance(source_save_data, target_save_data)

    if transfer_inventory and not spawn_mode:
        await progress("Transferring inventory...")
        _transfer_inventory(source, target, source_gvas, target_gvas)

    if transfer_pals:
        await progress("Transferring pals...")
        _transfer_pals(
            source, target,
            source_uid_str, target_uid_str,
            source_gvas, target_gvas,
        )

    await progress("Updating guild membership...")
    _transfer_guild(source, target, source_uid_str, target_uid_str, target_save_data)

    await progress("Syncing timestamps...")
    _sync_timestamps(target, target_uid_str)

    # --- Rebuild caches ---

    await progress("Rebuilding caches...")
    target.invalidate_performance_caches()
    target._players.clear()
    target._guilds.clear()
    target._loaded_players.clear()
    target._loaded_guilds.clear()
    target._extract_player_summaries()
    target._extract_guild_summaries()

    await progress("Transfer complete!")
    return {"success": True}


# ---------------------------------------------------------------------------
# Transfer: character entry in CharacterSaveParameterMap
# ---------------------------------------------------------------------------


def _transfer_character_entry(
    source, target,
    source_uid_str, source_instance_id,
    target_uid_str, target_instance_id,
):
    # Find source player's character entry
    source_character = None
    for entry in source._character_save_parameter_map:
        if not _is_player_entry(entry):
            continue
        if (
            _get_entry_player_uid(entry) == source_uid_str
            and _get_entry_instance_id(entry) == source_instance_id
        ):
            source_character = entry
            break

    if not source_character:
        logger.warning("Source character entry not found in CharacterSaveParameterMap")
        return

    logger.info("Found source character entry with instance_id=%s", source_instance_id)

    # Update existing target entry or append
    for entry in target._character_save_parameter_map:
        if not _is_player_entry(entry):
            continue
        if (
            _get_entry_player_uid(entry) == target_uid_str
            and _get_entry_instance_id(entry) == target_instance_id
        ):
            entry["value"] = copy.deepcopy(source_character["value"])
            return

    target._character_save_parameter_map.append(copy.deepcopy(source_character))
    logger.info(
        "Appended character entry to target (total entries: %d)",
        len(target._character_save_parameter_map),
    )


# ---------------------------------------------------------------------------
# Transfer: only containers referenced by the source player
# ---------------------------------------------------------------------------


def _transfer_player_containers(source, target, source_save_data):
    source_container_ids = _get_player_container_ids(source_save_data)
    logger.info(
        "Source player references %d containers: %s",
        len(source_container_ids),
        source_container_ids,
    )

    # Copy character containers (pal box, party) that don't exist in target
    _copy_missing_containers(
        source._character_container_save_data,
        target._character_container_save_data,
        source_container_ids,
    )

    # Copy item containers (inventory slots) that don't exist in target
    _copy_missing_containers(
        source._item_container_save_data,
        target._item_container_save_data,
        source_container_ids,
    )


def _copy_missing_containers(
    source_list: List, target_list: List, allowed_ids: Set[str]
):
    existing_target_ids = set()
    for container in target_list:
        try:
            container_id = str(PalObjects.get_guid(container["key"]["ID"])).lower()
            existing_target_ids.add(container_id)
        except (KeyError, TypeError):
            continue

    copied_count = 0
    for container in source_list:
        try:
            container_id = str(PalObjects.get_guid(container["key"]["ID"])).lower()
            if container_id in allowed_ids and container_id not in existing_target_ids:
                target_list.append(copy.deepcopy(container))
                copied_count += 1
        except (KeyError, TypeError):
            continue

    logger.info(
        "Copied %d containers (allowed: %d, already in target: %d)",
        copied_count, len(allowed_ids), len(existing_target_ids),
    )


# ---------------------------------------------------------------------------
# Transfer: technology, recipes, appearance
# ---------------------------------------------------------------------------


def _transfer_tech(source_save_data, target_save_data):
    for key in ("TechnologyPoint", "bossTechnologyPoint"):
        if key in source_save_data:
            target_save_data[key] = copy.deepcopy(source_save_data[key])
        elif key in target_save_data:
            PalObjects.set_value(target_save_data[key], 0)

    if "UnlockedRecipeTechnologyNames" in source_save_data:
        target_save_data["UnlockedRecipeTechnologyNames"] = copy.deepcopy(
            source_save_data["UnlockedRecipeTechnologyNames"]
        )

    if "RecordData" in source_save_data:
        target_save_data["RecordData"] = copy.deepcopy(source_save_data["RecordData"])
    elif "RecordData" in target_save_data:
        del target_save_data["RecordData"]


def _transfer_appearance(source_save_data, target_save_data):
    if "PlayerCharacterMakeData" in source_save_data:
        target_save_data["PlayerCharacterMakeData"] = copy.deepcopy(
            source_save_data["PlayerCharacterMakeData"]
        )


# ---------------------------------------------------------------------------
# Transfer: inventory
# ---------------------------------------------------------------------------


_INVENTORY_CONTAINER_KEYS = {
    "main": ("InventoryInfo", "value", "CommonContainerId", "value", "ID"),
    "key": ("CommonContainerId", "value", "ID"),
    "weps": ("WeaponLoadOutContainerId", "value", "ID"),
    "armor": ("PlayerEquipArmorContainerId", "value", "ID"),
    "foodbag": ("FoodEquipContainerId", "value", "ID"),
}


def _resolve_inventory_container_id(save_data: Dict, path: tuple) -> Optional[str]:
    try:
        node = save_data
        for key in path:
            node = node[key]
        container_id = PalObjects.get_guid(node)
        return str(container_id).lower() if container_id else None
    except (KeyError, TypeError):
        return None


def _transfer_inventory(source, target, source_gvas, target_gvas):
    source_save_data = PalObjects.get_value(source_gvas.sav.properties["SaveData"])
    target_save_data = PalObjects.get_value(target_gvas.sav.properties["SaveData"])

    for inv_type, path in _INVENTORY_CONTAINER_KEYS.items():
        source_id = _resolve_inventory_container_id(source_save_data, path)
        target_id = _resolve_inventory_container_id(target_save_data, path)
        if not source_id or not target_id:
            continue

        source_container = _find_item_container(source, source_id)
        target_container = _find_item_container(target, target_id)
        if source_container and target_container:
            target_container["value"] = copy.deepcopy(source_container["value"])


# ---------------------------------------------------------------------------
# Transfer: pals
# ---------------------------------------------------------------------------


def _transfer_pals(
    source, target,
    source_uid_str, target_uid_str,
    source_gvas, target_gvas,
):
    target_guild_id = _find_guild_id_for_player(
        target._group_save_data_map, target_uid_str
    )

    # Collect source pals, updating ownership to target
    transferred_pals = []
    for entry in source._character_save_parameter_map:
        save_param = _get_save_parameter(entry)
        if not save_param or _is_player_entry(entry):
            continue
        if _get_owner_uid(save_param) != source_uid_str:
            continue

        pal_copy = copy.deepcopy(entry)
        pal_raw = pal_copy["value"]["RawData"]["value"]
        pal_raw["group_id"] = target_guild_id

        pal_param = _get_save_parameter(pal_copy)
        if pal_param:
            PalObjects.set_value(pal_param["OwnerPlayerUId"], target_uid_str)
            pal_param.pop("OldOwnerPlayerUIds", None)
            pal_param.pop("MapObjectConcreteInstanceIdAssignedToExpedition", None)

        transferred_pals.append(pal_copy)

    logger.info(
        "Collected %d pals from source player %s",
        len(transferred_pals), source_uid_str[:8],
    )

    # Copy container slots and update pal SlotId references
    _copy_pal_container_slots(source, target, source_gvas, target_gvas, transferred_pals)

    # Replace target player's existing pals with transferred ones.
    # IMPORTANT: mutate the list in-place ([:] slice assignment) to preserve
    # the reference to _gvas_file.properties. Reassignment would break serialization.
    filtered_entries = [
        entry for entry in target._character_save_parameter_map
        if _is_player_entry(entry)
        or _get_owner_uid(_get_save_parameter(entry) or {}) != target_uid_str
    ]
    filtered_entries.extend(transferred_pals)
    target._character_save_parameter_map[:] = filtered_entries

    logger.info("Added %d pals to target save", len(transferred_pals))

    # Update guild handle list
    _update_guild_pal_handles(target, target_guild_id, transferred_pals)


def _copy_pal_container_slots(source, target, source_gvas, target_gvas, transferred_pals):
    """Copy palbox and party slot data from source to target containers,
    and update each pal's SlotId.ContainerId to reference the target container."""
    source_save_data = PalObjects.get_value(source_gvas.sav.properties["SaveData"])
    target_save_data = PalObjects.get_value(target_gvas.sav.properties["SaveData"])

    container_pairs = [
        ("PalStorageContainerId", "palbox"),
        ("OtomoCharacterContainerId", "party"),
    ]

    for container_key, label in container_pairs:
        try:
            source_container_id = str(
                PalObjects.get_guid(source_save_data[container_key]["value"]["ID"])
            ).lower()
            target_container_id = str(
                PalObjects.get_guid(target_save_data[container_key]["value"]["ID"])
            ).lower()
        except (KeyError, TypeError):
            logger.warning("Could not resolve %s container IDs", label)
            continue

        source_container = _find_character_container(source, source_container_id)
        target_container = _find_character_container(target, target_container_id)

        if not source_container or not target_container:
            logger.warning("Could not find %s containers", label)
            continue

        try:
            target_container["value"]["Slots"]["value"]["values"] = copy.deepcopy(
                source_container["value"]["Slots"]["value"].get("values", [])
            )
        except (KeyError, TypeError) as error:
            logger.warning("Failed to copy %s slots: %s", label, error)
            continue

        # Update each transferred pal's SlotId.ContainerId to point to the target container
        if source_container_id != target_container_id:
            for pal_entry in transferred_pals:
                pal_param = _get_save_parameter(pal_entry)
                if not pal_param or "SlotId" not in pal_param:
                    continue
                try:
                    slot_container_id = str(PalObjects.get_guid(
                        pal_param["SlotId"]["value"]["ContainerId"]["value"]["ID"]
                    )).lower()
                    if slot_container_id == source_container_id:
                        PalObjects.set_value(
                            pal_param["SlotId"]["value"]["ContainerId"]["value"]["ID"],
                            target_container_id,
                        )
                except (KeyError, TypeError):
                    continue


def _update_guild_pal_handles(target, target_guild_id, transferred_pals):
    """Update the guild's individual_character_handle_ids to include transferred pals."""
    transferred_ids = {
        _get_entry_instance_id(entry)
        for entry in transferred_pals
        if _get_entry_instance_id(entry)
    }

    for group_entry in target._group_save_data_map:
        try:
            raw_data = group_entry["value"]["RawData"]["value"]
            if str(raw_data.get("group_id", "")).lower() != target_guild_id:
                continue

            handles = raw_data.get("individual_character_handle_ids", [])
            handles[:] = [
                handle for handle in handles
                if str(handle.get("instance_id", "")).lower() not in transferred_ids
            ]
            for instance_id in transferred_ids:
                handles.append({"guid": EMPTY_UUID, "instance_id": instance_id})
            break
        except (KeyError, TypeError):
            continue


# ---------------------------------------------------------------------------
# Transfer: guild membership
# ---------------------------------------------------------------------------


def _transfer_guild(source, target, source_uid_str, target_uid_str, target_save_data):
    # Find source player's guild membership info
    source_guild_info = None
    for group_entry in source._group_save_data_map:
        try:
            raw_data = group_entry["value"]["RawData"]["value"]
            for player_info in raw_data.get("players", []):
                if str(player_info.get("player_uid", "")).lower() == source_uid_str:
                    source_guild_info = copy.deepcopy(player_info)
                    break
            if source_guild_info:
                break
        except (KeyError, TypeError):
            continue

    if not source_guild_info:
        logger.warning("Source player has no guild membership to transfer")
        return

    source_guild_info["player_uid"] = target_uid_str

    # Try to update existing guild membership in target
    for group_entry in target._group_save_data_map:
        try:
            raw_data = group_entry["value"]["RawData"]["value"]
            for player_info in raw_data.get("players", []):
                if str(player_info.get("player_uid", "")).lower() == target_uid_str:
                    raw_data["players"] = [
                        existing for existing in raw_data["players"]
                        if str(existing.get("player_uid", "")).lower() != target_uid_str
                    ]
                    raw_data["players"].append(source_guild_info)
                    return
        except (KeyError, TypeError):
            continue

    # Target player has no guild — create a new guild entry from source guild structure
    _create_guild_for_player(source, target, source_uid_str, target_uid_str, source_guild_info)


def _create_guild_for_player(source, target, source_uid_str, target_uid_str, player_info):
    """Create a new guild in the target save for a player who has no guild."""
    import uuid as uuid_mod

    # Find source guild to use as template
    source_guild_entry = None
    for group_entry in source._group_save_data_map:
        try:
            raw_data = group_entry["value"]["RawData"]["value"]
            for pi in raw_data.get("players", []):
                if str(pi.get("player_uid", "")).lower() == source_uid_str:
                    source_guild_entry = group_entry
                    break
            if source_guild_entry:
                break
        except (KeyError, TypeError):
            continue

    if not source_guild_entry:
        logger.warning("Could not find source guild to use as template")
        return

    new_guild = copy.deepcopy(source_guild_entry)
    new_guild_id = str(uuid_mod.uuid4()).lower()

    # Reset guild to a clean state with only the transferred player
    try:
        new_guild["key"] = new_guild_id
        raw_data = new_guild["value"]["RawData"]["value"]
        raw_data["group_id"] = new_guild_id
        raw_data["guild_name"] = "Transferred Guild"
        raw_data["admin_player_uid"] = target_uid_str
        raw_data["players"] = [player_info]
        raw_data["base_ids"] = []
        raw_data["base_camp_level"] = 1
        raw_data["map_object_instance_ids_base_camp_points"] = []
        raw_data["individual_character_handle_ids"] = []
        target._group_save_data_map.append(new_guild)
        logger.info("Created new guild %s for transferred player", new_guild_id[:8])
    except (KeyError, TypeError) as error:
        logger.warning("Failed to create guild for transferred player: %s", error)


# ---------------------------------------------------------------------------
# Transfer: timestamps
# ---------------------------------------------------------------------------


def _sync_timestamps(target, target_uid_str):
    try:
        world_save = PalObjects.get_value(
            target._gvas_file.properties["worldSaveData"]
        )
        world_tick = PalObjects.get_value(
            world_save["GameTimeSaveData"]["value"]["RealDateTimeTicks"]
        )
        if not world_tick:
            return
    except (KeyError, TypeError):
        return

    # Update in CharacterSaveParameterMap
    for entry in target._character_save_parameter_map:
        if not _is_player_entry(entry):
            continue
        try:
            if _get_entry_player_uid(entry) == target_uid_str:
                raw_data = entry["value"]["RawData"]["value"]
                raw_data["last_online_real_time"] = world_tick
                save_param = _get_save_parameter(entry)
                if save_param and "LastOnlineRealTime" in save_param:
                    PalObjects.set_value(save_param["LastOnlineRealTime"], world_tick)
                break
        except (KeyError, TypeError):
            continue

    # Update in guild player list
    for group_entry in target._group_save_data_map:
        try:
            raw_data = group_entry["value"]["RawData"]["value"]
            for player_info in raw_data.get("players", []):
                if str(player_info.get("player_uid", "")).lower() == target_uid_str:
                    if "player_info" in player_info:
                        player_info["player_info"]["last_online_real_time"] = world_tick
        except (KeyError, TypeError):
            continue
