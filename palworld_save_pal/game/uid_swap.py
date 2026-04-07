from typing import Any, Callable, Dict, List, Optional
from uuid import UUID

from palworld_save_pal.game.pal_objects import PalObjects
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.uuid import are_equal_uuids

logger = create_logger(__name__)

OWNERSHIP_KEYS = (
    "OwnerPlayerUId",
    "owner_player_uid",
    "build_player_uid",
    "private_lock_player_uid",
)


def _deep_swap_uids(data: Any, old_uid: str, new_uid: str) -> None:
    if isinstance(data, dict):
        for key in OWNERSHIP_KEYS:
            val = data.get(key)
            if val is None:
                continue
            if isinstance(val, dict):
                inner = val.get("value")
                if isinstance(inner, str) and inner.lower() == old_uid:
                    val["value"] = new_uid
                elif isinstance(inner, str) and inner.lower() == new_uid:
                    val["value"] = old_uid
            elif isinstance(val, str):
                if val.lower() == old_uid:
                    data[key] = new_uid
                elif val.lower() == new_uid:
                    data[key] = old_uid
        for v in data.values():
            _deep_swap_uids(v, old_uid, new_uid)
    elif isinstance(data, list):
        for item in data:
            _deep_swap_uids(item, old_uid, new_uid)


