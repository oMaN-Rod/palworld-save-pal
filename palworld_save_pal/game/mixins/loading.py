import time
from typing import TYPE_CHECKING, Any, Dict, List, Optional
from uuid import UUID

if TYPE_CHECKING:
    from palworld_save_pal.game.mixins._save_manager_protocol import (
        SaveManagerProtocol,
    )

    _Base = SaveManagerProtocol
else:
    _Base = object

from palworld_save_tools.gvas import GvasFile
from palworld_save_tools.palsav import decompress_sav_to_gvas
from palworld_save_tools.paltypes import PALWORLD_TYPE_HINTS

from palworld_save_pal.game.base import Base
from palworld_save_pal.game.guild import Guild
from palworld_save_pal.game.gvas_codec import CUSTOM_PROPERTIES
from palworld_save_pal.game.pal import Pal
from palworld_save_pal.game.pal_objects import PalObjects
from palworld_save_pal.game.enum import GroupType
from palworld_save_pal.utils.logging_config import create_logger
from palworld_save_pal.utils.perf import gc_paused
from palworld_save_pal.game.player import Player, PlayerGvasFiles
from palworld_save_pal.utils.uuid import are_equal_uuids, is_empty_uuid

logger = create_logger(__name__)


class LoadingMixin(_Base):
    def is_player_loaded(self, player_id: UUID) -> bool:
        return player_id in self._loaded_players

    def is_guild_loaded(self, guild_id: UUID) -> bool:
        return guild_id in self._loaded_guilds

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
        with gc_paused():
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
            with gc_paused():
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
            with gc_paused():
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
                with gc_paused():
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
                    nickname = f"\U0001f977 ({str(uid).split('-')[0]})"

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