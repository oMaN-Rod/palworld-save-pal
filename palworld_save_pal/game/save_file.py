import copy
from enum import Enum
import json
import os
from typing import Any, Dict, List, Optional, Union
from uuid import UUID
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

from palworld_save_pal.game.base import Base, BaseDTO
from palworld_save_pal.game.guild import Guild, GuildDTO
from palworld_save_pal.game.pal import Pal, PalDTO
from palworld_save_pal.game.pal_objects import GroupType, PalObjects
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.game.player import Player, PlayerDTO
from palworld_save_pal.utils.uuid import are_equal_uuids

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


class SaveType(int, Enum):
    STEAM = 0
    GAMEPASS = 1


class SaveFile(BaseModel):
    name: str = ""
    size: int = 0
    world_name: str = ""

    model_config = ConfigDict(arbitrary_types_allowed=True)

    _players: Dict[UUID, Player] = PrivateAttr(default_factory=dict)
    _pals: Dict[UUID, Pal] = PrivateAttr(default_factory=dict)
    _guilds: Dict[UUID, Guild] = PrivateAttr(default_factory=dict)

    _gvas_file: Optional[GvasFile] = PrivateAttr(default=None)
    _level_meta_gvas_file: Optional[GvasFile] = PrivateAttr(default=None)
    _player_gvas_files: Dict[UUID, GvasFile] = PrivateAttr(default_factory=dict)

    _character_save_parameter_map: List[Dict[str, Any]] = PrivateAttr(
        default_factory=list
    )
    _item_container_save_data: List[Dict[str, Any]] = PrivateAttr(default_factory=list)
    _dynamic_item_save_data: List[Dict[str, Any]] = PrivateAttr(default_factory=list)
    _character_container_save_data: List[Dict[str, Any]] = PrivateAttr(
        default_factory=list
    )
    _group_save_data_map: List[Dict[str, Any]] = PrivateAttr(default_factory=list)
    _base_camp_save_data_map: List[Dict[str, Any]] = PrivateAttr(default_factory=list)
    _map_object_save_data: List[Dict[str, Any]] = PrivateAttr(default_factory=list)
    _guild_extra_save_data_map: List[Dict[str, Any]] = PrivateAttr(default_factory=list)

    def _should_delete_map_object(self, map_object: dict, guild_id: UUID, player_ids: List[UUID]) -> bool:
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
        if are_equal_uuids(group_id, guild_id):
            return True
        
        # Check if any player in the list is the builder
        if any(are_equal_uuids(build_player_uid, player_id) for player_id in player_ids):
            return True

        # Handle edge cases
        if "ConcreteModel" in map_object:
            concrete_model_raw_data = map_object["ConcreteModel"]["value"]["RawData"]["value"]
            private_lock_player_uid = PalObjects.as_uuid(concrete_model_raw_data.get("private_lock_player_uid"))
            
            # Check if any player in the list is the private lock owner
            if any(are_equal_uuids(private_lock_player_uid, player_id) for player_id in player_ids):
                return True

            # Check trade info sellers
            for trade_info in concrete_model_raw_data.get("trade_infos", []):
                seller_player_uid = PalObjects.as_uuid(trade_info.get("seller_player_uid"))
                if any(are_equal_uuids(seller_player_uid, player_id) for player_id in player_ids):
                    return True

            # Check password lock module
            for module in concrete_model_raw_data.get("ModuleMap", {}).get("value", []):
                if module["key"] == "EPalMapObjectConcreteModelModuleType::PasswordLock":
                    for player_info in module["value"]["RawData"]["value"].get("player_infos", []):
                        player_uid = PalObjects.as_uuid(player_info.get("player_uid"))
                        if any(are_equal_uuids(player_uid, player_id) for player_id in player_ids):
                            return True

        return False

    def delete_guild_and_players(self, guild_id: UUID) -> None:
        guild = self._guilds.get(guild_id)
        if not guild:
            raise ValueError(f"Guild {guild_id} not found in the save file.")
        
        # Get all players in the guild
        players_in_guild = list(guild.players)

        # Container ids to delete
        container_ids_to_delete = []

        # Character container ids to delete
        character_container_ids_to_delete = []

        # Delete all map objects owned by guild or player in guild
        self._map_object_save_data["values"][:] = [
            obj for obj in self._map_object_save_data["values"]
            if not self._should_delete_map_object(obj, guild_id, players_in_guild)
        ]

        # Delete all players in the guild
        for player_id in players_in_guild:
            if player_id not in self._players:
                continue

            player = self._players[player_id]
            container_ids_to_delete = container_ids_to_delete + [
                player.common_container.id,
                player.essential_container.id,
                player.weapon_load_out_container.id,
                player.player_equipment_armor_container.id,
                player.food_equip_container.id
            ]
            character_container_ids_to_delete = character_container_ids_to_delete + [
                player.otomo_container_id,
                player.pal_box_id
            ]
            
            for pal_slot in list(player.pal_box.slots):
                # player.delete_pal(pal_slot.pal_id)
                self._delete_pal_by_id(pal_slot.pal_id) 

            for pal_slot in list(player.party.slots):
                # player.delete_pal(pal_slot.pal_id)
                self._delete_pal_by_id(pal_slot.pal_id)

            # Delete the player
            self._players = {
                pid: player for pid, player in self._players.items()
                if pid != player_id
            }

            # Delete player parameters
            self._character_save_parameter_map[:] = [
                entry for entry in self._character_save_parameter_map
                if not are_equal_uuids(PalObjects.get_guid(PalObjects.get_nested(entry, "key", "PlayerUId")), player_id)
            ]
            
            # Delete player save file
            self._player_gvas_files = {
                pid: gvas_file for pid, gvas_file in self._player_gvas_files.items()
                if pid != player_id
            }

        # Remove guild extra save data
        self._guild_extra_save_data_map[:] = [
            entry for entry in self._guild_extra_save_data_map
            if not are_equal_uuids(entry["key"], guild_id)
        ]

        # Delete all bases in the guild
        for base_id, base in guild.bases.items():
            container_ids_to_delete = container_ids_to_delete + list(base.storage_containers.keys())
            
            self.delete_guild_pals(guild_id, base_id, list(base.pals.keys()))

            self._base_camp_save_data_map[:] = [
                base for base in self._base_camp_save_data_map
                if not are_equal_uuids(PalObjects.get_nested(base, "key"), base_id)
            ]
 
        # Delete player items and guild items
        self._item_container_save_data[:] = [
            entry for entry in self._item_container_save_data
            if not any(
                are_equal_uuids(PalObjects.get_guid(entry["key"]["ID"]), container_id) or
                are_equal_uuids(PalObjects.get_guid(PalObjects.get_nested(entry, "value", "BelongInfo", "value", "GroupId")), guild_id) or
                are_equal_uuids(PalObjects.get_guid(PalObjects.get_nested(entry, "value", "BelongInfo", "value", "GroupId")), player_id)
                for container_id in container_ids_to_delete
            )
        ]

        # Delete character containers
        self._character_container_save_data[:] = [
            entry for entry in self._character_container_save_data
            if not any(
                are_equal_uuids(PalObjects.get_guid(entry["key"]["ID"]), container_id)
                for container_id in character_container_ids_to_delete
            )
        ]

        # Delete the guild
        self._group_save_data_map[:] = [
            group for group in self._group_save_data_map
            if not are_equal_uuids(PalObjects.get_nested(group, "key"), guild_id)
        ]

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
        self._character_save_parameter_map.append(new_pal.character_save)
        self._pals[new_pal.instance_id] = new_pal
        return new_pal

    def add_guild_pal(
        self,
        character_id: str,
        nickname: str,
        guild_id: UUID,
        base_id: UUID,
        storage_slot: Union[int | None] = None,
    ):
        guild = self._guilds.get(guild_id)
        if not guild:
            raise ValueError(f"Guild {guild_id} not found in the save file.")
        new_pal = guild.add_base_pal(character_id, nickname, base_id, storage_slot)
        if new_pal is None:
            return
        self._character_save_parameter_map.append(new_pal.character_save)
        self._pals[new_pal.instance_id] = new_pal
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
        self._character_save_parameter_map.append(new_pal.character_save)
        self._pals[new_pal.instance_id] = new_pal
        return new_pal

    def clone_guild_pal(
        self, guild_id: UUID, base_id: UUID, pal: PalDTO
    ) -> Optional[Pal]:
        guild = self._guilds.get(guild_id)
        if not guild:
            raise ValueError(f"Base {base_id} not found in the guild {guild_id}.")
        new_pal = guild.clone_base_pal(base_id, pal)
        if new_pal is None:
            return
        self._character_save_parameter_map.append(new_pal.character_save)
        self._pals[new_pal.instance_id] = new_pal
        return new_pal

    def delete_player_pals(self, player_id: UUID, pal_ids: List[UUID]) -> None:
        player = self._players.get(player_id)
        if not player:
            raise ValueError(f"Player {player_id} not found in the save file.")

        for pal_id in pal_ids:
            player.delete_pal(pal_id)
            self._delete_pal_by_id(pal_id)

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
        logger.info("Converting %s to JSON", self.name)
        return json.dumps(
            self._gvas_file.dump(),
            indent=None if minify else "\t",
            cls=CustomEncoder,
            allow_nan=allow_nan,
        )

    def get_pal(self, pal_id: UUID) -> Pal:
        return self._pals.get(pal_id)

    def get_pals(self):
        return self._pals

    def get_players(self):
        return self._players

    def get_player(self, player_id: UUID) -> Player:
        return self._players.get(player_id)

    def get_guild(self, guild_id: UUID) -> Guild:
        return self._guilds.get(guild_id)

    def get_guilds(self):
        return self._guilds

    def get_base(self, base_id: UUID) -> Base:
        for guild in self._guilds.values():
            base = guild.bases.get(base_id)
            if base:
                return base
        return None

    def get_character_container(self, container_id: UUID) -> Dict[str, Any]:
        for entry in self._character_container_save_data:
            if are_equal_uuids(PalObjects.get_guid(entry["key"]["ID"]), container_id):
                return entry
        return None

    def get_item_container(self, container_id: UUID) -> Dict[str, Any]:
        for entry in self._item_container_save_data:
            if are_equal_uuids(PalObjects.get_guid(entry["key"]["ID"]), container_id):
                return entry
        return None

    def load_json(self, data: bytes):
        logger.info("Loading %s as JSON", self.name)
        self._gvas_file = GvasFile.load(json.loads(data))
        return self

    def load_level_meta(self, data: bytes):
        logger.info("Loading %s as GVAS", self.name)
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
        logger.info("Loading %s as GVAS", self.name)
        raw_gvas, _ = decompress_sav_to_gvas(data)
        logger.debug("Reading GVAS file")
        gvas_file = GvasFile.read(
            raw_gvas, PALWORLD_TYPE_HINTS, CUSTOM_PROPERTIES, allow_nan=True
        )
        self._gvas_file = gvas_file
        self._get_file_size(data)
        return self

    def pal_count(self):
        return len(self._pals)

    def load_sav_files(
        self,
        level_sav: bytes,
        player_sav_files: Dict[str, bytes],
        level_meta: Optional[bytes] = None,
    ):
        logger.info("Loading %s", self.name)
        raw_gvas, _ = decompress_sav_to_gvas(level_sav)
        gvas_file = GvasFile.read(
            raw_gvas, PALWORLD_TYPE_HINTS, CUSTOM_PROPERTIES, allow_nan=True
        )
        self._gvas_file = gvas_file

        if level_meta:
            self.load_level_meta(level_meta)
            self._load_world_name()
        else:
            self.world_name = "No LevelMeta.sav found"

        self._get_file_size(level_sav)
        self._set_data()
        self._load_pals()
        self._load_guilds()
        self._load_players(player_sav_files)
        self._load_bases()
        return self

    def sav(self, gvas_file: GvasFile = None) -> bytes:
        logger.info("Converting %s to SAV", self.name)
        target_gvas = gvas_file if gvas_file else self._gvas_file
        if (
            "Pal.PalWorldSaveGame" in target_gvas.header.save_game_class_name
            or "Pal.PalLocalWorldSaveGame" in target_gvas.header.save_game_class_name
        ):
            save_type = 0x32
        else:
            save_type = 0x31
        gvas = copy.deepcopy(target_gvas)
        return compress_gvas_to_sav(gvas.write(CUSTOM_PROPERTIES), save_type)

    def player_savs(self) -> Dict[UUID, bytes]:
        logger.info("Converting player save files to SAV", len(self._player_gvas_files))
        return {
            uid: compress_gvas_to_sav(
                self._player_gvas_files[uid].write(CUSTOM_PROPERTIES), 0x32
            )
            for uid in self._player_gvas_files
        }

    def to_json_file(
        self,
        output_path,
        minify=False,
        allow_nan=True,
    ):
        logger.info("Converting %s to JSON, saving to %s", self.name, output_path)
        with open(output_path, "w", encoding="utf8") as f:
            indent = None if minify else "\t"
            json.dump(
                self._gvas_file.dump(),
                f,
                indent=indent,
                cls=CustomEncoder,
                allow_nan=allow_nan,
            )

    def to_sav_file(self, output_path):
        logger.info("Converting %s to SAV, saving to %s", self.name, output_path)
        if (
            "Pal.PalWorldSaveGame" in self._gvas_file.header.save_game_class_name
            or "Pal.PalLocalWorldSaveGame"
            in self._gvas_file.header.save_game_class_name
        ):
            save_type = 0x32
        else:

            save_type = 0x31

        logger.info("Compressing GVAS to SAV with save type %s", save_type)
        gvas = copy.deepcopy(self._gvas_file)
        sav_file = compress_gvas_to_sav(gvas.write(CUSTOM_PROPERTIES), save_type)
        with open(output_path, "wb") as f:
            f.write(sav_file)

    def to_player_sav_files(self, output_path):
        logger.info("Converting player save files to SAV, saving to %s", output_path)
        for uid, gvas in self._player_gvas_files.items():
            sav_file = compress_gvas_to_sav(gvas.write(CUSTOM_PROPERTIES), 0x32)
            uid = str(uid).replace("-", "")
            with open(os.path.join(output_path, f"{uid}.sav"), "wb") as f:
                f.write(sav_file)

    async def update_pals(self, modified_pals: Dict[UUID, PalDTO], ws_callback) -> None:
        if not self._gvas_file:
            raise ValueError("No GvasFile has been loaded.")

        for pal_id, pal in modified_pals.items():
            pal_name = pal.nickname if pal.nickname else pal.character_id
            await ws_callback(f"Updating pal {pal_name}")
            self._update_pal(pal_id, pal)

        logger.info("Updated %d pals in the save file.", len(modified_pals))

        await ws_callback("Saving changes to file")

    async def update_players(
        self, modified_players: Dict[UUID, Player], ws_callback
    ) -> None:
        if not self._gvas_file:
            raise ValueError("No GvasFile has been loaded.")

        for _, player in modified_players.items():
            await ws_callback(f"Updating player {player.nickname}")
            self._update_player(player)

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
            await ws_callback(f"Updating guild {id}")
            guild = self._guilds.get(id)
            guild.update_from(dto)

        logger.info("Updated %d bases in the save file.", len(modified_guilds))

    def _delete_pal_by_id(self, pal_id: UUID) -> None:
        del self._pals[pal_id]
        for entry in self._character_save_parameter_map:
            if are_equal_uuids(PalObjects.get_guid(entry["key"]["InstanceId"]), pal_id):
                logger.debug("Deleting pal %s from CharacterSaveParameterMap", pal_id)
                self._character_save_parameter_map.remove(entry)
                break

    def _get_file_size(self, data: bytes):
        if hasattr(data, "seek") and hasattr(data, "tell"):
            data.seek(0, os.SEEK_END)
            self.size = data.tell()
            data.seek(0)
        else:
            self.size = data.__sizeof__()

    def _get_player_pals(self, uid):
        logger.info("Loading Pals for player %s", uid)
        pals = {}
        pals = {
            k: v for k, v in self._pals.items() if are_equal_uuids(v.owner_uid, uid)
        }
        return pals

    def _get_player_save_data(self, player_gvas: Dict[str, Any]):
        player_save_data = PalObjects.get_value(player_gvas.properties["SaveData"])
        return player_save_data

    def _is_player(self, entry):
        save_parameter_path = PalObjects.get_nested(
            entry, "value", "RawData", "value", "object", "SaveParameter", "value"
        )
        return (
            PalObjects.get_value(save_parameter_path["IsPlayer"])
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
            guild_extra_save_data = next(
                (
                    g
                    for g in self._guild_extra_save_data_map
                    if are_equal_uuids(g["key"], guild_id)
                ),
                None,
            )
            self._guilds[guild_id] = Guild(
                group_save_data=entry,
                guild_extra_data=guild_extra_save_data,
                item_container_save_data=self._item_container_save_data,
                dynamic_item_save_data=self._dynamic_item_save_data,
            )

    def _load_bases(self):
        if not self._base_camp_save_data_map:
            logger.warning("No bases found in the save file.")
            return

        for entry in self._base_camp_save_data_map:
            # Guild to add to
            group_id_belong_to = PalObjects.as_uuid(
                PalObjects.get_nested(
                    entry, "value", "RawData", "value", "group_id_belong_to"
                )
            )
            # Pal Container ID
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

            # Find all pals that have that container ID
            pals = {
                pal.instance_id: pal
                for pal in self._pals.values()
                if pal.storage_id == container_id
            }

            base = Base(
                data=entry,
                pals=pals,
                container_id=container_id,
                slot_count=container_slot_count,
                character_container_save_data=self._character_container_save_data,
                map_object_save_data=self._map_object_save_data,
                item_container_save_data=self._item_container_save_data,
                dynamic_item_save_data=self._dynamic_item_save_data,
            )
            self._guilds[group_id_belong_to].add_base(base)

            # Debug, print the guild name, and pals at base
            logger.debug(
                "Guild %s has %d pals at base",
                self._guilds[group_id_belong_to].name,
                len(pals),
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

    def _load_world_name(self):
        world_name = PalObjects.get_nested(
            self._level_meta_gvas_file.properties,
            "SaveData",
            "value",
            "WorldName",
            "value",
        )
        self.world_name = world_name if world_name else "Unknown"

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

    def _load_players(self, player_sav_files: Dict[UUID, bytes] = None):
        if not self._character_save_parameter_map:
            return {}
        logger.info("Loading Players")

        loaded_sav_files: Dict[UUID, GvasFile] = {}
        # This is a temp fix, need to look into fixing player uid
        # mismatches due to host fix
        host_fix_players = {}

        for uid, player_sav_bytes in player_sav_files.items():
            raw_gvas, _ = decompress_sav_to_gvas(player_sav_bytes)
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
                host_fix_players[player_uuid] = uid
            loaded_sav_files[player_uuid] = gvas_file

        players = {}
        for entry in self._character_save_parameter_map:
            if self._is_player(entry):
                uid = PalObjects.get_guid(entry["key"]["PlayerUId"])
                if uid not in loaded_sav_files:
                    logger.warning("No player save file found for player %s", uid)
                    continue

                self._player_gvas_files[uid] = loaded_sav_files[uid]
                player_pals = self._get_player_pals(uid)
                if uid in host_fix_players:
                    player_pals = player_pals | self._get_player_pals(
                        host_fix_players[uid]
                    )
                player = Player(
                    gvas_file=self._player_gvas_files[uid],
                    item_container_save_data=self._item_container_save_data,
                    dynamic_item_save_data=self._dynamic_item_save_data,
                    character_container_save_data=self._character_container_save_data,
                    character_save_parameter=entry,
                    pals=player_pals,
                    guild=self._player_guild(uid),
                )
                players[uid] = player

        self._players = players

    def _update_pal(self, pal_id: UUID, updated_pal: PalDTO) -> None:
        existing_pal = self._pals[pal_id]
        existing_pal.update_from(updated_pal)

    def _update_player(self, player: PlayerDTO) -> None:
        existing_player = self._players.get(player.uid)
        existing_player.update_from(player)