async def swap_player_uids(
    save_manager,
    old_player_uid: UUID,
    new_player_uid: UUID,
    ws_callback: Optional[Callable] = None,
) -> Dict[str, Any]:
    async def progress(msg: str):
        logger.info(msg)
        if ws_callback:
            await ws_callback(msg)

    old_uid_str = str(old_player_uid).lower()
    new_uid_str = str(new_player_uid).lower()

    if old_uid_str == new_uid_str:
        return {"error": "Both players are the same."}

    await progress("Validating players...")

    # Validate both players have file refs (i.e. exist as players with .sav files)
    if old_player_uid not in save_manager._player_file_refs:
        return {"error": f"Player {old_player_uid} not found in save."}
    if new_player_uid not in save_manager._player_file_refs:
        return {"error": f"Player {new_player_uid} not found in save."}

    # Load player GVAS files if not already loaded
    if old_player_uid not in save_manager._player_gvas_files:
        await save_manager.load_player_on_demand(old_player_uid)
    if new_player_uid not in save_manager._player_gvas_files:
        await save_manager.load_player_on_demand(new_player_uid)

    old_gvas = save_manager._player_gvas_files.get(old_player_uid)
    new_gvas = save_manager._player_gvas_files.get(new_player_uid)
    if not old_gvas or not new_gvas:
        return {"error": "Failed to load player save files."}

    # Validate level >= 2 using summaries (level is in CharacterSaveParameterMap, not player GVAS)
    old_summary = save_manager._player_summaries.get(old_player_uid)
    new_summary = save_manager._player_summaries.get(new_player_uid)
    old_level = (old_summary.level or 1) if old_summary else 1
    new_level = (new_summary.level or 1) if new_summary else 1
    if old_level < 2 or new_level < 2:
        return {
            "error": f"Both players must be at least level 2. "
            f"Player 1 is level {old_level}, Player 2 is level {new_level}."
        }

    # Get save data from player GVAS files
    old_save_data = PalObjects.get_value(old_gvas.sav.properties["SaveData"])
    new_save_data = PalObjects.get_value(new_gvas.sav.properties["SaveData"])

    # Get instance IDs
    old_instance_id = PalObjects.get_guid(
        old_save_data["IndividualId"]["value"]["InstanceId"]
    )
    new_instance_id = PalObjects.get_guid(
        new_save_data["IndividualId"]["value"]["InstanceId"]
    )

    await progress("Swapping player UIDs in save data...")

    # Step 1: Swap PlayerUId in player GVAS files
    PalObjects.set_value(old_save_data["PlayerUId"], new_uid_str)
    PalObjects.set_value(
        old_save_data["IndividualId"]["value"]["PlayerUId"], new_uid_str
    )
    PalObjects.set_value(new_save_data["PlayerUId"], old_uid_str)
    PalObjects.set_value(
        new_save_data["IndividualId"]["value"]["PlayerUId"], old_uid_str
    )

    await progress("Swapping UIDs in character save parameter map...")

    # Step 2: Swap PlayerUId in CharacterSaveParameterMap
    for entry in save_manager._character_save_parameter_map:
        try:
            instance_id = PalObjects.get_guid(entry["key"]["InstanceId"])
            if are_equal_uuids(instance_id, old_instance_id):
                PalObjects.set_value(entry["key"]["PlayerUId"], new_uid_str)
            elif are_equal_uuids(instance_id, new_instance_id):
                PalObjects.set_value(entry["key"]["PlayerUId"], old_uid_str)
        except (KeyError, TypeError):
            continue

    await progress("Swapping UIDs in guild data...")

    # Step 3: Swap in guild data
    for group_entry in save_manager._group_save_data_map:
        try:
            group_type_val = PalObjects.get_enum_property(
                PalObjects.get_nested(group_entry, "value", "GroupType")
            )
            if "Guild" not in str(group_type_val):
                continue
        except (KeyError, TypeError):
            continue

        try:
            raw_data = group_entry["value"]["RawData"]["value"]
        except (KeyError, TypeError):
            continue

        # Swap in individual_character_handle_ids
        for handle in raw_data.get("individual_character_handle_ids", []):
            handle_guid = handle.get("guid")
            handle_inst = handle.get("instance_id")
            if handle_inst and are_equal_uuids(handle_inst, old_instance_id):
                handle["guid"] = new_uid_str
            elif handle_inst and are_equal_uuids(handle_inst, new_instance_id):
                handle["guid"] = old_uid_str

        # Swap admin_player_uid
        admin_uid = raw_data.get("admin_player_uid")
        if admin_uid:
            admin_str = str(admin_uid).lower() if not isinstance(admin_uid, str) else admin_uid.lower()
            if admin_str == old_uid_str:
                raw_data["admin_player_uid"] = new_uid_str
            elif admin_str == new_uid_str:
                raw_data["admin_player_uid"] = old_uid_str

        # Swap in players list
        for player_entry in raw_data.get("players", []):
            p_uid = player_entry.get("player_uid")
            if p_uid:
                p_str = str(p_uid).lower() if not isinstance(p_uid, str) else p_uid.lower()
                if p_str == old_uid_str:
                    player_entry["player_uid"] = new_uid_str
                elif p_str == new_uid_str:
                    player_entry["player_uid"] = old_uid_str

    await progress("Swapping ownership references across all data...")

    # Step 4: Deep recursive swap across all level data
    _deep_swap_uids(save_manager._gvas_file.properties, old_uid_str, new_uid_str)

    await progress("Swapping player file references...")

    # Step 5: Swap DPS data if present
    old_dps = old_gvas.dps
    new_dps = new_gvas.dps
    # Swap the DPS references between the two PlayerGvasFiles
    old_gvas.dps = new_dps
    new_gvas.dps = old_dps

    # Step 6: Swap player GVAS files and file refs in SaveManager dicts
    save_manager._player_gvas_files[old_player_uid] = new_gvas
    save_manager._player_gvas_files[new_player_uid] = old_gvas

    old_file_ref = save_manager._player_file_refs.get(old_player_uid)
    new_file_ref = save_manager._player_file_refs.get(new_player_uid)
    save_manager._player_file_refs[old_player_uid] = new_file_ref
    save_manager._player_file_refs[new_player_uid] = old_file_ref

    await progress("Rebuilding caches...")

    # Step 7: Invalidate all caches and rebuild summaries
    save_manager.invalidate_performance_caches()
    save_manager._players.clear()
    save_manager._guilds.clear()
    save_manager._loaded_players.clear()
    save_manager._loaded_guilds.clear()
    save_manager._extract_player_summaries()
    save_manager._extract_guild_summaries()

    await progress("UID swap complete!")

    return {"success": True}
