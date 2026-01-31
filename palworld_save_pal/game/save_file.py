import copy
from concurrent.futures import ThreadPoolExecutor, as_completed
from enum import Enum
import json
import os
import time
from typing import Any, Dict, List, Optional, Tuple, Union
from uuid import UUID
import uuid
from pydantic import BaseModel, ConfigDict, PrivateAttr

from palworld_save_tools.archive import (
    FArchiveReader,
    FArchiveWriter,
)
from palworld_save_tools.gvas import GvasFile
from palworld_save_tools.json_tools import CustomEncoder
from palworld_save_tools.palsav import compress_gvas_to_sav, decompress_sav_to_gvas
from palworld_save_tools.paltypes import (
    DISABLED_PROPERTIES,
    PALWORLD_CUSTOM_PROPERTIES,
    PALWORLD_TYPE_HINTS,
)

from palworld_save_pal.dto.pal import PalDTO
from palworld_save_pal.dto.guild import GuildDTO
from palworld_save_pal.dto.summary import PlayerSummary, GuildSummary
from palworld_save_pal.game.base import Base
from palworld_save_pal.game.guild import Guild
from palworld_save_pal.game.pal import Pal
from palworld_save_pal.game.pal_objects import PalObjects
from palworld_save_pal.game.enum import GroupType, PalGender
from palworld_save_pal.utils.indexed_collection import IndexedCollection
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.game.player import Player, PlayerGvasFiles
from palworld_save_pal.dto.player import PlayerDTO
from palworld_save_pal.utils.uuid import are_equal_uuids, is_empty_uuid
from palworld_save_pal.utils.json_manager import sanitize_string

logger = create_logger(__name__)


def skip_decode(reader: FArchiveReader, type_name: str, size: int, path: str):
    if type_name == "ArrayProperty":
        array_type = reader.fstring()
        value = {
            "skip_type": type_name,
            "array_type": array_type,
            "id": reader.optional_guid(),
            "value": reader.read(size),
        }
    elif type_name == "MapProperty":
        key_type = reader.fstring()
        value_type = reader.fstring()
        _id = reader.optional_guid()
        value = {
            "skip_type": type_name,
            "key_type": key_type,
            "value_type": value_type,
            "id": _id,
            "value": reader.read(size),
        }
    elif type_name == "StructProperty":
        value = {
            "skip_type": type_name,
            "struct_type": reader.fstring(),
            "struct_id": reader.guid(),
            "id": reader.optional_guid(),
            "value": reader.read(size),
        }
    else:
        raise ValueError(
            f"Expected ArrayProperty or MapProperty or StructProperty, got {type_name} in {path}"
        )
    return value


def skip_encode(writer: FArchiveWriter, property_type: str, properties: dict) -> int:
    if "skip_type" not in properties:
        if (
            properties["custom_type"] in PALWORLD_CUSTOM_PROPERTIES
            and PALWORLD_CUSTOM_PROPERTIES[properties["custom_type"]] is not None
        ):
            return PALWORLD_CUSTOM_PROPERTIES[properties["custom_type"]][1](
                writer, property_type, properties
            )
        else:
            # Never be run to here
            return writer.property_inner(writer, property_type, properties)
    if property_type == "ArrayProperty":
        del properties["custom_type"]
        del properties["skip_type"]
        writer.fstring(properties["array_type"])
        writer.optional_guid(properties.get("id", None))
        writer.write(properties["value"])
        return len(properties["value"])
    elif property_type == "MapProperty":
        del properties["custom_type"]
        del properties["skip_type"]
        writer.fstring(properties["key_type"])
        writer.fstring(properties["value_type"])
        writer.optional_guid(properties.get("id", None))
        writer.write(properties["value"])
        return len(properties["value"])
    elif property_type == "StructProperty":
        del properties["custom_type"]
        del properties["skip_type"]
        writer.fstring(properties["struct_type"])
        writer.guid(properties["struct_id"])
        writer.optional_guid(properties.get("id", None))
        writer.write(properties["value"])
        return len(properties["value"])
    else:
        raise ValueError(
            f"Expected ArrayProperty or MapProperty or StructProperty, got {property_type}"
        )


CUSTOM_PROPERTIES = {k: v for k, v in PALWORLD_CUSTOM_PROPERTIES.items()}
CUSTOM_PROPERTIES[".worldSaveData.FoliageGridSaveDataMap"] = (skip_decode, skip_encode)
CUSTOM_PROPERTIES[".worldSaveData.MapObjectSpawnerInStageSaveData"] = (
    skip_decode,
    skip_encode,
)
CUSTOM_PROPERTIES[".worldSaveData.DungeonSaveData"] = (skip_decode, skip_encode)
CUSTOM_PROPERTIES[".worldSaveData.EnemyCampSaveData"] = (skip_decode, skip_encode)
CUSTOM_PROPERTIES[".worldSaveData.InvaderSaveData"] = (skip_decode, skip_encode)
CUSTOM_PROPERTIES[".worldSaveData.DungeonPointMarkerSaveData"] = (
    skip_decode,
    skip_encode,
)
CUSTOM_PROPERTIES[".worldSaveData.GameTimeSaveData"] = (skip_decode, skip_encode)
CUSTOM_PROPERTIES[".worldSaveData.OilrigSaveData"] = (skip_decode, skip_encode)
CUSTOM_PROPERTIES[".worldSaveData.SupplySaveData"] = (skip_decode, skip_encode)
CUSTOM_PROPERTIES[".worldSaveData.BaseCampSaveData.Value.ModuleMap"] = (
    skip_decode,
    skip_encode,
)


class SaveType(int, Enum):
    STEAM = 0
    GAMEPASS = 1


