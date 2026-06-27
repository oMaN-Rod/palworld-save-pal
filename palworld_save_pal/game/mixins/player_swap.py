from typing import TYPE_CHECKING, Any, Callable, Dict, Optional
from uuid import UUID

if TYPE_CHECKING:
    from palworld_save_pal.game.mixins._save_manager_protocol import (
        SaveManagerProtocol,
    )

    _Base = SaveManagerProtocol
else:
    _Base = object

from palworld_save_pal.game.pal_objects import PalObjects
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.uuid import are_equal_uuids

logger = create_logger(__name__)

_OWNERSHIP_KEYS = (
    "OwnerPlayerUId",
    "owner_player_uid",
    "build_player_uid",
    "private_lock_player_uid",
)


def _swap_uid_value(current_value: str, old_uid: str, new_uid: str) -> Optional[str]:
    """Return the swapped UID if current_value matches old or new, else None."""
    normalized = current_value.lower()
    if normalized == old_uid:
        return new_uid
    if normalized == new_uid:
        return old_uid
    return None


def _deep_swap_uids(data: Any, old_uid: str, new_uid: str) -> None:
    """Recursively swap ownership UIDs throughout a nested data structure."""
    if isinstance(data, dict):
        for key in _OWNERSHIP_KEYS:
            value = data.get(key)
            if value is None:
                continue
            if isinstance(value, dict):
                inner = value.get("value")
                if isinstance(inner, str):
                    swapped = _swap_uid_value(inner, old_uid, new_uid)
                    if swapped:
                        value["value"] = swapped
            elif isinstance(value, str):
                swapped = _swap_uid_value(value, old_uid, new_uid)
                if swapped:
                    data[key] = swapped
        for child in data.values():
            _deep_swap_uids(child, old_uid, new_uid)
    elif isinstance(data, list):
        for item in data:
            _deep_swap_uids(item, old_uid, new_uid)


