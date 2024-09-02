import copy
import json
import os
from typing import Any, Dict, List, Optional
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

from palworld_save_pal.save_file.guild import Guild
from palworld_save_pal.save_file.pal import Pal
from palworld_save_pal.save_file.pal_objects import GroupType, PalObjects
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.save_file.player import Player
from palworld_save_pal.save_file.utils import (
    are_equal_uuids,
)

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
        raise Exception(
            f"Expected ArrayProperty or MapProperty or StructProperty, got {type_name} in {path}"
        )
    return value


def skip_encode(writer: FArchiveWriter, property_type: str, properties: dict) -> int:
    if "skip_type" not in properties:
        if properties["custom_type"] in PALWORLD_CUSTOM_PROPERTIES is not None:
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
        raise Exception(
            f"Expected ArrayProperty or MapProperty or StructProperty, got {property_type}"
        )


CUSTOM_PROPERTIES = {
    k: v for k, v in PALWORLD_CUSTOM_PROPERTIES.items() if k not in DISABLED_PROPERTIES
}
CUSTOM_PROPERTIES[".worldSaveData.MapObjectSaveData"] = (skip_decode, skip_encode)
CUSTOM_PROPERTIES[".worldSaveData.FoliageGridSaveDataMap"] = (skip_decode, skip_encode)
CUSTOM_PROPERTIES[".worldSaveData.MapObjectSpawnerInStageSaveData"] = (
    skip_decode,
    skip_encode,
)
CUSTOM_PROPERTIES[".worldSaveData.WorkSaveData"] = (skip_decode, skip_encode)
CUSTOM_PROPERTIES[".worldSaveData.DungeonSaveData"] = (skip_decode, skip_encode)
CUSTOM_PROPERTIES[".worldSaveData.EnemyCampSaveData"] = (skip_decode, skip_encode)
CUSTOM_PROPERTIES[".worldSaveData.CharacterParameterStorageSaveData"] = (
    skip_decode,
    skip_encode,
)
CUSTOM_PROPERTIES[".worldSaveData.InvaderSaveData"] = (skip_decode, skip_encode)
CUSTOM_PROPERTIES[".worldSaveData.DungeonPointMarkerSaveData"] = (
    skip_decode,
    skip_encode,
)
CUSTOM_PROPERTIES[".worldSaveData.GameTimeSaveData"] = (skip_decode, skip_encode)
CUSTOM_PROPERTIES[".worldSaveData.OilrigSaveData"] = (skip_decode, skip_encode)
CUSTOM_PROPERTIES[".worldSaveData.SupplySaveData"] = (skip_decode, skip_encode)