class SaveFile(BaseModel):
    level_sav_path: str = ""
    size: int = 0
    world_name: str = ""

    model_config = ConfigDict(arbitrary_types_allowed=True)

    _players: Dict[UUID, Player] = PrivateAttr(default_factory=dict)
    _pals: Dict[UUID, Pal] = PrivateAttr(default_factory=dict)
    _guilds: Dict[UUID, Guild] = PrivateAttr(default_factory=dict)
    _gps_pals: Optional[Dict[int, Pal]] = PrivateAttr(default_factory=dict)

    _gvas_file: Optional[GvasFile] = PrivateAttr(default=None)
    _level_meta_gvas_file: Optional[GvasFile] = PrivateAttr(default=None)
    _player_gvas_files: Dict[UUID, PlayerGvasFiles] = PrivateAttr(default_factory=dict)
    _gps_gvas_file: Optional[GvasFile] = PrivateAttr(default=None)

    _character_save_parameter_map: List[Dict[str, Any]] = PrivateAttr(
        default_factory=list
    )
    _character_save_parameters: Optional[IndexedCollection[UUID, Dict[str, Any]]] = (
        PrivateAttr(default=None)
    )
    _item_container_save_data: List[Dict[str, Any]] = PrivateAttr(default_factory=list)
    _item_containers: Optional[IndexedCollection[UUID, Dict[str, Any]]] = PrivateAttr(
        default=None
    )
    _dynamic_item_save_data: List[Dict[str, Any]] = PrivateAttr(default_factory=list)
    _dynamic_items: Optional[IndexedCollection[UUID, Dict[str, Any]]] = PrivateAttr(
        default=None
    )
    _character_container_save_data: List[Dict[str, Any]] = PrivateAttr(
        default_factory=list
    )
    _character_containers: Optional[IndexedCollection[UUID, Dict[str, Any]]] = (
        PrivateAttr(default=None)
    )
    _group_save_data_map: List[Dict[str, Any]] = PrivateAttr(default_factory=list)
    _base_camp_save_data_map: List[Dict[str, Any]] = PrivateAttr(default_factory=list)
    _map_object_save_data: List[Dict[str, Any]] = PrivateAttr(default_factory=list)
    _guild_extra_save_data_map: List[Dict[str, Any]] = PrivateAttr(default_factory=list)

    _player_summaries: Dict[UUID, PlayerSummary] = PrivateAttr(default_factory=dict)
    _guild_summaries: Dict[UUID, GuildSummary] = PrivateAttr(default_factory=dict)
    _loaded_players: set = PrivateAttr(default_factory=set)
    _loaded_guilds: set = PrivateAttr(default_factory=set)

    _player_file_refs: Dict[UUID, Dict[str, Any]] = PrivateAttr(default_factory=dict)

    _pal_owner_counts_cache: Optional[Dict[UUID, int]] = PrivateAttr(default=None)
    _player_guild_map_cache: Optional[Dict[UUID, UUID]] = PrivateAttr(default=None)
    _map_object_index: Optional[Dict[UUID, List[Dict[str, Any]]]] = PrivateAttr(
        default=None
    )

    def _should_delete_map_object(
        self, map_object: dict, guild_id: UUID | None, player_ids: List[UUID]
    ) -> bool:
        """
        Determine if a map object should be deleted based on guild and player ownership.

        Args:
            map_object: The map object data
            guild_id: The guild ID to check against
            player_ids: List of player UUIDs to check against

        Returns:
            bool: True if the map object should be deleted, False otherwise
        """
        raw_data = map_object["Model"]["value"]["RawData"]["value"]
        group_id = PalObjects.as_uuid(raw_data.get("group_id_belong_to"))
        build_player_uid = PalObjects.as_uuid(raw_data.get("build_player_uid"))

        # Check guild ownership
        if guild_id and are_equal_uuids(group_id, guild_id):
            return True

        # Check if any player in the list is the builder
        if any(
            are_equal_uuids(build_player_uid, player_id) for player_id in player_ids
        ):
            return True

        # Handle edge cases
        if "ConcreteModel" in map_object:
            concrete_model_raw_data = map_object["ConcreteModel"]["value"]["RawData"][
                "value"
            ]
            private_lock_player_uid = PalObjects.as_uuid(
                concrete_model_raw_data.get("private_lock_player_uid")
            )

            # Check if any player in the list is the private lock owner
            if any(
                are_equal_uuids(private_lock_player_uid, player_id)
                for player_id in player_ids
            ):
                return True

            # Check trade info sellers
            for trade_info in concrete_model_raw_data.get("trade_infos", []):
                seller_player_uid = PalObjects.as_uuid(
                    trade_info.get("seller_player_uid")
                )
                if any(
                    are_equal_uuids(seller_player_uid, player_id)
                    for player_id in player_ids
                ):
                    return True

            # Check password lock module
            for module in concrete_model_raw_data.get("ModuleMap", {}).get("value", []):
                if (
                    module["key"]
                    == "EPalMapObjectConcreteModelModuleType::PasswordLock"
                ):
                    for player_info in module["value"]["RawData"]["value"].get(
                        "player_infos", []
                    ):
                        player_uid = PalObjects.as_uuid(player_info.get("player_uid"))
                        if any(
                            are_equal_uuids(player_uid, player_id)
                            for player_id in player_ids
                        ):
                            return True

        return False

    async def _delete_player_and_pals(
        self, player_id: UUID, ws_callback
    ) -> Tuple[List[UUID], List[UUID]] | None:
        player = self._players[player_id]
        logger.debug("Deleting player %s with %s pals", player_id, len(player.pals))
        await ws_callback(
            f"Deleting player {player.nickname} with {len(player.pals)} pals"
        )

        # Container ids to delete
        container_ids_to_delete = [
            player.common_container.id,
            player.essential_container.id,
            player.weapon_load_out_container.id,
            player.player_equipment_armor_container.id,
            player.food_equip_container.id,
        ]

        # Character container ids to delete
        character_container_ids_to_delete = [
            player.otomo_container_id,
            player.pal_box_id,
        ]

        await ws_callback(
            f"Deleting {len(player.pal_box.slots)} pals of player {player.nickname} from PalBox"
        )
        for pal_slot in list(player.pal_box.slots):
            self._delete_pal_by_id(pal_slot.pal_id)

        await ws_callback(
            f"Deleting {len(player.party.slots)} pals of player {player.nickname} from Party"
        )
        for pal_slot in list(player.party.slots):
            self._delete_pal_by_id(pal_slot.pal_id)

        # Delete the player
        del self._players[player_id]

        # Delete player parameters
        self._character_save_parameter_map[:] = [
            entry
            for entry in self._character_save_parameter_map
            if not are_equal_uuids(
                PalObjects.get_guid(PalObjects.get_nested(entry, "key", "PlayerUId")),
                player_id,
            )
        ]
        self.invalidate_performance_caches()

        # Delete player save file
        del self._player_gvas_files[player_id]

        return container_ids_to_delete, character_container_ids_to_delete

    async def delete_player(self, player_id: UUID, ws_callback) -> bool:
        player = self._players.get(player_id)
        if not player:
            raise ValueError(f"Player {player_id} not found in the save file.")

        player_guild = self._player_guild(player_id)
        if player_guild and are_equal_uuids(player_guild.admin_player_uid, player_id):
            logger.warning(
                "Cannot delete admin player %s from guild %s",
                player_id,
                player_guild.id,
            )
            return False
        elif player_guild:
            logger.debug(
                "Deleting player %s (%s) from guild %s (%s)",
                player.nickname,
                player.uid,
                player_guild.name,
                player_guild.id,
            )
            await ws_callback(
                f"Deleting player {player.nickname} from guild {player_guild.name}"
            )
            player_guild.delete_player(player_id)

        (
            container_ids_to_delete,
            character_container_ids_to_delete,
        ) = await self._delete_player_and_pals(player_id, ws_callback)

        # Delete all map objects owned by guild or player in guild
        await ws_callback(f"Deleting map objects of player {player.nickname}")
        self._map_object_save_data["values"][:] = [
            obj
            for obj in self._map_object_save_data["values"]
            if not self._should_delete_map_object(obj, None, [player_id])
        ]

        # Delete player items
        await ws_callback(f"Deleting item containers of player {player.nickname}")
        self._delete_item_containers(player_id, container_ids_to_delete)

        # Delete character containers
        await ws_callback(f"Deleting character containers of player {player.nickname}")
        self._delete_character_containers(character_container_ids_to_delete)
        return True

    async def delete_guild_and_players(self, guild_id: UUID, ws_callback) -> None:
        guild = self._guilds.get(guild_id)
        if not guild:
            raise ValueError(f"Guild {guild_id} not found in the save file.")
        logger.debug("Deleting guild %s with %s players", guild_id, len(guild.players))
        await ws_callback(
            f"Deleting guild {guild.name} with {len(guild.players)} players"
        )
        # Container ids to delete
        container_ids_to_delete = []

        # Character container ids to delete
        character_container_ids_to_delete = []

        # Delete all map objects owned by guild or player in guild
        self._map_object_save_data["values"][:] = [
            obj
            for obj in self._map_object_save_data["values"]
            if not self._should_delete_map_object(obj, guild_id, guild.players)
        ]

        # Delete all players in the guild
        for player_id in guild.players:
            if player_id not in self._players:
                continue
            container_ids, character_container_ids = await self._delete_player_and_pals(
                player_id, ws_callback
            )
            container_ids_to_delete.extend(container_ids)
            character_container_ids_to_delete.extend(character_container_ids)

        # Remove guild extra save data
        self._guild_extra_save_data_map[:] = [
            entry
            for entry in self._guild_extra_save_data_map
            if not are_equal_uuids(entry["key"], guild_id)
        ]

        # Delete all bases in the guild
        for base_id, base in guild.bases.items():
            logger.debug("Deleting base %s", base_id)
            await ws_callback(f"Deleting base {base.id}")
            container_ids_to_delete.extend(list(base.storage_containers.keys()))
            character_container_ids_to_delete.append(base.container_id)

            self.delete_guild_pals(guild_id, base_id, list(base.pals.keys()))

            self._base_camp_save_data_map[:] = [
                base
                for base in self._base_camp_save_data_map
                if not are_equal_uuids(PalObjects.get_nested(base, "key"), base_id)
            ]

        # Delete player items and guild items
        await ws_callback(f"Deleting item containers of guild {guild.name}")
        self._delete_item_containers(guild_id, container_ids_to_delete)
        self._delete_item_containers(player_id, container_ids_to_delete)

        # Delete character containers
        await ws_callback(f"Deleting character containers of guild {guild.name}")
        self._delete_character_containers(character_container_ids_to_delete)

        # Delete the guild
        self._group_save_data_map[:] = [
            group
            for group in self._group_save_data_map
            if not are_equal_uuids(PalObjects.get_nested(group, "key"), guild_id)
        ]
        del self._guilds[guild_id]

    def _delete_item_containers(
        self, target_id: UUID, container_ids_to_delete: List[UUID]
    ) -> None:
        logger.debug(
            "Deleting %s item containers for %s",
            len(container_ids_to_delete),
            target_id,
        )
        item_containers = self._get_item_containers()
        for container_id in container_ids_to_delete:
            entry = item_containers.get(container_id)
            if entry:
                self._delete_dynamic_items(entry)
                item_containers.remove_by_key(container_id)
            else:
                # Fallback: search by GroupId if not found by container_id
                for entry in item_containers.data:
                    if are_equal_uuids(
                        PalObjects.get_guid(
                            PalObjects.get_nested(
                                entry, "value", "BelongInfo", "value", "GroupId"
                            )
                        ),
                        target_id,
                    ):
                        self._delete_dynamic_items(entry)
                        item_containers.remove(entry)
                        break

    def _delete_dynamic_items(self, item_container: UUID) -> None:
        slots = PalObjects.get_array_property(
            PalObjects.get_nested(item_container, "value", "Slots")
        )
        dynamic_items = self._get_dynamic_items()
        for slot in slots:
            raw_data = PalObjects.get_value(slot["RawData"])
            local_id = PalObjects.as_uuid(
                PalObjects.get_nested(
                    raw_data, "item", "dynamic_id", "local_id_in_created_world"
                )
            )
            if local_id and not is_empty_uuid(local_id):
                logger.debug("Deleting dynamic item %s", local_id)
                if dynamic_items.remove_by_key(local_id):
                    logger.debug("Deleted dynamic item %s", local_id)

    def _delete_character_containers(self, container_ids_to_delete: List[UUID]) -> None:
        logger.debug("Deleting character containers for %s", container_ids_to_delete)
        character_containers = self._get_character_containers()
        for container_id in container_ids_to_delete:
            character_containers.remove_by_key(container_id)

    def add_player_pal(
        self,
        player_id: UUID,
        character_id: str,
        nickname: str,
        container_id: UUID,
        storage_slot: Union[int | None] = None,
    ) -> Optional[Pal]:
        player = self._players.get(player_id)
        if not player:
            raise ValueError(f"Player {player_id} not found in the save file.")

        new_pal = player.add_pal(character_id, nickname, container_id, storage_slot)
        if new_pal is None:
            return
        self._get_character_save_parameters().add(new_pal.character_save)
        self._pals[new_pal.instance_id] = new_pal
        self.invalidate_performance_caches()
        return new_pal

    def add_player_pal_from_dto(
        self,
        player_id: UUID,
        pal_dto: PalDTO,
        container_id: UUID,
        storage_slot: Union[int | None] = None,
    ) -> Optional[Pal]:
        """Add a pal to player with complete data preservation."""
        player = self._players.get(player_id)
        if not player:
            raise ValueError(f"Player {player_id} not found in the save file.")

        new_pal = player.add_pal_from_dto(pal_dto, container_id, storage_slot)
        if new_pal is None:
            return
        self._get_character_save_parameters().add(new_pal.character_save)
        self._pals[new_pal.instance_id] = new_pal
        self.invalidate_performance_caches()
        return new_pal

    def add_player_dps_pal(
        self,
        player_id: UUID,
        character_id: str,
        nickname: str,
        storage_slot: Optional[int] = None,
    ) -> Optional[Tuple[int, Pal]]:
        player = self._players.get(player_id)
        if not player:
            raise ValueError(f"Player {player_id} not found in the save file.")

        slot_idx, new_pal = player.add_dps_pal(character_id, nickname, storage_slot)
        if new_pal is None:
            return
        return slot_idx, new_pal

    def add_player_dps_pal_from_dto(
        self,
        player_id: UUID,
        pal_dto: PalDTO,
        storage_slot: Optional[int] = None,
    ) -> Optional[Tuple[int, Pal]]:
        """Add a DPS pal to player with complete data preservation."""
        player = self._players.get(player_id)
        if not player:
            raise ValueError(f"Player {player_id} not found in the save file.")

        slot_idx, new_pal = player.add_dps_pal_from_dto(pal_dto, storage_slot)
        if new_pal is None:
            return
        return slot_idx, new_pal

    def _find_first_empty_gps_slot(self) -> Optional[int]:
        if not self._gps_gvas_file:
            raise ValueError("GPS Gvas file is not initialized.")

        save_parameter_array = PalObjects.get_array_property(
            self._gps_gvas_file.properties["SaveParameterArray"]
        )
        for index, entry in enumerate(save_parameter_array):
            save_parameter = PalObjects.get_value(entry["SaveParameter"])
            character_id = (
                PalObjects.get_value(save_parameter["CharacterID"])
                if save_parameter and "CharacterID" in save_parameter
                else None
            )
            if character_id is None or character_id == "None":
                logger.debug(
                    "Found empty GPS slot at index %s",
                    index,
                )
                return index
        return None

    def add_gps_pal(
        self,
        character_id: str,
        nickname: str,
        storage_slot: Optional[int] = None,
    ) -> Optional[Tuple[Pal, int]]:
        if not self._gps_gvas_file:
            raise ValueError("GPS Gvas file is not initialized.")

        slot_idx = (
            storage_slot
            if storage_slot is not None
            else self._find_first_empty_gps_slot()
        )
        if slot_idx is None:
            logger.error("No empty GPS slot found.")
            return None
        pal_data = PalObjects.get_array_property(
            self._gps_gvas_file.properties["SaveParameterArray"]
        )[slot_idx]

        pal = Pal(data=pal_data, dps=True)
        pal.reset()
        pal.owner_uid = PalObjects.EMPTY_UUID
        pal.instance_id = uuid.uuid4()
        pal.character_id = character_id
        pal.nickname = nickname
        pal.filtered_nickname = nickname
        pal.storage_id = PalObjects.EMPTY_UUID
        pal.storage_slot = 0
        pal.gender = PalGender.FEMALE
        pal.populate_status_point_lists()
        pal.hp = pal.max_hp
        if not self._gps_pals:
            self._gps_pals = {}
        self._gps_pals[slot_idx] = pal
        return pal, slot_idx

    def add_gps_pal_from_dto(
        self,
        pal_dto: PalDTO,
        storage_slot: Optional[int] = None,
    ) -> Optional[Tuple[int, Pal]]:
        """Add a GPS pal with complete data preservation."""
        if not self._gps_gvas_file:
            raise ValueError("GPS Gvas file is not initialized.")

        slot_idx = (
            storage_slot
            if storage_slot is not None
            else self._find_first_empty_gps_slot()
        )
        if slot_idx is None:
            logger.error("No empty GPS slot found.")
            return None
        pal_data = PalObjects.get_array_property(
            self._gps_gvas_file.properties["SaveParameterArray"]
        )[slot_idx]

        pal = Pal(data=pal_data, dps=True)
        new_pal_id = uuid.uuid4()
        pal_dto.owner_uid = PalObjects.EMPTY_UUID
        pal_dto.instance_id = new_pal_id
        pal_dto.storage_id = PalObjects.EMPTY_UUID
        pal_dto.storage_slot = 0
        pal.update_from(pal_dto)
        pal.populate_status_point_lists()
        if not self._gps_pals:
            self._gps_pals = {}
        self._gps_pals[slot_idx] = pal
        return slot_idx, pal

    def add_guild_pal(
        self,
        character_id: str,
        nickname: str,
        guild_id: UUID,
        base_id: UUID,
        storage_slot: Optional[int] = None,
    ):
        guild = self._guilds.get(guild_id)
        if not guild:
            raise ValueError(f"Guild {guild_id} not found in the save file.")
        new_pal = guild.add_base_pal(character_id, nickname, base_id, storage_slot)
        if new_pal is None:
            return
        self._get_character_save_parameters().add(new_pal.character_save)
        self._pals[new_pal.instance_id] = new_pal
        self.invalidate_performance_caches()
        return new_pal

    def move_pal(self, player_id: UUID, pal_id: UUID, container_id: UUID) -> Pal | None:
        player = self._players.get(player_id)
        if not player:
            raise ValueError(f"Player {player_id} not found in the save file.")

        return player.move_pal(pal_id, container_id)

    def clone_pal(self, pal: PalDTO) -> Optional[Pal]:
        player = self._players.get(pal.owner_uid)
        if not player:
            raise ValueError(f"Player {pal.owner_uid} not found in the save file.")

        new_pal = player.clone_pal(pal)
        if new_pal is None:
            return
        self._get_character_save_parameters().add(new_pal.character_save)
        self._pals[new_pal.instance_id] = new_pal
        self.invalidate_performance_caches()
        return new_pal

    def clone_dps_pal(self, pal: PalDTO) -> Optional[Pal]:
        player = self._players.get(pal.owner_uid)
        if not player:
            raise ValueError(f"Player {pal.owner_uid} not found in the save file.")

        slot_idx, new_pal = player.clone_dps_pal(pal)
        if new_pal is None:
            return
        return slot_idx, new_pal

    def clone_guild_pal(
        self, guild_id: UUID, base_id: UUID, pal: PalDTO
    ) -> Optional[Pal]:
        guild = self._guilds.get(guild_id)
        if not guild:
            raise ValueError(f"Base {base_id} not found in the guild {guild_id}.")
        new_pal = guild.clone_base_pal(base_id, pal)
        if new_pal is None:
            return
        self._get_character_save_parameters().add(new_pal.character_save)
        self._pals[new_pal.instance_id] = new_pal
        self.invalidate_performance_caches()
        return new_pal

    def delete_player_pals(self, player_id: UUID, pal_ids: List[UUID]) -> None:
        player = self._players.get(player_id)
        if not player:
            raise ValueError(f"Player {player_id} not found in the save file.")

        for pal_id in pal_ids:
            player.delete_pal(pal_id)
            self._delete_pal_by_id(pal_id)

    def delete_player_dps_pals(self, player_id: UUID, pal_indexes: List[int]) -> None:
        player = self._players.get(player_id)
        if not player:
            raise ValueError(f"Player {player_id} not found in the save file.")
        player.delete_dps_pals(pal_indexes)

    def delete_guild_pals(
        self, guild_id: UUID, base_id: UUID, pal_ids: List[UUID]
    ) -> None:
        guild = self._guilds.get(guild_id)
        if not guild:
            raise ValueError(f"Base {base_id} not found in the guild {guild_id}.")

        for pal_id in pal_ids:
            guild.delete_base_pal(base_id, pal_id)
            self._delete_pal_by_id(pal_id)

    def heal_pals(self, pal_ids: List[UUID]) -> None:
        for pal_id in pal_ids:
            pal = self._pals.get(pal_id)
            if not pal:
                logger.error("Pal %s not found in the save file.", pal_id)
                continue
            pal.heal()

    def heal_all_player_pals(self, player_id: UUID) -> None:
        player = self._players.get(player_id)
        if not player:
            raise ValueError(f"Player {player_id} not found in the save file.")
        for pal in player.pals.values():
            pal.heal()

    def heal_all_base_pals(self, guild_id: UUID, base_id: UUID) -> None:
        base = self._guilds.get(guild_id).bases.get(base_id)
        if not base:
            raise ValueError(f"Base {base_id} not found in the guild {guild_id}.")
        for pal in base.pals.values():
            pal.heal()

    def get_json(self, minify=False, allow_nan=True):
        logger.info("Converting %s to JSON", self.level_sav_path)
        return json.dumps(
            self._gvas_file.dump(),
            indent=None if minify else "\t",
            cls=CustomEncoder,
            allow_nan=allow_nan,
        )

    def get_dict(self):
        logger.info("Converting %s to dict", self.level_sav_path)
        return self._gvas_file.dump()

    def get_pal(self, pal_id: UUID) -> Pal:
        return self._pals.get(pal_id)

    def get_pals(self):
        return self._pals

    def get_players(self):
        return self._players

    def get_player(self, player_id: UUID) -> Player:
        return self._players.get(player_id)

    def get_guild(self, guild_id: UUID) -> Optional[Guild]:
        return self._guilds.get(guild_id, None)

    def get_guilds(self):
        return self._guilds

    def get_gps(self) -> Optional[Dict[int, Pal]]:
        return self._gps_pals

    def load_gps(self, global_pal_storage_sav: bytes):
        logger.info("Loading global pal storage")
        raw_gvas, _ = decompress_sav_to_gvas(global_pal_storage_sav)
        gvas_file = GvasFile.read(
            raw_gvas, PALWORLD_TYPE_HINTS, CUSTOM_PROPERTIES, allow_nan=True
        )
        self._gps_gvas_file = gvas_file
        self._load_gps_pals()
        return self._gps_pals

    def get_base(self, base_id: UUID) -> Base:
        for guild in self._guilds.values():
            base = guild.bases.get(base_id)
            if base:
                return base
        return None

    def get_character_container(self, container_id: UUID) -> Dict[str, Any]:
        return self._get_character_containers().get(container_id)

    def get_item_container(self, container_id: UUID) -> Dict[str, Any]:
        return self._get_item_containers().get(container_id)

    def load_json(self, data: bytes):
        logger.info("Loading %s as JSON", self.level_sav_path)
        self._gvas_file = GvasFile.load(json.loads(data))
        return self

    def load_level_meta(self, data: bytes):
        logger.info("Loading %s as GVAS", self.level_sav_path)
        raw_gvas, _ = decompress_sav_to_gvas(data)
        custom_properties = {
            k: v
            for k, v in PALWORLD_CUSTOM_PROPERTIES.items()
            if k not in DISABLED_PROPERTIES
        }
        gvas_file = GvasFile.read(
            raw_gvas, PALWORLD_TYPE_HINTS, custom_properties, allow_nan=True
        )
        self._level_meta_gvas_file = gvas_file
        return self._level_meta_gvas_file

    def load_level_sav(self, data: bytes):
        logger.info("Loading %s as GVAS", self.level_sav_path)
        start_time = time.perf_counter()
        raw_gvas, _ = decompress_sav_to_gvas(data)
        logger.info(f"Decompressed in {time.perf_counter() - start_time} seconds")
        gvas_start_time = time.perf_counter()
        gvas_file = GvasFile.read(
            raw_gvas, PALWORLD_TYPE_HINTS, CUSTOM_PROPERTIES, allow_nan=True
        )
        logger.info(
            f"GvasFile read in {time.perf_counter() - gvas_start_time:.2f} seconds"
        )
        self._gvas_file = gvas_file
        self._get_file_size(data)
        return self

    def pal_count(self):
        return len(self._pals)

    def get_player_summaries(self) -> Dict[UUID, PlayerSummary]:
        valid_summaries = {}
        filtered_count = 0

        for player_id, summary in self._player_summaries.items():
            if player_id in self._player_file_refs:
                valid_summaries[player_id] = summary
            else:
                filtered_count += 1
                logger.warning(
                    f"Filtering out player {player_id} ({summary.nickname}) - no .sav file reference"
                )

        if filtered_count > 0:
            logger.info(
                f"Filtered {filtered_count} players without .sav files, "
                f"returning {len(valid_summaries)} valid players"
            )

        return valid_summaries

    def get_guild_summaries(self) -> Dict[UUID, GuildSummary]:
        return self._guild_summaries

    def is_player_loaded(self, player_id: UUID) -> bool:
        return player_id in self._loaded_players

    def is_guild_loaded(self, guild_id: UUID) -> bool:
        return guild_id in self._loaded_guilds

    async def load_sav_files(
        self,
        level_sav: bytes,
        player_file_refs: Dict[UUID, Dict[str, Any]],
        level_meta: Optional[bytes] = None,
        ws_callback=None,
    ) -> "SaveFile":
        logger.info("Loading %s (minimal mode)", self.level_sav_path)
        start_time = time.perf_counter()

        raw_gvas, _ = decompress_sav_to_gvas(level_sav)
        gvas_file = GvasFile.read(
            raw_gvas, PALWORLD_TYPE_HINTS, CUSTOM_PROPERTIES, allow_nan=True
        )
        self._gvas_file = gvas_file
        logger.info(f"Level.sav parsed in {time.perf_counter() - start_time:.2f}s")

        if level_meta:
            await ws_callback("Loading level meta...")
            self.load_level_meta(level_meta)
            self._load_world_name()
        else:
            await ws_callback("No LevelMeta.sav found, skipped.")
            self.world_name = "No LevelMeta.sav found"

        self._get_file_size(level_sav)

        self._set_data()

        self._player_file_refs = player_file_refs

        await ws_callback("Extracting player summaries...")
        self._extract_player_summaries()

        await ws_callback("Extracting guild summaries...")
        self._extract_guild_summaries()

        logger.info(
            f"Load complete in {time.perf_counter() - start_time:.2f}s - "
            f"{len(self._player_summaries)} players, {len(self._guild_summaries)} guilds"
        )

        return self

    def _extract_player_summaries(self) -> Dict[UUID, PlayerSummary]:
        start_time = time.perf_counter()
        players_data, pal_owner_counts = self._categorize_character_entries()
        player_guild_map = self._get_player_guild_map()

        if len(players_data) > 2:
            summaries = self._extract_players_parallel(
                players_data, player_guild_map, pal_owner_counts
            )
        else:
            summaries = self._extract_players_sequential(
                players_data, player_guild_map, pal_owner_counts
            )

        self._player_summaries = summaries
        elapsed = time.perf_counter() - start_time
        logger.info(f"Extracted {len(summaries)} player summaries in {elapsed:.3f}s")
        return summaries

    def _categorize_character_entries(
        self,
    ) -> Tuple[List[Tuple[UUID, Dict[str, Any]]], Dict[UUID, int]]:
        players_data: List[Tuple[UUID, Dict[str, Any]]] = []
        pal_owner_counts: Dict[UUID, int] = {}

        for entry in self._character_save_parameter_map:
            try:
                save_parameter = entry["value"]["RawData"]["value"]["object"][
                    "SaveParameter"
                ]["value"]
            except (KeyError, TypeError):
                continue

            if self._is_player(entry):
                try:
                    uid = PalObjects.get_guid(entry["key"]["PlayerUId"])
                    if uid and not is_empty_uuid(uid):
                        players_data.append((uid, save_parameter))
                except (KeyError, TypeError):
                    continue
            else:
                owner_uid_data = save_parameter.get("OwnerPlayerUId")
                if owner_uid_data:
                    owner_uid = PalObjects.get_guid(owner_uid_data)
                    if owner_uid:
                        pal_owner_counts[owner_uid] = (
                            pal_owner_counts.get(owner_uid, 0) + 1
                        )

        self._pal_owner_counts_cache = pal_owner_counts

        return players_data, pal_owner_counts

    def _get_player_guild_map(self) -> Dict[UUID, UUID]:
        if self._player_guild_map_cache is not None:
            return self._player_guild_map_cache

        self._player_guild_map_cache = self._build_player_guild_index()
        return self._player_guild_map_cache

    def _build_player_guild_index(self) -> Dict[UUID, UUID]:
        player_guild_map: Dict[UUID, UUID] = {}

        if not self._group_save_data_map:
            return player_guild_map

        for entry in self._group_save_data_map:
            group_type = PalObjects.get_enum_property(
                PalObjects.get_nested(entry, "value", "GroupType")
            )
            if GroupType.from_value(group_type) != GroupType.GUILD:
                continue

            guild_id = PalObjects.as_uuid(PalObjects.get_nested(entry, "key"))
            if not guild_id:
                continue

            try:
                raw_data = entry["value"]["RawData"]["value"]
            except (KeyError, TypeError):
                continue

            for player_entry in raw_data.get("players", []):
                player_uid = PalObjects.as_uuid(player_entry.get("player_uid"))
                if player_uid:
                    player_guild_map[player_uid] = guild_id

        return player_guild_map

    def _create_player_summary(
        self,
        uid: UUID,
        save_parameter: Dict[str, Any],
        player_guild_map: Dict[UUID, UUID],
        pal_owner_counts: Dict[UUID, int],
    ) -> PlayerSummary:
        nickname = None
        if "NickName" in save_parameter:
            nickname_data = save_parameter["NickName"]
            if isinstance(nickname_data, dict):
                nickname = nickname_data.get("value")
            else:
                nickname = nickname_data
        if not nickname:
            nickname = f"Player ({str(uid)[:8]})"
        nickname = sanitize_string(nickname)

        level = None
        if "Level" in save_parameter:
            level = PalObjects.get_byte_property(save_parameter["Level"])

        return PlayerSummary(
            uid=uid,
            nickname=nickname,
            level=level,
            guild_id=player_guild_map.get(uid),
            pal_count=pal_owner_counts.get(uid, 0),
            loaded=False,
        )

    def _extract_players_sequential(
        self,
        players_data: List[Tuple[UUID, Dict[str, Any]]],
        player_guild_map: Dict[UUID, UUID],
        pal_owner_counts: Dict[UUID, int],
    ) -> Dict[UUID, PlayerSummary]:
        summaries = {}
        for uid, save_parameter in players_data:
            summaries[uid] = self._create_player_summary(
                uid, save_parameter, player_guild_map, pal_owner_counts
            )
            logger.debug(
                "Extracted player summary for player: %s", summaries[uid].nickname
            )
        return summaries

    def _extract_players_parallel(
        self,
        players_data: List[Tuple[UUID, Dict[str, Any]]],
        player_guild_map: Dict[UUID, UUID],
        pal_owner_counts: Dict[UUID, int],
    ) -> Dict[UUID, PlayerSummary]:
        summaries = {}
        max_workers = min(4, len(players_data))

        with ThreadPoolExecutor(max_workers=max_workers) as executor:
            future_to_uid = {
                executor.submit(
                    self._create_player_summary,
                    uid,
                    save_parameter,
                    player_guild_map,
                    pal_owner_counts,
                ): uid
                for uid, save_parameter in players_data
            }

            for future in as_completed(future_to_uid):
                uid = future_to_uid[future]
                try:
                    summaries[uid] = future.result()
                    logger.debug(
                        "Extracted player summary for player: %s",
                        summaries[uid].nickname,
                    )
                except Exception as e:
                    logger.error(f"Error extracting player {uid}: {e}")

        return summaries

    def invalidate_performance_caches(self) -> None:
        self._pal_owner_counts_cache = None
        self._player_guild_map_cache = None
        self._map_object_index = None
        if self._item_containers is not None:
            self._item_containers.invalidate()
        if self._dynamic_items is not None:
            self._dynamic_items.invalidate()
        if self._character_containers is not None:
            self._character_containers.invalidate()
        if self._character_save_parameters is not None:
            self._character_save_parameters.invalidate()
        logger.debug("Performance caches invalidated")

    def _get_map_object_index(self) -> Dict[UUID, List[Dict[str, Any]]]:
        if self._map_object_index is not None:
            return self._map_object_index
        self._map_object_index = self._build_map_object_index()
        return self._map_object_index

    def _build_map_object_index(self) -> Dict[UUID, List[Dict[str, Any]]]:
        index: Dict[UUID, List[Dict[str, Any]]] = {}
        if not self._map_object_save_data or "values" not in self._map_object_save_data:
            return index
        for map_object in self._map_object_save_data["values"]:
            try:
                base_camp_id = PalObjects.as_uuid(
                    map_object["Model"]["value"]["RawData"]["value"][
                        "base_camp_id_belong_to"
                    ]
                )
            except (KeyError, TypeError):
                continue
            if base_camp_id:
                if base_camp_id not in index:
                    index[base_camp_id] = []
                index[base_camp_id].append(map_object)
        return index

    def _get_item_containers(self) -> IndexedCollection[UUID, Dict[str, Any]]:
        if self._item_containers is not None:
            return self._item_containers
        self._item_containers = self._build_item_containers_collection()
        return self._item_containers

    def _build_item_containers_collection(
        self,
    ) -> IndexedCollection[UUID, Dict[str, Any]]:
        def key_extractor(entry: Dict[str, Any]) -> Optional[UUID]:
            try:
                return PalObjects.get_guid(entry["key"]["ID"])
            except (KeyError, TypeError):
                return None

        return IndexedCollection(
            data=self._item_container_save_data,
            key_extractor=key_extractor,
        )

    def _get_dynamic_items(self) -> IndexedCollection[UUID, Dict[str, Any]]:
        if self._dynamic_items is not None:
            return self._dynamic_items
        self._dynamic_items = self._build_dynamic_items_collection()
        return self._dynamic_items

    def _build_dynamic_items_collection(
        self,
    ) -> IndexedCollection[UUID, Dict[str, Any]]:
        def key_extractor(entry: Dict[str, Any]) -> Optional[UUID]:
            try:
                return PalObjects.as_uuid(
                    entry["RawData"]["value"]["id"]["local_id_in_created_world"]
                )
            except (KeyError, TypeError):
                return None

        return IndexedCollection(
            data=self._dynamic_item_save_data,
            key_extractor=key_extractor,
        )

    def _get_character_containers(self) -> IndexedCollection[UUID, Dict[str, Any]]:
        if self._character_containers is not None:
            return self._character_containers
        self._character_containers = self._build_character_containers_collection()
        return self._character_containers

    def _build_character_containers_collection(
        self,
    ) -> IndexedCollection[UUID, Dict[str, Any]]:
        def key_extractor(entry: Dict[str, Any]) -> Optional[UUID]:
            try:
                return PalObjects.get_guid(entry["key"]["ID"])
            except (KeyError, TypeError):
                return None

        return IndexedCollection(
            data=self._character_container_save_data,
            key_extractor=key_extractor,
        )

    def _get_character_save_parameters(self) -> IndexedCollection[UUID, Dict[str, Any]]:
        if self._character_save_parameters is not None:
            return self._character_save_parameters
        self._character_save_parameters = (
            self._build_character_save_parameters_collection()
        )
        return self._character_save_parameters

    def _build_character_save_parameters_collection(
        self,
    ) -> IndexedCollection[UUID, Dict[str, Any]]:
        def key_extractor(entry: Dict[str, Any]) -> Optional[UUID]:
            try:
                return PalObjects.get_guid(entry["key"]["InstanceId"])
            except (KeyError, TypeError):
                return None

        return IndexedCollection(
            data=self._character_save_parameter_map,
            key_extractor=key_extractor,
        )

    def _extract_guild_summaries(self) -> Dict[UUID, GuildSummary]:
        summaries = {}

        if not self._group_save_data_map:
            self._guild_summaries = summaries
            return summaries

        for entry in self._group_save_data_map:
            group_type = PalObjects.get_enum_property(
                PalObjects.get_nested(entry, "value", "GroupType")
            )
            group_type = GroupType.from_value(group_type)
            if group_type != GroupType.GUILD:
                continue

            guild_id = PalObjects.as_uuid(PalObjects.get_nested(entry, "key"))
            if not guild_id or is_empty_uuid(guild_id):
                continue

            raw_data = PalObjects.get_nested(entry, "value", "RawData", "value")
            if not raw_data:
                continue

            name = raw_data.get("guild_name", "Unknown Guild")
            name = sanitize_string(name)
            players = raw_data.get("players", [])
            admin_player_uid = PalObjects.as_uuid(raw_data.get("admin_player_uid"))

            base_count = 0
            if self._base_camp_save_data_map:
                base_count = sum(
                    1
                    for base in self._base_camp_save_data_map
                    if are_equal_uuids(
                        PalObjects.as_uuid(
                            PalObjects.get_nested(
                                base, "value", "RawData", "value", "group_id_belong_to"
                            )
                        ),
                        guild_id,
                    )
                )

            summaries[guild_id] = GuildSummary(
                id=guild_id,
                name=name,
                admin_player_uid=admin_player_uid,
                player_count=len(players),
                base_count=base_count,
                loaded=False,
            )

        self._guild_summaries = summaries
        logger.info(f"Extracted {len(summaries)} guild summaries")
        return summaries

    def _find_player_guild_id(self, player_id: UUID):
        if self._player_guild_map_cache is not None:
            guild_id = self._player_guild_map_cache.get(player_id)
            if guild_id:
                for entry in self._group_save_data_map:
                    entry_guild_id = PalObjects.as_uuid(
                        PalObjects.get_nested(entry, "key")
                    )
                    if are_equal_uuids(entry_guild_id, guild_id):
                        yield guild_id, entry
                        return

        if not self._group_save_data_map:
            return

        for entry in self._group_save_data_map:
            group_type = PalObjects.get_enum_property(
                PalObjects.get_nested(entry, "value", "GroupType")
            )
            if GroupType.from_value(group_type) != GroupType.GUILD:
                continue

            guild_id = PalObjects.as_uuid(PalObjects.get_nested(entry, "key"))
            raw_data = PalObjects.get_nested(entry, "value", "RawData", "value")
            if not raw_data:
                continue

            players = raw_data.get("players", [])
            for player_entry in players:
                player_uid = PalObjects.as_uuid(player_entry.get("player_uid"))
                if are_equal_uuids(player_uid, player_id):
                    yield guild_id, entry

    def _pal_belongs_to_player(self, entry: Dict[str, Any], player_id: UUID) -> bool:
        save_parameter = PalObjects.get_nested(
            entry, "value", "RawData", "value", "object", "SaveParameter", "value"
        )
        if not save_parameter:
            return False

        owner_uid = PalObjects.get_guid(save_parameter.get("OwnerPlayerUId"))
        return are_equal_uuids(owner_uid, player_id)

    async def load_player_on_demand(
        self,
        player_id: UUID,
        ws_callback=None,
    ) -> Optional[Player]:
        if player_id in self._players:
            logger.info(f"Player {player_id} already loaded, returning cached")
            return self._players[player_id]

        if player_id not in self._player_file_refs:
            logger.warning(f"No file reference for player {player_id}")
            return None

        file_ref = self._player_file_refs[player_id]
        start_time = time.perf_counter()

        if ws_callback:
            nickname = self._player_summaries.get(player_id)
            name = nickname.nickname if nickname else str(player_id)[:8]
            await ws_callback(f"Loading player {name}...")

        sav_data = file_ref.get("sav")
        if sav_data is None:
            logger.warning(f"No save data for player {player_id}")
            return None

        if isinstance(sav_data, bytes):
            sav_bytes = sav_data
        else:
            with open(sav_data, "rb") as f:
                sav_bytes = f.read()

        raw_gvas, _ = decompress_sav_to_gvas(sav_bytes)
        gvas_file = GvasFile.read(
            raw_gvas, PALWORLD_TYPE_HINTS, CUSTOM_PROPERTIES, allow_nan=True
        )

        dps_gvas = None
        dps_data = file_ref.get("dps")
        if dps_data:
            if isinstance(dps_data, bytes):
                dps_bytes = dps_data
            else:
                with open(dps_data, "rb") as f:
                    dps_bytes = f.read()
            raw_dps, _ = decompress_sav_to_gvas(dps_bytes)
            dps_gvas = GvasFile.read(
                raw_dps, PALWORLD_TYPE_HINTS, CUSTOM_PROPERTIES, allow_nan=True
            )

        self._player_gvas_files[player_id] = PlayerGvasFiles(
            sav=gvas_file, dps=dps_gvas
        )

        player_entry = None
        for entry in self._character_save_parameter_map:
            if self._is_player(entry):
                if are_equal_uuids(
                    PalObjects.get_guid(entry["key"]["PlayerUId"]), player_id
                ):
                    player_entry = entry
                    break

        if not player_entry:
            logger.warning(f"No character entry for player {player_id}")
            return None

        if ws_callback:
            await ws_callback("Loading pals...")
        player_pals = self._load_player_pals_only(player_id)

        guild = self._player_guild(player_id)
        if not guild:
            for gid, _ in self._find_player_guild_id(player_id):
                if gid not in self._guilds:
                    self._load_guild_by_id(gid)
                guild = self._guilds.get(gid)
                break

        player = Player(
            gvas_files=self._player_gvas_files[player_id],
            character_save_parameter=player_entry,
            guild=guild,
            item_container_index=self._get_item_containers().index,
            dynamic_items=self._get_dynamic_items(),
            character_container_index=self._get_character_containers().index,
            pals=player_pals,
        )

        self._players[player_id] = player
        self._loaded_players.add(player_id)

        if player_id in self._player_summaries:
            self._player_summaries[player_id].loaded = True

        logger.info(
            f"Player {player_id} loaded on demand in {time.perf_counter() - start_time:.2f}s "
            f"with {len(player_pals)} pals"
        )

        return player

    def _load_player_pals_only(self, player_id: UUID) -> Dict[UUID, Pal]:
        pals = {}
        for entry in self._character_save_parameter_map:
            if self._is_player(entry):
                continue
            if self._pal_belongs_to_player(entry, player_id):
                pal = Pal(entry)
                if pal:
                    pals[pal.instance_id] = pal
                    self._pals[pal.instance_id] = pal
        return pals

    def _load_guild_by_id(self, guild_id: UUID) -> Optional[Guild]:
        logger.debug(f"Loading guild {guild_id}")
        if guild_id in self._guilds:
            logger.info(f"Guild {guild_id} already loaded, returning cached")
            return self._guilds[guild_id]

        for entry in self._group_save_data_map:
            gid = PalObjects.as_uuid(PalObjects.get_nested(entry, "key"))
            if not are_equal_uuids(gid, guild_id):
                continue

            guild_extra_save_data = next(
                (
                    g
                    for g in self._guild_extra_save_data_map
                    if are_equal_uuids(g["key"], guild_id)
                ),
                None,
            )
            if not guild_extra_save_data:
                logger.warning(f"Guild extra save data not found for guild {guild_id}")
                return None

            guild = Guild(
                group_save_data=entry,
                guild_extra_data=guild_extra_save_data,
                item_container_index=self._get_item_containers().index,
                dynamic_items=self._get_dynamic_items(),
            )
            self._guilds[guild_id] = guild
            self._loaded_guilds.add(guild_id)

            self._load_bases_for_guild(guild_id)

            if guild_id in self._guild_summaries:
                self._guild_summaries[guild_id].loaded = True

            return guild

        return None

    def _load_bases_for_guild(self, guild_id: UUID) -> None:
        logger.debug(f"Loading bases for guild {guild_id}")
        if not self._base_camp_save_data_map:
            logger.warning("No bases found in the save file.")
            return

        if guild_id not in self._guilds:
            logger.warning(f"Guild {guild_id} not loaded, cannot load bases")
            return

        map_object_index = self._get_map_object_index()
        item_container_index = self._get_item_containers().index
        dynamic_items = self._get_dynamic_items()

        for entry in self._base_camp_save_data_map:
            group_id_belong_to = PalObjects.as_uuid(
                PalObjects.get_nested(
                    entry, "value", "RawData", "value", "group_id_belong_to"
                )
            )
            if not are_equal_uuids(group_id_belong_to, guild_id):
                continue

            container_id = PalObjects.as_uuid(
                PalObjects.get_nested(
                    entry,
                    "value",
                    "WorkerDirector",
                    "value",
                    "RawData",
                    "value",
                    "container_id",
                )
            )

            character_container = next(
                (
                    c
                    for c in self._character_container_save_data
                    if are_equal_uuids(
                        PalObjects.get_guid(PalObjects.get_nested(c, "key", "ID")),
                        container_id,
                    )
                ),
                None,
            )

            if not character_container:
                logger.warning(f"Character container not found for base {entry['key']}")
                continue

            container_slot_count = PalObjects.get_value(
                character_container["value"]["SlotNum"]
            )

            pals = self._load_pals_for_container(container_id)
            base_id = PalObjects.as_uuid(entry["key"])
            base_map_objects = map_object_index.get(base_id, [])

            base = Base(
                data=entry,
                pals=pals,
                container_id=container_id,
                slot_count=container_slot_count,
                character_container_index=self._get_character_containers().index,
                base_map_objects=base_map_objects,
                item_container_index=item_container_index,
                dynamic_items=dynamic_items,
            )
            self._guilds[guild_id].add_base(base)

            logger.debug(f"Loaded base for guild {guild_id} with {len(pals)} pals")

    def _load_pals_for_container(self, container_id: UUID) -> Dict[UUID, Pal]:
        pals = {}
        logger.debug(f"Loading pals for container {container_id}")
        for entry in self._character_save_parameter_map:
            if self._is_player(entry):
                continue

            save_parameter = PalObjects.get_nested(
                entry, "value", "RawData", "value", "object", "SaveParameter", "value"
            )
            if not save_parameter:
                continue

            slot_id = save_parameter.get("SlotId")
            if not slot_id:
                logger.debug("Pal entry has no SlotID, skipping")
                continue

            pal_container_id = PalObjects.get_guid(
                PalObjects.get_nested(slot_id, "value", "ContainerId", "value", "ID")
            )

            if are_equal_uuids(pal_container_id, container_id):
                logger.debug(f"Found pal in container {container_id}")
                pal = Pal(entry)
                if pal:
                    pals[pal.instance_id] = pal
                    self._pals[pal.instance_id] = pal

        return pals

    def sav(self, gvas_file: GvasFile = None) -> bytes:
        logger.info("Converting %s to SAV", self.level_sav_path)
        target_gvas = gvas_file if gvas_file else self._gvas_file
        gvas = copy.deepcopy(target_gvas)
        return compress_gvas_to_sav(gvas.write(CUSTOM_PROPERTIES), 0x31)

    def player_savs(self) -> Dict[UUID, bytes]:
        logger.info("Converting player save files to SAV", len(self._player_gvas_files))
        return {
            uid: compress_gvas_to_sav(
                self._player_gvas_files[uid].sav.write(CUSTOM_PROPERTIES),
                0x31,
            )
            for uid in self._player_gvas_files
        }

    def player_gvas_files(self) -> Dict[UUID, Dict[str, Optional[bytes]]]:
        logger.info(
            "Converting player save files to SAV: %s", len(self._player_gvas_files)
        )
        return {
            uid: {
                "sav": compress_gvas_to_sav(
                    files.sav.write(CUSTOM_PROPERTIES),
                    0x31,
                ),
                "dps": (
                    compress_gvas_to_sav(
                        files.dps.write(CUSTOM_PROPERTIES),
                        0x31,
                    )
                    if files.dps
                    else None
                ),
            }
            for uid, files in self._player_gvas_files.items()
        }

    def to_json_file(
        self,
        output_path,
        minify=False,
        allow_nan=True,
    ):
        logger.info(
            "Converting %s to JSON, saving to %s", self.level_sav_path, output_path
        )
        with open(output_path, "w", encoding="utf8") as f:
            indent = None if minify else "\t"
            json.dump(
                self._gvas_file.dump(),
                f,
                indent=indent,
                cls=CustomEncoder,
                allow_nan=allow_nan,
            )

    def to_level_sav_file(self, output_path):
        logger.info(
            "Converting %s to SAV, saving to %s", self.level_sav_path, output_path
        )
        gvas = copy.deepcopy(self._gvas_file)
        sav_file = compress_gvas_to_sav(gvas.write(CUSTOM_PROPERTIES), 0x31)
        with open(output_path, "wb") as f:
            f.write(sav_file)

    def to_level_meta_sav_file(self, output_path):
        if not self._level_meta_gvas_file:
            raise ValueError("No LevelMeta GvasFile has been loaded.")
        logger.info("Converting LevelMeta to SAV, saving to %s", output_path)
        sav_file = compress_gvas_to_sav(
            self._level_meta_gvas_file.write(CUSTOM_PROPERTIES), 0x31
        )
        with open(output_path, "wb") as f:
            f.write(sav_file)

    def to_gps_save_file(self, output_path: str) -> None:
        if not self._gps_gvas_file:
            raise ValueError("No GPS GvasFile has been loaded.")
        logger.info("Converting GPS save file to SAV, saving to %s", output_path)
        gvas = copy.deepcopy(self._gps_gvas_file)
        sav_file = compress_gvas_to_sav(
            gvas.write(CUSTOM_PROPERTIES),
            0x31,
        )
        with open(output_path, "wb") as f:
            f.write(sav_file)

    def to_player_sav_files(self, output_path: str) -> None:
        logger.info("Converting player save files to SAV, saving to %s", output_path)
        for uid, player_files in self._player_gvas_files.items():
            sav_file = compress_gvas_to_sav(
                player_files.sav.write(CUSTOM_PROPERTIES),
                0x31,
            )
            uid = str(uid).replace("-", "")
            with open(os.path.join(output_path, f"{uid}.sav"), "wb") as f:
                f.write(sav_file)
            if player_files.dps:
                dps_sav_file = compress_gvas_to_sav(
                    player_files.dps.write(CUSTOM_PROPERTIES),
                    0x31,
                )
                with open(os.path.join(output_path, f"{uid}_dps.sav"), "wb") as f_dps:
                    f_dps.write(dps_sav_file)

    async def update_pals(self, modified_pals: Dict[UUID, PalDTO], ws_callback) -> None:
        if not self._gvas_file:
            raise ValueError("No GvasFile has been loaded.")

        for pal_id, pal in modified_pals.items():
            pal_name = pal.nickname if pal.nickname else pal.character_id
            await ws_callback(f"Updating pal {pal_name}")
            existing_pal = self._pals[pal_id]
            existing_pal.update_from(pal)

        logger.info("Updated %d pals in the save file.", len(modified_pals))

        await ws_callback("Saving changes to file")

    async def update_dps_pals(
        self, modified_pals: Dict[int, PalDTO], ws_callback
    ) -> None:
        for pal_idx, pal in modified_pals.items():
            pal_name = pal.nickname if pal.nickname else pal.character_id
            await ws_callback(f"Updating DPS pal {pal_name}")
            player = self._players.get(pal.owner_uid)
            player.update_dps_pal(pal_idx, pal)

        logger.info("Updated %d pals in the save file.", len(modified_pals))

        await ws_callback("Saving changes to file")

    async def update_gps_pals(
        self, modified_pals: Dict[int, PalDTO], ws_callback
    ) -> None:
        if not self._gps_pals:
            raise ValueError("No GPS pals to update.")

        for pal_idx, pal in modified_pals.items():
            pal_name = pal.nickname if pal.nickname else pal.character_id
            await ws_callback(f"Updating GPS pal {pal_name}")
            if pal_idx not in self._gps_pals:
                logger.error("GPS pal index %d not found in the save file.", pal_idx)
                continue
            existing_pal = self._gps_pals[pal_idx]
            existing_pal.update_from(pal)

    def delete_gps_pals(self, pal_indexes: List[int]) -> None:
        if not self._gps_pals:
            logger.warning("No GPS pals to delete.")
            return
        for pal_idx in sorted(pal_indexes, reverse=True):
            if pal_idx in self._gps_pals:
                logger.debug("Deleting GPS pal at index %d", pal_idx)
                pal = self._gps_pals[pal_idx]
                pal.reset()
                del self._gps_pals[pal_idx]

    async def update_players(
        self, modified_players: Dict[UUID, PlayerDTO], ws_callback
    ) -> None:
        if not self._gvas_file:
            raise ValueError("No GvasFile has been loaded.")

        for _, player in modified_players.items():
            await ws_callback(f"Updating player {player.nickname}")
            existing_player = self._players.get(player.uid)
            existing_player.update_from(player)

        logger.info("Updated %d players in the save file.", len(modified_players))

    async def update_player_technologies(
        self,
        player_id: UUID,
        technologies: Optional[list[str]] = None,
        technology_points: Optional[int] = None,
        boss_technology_points: Optional[int] = None,
        ws_callback=None,
    ) -> None:
        if not self._gvas_file:
            raise ValueError("No GvasFile has been loaded.")

        player = self._players.get(player_id)
        if not player:
            raise ValueError(f"Player {player_id} not found in the save file.")

        if technologies is not None:
            player.technologies = technologies
        if technology_points is not None:
            player.technology_points = technology_points
        if boss_technology_points is not None:
            player.boss_technology_points = boss_technology_points

        if ws_callback:
            await ws_callback("Updating player technologies and points")

    async def update_guilds(
        self, modified_guilds: Dict[UUID, GuildDTO], ws_callback
    ) -> None:
        if not self._gvas_file:
            raise ValueError("No GvasFile has been loaded.")

        for id, dto in modified_guilds.items():
            logger.debug("Updating guild %s", id)
            await ws_callback(f"Updating guild {id}")
            guild = self._guilds.get(id)
            guild.update_from(dto)

        logger.info("Updated %d bases in the save file.", len(modified_guilds))

    def _delete_pal_by_id(self, pal_id: UUID) -> None:
        del self._pals[pal_id]
        character_params = self._get_character_save_parameters()
        if character_params.remove_by_key(pal_id):
            logger.debug("Deleting pal %s from CharacterSaveParameterMap", pal_id)
            self.invalidate_performance_caches()

    def _get_file_size(self, data: bytes):
        if hasattr(data, "seek") and hasattr(data, "tell"):
            data.seek(0, os.SEEK_END)
            self.size = data.tell()
            data.seek(0)
        else:
            self.size = data.__sizeof__()

    def _get_player_pals(self, uid: UUID) -> Dict[UUID, Pal]:
        logger.info("Loading Pals for player %s", uid)
        pals = {}
        pals = {
            k: v for k, v in self._pals.items() if are_equal_uuids(v.owner_uid, uid)
        }
        return pals

    def _get_player_save_data(self, player_gvas: GvasFile) -> Optional[Dict[str, Any]]:
        player_save_data = PalObjects.get_value(player_gvas.properties["SaveData"])
        return player_save_data

    def _is_player(self, entry: Dict[str, Any]) -> bool:
        save_parameter_path = PalObjects.get_nested(
            entry, "value", "RawData", "value", "object", "SaveParameter", "value"
        )
        return (
            bool(PalObjects.get_value(save_parameter_path["IsPlayer"]))
            if "IsPlayer" in save_parameter_path
            else False
        )

    def _load_guilds(self):
        if not self._group_save_data_map:
            logger.warning("No guilds found in the save file.")

        for entry in self._group_save_data_map:
            group_type = PalObjects.get_enum_property(
                PalObjects.get_nested(entry, "value", "GroupType")
            )
            group_type = GroupType.from_value(group_type)
            if group_type != GroupType.GUILD:
                continue
            guild_id = PalObjects.as_uuid(PalObjects.get_nested(entry, "key"))
            if not guild_id or is_empty_uuid(guild_id):
                logger.warning("Guild with empty or invalid ID found, skipping.")
                continue
            guild_extra_save_data = next(
                (
                    g
                    for g in self._guild_extra_save_data_map
                    if are_equal_uuids(g["key"], guild_id)
                ),
                None,
            )
            if not guild_extra_save_data:
                logger.warning(
                    "Guild extra save data not found for guild %s, skipping.",
                    guild_id,
                )
                continue
            self._guilds[guild_id] = Guild(
                group_save_data=entry,
                guild_extra_data=guild_extra_save_data,
                item_container_index=self._get_item_containers().index,
                dynamic_items=self._get_dynamic_items(),
            )

    async def _load_bases(self, ws_callback):
        if not self._base_camp_save_data_map:
            logger.warning("No bases found in the save file.")
            ws_callback("No bases found in the save file.")
            return

        map_object_index = self._get_map_object_index()
        item_container_index = self._get_item_containers().index
        dynamic_items = self._get_dynamic_items()

        for entry in self._base_camp_save_data_map:
            group_id_belong_to = PalObjects.as_uuid(
                PalObjects.get_nested(
                    entry, "value", "RawData", "value", "group_id_belong_to"
                )
            )
            if group_id_belong_to not in self._guilds:
                logger.warning(
                    "Base %s does not belong to a guild, skipping.", entry["key"]
                )
                continue

            container_id = PalObjects.as_uuid(
                PalObjects.get_nested(
                    entry,
                    "value",
                    "WorkerDirector",
                    "value",
                    "RawData",
                    "value",
                    "container_id",
                )
            )
            character_container = next(
                (
                    c
                    for c in self._character_container_save_data
                    if are_equal_uuids(
                        PalObjects.get_guid(PalObjects.get_nested(c, "key", "ID")),
                        container_id,
                    )
                )
            )
            container_slot_count = PalObjects.get_value(
                character_container["value"]["SlotNum"]
            )

            pals = {
                pal.instance_id: pal
                for pal in self._pals.values()
                if pal.storage_id == container_id
            }

            base_id = PalObjects.as_uuid(entry["key"])
            base_map_objects = map_object_index.get(base_id, [])

            base = Base(
                data=entry,
                pals=pals,
                container_id=container_id,
                slot_count=container_slot_count,
                character_container_index=self._get_character_containers().index,
                base_map_objects=base_map_objects,
                item_container_index=item_container_index,
                dynamic_items=dynamic_items,
            )
            self._guilds[group_id_belong_to].add_base(base)

            logger.debug(
                "Guild %s has %d pals at base %s",
                self._guilds[group_id_belong_to].name,
                len(pals),
                base.id,
            )
            await ws_callback(
                f"Loaded base {base.id} with {len(pals)} pals from guild {self._guilds[group_id_belong_to].name}"
            )

    def _load_pals(self):
        if not self._gvas_file:
            raise ValueError("No GvasFile has been loaded.")
        self._pals = {}
        logger.info("Loading Pals")
        for e in self._character_save_parameter_map:
            if self._is_player(e):
                continue
            pal = Pal(e)
            if pal:
                self._pals[pal.instance_id] = pal
                logger.debug("Loaded Pal %s", pal)
            else:
                logger.warning("Failed to create PalEntity summary")

    def _load_gps_pals(self):
        if not self._gps_gvas_file:
            raise ValueError("No Global Pal Storage GvasFile has been loaded.")
        self._gps_pals = {}
        logger.info("Loading Global Pal Storage Pals")
        save_parameter_array = PalObjects.get_array_property(
            self._gps_gvas_file.properties["SaveParameterArray"]
        )
        for index, entry in enumerate(save_parameter_array):
            pal = Pal(data=entry, dps=True)
            if pal.character_id != "None":
                self._gps_pals[index] = pal

    def _load_world_name(self):
        world_name = PalObjects.get_nested(
            self._level_meta_gvas_file.properties,
            "SaveData",
            "value",
            "WorldName",
            "value",
        )
        self.world_name = world_name if world_name else "Unknown"

    def set_world_name(self, name: str) -> None:
        if not self._level_meta_gvas_file:
            raise ValueError("No LevelMeta GvasFile has been loaded.")
        old_world_name = self.world_name
        self.world_name = name
        PalObjects.set_nested(
            self._level_meta_gvas_file.properties,
            "SaveData",
            "value",
            "WorldName",
            "value",
            value=name,
        )
        logger.info("Changed world name from %s to %s", old_world_name, name)

    def _set_data(self) -> None:
        logger.debug("Properties keys: %s", self._gvas_file.properties.keys())
        world_save_data = PalObjects.get_value(
            self._gvas_file.properties["worldSaveData"]
        )
        logger.debug("World Save Data keys: %s", world_save_data.keys())
        self._character_save_parameter_map = PalObjects.get_value(
            world_save_data["CharacterSaveParameterMap"]
        )
        self._item_container_save_data = PalObjects.get_value(
            world_save_data["ItemContainerSaveData"]
        )
        self._dynamic_item_save_data = PalObjects.get_array_property(
            world_save_data["DynamicItemSaveData"]
        )
        self._character_container_save_data = PalObjects.get_value(
            world_save_data["CharacterContainerSaveData"]
        )
        self._group_save_data_map = PalObjects.get_value(
            world_save_data["GroupSaveDataMap"]
        )
        self._base_camp_save_data_map = (
            PalObjects.get_value(world_save_data["BaseCampSaveData"])
            if "BaseCampSaveData" in world_save_data
            else None
        )
        self._map_object_save_data = PalObjects.get_value(
            world_save_data["MapObjectSaveData"]
            if "MapObjectSaveData" in world_save_data
            else None
        )
        self._guild_extra_save_data_map = (
            PalObjects.get_value(world_save_data["GuildExtraSaveDataMap"])
            if "GuildExtraSaveDataMap" in world_save_data
            else None
        )

    def _player_guild(self, player_id: UUID) -> Optional[Guild]:
        if not self._guilds:
            return
        for guild in self._guilds.values():
            if player_id in guild.players:
                return guild
        return

    async def _load_players(
        self, player_sav_files: Dict[UUID, Dict[str, bytes]] = None, ws_callback=None
    ):
        if not self._character_save_parameter_map:
            return {}
        logger.info("Loading Players")

        loaded_sav_files: Dict[UUID, PlayerGvasFiles] = {}
        # This is a temp fix, need to look into fixing player uid
        # mismatches due to host fix
        host_fix_players = {}

        for uid, sav_files in player_sav_files.items():
            if "sav" not in sav_files or sav_files["sav"] is None:
                logger.warning("No save file found for player %s", uid)
                continue
            raw_gvas, _ = decompress_sav_to_gvas(sav_files["sav"])
            await ws_callback(f"Loading player {uid}...")
            gvas_file = GvasFile.read(
                raw_gvas, PALWORLD_TYPE_HINTS, CUSTOM_PROPERTIES, allow_nan=True
            )
            player_uuid = PalObjects.get_guid(
                PalObjects.get_nested(
                    gvas_file.properties,
                    "SaveData",
                    "value",
                    "IndividualId",
                    "value",
                    "PlayerUId",
                )
            )
            if not are_equal_uuids(uid, player_uuid):
                logger.warning(
                    "Player UIDs do not match (host fix detected): %s != %s",
                    uid,
                    player_uuid,
                )
                await ws_callback(
                    f"Player UIDs do not match (host fix detected): {uid} != {player_uuid}"
                )
                host_fix_players[player_uuid] = uid
            dps = None
            if "dps" in sav_files and sav_files["dps"] is not None:
                logger.debug("Loading player DPS save for %s", player_uuid)
                raw_dps_gvas, _ = decompress_sav_to_gvas(sav_files["dps"])
                dps_gvas_file = GvasFile.read(
                    raw_dps_gvas,
                    PALWORLD_TYPE_HINTS,
                    CUSTOM_PROPERTIES,
                    allow_nan=True,
                )
                dps = dps_gvas_file
            loaded_sav_files[player_uuid] = PlayerGvasFiles(sav=gvas_file, dps=dps)

        players = {}
        for entry in self._character_save_parameter_map:
            if self._is_player(entry):
                uid = PalObjects.get_guid(entry["key"]["PlayerUId"])
                if uid not in loaded_sav_files:
                    logger.warning("No player save file found for player %s", uid)
                    continue

                save_parameter = PalObjects.get_nested(
                    entry,
                    "value",
                    "RawData",
                    "value",
                    "object",
                    "SaveParameter",
                    "value",
                )
                if "NickName" in save_parameter:
                    nickname = PalObjects.get_value(save_parameter["NickName"])
                else:
                    nickname = f"🥷 ({str(uid).split('-')[0]})"

                await ws_callback(f"Loading player {nickname}...")
                self._player_gvas_files[uid] = loaded_sav_files[uid]
                player_pals = self._get_player_pals(uid)
                if uid in host_fix_players:
                    player_pals = player_pals | self._get_player_pals(
                        host_fix_players[uid]
                    )
                player = Player(
                    gvas_files=self._player_gvas_files[uid],
                    character_save_parameter=entry,
                    guild=self._player_guild(uid),
                    item_container_index=self._get_item_containers().index,
                    dynamic_items=self._get_dynamic_items(),
                    character_container_index=self._get_character_containers().index,
                    pals=player_pals,
                )
                players[uid] = player

        self._players = players