class PlayerSwapMixin(_Base):
    async def swap_player_uids(
        self,
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
        validation_error = await self._validate_swap_players(
            old_player_uid, new_player_uid
        )
        if validation_error:
            return validation_error

        old_gvas = self._player_gvas_files[old_player_uid]
        new_gvas = self._player_gvas_files[new_player_uid]

        old_save_data = PalObjects.get_value(old_gvas.sav.properties["SaveData"])
        new_save_data = PalObjects.get_value(new_gvas.sav.properties["SaveData"])

        old_instance_id = PalObjects.get_guid(
            old_save_data["IndividualId"]["value"]["InstanceId"]
        )
        new_instance_id = PalObjects.get_guid(
            new_save_data["IndividualId"]["value"]["InstanceId"]
        )

        await progress("Swapping player UIDs in save data...")
        self._swap_player_gvas_uids(
            old_save_data, new_save_data, old_uid_str, new_uid_str
        )

        await progress("Swapping UIDs in character save parameter map...")
        self._swap_character_save_parameters(
            old_uid_str, new_uid_str, old_instance_id, new_instance_id
        )

        await progress("Swapping UIDs in guild data...")
        self._swap_guild_member_uids(
            old_uid_str, new_uid_str, old_instance_id, new_instance_id
        )

        await progress("Swapping ownership references across all data...")
        _deep_swap_uids(self._gvas_file.properties, old_uid_str, new_uid_str)

        await progress("Swapping player file references...")
        self._swap_player_file_refs(old_player_uid, new_player_uid)

        await progress("Rebuilding caches...")
        self.rebuild_player_caches()

        await progress("UID swap complete!")
        return {"success": True}

    async def _validate_swap_players(
        self, old_player_uid: UUID, new_player_uid: UUID
    ) -> Optional[Dict[str, Any]]:
        """Validate both players exist and meet level requirements. Returns error dict or None."""
        if old_player_uid not in self._player_file_refs:
            return {"error": f"Player {old_player_uid} not found in save."}
        if new_player_uid not in self._player_file_refs:
            return {"error": f"Player {new_player_uid} not found in save."}

        if old_player_uid not in self._player_gvas_files:
            await self.load_player_on_demand(old_player_uid)
        if new_player_uid not in self._player_gvas_files:
            await self.load_player_on_demand(new_player_uid)

        if not self._player_gvas_files.get(old_player_uid) or not self._player_gvas_files.get(new_player_uid):
            return {"error": "Failed to load player save files."}

        old_summary = self._player_summaries.get(old_player_uid)
        new_summary = self._player_summaries.get(new_player_uid)
        old_level = (old_summary.level or 1) if old_summary else 1
        new_level = (new_summary.level or 1) if new_summary else 1

        if old_level < 2 or new_level < 2:
            return {
                "error": f"Both players must be at least level 2. "
                f"Player 1 is level {old_level}, Player 2 is level {new_level}."
            }

        return None

    def _swap_player_gvas_uids(
        self,
        old_save_data: Dict[str, Any],
        new_save_data: Dict[str, Any],
        old_uid_str: str,
        new_uid_str: str,
    ) -> None:
        """Swap PlayerUId fields in both player GVAS save data structures."""
        PalObjects.set_value(old_save_data["PlayerUId"], new_uid_str)
        PalObjects.set_value(
            old_save_data["IndividualId"]["value"]["PlayerUId"], new_uid_str
        )
        PalObjects.set_value(new_save_data["PlayerUId"], old_uid_str)
        PalObjects.set_value(
            new_save_data["IndividualId"]["value"]["PlayerUId"], old_uid_str
        )

    def _swap_character_save_parameters(
        self,
        old_uid_str: str,
        new_uid_str: str,
        old_instance_id: UUID,
        new_instance_id: UUID,
    ) -> None:
        """Swap PlayerUId in CharacterSaveParameterMap entries matching instance IDs."""
        for entry in self._character_save_parameter_map:
            try:
                instance_id = PalObjects.get_guid(entry["key"]["InstanceId"])
                if are_equal_uuids(instance_id, old_instance_id):
                    PalObjects.set_value(entry["key"]["PlayerUId"], new_uid_str)
                elif are_equal_uuids(instance_id, new_instance_id):
                    PalObjects.set_value(entry["key"]["PlayerUId"], old_uid_str)
            except (KeyError, TypeError):
                continue

    def _swap_guild_member_uids(
        self,
        old_uid_str: str,
        new_uid_str: str,
        old_instance_id: UUID,
        new_instance_id: UUID,
    ) -> None:
        """Swap player UIDs in all guild data structures."""
        for group_entry in self._group_save_data_map:
            try:
                group_type = PalObjects.get_enum_property(
                    PalObjects.get_nested(group_entry, "value", "GroupType")
                )
                if "Guild" not in str(group_type):
                    continue
            except (KeyError, TypeError):
                continue

            raw_data = PalObjects.get_nested(
                group_entry, "value", "RawData", "value"
            )
            if not raw_data:
                continue

            self._swap_guild_character_handles(
                raw_data, old_uid_str, new_uid_str, old_instance_id, new_instance_id
            )
            self._swap_guild_admin_uid(raw_data, old_uid_str, new_uid_str)
            self._swap_guild_player_list(raw_data, old_uid_str, new_uid_str)

    def _swap_guild_character_handles(
        self,
        raw_data: Dict[str, Any],
        old_uid_str: str,
        new_uid_str: str,
        old_instance_id: UUID,
        new_instance_id: UUID,
    ) -> None:
        for handle in raw_data.get("individual_character_handle_ids", []):
            handle_instance = PalObjects.as_uuid(handle.get("instance_id"))
            if handle_instance and are_equal_uuids(handle_instance, old_instance_id):
                handle["guid"] = new_uid_str
            elif handle_instance and are_equal_uuids(handle_instance, new_instance_id):
                handle["guid"] = old_uid_str

    def _swap_guild_admin_uid(
        self,
        raw_data: Dict[str, Any],
        old_uid_str: str,
        new_uid_str: str,
    ) -> None:
        admin_uid = PalObjects.as_uuid(raw_data.get("admin_player_uid"))
        if not admin_uid:
            return
        if are_equal_uuids(admin_uid, old_uid_str):
            raw_data["admin_player_uid"] = new_uid_str
        elif are_equal_uuids(admin_uid, new_uid_str):
            raw_data["admin_player_uid"] = old_uid_str

    def _swap_guild_player_list(
        self,
        raw_data: Dict[str, Any],
        old_uid_str: str,
        new_uid_str: str,
    ) -> None:
        for player_entry in raw_data.get("players", []):
            player_uid = PalObjects.as_uuid(player_entry.get("player_uid"))
            if not player_uid:
                continue
            if are_equal_uuids(player_uid, old_uid_str):
                player_entry["player_uid"] = new_uid_str
            elif are_equal_uuids(player_uid, new_uid_str):
                player_entry["player_uid"] = old_uid_str

    def _swap_player_file_refs(
        self, old_player_uid: UUID, new_player_uid: UUID
    ) -> None:
        """Swap DPS data and file references between two players."""
        old_gvas = self._player_gvas_files[old_player_uid]
        new_gvas = self._player_gvas_files[new_player_uid]

        old_gvas.dps, new_gvas.dps = new_gvas.dps, old_gvas.dps

        self._player_gvas_files[old_player_uid] = new_gvas
        self._player_gvas_files[new_player_uid] = old_gvas

        old_file_ref = self._player_file_refs.get(old_player_uid)
        new_file_ref = self._player_file_refs.get(new_player_uid)
        self._player_file_refs[old_player_uid] = new_file_ref
        self._player_file_refs[new_player_uid] = old_file_ref

    def rebuild_player_caches(self) -> None:
        """Invalidate all caches and rebuild player/guild summaries."""
        self.invalidate_performance_caches()
        self._players.clear()
        self._guilds.clear()
        self._loaded_players.clear()
        self._loaded_guilds.clear()
        self._extract_player_summaries()
        self._extract_guild_summaries()