class SaveFile(BaseModel):
    name: str = ""
    size: int = 0

    model_config = ConfigDict(arbitrary_types_allowed=True)

    _players: Dict[UUID, Player] = PrivateAttr(default_factory=dict)
    _pals: Dict[UUID, Pal] = PrivateAttr(default_factory=dict)
    _guilds: Dict[UUID, Guild] = PrivateAttr(default_factory=dict)

    _gvas_file: Optional[GvasFile] = PrivateAttr(default=None)
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

    def add_pal(
        self, player_id: UUID, pal_code_name: str, nickname: str
    ) -> Optional[Pal]:
        player = self._players.get(player_id)
        if not player:
            logger.error("Player %s not found in the save file.", player_id)
            return

        data = player.add_pal(pal_code_name, nickname)
        if data is None:
            return
        new_pal, new_pal_data = data
        self._character_save_parameter_map.append(new_pal_data)
        self._pals[new_pal.instance_id] = new_pal
        return new_pal

    def clone_pal(self, pal: Pal) -> Optional[Pal]:
        player = self._players.get(pal.owner_uid)
        if not player:
            logger.error("Player %s not found in the save file.", pal.owner_uid)
            return

        new_pal = player.clone_pal(pal)
        if new_pal is None:
            return
        self._character_save_parameter_map.append(new_pal.character_save())
        self._pals[new_pal.instance_id] = new_pal
        return new_pal

    def delete_pals(self, player_id: UUID, pal_ids: List[UUID]) -> None:
        player = self._players.get(player_id)
        if not player:
            logger.error("Player %s not found in the save file.", player_id)
            return

        for pal_id in pal_ids:
            player.delete_pal(pal_id)

    def get_json(self, minify=False, allow_nan=True):
        logger.info("Converting %s to JSON", self.name)
        return json.dumps(
            self._gvas_file.dump(),
            indent=None if minify else "\t",
            cls=CustomEncoder,
            allow_nan=allow_nan,
        )

    def get_pals(self):
        return self._pals

    def get_players(self):
        return self._players

    def load_json(self, data: bytes):
        logger.info("Loading %s as JSON", self.name)
        self._gvas_file = GvasFile.load(json.loads(data))
        return self

    def load_level_sav(self, data: bytes):
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
        self._gvas_file = gvas_file
        self._get_file_size(data)
        return self

    def pal_count(self):
        return len(self._pals)

    def load_sav_files(self, level_sav: bytes, player_sav_files: Dict[str, bytes]):
        logger.info("Loading %s as SAV", self.name)
        raw_gvas, _ = decompress_sav_to_gvas(level_sav)
        gvas_file = GvasFile.read(
            raw_gvas, PALWORLD_TYPE_HINTS, CUSTOM_PROPERTIES, allow_nan=True
        )
        self._gvas_file = gvas_file
        self._get_file_size(level_sav)
        self._set_data()
        self._load_pals()
        self._load_guilds()
        self._load_players(player_sav_files)
        self._load_guilds()
        return self

    def pal_count(self):
        return len(self._pals)

    def sav(self):
        logger.info("Converting %s to SAV", self.name)
        if (
            "Pal.PalWorldSaveGame" in self._gvas_file.header.save_game_class_name
            or "Pal.PalLocalWorldSaveGame"
            in self._gvas_file.header.save_game_class_name
        ):
            save_type = 0x32
        else:
            save_type = 0x31
        gvas = copy.deepcopy(self._gvas_file)
        return compress_gvas_to_sav(gvas.write(CUSTOM_PROPERTIES), save_type)

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

        sav_file = compress_gvas_to_sav(
            self._gvas_file.write(CUSTOM_PROPERTIES), save_type
        )
        with open(output_path, "wb") as f:
            f.write(sav_file)

    async def update_pals(self, modified_pals: Dict[UUID, Pal], ws_callback) -> None:
        if not self._gvas_file:
            raise ValueError("No GvasFile has been loaded.")

        for pal_id, pal in modified_pals.items():
            await ws_callback(f"Updating pal {pal.nickname}")
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
            self._guilds[guild_id] = Guild(
                id=guild_id,
                group_save_data=entry,
            )

    def _load_pals(self):
        if not self._gvas_file:
            raise ValueError("No GvasFile has been loaded.")
        self._pals = {}
        logger.info("Loading Pals")
        for e in self._character_save_parameter_map:
            if self._is_player(e):
                continue
            instance = Pal(e)
            if instance:
                self._pals[instance.instance_id] = instance
            else:
                logger.warning("Failed to create PalEntity summary")

    def _set_data(self) -> None:
        world_save_data = PalObjects.get_value(
            self._gvas_file.properties["worldSaveData"]
        )
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

        def extract_player_info(entry):
            uid = PalObjects.get_guid(entry["key"]["PlayerUId"])
            save_parameter = PalObjects.get_nested(
                entry,
                "value",
                "RawData",
                "value",
                "object",
                "SaveParameter",
                "value",
            )
            nickname = PalObjects.get_value(save_parameter["NickName"])
            level = (
                PalObjects.get_value(save_parameter["Level"])
                if "Level" in save_parameter
                else 1
            )
            player_sav_bytes = player_sav_files.get(uid)
            if not player_sav_bytes:
                logger.warning("No player save file found for player %s", uid)
                return
            raw_gvas, _ = decompress_sav_to_gvas(player_sav_bytes)
            gvas_file = GvasFile.read(
                raw_gvas, PALWORLD_TYPE_HINTS, CUSTOM_PROPERTIES, allow_nan=True
            )
            self._player_gvas_files[uid] = gvas_file
            player = Player(
                uid=uid,
                nickname=nickname,
                level=level,
                gvas_file=gvas_file,
                item_container_save_data=self._item_container_save_data,
                dynamic_item_save_data=self._dynamic_item_save_data,
                character_container_save_data=self._character_container_save_data,
                guild=self._player_guild(uid),
            )
            logger.info("Loaded player %s", player.model_dump())
            player.pals = self._get_player_pals(uid)
            return player

        players = {
            x.uid: x
            for x in [
                extract_player_info(entry)
                for entry in self._character_save_parameter_map
                if self._is_player(entry)
            ]
        }
        self._players = players

    def _update_pal(self, pal_id: UUID, updated_pal: Pal) -> None:
        existing_pal = self._pals[pal_id]
        existing_pal.update_from(updated_pal)

    def _update_player(self, player: Player) -> None:
        existing_player = self._players.get(player.uid)
        existing_player.update_from(player)
