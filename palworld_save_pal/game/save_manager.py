import os
import time
from typing import Any, Dict, List, Optional
from uuid import UUID

from pydantic import BaseModel, ConfigDict, PrivateAttr

from palworld_save_tools.gvas import GvasFile
from palworld_save_tools.palsav import compress_gvas_to_sav, decompress_sav_to_gvas
from palworld_save_tools.paltypes import PALWORLD_TYPE_HINTS

from palworld_save_pal.game.gvas_codec import CUSTOM_PROPERTIES
from palworld_save_pal.game.guild import Guild
from palworld_save_pal.game.pal import Pal
from palworld_save_pal.game.pal_objects import PalObjects
from palworld_save_pal.game.player import Player, PlayerGvasFiles
from palworld_save_pal.utils.indexed_collection import IndexedCollection
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.perf import gc_paused
from palworld_save_pal.utils.uuid import are_equal_uuids

from palworld_save_pal.game.mixins.indexing import IndexingMixin
from palworld_save_pal.game.mixins.summaries import SummariesMixin
from palworld_save_pal.game.mixins.loading import LoadingMixin
from palworld_save_pal.game.mixins.serialization import SerializationMixin
from palworld_save_pal.game.mixins.pal_ops import PalOpsMixin
from palworld_save_pal.game.mixins.player_ops import PlayerOpsMixin
from palworld_save_pal.game.mixins.guild_ops import GuildOpsMixin
from palworld_save_pal.game.mixins.player_swap import PlayerSwapMixin

logger = create_logger(__name__)


class SaveManager(
    IndexingMixin,
    SummariesMixin,
    LoadingMixin,
    SerializationMixin,
    PalOpsMixin,
    PlayerOpsMixin,
    GuildOpsMixin,
    PlayerSwapMixin,
    BaseModel,
):
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

    _player_summaries: Dict[UUID, Any] = PrivateAttr(default_factory=dict)
    _guild_summaries: Dict[UUID, Any] = PrivateAttr(default_factory=dict)
    _loaded_players: set = PrivateAttr(default_factory=set)
    _loaded_guilds: set = PrivateAttr(default_factory=set)

    _player_file_refs: Dict[UUID, Dict[str, Any]] = PrivateAttr(default_factory=dict)
    _player_gvas_sav_cache: Dict[UUID, GvasFile] = PrivateAttr(default_factory=dict)

    _pal_owner_counts_cache: Optional[Dict[UUID, int]] = PrivateAttr(default=None)
    _player_guild_map_cache: Optional[Dict[UUID, UUID]] = PrivateAttr(default=None)
    _map_object_index: Optional[Dict[UUID, List[Dict[str, Any]]]] = PrivateAttr(
        default=None
    )

    def get_character_container(self, container_id: UUID) -> Dict[str, Any]:
        return self._get_character_containers().get(container_id)

    def get_item_container(self, container_id: UUID) -> Dict[str, Any]:
        return self._get_item_containers().get(container_id)

    async def load_sav_files(
        self,
        level_sav: bytes,
        player_file_refs: Dict[UUID, Dict[str, Any]],
        level_meta: Optional[bytes] = None,
        ws_callback=None,
    ) -> "SaveManager":
        logger.info("Loading %s (minimal mode)", self.level_sav_path)
        start_time = time.perf_counter()

        raw_gvas, _ = decompress_sav_to_gvas(level_sav)
        with gc_paused():
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